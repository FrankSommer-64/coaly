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

//! Output resources.

use chrono::{DateTime, Local};
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;
use crate::coalyxe;
use crate::config::Configuration;
use crate::config::resource::{ResourceDesc, ResourceKind};
use crate::errorhandling::*;
use crate::policies::*;
use crate::record::originator::OriginatorInfo;
use crate::record::recorddata::RecordData;
use super::formatspec::FormatSpec;
use super::outputformat::OutputFormat;
use super::recordbuffer::RecordBuffer;

mod file;
mod rollover;
use file::{FileData, FileTemplateData, MemMappedFileData, MemMappedFileTemplateData};

#[cfg(feature="net")]
pub(crate) mod network;
#[cfg(feature="net")]
pub(crate) mod syslog;
#[cfg(feature="net")]
use network::NetworkData;
#[cfg(feature="net")]
use syslog::SyslogData;
#[cfg(feature="net")]
use crate::config::resource::{NetworkResourceDesc, SyslogResourceDesc};
#[cfg(feature="net")]
use crate::net::{parse_url, PeerAddr};

pub(crate) type ResourceRef = Rc<RefCell<Resource>>;

/// Logical output resource, a physical resource enhanced with common attributes needed for all
/// kinds of physical resources.
pub(crate) struct Resource {
    // bit mask with all record levels associated with the resource
    levels: u32,
    // memory buffer policy
    buffer_policy: BufferPolicy,
    // memory buffer
    buffer: Option<RecordBuffer>,
    // output format for log and trace records as defined in configuration, i.e. not optimized for
    // a specific originator and thread
    output_format_template: OutputFormat,
    // physical resource
    physical_resource: PhysicalResource,
    // buffer for local record serialization
    #[cfg(feature="net")]
    serialization_buffer: Option<Vec<u8>>
}
impl Resource {
    /// Creates a resource from the system configuration.
    /// Invoked by inventory upon application start to determine all resources serving as
    /// templates for the application's threads.
    ///
    /// # Arguments
    /// * `desc` - the resource descriptor
    /// * `config` - the system configuration
    /// * `orig_info` - information about local host and process
    pub(crate) fn from_config(desc: &ResourceDesc,
                              config: &Configuration,
                              #[cfg(feature="net")]
                              orig_info: &OriginatorInfo) -> Result<Resource, CoalyException> {
        let buf_pol = config.buffer_policy(desc.buffer_policy_name());
        let levels = config.system_properties().record_levels();
        let ofmt_desc = config.output_format(desc.output_format_name());
        let ofmt = OutputFormat::from_desc(ofmt_desc, config.date_time_formats(), levels);
        let output_dir = Path::new(config.system_properties().output_path());
        match desc.kind() {
            ResourceKind::PlainFile => {
                let fdata = desc.file_data().unwrap();
                let rov_pol = config.rollover_policy(fdata.rollover_policy_name());
                let name_spec = FormatSpec::from_str(fdata.file_name_spec()).unwrap();
                Resource::plain_file(desc.levels(), &output_dir, name_spec,
                                     buf_pol, rov_pol, ofmt)
            },
            ResourceKind::MemoryMappedFile => {
                let fdata = desc.file_data().unwrap();
                let rov_pol = config.rollover_policy(fdata.rollover_policy_name());
                let name_spec = FormatSpec::from_str(fdata.file_name_spec()).unwrap();
                let fsize = fdata.file_size();
                Resource::mm_file(desc.levels(), &output_dir, name_spec, fsize,
                                  buf_pol, rov_pol, ofmt)
            },
            ResourceKind::StdOut => Ok(Resource::stdout(desc.levels(), buf_pol, ofmt)),
            ResourceKind::StdErr => Ok(Resource::stderr(desc.levels(), buf_pol, ofmt)),
            #[cfg(feature="net")]
            ResourceKind::Syslog => {
                let ldata = desc.syslog_data().unwrap();
                Resource::syslog(desc.levels(), ldata, buf_pol, orig_info, ofmt)
            },
            #[cfg(feature="net")]
            ResourceKind::Network => {
                let ndata = desc.network_data().unwrap();
                Resource::network(desc.levels(), ndata, buf_pol, orig_info, ofmt)
            }
        }
    }

