// ------------------------------------------------------------------------------------------------
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
// ------------------------------------------------------------------------------------------------

//! Lexical analyzer for TOML formatted strings.

mod basicstates;
mod datetimestates;
mod numberstates;
mod stringstates;

extern crate num_traits;

use crate::errorhandling::*;
use super::*;
use basicstates::*;
use datetimestates::*;
use numberstates::*;
use stringstates::*;
use chrono::{DateTime, ParseError};
use chrono::naive::{NaiveDate, NaiveDateTime, NaiveTime};
use chrono::offset::{FixedOffset};
use num_traits::float::{FloatCore};
use std::cell::RefCell;
use std::collections::HashMap;
use std::num::{ParseFloatError, ParseIntError};
use std::rc::Rc;
use std::str::{FromStr, ParseBoolError};

const NULL: char = '\0';
const TAB: char = '\t';
const LINE_FEED: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const SPACE: char = ' ';

/// Lexical TOML tokens
#[derive (Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TokenId {
    Equal,
    Comma,
    Dot,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    DoubleLeftBracket,
    DoubleRightBracket,
    Key,
    Value,
    LineBreak,
    EndOfInput
}
impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenId::Equal => write!(f, "="),
            TokenId::Comma => write!(f, ","),
            TokenId::Dot => write!(f, "."),
            TokenId::LeftBrace => write!(f, "{{"),
            TokenId::RightBrace => write!(f, "}}"),
            TokenId::LeftBracket => write!(f, "["),
            TokenId::RightBracket => write!(f, "]"),
            TokenId::DoubleLeftBracket => write!(f, "[["),
            TokenId::DoubleRightBracket => write!(f, "]]"),
            TokenId::Key => write!(f, "<KEY>"),
            TokenId::Value => write!(f, "<VALUE>"),
            TokenId::LineBreak => write!(f, "<LINE_BREAK>"),
            TokenId::EndOfInput => write!(f, "<END-OF-INPUT>")
        }
    }
}

#[derive (Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TokenValueType {
    String,
    Boolean,
    Integer,
    Float,
    OffsetDateTime,
    LocalDateTime,
    LocalDate,
    LocalTime
}

/// TOML scanner.
/// Separates a TOML formatted string into a stream of tokens.
pub(super) struct TomlScanner {
    // TOML formatted character sequence
    data: Vec<char>,
    // end-of-data marker, length of input character sequence
    end_of_data_index: usize,
    // index of next character in input sequence, starting with 0
    current_index: usize,
    // index in input sequence, where current token begins
    token_index: usize,
    // value type of found token
    token_value_type: TokenValueType,
    // value of found token
    token_value: String,
    // stack of currently suspended states
    suspended_states: Vec<ScannerStateId>,
    // hash table containing all handler states
    states: ScannerStateMap
}
impl TomlScanner {
    /// Creates a scanner for the given TOML string.
    /// 
    /// # Arguments
    /// * `data` - the string containing the input data to scan
    pub(super) fn new(data: &str) -> TomlScanner {
        let vdata: Vec::<char> = data.chars().collect();
        let vdata_len = vdata.len();
        TomlScanner {
            data: vdata,
            end_of_data_index: vdata_len,
            current_index: 0,
            token_index: 0,
            token_value_type: TokenValueType::String,
            token_value: String::with_capacity(64),
            suspended_states: Vec::new(),
            states: TomlScanner::handler_states()
        }
    }

    /// Returns the current line and column number.
    /// Needed in case of errors.
    #[inline]
    pub(super) fn current_position(&self) -> (usize, usize) {
        self.position_from_index(self.current_index)
    }

    /// Returns the line and column number, where current token begins.
    /// Needed in case of errors.
    #[inline]
    pub(super) fn token_position(&self) -> (usize, usize) {
        self.position_from_index(self.token_index)
    }

    /// Returns the value type of the last scanned token.
    #[inline]
    pub(super) fn token_value_type(&self) -> TokenValueType {
        self.token_value_type
    }

    /// Returns the string value of the last scanned token.
    #[inline]
    pub(super) fn token_value(&self) -> &str {
        self.token_value.as_str()
    }

    /// Returns the boolean value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not a boolean value
    pub(super) fn bool_token_value(&self) -> Result<bool, ParseBoolError> {
        self.token_value.parse::<bool>()
    }

