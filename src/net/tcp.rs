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

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::*;


/// TCP listener function to handle incoming connections for administrative messages
/// The handler terminates upon the following events:
/// - a shutdown message was received
/// - a socket I/O error ocurred
/// - a shutdown was signaled from another part of the server
pub(super) async fn tcp_admin_listener(socket: TcpListener,
                                       adm_key: String,
                                       client_whitelist: &ClientWhitelist,
                                       shutdown_sender: Sender<bool>,
                                       mut shutdown_listener: Receiver<bool>) {
    loginfo!("Started TCP admin listener waiting for connections on address {}",
             local_listener_addr_of(&socket));
    loop {
        tokio::select! {
            accept_res = socket.accept() => {
                match accept_res {
                    Ok((sock, addr)) => {
                        if ! client_whitelist.allows_addr(&addr) {
                            drop(sock);
                            loginfo!("Rejected admin access, client {} not allowed", addr);
                            continue;
                        }
                        let mut handler = TcpAdminHandler::new(sock);
                        tokio::spawn(async move {
                            handler.run(addr, &adm_key, shutdown_sender.clone(),
                                        shutdown_sender.subscribe()).await;
                        });
                    },
                    Err(e) => {
                        logerror!("Terminating TCP admin listener, accept on admin listen socket failed: {}", e);
                        drop(socket);
                    }
                }
            }
            _ = shutdown_listener.recv() => { drop(socket); }
        }
        break
    }
}

/// TCP listener function to handle incoming connections for log or trace messages
/// The handler terminates upon the following events:
/// - a socket I/O error ocurred
/// - a shutdown was signaled from another part of the server
pub(super) async fn tcp_record_listener(socket: TcpListener,
                                        max_conns: usize,
                                        max_msg_size: usize,
                                        client_whitelist: &ClientWhitelist,
                                        shutdown_sender: Sender<bool>,
                                        mut shutdown_listener: Receiver<bool>) {
    loginfo!("Started TCP record listener waiting for connections on address {}",
             local_listener_addr_of(&socket));
    let mut conn_table = ClientConnectionTable::new(max_conns, u32::MAX);
    let mut rx_buf = ReceiveBuffer::new(PROTOCOL_VERSION as u32, 1024);
    loop {
        tokio::select! {
            accept_res = socket.accept() => {
                match accept_res {
                    Ok((mut sock, addr)) => {
                        tokio::select! {
                            maybe_msg = sock.read(rx_buf.as_mut_slice()) => {
                                match maybe_msg {
                                    Ok(n) => {
                                        match rx_buf.message(n) {
                                            Ok(Message::ClientNotification(client)) => {
                                                let app_id = client.application_id_value();
                                                if ! client_whitelist.allows_addr_and_appid(&addr, app_id) {
                                                    loginfo!("Access for client {} with app ID {} denied", addr, app_id);
                                                    drop(sock);
                                                    continue;
                                                }
                                                if ! conn_table.add(&addr, &client, false) {
                                                    loginfo!("Connection limit exceeded, could not accept client {}", addr);
                                                    drop(sock);
                                                    continue;
                                                }
                                                loginfo!("Client {} with app ID {} accepted", addr, app_id);
                                                agent::remote_client_connected(&addr, client);
                                                let mut handler = TcpRecordHandler::new(max_msg_size);
                                                tokio::spawn(async move {
                                                    handler.run(sock, addr, shutdown_sender.subscribe()).await;
                                                    conn_table.remove(&addr);
                                                });
                                            },
                                            Ok(_) => {
                                                loginfo!("Client {} did not send connect message first", addr);
                                                drop(sock);
                                                continue
                                            },
                                            Err(_) => {
                                                loginfo!("Failed to read connect message from client {}", addr);
                                                drop(sock);
                                                continue
                                            }
                                        }
                                    },
                                    Err(_) => {
                                        loginfo!("Failed to read inital data from client {}", addr);
                                        drop(sock);
                                        continue
                                    }
                                }
                            }
                            _ = shutdown_listener.recv() => { drop(socket); }
                        }
                    },
                    Err(e) => {
                        logerror!("I/O error reading from listen socket: {}, terminating server", e);
                        drop(socket);
                        let _ = shutdown_sender.send(true);
                        return
                    }
                }
            },
            _ = shutdown_listener.recv() => { drop(socket); }
        }
        break
    }
}

