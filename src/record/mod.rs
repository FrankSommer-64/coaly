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

//! Types and functionality around log or trace records.

use std::collections::btree_map::Values;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use std::iter::Iterator;
use std::str::FromStr;

pub mod originator;
pub mod recorddata;

/// Record trigger, denoting the cause(s) when a log or trace message shall be issued.
#[derive (Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub enum RecordTrigger {
    /// record to be written within the function body
    Message = 0b001,
    /// record to be written when a function or module has been entered or a custom application
    /// object has been created
    ObserverCreated = 0b010,
    /// record to be written when a function or module has been left or a custom application
    /// object has been dropped
    ObserverDropped = 0b100,
    /// record to be written on all possible causes
    All = 0b111
}
impl RecordTrigger {
    fn dump(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordTrigger::Message => write!(f, "{}", RECORD_TRIGGER_MSG),
            RecordTrigger::ObserverCreated => write!(f, "{}", RECORD_TRIGGER_CRE),
            RecordTrigger::ObserverDropped => write!(f, "{}", RECORD_TRIGGER_DROP),
            RecordTrigger::All => write!(f, "{}", RECORD_TRIGGER_ALL)
        }
    }
}
impl Display for RecordTrigger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl Debug for RecordTrigger {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl FromStr for RecordTrigger {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            RECORD_TRIGGER_MSG => Ok(RecordTrigger::Message),
            RECORD_TRIGGER_CRE => Ok(RecordTrigger::ObserverCreated),
            RECORD_TRIGGER_DROP => Ok(RecordTrigger::ObserverDropped),
            RECORD_TRIGGER_ALL => Ok(RecordTrigger::All),
            _ => Err(false)
        }
    }
}
impl From<u32> for RecordTrigger {
    fn from(trigger: u32) -> RecordTrigger {
        if trigger == (RecordTrigger::All as u32) { return RecordTrigger::All }
        if trigger == (RecordTrigger::Message as u32) { return RecordTrigger::Message }
        if trigger == (RecordTrigger::ObserverCreated as u32) { return RecordTrigger::ObserverCreated }
        RecordTrigger::ObserverDropped
    }
}

/// Record level ID enumeration. Used as key in record level table.
#[derive (Clone, Copy, Eq, Hash, Ord, PartialOrd, PartialEq)]
#[repr(u32)]
pub enum RecordLevelId {
    /// system unusable
    Emergency = 0b000000000001,
    /// immediate action required
    Alert = 0b000000000010,
    /// critical condition
    Critical = 0b000000000100,
    /// operation failure
    Error = 0b000000001000,
    /// recoverable failure
    Warning = 0b000000010000,
    /// significant information
    Notice = 0b000000100000,
    /// general useful information, e.g. application progress
    Info = 0b000001000000,
    /// detailed diagnostic informations
    Debug = 0b000010000000,
    /// function entry and exit
    Function = 0b000100000000,
    /// module entry and exit
    Module = 0b001000000000,
    /// information concerning specific application objects
    Object = 0b010000000000,
    /// groups levels emergency through info
    Logs = 0b000001111111,
    /// groups levels emergency through warning
    Problems = 0b000000011111,
    /// groups levels Debug, Function, Module and Object
    Traces = 0b011110000000,
    /// groups levels Function and Module
    Units = 0b001100000000,
    /// all levels
    All = 0b011111111111
}
impl RecordLevelId {
    /// Indicates whether this record level ID stands for a group of fundamental levels.
    pub fn is_group(&self) -> bool { (*self as u32).count_ones() > 1 }

    /// Returns all essential record level IDs in the given bit mask.
    /// Essential means all ID's not denoting a group.
    pub fn essential_ids_in(levels_mask: u32) -> Vec<RecordLevelId> {
        let mut ids = Vec::<RecordLevelId>::new();
        let mut bit = 1;
        while bit <= 0b010000000000 {
            if levels_mask & bit != 0 { ids.push(RecordLevelId::from(bit)); }
            bit <<= 1;
        }
        ids
    }

