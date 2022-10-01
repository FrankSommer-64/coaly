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

//! TOML scanner states for string processing.

use super::*;
use std::char;
use std::rc::Rc;

/// Handler state for beginning of strings.
/// The state is activated after a single or double quote has been consumed.
pub(super) struct StartOfStringState {
    // number of currently consumed delimiter characters,
    // value is initialized with 1, since the first or only delimiter was already consumed by
    // the predecessor state
    delim_count: u32,
    // delimiter character (' or ")
    delim: char,
    // successor state, if a single delimiter is detected
    single_delim_state_id: ScannerStateId,
    // successor state, if a triple delimiter is detected
    triple_delim_state_id: ScannerStateId
}
impl StartOfStringState {
    /// Creates a handler state for the beginning of basic or literal strings.
    /// #Arguments
    /// * `delim` - the delimiter character (' or ")
    /// * `single_delim_state_id` - the successor state's ID, if a single delimiter is detected
    /// * `triple_delim_state_id` - the successor state's ID, if a triple delimiter is detected
    pub(super) fn new(delim: char,
                      single_delim_state_id: ScannerStateId,
                      triple_delim_state_id: ScannerStateId) -> Rc<RefCell<StartOfStringState>> {
        Rc::new(RefCell::new(StartOfStringState { delim_count: 1,
                                                  delim,
                                                  single_delim_state_id,
                                                  triple_delim_state_id }))
    }
}
impl TokenAnalyzer for StartOfStringState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == self.delim {
            // delimiter character found
            if self.delim_count >= 2 {
                // start of multi-line string
                return StateResult::Finished(0, false, false, self.triple_delim_state_id)
            }
            self.delim_count += 1;
            return StateResult::CharProcessed(false)
        }
        // other char than delimiter
        if self.delim_count == 2 {
            // empty single line string
            return StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::String, None)
        }
        if ch == CARRIAGE_RETURN || ch == LINE_FEED || ch == NULL {
            // single line strings cannot span multiple lines
            StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None)
        } else {
            // start of single line string
            StateResult::Finished(1, false, false, self.single_delim_state_id)
        }
    }
    fn activate(&mut self) {
        self.delim_count = 1;
    }
}

/// Handler state for single-line basic strings.
/// Basic strings are surrounded by quotation marks (").
/// Any Unicode character may be used except those that must be escaped:
///   quotation mark, backslash, and the control characters other than tab
///   (U+0000 to U+0008, U+000A to U+001F, U+007F).
pub(super) struct BasicStringState {
}
impl BasicStringState {
    /// Creates a handler state for single-line basic strings.
    pub(super) fn new() -> Rc<RefCell<BasicStringState>> {
        Rc::new(RefCell::new(BasicStringState {}))
    }
}
impl TokenAnalyzer for BasicStringState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '"' => StateResult::TokenFound(0, false, TokenId::Value, TokenValueType::String, None),
            '\\' => StateResult::Suspended(0, ScannerStateId::SingleLineEscSequence),
            NULL | LINE_FEED | CARRIAGE_RETURN => {
                StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None)
            },
            '\u{0001}'..='\u{0008}' | '\u{000b}'| '\u{000d}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            _ => StateResult::CharProcessed(true)
        }
   }
}

/// Handler state for multi-line basic strings.
/// Multi-line basic strings are surrounded by three quotation marks on each side
/// and allow newlines. A newline immediately following the opening delimiter will be trimmed.
/// All other whitespace and newline characters remain intact.
/// Any Unicode character may be used except those that must be escaped:
/// backslash and the control characters other than tab, line feed, and carriage return
/// (U+0000 to U+0008, U+000B, U+000C, U+000E to U+001F, U+007F).
/// You can write a quotation mark, or two adjacent quotation marks, anywhere inside a
/// multi-line basic string. They can also be written just inside the delimiters.
pub(super) struct MultiLineBasicStringState {
    // indicates, whether the next character to process is the first in the string.
    // needed to recognize an initial line break, which needs to be ignored
    first_char: bool
}
impl MultiLineBasicStringState {
    /// Creates a handler state for multi-line basic strings.
    pub(super) fn new() -> Rc<RefCell<MultiLineBasicStringState>> {
        Rc::new(RefCell::new(MultiLineBasicStringState { first_char: true }))
    }
}
impl TokenAnalyzer for MultiLineBasicStringState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            LINE_FEED => {
                if self.first_char {
                    self.first_char = false;
                    return StateResult::CharProcessed(false)
                }
                StateResult::CharProcessed(true)
            },
            CARRIAGE_RETURN => {
                if self.first_char {
                    self.first_char = false;
                    return StateResult::Suspended(0, ScannerStateId::InitialMultiLineCr)
                }
                StateResult::CharProcessed(true)
            },
            '"' => {
                self.first_char = false;
                StateResult::Suspended(0, ScannerStateId::DoubleQuoteDelimSequence)
            },
            '\\' => {
                self.first_char = false;
                StateResult::Suspended(0, ScannerStateId::MultiLineEscSequence)
            },
            NULL => StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None),
            '\u{0001}'..='\u{0008}' | '\u{000b}' | '\u{000c}'
                                    | '\u{000e}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            _ => {
                self.first_char = false;
                StateResult::CharProcessed(true)
            }
        }
    }
    fn activate(&mut self) {
        self.first_char = true;
    }
}

