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

//! Format specifications are used to define the fields of a log or trace record resp. the parts
//! of an output file name.
//! A specification may contain placeholder variables, which are replaced with actual values
//! during runtime.
//! The specifications are usually read from the configuration file. If no such file is supplied
//! or the file can't be read, default specification are used instead.

use chrono::Local;
use regex::{Error, Regex};
use std::str::FromStr;
use crate::record::RecordLevelMap;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;
use crate::util::{DIR_SEP, regex_escaped_str};
use crate::variables::{Variable, VariableMap, VAR_NAME_ENV};
#[cfg(test)]
use chrono::DateTime;

/// Single item within a record or name format specification.
/// Items can either be constant strings or placeholder variables, which are replaced with their
/// actual values at runtime.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FormatItem {
    ConstantItem(String),
    VariableItem(Variable)
}

/// Descriptor for the fields of a log/trace record or the parts of an output filename.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FormatSpec(Vec<FormatItem>);
impl FormatSpec {
    /// Indicates whether this format specification is specific for a thread.
    /// 
    /// # Return values
    /// **true** if the format contains at least one of the variables ThreadId or ThreadName
    pub(crate) fn is_thread_specific(&self) -> bool {
        for item in &self.0 {
            if let FormatItem::VariableItem(v) = item {
                if matches!(v, Variable::ThreadId | Variable::ThreadName) { return true; }
            }
        }
        false
    }

    /// Indicates whether this format specification is specific for an originator.
    /// 
    /// # Return values
    /// **true** if the format contains at least one of the originator specific variables
    pub(crate) fn is_originator_specific(&self) -> bool {
        for item in &self.0 {
            if let FormatItem::VariableItem(v) = item {
                if matches!(v, Variable::ApplicationId | Variable::ApplicationName |
                               Variable::HostName | Variable::IpAddress |
                               Variable::ProcessId | Variable::ProcessName | Variable::Env(_)
                               ) { return true; }
            }
        }
        false
    }

    /// Indicates whether this format specification is indepenent from date and time.
    /// 
    /// # Return values
    /// **true** if the format does not contain at least one of the variables Date, Time
    /// or TimeStamp
    pub(crate) fn is_datetime_independent(&self) -> bool {
        for item in &self.0 {
            if let FormatItem::VariableItem(v) = item {
                if matches!(v, Variable::Date | Variable::Time |
                               Variable::TimeStamp) { return false; }
            }
        }
        true
    }

    /// Returns this format specification optimized for an originator thread.
    /// Variable items, whose values remain constant throughout the entire lifetime of the
    /// originator thread are replaced by constant items with the corresponding value.
    /// Adjacent constant items are combined.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    /// 
    /// # Return values
    /// * the optimized format specification
    pub(crate) fn optimized_for(&self,
                                orig_info: &OriginatorInfo,
                                thread_id: u64,
                                thread_name: &str) -> FormatSpec {
        let mut opt_fmt = Vec::<FormatItem>::new();
        let mut item_str = String::new();
        for source_item in &self.0 {
            match source_item {
                FormatItem::ConstantItem(item) => item_str.push_str(item),
                FormatItem::VariableItem(item) => {
                    match item {
                        Variable::ApplicationId => {
                            item_str.push_str(&orig_info.application_id());
                        },
                        Variable::ApplicationName => {
                            item_str.push_str(orig_info.application_name());
                        },
                        Variable::Env(v) => {
                            if let Some(value) = orig_info.env_var_value(v) {
                                item_str.push_str(value);
                            }
                        },
                        Variable::HostName => item_str.push_str(orig_info.host_name()),
                        Variable::IpAddress => {
                            item_str.push_str(orig_info.ip_address());
                        },
                        Variable::ProcessId => {
                            item_str.push_str(&orig_info.process_id());
                        },
                        Variable::ProcessName => {
                            item_str.push_str(orig_info.process_name());
                        },
                        Variable::ThreadId => item_str.push_str(&thread_id.to_string()),
                        Variable::ThreadName => item_str.push_str(thread_name),
                        _ => {
                            if ! item_str.is_empty() {
                                opt_fmt.push(FormatItem::ConstantItem(item_str.to_string()));
                                item_str.clear();
                            }
                            opt_fmt.push(source_item.clone());
                        }
                    }
                }
            }
        }
        if ! item_str.is_empty() { opt_fmt.push(FormatItem::ConstantItem(item_str)); }
        FormatSpec { 0: opt_fmt }
    }

