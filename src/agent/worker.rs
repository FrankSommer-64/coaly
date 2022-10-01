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

//! Worker thread handling all events in the local Coaly agent.

use chrono::{DateTime, Local};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use crate::coalyxw;
use crate::errorhandling::*;
use crate::event::CoalyEvent;
use crate::modechange::{ModeChangeDescList, OverrideModeMap};
use crate::output::inventory::Inventory;
use crate::output::standaloneinventory::StandaloneInventory;
use crate::record::{RecordLevelId, RecordTrigger};
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::{LocalRecordData, RecordData};
use crate::util;
use super::threadstatus::{ThreadStatus, ThreadStatusTable};
use super::config;

#[cfg(feature="net")]
use std::collections::HashMap;

#[cfg(feature="net")]
use std::net::SocketAddr;

#[cfg(feature="net")]
use crate::output::Interface;

#[cfg(feature="net")]
use crate::output::serverinventory::ServerInventory;

#[cfg(feature="net")]
use crate::record::recorddata::RemoteRecordData;

/// Starts background thread for record processing.
/// 
/// # Arguments
/// * `rx_channel` - receiver end of communication channel between client threads and worker
/// 
/// # Return values
/// the join handle of the created worker thread
pub(crate) fn spawn(rx_channel: Receiver<CoalyEvent>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut worker = Worker::new();
        let launch_instant = Instant::now();
        let mut last_rollover_check = launch_instant.elapsed().as_secs();
        loop {
            let rx_res = rx_channel.recv_timeout(Duration::from_secs(1));
            let now = Local::now();
            match rx_res {
                Ok(event) => {
                    match event {
                        CoalyEvent::LocalRecord(record) => {
                            let app_duration = launch_instant.elapsed().as_secs();
                            worker.handle_local_record_event(record);
                            if app_duration > last_rollover_check {
                                last_rollover_check = app_duration;
                                worker.handle_timer_event(&now);
                            }
                        },
                        #[cfg(feature="net")]
                        CoalyEvent::RemoteRecord((client_addr, record)) => {
                            let app_duration = launch_instant.elapsed().as_secs();
                            worker.handle_remote_record_event(client_addr, record);
                            if app_duration > last_rollover_check {
                                last_rollover_check = app_duration;
                                worker.handle_timer_event(&now);
                            }
                        },
                        CoalyEvent::Config(cfg_fn) => {
                            worker.handle_config_event(&cfg_fn);
                        },
                        #[cfg(feature="net")]
                        CoalyEvent::RemoteClientConnected((addr, orig_info)) => {
                            worker.handle_client_connected_event(addr, orig_info);
                        },
                        #[cfg(feature="net")]
                        CoalyEvent::RemoteClientDisconnected(addr) => {
                            worker.handle_client_disconnected_event(addr);
                        },
                        CoalyEvent::Shutdown => {
                            worker.handle_shutdown_event();
                            break
                        }
                    }
                },
                Err(cause) => {
                    match cause {
                        RecvTimeoutError::Timeout => {
                            last_rollover_check = launch_instant.elapsed().as_secs();
                            worker.handle_timer_event(&now);
                        },
                        _ => break
                    }
                }
            }
        }
    })
}

/// Holds all administrative data needed by the background worker thread.
struct Worker {
    // configuration from configuration file or defaults
    configuration: Option<Rc<config::Configuration>>,
    // the runtime states of all client threads having accessed Coaly
    thread_states: ThreadStatusTable,
    // information about application and local host
    originator: OriginatorInfo,
    // resource manager
    res_inventory: Option<Box<dyn Inventory>>,
    // map for global output mode
    mode_map: OverrideModeMap,
    // information about remote clients
    #[cfg(feature="net")]
    remote_clients: HashMap<SocketAddr, HashMap<u64, Interface>>,
}
impl Worker {
    /// Creates administrative data structure for background worker thread.
    pub fn new() -> Worker {
        Worker {
            configuration: None,
            thread_states: ThreadStatusTable::new(),
            originator: util::originator_info(),
            res_inventory: None,
            mode_map: OverrideModeMap::new(4096),
            #[cfg(feature="net")]
            remote_clients: HashMap::new()
        }
    }

