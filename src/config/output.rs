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

//! Output format descriptors.

use std::fmt::{Debug, Formatter};
use crate::collections::MapWithDefault;
use crate::record::{RecordLevelId, RecordTrigger};

/// An output format descriptor describes how log or trace records are formatted for a resource.
/// An output format contains of a list of record format descriptors, since different
/// formats can be used depending on the record level and/or the cause,
/// why a record was triggered.
#[derive (Clone)]
pub struct OutputFormatDesc {
    // format name
    name: String,
    // formats specific for record level and/or trigger
    specific_formats: RecordFormatDescList
}
impl OutputFormatDesc {
    /// Creates a output format descriptor.
    ///
    /// # Arguments
    /// * `name` - the format name
    /// * `specific_formats` - the specific format descriptors
    #[inline]
    pub fn new(name: &str, specific_formats: RecordFormatDescList) -> OutputFormatDesc {
        OutputFormatDesc { name: name.to_string(), specific_formats }
    }

    /// Returns the name of this output format descriptor.
    #[inline]
    pub fn name(&self) -> &String { &self.name }

    /// Returns the level and trigger specific formats of this output format descriptor.
    #[inline]
    pub fn specific_formats(&self) -> &RecordFormatDescList { &self.specific_formats }

    /// Adds name of all record trigger/level combinations not covered by this format to the
    /// given string buffer.
    ///
    /// # Arguments
    /// * `buf` - the string receiving the comma separated list of trigger/level combinations
    pub fn list_uncovered_level_trigger_combinations(&self, buf: &mut String) {
        let msg_trg = RecordTrigger::Message;
        let cre_trg = RecordTrigger::ObserverCreated;
        let drop_trg = RecordTrigger::ObserverDropped;
        let mut msg_mask = RecordLevelId::Units as u32;
        let mut cre_mask = ! (RecordLevelId::Units as u32 | RecordLevelId::Object as u32);
        let mut drop_mask = ! (RecordLevelId::Units as u32 | RecordLevelId::Object as u32);
        for sp_fmt in &self.specific_formats {
            msg_mask |= sp_fmt.levels_covered_by_trigger(msg_trg as u32);
            cre_mask |= sp_fmt.levels_covered_by_trigger(cre_trg as u32);
            drop_mask |= sp_fmt.levels_covered_by_trigger(drop_trg as u32);
        }
        OutputFormatDesc::list_level_trigger_combination(&msg_trg, !msg_mask, buf);
        OutputFormatDesc::list_level_trigger_combination(&cre_trg, !cre_mask, buf);
        OutputFormatDesc::list_level_trigger_combination(&drop_trg, !drop_mask, buf);
    }

    fn list_level_trigger_combination(trigger: &RecordTrigger, levels: u32, buf: &mut String) {
        if levels & RecordLevelId::All as u32 == 0 { return }
        if ! buf.is_empty() { buf.push_str(", "); }
        buf.push_str(&format!("{}", trigger));
        buf.push(':');
        RecordLevelId::list_essential_id_names_in(levels, buf);
    }
}
impl Default for OutputFormatDesc {
    fn default() -> Self {
        Self {
            name: DEFAULT_FORMAT_NAME.to_string(),
            specific_formats: vec![RecordFormatDesc::message_default(),
                                   RecordFormatDesc::object_creation_default(),
                                   RecordFormatDesc::object_drop_default(),
                                   RecordFormatDesc::unit_entered_default(),
                                   RecordFormatDesc::unit_left_default()
                                  ]
        }
    }
}
impl Debug for OutputFormatDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::with_capacity(512);
        for fmt in self.specific_formats.iter() {
            if ! buf.is_empty() { buf.push(','); }
            buf.push_str(&format!("{{{:?}}}", fmt));
        }
        write!(f, "FMTS:{{{}}}", buf)
    }
}

/// A record format descriptor specifies the fields of a log or trace message in the output.
/// The components of a log or trace record are converted to a string according to this format.
#[derive(Clone)]
pub struct RecordFormatDesc {
    // bit mask of all record levels, for which the format is defined
    levels: u32,
    // bit mask of all record triggers, for which the format is defined
    triggers: u32,
    // name of custom date time format
    date_time_format_name: Option<String>,
    // list of fields that form the record format
    items: String
}
impl RecordFormatDesc {
    /// Creates a record format descriptor.
    /// 
    /// # Arguments
    /// * `levels` - the bit mask of all record levels valid for the format
    /// * `triggers` - the bit mask of all record triggers valid for the format
    /// * `items` - the format string with the specification of all fields in the format
    /// * `date_time_format_name` - the optional name of the date-time format to use
    pub fn new(levels: u32, triggers: u32, items: &str,
               date_time_format_name: Option<String>) -> RecordFormatDesc {
        RecordFormatDesc {
            levels,
            triggers,
            items: items.to_string(),
            date_time_format_name
        }
    }