/// Handler state for single-line literal strings.
/// Literal strings are surrounded by single quotes.
/// Like basic strings, they must appear on a single line.
/// There is no escaping.
/// Control characters other than tab are not permitted in a literal string
pub(super) struct LiteralStringState {
}
impl LiteralStringState {
    /// Creates a handler state for single-line literal strings.
    pub(super) fn new() -> Rc<RefCell<LiteralStringState>> {
        Rc::new(RefCell::new(LiteralStringState {}))
    }
}
impl TokenAnalyzer for LiteralStringState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '\'' => StateResult::TokenFound(0, false, TokenId::Value,
                                            TokenValueType::String, None),
            NULL => StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None),
            '\u{0001}'..='\u{0008}' | '\u{000a}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            _ => StateResult::CharProcessed(true)
        }
   }
}

/// Handler state for multi-line literal strings.
/// Multi-line literal strings are surrounded by three single quotes on each side
/// and allow newlines. Like literal strings, there is no escaping whatsoever.
/// A newline immediately following the opening delimiter will be trimmed.
/// All other content between the delimiters is interpreted as-is without modification.
pub(super) struct MultiLineLiteralStringState {
    // indicates, whether the next character to process is the first in the string.
    // needed to recognize an initial line break, which needs to be ignored
    first_char: bool
}
impl MultiLineLiteralStringState {
    /// Creates a handler state for multi-line literal strings.
    pub(super) fn new() -> Rc<RefCell<MultiLineLiteralStringState>> {
        Rc::new(RefCell::new(MultiLineLiteralStringState { first_char: true }))
    }
}
impl TokenAnalyzer for MultiLineLiteralStringState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            LINE_FEED => {
                if self.first_char {
                    self.first_char = false;
                    return StateResult::CharProcessed(false)
                }
                StateResult::CharProcessed(true)
            },
            CARRIAGE_RETURN => {
                if self.first_char {
                    self.first_char = false;
                    return StateResult::Suspended(0, ScannerStateId::InitialMultiLineCr)
                }
                StateResult::CharProcessed(true)
            },
            '\'' => StateResult::Suspended(0, ScannerStateId::SingleQuoteDelimSequence),
            NULL => StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None),
            '\u{0001}'..='\u{0008}' | '\u{000b}' | '\u{000c}'
                                    | '\u{000e}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            _ => {
                self.first_char = false;
                StateResult::CharProcessed(true)
            }
        }
   }
    fn activate(&mut self) {
        self.first_char = true;
    }
}

