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

//! Output resources of type plain or memory mapped file.

use chrono::{DateTime, Local, TimeZone};
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use crate::{coalyxe, coalyxw};
use crate::errorhandling::*;
use crate::output::formatspec::FormatSpec;
use crate::output::recordbuffer::RecordBuffer;
use crate::policies::*;
use crate::record::originator::OriginatorInfo;
use super::rollover::archive_resource;

/// Specific data for physical resources of kind plain file.
pub(crate) struct FileData {
    // pure file name, without path
    name: String,
    // file handle
    f: Option<File>,
    // meta data for rollover handling
    meta_data: RolloverMetaData,
    // number of bytes written to file
    bytes_written: usize
}
impl FileData {
    /// Creates descriptive data for a plain file.
    /// Does not create the file yet.
    ///
    /// # Arguments
    /// * `output_dir` - the output directory path
    /// * `name_spec` - the file name specification, already optimized for process
    /// * `rollover_policy` - the rollover policy descriptor
    pub(crate) fn new(output_dir: &Path,
                      name_spec: FormatSpec,
                      rollover_policy: &RolloverPolicy) -> Result<FileData, CoalyException> {
        let meta_data = RolloverMetaData::new(output_dir, name_spec, rollover_policy, 0);
        Ok(FileData {
               name: String::from(""),
               f: None,
               meta_data,
               bytes_written: 0
           })
    }

    /// Indicates, whether this file is specific for an originator.
    pub(crate) fn is_originator_specific(&self) -> bool {
        self.meta_data.name_spec.is_originator_specific()
    }

    /// Returns the file name specification with all originator specific variable items
    /// replaced with values from given originator information structure.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator information
    pub(crate) fn originator_optimized_name(&self,
                                            orig_info: &OriginatorInfo) -> FormatSpec {
        self.meta_data.name_spec.optimized_for_originator(orig_info)
    }

    /// Replaces the internal file name specification with the given value.
    /// To be called with the return value of method originator_optimized_namespec.
    /// 
    /// # Arguments
    /// * `new_spec` - the file name specification, optimized for originator
    pub(crate) fn update_namespec(&mut self, new_spec: FormatSpec) {
        self.meta_data.name_spec = new_spec;
    }

    /// Writes the given slice to the associated file.
    ///
    /// # Arguments
    /// * `data` - the data to write
    ///
    /// # Errors
    /// Returns an error structure if the write operation fails
    pub(crate) fn write(&mut self, data: &[u8]) -> Result<(), CoalyException> {
        if self.f.is_none() { self.open()?;  }
        if let Err(m) = self.f.as_ref().unwrap().write_all(data) {
            return Err(coalyxe!(E_FILE_WRITE_ERR, self.name.to_string(), m.to_string()))
        }
        self.bytes_written += data.len();
        // check if rollover is needed, only size based rollover must be considered here
        if self.meta_data.max_size > 0 && self.bytes_written >= self.meta_data.max_size {
            return self.rollover()
        }
        Ok(())
    }    

    /// Opens the associated file.
    /// It is guaranteed, that the structure's file handle is valid in case of success.
    ///
    /// # Errors
    /// Returns an error structure if the output file can't be created
    fn open(&mut self) -> Result<(), CoalyException> {
        self.close();
        self.name = self.meta_data.file_name();
        self.f = Some(create_file(self.meta_data.output_dir(), &self.name)?);
        Ok(())
    }

    /// Closes the associatedfile.
    /// It is guaranteed, that the structure's file handle is None after a call to this function.
    pub(crate) fn close(&mut self) {
        self.bytes_written = 0;
        if let Some(ref mut f) = &mut self.f {
            let _ = f.flush();
            let _ = f.sync_all();
            self.f = None;
        }
    }

    /// Performs a rollover if it is due.
    ///
    /// # Arguments
    /// * `now` - current timestamp
    ///
    /// # Errors
    /// Returns an error descriptor if any part of the rollover process fails
    pub(crate) fn rollover_if_due(&mut self,
                                  now: &DateTime<Local>) -> Result<(), CoalyException> {
        if self.meta_data.is_rollover_due(now) {
            self.meta_data.determine_next_rollover();
            return self.rollover()
        }
        Ok(())
    }

