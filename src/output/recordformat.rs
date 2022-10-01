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

//! Format descriptor for Coaly log or trace records.

use std::str::FromStr;
use crate::config::datetimeformat::{DateTimeFormatDesc, DateTimeFormatDescMap};
use crate::config::output::{RecordFormatDesc};
use crate::record::{RecordLevelId, RecordLevelMap, RecordTrigger};
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;
use super::formatspec::{FormatSpec};

/// A record format structure specifies the fields of a log or trace message in the output.
/// The components of a log or trace record are converted to a string according to this format.
#[derive(Clone, Debug)]
pub(crate) struct RecordFormat {
    // bit mask of all record levels, for which the format is defined
    levels: u32,
    // bit mask of all record triggers, for which the format is defined
    triggers: u32,
    // format for timestamp values
    timestamp_format: String,
    // format for date values
    date_format: String,
    // format for time values
    time_format: String,
    // list of fields that form the record format
    fields: FormatSpec
}
impl RecordFormat {
    /// Creates a record format.
    /// 
    /// # Arguments
    /// * `levels` - the bit mask of all record levels valid for the format
    /// * `triggers` - the bit mask of all record triggers valid for the format
    /// * `ts_fmt` - the format string for timestamp values
    /// * `date_fmt` - the format string for date values
    /// * `tm_fmt` - the format string for time values
    /// * `fields` - the specification of all fields in the format
    pub(crate) fn new(levels: u32, triggers: u32,
               ts_fmt: &str, date_fmt: &str, tm_fmt: &str,
               fields: FormatSpec) -> RecordFormat {
        RecordFormat {
            levels,
            triggers,
            timestamp_format: ts_fmt.to_string(),
            date_format: date_fmt.to_string(),
            time_format: tm_fmt.to_string(),
            fields
        }
    }

    /// Creates a specific format.
    ///
    /// # Arguments
    /// * `desc` - the record format descriptor from the configuration
    /// * `dtm_formats` - the map with all date time formats
    pub(crate) fn from_desc(desc: &RecordFormatDesc,
                            dtm_formats: &DateTimeFormatDescMap) -> RecordFormat {
        let dtm_fmt: &DateTimeFormatDesc = dtm_formats.find(desc.date_time_format_name());
        let items = FormatSpec::from_str(desc.items()).unwrap();
        RecordFormat::new(desc.levels(), desc.triggers(),
                          &dtm_fmt.timestamp_format_for_recs(),
                          &dtm_fmt.date_format_for_recs(),
                          &dtm_fmt.time_format_for_recs(),
                          items)
    }

    /// Indicates, whether the given record level and trigger are within the scope of
    /// this record format.
    ///
    /// # Arguments
    /// * `level` - the bit mask of the record level
    /// * `trigger` - the bit mask of the record trigger
    ///
    /// # Return values
    /// `true` if the record format is appropriate for the level and trigger
    pub(crate) fn applies_to(&self, level: RecordLevelId, trigger: RecordTrigger) -> bool {
        self.levels & level as u32 != 0 && self.triggers & trigger as u32 != 0
    }

    /// Converts the specified log or trace record to a string according to this format.
    /// The caller must make sure, that the record is within the scope of this format by invoking
    /// function `applies_to`. The check is not done within this function.
    ///
    /// # Arguments
    /// * `record` - the record data
    /// * `levels` - the hash table with the ID character for every record level
    ///
    /// # Return values
    /// the formatted string, to be written to output resource
    pub(crate) fn apply_to(&self, record: &dyn RecordData, levels: &RecordLevelMap) -> String {
        self.fields.apply_to_record(record, levels,
                                    &self.timestamp_format, &self.date_format, &self.time_format)
    }

    /// Optimizes the format.
    /// Variable items, whose values remain constant throughout the entire lifetime of the
    /// originator thread are replaced by constant items with the corresponding value.
    /// Adjacent constant items are combined.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    pub(crate) fn optimize_for(&mut self,
                               orig_info: &OriginatorInfo,
                               thread_id: u64,
                               thread_name: &str) {
        self.fields = self.fields.optimized_for(orig_info, thread_id, thread_name);
    }
}
