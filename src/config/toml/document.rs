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

//! Coaly TOML document.

use std::collections::BTreeMap;
use std::collections::btree_map::Iter;
use crate::errorhandling::*;
use crate::coalyxe;
use super::*;

/// TOML document.
/// A document represents all definitions made in a TOML formatted string or file, structured in a
/// suitable form for processing by software.
#[derive(Debug)]
pub struct TomlDocument {
    // root structure is a TOML table
    root: TomlValueItem,
    // key of currently selected table or array of tables
    selection: Option<TomlKey>
}
impl TomlDocument {
    /// Returns all items in the document's root table.
    pub fn root_items(&self) -> Iter<String, TomlValueItem> {
        self.root_table().unwrap().iter()
    }

    /// Selects a certain item within the document for subsequent insertion of key-value-pairs.
    /// The item has type table and the item and all eventual parents are created, if they don't
    /// exist.
    /// Called by the parser, when a table header has been detected.
    /// 
    /// # Arguments
    /// * `key` - the key within the (double) brackets of the header in the TOML formatted input
    /// * `double_brackets` - indicates whether the header denotes an array of tables (true)
    ///                       or a table (false)
    /// 
    /// # Return values
    /// always **true**
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the item for at least one part
    /// of the key's prefix exists with an unsuitable type or the item for the key's main part
    /// exists and at least one sub-item has not type table.
    pub(super) fn header_selected(&mut self, key: &TomlKey,
                                  double_brackets: bool) -> Result<bool, CoalyException> {
        // walk through the key's prefix parts, any missing items are created
        // with type mutable table
        let item = mk_prefix_items(&mut self.root, &key.prefix(), true)?;
        // process the key's main part:
        // if it exists, it must have type mutable table or mutable array
        // if it doesn't exist, it is created with type immutable table (for tables) or with
        // mutable array (array of tables)
        if double_brackets { mk_array_item(item, key)?; } else { mk_table_item(item, key)?; }
        // save item for subsequent insertions of key-value-pairs
        self.selection = Some(key.clone());
        Ok(true)
    }

    /// Inserts a TOML value into a table in the document.
    /// 
    /// # Arguments
    /// * `key` - the key used on the left hand side of the key-value-pair
    /// * `value` - the value specified on the right hand side of the key-value-pair
    /// 
    /// # Return values
    /// * always **true**
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the insertion fails
    pub(super) fn insert(&mut self, key: &TomlKey,
                         value: TomlValueItem) -> Result<bool, CoalyException> {
        self.root.insert(&self.selection, key, value)
    }

    /// Returns the document's root table.
    /// Return value will always be Some, hence using unwrap without check is safe.
    fn root_table(&self) -> Option<&TomlTable> {
        match self.root.value() {
            TomlValue::Table(r) => Some(r),
            _ => None
        }
    }

    /// Converts the document to a JSON formatted string.
    #[cfg(test)]
    pub fn to_json(&self) -> String {
        let mut json_buffer = String::with_capacity(8192);
        self.root.to_json(&mut json_buffer, 0);
        json_buffer
    }
}
impl Default for TomlDocument {
    fn default() -> Self {
        Self { root: TomlValueItem::new_table(1, true), selection: None }
    }
}

/// Wrapper structure for TOML values within a TOML document.
/// The pure value is enhanced with an indicator flag allowing distinction between normal and
/// inline tables as well as between value arrays and arrays of values. 
#[derive (Clone, Debug, PartialEq)]
pub struct TomlValueItem {
    // the contained value
    value: TomlValue,
    // the line number in the TOML source file
    line_nr: usize,
    // indicator, whether the item can be referenced to insert leaf values
    mutable_flag: bool
}
impl TomlValueItem {
    /// Creates a value item for the specified TOML value.
    /// To be used for all value types other than array and table, since the mutable flag cannot be
    /// set through this constructor.
    /// 
    /// # Arguments
    /// * `value` - the TOML value
    /// * `line_nr` - the line number in the TOML source file
    #[inline]
    pub fn new(value: TomlValue, line_nr: usize) -> TomlValueItem {
        TomlValueItem { value, line_nr, mutable_flag: false }
    }

