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

//! Coaly server properties.

use std::fmt::{Debug, Formatter};
use crate::coalyxw;
use crate::config::{int_par, not_table_item, read_app_ids, size_par, str_par};
use crate::config::toml::document::TomlValueItem;
use crate::errorhandling::*;

// Default value and range for maximum number of connections
pub const DEF_MAX_CXNS: usize = 10;
pub const MIN_MAX_CXNS: usize = 0;
pub const MAX_MAX_CXNS: usize = 4096;

// Default value and range for time to consider connections active after last message
pub const DEF_KEEP_CXN: usize = 86400;
pub const MIN_KEEP_CXN: usize = 0;
pub const MAX_KEEP_CXN: usize = 86400 * 30;

// Default value and range for maximum message size
pub const DEF_MAX_MSG_SIZE: usize = 65536;
pub const MIN_MAX_MSG_SIZE: usize = 128;
pub const MAX_MAX_MSG_SIZE: usize = u32::MAX as usize;

/// Coaly server properties.
/// All properties are specified under TOML table server in the custom configuration file.
#[derive (Clone)]
pub struct ServerProperties {
    // local network address to listen for client records, mandatory
    data_listen_address: String,
    // local network address to listen for administrative commands, defaults to empty string
    admin_listen_address: String,
    // maximum number of client connections, defaults to 10
    max_connections: usize,
    // time span in seconds to consider a connection active after last message, defaults to one day
    keep_connection: u32,
    // maximum message size in bytes, defaults to 64 KByte
    max_msg_size: usize,
    // secret key for administrative commands, defaults to empty string
    admin_key: String,
    // list of allowed clients to send records (IP address, application IDs),
    // defaults to any application ID on local host [("127.0.0.1", [0]),("[::1]", [0])]
    data_clients: Vec<(String, Vec<u32>)>,
    // list of allowed clients to administer the server (IP address),
    // defaults to any port on local host ["127.0.0.1:0","[::1]:0"]
    admin_clients: Vec<String>
}
impl ServerProperties {
    /// Returns the local network address to be used as listen address for
    /// log or trace records in a trace server.
    #[inline]
    pub fn data_listen_address(&self) -> &String { &self.data_listen_address }

    /// Sets the local network address to be used as listen address for
    /// administrative commands in a trace server.
    /// * hostname:port - for IPv4, port number is optional and defaults to 0
    /// * n.n.n.n:port - for IPv4, port number is optional and defaults to 0
    /// * [hostname]:port - for IPv6, port number is optional and defaults to 0
    /// * [n:...:n]:port - for IPv6, port number is optional and defaults to 0
    /// 
    /// # Arguments
    /// * `addr` - the local network address
    #[inline]
    pub fn set_data_listen_address(&mut self, addr: &str) {
        self.data_listen_address = addr.to_string()
    }

    /// Returns the local network address to be used as listen address for
    /// administrative commands in a trace server.
    #[inline]
    pub fn admin_listen_address(&self) -> &String { &self.admin_listen_address }

    /// Sets the local network address to be used as listen address for
    /// administrative commands in a trace server.
    /// * hostname:port - for IPv4, port number is optional and defaults to 0
    /// * n.n.n.n:port - for IPv4, port number is optional and defaults to 0
    /// * [hostname]:port - for IPv6, port number is optional and defaults to 0
    /// * [n:...:n]:port - for IPv6, port number is optional and defaults to 0
    /// 
    /// # Arguments
    /// * `addr` - the local network address
    #[inline]
    pub fn set_admin_listen_address(&mut self, addr: &str) {
        self.admin_listen_address = addr.to_string()
    }

    /// Returns the maximum number of client connections
    #[inline]
    pub fn max_connections(&self) -> usize { self.max_connections }

    /// Sets the maximum number of client connections
    #[inline]
    pub fn set_max_connections(&mut self, conns: usize) { self.max_connections = conns; }

    /// Returns the time to consider a connection active after last message
    #[inline]
    pub fn keep_connection(&self) -> u32 { self.keep_connection }

    /// Sets the time to consider a connection active after last message
    #[inline]
    pub fn set_keep_connection(&mut self, seconds: u32) { self.keep_connection = seconds; }

    /// Returns the maximum message size
    #[inline]
    pub fn max_msg_size(&self) -> usize { self.max_msg_size }

    /// Sets the maximum message size
    #[inline]
    pub fn set_max_msg_size(&mut self, size: usize) { self.max_msg_size = size; }

    /// Returns the secret key to shutdown trace server
    #[inline]
    pub fn admin_key(&self) -> &String { &self.admin_key }

    /// Sets the secret key to shutdown trace server
    #[inline]
    pub fn set_admin_key(&mut self, key: &str) { self.admin_key = key.to_string() }

    /// Returns the clients allowed to access the trace server
    #[inline]
    pub fn data_clients(&self) -> &[(String, Vec<u32>)] { &self.data_clients }

