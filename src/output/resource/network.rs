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

//! Output resources of type network.

use std::io::Write;
use std::net::*;
use crate::coalyxe;
use crate::errorhandling::*;
use crate::net::*;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;
use crate::net::buffer::SendBuffer;
#[cfg(unix)]
use std::os::unix::net::UnixStream;


/// Specific data for physical resources of kind network interface.
/// Can be used to communicate both with a trace server and syslog service.
pub struct NetworkData {
    // communication data buffer
    send_buffer: SendBuffer,
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
impl NetworkData {
    /// Creates specific structure to communicate over network.
    ///
    /// # Arguments
    /// * `peer_addr` - network protocol and address of communication partner
    pub fn new(remote_addr: PeerAddr) -> NetworkData {
        let send_buffer = SendBuffer::new(PROTOCOL_VERSION as u32, 1024);
        NetworkData {
            send_buffer,
            remote_addr,
            tcp_stream: None,
            udp_socket: None,
            #[cfg(unix)]
            unix_stream: None
        }
    }

    /// Creates suitable communication socket and connects to a trace server.
    ///
    /// # Arguments
    /// * `local_addr` - the optional socket address for the local network socket
    /// * `orig_info` - information about process and local host
    pub fn connect(&mut self,
                   local_addr: Option<PeerAddr>,
                   orig_info: &OriginatorInfo) -> Result<(), CoalyException> {
        match &self.remote_addr {
            PeerAddr::IpSocket(prot, ip_addr) => {
                if *prot == NetworkProtocol::Tcp {
                    if self.tcp_stream.is_some() {
                        return Err(coalyxe!(E_ALREADY_CONNECTED, self.remote_addr.to_string()))
                    }
                    self.tcp_stream = Some(NetworkData::connect_tcp(&ip_addr, orig_info,
                                                                    &mut self.send_buffer)?);
                } else {
                    if self.udp_socket.is_some() {
                        return Err(coalyxe!(E_ALREADY_CONNECTED, self.remote_addr.to_string()))
                    }
                    self.udp_socket = Some(NetworkData::connect_udp(&ip_addr, local_addr,
                                                                    orig_info,
                                                                    &mut self.send_buffer)?);
                }
            },
            #[cfg(unix)]
            PeerAddr::UnixSocket(path) => {
                if self.unix_stream.is_some() {
                    return Err(coalyxe!(E_ALREADY_CONNECTED, self.remote_addr.to_string()))
                }
                self.unix_stream = Some(NetworkData::connect_unix(&path, orig_info,
                                                                  &mut self.send_buffer)?);
            }
        }
        Ok(())
    }

    /// Connects the client's network resource to a trace server using TCP.
    ///
    /// # Arguments
    /// * `remote_addr` - the socket address of remote Coaly server
    /// * `orig_info` - information about process and local host
    /// * `send_buffer` - buffer to use for sending messages to the server
    fn connect_tcp(remote_addr: &SocketAddr,
                   orig_info: &OriginatorInfo,
                   send_buffer: &mut SendBuffer) -> Result<TcpStream, CoalyException> {
        match TcpStream::connect(remote_addr) {
            Ok(mut s) => {
                // send connect request to server
                send_buffer.store_client_notification(orig_info);
                if let Err(e) = s.write(send_buffer.as_slice()) {
                    let local_addr = match s.local_addr() {
                        Ok(a) => a.to_string(),
                        _ => String::from("?")
                    };
                    let _ = s.shutdown(Shutdown::Both);
                    return Err(coalyxe!(E_SOCKET_WRITE_ERR, local_addr,
                                      remote_addr.to_string(), e.to_string()))
                }
                Ok(s)
            },
            Err(m) =>  Err(coalyxe!(E_SOCKET_CRE_ERR, remote_addr.to_string(), m.to_string()))
        }
    }

    /// Connects the client's network resource to a trace server using UDP.
    ///
    /// # Arguments
    /// * `remote_addr` - the socket address of remote Coaly server
    /// * `local_addr` - the optional socket address for the local network socket
    /// * `orig_info` - information about process and local host
    /// * `send_buffer` - buffer to use for sending messages to the server
    fn connect_udp(remote_addr: &SocketAddr,
                   local_addr: Option<PeerAddr>,
                   orig_info: &OriginatorInfo,
                   send_buffer: &mut SendBuffer) -> Result<UdpSocket, CoalyException> {
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
        match UdpSocket::bind(laddr) {
            Ok(s) => {
                match s.connect(remote_addr) {
                    Ok(_) => {
                        // send connect request to server
                        send_buffer.store_client_notification(orig_info);
                        if let Err(e) = s.send(send_buffer.as_slice()) {
                            return Err(coalyxe!(E_SOCKET_WRITE_ERR, laddr.to_string(),
                                              remote_addr.to_string(), e.to_string()))
                        }
                        Ok(s)
                    },
                    Err(m) => Err(coalyxe!(E_SOCKET_CRE_ERR, remote_addr.to_string(), m.to_string()))
                }
            },
            Err(m) => Err(coalyxe!(E_SOCKET_CRE_ERR, laddr.to_string(), m.to_string()))
        }
    }

