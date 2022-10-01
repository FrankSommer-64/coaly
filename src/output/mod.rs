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

//! Top level module for output handling.

mod formatspec;
pub mod inventory;
mod outputformat;
mod recordbuffer;
mod recordformat;
pub(crate) mod resource;
pub mod standaloneinventory;
#[cfg(feature="net")]
pub mod serverinventory;

use crate::errorhandling::CoalyException;
use crate::record::recorddata::RecordData;
use resource::ResourceRef;
use outputformat::OutputFormat;

/// An output interface contains all output resources for a thread.
/// Process wide resources shared by all threads are also included.
#[derive(Clone)]
pub(crate) struct Interface {
    // all potential output resources bundled by the interface, including output formats
    resources: Vec<(OutputFormat, ResourceRef)>,
    // holds all errors to be reported to the caller after a write operation
    errors: Vec<CoalyException>
}
impl Interface {
    /// Creates an output interface containing the specified output resources.
    /// The interface takes ownership of the vector only, the contained resources
    /// are shared between the interface and the inventory maintaining all resources.
    /// 
    /// # Arguments
    /// * `resources` - the resources for the interface
    pub(crate) fn new(resources: Vec<(OutputFormat, ResourceRef)>) -> Interface {
        Interface { resources, errors: Vec::<CoalyException>::new() }
    }

    /// Writes a log or trace record.
    /// The record is written to all resources associated with the record's level.
    /// The check whether the record level is enabled should be done by the caller.
    /// 
    /// # Arguments
    /// * `record` - the log or trace record
    /// * `use_buffer` - indicates whether to buffer the record in memory instead of writing to
    ///                  physical resource
    /// 
    /// # Errors
    /// Returns a vector with error structures if the write operation to one or more resources
    /// failed
    pub(crate) fn write(&mut self,
                        record: &dyn RecordData,
                        use_buffer: bool) -> Result<(), Vec<CoalyException>> {
        self.errors.clear();
        for (f, r) in &self.resources {
            if let Err(m) = r.borrow_mut().write(record, f, use_buffer) {
                self.errors.extend_from_slice(&m);
            }
        }
        if self.errors.is_empty() { return Ok(()) }
        Err(self.errors.clone())
    }
}
