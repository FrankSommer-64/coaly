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
use crate::net::serverproperties::ServerProperties;
use crate::errorhandling::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::broadcast::*;

use super::{NetworkProtocol, parse_url, is_valid_url};
use super::clientwhitelist::ClientWhitelist;
use super::tcp::{tcp_admin_listener, tcp_record_listener};
use super::udp::{UdpAdminHandler, UdpRecordHandler};


pub struct TraceServer {
    properties: ServerProperties,
    shutdown_ch_tx: Sender<bool>,
    shutdown_ch_rx: Receiver<bool>,
    is_running: AtomicBool
}
impl TraceServer {
    /// Creates a log and trace server from the specified configuration file.
    /// Use this constructor if the server shall also initialize Coaly.
    ///
    /// # Arguments
    /// * `file_name` - the name of the configuration file
    ///
    /// # Return values
    /// The server structure upon success.
    ///
    /// # Errors
    /// Returns an error structure if the connfiguration file could not be found or in case of an
    /// erroneous specification
    pub fn from_config_file(file_name: &str) -> Result<TraceServer, CoalyException> {
        // check whether configuration file exists
        if ! std::path::Path::new(file_name).exists() {
            return Err(coalyxe!(E_FILE_NOT_FOUND, file_name.to_string()));
        }
        // parse configuration file
        let oinfo = util::originator_info();
        let cfg = config::configuration(&oinfo, Some(file_name));
        // check server properties
        match cfg.server_properties() {
            Some(ref srv_props) => {
                let data_listen_addr = srv_props.data_listen_address();
                if ! is_valid_url(data_listen_addr) {
                    return Err(coalyxe!(E_SRV_INV_DATA_ADDR_IN_FILE,
                                      data_listen_addr.clone(), file_name.to_string()))
                }
                initialize(file_name);
                let (shutdown_ch_tx, shutdown_ch_rx) = channel::<bool>(1);
                Ok(TraceServer { properties: srv_props.clone(),
                                 shutdown_ch_tx,
                                 shutdown_ch_rx,
                                 is_running: AtomicBool::new(false) } )
            },
            None => Err(coalyxe!(E_SRV_PROPS_MISSING, file_name.to_string()))
        }
    }

    /// Creates a log and trace server from the specified properties.
    /// Use this constructor if the process has already initialized Coaly.
    ///
    /// # Arguments
    /// * `properties` - the server properties from the configuration file
    ///
    /// # Return values
    /// The server structure upon success.
    ///
    /// # Errors
    /// Returns an error structure if the connfiguration file could not be found or in case of an
    /// erroneous specification
    pub fn from_properties(properties: &ServerProperties) -> Result<TraceServer, CoalyException> {
        let data_listen_addr = properties.data_listen_address();
        if ! is_valid_url(data_listen_addr) {
            return Err(coalyxe!(E_SRV_INV_DATA_ADDR, data_listen_addr.clone()));
        }
        let (shutdown_ch_tx, shutdown_ch_rx) = channel::<bool>(1);
        Ok(TraceServer { properties: properties.clone(),
                         shutdown_ch_tx,
                         shutdown_ch_rx,
                         is_running: AtomicBool::new(false) } )
    }

    /// Runs the log and trace server.
    /// Terminates, if a shutdown message has been sent to the administrative network port, or
    /// method terminate has been called.
    ///
    /// # Arguments
    /// * `handle_ctrlc` - **true**, if a handler to detect CTRL-C shall be installed
    /// * `handle_term` - **true**, if a handler to detect termination from shell command
    ///                   shall be installed
    pub async fn run(&mut self,
                     handle_ctrlc: bool,
                     handle_term: bool) {
        // if server is already running, there's nothing to do
        if self.is_running.compare_exchange(false, true,
                                            Ordering::Relaxed, Ordering::Relaxed).is_err() {
            return
        }
        // install termination detection handlers, if specified
        if handle_ctrlc {
            let bc_tx = self.shutdown_ch_tx.clone();
            let bc_rx = self.shutdown_ch_tx.subscribe();
            tokio::spawn(async move { detect_ctrlc(bc_tx, bc_rx).await; });
        }
        if handle_term {
            let bc_tx = self.shutdown_ch_tx.clone();
            let bc_rx = self.shutdown_ch_tx.subscribe();
            tokio::spawn(async move { detect_term(bc_tx, bc_rx).await; });
        }
        // install handler for administrative commands over the network, if specified in the
        // server properties
        self.install_admin_handler().await;
        // install handler for log and trace records from the network
        self.install_data_handler().await;
        
        // wait for termination event
        let _ = self.shutdown_ch_rx.recv().await;
    }

