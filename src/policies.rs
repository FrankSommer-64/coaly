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

//! Buffer handling and file rollover policies.

use regex::Regex;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use crate::coalyxw;
use crate::datetime::{Interval, TimeSpan, TimeSpanUnit, TimeStampAnchor};
use crate::errorhandling::*;
use crate::collections::MapWithDefault;
use crate::util::parse_size_str;

// Default size for memory buffer contents
pub(crate) const DEF_BUFFER_CONT_SIZE: usize = 8 * 1024 * 1024; // 8 MByte
// Minimum size for memory buffer contents
pub(crate) const MIN_BUFFER_CONT_SIZE: usize = 4096; // 4 KByte
// Maximum size for memory buffer contents
pub(crate) const MAX_BUFFER_CONT_SIZE: usize = u32::MAX as usize; // 4 GByte

// Default size for memory buffer record index
pub(crate) const DEF_BUFFER_INDEX_SIZE: usize = 1024 * 1024; // 1 M entries
// Minimum size for memory buffer record index
pub(crate) const MIN_BUFFER_INDEX_SIZE: usize = 4096; // 4 KByte
// Maximum size for memory buffer record index
pub(crate) const MAX_BUFFER_INDEX_SIZE: usize = u32::MAX as usize; // 4 GByte

// Default value and range for maximum record length in an output buffer
pub(crate) const DEF_MAX_REC_LEN: usize = 4096;
pub(crate) const MIN_MAX_REC_LEN: usize = 1;
pub(crate) const MAX_MAX_REC_LEN: usize = isize::MAX as usize;

// Default value and range for size of output files
pub(crate) const DEF_FILE_SIZE: usize = 20 * 1024 * 1024;
pub(crate) const MIN_FILE_SIZE: usize = 4096;
pub(crate) const MAX_FILE_SIZE: usize = isize::MAX as usize;

// Default number of old files to keep before deletion
pub(crate) const DEFAULT_KEEP_COUNT: usize = 9;
pub(crate) const MIN_KEEP_COUNT: usize = 1;
pub(crate) const MAX_KEEP_COUNT: usize = 255;

// Name for default policy
pub(crate) const DEFAULT_POLICY_NAME: &str = "default";

/// Conditions when to flush memory buffer to associated physical resource
#[derive (Clone, Copy, Eq, Hash, PartialEq)]
#[repr(u32)]
pub(crate) enum BufferFlushCondition {
    /// Flush buffer if an output record with level Error has been issued.
    /// The current contents plus the output record is written to the associated physical resource,
    /// then the buffer is cleared.
    Error = 0b1,
    /// Flush if an output record with level Warning has been issued.
    /// The current contents plus the output record is written to the associated physical resource,
    /// then the buffer is cleared.
    Warning = 0b10,
    /// Flush if the memory buffer is full, i.e. if an output record with any level has been issued
    /// that would exceed the buffer's capacity.
    /// The current contents plus the output record is written to the associated physical resource,
    /// then the buffer is cleared.
    Full = 0b100,
    /// Flush if the associated physical resource is a file and the file is rolled over to the
    /// next version.
    /// The current contents plus the output record is written to the current file version
    /// (before rollover), then the buffer is cleared. Afterwards the rollover takes place.
    Rollover = 0b1000,
    /// Flush if the application exits.
    /// The current contents is written to the associated physical resource.
    Exit = 0b10000
}
impl Debug for BufferFlushCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BufferFlushCondition::Error => write!(f, "{}", FLUSH_ON_ERROR),
            BufferFlushCondition::Warning => write!(f, "{}", FLUSH_ON_WARNING),
            BufferFlushCondition::Full => write!(f, "{}", FLUSH_ON_FULL),
            BufferFlushCondition::Rollover => write!(f, "{}", FLUSH_ON_ROLLOVER),
            BufferFlushCondition::Exit => write!(f, "{}", FLUSH_ON_EXIT),
        }
    }
}
impl Default for BufferFlushCondition {
    fn default() -> Self { BufferFlushCondition::Exit }
}
impl FromStr for BufferFlushCondition {
    type Err = CoalyException;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            FLUSH_ON_ERROR | "" => Ok(BufferFlushCondition::Error),
            FLUSH_ON_WARNING => Ok(BufferFlushCondition::Warning),
            FLUSH_ON_FULL => Ok(BufferFlushCondition::Full),
            FLUSH_ON_ROLLOVER => Ok(BufferFlushCondition::Rollover),
            FLUSH_ON_EXIT => Ok(BufferFlushCondition::Exit),
            _ => Err(coalyxw!(W_CFG_UNKNOWN_BUF_FLUSH_CONDITION, s.to_string()))
        }
    }
}

