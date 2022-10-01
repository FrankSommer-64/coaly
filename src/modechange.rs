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

//! Types and descriptor for output mode changes.

use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::str::FromStr;
use crate::observer::ObserverKind;

/// Scope being affected by an output mode change
#[derive (Clone, Copy, PartialEq)]
pub(crate) enum ModeChangeScope {
    /// mode change affects all application threads
    Process,
    /// mode change affects application thread that triggered the mode change only
    Thread
}
impl Default for ModeChangeScope {
    fn default() -> Self { ModeChangeScope::Thread }
}
impl Debug for ModeChangeScope {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeChangeScope::Process => write!(f, "{}", SCOPE_PROCESS),
            ModeChangeScope::Thread => write!(f, "{}", SCOPE_THREAD)
        }
    }
}
impl FromStr for ModeChangeScope {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            SCOPE_PROCESS => Ok(ModeChangeScope::Process),
            SCOPE_THREAD => Ok(ModeChangeScope::Thread),
            _ => Err(false)
        }
    }
}

/// Descriptor for an output mode change triggered by a Coaly observer structure.
#[derive(Clone)]
pub(crate) struct ModeChangeDesc {
    // scope for the mode change (process or thread)
    scope: ModeChangeScope,
    // kind of the observer responsible for the mode change (function, module or object)
    observer_kind: ObserverKind,
    // name of the observer responsible for the mode change, mandatory for function or module,
    // optional for object (but then observer value must be specified)
    observer_name: Option<Regex>,
    // value of the observer responsible for the mode change, None for function or module,
    // optional for object (but then observer name must be specified)
    observer_value: Option<Regex>,
    // bit mask with all record levels enabled after the change
    enabled_levels: u32,
    // bit mask with all record levels buffered after the change
    buffered_levels: u32
}
impl ModeChangeDesc {
    /// Creates a mode change descriptor for a unit boundary observer structure.
    /// The mode change is activated immediately upon enter of a function or a module, and is
    /// deactivated after the function or module is left.
    ///
    /// # Arguments
    /// * `observer_kind` - the kind of the structure (function, module)
    /// * `observer_name` - the user defined name of the structure. Pattern must be checked in
    ///                     forehand, otherwise this function will panic
    /// * `enabled_levels` - the bit mask with all record levels enabled after the change
    /// * `buffered_levels` - the bit mask with all record levels buffered after the change
    pub(crate) fn for_unit(observer_kind: ObserverKind,
                           observer_name: Option<Regex>,
                           enabled_levels: u32,
                           buffered_levels: u32) -> ModeChangeDesc {
        ModeChangeDesc {
            scope: ModeChangeScope::Thread,
            observer_kind,
            observer_name,
            observer_value: None,
            enabled_levels,
            buffered_levels
        }
    }

    /// Creates a mode change descriptor for a user defined observer structure.
    /// The observer structure must implement the CoalyObserver trait.
    ///
    /// # Arguments
    /// * `scope` - the scope for the mode change (process or thread)
    /// * `observer_name` - the optional name of the user defined observer structure
    /// * `observer_value` - the optional value of the user defined observer structure
    /// * `enabled_levels` - the bit mask with all record levels enabled after the change
    /// * `buffered_levels` - the bit mask with all record levels buffered after the change
    pub(crate) fn for_object(scope: ModeChangeScope,
                             observer_name: Option<Regex>,
                             observer_value: Option<Regex>,
                             enabled_levels: u32,
                             buffered_levels: u32) -> ModeChangeDesc {
        ModeChangeDesc {
            scope,
            observer_kind: ObserverKind::Object,
            observer_name,
            observer_value,
            enabled_levels,
            buffered_levels
        }
    }

