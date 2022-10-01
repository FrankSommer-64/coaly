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

//! Common exceptions for all parts of Coaly.

use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};

/// Raise an exception with severity error
#[macro_export]
macro_rules! coalyxe {
    ($id: expr) => {
        CoalyException::new($id, Severity::Error)
    };
    ($id: expr $(,$arg: expr)+) => {
        CoalyException::with_args($id, Severity::Error, &[$($arg),+])
    };
}

/// Raise an exception with severity warning
#[macro_export]
macro_rules! coalyxw {
    ($id: expr) => {
        CoalyException::new($id, Severity::Warning)
    };
    ($id: expr $(,$arg: expr)+) => {
        CoalyException::with_args($id, Severity::Warning, &[$($arg),+])
    };
}

/// Exception IDs with severity Error

// General errors
pub const E_FILE_NOT_FOUND: &str = "E-FileNotFound";
pub const E_FILE_READ_ERR: &str = "E-FileReadError";
pub const E_FILE_WRITE_ERR: &str = "E-FileWriteError";
pub const E_FILE_CRE_ERR: &str = "E-FileCreationError";
pub const E_INTERNAL_INV_TEMPLATE: &str = "E-Int-InvalidResourceTemplate";
pub const E_INTERNAL_NOT_YET_IMPLEMENTED: &str = "E-Int-NotYetImplemented";
pub const E_INTERNAL_EVENT_FAILED: &str = "E-Int-EventFailed";
pub const E_INTERNAL_EVENTS_FAILED: &str = "E-Int-EventsFailed";
pub const E_INVALID_URL: &str = "E-Net-InvalidUrl";
pub const E_SOCKET_CRE_ERR: &str = "E-Net-SocketCreationError";
pub const E_SOCKET_READ_ERR: &str = "E-Net-SocketReadError";
pub const E_SOCKET_WRITE_ERR: &str = "E-Net-SocketWriteError";
pub const E_DESER_ERR: &str = "E-DeserializationError";
pub const E_ACCESS_DENIED_BY_SRV: &str = "E-AccessDeniedByServer";
pub const E_CONNECT_PROT_ERROR: &str = "E-ConnectProtocolError";
pub const E_MSG_TOO_SHORT: &str = "E-MessageTooShort";
pub const E_MSG_SIZE_MISMATCH: &str = "E-MessageSizeMismatch";
pub const E_INVALID_ADDR_PATTERN: &str = "E-Net-InvalidAddressPattern";
pub const E_IP4_OCTET_TOO_LARGE: &str = "E-Net-IP4OctetTooLarge";
pub const E_IP_PORT_TOO_LARGE: &str = "E-Net-IPPortTooLarge";
pub const E_ALREADY_CONNECTED: &str = "E-Net-AlreadyConnected";

