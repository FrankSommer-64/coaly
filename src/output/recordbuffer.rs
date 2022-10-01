// ---------------------------------------------------------------------------------------------
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
// ---------------------------------------------------------------------------------------------

//! Cyclic buffer for string records.
//! If the buffer is full, a write to the buffer will overwrite as many of the oldest records as
//! needed to store the new record.
//! The buffer is allocated by the caller and may be a pure memory buffer or backed by a file.

use memmap2::MmapMut;
use std::alloc::*;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::fmt::Formatter;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::rc::Rc;
use std::slice::from_raw_parts;
use crate::coalyxe;
use crate::errorhandling::*;

/// Cyclic buffer for string or binary records.
#[derive(Clone)]
pub struct RecordBuffer {
    /// optional memory map
    map: Option<Rc<RefCell<MmapMut>>>,
    /// pointer array with record start positions
    records: Vec<*mut u8>,
    /// index in records array containing the raw pointer to the first free byte in the buffer
    ins_index: usize,
    /// index in records array containing the raw pointer to the first byte of oldest stored record
    oldest_rec_index : usize,
    /// maximum index in records array
    max_rec_index : usize,
    /// number of records currently stored
    record_count : usize,
    /// buffer size
    buffer_size: usize,
    /// buffer content size
    content_size: usize,
    /// maximum length allowed for records, in bytes
    max_rec_len: usize,
    /// raw pointer to first byte in buffer
    head: *mut u8,
    /// raw pointer to byte after last content byte in buffer
    tail: *mut u8,
    /// possible extra bytes after last content byte
    extra_bytes : usize
}

impl RecordBuffer {
    /// Constructs a record buffer in main memory.
    /// The allocated buffer uses the last 4 bytes internally for easier multi-byte character
    /// handling.
    ///
    /// # Arguments
    /// * `buf_size` - the size of the buffer in bytes
    /// * `max_record_count` - the maximum number of records
    /// * `max_record_len` - the maximum allowed length for records
    pub fn in_memory(buf_size: usize,
                     max_record_count: usize,
                     max_record_len: usize) -> RecordBuffer {
        unsafe {
            let buffer_size = max(MIN_MEM_BUFFER_SIZE, buf_size);
            let content_size = buf_size - 4;
            let layout = Layout::from_size_align_unchecked(buffer_size, 8);
            let head = System.alloc(layout);
            let index_size = max(MIN_INDEX_SIZE, max_record_count);
            let mut records = Vec::<*mut u8>::with_capacity(index_size);
            records.resize(index_size, head);
            RecordBuffer {
                map: None,
                records,
                buffer_size,
                content_size,
                max_rec_len: min(max_record_len, content_size),
                head,
                tail: head.add(content_size),
                ins_index: 0,
                oldest_rec_index: 0,
                max_rec_index: index_size - 1,
                record_count: 0,
                extra_bytes: 0
            }
        }
    }

    /// Constructs a record buffer backed by a file.
    /// The allocated buffer uses the last 32 bytes internally for easier multi-byte character
    /// handling and offset storage.
    ///
    /// # Arguments
    /// * `file_path` - the full path of the backing file
    /// * `buf_size` - the size of the buffer in bytes
    /// * `max_record_count` - the maximum number of records
    pub fn backed_by_file(file_path: &PathBuf,
                          buf_size: usize,
                          max_record_count: usize) -> Result<RecordBuffer, CoalyException> {
        unsafe {
            let buffer_size = max(MIN_MAPPED_BUFFER_SIZE, buf_size);
            let content_size = buffer_size - 32;
            let res = OpenOptions::new().read(true).write(true).create(true).open(file_path);
            if let Err(io_err) = res {
                let file_name = file_path.to_string_lossy().to_string();
                return Err(coalyxe!(E_FILE_CRE_ERR, file_name, io_err.to_string()))
            }
            let f = res.unwrap();
            if let Err(io_err) = f.set_len(buffer_size as u64) {
                let file_name = file_path.to_string_lossy().to_string();
                return Err(coalyxe!(E_FILE_CRE_ERR, file_name, io_err.to_string()))
            }
            let res = MmapMut::map_mut(&f);
            if let Err(io_err) = res {
                let file_name = file_path.to_string_lossy().to_string();
                return Err(coalyxe!(E_FILE_CRE_ERR, file_name, io_err.to_string()))
            }
            let mut m = res.unwrap();
            let head = m.as_mut().as_mut_ptr();
            let index_size = max(MIN_INDEX_SIZE, max_record_count);
            let mut records = Vec::<*mut u8>::with_capacity(index_size);
            records.resize(index_size, head);
            Ok(RecordBuffer {
                map: Some(Rc::new(RefCell::new(m))),
                records,
                buffer_size,
                content_size,
                max_rec_len: content_size,
                head,
                tail: head.add(content_size),
                ins_index: 0,
                oldest_rec_index: 0,
                max_rec_index: index_size - 1,
                record_count: 0,
                extra_bytes: 0
            })
        }
    }