    /// Returns this format specification optimized for a process.
    /// Variable items, whose values remain constant throughout the entire lifetime of the
    /// application are replaced by constant items with the corresponding value.
    /// Adjacent constant items are combined.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    /// 
    /// # Return values
    /// * the optimized format specification
    pub(crate) fn optimized_for_originator(&self, orig_info: &OriginatorInfo) -> FormatSpec {
        let mut opt_fmt = Vec::<FormatItem>::new();
        let mut item_str = String::new();
        for source_item in &self.0 {
            match source_item {
                FormatItem::ConstantItem(item) => item_str.push_str(item),
                FormatItem::VariableItem(item) => {
                    match item {
                        Variable::ApplicationId => {
                            item_str.push_str(&orig_info.application_id());
                        },
                        Variable::ApplicationName => {
                            item_str.push_str(orig_info.application_name());
                        },
                        Variable::Env(v) => {
                            if let Some(value) = orig_info.env_var_value(v) {
                                item_str.push_str(value);
                            }
                        },
                        Variable::HostName => item_str.push_str(orig_info.host_name()),
                        Variable::IpAddress => {
                            item_str.push_str(orig_info.ip_address());
                        },
                        Variable::ProcessId => {
                            item_str.push_str(&orig_info.process_id());
                        },
                        Variable::ProcessName => {
                            item_str.push_str(orig_info.process_name());
                        },
                        _ => {
                            if ! item_str.is_empty() {
                                opt_fmt.push(FormatItem::ConstantItem(item_str.to_string()));
                                item_str.clear();
                            }
                            opt_fmt.push(source_item.clone());
                        }
                    }
                }
            }
        }
        if ! item_str.is_empty() { opt_fmt.push(FormatItem::ConstantItem(item_str)); }
        FormatSpec { 0: opt_fmt }
    }

    /// Returns this format specification optimized for a thread.
    /// Variable items of type ThreadId or ThreadName are replace by constant items with the
    /// values given to this function.
    /// Adjacent constant items are combined.
    /// 
    /// # Arguments
    /// * `thread_id` - the thread's ID
    /// * `thread_name` - the thread's name
    /// 
    /// # Return values
    /// * the optimized format specification
    pub(crate) fn optimized_for_thread(&self, thread_id: u64, thread_name: &str) -> FormatSpec {
        let mut opt_fmt = Vec::<FormatItem>::new();
        let mut item_str = String::new();
        for source_item in &self.0 {
            match source_item {
                FormatItem::ConstantItem(item) => {
                    item_str.push_str(item);
                }
                FormatItem::VariableItem(item) => {
                    match item {
                        Variable::ThreadId => item_str.push_str(&thread_id.to_string()),
                        Variable::ThreadName => item_str.push_str(thread_name),
                        _ => {
                            if ! item_str.is_empty() {
                                opt_fmt.push(FormatItem::ConstantItem(item_str.to_string()));
                                item_str.clear();
                            }
                            opt_fmt.push(source_item.clone());
                        }
                    }
                }
            }
        }
        if ! item_str.is_empty() { opt_fmt.push(FormatItem::ConstantItem(item_str)); }
        FormatSpec { 0: opt_fmt }
    }

