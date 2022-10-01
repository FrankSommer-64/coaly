// ---------------------------------------------------------------------------------------------
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
// ---------------------------------------------------------------------------------------------

use core::str::FromStr;
use regex::Regex;
use crate::errorhandling::*;
use crate::coalyxe;
use std::net::SocketAddr;

/// Whitelist structure to check client access to server
#[derive(Clone, Debug)]
pub(super) struct ClientWhitelist(Vec<(SocketAddrPattern, Vec<u32>)>);
impl ClientWhitelist {
    /// Creates a whitelist from the given (socket address, application IDs)-tuples.
    ///
    /// # Arguments
    /// * `desc_list` - the slice containing tuples of allowed socket addresses and application IDs
    pub(super) fn from_ip_and_app_ids(desc_list: &[(String, Vec<u32>)]) -> ClientWhitelist {
        let mut wl = ClientWhitelist { 0: Vec::new() };
        for (addr, app_ids) in desc_list { wl.add(addr, app_ids); }
        wl
    }

    /// Creates a whitelist from the given IP-addresses
    ///
    /// # Arguments
    /// * `desc_list` - the slice containing all allowed client IP addresses
    pub(super) fn from_ip(desc_list: &[String]) -> ClientWhitelist {
        let mut wl = ClientWhitelist { 0: Vec::new() };
        for desc in desc_list { wl.add(desc, &[]); }
        wl
    }

    /// Adds a descriptor into the white list.
    ///
    /// # Arguments
    /// * `addr_pattern` - the pattern for the client's IP address
    /// * `app_ids` - the slice with all allowed application IDs for the address
    fn add(&mut self,
           addr_pattern: &str,
           app_ids: &[u32]) {
        if let Ok(addr_desc) = addr_pattern.parse() {
            self.0.push((addr_desc, app_ids.to_vec()));
        }
    }

    /// Checks, whether the whitelist permits a client using the specified socket address and
    /// application ID to access the server.
    /// Used for log and trace records.
    ///
    /// # Arguments
    /// * `addr` - the client's socket address
    /// * `app_id` - the client's application ID
    pub(super) fn allows_addr_and_appid(&self,
                                        addr: &SocketAddr,
                                        app_id: u32) -> bool {
        for (addr_desc, wl_app_ids) in &self.0 {
            if ! addr_desc.matches(&addr) { continue; }
            for wl_app_id in wl_app_ids {
                if *wl_app_id == 0 || *wl_app_id == app_id { return true }
            }
        }
        false
    }

    /// Checks, whether the whitelist permits a client using the specified socket address
    /// to access the server.
    /// Used for administrative commands.
    ///
    /// # Arguments
    /// * `addr` - the client's socket address
    pub(super) fn allows_addr(&self, addr: &SocketAddr) -> bool {
        for (addr_desc, _) in &self.0 {
            if addr_desc.matches(addr) { return true }
        }
        false
    }
}


/// Descriptor structure for fast matching of socket addresses against an allowed pattern.
#[derive(Clone, Debug)]
struct SocketAddrPattern {
    // indicators for every part of the address, false means wildcard, i.e. segment can be ignored
    segment_flags: [bool; 8],
    // specific value for every part of the address
    segment_patterns: [u16; 8],
    // required port number with 0 acting as wildcard
    port: u16
}
impl SocketAddrPattern {
    /// Indicates whether the specified socket address matches the pattern.
    /// For IPv4 addresses, each octet is compared against this descriptor's value,
    /// unless the octet has been marked as any value accepted.
    /// For IPv6 addresses, each 16-bit segment is compared against this descriptor's value,
    /// unless the segment has been marked as any value accepted.
    fn matches(&self, addr: &SocketAddr) -> bool {
        match addr {
            SocketAddr::V4(addr4) => {
                let octets = addr4.ip().octets();
                for i in 0 .. 4  {
                    if self.segment_flags[i] {
                        if octets[i] != self.segment_patterns[i] as u8 { return false }
                    }
                }
            },
            SocketAddr::V6(addr6) => {
                let segments = addr6.ip().segments();
                for i in 0 .. 8  {
                    if self.segment_flags[i] {
                        if segments[i] != self.segment_patterns[i] { return false }
                    }
                }
            }
        }
        self.port == 0 || self.port == addr.port()
    }
}
impl FromStr for SocketAddrPattern {
    type Err = CoalyException;