    /// Re-opens the memory mapped file
    /// The allocated buffer uses the last 32 bytes internally for easier multi-byte character
    /// handling and offset storage.
    ///
    /// # Arguments
    /// * `file_path` - the full path of the backing file
    /// * `create_file` - indicates whether to create the backing file 
    pub fn reopen(&mut self,
                  file_path: &PathBuf,
                  create_file: bool) -> Result<(), CoalyException> {
        unsafe {
            let res = OpenOptions::new().read(true).write(true).create(create_file).open(file_path);
            if let Err(io_err) = res {
                let file_name = file_path.to_string_lossy().to_string();
                return Err(coalyxe!(E_FILE_CRE_ERR, file_name, io_err.to_string()))
            }
            let f = res.unwrap();
            if let Err(io_err) = f.set_len(self.buffer_size as u64) {
                let file_name = file_path.to_string_lossy().to_string();
                return Err(coalyxe!(E_FILE_CRE_ERR, file_name, io_err.to_string()))
            }
            let res = MmapMut::map_mut(&f);
            if let Err(io_err) = res {
                let file_name = file_path.to_string_lossy().to_string();
                return Err(coalyxe!(E_FILE_CRE_ERR, file_name, io_err.to_string()))
            }
            self.map = Some(Rc::new(RefCell::new(res.unwrap())));
            self.clear();
            Ok(())
        }
    }

    /// Writes a record to this buffer.
    /// Older records will be overwritten if there's not enough free space in the buffer to store
    /// the record.
    /// The record will be silently truncated if its length exceeds buffer capacity or maximum
    /// record length.
    ///
    /// # Arguments
    /// * `rec` - the record to write
    #[inline]
    pub fn write(&mut self, rec: &str) {
        if rec.is_empty() { return }
        self.create_free_space(min(self.max_rec_len, rec.len()));
        self.push_str(rec);
    }

    /// Writes a record to this buffer.
    /// Older records will be overwritten if there's not enough free space in the buffer to store
    /// the record.
    /// The record will be silently truncated if its length exceeds buffer capacity or maximum
    /// record length.
    ///
    /// # Arguments
    /// * `rec` - the record to write
    #[cfg(feature="net")]
    #[inline]
    pub fn cache(&mut self, rec: &[u8]) {
        self.create_free_space(min(self.max_rec_len, rec.len()));
        self.push_slice(rec);
    }

    /// Closes the buffer.
    pub fn close(&mut self) {
        if let Some(ref mut m) = self.map { let _ = m.borrow_mut().flush(); }
        self.map = None;
    }

    /// Clears the buffer.
    pub fn clear(&mut self) {
        self.record_count = 0;
        self.extra_bytes = 0;
        self.ins_index = 0;
        self.oldest_rec_index = 0;
        *self.records.get_mut(0).unwrap() = self.head;
    }

    /// Returns a chunk containing log or trace records.
    /// A buffer may contain up to two such chunks, since it is implemented as a circular buffer
    /// overwriting old data if not enough free space is available.
    ///
    /// # Arguments
    /// * `index` - the chunk index (0 for only chunk or older records, 1 for most recent records)
    ///
    /// # Return values
    /// the slice containing the records, if chunk with desired index contains data
    pub fn chunk(&self, index: u32) -> Option<&[u8]> {
        if self.is_empty() || index > 1 { return None }
        let oldest = *self.records.get(self.oldest_rec_index).unwrap();
        let ins = *self.records.get(self.ins_index).unwrap();
        unsafe {
            if ins as usize > oldest as usize {
                if index == 0 {
                    let chunk_size = ins as usize - oldest as usize;
                    return Some(from_raw_parts(oldest, chunk_size))
                }
            } else if index == 0 {
                let chunk_size = self.tail as usize - oldest as usize + self.extra_bytes;
                return Some(from_raw_parts(oldest, chunk_size))
            } else {
                if ins == self.head { return None }
                let chunk_size = ins as usize - self.head as usize;
                return Some(from_raw_parts(self.head, chunk_size))
            }
        }
        None
    }

    /// Indicates whether this buffer can store the given number of bytes without
    /// overwriting existing data in the buffer.
    ///
    /// # Arguments
    /// * `no_of_bytes` - the number of bytes about to be written
    #[inline]
    pub fn can_lossless_hold(&self, no_of_bytes: usize) -> bool {
        // at least one unused entry in record index table is needed
        if self.record_count >= self.records.capacity() { return false }
        // and enough bytes in the content
        let needed_space = min(self.max_rec_len, no_of_bytes);
        self.free_space() >= needed_space
    }

    /// Returns the maximum length of a record, that can be stored without truncation
    #[cfg(feature="net")]
    #[inline]
    pub fn max_rec_len(&self) -> usize { self.max_rec_len }

    /// Indicates whether this buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool { self.record_count == 0 }

