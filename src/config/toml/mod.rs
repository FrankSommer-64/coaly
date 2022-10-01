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

//! Coaly TOML processor.

use chrono::DateTime;
use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};
use chrono::offset::FixedOffset;
use std::fmt;
use std::fs::read_to_string;
use crate::errorhandling::*;
use crate::coalyxe;
use document::TomlDocument;
use parser::TomlParser;

pub mod document;
mod parser;
mod scanner;

/// Parses the specified TOML file.
/// The parsing process quits as soon as the first error is encountered.
/// 
/// # Arguments
/// * `file_name` - the name of the TOML file
/// 
/// # Return values
/// A TOML document structure with all TOML definitions parsed
/// 
/// # Errors
/// Returns a structure containing error information, if the file can't be read or parsed
pub fn parse_file(file_name: &str) -> Result<TomlDocument, CoalyException> {
    // read file contents into string
    let res = read_to_string(file_name);
    if let Err(error) = res {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Err(coalyxe!(E_FILE_NOT_FOUND, file_name.to_string()))
        }
        return Err(coalyxe!(E_FILE_READ_ERR, file_name.to_string(), format!("{}", error)))
    }
    // parse contents
    match TomlParser::new(&res.unwrap()).parse() {
        Ok(doc) => Ok(doc),
        Err(ex) => {
            let mut parse_ex = coalyxe!(E_CFG_TOML_PARSE_FAILED, file_name.to_string());
            parse_ex.set_cause(ex);
            Err(parse_ex)
        }
    }
}

/// Encloses a string in double quotes, if it doesn't start already with double quotes.
/// 
/// # Arguments
/// * `s` - the string
/// 
/// # Return values
/// The string enclosed within double quotes
fn quoted(s: &str) -> String {
    if s.starts_with('"') { return s.to_string() }
    let mut quoted_val = String::with_capacity(s.len() + 2);
    quoted_val.push('"');
    quoted_val.push_str(s);
    quoted_val.push('"');
    quoted_val
}

#[cfg(test)]
mod tests {
    use crate::errorhandling::COALY_MSG_TABLE;
    use crate::util::tests::run_unit_tests;
    use regex::Regex;
    use std::char;
    use std::collections::HashMap;
    use std::env;
    use std::fs::{File, read_to_string};
    use std::io::{BufRead, BufReader};
    use std::path::Path;
    use super::parse_file;
    use super::scanner::{TokenId, TokenValueType};

    /// Fields in the test specification file.
    /// First character is taken as field separator, hence the first field is always empty as a
    /// result of the string split with the separator character.
    pub(super) const SF_NAME: &str = "name";
    pub(super) const SF_DATA: &str = "data";
    pub(super) const SF_SUCCESS: &str = "success";
    pub(super) const SF_EXP_KEY: &str = "expectkey";
    pub(super) const SF_TOKENID: &str = "tokenid";
    pub(super) const SF_VALTYPE: &str = "type";
    pub(super) const SF_STRING_VALUE: &str = "stringvalue";
    pub(super) const SF_SPECIFIC_VALUE: &str = "specificvalue";
    pub(super) const SF_EXID: &str = "exceptionid";
    pub(super) const SPEC_FIELDS: &[& str] = &[ "", SF_NAME, SF_DATA, SF_SUCCESS, SF_EXP_KEY,
                                         SF_TOKENID, SF_VALTYPE, SF_STRING_VALUE,
                                         SF_SPECIFIC_VALUE, SF_EXID ];

    /// Field indices in the test specification file.
    pub(super) const SPEC_FIELD_COUNT: usize = 10;
    pub(super) const SFI_NAME: usize = 1;
    pub(super) const SFI_DATA: usize = 2;
    pub(super) const SFI_SUCCESS: usize = 3;
    pub(super) const SFI_EXP_KEY: usize = 4;
    pub(super) const SFI_TOKENID: usize = 5;
    pub(super) const SFI_VALTYPE: usize = 6;
    pub(super) const SFI_STRING_VALUE: usize = 7;
    pub(super) const SFI_SPECIFIC_VALUE: usize = 8;
    pub(super) const SFI_EXID: usize = 9;


    /// Test descriptor, one line from the test specification file
    pub(super) type TestDescriptor = HashMap<String, String>;

    enum UnescapeStatus {
        PlainText,
        InEscape,
        InUnicodeEscape
    }

