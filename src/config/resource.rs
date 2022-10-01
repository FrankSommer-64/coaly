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

//! Descriptor structures for output resources.

use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use crate::collections::VecWithDefault;
use crate::record::RecordLevelId;

/// Default output file name
pub const DEFAULT_OUTPUT_FILE_NAME: &str = "coaly.log";

/// Kinds of output resources
#[derive (Clone, Copy)]
pub enum ResourceKind {
    // normal file
    PlainFile,
    // memory mapped file
    MemoryMappedFile,
    // standard output device (usually console)
    StdOut,
    // standard error device (usually console)
    StdErr,
    // syslog (Unix) or Event Logger (Windows)
    #[cfg(feature="net")]
    Syslog,
    // connection to remote trace server
    #[cfg(feature="net")]
    Network
}
impl ResourceKind {
    fn dump(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceKind::PlainFile => write!(f, "{}", RES_KIND_FILE),
            ResourceKind::MemoryMappedFile => write!(f, "{}", RES_KIND_MM_FILE),
            ResourceKind::StdOut => write!(f, "{}", RES_KIND_STDOUT),
            ResourceKind::StdErr => write!(f, "{}", RES_KIND_STDERR),
            #[cfg(feature="net")]
            ResourceKind::Syslog => write!(f, "{}", RES_KIND_SYSLOG),
            #[cfg(feature="net")]
            ResourceKind::Network => write!(f, "{}", RES_KIND_NETWORK)
        }
    }
}
impl Debug for ResourceKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl Display for ResourceKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { self.dump(f) }
}
impl FromStr for ResourceKind {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            RES_KIND_FILE => Ok(ResourceKind::PlainFile),
            RES_KIND_MM_FILE => Ok(ResourceKind::MemoryMappedFile),
            RES_KIND_STDOUT => Ok(ResourceKind::StdOut),
            RES_KIND_STDERR => Ok(ResourceKind::StdErr),
            #[cfg(feature="net")]
            RES_KIND_SYSLOG => Ok(ResourceKind::Syslog),
            #[cfg(feature="net")]
            RES_KIND_NETWORK => Ok(ResourceKind::Network),
            _ => Err(false)
        }
    }
}

/// Descriptor for the specific data of a file based output resource.
#[derive (Clone)]
pub struct FileResourceDesc {
    // name of file or memory mapped file
    file_name_spec: String,
    // file size in bytes, relevant for memory mapped file only
    file_size: usize,
    // optional rollover policy
    rollover_policy_name: Option<String>
}
impl FileResourceDesc {
    /// Creates a descriptor for the specific data of a file based output resource.
    /// Since rollover policies are referenced by name only in the resources section of the
    /// custom configuration file, this constructor provides string parameters for them.
    ///
    /// # Arguments
    /// * `file_name_spec` - the file name specification, may contain variables
    /// * `file_size` - file size in bytes, relevant for memory mapped file only
    /// * `rollover_policy_name` - the optional name of the rollover policy
    pub fn new(file_name_spec: &str, file_size: usize,
               rollover_policy_name: Option<&String>) -> FileResourceDesc {
        FileResourceDesc {
            file_name_spec: file_name_spec.to_string(),
            file_size,
            rollover_policy_name: rollover_policy_name.map(|n| n.to_string())
        }
    }

    /// Returns the file name specification
    #[inline]
    pub fn file_name_spec(&self) -> &String { &self.file_name_spec }

    /// Returns the file size
    #[inline]
    pub fn file_size(&self) -> usize { self.file_size }

    /// Returns the optional rollover policy name
    #[inline]
    pub fn rollover_policy_name(&self) -> &Option<String> { &self.rollover_policy_name }
}
impl Debug for FileResourceDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.rollover_policy_name.is_none() {
            return write!(f, "N:{}/SZ:{}/RP:-", self.file_name_spec, self.file_size)
        }
        write!(f, "N:{}/SZ:{}/RP:{}", self.file_name_spec, self.file_size,
               self.rollover_policy_name.as_ref().unwrap())
    }
}

/// Descriptor for the specific data of syslog service.
#[derive (Clone)]
#[cfg(feature="net")]
pub struct SyslogResourceDesc {
    // facility
    facility: u32,
    // URL where to send the trace records to
    remote_url: String,
    // optional URL to use to bind local socket
    local_url: Option<String>
}
#[cfg(feature="net")]
impl SyslogResourceDesc {
    /// Creates a descriptor for the specific data of syslog service.
    ///
    /// # Arguments
    /// * `facility` - facility
    /// * `remote_url` - the URL where to send the trace records to
    /// * `local_url` - the optional URL to use to bind local socket
    pub fn new(facility: u32, remote_url: &str, local_url: Option<&String>) -> SyslogResourceDesc {
        SyslogResourceDesc {
            facility,
            remote_url: remote_url.to_string(),
            local_url: local_url.map(|u| u.to_string())
        }
    }

