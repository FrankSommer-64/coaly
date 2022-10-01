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

//! Rollover functionality for file based output resources.
//! 
//! Rollover denotes an archiving mechanism where the current output file is closed, renamed and
//! then re-opened with empty contents. Rollover files are preserved up to a configurable limit
//! for the number of files (keep limit). If the number of rollover files exceeds this limit, the
//! oldest ones are deleted.
//! 
//! Rollover in Coaly follows the principles below.
//! 
//! * Assumptions
//!     1. The file name specification for an output resource must not end with a dot followed
//!        by digits only, otherwise the sequence numbering scheme could fail.
//!     2. Date/Time related variables must not be used for parent directories of the output
//!        resource, otherwise the sorting algorithm will fail.
//! 
//! * Rollover file names
//!     1. If the output file name specification is date/time independent, a dot followed by a
//!        sequence number is appended at the end, optionally followed by the file extension
//!        corresponding to the compression algorithm used. Numbering starts with 1 and uses pure
//!        numbers without leading zeroes. Examples: "app.log.1" resp. "app.log.1.zip".
//!     2. If the output file name contains at least one of the variables $Date, $Time or
//!        $TimeStamp, the concrete name for the new output file is determined based on the current
//!        time. Hence, the rollover file names will optionally have the file extension
//!        corresponding to the compression algorithm appended, but otherwise remain unchanged.
//!        The sequence numbering is used only in cases, where a rollover file with the same name
//!        already exists. Examples: "app_20220315.log" resp. "app_20220315.log.zip".
//! 
//! * Rollover file sorting
//!     1. If the output file name specification is date/time independent, it will contain no
//!        variables at all, since they are all replaced upon first instantiation of the output
//!        resource. The match pattern is built from the output file name specification, followed
//!        by a dot and a group of one or more digits, optionally followed by the file extension
//!        corresponding to  the compression algorithm used.
//!        Sorting is based on the group of digits.
//!     2. If the output file name specification depends on date/time, it contains one or more
//!        date/time variables, but no other ones. For every variable a match group is assembled
//!        under consideration of the date/time format descriptor. Furthermore, a mapping is built,
//!        so that the value of all date/time variables can be transferred into the best possible
//!        comprehensive timestamp.
//!        The match pattern is built from the output file name specification using the match groups
//!        for the date/time variables, followed by a dot and a group of one or more digits,
//!        optionally followed by the file extension corresponding to the compression algorithm.
//!        Sorting is based on the timestamp first, then on the group of digits for files having
//!        equal timestamp values.
//! 
//! * Rollover algorithm
//!     1. All rollover files for an output resource are ordered chronologically, newest file first.
//!        Sorting is based on file names only, file attributes like creation time are not taken
//!        into account.
//!     2. If necessary, the oldest rollover files are deleted until their number is less than the
//!        keep limit minus 1 (the current output file will add to the rollover files,
//!        hence minus 1).
//!     3. The name for the first rollover file is determined. If a file with that name exists, the
//!        current rollover files are renamed recursively until all name conflicts are resolved.
//!     4. The name for the new output file is determined and the current output file is closed.
//!        If a file with the new name exists, the current output file is renamed to the name for
//!        the first rollover file. The current output file is eventually compressed.
//!     5. The new output file is opened.

#[cfg(feature="compression")]
use zip::write::FileOptions;
#[cfg(feature="compression")]
use zip::ZipWriter;
#[cfg(feature="compression")]
use bzip2::write::BzEncoder;
#[cfg(feature="compression")]
use flate2::GzBuilder;
use regex::{Captures, Regex};
#[cfg(feature="compression")]
use xz2::write::XzEncoder;
use std::cmp::Ordering;
#[cfg(feature="compression")]
use std::fs::File;
#[cfg(feature="compression")]
use std::io::Write;
use std::path::{Path, PathBuf};
use crate::coalyxe;
use crate::errorhandling::*;
use crate::output::formatspec::FormatSpec;
use crate::policies::*;



/// Archives an output resource file and performs a rollover for existing archive files.
/// The current output resource must have been closed a priori.
/// The archival is aborted upon the first failed part of the entire operation.
///
/// # Arguments
/// * `output_dir` - the output directory path
/// * `active_file_name` - the pure name of the currently active output resource file
/// * `new_file_name` - the pure name of the active output resource file after rollover
/// * `name_spec` - the resource file name specification
/// * `keep_count` - the maximum number of archive files to keep, if the limit is exceeded, the
///                  oldest archive files are removed
/// * `compression` - the compression algorithm to use for the archive file
///
/// # Errors
/// Returns an error descriptor if any sub-operation fails
pub(crate) fn archive_resource(output_dir: &PathBuf,
                               active_file_name: &str,
                               new_file_name: &str,
                               name_spec: &FormatSpec,
                               keep_count: u32,
                               compression: &CompressionAlgorithm) -> Result<(), CoalyException> {
    // determine a list of all files belonging to the output resource, newest files first
    // if we don't find any files, we assume that nothing has been logged yet
    let active_file_path = output_dir.join(active_file_name);
    let compr_ext = compression.file_extension();
    let name_dtm_dep = ! name_spec.is_datetime_independent();
    let find_pattern = name_spec.file_name_pattern(compr_ext);
    if let Err(e) = find_pattern {
        return Err(coalyxe!(E_ROVR_FAILED,
                            active_file_path.to_string_lossy().to_string(), e.to_string()))
    }
    let find_pattern = find_pattern.unwrap();
    let res_files = find_resource_files(output_dir, &active_file_name, name_dtm_dep,
                                        &find_pattern, compr_ext)?;
    if res_files.is_empty() { return Ok(()) }

    // Remove oldest rollover files exceeding the keep limit and eventually rename the files kept
    let res_files = remove_rollover_files(output_dir, &res_files, keep_count)?;
    shift_rollover_files(output_dir, new_file_name, &res_files)?;

    // archive current file
    let ar_file_name = if active_file_name == new_file_name { res_files[0].shifted_file_name() }
                       else { format!("{}{}", active_file_name, compression.file_extension()) };
    let ar_file_path = output_dir.join(&ar_file_name);
    #[cfg(feature="compression")]
    return archive_active_file(&active_file_path, &ar_file_path, compression)
               .map_err(|e| coalyxe!(E_ROVR_FAILED, active_file_path.to_string_lossy().to_string(),
                                     e.to_string()));
    #[cfg(not(feature="compression"))]
    { let _ = std::fs::rename(active_file_path, ar_file_path); Ok(()) }
}