    pub(super) fn token_id_from_str(s: &str) -> Option<TokenId> {
        match s {
            "Equal" => Some(TokenId::Equal),
            "Comma" => Some(TokenId::Comma),
            "Dot" => Some(TokenId::Dot),
            "LeftBrace" => Some(TokenId::LeftBrace),
            "RightBrace" => Some(TokenId::RightBrace),
            "LeftBracket" => Some(TokenId::LeftBracket),
            "RightBracket" => Some(TokenId::RightBracket),
            "DoubleLeftBracket" => Some(TokenId::DoubleLeftBracket),
            "DoubleRightBracket" => Some(TokenId::DoubleRightBracket),
            "Key" => Some(TokenId::Key),
            "Value" => Some(TokenId::Value),
            "LineBreak" => Some(TokenId::LineBreak),
            "EndOfInput" => Some(TokenId::EndOfInput),
            _ => None
        }
    }

    pub(super) fn token_valtype_from_str(s: &str) -> Option<TokenValueType> {
        match s {
            "String" => Some(TokenValueType::String),
            "Boolean" => Some(TokenValueType::Boolean),
            "Integer" => Some(TokenValueType::Integer),
            "Float" => Some(TokenValueType::Float),
            "OffsetDateTime" => Some(TokenValueType::OffsetDateTime),
            "LocalDateTime" => Some(TokenValueType::LocalDateTime),
            "LocalDate" => Some(TokenValueType::LocalDate),
            "LocalTime" => Some(TokenValueType::LocalTime),
            _ => None
        }
    }

    pub(super) fn read_test_spec(file: &Path) -> Result<Vec<TestDescriptor>, String> {
        match File::open(file) {
            Ok(f) => {
                let perm_op_pat = Regex::new("\\$\\{([a-z0-9]+)\\}").unwrap();
                let mut test_cases = Vec::<TestDescriptor>::new();
                let mut permuted_data = Vec::<String>::new();
                let mut permuted_str_vals = Vec::<String>::new();
                let mut permuted_spec_vals = Vec::<String>::new();
                let reader = BufReader::new(f);
                for (line_nr, l) in reader.lines().enumerate() {
                    if  line_nr == 0 {
                        // first line is ignored (header with fields description)
                        continue;
                    }
                    match l {
                        Ok(line) => {
                            permuted_data.clear();
                            permuted_str_vals.clear();
                            permuted_spec_vals.clear();
                            let sep_char: char;
                            let first_char = line.chars().next();
                            match first_char {
                                Some(ch) => sep_char = ch,
                                _ => continue
                            }
                            let fields: Vec::<&str> = line.split(sep_char).collect();
                            if fields.len() != SPEC_FIELD_COUNT {
                                return Err(test_spec_error(line_nr,
                                                          "Invalid field count in test desc"))
                            }
                            let mut tc = TestDescriptor::new();
                            for i in SFI_NAME .. SPEC_FIELD_COUNT {
                                let val_str = fields[i];
                                let val_defined = ! val_str.is_empty();
                                if i == SFI_EXP_KEY {
                                    match fields[i].to_lowercase().as_str() {
                                        "true" | "false" => (),
                                        _ => {
                                        return Err(test_spec_error(line_nr,
                                                            "expected key must be true or false"))
                                        }
                                    }
                                }
                                if i == SFI_TOKENID && val_defined
                                                    && token_id_from_str(val_str).is_none() {
                                    return Err(test_spec_error(line_nr,
                                                               "Invalid token id specified"))
                                }
                                if i == SFI_VALTYPE && val_defined
                                                    && token_valtype_from_str(val_str).is_none() {
                                    return Err(test_spec_error(line_nr,
                                                               "Invalid token value type"))
                                }
                                if i == SFI_DATA {
                                    match unescape(val_str) {
                                        Ok(unescaped_val) => {
                                            permuted_data.extend(permuted_vals(&unescaped_val,
                                                                               &perm_op_pat));
                                        },
                                        Err(msg) => return Err(msg)
                                    }
                                    continue;
                                }
                                if i == SFI_STRING_VALUE {
                                    match unescape(val_str) {
                                        Ok(unescaped_val) => {
                                            permuted_str_vals.extend(permuted_vals(&unescaped_val,
                                                                                   &perm_op_pat));
                                        },
                                        Err(msg) => return Err(msg)
                                    }
                                    continue;
                                }
                                if i == SFI_SPECIFIC_VALUE {
                                    permuted_spec_vals.extend(permuted_vals(val_str,
                                                                            &perm_op_pat));
                                    continue;
                                }
                                tc.insert(SPEC_FIELDS[i].to_string(), val_str.to_string());
                            }
                            let ptcs = permuted_tcs(tc, &permuted_data,
                                                    &permuted_str_vals, &permuted_spec_vals);
                            test_cases.extend(ptcs);
                        },
                        _ => return Err(test_spec_error(line_nr, "Invalid test descriptor"))
                    }
                }
                Ok(test_cases)
            },
            Err(io_err) => {
                let emsg = format!("{}", io_err);
                Err(emsg)
            }
        }
    }