/// Policy for the buffer of a physical resource
#[derive (Clone)]
pub(crate) struct BufferPolicy {
    // policy name
    name: String,
    // buffer content size in bytes
    content_size: usize,
    // buffer record index size in entries
    index_size: usize,
    // bit mask with all conditions causing the buffer to be flushed
    // to associated physical resource
    flush_conditions: u32,
    // maximum length for a trace or log record, otherwise it is truncated
    max_record_length: usize
}
impl BufferPolicy {
    /// Creates a buffer policy.
    /// Used for a policy defined in the policies.buffer section of the custom configuration file.
    ///
    /// # Arguments
    /// * `name` - the policy name
    /// * `content_size` - the buffer content size in bytes
    /// * `index_size` - the buffer record index size in entries
    /// * `flush_conditions` - the bit mask indicating all conditions causing the buffer contents
    ///                        to be flushed to associated physical resource
    #[inline]
    pub(crate) fn new(name: &str,
                      content_size: usize,
                      index_size: usize,
                      flush_conditions: u32,
                      max_record_length: usize) -> BufferPolicy {
        BufferPolicy {
            name: name.to_string(),
            content_size,
            index_size,
            flush_conditions,
            max_record_length }
    }

    /// Returns the buffer content size for this policy, in bytes.
    #[inline]
    pub(crate) fn content_size(&self) -> usize { self.content_size }

    /// Returns the buffer record index size for this policy, in number of records.
    #[inline]
    pub(crate) fn index_size(&self) -> usize { self.index_size }

    /// Returns the flush conditions for this policy.
    #[inline]
    pub(crate) fn flush_conditions(&self) -> u32 { self.flush_conditions }

    /// Returns the maximum record length for this policy, in bytes.
    #[inline]
    pub(crate) fn max_record_length(&self) -> usize { self.max_record_length }

    /// Returns the default flush conditions for buffer policies.
    #[inline]
    pub(crate) fn default_flush_conditions() -> u32 {
        (BufferFlushCondition::Error as u32) | (BufferFlushCondition::Exit as u32)
    }
}
impl Default for BufferPolicy {
    fn default() -> Self {
        Self {
            name: DEFAULT_POLICY_NAME.to_string(),
            content_size: DEF_BUFFER_CONT_SIZE,
            index_size: DEF_BUFFER_INDEX_SIZE,
            flush_conditions: BufferPolicy::default_flush_conditions(),
            max_record_length: DEF_MAX_REC_LEN
        }
    }
}
impl Debug for BufferPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "N:{}/CS:{}/IS:{}/C:{:b}/L:{}", self.name, self.content_size, self.index_size,
                                           self.flush_conditions, self.max_record_length)
    }
}

pub(crate) type BufferPolicyMap = MapWithDefault<BufferPolicy>;