    /// Writes a log or trace record to this resource.
    /// 
    /// # Arguments
    /// * `record` - the log or trace record
    /// * `output_format` - the output format to use
    /// * `use_buffer` - indicates whether to buffer the record in memory instead of writing to
    ///                  physical resource
    /// 
    /// # Errors
    /// Returns an error structure if the write operation fails
    pub(crate) fn write(&mut self,
                        record: &dyn RecordData,
                        output_format: &OutputFormat,
                        use_buffer: bool) -> Result<(), Vec<CoalyException>> {
        // if record level is not associated with this resource, we're finished
        if self.levels & record.level() as u32  == 0 { return Ok(()) }
        // without buffering, write record to physical resource
        if ! use_buffer { return self.write_through(record, output_format) }
        // write record to memory buffer
        #[cfg(not(feature="net"))]
        let msg = output_format.apply_to(record);
        #[cfg(not(feature="net"))]
        let bytes_to_write = msg.len();
        #[cfg(feature="net")]
        let msg: Option<String> = if self.physical_resource.is_proxy() { None }
                                  else { Some(output_format.apply_to(record)) };
        #[cfg(feature="net")]
        let bytes_to_write = if msg.is_some() { msg.as_ref().unwrap().len() } 
                             else { record.serialized_size() };
        if self.buffer.is_none() {
            // buffer doesn't exist, allocate it
            self.buffer = Some(RecordBuffer::in_memory(self.buffer_policy.content_size(),
                                                       self.buffer_policy.index_size(),
                                                       self.buffer_policy.max_record_length()));
        } else {
            // eventually flush buffer before write operation
            if self.buffer_flush_required_upon(record.level() as u32) {
                // buffer needs to be flushed, because we got a corresponding record level
                self.flush_buffer()?;
                // in this case, we also write the current record to physical resource
                #[cfg(feature="net")]
                if let Some(ref plain_msg) = msg {
                    return self.physical_resource.write_record(&plain_msg)
                } else {
                    return self.physical_resource.send_record(record)
                }
                #[cfg(not(feature="net"))]
                return self.physical_resource.write_record(&msg)
            }
            if self.buffer_flush_required_upon(BufferFlushCondition::Full as u32) {
                if ! self.buffer.as_mut().unwrap().can_lossless_hold(bytes_to_write) {
                    self.flush_buffer()?;
                }
            }
        }
        #[cfg(not(feature="net"))]
        return Ok(self.buffer.as_mut().unwrap().write(&msg));
        #[cfg(feature="net")]
        if let Some(plain_msg) = msg {
            return Ok(self.buffer.as_mut().unwrap().write(&plain_msg))
        } else {
            if bytes_to_write > self.buffer.as_mut().unwrap().max_rec_len() {
                return self.physical_resource.send_record(record)
            } else {
                if self.serialization_buffer.is_none() {
                    self.serialization_buffer = Some(Vec::<u8>::with_capacity(bytes_to_write));
                }
                let buf = self.serialization_buffer.as_mut().unwrap();
                if bytes_to_write > buf.capacity() { buf.reserve(bytes_to_write - buf.capacity()); }
                record.serialize_to(buf);
                let buf = self.buffer.as_mut().unwrap();
                return Ok(buf.cache(self.serialization_buffer.as_ref().unwrap().as_slice()))
            }
        }
    }