// TOML scanner related errors
pub const E_CFG_TOML_2DIGIT_DAY_REQUIRED: &str = "E-Cfg-Toml-TwoDigitDayRequired";
pub const E_CFG_TOML_2DIGIT_HOUR_REQUIRED: &str = "E-Cfg-Toml-TwoDigitHourRequired";
pub const E_CFG_TOML_2DIGIT_MONTH_REQUIRED: &str = "E-Cfg-Toml-TwoDigitMonthRequired";
pub const E_CFG_TOML_4DIGIT_YEAR_REQUIRED: &str = "E-Cfg-Toml-FourDigitYearRequired";
pub const E_CFG_TOML_DIGIT_DELIM_NOT_EMBEDDED: &str = "E-Cfg-Toml-DigitDelimiterNotEmbedded";
pub const E_CFG_TOML_DIGIT_EXPECTED: &str = "E-Cfg-Toml-DigitExpected";
pub const E_CFG_TOML_EMPTY_FLOAT_FRACT: &str = "E-Cfg-Toml-EmptyFloatFract";
pub const E_CFG_TOML_INVALID_CHAR: &str = "E-Cfg-Toml-InvalidChar";
pub const E_CFG_TOML_INV_CTRL_CHAR: &str = "E-Cfg-Toml-InvalidControlChar";
pub const E_CFG_TOML_INV_DATE: &str = "E-Cfg-Toml-InvalidDate";
pub const E_CFG_TOML_INV_EOL_ESC: &str = "E-Cfg-Toml-InvalidLineEndingEscape";
pub const E_CFG_TOML_INV_ESC_CHAR: &str = "E-Cfg-Toml-InvalidEscapeChar";
pub const E_CFG_TOML_INV_FLOAT_EXP: &str = "E-Cfg-Toml-InvalidFloatExp";
pub const E_CFG_TOML_INV_KEY_START: &str = "E-Cfg-Toml-InvalidKeyStart";
pub const E_CFG_TOML_INV_NUMDT_CHAR: &str = "E-Cfg-Toml-InvalidNumDateTimeChar";
pub const E_CFG_TOML_INV_NUM_CHAR: &str = "E-Cfg-Toml-InvalidNumChar";
pub const E_CFG_TOML_INV_RADIX_PREFIX: &str = "E-Cfg-Toml-InvalidRadixPrefix";
pub const E_CFG_TOML_INV_TIME: &str = "E-Cfg-Toml-InvalidTime";
pub const E_CFG_TOML_INV_UNICODE_ESC_CHAR: &str = "E-Cfg-Toml-InvalidUnicodeEscapeChar";
pub const E_CFG_TOML_INV_UNICODE_ESC_SEQ: &str = "E-Cfg-Toml-InvalidUnicodeEscapeSeq";
pub const E_CFG_TOML_INV_VALUE: &str = "E-Cfg-Toml-InvalidValue";
pub const E_CFG_TOML_INV_VALUE_START: &str = "E-Cfg-Toml-InvalidValueStart";
pub const E_CFG_TOML_LEADING_ZERO_NOT_ALLOWED: &str = "E-Cfg-Toml-LeadingZeroNotAllowed";
pub const E_CFG_TOML_SGL_LINE_TERM: &str = "E-Cfg-Toml-LineTermInSingleLineString";
pub const E_CFG_TOML_TOO_MANY_QUOTES: &str = "E-Cfg-Toml-TooManyQuotes";
pub const E_CFG_TOML_TZ_OR_MS_EXPECTED: &str = "E-Cfg-Toml-TimezoneOrMillisExpected";
pub const E_CFG_TOML_UNTERMINATED_STR: &str = "E-Cfg-Toml-UnterminatedString";