/// Handler state for escape sequences within basic strings.
/// Valid escape sequences are:
/// \b         - backspace       (U+0008)
/// \t         - tab             (U+0009)
/// \n         - linefeed        (U+000A)
/// \f         - form feed       (U+000C)
/// \r         - carriage return (U+000D)
/// \"         - quote           (U+0022)
/// \\         - backslash       (U+005C)
/// \uXXXX     - unicode         (U+XXXX)
/// \UXXXXXXXX - unicode         (U+XXXXXXXX)
/// Multi-line basic strings may use a "line ending backslash":
/// When the last non-whitespace character on a line is an unescaped \,
/// it will be trimmed along with all whitespace (including newlines)
/// up to the next non-whitespace character or closing delimiter.
pub(super) struct EscapeSequenceState {
    // indicates whether the state handles escape sequences within multi-line basic strings
    within_multi_line_string: bool,
    // indicates that a backslash followed by a whitespace other than line feed  has been detected
    handling_line_ending: bool,
    // number of characters expected after the initial backslash
    expected_char_count: u32,
    // number of characters processed after the initial backslash
    chars_processed: u32,
    // buffer for escaped unicode value
    unicode_buffer: String
}
impl EscapeSequenceState {
    /// Creates a handler state for escape sequences within basic strings.
    ///
    /// # Arguments
    /// * `within_multi_line_string` - **true** if the state shall handle mult-line strings,
    ///                                specify **false** for single-line strings
    pub(super) fn new(within_multi_line_string: bool) -> Rc<RefCell<EscapeSequenceState>> {
        Rc::new(RefCell::new(EscapeSequenceState {
                    within_multi_line_string,
                    handling_line_ending: false,
                    expected_char_count: 1,
                    chars_processed: 0,
                    unicode_buffer: String::with_capacity(16)
        }))
    }
}
impl TokenAnalyzer for EscapeSequenceState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if self.handling_line_ending {
            // we've found a backslash followed by whitespace other than line feed in a
            // multi-line string
            match ch {
                LINE_FEED => {
                    // valid line terminating backslash, state transition to ignore all
                    // following whitespace characters
                    return StateResult::Finished(0, false, false,
                                                 ScannerStateId::ExtraneousWhitespace)
                },
                TAB | CARRIAGE_RETURN | SPACE => {
                    // whitespace characters up to line feed are allowed and ignored
                    return StateResult::CharProcessed(false)
                },
                _ => {
                    // non whitespace is not allowed
                    return StateResult::CharError(0, E_CFG_TOML_INV_EOL_ESC, ch)
                }
            }
        }
        self.chars_processed += 1;
        if self.chars_processed > self.expected_char_count {
            return StateResult::ResumeCallingState(1, NULL, 0)
        }
        if self.chars_processed == 1 {
            // first character after initial backslash
            match ch {
                '"' | '\\' => return StateResult::CharProcessed(true),
                'b' => return StateResult::ResumeCallingState(0, '\u{0008}', 1),
                't' => return StateResult::ResumeCallingState(0, '\t', 1),
                'n' => return StateResult::ResumeCallingState(0, '\n', 1),
                'f' => return StateResult::ResumeCallingState(0, '\u{000c}', 1),
                'r' => return StateResult::ResumeCallingState(0, '\r', 1),
                'u' => {
                    // 4 digit unicode
                    self.expected_char_count = 5;
                    return StateResult::CharProcessed(false)
                 },
                'U' => {
                    // 8 digit unicode
                    self.expected_char_count = 9;
                    return StateResult::CharProcessed(false)
                 },
                LINE_FEED => {
                    if self.within_multi_line_string {
                        // valid line terminating backslash, state transition to ignore all
                        // following whitespace characters
                        return StateResult::Finished(0, false, false,
                                                     ScannerStateId::ExtraneousWhitespace)
                    }
                    // line terminating backslash is not allowed within single-line strings
                    return StateResult::CharError(1, E_CFG_TOML_SGL_LINE_TERM, '\\')
                },
                TAB | CARRIAGE_RETURN | SPACE => {
                    if self.within_multi_line_string {
                        // assume start of line terminating backslash within multi-line string
                        self.handling_line_ending = true;
                        return StateResult::CharProcessed(false)
                    }
                    // line terminating backslash is not allowed within single-line strings
                    return StateResult::CharError(1, E_CFG_TOML_SGL_LINE_TERM, '\\')
                },
                NULL => return StateResult::ResumeCallingState(1, NULL, 0),
                // all other characters are not allowed after the initial backslash
                _ => return StateResult::CharError(0, E_CFG_TOML_INV_ESC_CHAR, ch)
            }
        }
        // when we get here we're processing a unicode escape sequence
        if (ch == LINE_FEED || ch == CARRIAGE_RETURN || ch == NULL || ch == '"' || ch == '\'') &&
           (self.chars_processed < self.expected_char_count) {
            return StateResult::Error(E_CFG_TOML_INV_UNICODE_ESC_SEQ, false,
                                      Some(self.unicode_buffer.to_string()))
        }
        if ! ch.is_ascii_hexdigit() {
            // unicode character may be specified by hex digits only
            return StateResult::CharError(1, E_CFG_TOML_INV_UNICODE_ESC_CHAR, ch)
        }
        self.unicode_buffer.push(ch);
        if self.chars_processed == self.expected_char_count {
            // unicode escape character completely processed
            let u_value = u32::from_str_radix(&self.unicode_buffer, 16).unwrap();
            if let Some(u_char) = char::from_u32(u_value) {
                // unicode escape character is valid, return it. State transition is done with next
                // call to this function, then we have `actual_ch_count > needed_ch_count` causing
                // to leave the state further above
                return StateResult::ResumeCallingState(0, u_char, 1)
            }
            // specified unicode character is not allowed
            return StateResult::Error(E_CFG_TOML_INV_UNICODE_ESC_SEQ, false,
                                      Some(self.unicode_buffer.to_string()))
        }
        // next unicode escape character processed successfully
        StateResult::CharProcessed(false)
    }
    fn activate(&mut self) {
        self.handling_line_ending = false;
        self.expected_char_count = 1;
        self.chars_processed = 0;
        self.unicode_buffer.clear();
    }
}