    /// Creates a value item for an empty TOML table.
    /// Tables are created during key processing, either within a table/array of tables header or
    /// the left hand side of a key-value pair.
    /// 
    /// # Arguments
    /// * `line_nr` - the line number in the TOML source file
    /// * `mutable_flag` - indicates whether the value should be marked as mutable.
    ///                    Use **true** for prefix key parts, **false** for the main key part
    /// 
    /// # Examples
    /// - Table header [a.b.c]: mark prefix key parts (a and b) as mutable,
    ///   main key part (c) as not mutable
    /// - Array of tables header [[a.b.c]]: mark a and b as mutable, c is not a table
    /// - a.b = true: mark a as mutable, b is not a table
    /// - a.b = {1,2,3}: mark a as mutable, b as not mutable
    #[inline]
    pub fn new_table(line_nr: usize, mutable_flag: bool) -> TomlValueItem {
        TomlValueItem { value: TomlValue::Table(TomlTable::new()), line_nr, mutable_flag }
    }

    /// Creates a value item for an empty TOML array.
    /// Arrays are created either for an array of tables or a value array.
    /// 
    /// # Arguments
    /// * `line_nr` - the line number in the TOML source file
    /// * `mutable_flag` - indicates whether the value should be marked as mutable.
    ///                    Use **true** for an array of tables, **false** for value arrays
    /// 
    /// # Examples
    /// - Array of tables header [[a.b.c]]: mark c as mutable, a and b are not arrays
    /// - a.b = [1,2,3]: mark b as not mutable, a is not an array
    #[inline]
    pub fn new_array(line_nr: usize, mutable_flag: bool) -> TomlValueItem {
        TomlValueItem { value: TomlValue::Array(TomlArray::new()), line_nr, mutable_flag }
    }

    /// Returns the TOML value of this item.
    /// 
    /// # Return values
    /// a reference to the TOML valueof this item
    #[inline]
    pub fn value(&self) -> &TomlValue {
        &self.value
    }

    /// Returns the TOML value of this item.
    /// 
    /// # Return values
    /// a mutable reference to the TOML valueof this item
    #[inline]
    pub fn value_mut(&mut self) -> &mut TomlValue {
        &mut self.value
    }

    /// Returns the line number in the source file, where this TOML value is specified.
    /// Needed for error messages, hence return type is String.
    #[inline]
    pub fn line_nr(&self) -> String { self.line_nr.to_string() }

    /// Indicates whether this item has been explicitly referenced.
    /// Relies on the mutable flag for all TOML types except for array of tables, where the
    /// mutable flag of the last contained Table element is relevant.
    fn is_in_use(&self) -> bool {
        if let TomlValue::Array(ref a) = self.value {
            if ! self.mutable_flag { return true }
            let last_arr_item = a.last().unwrap();
            if let TomlValue::Table(_) = last_arr_item.value() {
                return ! last_arr_item.mutable_flag
            }
        }
        ! self.mutable_flag
    }

    /// Indicates whether this item can be modified or not.
    /// 
    /// # Return values
    /// **true** if it contains an array of tables or a normal table not having been referenced
    /// by the main part of a key, **false** if this item contains a simple value,
    /// an inline table or an array value
    #[inline]
    pub fn is_mutable(&self) -> bool {
        self.mutable_flag
    }

    /// Indicates whether this item is an array of tables.
    /// 
    /// # Return values
    /// **true** if the item is an array of tables; otherwise **false**
    pub fn is_array_of_tables(&self) -> bool {
        if let TomlValue::Array(_) = self.value { return self.is_mutable() }
        false
    }

