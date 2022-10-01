// -----------------------------------------------------------------------------------------------
// Coaly - context aware logging and tracing system
//
// Copyright (c) 2022, Frank Sommer.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// * Neither the name of the copyright holder nor the names of its
//   contributors may be used to endorse or promote products derived from
//   this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
// -----------------------------------------------------------------------------------------------

//! Output resources of type syslog.

use std::io::Write;
use std::net::*;
use crate::coalyxe;
use crate::errorhandling::*;
use crate::net::*;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;
#[cfg(unix)]
use std::os::unix::net::UnixStream;


/// Specific data for physical resources of kind syslog.
pub struct SyslogData {
    // syslog facility
    facility: u32,
    // buffer for serialized messages
    buffer: Vec<u8>,
    // buffer with constant header data
    fix_header: Vec<u8>,
    // remote address
    remote_addr: PeerAddr,
    // TCP communication stream
    tcp_stream: Option<TcpStream>,
    // UDP communication socket
    udp_socket: Option<UdpSocket>,
    // Unix communication stream
    #[cfg(unix)]
    unix_stream: Option<UnixStream>
}
impl SyslogData {
    /// Creates specific structure to communicate to syslog service.
    ///
    /// # Arguments
    /// * `remote_addr` - network protocol and address of syslog service
    /// * `facility` - client's facility in syslog terms
    /// * `orig_info` - local info with host name, application name and process ID
    pub fn new(remote_addr: PeerAddr,
               facility: u32,
               orig_info: &OriginatorInfo) -> SyslogData {
        let buffer = Vec::<u8>::with_capacity(1024);
        let app_name = orig_info.application_name();
        let process_id = orig_info.process_id();
        let h_size: usize = 4 + process_id.len() + app_name.len();
        let mut fix_header = Vec::<u8>::with_capacity(h_size);
        if ! app_name.is_empty() || app_name.is_ascii() {
            fix_header.extend_from_slice(app_name.as_bytes());
        }
        fix_header.push(L_BRACKET);
        fix_header.extend_from_slice(process_id.as_bytes());
        fix_header.push(R_BRACKET);
        fix_header.push(COLON);
        fix_header.push(SPACE);
        SyslogData {
            facility: facility << 3,
            buffer,
            fix_header,
            remote_addr,
            tcp_stream: None,
            udp_socket: None,
            #[cfg(unix)]
            unix_stream: None
        }
    }

    /// Creates suitable communication socket and connects to syslog service.
    ///
    /// # Arguments
    /// * `local_addr` - the optional socket address for the local network socket
    #[cfg(unix)]
    pub fn connect(&mut self, local_addr: Option<PeerAddr>) -> Result<(), CoalyException> {
        match &self.remote_addr {
            PeerAddr::IpSocket(prot, ip_addr) => {
                if *prot == NetworkProtocol::Tcp {
                    if self.tcp_stream.is_some() {
                        return Err(coalyxe!(E_ALREADY_CONNECTED, self.remote_addr.to_string()))
                    }
                    self.tcp_stream = Some(SyslogData::open_tcp(&ip_addr)?);
                } else {
                    if self.udp_socket.is_some() {
                        return Err(coalyxe!(E_ALREADY_CONNECTED, self.remote_addr.to_string()))
                    }
                    self.udp_socket = Some(SyslogData::open_udp(&ip_addr, local_addr)?);
                }
            },
            #[cfg(unix)]
            PeerAddr::UnixSocket(path) => {
                if self.unix_stream.is_some() {
                    return Err(coalyxe!(E_ALREADY_CONNECTED, self.remote_addr.to_string()))
                }
                self.unix_stream = Some(SyslogData::open_unix(&path)?);
            }
        }
        Ok(())
    }