/// Handler state for extraneous whitespace after a "line ending backslash" within a multi-line
/// basic string.
/// All characters up to a non-whitespace character are ignored.
pub(super) struct ExtraneousWhitespaceState {
}
impl ExtraneousWhitespaceState {
    /// Creates a handler state for extraneous whitespace
    pub(super) fn new() -> Rc<RefCell<ExtraneousWhitespaceState>> {
        Rc::new(RefCell::new(ExtraneousWhitespaceState {}))
    }
}
impl TokenAnalyzer for ExtraneousWhitespaceState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        // ignore whitespace, let calling state handle all other characters
        match ch {
            TAB | LINE_FEED | CARRIAGE_RETURN | SPACE => StateResult::CharProcessed(false),
            _ => StateResult::ResumeCallingState(1, NULL, 0)
        }
    }
}

/// Handler state, when the delimiter character was found within a multi-line string.
/// This structure works both for basic and literal strings, since the concrete delimiter character
/// must be specified on instantiation.
pub(super) struct DelimSequenceState {
    // delimiter character (" for basic, ' for literal strings)
    delim: char,
    // number of currently consumed delimiter characters,
    // value is initialized with 1, since the first or only delimiter was already consumed by
    // the calling state
    delim_count: u32
}
impl DelimSequenceState {
    /// Creates a handler state for delimiter within a multi-line string
    ///
    /// # Arguments
    /// * `delim` - the delimiter character (" for basic, ' for literal strings)
    pub(super) fn new(delim: char) -> Rc<RefCell<DelimSequenceState>> {
        Rc::new(RefCell::new(DelimSequenceState { delim, delim_count: 1 }))
    }
}
impl TokenAnalyzer for DelimSequenceState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == self.delim {
            // delimiter character found, we just increment the counter and otherwise ignore it,
            // except when there are too many
            if self.delim_count >= 5 {
                return StateResult::CharError(0, E_CFG_TOML_TOO_MANY_QUOTES, ch)
            }
            self.delim_count += 1;
            return StateResult::CharProcessed(false)
        }
        // non delimiter character found
        if self.delim_count < 3 {
            // less than three delimiter characters are allowed and simpy part of the string
            return StateResult::ResumeCallingState(1, self.delim, self.delim_count)
        }
        if self.delim_count == 3 {
            // exactly three delimiter characters mark the end of the string
            return StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::String, None)
        }
        // more than three delimiter characters means up to two delimiter characters at the end of
        // the string.
        let mut cont_str = self.delim.to_string();
        if self.delim_count > 4 { cont_str.push(self.delim); }
        StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::String, Some(cont_str))
    }
    fn activate(&mut self) {
        self.delim_count = 1;
    }
}

/// Handler state, when a carriage return was found immediately after the opening delimiter of a
/// multi-line string.
pub(super) struct InitialMultiLineCrState {
}
impl InitialMultiLineCrState {
    /// Creates a handler state for initial carriage return in multiline strings
    pub(super) fn new() -> Rc<RefCell<InitialMultiLineCrState>> {
        Rc::new(RefCell::new(InitialMultiLineCrState {}))
    }
}
impl TokenAnalyzer for InitialMultiLineCrState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == LINE_FEED {
            // line feed follows after carriage return, i.e. we found an end of line after the
            // opening delimiter. Both carriage return and line feed are ignored then.
            return StateResult::ResumeCallingState(0, NULL, 0)
        }
        // carriage return is not followed by line feed, then both carriage return and current
        // character must be considered as part of the string.
        // We return the carriage return and move the input index one to the left to process the
        // current character again, this time in the suspended state.
        StateResult::ResumeCallingState(1, CARRIAGE_RETURN, 1)
    }
}