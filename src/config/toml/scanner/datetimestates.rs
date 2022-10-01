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

//! TOML scanner states for date and time handling.

use super::*;
use std::cell::RefCell;
use std::char;
use std::rc::Rc;

// Array of functions for the validation of a time value with hour, minute and second
const TIME_VALIDATORS_HMS: &[ValidateChar] = &[validate_hour1, validate_dec_digit,
                                               validate_colon,
                                               validate_min_sec1, validate_dec_digit,
                                               validate_colon,
                                               validate_min_sec1, validate_dec_digit];
// Array of functions for the validation of a time value with hour and minute
const TIME_VALIDATORS_HM: &[ValidateChar] = &[validate_hour1, validate_dec_digit,
                                              validate_colon,
                                              validate_min_sec1, validate_dec_digit];
// Array of functions for the validation of a time value with minute and second
const TIME_VALIDATORS_MS: &[ValidateChar] = &[validate_min_sec1, validate_dec_digit,
                                              validate_colon,
                                              validate_min_sec1, validate_dec_digit];
// Length of array of functions for the validation of a time value with hour, minute and second
const TIME_VALIDATORS_HMS_LEN: usize = TIME_VALIDATORS_HMS.len();
// Length of array of functions for the validation of a time value with hour and minute
const TIME_VALIDATORS_HM_LEN: usize = TIME_VALIDATORS_HM.len();
// Length of array of functions for the validation of a time value with minute and second
const TIME_VALIDATORS_MS_LEN: usize = TIME_VALIDATORS_MS.len();

/// Checks whether the given character is allowed as the first digit of an hour value.
fn validate_hour1(ch: char) -> bool { ('0' ..= '2').contains(&ch) }

/// Checks whether the given character is allowed as the second digit of
/// an hour, minute or second value.
fn validate_dec_digit(ch: char) -> bool { ch.is_ascii_digit() }

/// Checks whether the given character is allowed as the first digit of a minute or second value.
fn validate_min_sec1(ch: char) -> bool { ('0' ..= '5').contains(&ch) }

/// Checks whether the given character is allowed as a delimiter between time value parts
fn validate_colon(ch: char) -> bool { ch == ':' }

/// Handler state for a local time value.
/// The state is activated after the first colon has been read, i.e.
/// starting with the minute specification.
/// Values have format hh:mm:ss[.u+]
pub(super) struct LocalTimeState {
    // number of currently consumed characters, needed to validate a local time.
    // value is preset with 0, since the last character consumed by the predecessor state was a
    // colon
    char_count: usize
}
impl LocalTimeState {
    /// Creates a handler state for a local time value.
    pub(super) fn new() -> Rc<RefCell<LocalTimeState>> {
        Rc::new(RefCell::new(LocalTimeState { char_count: 0 }))
    }
}
impl TokenAnalyzer for LocalTimeState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == SPACE || ch == TAB || ch == LINE_FEED || ch == CARRIAGE_RETURN || ch == NULL {
            if self.char_count < TIME_VALIDATORS_MS_LEN {
                return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
            }
            return StateResult::TokenFound(1, false, TokenId::Value,
                                           TokenValueType::LocalTime, None)
        }
        if self.char_count < TIME_VALIDATORS_MS_LEN && TIME_VALIDATORS_MS[self.char_count](ch) {
            self.char_count += 1;
            return StateResult::CharProcessed(true)
        }
        if self.char_count == TIME_VALIDATORS_MS_LEN {
            if ch == '.' {
                self.char_count += 1;
                return StateResult::Suspended(1, ScannerStateId::FractionalSeconds)
            }
            return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
        }
        StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
    }
    fn activate(&mut self) {
        self.char_count = 0;
    }
}