    /// Returns a bit mask indicating that no record levels shall be changed.
    /// Used in mode change descriptors.
    #[inline]
    pub fn no_change_ind() -> u32 { u32::MAX }

    /// Indicates whether the given bit mask indicates that no record levels shall be changed.
    /// Used in mode change descriptors.
    #[inline]
    pub fn is_no_change_ind(bit_mask: u32) -> bool { bit_mask == u32::MAX }

    /// writes names of all essential record level IDs in the given bit mask to the specified
    /// string buffer.
    pub fn list_essential_id_names_in(levels_mask: u32, buf: &mut String) {
        for (index, id) in RecordLevelId::essential_ids_in(levels_mask).iter().enumerate() {
            if index > 0 { buf.push(','); }
            buf.push_str(&format!("{}", id));
        }
    }
    fn dump(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordLevelId::Emergency => write!(f, "{}", RECORD_LEVEL_EMERGENCY),
            RecordLevelId::Alert => write!(f, "{}", RECORD_LEVEL_ALERT),
            RecordLevelId::Critical => write!(f, "{}", RECORD_LEVEL_CRITICAL),
            RecordLevelId::Error => write!(f, "{}", RECORD_LEVEL_ERROR),
            RecordLevelId::Warning => write!(f, "{}", RECORD_LEVEL_WARNING),
            RecordLevelId::Notice => write!(f, "{}", RECORD_LEVEL_NOTICE),
            RecordLevelId::Info => write!(f, "{}", RECORD_LEVEL_INFO),
            RecordLevelId::Debug => write!(f, "{}", RECORD_LEVEL_DEBUG),
            RecordLevelId::Function => write!(f, "{}", RECORD_LEVEL_FUNCTION),
            RecordLevelId::Module => write!(f, "{}", RECORD_LEVEL_MODULE),
            RecordLevelId::Object => write!(f, "{}", RECORD_LEVEL_OBJECT),
            RecordLevelId::Logs => write!(f, "{}", RECORD_LEVEL_LOGS),
            RecordLevelId::Problems => write!(f, "{}", RECORD_LEVEL_PROBLEMS),
            RecordLevelId::Traces => write!(f, "{}", RECORD_LEVEL_TRACES),
            RecordLevelId::Units => write!(f, "{}", RECORD_LEVEL_UNITS),
            RecordLevelId::All => write!(f, "{}", RECORD_LEVEL_ALL),
        }
    }
}
impl Debug for RecordLevelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl Display for RecordLevelId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl FromStr for RecordLevelId {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            RECORD_LEVEL_EMERGENCY => Ok(RecordLevelId::Emergency),
            RECORD_LEVEL_ALERT => Ok(RecordLevelId::Alert),
            RECORD_LEVEL_CRITICAL => Ok(RecordLevelId::Critical),
            RECORD_LEVEL_ERROR => Ok(RecordLevelId::Error),
            RECORD_LEVEL_WARNING => Ok(RecordLevelId::Warning),
            RECORD_LEVEL_NOTICE => Ok(RecordLevelId::Notice),
            RECORD_LEVEL_INFO => Ok(RecordLevelId::Info),
            RECORD_LEVEL_DEBUG => Ok(RecordLevelId::Debug),
            RECORD_LEVEL_FUNCTION => Ok(RecordLevelId::Function),
            RECORD_LEVEL_MODULE => Ok(RecordLevelId::Module),
            RECORD_LEVEL_OBJECT => Ok(RecordLevelId::Object),
            RECORD_LEVEL_LOGS => Ok(RecordLevelId::Logs),
            RECORD_LEVEL_PROBLEMS => Ok(RecordLevelId::Problems),
            RECORD_LEVEL_TRACES => Ok(RecordLevelId::Traces),
            RECORD_LEVEL_UNITS => Ok(RecordLevelId::Units),
            RECORD_LEVEL_ALL => Ok(RecordLevelId::All),
            _ => Err(false)
        }
    }
}
impl From<u32> for RecordLevelId {
    /// Converts a 32-Bit bit mask to RecordLevelId.
    /// Needed for conversion from ObserverKind enumeration value.
    /// 
    /// # Arguments
    /// * `bit_mask` - the bit mask for observer kind
    ///
    /// # Return values
    /// The record level ID
    fn from(bit_mask: u32) -> RecordLevelId {
        if bit_mask & (RecordLevelId::Emergency as u32) != 0 { return RecordLevelId::Emergency }
        if bit_mask & (RecordLevelId::Alert as u32) != 0 { return RecordLevelId::Alert }
        if bit_mask & (RecordLevelId::Critical as u32) != 0 { return RecordLevelId::Critical }
        if bit_mask & (RecordLevelId::Error as u32) != 0 { return RecordLevelId::Error }
        if bit_mask & (RecordLevelId::Warning as u32) != 0 { return RecordLevelId::Warning }
        if bit_mask & (RecordLevelId::Notice as u32) != 0 { return RecordLevelId::Notice }
        if bit_mask & (RecordLevelId::Info as u32) != 0 { return RecordLevelId::Info }
        if bit_mask & (RecordLevelId::Debug as u32) != 0 { return RecordLevelId::Debug }
        if bit_mask & (RecordLevelId::Function as u32) != 0 { return RecordLevelId::Function }
        if bit_mask & (RecordLevelId::Module as u32) != 0 { return RecordLevelId::Module }
        RecordLevelId::Object
    }
}