/// Conditions causing an output file to be closed and a new one to be opened
#[derive (Clone)]
pub(crate) enum RolloverCondition {
    /// New version of a file started if the current one reaches or exceeds size limit
    SizeReached(usize),
    /// New version of a file started if a specific time span has elapsed
    TimeElapsed(Interval),
    /// No rollover, only one file
    Never
}
impl Default for RolloverCondition {
    fn default() -> Self { RolloverCondition::SizeReached(DEF_FILE_SIZE) }
}
impl Debug for RolloverCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RolloverCondition::SizeReached(s) => write!(f, "SZ:{}", s),
            RolloverCondition::TimeElapsed(i) => write!(f, "INT:{:?}", i),
            RolloverCondition::Never => write!(f, "NEVER"),
        }
    }
}
impl FromStr for RolloverCondition {
    type Err = CoalyException;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cond_str = s.to_lowercase();
        if cond_str.is_empty() || cond_str.eq(ROVR_COND_NEVER) {
            return Ok(RolloverCondition::Never)
        }
        let size_pat = Regex::new(ROVR_COND_SIZE_PATTERN).unwrap();
        if let Some(capts) = size_pat.captures(&cond_str) {
            // Rollover based on file size
            // size > n[k|m|g]
            let size_def = capts.get(1).unwrap().as_str();
            if let Some(size_val) = parse_size_str(size_def) {
                return Ok(RolloverCondition::SizeReached(size_val))
            }
            return Err(coalyxw!(W_CFG_INV_ROVR_FILE_SIZE, size_def.to_string()))
        }
        let intvl_pat = Regex::new(ROVR_COND_INTVL_PATTERN).unwrap();
        if let Some(capts) = intvl_pat.captures(&cond_str) {
            // Periodic rollover every time an interval after application start elapses
            // every [n] timespanunit
            let mut ts_val: u32 = 1;
            if let Some(valspec) = capts.get(1) {
                let valspec = valspec.as_str().trim();
                if let Ok(val) = u32::from_str(valspec) {
                    ts_val = val;
                } else {
                    return Err(coalyxw!(W_CFG_INV_NUM_IN_INTVL, valspec.to_string()))
                }
            }
            let unit_spec = capts.get(2).unwrap().as_str();
            if let Ok(unit_val) = TimeSpanUnit::from_str(unit_spec) {
                let ts = TimeSpan::new(unit_val, ts_val);
                let intvl = Interval::unanchored(ts);
                return Ok(RolloverCondition::TimeElapsed(intvl))
            }
            return Err(coalyxw!(W_CFG_INV_UNIT_IN_INTVL, unit_spec.to_string()))
        }
        let intvl_pat = Regex::new(ROVR_COND_INTVL_AT_PATTERN).unwrap();
        if let Some(capts) = intvl_pat.captures(&cond_str) {
            // Periodic rollover every time a specific moment of time is reached
            // every [n] timespanunit at moment
            let mut ts_val: u32 = 1;
            if let Some(valspec) = capts.get(1) {
                let valspec = valspec.as_str().trim();
                if let Ok(val) = u32::from_str(valspec) {
                    ts_val = val;
                } else {
                    return Err(coalyxw!(W_CFG_INV_NUM_IN_INTVL, valspec.to_string()))
                }
            }
            let unit_spec = capts.get(2).unwrap().as_str();
            let unit_val = TimeSpanUnit::from_str(unit_spec);
            if unit_val.is_err() {
                return Err(coalyxw!(W_CFG_INV_UNIT_IN_INTVL, unit_spec.to_string()))
            }
            let unit_val = unit_val.unwrap();
            let ts = TimeSpan::new(unit_val, ts_val);
            let anchor_spec = capts.get(3).unwrap().as_str();
            let anchor_val = TimeStampAnchor::for_unit(anchor_spec, &unit_val)?;
            let intvl = Interval::anchored(ts, anchor_val);
            return Ok(RolloverCondition::TimeElapsed(intvl))
        }
        Err(coalyxw!(W_CFG_INV_ROVER_COND_PATTERN, s.to_string()))
    }
}

/// Policy for the rollover of output files
#[derive (Clone)]
pub(crate) struct RolloverPolicy {
    // policy name
    name: String,
    // rollover condition
    condition: RolloverCondition,
    // number of older files to keep before deletion
    keep_count: u32,
    // compression type for older files
    compression: CompressionAlgorithm
}
impl RolloverPolicy {
    /// Creates a rollover policy.
    /// Used for a policy defined in the policies.rollover section of the custom configuration file.
    ///
    /// # Arguments
    /// * `name` - the policy name
    /// * `condition` - the condition causing rollover
    /// * `keep_count` - the number of older files to keep before deletion
    /// * `compression` - the compression algorithm to use for older files
    #[inline]
    pub(crate) fn new(name: &str,
                      condition: RolloverCondition,
                      keep_count: u32,
                      compression: CompressionAlgorithm) -> RolloverPolicy {
        RolloverPolicy { name: name.to_string(), condition, keep_count, compression }
    }

    /// Returns the rollover condition for this policy.
    #[inline]
    pub(crate) fn condition(&self) -> &RolloverCondition { &self.condition }

    /// Returns the number of old files to keep for this policy.
    #[inline]
    pub(crate) fn keep_count(&self) -> u32 { self.keep_count }

    /// Returns the compression algorithm for this policy.
    #[inline]
    pub(crate) fn compression(&self) -> CompressionAlgorithm { self.compression }
}
impl Default for RolloverPolicy {
    fn default() -> Self {
        Self {
            name: DEFAULT_POLICY_NAME.to_string(),
            condition: RolloverCondition::default(),
            keep_count: 9,
            compression: CompressionAlgorithm::default()
        }
    }
}
impl Debug for RolloverPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "N:{}/COND:{:?}/KEEP:{}/CMPR:{:?}", self.name,
               self.condition, self.keep_count, self.compression)
    }
}

