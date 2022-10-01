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

//! Coaly is a library for handling log and trace messages.
//! Besides the usual levels indicating message severity it offers various formatting options 
//! and supports files, memory mapped files and remote servers over network as output resource.
//! Advanced features are output buffering in main memory and configurable behaviour dependent
//! on the location in the code. E.g. for one module or function all levels may be switched off,
//! whereas for another messages of all levels are included in the output.
//! The behaviour is entirely specified in a configuration file that is read once at application
//! start and cannot be changed during runtime.

#[macro_use]
extern crate lazy_static;

pub mod agent;
pub mod collections;
pub mod config;
pub mod errorhandling;
pub mod observer;
pub mod output;
pub mod util;
mod datetime;
mod event;
mod modechange;
mod policies;
mod record;
mod variables;

use observer::ObserverData;
pub use errorhandling::CoalyException;
pub use record::originator::OriginatorInfo;
pub use record::RecordLevelId;

#[cfg(feature="net")]
pub mod net;

/// Result type used throughout the library for error handling
pub type CoalyResult<T> = Result<T, CoalyException>;


/// Initializes the system.
/// 
/// If the function has not been called prior to any message output, the system will assume
/// default settings. This is also the case, if an error during configuration file processing
/// occurs.
/// Calling the function for an already initialized system has no effect.
/// 
/// # Arguments
/// * `config_file_name` - the name of the configuration file
#[inline]
pub fn initialize(config_file_name: &str) { agent::initialize(config_file_name); }

/// Terminates the system.
#[inline]
pub fn shutdown() { agent::shutdown(); }