/// A RecordLevel denotes the severity of a log or trace message.
/// Both ID character and name are customizable.
#[derive (Clone)]
pub struct RecordLevel {
    id: RecordLevelId,
    id_char: char,
    name: String
}
impl RecordLevel {
    /// Creates a RecordLevel.
    ///
    /// # Arguments
    /// * `id' - the record level ID
    /// * `id_char' - the record level ID character
    /// * `name' - the record level name
    pub fn new (id: RecordLevelId, id_char: char, name: &str) -> RecordLevel {
        RecordLevel {
            id,
            id_char,
            name: name.to_string()
        }
    }

    /// Returns the ID of this RecordLevel.
    #[inline]
    pub fn id (&self) -> &RecordLevelId { &self.id }

    /// Returns the ID character of this RecordLevel.
    #[inline]
    pub fn id_char (&self) -> char { self.id_char }

    /// Sets the ID character of this RecordLevel.
    #[inline]
    pub fn set_id_char (&mut self, ch: char) { self.id_char = ch }

    /// Returns the name of this RecordLevel.
    #[inline]
    pub fn name (&self) -> &String { &self.name }

    /// Sets the name of this RecordLevel.
    #[inline]
    pub fn set_name (&mut self, name: &str) { self.name = name.to_string() }

    /// Creates a RecordLevel with default ID character and name for the specified level.
    ///
    /// # Arguments
    /// * `id' - the record level ID
    pub fn default_for (lvl_str: &str) -> Result<RecordLevel, bool> {
        let l_id = RecordLevelId::from_str(lvl_str);
        if l_id.is_err() { return Err(false) }
        let l_id = l_id.unwrap();
        if l_id.is_group() { return Err(false) }
        Ok(RecordLevel::new(l_id, RecordLevel::default_id_char_for(&l_id),
                            RecordLevel::default_name_for(&l_id)))
    }

    /// Returns the default ID character for the specified record level ID.
    /// Returns an asterisk if a grouped level is passed.
    ///
    /// # Arguments
    /// * `id' - the record level ID
    pub fn default_id_char_for (id: &RecordLevelId) -> char {
        match id {
            RecordLevelId::Emergency => DEFAULT_RECORD_LEVEL_ID_EMERGENCY,
            RecordLevelId::Alert => DEFAULT_RECORD_LEVEL_ID_ALERT,
            RecordLevelId::Critical => DEFAULT_RECORD_LEVEL_ID_CRITICAL,
            RecordLevelId::Error => DEFAULT_RECORD_LEVEL_ID_ERROR,
            RecordLevelId::Warning => DEFAULT_RECORD_LEVEL_ID_WARNING,
            RecordLevelId::Notice => DEFAULT_RECORD_LEVEL_ID_NOTICE,
            RecordLevelId::Info => DEFAULT_RECORD_LEVEL_ID_INFO,
            RecordLevelId::Debug => DEFAULT_RECORD_LEVEL_ID_DEBUG,
            RecordLevelId::Function => DEFAULT_RECORD_LEVEL_ID_FUNCTION,
            RecordLevelId::Module => DEFAULT_RECORD_LEVEL_ID_MODULE,
            RecordLevelId::Object => DEFAULT_RECORD_LEVEL_ID_OBJECT,
            _ => DEFAULT_RECORD_LEVEL_ID_GROUP
        }
    }