pub(crate) type RolloverPolicyMap = MapWithDefault<RolloverPolicy>;

/// File compression algorithms
#[derive (Clone, Copy, Eq, PartialEq)]
pub(crate) enum CompressionAlgorithm {
    None,
    Bzip2,
    Gzip,
    Lzma,
    Zip
}
impl CompressionAlgorithm {

    /// Returns the default compression algorithm (no compression)
    pub(crate) fn default() -> CompressionAlgorithm { CompressionAlgorithm::None }

    /// Returns the matching file extension for this compression algorithm, including leading dot
    pub(crate) fn file_extension(&self) -> &str {
        match self {
            CompressionAlgorithm::None => COMPR_EXT_NONE,
            CompressionAlgorithm::Bzip2 => COMPR_EXT_BZIP2,
            CompressionAlgorithm::Gzip => COMPR_EXT_GZIP,
            CompressionAlgorithm::Lzma => COMPR_EXT_LZMA,
            CompressionAlgorithm::Zip => COMPR_EXT_ZIP
        }
    }
    fn dump(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionAlgorithm::None => write!(f, "{}", COMPR_ALGO_NONE),
            CompressionAlgorithm::Bzip2 => write!(f, "{}", COMPR_ALGO_BZIP2),
            CompressionAlgorithm::Gzip => write!(f, "{}", COMPR_ALGO_GZIP),
            CompressionAlgorithm::Lzma => write!(f, "{}", COMPR_ALGO_LZMA),
            CompressionAlgorithm::Zip => write!(f, "{}", COMPR_ALGO_ZIP)
        }
    }
}
impl Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl Debug for CompressionAlgorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl FromStr for CompressionAlgorithm {
    type Err = CoalyException;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            COMPR_ALGO_NONE | "" => Ok(CompressionAlgorithm::None),
            COMPR_ALGO_BZIP2 => Ok(CompressionAlgorithm::Bzip2),
            COMPR_ALGO_GZIP => Ok(CompressionAlgorithm::Gzip),
            COMPR_ALGO_LZMA => Ok(CompressionAlgorithm::Lzma),
            COMPR_ALGO_ZIP => Ok(CompressionAlgorithm::Zip),
            _ => Err(coalyxw!(W_CFG_UNKNOWN_COMPR_ALGO, s.to_string()))
        }
    }
}

// Buffer flush condition names
const FLUSH_ON_ERROR: &str = "error";
const FLUSH_ON_WARNING: &str = "warning";
const FLUSH_ON_FULL: &str = "full";
const FLUSH_ON_ROLLOVER: &str = "rollover";
const FLUSH_ON_EXIT: &str = "exit";

// Compression algorithm names
const COMPR_ALGO_NONE: &str = "none";
const COMPR_ALGO_BZIP2: &str = "bzip2";
const COMPR_ALGO_GZIP: &str = "gzip";
const COMPR_ALGO_LZMA: &str = "lzma";
const COMPR_ALGO_ZIP: &str = "zip";

// File extensions for compression algorithms
const COMPR_EXT_NONE: &str = "";
const COMPR_EXT_BZIP2: &str = ".bz2";
const COMPR_EXT_GZIP: &str = ".gz";
#[cfg(not(windows))]
const COMPR_EXT_LZMA: &str = ".xz";
#[cfg(windows)]
const COMPR_EXT_LZMA: &str = ".7z";
const COMPR_EXT_ZIP: &str = ".zip";

// Rollover condition patterns
const ROVR_COND_NEVER: &str = "never";
const ROVR_COND_SIZE_PATTERN: &str = r"^\s*size\s*>\s*([0-9]+\s*[kmg]{0,1})\s*$";
const ROVR_COND_INTVL_PATTERN: &str =
    r"^\s*every\s+([0-9]+\s+){0,1}(second[s]{0,1}|minute[s]{0,1}|hour[s]{0,1}|day[s]{0,1})\s*$";
const ROVR_COND_INTVL_AT_PATTERN: &str =
    r"^\s*every\s+([0-9]+\s+){0,1}(hour[s]{0,1}|day[s]{0,1}|week[s]{0,1}|month[s]{0,1}|)\s+at\s+(.*)\s*$";
