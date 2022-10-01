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

//! Functionality needed by network resources and log/trace server.

use regex::Regex;
use std::fmt::{Debug, Display, Formatter};
use std::net::*;
use std::path::Path;
use std::str::FromStr;
use crate::{coalyxe, coalyxw};
use crate::errorhandling::*;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RemoteRecordData;
use serializable::Serializable;

pub mod buffer;
pub mod serializable;
pub mod server;
pub mod serverproperties;
mod clientconnection;
mod clientwhitelist;
mod tcp;
mod udp;


/// Current version for message formats
pub const PROTOCOL_VERSION: u8 = 1;


#[derive(Clone,PartialEq)]
#[repr(u32)]
pub enum NetworkProtocol {
    Tcp,
    Udp,
    #[cfg(unix)]
    Unix
}
impl FromStr for NetworkProtocol {
    type Err = CoalyException;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            NW_PROT_TCP => Ok(NetworkProtocol::Tcp),
            NW_PROT_UDP => Ok(NetworkProtocol::Udp),
            NW_PROT_UNIX => Ok(NetworkProtocol::Unix),
            _ => Err(coalyxw!(E_CFG_INV_NW_PROTOCOL, s.to_string()))
        }
    }
}
impl NetworkProtocol {
    fn dump(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkProtocol::Tcp => write!(f, "{}", NW_PROT_TCP),
            NetworkProtocol::Udp => write!(f, "{}", NW_PROT_UDP),
            #[cfg(unix)]
            NetworkProtocol::Unix => write!(f, "{}", NW_PROT_UNIX)
        }
    }
}
impl Display for NetworkProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl Debug for NetworkProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}

/// Address of a remote peer
pub enum PeerAddr {
    // Address of TCP or UDP socket
    IpSocket(NetworkProtocol, SocketAddr),
    // Path to Unix socket
    #[cfg(unix)]
    UnixSocket(String)
}
impl PeerAddr {
    pub(crate) fn can_talk_to(&self, other: &PeerAddr) -> bool {
        if self.protocol() != other.protocol() { return false }
        self.protocol_version() == other.protocol_version()
    }
    pub(crate) fn ip_addr(&self) -> Option<&SocketAddr> {
        match self {
            PeerAddr::IpSocket(_, addr) => Some(addr),
            _ => None
        }
    }
    pub(crate) fn protocol(&self) -> &NetworkProtocol {
        match self {
            PeerAddr::IpSocket(prot, _) => prot,
            PeerAddr::UnixSocket(_) => &NetworkProtocol::Unix
        }
    }
    fn protocol_version(&self) -> u32 {
        match self {
            PeerAddr::IpSocket(_, addr) => if addr.is_ipv4() { 4 } else { 6 },
            PeerAddr::UnixSocket(_) => 0
        }
    }
    fn dump(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerAddr::IpSocket(prot, addr) => {
                write!(f, "{}:{}", prot, addr)
            },
            #[cfg(unix)]
            PeerAddr::UnixSocket(path) => { write!(f, "unix:{}", path) }
        }
    }
}
impl Display for PeerAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl Debug for PeerAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}