    /// Returns the integer value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not an integer value or the value
    /// is out of isize range
    pub(super) fn int_token_value(&self) -> Result<isize, ParseIntError> {
        if self.token_value.starts_with("0b") {
            return isize::from_str_radix(&self.token_value()[2..], 2)
        }
        if self.token_value.starts_with("0o") {
            return isize::from_str_radix(&self.token_value()[2..], 8)
        }
        if self.token_value.starts_with("0x") {
            return isize::from_str_radix(&self.token_value()[2..], 16)
        }
        isize::from_str(self.token_value())
    }

    /// Returns the float value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not a float value or the value
    /// is out of f64 range
    pub(super) fn f64_token_value(&self) -> Result<f64, ParseFloatError> {
        match self.token_value.as_str() {
            "inf" | "+inf" => Ok(f64::infinity()),
            "-inf" => Ok(f64::neg_infinity()),
            "nan" | "+nan" => Ok(f64::nan()),
            "-nan" => Ok(f64::from_bits(f64::nan().to_bits() | 0x8000000000000000)),
            _ => self.token_value.parse::<f64>()
        }
    }

    /// Returns the offset date-time value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not an offset date-time value or the
    /// value specified is not valid 
    pub(super) fn offset_datetime_token_value(&self) -> Result<DateTime<FixedOffset>, ParseError> {
        to_offset_datetime(&self.token_value)
    }

    /// Returns the local date-time value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not a local date-time value or the
    /// value specified is not valid 
    pub(super) fn local_datetime_token_value(&self) -> Result<NaiveDateTime, ParseError> {
        to_naive_datetime(&self.token_value)
    }

    /// Returns the local date value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not a local date value or the
    /// value specified is not valid 
    pub(super) fn local_date_token_value(&self) -> Result<NaiveDate, ParseError> {
        NaiveDate::parse_from_str(&self.token_value, "%Y-%m-%d")
    }

    /// Returns the local time value of the last scanned token.
    /// 
    /// # Errors
    /// Returns a ParseError if the last token scanned was not a local time value or the
    /// value specified is not valid 
    pub(super) fn local_time_token_value(&self) -> Result<NaiveTime, ParseError> {
        to_naive_time(&self.token_value)
    }