    /// Returns the facility
    pub fn facility(&self) -> u32 { self.facility }

    /// Returns the remote URL
    pub fn remote_url(&self) -> &String { &self.remote_url }

    /// Returns the optional local URL
    pub fn local_url(&self) -> &Option<String> { &self.local_url }
}
#[cfg(feature="net")]
impl Debug for SyslogResourceDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.local_url.is_none() {
            return write!(f, "F:{}/R:{}/L:-", self.facility, self.remote_url)
        }
        write!(f, "F:{}/R:{}/L:{}", self.facility, self.remote_url,
               self.local_url.as_ref().unwrap())
    }
}

/// Descriptor for the specific data of a network output resource.
#[derive (Clone)]
#[cfg(feature="net")]
pub struct NetworkResourceDesc {
    // URL where to send the trace records to
    remote_url: String,
    // optional URL to use to bind local socket
    local_url: Option<String>
}
#[cfg(feature="net")]
impl NetworkResourceDesc {
    /// Creates a descriptor for the specific data of a network output resource.
    ///
    /// # Arguments
    /// * `remote_url` - the URL where to send the trace records to
    /// * `local_url` - the optional URL to use to bind local socket
    pub fn new(remote_url: &str, local_url: Option<&String>) -> NetworkResourceDesc {
        NetworkResourceDesc {
            remote_url: remote_url.to_string(),
            local_url: local_url.map(|u| u.to_string())
        }
    }

    /// Returns the remote URL
    pub fn remote_url(&self) -> &String { &self.remote_url }

    /// Returns the optional local URL
    pub fn local_url(&self) -> &Option<String> { &self.local_url }
}
#[cfg(feature="net")]
impl Debug for NetworkResourceDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.local_url.is_none() {
            return write!(f, "R:{}/L:-", self.remote_url)
        }
        write!(f, "R:{}/L:{}", self.remote_url, self.local_url.as_ref().unwrap())
    }
}

/// Enumeration for the specific data of output resources.
#[derive (Clone)]
pub enum SpecificResourceDesc {
    /// Data specific to file based resources
    File(FileResourceDesc),
    /// StdOut and StdErr don't need specific data
    Console,
    /// Data specific to syslog service
    #[cfg(feature="net")]
    Syslog(SyslogResourceDesc),
    /// Data specific to network resources
    #[cfg(feature="net")]
    Network(NetworkResourceDesc),
}
impl SpecificResourceDesc {
    /// Returns file specific data, if the resource is a file or memory mapped file.
    fn file_data(&self) -> Option<&FileResourceDesc> {
        match self {
            SpecificResourceDesc::File(d) => Some(d),
            _ => None
        }
    }

    /// Returns syslog specific data, if the resource is syslog service
    #[cfg(feature="net")]
    fn syslog_data(&self) -> Option<&SyslogResourceDesc> {
        match self {
            SpecificResourceDesc::Syslog(d) => Some(d),
            _ => None
        }
    }

    /// Returns network interface specific data, if the resource is a network interface
    #[cfg(feature="net")]
    fn network_data(&self) -> Option<&NetworkResourceDesc> {
        match self {
            SpecificResourceDesc::Network(d) => Some(d),
            _ => None
        }
    }
}
impl Debug for SpecificResourceDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecificResourceDesc::File(d) => d.fmt(f),
            #[cfg(feature="net")]
            SpecificResourceDesc::Syslog(d) => d.fmt(f),
            #[cfg(feature="net")]
            SpecificResourceDesc::Network(d) => d.fmt(f),
            _ => Ok(())
        }
    }
}

/// Descriptor for an output resource, reflects the specification in the custom configuration file.
#[derive (Clone)]
pub struct ResourceDesc {
    // the scope of the resource (application ID)
    scope: Vec<u32>,
    // the kind of the resource
    kind: ResourceKind,
    // bit mask with all record levels to be written to the resource
    levels: u32,
    // optional buffer policy
    buffer_policy_name: Option<String>,
    // optional output format name
    output_format_name: Option<String>,
    // resource specific data
    specific_data: SpecificResourceDesc
}
impl ResourceDesc {
    /// Creates a resource descriptor for a file based output resource.
    ///
    /// # Arguments
    /// * `scope` - the resource scope (application IDs)
    /// * `levels` - the bit mask with all record levels to be written to the resource
    /// * `buffer_policy_name` - the optional name of the buffer policy
    /// * `output_format_name` - the optional name of the output format to use
    /// * `file_name_spec` - the file name specification, may contain variables
    /// * `rollover_policy_name` - the optional name of the rollover policy
    pub fn for_plain_file(scope: &[u32],
                          levels: u32,
                          buffer_policy_name: Option<&String>,
                          output_format_name: Option<&String>,
                          file_name_spec: &str,
                          rollover_policy_name: Option<&String>) -> ResourceDesc {
        let f = FileResourceDesc::new(file_name_spec, 0, rollover_policy_name);
        ResourceDesc {
            scope: scope.to_vec(),
            kind: ResourceKind::PlainFile,
            levels,
            buffer_policy_name: buffer_policy_name.map(|n| n.to_string()),
            output_format_name: output_format_name.map(|n| n.to_string()),
            specific_data: SpecificResourceDesc::File(f)
        }
    }

