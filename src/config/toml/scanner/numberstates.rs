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

//! Coaly TOML number handling scanner states.

use super::*;
use std::cell::RefCell;
use std::char;
use std::rc::Rc;

/// Handler state for tokens starting with a sign followed by character '0'.
pub(super) struct SignedZeroState {
}
impl SignedZeroState {
    /// Creates a handler state for values starting with  a sign followed by character '0'.
    pub(super) fn new() -> Rc<RefCell<SignedZeroState>> {
        Rc::new(RefCell::new(SignedZeroState {}))
    }
}
impl TokenAnalyzer for SignedZeroState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            'e' | 'E' => StateResult::Finished(0, false, true, ScannerStateId::FloatExponent),
            '.' => StateResult::Finished(0, false, true, ScannerStateId::FloatFraction),
            SPACE | TAB | LINE_FEED | CARRIAGE_RETURN | NULL | ',' | ';' | '}' | ']' => {
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Integer, None)
            },
            '0' ..= '9' => StateResult::CharError(1, E_CFG_TOML_LEADING_ZERO_NOT_ALLOWED, '0'),
            _ => StateResult::CharError(0, E_CFG_TOML_INV_NUM_CHAR, ch)
        }
    }
}

/// Handler state for symbolic values.
/// Valid symbolic values are:
/// true  - boolean true
/// false - boolean false
/// inf   - float positive infinity
/// +inf  - float positive infinity
/// -inf  - float negative infinity
/// nan   - float invalid positive number
/// +nan  - float invalid positive number
/// -nan  - float invalid negative number
pub(super) struct SymbolicValueState {
    // buffer for symbol name
    sym_buffer: Vec<char>
}
impl SymbolicValueState {
    /// Creates a handler state for symbolic boolean or float values.
    pub(super) fn new() -> Rc<RefCell<SymbolicValueState>> {
        Rc::new(RefCell::new(SymbolicValueState { sym_buffer: Vec::with_capacity(8) }))
    }
    fn check_sym(&self) -> StateResult {
        let sym = self.sym_buffer.iter().collect::<String>();
        match sym.as_str() {
            "true" | "false" => {
                StateResult::TokenFound(1, false, TokenId::Value, 
                                        TokenValueType::Boolean, Some(sym))
            },
            "inf" | "+inf" | "-inf" | "nan" | "+nan" | "-nan" => {
                StateResult::TokenFound(1, false, TokenId::Value,
                                        TokenValueType::Float, Some(sym))
            },
            _ => StateResult::Error(E_CFG_TOML_INV_VALUE, false, Some(quoted(&sym)))
        }
    }
}
impl TokenAnalyzer for SymbolicValueState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            'A' ..= 'Z' | 'a' ..= 'z' | '0' ..= '9' | '_' | '+' | '-' => {
                self.sym_buffer.push(ch);
                StateResult::CharProcessed(false)
            }
            _ => self.check_sym()
        }
    }
    fn activate(&mut self) {
        self.sym_buffer.clear();
    }
}

/// Handler state for an integer with a defined radix.
pub(super) struct RadixIntState {
    // validator function for the characters allowed
    validate_digit: ValidateChar,
    // holds the last character processed, needed to detect invalid underscore usage
    last_char: char
}
impl RadixIntState {
    /// Creates a handler state for an integer with a defined radix.
    ///
    /// # Arguments
    /// * `digit_validator` - function checking whether a digit is allowed
    pub(super) fn new(digit_validator: ValidateChar) -> Rc<RefCell<RadixIntState>> {
        Rc::new(RefCell::new(RadixIntState { validate_digit: digit_validator, last_char: '_' }))
    }
}
impl TokenAnalyzer for RadixIntState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if (self.validate_digit)(ch) {
            self.last_char = ch;
            return StateResult::CharProcessed(true)
        }
        if ch == '_' {
            if self.last_char == '_' {
                return StateResult::CharError(0, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, ch)
            }
            self.last_char = '_';
            return StateResult::CharProcessed(false)
        }
        if ch.is_whitespace() || ch == NULL || ch == ',' || ch == ';' || ch == '}' || ch == ']' {
            if self.last_char == '_' {
                return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
            }
            return StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Integer, None)
        }
        StateResult::CharError(0, E_CFG_TOML_INV_NUM_CHAR, ch)
    }
    fn activate(&mut self) {
        self.last_char = '_';
    }
}

/// Handler state for a number.
/// State is activated as soon as it's clear that the current token cannot be a date or time and
/// a possible leading sign has been processed.
pub(super) struct NumberState {
    // number of currently consumed digits, underscores are not counted.
    // needed to validate that the number has at least one digit.
    // value is preset with 0
    digit_count: u32,
    // holds the last character processed, needed to detect invalid underscore usage
    last_char: char
}
impl NumberState {
    /// Creates a handler state for a number.
    pub(super) fn new() -> Rc<RefCell<NumberState>> {
        Rc::new(RefCell::new(NumberState { digit_count: 0, last_char: '_' }))
    }
}
impl TokenAnalyzer for NumberState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '0' ..= '9' => {
                self.digit_count += 1;
                self.last_char = ch;
                StateResult::CharProcessed(true)
            },
            '_' => {
                if self.last_char == '_' {
                    return StateResult::CharError(0, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, ch)
                }
                self.last_char = ch;
                StateResult::CharProcessed(false)
            },
            TAB | SPACE | LINE_FEED | CARRIAGE_RETURN | NULL | ',' | ';' | '}' | ']' => {
                if self.last_char == '_' {
                    return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
                }
                if self.digit_count == 0 {
                    return StateResult::CharError(0, E_CFG_TOML_DIGIT_EXPECTED, ch)
                }
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Integer, None)
            },
            'e' | 'E' => {
                if self.last_char == '_' {
                    return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
                }
                StateResult::Finished(0, false, true, ScannerStateId::FloatExponent)
            },
            '.' => {
                if self.last_char == '_' {
                    return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
                }
                StateResult::Finished(0, false, true, ScannerStateId::FloatFraction)
            },
            _ => StateResult::CharError(0, E_CFG_TOML_INV_NUM_CHAR, ch)
        }
    }
    fn activate(&mut self) {
        self.digit_count = 0;
        self.last_char = '_';
    }
}

