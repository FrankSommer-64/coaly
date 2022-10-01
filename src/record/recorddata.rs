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

//! Types and functionality around log or trace records.

use chrono::{DateTime, Local, TimeZone};
use crate::observer::ObserverData;
use super::{RecordLevelId, RecordTrigger};

#[cfg(feature="net")]
use crate::CoalyException;

#[cfg(feature="net")]
use crate::net::serializable::Serializable;

#[cfg(feature="net")]
use std::convert::From;

/// Trait to access data of both local and remote log or trace record
#[cfg(not(feature="net"))]
pub trait RecordData<'a> {
    // Returns the thread ID
    fn thread_id(&self) -> u64;

    /// Returns the thread name
    fn thread_name(&self) -> &str;

    /// Returns the seconds since epoch when the record was created
    fn ts_secs(&self) -> i64;

    /// Returns the exact nano seconds within the second when the record was created
    fn ts_nano_secs(&self) -> u32;

    /// Returns the record level
    fn level(&self) -> RecordLevelId;

    /// Returns the record trigger
    fn trigger(&self) -> RecordTrigger;

    /// Returns the source file name
    fn source_fn(&self) -> &str;

    /// Returns the line number in the source file
    fn line_nr(&self) -> &Option<u32>;

    /// Returns the record message
    fn message(&self) -> &Option<String>;

    /// Returns the observer name
    fn observer_name(&self) -> &Option<String>;

    /// Returns the observer value
    fn observer_value(&self) -> &Option<String>;

    /// Returns the observer ID
    fn observer_id(&self) -> u64;

    /// Returns the timestamp when the record was issued as local datetime.
    fn timestamp(&self) -> DateTime<Local>;
}
#[cfg(feature="net")]
pub trait RecordData<'a> : Serializable<'a> {
    // Returns the thread ID
    fn thread_id(&self) -> u64;

    /// Returns the thread name
    fn thread_name(&self) -> &str;

    /// Returns the seconds since epoch when the record was created
    fn ts_secs(&self) -> i64;

    /// Returns the exact nano seconds within the second when the record was created
    fn ts_nano_secs(&self) -> u32;

    /// Returns the record level
    fn level(&self) -> RecordLevelId;

    /// Returns the record trigger
    fn trigger(&self) -> RecordTrigger;

    /// Returns the source file name
    fn source_fn(&self) -> &str;

    /// Returns the line number in the source file
    fn line_nr(&self) -> &Option<u32>;

    /// Returns the record message
    fn message(&self) -> &Option<String>;

    /// Returns the observer name
    fn observer_name(&self) -> &Option<String>;

    /// Returns the observer value
    fn observer_value(&self) -> &Option<String>;

    /// Returns the observer ID
    fn observer_id(&self) -> u64;

    /// Returns the timestamp when the record was issued as local datetime.
    fn timestamp(&self) -> DateTime<Local>;
}

/// Log or trace record within a process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalRecordData {
    common_data: CommonRecordData,
    source_fn: &'static str,
}
impl LocalRecordData {
    /// Creates local record data for a plain output message to be written to output
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `level` - the record level
    /// * `file_name` - the name of the source code file, where the message was issued
    /// * `line_nr` - the line number in the source code file, where the message was issued
    /// * `msg` - the log or trace message
    pub(crate) fn for_write(thread_id: u64,
                            thread_name: &str,
                            level: RecordLevelId,
                            file_name: &'static str,
                            line_nr: u32,
                            msg: &str) -> LocalRecordData {
        LocalRecordData {
            common_data: CommonRecordData::for_write(thread_id, thread_name, level, line_nr, msg),
            source_fn: file_name
        }
    }

    /// Creates local record data for a plain output message to be written to output
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `level` - the record level
    /// * `file_name` - the name of the source code file, where the message was issued
    /// * `line_nr` - the line number in the source code file, where the message was issued
    /// * `msg` - the log or trace message
    pub(crate) fn for_write_obs(thread_id: u64,
                                thread_name: &str,
                                observer_data: &ObserverData,
                                file_name: &'static str,
                                line_nr: u32,
                                msg: &str) -> LocalRecordData {
        LocalRecordData {
            common_data: CommonRecordData::for_write_obs(thread_id, thread_name,
                                                   observer_data, line_nr, msg),
            source_fn: file_name
        }
    }

