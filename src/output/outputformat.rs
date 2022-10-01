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

//! Record and name formats for Coaly.

use crate::config::datetimeformat::DateTimeFormatDescMap;
use crate::config::output::{OutputFormatDesc, RecordFormatDesc};
use crate::record::RecordLevelMap;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;
use super::recordformat::RecordFormat;

/// An output format structure defines how log or trace records are formatted for a resource.
/// An output format consists of a list of record formats, since different formats can be used
/// depending on the record level and/or the occasion, why the record was triggered.
#[derive (Clone, Debug)]
pub(crate) struct OutputFormat {
    specific_formats: Vec<RecordFormat>,
    default_format: RecordFormat,
    levels: RecordLevelMap
}
impl OutputFormat {
    /// Creates an output format for a resource.
    ///
    /// # Arguments
    /// * `desc` - the output format descriptor from the configuration
    /// * `dtm_formats` - the map with all date time formats
    pub(crate) fn from_desc(desc: &OutputFormatDesc,
                            dtm_formats: &DateTimeFormatDescMap,
                            levels: &RecordLevelMap) -> OutputFormat {
        let mut specific_formats = Vec::<RecordFormat>::new();
        for sp_desc in desc.specific_formats() {
            specific_formats.push(RecordFormat::from_desc(sp_desc, dtm_formats));
        }
        let default_format = RecordFormat::from_desc(&RecordFormatDesc::default(), dtm_formats);
        OutputFormat { specific_formats, default_format, levels: levels.clone() }
    }

    /// Converts the specified log or trace record to a string according to this format.
    ///
    /// # Arguments
    /// * `record` - the record data
    /// * `levels` - the hash table with name and ID character for every record level
    ///
    /// # Return values
    /// the formatted string, to be written to output resource
    pub(crate) fn apply_to(&self, record: &dyn RecordData) -> String {
        let level = record.level();
        let trigger = record.trigger();
        for sf in self.specific_formats.iter() {
            if sf.applies_to(level, trigger) {
                return sf.apply_to(record, &self.levels);
            }
        }
        // we should never get here, but then apply default "all triggers/levels" format
        self.default_format.apply_to(record, &self.levels)
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
        // default format doesn't contain process or thread specific items
        self.specific_formats.iter_mut().for_each(|sf| sf.optimize_for(orig_info,
                                                                       thread_id, thread_name));
    }

    /// Returns a clone optimized for the specified originator thread.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    pub(crate) fn optimized_for(&self,
                                orig_info: &OriginatorInfo,
                                thread_id: u64,
                                thread_name: &str) -> OutputFormat {
        let mut opt_fmt = self.clone();
        opt_fmt.optimize_for(orig_info, thread_id, thread_name);
        opt_fmt
    }
}
