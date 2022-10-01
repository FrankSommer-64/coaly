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

//! Event structure used to carry information in the communication between application threads4
//! and Coaly's worker thread.

use crate::observer::{ObserverData};
use crate::record::RecordLevelId;
use crate::record::recorddata::LocalRecordData;

#[cfg(feature="net")]
use std::net::SocketAddr;

#[cfg(feature="net")]
use crate::record::originator::OriginatorInfo;

#[cfg(feature="net")]
use crate::record::recorddata::RemoteRecordData;

/// Event structure passed from application thread to local agent worker thread.
#[derive(Debug)]
pub(crate) enum CoalyEvent {
    // Log or trace record from a thread within current process
    LocalRecord(LocalRecordData),
    // Log or trace record from remote client
    #[cfg(feature="net")]
    RemoteRecord((SocketAddr, RemoteRecordData)),
    // Process custom configuration file
    Config(String),
    // Connect from remote client
    #[cfg(feature="net")]
    RemoteClientConnected((SocketAddr, OriginatorInfo)),
    // Disconnect from remote client
    #[cfg(feature="net")]
    RemoteClientDisconnected(SocketAddr),
    // Current process terminates
    Shutdown
}

impl CoalyEvent {
    /// Creates an event representing a plain log or trace record.
    ///
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `level` - the record level
    /// * `file_name` - the name of the source code file, where the message was issued
    /// * `line_nr` - the line number in the source code file, where the message was issued
    /// * `msg` - the log or trace message
    #[inline]
    pub(crate) fn for_msg(thread_id: u64,
                          thread_name: &str,
                          level: RecordLevelId,
                          file_name: &'static str,
                          line_nr: u32,
                          msg: &str) -> CoalyEvent {
        CoalyEvent::LocalRecord(LocalRecordData::for_write(thread_id, thread_name, level,
                                                         file_name, line_nr, msg))
    }

    /// Creates an event representing a log or trace record for an observer object.
    ///
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer_data` - the data describing the application object
    /// * `file_name` - the name of the source code file, where the message was issued
    /// * `line_nr` - the line number in the source code file, where the message was issued
    /// * `msg` - the log or trace message
    #[inline]
    pub(crate) fn for_obs_msg(thread_id: u64,
                              thread_name: &str,
                              observer_data: &ObserverData,
                              file_name: &'static str,
                              line_nr: u32,
                              msg: &str) -> CoalyEvent {
        CoalyEvent::LocalRecord(LocalRecordData::for_write_obs(thread_id, thread_name, observer_data,
                                                             file_name, line_nr, msg))
    }

    /// Creates an event representing the entry of a function or module resp.
    /// the creation of a user defined Coaly observer structure.
    ///
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer` - the observer's descriptor
    /// * `line_nr` - the line number in the source code file where the structure was created
    #[inline]
    pub(crate) fn for_create(thread_id: u64,
                             thread_name: &str,
                             observer: &ObserverData,
                             line_nr: u32) -> CoalyEvent {
        CoalyEvent::LocalRecord(LocalRecordData::for_create(thread_id, thread_name,
                                                          observer, line_nr))
    }

    /// Creates an event representing the exit of a function or module resp.
    /// the deletion of a user defined Coaly observer structure.
    ///
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer` - the observer's descriptor
    #[inline]
    pub(crate) fn for_drop(thread_id: u64,
                           thread_name: &str,
                           observer: &ObserverData) -> CoalyEvent {
        CoalyEvent::LocalRecord(LocalRecordData::for_drop(thread_id, thread_name, observer))
    }

    /// Creates an event representing a configuration request.
    ///
    /// # Arguments
    /// * `cfg_fn` - configuration file name
    #[inline]
    pub(crate) fn for_config(cfg_fn: &str) -> CoalyEvent { CoalyEvent::Config(String::from(cfg_fn)) }

    /// Creates an event representing a shutdown request.
    #[inline]
    pub(crate) fn for_shutdown() -> CoalyEvent { CoalyEvent::Shutdown }
}