    /// Terminates the log and trace server.
    /// Invoke this function, if CTRL-C or a termination signal has been detected.
    pub fn terminate(&mut self) {
    }

    /// Installs a handler for administrative commands, if a valid address is specified in the
    /// server properties.
    async fn install_admin_handler(&mut self) {
        if let Ok(listen_addr) = parse_url(self.properties.admin_listen_address()) {
            let prot = listen_addr.protocol();
            let bc_rx = self.shutdown_ch_tx.subscribe();
            let bc_tx = self.shutdown_ch_tx.clone();
            let adm_key = self.properties.admin_key().to_string();
            let allowed_ips = self.properties.admin_clients().to_vec();
            let client_whitelist = ClientWhitelist::from_ip(&allowed_ips);
            match prot {
                NetworkProtocol::Udp => {
                    let listen_addr = listen_addr.ip_addr().unwrap();
                    if let Ok(sock) = UdpSocket::bind(&listen_addr).await {
                        let mut adm_handler = UdpAdminHandler::new(sock);
                        tokio::spawn(async move { adm_handler.run(adm_key, &client_whitelist,
                                                                  bc_tx, bc_rx).await; });
                    }
                },
                NetworkProtocol::Tcp => {
                    let listen_addr = listen_addr.ip_addr().unwrap();
                    if let Ok(sock) = TcpListener::bind(&listen_addr).await {
                        tokio::spawn(async move {
                            tcp_admin_listener(sock, adm_key, &client_whitelist,
                                               bc_tx, bc_rx).await;
                        });
                    }
                },
                #[cfg(unix)]
                NetworkProtocol::Unix => {
                    // TODO
                }
            }
        }
    }

    /// Installs a handler for log and trace records sent over the network.
    async fn install_data_handler(&mut self) {
        let listen_addr = parse_url(self.properties.data_listen_address()).unwrap();
        let prot = listen_addr.protocol();
        let max_conns = self.properties.max_connections();
        let max_msg_size = self.properties.max_msg_size();
        let keep_time = self.properties.keep_connection();
        let allowed_ips = self.properties.data_clients();
        let client_whitelist = ClientWhitelist::from_ip_and_app_ids(allowed_ips);
        let bc_tx = self.shutdown_ch_tx.clone();
        let bc_rx = self.shutdown_ch_tx.subscribe();
        match prot {
            NetworkProtocol::Udp => {
                let listen_addr = listen_addr.ip_addr().unwrap();
                if let Ok(sock) = UdpSocket::bind(&listen_addr).await {
                    let mut rec_handler = UdpRecordHandler::new(sock, client_whitelist,
                                                                bc_tx, bc_rx, max_msg_size);
                    tokio::spawn(async move { rec_handler.run(max_conns, keep_time).await; });
                }
            },
            NetworkProtocol::Tcp => {
                let listen_addr = listen_addr.ip_addr().unwrap();
                if let Ok(sock) = TcpListener::bind(&listen_addr).await {
                    tokio::spawn(async move {
                        tcp_record_listener(sock, max_conns, max_msg_size, &client_whitelist,
                                            bc_tx, bc_rx).await;
                    });
                }
            },
            #[cfg(unix)]
            NetworkProtocol::Unix => {
                // TODO
            }
        }
    }
}


/// Handler to detect CTRL-C from terminal.
/// 
/// # Arguments
/// * `tx_channel` - the sender side of the broadcast channel, used to indicate that CTRL-C has
///                  been detected
/// * `rx_channel` - the receiver side of the broadcast channel, used to terminate this handler,
///                  if the server will shutdown because of another event
#[cfg(unix)]
async fn detect_ctrlc(tx_channel: Sender<bool>, mut rx_channel: Receiver<bool>) {
    if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
        tokio::select! {
            _ = sig.recv() => { let _ = tx_channel.send(true); }
            _ = rx_channel.recv() => {}
        }
    }
}

/// Handler to detect SIGTERM signal, usually from kill command.
/// 
/// # Arguments
/// * `tx_channel` - the sender side of the broadcast channel, used to indicate that CTRL-C has
///                  been detected
/// * `rx_channel` - the receiver side of the broadcast channel, used to terminate this handler,
///                  if the server will shutdown because of another event
#[cfg(unix)]
async fn detect_term(tx_channel: Sender<bool>, mut rx_channel: Receiver<bool>) {
    if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        tokio::select! {
            _ = sig.recv() => { let _ = tx_channel.send(true); }
            _ = rx_channel.recv() => {}
        }
    }
}