    /// Parses a socket address descriptor from the specified string.
    ///
    /// IPv4 patterns allowed:
    ///
    ///  `*` - any address and port allowed
    ///  `*:*` - any address and port allowed
    ///  `*:0` - any address and port allowed
    ///  `n.n.n.n` - each octet must match given number unless n is '*', any port allowed
    ///  `n.n.n.n:p` - each octet must match given number unless n is '*',
    ///                port match given port number unless p is '*' or '0'
    ///
    /// IPv6 patterns allowed:
    ///
    ///  `[*]` - any address and port allowed
    ///  `[*]:*` - any address and port allowed
    ///  `[*]:0` - any address and port allowed
    ///  `[n:n:n:n:n:n:n:n]` - each segment must match given number unless n is '*' or '',
    ///                        any port allowed. Segment numbers must be specified in hexadecimal.
    ///  `[n:n:n:n:n:n:n:n]:p` - each segment must match given number unless n is '*' or '',
    ///                          port must match given port number unless p is '*' or '0'.
    ///                          Segment numbers must be specified in hexadecimal, port decimal.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut segment_flags: [bool; 8] = [false; 8];
        let mut segment_patterns: [u16; 8] = [0; 8];
        let mut port: u16 = 0;

        // any IP address and port
        if s == IP4_ADDR_ANY || s == IP4_ADDR_ANY_PORT_ANY || s == IP4_ADDR_ANY_PORT_0 ||
           s == IP6_ADDR_ANY || s == IP6_ADDR_ANY_PORT_ANY || s == IP6_ADDR_ANY_PORT_0 {
            return Ok(SocketAddrPattern { segment_flags, segment_patterns, port })
        }

        // IP4 address with optional port
        let p = Regex::new(IP4_ADDR_PATTERN).unwrap();
        if let Some(caps) = p.captures(s) {
            for i in 1 ..= 4 {
                let octet = caps.get(i).unwrap().as_str();
                if octet != "*" {
                    segment_flags[i-1] = true;
                    match u8::from_str_radix(octet, 10) {
                        Ok(num) => segment_patterns[i-1] = num as u16,
                        Err(_) => return Err(coalyxe!(E_IP4_OCTET_TOO_LARGE, octet.to_string()))
                    }
                }
            }
            if let Some(octet) = caps.get(6) {
                let octet = octet.as_str();
                if octet != "*" {
                    match u16::from_str_radix(octet, 10) {
                        Ok(num) => port = num,
                        Err(_) => return Err(coalyxe!(E_IP_PORT_TOO_LARGE, octet.to_string()))
                    }
                    
                }
            }
            return Ok(SocketAddrPattern { segment_flags, segment_patterns, port })
        }

        // IP6 address with optional port
        let p = Regex::new(IP6_ADDR_PATTERN).unwrap();
        if let Some(caps) = p.captures(s) {
            for i in 1 ..= 8 {
                let segment = caps.get(i).unwrap().as_str();
                if ! segment.is_empty() && segment != "*" {
                    segment_flags[i-1] = true;
                    segment_patterns[i-1] = u16::from_str_radix(segment, 16).unwrap();
                }
            }
            if let Some(segment) = caps.get(10) {
                let segment = segment.as_str();
                if ! segment.is_empty() && segment != "*" {
                    match u16::from_str_radix(segment, 10) {
                        Ok(num) => port = num,
                        Err(_) => return Err(coalyxe!(E_IP_PORT_TOO_LARGE, segment.to_string()))
                    }
                }
            }
            return Ok(SocketAddrPattern { segment_flags, segment_patterns, port })
        }
        Err(coalyxe!(E_INVALID_ADDR_PATTERN, s.to_string()))
    }
}