    #[inline]
    fn test_spec_error(line_nr: usize, msg: &str) -> String {
        format!("Line {0}: {1}", line_nr+1, msg)
    }

    fn permuted_tcs(tc: TestDescriptor, d: &[String],
                    str_v: &[String], sp_v: &[String]) -> Vec<TestDescriptor> {
        let mut ptcs = Vec::<TestDescriptor>::new();
        for i in 0 .. d.len() {
            let mut ptc = TestDescriptor::new();
            ptc.insert(SF_NAME.to_string(), tc[SF_NAME].clone());
            ptc.insert(SF_DATA.to_string(), d[i].clone());
            ptc.insert(SF_SUCCESS.to_string(), tc[SF_SUCCESS].clone());
            ptc.insert(SF_EXP_KEY.to_string(), tc[SF_EXP_KEY].clone());
            ptc.insert(SF_TOKENID.to_string(), tc[SF_TOKENID].clone());
            ptc.insert(SF_VALTYPE.to_string(), tc[SF_VALTYPE].clone());
            if i < str_v.len() {
                ptc.insert(SF_STRING_VALUE.to_string(), str_v[i].clone());
            } else {
                ptc.insert(SF_STRING_VALUE.to_string(), str_v[0].clone());
            }
            if i < sp_v.len() {
                ptc.insert(SF_SPECIFIC_VALUE.to_string(), sp_v[i].clone());
            } else {
                ptc.insert(SF_SPECIFIC_VALUE.to_string(), sp_v[0].clone());
            }
            ptc.insert(SF_EXID.to_string(), tc[SF_EXID].clone());
            ptcs.push(ptc);
        }
        ptcs
    }