    /// Creates a resource descriptor for a file based output resource.
    ///
    /// # Arguments
    /// * `scope` - the resource scope (application IDs)
    /// * `levels` - the bit mask with all record levels to be written to the resource
    /// * `output_format_name` - the optional name of the output format to use
    /// * `file_name_spec` - the file name specification, may contain variables
    /// * `file_size` - file size in bytes
    /// * `rollover_policy_name` - the optional name of the rollover policy
    pub fn for_mem_mapped_file(scope: &[u32],
                               levels: u32,
                               output_format_name: Option<&String>,
                               file_name_spec: &str,
                               file_size: usize,
                               rollover_policy_name: Option<&String>) -> ResourceDesc {
        let f = FileResourceDesc::new(file_name_spec, file_size, rollover_policy_name);
        ResourceDesc {
            scope: scope.to_vec(),
            kind: ResourceKind::MemoryMappedFile,
            levels,
            buffer_policy_name: None,
            output_format_name: output_format_name.map(|n| n.to_string()),
            specific_data: SpecificResourceDesc::File(f)
        }
    }

    /// Creates a resource descriptor for a console based output resource.
    ///
    /// # Arguments
    /// * `scope` - the resource scope (application IDs)
    /// * `kind` - the resource kind (stdout or stderr)
    /// * `levels` - the bit mask with all record levels to be written to the resource
    /// * `buffer_policy_name` - the optional name of the buffer policy
    /// * `output_format_name` - the optional name of the output format to use
    pub fn for_console(scope: &[u32],
                       kind: ResourceKind,
                       levels: u32,
                       buffer_policy_name: Option<&String>,
                       output_format_name: Option<&String>) -> ResourceDesc {
        ResourceDesc {
            scope: scope.to_vec(),
            kind,
            levels,
            buffer_policy_name: buffer_policy_name.map(|n| n.to_string()),
            output_format_name: output_format_name.map(|n| n.to_string()),
            specific_data: SpecificResourceDesc::Console
        }
    }

    /// Creates a resource descriptor for syslog.
    ///
    /// # Arguments
    /// * `scope` - the resource scope (application IDs)
    /// * `levels` - the bit mask with all record levels to be written to the resource
    /// * `buffer_policy_name` - the optional name of the buffer policy
    /// * `remote_url` - the URL where to send the trace records to
    /// * `local_url` - the optional URL to use to bind local socket
    #[cfg(feature="net")]
    pub fn for_syslog(scope: &[u32],
                      levels: u32,
                      buffer_policy_name: Option<&String>,
                      facility: u32,
                      remote_url: &str,
                      local_url: Option<&String>) -> ResourceDesc {
        let spd = SyslogResourceDesc::new(facility, remote_url, local_url);
        ResourceDesc {
            scope: scope.to_vec(),
            kind: ResourceKind::Syslog,
            levels,
            buffer_policy_name: buffer_policy_name.map(|n| n.to_string()),
            output_format_name: None,
            specific_data: SpecificResourceDesc::Syslog(spd)
        }
    }

    /// Creates a resource descriptor for a network resource.
    ///
    /// # Arguments
    /// * `scope` - the resource scope (application IDs)
    /// * `levels` - the bit mask with all record levels to be written to the resource
    /// * `buffer_policy_name` - the optional name of the buffer policy
    /// * `remote_url` - the URL where to send the trace records to
    /// * `local_url` - the optional URL to use to bind local socket
    #[cfg(feature="net")]
    pub fn for_network(scope: &[u32],
                       levels: u32,
                       buffer_policy_name: Option<&String>,
                       remote_url: &str,
                       local_url: Option<&String>) -> ResourceDesc {
        let spd = NetworkResourceDesc::new(remote_url, local_url);
        ResourceDesc {
            scope: scope.to_vec(),
            kind: ResourceKind::Network,
            levels,
            buffer_policy_name: buffer_policy_name.map(|n| n.to_string()),
            output_format_name: None,
            specific_data: SpecificResourceDesc::Network(spd)
        }
    }