    /// Writes administrative data to buffer.
    /// Used for memory mapped files only, where offset of oldest record and first free byte may be
    /// needed to reconstruct the file in case of application crash.
    pub fn update_admin_data(&mut self) {
        unsafe {
            // write extra byte count to tail+3, length 1 byte
            let mut p = self.tail.add(3);
            let xb = self.extra_bytes as u8 + b'0';
            *p = xb;
            p = p.add(1);
            // write offset of beginning of oldest record to tail+4, length 14 bytes
            let u_head = self.head as usize;
            let oldest = *self.records.get(self.oldest_rec_index).unwrap() as usize - u_head;
            let oldest_str = format!("{:0>14}", oldest);
            for b in oldest_str.as_bytes() {
                *p = *b;
                p = p.add(1);
            }
            // write offset of first free byte to tail+18, length 14 bytes
            let ins = *self.records.get(self.ins_index).unwrap() as usize - u_head;
            let ins_str = format!("{:0>14}", ins);
            for b in ins_str.as_bytes() {
                *p = *b;
                p = p.add(1);
            }
        }
    }

    /// Returns all records in this buffer for iteration.
    /// Because of the circular buffer nature, one record may consist of two parts.
    /// This is the reason for the tuple items in the returned vector.
    #[cfg(feature="net")]
    pub fn records(&self) -> Vec::<(&[u8], Option<&[u8]>)> {
        if self.record_count == 0 { return Vec::<(&[u8], Option<&[u8]>)>::new() }
        let mut recs = Vec::<(&[u8], Option<&[u8]>)>::with_capacity(self.record_count);
        let mut rec_index = self.oldest_rec_index;
        for _ in 0..self.record_count {
            let next_rec_index = if rec_index < self.max_rec_index { rec_index+1 } else { 0 };
            let rec_start = *self.records.get(rec_index).unwrap();
            let next_rec_start = *self.records.get(next_rec_index).unwrap();
            if next_rec_start < rec_start {
                // wrapping record data
                let rec_len1 = self.tail as usize - rec_start as usize + self.extra_bytes;
                let rec_len2 = next_rec_start as usize - self.head as usize;
                unsafe {
                    recs.push((from_raw_parts(rec_start, rec_len1),
                               Some(from_raw_parts(self.head, rec_len2))));
                }
            } else {
                // consecutive record data
                let rec_len = next_rec_start as usize - rec_start as usize;
                unsafe { recs.push((from_raw_parts(rec_start, rec_len), None)); }
            }
            rec_index = next_rec_index;
        }
        recs
    }

    /// Returns the free space in this buffer.
    fn free_space(&self) -> usize {
        if self.is_empty() { return self.content_size }
        let oldest = *self.records.get(self.oldest_rec_index).unwrap() as usize;
        let ins = *self.records.get(self.ins_index).unwrap() as usize;
        if ins > oldest { return oldest - self.head as usize + self.tail as usize - ins }
        oldest - ins
    }

    /// Creates free space in the buffer.
    fn create_free_space(&mut self, needed_space: usize) {
        if needed_space >= self.content_size {
            self.clear();
            return
        }
        let mut available_space = self.free_space();
        while available_space < needed_space { available_space += self.remove_oldest_record(); }
    }

    /// Stores the given record in the buffer.
    /// The caller must have made sure that the buffer can store the record without overwriting
    /// older records.
    fn push_str(&mut self, rec: &str) {
        let mut ins = *self.records.get(self.ins_index).unwrap();
        let oldest = *self.records.get(self.oldest_rec_index).unwrap() as usize;
        if ins as usize <= oldest {
            // insertion position before start of data means the record can be stored entirely in
            // one contiguous part of the buffer
            let data = if rec.len() > self.max_rec_len { truncate_rec(rec, self.max_rec_len) }
                       else { rec.as_bytes() };
            ins = self.push_chunk(ins, data);
        } else {
            // insertion position after start of data, determine gap to buffer tail
            // one contiguous part of the buffer
            let gap = self.tail as usize - ins as usize;
            if gap >= rec.len() {
                // gap after insertion position is large enough to store entire record
                let data = if rec.len() > self.max_rec_len { truncate_rec(rec, self.max_rec_len) }
                           else { rec.as_bytes() };
                ins = self.push_chunk(ins, data);
            } else {
                // gap after insertion position is too small to store entire record,
                // split record into tow parts
                let (part1, part2) = split_rec(rec, gap);
                self.push_chunk(ins, part1);
                ins = self.head;
                ins = self.push_chunk(ins, part2);
            }
        }
        let new_ins_index = if self.ins_index == self.max_rec_index { 0 }
                            else { self.ins_index + 1 };
        *self.records.get_mut(new_ins_index).unwrap() = ins;
        self.ins_index = new_ins_index;
        self.record_count += 1;
        if self.map.is_some() { self.update_admin_data(); }
    }