    /// Returns the next lexical unit of the TOML formatted data.
    /// 
    /// # Arguments
    /// * `data` - the string containing the input data to scan
    /// * `expect_key` - indicates whether a key is expected as next token or not
    /// 
    /// # Errors
    /// Returns a structure containing information if an error was encountered during the
    /// scan process
    pub(super) fn next_token(&mut self, expect_key: bool) -> Result<TokenId, CoalyException> {
        self.token_value.clear();
        self.suspended_states.clear();
        // we always start with IDLE state
        let mut current_state_id = ScannerStateId::Idle;
        let mut current_state = self.states.get(&current_state_id).unwrap();
        // scan loop
        while self.current_index <= self.end_of_data_index {
            if self.current_index > self.end_of_data_index { return Ok(TokenId::EndOfInput) }
            let ch = if self.current_index >= self.end_of_data_index {
                NULL } else { self.data[self.current_index] };
            self.current_index += 1;
            // contents is handled in the state structures
            match current_state.borrow_mut().process_char(ch, expect_key) {
                StateResult::TokenFound(correction, mark_beg, t_id, t_type, t_value) => {
                    // Token found, we're through
                    self.token_value_type = t_type;
                    if mark_beg {
                        self.token_index = self.current_index - 1;
                    }
                    self.current_index -= correction;
                    if let Some(tval) = t_value { self.token_value.push_str(&tval); }
                    return Ok(t_id)
                },
                StateResult::Finished(correction, mark_beg, consume_char, follow_state_id) => {
                    // Current state fulfilled its duty, transfer to next state
                    if mark_beg {
                        self.token_index = self.current_index - 1;
                    }
                    self.current_index -= correction;
                    if consume_char { self.token_value.push(ch); }
                    current_state_id = follow_state_id;
                    current_state = self.states.get(&current_state_id).unwrap();
                    current_state.borrow_mut().activate();
                },
                StateResult::Suspended(correction, follow_state_id) => {
                    // Current state needs help of another state
                    self.current_index -= correction;
                    self.suspended_states.push(current_state_id);
                    current_state_id = follow_state_id;
                    current_state = self.states.get(&current_state_id).unwrap();
                    current_state.borrow_mut().activate();
                },
                StateResult::CharError(correction, error_id, ch) => {
                    // Current state encountered an invalid character.
                    // Error messages always start with placeholders for file name, line number
                    // and column number.
                    // For invalid characters the current line and column number are relevant.
                    let (line_nr, col_nr) = self.position_from_index(self.current_index-1);
                    self.current_index -= correction;
                    let ch_str = if ch == '\'' { String::from("\"'\"") }
                                 else { quoted(format!("{:?}", ch).trim_matches('\'')) };
                    let x_params = vec!(line_nr.to_string(), col_nr.to_string(), ch_str);
                    return Err(CoalyException::with_args(error_id, Severity::Error, &x_params))
                },
                StateResult::Error(error_id, incl_token_val, params) => {
                    // Current state encountered an error
                    // Error messages always start with placeholders for file name, line number
                    // and column number.
                    // For general errors the line and column number where the current token
                    // starts is relevant.
                    let (line_nr, col_nr) = self.position_from_index(self.token_index);
                    let mut x_params = vec!(line_nr.to_string(), col_nr.to_string());
                    if incl_token_val { x_params.push(quoted(&self.token_value)); }
                    if let Some(p) = params { x_params.push(quoted(&p)); }
                    return Err(CoalyException::with_args(error_id, Severity::Error, &x_params))
                },
                StateResult::ResumeCallingState(correction, value_char, count) => {
                    // Current state fulfilled its duty, transfer to last suspended state
                    self.current_index -= correction;
                    for _n in 1 ..= count {
                        self.token_value.push(value_char);
                    }
                    current_state_id = self.suspended_states.pop().unwrap();
                    current_state = self.states.get(&current_state_id).unwrap();
                },
                StateResult::CharProcessed(consume_char) => {
                    // Current state remains active, just store current character in token_value
                    // attribute
                    if consume_char { self.token_value.push(ch); }
                }
            }
        }
        Ok(TokenId::EndOfInput)
    }

    /// Creates all handler states for a TOML scanner.
    /// 
    /// # Return values
    /// A hashmap with all handler states, indexed by their state ID
    fn handler_states() -> ScannerStateMap {
        let mut m = ScannerStateMap::new();
        m.insert(ScannerStateId::Idle, IdleState::new());
        m.insert(ScannerStateId::Comment, CommentState::new());
        m.insert(ScannerStateId::LineBreak, LineBreakState::new());
        m.insert(ScannerStateId::LBracket, BracketState::new('[', TokenId::LeftBracket,
                                                                    TokenId::DoubleLeftBracket));
        m.insert(ScannerStateId::RBracket, BracketState::new(']', TokenId::RightBracket,
                                                                    TokenId::DoubleRightBracket));
        m.insert(ScannerStateId::BareKey, BareKeyState::new());
        m.insert(ScannerStateId::DoubleQuotedKey, DoubleQuotedKeyState::new());
        m.insert(ScannerStateId::SingleQuotedKey, SingleQuotedKeyState::new());
        m.insert(ScannerStateId::StartOfBasicString,
                        StartOfStringState::new('"', ScannerStateId::BasicString,
                                                ScannerStateId::MultiLineBasicString));
        m.insert(ScannerStateId::StartOfLiteralString,
                        StartOfStringState::new('\'', ScannerStateId::LiteralString,
                                                ScannerStateId::MultiLineLiteralString));
        m.insert(ScannerStateId::BasicString, BasicStringState::new());
        m.insert(ScannerStateId::MultiLineBasicString, MultiLineBasicStringState::new());
        m.insert(ScannerStateId::LiteralString, LiteralStringState::new());
        m.insert(ScannerStateId::MultiLineLiteralString, MultiLineLiteralStringState::new());
        m.insert(ScannerStateId::Zero, ZeroState::new());
        m.insert(ScannerStateId::SignedZero, SignedZeroState::new());
        m.insert(ScannerStateId::BinInt, RadixIntState::new(validate_bin_digit));
        m.insert(ScannerStateId::OctInt, RadixIntState::new(validate_oct_digit));
        m.insert(ScannerStateId::HexInt, RadixIntState::new(validate_hex_digit));
        m.insert(ScannerStateId::FloatFraction, FloatFractionState::new());
        m.insert(ScannerStateId::FloatExponent, FloatExponentState::new());
        m.insert(ScannerStateId::Number, NumberState::new());
        m.insert(ScannerStateId::SignedNumber, SignedNumberState::new());
        m.insert(ScannerStateId::NumberOrDateTime, NumberOrDateTimeState::new());
        m.insert(ScannerStateId::SymbolicValue, SymbolicValueState::new());
        m.insert(ScannerStateId::DateOrDateTime, DateOrDateTimeState::new());
        m.insert(ScannerStateId::SpaceAfterDate, SpaceAfterDateState::new());
        m.insert(ScannerStateId::LocalTime, LocalTimeState::new());
        m.insert(ScannerStateId::OffsetTime, OffsetTimeState::new());
        m.insert(ScannerStateId::FractionalSeconds, FractionalSecondsState::new());
        m.insert(ScannerStateId::TimeZoneOffset, TimeZoneOffsetState::new());
        m.insert(ScannerStateId::SingleLineEscSequence, EscapeSequenceState::new(false));
        m.insert(ScannerStateId::MultiLineEscSequence, EscapeSequenceState::new(true));
        m.insert(ScannerStateId::ExtraneousWhitespace, ExtraneousWhitespaceState::new());
        m.insert(ScannerStateId::DoubleQuoteDelimSequence, DelimSequenceState::new('"'));
        m.insert(ScannerStateId::SingleQuoteDelimSequence, DelimSequenceState::new('\''));
        m.insert(ScannerStateId::InitialMultiLineCr, InitialMultiLineCrState::new());
        m
    }