    fn permuted_vals(val: &str, pattern: &Regex) -> Vec<String> {
        if val.is_empty() {
            return vec!(val.to_string())
        }
        let mut parray = vec![Vec::<String>::new(), Vec::<String>::new()];
        let mut from_index = 0;
        let mut to_index = 1;
        parray[0].push(val.to_string());
        while ! parray[from_index].is_empty() {
            parray[to_index].clear();
            for s in &parray[from_index].clone() {
                if let Some(perm_key) = pattern.find(s) {
                    match perm_key.as_str() {
                        "${pm}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), "+"));
                            parray[to_index].push(s.replace(perm_key.as_str(), "-"));
                        },
                        "${sign}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                            parray[to_index].push(s.replace(perm_key.as_str(), "+"));
                            parray[to_index].push(s.replace(perm_key.as_str(), "-"));
                        },
                        "${nsign}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                            parray[to_index].push(s.replace(perm_key.as_str(), "-"));
                        },
                        "${ws}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), " "));
                            parray[to_index].push(s.replace(perm_key.as_str(), "\t"));
                        },
                        "${wseol}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), " "));
                            parray[to_index].push(s.replace(perm_key.as_str(), "\t"));
                            parray[to_index].push(s.replace(perm_key.as_str(), "\n"));
                            parray[to_index].push(s.replace(perm_key.as_str(), "\r\n"));
                        },
                        "${tofs}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), "T"));
                            parray[to_index].push(s.replace(perm_key.as_str(), " "));
                        },
                        "${4z}" => {
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                            parray[to_index].push(s.replace(perm_key.as_str(), ""));
                        },
                        _ => { parray[to_index].push(s.replace(perm_key.as_str(), "")); }
                    }
                }
            }
            from_index = (from_index + 1) & 1;
            to_index = (to_index + 1) & 1;
        }
        if parray[to_index].is_empty() { parray[to_index].push(val.to_string()); }
        parray[to_index].clone()
    }

    fn unescape(s: &str) -> Result<String, String> {
        let mut res = String::with_capacity(s.len());
        let mut u_value: u32 = 0;
        let mut s_chars = s.chars();
        let mut status = UnescapeStatus::PlainText;
        loop {
            match s_chars.next() {
                None => {
                    return Ok(res)
                },
                Some(ch) => {
                    match status {
                        UnescapeStatus::InEscape => {
                            status = UnescapeStatus::PlainText;
                            match ch {
                                'u' => {
                                    u_value = 0;
                                    status = UnescapeStatus::InUnicodeEscape;
                                },
                                'b' => res.push('\u{0008}'),
                                't' => res.push('\t'),
                                'n' => res.push('\n'),
                                'f' => res.push('\u{000c}'),
                                'r' => res.push('\r'),
                                '0' => res.push('\0'),
                                '\\' | '"' => res.push(ch),
                                _ => return Err(String::from("Invalid escape sequence"))
                            }
                        },
                        UnescapeStatus::InUnicodeEscape => {
                            match ch {
                                '{' => (),
                                '}' => {
                                    status = UnescapeStatus::PlainText;
                                    if let Some(uchar) = char::from_u32(u_value) {
                                        res.push(uchar);
                                        continue;
                                    }
                                    return Err(String::from("Invalid unicode escape char"))
                                },
                                _ => {
                                    if ! ch.is_ascii_hexdigit() {
                                        return Err(String::from("Invalid unicode escape char"))
                                    }
                                    u_value <<= 4;
                                    match ch {
                                        '0' ..= '9' => {
                                            u_value += (ch as u32) - ('0' as u32);
                                        },
                                        'A' ..= 'F' => {
                                            u_value += (ch as u32) - ('A' as u32) + 10;
                                        },
                                        'a' ..= 'f' => {
                                            u_value += (ch as u32) - ('a' as u32) + 10;
                                        },
                                        _ => ()
                                    }
                                }
                            }
                        },
                        UnescapeStatus::PlainText => {
                            match ch {
                                '\\' => status = UnescapeStatus::InEscape,
                                _ => res.push(ch)
                            }
                        }
                    }
                }
            }
        }
    }

    /// Unit test function for TOML parser tests.
    fn run_parse_test(success_expected: bool,
                      proj_root_dir: &str,
                      input_fn: &str,
                      ref_fn: &str) -> Option<String> {
        let test_name = &input_fn[input_fn.rfind('/').unwrap()+1 ..];
        let test_name = &test_name[0 .. test_name.find('.').unwrap()];
        match read_to_string(ref_fn) {
            Ok(expected_result) => {
                let fallback_path = std::env::var("COALY_FALLBACK_PATH").unwrap();
                let ro_path = format!("{}/readonly", std::env::var("TESTING_ROOT").unwrap());
                let sys_tmp_dir = std::env::temp_dir().to_string_lossy().to_string();
                let expected_result = expected_result.replace("%readonlypath", &ro_path);
                let expected_result = expected_result.replace("%fallbackpath", &fallback_path);
                let expected_result = expected_result.replace("%inputfile", input_fn);
                let expected_result = expected_result.replace("%projroot", proj_root_dir);
                let expected_result = expected_result.replace("%systmp", &sys_tmp_dir);
                match parse_file(input_fn) {
                    Ok(toml_doc) => {
                        let actual_result = toml_doc.to_json();
                        if ! success_expected {
                            return Some(String::from("Expected failure, but test succeeded"))
                        }
                        assert_eq!(expected_result, actual_result, "{}", test_name);
                        None
                    },
                    Err(ex) => {
                        let ex_msg = ex.evaluate(&COALY_MSG_TABLE);
                        if success_expected {
                            return Some(format!("Expected success, but got exception {}", &ex_msg))
                        }
                        assert_eq!(expected_result, ex_msg, "{}", test_name);
                        None
                    }
                }
            },
            Err(_) => Some(format!("Could not read reference file {}", ref_fn))
        }
    }

    #[test]
    fn toml_parse_tests() {
        let test_lang = "en";
        let proj_root = env::var("COALY_PROJ_ROOT").unwrap();
        // Coaly tests
        if let Some(err_msg) = run_unit_tests(&proj_root, "toml_parser", true, ".toml", ".json",
                                              test_lang, run_parse_test) {
            panic!("Coaly success tests failed: {}", &err_msg)
        }
        // Coaly failure tests
        if let Some(err_msg) = run_unit_tests(&proj_root, "toml_parser", false, ".toml", ".txt",
                                              test_lang, run_parse_test) {
            panic!("Coaly failure tests failed: {}", &err_msg)
        }
        // toml-test-master tests
        if let Some(err_msg) = run_unit_tests(&proj_root, "toml_master", true, ".toml", ".json",
                                              test_lang, run_parse_test) {
            panic!("toml-test-master success tests failed: {}", &err_msg)
            
        }
        if let Some(err_msg) = run_unit_tests(&proj_root, "toml_master", false, ".toml", ".txt",
                                              test_lang, run_parse_test) {
            panic!("toml-test-master failure tests failed: {}", &err_msg)
        }
    }
}