    /// Creates record data for the creation of a Coaly function, module or
    /// user defined observer structure.
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer` - the observer's descriptor data
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub(crate) fn for_create(thread_id: u64,
                             thread_name: &str,
                             observer: &ObserverData,
                             line_nr: u32) -> LocalRecordData {
        LocalRecordData {
            common_data: CommonRecordData::for_create(thread_id, thread_name, observer, line_nr),
            source_fn: observer.file_name()
        }
    }

    /// Creates record data for the deletion of a Coaly function, module or
    /// user defined observer structure.
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer` - the observer's descriptor
    pub(crate) fn for_drop(thread_id: u64,
                           thread_name: &str,
                           observer: &ObserverData) -> LocalRecordData {
        LocalRecordData {
            common_data: CommonRecordData::for_drop(thread_id, thread_name, observer),
            source_fn: observer.file_name()
        }
    }
}
impl<'a> RecordData<'a> for LocalRecordData {
    /// Returns the thread ID
    #[inline]
    fn thread_id(&self) -> u64 { self.common_data.thread_id() }

    /// Returns the thread name
    #[inline]
    fn thread_name(&self) -> &str { self.common_data.thread_name() }

    /// Returns the seconds since epoch when the record was created
    #[inline]
    fn ts_secs(&self) -> i64 { self.common_data.ts_secs() }

    /// Returns the exact nano seconds within the second when the record was created
    #[inline]
    fn ts_nano_secs(&self) -> u32 { self.common_data.ts_nano_secs() }

    /// Returns the record level
    #[inline]
    fn level(&self) -> RecordLevelId { self.common_data.level() }

    /// Returns the record trigger
    #[inline]
    fn trigger(&self) -> RecordTrigger { self.common_data.trigger() }

    /// Returns the source file name
    #[inline]
    fn source_fn(&self) -> &str { self.source_fn }

    /// Returns the line number in the source file
    #[inline]
    fn line_nr(&self) -> &Option<u32> { self.common_data.line_nr() }

    /// Returns the record message
    #[inline]
    fn message(&self) -> &Option<String> { self.common_data.message() }

    /// Returns the observer name
    #[inline]
    fn observer_name(&self) -> &Option<String> { self.common_data.observer_name() }

    /// Returns the observer value
    #[inline]
    fn observer_value(&self) -> &Option<String> { self.common_data.observer_value() }

    /// Returns the observer ID
    #[inline]
    fn observer_id(&self) -> u64 { self.common_data.observer_id() }

    /// Returns the timestamp when the record was issued as local datetime.
    #[inline]
    fn timestamp(&self) -> DateTime<Local> { self.common_data.timestamp() }
}
#[cfg(feature="net")]
impl<'a> Serializable<'a> for LocalRecordData {
    fn serialized_size(&self) -> usize {
        self.common_data.serialized_size() +
        self.source_fn.serialized_size()
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let mut n = self.common_data.serialize_to(buffer);
        n += self.source_fn.serialize_to(buffer);
        n
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        let common_data = CommonRecordData::deserialize_from(buffer)?;
        // local record data is not deserialized on server side, so we skip messing around
        // with lifetimes for source file name
        // TODO mess around with source file name because needed in buffering for network resources
        let source_fn = "";
        Ok(LocalRecordData { common_data, source_fn })
    }
}

/// Log or trace record from a remote client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteRecordData {
    common_data: CommonRecordData,
    source_fn: String
}
impl<'a> RecordData<'a> for RemoteRecordData {
    /// Returns the thread ID
    #[inline]
    fn thread_id(&self) -> u64 { self.common_data.thread_id() }

    /// Returns the thread name
    #[inline]
    fn thread_name(&self) -> &str { self.common_data.thread_name() }

    /// Returns the seconds since epoch when the record was created
    #[inline]
    fn ts_secs(&self) -> i64 { self.common_data.ts_secs() }

    /// Returns the exact nano seconds within the second when the record was created
    #[inline]
    fn ts_nano_secs(&self) -> u32 { self.common_data.ts_nano_secs() }

    /// Returns the record level
    #[inline]
    fn level(&self) -> RecordLevelId { self.common_data.level() }

    /// Returns the record trigger
    #[inline]
    fn trigger(&self) -> RecordTrigger { self.common_data.trigger() }

