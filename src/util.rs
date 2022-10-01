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

//! Utility functions.

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
extern crate winapi;

use regex::Regex;
use std::path::Path;
use std::process;
use std::string::FromUtf8Error;
use std::thread;
use crate::record::originator::OriginatorInfo;

#[cfg(unix)]
pub(crate) const DIR_SEP: char = '/';
#[cfg(windows)]
pub(crate) const DIR_SEP: char = '\\';


/// Escapes all regular expression special characters in the specified string.
pub(crate) fn regex_escaped_str(s: &str) -> String {
    let mut esc_str = String::with_capacity(s.len() * 2);
    for ch in s.chars() {
        match ch {
            '.' | '[' | ']' | '{' | '}' | '(' | ')' |
            '*' | '+' | '?' | '|' | '^' | '$' | '\\' | '-' => {
                esc_str.push('\\');
                esc_str.push(ch);
            },
            _ => { esc_str.push(ch); }
        }
    }
    esc_str
}

/// Converts a string containing a size specification to an integer value.
/// The string must contain digits only plus an optional unit specifier character at the end.
/// Allowed unit specifier are k, m or g for Kilobyte, Megabyte or Gigabyte.
pub(crate) fn parse_size_str(size_str: &str) -> Option<usize> {
    let pattern = Regex::new(SIZE_STR_PATTERN).unwrap();
    if ! pattern.is_match(size_str) { return None }
    let mut num: usize = 0;
    for ch in size_str.chars() {
        match ch {
            '0' ..= '9' => {
                num *= 10;
                num += char::to_digit(ch, 10).unwrap() as usize;
            },
            'k' | 'K' => num *= 1024,
            'm' | 'M' => num *= 1024 * 1024,
            'g' | 'G' => num *= 1024 * 1024 * 1024,
            _ => ()
        }
    }
    Some(num)
}

/// Returns ID and name of the current process.
/// If process name cannot be determined, returns PID instead.
/// These values are used to replace the variables $ProcessId and $ProcessName inside record
/// format or file name specifications in the Coaly configuration file.
/// 
/// #Return values
/// Tuple (Process-ID, Process-Name)
pub(crate) fn process_info() -> (u32, String) {
    let pid = process::id();
    let pname = process_name(pid);
    (pid, pname)
}

/// Returns ID and name of the current thread.
/// If thread name has not been set by the application, returns thread ID instead.
/// 
/// #Return values
/// Tuple (Thread-ID, Thread-Name)
pub(crate) fn thread_info() -> (u64, String) {
    let tid = get_thread_id() as u64;
    let t = thread::current();
    let tname = t.name().unwrap_or("");
    if tname.is_empty() {
        return (tid, tid.to_string())
    }
    (tid, tname.to_string())
}

/// Returns hostname and IP address of the local host.
/// If any of the values could not be determined, returns empty string instead.
/// 
/// #Return values
/// Tuple (Hostname, IP4-Address, IP6-Adress)
pub(crate) fn host_info() -> (String, String, String) {
    let host_name = shell_cmd("hostname").unwrap_or_default().trim().to_string();
    let ip4_addr = ip_address(4);
    let ip6_addr = ip_address(6);
    (host_name, ip4_addr, ip6_addr)
}

/// Returns information structure with attributes characterizing the application and the host it
/// is running on.
/// 
/// #Return values
/// Information structure
pub(crate) fn originator_info() -> OriginatorInfo {
    let (pid, pname) = process_info();
    let (host, ip4, ip6) = host_info();
    if ! ip6.is_empty() { return OriginatorInfo::new(pid, &pname, &host, &ip6) }
    OriginatorInfo::new(pid, &pname, &host, &ip4)
}

#[cfg(unix)]
fn process_name(pid: u32) -> String {
    let cmd = format!("cat /proc/{}/cmdline", pid);
    if let Ok(cmd_line) = shell_cmd(&cmd) {
        if let Some(full_proc_name) = cmd_line.split('\u{0}').next() {
            let proc_path = Path::new(full_proc_name);
            if let Some(proc_name) = proc_path.file_name() {
                return proc_name.to_string_lossy().to_string()
            }
        }
    }
    pid.to_string()
}

