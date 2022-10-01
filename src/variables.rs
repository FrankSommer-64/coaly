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

//! Coaly placeholder variables, can be used in format and file name descriptors.

use regex::Regex;
use std::collections::BTreeMap;
use std::collections::btree_map::Iter;
use std::fmt::{Debug, Formatter};
use std::str::FromStr;

/// Names of all supported placeholder variables
pub(crate) const VAR_NAME_APP_ID: &str = "AppId";
pub(crate) const VAR_NAME_APP_NAME: &str = "AppName";
pub(crate) const VAR_NAME_DATE: &str = "Date";
pub(crate) const VAR_NAME_ENV: &str = "Env";
pub(crate) const VAR_NAME_HOST_NAME: &str = "HostName";
pub(crate) const VAR_NAME_IP_ADDR: &str = "IpAddress";
pub(crate) const VAR_NAME_LEVEL: &str = "Level";
pub(crate) const VAR_NAME_LEVEL_ID: &str = "LevelId";
pub(crate) const VAR_NAME_MESSAGE: &str = "Message";
pub(crate) const VAR_NAME_OBSERVER_NAME: &str = "ObserverName";
pub(crate) const VAR_NAME_OBSERVER_VALUE: &str = "ObserverValue";
pub(crate) const VAR_NAME_PROCESS_ID: &str = "ProcessId";
pub(crate) const VAR_NAME_PROCESS_NAME: &str = "ProcessName";
pub(crate) const VAR_NAME_PURE_SOURCE_FILE_NAME: &str = "PureSourceFileName";
pub(crate) const VAR_NAME_SOURCE_FILE_NAME: &str = "SourceFileName";
pub(crate) const VAR_NAME_SOURCE_LINE_NR: &str = "SourceLineNr";
pub(crate) const VAR_NAME_THREAD_ID: &str = "ThreadId";
pub(crate) const VAR_NAME_THREAD_NAME: &str = "ThreadName";
pub(crate) const VAR_NAME_TIME: &str = "Time";
pub(crate) const VAR_NAME_TIME_STAMP: &str = "TimeStamp";

/// Variables that may be used in record formats and/or file names inside the configuration file.
#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) enum Variable {
    // user defined application ID
    ApplicationId,
    // user defined application name
    ApplicationName,
    // current date
    Date,
    // environment variable
    Env(String),
    // host name
    HostName,
    // host's IP address (V4 or V6)
    IpAddress,
    // record level of the log or trace message
    Level,
    // record level ID character of the log or trace message
    LevelId,
    // log or trace message issued by the application
    Message,
    // name of the observer struct that triggered the event
    ObserverName,
    // user defined value of the observer struct that triggered the event
    ObserverValue,
    // process ID of the application
    ProcessId,
    // process (executable) name of the application
    ProcessName,
    // name of the source file that issued the log or trace, without path
    PureSourceFileName,
    // name of the source file that issued the log or trace, including path beginning under src
    SourceFileName, 
    // line number in the source file, where a log or trace message was issued
    SourceLineNr,
    // ID of the thread that issued the log or trace message
    ThreadId,
    // user defined name of the thread that issued the log or trace message, defaults to thread ID
    ThreadName,
    // current time
    Time,
    // current date and time
    TimeStamp
}
impl Debug for Variable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Variable::Env(v) = self {
            return write!(f, "{}[{}]", VAR_NAME_ENV, v)
        }
        write!(f, "{}", match self {
            Variable::ApplicationId => VAR_NAME_APP_ID,
            Variable::ApplicationName => VAR_NAME_APP_NAME,
            Variable::Date => VAR_NAME_DATE,
            Variable::Env(_) => "",
            Variable::HostName => VAR_NAME_HOST_NAME,
            Variable::IpAddress => VAR_NAME_IP_ADDR,
            Variable::Level => VAR_NAME_LEVEL,
            Variable::LevelId => VAR_NAME_LEVEL_ID,
            Variable::Message => VAR_NAME_MESSAGE,
            Variable::ObserverName => VAR_NAME_OBSERVER_NAME,
            Variable::ObserverValue => VAR_NAME_OBSERVER_VALUE,
            Variable::ProcessId => VAR_NAME_PROCESS_ID,
            Variable::ProcessName => VAR_NAME_PROCESS_NAME,
            Variable::PureSourceFileName => VAR_NAME_PURE_SOURCE_FILE_NAME,
            Variable::SourceFileName => VAR_NAME_SOURCE_FILE_NAME, 
            Variable::SourceLineNr => VAR_NAME_SOURCE_LINE_NR,
            Variable::ThreadId => VAR_NAME_THREAD_ID,
            Variable::ThreadName => VAR_NAME_THREAD_NAME,
            Variable::Time => VAR_NAME_TIME,
            Variable::TimeStamp => VAR_NAME_TIME_STAMP
        })
    }
}
impl FromStr for Variable {
    type Err = bool;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(grps) = Regex::new(ENV_VAR_PATTERN).unwrap().captures(s) {
            return Ok(Variable::Env(grps.get(1).unwrap().as_str().to_string()))
        }
        match s {
            VAR_NAME_APP_ID => Ok(Variable::ApplicationId),
            VAR_NAME_APP_NAME => Ok(Variable::ApplicationName),
            VAR_NAME_DATE => Ok(Variable::Date),
            VAR_NAME_HOST_NAME => Ok(Variable::HostName),
            VAR_NAME_IP_ADDR => Ok(Variable::IpAddress),
            VAR_NAME_LEVEL => Ok(Variable::Level),
            VAR_NAME_LEVEL_ID => Ok(Variable::LevelId),
            VAR_NAME_MESSAGE => Ok(Variable::Message),
            VAR_NAME_OBSERVER_NAME => Ok(Variable::ObserverName),
            VAR_NAME_OBSERVER_VALUE => Ok(Variable::ObserverValue),
            VAR_NAME_PROCESS_ID => Ok(Variable::ProcessId),
            VAR_NAME_PROCESS_NAME => Ok(Variable::ProcessName),
            VAR_NAME_PURE_SOURCE_FILE_NAME => Ok(Variable::PureSourceFileName),
            VAR_NAME_SOURCE_FILE_NAME => Ok(Variable::SourceFileName), 
            VAR_NAME_SOURCE_LINE_NR => Ok(Variable::SourceLineNr),
            VAR_NAME_THREAD_ID => Ok(Variable::ThreadId),
            VAR_NAME_THREAD_NAME => Ok(Variable::ThreadName),
            VAR_NAME_TIME => Ok(Variable::Time),
            VAR_NAME_TIME_STAMP => Ok(Variable::TimeStamp),
            _ => Err(false)
        }
    }
}