    /// Converts the specified log or trace record to a string according to this format.
    /// The caller must make sure, that the record is within the scope of this format by invoking
    /// function `applies_to`. The check is not done within this function.
    ///
    /// # Arguments
    /// * `record` - the record data
    /// * `levels` - the hash table with the ID character for every record level
    /// * `ts_fmt` - the optional format string for timestamp values
    /// * `date_fmt` - the optional format string for date values
    /// * `tm_fmt` - the optional format string for time values
    ///
    /// # Return values
    /// the formatted string, to be written to output resource
    pub(crate) fn apply_to_record(&self, record: &dyn RecordData, levels: &RecordLevelMap,
                           ts_fmt: &str, date_fmt: &str, tm_fmt: &str) -> String {
        let mut result = String::with_capacity(128);
        for field in self.0.iter() {
            match field {
                FormatItem::ConstantItem(c) => {
                    // constant fields can be copied unchanged to result string
                    result.push_str(c);
                }
                FormatItem::VariableItem(v) => {
                    // for variable fields determine the actual values 
                    match v {
                        Variable::Date => {
                            result.push_str(&record.timestamp().format(date_fmt).to_string());
                        },
                        Variable::Level => {
                            let ldesc = &*levels.get(&record.level()).unwrap();
                            result.push_str(&ldesc.name().to_string());
                        },
                        Variable::LevelId => {
                            let ldesc = &*levels.get(&record.level()).unwrap();
                            result.push(ldesc.id_char());
                        },
                        Variable::Message | Variable::ObserverValue => {
                            result.push_str(record.message().as_ref().unwrap());
                        },
                        Variable::PureSourceFileName => {
                            let pure_fn = record.source_fn().rsplit(DIR_SEP).next().unwrap_or("-");
                            result.push_str(pure_fn);
                        },
                        Variable::SourceFileName => {
                            result.push_str(record.source_fn());
                        },
                        Variable::SourceLineNr => {
                            let mut line_nr_str = String::from("-");
                            if let Some(line_nr) = record.line_nr() {
                                line_nr_str = line_nr.to_string();
                            }
                            result.push_str(&line_nr_str);
                        },
                        Variable::ObserverName => {
                            result.push_str(record.observer_name().as_ref().unwrap());
                        },
                        Variable::TimeStamp => {
                            result.push_str(&record.timestamp().format(ts_fmt).to_string());
                        },
                        Variable::Time => {
                            result.push_str(&record.timestamp().format(tm_fmt).to_string());
                        },
                        // other variables already covered by preceding optimization calls
                        _ => {}
                    }
                }
            }
        }
        result.push_str(EOL);
        result
    }

    /// Creates a filename string from this format.
    /// All placeholder variables not related to date or time must have been resolved prior to
    /// calling this function. 
    ///
    /// # Return values
    /// the filename string
    pub(crate) fn to_file_name(&self) -> String {
        let now = Local::now();
        let mut result = String::with_capacity(256);
        for field in self.0.iter() {
            match field {
                FormatItem::ConstantItem(c) => result.push_str(c),
                FormatItem::VariableItem(v) => {
                    match v {
                        Variable::Date => {
                            result.push_str(&now.format(FN_DATE_FORMAT).to_string());
                        },
                        Variable::TimeStamp => {
                            result.push_str(&now.format(FN_TIMESTAMP_FORMAT).to_string());
                        },
                        Variable::Time => {
                            result.push_str(&now.format(FN_TIME_FORMAT).to_string());
                        },
                        // other variables already covered by preceding optimization calls
                        _ => {}
                    }
                }
            }
        }
        result
    }

    /// Creates a regular expression to find and sort files from this specification.
    /// All placeholder variables not related to date or time must have been resolved prior to
    /// calling this function. 
    ///
    /// # Arguments
    /// * `compr_ext` - the file extension for compressed rollover files,
    ///                 including leading dot, empty string if compression is not used
    ///
    /// # Return values
    /// the regular expression to find and sort matching files
    ///
    /// # Errors
    /// Returns an error if the created regular expression pattern contains a syntax error
    pub(crate) fn file_name_pattern(&self, compr_ext: &str) -> Result<Regex, Error> {
        let mut pattern_str = String::with_capacity(256);
        pattern_str.push_str("^(");
        for field in self.0.iter() {
            match field {
                FormatItem::ConstantItem(c) => { pattern_str.push_str(&regex_escaped_str(c)); },
                FormatItem::VariableItem(v) => {
                    match v {
                        Variable::Date => { pattern_str.push_str(FN_DATE_PATTERN); },
                        Variable::TimeStamp => { pattern_str.push_str(FN_TIMESTAMP_PATTERN); },
                        Variable::Time => { pattern_str.push_str(FN_TIME_PATTERN); },
                        _ => { }
                    }
                }
            }
        }
        pattern_str.push_str(r")(\.\d+){0,1}");
        if ! compr_ext.is_empty() { pattern_str.push_str(&format!(r"(\{}){{0,1}}", compr_ext)); }
        pattern_str.push('$');
        Regex::new(&pattern_str)
    }

