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

//! Resource inventory for handling of all output resources for a trace server.

use chrono::{DateTime, Local};
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::rc::Rc;
use crate::config::Configuration;
use crate::errorhandling::{CoalyException, log_problems};
use crate::record::originator::OriginatorInfo;
use super::Interface;
use super::formatspec::FormatSpec;
use super::inventory::Inventory;
use super::outputformat::OutputFormat;
use super::resource::{Resource, ResourceRef};


/// Manages all output resources for a trace server.
/// Output resources may be either "final" (then associated with a physical resource) or
/// "generic" (file based resources where the name specification contains variables like
/// $ThreadId or $ProcessName).
pub(crate) struct ServerInventory {
    // all final output resources, needed to serve timer events and system cleanup
    all_resources: Vec<ResourceRef>,
    // interface template containing all resources from configuration, not optimized
    global_template: Vec<ResourceRef>,
    // interface template containing all resources from configuration, optimized for application
    // and originator. May hold thread-specific generic resources.
    local_template: Vec<ResourceRef>,
    // interface templates containing all resources for a specific originator.
    // May hold thread-specific generic resources.
    originator_templates: HashMap<SocketAddr, (OriginatorInfo, Vec<ResourceRef>)>,
    // all currently allocated final originator- and/or thread-specific resources.
    specific_resources: HashMap<FormatSpec, ResourceRef>,
    // originator information for local application
    local_app_data: OriginatorInfo
}
impl ServerInventory {
    /// Creates the inventory for a standalone application from the specifications
    /// in the configuration file.
    ///
    /// # Arguments
    /// * `config` - the configuration, either from configuration file
    /// * `orig_info` - information about application process and local host
    pub(crate) fn new(config: &Rc<Configuration>,
                      orig_info: &OriginatorInfo) -> Box<ServerInventory> {
        let mut problems = Vec::<CoalyException>::new();
        let mut all_resources = Vec::<ResourceRef>::new();
        let mut global_template = Vec::<ResourceRef>::new();
        let mut local_template = Vec::<ResourceRef>::new();
        let mut specific_resources = HashMap::<FormatSpec, ResourceRef>::new();
        for rdesc in config.resources().elements() {
            match Resource::from_config(rdesc, config, orig_info) {
                Ok(res) => {
                    let orig_spec_flag = res.is_originator_specific();
                    let thread_spec_flag = res.is_thread_specific();
                    // add unchanged resource to global template
                    let res_ref = Rc::new(RefCell::new(res));
                    global_template.push(res_ref.clone());
                    if orig_spec_flag {
                        // create originator optimized resource for local template
                        let opt_name = res_ref.borrow().originator_optimized_name(orig_info).unwrap();
                        let opt_res = res_ref.borrow().for_originator(opt_name.clone()).unwrap();
                        let opt_res_ref = Rc::new(RefCell::new(opt_res));
                        if ! thread_spec_flag {
                            // originator-specific only
                            specific_resources.insert(opt_name, opt_res_ref.clone());
                            all_resources.push(opt_res_ref.clone());
                        }
                        local_template.push(opt_res_ref);
                    } else {
                        // not originator-specific
                        if ! thread_spec_flag { all_resources.push(res_ref.clone()); }
                        local_template.push(res_ref);
                    }
                },
                Err(ex) => problems.push(ex)
            }
        }
        if ! problems.is_empty() { log_problems(&problems); }
        Box::new(ServerInventory {
                     all_resources,
                     global_template,
                     local_template,
                     originator_templates: HashMap::new(),
                     specific_resources,
                     local_app_data: orig_info.clone()
                })
    }
}
impl Inventory for ServerInventory {
    /// Closes the inventory.
    /// Flushes all buffer configured for flush on exit.
    fn close(&mut self) {
        self.all_resources.iter_mut().for_each(|x| Resource::close(&mut x.borrow_mut()));
    }

    /// Performs a rollover for file based resources if rollover is due.
    /// 
    /// # Arguments
    /// * `now` - current timestamp
    fn rollover_if_due(&mut self, now: &DateTime<Local>) {
        let mut problems = Vec::<CoalyException>::new();
        for res in self.all_resources.iter_mut() {
            if let Err(ex) = res.borrow_mut().rollover_if_due(now) {
                problems.push(ex);
            }
        }
        if ! problems.is_empty() { log_problems(&problems); }
    }