// TOML parser related errors
pub const E_CFG_TOML_CLOSING_BRACKET_EXPECTED: &str = "E-Cfg-Toml-ClosingBracketExpected";
pub const E_CFG_TOML_COMMA_EXPECTED: &str = "E-Cfg-Toml-CommaExpected";
pub const E_CFG_TOML_COMMA_OR_RBRACE_EXPECTED: &str = "E-Cfg-Toml-CommaOrRBraceExpected";
pub const E_CFG_TOML_DUP_SEP_TOKEN: &str = "E-Cfg-Toml-DuplicateSeparatorToken";
pub const E_CFG_TOML_EQUAL_EXPECTED: &str = "E-Cfg-Toml-EqualExpected";
pub const E_CFG_TOML_INV_ARRAY_TOKEN: &str = "E-Cfg-Toml-InvalidArrayToken";
pub const E_CFG_TOML_INV_KEY_TERM: &str = "E-Cfg-Toml-InvalidKeyTermination";
pub const E_CFG_TOML_KEY_ALREADY_IN_USE: &str = "E-Cfg-Toml-KeyAlreadyInUse";
pub const E_CFG_TOML_KEY_EXPECTED: &str = "E-Cfg-Toml-KeyExpected";
pub const E_CFG_TOML_KEY_OR_TABLE_EXPECTED: &str = "E-Cfg-Toml-KeyOrTableExpected";
pub const E_CFG_TOML_KEY_USED_FOR_ARRAY_OF_TABLES: &str = "E-Cfg-Toml-KeyUsedForArrayOfTables";
pub const E_CFG_TOML_KEY_USED_FOR_SIMPLE_VALUE: &str = "E-Cfg-Toml-KeyUsedForSimpleValue";
pub const E_CFG_TOML_KEY_USED_FOR_TABLE: &str = "E-Cfg-Toml-KeyUsedForTable";
pub const E_CFG_TOML_KEY_USED_FOR_VALUE_ARRAY: &str = "E-Cfg-Toml-KeyUsedForValueArray";
pub const E_CFG_TOML_LEADING_SEP: &str = "E-Cfg-Toml-LeadingSeparator";
pub const E_CFG_TOML_NO_LINE_BREAK_AFTER_HEADER: &str = "E-Cfg-Toml-NoLineBreakAfterHeader";
pub const E_CFG_TOML_NO_LINE_BREAK_AFTER_KVP: &str = "E-Cfg-Toml-NoLineBreakAfterKeyValuePair";
pub const E_CFG_TOML_NOT_A_TABLE: &str = "E-Cfg-Toml-NotATable";
pub const E_CFG_TOML_TABLE_EXISTS: &str = "E-Cfg-Toml-TableExists";
pub const E_CFG_TOML_TRAILING_DOT_IN_KEY: &str = "E-Cfg-Toml-TrailingDotInKey";
pub const E_CFG_TOML_TRAILING_SEP: &str = "E-Cfg-Toml-TrailingSeparator";
pub const E_CFG_TOML_TWO_DOTS_WITHIN_KEY: &str = "E-Cfg-Toml-TwoDotsWithinKey";
pub const E_CFG_TOML_UNEXPECTED_KEY_TOKEN: &str = "E-Cfg-Toml-UnexpectedKeyToken";
pub const E_CFG_TOML_UNSEP_ARRAY_ITEMS: &str = "E-Cfg-Toml-UnseparatedArrayItems";
pub const E_CFG_TOML_UNSEP_KEYPARTS: &str = "E-Cfg-Toml-UnseparatedKeyParts";
pub const E_CFG_TOML_UNTERM_ARRAY: &str = "E-Cfg-Toml-UnterminatedArray";
pub const E_CFG_TOML_UNTERM_INLINE_TABLE: &str = "E-Cfg-Toml-UnterminatedInlineTable";
pub const E_CFG_TOML_VALUE_EXPECTED: &str = "E-Cfg-Toml-ValueExpected";
pub const E_CFG_TOML_WS_BETWEEN_BRACKETS: &str = "E-Cfg-Toml-WhitespaceBetweenBrackets";
pub const E_CFG_INV_NW_PROTOCOL: &str = "E-Cfg-InvalidNetworkProtocol";
pub const E_CFG_NW_PROT_MISMATCH: &str = "E-Cfg-NetworkProtocolMismatch";

pub const E_CFG_TOML_PARSE_FAILED: &str = "E-Cfg-Toml-ParseFailed";
pub const E_CFG_FOUND_ISSUES: &str = "E-Cfg-FoundIssues";

// Rollover related errors
pub const E_ROVR_FAILED: &str = "E-Rovr-Failed";
pub const E_ROVR_OPEN_IN_FAILED: &str = "E-Rovr-OpenInputFileFailed";
pub const E_ROVR_OPEN_OUT_FAILED: &str = "E-Rovr-OpenOutputFileFailed";
pub const E_ROVR_WRITE_OUT_FAILED: &str = "E-Rovr-WriteOutFileFailed";
pub const W_ROVR_REMOVE_FAILED: &str = "W-Rovr-RemoveFileFailed";
pub const E_ROVR_RENAME_FAILED: &str = "E-Rovr-RenameFileFailed";
pub const W_ROVR_COMPRESS_FAILED: &str = "W-Rovr-CompressFailed";
pub const W_ROVR_GENERIC_FAILURE: &str = "W-Rovr-GenericFailure";
pub const W_ROVR_GENERIC_FILE_FAILURE: &str = "W-Rovr-GenericFileFailure";
pub const W_ROVR_USING_OLD: &str = "W-Rovr-UsingOldOutputFile";