    /// Mark this item as **not** mutable.
    /// To be called if an item containing a TOML table is referenced by the main part of a key.
    pub fn explicitly_referenced(&mut self) {
        match self.value {
            TomlValue::Table(_) => self.mutable_flag = false,
            TomlValue::Array(ref mut a) => {
                let last_arr_item = a.last_mut().unwrap();
                if let TomlValue::Table(_) = last_arr_item.value() {
                    last_arr_item.mutable_flag = true;
                }
            },
            _ => ()
        }
    }

    /// Returns the key and value items of all direct children.
    /// 
    /// # Return values
    /// an iterator over all child keys and value items; **None** if this item has not type table
    pub fn child_items(&self) -> Option<Iter<String, TomlValueItem>> {
        match &self.value {
            TomlValue::Table(t) => Some(t.iter()),
            _ => None
        }
    }

    /// Returns the value items of all direct children.
    /// 
    /// # Return values
    /// an iterator over all child value items; **None** if this item has not type array
    pub fn child_values(&self) -> Option<std::slice::Iter<TomlValueItem>> {
        match &self.value {
            TomlValue::Array(a) => Some(a.iter()),
            _ => None
        }
    }

    /// Adds a value item to an array.
    /// If this item is not an array, a call to this function has no effect.
    /// 
    /// # Arguments
    /// * `value_item` - the value item to add
    pub fn push(&mut self, value_item: TomlValueItem) {
        if let TomlValue::Array(ref mut a) = self.value { a.push(value_item); }
    }

    /// Inserts a TOML value into a table or array of tables.
    /// If an item for one or more parts of the key already exists, it must be marked as mutable,
    /// otherwise 
    /// 
    /// # Arguments
    /// * `parent` - the key of the current TOML table; **None** if the value
    ///              shall be inserted directly under the root table
    /// * `key` - the key used on the left hand side of the key-value-pair
    /// * `value_item` - the value specified on the right hand side of the key-value-pair
    /// 
    /// # Return values
    /// always **true**
    /// 
    /// # Errors
    /// Returns a structure containing error information, if the insertion fails
    pub fn insert(&mut self, parent: &Option<TomlKey>,
                  key: &TomlKey, value_item: TomlValueItem) -> Result<bool, CoalyException> {
        // walk through the key's prefix parts, any missing items are created
        // with type mutable table
        let mut prefix_items = Vec::<&str>::new();
        if let Some(parent_items) = parent {
            prefix_items.extend_from_slice(&parent_items.all_parts());
        }
        prefix_items.extend_from_slice(&key.prefix());
        let parent_table = mk_prefix_items(self, &prefix_items, false)?;
        if parent_table.insert(key.main_part().to_string(), value_item).is_some() {
            return Err(coalyxe!(E_CFG_TOML_KEY_ALREADY_IN_USE, quoted(key.main_part())))
        }
        Ok(true)
    }

    /// Converts the item to a JSON formatted string.
    /// 
    /// # Arguments
    /// * `buffer` - the string buffer receiving this item's JSON formatted data
    /// * `indent` - the number of spaces to prepend before each output line
    #[cfg(test)]
    pub fn to_json(&self, buffer: &mut String, indent: usize) {
        self.value.to_json(buffer, indent);
    }
}

/// Type for TOML values of kind Table.
/// BTreeMap is used, since it's using sorted entries and hence making test verification easier
pub type TomlTable = BTreeMap<String, TomlValueItem>;

/// Type for TOML values of kind Array
pub type TomlArray = Vec<TomlValueItem>;