    /// Creates and returns the output interface for a local thread.
    ///
    /// # Arguments
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    fn local_thread_interface(&mut self,
                              thread_id: u64,
                              thread_name: &str) -> Interface {
        let mut problems = Vec::<CoalyException>::new();
        let mut output_resources = Vec::<(OutputFormat, ResourceRef)>::new();
        for res in &self.local_template {
            let ofmt = res.borrow().optimized_output_format(&self.local_app_data,
                                                            thread_id, thread_name);
            if res.borrow().is_thread_specific() {
                // check whether matching resource exists
                let res_name = res.borrow().thread_optimized_name(thread_id, thread_name).unwrap();
                if self.specific_resources.contains_key(&res_name) {
                    output_resources.push((ofmt, res.clone()));
                } else {
                    // instantiate template for the thread
                    match res.borrow().for_thread(res_name.clone()) {
                        Ok(spec_res) => {
                            let spec_res = Rc::new(RefCell::new(spec_res));
                            output_resources.push((ofmt.clone(), spec_res.clone()));
                            self.specific_resources.insert(res_name, spec_res.clone());
                            self.all_resources.push(spec_res);
                        },
                        Err(ex) => problems.push(ex)
                    }
                }
            } else {
                // process wide resources can be used unchanged
                output_resources.push((ofmt, res.clone()));
            }
        }
        if ! problems.is_empty() { log_problems(&problems); }
        Interface::new(output_resources)
    }

    /// Creates and returns the output interface for a remote thread.
    /// 
    /// # Arguments
    /// * `remote_addr` - remote client address
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    fn remote_thread_interface(&mut self,
                               remote_addr: &SocketAddr,
                               thread_id: u64,
                               thread_name: &str) -> Interface {
        let mut problems = Vec::<CoalyException>::new();
        let mut output_resources = Vec::<(OutputFormat, ResourceRef)>::new();
        if let Some((orig_info, resources)) = self.originator_templates.get(remote_addr) {
            for res_ref in resources {
                let ofmt = res_ref.borrow().optimized_output_format(&orig_info,
                                                                    thread_id, thread_name);
                if res_ref.borrow().is_thread_specific() {
                    // check whether matching resource exists
                    let res_name = res_ref.borrow()
                                          .thread_optimized_name(thread_id, thread_name)
                                          .unwrap();
                    if let Some(spec_res_ref) = self.specific_resources.get(&res_name) {
                        output_resources.push((ofmt, spec_res_ref.clone()));
                    } else {
                        // instantiate template for the thread
                        match res_ref.borrow().for_thread(res_name.clone()) {
                            Ok(spec_res) => {
                                let spec_res = Rc::new(RefCell::new(spec_res));
                                output_resources.push((ofmt.clone(), spec_res.clone()));
                                self.specific_resources.insert(res_name, spec_res.clone());
                                self.all_resources.push(spec_res);
                            },
                            Err(ex) => problems.push(ex)
                        }
                    }
                } else {
                    // originator wide resources can be used unchanged
                    output_resources.push((ofmt, res_ref.clone()));
                }
            }
        }
        if ! problems.is_empty() { log_problems(&problems); }
        Interface::new(output_resources)
    }

    /// Updates the inventory when a remote client connects.
    /// Prepares an interface template for the remote client.
    /// Creates thread-independent resources specific for the remote client.
    /// 
    /// # Arguments
    /// * `remote_addr` - the client's socket address
    /// * `orig_info` - information about the remote client
    fn add_remote_client(&mut self,
                         remote_addr: &SocketAddr,
                         orig_info: OriginatorInfo) {
        let mut orig_resources = Vec::<ResourceRef>::new();
        for res_ref in &self.global_template {
            let orig_spec_flag = res_ref.borrow().is_originator_specific();
            let thread_spec_flag = res_ref.borrow().is_thread_specific();
            if orig_spec_flag {
                let opt_name = res_ref.borrow().originator_optimized_name(&orig_info).unwrap();
                if let Some(res) = self.specific_resources.get(&opt_name) {
                    // originator optimized resource already exists, use it
                    orig_resources.push(res.clone());
                    continue;
                }
                // create originator optimized resource
                let opt_res = res_ref.borrow().for_originator(opt_name.clone()).unwrap();
                let opt_res_ref = Rc::new(RefCell::new(opt_res));
                if ! thread_spec_flag {
                    self.specific_resources.insert(opt_name, opt_res_ref.clone());
                    self.all_resources.push(opt_res_ref.clone());
                }
                orig_resources.push(opt_res_ref);
            } else {
                // not originator-specific, can be directly used and has already been stored
                // in all resources during inventory construction
                orig_resources.push(res_ref.clone());
            }
        }
        self.originator_templates.insert(remote_addr.clone(), (orig_info, orig_resources));
    }

    /// Updates the inventory when a remote client disconnects.
    /// Removes all interface templates specific for the remote client from internal lists.
    /// Closes all resources specific for the remote client.
    /// 
    /// # Arguments
    /// * `remote_addr` - the client's socket address
    fn remove_remote_client(&mut self,
                            remote_addr: &SocketAddr) {
        self.originator_templates.remove(remote_addr);
    }
}