    /// Adds a client allowed to access the trace server
    #[inline]
    pub fn add_data_client(&mut self, addr: &str, app_ids: &[u32]) {
        self.data_clients.push((addr.to_string(), app_ids.to_vec()));
    }

    /// Removes all currently allowed trace clients
    #[inline]
    pub fn clear_data_clients(&mut self) { self.data_clients.clear(); }

    /// Returns the clients allowed to administer the trace server
    #[inline]
    pub fn admin_clients(&self) -> &[String] { &self.admin_clients }

    /// Adds a client allowed to administer the trace server
    #[inline]
    pub fn add_admin_client(&mut self, addr: &str) {
        self.admin_clients.push(addr.to_string());
    }

    /// Removes all currently allowed admin clients
    #[inline]
    pub fn clear_admin_clients(&mut self) { self.admin_clients.clear(); }
}
impl Default for ServerProperties {
    fn default() -> Self {
        let mut dcls = Vec::<(String, Vec<u32>)>::with_capacity(4);
        dcls.push((String::from("127.0.0.1"), vec!(0)));
        dcls.push((String::from("[::1]"), vec!(0)));
        let mut acls = Vec::<String>::with_capacity(4);
        acls.push(String::from("127.0.0.1:0"));
        acls.push(String::from("[::1]:0"));
        ServerProperties {
            data_listen_address: String::from(""),
            admin_listen_address: String::from(""),
            max_connections: DEF_MAX_CXNS,
            keep_connection: DEF_KEEP_CXN as u32,
            max_msg_size: DEF_MAX_MSG_SIZE,
            admin_key: String::from(""),
            data_clients: dcls,
            admin_clients: acls
        }
    }
}
impl Debug for ServerProperties {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // data clients
        let mut dcl_buf = String::with_capacity(512);
        let mut aid_buf = String::with_capacity(128);
        dcl_buf.push('[');
        for (addr, app_ids) in &self.data_clients {
            if dcl_buf.len() > 1 { dcl_buf.push(','); }
            dcl_buf.push_str(&format!("(ADDR:{},IDS:[", addr));
            aid_buf.clear();
            for app_id in app_ids {
                if aid_buf.len() > 1 { aid_buf.push(','); }
                aid_buf.push_str(&app_id.to_string());
            }
            dcl_buf.push_str(&aid_buf);
            dcl_buf.push(']');
            dcl_buf.push(')');
        }
        dcl_buf.push(']');
        // admin clients
        let mut acl_buf = String::with_capacity(512);
        acl_buf.push('[');
        for addr in &self.admin_clients {
            if acl_buf.len() > 1 { acl_buf.push(','); }
            acl_buf.push_str(addr);
        }
        acl_buf.push(']');
        write!(f,
               "DLA:{}/ALA:{}/MCX:{}/KCX:{}/MMS:{}/KEY:{}/DCL:{}/ACL:{}",
               self.data_listen_address, self.admin_listen_address, self.max_connections,
               self.keep_connection, self.max_msg_size, self.admin_key, dcl_buf, acl_buf)
    }
}

/// Reads server properties specifications from the custom configuration file.
/// 
/// # Arguments
/// * `srv_item` - the value item for the server settings in the custom TOML document
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// the custom server properties read, **None** if no valid property has been found
pub(crate) fn read_server_properties(srv_item: &TomlValueItem,
                                     msgs: &mut Vec<CoalyException>) -> Option<ServerProperties> {
    if not_table_item(srv_item, TOML_GRP_SERVER, None, msgs) { return None }
    let mut sp = ServerProperties::default();
    for (srv_key, srv_val) in srv_item.child_items().unwrap() {
        match srv_key.as_str() {
            TOML_PAR_DATA_ADDR => {
                if str_par(srv_val, srv_key, TOML_GRP_SERVER, msgs) {
                    sp.set_data_listen_address(&srv_val.value().as_str().unwrap());
                }
            },
            TOML_PAR_ADMIN_ADDR => {
                if str_par(srv_val, srv_key, TOML_GRP_SERVER, msgs) {
                    sp.set_admin_listen_address(&srv_val.value().as_str().unwrap());
                }
            },
            TOML_PAR_MAX_CONNECTIONS => {
                if int_par(srv_val, srv_key, TOML_GRP_SERVER,
                           MIN_MAX_CXNS, MAX_MAX_CXNS, DEF_MAX_CXNS, msgs) {
                    sp.set_max_connections(srv_val.value().as_integer().unwrap() as usize);
                }
            },
            TOML_PAR_KEEP_CONNECTION => {
                if int_par(srv_val, srv_key, TOML_GRP_SERVER,
                           MIN_KEEP_CXN, MAX_KEEP_CXN, DEF_KEEP_CXN, msgs) {
                    sp.set_keep_connection(srv_val.value().as_integer().unwrap() as u32);
                }
            },
            TOML_PAR_MAX_MSG_SIZE => {
                if let Some(msize) = size_par(srv_val, srv_key, TOML_GRP_SERVER,
                                              MIN_MAX_MSG_SIZE, MAX_MAX_MSG_SIZE,
                                              DEF_MAX_MSG_SIZE, msgs) {
                    sp.set_max_msg_size(msize as usize);
                }
            },
            TOML_PAR_ADMIN_KEY => {
                if str_par(srv_val, srv_key, TOML_GRP_SERVER, msgs) {
                    sp.set_admin_key(&srv_val.value().as_str().unwrap());
                }
            },
            TOML_GRP_DATA_CLIENTS => {
                let full_clients_key = format!("{}.{}", TOML_GRP_SERVER, srv_key);
                read_allowed_data_clients(srv_val, &full_clients_key, &mut sp, msgs);
            },
            TOML_PAR_ADMIN_CLIENTS => {
                let full_clients_key = format!("{}.{}", TOML_GRP_SERVER, srv_key);
                read_allowed_admin_clients(srv_val, &full_clients_key, &mut sp, msgs);
            },
            _ => {
                let full_key = format!("{}.{}", TOML_GRP_SERVER, srv_key);
                msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, srv_val.line_nr(), full_key));
            }
        }
    }
    Some(sp)
}