    /// Performs a rollover.
    ///
    /// # Errors
    /// Returns an error descriptor if any part of the rollover process fails
    fn rollover(&mut self) -> Result<(), CoalyException> {
        // close current output file
        self.close();
        // archive current output file
        let new_name = self.meta_data.file_name();
        let dir = self.meta_data.output_dir();
        if let Err(e) = archive_resource(dir, &self.name, &new_name, self.meta_data.name_spec(),
                                         self.meta_data.keep_count(),
                                         &self.meta_data.compression()) {
            // archive operation failed, try to re-open old output file
            let old_path = dir.join(&self.name);
            let old_path_name = old_path.to_string_lossy().to_string();
            match File::options().append(true).open(&old_path) {
                Ok(f) => {
                    // re-open old file succeeded
                    self.f = Some(f);
                    let new_path_name = dir.join(&new_name).to_string_lossy().to_string();
                    let mut ex = coalyxw!(W_ROVR_USING_OLD, new_path_name, old_path_name);
                    ex.set_cause(e);
                    return Err(ex)
                },
                Err(e) => {
                    // re-open old file failed
                    return Err(coalyxe!(E_FILE_CRE_ERR, old_path_name, e.to_string()))
                }
            }
        }
        self.name = new_name;
        self.f = Some(create_file(dir, &self.name)?);
        Ok(())
    }
}

/// Specific data for templates of plain file physical resources.
pub(crate) struct FileTemplateData(RolloverMetaData);
impl FileTemplateData {
    /// Creates template for a plain file.
    ///
    /// # Arguments
    /// * `output_dir` - the output directory path
    /// * `name_spec` - the file name specification, already optimized for process
    /// * `rollover_policy` - the rollover policy descriptor
    pub(crate) fn new(output_dir: &Path,
                      name_spec: FormatSpec,
                      rollover_policy: &RolloverPolicy) -> FileTemplateData {
        FileTemplateData {
            0: RolloverMetaData::new(output_dir, name_spec, rollover_policy, 0)
        }
    }

    /// Creates a final resource from this template.
    ///
    /// # Arguments
    /// * `namespec` - name specification, optimized for thread ID and name
    /// 
    /// # Return values
    /// final file resource
    pub(crate) fn instantiate(&self,
                              namespec: FormatSpec) -> Result<FileData, CoalyException> {
        let name = namespec.to_file_name();
        let f = create_file(self.0.output_dir(), &name)?;
        let mut meta_data = self.0.clone();
        meta_data.name_spec = namespec;
        Ok(FileData { name, f: Some(f), meta_data, bytes_written: 0 })
    }

    /// Creates a thread-specific template from this template.
    ///
    /// # Arguments
    /// * `namespec` - name specification, optimized for originator
    /// 
    /// # Return values
    /// thread-specific template
    #[cfg(feature="net")]
    pub(crate) fn for_originator(&self,
                                 namespec: FormatSpec) -> FileTemplateData {
        let mut opt_meta_data = self.0.clone();
        opt_meta_data.name_spec = namespec;
        FileTemplateData { 0: opt_meta_data }
    }

    /// Indicates, whether this template is specific for an originator.
    pub(crate) fn is_originator_specific(&self) -> bool {
        self.0.name_spec.is_originator_specific()
    }

    /// Indicates, whether this template is specific for a thread.
    pub(crate) fn is_thread_specific(&self) -> bool {
        self.0.name_spec.is_thread_specific()
    }

    /// Replaces the internal file name specification with the given value.
    /// To be called with the return value of method originator_optimized_namespec.
    /// 
    /// # Arguments
    /// * `new_spec` - the file name specification, optimized for originator
    pub(crate) fn update_namespec(&mut self, new_spec: FormatSpec) {
        self.0.name_spec = new_spec;
    }