    /// Creates default record format descriptor for record trigger message.
    pub fn message_default() -> RecordFormatDesc {
        RecordFormatDesc {
            levels: RecordLevelId::All as u32,
            triggers: RecordTrigger::Message as u32,
            items: DEFAULT_ITEMS_MESSAGE.to_string(),
            date_time_format_name: None
        }
    }

    /// Creates default record format descriptor for record trigger message.
    pub fn object_creation_default() -> RecordFormatDesc {
        RecordFormatDesc {
            levels: RecordLevelId::Object as u32,
            triggers: RecordTrigger::ObserverCreated as u32,
            items: DEFAULT_ITEMS_OBJ_CREATED.to_string(),
            date_time_format_name: None
        }
    }

    /// Creates default record format descriptor for record trigger message.
    pub fn object_drop_default() -> RecordFormatDesc {
        RecordFormatDesc {
            levels: RecordLevelId::Object as u32,
            triggers: RecordTrigger::ObserverDropped as u32,
            items: DEFAULT_ITEMS_OBJ_DROPPED.to_string(),
            date_time_format_name: None
        }
    }

    /// Creates default record format descriptor for record trigger message.
    pub fn unit_entered_default() -> RecordFormatDesc {
        RecordFormatDesc {
            levels: RecordLevelId::Units as u32,
            triggers: RecordTrigger::ObserverCreated as u32,
            items: DEFAULT_ITEMS_UNIT_ENTERED.to_string(),
            date_time_format_name: None
        }
    }

    /// Creates default record format descriptor for record trigger message.
    pub fn unit_left_default() -> RecordFormatDesc {
        RecordFormatDesc {
            levels: RecordLevelId::Units as u32,
            triggers: RecordTrigger::ObserverDropped as u32,
            items: DEFAULT_ITEMS_UNIT_LEFT.to_string(),
            date_time_format_name: None
        }
    }

    /// Returns the bit mask of all record levels this format applies to
    #[inline]
    pub fn levels(&self) -> u32 { self.levels }

    /// Returns the bit mask of all record triggers this format applies to
    #[inline]
    pub fn triggers(&self) -> u32 { self.triggers }

    /// Returns the format string with the specification of all fields in the format
    #[inline]
    pub fn items(&self) -> &String { &self.items }

    /// Returns the optional date-time format
    #[inline]
    pub fn date_time_format_name(&self) -> &Option<String> { &self.date_time_format_name }

    /// Returns the bit mask of all record levels covered by the given record trigger.
    #[inline]
    pub fn levels_covered_by_trigger(&self, trigger: u32) -> u32 {
        if self.triggers & trigger != 0 { self.levels } else { 0 }
    }
}
impl Default for RecordFormatDesc {
    fn default() -> Self {
        Self {
            levels: RecordLevelId::All as u32,
            triggers: RecordTrigger::All as u32,
            items: DEFAULT_ITEMS.to_string(),
            date_time_format_name: None
        }
    }
}
impl Debug for RecordFormatDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.date_time_format_name.is_none() {
            return write!(f, "L:{:b}/T:{:b}/I:{}/DF:-", self.levels, self.triggers, self.items)
        }
        write!(f, "L:{:b}/T:{:b}/I:{}/DF:{}", self.levels, self.triggers, self.items,
               self.date_time_format_name.as_ref().unwrap())
    }
}

/// Map with output format descriptors
pub(crate) type OutputFormatDescMap = MapWithDefault<OutputFormatDesc>;

/// List with specific output format descriptors
pub(crate) type RecordFormatDescList = Vec<RecordFormatDesc>;

// System default name for output formats
const DEFAULT_FORMAT_NAME: &str = "default";

// Default record format string for "plain" trace and log messages
const DEFAULT_ITEMS_MESSAGE: &str = "$TimeStamp|$LevelId|$SourceFileName:$SourceLineNr|$Message";

// Default record format string when a trace object is created
const DEFAULT_ITEMS_OBJ_CREATED: &str =
    "$TimeStamp|$LevelId|$SourceFileName:$SourceLineNr|$ObserverName created";

// Default record format string when a trace object is dropped
const DEFAULT_ITEMS_OBJ_DROPPED: &str = "$TimeStamp|$LevelId|$SourceFileName|$ObserverName dropped";

// Default record format string when a function or module is entered
const DEFAULT_ITEMS_UNIT_ENTERED: &str =
    "$TimeStamp|$LevelId|$SourceFileName:$SourceLineNr|$ObserverName -in-";

// Default record format string when a function or module is left
const DEFAULT_ITEMS_UNIT_LEFT: &str = "$TimeStamp|$LevelId|$SourceFileName|$ObserverName -out-";

// Default record format string for "plain" trace and log messages
const DEFAULT_ITEMS: &str = "$TimeStamp|$LevelId|$SourceFileName:$SourceLineNr|$ObserverName$Message";