    /// Stores the given record in the buffer.
    /// The caller must have made sure that the buffer can store the record without overwriting
    /// older records.
    #[cfg(feature="net")]
    fn push_slice(&mut self, rec: &[u8]) {
        let mut ins = *self.records.get(self.ins_index).unwrap();
        let oldest = *self.records.get(self.oldest_rec_index).unwrap() as usize;
        if ins as usize <= oldest {
            // insertion position before start of data means the record can be stored entirely in
            // one contiguous part of the buffer
            ins = self.push_chunk(ins, rec);
        } else {
            // insertion position after start of data, determine gap to buffer tail
            // one contiguous part of the buffer
            let gap = self.tail as usize - ins as usize;
            if gap >= rec.len() {
                // gap after insertion position is large enough to store entire record
                ins = self.push_chunk(ins, rec);
            } else {
                // gap after insertion position is too small to store entire record,
                // split record into tow parts
                self.push_chunk(ins, &rec[0..gap]);
                ins = self.head;
                ins = self.push_chunk(ins, &rec[gap..]);
            }
        }
        let new_ins_index = if self.ins_index == self.max_rec_index { 0 }
                            else { self.ins_index + 1 };
        *self.records.get_mut(new_ins_index).unwrap() = ins;
        self.ins_index = new_ins_index;
        self.record_count += 1;
    }

    /// Stores the given record in the buffer.
    /// The caller must have made sure that the buffer can store the record without overwriting
    /// older records.
    fn push_chunk(&mut self, to: *mut u8, from: &[u8]) -> *mut u8 {
        unsafe {
            let mut p = to;
            for b in from {
                *p = *b;
                p = p.add(1);
            }
            if (p as usize) < (self.tail as usize) { return p }
            if (p as usize) > (self.tail as usize) {
                self.extra_bytes = (p as usize) - (self.tail as usize);
            }
            self.head
        }
    }

    /// Removes the oldest record in the buffer.
    /// Function must not be called if the buffer is empty.
    ///
    /// # Return values
    /// the number of bytes occupied by the removed record
    fn remove_oldest_record(&mut self) -> usize {
        if self.record_count <= 1 {
            self.clear();
            return self.max_rec_len
        }
        // at this point we can be sure there are at least two records stored in the buffer
        let oldest = *self.records.get(self.oldest_rec_index).unwrap() as usize;
        let second_oldest_index = if self.oldest_rec_index == self.max_rec_index { 0 }
                                  else { self.oldest_rec_index + 1 };
        let second_oldest = *self.records.get(second_oldest_index).unwrap() as usize;
        self.oldest_rec_index = second_oldest_index;
        self.record_count -= 1;
        if second_oldest > oldest {
            // oldest record occupies contigous memory
            return second_oldest - oldest
        }
        // oldest record wraps around
        self.extra_bytes = 0;
        self.tail as usize - oldest + second_oldest - self.head as usize
    }
}

impl std::fmt::Debug for RecordBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CS:{}/IS:{}/ML:{}/MI:{}/RC:{}/EX:{}/IX:{}/IP:{}/OX:{}/OP:{}",
                  self.content_size, self.records.capacity(),
                  self.max_rec_len, self.max_rec_index, self.record_count, self.extra_bytes,
                  self.ins_index, self.records[self.ins_index] as usize - self.head as usize,
                  self.oldest_rec_index,
                  self.records[self.oldest_rec_index] as usize - self.head as usize)
    }
}
impl Drop for RecordBuffer {
    fn drop(&mut self) {
        if self.map.is_none() {
            unsafe {
                let layout = Layout::from_size_align_unchecked(self.buffer_size, 8);
                System.dealloc(self.head, layout);
            }
        }
    }
}

/// Returns the encoded bytes for the given record, truncated to the specified length.
/// Function considers character boundaries, hence the length of the returned bytes may be
/// smaller than the specified maximum.
///
/// # Arguments
/// * `rec` - the record
/// * `max_encoded_length` - the maximum allowed length for the encoded bytes
fn truncate_rec(rec: &str, max_encoded_length: usize) -> &[u8] {
    let mut enc_buf: [u8; 4] = [0,0,0,0];
    let mut trunc_len = rec.len();
    let mut rec_chars = rec.chars();
    while trunc_len > max_encoded_length {
        let ch = rec_chars.next_back().unwrap();
        trunc_len -= ch.encode_utf8(&mut enc_buf).len();
    }
    &rec.as_bytes()[0..trunc_len]
}

/// Splits a record into two byte slices at the desired byte position.
/// Function considers character boundaries, the length of the first part may exceed the
/// specified split position by a maximum of two bytes.
///
/// # Arguments
/// * `rec` - the record
/// * `desired_split_pos` - the byte-based position where to split the record
///
    /// # Return values