#[cfg(windows)]
fn process_name(pid: u32) -> String {
    let cmd = format!(r#"tasklist /fi "PID eq {}""#, pid);
    let cmd_result = shell_cmd(&cmd);
    if cmd_result.is_ok() {
        let cmd_result = cmd_result.unwrap();
        let blank_pos = cmd_result.find(' ').unwrap();
        return cmd_result[0 .. blank_pos].to_string()
    }
    pid.to_string()
}

/// Returns local IP address for given IP protocol version.
/// 
/// #Arguments
/// * `ip_version` - the IP protocol version, 4 or 6
/// 
/// #Return values
/// The local IP address, or empty string, if protocol version is not active
fn ip_address(ip_version: u32) -> String {
    let ip_pattern = Regex::new(IP_ROUTE_PATTERN).unwrap();
    let cmd = format!("ip -{} route|grep src", ip_version);
    let ip_route_info = shell_cmd(&cmd).unwrap_or_default();
    let mut ip_addr = String::from("");
    if let Some(caps) = ip_pattern.captures(&ip_route_info) {
        if let Some(m) = caps.get(1) { ip_addr = m.as_str().to_string() }
    }
    ip_addr
}

/// Executes the given Unix shell command and returns the standard output.
/// 
/// #Arguments
/// * cmd - the command to execute
/// 
/// #Return values
/// The command output
#[cfg(unix)]
fn shell_cmd(cmd: &str) -> Result<String, FromUtf8Error> {
    let cmd_output = process::Command::new("sh")
                                       .arg("-c")
                                       .arg(cmd)
                                       .arg("-o comm=")
                                       .output()
                                       .expect("failed to determine process name");
    String::from_utf8(cmd_output.stdout)
}

/// Executes the given Windows shell command and returns the standard output.
/// 
/// #Arguments
/// * cmd - the command to execute
/// 
/// #Return values
/// The command output
#[cfg(windows)]
fn shell_cmd(cmd: &str) -> Result<String, FromUtf8Error> {
    let cmd_output = process::Command::new(cmd)
                                       .output()
                                       .expect("failed to determine process name");
    String::from_utf8(cmd_output.stdout)
}

#[cfg(unix)]
#[inline]
fn get_thread_id() -> u64 {
    unsafe { libc::pthread_self() }
}

#[cfg(windows)]
#[inline]
fn get_thread_id() -> usize {
    unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as usize }
}

const SIZE_STR_PATTERN: &str = "^[0-9]+\\s*[kKmMgG]{0,1}$";

const IP_ROUTE_PATTERN: &str = r".*\s+src\s+(.*?)\s+.*";

#[cfg(test)]
pub(crate) mod tests {
    use std::fs::{metadata, read_dir};

    /// Function signature for a specific unit test
    /// Arguments are:
    ///   bool success expected
    ///   &str Coaly project root directory
    ///   &str input file name
    ///   &str footprint file name
    pub(crate) type UnitTestFn = fn(bool, &str, &str, &str) -> Option<String>;

    /// Runs a unit test.
    /// 
    /// # Arguments
    /// * `root_dir` the root directory of the test data
    /// * `success_expected` indicates whether it is expected that all tests succeed
    /// * `inp_file_ext` file name extension of all input test data files
    /// * `footprint_file_ext` file name extension of all footprint test data files
    /// * `lang` the language to test; empty string if not relevant
    /// * `test_fn` the specific test function running the test
    /// 
    /// # Return values
    /// **None** in case the test result meets the expectation;
    /// a string indicating an error in case of failure
    pub(crate) fn run_unit_tests(proj_root_dir: &str,
                                 test_mod: &str,
                                 success_expected: bool,
                                 inp_file_ext: &str,
                                 footprint_file_ext: &str,
                                 lang: &str,
                                 test_fn: UnitTestFn) -> Option<String> {
        let testdata_root_dir = format!("{}/testdata/ut/{}", proj_root_dir, test_mod);
        let mut input_dir = format!("{}/input", testdata_root_dir);
        let mut footprint_dir = format!("{}/footprint", testdata_root_dir);
        if success_expected {
            input_dir = format!("{}/success", input_dir);
            footprint_dir = format!("{}/success", footprint_dir);
        } else {
            input_dir = format!("{}/fail", input_dir);
            footprint_dir = format!("{}/fail", footprint_dir);
            if ! lang.is_empty() {footprint_dir = format!("{}/{}", footprint_dir, lang); }
        }
        if let Ok(inp_dir) = read_dir(&input_dir) {
            for node in inp_dir {
                let path = node.unwrap().path();
                let metadata = metadata(&path).unwrap();
                if ! metadata.is_file() { continue; }
                let input_fn = path.file_name().unwrap().to_str().unwrap();
                let ref_fn = input_fn.to_string().replace(inp_file_ext, footprint_file_ext);
                let full_input_fn = format!("{}/{}", &input_dir, &input_fn);
                let full_ref_fn = format!("{}/{}", &footprint_dir, &ref_fn);
                if let Some(err_msg) = test_fn(success_expected, proj_root_dir,
                                               &full_input_fn, &full_ref_fn) {
                    return Some(format!("Test {} failed. {}", input_fn, &err_msg))
                }
            }
        } else {
            return Some(format!("Could not find test data input directory {}", &input_dir))
        }
        None
    }
}