// Server errors
pub const E_SRV_CFG_FILE_NOT_SPECIFIED: &str = "E-Srv-CfgFileNotSpecified";
pub const E_SRV_PROPS_MISSING: &str = "E-Srv-PropertiesMissing";
pub const E_SRV_INV_DATA_ADDR: &str = "E-Srv-InvalidDataAddress";
pub const E_SRV_INV_DATA_ADDR_IN_FILE: &str = "E-Srv-InvalidDataAddressInFile";
pub const E_SRV_ACCESS_DENIED: &str = "E-Srv-AccessDenied";
pub const E_SRV_CLIENT_LIMIT_EXCEEDED: &str = "E-Srv-ClientLimitExceeded";
pub const E_SRV_INTERNAL_ERROR: &str = "E-Srv-InternalError";
pub const E_SRV_ACC_CXN_FAILED: &str = "E-Srv-AcceptConnectionFailed";

// Coaly configuration related errors
pub const W_CFG_UNKNOWN_KEY: &str = "W-Cfg-UnknownKey";
pub const W_CFG_KEY_NOT_A_STRING: &str = "W-Cfg-KeyIsNotAString";
pub const W_CFG_KEY_NOT_A_TABLE: &str = "W-Cfg-KeyIsNotATable";
pub const W_CFG_KEY_NOT_AN_ARRAY: &str = "W-Cfg-KeyIsNotAnArray";
pub const W_CFG_NUM_REQUIRED: &str = "W-Cfg-NumberRequired";
pub const W_CFG_INV_LVL_ID_CHAR: &str = "W-Cfg-InvalidLevelIdChar";
pub const W_CFG_INV_LVL_NAME: &str = "W-Cfg-InvalidLevelName";
pub const W_CFG_EMPTY_LVL_NAME: &str = "W-Cfg-EmptyLevelName";
pub const W_CFG_INV_LVL_ATTR: &str = "W-Cfg-InvalidLevelAttribute";
pub const W_CFG_DUP_LVL_VALUE: &str = "W-Cfg-DuplicateLevelValue";
pub const W_CFG_DUP_LVL_VALUES: &str = "W-Cfg-DuplicateLevelValues";
pub const W_CFG_INV_LVL: &str = "W-Cfg-InvalidLevel";
pub const W_CFG_DUP_LVL: &str = "W-Cfg-DuplicateLevel";
pub const W_CFG_INV_LVL_REF: &str = "W-Cfg-InvalidLevelReference";
pub const W_CFG_INV_TRG: &str = "W-Cfg-InvalidTrigger";
pub const W_CFG_DUP_TRG: &str = "W-Cfg-DuplicateTrigger";
pub const W_CFG_INV_ROVR_FILE_SIZE: &str = "W-Cfg-InvalidRolloverFileSize";
pub const W_CFG_INV_ROLLOVER_ATTR: &str = "W-Cfg-InvalidRolloverAttribute";
pub const W_CFG_INV_ROVER_COND_PATTERN: &str = "W-Cfg-InvalidRolloverCondPattern";
pub const W_CFG_MISSING_ROVR_COND: &str = "W-Cfg-MissingRolloverCondition";
pub const W_CFG_INV_ROLLOVER_COND: &str = "W-Cfg-InvalidRolloverCondition";
pub const W_CFG_COMPR_NOT_SUPPORTED: &str = "W-Cfg-CompressionNotSupported";
pub const W_CFG_UNKNOWN_COMPR_ALGO: &str = "W-Cfg-UnknownCompressionAlgorithm";
pub const W_CFG_INV_COMPR_ALGO: &str = "W-Cfg-InvalidCompressionAlgorithm";
pub const W_CFG_INV_KEEP_COUNT: &str = "W-Cfg-InvalidKeepCount";
pub const W_CFG_MISSING_KEEP_COUNT: &str = "W-Cfg-MissingKeepCount";
pub const W_CFG_INV_BUFFER_ATTR: &str = "W-Cfg-InvalidBufferAttribute";
pub const W_CFG_MISSING_BUF_CONT_SIZE: &str = "W-Cfg-MissingBufferContentSize";
pub const W_CFG_MISSING_BUF_INDEX_SIZE: &str = "W-Cfg-MissingBufferIndexSize";
pub const W_CFG_INV_SIZE_SPEC: &str = "W-Cfg-InvalidSizeSpecification";
pub const W_CFG_INV_OR_MISSING_BUF_FLUSH_SPEC: &str = "W-Cfg-InvOrMissingBufferFlushSpecification";
pub const W_CFG_UNKNOWN_BUF_FLUSH_CONDITION: &str = "W-Cfg-UnknownBufferFlushCondition";
pub const W_CFG_INV_BUF_FLUSH_CONDITION: &str = "W-Cfg-InvalidBufferFlushCondition";
pub const W_CFG_DUP_BUF_FLUSH_CONDITION: &str = "W-Cfg-DuplicateBufferFlushCondition";
pub const W_CFG_RECLEN_EXCEEDS_SIZE: &str = "W-Cfg-RecLenExceedsSize";
pub const W_CFG_INV_NUM_IN_INTVL: &str = "W-Cfg-InvalidNumberInInterval";
pub const W_CFG_INV_UNIT_IN_INTVL: &str = "W-Cfg-InvalidUnitInInterval";
pub const W_CFG_INV_RECFMT_HDR: &str = "W-Cfg-InvalidRecordFormatHeader";
pub const W_CFG_INV_RECFMT_SPEC: &str = "W-Cfg-InvalidRecordFormatSpecification";
pub const W_CFG_INV_DFMT_ATTR: &str = "W-Cfg-InvalidDateTimeFormatAttribute";
pub const W_CFG_INV_DTFMT_SPEC: &str = "W-Cfg-InvalidDateTimeFormatSpecifier";
pub const W_CFG_OUTFMT_TRIGGERS_EMPTY: &str = "W-Cfg-OutputFormatTriggersEmpty";
pub const W_CFG_OUTFMT_LEVELS_EMPTY: &str = "W-Cfg-OutputFormatLevelsEmpty";
pub const W_CFG_INV_MODES_HDR: &str = "W-Cfg-InvalidModesHeader";
pub const W_CFG_INV_MODE_ATTR: &str = "W-Cfg-InvalidModeAttribute";
pub const W_CFG_INV_SCOPE: &str = "W-Cfg-InvalidScope";
pub const W_CFG_INV_MODE_SPEC: &str = "W-Cfg-InvalidModeSpecification";
pub const W_CFG_INV_MODE_TRIGGER: &str = "W-Cfg-InvalidModeTrigger";
pub const W_CFG_MISSING_MODE_NAME: &str = "W-Cfg-MissingModeName";
pub const W_CFG_MODE_VALUE_IGNORED: &str = "W-Cfg-ModeValueIgnored";
pub const W_CFG_MODE_SCOPE_IGNORED: &str = "W-Cfg-ModeScopeIgnored";
pub const W_CFG_INV_RESOURCES_HDR: &str = "W-Cfg-InvalidResourcesHeader";
pub const W_CFG_INV_RES_ATTR: &str = "W-Cfg-InvalidResourceAttribute";
pub const W_CFG_INV_RES_KIND: &str = "W-Cfg-InvalidResourceKind";
pub const W_CFG_INV_RES_SCOPE: &str = "W-Cfg-InvalidResourceScope";
pub const W_CFG_INV_RES_SPEC: &str = "W-Cfg-InvalidResourceSpecification";
pub const W_CFG_INV_RES_URL: &str = "W-Cfg-InvalidResourceUrl";
pub const W_CFG_RES_FN_MISSING: &str = "W-Cfg-ResourceFileNameMissing";
pub const W_CFG_FILE_SIZE_MISSING: &str = "W-Cfg-FileSizeMissing";
pub const W_CFG_RECFMT_INCOMPLETE: &str = "W-Cfg-RecordFormatIncomplete";
pub const W_CFG_ANCHOR_MIN_REQ: &str = "W-Cfg-AnchorMinuteRequired";
pub const W_CFG_ANCHOR_HHMM_REQ: &str = "W-Cfg-AnchorHourMinRequired";
pub const W_CFG_ANCHOR_DOWHM_REQ: &str = "W-Cfg-AnchorDowHourMinRequired";
pub const W_CFG_ANCHOR_DOMHM_REQ: &str = "W-Cfg-AnchorDomHourMinRequired";
pub const W_CFG_ANCHOR_NOT_ALLOWED: &str = "W-Cfg-AnchorNotAllowed";
pub const W_CFG_MEANINGLESS_RES_PAR: &str = "W-Cfg-MeaninglessResourcePar";
pub const W_CFG_MEANINGLESS_ROVR_ATTR: &str = "W-Cfg-MeaninglessRolloverAttr";
pub const W_CFG_ANONYMOUS_OBSERVER_IGNORED: &str = "W-Cfg-AnonymousObserverIgnored";
pub const W_CFG_INV_OBSERVER_NAME: &str = "W-Cfg-InvalidObserverName";
pub const W_CFG_INV_OBSERVER_VALUE: &str = "W-Cfg-InvalidObserverValue";
pub const W_CFG_INV_FALLBACK_PATH: &str = "W-Cfg-InvalidFallbackPath";
pub const W_CFG_INV_OUTPUT_PATH: &str = "W-Cfg-InvalidOutputPath";