/// Enumeration for all kinds of TOML values.
/// TOML values are on the right hand side of a key-value pair.
#[derive (Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum TomlValue {
    String (String),
    Boolean (bool),
    Integer (isize),
    Float (f64),
    OffsetDateTime (DateTime<FixedOffset>),
    LocalDateTime (NaiveDateTime),
    LocalDate (NaiveDate),
    LocalTime (NaiveTime),
    Table (TomlTable),
    Array (TomlArray)
}
impl TomlValue {
    /// Returns the string value, if the variant is a simple value.
    pub fn as_str(&self) -> Option<String> {
        match *self {
            TomlValue::String(ref val) => Some(val.to_string()),
            TomlValue::Boolean(val) => Some(val.to_string()),
            TomlValue::Integer(val) => Some(val.to_string()),
            TomlValue::Float(val) => Some(val.to_string()),
            TomlValue::OffsetDateTime(val) => Some(val.to_string()),
            TomlValue::LocalDateTime(val) => Some(val.to_string()),
            TomlValue::LocalDate(val) => Some(val.to_string()),
            TomlValue::LocalTime(val) => Some(val.to_string()),
            _ => None
        }
    }

    /// Returns the boolean value, if the variant is a boolean value.
    pub fn _as_bool(&self) -> Option<bool> {
        match *self { TomlValue::Boolean(val) => Some(val), _ => None }
    }

    /// Returns the integer value, if the variant is a number or boolean value.
    /// For float and boolean values, default rust conversion is used.
    pub fn as_integer(&self) -> Option<isize> {
        match *self {
            TomlValue::Integer(val) => Some(val),
            TomlValue::Float(val) => Some(val as isize),
            _ => None
        }
    }

    /// Returns the float value, if the variant is a number value.
    /// For integer values, default rust conversion is used.
    pub fn _as_float(&self) -> Option<f64> {
        match *self {
            TomlValue::Float(val) => Some(val),
            TomlValue::Integer(val) => Some(val as f64),
            _ => None
        }
    }

    /// Returns the date-time value, if the variant is a date-time value including timezone offset.
    pub fn _as_offset_datetime(&self) -> Option<&DateTime<FixedOffset>> {
        match *self { TomlValue::OffsetDateTime(ref val) => Some(val), _ => None }
    }

    /// Returns the date-time value, if the variant is a local date-time value.
    pub fn _as_local_datetime(&self) -> Option<&NaiveDateTime> {
        match *self { TomlValue::LocalDateTime(ref val) => Some(val), _ => None }
    }

    /// Returns the date value, if the variant is a local date value.
    pub fn _as_local_date(&self) -> Option<&NaiveDate> {
        match *self { TomlValue::LocalDate(ref val) => Some(val), _ => None }
    }

    /// Returns the time value, if the variant is a local time value.
    pub fn _as_local_time(&self) -> Option<&NaiveTime> {
        match *self { TomlValue::LocalTime(ref val) => Some(val), _ => None }
    }

    /// Returns the table value, if the variant is a table value.
    pub fn _as_table(&self) -> Option<&TomlTable> {
        match *self { TomlValue::Table(ref val) => Some(val), _ => None }
    }

    /// Returns the array value, if the variant is an array value.
    pub fn _as_array(&self) -> Option<&[TomlValueItem]> {
        match *self { TomlValue::Array(ref val) => Some(&**val), _ => None }
    }