    /// Returns the default name for the specified record level ID.
    /// Returns three asterisks if a grouped level is passed.
    ///
    /// # Arguments
    /// * `id' - the record level ID
    pub fn default_name_for (id: &RecordLevelId) -> &str {
        match id {
            RecordLevelId::Emergency => DEFAULT_RECORD_LEVEL_NAME_EMERGENCY,
            RecordLevelId::Alert => DEFAULT_RECORD_LEVEL_NAME_ALERT,
            RecordLevelId::Critical => DEFAULT_RECORD_LEVEL_NAME_CRITICAL,
            RecordLevelId::Error => DEFAULT_RECORD_LEVEL_NAME_ERROR,
            RecordLevelId::Warning => DEFAULT_RECORD_LEVEL_NAME_WARNING,
            RecordLevelId::Notice => DEFAULT_RECORD_LEVEL_NAME_NOTICE,
            RecordLevelId::Info => DEFAULT_RECORD_LEVEL_NAME_INFO,
            RecordLevelId::Debug => DEFAULT_RECORD_LEVEL_NAME_DEBUG,
            RecordLevelId::Function => DEFAULT_RECORD_LEVEL_NAME_FUNCTION,
            RecordLevelId::Module => DEFAULT_RECORD_LEVEL_NAME_MODULE,
            RecordLevelId::Object => DEFAULT_RECORD_LEVEL_NAME_OBJECT,
            _ => DEFAULT_RECORD_LEVEL_NAME_GROUP
        }
    }
}
impl Debug for RecordLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ID:{}/CH:{}/N:{}}}", self.id, self.id_char, self.name)
    }
}

/// Hash map with all record level informations.
#[derive (Clone)]
pub struct RecordLevelMap(BTreeMap<RecordLevelId, RecordLevel>);
impl RecordLevelMap {
    /// Creates an empty record level map.
    #[inline]
    pub fn new() -> RecordLevelMap {
        RecordLevelMap { 0: BTreeMap::<RecordLevelId, RecordLevel>::new() }
    }

    /// Returns a reference to the record level matching the specified record level ID.
    ///
    /// # Arguments
    /// * `id' - the record level ID
    ///
    /// # Return values
    /// The record level descriptor for the specified record level ID, if it exists;
    /// otherwise **None**
    #[inline]
    pub fn get(&self, id: &RecordLevelId) -> Option<&RecordLevel> { self.0.get(id) }

    /// Adds a record level to the map.
    /// Insertion fails, if the map contains already an entry with the same record level ID,
    /// or the record level ID character or name is already used for another record level.
    #[inline]
    pub fn add(&mut self, lvl: RecordLevel) -> bool {
        if self.0.contains_key(&lvl.id) { return false }
        for l in self.0.values() {
            if l.id_char == lvl.id_char || l.name == lvl.name { return false }
        }
        self.0.insert(lvl.id, lvl);
        true
    }

    /// Returns a reference to the record level matching the specified record level ID.
    ///
    /// # Arguments
    /// * `id' - the record level ID
    ///
    /// # Return values
    /// The record level descriptor for the specified record level ID, if it exists;
    /// otherwise **None**
    #[inline]
    pub fn values(&self) -> Values<RecordLevelId, RecordLevel> { self.0.values() }