/// tuple containing the slices with both parts of the split record
fn split_rec(rec: &str, desired_split_pos: usize) -> (&[u8], &[u8]) {
    let rec_len = rec.len();
    let mut enc_buf: [u8; 4] = [0, 0, 0, 0];
    let mut act_split_pos = 0usize;
    let mut rec_chars = rec.chars();
    if desired_split_pos > rec_len >> 1 {
        // First part larger. Sum up byte lengths of all characters starting from the end of
        // the record until we reach desired split position.
        let mut last_enc_len = 0;
        act_split_pos = rec_len;
        while act_split_pos > desired_split_pos {
            let ch = rec_chars.next_back().unwrap();
            last_enc_len = ch.encode_utf8(&mut enc_buf).len();
            act_split_pos -= last_enc_len;
        }
        // If desired split position was not on a character boundary, that character must be
        // assigned to the first part
        if act_split_pos < desired_split_pos { act_split_pos += last_enc_len; }
    } else {
        // Parts equal in size or second part larger. Sum up byte lengths of all characters starting
        // from the beginning of the record until we reach desired split position.
        while act_split_pos < desired_split_pos {
            let ch = rec_chars.next().unwrap();
            act_split_pos += ch.encode_utf8(&mut enc_buf).len();
        }
    }
    (&rec.as_bytes()[0..act_split_pos], &rec.as_bytes()[act_split_pos..rec.len()])
}

