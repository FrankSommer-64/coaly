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

//! Status handling for every application thread.

use std::collections::HashMap;
use crate::collections::RecoverableStack;
use crate::config::Configuration;
use crate::modechange::OverrideModeMap;
use crate::output::Interface;

/// Holds the data about a client thread as it is needed by the worker thread.
pub(crate) struct ThreadStatus {
    // Map for output modes controlled by custom objects
    obj_mode_map: OverrideModeMap,
    // Stack for output modes controlled by functions and modules
    unit_mode_stack: RecoverableStack<u32>,
    // List of output resources
    pub(crate) output_interface: Interface
}
impl ThreadStatus {
    pub(crate) fn new(intf: Interface, config: &Configuration) -> ThreadStatus {
        let st_size = config.system_properties().change_stack_size();
        let mut unit_mode_stack = RecoverableStack::<u32>::new(st_size, 256);
        unit_mode_stack.push(config.system_properties().initial_output_mode());
        ThreadStatus {
            obj_mode_map: OverrideModeMap::new(32768),
            unit_mode_stack,
            output_interface: intf
        }
    }

    /// Returns the active output mode.
    /// Mode changes triggered by custom objects have priority over functions and modules.
    /// 
    /// # Return values
    /// the bit mask with buffered/enabled record levels
    pub(crate) fn active_mode(&self) -> u32 {
        let mode = self.obj_mode_map.active_mode();
        if mode != u32::MAX { return mode }
        *self.unit_mode_stack.last().unwrap()
    }

    /// Pushes a last mode change to the functions and modules stack.
    pub(crate) fn unit_entered(&mut self, mode: u32) -> u32 {
        let new_mode = self.actual_mode(mode);
        self.unit_mode_stack.push(new_mode);
        new_mode
    }

    /// Removes the last mode change from the functions and modules stack.
    pub(crate) fn unit_left(&mut self) { self.unit_mode_stack.pop(); }

    /// Adds a mode change to the custom objects map.
    pub(crate) fn object_created(&mut self, observer_id: u64, mode: u32) -> u32 {
        let new_mode = self.actual_mode(mode);
        self.obj_mode_map.matching_observer_created(observer_id, new_mode);
        new_mode
    }

    /// Removes the last mode change from the custom objects map.
    pub(crate) fn object_dropped(&mut self, observer_id: u64) {
        self.obj_mode_map.matching_observer_dropped(observer_id);
    }

    fn actual_mode(&self, mode: u32) -> u32 {
        let curr_mode = self.active_mode();
        let curr_enabled = curr_mode & 0xffff;
        let curr_buffered = curr_mode & 0xffff0000;
        let mut new_enabled = mode & 0xffff;
        let mut new_buffered = mode & 0xffff0000;
        if new_enabled == 0xffff { new_enabled = curr_enabled; }
        if new_buffered == 0xffff0000 { new_buffered = curr_buffered; }
        new_buffered | new_enabled
    }
}
pub(crate) type ThreadStatusTable = HashMap<u64, ThreadStatus>;