/// Handler state for a time value within a date-time specification.
/// In contrast to a local time value an optional time zone offset is allowed here.
/// The state is acivated from the beginning of the time specification.
/// Values have format hh:mm:ss[.u+][timezone offset]
pub(super) struct OffsetTimeState {
    // number of currently consumed characters, needed to validate a local time.
    // value is preset with 0, since the last character consumed by the predecessor state was a
    // T, space or the last digit of the date
    char_count: usize
}
impl OffsetTimeState {
    /// Creates a handler state for a time value within a date-time specification.
    pub(super) fn new() -> Rc<RefCell<OffsetTimeState>> {
        Rc::new(RefCell::new(OffsetTimeState { char_count: 0 }))
    }
}
impl TokenAnalyzer for OffsetTimeState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == SPACE || ch == TAB || ch == LINE_FEED || ch == CARRIAGE_RETURN || ch == NULL {
            match self.char_count {
                TIME_VALIDATORS_HMS_LEN => {
                    return StateResult::TokenFound(1, false, TokenId::Value,
                                                   TokenValueType::LocalDateTime, None)
                },
                _ => return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
            }
        }
        if self.char_count < TIME_VALIDATORS_HMS_LEN {
            if TIME_VALIDATORS_HMS[self.char_count](ch) {
                self.char_count += 1;
                return StateResult::CharProcessed(true)
            }
            return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
        }
        match ch {
            '.' => {
                if self.char_count != TIME_VALIDATORS_HMS_LEN {
                    return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
                }
                StateResult::Suspended(1, ScannerStateId::FractionalSeconds)
            },
            '+' | '-' | 'Z' => StateResult::Finished(1, false, false,
                                                     ScannerStateId::TimeZoneOffset),
            _ => StateResult::Error(E_CFG_TOML_TZ_OR_MS_EXPECTED, true, None)
        }
    }
    fn activate(&mut self) {
        self.char_count = 0;
    }
}

/// Handler state for a time zone offset.
/// A time zone offset specifies the difference of the local time to UTC and is given either by
/// character 'Z' (for UTC) or an hour/minute offset relative to UTC ([+|-]hh:mm)
/// The state is activated after the raw time specification has been completely read, i.e. from
/// the beginning of the offset specification.
pub(super) struct TimeZoneOffsetState {
    // number of currently consumed characters, needed to validate the time offset.
    // value is preset with 0, since the last character consumed by the predecessor state was
    // the last digit of the time
    char_count: usize,
    // indicator for UTC
    is_utc: bool
}
impl TimeZoneOffsetState {
    /// Creates a handler state for a time zone offset.
    pub(super) fn new() -> Rc<RefCell<TimeZoneOffsetState>> {
        Rc::new(RefCell::new(TimeZoneOffsetState { char_count: 0, is_utc: false }))
    }
}
impl TokenAnalyzer for TimeZoneOffsetState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch == SPACE || ch == TAB || ch == LINE_FEED || ch == CARRIAGE_RETURN || ch == NULL {
            if self.is_utc || self.char_count > TIME_VALIDATORS_HM_LEN {
                return StateResult::TokenFound(1, false, TokenId::Value,
                                               TokenValueType::OffsetDateTime, None)
            }
            return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
        }
        if self.char_count == 0 {
            self.char_count = 1;
            match ch {
                '+' | '-' => return StateResult::CharProcessed(true),
                'Z' => {
                    self.is_utc = true;
                    return StateResult::TokenFound(0, false, TokenId::Value,
                                                   TokenValueType::OffsetDateTime,
                                                   Some(ch.to_string()))
                },
                _ => return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
            }
        }
        if self.is_utc || self.char_count > TIME_VALIDATORS_HM_LEN {
            return StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
        }
        if (TIME_VALIDATORS_HM[self.char_count-1])(ch) {
            self.char_count += 1;
            return StateResult::CharProcessed(true)
        }
        StateResult::Error(E_CFG_TOML_INV_TIME, true, None)
    }
    fn activate(&mut self) {
        self.char_count = 0;
        self.is_utc = false;
    }
}

