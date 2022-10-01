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

//! The local agent is the central trace and log management instance within a process.

extern crate chrono;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;
use crate::{coalyxe, CoalyObservable};
use crate::config;
use crate::errorhandling::*;
use crate::event::CoalyEvent;
use crate::observer::ObserverData;
use crate::record::RecordLevelId;
use crate::util;

#[cfg(feature="net")]
use std::net::SocketAddr;

#[cfg(feature="net")]
use crate::record::originator::OriginatorInfo;

#[cfg(feature="net")]
use crate::record::recorddata::RemoteRecordData;

mod threadstatus;
mod worker;

lazy_static! {
    /// Singleton instance of local agent
    static ref LOCAL_AGENT: Arc<Mutex<CoalyAgent>> = Arc::new(Mutex::new(CoalyAgent::new()));
}

/// Initializes the local agent.
/// 
/// If the function has not been called prior to any message output, the system will assume
/// default settings. This is also the case, if an error during configuration file processing
/// occurs.
/// Calling the function for an already initialized system has no effect.
/// 
/// # Arguments
/// * `config_file_name` - the name of the configuration file
pub fn initialize(config_file_name: &str) {
    if let Ok(mut agent) = LOCAL_AGENT.try_lock() { agent.configure(config_file_name); }
}

/// Terminates the local agent.
/// Sends shutdown event to worker thread and waits for worker thread termination.
pub fn shutdown() {
    if let Ok(mut agent) = LOCAL_AGENT.lock() { agent.shutdown(); }
}

/// Processes a log or trace record according to the specified behaviour.
/// 
/// # Arguments
/// * `level` - the record level
/// * `file_name` - the name of the source code file, where the message was issued
/// * `line_nr` - the line number in the source code file, where the message was issued
/// * `msg` - the log or trace message
pub fn write(level: RecordLevelId,
             file_name: &'static str,
             line_nr: u32,
             msg: &str) {
    if let Some(thread_desc) = app_thread_desc() {
        let event = CoalyEvent::for_msg(thread_desc.id, &thread_desc.name,
                                        level, file_name, line_nr, msg);
        thread_desc.send(event);
    }
}

/// Processes a log or trace record according to the specified behaviour.
/// 
/// # Arguments
/// * `level` - the record level
/// * `file_name` - the name of the source code file, where the message was issued
/// * `line_nr` - the line number in the source code file, where the message was issued
/// * `msg` - the log or trace message
pub fn write_obs(observer: &dyn CoalyObservable,
                 file_name: &'static str,
                 line_nr: u32,
                 msg: &str) {
    if let Some(thread_desc) = app_thread_desc() {
        let obs_data = &observer.coaly_observer().0;
        let event = CoalyEvent::for_obs_msg(thread_desc.id, &thread_desc.name,
                                            obs_data, file_name, line_nr, msg);
        thread_desc.send(event);
    }
}

/// Processes the creation of a Coaly observer structure.
/// 
/// # Arguments
/// * `observer` - the observer's descriptor
/// * `line_nr` - the line number in the source code file where the structure was created
pub fn observer_created(observer: &ObserverData,
                        line_nr: u32) {
    if let Some(thread_desc) = app_thread_desc() {
        let event = CoalyEvent::for_create(thread_desc.id, &thread_desc.name, observer, line_nr);
        thread_desc.send(event);
    }
}

/// Processes the deletion of a Coaly specific structure.
/// 
/// # Arguments
/// * `observer` - the observer's descriptor
pub fn observer_dropped(observer: &ObserverData) {
    if let Some(thread_desc) = app_thread_desc() {
        let event = CoalyEvent::for_drop(thread_desc.id, &thread_desc.name, observer);
        thread_desc.send(event);
    }
}

/// Sends a log or trace record from a remote application to Coaly worker thread
///
/// # Arguments
/// * `remote_addr` - the remote application's network address
/// * `rec` - the log or trace record
#[cfg(feature="net")]
pub(crate) fn write_rec(remote_addr: &SocketAddr,
                        rec: RemoteRecordData) {
    if let Some(thread_desc) = app_thread_desc() {
        let event = CoalyEvent::RemoteRecord((*remote_addr, rec));
        thread_desc.send(event);
    }
}