    /// Returns resource kind of this resource
    #[inline]
    pub fn kind(&self) -> &ResourceKind { &self.kind }

    /// Returns record levels to be written to this resource
    #[inline]
    pub fn levels(&self) -> u32 { self.levels }

    /// Returns name of the buffer policy to use for this resource
    #[inline]
    pub fn buffer_policy_name(&self) -> &Option<String> { &self.buffer_policy_name }

    /// Returns name of the output format to use for this resource
    #[inline]
    pub fn output_format_name(&self) -> &Option<String> { &self.output_format_name }

    /// Returns file specific data, if the resource is a file or memory mapped file.
    #[inline]
    pub fn file_data(&self) -> Option<&FileResourceDesc> { self.specific_data.file_data() }

    /// Returns syslog specific data, if the resource is a network interface
    #[cfg(feature="net")]
    #[inline]
    pub fn syslog_data(&self) -> Option<&SyslogResourceDesc> {self.specific_data.syslog_data()}

    /// Returns network interface specific data, if the resource is a network interface
    #[cfg(feature="net")]
    #[inline]
    pub fn network_data(&self) -> Option<&NetworkResourceDesc> {self.specific_data.network_data()}

    /// Indicates whether this resource requires a fallback path, if there is a temporary problem
    pub fn may_need_fallback_path(&self) -> bool {
        match &self.kind {
            &ResourceKind::PlainFile | &ResourceKind::MemoryMappedFile => true,
            #[cfg(feature="net")]
            &ResourceKind::Network | &ResourceKind::Syslog => true,
            _ => false
        }
    }

    /// Indicates whether this resource requires an output path
    pub fn needs_output_path(&self) -> bool {
        match &self.kind {
            &ResourceKind::PlainFile | &ResourceKind::MemoryMappedFile => true,
            _ => false
        }
    }
}
impl Default for ResourceDesc {
    fn default() -> Self {
        ResourceDesc::for_plain_file(&[0], RecordLevelId::All as u32, None, None,
                                     DEFAULT_OUTPUT_FILE_NAME, None)
    }
}
impl Debug for ResourceDesc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut scope_buf = String::with_capacity(128);
        scope_buf.push('[');
        for aid in &self.scope {
            if scope_buf.len() > 1 { scope_buf.push(','); }
            scope_buf.push_str(&aid.to_string());
        }
        scope_buf.push(']');
        if self.buffer_policy_name.is_none() && self.output_format_name.is_none() {
            return write!(f, "S:{}/K:{:?}/L:{:b}/BP:-/OF:-/SD:{:?}", scope_buf, self.kind,
                          self.levels, self.specific_data)
        }
        if self.buffer_policy_name.is_none() {
            return write!(f, "S:{}/K:{:?}/L:{:b}/BP:-/OF:{}/SD:{:?}", scope_buf, self.kind,
                          self.levels, self.output_format_name.as_ref().unwrap(),
                          self.specific_data)
        }
        if self.output_format_name.is_none() {
            return write!(f, "S:{}/K:{:?}/L:{:b}/BP:{}/OF:-/SD:{:?}", scope_buf, self.kind,
                          self.levels, self.buffer_policy_name.as_ref().unwrap(),
                          self.specific_data)
        }
        write!(f, "S:{}/K:{:?}/L:{:b}/BP:{}/OF:{}/SD:{:?}", scope_buf,
               self.kind, self.levels, self.buffer_policy_name.as_ref().unwrap(),
               self.output_format_name.as_ref().unwrap(), self.specific_data)
    }
}

/// List with resource descriptors
pub(crate) type ResourceDescList = VecWithDefault<ResourceDesc>;
impl ResourceDescList {
    /// Indicates whether at least one of the resources requires a fallback path,
    /// if there is a temporary problem
    pub(crate) fn may_need_fallback_path(&self) -> bool {
        for rdesc in self.elements() {
            if rdesc.may_need_fallback_path() { return true}
        }
        false
    }

    /// Indicates whether at least one of the resources requires an output path
    pub fn needs_output_path(&self) -> bool {
        for rdesc in self.elements() {
            if rdesc.needs_output_path() { return true}
        }
        false
    }
}

// Names for all resource kinds
const RES_KIND_FILE: &str = "file";
const RES_KIND_MM_FILE: &str = "mmfile";
const RES_KIND_STDOUT: &str = "stdout";
const RES_KIND_STDERR: &str = "stderr";

#[cfg(feature="net")]
const RES_KIND_SYSLOG: &str = "syslog";

#[cfg(feature="net")]
const RES_KIND_NETWORK: &str = "network";