    /// Returns the items of this format specification.
    #[cfg(test)]
    pub(crate) fn items(&self) -> &Vec<FormatItem> { &self.0 }

    /// Creates file name from given local timestamp.
    #[cfg(test)]
    pub(crate) fn to_test_file_name(&self, dtm: &DateTime<Local>) -> String {
        let mut result = String::with_capacity(256);
        for field in self.0.iter() {
            match field {
                FormatItem::ConstantItem(c) => result.push_str(c),
                FormatItem::VariableItem(v) => {
                    match v {
                        Variable::Date => {
                            result.push_str(&dtm.format(FN_DATE_FORMAT).to_string());
                        },
                        Variable::TimeStamp => {
                            result.push_str(&dtm.format(FN_TIMESTAMP_FORMAT).to_string());
                        },
                        Variable::Time => {
                            result.push_str(&dtm.format(FN_TIME_FORMAT).to_string());
                        },
                        // other variables not used
                        _ => {}
                    }
                }
            }
        }
        result
    }
}
impl FromStr for FormatSpec {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const STATE_IDLE: u32 = 0;
        const STATE_IN_CONST: u32 = 1;
        const STATE_IN_VAR: u32 = 2;
        let var_map = VariableMap::default();
        let env_pattern = Regex::new(&format!(r"^{}\[(.*)\]", VAR_NAME_ENV)).unwrap();
        let mut items = Vec::new();
        let mut cur_item = String::with_capacity(64);
        let mut state = STATE_IDLE;
        let mut var_end_index: usize = 0;
        for (index, val) in s.char_indices() {
            match state {
                STATE_IDLE => {
                    if index < var_end_index {
                        continue;
                    }
                    var_end_index = 0;
                    if val == '$' {
                        state = STATE_IN_VAR;
                        continue;
                    }
                    state = STATE_IN_CONST;
                    cur_item.push(val);
                }
                STATE_IN_CONST => {
                    if val != '$' {
                        cur_item.push(val);
                        continue;
                    }
                    items.push(FormatItem::ConstantItem(cur_item.clone()));
                    cur_item.clear();
                    state = STATE_IN_VAR;
                }
                _ => {
                    if env_pattern.is_match(&s[index..]) {
                        let vname = env_pattern.captures(&s[index..]).unwrap()
                                               .get(1).unwrap().as_str();
                        items.push(FormatItem::VariableItem(Variable::Env(vname.to_string())));
                        // skip var (Env[] + length of env var name)
                        var_end_index = index + vname.len() + 5;
                        state = STATE_IDLE;
                        continue;
                    }
                    let mut cur_var_len = 0;
                    let mut cur_var_id: Option<Variable> = None;
                    for (vname, vid) in var_map.iter() {
                        if s[index..].starts_with(vname) {
                            let var_len = vname.len();
                            if var_len > cur_var_len {
                                cur_var_len = var_len;
                                cur_var_id = Some(vid.clone());
                            }
                        }
                    }
                    match cur_var_id {
                        Some(vid) => {
                            items.push(FormatItem::VariableItem(vid));
                            var_end_index = index + cur_var_len;
                            state = STATE_IDLE;
                        }
                        None => {
                            state = STATE_IN_CONST;
                            cur_item.push(val);
                            continue;
                        }
                    }
                }
            }
        }
        if ! cur_item.is_empty() { items.push(FormatItem::ConstantItem(cur_item)); }
        Ok(FormatSpec { 0: items })
    }
}