/// Writes a log message with level alert.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logalert {
    ($msg: literal) => {
        agent::write(RecordLevelId::Alert, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Alert, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a log message with level critical.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logcrit {
    ($msg: literal) => {
        agent::write(RecordLevelId::Critical, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Critical, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a trace message with level debug.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logdebug {
    ($msg: literal) => {
        agent::write(RecordLevelId::Debug, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Debug, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a log message with level emergency.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logemgcy {
    ($msg: literal) => {
        agent::write(RecordLevelId::Emergency, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Emergency, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a log message with level error.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logerror {
    ($msg: literal) => {
        agent::write(RecordLevelId::Error, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Error, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a log message with level information.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! loginfo {
    ($msg: literal) => {
        agent::write(RecordLevelId::Info, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Info, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a trace message with level notice.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! lognote {
    ($msg: literal) => {
        agent::write(RecordLevelId::Notice, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Notice, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Writes a log message with level warning.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logwarn {
    ($msg: literal) => {
        agent::write(RecordLevelId::Warning, std::file!(), std::line!(), $msg);
    };
    ($($arg:tt)+) => {
        agent::write(RecordLevelId::Warning, std::file!(), std::line!(), &std::fmt::format(format_args!($($arg)+)));
    }
}

/// Traces a function's boundaries.
/// Writes immediately a record upon the entry of the function and another message upon
/// leaving of the function using the drop method of the instantiated Coaly observer structure.
/// Depending on the configuration, the system's behaviour may change after the function
/// entry.
/// Function parameters can optionally be traced by additional arguments separated with a comma.
/// Each argument value must be convertible to a string.
/// 
/// # Arguments
/// * `func_name` - the name of the function
/// * `args` - optional the arguments of the function call, each argument prepended by a comma
#[macro_export]
macro_rules! logfn {
    ($func_name: literal) => {
        let _cfn = CoalyObserver::for_fn($func_name, None, std::file!(),std::line!());
    };
    ($func_name: literal $(,$arg: expr)+) => {
        let arg_str = String::new();
        $(
            let arg_str = if arg_str.len() == 0 { arg_str + &format!("{}", $arg) }
                          else { arg_str + &format!(",{}", $arg) };  
        )+
        let _cfn = CoalyObserver::for_fn($func_name, Option::from(arg_str.as_str()),
                                         std::file!(),std::line!());
    };
}

/// Traces a module's boundaries.
/// Writes immediately a record upon the entry of the module and another message upon
/// leaving of the module using the drop method of the instantiated Coaly observer structure.
/// Depending on the configuration, the system's behaviour may change after the module
/// entry.
/// 
/// # Arguments
/// * `module_name` - the name of the module
#[macro_export]
macro_rules! logmod {
    ($module_name: literal) => {
        let _cmod = CoalyObserver::for_mod($module_name, std::file!(), std::line!());
    }
}

/// Writes a log message concerning the specified application object.
/// Object level is used for the message.
/// The application object must implement the CoalyObservable trait.
/// 
/// # Arguments
/// * `msg` - the message
#[macro_export]
macro_rules! logobj {
    ($obj: expr, $msg: literal) => {
        agent::write_obs($obj, std::file!(), std::line!(), $msg);
    }
}

/// Creates and returns an observer structure.
/// Writes immediately an output record upon instantiation of the structure and another record
/// upon drop.
/// Depending on the configuration, the system's behaviour may change after the structure
/// instantiation.
/// 
/// # Arguments
/// * `obj_name` - the name of the user defined observer
/// * `obj_value` - the optional value of the user defined observer
#[macro_export]
macro_rules! newcoalyobs {
    ($obj_name: expr) => {
        CoalyObserver::for_obj($obj_name, None, std::file!(),std::line!())
    };
    ($obj_name: expr ,$obj_value: expr) => {
        CoalyObserver::for_obj($obj_name, Option::from($obj_value), std::file!(),std::line!())
    };
}

/// Coaly observer structure.
/// An observer structure is created upon entry of a function or during instantiation of a logging
/// relevant user structure.
/// An observer structure marks both beginning and end of a function or structure lifetime, it is
/// the basis for output mode control.
pub struct CoalyObserver(ObserverData);
impl CoalyObserver {
    /// Creates an observer structure for a function
    ///
    /// # Arguments
    /// * `name` - the function's name
    /// * `args` - the optional function's' arguments
    /// * `file_name` - the name of the source code file where the structure was created
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub fn for_fn(name: &'static str,
                  args: Option<&str>,
                  file_name: &'static str,
                  line_nr: u32) -> CoalyObserver {
        let data = ObserverData::for_fn(name, args, file_name);
        agent::observer_created(&data, line_nr);
        CoalyObserver { 0: data }
    }

    /// Creates an observer structure for a module.
    /// Since a module doesn't provide an entry point by itself, it must be created at the
    /// beginning of a module's public function.
    ///
    /// # Arguments
    /// * `name` - the module's name
    /// * `file_name` - the name of the source code file where the structure was created
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub fn for_mod(name: &'static str,
                   file_name: &'static str,
                   line_nr: u32) -> CoalyObserver {
        let data = ObserverData::for_mod(name, file_name);
        agent::observer_created(&data, line_nr);
        CoalyObserver { 0: data }
    }

    /// Creates an observer structure for a logging relevant user object
    ///
    /// # Arguments
    /// * `name` - the object's name
    /// * `value` - the optional object's value
    /// * `file_name` - the name of the source code file where the structure was created
    /// * `line_nr` - the line number in the source code file where the structure was created
    pub fn for_obj(name: &str,
                  value: Option<&str>,
                  file_name: &'static str,
                  line_nr: u32) -> CoalyObserver {
        let data = ObserverData::for_obj(name, value, file_name);
        agent::observer_created(&data, line_nr);
        CoalyObserver { 0: data }
    }
}
impl Drop for CoalyObserver {
    /// Invoked automatically when the observer structure goes out of scope.
    /// Writes an output record indicating that the structure has been dropped and may also revert
    /// the changes in the system behaviour to the status before the struct was created.
    fn drop(&mut self) { agent::observer_dropped(&self.0); }
}

pub trait CoalyObservable {
    /// Returns a reference to the Coaly observer structure
    fn coaly_observer(&self) -> &CoalyObserver;
}