/// Reads clients allowed to send data to a trace server from custom configuration.
/// 
/// # Arguments
/// * `clients_item` - the value item for the clients
/// * `clients_full_key` - the full name of the clients TOML item
/// * `sys_props` - the system properties where to store the data parsed
/// * `msgs` - the array, where error messages shall be stored
fn read_allowed_data_clients(clients_item: &TomlValueItem,
                             clients_full_key: &str,
                             srv_props: &mut ServerProperties,
                             msgs: &mut Vec<CoalyException>) {
    if let Some(clients) = clients_item.child_values() {
        srv_props.clear_data_clients();
        for client in clients {
            if let Some(client_attrs) = client.child_items() {
                let mut source_addr = String::from("127.0.0.1");
                let mut app_ids = vec!();
                for (attr_key, attr_val) in client_attrs {
                    match attr_key.as_str() {
                        TOML_PAR_SOURCE => {
                            if str_par(attr_val, attr_key, clients_full_key, msgs) {
                                source_addr = attr_val.value().as_str().unwrap();
                            }
                        },
                        TOML_PAR_APP_IDS => {
                            app_ids = read_app_ids(attr_val, attr_key, msgs);
                        },
                        _ => {
                            let full_key = format!("{}.{}", clients_full_key, attr_key);
                            msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, attr_val.line_nr(), full_key));
                        }
                    }
                }
                srv_props.add_data_client(&source_addr, &app_ids);
                continue;
            }
            msgs.push(coalyxw!(W_CFG_KEY_NOT_A_TABLE,client.line_nr(),clients_full_key.to_string()));
        }
        return
    }
    msgs.push(coalyxw!(W_CFG_KEY_NOT_AN_ARRAY, clients_item.line_nr(),
                     TOML_GRP_DATA_CLIENTS.to_string()));
}

/// Reads clients allowed to send data to a trace server from custom configuration.
/// 
/// # Arguments
/// * `clients_item` - the value item for the clients
/// * `clients_full_key` - the full name of the clients TOML item
/// * `srv_props` - the server properties where to store the data parsed
/// * `msgs` - the array, where error messages shall be stored
fn read_allowed_admin_clients(clients_item: &TomlValueItem,
                              clients_full_key: &str,
                              srv_props: &mut ServerProperties,
                              msgs: &mut Vec<CoalyException>) {
    if let Some(addrs) = clients_item.child_values() {
        srv_props.clear_admin_clients();
        for addr_item in addrs {
            if str_par(addr_item, "", clients_full_key, msgs) {
                let addr = addr_item.value().as_str().unwrap();
                srv_props.add_admin_client(&addr);
            }
        }
        return
    }
    msgs.push(coalyxw!(W_CFG_KEY_NOT_AN_ARRAY, clients_item.line_nr(),
                     TOML_PAR_ADMIN_CLIENTS.to_string()));
}

const TOML_GRP_DATA_CLIENTS: &str = "data_clients";
const TOML_GRP_SERVER: &str = "server";

const TOML_PAR_ADMIN_ADDR: &str = "admin_addr";
const TOML_PAR_ADMIN_CLIENTS: &str = "admin_clients";
const TOML_PAR_ADMIN_KEY: &str = "admin_key";
const TOML_PAR_APP_IDS: &str = "app_ids";
const TOML_PAR_DATA_ADDR: &str = "data_addr";
const TOML_PAR_KEEP_CONNECTION: &str = "keep_connection";
const TOML_PAR_MAX_CONNECTIONS: &str = "max_connections";
const TOML_PAR_MAX_MSG_SIZE: &str = "max_msg_size";
const TOML_PAR_SOURCE: &str = "source";
