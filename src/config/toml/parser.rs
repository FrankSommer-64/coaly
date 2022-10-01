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

//! Coaly TOML parser.

use super::document::{TomlDocument, TomlKey, TomlValue, TomlValueItem};
use super::scanner::{TokenId, TokenValueType, TomlScanner};
use super::quoted;
use crate::errorhandling::*;

/// TOML parser.
/// Separates a TOML formatted string into a stream of tokens.
pub(super) struct TomlParser {
    // Lexical analyzer
    scanner: TomlScanner,
    // Key of latest defined table, if last defined bracket key hasn't been an array of tables
    latest_table_key: Option<TomlKey>,
    // Key of latest defined array of tables, if last defined bracket key was an array of tables
    latest_array_of_tables_key: Option<TomlKey>,
    // line number of most recent parsed key, used for error messages only
    key_line_nr: usize
}
impl TomlParser {
    /// Creates a parser for the given TOML string.
    /// 
    /// # Arguments
    /// * `data` - the string containing the input data to scan
    pub(super) fn new(data: &str) -> TomlParser {
        TomlParser {
            scanner: TomlScanner::new(data),
            latest_table_key: Some(TomlKey::root_key()),
            latest_array_of_tables_key: None,
            key_line_nr: 1
        }
    }

    /// Parses a TOML formatted string.
    /// The parsing process quits as soon as the first error is encountered.
    /// 
    /// # Return values
    /// A hash table with all TOML definitions parsed
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the string can't be parsed
    pub(super) fn parse(&mut self) -> Result<TomlDocument, CoalyException> {
        let mut document = TomlDocument::default();
        loop {
            let token = self.scanner.next_token(true)?;
            match token {
                TokenId::LeftBracket => {
                    let key = self.table_header(TokenId::RightBracket)?;
                    self.latest_table_key = Some(key.clone());
                    self.latest_array_of_tables_key = None;
                    if let Err(ex) = document.header_selected(&key, false) {
                        return Err(self.enhance_error(ex))
                    }
                },
                TokenId::DoubleLeftBracket => {
                    let key = self.table_header(TokenId::DoubleRightBracket)?;
                    self.latest_table_key = None;
                    self.latest_array_of_tables_key = Some(key.clone());
                    if let Err(ex) = document.header_selected(&key, true) {
                        return Err(self.enhance_error(ex))
                    }
                },
                TokenId::Key => {
                    let kvp = self.key_value_pair(true)?;
                    if let Err(ex) = document.insert(&kvp.key, kvp.value) {
                        // key already exists or at least one ancestor key is not a table
                       return Err(self.enhance_error(ex))
                    }
                },
                TokenId::LineBreak => (),
                TokenId::EndOfInput => break,
                _ => return Err(self.token_pos_error(E_CFG_TOML_KEY_OR_TABLE_EXPECTED, true))
            }
        }
        Ok(document)
    }

    /// Parses a key-value pair (<key> = <value> [<linebreak>]).
    /// 
    /// # Arguments
    /// * `lbreak_needed` - indicates whether a line break is needed after the key-value pair
    /// 
    /// # Return values
    /// The parsed TOML key-value pair
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the definition does not conform to
    /// the TOML specification or key resp. a specific value is invalid
    fn key_value_pair(&mut self, lbreak_needed: bool) -> Result<TomlKeyValuePair, CoalyException> {
        let key = self.key(TokenId::Equal)?;
        let value = self.value()?;
        if lbreak_needed {
            let token = self.scanner.next_token(true)?;
            if token != TokenId::LineBreak {
                return Err(self.key_line_error(E_CFG_TOML_NO_LINE_BREAK_AFTER_KVP, false))
            }
        }
        Ok(TomlKeyValuePair::new(key, value))
    }

    /// Parses the header of a table or an array of tables ([ <key> ] resp. [[ <key> ]]).
    /// 
    /// # Arguments
    /// * `term_token` - the token terminating the header
    /// 
    /// # Return values
    /// The parsed TOML key
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the definition does not conform to
    /// the TOML specification or the key is invalid
    fn table_header(&mut self, term_token: TokenId) -> Result<TomlKey, CoalyException> {
        let token = self.scanner.next_token(true)?;
        if token != TokenId::Key {
            if token == TokenId::LeftBracket {
                return Err(self.token_pos_error(E_CFG_TOML_WS_BETWEEN_BRACKETS, true))
            }
            return Err(self.token_pos_error(E_CFG_TOML_KEY_EXPECTED, true))
        }
        let key = self.key(term_token)?;
        let token = self.scanner.next_token(true)?;
        if token != TokenId::LineBreak && token != TokenId::EndOfInput {
            return Err(self.token_pos_error(E_CFG_TOML_NO_LINE_BREAK_AFTER_HEADER, true))
        }
        Ok(key)
    }