    /// Adds missing entries with their default values.
    /// Operation fails, if the existing entries use the same ID character or name as one of
    /// the missing default entries.
    pub fn fill_defaults(&mut self) -> bool {
        let def_map = RecordLevelMap::default();
        for def_lvl in def_map.values() {
            if self.0.contains_key(def_lvl.id()) { continue }
            if ! self.add(def_lvl.clone()) { return false }
        }
        true
    }
}
impl Default for RecordLevelMap {
    fn default() -> Self {
        let mut t = RecordLevelMap { 0: BTreeMap::<RecordLevelId, RecordLevel>::new() };
        t.0.insert(RecordLevelId::Emergency, RecordLevel::new(RecordLevelId::Emergency,
                                                          DEFAULT_RECORD_LEVEL_ID_EMERGENCY,
                                                          DEFAULT_RECORD_LEVEL_NAME_EMERGENCY));
        t.0.insert(RecordLevelId::Alert, RecordLevel::new(RecordLevelId::Alert,
                                                          DEFAULT_RECORD_LEVEL_ID_ALERT,
                                                          DEFAULT_RECORD_LEVEL_NAME_ALERT));
        t.0.insert(RecordLevelId::Critical, RecordLevel::new(RecordLevelId::Critical,
                                                          DEFAULT_RECORD_LEVEL_ID_CRITICAL,
                                                          DEFAULT_RECORD_LEVEL_NAME_CRITICAL));
        t.0.insert(RecordLevelId::Error, RecordLevel::new(RecordLevelId::Error,
                                                          DEFAULT_RECORD_LEVEL_ID_ERROR,
                                                          DEFAULT_RECORD_LEVEL_NAME_ERROR));
        t.0.insert(RecordLevelId::Warning, RecordLevel::new(RecordLevelId::Warning,
                                                            DEFAULT_RECORD_LEVEL_ID_WARNING,
                                                            DEFAULT_RECORD_LEVEL_NAME_WARNING));
        t.0.insert(RecordLevelId::Notice, RecordLevel::new(RecordLevelId::Notice,
                                                         DEFAULT_RECORD_LEVEL_ID_NOTICE,
                                                         DEFAULT_RECORD_LEVEL_NAME_NOTICE));
        t.0.insert(RecordLevelId::Info, RecordLevel::new(RecordLevelId::Info,
                                                         DEFAULT_RECORD_LEVEL_ID_INFO,
                                                         DEFAULT_RECORD_LEVEL_NAME_INFO));
        t.0.insert(RecordLevelId::Debug, RecordLevel::new(RecordLevelId::Debug,
                                                          DEFAULT_RECORD_LEVEL_ID_DEBUG,
                                                          DEFAULT_RECORD_LEVEL_NAME_DEBUG));
        t.0.insert(RecordLevelId::Function, RecordLevel::new(RecordLevelId::Function,
                                                             DEFAULT_RECORD_LEVEL_ID_FUNCTION,
                                                             DEFAULT_RECORD_LEVEL_NAME_FUNCTION));
        t.0.insert(RecordLevelId::Module, RecordLevel::new(RecordLevelId::Module,
                                                           DEFAULT_RECORD_LEVEL_ID_MODULE,
                                                           DEFAULT_RECORD_LEVEL_NAME_MODULE));
        t.0.insert(RecordLevelId::Object, RecordLevel::new(RecordLevelId::Object,
                                                           DEFAULT_RECORD_LEVEL_ID_OBJECT,
                                                           DEFAULT_RECORD_LEVEL_NAME_OBJECT));
        t
    }
}
impl Debug for RecordLevelMap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::with_capacity(128);
        for lvl in self.0.values() {
            if ! buf.is_empty() { buf.push(','); }
            buf.push_str(&format!("{:?}", lvl));
        }
        write!(f, "{}", buf)
    }
}

// Names for all record triggers
const RECORD_TRIGGER_ALL: &str = "all";
const RECORD_TRIGGER_CRE: &str = "creation";
const RECORD_TRIGGER_DROP: &str = "drop";
const RECORD_TRIGGER_MSG: &str = "message";