    /// Returns the file name specification with all originator specific variable items
    /// replaced with values from given originator information structure.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator information
    pub(crate) fn originator_optimized_name(&self,
                                            orig_info: &OriginatorInfo) -> FormatSpec {
        self.0.name_spec.optimized_for_originator(orig_info)
    }

    /// Returns the file name specification with all thread specific variable items
    /// replaced with given values.
    /// 
    /// # Arguments
    /// * `thread_id` - the thread ID
    /// * `thread_name` - the thread name
    pub(crate) fn thread_optimized_name(&self,
                                        thread_id: u64,
                                        thread_name: &str) -> FormatSpec {
        self.0.name_spec.optimized_for_thread(thread_id, thread_name)
    }
}

/// Specific data for physical resources of kind memory mapped file.
/// 
pub(crate) struct MemMappedFileData {
    // pure file name without path
    name: String,
    // buffer wrapped around memory map
    rec_buffer: RecordBuffer,
    // meta data for rollover handling
    meta_data: RolloverMetaData
}
impl MemMappedFileData {
    /// Creates data for a memory mapped file.
    ///
    /// # Arguments
    /// * `output_dir` - the output directory path
    /// * `name_spec` - the file name specification, already optimized for process
    /// * `file_size` - the size of the backing file
    /// * `rollover_policy` - the rollover policy descriptor
    pub(crate) fn new(output_dir: &Path,
                      name_spec: FormatSpec,
                      file_size: usize,
                      rollover_policy: &RolloverPolicy) -> Result<MemMappedFileData, CoalyException> {
        let name = name_spec.to_file_name();
        let f_path = output_dir.join(&name);
        let f_size = min(MIN_FILE_SIZE, file_size);
        let max_rec_count = f_size >> 5;
        let rec_buffer = RecordBuffer::backed_by_file(&f_path, f_size, max_rec_count)?;
        Ok(MemMappedFileData {
               name,
               rec_buffer,
               meta_data: RolloverMetaData::new(output_dir, name_spec, rollover_policy, f_size)
        })
    }

    /// Indicates, whether this file is specific for an originator.
    pub(crate) fn is_originator_specific(&self) -> bool {
        self.meta_data.name_spec.is_originator_specific()
    }

    /// Returns the file name specification with all originator specific variable items
    /// replaced with values from given originator information structure.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator information
    pub(crate) fn originator_optimized_name(&self,
                                            orig_info: &OriginatorInfo) -> FormatSpec {
        self.meta_data.name_spec.optimized_for_originator(orig_info)
    }

    /// Replaces the internal file name specification with the given value.
    /// To be called with the return value of method originator_optimized_namespec.
    /// 
    /// # Arguments
    /// * `new_spec` - the file name specification, optimized for originator
    pub(crate) fn update_namespec(&mut self, new_spec: FormatSpec) {
        self.meta_data.name_spec = new_spec;
    }

    /// Writes the given slice to the memory mapped file.
    ///
    /// # Arguments
    /// * `data` - the data to write
    /// 
    /// # Errors
    /// Returns an error structure if the write operation fails
    pub(crate) fn write_record(&mut self, s: &str) { self.rec_buffer.write(s); }

    /// Closes the memory mapped file.
    pub(crate) fn close(&mut self) { self.rec_buffer.close(); }

    /// Performs a rollover if it is due.
    /// 
    /// # Arguments
    /// * `now` - current timestamp
    pub(crate) fn rollover_if_due(&mut self,
                                  now: &DateTime<Local>) -> Result<(), CoalyException> {
        if self.meta_data.is_rollover_due(now) {
            self.meta_data.determine_next_rollover();
            return self.rollover()
        }
        Ok(())
    }