// Format for timestamps within file names
const FN_TIMESTAMP_FORMAT: &str = "%Y%m%d%H%M%S";

// Format for dates within file names
const FN_DATE_FORMAT: &str = "%Y%m%d";

// Format for times within file names
const FN_TIME_FORMAT: &str = "%H%M%S";

const FN_TIMESTAMP_PATTERN: &str = r"\d{14}";
const FN_DATE_PATTERN: &str = r"\d{8}";
const FN_TIME_PATTERN: &str = r"\d{6}";

#[cfg(windows)]
const EOL: &str = "\r\n";

#[cfg(not(windows))]
const EOL: &str = "\n";

#[cfg(test)]
mod tests {
    extern crate regex;
    use regex::Regex;
    use super::*;
    use std::mem;

    fn build_format_spec(items: &[&str]) -> FormatSpec {
        let mut spec = Vec::<FormatItem>::new();
        for item in items {
            if item.starts_with('$') {
                let pure_var_name = item.chars().skip(1).collect::<String>();
                let v = pure_var_name.parse::<Variable>().unwrap();
                spec.push(FormatItem::VariableItem(v.clone()));
            } else {
                spec.push(FormatItem::ConstantItem((*item).to_string()));
            }
        }
        FormatSpec { 0: spec }
    }

    fn verify_format_spec(fmt: &[FormatItem], expected_items: &[&str]) {
        assert_eq!(expected_items.len(), fmt.len());
        let vm = VariableMap::default();
        let env_pattern = Regex::new(r"^\$Env\[(.*)\]$").unwrap();
        for (i, fmt_item) in fmt.iter().enumerate() {
            let exp_item_str = expected_items[i];
            match &*fmt_item {
                FormatItem::ConstantItem(item_str) => {
                    assert_eq!(exp_item_str, item_str, "Check item #{}", i);
                }
                FormatItem::VariableItem(var_id) => {
                    match var_id {
                        Variable::Env(v) => {
                            assert!(env_pattern.is_match(exp_item_str));
                            let exp_vname = env_pattern.captures(exp_item_str).unwrap()
                                                       .get(1).unwrap().as_str();
                            assert_eq!(exp_vname, v);
                        },
                        _ => {
                            let expected_var_id = vm.get(&exp_item_str[1..]).unwrap().clone();
                            let expected_discr = mem::discriminant(&expected_var_id);
                            let actual_discr = mem::discriminant(var_id);
                            assert_eq!(expected_discr, actual_discr, "Check item #{}", i);
                        }
                    }
                }
            }
        }
    }

    fn check_format_spec_creation(fmt_str: &str, expected_items: &[&str]) {
        let spec = FormatSpec::from_str(fmt_str).unwrap();
        verify_format_spec(spec.items().as_slice(), expected_items);
    }

    fn check_thread_optimization(items: &[&str], expected_items: &[&str]) {
        let tid = 1234;
        let tname = "MyThread";
        let fmt = build_format_spec(items);
        let opt_spec = fmt.optimized_for_thread(tid, tname);
        verify_format_spec(opt_spec.items().as_slice(), expected_items);
    }

    fn check_process_optimization(items: &[&str], expected_items: &[&str]) {
        let mut oinfo = OriginatorInfo::new(1391, "coalyprocess", "coalyhost", "1.2.3.4");
        oinfo.set_application_id(9876);
        oinfo.set_application_name("coalyapp");
        oinfo.add_env_var("COALYTEST", "FromEnv");
        let fmt = build_format_spec(items);
        let opt_spec = fmt.optimized_for_originator(&oinfo);
        verify_format_spec(opt_spec.items().as_slice(), expected_items);
    }

