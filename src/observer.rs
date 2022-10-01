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

//! Coaly observer types

use std::fmt::{Debug, Formatter};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

/// Kinds of observer structs that may control the output settings for log and trace records
#[derive (Clone, Copy, PartialEq)]
#[repr(u32)]
pub(crate) enum ObserverKind {
    /// function
    Function = 0b000100000000,
    /// module
    Module = 0b001000000000,
    /// custom application structure
    Object = 0b010000000000
}
impl Debug for ObserverKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ObserverKind::Function => write!(f, "{}", OBSERVER_KIND_FUNCTION),
            ObserverKind::Module => write!(f, "{}", OBSERVER_KIND_MODULE),
            ObserverKind::Object => write!(f, "{}", OBSERVER_KIND_OBJECT)
        }
    }
}
impl FromStr for ObserverKind {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            OBSERVER_KIND_FUNCTION => Ok(ObserverKind::Function),
            OBSERVER_KIND_MODULE => Ok(ObserverKind::Module),
            OBSERVER_KIND_OBJECT => Ok(ObserverKind::Object),
            _ => Err(false)
        }
    }
}

/// Observer data, a descriptor for a function, a module or a user defined object.
pub struct ObserverData {
    // the unique observer ID
    id: u64,
    // the name of the observer structure, supplied by user
    name: String,
    // the name of the source code file where the structure is created, from macro std::file!
    file_name: &'static str,
    // the kind of the observer structure
    kind: ObserverKind,
    // the optional value of the observer structure, used for user defined observers only
    value: Option<String>
}
impl ObserverData {
    /// Creates an observer descriptor structure for a function
    ///
    /// # Arguments
    /// * `name` - the name of the function
    /// * `args` - the optional function arguments
    /// * `file_name` - the name of the source code file where the structure was created
    pub(crate) fn for_fn(name: &'static str,
                         args: Option<&str>,
                         file_name: &'static str) -> ObserverData {
        ObserverData {
            id: CURR_OBSERVER_ID.fetch_add(1, Ordering::SeqCst),
            kind: ObserverKind::Function,
            name: name.to_string(), file_name, value: args.map(str::to_string)
        }
    }

    /// Creates an observer descriptor structure for a module
    ///
    /// # Arguments
    /// * `name` - the name of the module
    /// * `file_name` - the name of the source code file where the structure was created
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub(crate) fn for_mod(name: &'static str,
                          file_name: &'static str) -> ObserverData {
        ObserverData {
            id: CURR_OBSERVER_ID.fetch_add(1, Ordering::SeqCst),
            kind: ObserverKind::Module,
            name: name.to_string(), file_name, value: None
        }
    }

    /// Creates a new observer structure for a user defined object
    ///
    /// # Arguments
    /// * `name` - the name of the object
    /// * `value` - the optional object value
    /// * `file_name` - the name of the source code file where the structure was created
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub(crate) fn for_obj(name: &str,
                          value: Option<&str>,
                          file_name: &'static str) -> ObserverData {
        ObserverData {
            id: CURR_OBSERVER_ID.fetch_add(1, Ordering::SeqCst),
            kind: ObserverKind::Object,
            name: name.to_string(), file_name, value: value.map(str::to_string)
        }
    }

    /// Returns the observer ID
    #[inline]
    pub(crate) fn id(&self) -> u64 { self.id }

    /// Returns the observer name
    #[inline]
    pub(crate) fn name(&self) -> &String { &self.name }

    /// Returns the name of the source code file where the structure is created
    #[inline]
    pub(crate) fn file_name(&self) -> &'static str { self.file_name }

    /// Returns the kind of the observer structure
    #[inline]
    pub(crate) fn kind(&self) -> &ObserverKind { &self.kind }

    /// Returns the optional value of the observer structure
    #[inline]
    pub(crate) fn value(&self) -> &Option<String> { &self.value }
}

static CURR_OBSERVER_ID: AtomicU64 = AtomicU64::new(1);

// Observer kind names
const OBSERVER_KIND_FUNCTION: &str = "function";
const OBSERVER_KIND_MODULE: &str = "module";
const OBSERVER_KIND_OBJECT: &str = "object";