    /// Writes a log or trace record unconditional to physical resource.
    /// 
    /// # Arguments
    /// * `record` - the log or trace record
    /// * `output_format` - the output format to use
    /// 
    /// # Errors
    /// Returns an error structure if the write operation fails
    fn write_through(&mut self,
                     record: &dyn RecordData,
                     output_format: &OutputFormat) -> Result<(), Vec<CoalyException>> {
        #[cfg(feature="net")]
        if self.physical_resource.is_proxy() {
            return self.physical_resource.send_record(record)
        }
        let msg = output_format.apply_to(record);
        self.physical_resource.write_record(&msg)
    }

    /// Closes the resource.
    /// Flushes buffer to physical resource, if configured for flush on exit.
    /// Closes physical resource, if applicable.
    pub(crate) fn close(&mut self) {
        let _ = self.flush_buffer();
        self.physical_resource.close();
    }

    /// Performs a rollover of a file based resource if the rollover is due.
    /// 
    /// # Arguments
    /// * `now` - current timestamp
    pub(crate) fn rollover_if_due(&mut self,
                                  now: &DateTime<Local>) -> Result<(), CoalyException> {
        self.physical_resource.rollover_if_due(now)
    }

    /// Indicates, whether this resource is specific for an originator.
    #[inline]
    pub(crate) fn is_originator_specific(&self) -> bool {
        self.physical_resource.is_originator_specific()
    }

    /// Indicates, whether this resource is specific for a thread.
    #[inline]
    pub(crate) fn is_thread_specific(&self) -> bool {
        self.physical_resource.is_thread_specific()
    }

    /// Returns the output format for this resource, optimized for the specified originator thread.
    /// file name specificatons with the given values.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    /// * `thread_id` - thread ID
    /// * `thread_name` - thread name
    pub(crate) fn optimized_output_format(&self,
                                          orig_info: &OriginatorInfo,
                                          thread_id: u64,
                                          thread_name: &str) -> OutputFormat {
        self.output_format_template.optimized_for(orig_info, thread_id, thread_name)
    }

