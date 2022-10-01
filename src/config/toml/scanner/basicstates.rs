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

//! Basic TOML scanner states

use super::*;
use std::cell::RefCell;
use std::char;
use std::rc::Rc;

/// Handler state when outside any lexical TOML unit.
pub(super) struct IdleState {
}
impl IdleState {
    // Creates a handler state, where processing of the next token starts.
    pub(super) fn new() -> Rc<RefCell<IdleState>> {
        Rc::new(RefCell::new(IdleState{}))
    }
}
impl TokenAnalyzer for IdleState {
    fn process_char(&mut self, ch: char, expect_key: bool) -> StateResult {
        match ch {
            LINE_FEED => StateResult::TokenFound(0, true, TokenId::LineBreak,
                                                 TokenValueType::String,
                                                 Some(String::from("<END-OF-LINE>"))),
            NULL => StateResult::TokenFound(0, true, TokenId::LineBreak,
                                            TokenValueType::String,
                                            Some(String::from("<END-OF-INPUT>"))),
            CARRIAGE_RETURN => StateResult::Finished(0, true, false, ScannerStateId::LineBreak),
            TAB | SPACE  => StateResult::CharProcessed(false),
            '=' => StateResult::TokenFound(0, true, TokenId::Equal, TokenValueType::String,
                                           Some(String::from("="))),
            ',' => StateResult::TokenFound(0, true, TokenId::Comma, TokenValueType::String,
                                           Some(String::from(","))),
            '.' => StateResult::TokenFound(0, true, TokenId::Dot, TokenValueType::String,
                                           Some(String::from("."))),
            '{' => StateResult::TokenFound(0, true, TokenId::LeftBrace,
                                           TokenValueType::String, Some(String::from("{"))),
            '}' => StateResult::TokenFound(0, true, TokenId::RightBrace,
                                           TokenValueType::String, Some(String::from("}"))),
            '[' => {
                if expect_key {
                    return StateResult::Finished(0, true, true, ScannerStateId::LBracket)
                }
                StateResult::TokenFound(0, true, TokenId::LeftBracket,
                                        TokenValueType::String, Some(String::from("[")))
            },
            ']' => {
                if expect_key {
                    return StateResult::Finished(0, true, true, ScannerStateId::RBracket)
                }
                StateResult::TokenFound(0, true, TokenId::RightBracket,
                                        TokenValueType::String, Some(String::from("]")))
            },
            '"' => {
                if expect_key {
                    return StateResult::Finished(0, true, false, ScannerStateId::DoubleQuotedKey)
                }
                StateResult::Finished(0, true, false, ScannerStateId::StartOfBasicString)
            },
            '\'' => {
                if expect_key {
                    return StateResult::Finished(0, true, false, ScannerStateId::SingleQuotedKey)
                }
                StateResult::Finished(0, true, false, ScannerStateId::StartOfLiteralString)
            },
            '+' => {
                if expect_key {
                    return StateResult::CharError(0, E_CFG_TOML_INV_KEY_START, ch)
                }
                StateResult::Finished(0, true, false, ScannerStateId::SignedNumber)
            },
            '-' => {
                if expect_key {
                    return StateResult::Finished(0, true, true, ScannerStateId::BareKey)
                }
                StateResult::Finished(0, true, true, ScannerStateId::SignedNumber)
            },
            'A' ..= 'Z' | 'a' ..= 'z' => {
                if expect_key {
                    return StateResult::Finished(0, true, true, ScannerStateId::BareKey)
                }
                StateResult::Finished(1, true, false, ScannerStateId::SymbolicValue)
            },
            '_' => {
                if expect_key {
                    return StateResult::Finished(0, true, true, ScannerStateId::BareKey)
                }
                StateResult::CharError(0, E_CFG_TOML_INV_VALUE_START, ch)
            },
            '0' => {
                if expect_key {
                    return StateResult::Finished(1, true, false, ScannerStateId::BareKey)
                }
                StateResult::Finished(0, true, true, ScannerStateId::Zero)
            },
            '1' ..= '9' => {
                if expect_key {
                    return StateResult::Finished(0, true, true, ScannerStateId::BareKey)
                }
                StateResult::Finished(0, true, true, ScannerStateId::NumberOrDateTime)
            },
            '#' => StateResult::Finished(0, false, false, ScannerStateId::Comment),
            _ => StateResult::CharError(0, E_CFG_TOML_INVALID_CHAR, ch)
        }
    }
}

/// Handler state for TOML comments.
/// - A hash symbol marks the rest of the line as a comment, except when inside a string.
/// - Control characters other than tab (U+0000 to U+0008, U+000A to U+001F, U+007F)
///   are not permitted in comments.
pub(super) struct CommentState {
}
impl CommentState {
    // Creates a handler state for TOML comments.
    pub(super) fn new() -> Rc<RefCell<CommentState>> {
        Rc::new(RefCell::new(CommentState {}))
    }
}
impl TokenAnalyzer for CommentState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            LINE_FEED | NULL => StateResult::TokenFound(0, true, TokenId::LineBreak,
                                                        TokenValueType::String, None),
            CARRIAGE_RETURN => StateResult::Finished(0, true, false, ScannerStateId::LineBreak),
            '\u{0001}'..='\u{0008}' | '\u{000b}' | '\u{000c}'
                                    | '\u{000e}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            _ => StateResult::CharProcessed(false)
        }
    }
}