lazy_static! {
    /// Singleton instance of hash table with language dependent resources
    pub static ref COALY_MSG_TABLE: HashMap<String, String> = {
        let loc = locale().to_lowercase();
        if loc.starts_with("de") {
            let res = include_str!("messages_de.txt");
            return parse_resource(res)
        }
        let res = include_str!("messages_en.txt");
        parse_resource(res)
    };
}

/// Returns localized message for given message ID
pub fn localized_message(msg_id: &str) -> String {
    COALY_MSG_TABLE.get(msg_id).unwrap_or(&msg_id.to_string()).clone()
}

/// Exception severities
#[derive (Clone, Copy, Debug, PartialEq)]
pub enum Severity {
    Error,
    Warning
}

/// Warning or error describing a problem found during runtime.
#[derive (Clone, Debug)]
pub struct CoalyException {
    // Exception ID
    // May contain `%s` placeholders which will be replaced with parameter values.
    id: &'static str,
    // Exception severity
    severity: Severity,
    // Argument values in case the message contains placeholders
    args: Option<Vec<String>>,
    // optional root cause
    cause: Option<Box<CoalyException>>
}
impl CoalyException {
    /// Creates an exception without arguments.
    /// 
    /// # Arguments
    /// * `id' - the exception ID
    /// * `severity' - the exception severity
    #[inline]
    pub fn new (id: &'static str, severity: Severity) -> CoalyException {
        CoalyException { id, severity, args: None, cause: None }
    }