/// Archives the currently active output file of a resource.
///
/// # Arguments
/// * `active_file_path` - the path of the active output file
/// * `arch_file_path` - the path for the archived active output file
/// * `compression` - compression algorithm to use
///
/// # Errors
/// Returns an error structure if an I/O error occurs
#[cfg(feature="compression")]
fn archive_active_file(active_file_path: &PathBuf,
                       arch_file_path: &PathBuf,
                       compression: &CompressionAlgorithm) -> Result<(), std::io::Error> {
    #[cfg(feature="compression")]
    match compression {
        CompressionAlgorithm::Bzip2 => {
            let f = File::create(arch_file_path)?;
            let data = std::fs::read(&active_file_path)?;
            let mut enc = BzEncoder::new(f, bzip2::Compression::fast());
            enc.write_all(&data)?;
            enc.finish()?;
            let _ = std::fs::remove_file(active_file_path);
            Ok(())
        },
        CompressionAlgorithm::Zip => {
            let f = File::create(arch_file_path)?;
            let fname = active_file_path.file_name().unwrap().to_string_lossy();
            let data = std::fs::read(&active_file_path)?;
            let mut enc = ZipWriter::new(f);
            let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            enc.start_file(fname, opts)?;
            enc.write_all(&data)?;
            enc.finish()?;
            let _ = std::fs::remove_file(active_file_path);
            Ok(())
        },
        CompressionAlgorithm::Gzip => {
            let f = File::create(arch_file_path)?;
            let fname = active_file_path.file_name().unwrap().to_string_lossy();
            let data = std::fs::read(&active_file_path)?;
            let mut enc = GzBuilder::new().filename(&*fname)
                                         .write(f, flate2::Compression::default());
            enc.write_all(&data)?;
            enc.finish()?;
            let _ = std::fs::remove_file(active_file_path);
            Ok(())
        },
        CompressionAlgorithm::Lzma => {
            let f = File::create(arch_file_path)?;
            let data = std::fs::read(&active_file_path)?;
            let mut enc = XzEncoder::new(f, 9);
            enc.write_all(&data)?;
            enc.finish()?;
            let _ = std::fs::remove_file(active_file_path);
            Ok(())
        },
        CompressionAlgorithm::None => {
            // without compression we can simply rename the file
            std::fs::rename(active_file_path, arch_file_path)
        }
    }
}

/// Finds all files related to an output resource.
/// This includes current output file and all rollover files, if any.
///
/// # Arguments
/// * `dir` - the output directory for the resource
/// * `current_file_name` - the pure name of the current output resource file without path
/// * `datetime_dependent` - indicates whether the file name contains date/time related variables
///                          (true) or not (false)
/// * `pattern` - the regular expression to find matching files
/// * `compr_ext` - compression specific file extension including dot, empty string for no
///                 compression
///
/// # Return values
/// sorted vector with all files found, current output file first, then all existing rollover files,
/// oldest last
///
/// # Errors
/// Returns an error structure if an I/O error occurs
fn find_resource_files(dir: &Path,
                       current_file_name: &str,
                       datetime_dependent: bool,
                       pattern: &Regex,
                       compr_ext: &str) -> Result<Vec<AssociatedResFile>, CoalyException> {
    match std::fs::read_dir(dir) {
        Ok(dir_list) => {
            let mut files = Vec::<AssociatedResFile>::new();
            for entry in dir_list.flatten() {
                let elem_name = entry.file_name().to_string_lossy().to_string();
                if pattern.is_match(&elem_name) {
                    let act_flag = elem_name == current_file_name;
                    let caps = pattern.captures(&elem_name).unwrap();
                    let desc = AssociatedResFile::new(&caps, act_flag,
                                                      datetime_dependent, compr_ext);
                    files.push(desc);
                }
            }
            files.sort();
            Ok(files)
        },
        // read directory failed
        Err(e) => {
            Err(coalyxe!(E_ROVR_FAILED, current_file_name.to_string(), e.to_string()))
        }
    }
}