    /// Opens a TCP socket to syslog service.
    ///
    /// # Arguments
    /// * `remote_addr` - the the socket address of syslog service
    fn open_tcp(remote_addr: &SocketAddr) -> Result<TcpStream, CoalyException> {
        TcpStream::connect(remote_addr).map_err(|e| coalyxe!(E_SOCKET_CRE_ERR,
                                                           remote_addr.to_string(),
                                                           e.to_string()))
    }

    /// Opens a UDP socket to syslog service.
    ///
    /// # Arguments
    /// * `remote_addr` - the the socket address of syslog service
    fn open_udp(remote_addr: &SocketAddr,
                local_addr: Option<PeerAddr>) -> Result<UdpSocket, CoalyException> {
        let mut laddr: Option<SocketAddr> = None;
        if let Some(l) = local_addr {
            if let Some(a) = l.ip_addr() { laddr = Some(*a); }
        }
        if laddr.is_none() {
            laddr = if remote_addr.is_ipv4() {
                        Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
                    } else {
                        Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0))
                    };
        };
        let laddr = laddr.unwrap();
        let s = UdpSocket::bind(laddr).map_err(|e| coalyxe!(E_SOCKET_CRE_ERR,
                                                          remote_addr.to_string(),
                                                          e.to_string()))?;
        if let Err(e) = UdpSocket::bind(laddr) {
            return Err(coalyxe!(E_SOCKET_CRE_ERR, laddr.to_string(), e.to_string()))
        }
        Ok(s)
    }

    /// Opens a Unix socket to syslog service.
    ///
    /// # Arguments
    /// * `remote_addr` - the path of the Unix socket of syslog service
    #[cfg(unix)]
    fn open_unix(remote_addr: &str) -> Result<UnixStream, CoalyException> {
        UnixStream::connect(remote_addr).map_err(|e| coalyxe!(E_SOCKET_CRE_ERR,
                                                            remote_addr.to_string(),
                                                            e.to_string()))
    }

    /// Sends a log or trace record to a remote application.
    /// 
    /// # Arguments
    /// * `rec` - the log or trace record
    /// 
    /// # Errors
    /// Returns an error structure if the send operation fails
    pub fn send_record(&mut self, rec: &dyn RecordData) -> Result<(), Vec<CoalyException>> {
        let lvl = std::cmp::max(rec.level() as u32, 7);
        let pri_n_ver = format!("<{}>", self.facility + lvl);
        self.buffer.clear();
        self.buffer.extend_from_slice(pri_n_ver.as_bytes());
        self.buffer.extend_from_slice(self.fix_header.as_slice());
        let rec_msg = rec.message();
        if let Some(ref msg) = rec_msg { self.buffer.extend_from_slice(msg.as_bytes()); }
        if let Some(s) = self.tcp_stream.as_mut() {
            if let Err(e) = s.write(self.buffer.as_slice()) {
                let local_addr = match s.local_addr() {
                    Ok(a) => a.to_string(),
                    _ => String::from("?")
                };
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, local_addr.to_string(),
                                       self.remote_addr.to_string(), e.to_string())))
            }
        }
        if let Some(s) = self.udp_socket.as_mut() {
            if let Err(e) = s.send(self.buffer.as_slice()) {
                let local_addr = match s.local_addr() {
                    Ok(a) => a.to_string(),
                    _ => String::from("?")
                };
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, local_addr.to_string(),
                                       self.remote_addr.to_string(), e.to_string())))
            }
        }
        #[cfg(unix)]
        if let Some(s) = self.unix_stream.as_mut() {
            if let Err(e) = s.write(self.buffer.as_slice()) {
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, String::from(""),
                                       self.remote_addr.to_string(), e.to_string())))
            }
        }
        Ok(())
    }

    /// Closes the connection to syslog service.
    pub fn close(&mut self) {
        self.tcp_stream = None;
        self.udp_socket = None;
        self.unix_stream = None;
    }
}

const SPACE: u8 = 32;
const COLON: u8 = 58;
const L_BRACKET: u8 = 91;
const R_BRACKET: u8 = 93;