/// Handler for administrative messages sent to Coaly logging server via TCP.
pub(super) struct TcpAdminHandler {
    // TCP socket to use for communication
    socket: TcpStream,
    // receive buffer for incoming messages
    rx_buf: ReceiveBuffer,
    // send buffer for outgoing messages
    tx_buf: SendBuffer,
}
impl TcpAdminHandler {
    /// Creates a TCP admin handler on the socket supplied.
    pub(super) fn new(socket: TcpStream) -> TcpAdminHandler  {
        TcpAdminHandler {
            socket,
            rx_buf: ReceiveBuffer::new(PROTOCOL_VERSION as u32, 128),
            tx_buf: SendBuffer::new(PROTOCOL_VERSION as u32, 128)
        }
    }

    /// Runs the TCP admin handler.
    /// The handler terminates upon the following events:
    /// - a shutdown message was received
    /// - a socket I/O error ocurred
    /// - a shutdown was signaled from another part of the server
    pub(super) async fn run(&mut self,
                            client_addr: SocketAddr,
                            adm_key: &str,
                            shutdown_sender: Sender<bool>,
                            mut shutdown_listener: Receiver<bool>) {
        loginfo!("Started TCP admin handler waiting for messages on address {}",
                 local_addr_of(&self.socket));
        loop {
            tokio::select! {
                maybe_shutdown_msg = self.socket.read(self.rx_buf.as_mut_slice()) => {
                    match maybe_shutdown_msg {
                        Ok(n) => {
                            if let Ok(Message::ShutdownRequest(req)) = self.rx_buf.message(n) {
                                // client address already checked by listener
                                if req != *adm_key {
                                    loginfo!("Rejected shutdown message from {}, invalid key",
                                             client_addr);
                                    continue;
                                }
                                loginfo!("Accepted shutdown message from {}", client_addr);
                                self.tx_buf.store_shutdown_response();
                                let _ = self.socket.write(self.tx_buf.as_slice()).await;
                                let _ = shutdown_sender.send(true);
                                return;
                            }
                            loginfo!("Ignored invalid admin message from {}", client_addr);
                            continue;
                        },
                        Err(io_err) => {
                            logwarn!("Terminated TCP admin handler due to socket error: {}",
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

/// Handler for log and trace messages sent to Coaly logging server via TCP.
pub(super) struct TcpRecordHandler {
    // receive buffer for incoming messages
    rx_buf: ReceiveBuffer
}
impl TcpRecordHandler {
    /// Creates a TCP admin handler on the socket supplied.
    pub(super) fn new(max_msg_size: usize) -> TcpRecordHandler  {
        TcpRecordHandler {
            rx_buf: ReceiveBuffer::new(PROTOCOL_VERSION as u32, max_msg_size)
        }
    }

    /// Runs the TCP admin handler.
    /// The handler terminates upon the following events:
    /// - a shutdown message was received
    /// - a socket I/O error ocurred
    /// - a shutdown was signaled from another part of the server
    pub(super) async fn run(&mut self,
                            mut socket: TcpStream,
                            client_addr: SocketAddr,
                            mut shutdown_listener: Receiver<bool>) {
        loginfo!("Started TCP record handler waiting for messages on address {}",
                 local_addr_of(&socket));
        loop {
            tokio::select! {
                maybe_msg = socket.read(self.rx_buf.as_mut_slice()) => {
                    match maybe_msg {
                        Ok(n) => {
                            match self.rx_buf.message(n) {
                                Ok(msg) => {
                                    match msg {
                                        Message::RecordNotification(rec) => {
                                            agent::write_rec(&client_addr, rec);
                                        },
                                        Message::DisconnectNotification => {
                                            loginfo!("Client {} disconnected", client_addr);
                                            agent::remote_client_disconnected(&client_addr);
                                        },
                                        _ =>  {
                                            loginfo!("Ignored unexpected message {:?} from {}", msg, client_addr);
                                        }
                                    }
                                },
                                Err(e) => {
                                    logerror!("Error receiving message: {}", e.localized_message());
                                }
                            }
                        },
                        Err(e) => {
                            logerror!("Error reading from TCP socket: {}, terminating TCP record handler", e);
                            return
                        }
                    }
                    continue;
                }
                _ = shutdown_listener.recv() => { return }
            }
        }
    }
}

#[inline]
fn local_addr_of(socket: &TcpStream) -> String {
    if let Ok(addr) = socket.local_addr() { return addr.to_string() }
    String::from("-unknown-")
}

#[inline]
fn local_listener_addr_of(socket: &TcpListener) -> String {
    if let Ok(addr) = socket.local_addr() { return addr.to_string() }
    String::from("-unknown-")
}