/// Shifts the names of all existing rollover files for an output resource.
///
/// # Arguments
/// * `dir` - the output directory for the resource
/// * `new_file_name` - the name for the active file after rollover
/// * `files` - sorted list with all existing rollover files, newest first
/// 
/// # Errors
/// Returns an error structure in case of an I/O error
fn shift_rollover_files(dir: &Path,
                        new_file_name: &str,
                        files: &[AssociatedResFile]) -> Result<(), CoalyException> {
    let file_count = files.len();
    for (i, f) in files.iter().skip(1).rev().enumerate() {
        let old_fn = f.file_name();
        if f.is_datetime_specific() {
            if i == file_count - 1 {
                if new_file_name != f.stem() { continue; }
            } else {
                if files[i+1].stem() != f.stem() { continue; }
            }
        }
        let old_path = dir.join(&old_fn);
        let new_path = dir.join(f.shifted_file_name());
        if let Err(e) = std::fs::rename(old_path, &new_path) {
            return Err(coalyxe!(E_ROVR_FAILED, old_fn, e.to_string()))
        }
    }
    Ok(())
}

/// Removes all existing rollover files exceeding the allowed keep count.
///
/// # Arguments
/// * `files` - sorted list with all existing rollover files, newest first
/// * `keep_count` - the maximum number of rollover files to keep
/// 
/// # Errors
/// Returns a list of error structures if one or more remove operations fail
fn remove_rollover_files<'a>(dir: &Path,
                         files: &'a[AssociatedResFile],
                         keep_count: u32) -> Result<&'a[AssociatedResFile], CoalyException> {
    let mut file_count: u32 = 0;
    for file_desc in files {
        file_count += 1;
        if file_count <= keep_count { continue }
        let file_name = file_desc.file_name();
        let file_path = dir.join(&file_name);
        if let Err(e) = std::fs::remove_file(&file_path) {
            return Err(coalyxe!(E_ROVR_FAILED, file_name, e.to_string()))
        }
    }
    let result_count = std::cmp::min(files.len(), keep_count as usize);
    Ok(&files[..result_count])
}

/// Descriptor for a file belonging to an output resource.
/// Covers active file and all archive files created by rollover.
#[derive(Clone,Debug)]
struct AssociatedResFile {
    stem: String,
    ext: String,
    seq_nr: usize,
    active_flag: bool,
    date_time_flag: bool,
    compr_ext: String
}
impl AssociatedResFile {
    /// Creates an output resource file descriptor.
    ///
    /// # Arguments
    /// * `caps` - regular expression captures for the file name
    /// * `active_flag` - indicates whether this is the active output file
    /// * `date_time_flag` - indicates whether the file name contains date/time related
    ///                      variables (true) or not (false)
    /// * `compr_ext` - compression specific file extension including dot, empty string for no
    ///                 compression
    fn new(caps: &Captures, active_flag: bool, date_time_flag: bool,
           compr_ext: &str) -> AssociatedResFile {
        let stem = caps.get(1).unwrap().as_str().to_string();
        let seq = if let Some(s) = caps.get(2) { s.as_str()[1..].to_string() }
                  else { String::from("0") };
        let seq_nr = usize::from_str_radix(&seq, 10).unwrap();
        let ext = if let Some(e) = caps.get(3) { e.as_str().to_string() } else { String::from("") };
        AssociatedResFile {
            stem,
            ext,
            seq_nr,
            active_flag,
            date_time_flag,
            compr_ext: compr_ext.to_string()
        }
    }

    /// Returns the pure name of the resource file.
    fn file_name(&self) -> String {
        let mut file_name = String::with_capacity(256);
        file_name.push_str(&self.stem);
        if self.seq_nr > 0 { file_name.push_str(&format!(".{}", self.seq_nr)); }
        file_name.push_str(&self.ext);
        file_name
    }

    /// Returns the pure name for the rename of the resource file in rollover.
    fn shifted_file_name(&self) -> String {
        let mut file_name = String::with_capacity(256);
        file_name.push_str(&self.stem);
        let new_seq_nr = format!(".{}", self.seq_nr + 1);
        file_name.push_str(&new_seq_nr);
        file_name.push_str(&self.compr_ext);
        file_name
    }

    /// Returns the file name's stem, without eventual sequence number and extension
    fn stem(&self) -> &str { &self.stem }

    /// Indicates whether this descriptor denotes a resource with date/time independent file name
    fn is_datetime_specific(&self) -> bool { self.date_time_flag }