    /// Indicates, whether this mode change applies to an observer structure
    /// with specified name and/or value.
    /// At least one of name and value must be specified, otherwise this function will return
    /// **false**.
    ///
    /// # Arguments
    /// * `observer_name` - the observer name
    /// * `observer_value` - the observer value
    ///
    /// # Return values
    /// `true` if this mode change is defined for observers with the specified name and/or
    /// value matches 
    pub(crate) fn applies_to(&self,
                             observer_name: Option<&str>,
                             observer_value: Option<&str>) -> bool {
        if let Some(my_oname) = self.observer_name.as_ref() {
            if observer_name.is_none() { return false }
            if ! my_oname.is_match(observer_name.unwrap()) { return false }
        }
        if let Some(my_ovalue) = self.observer_value.as_ref() {
            if observer_value.is_none() { return false }
            return my_ovalue.is_match(observer_value.unwrap())
        }
        true
    }
}
impl Debug for ModeChangeDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.observer_name.is_none() && self.observer_value.is_none() {
            return write!(f, "SC:{:?}/K:{:?}/N:-/V:-/ENA:{:b}/BUF:{:b}",
                          self.scope, self.observer_kind,
                          self.enabled_levels, self.buffered_levels)
        }
        if self.observer_name.is_none() {
            return write!(f, "SC:{:?}/K:{:?}/N:-/V:{}/ENA:{:b}/BUF:{:b}",
                          self.scope, self.observer_kind, self.observer_value.as_ref().unwrap(),
                          self.enabled_levels, self.buffered_levels)
        }
        if self.observer_value.is_none() {
            return write!(f, "SC:{:?}/K:{:?}/N:{}/V:-/ENA:{:b}/BUF:{:b}",
                          self.scope, self.observer_kind, self.observer_name.as_ref().unwrap(),
                          self.enabled_levels, self.buffered_levels)
        }
        write!(f, "SC:{:?}/K:{:?}/N:{}/V:{}/ENA:{:b}/BUF:{:b}",
               self.scope, self.observer_kind,
               self.observer_name.as_ref().unwrap(), self.observer_value.as_ref().unwrap(),
               self.enabled_levels, self.buffered_levels)
    }
}

/// List with all custom mode change descriptors.
/// Internally holds 3 different lists, one for global changes triggered by custom objects and
/// each one for thread local changes triggered by units resp. custom objects.
#[derive(Clone)]
pub(crate) struct ModeChangeDescList {
    // Descriptors for process wide mode changes, triggered by custom objects
    global_obj_descs: Vec<ModeChangeDesc>,
    // Descriptors for thread specific mode changes, triggered by custom objects
    local_obj_descs: Vec<ModeChangeDesc>,
    // Descriptors for thread specific mode changes, triggered by functions or modules
    local_unit_descs: Vec<ModeChangeDesc>
}
impl ModeChangeDescList {
    /// Creates an empty list of mode change descriptors.
    #[inline]
    pub(crate) fn new() -> ModeChangeDescList {
        ModeChangeDescList {
            global_obj_descs: Vec::<ModeChangeDesc>::new(),
            local_obj_descs: Vec::<ModeChangeDesc>::new(),
            local_unit_descs: Vec::<ModeChangeDesc>::new()
        }
    }

    /// Inserts a mode change descriptor.
    /// The descriptor is appended to the end of the internal list matching the descriptor's
    /// trigger and scope.
    /// 
    /// # Arguments
    /// * `desc` - the mode change descriptor to add
    pub(crate) fn push(&mut self, desc: ModeChangeDesc) {
        match desc.observer_kind {
            ObserverKind::Object => {
                if desc.scope == ModeChangeScope::Process {
                    self.global_obj_descs.push(desc)
                } else {
                    self.local_obj_descs.push(desc)
                }
            },
            _ => self.local_unit_descs.push(desc)
        }
    }

    /// Iterates over all process wide mode change descriptors and returns the bit mask
    /// for enabled and buffered record levels specified in the first matching descriptor.
    /// 
    /// # Arguments
    /// * `observer_name` - the observer's name
    /// * `observer_value` - the observer's value
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels, u32::MAX if no match found
    #[inline]
    pub(crate) fn global_mode_for_obj(&self,
                                      observer_name: Option<&str>,
                                      observer_value: Option<&str>) -> u32 {
        ModeChangeDescList::mode_for(&self.global_obj_descs, observer_name, observer_value)
    }

    /// Iterates over all thread specific mode change descriptors for custom objects and returns
    /// the bit mask for enabled and buffered record levels specified in the first
    /// matching descriptor.
    /// 
    /// # Arguments
    /// * `observer_name` - the observer's name
    /// * `observer_value` - the observer's value
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels, u32::MAX if no match found
    #[inline]
    pub(crate) fn local_mode_for_obj(&self,
                                     observer_name: Option<&str>,
                                     observer_value: Option<&str>) -> u32 {
        ModeChangeDescList::mode_for(&self.local_obj_descs, observer_name, observer_value)
    }

    /// Iterates over all thread specific mode change descriptors for units and returns
    /// the bit mask for enabled and buffered record levels specified in the first
    /// matching descriptor.
    /// 
    /// # Arguments
    /// * `observer_name` - the observer's name
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels, u32::MAX if no match found
    #[inline]
    pub(crate) fn local_mode_for_unit(&self, observer_name: Option<&str>) -> u32 {
        ModeChangeDescList::mode_for(&self.local_unit_descs, observer_name, None)
    }