const IP4_ADDR_ANY: &str = "*";
const IP4_ADDR_ANY_PORT_ANY: &str = "*:*";
const IP4_ADDR_ANY_PORT_0: &str = "*:0";
const IP6_ADDR_ANY: &str = "[*]";
const IP6_ADDR_ANY_PORT_ANY: &str = "[*]:*";
const IP6_ADDR_ANY_PORT_0: &str = "[*]:0";

const IP4_ADDR_PATTERN: &str = r"^(\*|[\d]{1,3})\.(\*|[\d]{1,3})\.(\*|[\d]{1,3})\.(\*|[\d]{1,3})(:(\*|[\d]{1,5})){0,1}$";
const IP6_ADDR_PATTERN: &str = r"^\[(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4}):(\*|[\da-fA-F]{0,4})\](:(\*|[\d]{1,5})){0,1}$";

#[cfg(test)]
mod test {
    use super::*;
    use std::net::SocketAddr;

    const FLAGS_ALL_FALSE: [bool; 8] = [false; 8];
    const FLAGS_ALL_TRUE: [bool; 8] = [true; 8];
    const FLAGS_7T: [bool; 8] = [true,true,true,true,true,true,true,false];
    const FLAGS_4T: [bool; 8] = [true,true,true,true,false,false,false,false];
    const FLAGS_3T: [bool; 8] = [true,true,true,false,false,false,false,false];
    const VALUES_ALL_0: [u16; 8] = [0; 8];
    const VALUES_IP4_LOOPBACK: [u16; 8] = [127, 0, 0, 1, 0, 0, 0, 0];
    const VALUES_IP6_LOOPBACK: [u16; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
    const VALUES_192_168_2_X: [u16; 8] = [192, 168, 2, 0, 0, 0, 0, 0];
    const VALUES_DEAD_BEEF_X: [u16; 8] = [1, 0xab, 0xcd, 0xffff, 0, 0xdead, 0xbeef, 0];

    #[test]
    fn test_whitelist_addr_only() {
        // IPv4
        let ip4_addr1: SocketAddr = "127.0.0.1:1111".parse().unwrap();
        let ip4_addr2: SocketAddr = "192.168.203.88:7654".parse().unwrap();
        let ip4_addr3: SocketAddr = "192.168.203.88:6000".parse().unwrap();
        let ip4_addr4: SocketAddr = "192.168.203.99:7654".parse().unwrap();
        let desc_list = [ String::from("127.0.0.1:*"), String::from("192.168.203.88:7654") ];
        let white_list = ClientWhitelist::from_ip(&desc_list);
        assert!(white_list.allows_addr(&ip4_addr1));
        assert!(white_list.allows_addr(&ip4_addr2));
        assert!(! white_list.allows_addr(&ip4_addr3));
        assert!(! white_list.allows_addr(&ip4_addr4));

        // IPv6
        let ip6_addr1: SocketAddr = "[0:0:0:0:0:0:0:1]:1111".parse().unwrap();
        let ip6_addr2: SocketAddr = "[2a02:2e0:3fe:1001:302::]:7654".parse().unwrap();
        let ip6_addr3: SocketAddr = "[2a02:2e0:3fe:1001:302::]:6000".parse().unwrap();
        let ip6_addr4: SocketAddr = "[2a02:2e0:3fe:1001:303::]:7654".parse().unwrap();
        let desc_list = [ String::from("[0:0:0:0:0:0:0:1]:*"),
                          String::from("[2a02:2e0:3fe:1001:302:::]:7654") ];
        let white_list = ClientWhitelist::from_ip(&desc_list);
        assert!(white_list.allows_addr(&ip6_addr1));
        assert!(white_list.allows_addr(&ip6_addr2));
        assert!(! white_list.allows_addr(&ip6_addr3));
        assert!(! white_list.allows_addr(&ip6_addr4));
    }

    #[test]
    fn test_whitelist_addr_and_appid() {
        let appids_0 = vec!(0u32);
        let appids_2 = vec!(1u32, 100u32);

        // IPv4
        let ip4_addr1: SocketAddr = "127.0.0.1:1111".parse().unwrap();
        let ip4_addr2: SocketAddr = "192.168.203.88:7654".parse().unwrap();
        let ip4_addr3: SocketAddr = "192.168.203.99:7654".parse().unwrap();
        let desc_list = [ (String::from("127.0.0.1:*"), appids_0.clone()),
                          (String::from("192.168.203.88:7654"), appids_2.clone()) ];
        let white_list = ClientWhitelist::from_ip_and_app_ids(&desc_list);
        assert!(white_list.allows_addr_and_appid(&ip4_addr1, 123));
        assert!(white_list.allows_addr_and_appid(&ip4_addr2, 1));
        assert!(white_list.allows_addr_and_appid(&ip4_addr2, 100));
        assert!(! white_list.allows_addr_and_appid(&ip4_addr2, 3));
        assert!(! white_list.allows_addr_and_appid(&ip4_addr3, 1));

        // IPv6
        let ip6_addr1: SocketAddr = "[0:0:0:0:0:0:0:1]:1111".parse().unwrap();
        let ip6_addr2: SocketAddr = "[2a02:2e0:3fe:1001:302::]:7654".parse().unwrap();
        let ip6_addr3: SocketAddr = "[2a02:2e0:3fe:1001:303::]:7654".parse().unwrap();
        let desc_list = [ (String::from("[0:0:0:0:0:0:0:1]:*"), appids_0),
                          (String::from("[2a02:2e0:3fe:1001:302:::]:7654"), appids_2) ];
        let white_list = ClientWhitelist::from_ip_and_app_ids(&desc_list);
        assert!(white_list.allows_addr_and_appid(&ip6_addr1, 123));
        assert!(white_list.allows_addr_and_appid(&ip6_addr2, 1));
        assert!(white_list.allows_addr_and_appid(&ip6_addr2, 100));
        assert!(! white_list.allows_addr_and_appid(&ip6_addr2, 3));
        assert!(! white_list.allows_addr_and_appid(&ip6_addr3, 1));
    }

    #[test]
    fn test_socket_addr_pattern_creation() {
        // IPv4, valid
        validate_pattern_creation("*", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("*:*", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("*:0", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("*.*.*.*:*", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("*.*.*.*:0", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("127.0.0.1:4000", true, &FLAGS_4T, &VALUES_IP4_LOOPBACK, 4000);
        validate_pattern_creation("192.168.2.*:8888", true, &FLAGS_3T, &VALUES_192_168_2_X, 8888);
        // IPv4, starting dot
        validate_pattern_creation(".0.0.1", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, ending dot
        validate_pattern_creation("127.0.0.", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, adjacent dots
        validate_pattern_creation("127.0..1", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, too few dots
        validate_pattern_creation("127.0.1", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, too many dots
        validate_pattern_creation("127.0.0.1.2", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, too many segment digits
        validate_pattern_creation("127.0.0.1234", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, segment too large
        validate_pattern_creation("127.999.0.1", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, port missing
        validate_pattern_creation("127.0.0.1:", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, double colon port
        validate_pattern_creation("127.0.0.1::0", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, port too large
        validate_pattern_creation("127.0.0.1:72345", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, invalid segment char
        validate_pattern_creation("127.0.0.aa:0", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv4, invalid port char
        validate_pattern_creation("127.0.0.1:abcd", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);

        // IPv6, valid
        validate_pattern_creation("[*]", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[*]:*", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[*]:0", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[*:*:*:*:*:*:*:*]:*", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[*:*:*:*:*:*:*:*]:0", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[:::::::]:*", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[:::::::]:0", true, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        validate_pattern_creation("[0:0:0:0:0:0:0:1]:3333", true, &FLAGS_ALL_TRUE,
                                  &VALUES_IP6_LOOPBACK, 3333);
        validate_pattern_creation("[1:ab:cd:ffff:0:DEAD:BEEF:*]:1234", true, &FLAGS_7T,
                                  &VALUES_DEAD_BEEF_X, 1234);
        // IPv6, addr missing
        validate_pattern_creation("[]", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, closing bracket missing
        validate_pattern_creation("[", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, port missing
        validate_pattern_creation("[*]:", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, double colon port
        validate_pattern_creation("[*]::0", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, port too large
        validate_pattern_creation("[*]:99999", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, too few colons
        validate_pattern_creation("[:::]:*", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, too many colons
        validate_pattern_creation("[::::::::]", false, &FLAGS_ALL_FALSE, &VALUES_ALL_0, 0);
        // IPv6, invalid segment char
        validate_pattern_creation("[0:0:0:0:wxyz:0:0:1]:0", false, &FLAGS_ALL_FALSE,
                                  &VALUES_ALL_0, 0);
        // IPv6, invalid port char
        validate_pattern_creation("[0:0:0:0:0:0:0:1]:abcd", false, &FLAGS_ALL_FALSE,
                                  &VALUES_ALL_0, 0);
    }

    #[test]
    fn test_socket_addr_pattern_match() {
        // IPv4
        let ip4_addr1: SocketAddr = "127.0.0.1:1111".parse().unwrap();
        let ip4_addr2: SocketAddr = "192.168.203.99:6000".parse().unwrap();
        let ip4_addr3: SocketAddr = "192.168.203.199:6000".parse().unwrap();
        let ip4_addrs = [&ip4_addr1, &ip4_addr2, &ip4_addr3];
        let ip4_pat1 = "127.0.0.1:*".parse::<SocketAddrPattern>().unwrap();
        let ip4_pat2 = "127.0.0.1:1111".parse::<SocketAddrPattern>().unwrap();
        let ip4_pat3 = "127.0.0.1:2222".parse::<SocketAddrPattern>().unwrap();
        let ip4_pat4 = "192.168.203.*:*".parse::<SocketAddrPattern>().unwrap();
        let ip4_pat5 = "192.168.203.*:6000".parse::<SocketAddrPattern>().unwrap();
        let ip4_pat6 = "192.168.203.*:7000".parse::<SocketAddrPattern>().unwrap();
        let ip4_pat7 = "192.168.203.99:6000".parse::<SocketAddrPattern>().unwrap();

        validate_pattern_match(&ip4_pat1, &ip4_addrs, &[true,false,false]);
        validate_pattern_match(&ip4_pat2, &ip4_addrs, &[true,false,false]);
        validate_pattern_match(&ip4_pat3, &ip4_addrs, &[false,false,false]);
        validate_pattern_match(&ip4_pat4, &ip4_addrs, &[false,true,true]);
        validate_pattern_match(&ip4_pat5, &ip4_addrs, &[false,true,true]);
        validate_pattern_match(&ip4_pat6, &ip4_addrs, &[false,false,false]);
        validate_pattern_match(&ip4_pat7, &ip4_addrs, &[false,true,false]);
    }

    fn validate_pattern_creation(p: &str,
                                 ok_expected: bool,
                                 seg_flags: &[bool],
                                 seg_values: &[u16],
                                 port: u16) {
        let addr_pattern = p.parse::<SocketAddrPattern>();
        assert_eq!(ok_expected, addr_pattern.is_ok());
        if let Ok(pattern) = addr_pattern {
            for i in 0 .. 8 {
                assert_eq!(seg_flags[i], pattern.segment_flags[i]);
                assert_eq!(seg_values[i], pattern.segment_patterns[i]);
            }
            assert_eq!(port, pattern.port);
        }
    }

    fn validate_pattern_match(pattern: &SocketAddrPattern,
                              addrs: &[&SocketAddr],
                              expected_results: &[bool]) {
        for (i, a) in addrs.iter().enumerate() {
            assert_eq!(expected_results[i], pattern.matches(a));
        }
    }
}