    /// Indicates whether this descriptor denotes a resource with a sequence number in file name
    #[cfg(test)]
    fn has_seq_nr(&self) -> bool { self.seq_nr > 0 }
}
impl Ord for AssociatedResFile {
    fn cmp(&self, other: &Self) -> Ordering {
        (!self.active_flag, &other.stem, self.seq_nr).cmp(&(!other.active_flag, &self.stem, other.seq_nr))
    }
}
impl PartialOrd for AssociatedResFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for AssociatedResFile {
    fn eq(&self, other: &Self) -> bool {
        (self.active_flag, &self.stem, self.seq_nr) == (other.active_flag, &other.stem, other.seq_nr)
    }
}
impl Eq for AssociatedResFile { }

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write;
    use std::ops::Sub;
    use std::path::PathBuf;
    use std::str::FromStr;
    use super::*;
    use crate::output::resource::tests::{clear_test_dir, test_dir_path};

    const DEF_RES_NAME: &str = "myapp.log";
    const DEF_COMP_EXT: &str = ".gz";

    /// Returns the path or name for a file in rollover tests.
    ///
    /// # Arguments
    /// * `tf_path` - test function's output path
    /// * `res_name` - name spec of output resource with all variables replaced by actual values
    /// * `compr_ext` - compression specific file extension including dot, empty string for no
    ///                 compression
    /// * `num` - sequence number of rollover file, 0 for current output file
    fn res_file_name(res_name: &str, compr_ext: &str, num: usize) -> String {
        if num > 0 { format!("{}.{}{}", res_name, num, compr_ext) }
        else { format!("{}{}", res_name, compr_ext) }
    }
    fn res_file_path(tf_path: &Path, res_name: &str, compr_ext: &str, num: usize) -> PathBuf {
        tf_path.join(res_file_name(res_name, compr_ext, num))
    }

    /// Creates test files for an output resource.
    ///
    /// # Arguments
    /// * `tf_path` - test function output path
    /// * `name_spec` - output resource file name specification
    /// * `count` - number of rollover files to create, additional to "active" output file
    /// * `dtm_fmt` - the format strings to use for replacement of date/time variables
    /// * `compr_ext` - compression specific file extension including dot, empty string for no
    ///                 compression
    /// 
    /// # Return values
    /// sorted array with all rollover files from newer to older, "active" file first
    fn create_res_files(tf_path: &Path, name_spec: &FormatSpec, count: usize,
                        compr_ext: &str) -> Vec<PathBuf> {
        clear_test_dir(tf_path);
        let mut files = Vec::<PathBuf>::new();
        //let out_res_path = res_file_path(tf_path, &name, compr_ext, 0);
        let _ = std::fs::create_dir_all(tf_path);
        let mut name = name_spec.to_file_name();
        if name_spec.is_datetime_independent() {
            for i in 1..=count {
                let file_path = res_file_path(tf_path, &name, compr_ext, i);
                File::create(&file_path).unwrap();
                files.push(file_path);
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1200));
            let rovr_name = name_spec.to_file_name();
            let name_changes_every_sec = rovr_name != name;
            for _ in 1..=count {
                let mut seq_nr = if name_changes_every_sec { 0 } else { 1 };
                let mut rovr_path = res_file_path(tf_path, &name, compr_ext, seq_nr);
                while rovr_path.exists() {
                    seq_nr = seq_nr + 1;
                    rovr_path = res_file_path(tf_path, &name, compr_ext, seq_nr);
                }
                File::create(&rovr_path).unwrap();
                if seq_nr == 0 { files.insert(0, rovr_path); } else { files.push(rovr_path); }
                std::thread::sleep(std::time::Duration::from_millis(1200));
                name = name_spec.to_file_name();
            }
        }
        let name = name_spec.to_file_name();
        let out_res_path = res_file_path(tf_path, &name, "", 0);
        File::create(&out_res_path).unwrap();
        files.insert(0, out_res_path);
        files
    }

    fn create_assoc_res (stem: &str, active: bool, dtm_dep: bool, seq_nr: usize,
                         compr_ext: &str) -> AssociatedResFile {
        let ext = if active { String::from("") } else { compr_ext.to_string() };
        AssociatedResFile {
            stem: stem.to_string(),
            ext,
            seq_nr,
            active_flag: active,
            date_time_flag: dtm_dep,
            compr_ext: compr_ext.to_string()
        }
    }

    /// Creates given number of rollover files.
    /// Numbers contained in dir_indexes are created as directories instead of files to provoke
    /// an error during later deletion.
    fn create_rovr_res_files(tf_path: &Path, count: usize,
                             dir_indexes: &[usize]) -> Vec::<AssociatedResFile> {
        let mut files = Vec::<AssociatedResFile>::with_capacity(count);
        clear_test_dir(tf_path);
        for i in 1..=count {
            let file_path = res_file_path(tf_path, DEF_RES_NAME, DEF_COMP_EXT, i);
            let _ = std::fs::create_dir_all(file_path.parent().unwrap());
            if dir_indexes.contains(&i) {
                std::fs::create_dir(&file_path).unwrap();
            } else {
                File::create(&file_path).unwrap();
            }
            let resf = AssociatedResFile {
                            stem: String::from(DEF_RES_NAME),
                            ext: String::from(DEF_COMP_EXT),
                            seq_nr: i,
                            active_flag: false,
                            date_time_flag: false,
                            compr_ext: String::from(DEF_COMP_EXT)
            };
            files.push(resf);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        files
    }

    /// Checks result of a rollover file removal.
    ///
    /// # Arguments
    /// * `tfn` - test function name
    /// * `remove_result` - result of the remove operation
    /// * `exp_errs` - sequence numbers of rollover files where remove should have failed
    /// * `exp_keeps` - sequence numbers of rollover files expected to be kept
    /// * `exp_rems` - sequence numbers of rollover files expected to be removed
    fn check_rovr_file_removal(tf_path: &Path,
                               remove_result: &Result::<&[AssociatedResFile], CoalyException>,
                               exp_errs: &[usize],
                               exp_keeps: &[usize],
                               exp_rems: &[usize]) {
        let err_fns: Vec::<String> = exp_errs.iter()
                                             .map(|n| res_file_name(DEF_RES_NAME, DEF_COMP_EXT, *n))
                                             .collect();
        let kp_fns: Vec::<String> = exp_keeps.iter()
                                             .map(|n| res_file_name(DEF_RES_NAME, DEF_COMP_EXT, *n))
                                             .collect();
        let rem_fns: Vec::<String> = exp_rems.iter()
                                             .map(|n| res_file_name(DEF_RES_NAME, DEF_COMP_EXT, *n))
                                             .collect();
        // make sure all files where error is expected still exist
        for file_name in &err_fns {
            let file_path = tf_path.join(&file_name);
            assert!(file_path.exists(), "{}", file_name);
        }
        // make sure all files expected to be kept still exist
        for file_name in &kp_fns {
            let file_path = tf_path.join(&file_name);
            assert!(file_path.exists(), "{}", file_name);
        }
        // make sure all files expected to be removed don't exist
        for file_name in &rem_fns {
            let file_path = tf_path.join(&file_name);
            assert!(! file_path.exists(), "{}", file_name);
        }
        // check expected vs actual error
        if ! exp_errs.is_empty() { assert!(remove_result.is_err()); }
        if remove_result.is_err() { assert!(! exp_errs.is_empty()); }
    }

    fn run_shift_test(tf_path: &Path, fn_spec: &str, same_new_name: bool,
                      res_count: usize, compr_ext: &str) {
        let spec = FormatSpec::from_str(fn_spec).unwrap();
        let files = create_res_files(&tf_path, &spec, res_count, compr_ext);
        let cur_fn = files[0].file_name().unwrap().to_string_lossy();
        let new_fn = if same_new_name {
                         cur_fn.to_string()
                     } else {
                         std::thread::sleep(std::time::Duration::from_millis(1200));
                         spec.to_file_name()
                     };
        let dtm_dep = ! spec.is_datetime_independent();
        let pat = spec.file_name_pattern(compr_ext).unwrap();
        let rovr_files = files.iter().skip(1)
                              .map(|p| {
                                   let fname = p.file_name().unwrap().to_string_lossy();
                                   let caps = pat.captures(&fname).unwrap();
                                   AssociatedResFile::new(&caps, false, dtm_dep, compr_ext)
                               })
                              .collect::<Vec::<AssociatedResFile>>();
        let _ = shift_rollover_files(tf_path, &new_fn, &rovr_files);
        check_shift_files(tf_path, &cur_fn, &rovr_files, "");
    }

    /// Checks the function to shift all existing rollover files of an output resource.
    ///
    /// # Arguments
    /// * `tfn` - test function name
    /// * `cur_fn` - the name of the current output file
    /// * `rovr_files` - the file names to be renamed
    /// * `compr_ext` - the file extension indicating the compression algorithm,
    ///                 including leading dot, empty string if compression is not used
    fn check_shift_files(tf_path: &Path, cur_fn: &str,
                         rovr_files: &[AssociatedResFile], compr_ext: &str) {
        // current output file must exist
        let cur_file_path = res_file_path(tf_path, cur_fn, compr_ext, 0);
        assert!(cur_file_path.exists(), "{}", cur_fn);
        if rovr_files.is_empty() { return }
        // make sure all rollover files with sequence number have their number increased
        for rovr_f in rovr_files.iter().skip(1) {
            let rovr_fn = rovr_f.file_name();
            if rovr_f.has_seq_nr() {
                // rollover files with sequence number must have their number increased
                let new_fn = rovr_f.shifted_file_name();
                assert!(tf_path.join(&new_fn).exists(), "{}", new_fn);
            } else {
                // rollover files without sequence number must not be renamed
                assert!(tf_path.join(&rovr_fn).exists(), "{}", rovr_fn);
            }
        }
    }

    fn run_find_test(tf_path: &Path, fn_spec: &str, res_count: usize, compr_ext: &str) {
        let spec = FormatSpec::from_str(fn_spec).unwrap();
        let files = create_res_files(&tf_path, &spec, res_count, compr_ext);
        let cur_fn = files[0].file_name().unwrap().to_string_lossy();
        let indep = spec.is_datetime_independent();
        let find_pattern = spec.file_name_pattern(compr_ext).unwrap();
        let find_result = find_resource_files(&tf_path, &cur_fn, indep, &find_pattern, compr_ext);
        check_find_result(&tf_path, &files, &find_result);
    }

    fn run_single_assoc_res_test(fn_spec: &str, fn_ext: &str, active_flag: bool,
                                 exp_shifted_name: &str, compr_ext: &str) {
        let spec = FormatSpec::from_str(fn_spec).unwrap();
        let exp_name = format!("{}{}", spec.to_file_name(), fn_ext);
        let dtm_dep = ! spec.is_datetime_independent();
        let find_pattern = spec.file_name_pattern(compr_ext).unwrap();
        let caps = find_pattern.captures(&exp_name).unwrap();
        let fdesc = AssociatedResFile::new(&caps, active_flag, dtm_dep, compr_ext);
        let act_name = fdesc.file_name();
        let act_shifted_name = fdesc.shifted_file_name();
        assert_eq!(active_flag, fdesc.active_flag, "{}", &act_name);
        assert_eq!(exp_name, act_name, "{}", &act_name);
        assert_eq!(exp_shifted_name, &act_shifted_name, "{}", &act_name);
    }

    fn run_assoc_res_sort_test(descriptors: &[(AssociatedResFile, usize)]) {
        let mut res_files = descriptors.iter()
                                       .map(|f| f.0.clone())
                                       .collect::<Vec<AssociatedResFile>>();
        let order_map = descriptors.iter()
                                   .map(|f| return (f.1, f.0.clone()))
                                   .collect::<HashMap<usize, AssociatedResFile>>();
        res_files.sort();
        for (i, act_rf) in res_files.iter().enumerate() {
            let exp_rf = order_map.get(&i).unwrap();
            assert_eq!(exp_rf, act_rf);
        }
    }

    #[cfg(feature="compression")]
    fn run_arch_active_file(tf_path: &Path, file_name: &str, compression: &CompressionAlgorithm) {
        clear_test_dir(&tf_path);
        let compr_ext = compression.file_extension();
        let act_file_path = res_file_path(tf_path, file_name, "", 0);
        let arch_file_path = res_file_path(tf_path, file_name, compr_ext, 1);
        {
            let mut f = File::create(&act_file_path).unwrap();
            let _ = f.write_all(b"Testing rollover");
        }
        let ares = archive_active_file(&act_file_path,
                                       &arch_file_path,
                                       &compression);
        assert!(ares.is_ok());
        assert!(arch_file_path.exists());
        assert!(! act_file_path.exists());
    }

    fn create_resource_file(tf_path: &Path, file_name: &str) {
        let full_file_name = tf_path.join(file_name);
        let mut f = File::create(full_file_name).unwrap();
        let _ = f.write_all(b"LOGDATA");
        let _ = f.sync_all();
    }

    fn create_arch_files(tf_path: &Path, fn_spec: &FormatSpec,
                         dtm: &DateTime<Local>, arch_file_count: u32,
                         compression: &CompressionAlgorithm) -> Vec<PathBuf> {
        if arch_file_count == 0 { return vec!() }
        let mut files = Vec::<PathBuf>::with_capacity(arch_file_count as usize);
        for i in 0..arch_file_count {
            let seq_nr = if fn_spec.is_datetime_independent() { (i+1) as i64 } else { 0i64 };
            let file_name = arch_file_name_for(fn_spec, dtm, seq_nr, compression);
            let file_path = tf_path.join(&file_name);
            let mut f = File::create(&file_path).unwrap();
            let _ = f.write_all(b"LOGDATA");
            let _ = f.sync_all();
            files.push(file_path);
        }
        files
    }

    fn act_file_name_for(fn_spec: &FormatSpec, dtm: &DateTime<Local>, age: i64) -> String {
        let time_span = Duration::minutes(age);
        let f_date = dtm.sub(time_span);
        fn_spec.to_test_file_name(&f_date)
    }

    fn arch_file_name_for(fn_spec: &FormatSpec, dtm: &DateTime<Local>,
                          seq_nr: i64, compression: &CompressionAlgorithm) -> String {
        let time_span = Duration::minutes(seq_nr << 1);
        let f_date = dtm.sub(time_span);
        let file_name_stem = fn_spec.to_test_file_name(&f_date);
        let mut arch_file_name = String::with_capacity(256);
        arch_file_name.push_str(&file_name_stem);
        if seq_nr > 0 {
            arch_file_name.push('.');
            arch_file_name.push_str(&seq_nr.to_string());
        }
        arch_file_name.push_str(compression.file_extension());
        arch_file_name
    }

    fn check_archive_resource(tf_path: &Path, file_name_spec: &FormatSpec,
                              act_file_name: Option<String>, arch_files: &[PathBuf],
                              keep_count: u32, compression: &CompressionAlgorithm) {
        // active file must exist, if and only if it existed before archival,
        // it's name spec is date/time dependent and compression is not used
        if let Some(ref file_name) = act_file_name {
            let act_file_path = tf_path.join(file_name);
            if ! file_name_spec.is_datetime_independent() &&
               *compression == CompressionAlgorithm::None {
                assert!(act_file_path.exists());
            } else {
                assert!(! act_file_path.exists());
            }
        }
        // number of files in output path must be the same as before archival, except if
        // keep limit was exceeded
        let mut expected_file_count: usize = if act_file_name.is_some() { 1 } else { 0 };
        expected_file_count += arch_files.len();
        expected_file_count = std::cmp::min(expected_file_count, keep_count as usize); 
        let files = std::fs::read_dir(tf_path).unwrap();
        assert_eq!(expected_file_count, files.count());
    }

    fn run_archive_resource(tf_path: &Path, file_name_desc: &str,
                            act_file_exists: bool, arch_file_count: u32,
                            keep_count: u32, compression: &CompressionAlgorithm) {
        clear_test_dir(&tf_path);
        let file_name_spec = FormatSpec::from_str(file_name_desc).unwrap();
        let ref_date = NaiveDate::from_ymd(2021, 12, 31).and_hms(23, 50, 00);
        let ref_date = Local.from_local_datetime(&ref_date).unwrap();
        let act_file_name = act_file_name_for(&file_name_spec, &ref_date, 1);
        let new_file_name = act_file_name_for(&file_name_spec, &ref_date, 0);
        if act_file_exists { create_resource_file(tf_path, &act_file_name); }
        let arch_files = create_arch_files(tf_path, &file_name_spec, &ref_date,
                                           arch_file_count, compression);
        let res = archive_resource(&tf_path.to_path_buf(),
                                   &act_file_name,
                                   &new_file_name,
                                   &file_name_spec,
                                   keep_count,
                                   compression);
        // archival must succeed
        assert!(res.is_ok(), "archive operation failed");
        // check archival effect
        let act_file_name = if act_file_exists { Some(act_file_name) } else { None };
        check_archive_resource(tf_path, &file_name_spec, act_file_name, &arch_files,
                               keep_count, compression);
    }

    /// Checks the function to find all files belonging to a resource.
    ///
    /// # Arguments
    /// * `tfn` - test function name
    /// * `pat` - the pattern , wrapped in a Result
    /// * `exp_files` - the sorted list of files expected to be found with the descriptor
    /// * `find_result` - the actual files found
    fn check_find_result(tf_path: &Path, exp_files: &[PathBuf],
                         find_result: &Result<Vec<AssociatedResFile>, CoalyException>) {
        assert!(find_result.is_ok());
        let actual_files = find_result.as_ref().unwrap();
        assert_eq!(exp_files.len(), actual_files.len());
        for i in 0..exp_files.len() {
            let expected_fn = exp_files[i].to_string_lossy().to_string();
            let actual_fn = tf_path.join(&actual_files[i].file_name()).to_string_lossy().to_string();
            assert_eq!(expected_fn, actual_fn);
        }
    }

    #[test]
    /// Tests removal of older rollover files.
    fn test_remove_rollover_files() {
        let tf_path = test_dir_path(&["rollover", "test_remove_rollover_files"]);
        clear_test_dir(&tf_path);

        // standard case, oldest file must be removed
        let rovr_files = create_rovr_res_files(&tf_path, 3, &[]);
        let rovr_result = remove_rollover_files(&tf_path, &rovr_files, 2);
        check_rovr_file_removal(&tf_path, &rovr_result, &[], &[1,2], &[3]);

        // more than one file must be removed
        let rovr_files = create_rovr_res_files(&tf_path, 6, &[]);
        let rovr_result = remove_rollover_files(&tf_path, &rovr_files, 3);
        check_rovr_file_removal(&tf_path, &rovr_result, &[], &[1,2,3], &[4,5,6]);

        // no files to be removed, exact keep count
        let rovr_files = create_rovr_res_files(&tf_path, 3, &[]);
        let rovr_result = remove_rollover_files(&tf_path, &rovr_files, 3);
        check_rovr_file_removal(&tf_path, &rovr_result, &[], &[1,2,3], &[]);

        // no files to be removed, one less than keep count
        let rovr_files = create_rovr_res_files(&tf_path, 2, &[]);
        let rovr_result = remove_rollover_files(&tf_path, &rovr_files, 3);
        check_rovr_file_removal(&tf_path, &rovr_result, &[], &[1,2], &[]);

        // no files to be removed, no rollover files at all
        clear_test_dir(&tf_path);
        let rovr_result = remove_rollover_files(&tf_path, &[], 2);
        check_rovr_file_removal(&tf_path, &rovr_result, &[], &[], &[]);

        // one file can't be deleted
        let rovr_files = create_rovr_res_files(&tf_path, 3, &[3]);
        let rovr_result = remove_rollover_files(&tf_path, &rovr_files, 2);
        check_rovr_file_removal(&tf_path, &rovr_result, &[3], &[1,2,3], &[]);

        // two files can't be deleted
        let rovr_files = create_rovr_res_files(&tf_path, 4, &[3,4]);
        let rovr_result = remove_rollover_files(&tf_path, &rovr_files, 2);
        check_rovr_file_removal(&tf_path, &rovr_result, &[3,4], &[1,2,3,4], &[]);
    }

    #[test]
    /// Tests shift (renaming) of rollover files.
    fn test_shift_rollover_files() {
        // Test preparation, create files for another resource
        let tf_path = test_dir_path(&["rollover", "test_shift_rollover_files"]);
        clear_test_dir(&tf_path);
        let others_spec = FormatSpec::from_str("otherapp.log").unwrap();
        let _ = create_res_files(&tf_path, &others_spec, 3, ".zip");

        // date/time independent, no compression
        run_shift_test(&tf_path, "myapp.log", true, 3, "");

        // date/time independent, with compression
        run_shift_test(&tf_path, "myapp.log", true, 3, ".zip");

        // timestamp, no compression
        run_shift_test(&tf_path, "myapp_$TimeStamp.log", false, 3, "");

        // timestamp, with compression
        run_shift_test(&tf_path, "myapp_$TimeStamp.log", false, 3, ".zip");

        // date, no compression
        run_shift_test(&tf_path, "myapp_$Date.log", true, 3, "");

        // date, with compression
        run_shift_test(&tf_path, "myapp_$Date.log", true, 3, ".zip");
    }

    #[test]
    /// Tests function to find and sort resource files
    fn test_find_resource_files() {
        // Test preparation, create files for another resource
        let tf_path = test_dir_path(&["rollover", "test_find_resource_files"]);
        clear_test_dir(&tf_path);
        let others_spec = FormatSpec::from_str("otherapp.log").unwrap();
        let _ = create_res_files(&tf_path, &others_spec, 3, ".zip");

        // date/time independent, no compression
        run_find_test(&tf_path, "myapp.log", 3, "");

        // date/time independent, zip compression
        run_find_test(&tf_path, "myapp.log", 3, ".zip");

        // timestamp, no compression
        run_find_test(&tf_path, "myapp_$TimeStamp.log", 3, "");

        // timestamp, compression gz
        run_find_test(&tf_path, "myapp_$TimeStamp.log", 3, ".gz");

        // date, no compression
        run_find_test(&tf_path, "myapp_$Date_thread22.log", 3, "");

        // date, compression gz
        run_find_test(&tf_path, "myapp_$Date_thread24.log", 3, ".gz");

        // time, no compression
        run_find_test(&tf_path, "myapp_$Time.log", 3, "");

        // time, compression gz
        run_find_test(&tf_path, "myapp_$Time.log", 3, ".gz");

        // date/time mix, no compression
        run_find_test(&tf_path, "myapp_$Date_thread07_$Time.log", 3, "");

        // date/time mix, compression gz
        run_find_test(&tf_path, "myapp_$Time_thread_$Date_08.log", 3, ".gz");
    }

    #[test]
    /// Tests descriptor structure for files belonging to a resource
    fn test_associated_res_file() {
        // active file, no compression
        run_single_assoc_res_test("myapp.log", "", true, "myapp.log.1", "");
        // active file, compression zip
        run_single_assoc_res_test("myapp.log", "", true, "myapp.log.1.zip", ".zip");
        // same-digit rollover file, no compression
        run_single_assoc_res_test("myapp.log", ".1", false, "myapp.log.2", "");
        // same-digit rollover file, compression zip
        run_single_assoc_res_test("myapp.log", ".5", false, "myapp.log.6.zip", ".zip");
        // digit change rollover file, no compression
        run_single_assoc_res_test("myapp.log", ".9", false, "myapp.log.10", "");
        // digit change rollover file, compression zip
        run_single_assoc_res_test("myapp.log", ".99", false, "myapp.log.100.zip", ".zip");

        // simple sort test
        let descs = [(create_assoc_res("myapp.log",false,false,3,""), 3),
                     (create_assoc_res("myapp.log",false,false,2,""), 2),
                     (create_assoc_res("myapp.log",false,false,1,""), 1),
                     (create_assoc_res("myapp.log",true,false,0,""), 0)];
        run_assoc_res_sort_test(&descs);

        // complex sort test
        let descs = [(create_assoc_res("myapp_20220303.log",true,true,0,""), 0),
                     (create_assoc_res("myapp_20220302.log",false,true,0,".gz"), 1),
                     (create_assoc_res("myapp_20220301.log",false,true,1,".gz"), 3),
                     (create_assoc_res("myapp_20220301.log",false,true,0,".gz"), 2)];
        run_assoc_res_sort_test(&descs);
    }

    #[cfg(feature="compression")]
    #[test]
    /// Tests archival of active file
    fn test_archive_active_file() {
        let tf_path = test_dir_path(&["rollover", "test_archive_active_file"]);
        let _ = std::fs::create_dir_all(&tf_path);
        // no compression
        run_arch_active_file(&tf_path, "myapp.log", &CompressionAlgorithm::None);
        // gzip compression
        run_arch_active_file(&tf_path, "myapp.log", &CompressionAlgorithm::Gzip);
        // zip compression
        run_arch_active_file(&tf_path, "myapp.log", &CompressionAlgorithm::Zip);
        // bzip2 compression
        run_arch_active_file(&tf_path, "myapp.log", &CompressionAlgorithm::Bzip2);
        // lzma compression
        run_arch_active_file(&tf_path, "myapp.log", &CompressionAlgorithm::Lzma);
    }

    #[test]
    /// Tests archival of active file
    fn test_archive_resource() {
        let tf_path = test_dir_path(&["rollover", "test_archive_resource"]);
        let _ = std::fs::create_dir_all(&tf_path);

        // no compression, date/time independent, no files at all
        run_archive_resource(&tf_path, "myapp.log", false, 0, 2, &CompressionAlgorithm::None);
        // no compression, date/time independent, no archive files
        run_archive_resource(&tf_path, "myapp.log", true, 0, 2, &CompressionAlgorithm::None);
        // no compression, date/time independent, archive file count 1 below keep limit
        run_archive_resource(&tf_path, "myapp.log", true, 1, 2, &CompressionAlgorithm::None);
        // no compression, date/time independent, archive file count at keep limit
        run_archive_resource(&tf_path, "myapp.log", true, 2, 2, &CompressionAlgorithm::None);
        // no compression, date/time dependent, no archive files
        run_archive_resource(&tf_path, "myapp_$TimeStamp.log", true, 0, 2, &CompressionAlgorithm::None);
        // no compression, date/time dependent, archive file count 1 below keep limit
        run_archive_resource(&tf_path, "myapp_$TimeStamp.log", true, 1, 2, &CompressionAlgorithm::None);
        // no compression, date/time dependent, archive file count at keep limit
        run_archive_resource(&tf_path, "myapp_$TimeStamp.log", true, 2, 2, &CompressionAlgorithm::None);

        // compression gzip, date/time independent, no files at all
        run_archive_resource(&tf_path, "myapp.log", false, 0, 2, &CompressionAlgorithm::Gzip);
        // compression gzip, date/time independent, no archive files
        run_archive_resource(&tf_path, "myapp.log", true, 0, 2, &CompressionAlgorithm::Gzip);
        // compression gzip, date/time independent, archive file count 1 below keep limit
        run_archive_resource(&tf_path, "myapp.log", true, 1, 2, &CompressionAlgorithm::Gzip);
        // compression gzip, date/time independent, archive file count at keep limit
        run_archive_resource(&tf_path, "myapp.log", true, 2, 2, &CompressionAlgorithm::Gzip);
        // compression gzip, date/time dependent, no archive files
        run_archive_resource(&tf_path, "myapp_$TimeStamp.log", true, 0, 2, &CompressionAlgorithm::Gzip);
        // compression gzip, date/time dependent, archive file count 1 below keep limit
        run_archive_resource(&tf_path, "myapp_$TimeStamp.log", true, 1, 2, &CompressionAlgorithm::Gzip);
        // compression gzip, date/time dependent, archive file count at keep limit
        run_archive_resource(&tf_path, "myapp_$TimeStamp.log", true, 2, 2, &CompressionAlgorithm::Gzip);
    }
}
