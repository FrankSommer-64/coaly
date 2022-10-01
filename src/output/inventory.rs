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

//! Resource inventory for handling of all output resources.

use chrono::{DateTime, Local};
use super::Interface;

#[cfg(feature="net")]
use std::net::SocketAddr;

#[cfg(feature="net")]
use crate::record::originator::OriginatorInfo;

/// Manages all output resources.
/// Output resources may be either "final" (then associated with a physical resource) or
/// "generic" (file based resources where the name specification contains variables like
/// $ThreadId or $ProcessName).
pub(crate) trait Inventory {

    /// Closes the inventory.
    /// Flushes all buffer configured for flush on exit.
    fn close(&mut self);

    /// Performs a rollover for file based resources if rollover is due.
    /// 
    /// # Arguments
    /// * `now` - current timestamp
    fn rollover_if_due(&mut self, now: &DateTime<Local>);

    /// Creates and returns the output interface for a local thread.
    /// The caller must make sure that resources for the thread have not been allocated yet.
    /// 
    /// # Arguments
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    fn local_thread_interface(&mut self,
                              thread_id: u64,
                              thread_name: &str) -> Interface;

    /// Creates and returns the output interface for a remote thread.
    /// The caller must make sure that resources for the thread have not been allocated yet.
    /// 
    /// # Arguments
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    #[cfg(feature="net")]
    fn remote_thread_interface(&mut self,
                               remote_addr: &SocketAddr,
                               thread_id: u64,
                               thread_name: &str) -> Interface;

    /// Updates the inventory when a remote client connects.
    /// Prepares an interface template for the remote client.
    /// Creates thread-independent resources specific for the remote client.
    /// 
    /// # Arguments
    /// * `remote_addr` - the client's socket address
    /// * `orig_info` - information about the remote client
    #[cfg(feature="net")]
    fn add_remote_client(&mut self,
                         remote_addr: &SocketAddr,
                         orig_info: OriginatorInfo);

    /// Updates the inventory when a remote client disconnects.
    /// Removes all interface templates specific for the remote client from internal lists.
    /// Closes all resources specific for the remote client.
    /// 
    /// # Arguments
    /// * `remote_addr` - the client's socket address
    #[cfg(feature="net")]
    fn remove_remote_client(&mut self,
                            remote_addr: &SocketAddr);
}