/// Handler state for a signed number.
pub(super) struct SignedNumberState {
}
impl SignedNumberState {
    /// Creates a handler state for a number.
    pub(super) fn new() -> Rc<RefCell<SignedNumberState>> {
        Rc::new(RefCell::new(SignedNumberState {}))
    }
}
impl TokenAnalyzer for SignedNumberState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '0' => StateResult::Finished(0, false, true, ScannerStateId::SignedZero),
            '1' ..= '9' => StateResult::Finished(1, false, false, ScannerStateId::Number),
            'i' | 'n' => StateResult::Finished(1, false, false, ScannerStateId::SymbolicValue),
            _ => StateResult::CharError(0, E_CFG_TOML_DIGIT_EXPECTED, ch)
        }
    }
}

/// Handler state for the fraction part of a floating point number.
/// The state is activated when a dot was detected within a number.
pub(super) struct FloatFractionState {
    // number of currently consumed digits, underscores are not counted.
    // needed to validate that the number has at least one digit.
    // value is preset with 0
    digit_count: u32,
    // holds the last character processed, needed to detect invalid underscore usage
    last_char: char
}
impl FloatFractionState {
    /// Creates a handler state for the fraction part of a floating point number
    pub(super) fn new() -> Rc<RefCell<FloatFractionState>> {
        Rc::new(RefCell::new(FloatFractionState {digit_count: 0, last_char: '_' }))
    }
}
impl TokenAnalyzer for FloatFractionState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '0' ..= '9' => {
                self.digit_count += 1;
                self.last_char = ch;
                StateResult::CharProcessed(true)
            },
            '_' => {
                if self.last_char == '_' {
                    return StateResult::CharError(0, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, ch)
                }
                self.last_char = ch;
                StateResult::CharProcessed(false)
            },
            'e' | 'E' => {
                if self.digit_count == 0 {
                    return StateResult::Error(E_CFG_TOML_EMPTY_FLOAT_FRACT, true, None)
                }
                if self.last_char == '_' {
                    return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
                }
                StateResult::Finished(0, false, true, ScannerStateId::FloatExponent)
            },
            SPACE | TAB | LINE_FEED | CARRIAGE_RETURN | NULL | ',' | ';' | '}' | ']' => {
                if self.digit_count == 0 {
                    return StateResult::Error(E_CFG_TOML_EMPTY_FLOAT_FRACT, true, None)
                }
                if self.last_char == '_' {
                    return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
                }
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Float, None)
            },
            _ => StateResult::CharError(0, E_CFG_TOML_INV_NUM_CHAR, ch)
        }
    }
    fn activate(&mut self) {
        self.digit_count = 0;
        self.last_char = '_';
    }
}

/// Handler state for the exponent part of a floating point number.
/// The state is activated when the letter e or E was detected within a number.
pub(super) struct FloatExponentState {
    // number of currently consumed digits, underscores are not counted.
    // needed to validate that the number has at least one digit.
    // value is preset with 0
    digit_count: u32,
    // holds the last character processed, needed to detect invalid underscore usage
    last_char: char,
    // indicator for a detected exponent sign
    sign_defined: bool
}
impl FloatExponentState {
    /// Creates a handler state for the fraction part of a floating point number
    pub(super) fn new() -> Rc<RefCell<FloatExponentState>> {
        Rc::new(RefCell::new(FloatExponentState { digit_count: 0, last_char: '_',
                                                  sign_defined: false }))
    }
}
impl TokenAnalyzer for FloatExponentState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        match ch {
            '0' ..= '9' => {
                self.digit_count += 1;
                self.last_char = ch;
                StateResult::CharProcessed(true)
            },
            '_' => {
                if self.last_char == '_' {
                    return StateResult::CharError(0, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, ch)
                }
                self.last_char = ch;
                StateResult::CharProcessed(false)
            },
            '+' | '-' => {
                if self.digit_count > 0 || self.sign_defined {
                    return StateResult::Error(E_CFG_TOML_INV_FLOAT_EXP, true, None)
                }
                self.sign_defined = true;
                StateResult::CharProcessed(ch == '-')
            },
            SPACE | TAB | LINE_FEED | CARRIAGE_RETURN | NULL | ',' | ';' | '}' | ']' => {
                if self.digit_count == 0 {
                    return StateResult::Error(E_CFG_TOML_INV_FLOAT_EXP, true, None)
                }
                if self.last_char == '_' {
                    return StateResult::CharError(1, E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED, '_')
                }
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::Float, None)
            },
            _ => StateResult::CharError(0, E_CFG_TOML_INV_NUM_CHAR, ch)
        }
    }
    fn activate(&mut self) {
        self.digit_count = 0;
        self.last_char = '_';
        self.sign_defined = false;
    }
}