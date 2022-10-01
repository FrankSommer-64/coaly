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

//! Coaly system properties.

use std::fmt::{Debug, Formatter};
use crate::record::{RecordLevelId, RecordLevelMap};


// Default value and range for size of mode change stack
pub(crate) const DEFAULT_CHANGE_STACK_SIZE: usize = 32768;
pub(crate) const MIN_CHANGE_STACK_SIZE: usize = 16;
pub(crate) const MAX_CHANGE_STACK_SIZE: usize = 2147483647;


/// Coaly system properties.
/// All properties are specified under TOML table system in the custom configuration file.
#[derive (Clone)]
pub struct SystemProperties {
    // optional application ID
    application_id: u32,
    // optional application name, read from custom configuration
    application_name: String,
    // size of stack for mode changes, in number of entries
    change_stack_size: usize,
    // root directory for output files, defaults to current directory or system temp dir, if
    // current directory isn't writable
    output_path: String,
    // root directory for emergency cases, defaults to contents of environment variable TEMP or
    // or system temp dir, if the variable isn't defined
    fallback_path: String,
    // bit mask with all enabled record levels upon application start
    enabled_levels: u32,
    // bit mask with all buffered record levels upon application start
    buffered_levels: u32,
    // ID character and name for all record levels
    record_levels: RecordLevelMap
}
impl SystemProperties {
    /// Returns the application ID.
    #[inline]
    pub fn application_id(&self) -> u32 { self.application_id }

    /// Returns the application ID as string
    #[inline]
    pub fn application_id_str(&self) -> String { self.application_id.to_string() }

    /// Sets the application ID.
    /// 
    /// # Arguments
    /// * `app_id` - the custom application ID
    #[inline]
    pub fn set_application_id(&mut self, app_id: u32) { self.application_id = app_id; }

    /// Returns the application name.
    /// If parameter is not specified in the custom configuration file, it will be set with the
    /// process name or, if the process name can't be determined, with the process ID.
    #[inline]
    pub fn application_name(&self) -> &String { &self.application_name }

    /// Sets the application name.
    /// 
    /// # Arguments
    /// * `name` - the custom application name
    #[inline]
    pub fn set_application_name(&mut self, name: &str) { self.application_name = name.to_string() }

    /// Returns the size of the stack for pending mode changes.
    #[inline]
    pub fn change_stack_size(&self) -> usize { self.change_stack_size }

    /// Sets the size of the stack for pending mode changes.
    /// 
    /// # Arguments
    /// * `size` - the stack size in number of entries, between 16 and 512M
    #[inline]
    pub fn set_change_stack_size(&mut self, size: usize) {
        if (MIN_CHANGE_STACK_SIZE..=MAX_CHANGE_STACK_SIZE).contains(&size) {
            self.change_stack_size = size as usize;
        }
    }

    /// Returns the root directory for output files.
    /// If parameter is not specified in the custom configuration file, it defaults to
    /// the directory where the application binary resides. System temp directory will be used,
    /// if application binary directory isn't writable.
    #[inline]
    pub fn output_path(&self) -> &str { &self.output_path }

    /// Sets the root directory for output files.
    /// 
    /// # Arguments
    /// * `path` - the root path for output files
    #[inline]
    pub fn set_output_path(&mut self, path: &str) { self.output_path = path.to_string(); }

    /// Returns the root directory for emergency.
    #[inline]
    pub fn fallback_path(&self) -> &str { &self.fallback_path }

    /// Sets the root directory for emergency.
    /// 
    /// # Arguments
    /// * `path` - the root path for emergency
    #[inline]
    pub fn set_fallback_path(&mut self, path: &str) { self.fallback_path = path.to_string(); }

    /// Returns the bit mask with the record levels enabled upon application start
    #[inline]
    pub fn initial_output_mode(&self) -> u32 {
        (self.buffered_levels << 16) | self.enabled_levels
    }

    /// Sets the bit mask with the record levels enabled upon application start
    /// 
    /// # Arguments
    /// * `levels` - the initially enabled record levels
    #[inline]
    pub fn set_initially_enabled_levels(&mut self, levels: u32) { self.enabled_levels = levels }

    /// Sets the bit mask with the record levels buffered upon application start
    /// 
    /// # Arguments
    /// * `levels` - the initially buffered record levels
    #[inline]
    pub fn set_initially_buffered_levels(&mut self, levels: u32) { self.buffered_levels = levels }

    /// Returns ID character and name for all record levels
    #[inline]
    pub fn record_levels(&self) -> &RecordLevelMap { &self.record_levels }

    /// Sets the record level ID characters and names
    /// 
    /// # Arguments
    /// * `levels` - the record level ID characters and names
    #[inline]
    pub fn set_record_levels(&mut self, levels: RecordLevelMap) { self.record_levels = levels }
}
impl Default for SystemProperties {
    fn default() -> Self {
        let mut opath = std::env::temp_dir();
        if let Ok(cwd) = std::env::current_dir() { opath = cwd; }
        Self {
            application_id: 0,
            application_name: String::from(""),
            change_stack_size: DEFAULT_CHANGE_STACK_SIZE,
            output_path: opath.to_string_lossy().to_string(),
            fallback_path: std::env::temp_dir().to_string_lossy().to_string(),
            enabled_levels: RecordLevelId::Logs as u32,
            buffered_levels: 0,
            record_levels: RecordLevelMap::default()
        }
    }
}
impl Debug for SystemProperties {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,
               "AID:{}/APP:{}/CSS:{}/OPP:{}/FBP:{}/ENA:{:b}/BUF:{:b}/LVL:{:?}",
               self.application_id, self.application_name(), self.change_stack_size,
               self.output_path, self.fallback_path,
               self.enabled_levels,self.buffered_levels,self.record_levels)
    }
}