    /// Iterates over all mode change descriptors in the given list and returns the bit mask
    /// for enabled and buffered record levels specified in the first matching descriptor.
    /// 
    /// # Arguments
    /// * `observer_name` - the observer's name
    /// * `observer_value` - the observer's value
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels, u32::MAX if no match found
    fn mode_for(descs: &[ModeChangeDesc],
                observer_name: Option<&str>,
                observer_value: Option<&str>) -> u32 {
        for desc in descs.iter() {
            if desc.applies_to(observer_name, observer_value) {
                return (desc.buffered_levels << 16) | desc.enabled_levels
            }
        }
        u32::MAX
    }

    /// Iterates over all mode change descriptors in the given list and returns the bit mask
    /// for enabled and buffered record levels specified in the first matching descriptor.
    /// 
    /// # Arguments
    /// * `observer_name` - the observer's name
    /// * `observer_value` - the observer's value
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels, u32::MAX if no match found
    fn dump(hdr: &str,
            descs: &[ModeChangeDesc],
            buffer: &mut String) {
        buffer.push_str(hdr);
        buffer.push('[');
        for (index, desc) in descs.iter().enumerate() {
            if index > 0 { buffer.push(','); }
            buffer.push_str(&format!("{{{:?}}}", desc));
        }
        buffer.push(']');
    }
}
impl Default for ModeChangeDescList {
    fn default() -> Self { ModeChangeDescList::new() }
}
impl Debug for ModeChangeDescList {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::with_capacity(512);
        ModeChangeDescList::dump("GO:", &self.global_obj_descs, &mut buf);
        buf.push('/');
        ModeChangeDescList::dump("LO:", &self.local_obj_descs, &mut buf);
        buf.push('/');
        ModeChangeDescList::dump("LU:", &self.local_unit_descs, &mut buf);
        write!(f, "{{{}}}", buf)
    }
}

/// Structure for the administration of process wide mode changes.
/// These mode changes may have overlapping lifetimes, hence a stack as for thread specific
/// mode changes cannot be used. Significant for the current mode is the most recent activated
/// change, changes may get deactivated and removed from the structure without being the most
/// recent one.
#[derive(Clone, Debug)]
pub(crate) struct OverrideModeMap {
    // active changes, key is observer ID and value the bit mask for active/buffered record levels
    active_changes: BTreeMap<u64, u32>,
    // maximum allowed number of entries of the map
    size_limit: usize
}
impl OverrideModeMap {
    /// Creates an empty map for the administration of process wide mode changes.
    /// 
    /// # Arguments
    /// * `size_limit` - the maximum allowed number of entries
    #[inline]
    pub(crate) fn new(size_limit: usize) -> OverrideModeMap {
        OverrideModeMap {
            active_changes: BTreeMap::<u64, u32>::new(),
            size_limit
        }
    }

    /// Inserts a mode change into the list.
    /// Invoked, if an observer matching the specified conditions has been created.
    /// If the size limit of the list was exceeded, the call is ignored.
    /// 
    /// # Arguments
    /// * `observer_id` - the observer's ID
    /// * `mode` - the bit mask for active/buffered record levels
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels to use for the current output record
    pub(crate) fn matching_observer_created(&mut self,
                                            observer_id: u64,
                                            mode: u32) -> u32 {
        if self.active_changes.len() >= self.size_limit { return self.active_mode() }
        self.active_changes.insert(observer_id, mode);
        mode
    }

    /// Checks whether the given record indicating an observer drop causes a change
    /// to be reverted and updates the map if this is the case.
    /// 
    /// # Arguments
    /// * `observer_id` - the observer's ID
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels to use for the current output record
    #[inline]
    pub(crate) fn matching_observer_dropped(&mut self,
                                            observer_id: u64) -> u32 {
        let mode = self.active_mode();
        self.active_changes.remove(&observer_id);
        mode
    }

    /// Returns the bit mask for currently active/buffered record levels.
    ///
    /// # Return values
    /// the bit mask for active/buffered record levels, u32::MAX if no changes are active
    #[inline]
    pub(crate) fn active_mode(&self) -> u32 {
        if self.active_changes.is_empty() { return u32::MAX }
        *self.active_changes.iter().last().unwrap().1
    }
}

// Mode change scope names
const SCOPE_PROCESS: &str = "process";
const SCOPE_THREAD: &str = "thread";