/// Handler state for a date or date and time value.
/// This state is activated when four decimal digits followed by a dash have been detected.
pub(super) struct DateOrDateTimeState {
    // number of currently consumed characters, needed to validate the date or date time.
    // value is preset with 0, since the last character consumed by the predecessor state was
    // the dash after the year value.
    char_count: u32
}
impl DateOrDateTimeState {
    /// Creates a handler state for a date and time.
    pub(super) fn new() -> Rc<RefCell<DateOrDateTimeState>> {
        Rc::new(RefCell::new(DateOrDateTimeState { char_count: 0 }))
    }
}
impl TokenAnalyzer for DateOrDateTimeState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        self.char_count += 1;
        match ch {
            '0' ..= '9' => {
                if self.char_count > 6 {
                    return StateResult::Finished(1, false, false, ScannerStateId::OffsetTime)
                }
                if self.char_count == 3 || self.char_count > 5 {
                    return StateResult::Error(E_CFG_TOML_INV_DATE, true, None)
                }
                StateResult::CharProcessed(true)
            },
            '-' => {
                if self.char_count < 3 {
                    return StateResult::Error(E_CFG_TOML_2DIGIT_MONTH_REQUIRED, false, None)
                }
                if self.char_count > 3 {
                    return StateResult::Error(E_CFG_TOML_INV_DATE, true, None)
                }
                StateResult::CharProcessed(true)
            },
            'T' => {
                if self.char_count == 5 {
                    return StateResult::Error(E_CFG_TOML_2DIGIT_DAY_REQUIRED, false, None)
                }
                if self.char_count != 6 {
                    return StateResult::Error(E_CFG_TOML_INV_DATE, true, None)
                }
                StateResult::Finished(0, false, true, ScannerStateId::OffsetTime)
            },
            SPACE => {
                if self.char_count == 5 {
                    return StateResult::Error(E_CFG_TOML_2DIGIT_DAY_REQUIRED, false, None)
                }
                if self.char_count != 6 {
                    return StateResult::Error(E_CFG_TOML_INV_DATE, true, None)
                }
                StateResult::Suspended(0, ScannerStateId::SpaceAfterDate)
            },
            TAB | CARRIAGE_RETURN | LINE_FEED | NULL => {
                if self.char_count == 5 {
                    return StateResult::Error(E_CFG_TOML_2DIGIT_DAY_REQUIRED, true, None)
                }
                if self.char_count < 6 {
                    return StateResult::Error(E_CFG_TOML_INV_DATE, true, None)
                }
                StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::LocalDate, None)
            },
            _ => {
                StateResult::Error(E_CFG_TOML_INV_DATE, true, None)
            }
        }
    }
    fn activate(&mut self) {
        self.char_count = 0;
    }
}

/// Handler state for a date followed by a space.
/// The space has already been processed, but not stored in the scanner's token value attribute.
pub(super) struct SpaceAfterDateState {
}
impl SpaceAfterDateState {
    /// Creates a handler state for a date followed by a space.
    pub(super) fn new() -> Rc<RefCell<SpaceAfterDateState>> {
        Rc::new(RefCell::new(SpaceAfterDateState {}))
    }
}
impl TokenAnalyzer for SpaceAfterDateState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if ch.is_ascii_digit() {
            // space is followed by a digit: date is extended with a time offset.
            // store space character in token value and decrease input index to process digit
            // again by calling state
            return StateResult::ResumeCallingState(1, ' ', 1)
        }
        // space is not followed by digit: pure date
        StateResult::TokenFound(1, false, TokenId::Value, TokenValueType::LocalDate, None)
    }
}

/// Handler state for the fractional seconds of a time value.
/// The state is activated when the dot within a time value has been read, but input index is
/// decremented by 1. Hence the first character is always a dot.
pub(super) struct FractionalSecondsState {
    // number of currently consumed characters, needed to validate the fractional seconds.
    // value is preset with 0, since the last character consumed by the predecessor state was
    // the last digit of the second value.
    char_count: usize
}
impl FractionalSecondsState {
    /// Creates a handler state for the fractional seconds of a time value.
    pub(super) fn new() -> Rc<RefCell<FractionalSecondsState>> {
        Rc::new(RefCell::new(FractionalSecondsState { char_count: 0 }))
    }
}
impl TokenAnalyzer for FractionalSecondsState {
    fn process_char(&mut self, ch: char, _expect_key: bool) -> StateResult {
        if self.char_count == 0  {
            self.char_count = 1;
            return StateResult::CharProcessed(true)
        }
        if ch.is_ascii_digit() {
            self.char_count += 1;
            return StateResult::CharProcessed(true)
        }
        if self.char_count < 2 { return StateResult::Error(E_CFG_TOML_INV_TIME, true, None) }
        StateResult::ResumeCallingState(1, NULL, 0)
    }
    fn activate(&mut self) {
        self.char_count = 0;
    }
}