    /// Returns the source file name
    #[inline]
    fn source_fn(&self) -> &str { &self.source_fn }

    /// Returns the line number in the source file
    #[inline]
    fn line_nr(&self) -> &Option<u32> { self.common_data.line_nr() }

    /// Returns the record message
    #[inline]
    fn message(&self) -> &Option<String> { self.common_data.message() }

    /// Returns the observer name
    #[inline]
    fn observer_name(&self) -> &Option<String> { self.common_data.observer_name() }

    /// Returns the observer value
    #[inline]
    fn observer_value(&self) -> &Option<String> { self.common_data.observer_value() }

    /// Returns the observer ID
    #[inline]
    fn observer_id(&self) -> u64 { self.common_data.observer_id() }

    /// Returns the timestamp when the record was issued as local datetime.
    #[inline]
    fn timestamp(&self) -> DateTime<Local> { self.common_data.timestamp() }
}
#[cfg(feature="net")]
impl<'a> Serializable<'a> for RemoteRecordData {
    fn serialized_size(&self) -> usize {
        self.common_data.serialized_size() +
        self.source_fn.serialized_size()
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let mut n = self.common_data.serialize_to(buffer);
        n += self.source_fn.serialize_to(buffer);
        n
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        let common_data = CommonRecordData::deserialize_from(buffer)?;
        let buf = &buffer[common_data.serialized_size()..];
        let source_fn = String::deserialize_from(buf)?;
        Ok(RemoteRecordData { common_data, source_fn })
    }
}
#[cfg(feature="net")]
impl From<LocalRecordData> for RemoteRecordData {
    /// Create remote record from local
    fn from(local: LocalRecordData) -> Self {
        RemoteRecordData {
            common_data: local.common_data,
            source_fn: local.source_fn.to_string()
        }
    }
}

/// Data common to both local and remote log or trace record.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CommonRecordData {
    thread_id: u64,
    thread_name: String,
    ts_secs: i64,
    ts_nano_secs: u32,
    level: RecordLevelId,
    trigger: RecordTrigger,
    line_nr: Option<u32>,
    message: Option<String>,
    observer_name: Option<String>,
    observer_value: Option<String>,
    observer_id: u64
}
impl CommonRecordData {
    /// Creates record data for a plain output message to be written to output
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `level` - the record level
    /// * `line_nr` - the line number in the source code file, where the message was issued
    /// * `msg` - the log or trace message
    pub(crate) fn for_write(thread_id: u64,
                            thread_name: &str,
                            level: RecordLevelId,
                            line_nr: u32,
                            msg: &str) -> CommonRecordData {
        let now = Local::now();
        CommonRecordData {
            thread_id,
            thread_name: thread_name.to_string(),
            ts_secs: now.timestamp(),
            ts_nano_secs: now.timestamp_subsec_nanos(),
            level,
            trigger: RecordTrigger::Message,
            line_nr: Option::from(line_nr),
            message: Option::from(msg.to_string()),
            observer_name: None,
            observer_value: None,
            observer_id: 0
        }
    }

    /// Creates record data for a plain output message to be written to output
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `level` - the record level
    /// * `line_nr` - the line number in the source code file, where the message was issued
    /// * `msg` - the log or trace message
    pub(crate) fn for_write_obs(thread_id: u64,
                                thread_name: &str,
                                observer_data: &ObserverData,
                                line_nr: u32,
                                msg: &str) -> CommonRecordData {
        let now = Local::now();
        CommonRecordData {
            thread_id,
            thread_name: thread_name.to_string(),
            ts_secs: now.timestamp(),
            ts_nano_secs: now.timestamp_subsec_nanos(),
            level: RecordLevelId::Object,
            trigger: RecordTrigger::Message,
            line_nr: Option::from(line_nr),
            message: Option::from(msg.to_string()),
            observer_name: Option::from(observer_data.name().clone()),
            observer_value: observer_data.value().clone(),
            observer_id: observer_data.id()
        }
    }

