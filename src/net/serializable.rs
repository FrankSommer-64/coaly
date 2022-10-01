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

//! Byte serialization for Coaly.

use core::fmt;
use std::collections::BTreeMap;
use crate::errorhandling::*;
use crate::coalyxe;

/// Trait with functions that must be supported by data sent over the network.
pub trait Serializable<'a> {
    /// Returns the size of the element in serialized form in bytes.
    fn serialized_size(&self) -> usize;

    /// Serializes the element into the specified buffer.
    /// 
    /// # Arguments
    /// * `buffer` - the buffer receiving the serialized element
    /// 
    /// # Return values
    /// the number of bytes written to the buffer
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize;

    /// Serializes the element into the specified buffer.
    /// 
    /// # Arguments
    /// * `buffer` - the buffer containing the serialized element
    /// 
    /// # Return values
    /// the deserialized element in case of success; otherwise Error
    fn deserialize_from(buffer: &'a [u8]) -> Result<Self, CoalyException>
    where Self: std::marker::Sized;
}

impl<'a> fmt::Debug for dyn Serializable<'a> {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl<'a> Serializable<'a> for u8 {
    fn serialized_size(&self) -> usize { 1 }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        buffer.push(*self);
        1
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        if buffer.is_empty() { return Err(coalyxe!(E_DESER_ERR, String::from("u8"))) }
        Ok(buffer[0])
    }
}

impl<'a> Serializable<'a> for u32 {
    fn serialized_size(&self) -> usize { 4 }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        buffer.push(((*self >> 24) & 255) as u8);
        buffer.push(((*self >> 16) & 255) as u8);
        buffer.push(((*self >> 8) & 255) as u8);
        buffer.push((*self & 255) as u8);
        4
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        if buffer.len() < 4 { return Err(coalyxe!(E_DESER_ERR, String::from("u32"))) }
        let mut res: u32 = 0;
        for byte_val in buffer.iter().take(4) {
            res <<= 8;
            res += *byte_val as u32;
        }
        Ok(res)
    }
}

impl<'a> Serializable<'a> for u64 {
    fn serialized_size(&self) -> usize { 8 }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        buffer.push(((*self >> 56) & 255) as u8);
        buffer.push(((*self >> 48) & 255) as u8);
        buffer.push(((*self >> 40) & 255) as u8);
        buffer.push(((*self >> 32) & 255) as u8);
        buffer.push(((*self >> 24) & 255) as u8);
        buffer.push(((*self >> 16) & 255) as u8);
        buffer.push(((*self >> 8) & 255) as u8);
        buffer.push((*self & 255) as u8);
        8
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        if buffer.len() < 8 { return Err(coalyxe!(E_DESER_ERR, String::from("u64"))) }
        let mut res: u64 = 0;
        for byte_val in buffer.iter().take(8) {
            res <<= 8;
            res += *byte_val as u64;
        }
        Ok(res)
    }
}

impl<'a> Serializable<'a> for i64 {
    fn serialized_size(&self) -> usize { 8 }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        buffer.push(((*self >> 56) & 255) as u8);
        buffer.push(((*self >> 48) & 255) as u8);
        buffer.push(((*self >> 40) & 255) as u8);
        buffer.push(((*self >> 32) & 255) as u8);
        buffer.push(((*self >> 24) & 255) as u8);
        buffer.push(((*self >> 16) & 255) as u8);
        buffer.push(((*self >> 8) & 255) as u8);
        buffer.push((*self & 255) as u8);
        8
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        if buffer.len() < 8 { return Err(coalyxe!(E_DESER_ERR, String::from("ui64"))) }
        let mut res: i64 = 0;
        for byte_val in buffer.iter().take(8) {
            res <<= 8;
            res += *byte_val as i64;
        }
        Ok(res)
    }
}

impl <'a> Serializable<'a> for String {
    fn serialized_size(&self) -> usize { self.len() + 8 }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let slen = self.len() as u64;
        slen.serialize_to(buffer);
        buffer.extend_from_slice(self.as_bytes());
        (slen + 8) as usize
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        if buffer.len() < 8 { return Err(coalyxe!(E_DESER_ERR, String::from("String"))) }
        let slen = u64::deserialize_from(buffer)? as usize;
        if buffer.len() < slen + 8 { return Err(coalyxe!(E_DESER_ERR, String::from("String"))) }
        let scont = &buffer[8..8+slen];
        let mut vcont = Vec::with_capacity(slen);
        vcont.extend_from_slice(scont);
        if let Ok(s) = String::from_utf8(vcont) { return Ok(s) }
        Err(coalyxe!(E_DESER_ERR, String::from("String")))
    }
}

impl<'a> Serializable<'a> for &'a str {
    fn serialized_size(&self) -> usize { self.len() + 8 }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let slen = self.len() as u64;
        slen.serialize_to(buffer);
        buffer.extend_from_slice(self.as_bytes());
        (slen + 8) as usize
    }
    fn deserialize_from(buffer: &'a[u8]) -> Result<Self, CoalyException> {
        if buffer.len() < 8 { return Err(coalyxe!(E_DESER_ERR, String::from("String"))) }
        let slen = u64::deserialize_from(buffer)? as usize;
        if buffer.len() < slen + 8 { return Err(coalyxe!(E_DESER_ERR, String::from("String"))) }
        let scont = &buffer[8..8+slen];
        let mut vcont = Vec::with_capacity(slen);
        vcont.extend_from_slice(scont);
        if let Ok(s) = std::str::from_utf8(scont) { return Ok(s) }
        Err(coalyxe!(E_DESER_ERR, String::from("String")))
    }
}