/// Message sent between a Coaly client application and a log/trace server.
#[derive(Debug, Eq, PartialEq)]
pub enum Message {
    // client registration at log/trace server
    ClientNotification(OriginatorInfo),
    // log/trace record from client to log/trace server
    RecordNotification(RemoteRecordData),
    // information that a client terminates (client to log/trace server) or that the client's
    // admission has expired (log/trace server to client) 
    DisconnectNotification,
    // administrative client request to shutdown log/trace server
    ShutdownRequest(String),
    // shutdown confirmation response from log/trace server to administrative client
    ShutdownResponse
}
impl<'a> Serializable<'a> for Message {
    fn serialized_size(&self) -> usize {
        match self {
            Message::ClientNotification(orig_info) => 1 + orig_info.serialized_size(),
            Message::RecordNotification(rec) => 1 + rec.serialized_size(),
            Message::DisconnectNotification => 1,
            Message::ShutdownRequest(key) => 1 + key.serialized_size(),
            Message::ShutdownResponse => 1
        }
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        match self {
            Message::ClientNotification(orig_info) => {
                buffer.push(CLIENT_NOTIF_ID);
                1 + orig_info.serialize_to(buffer)
            },
            Message::RecordNotification(rec) => {
                buffer.push(RECORD_NOTIF_ID);
                1 + rec.serialize_to(buffer)
            },
            Message::DisconnectNotification => {
                buffer.push(DISCONNECT_NOTIF_ID);
                1
            },
            Message::ShutdownRequest(key) => {
                buffer.push(SHUTDOWN_REQ_ID);
                1 + key.serialize_to(buffer)
            },
            Message::ShutdownResponse => {
                buffer.push(SHUTDOWN_RESP_ID);
                1
            }
        }
    }
    fn deserialize_from(buffer: &'a [u8]) -> Result<Self, CoalyException> {
        let msg_type = u8::deserialize_from(buffer)?;
        if msg_type == RECORD_NOTIF_ID {
            let rec = RemoteRecordData::deserialize_from(&buffer[1..])?;
            return Ok(Message::RecordNotification(rec))
        }
        if msg_type == CLIENT_NOTIF_ID {
            let orig_info = OriginatorInfo::deserialize_from(&buffer[1..])?;
            return Ok(Message::ClientNotification(orig_info))
        }
        if msg_type == SHUTDOWN_REQ_ID {
            let key = String::deserialize_from(&buffer[1..])?;
            return Ok(Message::ShutdownRequest(key))
        }
        if msg_type == SHUTDOWN_RESP_ID { return Ok(Message::ShutdownResponse) }
        if msg_type == DISCONNECT_NOTIF_ID { return Ok(Message::DisconnectNotification) }
        Err(coalyxe!(E_DESER_ERR, String::from("Message")))
    }
}


/// Checks whether the given string contains a valid URL.
/// An URL must start with a protocol specification (either tcp or udp), followed by a colon and
/// two slashes, then an IP address (either IPv4 or IPv6),
/// optionally terminated with a colon and a port.
pub(crate) fn is_valid_url(url: &str) -> bool {
    let pattern = Regex::new(IP4_PATTERN).unwrap();
    if let Some(caps) = pattern.captures(url) {
        return SocketAddr::from_str(caps.get(2).unwrap().as_str()).is_ok()
    }
    let pattern = Regex::new(IP6_PATTERN).unwrap();
    if let Some(caps) = pattern.captures(url) {
        return SocketAddr::from_str(caps.get(2).unwrap().as_str()).is_ok()
    }
    let pattern = Regex::new(UNIX_PATTERN).unwrap();
    if let Some(caps) = pattern.captures(url) {
        let path_name = caps.get(2).unwrap().as_str();
        return Path::new(&path_name).is_file()
    }
    false
}

/// Parse URL string.
/// URL must start with a protocol identifier (tcp:, udp: or unix:) followed by either an IP network
/// address or a Unix file name.
pub(crate) fn parse_url(url: &str) -> Result<PeerAddr, CoalyException> {
    let pattern = Regex::new(IP4_PATTERN).unwrap();
    if let Some(capts) = pattern.captures(url) {
        let prot = NetworkProtocol::from_str(capts.get(1).unwrap().as_str())?;
        match SocketAddrV4::from_str(capts.get(2).unwrap().as_str()) {
            Ok(addr) => {
                let peer_addr = PeerAddr::IpSocket(prot, SocketAddr::V4(addr));
                return Ok(peer_addr)
            },
            Err(_) => return Err(coalyxe!(E_INVALID_URL, url.to_string()))
        }
    }
    let pattern = Regex::new(IP6_PATTERN).unwrap();
    if let Some(capts) = pattern.captures(url) {
        let prot = NetworkProtocol::from_str(capts.get(1).unwrap().as_str())?;
        match SocketAddrV6::from_str(capts.get(2).unwrap().as_str()) {
            Ok(addr) => {
                let peer_addr = PeerAddr::IpSocket(prot, SocketAddr::V6(addr));
                return Ok(peer_addr)
            },
            Err(_) => return Err(coalyxe!(E_INVALID_URL, url.to_string()))
        }
    }
    let pattern = Regex::new(UNIX_PATTERN).unwrap();
    if let Some(capts) = pattern.captures(url) {
        let path_name = capts.get(2).unwrap().as_str().to_string();
        let path = Path::new(&path_name);
        if ! path.is_file() { return Err(coalyxe!(E_INVALID_URL, url.to_string())) }
        return Ok(PeerAddr::UnixSocket(path_name))
    }
    Err(coalyxe!(E_INVALID_URL, url.to_string()))
}