// Keys for all record levels
const RECORD_LEVEL_EMERGENCY: &str = "emergency";
const RECORD_LEVEL_ALERT: &str = "alert";
const RECORD_LEVEL_CRITICAL: &str = "critical";
const RECORD_LEVEL_ERROR: &str = "error";
const RECORD_LEVEL_WARNING: &str = "warning";
const RECORD_LEVEL_NOTICE: &str = "notice";
const RECORD_LEVEL_INFO: &str = "info";
const RECORD_LEVEL_DEBUG: &str = "debug";
const RECORD_LEVEL_FUNCTION: &str = "function";
const RECORD_LEVEL_MODULE: &str = "module";
const RECORD_LEVEL_OBJECT: &str = "object";
const RECORD_LEVEL_LOGS: &str = "logs";
const RECORD_LEVEL_PROBLEMS: &str = "problems";
const RECORD_LEVEL_TRACES: &str = "traces";
const RECORD_LEVEL_UNITS: &str = "units";
const RECORD_LEVEL_ALL: &str = "all";

// Default ID characters for all record levels.
// The ID character replaces variable $LevelId in output formats.
const DEFAULT_RECORD_LEVEL_ID_EMERGENCY : char = 'Y';
const DEFAULT_RECORD_LEVEL_ID_ALERT : char = 'A';
const DEFAULT_RECORD_LEVEL_ID_CRITICAL : char = 'C';
const DEFAULT_RECORD_LEVEL_ID_ERROR : char = 'E';
const DEFAULT_RECORD_LEVEL_ID_WARNING : char = 'W';
const DEFAULT_RECORD_LEVEL_ID_NOTICE : char = 'N';
const DEFAULT_RECORD_LEVEL_ID_INFO : char = 'I';
const DEFAULT_RECORD_LEVEL_ID_DEBUG : char = 'D';
const DEFAULT_RECORD_LEVEL_ID_FUNCTION : char = 'F';
const DEFAULT_RECORD_LEVEL_ID_MODULE : char = 'M';
const DEFAULT_RECORD_LEVEL_ID_OBJECT : char = 'O';
const DEFAULT_RECORD_LEVEL_ID_GROUP : char = '*';

// Default names for all record levels.
// The name replaces variable $Level in output formats.
const DEFAULT_RECORD_LEVEL_NAME_EMERGENCY : &str = "EMGCY";
const DEFAULT_RECORD_LEVEL_NAME_ALERT : &str = "ALERT";
const DEFAULT_RECORD_LEVEL_NAME_CRITICAL : &str = "CRIT";
const DEFAULT_RECORD_LEVEL_NAME_ERROR : &str = "ERROR";
const DEFAULT_RECORD_LEVEL_NAME_WARNING : &str = "WARN";
const DEFAULT_RECORD_LEVEL_NAME_NOTICE : &str = "NOTICE";
const DEFAULT_RECORD_LEVEL_NAME_INFO : &str = "INFO";
const DEFAULT_RECORD_LEVEL_NAME_DEBUG : &str = "DEBUG";
const DEFAULT_RECORD_LEVEL_NAME_FUNCTION : &str = "FUNC";
const DEFAULT_RECORD_LEVEL_NAME_MODULE : &str = "MOD";
const DEFAULT_RECORD_LEVEL_NAME_OBJECT : &str = "OBJ";
const DEFAULT_RECORD_LEVEL_NAME_GROUP : &str = "***";

#[cfg(all(net, test))]
mod tests {
    use crate::net::serializable::Serializable;
    use core::fmt::Debug;

    pub fn check_serialization<'a, T>(item: &'a T, expected_size: usize, buffer: &'a mut Vec<u8>)
        where T: Serializable<'a> + Debug + Eq {
        buffer.clear();
        assert_eq!(expected_size, item.serialized_size());
        let sz = item.serialize_to(buffer);
        assert_eq!(expected_size, sz);
        assert_eq!(buffer.len(), sz);
        let clone = T::deserialize_from(buffer);
        assert!(clone.is_ok());
        assert_eq!(clone.unwrap(), *item);
    }
}