/// Handler state for line breaks outside of strings.
pub(super) struct LineBreakState {
}
impl LineBreakState {
    // Creates a handler state for line breaks outside of strings.
    pub(super) fn new() -> Rc<RefCell<LineBreakState>> {
        Rc::new(RefCell::new(LineBreakState {}))
    }
}
impl TokenAnalyzer for LineBreakState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == LINE_FEED {
            return StateResult::TokenFound(0, true, TokenId::LineBreak,
                                           TokenValueType::String, None)
        }
        StateResult::CharError(1, E_CFG_TOML_INV_CTRL_CHAR, CARRIAGE_RETURN)
    }
}

/// Handler state for single or double brackets.
/// State is activated, when a single bracket has been consumed.
pub(super) struct BracketState {
    // bracket character ([ or ])
    bracket_char: char,
    // token ID to return, if a single bracket is detected
    single_id: TokenId,
    // token ID to return, if a double bracket is detected
    double_id: TokenId
}
impl BracketState {
    /// Creates a handler state for single or double brackets.
    /// #Arguments
    /// * `bracket_char` - the bracket character ([ or ])
    /// * `single_id` - the token ID to return, if a single bracket is detected
    /// * `double_id` - the token ID to return, if a double bracket is detected
    pub(super) fn new(bracket_char: char,
               single_id: TokenId,
               double_id: TokenId) -> Rc<RefCell<BracketState>> {
        Rc::new(RefCell::new(BracketState {bracket_char, single_id, double_id}))
    }
}
impl TokenAnalyzer for BracketState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == self.bracket_char {
            return StateResult::TokenFound(0, false, self.double_id, TokenValueType::String,
                                           Some(self.bracket_char.to_string()))
        }
        StateResult::TokenFound(1, false, self.single_id, TokenValueType::String, None)
    }
}

/// Handler state for bare keys.
/// Bare keys may only contain ASCII letters, ASCII digits, underscores, and dashes (A-Za-z0-9_-).
/// Note that bare keys are allowed to be composed of only ASCII digits, e.g. 1234,
/// but are always interpreted as strings.
pub(super) struct BareKeyState {
}
impl BareKeyState {
    /// Creates a handler state for bare keys.
    pub(super) fn new() -> Rc<RefCell<BareKeyState>> {
        Rc::new(RefCell::new(BareKeyState {}))
    }
}
impl TokenAnalyzer for BareKeyState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            return StateResult::CharProcessed(true)
        }
        StateResult::TokenFound(1, false, TokenId::Key, TokenValueType::String, None)
    }
}

/// Handler state for double quoted keys.
/// Double quoted keys follow the exact same rules as basic strings and allow you to use a much
/// broader set of key names. Best practice is to use bare keys except when absolutely necessary.
/// Basic strings are surrounded by quotation marks (").
/// Any Unicode character may be used except those that must be escaped:
///   quotation mark, backslash, and the control characters other than tab
///   (U+0000 to U+0008, U+000A to U+001F, U+007F).
pub(super) struct DoubleQuotedKeyState {
}
impl DoubleQuotedKeyState {
    /// Creates a handler state for double quoted keys.
    pub(super) fn new() -> Rc<RefCell<DoubleQuotedKeyState>> {
        Rc::new(RefCell::new(DoubleQuotedKeyState {}))
    }
}
impl TokenAnalyzer for DoubleQuotedKeyState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '"' => StateResult::TokenFound(0, false, TokenId::Key, TokenValueType::String, None),
            '\\' => StateResult::Suspended(0, ScannerStateId::SingleLineEscSequence),
            '\u{0001}'..='\u{0008}' | '\u{000a}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            NULL => StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None),
            _ => StateResult::CharProcessed(true)
        }
   }
}

/// Handler state for single quoted keys.
/// Single quoted keys follow the exact same rules as literal strings and allow you to use a much
/// broader set of key names. Best practice is to use bare keys except when absolutely necessary.
/// Literal strings are surrounded by single quotes.
/// Like basic strings, they must appear on a single line.
/// There is no escaping.
/// Control characters other than tab are not permitted in a literal string
pub(super) struct SingleQuotedKeyState {
}
impl SingleQuotedKeyState {
    /// Creates a handler state for single quoted keys.
    pub(super) fn new() -> Rc<RefCell<SingleQuotedKeyState>> {
        Rc::new(RefCell::new(SingleQuotedKeyState {}))
    }
}
impl TokenAnalyzer for SingleQuotedKeyState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '\'' => StateResult::TokenFound(0, false, TokenId::Key, TokenValueType::String, None),
            '\u{0001}'..='\u{0008}' | '\u{000a}'..='\u{001f}' | '\u{007f}' => {
                StateResult::CharError(0, E_CFG_TOML_INV_CTRL_CHAR, ch)
            },
            NULL => StateResult::Error(E_CFG_TOML_UNTERMINATED_STR, false, None),
            _ => StateResult::CharProcessed(true)
        }
   }
}