    /// Creates an exception with an arbitrary number of arguments.
    /// 
    /// # Arguments
    /// * `id' - the exception ID
    /// * `severity' - the exception severity
    /// * `args' - the arguments
    pub fn with_args (id: &'static str, severity: Severity, args: &[String]) -> CoalyException {
        let mut v = Vec::<String>::new();
        v.extend(args.iter().map(|e| { (*e).to_string() }));
        CoalyException { id, severity, args: Some(v), cause: None }
    }

    /// Sets the root cause for this exception.
    /// 
    /// # Arguments
    /// * `cause' - the exception describing the root cause
    #[inline]
    pub fn set_cause(&mut self, cause: CoalyException) { self.cause = Some(Box::new(cause)); }

    /// Returns the exception ID.
    /// Severity prefix E_ for errors, W_ for warnings.
    /// Prefix is followed by a component indicator, if component specific.
    /// ID serves as a key in the mapping table to language dependent text.
    #[inline]
    pub fn id(&self) -> &'static str { self.id }

    /// Returns the exception severity.
    #[inline]
    pub fn severity(&self) -> Severity { self.severity }

    /// Returns the number of optional argument values contained in this message.
    #[inline]
    pub fn has_args(&self) -> bool {
        self.args.is_some()
    }

    /// Returns the number of optional argument values contained in this message.
    #[inline]
    pub fn arg_count(&self) -> usize {
        if let Some(p) = &self.args { return p.len() }
        0
    }