impl<'a, T> Serializable<'a> for Option<T> where T: Serializable<'a> {
    fn serialized_size(&self) -> usize {
        if let Some(v) = self { v.serialized_size() + 1 } else { 1 }
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        if let Some(v) = self {
            buffer.push(255);
            let contained_bytes = v.serialize_to(buffer);
            return contained_bytes + 1
        }
        buffer.push(0);
        1
    }
    fn deserialize_from(buffer: &'a[u8]) -> Result<Self, CoalyException> {
        if buffer.is_empty() { return Err(coalyxe!(E_DESER_ERR, String::from("Option"))) }
        if buffer[0] == 0 { return Ok(None) }
        let v = T::deserialize_from(&buffer[1..])?;
        Ok(Some(v))
    }
}

impl <'a, K, V> Serializable<'a> for BTreeMap<K,V>
where K: Serializable<'a> + std::cmp::Ord, V: Serializable<'a> {
    fn serialized_size(&self) -> usize {
        let mut sz = 8usize;
        for (key,val) in self.iter() {
            sz += (*key).serialized_size() + (*val).serialized_size();
        }
        sz
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let no_of_entries = self.len() as u64;
        let mut n = no_of_entries.serialize_to(buffer);
        for (key,val) in self.iter() {
            n += (*key).serialize_to(buffer) + (*val).serialize_to(buffer);
        }
        n
    }
    fn deserialize_from(buffer: &'a[u8]) -> Result<Self, CoalyException> {
        if buffer.len() < 8 { return Err(coalyxe!(E_DESER_ERR, String::from("BTReeMap"))) }
        let mut no_of_entries = u64::deserialize_from(buffer)? as usize;
        let mut offset = 8usize;
        let mut map = BTreeMap::<K,V>::new();
        while no_of_entries > 0 {
            let key = K::deserialize_from(&buffer[offset..])?;
            offset += key.serialized_size();
            let value = V::deserialize_from(&buffer[offset..])?;
            offset += value.serialized_size();
            map.insert(key, value);
            no_of_entries -= 1;
        }
        Ok(map)
    }
}

#[cfg(all(net, test))]
mod tests {
    use super::*;
    use core::fmt::Debug;
    
    fn check_serialization<'a, T>(item: &'a T, expected_size: usize, buffer: &'a mut Vec<u8>)
        where T: Serializable<'a> + Debug + Eq {
        buffer.clear();
        assert_eq!(expected_size, item.serialized_size());
        let sz = item.serialize_to(buffer);
        assert_eq!(expected_size, sz);
        let clone = T::deserialize_from(buffer);
        assert!(clone.is_ok());
        assert_eq!(clone.unwrap(), *item);
    }

    #[test]
    fn test_serialize_u8() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let zero_val = 0u8;
        check_serialization::<u8>(&zero_val, 1, &mut buffer);
        let max_val = u8::MAX;
        check_serialization::<u8>(&max_val, 1, &mut buffer);
    }

    #[test]
    fn test_serialize_u32() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let zero_val = 0u32;
        check_serialization::<u32>(&zero_val, 4, &mut buffer);
        let max_val = u32::MAX - 1;
        check_serialization::<u32>(&max_val, 4, &mut buffer);
    }

    #[test]
    fn test_serialize_u64() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let zero_val = 0u64;
        check_serialization::<u64>(&zero_val, 8, &mut buffer);
        let max_val = u64::MAX - 1;
        check_serialization::<u64>(&max_val, 8, &mut buffer);
    }

    #[test]
    fn test_serialize_i64() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let zero_val = 0i64;
        check_serialization::<i64>(&zero_val, 8, &mut buffer);
        let max_val = i64::MAX - 1;
        check_serialization::<i64>(&max_val, 8, &mut buffer);
        let min_val = i64::MIN + 1;
        check_serialization::<i64>(&min_val, 8, &mut buffer);
    }

    #[test]
    fn test_serialize_string() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let empty_str = String::from("");
        check_serialization::<String>(&empty_str, 8, &mut buffer);
        let ascii_str = String::from("deadbeef");
        check_serialization::<String>(&ascii_str, 16, &mut buffer);
        let unicode_str = String::from("Unicode\u{2122}inside");
        check_serialization::<String>(&unicode_str, 24, &mut buffer);
    }

    #[test]
    fn test_serialize_str() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let empty_str = "";
        check_serialization::<&str>(&empty_str, 8, &mut buffer);
        let ascii_str = "deadbeef";
        check_serialization::<&str>(&ascii_str, 16, &mut buffer);
    }

    #[test]
    fn test_serialize_opt_string() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let none_opt = None;
        check_serialization::<Option<String>>(&none_opt, 1, &mut buffer);
        let empty_str_opt = Some(String::from(""));
        check_serialization::<Option<String>>(&empty_str_opt, 9, &mut buffer);
        let ascii_str_opt = Some(String::from("deadbeef"));
        check_serialization::<Option<String>>(&ascii_str_opt, 17, &mut buffer);
    }

    #[test]
    fn test_serialize_btree_map() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let empty_map = BTreeMap::<String,String>::new();
        check_serialization::<BTreeMap<String,String>>(&empty_map, 8, &mut buffer);
        let mut single_entry_map = BTreeMap::<String,String>::new();
        single_entry_map.insert(String::from("key1"), String::from("value1"));
        check_serialization::<BTreeMap<String,String>>(&single_entry_map, 34, &mut buffer);
        let mut multi_entry_map = BTreeMap::<String,String>::new();
        multi_entry_map.insert(String::from("key1"), String::from("value1"));
        multi_entry_map.insert(String::from("key2"), String::from("value2"));
        multi_entry_map.insert(String::from("key3"), String::from("value3"));
        check_serialization::<BTreeMap<String,String>>(&multi_entry_map, 86, &mut buffer);
    }
}