    /// Performs a rollover.
    ///
    /// # Errors
    /// Returns a vector with an error message for every failed rename or write operation
    fn rollover(&mut self) -> Result<(), CoalyException> {
        // close current file
        self.close();
        // archive current file
        let new_name = self.meta_data.file_name();
        let dir = self.meta_data.output_dir();
        if let Err(e) = archive_resource(dir, &self.name, &new_name, self.meta_data.name_spec(),
                                         self.meta_data.keep_count(),
                                         &self.meta_data.compression()) {
            // archive operation failed, try to re-open old output file
            let old_path = dir.join(&self.name);
            let old_path_name = old_path.to_string_lossy().to_string();
            self.rec_buffer.reopen(&old_path, false)?;
            // re-open old file succeeded
            let new_path_name = dir.join(&new_name).to_string_lossy().to_string();
            let mut ex = coalyxw!(W_ROVR_USING_OLD, new_path_name, old_path_name);
            ex.set_cause(e);
            return Err(ex)
        }
        self.rec_buffer.reopen(&dir.join(&new_name), true)?;
        self.name = new_name;
        Ok(())
    }
}

/// Specific data for templates of memory mapped file physical resources.
pub(crate) struct MemMappedFileTemplateData(RolloverMetaData);
impl MemMappedFileTemplateData {
    /// Creates template for a memory mapped file.
    ///
    /// # Arguments
    /// * `output_dir` - the output directory path
    /// * `name_spec` - the file name specification, already optimized for process
    /// * `rollover_policy` - the rollover policy descriptor
    /// * `file_size` - the size of the backing file
    pub(crate) fn new(output_dir: &Path,
                      name_spec: FormatSpec,
                      file_size: usize,
                      rollover_policy: &RolloverPolicy) -> MemMappedFileTemplateData {
        MemMappedFileTemplateData {
            0: RolloverMetaData::new(output_dir, name_spec, rollover_policy, file_size)
        }
    }

    /// Creates a thread specific resource from this template.
    ///
    /// # Arguments
    /// * `namespec` - name specification, optimized for thread ID and name
    /// 
    /// # Return values
    /// thread specific file resource
    pub(crate) fn instantiate(&self,
                              namespec: FormatSpec) -> Result<MemMappedFileData, CoalyException> {
        let name = namespec.to_file_name();
        let f_path = self.0.dir.join(&name);
        let f_size = self.0.file_size;
        let buf_content_size = f_size - 32;
        let max_rec_count = buf_content_size >> 5;
        let rec_buffer = RecordBuffer::backed_by_file(&f_path, f_size, max_rec_count)?;
        let mut meta_data = self.0.clone();
        meta_data.name_spec = namespec;
        Ok(MemMappedFileData {
               name,
               rec_buffer,
               meta_data
        })
    }

    /// Creates a thread-specific template from this template.
    ///
    /// # Arguments
    /// * `namespec` - name specification, optimized for originator
    /// 
    /// # Return values
    /// thread-specific template
    #[cfg(feature="net")]
    pub(crate) fn for_originator(&self,
                                 namespec: FormatSpec) -> MemMappedFileTemplateData {
        let mut opt_meta_data = self.0.clone();
        opt_meta_data.name_spec = namespec;
        MemMappedFileTemplateData { 0: opt_meta_data }
    }

    /// Indicates, whether this template is specific for an originator.
    pub(crate) fn is_originator_specific(&self) -> bool {
        self.0.name_spec.is_originator_specific()
    }

    /// Indicates, whether this template is specific for a thread.
    pub(crate) fn is_thread_specific(&self) -> bool {
        self.0.name_spec.is_thread_specific()
    }

    /// Replaces the internal file name specification with the given value.
    /// To be called with the return value of method originator_optimized_namespec.
    /// 
    /// # Arguments
    /// * `new_spec` - the file name specification, optimized for originator
    pub(crate) fn update_namespec(&mut self, new_spec: FormatSpec) {
        self.0.name_spec = new_spec;
    }

    /// Returns the file name specification with all originator specific variable items
    /// replaced with values from given originator information structure.
    /// 
    /// # Arguments
    /// * `orig_info` - the originator information
    pub(crate) fn originator_optimized_name(&self,
                                            orig_info: &OriginatorInfo) -> FormatSpec {
        self.0.name_spec.optimized_for_originator(orig_info)
    }