    /// Converts the document to a JSON formatted string.
    /// 
    /// # Arguments
    /// * `buffer` - the string buffer receiving this item's JSON formatted data
    /// * `indent` - the number of spaces to prepend before each output line
    #[cfg(test)]
    pub fn to_json(&self, buffer: &mut String, indent: usize) {
        let indent_str = " ".repeat(indent);
        match *self {
            TomlValue::Array(ref a) => {
                let item_count = a.len();
                buffer.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    buffer.push_str(&indent_str);
                    buffer.push_str("  ");
                    v.to_json(buffer, indent + 2);
                    if i < item_count-1 { buffer.push(','); }
                    buffer.push('\n');
                }
                buffer.push_str(&indent_str);
                buffer.push(']');
            },
            TomlValue::Table(ref t) => {
                let item_count = t.len();
                buffer.push_str("{\n");
                for (i, (k, v)) in t.iter().enumerate() {
                    buffer.push_str(&indent_str);
                    buffer.push_str("  \"");
                    buffer.push_str(k);
                    buffer.push_str("\" : ");
                    v.to_json(buffer, indent + 2);
                    if i < item_count-1 { buffer.push(','); }
                    buffer.push('\n');
                }
                buffer.push_str(&indent_str);
                buffer.push('}');
            },
            TomlValue::String(ref s) => {
                buffer.push('"');
                buffer.push_str(s);
                buffer.push('"');
            },
            TomlValue::Boolean(val) => buffer.push_str(&val.to_string()),
            TomlValue::Integer(val) => buffer.push_str(&val.to_string()),
            TomlValue::Float(val) => buffer.push_str(&val.to_string()),
            TomlValue::OffsetDateTime(val) => buffer.push_str(&val.to_string()),
            TomlValue::LocalDateTime(val) => buffer.push_str(&val.to_string()),
            TomlValue::LocalDate(val) => buffer.push_str(&val.to_string()),
            TomlValue::LocalTime(val) => buffer.push_str(&val.to_string())
        }
    }
}

/// TOML key.
/// Keys are on the left hand side of a key-value pair definition, the central building block of
/// TOML.
/// Simple keys are in TOML terms bare or quoted keys.
/// Dotted keys are a sequence of bare or quoted keys, joined with a dot.
#[derive (Clone, Debug, Eq, PartialEq, Hash)]
pub struct TomlKey {
    // all parts of the key, separated by dots. Guaranteed minimum size is 1 element except for
    // the artificial root key.
    parts: Vec<String>,
    // the line number in the TOML source file
    line_nr: usize,
}
impl TomlKey {
    /// Creates a TOML key for the document root.
    pub fn root_key() -> TomlKey {
        TomlKey { parts: Vec::new(), line_nr: 1 }
    }

    /// Creates a TOML key.
    /// This function works for all types of TOML keys.
    /// 
    /// # Arguments
    /// * `quoted_key` - the quoted key as a vector with all its parts
    /// * `line_nr` - the line number in the TOML source file
    pub fn from_quoted(quoted_key: Vec<String>, line_nr: usize) -> TomlKey {
        TomlKey { parts: quoted_key, line_nr }
    }

    /// Returns the line number in the source file, where this TOML value is specified.
    #[inline]
    pub fn line_nr(&self) -> usize { self.line_nr }

    /// Returns the key's main part.
    /// Corresponds to entire string in case of simple keys and to the last part
    /// (i.e. the part after the rightmost dot) for dotted keys
    fn main_part(&self) -> &str {
        &self.parts[self.parts.len() - 1]
    }

    /// Returns the key's prefix part(s).
    /// Corresponds to empty slice in case of simple keys and to the part to the left of the
    /// rightmost dot for dotted keys
    fn prefix(&self) -> Vec<&str> {
        self.parts[0 .. self.parts.len() - 1].iter().map(|p| p.as_ref()).collect::<Vec<&str>>()
    }

    /// Returns all key parts, i.e. all items separated by dots.
    fn all_parts(&self) -> Vec<&str> {
        self.parts.iter().map(|p| p.as_ref()).collect::<Vec<&str>>()
    }

    /// Returns all key parts, i.e. all items separated by dots.
    pub fn full_name(&self) -> String {
        self.ancestor_name(self.parts.len())
    }

    /// Returns the name of the key part at the specified level, beginning with the root.
    /// 
    /// # Arguments
    /// * `level` - the ancestor level, root has number 1, 0 will return an empty string.
    fn ancestor_name(&self, level: usize) -> String {
        let mut name = String::with_capacity(64);
        let limit = if level <= self.parts.len() { level } else { self.parts.len() };
        for (i, p) in self.parts[0 .. limit].iter().enumerate() {
            if p.is_empty() || p.contains('.') {
                name.push('"');
                name.push_str(p);
                name.push('"');
            } else {
                name.push_str(p);
            }
            if i < limit - 1 { name.push('.'); }
        }
        name
    }
}
impl fmt::Display for TomlKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