/// Handler state for tokens starting with character '0'.
/// State is activated when a '0' has been detected.
pub(super) struct ZeroState {
    // number of currently consumed characters, needed to validate a local time resp. to detect
    // numbers with leading zero.
    // value is preset with 1, since the initial '0' was already consumed in predecessor state
    char_count: u32
}
impl ZeroState {
    /// Creates a handler state for values starting with character '0'.
    /// This may be a number or local time value only, since dates before 1000 are not supported.
    pub(super) fn new() -> Rc<RefCell<ZeroState>> {
        Rc::new(RefCell::new(ZeroState { char_count: 1 }))
    }
}
impl TokenAnalyzer for ZeroState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            'b' => {
                if self.char_count >= 2 {
                    return StateResult::Error(E_CFG_TOML_INV_RADIX_PREFIX, false, None)
                }
                StateResult::Finished(0, false, true, ScannerStateId::BinInt)
            },
            'o' => {
                if self.char_count >= 2 {
                    return StateResult::Error(E_CFG_TOML_INV_RADIX_PREFIX, false, None)
                }
                StateResult::Finished(0, false, true, ScannerStateId::OctInt)
            },
            'x' => {
                if self.char_count >= 2 {
                    return StateResult::Error(E_CFG_TOML_INV_RADIX_PREFIX, false, None)
                }
                StateResult::Finished(0, false, true, ScannerStateId::HexInt)
            },
            'e' | 'E' => StateResult::Finished(0, false, true, ScannerStateId::FloatExponent),
            '.' => {
                if self.char_count >= 2 {
                    return StateResult::CharError(1, E_CFG_TOML_LEADING_ZERO_NOT_ALLOWED, '0')
                }
                StateResult::Finished(0, false, true, ScannerStateId::FloatFraction)
            },
            SPACE | TAB | LINE_FEED | CARRIAGE_RETURN | NULL | ',' | '}' | ']' => {
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Integer, None)
            },
            '0' ..= '9' => {
                if self.char_count >= 2 {
                    return StateResult::CharError(1, E_CFG_TOML_LEADING_ZERO_NOT_ALLOWED, '0')
                }
                self.char_count = 2;
                StateResult::CharProcessed(true)
            },
            ':' => {
                if self.char_count < 2 {
                    return StateResult::CharError(1, E_CFG_TOML_2DIGIT_HOUR_REQUIRED, ch)
                }
                StateResult::Finished(0, false, true, ScannerStateId::LocalTime)
            },
            _ => StateResult::Error(E_CFG_TOML_INV_VALUE, true, None)
        }
    }
    fn activate(&mut self) {
        self.char_count = 1;
    }
}

/// Handler state for a number, date or time value.
/// State is activated when a decimal digit in range 1 to 9 has been detected.
pub(super) struct NumberOrDateTimeState {
    // number of currently consumed characters, needed to validate a local date or time.
    // value is preset with 1, since the initial '0' was already consumed in predecessor state
    char_count: u32
}
impl NumberOrDateTimeState {
    /// Creates a handler state for a number, date or time value.
    pub(super) fn new() -> Rc<RefCell<NumberOrDateTimeState>> {
        Rc::new(RefCell::new(NumberOrDateTimeState { char_count: 1 }))
    }
}
impl TokenAnalyzer for NumberOrDateTimeState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        self.char_count += 1;
        match ch {
            '0' ..= '9' => {
                if self.char_count > 4 {
                    return StateResult::Finished(1, false, false, ScannerStateId::Number)
                }
                StateResult::CharProcessed(true)
            },
            ':' => {
                if self.char_count != 3 {
                    return StateResult::CharError(1, E_CFG_TOML_2DIGIT_HOUR_REQUIRED, ch)
                }
                StateResult::Finished(0, false, true, ScannerStateId::LocalTime)
            },
            '-' => {
                if self.char_count != 5 {
                    return StateResult::CharError((self.char_count-1) as usize,
                                                  E_CFG_TOML_4DIGIT_YEAR_REQUIRED, ch)
                }
                StateResult::Finished(0, false, true, ScannerStateId::DateOrDateTime)
            },
            '_' => StateResult::Finished(0, false, false, ScannerStateId::Number),
            TAB | SPACE | LINE_FEED | CARRIAGE_RETURN | NULL | ',' | ';' | '}' | ']' => {
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Integer, None)
            },
            'e' | 'E' => StateResult::Finished(0, false, true, ScannerStateId::FloatExponent),
            '.' =>StateResult::Finished(0, false, true, ScannerStateId::FloatFraction),
            _ => StateResult::CharError(0, E_CFG_TOML_INV_NUMDT_CHAR, ch)
        }
    }
    fn activate(&mut self) {
        self.char_count = 1;
    }
}