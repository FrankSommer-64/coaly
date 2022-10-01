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

//! Descriptor structure for date-time types.

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use crate::collections::MapWithDefault;

/// Validates the specified date format string.
/// Returns the substring containing the erroneous portion for invalid format strings.
pub fn validate_date_format(fmt_str: &str) -> Result<(), String> {
    validate_format(fmt_str, DATE_FORMAT_VARS)
}

/// Validates the specified time format string.
/// Returns the substring containing the erroneous portion for invalid format strings.
pub fn validate_time_format(fmt_str: &str) -> Result<(), String> {
    validate_format(fmt_str, TIME_FORMAT_VARS)
}

/// Validates the specified timestamp format string.
/// Returns the substring containing the erroneous portion for invalid format strings.
pub fn validate_timestamp_format(fmt_str: &str) -> Result<(), String> {
    validate_format(fmt_str, TIMESTAMP_FORMAT_VARS)
}

/// Holds format strings for date, time and timestamp values.
/// Needed within record format and file name descriptors to use the desired form of these values.
#[derive (Clone, Default)]
pub(crate) struct DateTimeFormatDesc {
    // format name
    name: String,
    // format string for date values
    date_format: Option<String>,
    // format string for time values
    time_format: Option<String>,
    // format string for date-time values
    timestamp_format: Option<String>
}
impl DateTimeFormatDesc {
    /// Creates a date time format.
    /// Used for a format defined in the formats.datetime section of the custom configuration file.
    ///
    /// # Arguments
    /// * `name` - the format name
    /// * `date_format` - the optional format string for date values
    /// * `time_format` - the optional format string for time values
    /// * `timestamp_format` - the optional format string for timestamp (date and time) values
    #[inline]
    pub(crate) fn new(name: &str,
                      date_format: Option<String>,
                      time_format: Option<String>,
                      timestamp_format: Option<String>) -> DateTimeFormatDesc {
        DateTimeFormatDesc { name: name.to_string(), date_format, time_format, timestamp_format }
    }

    /// Returns the format string for date values used in output records.
    /// 
    /// # Return values
    /// the format string for date values used in output records, custom or default
    #[inline]
    pub(crate) fn date_format_for_recs(&self) -> &str {
        if let Some(dtf) = &self.date_format { return dtf }
        DEFAULT_REC_DATE_FORMAT
    }

    /// Returns the format string for time values used in output records.
    /// 
    /// # Return values
    /// the format string for time values used in output records, custom or default
    #[inline]
    pub(crate) fn time_format_for_recs(&self) -> &str {
        if let Some(tmf) = &self.time_format { return tmf }
        DEFAULT_REC_TIME_FORMAT
    }

    /// Returns the format string for timestamp values used in output records.
    /// 
    /// # Return values
    /// the format string for timestamp values used in output records, custom or default
    #[inline]
    pub(crate) fn timestamp_format_for_recs(&self) -> &str {
        if let Some(tsf) = &self.timestamp_format { return tsf }
        DEFAULT_REC_TIMESTAMP_FORMAT
    }
}
impl Debug for DateTimeFormatDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "N:{}/DT:{}/TM:{}/TS:{}",
               self.name,
               self.date_format.as_ref().unwrap_or(&String::from("-")),
               self.time_format.as_ref().unwrap_or(&String::from("-")),
               self.timestamp_format.as_ref().unwrap_or(&String::from("-"))
        )
    }
}

/// Map with date-time formats
pub(crate) type DateTimeFormatDescMap = MapWithDefault<DateTimeFormatDesc>;

/// Validates the specified format string.
/// Returns the substring containing the erroneous portion for invalid format strings.
fn validate_format(fmt_str: &str, var_str: &str) -> Result<(), String> {
    let var_map = var_str_to_map(var_str);
    let mut expect_var = false;
    let mut length_ind: u32 = 99;
    let mut var_buf = String::with_capacity(8);
    for ch in fmt_str.chars() {
        if expect_var {
            if ch == '%' {
                expect_var = false;
                continue
            }
            var_buf.push(ch);
            if ch.is_ascii_digit() {
                if length_ind < 10 { return Err(var_buf) }
                length_ind = ch.to_digit(10).unwrap();
                continue
            }
            if var_map.contains_key(&ch) {
                if length_ind < 99 {
                    let length_range = var_map.get(&ch).unwrap();
                    let min_length = length_range >> 4;
                    let max_length = length_range & 15;
                    if ! (min_length..=max_length).contains(&length_ind) { return Err(var_buf) }
                }
                expect_var = false;
                continue
            }
            return Err(var_buf)
        }
        if ch == '%' {
            var_buf.clear();
            var_buf.push(ch);
            length_ind = 99;
            expect_var = true;
        }
    }
    Ok(())
}

fn var_str_to_map(var_str: &str) -> HashMap<char, u32> {
    let mut map = HashMap::<char, u32>::with_capacity(var_str.len());
    let mut in_range_var = false;
    let mut range_val: u32 = 0;
    for ch in var_str.chars() {
        if in_range_var {
            if ch.is_ascii_digit() {
                range_val <<= 4;
                range_val |= (ch.to_digit(10).unwrap()) & 15;
                continue
            }
            map.insert(ch, range_val);
            in_range_var = false;
            continue
        }
        if ch == '\\' {
            in_range_var = true;
            range_val = 0;
            continue
        }
        map.insert(ch, 0);
    }
    map
}

// Default format for timestamps within records
const DEFAULT_REC_TIMESTAMP_FORMAT: &str = "%d.%m.%y %H:%M:%S%.3f";

// Default format for dates within records
const DEFAULT_REC_DATE_FORMAT: &str = "%d.%m.%y";

// Default format for times within records
const DEFAULT_REC_TIME_FORMAT: &str = "%H:%M:%S%.3f";

const DATE_FORMAT_VARS: &str = "dmyY";
const TIME_FORMAT_VARS: &str = "\\19fHIMpPS";
const TIMESTAMP_FORMAT_VARS: &str = "d\\19fHImMpPSyYzZ";