/// Returns a portion of the given key prefix as a dotted string.
/// 
/// # Arguments
/// * `key_prefix` - the key's prefix parts
/// * `up_to_part` - the limit, how many ancestors shall be considered, 1 for root ancestor,
///                  0 returns empty string
/// 
/// # Return values
/// the ancestor parts as dotted string
fn key_prefix_fragment(key_prefix: &[&str], up_to_part: usize) -> String {
    let mut frag = String::with_capacity(64);
    let prefix_count = key_prefix.len();
    if prefix_count == 0 { return frag }
    let limit = std::cmp::min(prefix_count - 1, up_to_part);
    frag.push_str(key_prefix[0]);
    for part in key_prefix.iter().take(limit + 1).skip(1) {
        frag.push('.');
        frag.push_str(*part);
    }
    frag
}

/// Selects or creates all prefix items for the given prefix names under the specified parent item.
/// 
/// # Arguments
/// * `item` - the parent item
/// * `prefix_names` - the names of all prefixes
/// * `for_header` - indicates whether to make the prefixes for a header (true)
///                   or a key-value-pair (false)
/// 
/// # Return values
/// a mutable reference to the value underneath the last prefix, always of type table
/// 
/// # Errors
/// Returns a structure containing error information, if the item for at least one prefix exists
/// with an unsuitable type
fn mk_prefix_items<'a>(mut item: &'a mut TomlValueItem, prefix_names: &[&str],
                       for_header: bool) -> Result<&'a mut TomlTable, CoalyException> {
    // without prefixes there's nothing to do
    if prefix_names.is_empty() {
        if let TomlValue::Table(t) = item.value_mut() {
            return Ok(t)
        }
        return Err(coalyxe!(E_CFG_TOML_NOT_A_TABLE, String::from("")))
    }

    // process the key's prefix parts:
    // if any part exists, it must have type table or mutable array.
    // Missing items are created with type mutable table
    for (i, prefix_name) in prefix_names.iter().enumerate() {
        let item_is_mutable = item.is_mutable();
        match item.value_mut() {
            TomlValue::Table(ref mut t) => {
                if ! t.contains_key(*prefix_name) {
                    // table item for key prefix doesn't exist, create it as mutable
                    // line number not relevant, set it to 0
                    t.insert(prefix_name.to_string(), TomlValueItem::new_table(0, true));
                }
                item = t.get_mut(*prefix_name).unwrap();
            },
            TomlValue::Array(ref mut a) => {
                if ! item_is_mutable {
                    // item references a value array
                    return Err(coalyxe!(E_CFG_TOML_KEY_USED_FOR_VALUE_ARRAY,
                                      quoted(&key_prefix_fragment(prefix_names, i))))
                }
                // is an array of tables, use last item in the array
                let last_arr_item = a.last_mut().unwrap();
                match last_arr_item.value_mut() {
                    TomlValue::Table(at) => item = at.get_mut(*prefix_name).unwrap(),
                    _ => return Err(coalyxe!(E_CFG_TOML_NOT_A_TABLE,
                                           quoted(&key_prefix_fragment(prefix_names,
                                                                       prefix_names.len()))))
                }
            },
            _ => {
                return Err(coalyxe!(E_CFG_TOML_KEY_USED_FOR_SIMPLE_VALUE,
                                  quoted(&key_prefix_fragment(prefix_names, i-1))))
            }
        }
    }
    if ! for_header { item.explicitly_referenced(); }
    match item.value {
        TomlValue::Array(ref mut a) => {
            let last_arr_item = a.last_mut().unwrap();
            if let TomlValue::Table(ref mut t) = last_arr_item.value_mut() {
                return Ok(t)
            }
            Err(coalyxe!(E_CFG_TOML_NOT_A_TABLE,
                       quoted(&key_prefix_fragment(prefix_names, prefix_names.len()))))
        },
        TomlValue::Table(ref mut t) => {
            Ok(t)
        },
        _ => Err(coalyxe!(E_CFG_TOML_NOT_A_TABLE,
                        quoted(&key_prefix_fragment(prefix_names, prefix_names.len()))))
    }
}

