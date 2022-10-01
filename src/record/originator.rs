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

//! Data structure indicating the originator of a log or trace record.

use std::collections::BTreeMap;

#[cfg(feature="net")]
use crate::CoalyException;

#[cfg(feature="net")]
use crate::net::serializable::{Serializable};


/// Information about the originator of a log or trace message when sent to a remote server.
/// Also used locally to replace variables used in record formats or file names
/// with their actual values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginatorInfo {
     process_id: u32,
     process_name: String,
     application_id: u32,
     application_name: String,
     host_name: String,
     ip_address: String,
     env_vars: BTreeMap<String, String>
}

impl OriginatorInfo {
    /// Creates originator information structure.
    /// 
    /// # Arguments
    /// * `pid` - the process ID of the application
    /// * `pname` - the process name of the application (name of executable file)
    /// * `host` - the local hostname
    /// * `ip` - the local IP address
    pub fn new(pid: u32,
               pname: &str,
               host: &str,
               ip: &str) -> OriginatorInfo {
        OriginatorInfo {
            process_id: pid,
            process_name: pname.to_string(),
            application_id: 0,
            application_name: String::from(""),
            host_name: host.to_string(),
            ip_address: ip.to_string(),
            env_vars: BTreeMap::<String,String>::new()
        }
    }

    /// Returns the process ID as string
    #[inline]
    pub fn process_id(&self) -> String { self.process_id.to_string() }

    /// Returns the process ID as numeric value
    #[inline]
    pub fn process_id_value(&self) -> u32 { self.process_id }

    /// Returns the process name
    #[inline]
    pub fn process_name(&self) -> &str { &self.process_name }

    /// Returns the application ID as string
    #[inline]
    pub fn application_id(&self) -> String { self.application_id.to_string() }

    /// Returns the application ID as numeric value
    #[inline]
    pub fn application_id_value(&self) -> u32 { self.application_id }

    /// Sets the application name
    #[inline]
    pub fn set_application_id(&mut self, app_id: u32) { self.application_id = app_id }

    /// Returns the application name
    #[inline]
    pub fn application_name(&self) -> &str { &self.application_name }

    /// Sets the application name
    #[inline]
    pub fn set_application_name(&mut self, name: &str) { self.application_name = name.to_string() }

    /// Returns the host name
    #[inline]
    pub fn host_name(&self) -> &str { &self.host_name }

    /// Returns the IP host address
    #[inline]
    pub fn ip_address(&self) -> &str { &self.ip_address }

    /// Returns the value of the given environment variable name, if defined
    #[inline]
    pub fn env_var_value(&self, var_name: &str) -> Option<&String> { self.env_vars.get(var_name) }

    /// Adds name and value of an environment variable
    #[inline]
    pub fn add_env_var(&mut self, name: &str, value: &str) {
        self.env_vars.insert(name.to_string(), value.to_string());
    }
}
#[cfg(feature="net")]
impl<'a> Serializable<'a> for OriginatorInfo {
    fn serialized_size(&self) -> usize {
        self.process_id.serialized_size() +
        self.process_name.serialized_size() +
        self.application_id.serialized_size() +
        self.application_name.serialized_size() +
        self.host_name.serialized_size() +
        self.ip_address.serialized_size() +
        self.env_vars.serialized_size()
    }
    fn serialize_to(&self, buffer: &mut Vec<u8>) -> usize {
        let mut n = self.process_id.serialize_to(buffer);
        n += self.process_name.serialize_to(buffer);
        n += self.application_id.serialize_to(buffer);
        n += self.application_name.serialize_to(buffer);
        n += self.host_name.serialize_to(buffer);
        n += self.ip_address.serialize_to(buffer);
        n += self.env_vars.serialize_to(buffer);
        n
    }
    fn deserialize_from(buffer: &[u8]) -> Result<Self, CoalyException> {
        let process_id = u32::deserialize_from(buffer)?;
        let buf = &buffer[process_id.serialized_size()..];
        let process_name = String::deserialize_from(buf)?;
        let buf = &buf[process_name.serialized_size()..];
        let application_id = u32::deserialize_from(buf)?;
        let buf = &buf[application_id.serialized_size()..];
        let application_name = String::deserialize_from(buf)?;
        let buf = &buf[application_name.serialized_size()..];
        let host_name = String::deserialize_from(buf)?;
        let buf = &buf[host_name.serialized_size()..];
        let ip_address = String::deserialize_from(buf)?;
        let buf = &buf[ip_address.serialized_size()..];
        let env_vars = BTreeMap::<String, String>::deserialize_from(buf)?;
        Ok(OriginatorInfo { process_id, process_name, application_id, application_name,
                            host_name, ip_address, env_vars } )
    }
}

#[cfg(all(test, net))]
mod tests {
    use super::OriginatorInfo;
    use crate::record::tests::check_serialization;

    #[test]
    fn test_serialize_orig_info() {
        let mut buffer = Vec::<u8>::with_capacity(256);
        // default app ID and name
        let oinfo_def_app = OriginatorInfo::new(1234, "testapp", "clienthost", "1.2.3.4");
        check_serialization::<OriginatorInfo>(&oinfo_def_app, 72, &mut buffer);
        // default app ID, custom app name
        let mut oinfo_def_app_id = OriginatorInfo::new(1234, "testapp", "clienthost", "1.2.3.4");
        oinfo_def_app_id.set_application_name("superapp");
        check_serialization::<OriginatorInfo>(&oinfo_def_app_id, 80, &mut buffer);
        // custom app ID and name
        let mut oinfo_cust_app = OriginatorInfo::new(1234, "testapp", "clienthost", "1.2.3.4");
        oinfo_cust_app.set_application_id(9876);
        oinfo_cust_app.set_application_name("superapp");
        check_serialization::<OriginatorInfo>(&oinfo_cust_app, 80, &mut buffer);
        // with environment variables
        let mut oinfo_with_enva = OriginatorInfo::new(1234, "testapp", "clienthost", "::1");
        oinfo_with_enva.set_application_id(9876);
        oinfo_with_enva.set_application_name("superapp");
        oinfo_with_enva.add_env_var("COALYROOT", "/var/log/superapp");
        oinfo_with_enva.add_env_var("LANG", "en");
        check_serialization::<OriginatorInfo>(&oinfo_with_enva, 138, &mut buffer);
    }
}