/// Parses an URL string and returns specified protocol and IP address. 
//pub(crate) fn parse_url(url: &str) -> Option<(NetworkProtocol, SocketAddr)> {
//    let url_pattern = Regex::new(URL_PATTERN).unwrap();
//    if let Some(caps) = url_pattern.captures(url) {
//        let prot = NetworkProtocol::from_str(caps.get(1).unwrap().as_str()).unwrap();
//        if let Ok(addr) = SocketAddr::from_str(caps.get(2).unwrap().as_str()) {
//            return Some((prot, addr))
//        }
//    }
//    None
//}

const NW_PROT_TCP: &str = "tcp";
const NW_PROT_UDP: &str = "udp";
const NW_PROT_UNIX: &str = "unix";
const IP4_PATTERN: &str = r"^(tcp|udp)://([\d\.]+:\d+)$";
const IP6_PATTERN: &str = r"^(tcp|udp)://\[(\d\.]+\]:\d+)$";
const UNIX_PATTERN: &str = r"^(unix):(.*)$";

/// Message type ID for new client notification
const CLIENT_NOTIF_ID: u8 = 11;

/// Message type ID for log/trace record notification
const RECORD_NOTIF_ID: u8 = 12;

/// Message type ID for disconnect notification
const DISCONNECT_NOTIF_ID: u8 = 13;

/// Message type ID for shutdown request
const SHUTDOWN_REQ_ID: u8 = 21;

/// Message type ID for shutdown response
const SHUTDOWN_RESP_ID: u8 = 31;

//const URL_PATTERN: &str = "^(tcp|udp)://(.*)$";

#[cfg(all(net, test))]
mod tests {
    use super::*;
    use core::fmt::Debug;
    use crate::record::RecordLevelId;
    use crate::record::recorddata::LocalRecordData;

    fn check_serialization<'a, T>(item: &'a T, expected_size: usize, buffer: &'a mut Vec<u8>)
        where T: Serializable<'a> + Debug + Eq {
        buffer.clear();
        assert_eq!(expected_size, item.serialized_size());
        let sz = item.serialize_to(buffer);
        assert_eq!(expected_size, sz);
        let clone = T::deserialize_from(buffer);
        assert!(clone.is_ok());
        assert_eq!(clone.unwrap(), *item);
    }

    #[test]
    fn test_serialize_client_notification() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let mut oinfo = OriginatorInfo::new(1234, "testapp", "clienthost", "1.2.3.4");
        oinfo.set_application_name("superapp");
        oinfo.add_env_var("COALYROOT", "/var/log/superapp");
        oinfo.add_env_var("LANG", "en");
        let msg = Message::ClientNotification(oinfo);
        check_serialization::<Message>(&msg, 143, &mut buffer);
    }

    #[test]
    fn test_serialize_record_notification() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let local_rec = LocalRecordData::for_write(1234, "mythread", RecordLevelId::Error, 
                                                   "test.rs", 393, "blabla");
        let remote_rec = local_rec.as_remote();
        let msg = Message::RecordNotification(remote_rec);
        check_serialization::<Message>(&msg, 90, &mut buffer);
    }

    #[test]
    fn test_serialize_disconnect_notification() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let msg = Message::DisconnectNotification;
        check_serialization::<Message>(&msg, 1, &mut buffer);
    }

    #[test]
    fn test_serialize_shutdown_request() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let key = String::from("TOPSECRET");
        let msg = Message::ShutdownRequest(key);
        check_serialization::<Message>(&msg, 18, &mut buffer);
    }

    #[test]
    fn test_serialize_shutdown_response() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let msg = Message::ShutdownResponse;
        check_serialization::<Message>(&msg, 1, &mut buffer);
    }
}