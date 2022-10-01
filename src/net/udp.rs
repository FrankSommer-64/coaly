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

use crate::*;
use crate::net::*;
use crate::net::buffer::{ReceiveBuffer, SendBuffer};
use super::clientconnection::ClientConnectionTable;
use super::clientwhitelist::ClientWhitelist;

use tokio::net::UdpSocket;
use tokio::sync::broadcast::*;

/// Handler for administrative messages sent to Coaly logging server via UDP.
pub(super) struct UdpAdminHandler {
    // UDP socket to use for communication
    socket: UdpSocket,
    // receive buffer for incoming messages
    rx_buf: ReceiveBuffer,
    // send buffer for outgoing messages
    tx_buf: SendBuffer
}
impl UdpAdminHandler {
    /// Creates a UDP admin handler on the socket supplied.
    pub(super) fn new(socket: UdpSocket) -> UdpAdminHandler  {
        UdpAdminHandler {
            socket,
            rx_buf: ReceiveBuffer::new(PROTOCOL_VERSION as u32, 128),
            tx_buf: SendBuffer::new(PROTOCOL_VERSION as u32, 128)
        }
    }

    /// Runs the UDP admin handler.
    /// The handler terminates upon the following events:
    /// - a shutdown message was received
    /// - a socket I/O error ocurred
    /// - a shutdown was signaled from another part of the server
    pub(super) async fn run(&mut self,
                            adm_key: String,
                            client_whitelist: &ClientWhitelist,
                            shutdown_sender: Sender<bool>,
                            mut shutdown_listener: Receiver<bool>) {
        loginfo!("Started UDP admin handler waiting for messages on address {}",
                 local_addr_of(&self.socket));
        loop {
            tokio::select! {
                maybe_shutdown_msg = self.socket.recv_from(self.rx_buf.as_mut_slice()) => {
                    match maybe_shutdown_msg {
                        Ok((n, addr)) => {
                            if ! client_whitelist.allows_addr(&addr) {
                                loginfo!("Rejected admin message, client {} not allowed", addr);
                                continue;
                            }
                            if let Ok(Message::ShutdownRequest(req)) = self.rx_buf.message(n) {
                                if req != *adm_key {
                                    loginfo!("Rejected shutdown message from {}, invalid key",
                                             addr);
                                    continue;
                                }
                                loginfo!("Accepted shutdown message from {}", addr);
                                self.tx_buf.store_shutdown_response();
                                let _ = self.socket.send_to(self.tx_buf.as_slice(), addr).await;
                                let _ = shutdown_sender.send(true);
                                return;
                            }
                            loginfo!("Ignored invalid admin message from {}", addr);
                            continue;
                        },
                        Err(io_err) => {
                            logwarn!("Terminated UDP admin handler due to socket error: {}",
                                     io_err);
                        }
                    }
                }
                _ = shutdown_listener.recv() => { return }
            }
            break
        }
    }
}

/// Handler for log and trace messages sent to Coaly logging server via UDP.
pub(super) struct UdpRecordHandler {
    // UDP socket to use for communication
    socket: UdpSocket,
    // receive buffer for incoming messages
    rx_buf: ReceiveBuffer,
    // list with IP addresses, optional port number and application ID of clients allowed
    // to send log and trace messages
    client_whitelist: ClientWhitelist,
    // sender part of broadcast channel for graceful shutdown
    // used to indicate a shutdown when an unrecoverable I/O Error on the socket occurs
    shutdown_sender: Sender<bool>,
    // used to handle a shutdown detected by another part of the server
    shutdown_listener: Receiver<bool>
}
impl UdpRecordHandler {
    /// Creates a UDP record handler on the socket supplied.
    pub(super) fn new(socket: UdpSocket,
                      client_whitelist: ClientWhitelist,
                      shutdown_sender: Sender<bool>,
                      shutdown_listener: Receiver<bool>,
                      max_msg_size: usize) -> UdpRecordHandler  {
        UdpRecordHandler {
            socket,
            rx_buf: ReceiveBuffer::new(PROTOCOL_VERSION as u32, max_msg_size),
            client_whitelist,
            shutdown_sender,
            shutdown_listener
        }
    }

    /// Runs the UDP record handler.
    /// The handler terminates upon the following events:
    /// - a socket I/O error ocurred
    /// - a shutdown was signaled from another part of the server
    pub(super) async fn run(&mut self,
                            max_conns: usize,
                            keep_time: u32) {
        loginfo!("Started UDP record handler waiting for messages on address {}",
                 local_addr_of(&self.socket));
        let mut conn_table = ClientConnectionTable::new(max_conns, keep_time);
        loop {
            tokio::select! {
                maybe_msg = self.socket.recv_from(self.rx_buf.as_mut_slice()) => {
                    match maybe_msg {
                        Ok((n, addr)) => {
                            match self.rx_buf.message(n) {
                                Ok(msg) => {
                                    match msg {
                                        Message::ClientNotification(client) => {
                                            let app_id = client.application_id_value();
                                            if ! self.client_whitelist.allows_addr_and_appid(&addr, app_id) {
                                                loginfo!("Access for client {} with app ID {} denied", addr, app_id);
                                                continue;
                                            }
                                            if ! conn_table.add(&addr, &client, true) {
                                                loginfo!("Connection limit exceeded, could not accept client {}", addr);
                                                continue;
                                            }
                                            loginfo!("Client {} with app ID {} accepted", addr, app_id);
                                            agent::remote_client_connected(&addr, client);
                                        },
                                        Message::RecordNotification(rec) => {
                                            if let Some(conn) = conn_table.get_mut(&addr) {
                                                conn.record_received(self.rx_buf.sequence_nr());
                                                agent::write_rec(&addr, rec);
                                            }
                                        },
                                        Message::DisconnectNotification => {
                                            loginfo!("Client {} disconnected", addr);
                                            conn_table.remove(&addr);
                                            agent::remote_client_disconnected(&addr);
                                        },
                                        _ =>  {
                                            loginfo!("Ignored invalid message {:?} from {}", msg, addr);
                                        }
                                    }
                                },
                                Err(e) => {
                                    logerror!("Error receiving message: {}", e.localized_message());
                                }
                            }
                        },
                        Err(e) => {
                            logerror!("Error reading from UDP socket: {}, terminating UDP record handler", e);
                            let _ = self.shutdown_sender.send(true);
                            return
                        }
                    }
                    continue;
                }
                _ = self.shutdown_listener.recv() => { return }
            }
        }
    }
}

#[inline]
fn local_addr_of(socket: &UdpSocket) -> String {
    if let Ok(addr) = socket.local_addr() { return addr.to_string() }
    String::from("-unknown-")
}