    /// Connects the client's network resource to a trace server using Unix socket.
    ///
    /// # Arguments
    /// * `remote_addr` - the path of the Unix socket of remote Coaly server
    #[cfg(unix)]
    fn connect_unix(remote_addr: &str,
                    orig_info: &OriginatorInfo,
                    send_buffer: &mut SendBuffer) -> Result<UnixStream, CoalyException> {
        match UnixStream::connect(remote_addr) {
            Ok(mut s) => {
                // send connect request to server
                send_buffer.store_client_notification(orig_info);
                if let Err(e) = s.write(send_buffer.as_slice()) {
                    let _ = s.shutdown(Shutdown::Both);
                    return Err(coalyxe!(E_SOCKET_WRITE_ERR, String::from("Unix socket"),
                                      remote_addr.to_string(), e.to_string()))
                }
                Ok(s)
            },
            Err(m) =>  Err(coalyxe!(E_SOCKET_CRE_ERR, remote_addr.to_string(), m.to_string()))
        }
    }

    /// Sends a log or trace record to a remote application.
    /// 
    /// # Arguments
    /// * `rec` - the log or trace record
    /// 
    /// # Errors
    /// Returns an error structure if the send operation fails
    pub fn send_record(&mut self, rec: &dyn RecordData) -> Result<(), Vec<CoalyException>> {
        self.send_buffer.store_record_notification(rec);
        if let Some(s) = self.tcp_stream.as_mut() {
            if let Err(e) = s.write(self.send_buffer.as_slice()) {
                let local_addr = match s.local_addr() {
                    Ok(a) => a.to_string(),
                    _ => String::from("?")
                };
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, local_addr.to_string(),
                                       self.remote_addr.to_string(), e.to_string())))
            }
        }
        if let Some(s) = self.udp_socket.as_mut() {
            if let Err(e) = s.send(self.send_buffer.as_slice()) {
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
            if let Err(e) = s.write(self.send_buffer.as_slice()) {
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, String::from(""),
                                       self.remote_addr.to_string(), e.to_string())))
            }
        }
        Ok(())
    }

    /// Writes the given slice to the network socket.
    ///
    /// # Arguments
    /// * `data` - the data to write
    /// 
    /// # Errors
    /// Returns an error structure if the write operation fails
    pub fn write(&mut self, data: &[u8]) -> Result<(), Vec<CoalyException>> {
        if let Some(s) = self.tcp_stream.as_mut() {
            if let Err(m) = s.write(data) {
                let local_addr = match s.local_addr() {
                    Ok(a) => a.to_string(),
                    _ => String::from("?")
                };
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, local_addr.to_string(),
                                       self.remote_addr.to_string(), m.to_string())))
            }
        }
        if let Some(s) = self.udp_socket.as_ref() {
            if let Err(m) = s.send(data) {
                let local_addr = match s.local_addr() {
                    Ok(a) => a.to_string(),
                    _ => String::from("?")
                };
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, local_addr.to_string(),
                                       self.remote_addr.to_string(), m.to_string())))
            }
        }
        #[cfg(unix)]
        if let Some(s) = self.unix_stream.as_mut() {
            if let Err(e) = s.write(data) {
                return Err(vec!(coalyxe!(E_SOCKET_WRITE_ERR, String::from(""),
                                       self.remote_addr.to_string(), e.to_string())))
            }
        }
        Ok(())
    }    

    /// Disconnects the network interface from the server.
    pub fn disconnect(&mut self) {
        self.send_buffer.store_disconnect_notification();
        if let Some(s) = self.tcp_stream.as_mut() {
            let _ = s.write(self.send_buffer.as_slice());
            let _ = s.shutdown(Shutdown::Both);
            self.tcp_stream = None;
        }
        if let Some(s) = self.udp_socket.as_mut() {
            let _ = s.send(self.send_buffer.as_slice());
            self.udp_socket = None;
        }
        if let Some(s) = self.unix_stream.as_mut() {
            let _ = s.write(self.send_buffer.as_slice());
            self.unix_stream = None;
        }
    }

//    /// Closes the network interface.
//    pub fn close(&mut self) {
//        self.tcp_stream = None;
//        self.udp_socket = None;
//        self.unix_stream = None;
//    }
}