    #[test]
    fn test_format_spec_creation() {
        // Format string including all variables
        const ALL_VARS_STR: &str = "$AppId|$AppName|$Date|$Env[COALYTEST]|$HostName|$IpAddress|\
                                    $Level|$LevelId|$Message|$ProcessId|$ProcessName|\
                                    $PureSourceFileName|$SourceFileName|$SourceLineNr|\
                                    $ObserverName|$ObserverValue|$ThreadId|$ThreadName|$Time|\
                                    $TimeStamp";
        let all_vars_items = ["$AppId", "|", "$AppName", "|", "$Date", "|", "$Env[COALYTEST]", "|",
                              "$HostName", "|", "$IpAddress", "|",
                              "$Level", "|", "$LevelId", "|", "$Message", "|", "$ProcessId","|",
                              "$ProcessName", "|", "$PureSourceFileName", "|",
                              "$SourceFileName", "|", "$SourceLineNr","|", "$ObserverName", "|",
                              "$ObserverValue", "|", "$ThreadId", "|","$ThreadName", "|",
                              "$Time", "|", "$TimeStamp"];
        check_format_spec_creation(ALL_VARS_STR, &all_vars_items);
        // Default format string
        const DEFAULT_STR: &str = "$TimeStamp|$LevelId|$SourceFileName:$SourceLineNr|$Message";
        let default_items = ["$TimeStamp", "|", "$LevelId", "|", "$SourceFileName", ":",
                             "$SourceLineNr", "|", "$Message"];
        check_format_spec_creation(DEFAULT_STR, &default_items);
    }

    #[test]
    fn test_optimize_for_process() {
        // empty spec
        check_process_optimization(&[], &[]);
        // Relevant variable at the beginning
        check_process_optimization(&["$AppId", "|", "$AppName", "|", "$Env[COALYTEST]", "|",
                                     "$Level", "|", "$HostName", "|", "$IpAddress", "|",
                                     "$TimeStamp", "|", "$ProcessId", "|", "$ProcessName","|",
                                     "$Message"],
                                   &["9876|coalyapp|FromEnv|", "$Level", "|coalyhost|1.2.3.4|",
                                     "$TimeStamp", "|1391|coalyprocess|", "$Message"]);
        // Relevant variables adjacent in the middle
        check_process_optimization(&["$Time", "|", "$ProcessId", "$ProcessName", "|", "$Message"],
                                   &["$Time", "|1391coalyprocess|", "$Message"]);
        // ThRelevant variable at the end
        check_process_optimization(&["$Time", "|", "$ProcessId", "|", "$ProcessName"],
                                   &["$Time", "|1391|coalyprocess"]);
        // Constant items only
        check_process_optimization(&["Field1", "|", "Field2", "|", "Field3"],
                                   &["Field1|Field2|Field3"]);
        // Other variables only
        check_process_optimization(&["$Time", "$LevelId", "$SourceFileName", "$Message"],
                                   &["$Time", "$LevelId", "$SourceFileName", "$Message"]);
    }

    #[test]
    fn test_optimize_for_thread() {
        // empty spec
        check_thread_optimization(&[], &[]);
        // Thread-ID and -Name at the beginning
        check_thread_optimization(&["$ThreadId", "|", "$ThreadName", "|", "$Message"],
                                  &["1234|MyThread|", "$Message"]);
        // Thread-ID and -Name in the middle and adjacent
        check_thread_optimization(&["$Time", "|", "$ThreadId", "$ThreadName", "|", "$Message"],
                                  &["$Time", "|1234MyThread|", "$Message"]);
        // Thread-ID and -Name at the end
        check_thread_optimization(&["$Time", "|", "$ThreadId", "|", "$ThreadName"],
                                  &["$Time", "|1234|MyThread"]);
        // Constant items only
        check_thread_optimization(&["Field1", "|", "Field2", "|", "Field3"],
                                  &["Field1|Field2|Field3"]);
        // Other variables only
        check_thread_optimization(&["$Time", "$LevelId", "$Env[COALYTEST]", "$Message"],
                                  &["$Time", "$LevelId", "$Env[COALYTEST]", "$Message"]);
    }
}