    /// Returns the file name specification with all thread specific variable items
    /// replaced with given values.
    /// 
    /// # Arguments
    /// * `thread_id` - the thread ID
    /// * `thread_name` - the thread name
    pub(crate) fn thread_optimized_name(&self,
                                        thread_id: u64,
                                        thread_name: &str) -> FormatSpec {
        self.0.name_spec.optimized_for_thread(thread_id, thread_name)
    }
}

/// Meta data for handling rollover of physical resources of kind plain or memory mapped file.
#[derive (Clone, Debug)]
struct RolloverMetaData {
    // output directory path
    dir: PathBuf,
    // file name specification from system configuration
    name_spec: FormatSpec,
    // fix file size, for memory mapped files only (0 for plain files)
    file_size: usize,
    // maximum file size, before a rollover takes place (0 means no rollover)
    max_size: usize,
    // rollover policy
    rollover_policy: RolloverPolicy,
    // timestamp for next rollover of the file
    next_rovr_ts: DateTime<Local>
}
impl RolloverMetaData {
    /// Creates rollover meta data for a file.
    ///
    /// # Arguments
    /// * `output_dir` - the output directory path
    /// * `name_spec` - the file name specification
    /// * `rollover_policy` - the rollover policy descriptor
    /// * `file_size` - the file size (memory mapped files only, 0 for plain files)
    fn new(output_dir: &Path,
           name_spec: FormatSpec,
           rollover_policy: &RolloverPolicy,
           file_size: usize) -> RolloverMetaData {
        let mut max_size: usize = 0;
        let mut next_rovr_ts = Local.ymd(2200, 12, 31).and_hms(23, 59, 59);
        match rollover_policy.condition() {
            RolloverCondition::SizeReached(s) => max_size = *s,
            RolloverCondition::TimeElapsed(i) => {
                next_rovr_ts = i.next_elapse(&Local::now())
            },
            _ => ()
        }
        RolloverMetaData {
            dir: output_dir.to_path_buf(),
            name_spec,
            file_size,
            max_size,
            rollover_policy: rollover_policy.clone(),
            next_rovr_ts
        }
    }

    /// Returns the output directory
    #[inline]
    fn output_dir(&self) -> &PathBuf { &self.dir }

    /// Returns the name specification from system configuration
    #[inline]
    fn name_spec(&self) -> &FormatSpec { &self.name_spec }

    /// Returns the file name from name specification and current timestamp
    #[inline]
    fn file_name(&self) -> String { self.name_spec.to_file_name() }

    /// Returns the compression algorithm to use for rollover files
    #[inline]
    fn compression(&self) -> CompressionAlgorithm { self.rollover_policy.compression() }

    /// Returns the maximum number of rollover files to keep
    #[inline]
    fn keep_count(&self) -> u32 { self.rollover_policy.keep_count() }

    /// Indicates whether a rollover must be executed.
    #[inline]
    fn is_rollover_due(&self, now: &DateTime<Local>) -> bool {
        now.timestamp() >= self.next_rovr_ts.timestamp()
    }

    /// Determines time stamp for next rollover
    #[inline]
    fn determine_next_rollover(&mut self) {
        if let RolloverCondition::TimeElapsed(intvl) = self.rollover_policy.condition() {
            self.next_rovr_ts = intvl.next_elapse(&self.next_rovr_ts);
        }
    }
}

/// Creates and opens a plain file for output.
/// Creates missing parent directories, if needed.
///
/// # Arguments
/// * `output_dir` - the output directory path
/// * `file_name` - the pure file name without path
/// 
/// # Return values
/// handle to the created file
/// 
/// # Errors
/// Returns an error structure if the file could not be created
fn create_file(dir: &PathBuf, file_name: &str) -> Result<File, CoalyException> {
    let file_path = dir.join(file_name);
    let full_file_name = file_path.to_string_lossy().to_string();
    if let Err(m) = std::fs::create_dir_all(dir) {
        return Err(coalyxe!(E_FILE_CRE_ERR, full_file_name, m.to_string()))
    }
    File::create(file_path).map_err(|e| coalyxe!(E_FILE_CRE_ERR, full_file_name.to_string(),
                                               e.to_string()))
}

#[cfg(test)]
mod tests {
}