/// Selects or creates an item of type table for the main part of the given key.
/// 
/// # Arguments
/// * `item` - the parent item
/// * `key` - the TOML key
/// 
/// # Return values
/// always **true**
/// 
/// # Errors
/// Returns a structure containing error information, if the item for key's main part exists
/// with an unsuitable type
fn mk_table_item(parent: &mut TomlTable, key: &TomlKey) -> Result<bool, CoalyException> {
    let main_key_name = key.main_part();
    if ! parent.contains_key(main_key_name) {
        let lnr = key.line_nr();
        parent.insert(main_key_name.to_string(), TomlValueItem::new_table(lnr, false));
        return Ok(true)
    }
    // main key part exists, it must be mutable
    let leaf_item = parent.get_mut(main_key_name).unwrap();
    let leaf_item_mutable = leaf_item.is_mutable();
    match leaf_item.value_mut() {
        TomlValue::Table(_) => {
            if leaf_item.is_in_use() {
                return Err(coalyxe!(E_CFG_TOML_KEY_ALREADY_IN_USE, quoted(&key.full_name())))
            }
            leaf_item.explicitly_referenced();
        },
        TomlValue::Array(_) => {
            let ecode = if leaf_item_mutable { E_CFG_TOML_KEY_USED_FOR_ARRAY_OF_TABLES }
                        else { E_CFG_TOML_KEY_USED_FOR_VALUE_ARRAY };
            return Err(coalyxe!(ecode, quoted(&key.full_name())))
        },
        _ => return Err(coalyxe!(E_CFG_TOML_KEY_USED_FOR_SIMPLE_VALUE,
                               quoted(&key.full_name())))
    }
    Ok(true)
}

/// Selects or creates an item of type array (of tables) for the main part of the given key.
/// 
/// # Arguments
/// * `item` - the parent item
/// * `key` - the TOML key
/// 
/// # Return values
/// always **true**
/// 
/// # Errors
/// Returns a structure containing error information, if the item for key's main part exists
/// with an unsuitable type
fn mk_array_item(parent: &mut TomlTable, key: &TomlKey) -> Result<bool, CoalyException> {
    let lnr = key.line_nr();
    let main_key_name = key.main_part();
    if ! parent.contains_key(main_key_name) {
        let mut array_val = TomlValueItem::new_array(lnr, true);
        array_val.push(TomlValueItem::new_table(lnr, false));
        parent.insert(main_key_name.to_string(), array_val);
        return Ok(true)
    }
    // main key part exists, it must be mutable
    let leaf_item = parent.get_mut(main_key_name).unwrap();
    let leaf_item_mutable = leaf_item.is_mutable();
    match leaf_item.value_mut() {
        TomlValue::Array(la) => {
            if ! leaf_item_mutable {
                return Err(coalyxe!(E_CFG_TOML_KEY_USED_FOR_VALUE_ARRAY, quoted(&key.full_name())))
            }
            la.push(TomlValueItem::new_table(lnr, false));
        },
        TomlValue::Table(_) => {
            return Err(coalyxe!(E_CFG_TOML_KEY_USED_FOR_TABLE, quoted(&key.full_name())))
        },
        _ => return Err(coalyxe!(E_CFG_TOML_KEY_USED_FOR_SIMPLE_VALUE, quoted(&key.full_name())))
    }
    Ok(true)
}
