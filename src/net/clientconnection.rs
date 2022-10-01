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

//! Structures to administer clients in a Coaly logging server.


use chrono::Local;
use std::collections::HashMap;
use std::net::SocketAddr;
use crate::OriginatorInfo;


/// Table holding descriptors for all active client connections
pub(super) struct ClientConnectionTable {
    // descriptors for every active client, clients are identified by their socket address
    connections: HashMap<SocketAddr, ClientConnection>,
    // maximum number of active connections allowed
    conn_limit: usize,
    // maximum timespan to consider a connection as active since last message was received.
    // Applies to UDP connections only, because we get an error on the server socket
    // when a TCP client disconnects
    conn_keep_time: i64
}
impl ClientConnectionTable {
    /// Creates a table for active client connections.
    ///
    /// # Arguments
    /// * `conn_limit` - the maximum number of active connections allowed
    /// * `conn_keep_time` - the maximum timespan to consider a connection as active, in seconds
    #[inline]
    pub(super) fn new(conn_limit: usize,
                      conn_keep_time: u32) -> ClientConnectionTable {
        ClientConnectionTable {
            connections: HashMap::with_capacity(conn_limit),
            conn_limit,
            conn_keep_time: conn_keep_time as i64
        }
    }

    /// Returns a mutable reference to the connection descriptor structure for given client address
    ///
    /// # Arguments
    /// * `client_addr` - the client's socket address (IP address plus port)
    #[inline]
    pub(super) fn get_mut(&mut self,
                          client_addr: &SocketAddr) -> Option<&mut ClientConnection> {
        self.connections.get_mut(client_addr)
    }

    /// Adds a new connection descriptor to the table.
    ///
    /// # Arguments
    /// * `client_addr` - the client's socket address (IP address plus port)
    /// * `client_info` - information about the client
    /// * `purge` - indicates whether to purge connections longer inactive than keep limit before
    ///             trying to insert the descriptor. Use **true** for UDP, **false** for TCP.
    ///
    /// # Return values
    /// **true** if a connection descriptor could be inserted or re-used; **false** if the
    /// maximum number of allowed connections was exceeded
    pub(super) fn add(&mut self,
                      client_addr: &SocketAddr,
                      client_info: &OriginatorInfo,
                      purge: bool) -> bool {
        // remove inactive connections, if desired
        if purge { self.purge_expired_connections(); }
        if let Some(desc) = self.connections.get_mut(client_addr) {
            // re-connect, just update client information
            desc.refresh(client_info.clone());
            return true
        }
        // check, whether we would exceed maximum number of allowed connections
        if self.connections.len() >= self.conn_limit { return false }
        // insert new descriptor
        let desc = ClientConnection::new(client_info.clone());
        self.connections.insert(*client_addr, desc);
        true
    }

    /// Removes a connection descriptor from the table.
    ///
    /// # Arguments
    /// * `client_addr` - the client's socket address (IP address plus port)
    #[inline]
    pub(super) fn remove(&mut self,
                         client_addr: &SocketAddr) {
        let _ = self.connections.remove(client_addr);
    }

    /// Removes all connection descriptors from the table, where last message was received
    /// before maximum time to keep.
    fn purge_expired_connections(&mut self) {
        let now = Local::now().timestamp();
        let mut clients_to_purge = Vec::<SocketAddr>::new();
        for (addr, desc) in &self.connections {
            if now - desc.last_rx_time() > self.conn_keep_time {
                clients_to_purge.push(*addr);
            }
        }
        for addr in clients_to_purge { self.connections.remove(&addr); }
    }
}


/// Descriptor for an active client connection
pub(crate) struct ClientConnection {
    // information about the client
    client_info: OriginatorInfo,
    // sequence number of last record received from client
    last_seq_nr: u64,
    // timestamp when last record was received from client
    last_rx_time: i64
}
impl ClientConnection {
    /// Creates a connection descriptor.
    ///
    /// # Arguments
    /// * `client_info` - information about the client
    #[inline]
    fn new(client_info: OriginatorInfo) -> ClientConnection {
        ClientConnection {
            client_info,
            last_seq_nr: 0,
            last_rx_time: Local::now().timestamp()
        }
    }