    /// Parses an inline table ({ <key> = <value>, ... }).
    /// The last consumed token must be a left brace.
    /// 
    /// # Return values
    /// The parsed TOML table
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the definition does not conform to
    /// the TOML specification or a specific value is invalid
    fn inline_table(&mut self) -> Result<TomlValueItem, CoalyException> {
        let (start_line, start_col) = self.scanner.token_position();
        let mut table_node = TomlValueItem::new_table(start_line, true);
        let mut last_token = TokenId::LineBreak;
        loop {
            let token = self.scanner.next_token(true)?;
            match token {
                TokenId::Key => {
                    if last_token == TokenId::Key {
                        return Err(self.token_pos_error(E_CFG_TOML_COMMA_EXPECTED, true))
                    }
                    let kvp = self.key_value_pair(false)?;
                    table_node.insert(&None, &kvp.key, kvp.value)?;
                    last_token = token;
                },
                TokenId::Comma => {
                    match last_token {
                        TokenId::Comma => {
                            return Err(self.token_pos_error(E_CFG_TOML_DUP_SEP_TOKEN, false))
                        },
                        TokenId::LineBreak => {
                            return Err(self.token_pos_error(E_CFG_TOML_LEADING_SEP, false))
                        },
                        _ => ()
                    }
                    last_token = token;
                },
                TokenId::RightBrace => {
                    if last_token == TokenId::Comma {
                        return Err(self.token_pos_error(E_CFG_TOML_TRAILING_SEP, false))
                    }
                    break
                },
                TokenId::LineBreak | TokenId::EndOfInput => {
                    return Err(self.parser_error(E_CFG_TOML_UNTERM_INLINE_TABLE,
                                                 start_line, Some(start_col), None, false))
                },
               _ => {
                    if last_token == TokenId::Comma || last_token == TokenId::LineBreak {
                        return Err(self.token_pos_error(E_CFG_TOML_KEY_EXPECTED, true))
                    }
                    return Err(self.token_pos_error(E_CFG_TOML_COMMA_OR_RBRACE_EXPECTED, true))
                }
            }
        }
        Ok(table_node)
    }

    /// Parses an array ([ <value>, ... ]).
    /// Last token consumed was left bracket.
    /// 
    /// # Return values
    /// The parsed TOML array
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the definition does not conform to
    /// the TOML specification or a specific value is invalid
    fn array(&mut self) -> Result<TomlValueItem, CoalyException> {
        let (start_line, start_col) = self.scanner.token_position();
        let mut array_data = TomlValueItem::new_array(start_line, false);
        let mut last_token = TokenId::LineBreak;
        loop {
            let token = self.scanner.next_token(false)?;
            match token {
                TokenId::Value => {
                    if last_token != TokenId::Comma && last_token != TokenId::LineBreak {
                        return Err(self.token_pos_error(E_CFG_TOML_UNSEP_ARRAY_ITEMS, false))
                    }
                    last_token = token;
                    let val = self.token_value()?;
                    array_data.push(val);
                },
                TokenId::Comma => {
                    match last_token {
                        TokenId::Comma => {
                            return Err(self.token_pos_error(E_CFG_TOML_DUP_SEP_TOKEN, false))
                        },
                        TokenId::LineBreak => {
                            return Err(self.token_pos_error(E_CFG_TOML_LEADING_SEP, false))
                        }
                        _ => ()
                    }
                    last_token = token;
                },
                TokenId::RightBracket => break,
                TokenId::LeftBracket => {
                    if last_token != TokenId::Comma && last_token != TokenId::LineBreak {
                        return Err(self.token_pos_error(E_CFG_TOML_UNSEP_ARRAY_ITEMS, false))
                    }
                    last_token = token;
                    let val = self.array()?;
                    array_data.push(val);
                },
                TokenId::LeftBrace => {
                    if last_token != TokenId::Comma && last_token != TokenId::LineBreak {
                        return Err(self.token_pos_error(E_CFG_TOML_UNSEP_ARRAY_ITEMS, false))
                    }
                    last_token = token;
                    let val = self.inline_table()?;
                    array_data.push(val);
                },
                TokenId::LineBreak => (),
                TokenId::EndOfInput => {
                    return Err(self.parser_error(E_CFG_TOML_UNTERM_ARRAY,
                                                 start_line, Some(start_col), None, false))
                },
                _ => return Err(self.token_pos_error(E_CFG_TOML_INV_ARRAY_TOKEN, true))
            }
        }
        Ok(array_data)
    }