    /// Returns the line and column number from the specified input data index.
    /// Needed in case of errors.
    fn position_from_index(&self, index: usize) -> (usize, usize) {
        let mut line_nr: usize = 1;
        let mut col_nr: usize = 1;
        for (i, ch) in self.data.iter().enumerate() {
            if i >= index { break; }
            col_nr += 1;
            if *ch == LINE_FEED {
                line_nr += 1;
                col_nr = 1;
            }
        }
        (line_nr, col_nr)
    }
}

/// ID's for all handler states
#[derive (Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[allow(dead_code)]
enum ScannerStateId {
    Idle,
    Comment,
    LineBreak,
    LBracket,
    RBracket,
    BareKey,
    DoubleQuotedKey,
    SingleQuotedKey,
    StartOfBasicString,
    StartOfLiteralString,
    BasicString,
    MultiLineBasicString,
    LiteralString,
    MultiLineLiteralString,
    Zero,
    SignedZero,
    BinInt,
    OctInt,
    HexInt,
    FloatFraction,
    FloatExponent,
    Number,
    SignedNumber,
    NumberOrDateTime,
    DateOrDateTime,
    SpaceAfterDate,
    LocalTime,
    OffsetTime,
    FractionalSeconds,
    TimeZoneOffset,
    SymbolicValue,
    SingleLineEscSequence,
    MultiLineEscSequence,
    ExtraneousWhitespace,
    DoubleQuoteDelimSequence,
    SingleQuoteDelimSequence,
    InitialMultiLineCr
}

/// Function signature for character validation
type ValidateChar = fn(char) -> bool;

/// Type alias for hash map of handler states
type ScannerStateMap = HashMap::<ScannerStateId, Rc<RefCell<dyn TokenAnalyzer>>>;