const MIN_MEM_BUFFER_SIZE: usize = 20;
const MIN_MAPPED_BUFFER_SIZE: usize = 48;
const MIN_INDEX_SIZE: usize = 4;

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::str;
    use std::time::{SystemTime, UNIX_EPOCH};
    use super::*;
    #[cfg(feature="net")]
    use crate::record::RecordLevelId;
    #[cfg(feature="net")]
    use crate::record::recorddata::*;
    #[cfg(feature="net")]
    use crate::net::serializable::Serializable;

    const EMPTY_STR: &str = "";
    const ASCII_4: &str = "1234";
    const ASCII_5: &str = "12345";
    const ASCII_6: &str = "123456";
    const ASCII_7: &str = "1234567";
    const ASCII_6_U3: &str = "123456\u{25b9}";
    const ASCII_6_U3_4: &str = "123456\u{25b9}1234";
    const ASCII_7_U2: &str = "1234567œ";
    const ASCII_7_U2_4: &str = "1234567œ1234";
    const ASCII_8: &str = "12345678";
    const ASCII_8_SYM: &str = "12341234";
    const ASCII_9: &str = "123456789";
    const ASCII_9_REPA5: &str = "123451234";
    const ASCII_9_REPA4: &str = "123412345";
    const U2: &str = "œ";
    const U2_A4: &str = "œ1234";
    const U3: &str = "\u{25b9}";
    const U3_A4: &str = "\u{25b9}1234";
    const REC_7: &str = ASCII_7;
    const REC_7_U2_4: &str = ASCII_7_U2_4;
    const REC_7_U3_4: &str = "1234567\u{25b9}1234";
    const REC_7_U4_4: &str = "1234567𝄞1234";
    const REC_8: &str = ASCII_8;
    const REC_8X2: &str = "1234567812345678";
    const REC_8X3: &str = "123456781234567812345678";
    const REC_8_16: &str = "123456781234567890123456";
    const REC_8_7_U2_4: &str = "123456781234567œ1234";
    const REC_8_7_U3_4: &str = "123456781234567\u{25b9}1234";
    const REC_8_7_U4_4: &str = "123456781234567𝄞1234";
    const REC_16: &str = "1234567890123456";
    const REC_20: &str = "12345678901234567890";
    const REC_42: &str = "12345678901234567890123456789012345678901\n";

    /// Verifies a buffer's administrative attributes
    fn verify_attrs(buf: &RecordBuffer, exp_attrs: &str, desc: &'static str) {
        let actual_attrs = format!("{:?}", buf);
        assert_eq!(exp_attrs, actual_attrs, "{}", desc);
    }

    /// Verifies buffer contents.
    fn verify_contents(buf: &RecordBuffer, exp_contents: &str, desc: &'static str) {
        let mut act_contents = String::from("");
        if let Some(s) = buf.chunk(0) { act_contents.push_str(str::from_utf8(s).unwrap()); }
        if let Some(s) = buf.chunk(1) { act_contents.push_str(str::from_utf8(s).unwrap()); }
        assert_eq!(exp_contents, act_contents, "{}", desc);
    }

    /// Modifies administrative attributes in the buffer.
    fn modify_buffer(buf: &mut RecordBuffer, rec_cnt: usize, ins_x: usize,
                     ins_p: usize, old_x: usize, old_p: usize, extra: usize) {
        buf.record_count = rec_cnt;
        buf.ins_index = ins_x;
        buf.oldest_rec_index = old_x;
        buf.extra_bytes = extra;
        unsafe {
            *buf.records.get_mut(ins_x).unwrap() = buf.head.add(ins_p);
            *buf.records.get_mut(old_x).unwrap() = buf.head.add(old_p);
        }
    }

    #[test]
    /// Tests truncation of unicode string to maximum encoded length.
    /// Truncated encoded string must not be longer than the given maximum, shorter if
    /// character at maximum length is a multi-byte unicode character.
    fn test_truncate_rec() {
        assert_eq!(EMPTY_STR.as_bytes(), truncate_rec(EMPTY_STR, 8));
        assert_eq!(ASCII_5.as_bytes(), truncate_rec(ASCII_5, 8));
        assert_eq!(ASCII_8.as_bytes(), truncate_rec(ASCII_8, 8));
        assert_eq!(ASCII_8.as_bytes(), truncate_rec(ASCII_9, 8));
        assert_eq!(ASCII_6.as_bytes(), truncate_rec(ASCII_6_U3, 8));
        assert_eq!(ASCII_7.as_bytes(), truncate_rec(ASCII_7_U2, 8));
    }

    #[test]
    /// Tests record splitting into two parts.
    /// Inside of a buffer the first part is always stored at the end of the content.
    /// In contrast to record truncation the first part may be larger than the specified length,
    /// the extra bytes after the buffer's tail are used for a multibyte character.
    fn test_split_rec() {
        // both parts equal length
        let (p1,p2) = split_rec(ASCII_8_SYM, 4);
        assert_eq!(ASCII_4.as_bytes(), p1);
        assert_eq!(ASCII_4.as_bytes(), p2);

        // first part longer
        let (p1,p2) = split_rec(ASCII_9_REPA5, 5);
        assert_eq!(ASCII_5.as_bytes(), p1);
        assert_eq!(ASCII_4.as_bytes(), p2);

        // second part longer
        let (p1,p2) = split_rec(ASCII_9_REPA4, 4);
        assert_eq!(ASCII_4.as_bytes(), p1);
        assert_eq!(ASCII_5.as_bytes(), p2);

        // two-byte character at split position
        let (p1,p2) = split_rec(ASCII_7_U2_4, 8);
        assert_eq!(ASCII_7_U2.as_bytes(), p1);
        assert_eq!(ASCII_4.as_bytes(), p2);

        // three-byte character at split position
        let (p1,p2) = split_rec(ASCII_6_U3_4, 7);
        assert_eq!(ASCII_6_U3.as_bytes(), p1);
        assert_eq!(ASCII_4.as_bytes(), p2);

        // two-byte character at split position 1
        let (p1,p2) = split_rec(U2_A4, 1);
        assert_eq!(U2.as_bytes(), p1);
        assert_eq!(ASCII_4.as_bytes(), p2);

        // three-byte character at split position 1
        let (p1,p2) = split_rec(U3_A4, 1);
        assert_eq!(U3.as_bytes(), p1);
        assert_eq!(ASCII_4.as_bytes(), p2);

        // empty record, split pos 0
        let (p1,p2) = split_rec(EMPTY_STR, 0);
        assert_eq!(EMPTY_STR.as_bytes(), p1);
        assert_eq!(EMPTY_STR.as_bytes(), p2);

        // empty record, split pos 1
        let (p1,p2) = split_rec(EMPTY_STR, 1);
        assert_eq!(EMPTY_STR.as_bytes(), p1);
        assert_eq!(EMPTY_STR.as_bytes(), p2);
    }

    #[test]
    /// Tests memory buffer construction with different ratios of max record length and buffer size
    fn test_mem_construction() {
        // max record length smaller than buffer size
        let buf = RecordBuffer::in_memory(128, 8, 80);
        verify_attrs(&buf, "CS:124/IS:8/ML:80/MI:7/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "maxrl<size");

        // max record length equal to buffer size
        let buf = RecordBuffer::in_memory(128, 8, 128);
        verify_attrs(&buf, "CS:124/IS:8/ML:124/MI:7/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "maxrl=size");

        // max record length larger than buffer size
        let buf = RecordBuffer::in_memory(128, 8, 256);
        verify_attrs(&buf, "CS:124/IS:8/ML:124/MI:7/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "maxrl>size");
    }

    #[test]
    /// Tests construction of buffer backed by a file.
    fn test_file_construction() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let pure_file_name = format!("recbuf{}.bin", now);
        // normal case
        let rw_dir = Path::new(&std::env::var("COALY_TESTING_ROOT").unwrap()).join("tmp");
        let file_name = rw_dir.join(&pure_file_name);
        if let Ok(mut buf) = RecordBuffer::backed_by_file(&file_name, 4096, 100) {
            verify_attrs(&buf, "CS:4064/IS:100/ML:4064/MI:99/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "");
            for _ in 0..100 { buf.write(REC_42); }
        } else {
            assert!(false);
        }
        // creation in non-writable directory must fail
        let ro_dir = Path::new(&std::env::var("TESTING_ROOT").unwrap()).join("readonly");
        let file_name = ro_dir.join(&pure_file_name);
        assert!(RecordBuffer::backed_by_file(&file_name, 4096, 100).is_err());
    }

    #[test]
    /// Tests determining buffer free space
    fn test_free_space() {
        // empty buffer
        let buf = RecordBuffer::in_memory(28, 8, 24);
        assert_eq!(24, buf.free_space());

        // buffer filled from head, remaining space > 1
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 8, 0, 0, 0);
        assert_eq!(16, buf.free_space());

        // buffer filled from head, remaining space == 1
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 3, 3, 23, 0, 0, 0);
        assert_eq!(1, buf.free_space());

        // buffer filled from head, buffer full
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 3, 3, 0, 0, 0, 0);
        assert_eq!(0, buf.free_space());

        // buffer starts with 1 free byte, remaining space after data > 1
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 8, 0, 1, 0);
        assert_eq!(17, buf.free_space());

        // buffer starts with 1 free byte, remaining space after data == 1
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 23, 0, 1, 0);
        assert_eq!(2, buf.free_space());

        // buffer starts with 1 free byte, remaining space after data 0
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 0, 0, 1, 0);
        assert_eq!(1, buf.free_space());

        // buffer starts with >1 free bytes, remaining space after data > 1
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 16, 0, 8, 0);
        assert_eq!(16, buf.free_space());

        // buffer starts with >1 free bytes, remaining space after data == 1
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 23, 0, 8, 0);
        assert_eq!(9, buf.free_space());

        // buffer starts with >1 free bytes, remaining space after data 0
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 0, 0, 8, 0);
        assert_eq!(8, buf.free_space());

        // buffer wraps around
        let mut buf = RecordBuffer::in_memory(28, 8, 24);
        modify_buffer(&mut buf, 1, 1, 8, 0, 16, 0);
        assert_eq!(8, buf.free_space());
    }

    #[test]
    /// Tests for writing a record into an empty buffer
    fn test_write_to_empty_buffer() {
        // empty record
        let mut buf = RecordBuffer::in_memory(52, 8, 32);
        assert!(buf.can_lossless_hold("".len()), "empty record");
        buf.write(EMPTY_STR);
        verify_attrs(&buf, "CS:48/IS:8/ML:32/MI:7/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "empty rec");

        // record smaller than max record length and buffer size
        let mut buf = RecordBuffer::in_memory(52, 8, 32);
        assert!(buf.can_lossless_hold(REC_16.len()));
        buf.write(REC_16);
        verify_attrs(&buf, "CS:48/IS:8/ML:32/MI:7/RC:1/EX:0/IX:1/IP:16/OX:0/OP:0", "reclen<maxrl");
        verify_contents(&buf, REC_16, "reclen<maxrl");

        // record equal to max record length, smaller than buffer size
        let mut buf = RecordBuffer::in_memory(52, 8, 16);
        assert!(buf.can_lossless_hold(REC_16.len()));
        buf.write(REC_16);
        verify_attrs(&buf, "CS:48/IS:8/ML:16/MI:7/RC:1/EX:0/IX:1/IP:16/OX:0/OP:0", "reclen=maxrl");
        verify_contents(&buf, REC_16, "reclen=maxrl");

        // record larger than max record length, smaller than buffer size
        let mut buf = RecordBuffer::in_memory(52, 8, 8);
        assert!(buf.can_lossless_hold(REC_16.len()));
        buf.write(REC_16);
        verify_attrs(&buf, "CS:48/IS:8/ML:8/MI:7/RC:1/EX:0/IX:1/IP:8/OX:0/OP:0", "reclen>maxrl");
        verify_contents(&buf, REC_8, "reclen>maxrl");

        // record equal to max record length and buffer size
        let mut buf = RecordBuffer::in_memory(20, 8, 16);
        assert!(buf.can_lossless_hold(REC_16.len()));
        buf.write(REC_16);
        verify_attrs(&buf, "CS:16/IS:8/ML:16/MI:7/RC:1/EX:0/IX:1/IP:0/OX:0/OP:0", "reclen=size");
        verify_contents(&buf, REC_16, "reclen=size");

        // record larger than max record length and buffer size
        let mut buf = RecordBuffer::in_memory(20, 8, 16);
        assert!(buf.can_lossless_hold(REC_20.len()));
        verify_contents(&buf, "", "reclen>size");
        buf.write(REC_20);
        verify_attrs(&buf, "CS:16/IS:8/ML:16/MI:7/RC:1/EX:0/IX:1/IP:0/OX:0/OP:0", "reclen>size");
        verify_contents(&buf, REC_16, "reclen>size");
    }

    #[test]
    /// Tests write to buffer containing records starting at content head
    fn test_filled_from_beginning() {
        // new record fits
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:0/IX:2/IP:16/OX:0/OP:0", "rec fits");
        verify_contents(&buf, REC_8X2, "rec fits");

        // new record occupies remaining space
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_8);
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:3/EX:0/IX:3/IP:0/OX:0/OP:0", "rec uses all");
        verify_contents(&buf, REC_8X3, "rec uses all");

        // new record doesn't fit
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_16);
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:0/IX:3/IP:8/OX:1/OP:8", "rec too long");
        verify_contents(&buf, REC_8_16, "rec too long");

        // new record with two-byte char doesn't fit
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_7_U2_4);
        assert_eq!(4, buf.free_space(), "rec 1 extra");
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:1/IX:3/IP:4/OX:1/OP:8", "rec 1 extra");
        verify_contents(&buf, REC_8_7_U2_4, "rec 1 extra");

        // new record with three-byte char doesn't fit
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_7_U3_4);
        assert_eq!(4, buf.free_space(), "rec 2 extra");
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:2/IX:3/IP:4/OX:1/OP:8", "rec 2 extra");
        verify_contents(&buf, REC_8_7_U3_4, "rec 2 extra");

        // new record with four-byte char doesn't fit
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_7_U4_4);
        assert_eq!(4, buf.free_space(), "rec 3 extra");
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:3/IX:3/IP:4/OX:1/OP:8", "rec 3 extra");
        verify_contents(&buf, REC_8_7_U4_4, "rec 3 extra");
    }

    #[test]
    /// Test remove oldest record from buffer
    fn test_remove_oldest() {
        // empty buffer must return max rec size
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        assert_eq!(16, buf.remove_oldest_record(), "empty buf");
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "empty buf");

        // one record in buffer must return max rec size
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        assert_eq!(16, buf.remove_oldest_record(), "1 rec");
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:0/EX:0/IX:0/IP:0/OX:0/OP:0", "1 rec");

        // second oldest after oldest
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        assert_eq!(8, buf.remove_oldest_record(), "old2>old");
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:1/EX:0/IX:2/IP:16/OX:1/OP:8", "old2>old");

        // oldest at buffer end, second oldest at beginning
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_8);
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:3/EX:0/IX:5/IP:16/OX:2/OP:16", "-olde old2b");
        assert_eq!(8, buf.remove_oldest_record());
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:0/IX:5/IP:16/OX:3/OP:0", "+olde old2b");

        // oldest wraps around buffer end
        let mut buf = RecordBuffer::in_memory(28, 8, 16);
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_7);
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:3/EX:0/IX:3/IP:23/OX:0/OP:0", "--old wraps");
        buf.write(REC_8);
        buf.write(REC_8);
        buf.write(REC_8);
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:3/EX:0/IX:6/IP:23/OX:3/OP:23", "-old wraps");
        assert_eq!(8, buf.remove_oldest_record());
        verify_attrs(&buf, "CS:24/IS:8/ML:16/MI:7/RC:2/EX:0/IX:6/IP:23/OX:4/OP:7", "+old wraps");
    }

    #[cfg(feature="net")]
    #[test]
    /// Test record data storage
    fn test_record_data() {
        let rec_data = LocalRecordData::for_write(1234, "thread1", RecordLevelId::Info,
                                                  "/src/myfilename.rs", 284,
                                                  "Very important message");
        let rec_data = RemoteRecordData::from(rec_data);
        let mut ser_buf = Vec::<u8>::with_capacity(1024);
        rec_data.serialize_to(&mut ser_buf);
        let mut rec_buf = RecordBuffer::in_memory(244, 8, 128);

        // one record, stored as single chunk
        rec_buf.cache(ser_buf.as_slice());
        verify_attrs(&rec_buf, "CS:240/IS:8/ML:128/MI:7/RC:1/EX:0/IX:1/IP:115/OX:0/OP:0", "1rec");
        let ch0 = rec_buf.chunk(0);
        assert!(ch0.is_some());
        let res = RemoteRecordData::deserialize_from(&ch0.unwrap());
        assert!(res.is_ok());
        assert_eq!(rec_data, res.unwrap());

        // two records, one stored as two chunks
        rec_buf.cache(ser_buf.as_slice());
        rec_buf.cache(ser_buf.as_slice());
        verify_attrs(&rec_buf, "CS:240/IS:8/ML:128/MI:7/RC:2/EX:0/IX:3/IP:105/OX:1/OP:115", "2recs");
        let recs = rec_buf.records();
        assert_eq!(2, recs.len());
        let rec_data0 = recs.get(0).unwrap();
        assert!(rec_data0.1.is_none());
        let rec0 = RemoteRecordData::deserialize_from(&rec_data0.0);
        assert!(rec0.is_ok());
        assert_eq!(rec_data, rec0.unwrap());
        let rec_data1 = recs.get(1).unwrap();
        assert!(rec_data1.1.is_some());
        let mut full_rec = Vec::<u8>::with_capacity(512);
        full_rec.extend_from_slice(rec_data1.0);
        full_rec.extend_from_slice(rec_data1.1.unwrap());
        let rec1 = RemoteRecordData::deserialize_from(full_rec.as_slice());
        assert!(rec1.is_ok());
        assert_eq!(rec_data, rec1.unwrap());
  }
}