/// Sends indication about successful connection of a remote application to Coaly worker thread
/// 
/// # Arguments
/// * `remote_addr` - the remote application's network address
/// * `orig_info` - information about the remote application
#[cfg(feature="net")]
pub(crate) fn remote_client_connected(remote_addr: &SocketAddr,
                                      orig_info: OriginatorInfo) {
    if let Some(thread_desc) = app_thread_desc() {
        let event = CoalyEvent::RemoteClientConnected((*remote_addr, orig_info));
        thread_desc.send(event);
    }
}

/// Indicates a remote client connection has been closed
/// 
/// # Arguments
/// * `client_addr` - the remote client's IP address and port
#[cfg(feature="net")]
pub(crate) fn remote_client_disconnected(client_addr: &SocketAddr) {
    if let Some(thread_desc) = app_thread_desc() {
        let event = CoalyEvent::RemoteClientDisconnected(*client_addr);
        thread_desc.send(event);
    }
}

/// Descriptor holding the data required for an application thread to communicate with Coaly
struct AppThreadDesc {
    // thread id
    id: u64,
    // thread name, if specified by the application; otherwise also thread ID
    name: String,
    // sender end of communication channel to Coaly worker thread
    channel: Sender<CoalyEvent>,
    // reason of last send error
    last_send_err: RefCell<String>,
    // timestamp of first send error not yet logged
    last_logged_send_err: Cell<Instant>,
    // total number of send errors
    total_send_err_count: Cell<u64>,
    // number of send errors not yet logged
    unlogged_send_err_count: Cell<u64>
}
impl AppThreadDesc {
    /// Creates an application thread descriptor structure.
    /// 
    /// # Arguments
    /// * ch - the sender end of the Coaly worker thread communication channel
    /// 
    /// # Return values
    /// application thread descriptor structure
    fn new(ch: Sender<CoalyEvent>) -> Arc<AppThreadDesc> {
        let (tid, tname) = util::thread_info();
        let t = AppThreadDesc {
                    id: tid,
                    name: tname,
                    channel: ch,
                    last_send_err: RefCell::new(String::from("")),
                    last_logged_send_err: Cell::new(Instant::now()),
                    total_send_err_count: Cell::new(0),
                    unlogged_send_err_count: Cell::new(0)
                };
        Arc::new(t)
    }

    /// Sends given event to the Coaly worker thread
    ///
    /// # Arguments
    /// * event - the event to send
    fn send(&self, event: CoalyEvent) {
        // don't send events during shutdown
        if SHUTDOWN_PENDING.load(Ordering::Relaxed) { return }
        if let Err(result) = self.channel.send(event) {
            let now = Instant::now();
            let total_err_count = self.total_send_err_count.get();
            self.total_send_err_count.set(total_err_count + 1);
            if total_err_count <= INITIAL_SEND_ERRORS_TO_LOG {
                // log first send errors unconditionally
                self.last_logged_send_err.set(now);
                let m = vec!(coalyxe!(E_INTERNAL_EVENT_FAILED, result.to_string()));
                log_problems(&m);
            } else {
                let unlogged_err_count = self.unlogged_send_err_count.get() + 1;
                self.unlogged_send_err_count.set(unlogged_err_count);
                let last_logging_time = self.last_logged_send_err.get();
                if now.duration_since(last_logging_time).as_secs() >= SEND_ERROR_IGNORE_DURATION {
                    self.log_send_errors(unlogged_err_count);
                }
            }
        } else {
            let unlogged_err_count = self.unlogged_send_err_count.get();
            if unlogged_err_count > 0 { self.log_send_errors(unlogged_err_count); }
        }
    }