pub(crate) struct VariableMap(BTreeMap<&'static str, Variable>);
impl VariableMap {
    /// Returns an iterator over the entries of the map
    pub(crate) fn iter(&self) -> Iter<&'static str, Variable> { self.0.iter() }

    /// Returns a reference to the placeholder variable with the specified name.
    ///
    /// # Arguments
    /// * `var_name` - the variable name
    ///
    /// # Return values
    /// the Variable matching the given name; **None** if no such variable exists
    #[cfg(test)]
    pub(crate) fn get(&self, var_name: &str) -> Option<&Variable> { self.0.get(var_name) }
}
impl Default for VariableMap {
    fn default() -> Self {
        let mut m = BTreeMap::<&'static str, Variable>::new();
        m.insert(VAR_NAME_APP_ID, Variable::ApplicationId);
        m.insert(VAR_NAME_APP_NAME, Variable::ApplicationName);
        m.insert(VAR_NAME_DATE, Variable::Date);
        m.insert(VAR_NAME_ENV, Variable::Env(String::from("")));
        m.insert(VAR_NAME_HOST_NAME, Variable::HostName);
        m.insert(VAR_NAME_IP_ADDR, Variable::IpAddress);
        m.insert(VAR_NAME_LEVEL, Variable::Level);
        m.insert(VAR_NAME_LEVEL_ID, Variable::LevelId);
        m.insert(VAR_NAME_MESSAGE, Variable::Message);
        m.insert(VAR_NAME_OBSERVER_NAME, Variable::ObserverName);
        m.insert(VAR_NAME_OBSERVER_VALUE, Variable::ObserverValue);
        m.insert(VAR_NAME_PROCESS_ID, Variable::ProcessId);
        m.insert(VAR_NAME_PROCESS_NAME, Variable::ProcessName);
        m.insert(VAR_NAME_PURE_SOURCE_FILE_NAME, Variable::PureSourceFileName);
        m.insert(VAR_NAME_SOURCE_FILE_NAME, Variable::SourceFileName);
        m.insert(VAR_NAME_SOURCE_LINE_NR, Variable::SourceLineNr);
        m.insert(VAR_NAME_THREAD_ID, Variable::ThreadId);
        m.insert(VAR_NAME_THREAD_NAME, Variable::ThreadName);
        m.insert(VAR_NAME_TIME, Variable::Time);
        m.insert(VAR_NAME_TIME_STAMP, Variable::TimeStamp);
        Self { 0: m }
    }
}

const ENV_VAR_PATTERN: &str = r"^Env\[(.*)\]$";