    /// Returns the name specification for this resource, optimized for the specified originator.
    /// Returns None, if the resource is not backed by a file template.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    pub(crate) fn originator_optimized_name(&self,
                                            orig_info: &OriginatorInfo) -> Option<FormatSpec> {
        self.physical_resource.originator_optimized_name(orig_info)
    }

    /// Returns the name specification for this resource, optimized for the specified thread.
    /// Returns None, if the resource is not backed by a file template.
    /// 
    /// # Arguments
    /// * `thread_id` - thread ID
    /// * `thread_name` - thread name
    pub(crate) fn thread_optimized_name(&self,
                                        thread_id: u64,
                                        thread_name: &str) -> Option<FormatSpec> {
        self.physical_resource.thread_optimized_name(thread_id, thread_name)
    }

    /// Updates the file name specification with the given value.
    /// If the resource is not backed by a file template, a call to this method has no effect.
    /// 
    /// # Arguments
    /// * `name_spec` - the optimized name specification
    pub(crate) fn use_optimized_name(&mut self, name_spec: FormatSpec) {
        self.physical_resource.use_optimized_name(name_spec);
    }

    /// Creates a thread specific resource from this template.
    ///
    /// # Arguments
    /// * `name_spec` - file name specification, optimized for thread
    /// 
    /// # Return values
    /// thread specific resource, if this resource is a template; otherwise **None**
    pub(crate) fn for_thread(&self,
                             name_spec: FormatSpec) -> Result<Resource, CoalyException> {
        let phy_res = self.physical_resource.for_thread(name_spec)?;
        Ok(Resource { levels: self.levels,
                      buffer: None,
                      buffer_policy: self.buffer_policy.clone(),
                      output_format_template: self.output_format_template.clone(),
                      physical_resource: phy_res,
                      #[cfg(feature="net")]
                      serialization_buffer: None
                    })
    }

    /// Creates an originator specific resource from this template.
    /// The resource created may be a template or final, depending on whether the given name
    /// is thread-specific or not.
    ///
    /// # Arguments
    /// * `name_spec` - file name specification, optimized for originator
    /// 
    /// # Return values
    /// originator specific resource, if this resource is a template; otherwise **None**
    #[cfg(feature="net")]
    pub(crate) fn for_originator(&self,
                                 name_spec: FormatSpec) -> Result<Resource, CoalyException> {
        let phy_res = self.physical_resource.for_originator(name_spec)?;
        Ok(Resource { levels: self.levels,
                      buffer: None,
                      buffer_policy: self.buffer_policy.clone(),
                      output_format_template: self.output_format_template.clone(),
                      physical_resource: phy_res,
                      #[cfg(feature="net")]
                      serialization_buffer: None
                   })
    }

    /// Indicates whether the memory buffer must be flushed upon the specified event.
    /// 
    /// # Arguments
    /// * `event` - the event, a bit for a record level or another event
    #[inline]
    fn buffer_flush_required_upon(&self, event: u32) -> bool {
        self.buffer_policy.flush_conditions() & event != 0
    }

    /// Creates a plain file based resource or resource template.
    /// A resource template is created, if the file is thread specific, otherwise a directly
    /// usable file or memory mapped file.
    ///
    /// # Arguments
    /// * `levels` - the bit mask with all record levels associated with the resource
    /// * `output_dir` - the output directory
    /// * `name_spec` - the file name specification
    /// * `buffer_policy` - the buffer policy
    /// * `rollover_policy` - the rollover policy
    /// * `output_format_template` - the output format template
    fn plain_file(levels: u32,
                  output_dir: &Path,
                  name_spec: FormatSpec,
                  buffer_policy: &BufferPolicy,
                  rollover_policy: &RolloverPolicy,
                  output_format_template: OutputFormat) -> Result<Resource, CoalyException> {
        if name_spec.is_thread_specific() {
            // name spec contains thread ID or name, create file template
            let tpl = FileTemplateData::new(output_dir, name_spec, rollover_policy);
            return Ok(Resource {
                          levels,
                          buffer: None,
                          buffer_policy: buffer_policy.clone(),
                          output_format_template,
                          physical_resource: PhysicalResource::FileTemplate(tpl),
                          #[cfg(feature="net")]
                          serialization_buffer: None
                        })
        }
        // name spec is not thread specific, create file
        let phy_res = FileData::new(output_dir, name_spec, rollover_policy)?;
        Ok(Resource {
               levels,
               buffer: None,
               buffer_policy: buffer_policy.clone(),
               output_format_template,
               physical_resource: PhysicalResource::File(phy_res),
                #[cfg(feature="net")]
                serialization_buffer: None
        })
    }

    /// Creates a memory mapped file based resource or resource template.
    /// A resource template is created, if the file is thread specific, otherwise a directly
    /// usable file or memory mapped file.
    ///
    /// # Arguments
    /// * `levels` - the bit mask with all record levels associated with the resource
    /// * `output_dir` - the output directory
    /// * `name_spec` - the file name specification
    /// * `buffer_policy` - the buffer policy
    /// * `rollover_policy` - the rollover policy
    /// * `output_format_template` - the output format template
    /// * `file_size` - the size of the backing file
    fn mm_file(levels: u32,
               output_dir: &Path,
               name_spec: FormatSpec,
               file_size: usize,
               buffer_policy: &BufferPolicy,
               rollover_policy: &RolloverPolicy,
               output_format_template: OutputFormat) -> Result<Resource, CoalyException> {
        if name_spec.is_thread_specific() {
            // name spec contains thread ID or name, create file template
            let tpl = MemMappedFileTemplateData::new(output_dir, name_spec,
                                                     file_size, rollover_policy);
            return Ok(Resource {
                          levels,
                          buffer: None,
                          buffer_policy: buffer_policy.clone(),
                          output_format_template,
                          physical_resource: PhysicalResource::MemMappedFileTemplate(tpl),
                          #[cfg(feature="net")]
                          serialization_buffer: None
                        })
        }
        // name spec is not thread specific, create file
        let phy_res = MemMappedFileData::new(output_dir, name_spec, file_size, rollover_policy)?;
        Ok(Resource {
            levels,
            buffer: None,
            buffer_policy: buffer_policy.clone(),
            output_format_template,
            physical_resource: PhysicalResource::MemMappedFile(phy_res),
            #[cfg(feature="net")]
            serialization_buffer: None
        })
    }

    /// Creates syslog resource.
    ///
    /// # Arguments
    /// * `levels` - the bit mask with all record levels associated with the resource
    /// * `desc` - the network interface resource descriptor
    /// * `buffer_policy` - the buffer policy
    /// * `output_format_template` - the output format template
    #[cfg(feature="net")]
    fn syslog(levels: u32,
              desc: &SyslogResourceDesc,
              buffer_policy: &BufferPolicy,
              orig_info: &OriginatorInfo,
              output_format_template: OutputFormat) -> Result<Resource, CoalyException> {
        let peer_addr = parse_url(desc.remote_url())?;
        let mut local_addr: Option<PeerAddr> = None;
        if let Some(la) = desc.local_url() {
            let laddr = parse_url(la)?;
            if ! peer_addr.can_talk_to(&laddr) { return Err(coalyxe!(E_CFG_NW_PROT_MISMATCH)) }
            local_addr = Some(laddr);
        }
        let mut syslog_res = SyslogData::new(peer_addr, desc.facility(), orig_info);
        syslog_res.connect(local_addr)?;
        Ok(Resource {
            levels,
            buffer: None,
            buffer_policy: buffer_policy.clone(),
            output_format_template,
            physical_resource: PhysicalResource::Syslog(syslog_res),
            serialization_buffer: None
        })
    }

    /// Creates network interface resource.
    ///
    /// # Arguments
    /// * `levels` - the bit mask with all record levels associated with the resource
    /// * `desc` - the network interface resource descriptor
    /// * `buffer_policy` - the buffer policy
    /// * `orig_info` - information about application process and local host
    /// * `output_format_template` - the output format template
    #[cfg(feature="net")]
    fn network(levels: u32,
               desc: &NetworkResourceDesc,
               buffer_policy: &BufferPolicy,
               orig_info: &OriginatorInfo,
               output_format_template: OutputFormat) -> Result<Resource, CoalyException> {
        let peer_addr = parse_url(desc.remote_url())?;
        let mut local_addr: Option<PeerAddr> = None;
        if let Some(la) = desc.local_url() {
            let laddr = parse_url(la)?;
            if ! peer_addr.can_talk_to(&laddr) { return Err(coalyxe!(E_CFG_NW_PROT_MISMATCH)) }
            local_addr = Some(laddr);
        }
        let mut nw_res = NetworkData::new(peer_addr);
        nw_res.connect(local_addr, orig_info)?;
        Ok(Resource {
            levels,
            buffer: None,
            buffer_policy: buffer_policy.clone(),
            output_format_template,
            physical_resource: PhysicalResource::Network(nw_res),
            serialization_buffer: None
        })
    }

    /// Creates a stdout resource.
    ///
    /// # Arguments
    /// * `levels` - the bit mask with all record levels associated with the resource
    /// * `buffer_policy` - the buffer policy
    /// * `output_format_template` - the output format template
    fn stdout(levels: u32,
              buffer_policy: &BufferPolicy,
              output_format_template: OutputFormat) -> Resource {
        Resource {
            levels,
            buffer: None,
            buffer_policy: buffer_policy.clone(),
            output_format_template,
            physical_resource: PhysicalResource::StdOut,
            #[cfg(feature="net")]
            serialization_buffer: None
        }
    }

    /// Creates a stderr resource.
    ///
    /// # Arguments
    /// * `levels` - the bit mask with all record levels associated with the resource
    /// * `buffer_policy` - the buffer policy
    /// * `output_format_template` - the output format template
    fn stderr(levels: u32,
              buffer_policy: &BufferPolicy,
              output_format_template: OutputFormat) -> Resource {
        Resource {
            levels,
            buffer: None,
            buffer_policy: buffer_policy.clone(),
            output_format_template,
            physical_resource: PhysicalResource::StdErr,
            #[cfg(feature="net")]
            serialization_buffer: None
        }
    }

    /// Flush contents of associated memory buffer to physical resource.
    /// 
    /// # Errors
    /// Returns an error structure if the write operation failed
    fn flush_buffer(&mut self) -> Result<(), Vec<CoalyException>> {
        if let Some(ref mut buf) = &mut self.buffer {
            match &self.physical_resource {
                PhysicalResource::File(_) | PhysicalResource::StdOut | PhysicalResource::StdErr => {
                    if let Some(data) = buf.chunk(0) { self.physical_resource.write_chunk(data)?; }
                    if let Some(data) = buf.chunk(1) { self.physical_resource.write_chunk(data)?; }
                    buf.clear();
                },
                PhysicalResource::FileTemplate(_) | PhysicalResource::MemMappedFileTemplate(_)
                                                  | PhysicalResource::MemMappedFile(_) => (),
                #[cfg(feature="net")]
                PhysicalResource::Network(_) | PhysicalResource::Syslog(_) => {
                    for rec in buf.records().iter() {
                        if let Some(rec1) = rec.1 {
                            let mut full_rec = Vec::<u8>::with_capacity(rec.0.len() + rec1.len());
                            full_rec.extend_from_slice(rec.0);
                            full_rec.extend_from_slice(rec1);
                            self.physical_resource.write_chunk(full_rec.as_slice())?;
                        } else {
                            self.physical_resource.write_chunk(rec.0)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

enum PhysicalResource {
    File(FileData),
    FileTemplate(FileTemplateData),
    MemMappedFile(MemMappedFileData),
    MemMappedFileTemplate(MemMappedFileTemplateData),
    StdOut,
    StdErr,
    #[cfg(feature="net")]
    Network(NetworkData),
    #[cfg(feature="net")]
    Syslog(SyslogData),
}
impl PhysicalResource {
    /// Indicates whether the resource is a proxy for a resource on a remote application.
    #[cfg(feature="net")]
    #[inline]
    fn is_proxy(&self) -> bool {
        match self {
            PhysicalResource::Network(_) | PhysicalResource::Syslog(_) => true,
            _ => false
        }
    }

    /// Sends a log or trace record to a remote application.
    /// 
    /// # Arguments
    /// * `rec` - the log or trace record
    /// 
    /// # Errors
    /// Returns an error structure if the send operation fails
    #[cfg(feature="net")]
    fn send_record(&mut self, rec: &dyn RecordData) -> Result<(), Vec<CoalyException>> {
        match self {
            PhysicalResource::Network(n) => n.send_record(rec),
            PhysicalResource::Syslog(s) => s.send_record(rec),
            _ => Ok(())
        }
    }

    /// Writes a log or trace record.
    /// 
    /// # Arguments
    /// * `s` - the log or trace record
    /// 
    /// # Errors
    /// Returns an error structure if the write operation fails
    fn write_record(&mut self, s: &str) -> Result<(), Vec<CoalyException>> {
        if let PhysicalResource::MemMappedFile(f) = self { f.write_record(s); return Ok(())  }
        self.write_chunk(s.as_bytes())
    }

    /// Writes the given output data.
    /// 
    /// # Arguments
    /// * `data` - the output data
    /// 
    /// # Errors
    /// Returns an error structure if the write operation fails
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), Vec<CoalyException>> {
        match self {
            PhysicalResource::File(f) => f.write(chunk).map_err(|e| vec!(e)),
            PhysicalResource::StdOut => {
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                let _ = handle.write_all(chunk);
                Ok(())
            },
            PhysicalResource::StdErr => {
                let stderr = io::stderr();
                let mut handle = stderr.lock();
                let _ = handle.write_all(chunk);
                Ok(())
            },
            #[cfg(feature="net")]
            PhysicalResource::Network(n) => n.write(chunk),
            _ => Ok(())
        }
    }

    /// Closes the physical resource.
    fn close(&mut self) {
        match self {
            PhysicalResource::File(f) => f.close(),
            PhysicalResource::MemMappedFile(f) => f.close(),
            #[cfg(feature="net")]
            PhysicalResource::Network(n) => n.disconnect(),
            #[cfg(feature="net")]
            PhysicalResource::Syslog(s) => s.close(),
            _ => ()
        }
    }

    /// Indicates, whether this resource is specific for an originator.
    pub(crate) fn is_originator_specific(&self) -> bool {
        match self {
            PhysicalResource::File(f) => f.is_originator_specific(),
            PhysicalResource::MemMappedFile(f) => f.is_originator_specific(),
            PhysicalResource::FileTemplate(t) => t.is_originator_specific(),
            PhysicalResource::MemMappedFileTemplate(t) => t.is_originator_specific(),
            _ => false
        }
    }

    /// Indicates, whether this resource is specific for a thread.
    pub(crate) fn is_thread_specific(&self) -> bool {
        match self {
            PhysicalResource::FileTemplate(t) => t.is_thread_specific(),
            PhysicalResource::MemMappedFileTemplate(t) => t.is_thread_specific(),
            _ => false
        }
    }

    /// Performs a rollover of a file based resource if the rollover is due.
    /// 
    /// # Arguments
    /// * `now` - current timestamp
    fn rollover_if_due(&mut self, now: &DateTime<Local>) -> Result<(), CoalyException> {
        match self {
            PhysicalResource::File(f) => f.rollover_if_due(now),
            PhysicalResource::MemMappedFile(f) => f.rollover_if_due(now),
            _ => Ok(())
        }
    }

    /// Returns the name specification for this resource, optimized for the specified originator.
    /// Returns None, if the resource is not backed by a file template.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator data with the potential variable values
    pub(crate) fn originator_optimized_name(&self,
                                            orig_info: &OriginatorInfo) -> Option<FormatSpec> {
        match self {
            PhysicalResource::File(f) => {
                Some(f.originator_optimized_name(orig_info))
            },
            PhysicalResource::MemMappedFile(f) => {
                Some(f.originator_optimized_name(orig_info))
            },
            PhysicalResource::FileTemplate(t) => {
                Some(t.originator_optimized_name(orig_info))
            },
            PhysicalResource::MemMappedFileTemplate(t) => {
                Some(t.originator_optimized_name(orig_info))
            },
            _ => None
        }
    }

    /// Returns the name specification for this resource, optimized for the specified thread.
    /// Returns None, if the resource is not backed by a file template.
    /// 
    /// # Arguments
    /// * `thread_id` - thread ID
    /// * `thread_name` - thread name
    pub(crate) fn thread_optimized_name(&self,
                                        thread_id: u64,
                                        thread_name: &str) -> Option<FormatSpec> {
        match self {
            PhysicalResource::FileTemplate(t) => {
                Some(t.thread_optimized_name(thread_id, thread_name))
            },
            PhysicalResource::MemMappedFileTemplate(t) => {
                Some(t.thread_optimized_name(thread_id, thread_name))
            },
            _ => None
        }
    }

    /// Updates the file name specification with the given value.
    /// If the resource is not backed by a file template, a call to this method has no effect.
    /// 
    /// # Arguments
    /// * `name_spec` - the optimized name specification
    pub(crate) fn use_optimized_name(&mut self, name_spec: FormatSpec) {
        match self {
            PhysicalResource::File(f) => f.update_namespec(name_spec),
            PhysicalResource::MemMappedFile(f) => f.update_namespec(name_spec),
            PhysicalResource::FileTemplate(t) => t.update_namespec(name_spec),
            PhysicalResource::MemMappedFileTemplate(t) => t.update_namespec(name_spec),
            _ => ()
        }
    }

    /// Creates a thread specific resource from this physical resource template.
    ///
    /// # Arguments
    /// * `name_spec` - file name specification, optimized for thread
    /// 
    /// # Return values
    /// thread specific resource, if this resource is a template; otherwise **None**
    fn for_thread(&self,
                  name_spec: FormatSpec) -> Result<PhysicalResource, CoalyException> {
        match self {
            PhysicalResource::FileTemplate(t) => {
                let r = t.instantiate(name_spec)?;
                Ok(PhysicalResource::File(r))
            },
            PhysicalResource::MemMappedFileTemplate(t) => {
                let r = t.instantiate(name_spec)?;
                Ok(PhysicalResource::MemMappedFile(r))
            },
            _ => Err(coalyxe!(E_INTERNAL_INV_TEMPLATE))
        }
    }

    /// Creates an originator specific resource from this template.
    /// The resource created may be a template or final, depending on whether the given name
    /// is thread-specific or not.
    ///
    /// # Arguments
    /// * `name_spec` - file name specification, optimized for originator
    /// 
    /// # Return values
    /// originator specific resource, if this resource is a template; otherwise **None**
    #[cfg(feature="net")]
    fn for_originator(&self,
                      name_spec: FormatSpec) -> Result<PhysicalResource, CoalyException> {
        match self {
            PhysicalResource::FileTemplate(t) => {
                if name_spec.is_thread_specific() {
                    let opt_templ = t.for_originator(name_spec);
                    return Ok(PhysicalResource::FileTemplate(opt_templ))
                }
                let r = t.instantiate(name_spec)?;
                Ok(PhysicalResource::File(r))
            },
            PhysicalResource::MemMappedFileTemplate(t) => {
                if name_spec.is_thread_specific() {
                    let opt_templ = t.for_originator(name_spec);
                    return Ok(PhysicalResource::MemMappedFileTemplate(opt_templ))
                }
                let r = t.instantiate(name_spec)?;
                Ok(PhysicalResource::MemMappedFile(r))
            },
            _ => Err(coalyxe!(E_INTERNAL_INV_TEMPLATE))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// Returns the root directory used for tests
    pub(crate) fn test_dir_root_path() -> PathBuf {
        Path::new(&std::env::var("COALY_TESTING_ROOT").unwrap()).join("tmp")
    }

    /// Returns the temporary directory used for a specific test function
    pub(crate) fn test_dir_path(fn_name: &[&str]) -> PathBuf {
        let mut dir = test_dir_root_path();
        fn_name.iter().for_each(|x| dir = dir.join(x));
        dir
    }

    /// Removes all elements in specified directory
    pub(crate) fn clear_test_dir(dir: &Path) {
        if ! dir.exists() { return }
        let dir_listing = std::fs::read_dir(dir);
        if dir_listing.is_err() {
            assert!(false, "Could not clear directory{}", dir.to_string_lossy());
        }
        for entry in dir_listing.unwrap() {
            if let Ok(elem) = entry {
                let elem_path = elem.path();
                if elem.file_type().unwrap().is_file() {
                    if let Err(e) = std::fs::remove_file(&elem_path) {
                        assert!(false, "Could not delete file {}: {}",
                                        elem_path.to_string_lossy(), e);
                    }
                } else {
                    if let Err(e) = std::fs::remove_dir_all(&elem_path) {
                        assert!(false, "Could not delete dir {}: {}",
                                        elem_path.to_string_lossy(), e);
                    }
                }
            } else {
                assert!(false, "Could not clear directory {}", dir.to_string_lossy());
            }
        }
    }

}