    /// Returns the optional argument values.
    #[inline]
    pub fn args(&self) -> &Option<Vec<String>> { &self.args }

    /// Replaces the current arguments with those specified.
    /// Used when a subordinate module issued an exception, and the arguments must be enhanced
    /// with informations not known by the subordinate module.
    pub fn replace_args(&mut self, new_args: &[String]) {
        self.args = Some(new_args.to_vec());
    }

    /// Returns the localized exception message.
    pub fn localized_message(&self) -> String { self.evaluate(&COALY_MSG_TABLE) }

    /// Localizes the exception and substitutes placeholder variables with their values.
    /// 
    /// # Arguments
    /// * `localized_texts' - the hash map with the language dependent resources
    pub fn evaluate(&self, localized_texts: &HashMap<String, String>) -> String {
        let mut res = String::with_capacity(160);
        let eid = &self.id.to_string();
        let msg = localized_texts.get(self.id).unwrap_or(eid);
        if self.args.is_none() && self.cause.is_none() { return msg.to_string() }
        let mut pars = self.args.as_ref().unwrap().clone();
        if let Some(inner_ex) = &self.cause { pars.push(inner_ex.evaluate(localized_texts)); }
        let par_count = pars.len();
        let mut expect_var = false;
        let mut par_index = 0;
        for c in msg.chars() {
            if expect_var {
                if c == 's' {
                    if par_index < par_count {
                        res.push_str(pars.get(par_index).unwrap());
                        par_index += 1;
                    }
                } else {
                    if c != '%' { res.push('%'); }
                    res.push(c);
                }
                expect_var = false;
                continue;
            }
            if c == '%' {
                expect_var = true;
                continue;
            }
            res.push(c);
        }
        res
    }
}

/// Logs the specified problems to an emergency resource.
pub fn log_problems(probs: &[CoalyException]) {
    // TODO try file/syslog first
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    for p in probs {
        let _ = handle.write_all(p.localized_message().as_bytes());
    }
}

#[cfg(unix)]
fn locale() -> String {
    #[cfg(test)]
    if let Ok(lang) = env::var(ENV_VAR_COALY_LANG) { return lang }
    if let Ok(lang) = env::var(ENV_VAR_LANG) { return lang }
    String::from(DEFAULT_LOCALE)
}

#[cfg(windows)]
fn locale() -> String {
    #[cfg(test)]
    if let Ok(lang) = env::var(ENV_VAR_COALY_LANG) { return lang }
    if let Ok(lang) = env::var(ENV_VAR_LANG) { return lang }
    String::from(DEFAULT_LOCALE)
}

/// Fills the language dependent resource table from file.
/// If no appropriate file exists, the English default resources are loaded instead.
///
/// # Arguments
/// * `lang_id` - the language ID
fn parse_resource(contents: &str) -> HashMap<String, String> {
    let mut t = HashMap::<String, String>::new();
    let ignore_pattern = Regex::new(r"^\s*#.*").unwrap();
    let def_pattern = Regex::new(r"^([\w\d_\-]+)\s+(.*)$").unwrap();
    for line in contents.split('\n') {
        let line = line.trim();
        if line.is_empty() || ignore_pattern.is_match(line) {
            continue;
        }
        if let Some(groups) = def_pattern.captures(line) {
            let id = groups.get(1).unwrap().as_str();
            let text = groups.get(2).unwrap().as_str();
            t.insert(id.to_string(), text.to_string());
        }
    }
    t
}

#[cfg(test)]
const ENV_VAR_COALY_LANG: &str = "COALY_LANG";