    /// Handles a record event from a client thread.
    /// The event is processed as follows:
    /// * Eventually change the output settings, if the event was triggered by a structure
    ///   creation or drop
    /// * determine the appropriate output settings for the event
    /// * format the record according to the configured record format
    /// * write the formatted record to the configured output resource
    /// 
    /// # Arguments
    /// * `record` - the record data
    pub fn handle_local_record_event(&mut self, record: LocalRecordData) {
        if self.configuration.is_none() {
            // no need to update originator info here, since default config doesn't use
            // environment variables
            self.configuration = Some(config::configuration(&self.originator, None));
        }
        let cnf = &self.configuration.as_ref().unwrap().clone();
        if self.res_inventory.is_none() {
            self.res_inventory = Some(StandaloneInventory::new(cnf, &self.originator));
        }
        let inv = self.res_inventory.as_mut().unwrap();
        let tid = record.thread_id();
        let tname = record.thread_name();
        let ts =
            self.thread_states.entry(tid)
                .or_insert_with(|| ThreadStatus::new(inv.local_thread_interface(tid, tname),
                                                     cnf));
        let current_mode = determine_mode(&mut self.mode_map, ts, cnf.mode_changes(), &record);
        if record.level() as u32 & current_mode == 0 { return }
        let use_buffering = (record.level() as u32) & (current_mode >> 16) != 0;
        if let Err(m) = ts.output_interface.write(&record, use_buffering) { log_problems(&m); }
    }

    /// Handles a record event from a client thread.
    /// The event is processed as follows:
    /// * Eventually change the output settings, if the event was triggered by a structure
    ///   creation or drop
    /// * determine the appropriate output settings for the event
    /// * format the record according to the configured record format
    /// * write the formatted record to the configured output resource
    /// 
    /// # Arguments
    /// * `record` - the record data
    #[cfg(feature="net")]
    pub fn handle_remote_record_event(&mut self,
                                      client_addr: SocketAddr,
                                      record: RemoteRecordData) {
        if let Some(client_info) = self.remote_clients.get_mut(&client_addr) {
            let tid = record.thread_id();
            let tname = record.thread_name();
            let thread_if = client_info.entry(tid)
                                       .or_insert_with(|| self.res_inventory
                                                              .as_mut()
                                                              .unwrap()
                                                              .remote_thread_interface(&client_addr,
                                                                                       tid, tname));
            if let Err(m) = thread_if.write(&record, false) { log_problems(&m); }
        }
        // ignore records from unconnected clients
    }

    /// Handles a configuration event from a client thread.
    /// Parses the specified configuration file and creates the corresponding structures.
    /// The caller must make sure that this function is invoked only once.
    /// Uses default configuration if an error is encountered during configuration file processing.
    /// 
    /// # Arguments
    /// * `config_file_name` - the name of the configuration file
    #[cfg(not(feature="net"))]
    pub fn handle_config_event(&mut self,
                               config_file_name: &str) {
        if self.res_inventory.is_none() {
            let cnf = config::configuration(&self.originator, Some(config_file_name));
            self.originator.set_application_id(cnf.system_properties().application_id());
            self.originator.set_application_name(cnf.system_properties().application_name());
            for ev_name in cnf.referenced_env_vars() {
                if let Ok(ev_val) = std::env::var(&ev_name) {
                    self.originator.add_env_var(&ev_name, &ev_val);
                }
            }
            let msgs = cnf.messages();
            if ! msgs.is_empty() {
                let header_msg = coalyxw!(E_CFG_FOUND_ISSUES, config_file_name.to_string());
                let mut emsgs = msgs.clone();
                emsgs.insert(0, header_msg);
                log_problems(&emsgs);
            }
            self.res_inventory = Some(StandaloneInventory::new(&cnf, &self.originator));
            self.configuration = Some(cnf);
        };
    }

    /// Handles a configuration event from a client thread.
    /// Parses the specified configuration file and creates the corresponding structures.
    /// The caller must make sure that this function is invoked only once.
    /// Uses default configuration if an error is encountered during configuration file processing.
    /// 
    /// # Arguments
    /// * `config_file_name` - the name of the configuration file
    #[cfg(feature="net")]
    pub fn handle_config_event(&mut self,
                               config_file_name: &str) {
        if self.res_inventory.is_none() {
            let cnf = config::configuration(&self.originator, Some(config_file_name));
            self.originator.set_application_id(cnf.system_properties().application_id());
            self.originator.set_application_name(cnf.system_properties().application_name());
            for ev_name in cnf.referenced_env_vars() {
                if let Ok(ev_val) = std::env::var(&ev_name) {
                    self.originator.add_env_var(&ev_name, &ev_val);
                }
            }
            let msgs = cnf.messages();
            if ! msgs.is_empty() {
                let header_msg = coalyxw!(E_CFG_FOUND_ISSUES, config_file_name.to_string());
                let mut emsgs = msgs.clone();
                emsgs.insert(0, header_msg);
                log_problems(&emsgs);
            }
            if cnf.server_properties().is_none() {
                self.res_inventory = Some(StandaloneInventory::new(&cnf, &self.originator));
            } else {
                self.res_inventory = Some(ServerInventory::new(&cnf, &self.originator));
            }
            self.configuration = Some(cnf);
        };
    }

