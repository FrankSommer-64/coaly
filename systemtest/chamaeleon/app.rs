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

//! Highly configurable application for integration and system test.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use coaly::*;
use super::config::{TestConfig, ThreadGroup};

/// Chamaeleon test application.
pub struct App {
    _test_name: String,
    config: TestConfig
}

impl App {
    // Creates a chamaeleon application.
    pub fn new(test_name: &str, config: TestConfig) -> App {
        App { _test_name: test_name.to_string(), config }
    }

    /// Runs the chamaeleon application.
    /// 
    /// # Arguments
    /// * `log_cfg_path` - the directory containing Ariadne configuration files
    pub fn run(&self, log_cfg_path: &PathBuf) -> Result<(), String> {
        // Initialize Ariadne
        let coaly_cfg_file = log_cfg_path.join(&self.config.coaly_config_file);
        let coaly_cfg_file_name = coaly_cfg_file.to_string_lossy().to_string();
        let mut check_log_cfg_file = true;
        if let Some(flag) = self.config.coaly_config_file_is_wrong { check_log_cfg_file = ! flag; }
        if check_log_cfg_file && ! coaly_cfg_file.is_file() {
            return Err(format!("Ariadne configuration file {} not found", &coaly_cfg_file_name))
        }
        coaly::initialize(&coaly_cfg_file_name);
        let m = self.run_main();
        thread::sleep(Duration::from_secs(1));
        coaly::shutdown();
        m
    }

    fn run_main(&self) -> Result<(), String> {
        let main_grp_name = &self.config.main_group;
        let main_thread_grp = self.thread_group(main_grp_name)?;
        let event_delay = main_thread_grp.event_delay_millis(0)?;
        let worker_threads = self.create_worker_threads(main_grp_name)?;
        match main_thread_grp.structure.as_str() {
            "sequential" => exec_sequential(main_thread_grp.runtime, main_thread_grp.loops,
                                            0, event_delay),
            _ => return Err(format!("Unknown structure {}", &main_thread_grp.structure))
        }
        for wt in worker_threads { let _ = wt.join(); }
        Ok(())
    }

    fn thread_group(&self, name: &str) -> Result<&ThreadGroup, String> {
        for tc in self.config.thread_groups.iter() {
            if tc.name == name { return Ok(&tc) }
        }
        Err(format!("Thread class {} not defined", name))
    }

    fn create_worker_threads(&self,
                             main_name: &str) -> Result<Vec<thread::JoinHandle<Result<(),String>>>, String> {
        let mut worker_groups = Vec::<thread::JoinHandle<Result<(),String>>>::new();
        let tgroups = self.config.thread_groups.clone();
        for tgrp in tgroups.into_iter() {
            if tgrp.name == main_name { continue; }
            let thread_count = tgrp.count.unwrap_or(1);
            for tnr in 1 ..= thread_count {
                let t_name = if thread_count == 1 { tgrp.name.clone() }
                             else { format!("{}-{}", tgrp.name, tnr) };
                let start_delay = tgrp.start_delay_millis(tnr)?;
                let event_delay = tgrp.event_delay_millis(tnr)?;
                let tbuilder = thread::Builder::new().name(t_name);
                match tgrp.structure.as_str() {
                    "sequential" => {
                        worker_groups.push(
                            tbuilder.spawn(move || {
                                exec_sequential(tgrp.runtime, tgrp.loops, start_delay, event_delay);
                                Ok(())
                        }).unwrap());
                    }
                    _ => return Err(format!("Unknown structure {}", tgrp.structure.as_str()))
                }
            }
        }
        Ok(worker_groups)
    }
}

fn run_seq_statements(delay: Duration) {
    logfn!("run_sequential");
    {
        thread::sleep(delay);
        let _ = Message::new("Msg", "ConnectRequest");
    }
    thread::sleep(delay);
    logemgcy!("PANIC");
    thread::sleep(delay);
    logalert!("ALERT");
    thread::sleep(delay);
    logcrit!("CRITICAL SITUATION");
    thread::sleep(delay);
    logerror!("Hatamal a fatal error");
    thread::sleep(delay);
    logwarn!("Beware of the dog");
    thread::sleep(delay);
    lognote!("Call mother");
    thread::sleep(delay);
    loginfo!("FYI");
    thread::sleep(delay);
    logdebug!("0xDEADBEEF");
    thread::sleep(delay);
    local_function();
    thread::sleep(delay);
    testmod::api_function();
    thread::sleep(delay);
    mode_change_function();
    thread::sleep(delay);
    object_function();
    thread::sleep(delay);
}

fn exec_sequential(runtime: usize, loop_count: usize, start_delay: u64, event_delay: u64) {
    let mut loop_nr = 0usize;
    let loop_count = if loop_count == 0 { usize::MAX } else { loop_count };
    let start_time = Instant::now();
    let runtime = if runtime == 0 { Duration::from_secs(86400) }
                  else { Duration::from_millis(runtime as u64) };
    let end_time = start_time + runtime;
    if start_delay > 0 { thread::sleep(Duration::from_millis(start_delay)); }
    let event_delay = Duration::from_millis(event_delay);
    while Instant::now() < end_time {
        if loop_nr >= loop_count { break; }
        loop_nr += 1;
        run_seq_statements(event_delay);
    }
}

fn local_function() {
    logfn!("local_function");
    { let _ = Message::new("LocalMsg", "DisonnectRequest"); }
    logemgcy!("LOCAL PANIC");
    logalert!("LOCAL ALERT");
    logcrit!("LOCAL CRITICAL SITUATION");
    logerror!("LOCAL ERROR");
    logwarn!("LOCAL WARNING");
    lognote!("LOCAL NOTICE");
    loginfo!("LOCAL INFO");
    logdebug!("LOCAL DEBUG");
}

fn object_function() {
    let _ = Message::new("EssentialMsg", "123");
    local_function();
}

fn mode_change_function() {
    logfn!("mode_change_function");
    local_function();
}

struct Message {
    _name: String,
    _value: String,
    obs: CoalyObserver
}
impl Message {
    fn new (name: &str, value: &str) -> Message {
        Message {
            _name: name.to_string(),
            _value: value.to_string(),
            obs: newcoalyobs!(name, value)
        }
    }
}
impl CoalyObservable for Message {
    fn coaly_observer(&self) -> &CoalyObserver { &self.obs }
}

pub mod testmod {
    use coaly::*;
    use super::Message;

    pub fn api_function() {
        logmod!("testmod");
        logfn!("api_function");
        mod_function();
    }

    pub fn mod_function() {
        logfn!("mod_function");
        { let _ = Message::new("TestmodMsg", "Data"); }
        logemgcy!("TESTMOD PANIC");
        logalert!("TESTMOD ALERT");
        logcrit!("TESTMOD CRITICAL SITUATION");
        logerror!("TESTMOD ERROR");
        logwarn!("TESTMOD WARNING");
        lognote!("TESTMOD NOTICE");
        loginfo!("TESTMOD INFO");
        logdebug!("TESTMOD DEBUG");
    }
}