    /// Logs all internal send errors to Coaly worker thread which have not been logged yet.
    ///
    /// # Arguments
    /// * unlogged_err_count - the number of failures, must be greater than 0
    fn log_send_errors(&self, unlogged_err_count: u64) {
        if unlogged_err_count == 1 {
            let m = vec!(coalyxe!(E_INTERNAL_EVENT_FAILED,
                                  self.last_send_err.borrow().to_string()));
            log_problems(&m);
        } else {
            let m = vec!(coalyxe!(E_INTERNAL_EVENTS_FAILED,
                                  unlogged_err_count.to_string(),
                                  self.last_send_err.borrow().to_string()));
            log_problems(&m);
        }
        self.unlogged_send_err_count.set(0);
        self.last_logged_send_err.set(Instant::now());
    }
}
unsafe impl Sync for AppThreadDesc {}

/// Coaly agent.
/// Gateway to access Coaly worker thread.
pub(crate) struct CoalyAgent {
    // descriptor structures for all known application threads
    threads: HashMap<thread::ThreadId, Arc<AppThreadDesc>>,
    // initial sender part of communication channel to Coaly worker thread,
    // cloned for every application thread
    tx_master: Sender<CoalyEvent>,
    // join handle to Coaly worker thread
    worker: Option<thread::JoinHandle<()>>
}
impl CoalyAgent {
    /// Creates the hash table for client thread administration
    fn new() -> CoalyAgent {
        // create communication channel to worker thread
        let (sender, receiver) = channel::<CoalyEvent>();
        // create hash table for client threads
        CoalyAgent {
            threads: HashMap::new(),
            tx_master: sender,
            worker: Some(worker::spawn(receiver))
        }
    }

    /// Sets the Coaly shutdown indicator and terminates the Coaly worker thread
    fn shutdown(&mut self) {
        if SHUTDOWN_PENDING.swap(true, Ordering::Relaxed) { return }
        let _ = self.tx_master.send(CoalyEvent::for_shutdown());
        self.worker.take().map(thread::JoinHandle::join);
    }

    /// Sends a configure event to the worker thread
    /// 
    /// # Arguments
    /// * `config_file_name` - the name of the configuration file
    fn configure(&mut self, config_file_name: &str) {
        if let Some(tdata) = self.desc_for(std::thread::current().id()) {
            tdata.send(CoalyEvent::for_config(config_file_name));
        }
    }

    /// Returns descriptor for the application thread with given thread ID.
    /// Descriptor structure is created, if the calling thread is not yet known to Coaly.
    /// 
    /// # Arguments
    /// * `thread_id` - the (Rust) thread ID
    /// 
    /// # Return values
    /// application thread descriptor structure; None, if the Coaly system is shutting down
    fn desc_for(&mut self, thread_id: thread::ThreadId) -> Option<Arc<AppThreadDesc>> {
        if SHUTDOWN_PENDING.load(Ordering::Relaxed) { return None }
        if ! self.threads.contains_key(&thread_id) {
            let tdata = AppThreadDesc::new(self.tx_master.clone());
            self.threads.insert(thread_id, tdata);
        };
        self.threads.get(&thread_id).cloned()
    }
}

/// Returns descriptor for the calling application thread needed to communicate with Coaly worker
/// thread. Descriptor structure is created, if the calling thread is not yet known to Coaly.
/// 
/// # Return values
/// application thread descriptor structure; None, if the Coaly system is shutting down or
/// the internal descriptor table can't be locked
fn app_thread_desc() -> Option<Arc<AppThreadDesc>> {
    let tid = std::thread::current().id();
    let app_thread_table = LOCAL_AGENT.clone();
    if let Ok(mut agent) = app_thread_table.lock() { return agent.desc_for(tid) }
    None
}

// number of send errors to Coaly worker thread that are logged unconditionally
const INITIAL_SEND_ERRORS_TO_LOG: u64 = 5;

// time span where consecutive send errors to Coaly worker thread are counted, but not logged,
// in seconds
const SEND_ERROR_IGNORE_DURATION: u64 = 60;

// shutdown indicator
static SHUTDOWN_PENDING: AtomicBool = AtomicBool::new(false);