    /// Parses a TOML value, i.e. the right hand side of a key-value pair.
    /// The last token consumed must be the equal sign.
    /// 
    /// # Return values
    /// The matching enum variant of the parsed TOML value
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the definition does not conform to
    /// the TOML specification or a specific value is invalid
    fn value(&mut self) -> Result<TomlValueItem, CoalyException> {
        let token = self.scanner.next_token(false)?;
        match token {
            TokenId::Value => self.token_value(),
            TokenId::LeftBracket => self.array(),
            TokenId::LeftBrace => self.inline_table(),
            TokenId::Dot => Err(self.token_pos_error(E_CFG_TOML_INV_VALUE_START, true)),
            _ => Err(self.token_pos_error(E_CFG_TOML_VALUE_EXPECTED, true))
        }
    }

    /// Parses a key, simple or dotted.
    /// The token with the initial (or only) part of the key must have already been consumed.
    /// 
    /// # Arguments
    /// * `sep_token` - the token that must follow the key
    /// 
    /// # Return values
    /// The parsed TOML key
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the key does not conform to
    /// the TOML specification
    fn key(&mut self, sep_token: TokenId) -> Result<TomlKey, CoalyException> {
        self.key_line_nr = self.scanner.token_position().0;
        let mut key_parts = vec!(self.scanner.token_value().to_string());
        let mut last_token = TokenId::Key;
        loop {
            let token = self.scanner.next_token(true)?;
            match token {
                TokenId::Dot => {
                    if last_token != TokenId::Key {
                        return Err(self.token_pos_error(E_CFG_TOML_TWO_DOTS_WITHIN_KEY, false))
                    }
                    last_token = token;
                },
                TokenId::Key => {
                    if last_token != TokenId::Dot {
                        return Err(self.token_pos_error(E_CFG_TOML_UNSEP_KEYPARTS, false))
                    }
                    last_token = token;
                    key_parts.push(self.scanner.token_value().to_string());
                },
                TokenId::Equal | TokenId::RightBracket | TokenId::DoubleRightBracket => {
                    if last_token != TokenId::Key {
                        return Err(self.token_pos_error(E_CFG_TOML_TRAILING_DOT_IN_KEY, false))
                    }
                    if token != sep_token {
                        return Err(self.parser_error(E_CFG_TOML_INV_KEY_TERM,
                                                     self.scanner.token_position().0, None,
                                                     Some(&sep_token.to_string()), true))
                    }
                    break
                },
                TokenId::LineBreak | TokenId::EndOfInput => {
                    if sep_token == TokenId::Equal {
                        return Err(self.key_line_error(E_CFG_TOML_EQUAL_EXPECTED, false))
                    }
                    return Err(self.key_line_error(E_CFG_TOML_CLOSING_BRACKET_EXPECTED, false))
                },
                _ => return Err(self.key_line_error(E_CFG_TOML_CLOSING_BRACKET_EXPECTED, false))
            }
        }
        Ok(TomlKey::from_quoted(key_parts, self.key_line_nr))
    }