    /// Creates record data for the creation of a Coaly function, module or
    /// user defined observer structure.
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer` - the observer's descriptor data
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub(crate) fn for_create(thread_id: u64,
                             thread_name: &str,
                             observer: &ObserverData,
                             line_nr: u32) -> CommonRecordData {
        let now = Local::now();
        CommonRecordData {
            thread_id,
            thread_name: thread_name.to_string(),
            ts_secs: now.timestamp(),
            ts_nano_secs: now.timestamp_subsec_nanos(),
            level: RecordLevelId::from(*observer.kind() as u32),
            trigger: RecordTrigger::ObserverCreated,
            line_nr: Option::from(line_nr),
            message: observer.value().clone(),
            observer_name: Option::from(observer.name().to_string()),
            observer_value: observer.value().clone(),
            observer_id: observer.id()
        }
    }

    /// Creates record data for the deletion of a Coaly function, module or
    /// user defined observer structure.
    /// 
    /// # Arguments
    /// * `thread_id` - the caller thread's ID
    /// * `thread_name` - the caller thread's name
    /// * `observer` - the observer's descriptor
    pub(crate) fn for_drop(thread_id: u64,
                           thread_name: &str,
                           observer: &ObserverData) -> CommonRecordData {
        let now = Local::now();
        CommonRecordData {
            thread_id,
            thread_name: thread_name.to_string(),
            ts_secs: now.timestamp(),
            ts_nano_secs: now.timestamp_subsec_nanos(),
            level: RecordLevelId::from(*observer.kind() as u32),
            trigger: RecordTrigger::ObserverDropped,
            line_nr: None,
            message: observer.value().clone(),
            observer_name: Option::from(observer.name().to_string()),
            observer_value: observer.value().clone(),
            observer_id: observer.id()
        }
    }

    /// Returns the thread ID
    #[inline]
    pub(crate) fn thread_id(&self) -> u64 { self.thread_id }

    /// Returns the thread name
    #[inline]
    pub(crate) fn thread_name(&self) -> &str { &self.thread_name }

    /// Returns the seconds since epoch when the record was created
    #[inline]
    pub(crate) fn ts_secs(&self) -> i64 { self.ts_secs }

    /// Returns the exact nano seconds within the second when the record was created
    #[inline]
    pub(crate) fn ts_nano_secs(&self) -> u32 { self.ts_nano_secs }

    /// Returns the record level
    #[inline]
    pub(crate) fn level(&self) -> RecordLevelId { self.level }

    /// Returns the record trigger
    #[inline]
    pub(crate) fn trigger(&self) -> RecordTrigger { self.trigger }

    /// Returns the line number in the source file
    #[inline]
    pub(crate) fn line_nr(&self) -> &Option<u32> { &self.line_nr }

    /// Returns the record message
    #[inline]
    pub(crate) fn message(&self) -> &Option<String> { &self.message }

    /// Returns the observer name
    #[inline]
    pub(crate) fn observer_name(&self) -> &Option<String> { &self.observer_name }

    /// Returns the observer value
    #[inline]
    pub(crate) fn observer_value(&self) -> &Option<String> { &self.observer_value }

    /// Returns the observer ID
    #[inline]
    pub(crate) fn observer_id(&self) -> u64 { self.observer_id }

    /// Returns the timestamp when the record was issued as local datetime.
    #[inline]
    pub(crate) fn timestamp(&self) -> DateTime<Local> {
        Local.timestamp(self.ts_secs, self.ts_nano_secs)
    }
}
#[cfg(feature="net")]
impl<'a> Serializable<'a> for CommonRecordData {
    fn serialized_size(&self) -> usize {
        self.thread_id.serialized_size() +
        self.thread_name.serialized_size() +
        self.ts_secs.serialized_size() +
        self.ts_nano_secs.serialized_size() +
        (self.level as u32).serialized_size() +
        (self.trigger as u32).serialized_size() +
        self.line_nr.serialized_size() +
        self.message.serialized_size() +
        self.observer_name.serialized_size() +
        self.observer_value.serialized_size() +
        self.observer_id.serialized_size()
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let mut n = self.thread_id.serialize_to(buffer);
        n += self.thread_name.serialize_to(buffer);
        n += self.ts_secs.serialize_to(buffer);
        n += self.ts_nano_secs.serialize_to(buffer);
        n += (self.level as u32).serialize_to(buffer);
        n += (self.trigger as u32).serialize_to(buffer);
        n += self.line_nr.serialize_to(buffer);
        n += self.message.serialize_to(buffer);
        n += self.observer_name.serialize_to(buffer);
        n += self.observer_value.serialize_to(buffer);
        n += self.observer_id.serialize_to(buffer);
        n
    }
    fn deserialize_from(buffer: &'a [u8]) -> Result<Self, CoalyException> {
        let thread_id = u64::deserialize_from(buffer)?;
        let buf = &buffer[thread_id.serialized_size()..];
        let thread_name = String::deserialize_from(buf)?;
        let buf = &buf[thread_name.serialized_size()..];
        let ts_secs = i64::deserialize_from(buf)?;
        let buf = &buf[ts_secs.serialized_size()..];
        let ts_nano_secs = u32::deserialize_from(buf)?;
        let buf = &buf[ts_nano_secs.serialized_size()..];
        let level = u32::deserialize_from(buf)?;
        let buf = &buf[level.serialized_size()..];
        let trigger = u32::deserialize_from(buf)?;
        let buf = &buf[trigger.serialized_size()..];
        let line_nr = Option::<u32>::deserialize_from(buf)?;
        let buf = &buf[line_nr.serialized_size()..];
        let message = Option::<String>::deserialize_from(buf)?;
        let buf = &buf[message.serialized_size()..];
        let observer_name = Option::<String>::deserialize_from(buf)?;
        let buf = &buf[observer_name.serialized_size()..];
        let observer_value = Option::<String>::deserialize_from(buf)?;
        let buf = &buf[observer_value.serialized_size()..];
        let observer_id = u64::deserialize_from(buf)?;
        Ok(CommonRecordData {
            thread_id,
            thread_name,
            ts_secs,
            ts_nano_secs,
            level: RecordLevelId::from(level),
            trigger: RecordTrigger::from(trigger),
            line_nr,
            message,
            observer_name,
            observer_value,
            observer_id
        })
    }
}

#[cfg(all(test, net))]
mod tests {
    use super::{LocalRecordData, CommonRecordData, RemoteRecordData};
    use crate::record::{RecordLevelId, RecordTrigger};
    use crate::record::tests::check_serialization;

    fn min_recdata() -> CommonRecordData {
        CommonRecordData {
            thread_id: 1234,
            thread_name: String::from(""),
            ts_secs: 9999,
            ts_nano_secs: 0,
            level: RecordLevelId::Error,
            trigger: RecordTrigger::ObserverCreated,
            line_nr: None,
            message: None,
            observer_name: None,
            observer_value: None,
            observer_id: 6543
        }
    }

    fn max_recdata() -> CommonRecordData {
        CommonRecordData {
            thread_id: 1234,
            thread_name: String::from("mythread"),
            ts_secs: 9999,
            ts_nano_secs: 0,
            level: RecordLevelId::Error,
            trigger: RecordTrigger::ObserverCreated,
            line_nr: Some(393),
            message: Some(String::from("blabla")),
            observer_name: Some(String::from("myfunc")),
            observer_value: Some(String::from("myvalue")),
            observer_id: 6543
        }
    }

    #[test]
    fn test_serialize_record_data() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let recdata_min = min_recdata();
        let recdata_max = max_recdata();
        check_serialization::<CommonRecordData>(&recdata_min, 48, &mut buffer);
        check_serialization::<CommonRecordData>(&recdata_max, 103, &mut buffer);
    }

    #[test]
    fn test_serialize_local_record_data() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let local_recdata_min = LocalRecordData {
            common_data: min_recdata(),
            source_fn: "",
        };
        check_serialization::<LocalRecordData>(&local_recdata_min, 56, &mut buffer);
        let local_recdata_max = LocalRecordData {
            common_data: max_recdata(),
            source_fn: ""
        };
        check_serialization::<LocalRecordData>(&local_recdata_max, 111, &mut buffer);
    }

    #[test]
    fn test_serialize_remote_record_data() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        let remote_recdata_min = RemoteRecordData {
            common_data: min_recdata(),
            source_fn: String::from("")
        };
        check_serialization::<RemoteRecordData>(&remote_recdata_min, 56, &mut buffer);
        let remote_recdata_max = RemoteRecordData {
            common_data: max_recdata(),
            source_fn: String::from("test.rs")
        };
        check_serialization::<RemoteRecordData>(&remote_recdata_max, 118, &mut buffer);
    }
}