    /// Called by record handler when a log or trace record was successfully received.
    /// Updates sequence number and timestamp indicating last activity from the client.
    ///
    /// # Arguments
    /// * `seq_nr` - the record sequence number as sent by the client
    #[inline]
    pub(super) fn record_received(&mut self,
                                  seq_nr: u64) {
        self.last_rx_time = Local::now().timestamp();
        self.last_seq_nr = seq_nr;
    }

    /// Returns the timestamp when last message was received from the client
    #[inline]
    pub(super) fn last_rx_time(&self) -> i64 { self.last_rx_time }

    /// Re-use the descriptor, eventually with changed client information.
    /// May happen, if we couldn't notice a client disconnected and now the client connects again
    /// using the same socket address.
    ///
    /// # Arguments
    /// * `client_info` - information about the client
    pub(super) fn refresh(&mut self,
                          client_info: OriginatorInfo) {
        self.client_info = client_info;
        self.last_seq_nr = 0;
        self.last_rx_time = Local::now().timestamp();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::OriginatorInfo;
    use std::net::SocketAddr;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_client_connection() {
        let oinfo1 = OriginatorInfo::new(1, "p1", "host1", "1.2.3.4");
        let oinfo2 = OriginatorInfo::new(2, "p2", "host2", "11.12.13.14");
        let mut cxn = ClientConnection::new(oinfo1.clone());
        let cre_ts = cxn.last_rx_time();
        assert_eq!(&oinfo1, &cxn.client_info);
        assert_eq!(0u64, cxn.last_seq_nr);
        thread::sleep(Duration::from_millis(1000));
        cxn.record_received(123);
        assert_eq!(&oinfo1, &cxn.client_info);
        assert_eq!(123u64, cxn.last_seq_nr);
        assert_ne!(cre_ts, cxn.last_rx_time());
        let cre_ts = cxn.last_rx_time();
        thread::sleep(Duration::from_millis(1000));
        cxn.refresh(oinfo2.clone());
        assert_eq!(&oinfo2, &cxn.client_info);
        assert_eq!(0u64, cxn.last_seq_nr);
        assert_ne!(cre_ts, cxn.last_rx_time());
    }

    #[test]
    fn test_client_connection_table() {
        let addr1: SocketAddr = "1.2.3.4:1111".parse().unwrap();
        let addr2: SocketAddr = "11.12.13.14:2222".parse().unwrap();
        let addr3: SocketAddr = "21.22.23.24:3333".parse().unwrap();
        let addr4: SocketAddr = "31.32.33.34:4444".parse().unwrap();
        let oinfo1 = OriginatorInfo::new(1, "p1", "host1", "1.2.3.4");
        let oinfo2 = OriginatorInfo::new(2, "p2", "host2", "11.12.13.14");
        let oinfo3 = OriginatorInfo::new(3, "p3", "host3", "21.22.23.24");
        let oinfo4 = OriginatorInfo::new(4, "p3", "host4", "31.32.33.34");
        let mut cxn_table = ClientConnectionTable::new(3, 1);
        // check table after construction
        assert_eq!(3, cxn_table.conn_limit);
        assert_eq!(1, cxn_table.conn_keep_time);
        assert!(cxn_table.connections.is_empty());
        // check insertions of different addresses
        assert!(cxn_table.add(&addr1, &oinfo1, false));
        assert!(cxn_table.add(&addr2, &oinfo2, false));
        assert!(cxn_table.add(&addr3, &oinfo3, false));
        assert!(! cxn_table.add(&addr4, &oinfo4, false));
        assert!(cxn_table.get_mut(&addr1).is_some());
        assert!(cxn_table.get_mut(&addr2).is_some());
        assert!(cxn_table.get_mut(&addr3).is_some());
        assert!(cxn_table.get_mut(&addr4).is_none());
        // check update for existing address
        assert!(cxn_table.add(&addr2, &oinfo4, false));
        assert_eq!(&oinfo4, &cxn_table.get_mut(&addr2).unwrap().client_info);
        // check purge
        thread::sleep(Duration::from_millis(2000));
        assert!(cxn_table.add(&addr4, &oinfo4, true));
        assert_eq!(1, cxn_table.connections.len());
    }
}
