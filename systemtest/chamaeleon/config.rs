// ---------------------------------------------------------------------------------------------
// Coaly - context aware logging and tracing system for Rust.
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

//! Configuration parameters for a chamaeleon application.

use rand::Rng;
use regex::Regex;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct ThreadGroup {
    /// group name
    pub name: String,
    /// number of threads in group
    pub count: Option<usize>,
    /// thread structure, one of "sequential", "layered"
    pub structure: String,
    /// number of structure loops, 0 for infinite
    pub loops: usize,
    /// thread runtime, 0 for infinite
    pub runtime: usize,
    /// delay between thread creation and structure start in milliseconds,
    /// format "func(args)", one of:
    /// const(number), rand(lower, upper), mul(number, number)
    /// ${tnr} may be used as argument indicating 0-based thread number
    pub start_delay: Option<String>,
    /// delay between two structure events in milliseconds
    pub event_delay: String
}
impl ThreadGroup {
    pub fn start_delay_millis(&self, thread_nr: usize) -> Result<u64, String> {
        if self.start_delay.is_none() { return Ok(0) }
        ThreadGroup::duration_millis(self.start_delay.as_ref().unwrap(), thread_nr)
    }

    pub fn event_delay_millis(&self, thread_nr: usize) -> Result<u64, String> {
        ThreadGroup::duration_millis(&self.event_delay, thread_nr)
    }

    fn duration_arg_value(val: &str, thread_nr: usize) -> Result<u64, String> {
        if val == TNR_VAR { return Ok(thread_nr as u64) }
        if let Ok(v) = val.parse::<u64>() { return Ok(v) }
        Err(format!("Invalid duration argument {}", val))
    }

    fn duration_millis(val: &str, thread_nr: usize) -> Result<u64, String> {
        let pattern = Regex::new(DURATION_PATTERN).unwrap();
        if let Some(caps) = pattern.captures(val) {
            let func = caps.get(1).unwrap().as_str();
            let val1 = ThreadGroup::duration_arg_value(caps.get(2).unwrap().as_str(), thread_nr)?;
            if func == "const" { return Ok(val1) }
            if caps.get(3).is_none() {
                return Err(format!("Invalid duration specification {}", val))
            }
            let val2 = &caps.get(3).unwrap().as_str()[1..];
            let val2 = ThreadGroup::duration_arg_value(val2, thread_nr)?;
            if func == "rand" {
                let mut rng = rand::thread_rng();
                let dur = rng.gen_range(val1..val2);
                return Ok(dur)
            }
            if func == "mul" { return Ok(val1 * val2) }
        }
        Err(format!("Invalid duration specification {}", val))
    }
}

#[derive(Clone, Deserialize)]
pub struct MockServer {
    /// URL for log and trace records
    pub data_url: String,
    /// URL for administrative commands
    pub admin_url: Option<usize>,
    /// maximum number of simultaneous connections allowed
    pub max_connections: usize,
    /// maximum duration to keep an inactive connection alive, in seconds
    pub keep_connection: usize,
    /// maximum message size, in bytes
    pub max_msg_size: usize,
    /// secret key needed to issue administrative commands
    pub admin_key: Option<String>,
    /// list with IP addresses of clients allowed to send log or trace records to the server
    pub data_clients: Option<Vec<String>>,
    /// list with IP addresses of clients allowed to send administrative commands to the server
    pub admin_clients: Option<Vec<String>>,
    /// list with server downtimes (runtime, duration, re-login required)
    pub down_times: Option<Vec<(usize,usize,bool)>>
}

#[derive(Clone, Deserialize)]
pub struct TestConfig {
    /// description of the test
    pub desc: Option<String>,
    /// full description of the test
    pub full_desc: Option<String>,
    /// pure name of Ariadne configuration file to use
    pub coaly_config_file: String,
    /// set to true, if Ariadne configuration file name is wrong on purpose
    pub coaly_config_file_is_wrong: Option<bool>,
    /// regex pattern to extract essential portions of output
    pub footprint_pattern: String,
    /// regex pattern to sort output files
    pub file_sort_pattern: Option<String>,
    /// name of thread group for main thread
    pub main_group: String,
    /// settings for all thread groups
    pub thread_groups: Vec<ThreadGroup>,
    /// settings for all thread groups
    pub mock_servers: Option<Vec<MockServer>>
}
impl TestConfig {
    /// Creates a configuration descriptor structure from the specified file.
    ///
    /// # Errors
    /// Returns an error message string, if the configuration descriptor could not be built
    pub fn from_file(file_path: &PathBuf) -> Result<TestConfig, String> {
        let file_name = file_path.to_string_lossy();
        if ! file_path.is_file() {
            let emsg = format!("Test configuration file {} not found", file_name);
            return Err(emsg)
        }
        match std::fs::read_to_string(&file_path) {
            Ok(s) => {
                match toml::from_str(&s) {
                    Ok(cfg) => Ok(cfg),
                    Err(e) => Err(format!("Error parsing test configuration file {}: {}",
                                          file_name, e))
                }
            },
            Err(e) => Err(format!("Error reading test configuration file {}: {}", file_name, e))
        }
    }
    pub fn file_sort_pattern(&self) -> String {
        self.file_sort_pattern.as_ref().map_or(String::from("^(.*)$"), |p| p.to_string())
    }
}

const DURATION_PATTERN: &str = r"^(const|mul|rand)\((.*?)(,(.*)){0,1}\)$";
const TNR_VAR: &str = "${tnr}";