const ENV_VAR_LANG: &str = "LANG";
const DEFAULT_LOCALE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;

    const ID_P0: &str = "ExceptionWithoutArg";
    const ID_P1: &str = "ExceptionWithOneArg";
    const ID_P3: &str = "ExceptionWithThreeArgs";
    const TEXT_P0: &str = "Something went wrong.";
    const TEXT_P1: &str = "Line %s: Something went wrong.";
    const TEXT_P3: &str = "Line %s: Found %s, but expected %s.";
    const ARG_P1: &str = "123";
    const ARG_P3_1: &str = "99";
    const ARG_P3_2: &str = "=";
    const ARG_P3_3: &str = "String";
    const LOC_TEXT_P1: &str = "Line 123: Something went wrong.";
    const LOC_TEXT_P3: &str = "Line 99: Found =, but expected String.";
    const LOC_TEXT_P3_LINE_ONLY: &str = "Line 99: Found , but expected .";

    fn localized_texts() -> HashMap<String, String> {
        let mut map = HashMap::<String, String>::new();
        map.insert(ID_P0.to_string(), TEXT_P0.to_string());
        map.insert(ID_P1.to_string(), TEXT_P1.to_string());
        map.insert(ID_P3.to_string(), TEXT_P3.to_string());
        map
    }

    fn verify(x: &CoalyException, expected_id: &str, expected_severity: Severity,
              expected_args: &Option<&[&str]>, expected_text: &str) {
                assert_eq!(x.id(), expected_id);
                assert_eq!(x.severity(), expected_severity);
                match expected_args {
                    Some(exp_args) => {
                        match x.args() {
                            Some(actual_args) => {
                                assert_eq!(x.arg_count(), exp_args.len());
                                assert_eq!(x.arg_count(), actual_args.len());
                                for i in 0 .. exp_args.len()-1 {
                                    assert_eq!(exp_args[i], actual_args[i]);
                                }
                            },
                            None => { panic!("No args in exception found"); }
                        }
                    },
                    None => {
                        assert_eq!(x.arg_count(), 0);
                        assert!(x.args().is_none());
                    }
                }
                assert_eq!(x.evaluate(&localized_texts()), expected_text);
    }

    #[test]
    fn err_without_arg() {
        let x = coalyxe!(ID_P0);
        verify(&x, ID_P0, Severity::Error, &None, TEXT_P0);
    }

    #[test]
    fn err_with_one_arg() {
        let x = coalyxe!(ID_P1, ARG_P1.to_string());
        verify(&x, ID_P1, Severity::Error, &Some(&[ARG_P1]), LOC_TEXT_P1);
    }

    #[test]
    fn err_with_multiple_args() {
        let x = coalyxe!(ID_P3, ARG_P3_1.to_string(), ARG_P3_2.to_string(), ARG_P3_3.to_string());
        verify(&x, ID_P3, Severity::Error, &Some(&[ARG_P3_1,ARG_P3_2,ARG_P3_3]), LOC_TEXT_P3);
    }

    #[test]
    fn warning_without_arg() {
        let x = coalyxw!(ID_P0);
        verify(&x, ID_P0, Severity::Warning, &None, TEXT_P0);
    }

    #[test]
    fn warning_with_one_arg() {
        let x = coalyxw!(ID_P1, ARG_P1.to_string());
        verify(&x, ID_P1, Severity::Warning, &Some(&[ARG_P1]), LOC_TEXT_P1);
    }

    #[test]
    fn warning_with_multiple_args() {
        let x = coalyxw!(ID_P3, ARG_P3_1.to_string(), ARG_P3_2.to_string(), ARG_P3_3.to_string());
        verify(&x, ID_P3, Severity::Warning, &Some(&[ARG_P3_1,ARG_P3_2,ARG_P3_3]), LOC_TEXT_P3);
    }

    // Make sure, superfluous arguments are simply ignored
    #[test]
    fn too_many_args() {
        let x = coalyxe!(ID_P1, ARG_P1.to_string(), ARG_P1.to_string());
        verify(&x, ID_P1, Severity::Error, &Some(&[ARG_P1, ARG_P1]), LOC_TEXT_P1);
    }

    // Make sure, placeholders without supplied argument value are replaced with empty string
    #[test]
    fn too_few_args() {
        let x = coalyxw!(ID_P3, ARG_P3_1.to_string());
        verify(&x, ID_P3, Severity::Warning, &Some(&[ARG_P3_1]), LOC_TEXT_P3_LINE_ONLY);
    }
}