    /// Determines the variant and specific value of a simple TOML value.
    /// 
    /// # Return values
    /// The matching enum variant of a parsed simple TOML value
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the specific value is invalid
    fn token_value(&mut self) -> Result<TomlValueItem, CoalyException> {
        let lnr = self.scanner.token_position().0;
        match self.scanner.token_value_type() {
            TokenValueType::String => {
                Ok(TomlValueItem::new(TomlValue::String(self.scanner.token_value().to_string()),lnr))
            },
            TokenValueType::Boolean => {
                if let Ok(val) = self.scanner.bool_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::Boolean(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            },
            TokenValueType::Integer => {
                if let Ok(val) = self.scanner.int_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::Integer(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            },
            TokenValueType::Float => {
                if let Ok(val) = self.scanner.f64_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::Float(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            },
            TokenValueType::OffsetDateTime => {
                if let Ok(val) = self.scanner.offset_datetime_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::OffsetDateTime(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            },
            TokenValueType::LocalDateTime => {
                if let Ok(val) = self.scanner.local_datetime_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::LocalDateTime(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            },
            TokenValueType::LocalDate => {
                if let Ok(val) = self.scanner.local_date_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::LocalDate(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            },
            TokenValueType::LocalTime => {
                if let Ok(val) = self.scanner.local_time_token_value() {
                    return Ok(TomlValueItem::new(TomlValue::LocalTime(val),lnr))
                }
                Err(self.current_pos_error(E_CFG_TOML_INV_VALUE, true))
            }
        }
    }

    /// Creates an information structure in case of a parser error.
    /// Always inserts the specified line and - if specified - column number.
    /// Then follow parameter and the actual token value, if desired.
    /// 
    /// # Arguments
    /// * `code` - the error code
    /// * `line_nr` - the line number in the input data, where the error occurred
    /// * `col_nr` - the optional column number within the error line
    /// * `param` - an optional parameter
    /// * `incl_token_val` - indicates whether to include the token value in the exception params
    /// 
    /// # Return values
    /// a structure containing error information
    fn parser_error(&mut self, code: &'static str,
                    line_nr: usize, col_nr: Option<usize>,
                    param: Option<&str>, incl_token_val: bool) -> CoalyException {
        let mut x_params = vec!(line_nr.to_string());
        if let Some(cnr) = col_nr { x_params.push(cnr.to_string()); }
        if let Some(p) = param { x_params.push(quoted(p)); }
        if incl_token_val { x_params.push(quoted(self.scanner.token_value())); }
        CoalyException::with_args(code, Severity::Error, &x_params)
    }

    /// Creates a structure in case of a scanner or parser error.
    /// Always inserts the line and column number of the last scanned character,
    /// then - if desired - the actual token value.
    /// 
    /// # Arguments
    /// * `code` - the error code
    /// * `incl_token_val` - indicates whether to include the token value in the exception params
    /// 
    /// # Return values
    /// a structure containing error information
    fn current_pos_error(&mut self, code: &'static str, incl_token_val: bool) -> CoalyException {
        let (line, col) = self.scanner.current_position();
        self.parser_error(code, line, Some(col), None, incl_token_val)
    }

    /// Creates an information structure in case of a parser error.
    /// Always inserts the line and column number of the current token,
    /// then - if desired - the actual token value.
    /// 
    /// # Arguments
    /// * `code` - the error code
    /// * `incl_token_val` - indicates whether to include the token value in the exception params
    /// 
    /// # Return values
    /// a structure containing error information
    fn token_pos_error(&mut self, code: &'static str, incl_token_val: bool) -> CoalyException {
        let (line, col) = self.scanner.token_position();
        self.parser_error(code, line, Some(col), None, incl_token_val)
    }

    /// Creates a structure in case of a scanner or parser error.
    /// Always inserts the line number of the last key as first argument,
    /// then - if desired - the actual token value.
    /// 
    /// # Arguments
    /// * `code` - the error code
    /// * `incl_token_val` - indicates whether to include the token value in the exception params
    /// 
    /// # Return values
    /// a structure containing error information
    fn key_line_error(&mut self, code: &'static str, incl_token_val: bool) -> CoalyException {
        self.parser_error(code, self.key_line_nr, None, None, incl_token_val)
    }

    /// Inserts line number into exceptions from TOML document manipulations, since
    /// a document has no relation to the TOML source.
    /// 
    /// # Arguments
    /// * `ex` - the exception to enhance
    /// 
    /// # Return values
    /// the exception with TOML source file name and line number prepended to the original
    /// exception arguments
    fn enhance_error(&self, mut ex: CoalyException) -> CoalyException {
        let mut ex_args = vec!(self.key_line_nr.to_string());
        if ex.has_args() { ex_args.extend_from_slice(ex.args().as_ref().unwrap()); }
        ex.replace_args(&ex_args);
        ex
    }
}

/// TOML key-value pair.
/// Key-value pairs are the central building block of TOML, key = value.
struct TomlKeyValuePair {
    key: TomlKey,
    value: TomlValueItem
}
impl TomlKeyValuePair {
    /// Creates a key-value pair.
    /// 
    /// # Arguments
    /// * `key` - the TOML key
    /// * `value` - the TOML value
    fn new(key: TomlKey, value: TomlValueItem) -> TomlKeyValuePair {
        TomlKeyValuePair {key, value}
    }
}