    /// Handles a connect event from a remote client.
    /// Creates an output interface for the client.
    /// Adds interface and client information to the internal descriptor table.
    ///
    /// # Arguments
    /// * `client_addr` - the client's socket address
    /// * `client_info` - client information like hostname, process ID etc
    #[cfg(feature="net")]
    pub fn handle_client_connected_event(&mut self,
                                         client_addr: SocketAddr,
                                         client_info: OriginatorInfo) {
        let inv = self.res_inventory.as_mut().unwrap();
        inv.add_remote_client(&client_addr, client_info);
        self.remote_clients.insert(client_addr, HashMap::new());
    }

    /// Handles a disconnect event from a remote client.
    /// Removes the client information from the internal descriptor table.
    #[cfg(feature="net")]
    pub fn handle_client_disconnected_event(&mut self, client_addr: SocketAddr) {
        let inv = self.res_inventory.as_mut().unwrap();
        inv.remove_remote_client(&client_addr);
        let _ = self.remote_clients.remove(&client_addr);
    }

    /// Handles a shutdown event from a client thread.
    /// Executes configured actions upon application exit like buffer flushes, if any.
    /// Closes all output resources.
    pub fn handle_shutdown_event(&mut self) {
        if let Some(ref mut inv) = self.res_inventory.take() { inv.close(); }
    }

    /// Handles a periodic timer event, issued every second.
    /// Informs all resources in inventory to perform a file rollover if it is due.
    ///
    /// # Arguments
    /// * `now` - current timestamp
    pub fn handle_timer_event(&mut self, now: &DateTime<Local>) {
        if let Some(ref mut inv) = self.res_inventory { inv.rollover_if_due(now); }
    }
}

/// Determines output mode to be used for the given record.
/// 
/// # Arguments
/// * `glob_mode_map` - map with active global mode changes
/// * `thread_status` - the thread status descriptor
/// * `record` - the record data
/// 
/// # Return values
/// the bit mask with buffered/enabled record levels
fn determine_mode(glob_mode_map: &mut OverrideModeMap,
                  thread_status: &mut ThreadStatus,
                  mode_change_descs: &ModeChangeDescList,
                  record: &LocalRecordData) -> u32 {
    let mut mode = glob_mode_map.active_mode();
    match record.trigger() {
        RecordTrigger::ObserverCreated => {
            let obs_name = record.observer_name().as_deref();
            if record.level() == RecordLevelId::Object {
                let obs_value = record.message().as_deref();
                let glob_mode = mode_change_descs.global_mode_for_obj(obs_name, obs_value);
                if glob_mode != u32::MAX {
                    glob_mode_map.matching_observer_created(record.observer_id(), glob_mode);
                    mode = glob_mode;
                }
                let loc_mode = mode_change_descs.local_mode_for_obj(obs_name, obs_value);
                if loc_mode != u32::MAX {
                    let new_mode = thread_status.object_created(record.observer_id(), loc_mode);
                    if mode == u32::MAX { mode = new_mode; }
                }
            } else {
                let loc_mode = mode_change_descs.local_mode_for_unit(obs_name);
                if loc_mode != u32::MAX {
                    let new_mode = thread_status.unit_entered(loc_mode);
                    if mode == u32::MAX { mode = new_mode; }
                }
            }
        },
        RecordTrigger::ObserverDropped => {
            if mode == u32::MAX { mode = thread_status.active_mode(); }
            let obs_name = record.observer_name().as_deref();
            if record.level() == RecordLevelId::Object {
                let obs_value = record.message().as_deref();
                if mode_change_descs.global_mode_for_obj(obs_name, obs_value) != u32::MAX {
                    glob_mode_map.matching_observer_dropped(record.observer_id());
                }
                if mode_change_descs.local_mode_for_obj(obs_name, obs_value) != u32::MAX {
                    thread_status.object_dropped(record.observer_id());
                }
            } else if mode_change_descs.local_mode_for_unit(obs_name) != u32::MAX {
                thread_status.unit_left();
            }
        },
        _ => ()
    }
    if mode == u32::MAX { return thread_status.active_mode() }
    mode
}