/// Enumeration for all possible results returned by handler states
#[derive (Clone, Debug)]
enum StateResult {
    /// The current state wants to remain active
    /// * argument indicates whether to store current character in token_value attribute or not
    CharProcessed(bool),
    /// The current state finished and wants the last suspended state to be activated again
    /// * first argument holds the number of characters that the input data index shall be moved
    ///   backward 
    /// * second and third argument hold a character and the count, how often it shall be
    ///   appended to the token_value attribute 
    ResumeCallingState(usize, char, u32),
    /// The current state finished and wants the specified state to be activated
    /// * first argument holds the number of characters that the input data index shall be moved
    ///   backward 
    /// * second argument indicates whether the current input data index shall be marked as
    ///   beginning of the scanned token_value 
    /// * third argument indicates whether to store current character in token_value attribute
    /// * last argument holds the ID of the state to activate
    Finished(usize, bool, bool, ScannerStateId),
    /// The current state wants to be suspended and the specified state to be activated
    /// * first argument holds the number of characters that the input data index shall be moved
    ///   backward 
    /// * last argument holds the ID of the state to activate
    Suspended(usize, ScannerStateId),
    /// The current state encountered an unexpected character.
    /// * first argument holds the number of characters that the input data index shall be moved
    ///   backward 
    /// * second argument holds the error ID
    /// * last argument holds the character
    CharError(usize, &'static str, char),
    /// The current state encountered an error
    /// * first argument holds the error ID
    /// * second argument indicates whether to include the current token value in the
    ///   exception parameters 
    /// * last argument holds an optional parameter for the error
    Error(&'static str, bool, Option<String>),
    /// The current state detected a token
    /// * first argument holds the number of characters that the input data index shall be moved
    ///   backward 
    /// * second argument indicates whether the current input data index shall be marked as
    ///   beginning of the scanned token_value 
    /// * third argument holds the ID of the detected token
    /// * fourth argument holds the value type of the detected token
    /// * last argument holds the optional string that shall be appended to the token_value
    ///   attribute
    TokenFound(usize, bool, TokenId, TokenValueType, Option<String>)
}

/// Functions to be supported by all handler states 
trait TokenAnalyzer {
    /// Handles the next character from TOML input data.
    /// #Arguments
    /// * `ch` - the character to process
    /// * `expect_key` - set to **true** if a key is expected; **false** for other tokens
    ///
    /// # Return values
    /// * the processing result
    fn process_char(&mut self, ch: char, expect_key: bool) -> StateResult;

    /// Invoked by the scanner when the state is activated.
    /// When a suspended state is resumed, it is **not** activated again.
    fn activate(&mut self) {}
}

/// Checks, whether the given character is a binary digit ('0' or '1').
fn validate_bin_digit(digit: char) -> bool {
    digit == '0' || digit == '1'
}

/// Checks, whether the given character is an octal digit ('0' - '7').
fn validate_oct_digit(digit: char) -> bool {
    ('0' ..= '7').contains(&digit)
}

/// Checks, whether the given character is an octal digit ('0'-'9', 'A'-'F', 'a'-'f').
fn validate_hex_digit(digit: char) -> bool {
    digit.is_ascii_hexdigit()
}

/// Converts the given string to a naive time value.
/// #Arguments
/// * `val` - the value to convert
///
/// # Return values
/// * the naive time value
/// 
/// # Errors
/// Returns a ParseError if the specified string does not represent a valid time
fn to_naive_time(val: &str) -> Result<NaiveTime, ParseError> {
    if val.contains('.') { return NaiveTime::parse_from_str(val, "%T%.f") }
    NaiveTime::parse_from_str(val, "%T")
}

/// Converts the given string to a naive date-time value.
/// #Arguments
/// * `val` - the value to convert
///
/// # Return values
/// * the naive date-time value
/// 
/// # Errors
/// Returns a ParseError if the specified string does not represent a valid date-time
fn to_naive_datetime(val: &str) -> Result<NaiveDateTime, ParseError> {
    let mut fmt_str = String::with_capacity(32);
    fmt_str.push_str("%F");
    fmt_str.push(val.chars().nth(10).unwrap());
    fmt_str.push_str("%T");
    if val.contains('.') { fmt_str.push_str("%.f"); }
    NaiveDateTime::parse_from_str(val, &fmt_str)
}

/// Converts the given string to an offset date-time value.
/// #Arguments
/// * `val` - the value to convert
///
/// # Return values
/// * the offset date-time value
/// 
/// # Errors
/// Returns a ParseError if the specified string does not represent a valid offset date-time
fn to_offset_datetime(val: &str) -> Result<DateTime<FixedOffset>, ParseError> {
    let val_chars: Vec<char> = val.chars().collect();
    let mut fmt_str: String = String::with_capacity(32);
    fmt_str.push_str("%F");
    fmt_str.push(val_chars[10]);
    fmt_str.push_str("%T");
    if val.contains('.') { fmt_str.push_str("%.f"); }
    fmt_str.push_str("%:z");
    if val.ends_with('Z') {
        let val_str = val.to_string().replace("Z", "+00:00");
        return DateTime::parse_from_str(&val_str, &fmt_str)
    }
    DateTime::parse_from_str(val, &fmt_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;
    use std::env;
    use std::fs;
    use std::path::{Path};

    const SCANNER_UT_DIR: &str = "ut/toml_scanner";
    const EXCEPTION_ID_PREFIX: &str = "E-Cfg-Toml-";

    /// Runs a single TOML scanner unit test case.
    /// A test case corresponds to a line in the unit test specification file.
    fn run_test_case(tc: &HashMap<String,String>) {
        let tc_name = tc.get(SPEC_FIELDS[SFI_NAME]).unwrap();
        let input_data = tc.get(SPEC_FIELDS[SFI_DATA]).unwrap();
        let success_expected = tc.get(SPEC_FIELDS[SFI_SUCCESS]).unwrap() == "OK";
        let key_expected =
            tc.get(SPEC_FIELDS[SFI_EXP_KEY]).unwrap().to_lowercase().as_str() == "true";
        let exp_token_id = tc.get(SPEC_FIELDS[SFI_TOKENID]).unwrap();
        let exp_value_type = tc.get(SPEC_FIELDS[SFI_VALTYPE]).unwrap();
        let exp_str_value = tc.get(SPEC_FIELDS[SFI_STRING_VALUE]).unwrap();
        let exp_spec_value = tc.get(SPEC_FIELDS[SFI_SPECIFIC_VALUE]).unwrap();
        let exception_id = tc.get(SPEC_FIELDS[SFI_EXID]).unwrap();
        let mut scanner = TomlScanner::new(input_data);
        match scanner.next_token(key_expected) {
            Ok(actual_tid) => {
                if exception_id.is_empty() {
                    assert!(success_expected,
                            "TC {} succeeded, but expected exception", tc_name);
                }
                let tid = token_id_from_str(exp_token_id);
                if tid.is_none() {
                    panic!("TC {} failed, could not map token ID >{}<", tc_name, exp_token_id);
                }
                let tid = tid.unwrap();
                assert_eq!(tid, actual_tid, "TC {}", tc_name);
                if let Some(val_t) = token_valtype_from_str(exp_value_type) {
                    assert_eq!(val_t, scanner.token_value_type(), "TC {}", tc_name);
                    assert_eq!(exp_str_value, scanner.token_value(), "TC {}", tc_name);
                    match val_t {
                        TokenValueType::Boolean => {
                            if exception_id.is_empty() {
                                let expected_value = exp_spec_value == "true";
                                match scanner.bool_token_value() {
                                    Ok(actual_value) => {
                                        assert_eq!(expected_value, actual_value, "TC {}", tc_name)
                                    },
                                    _ => panic!("Not a bool in TC {}", tc_name)
                                }
                            } else {
                                assert!(scanner.bool_token_value().is_err(),
                                        "Valid bool in TC {}", tc_name);
                            }
                        },
                        TokenValueType::Integer => {
                            if exception_id.is_empty() {
                                let exp_value = isize::from_str(exp_spec_value).unwrap();
                                match scanner.int_token_value() {
                                    Ok(actual_value) => {
                                        assert_eq!(exp_value, actual_value, "TC {}", tc_name)
                                    },
                                    _ => panic!("Not an integer in TC {}", tc_name)
                                }
                            } else {
                                assert!(scanner.int_token_value().is_err(),
                                        "Valid int in TC {}", tc_name);
                            }
                        },
                        TokenValueType::Float => {
                            let actual_value = scanner.f64_token_value();
                            if actual_value.is_err() {
                                assert!(!exception_id.is_empty(), "Valid float in TC {}", tc_name);
                                return
                            }
                            let actual_value = actual_value.unwrap();
                            match exp_spec_value.as_str() {
                                "inf" => {
                                    assert!(actual_value.is_sign_positive(), "TC {}", tc_name);
                                    assert!(actual_value.is_infinite(), "TC {}", tc_name);
                                },
                                "-inf" => {
                                    assert!(actual_value.is_sign_negative(), "TC {}", tc_name);
                                    assert!(actual_value.is_infinite(), "TC {}", tc_name);
                                },
                                "nan" => {
                                    assert!(actual_value.is_sign_positive(), "TC {}", tc_name);
                                    assert!(actual_value.is_nan(), "TC {}", tc_name);
                                },
                                "-nan" => {
                                    assert!(actual_value.is_sign_negative(), "TC {}", tc_name);
                                    assert!(actual_value.is_nan(), "TC {}", tc_name);
                                },
                                _ => {
                                    let exp_value = f64::from_str(exp_spec_value).unwrap();
                                    assert_eq!(exp_value, actual_value, "TC {}", tc_name);
                                }
                            }
                        },
                        TokenValueType::LocalDate => {
                            if exception_id.is_empty() {
                                let expected_value =
                                    NaiveDate::parse_from_str(exp_spec_value, "%Y-%m-%d").unwrap();
                                match scanner.local_date_token_value() {
                                    Ok(actual_value) => {
                                        assert_eq!(expected_value, actual_value, "TC {}", tc_name)
                                    },
                                    _ => panic!("Not a date in TC {}", tc_name)
                                }
                            } else {
                                assert!(scanner.local_date_token_value().is_err(),
                                        "Valid date in TC {}", tc_name);
                            }
                        },
                        TokenValueType::LocalTime => {
                            if exception_id.is_empty() {
                                let expected_value = to_naive_time(exp_spec_value).unwrap();
                                match scanner.local_time_token_value() {
                                    Ok(actual_value) => {
                                        assert_eq!(expected_value, actual_value, "TC {}", tc_name)
                                    },
                                    _ => panic!("Not a time in TC {}", tc_name)
                                }
                            } else {
                                assert!(scanner.local_time_token_value().is_err(),
                                        "Valid time in TC {}", tc_name);
                            }
                        },
                        TokenValueType::LocalDateTime => {
                            if exception_id.is_empty() {
                                let expected_value = to_naive_datetime(exp_spec_value).unwrap();
                                match scanner.local_datetime_token_value() {
                                    Ok(actual_value) => {
                                        assert_eq!(expected_value, actual_value, "TC {}", tc_name)
                                    },
                                    _ => panic!("Not a time in TC {}", tc_name)
                                }
                            } else {
                                assert!(scanner.local_datetime_token_value().is_err(),
                                        "Valid date time in TC {}", tc_name);
                            }
                        },
                        TokenValueType::OffsetDateTime => {
                            if exception_id.is_empty() {
                                let expected_value = to_offset_datetime(exp_spec_value).unwrap();
                                match scanner.offset_datetime_token_value() {
                                    Ok(actual_value) => {
                                        assert_eq!(expected_value, actual_value, "TC {}", tc_name)
                                    },
                                    _ => panic!("Not a time in TC {}", tc_name)
                                }
                            } else {
                                assert!(scanner.offset_datetime_token_value().is_err(),
                                        "Valid date time in TC {}", tc_name);
                            }
                        },
                        _ => ()
                    }
                }
            },
            Err(ex) => {
                assert!(!success_expected,
                        "TC {0} failed unexpectedly with ID {1}",
                        tc_name, ex.id());
                let expected_exception_id =
                    format!("{0}{1}", EXCEPTION_ID_PREFIX, exception_id);
                assert_eq!(expected_exception_id, ex.id(), "TC {}", tc_name);
            }
        }
    }

    /// Runs a single TOML scanner unit test from the specified file.
    fn run_scanner_test(file: &Path) {
        match read_test_spec(file) {
            Err(msg) => {
                let emsg = format!("Failed to read test spec file {0:?}: {1}",
                                   file.file_name().unwrap(), msg);
                panic!("{}", emsg)
            }
            Ok(desc_list) => {
                for desc in desc_list {
                    run_test_case(&desc);
                }
            }
        }
    }

    /// Runs all TOML scanner unit test.
    /// The tests are specified in files in directory $COALY_PROJ_ROOT/testdata/ut/toml_scanner.
    #[test]
    fn toml_scanner_tests() {
        let proj_root = env::var("COALY_PROJ_ROOT").unwrap();
        let utestdata_root = format!("{}/testdata/{}", proj_root, SCANNER_UT_DIR);
        for entry in fs::read_dir(utestdata_root).unwrap() {
            let item = entry.unwrap().path();
            if item.is_file() {
                run_scanner_test(&item);
            }
        }
    }
}