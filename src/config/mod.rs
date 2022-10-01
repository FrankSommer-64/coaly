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

//! Coaly configuration handling.

use regex::Regex;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::fs::create_dir_all;
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;
use std::vec::Vec;
use crate::coalyxw;
use crate::errorhandling::*;
use crate::modechange::*;
use crate::observer::ObserverKind;
use crate::policies::*;
use crate::record::*;
use crate::record::originator::OriginatorInfo;
use crate::variables::*;
use datetimeformat::*;
use output::*;
use resource::{ResourceDesc, ResourceDescList, ResourceKind};
use systemproperties::*;
use crate::config::toml::document::*;
use crate::config::toml::parse_file;

pub(crate) mod datetimeformat;
pub(crate) mod output;
pub(crate) mod resource;
pub(crate) mod systemproperties;
pub(crate) mod toml;

#[cfg(feature="net")]
use crate::net::serverproperties::*;

#[cfg(feature="net")]
use crate::net::is_valid_url;

/// Returns the system's configuration.
/// If a filename is given, the configuration is read from that file, otherwise the defaults
/// are used. This is also the case, if an error during configuration file processing occurs.
/// 
/// # Arguments
/// * `orig_info` - information about application and local host
/// * `config_file_name` - optional the name of the configuration file
/// 
/// # Return values
/// Coaly system configuration
pub(crate) fn configuration(orig_info: &OriginatorInfo,
                            config_file_name: Option<&str>) -> Rc<Configuration> {
    let mut cfg = if config_file_name.is_none() {
                      // no configuration file is specified, use default configuration
                      Configuration::default()
                  } else {
                      // read configuration from file, use default in case of error
                      match Configuration::from_config_file(config_file_name.unwrap()) {
                          Ok(custom_cfg) => custom_cfg,
                          Err(msg) => Configuration::default_because_of_error(msg)
                      }
                  };
    if cfg.resources().needs_output_path() {
        let mut opath = std::env::temp_dir();
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(meta) = cwd.metadata() {
                if ! meta.permissions().readonly() { opath = cwd; }
            }
        }
        match prepare_path(cfg.system_properties().output_path(),
                           &opath.to_string_lossy(),
                           &cfg, orig_info, W_CFG_INV_OUTPUT_PATH) {
            Ok(p) => cfg.system_properties_mut().set_output_path(&p),
            Err(e) => {
                cfg.system_properties_mut().set_output_path(&opath.to_string_lossy().to_string());
                cfg.add_message(e)
            }
        }
    }
    if cfg.resources().may_need_fallback_path() {
        let tmp_dir = std::env::temp_dir();
        let def_fb_path = tmp_dir.to_string_lossy();
        match prepare_path(cfg.system_properties().fallback_path(),
                           &def_fb_path,
                           &cfg, orig_info, W_CFG_INV_FALLBACK_PATH) {
            Ok(p) => cfg.system_properties_mut().set_fallback_path(&p),
            Err(e) => {
                cfg.system_properties_mut().set_fallback_path(&def_fb_path.to_string());
                cfg.add_message(e)
            }
        }
    }
    Rc::new(cfg)
}

/// Holds all configuration definitions, either defaults or as specified in configuration file.
#[cfg(not(feature="net"))]
pub(crate) struct Configuration {
    // basic settings
    system_properties: SystemProperties,
    // date-time format descriptors
    date_time_formats: DateTimeFormatDescMap,
    // output format descriptors
    output_formats: OutputFormatDescMap,
    // settings when resources operate in buffered mode
    buffer_policies: BufferPolicyMap,
    // rollover behaviours for file based resources
    rollover_policies: RolloverPolicyMap,
    // output resource descriptors
    resources: ResourceDescList,
    // output mode change descriptors
    mode_changes: ModeChangeDescList,
    // errors or warnings which occurred during configuration file processing
    messages: Vec::<CoalyException>
}
#[cfg(feature="net")]
pub(crate) struct Configuration {
    // basic settings
    system_properties: SystemProperties,
    // optional server settings
    server_properties: Option<ServerProperties>,
    // date-time format descriptors
    date_time_formats: DateTimeFormatDescMap,
    // output format descriptors
    output_formats: OutputFormatDescMap,
    // settings when resources operate in buffered mode
    buffer_policies: BufferPolicyMap,
    // rollover behaviours for file based resources
    rollover_policies: RolloverPolicyMap,
    // output resource descriptors
    resources: ResourceDescList,
    // output mode change descriptors
    mode_changes: ModeChangeDescList,
    // errors or warnings which occurred during configuration file processing
    messages: Vec::<CoalyException>
}
impl Configuration {
    /// Returns the system properties
    #[inline]
    pub(crate) fn system_properties(&self) -> &SystemProperties { &self.system_properties }

    /// Returns the system properties
    #[inline]
    pub(crate) fn system_properties_mut(&mut self) -> &mut SystemProperties {
        &mut self.system_properties
    }

    /// Returns the server properties
    #[cfg(feature="net")]
    #[inline]
    pub(crate) fn server_properties(&self) -> &Option<ServerProperties> { &self.server_properties }

    /// Returns the date-time formats
    #[inline]
    pub(crate) fn date_time_formats(&self) -> &DateTimeFormatDescMap { &self.date_time_formats }

    /// Returns the output formats
//    #[cfg(test)]
//    pub(crate) fn output_formats(&self) -> &OutputFormatDescMap { &self.output_formats }

    /// Returns the output format descriptor with the given name or default.
    #[inline]
    pub(crate) fn output_format(&self, name: &Option<String>) -> &OutputFormatDesc {
        self.output_formats.find(name)
    }

    /// Returns the buffer policy with the given name or default.
    #[inline]
    pub(crate) fn buffer_policy(&self, name: &Option<String>) -> &BufferPolicy {
        self.buffer_policies.find(name)
    }

    /// Returns the rollover policy with the given name or default.
    #[inline]
    pub(crate) fn rollover_policy(&self, name: &Option<String>) -> &RolloverPolicy {
        self.rollover_policies.find(name)
    }

    /// Returns a reference to the output resource descriptors
    #[inline]
    pub(crate) fn resources(&self) -> &ResourceDescList { &self.resources }

    /// Returns a reference to the mode change descriptors
    #[inline]
    pub(crate) fn mode_changes(&self) -> &ModeChangeDescList { &self.mode_changes }

    /// Returns a reference to the list of warnings.
    #[inline]
    pub(crate) fn messages(&self) -> &Vec<CoalyException> { &self.messages }

    /// Returns the names of all environment variables referenced in output formats or file names
    pub(crate) fn referenced_env_vars(&self) -> HashSet<String> {
        let mut var_names = HashSet::<String>::new();
        for outp_fmt in self.output_formats.custom_values() {
            for rec_fmt in outp_fmt.specific_formats() {
                merge_env_vars(rec_fmt.items(), &mut var_names);
            }
        }
        for res in self.resources.custom_elements() {
            if let Some(file_data) = res.file_data() {
                merge_env_vars(file_data.file_name_spec(), &mut var_names);
            }
        }
        var_names
    }

    /// Returns a custom configuration from the file with the specified name.
    /// 
    /// # Arguments
    /// * `file_name` - the name of TOML formatted configuration file
    /// 
    /// # Return values
    /// The custom configuration
    /// 
    /// # Errors
    /// A structure containing error information, if the configuration file can't be read or
    /// contains errors
    #[cfg(not(feature="net"))]
    fn from_config_file(file_name: &str) -> Result<Configuration, CoalyException> {
        let mut sys_props: Option<SystemProperties> = None;
        let mut dt_fmts: Option<DateTimeFormatDescMap> = None;
        let mut outp_fmts: Option<OutputFormatDescMap> = None;
        let mut buf_pols: Option<BufferPolicyMap> = None;
        let mut rovr_pols: Option<RolloverPolicyMap> = None;
        let mut res: Option<ResourceDescList> = None;
        let mut mod_chgs: Option<ModeChangeDescList> = None;
        let mut msgs: Vec<CoalyException> = Vec::new();
        let cust_toml = parse_file(file_name)?;
        for (key, val) in cust_toml.root_items() {
            match key.as_str() {
                TOML_GRP_SYSTEM => sys_props = read_system_properties(val, &mut msgs),
                TOML_GRP_POLICIES => read_policies(val, &mut buf_pols, &mut rovr_pols, &mut msgs),
                TOML_GRP_FORMATS => read_formats(val, &mut dt_fmts, &mut outp_fmts, &mut msgs),
                TOML_GRP_RESOURCES => res = read_resources(val, &mut msgs),
                TOML_GRP_MODES => mod_chgs = read_modes(val, &mut msgs),
                _ => msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, val.line_nr(), key.clone()))
            }
        }
        let custom_cfg = Configuration {
            system_properties: sys_props.unwrap_or_default(),
            date_time_formats: dt_fmts.unwrap_or_default(),
            output_formats: outp_fmts.unwrap_or_default(),
            buffer_policies: buf_pols.unwrap_or_default(),
            rollover_policies: rovr_pols.unwrap_or_default(),
            resources: res.unwrap_or_default(),
            mode_changes:mod_chgs.unwrap_or_default(),
            messages: msgs
        };
        Ok(custom_cfg)
    }

    /// Returns a custom configuration from the file with the specified name.
    /// 
    /// # Arguments
    /// * `file_name` - the name of TOML formatted configuration file
    /// 
    /// # Return values
    /// The custom configuration
    /// 
    /// # Errors
    /// A structure containing error information, if the configuration file can't be read or
    /// contains errors
    #[cfg(feature="net")]
    fn from_config_file(file_name: &str) -> Result<Configuration, CoalyException> {
        let mut sys_props: Option<SystemProperties> = None;
        let mut srv_props: Option<ServerProperties> = None;
        let mut dt_fmts: Option<DateTimeFormatDescMap> = None;
        let mut outp_fmts: Option<OutputFormatDescMap> = None;
        let mut buf_pols: Option<BufferPolicyMap> = None;
        let mut rovr_pols: Option<RolloverPolicyMap> = None;
        let mut res: Option<ResourceDescList> = None;
        let mut mod_chgs: Option<ModeChangeDescList> = None;
        let mut msgs: Vec<CoalyException> = Vec::new();
        let cust_toml = parse_file(file_name)?;
        for (key, val) in cust_toml.root_items() {
            match key.as_str() {
                TOML_GRP_SYSTEM => sys_props = read_system_properties(val, &mut msgs),
                TOML_GRP_SERVER => srv_props = read_server_properties(val, &mut msgs),
                TOML_GRP_POLICIES => read_policies(val, &mut buf_pols, &mut rovr_pols, &mut msgs),
                TOML_GRP_FORMATS => read_formats(val, &mut dt_fmts, &mut outp_fmts, &mut msgs),
                TOML_GRP_RESOURCES => res = read_resources(val, &mut msgs),
                TOML_GRP_MODES => mod_chgs = read_modes(val, &mut msgs),
                _ => msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, val.line_nr(), key.clone()))
            }
        }
        let custom_cfg = Configuration {
            system_properties: sys_props.unwrap_or_default(),
            server_properties: srv_props,
            date_time_formats: dt_fmts.unwrap_or_default(),
            output_formats: outp_fmts.unwrap_or_default(),
            buffer_policies: buf_pols.unwrap_or_default(),
            rollover_policies: rovr_pols.unwrap_or_default(),
            resources: res.unwrap_or_default(),
            mode_changes:mod_chgs.unwrap_or_default(),
            messages: msgs
        };
        Ok(custom_cfg)
    }

    /// Returns default configuration with given error message.
    /// Used when a custom configuration could not be parsed from a file because of the error.
    /// 
    /// # Arguments
    /// * `message` - the error message from the parse operation
    fn default_because_of_error(message: CoalyException) -> Configuration {
        let mut cfg = Configuration::default();
        cfg.add_message(message);
        cfg
    }

    /// Returns a reference to the list of warnings.
    #[inline]
    fn add_message(&mut self, msg: CoalyException) { self.messages.push(msg) }
}
#[cfg(not(feature="net"))]
impl Default for Configuration {
    fn default() -> Self {
        Self {
            system_properties: SystemProperties::default(),
            date_time_formats: DateTimeFormatDescMap::default(),
            output_formats: OutputFormatDescMap::default(),
            buffer_policies: BufferPolicyMap::default(),
            rollover_policies: RolloverPolicyMap::default(),
            resources: ResourceDescList::default(),
            mode_changes: ModeChangeDescList::new(),
            messages: Vec::<CoalyException>::new()
        }
    }
}
#[cfg(feature="net")]
impl Default for Configuration {
    fn default() -> Self {
        Self {
            system_properties: SystemProperties::default(),
            server_properties: None,
            date_time_formats: DateTimeFormatDescMap::default(),
            output_formats: OutputFormatDescMap::default(),
            buffer_policies: BufferPolicyMap::default(),
            rollover_policies: RolloverPolicyMap::default(),
            resources: ResourceDescList::default(),
            mode_changes: ModeChangeDescList::new(),
            messages: Vec::<CoalyException>::new()
        }
    }
}
#[cfg(not(feature="net"))]
impl Debug for Configuration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SYSP:{:?}/SRVP:-/DATF:{:?}/OUTF:{:?}/BUFP:{:?}/ROVP:{:?}/RES:{:?}/MODS:{:?}",
                   self.system_properties, self.date_time_formats,
                   self.output_formats, self.buffer_policies, self.rollover_policies,
                   self.resources, self.mode_changes
              )
    }
}
#[cfg(feature="net")]
impl Debug for Configuration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.server_properties.is_none() {
            write!(f, "SYSP:{:?}/SRVP:-/DATF:{:?}/OUTF:{:?}/BUFP:{:?}/ROVP:{:?}/RES:{:?}/MODS:{:?}",
                   self.system_properties, self.date_time_formats,
                   self.output_formats, self.buffer_policies, self.rollover_policies,
                   self.resources, self.mode_changes
                  )
        } else {
            write!(f, "SYSP:{:?}/SRVP:{:?}/DATF:{:?}/OUTF:{:?}/BUFP:{:?}/ROVP:{:?}/RES:{:?}/MODS:{:?}",
                   self.system_properties, self.server_properties.as_ref().unwrap(),
                   self.date_time_formats, self.output_formats, self.buffer_policies,
                   self.rollover_policies, self.resources, self.mode_changes
                  )
        }
    }
}

/// Reads system properties specifications from the custom configuration file.
/// 
/// # Arguments
/// * `system_item` - the value item for the system settings in the custom TOML document
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// the custom system properties read, **None** if no valid property has been found
fn read_system_properties(system_item: &TomlValueItem,
                          msgs: &mut Vec<CoalyException>) -> Option<SystemProperties> {
    if not_table_item(system_item, TOML_GRP_SYSTEM, None, msgs) { return None }
    let mut sp = SystemProperties::default();
    for (sys_key, sys_val) in system_item.child_items().unwrap() {
        match sys_key.as_str() {
            TOML_PAR_VERSION => (
                // reserved for future use
            ),
            TOML_PAR_APP_ID => {
                if int_par(sys_val, sys_key, TOML_GRP_SYSTEM, 0,
                           usize::MAX, 0, msgs) {
                    sp.set_application_id(sys_val.value().as_integer().unwrap() as u32);
                }
            },
            TOML_PAR_APP_NAME => {
                if str_par(sys_val, sys_key, TOML_GRP_SYSTEM, msgs) {
                    sp.set_application_name(&sys_val.value().as_str().unwrap());
                }
            },
            TOML_PAR_CHG_STACK_SIZE => {
                if int_par(sys_val, sys_key, TOML_GRP_SYSTEM, MIN_CHANGE_STACK_SIZE,
                           MAX_CHANGE_STACK_SIZE, DEFAULT_CHANGE_STACK_SIZE, msgs) {
                    sp.set_change_stack_size(sys_val.value().as_integer().unwrap() as usize);
                }
            },
            TOML_PAR_FALLBACK_PATH => {
                if str_par(sys_val, sys_key, TOML_GRP_SYSTEM, msgs) {
                    sp.set_fallback_path(&sys_val.value().as_str().unwrap());
                }
            },
            TOML_PAR_OUTPUT_PATH => {
                if str_par(sys_val, sys_key, TOML_GRP_SYSTEM, msgs) {
                    sp.set_output_path(&sys_val.value().as_str().unwrap());
                }
            },
            TOML_GRP_LEVELS => {
                let cust_lvls = read_levels(sys_val, msgs);
                sp.set_record_levels(cust_lvls);
            },
            TOML_GRP_MODE => {
                let m_grp_key = format!("{}.{}", TOML_GRP_SYSTEM, TOML_GRP_MODE);
                if not_table_item(sys_val, &m_grp_key, None, msgs) { continue }
                for (m_key, m_val) in sys_val.child_items().unwrap() {
                    match m_key.as_str() {
                        TOML_PAR_BUFFERED => {
                            if let Some(l_mask) = read_levels_array(m_val, m_key, &m_grp_key, msgs) {
                                sp.set_initially_buffered_levels(l_mask);
                            }
                        },
                        TOML_PAR_ENABLED => {
                            if let Some(l_mask) = read_levels_array(m_val, m_key, &m_grp_key, msgs) {
                                sp.set_initially_enabled_levels(l_mask);
                            }
                        },
                        _ => {
                            let full_key = format!("{}.{}", m_grp_key, m_key);
                            msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, m_val.line_nr(), full_key));
                        }
                    }
                }
            },
            _ => {
                let full_key = format!("{}.{}", TOML_GRP_SYSTEM, sys_key);
                msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, sys_val.line_nr(), full_key));
            }
        }
    }
    Some(sp)
}

/// Reads rollover and buffer policies from custom configuration.
/// 
/// # Arguments
/// * `policies_item` - the value item for the policies in the custom TOML document
/// * `buffer_policies` - the hash map that shall receive the custom buffer policies
/// * `rollover_policies` - the hash map that shall receive the custom rollover policies
/// * `msgs` - the array, where error messages shall be stored
fn read_policies(policies_item: &TomlValueItem,
                 buffer_policies: &mut Option<BufferPolicyMap>,
                 rollover_policies: &mut Option<RolloverPolicyMap>,
                 msgs: &mut Vec<CoalyException>) {
    if not_table_item(policies_item, TOML_GRP_POLICIES, None, msgs) { return }
    for (key, val_item) in policies_item.child_items().unwrap() {
        match key.as_str() {
            TOML_GRP_BUFFER => *buffer_policies = read_buffer_policies(val_item, msgs),
            TOML_GRP_ROLLOVER => *rollover_policies = read_rollover_policies(val_item, msgs),
            _ => {
                let full_key = format!("{}.{}", TOML_GRP_POLICIES, key);
                msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, val_item.line_nr(), full_key));
            }
        }
    }
}

/// Reads output record and date time formats from custom configuration.
/// 
/// # Arguments
/// * `formats_item` - the value item for the formats in the custom TOML document
/// * `output_formats` - the hash map that shall receive the custom output formats
/// * `datetime_formats` - the hash map that shall receive the custom date time formats
/// * `msgs` - the array, where error messages shall be stored
fn read_formats(formats_item: &TomlValueItem,
                datetime_formats: &mut Option<DateTimeFormatDescMap>,
                output_formats: &mut Option<OutputFormatDescMap>,
                msgs: &mut Vec<CoalyException>) {
    if not_table_item(formats_item, TOML_GRP_FORMATS, None, msgs) { return }
    for (key, val_item) in formats_item.child_items().unwrap() {
        match key.as_str() {
            TOML_GRP_OUTPUT => {
                *output_formats = Some(read_output_formats(val_item, formats_item, msgs))
            },
            TOML_GRP_DATETIME => *datetime_formats = Some(read_datetime_formats(val_item, msgs)),
            _ => msgs.push(coalyxw!(W_CFG_UNKNOWN_KEY, val_item.line_nr(),
                                  format!("{}.{}", TOML_GRP_FORMATS, key)))
        }
    }
}

/// Reads mode changes from custom configuration.
/// 
/// # Arguments
/// * `modes_item` - the value item for the modes in the custom TOML document
/// * `cfg` - the default configuration settings
/// * `msgs` - the array, where error messages shall be stored
fn read_modes(modes_item: &TomlValueItem,
              msgs: &mut Vec<CoalyException>) -> Option<ModeChangeDescList> {
    if ! modes_item.is_array_of_tables() {
        msgs.push(coalyxw!(W_CFG_INV_MODES_HDR, modes_item.line_nr()));
        return None
    }
    let mut m_chgs = ModeChangeDescList::new();
    for mode_spec in modes_item.child_values().unwrap() {
        let mut trg: Option<ObserverKind> = None;
        let mut name: Option<String> = None;
        let mut value: Option<String> = None;
        let mut enabled_levels: u32 = RecordLevelId::no_change_ind();
        let mut buffered_levels: u32 = RecordLevelId::no_change_ind();
        let mut scope: Option<ModeChangeScope> = None;
        for (attr_key, attr_val) in mode_spec.child_items().unwrap() {
            match attr_key.as_str() {
                TOML_PAR_TRIGGER => {
                    if str_par(attr_val, attr_key, TOML_GRP_MODES, msgs) {
                        let obs_kind_name = attr_val.value().as_str().unwrap();
                        if let Ok(trg_id) = ObserverKind::from_str(&obs_kind_name) {
                            trg = Some(trg_id);
                            continue
                        }
                        msgs.push(coalyxw!(W_CFG_INV_MODE_TRIGGER, attr_val.line_nr(),
                                         obs_kind_name.to_string()));
                    }
                },
                TOML_PAR_NAME => {
                    if str_par(attr_val, attr_key, TOML_GRP_MODES, msgs) {
                       name = Some(attr_val.value().as_str().unwrap());
                    }
                },
                TOML_PAR_VALUE => {
                    if str_par(attr_val, attr_key, TOML_GRP_MODES, msgs) {
                       value = Some(attr_val.value().as_str().unwrap());
                    }
                },
                TOML_PAR_ENABLED => {
                    if let Some(l) = read_levels_array(attr_val, attr_key, TOML_GRP_MODES, msgs) {
                        enabled_levels = l;
                    }
                },
                TOML_PAR_BUFFERED => {
                    if let Some(l) = read_levels_array(attr_val, attr_key, TOML_GRP_MODES, msgs) {
                        buffered_levels = l;
                    }
                },
                TOML_PAR_SCOPE => {
                    if str_par(attr_val, attr_key, TOML_GRP_MODES, msgs) {
                        let scope_name = attr_val.value().as_str().unwrap();
                        if let Ok(scope_id) = ModeChangeScope::from_str(&scope_name) {
                            scope = Some(scope_id);
                            continue
                        }
                    }
                    msgs.push(coalyxw!(W_CFG_INV_SCOPE, attr_val.line_nr(), attr_key.to_string()));
                },
                _ => msgs.push(coalyxw!(W_CFG_INV_MODE_ATTR, attr_val.line_nr(), attr_key.to_string()))
            }
        }
        if trg.is_none() ||
            (RecordLevelId::is_no_change_ind(enabled_levels) &&
             RecordLevelId::is_no_change_ind(buffered_levels)) ||
            (name.is_none() && value.is_none()) {
            msgs.push(coalyxw!(W_CFG_INV_MODE_SPEC, modes_item.line_nr()));
            continue
        }
        match trg.unwrap() {
            ObserverKind::Object => {
                let mut name_pattern: Option<Regex> = None;
                let mut value_pattern: Option<Regex> = None;
                if name.is_none() && value.is_none() {
                    msgs.push(coalyxw!(W_CFG_ANONYMOUS_OBSERVER_IGNORED, modes_item.line_nr()));
                    continue;
                }
                if let Some(n) = name {
                    if let Ok(pattern) = Regex::new(&n) {
                        name_pattern = Some(pattern);
                    } else {
                        msgs.push(coalyxw!(W_CFG_INV_OBSERVER_NAME, n, modes_item.line_nr()));
                        continue;
                    }
                }
                if let Some(v) = value {
                    if let Ok(pattern) = Regex::new(&v) {
                        value_pattern = Some(pattern);
                    } else {
                        msgs.push(coalyxw!(W_CFG_INV_OBSERVER_VALUE, v, modes_item.line_nr()));
                        continue;
                    }
                }
                m_chgs.push(ModeChangeDesc::for_object(scope.unwrap_or_default(),
                                                       name_pattern, value_pattern,
                                                       enabled_levels, buffered_levels));
            },
            _ => {
                if value.is_some() {
                    msgs.push(coalyxw!(W_CFG_MODE_VALUE_IGNORED, modes_item.line_nr()));
                }
                if let Some(sc) = scope {
                    if sc == ModeChangeScope::Process {
                        msgs.push(coalyxw!(W_CFG_MODE_SCOPE_IGNORED, modes_item.line_nr()));
                    }
                }
                if let Some(u_name) = name {
                    if let Ok(pattern) = Regex::new(&u_name) {
                        m_chgs.push(ModeChangeDesc::for_unit(trg.unwrap(), Some(pattern),
                                                             enabled_levels, buffered_levels));
                    } else {
                        msgs.push(coalyxw!(W_CFG_INV_OBSERVER_NAME, u_name, modes_item.line_nr()));
                    }
                    continue
                }
                msgs.push(coalyxw!(W_CFG_MISSING_MODE_NAME, modes_item.line_nr()));
            }
        }
    }
    Some(m_chgs)
}

/// Reads mode changes from custom configuration.
/// 
/// # Arguments
/// * `res_item` - the value item for the resources in the custom TOML document
/// * `cfg` - the default configuration settings
/// * `msgs` - the array, where error messages shall be stored
fn read_resources(res_item: &TomlValueItem,
                  msgs: &mut Vec<CoalyException>) -> Option<ResourceDescList> {
    if ! res_item.is_array_of_tables() {
        msgs.push(coalyxw!(W_CFG_INV_RESOURCES_HDR, res_item.line_nr()));
        return None
    }
    let mut res = ResourceDescList::default();
    for res_spec in res_item.child_values().unwrap() {
        let mut kind: Option<ResourceKind> = None;
        let mut scope = vec!(0u32);
        let mut name: Option<String> = None;
        let mut local_url: Option<String> = None;
        let mut remote_url: Option<String> = None;
        let mut levels: Option<u32> = None;
        let mut file_size: Option<usize> = None;
        let mut bufp: Option<String> = None;
        let mut outp_format: Option<String> = None;
        let mut rovrp: Option<String> = None;
        let mut name_lnr: Option<String> = None;
        let mut local_url_lnr: Option<String> = None;
        let mut remote_url_lnr: Option<String> = None;
        let mut file_size_lnr: Option<String> = None;
        let mut bufp_lnr: Option<String> = None;
        let mut rovrp_lnr: Option<String> = None;
        let mut _assigned_levels: u32 = 0;
        #[cfg(feature="net")]
        let mut facility: Option<u32> = None;
        #[cfg(feature="net")]
        let mut outp_fmt_lnr: Option<String> = None;
        for (attr_key, attr_val) in res_spec.child_items().unwrap() {
            match attr_key.as_str() {
                TOML_PAR_KIND => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        let res_kind_name = attr_val.value().as_str().unwrap();
                        if let Ok(kind_id) = ResourceKind::from_str(&res_kind_name) {
                            kind = Some(kind_id);
                            continue
                        }
                        msgs.push(coalyxw!(W_CFG_INV_RES_KIND, attr_val.line_nr(),
                                         res_kind_name.to_string()));
                    }
                },
                TOML_PAR_APP_IDS => {
                    scope = read_app_ids(attr_val, TOML_GRP_RESOURCES, msgs);
                },
                TOML_PAR_NAME => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        name = Some(attr_val.value().as_str().unwrap());
                        name_lnr = Some(attr_val.line_nr());
                    }
                },
                TOML_PAR_SIZE => {
                    if let Some(fsize) = size_par(attr_val, attr_key, TOML_GRP_RESOURCES,
                                                  MIN_FILE_SIZE, MAX_FILE_SIZE,
                                                  DEF_FILE_SIZE, msgs) {
                        file_size = Some(fsize);
                        file_size_lnr = Some(attr_val.line_nr());
                        continue;
                    }
                    file_size = Some(DEF_FILE_SIZE);
                },
                TOML_PAR_LEVELS => {
                    levels = read_levels_array(attr_val, attr_key, TOML_GRP_RESOURCES, msgs);
                },
                TOML_PAR_OUTPUT_FORMAT => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        outp_format = Some(attr_val.value().as_str().unwrap());
                        #[cfg(feature="net")]
                        { outp_fmt_lnr = Some(attr_val.line_nr()); }
                    }
                },
                TOML_PAR_ROLLOVER => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        rovrp = Some(attr_val.value().as_str().unwrap());
                        rovrp_lnr = Some(attr_val.line_nr());
                    }
                },
                TOML_PAR_LOCAL_URL => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        local_url = Some(attr_val.value().as_str().unwrap());
                        local_url_lnr = Some(attr_val.line_nr());
                    }
                },
                TOML_PAR_REMOTE_URL => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        remote_url = Some(attr_val.value().as_str().unwrap());
                        remote_url_lnr = Some(attr_val.line_nr());
                    }
                },
                TOML_PAR_BUFFER => {
                    if str_par(attr_val, attr_key, TOML_GRP_RESOURCES, msgs) {
                        bufp = Some(attr_val.value().as_str().unwrap());
                        bufp_lnr = Some(attr_val.line_nr());
                    }
                },
                #[cfg(feature="net")]
                TOML_PAR_FACILITY => {
                    if int_par(attr_val, attr_key, TOML_GRP_RESOURCES, 0, 23, 1, msgs) {
                        facility = Some(attr_val.value().as_integer().unwrap() as u32);
                    }
                },
                _ => msgs.push(coalyxw!(W_CFG_INV_RES_ATTR,attr_val.line_nr(),attr_key.to_string()))
            }
        }
        if kind.is_none() || levels.is_none() || levels.unwrap() == 0 {
            // kind and at least one record level is mandatory for all resources
            msgs.push(coalyxw!(W_CFG_INV_RES_SPEC, res_item.line_nr()));
            continue
        }
        match kind.unwrap() {
            ResourceKind::PlainFile => {
                if name.is_none() {
                    msgs.push(coalyxw!(W_CFG_RES_FN_MISSING, res_item.line_nr()));
                    continue
                }
                if file_size.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, file_size_lnr.unwrap(),
                                     TOML_PAR_SIZE.to_string(),
                                     kind.unwrap().to_string()));
                }
                if local_url.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, local_url_lnr.unwrap(),
                                     TOML_PAR_LOCAL_URL.to_string(),
                                     kind.unwrap().to_string()));
                }
                if remote_url.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, remote_url_lnr.unwrap(),
                                     TOML_PAR_REMOTE_URL.to_string(),
                                     kind.unwrap().to_string()));
                }
                let r = ResourceDesc::for_plain_file(&scope,
                                                     levels.unwrap(), bufp.as_ref(),
                                                     outp_format.as_ref(), &name.unwrap(),
                                                     rovrp.as_ref());
                res.push(r);
            },
            ResourceKind::MemoryMappedFile => {
                if name.is_none() {
                    msgs.push(coalyxw!(W_CFG_RES_FN_MISSING, res_item.line_nr()));
                    continue
                }
                if file_size.is_none() {
                    msgs.push(coalyxw!(W_CFG_FILE_SIZE_MISSING, res_item.line_nr()));
                    continue
                }
                if bufp.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, bufp_lnr.unwrap(),
                                     TOML_PAR_BUFFER.to_string(),
                                     kind.unwrap().to_string()));
                }
                if local_url.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, local_url_lnr.unwrap(),
                                     TOML_PAR_LOCAL_URL.to_string(),
                                     kind.unwrap().to_string()));
                }
                if remote_url.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, remote_url_lnr.unwrap(),
                                     TOML_PAR_REMOTE_URL.to_string(),
                                     kind.unwrap().to_string()));
                }
                let r = ResourceDesc::for_mem_mapped_file(&scope, levels.unwrap(),
                                                          outp_format.as_ref(),
                                                          &name.unwrap(), file_size.unwrap(),
                                                          rovrp.as_ref());
                res.push(r);
            },
            ResourceKind::StdOut | ResourceKind::StdErr => {
                if name.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, name_lnr.unwrap(),
                                     TOML_PAR_NAME.to_string(),
                                     kind.unwrap().to_string()));
                }
                if file_size.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, file_size_lnr.unwrap(),
                                     TOML_PAR_SIZE.to_string(),
                                     kind.unwrap().to_string()));
                }
                if rovrp.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, rovrp_lnr.unwrap(),
                                     TOML_PAR_ROLLOVER.to_string(),
                                     kind.unwrap().to_string()));
                }
                if local_url.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, local_url_lnr.unwrap(),
                                     TOML_PAR_LOCAL_URL.to_string(),
                                     kind.unwrap().to_string()));
                }
                if remote_url.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, remote_url_lnr.unwrap(),
                                     TOML_PAR_REMOTE_URL.to_string(),
                                     kind.unwrap().to_string()));
                }
                let r = ResourceDesc::for_console(&scope, kind.unwrap(), levels.unwrap(),
                                                  bufp.as_ref(), outp_format.as_ref());
                res.push(r);
            },
            #[cfg(feature="net")]
            ResourceKind::Syslog => {
                if let Some(ref u) = remote_url {
                    if ! is_valid_url(u) {
                        msgs.push(coalyxw!(W_CFG_INV_RES_URL, res_item.line_nr()));
                        remote_url = Some(DEFAULT_SYSLOG_URL.to_string());
                    }
                }
                if let Some(ref u) = local_url {
                    if ! is_valid_url(u) {
                        msgs.push(coalyxw!(W_CFG_INV_RES_URL, res_item.line_nr()));
                        local_url = None;
                    }
                }
                if name.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, name_lnr.unwrap(),
                                     TOML_PAR_NAME.to_string(),
                                     kind.unwrap().to_string()));
                }
                if file_size.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, file_size_lnr.unwrap(),
                                     TOML_PAR_SIZE.to_string(),
                                     kind.unwrap().to_string()));
                }
                if rovrp.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, rovrp_lnr.unwrap(),
                                     TOML_PAR_ROLLOVER.to_string(),
                                     kind.unwrap().to_string()));
                }
                let r = ResourceDesc::for_syslog(&scope, levels.unwrap(), bufp.as_ref(),
                                                 facility.unwrap_or(1),
                                                 &remote_url.unwrap_or(String::from(DEFAULT_SYSLOG_URL)),
                                                 local_url.as_ref());
                res.push(r);
            },
            #[cfg(feature="net")]
            ResourceKind::Network => {
                if remote_url.is_none() || ! is_valid_url(&remote_url.clone().unwrap()) {
                    msgs.push(coalyxw!(W_CFG_INV_RES_URL, res_item.line_nr()));
                    continue
                }
                if let Some(ref u) = local_url {
                    if ! is_valid_url(u) {
                        msgs.push(coalyxw!(W_CFG_INV_RES_URL, res_item.line_nr()));
                        continue
                    }
                }
                if name.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, name_lnr.unwrap(),
                                     TOML_PAR_NAME.to_string(),
                                     kind.unwrap().to_string()));
                }
                if file_size.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, file_size_lnr.unwrap(),
                                     TOML_PAR_SIZE.to_string(),
                                     kind.unwrap().to_string()));
                }
                if outp_format.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, outp_fmt_lnr.unwrap(),
                                     TOML_PAR_OUTPUT_FORMAT.to_string(),
                                     kind.unwrap().to_string()));
                }
                if rovrp.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_RES_PAR, rovrp_lnr.unwrap(),
                                     TOML_PAR_ROLLOVER.to_string(),
                                     kind.unwrap().to_string()));
                }
                let r = ResourceDesc::for_network(&scope, levels.unwrap(), bufp.as_ref(),
                                                  &remote_url.unwrap(), local_url.as_ref());
                res.push(r);
            }
        }
    }
    Some(res)
}

/// Reads record level settings from the custom configuration file.
/// 
/// # Arguments
/// * `lvl_item` - the value item for the record level system settings in the custom TOML document
/// * `msgs` - the array, where error messages shall be stored
fn read_levels(lvl_item: &TomlValueItem, msgs: &mut Vec<CoalyException>) -> RecordLevelMap {
    let parent_grp_key = format!("{}.{}", TOML_GRP_SYSTEM, TOML_GRP_LEVELS);
    if not_table_item(lvl_item, &parent_grp_key, None, msgs) { return RecordLevelMap::default() }
    let mut lvl_map = RecordLevelMap::new();
    for (l_key, l_val) in lvl_item.child_items().unwrap() {
        if let Ok(mut lvl) = RecordLevel::default_for(l_key) {
            let l_grp_key = format!("{}.{}", parent_grp_key, l_key);
            if not_table_item(l_val, &l_grp_key, None, msgs) { continue }
            for (key, val) in l_val.child_items().unwrap() {
                let full_par_key = format!("{}.{}", l_grp_key, key);
                match key.as_str() {
                    TOML_PAR_ID => {
                        if str_par(val, key, &l_grp_key, msgs) {
                            let id_char_str = val.value().as_str().unwrap();
                            if id_char_str.len() != 1 {
                                msgs.push(coalyxw!(W_CFG_INV_LVL_ID_CHAR, val.line_nr(), l_grp_key));
                                return RecordLevelMap::default()
                            }
                            lvl.set_id_char(id_char_str.chars().next().unwrap());
                            continue
                        }
                        msgs.push(coalyxw!(W_CFG_INV_LVL_ID_CHAR, val.line_nr(), l_grp_key));
                        return RecordLevelMap::default()
                    },
                    TOML_PAR_NAME => {
                        if str_par(val, key, &l_grp_key, msgs) {
                            let lvl_name = val.value().as_str().unwrap();
                            if lvl_name.is_empty() {
                                msgs.push(coalyxw!(W_CFG_EMPTY_LVL_NAME, val.line_nr(), l_grp_key));
                                return RecordLevelMap::default()
                            }
                            lvl.set_name(&lvl_name);
                            continue
                        }
                        msgs.push(coalyxw!(W_CFG_INV_LVL_NAME, val.line_nr(), full_par_key));
                        return RecordLevelMap::default()
                    },
                    _ => {
                        msgs.push(coalyxw!(W_CFG_INV_LVL_ATTR, val.line_nr(),
                                         key.to_string(), full_par_key));
                        return RecordLevelMap::default()
                    }
                }
            }
            if ! lvl_map.add(lvl.clone()) {
                msgs.push(coalyxw!(W_CFG_DUP_LVL_VALUE, l_val.line_nr(), lvl.id_char().to_string(),
                                 lvl.name().to_string(), l_grp_key));
                return RecordLevelMap::default()
            }
            continue
        }
        msgs.push(coalyxw!(W_CFG_INV_LVL, l_val.line_nr(), l_key.to_string(), parent_grp_key));
        return RecordLevelMap::default()
    }
    if ! lvl_map.fill_defaults() {
        msgs.push(coalyxw!(W_CFG_DUP_LVL_VALUES, lvl_item.line_nr()));
        return RecordLevelMap::default()
    }
    lvl_map
}

/// Reads custom record formats.
/// Since specific formats depending on record level and/or trigger are allowed, these formats
/// must be specified by TOML arrays of tables.
/// 
/// # Arguments
/// * `parent_item` - the TOML table holding the record format specifications
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// the custom record level specifications
fn read_output_formats(parent_item: &TomlValueItem, formats_item: &TomlValueItem,
                       msgs: &mut Vec<CoalyException>) -> OutputFormatDescMap {
    let mut fmt_map = OutputFormatDescMap::default();
    for (fk, fi) in parent_item.child_items().unwrap() {
        if ! fi.is_array_of_tables() {
            msgs.push(coalyxw!(W_CFG_INV_RECFMT_HDR, fi.line_nr(), fk.to_string()));
            continue
        }
        let gk = format!("{}.{}.{}", TOML_GRP_FORMATS, TOML_GRP_OUTPUT, fk);
        let mut specific_fmts = RecordFormatDescList::new();
        for rfi in fi.child_values().unwrap() {
            let mut lvls: Option<u32> = None;
            let mut trgs: Option<u32> = None;
            let mut dtm_fmt_name: Option<String> = None;
            let mut items: Option<String> = None;
            for (spk, spi) in rfi.child_items().unwrap() {
                match spk.as_str() {
                    TOML_PAR_LEVELS => lvls = read_levels_array(spi, spk, &gk, msgs),
                    TOML_PAR_TRIGGERS => trgs = read_rec_triggers_array(spi, spk, &gk, msgs),
                    TOML_PAR_DATETIME_FORMAT => {
                        if str_par(spi, spk, &gk, msgs) {
                           dtm_fmt_name = Some(spi.value().as_str().unwrap());
                        }
                    },
                    TOML_PAR_ITEMS => {
                        if str_par(spi, spk, &gk, msgs) {
                           items = Some(spi.value().as_str().unwrap());
                        }
                    },
                    _ => ()
                }
            }
            if lvls.is_none() || trgs.is_none() || items.is_none() {
                msgs.push(coalyxw!(W_CFG_INV_RECFMT_SPEC, fi.line_nr(), fk.to_string()));
                continue
            }
            let trgs = trgs.unwrap();
            if trgs == 0 {
                msgs.push(coalyxw!(W_CFG_OUTFMT_TRIGGERS_EMPTY, fi.line_nr(), fk.to_string()));
                continue
            }
            let lvls = lvls.unwrap();
            if lvls == 0 {
                msgs.push(coalyxw!(W_CFG_OUTFMT_LEVELS_EMPTY, fi.line_nr(), fk.to_string()));
                continue
            }
            let rfmt = RecordFormatDesc::new(lvls, trgs, &items.unwrap(), dtm_fmt_name);
            specific_fmts.push(rfmt);
        }
        if ! specific_fmts.is_empty() {
            fmt_map.insert(fk, OutputFormatDesc::new(fk, specific_fmts));
        }
    }
    // check whether all trigger-level combinations are covered by every format
    let mut msg_buf = String::with_capacity(128);
    for desc in fmt_map.custom_values() {
        msg_buf.clear();
        desc.list_uncovered_level_trigger_combinations(&mut msg_buf);
        if ! msg_buf.is_empty() {
            msgs.push(coalyxw!(W_CFG_RECFMT_INCOMPLETE, formats_item.line_nr(),
                             desc.name().to_string(), msg_buf.to_string()));
        }
    }
    fmt_map
}

/// Reads custom date time formats.
/// 
/// # Arguments
/// * `parent_item` - the TOML table holding the date time format specifications
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// the custom record level specifications
fn read_datetime_formats(parent_item: &TomlValueItem,
                         msgs: &mut Vec<CoalyException>) -> DateTimeFormatDescMap {
    let mut res_table = DateTimeFormatDescMap::default();
    let parent_key = format!("{}.{}", TOML_GRP_FORMATS, TOML_GRP_DATETIME);
    if not_table_item(parent_item, &parent_key, None, msgs) { return res_table }
    for (fk, fi) in parent_item.child_items().unwrap() {
        let gk = format!("{}.{}", parent_key, fk);
        let mut tstamp: Option<String> = None;
        let mut time: Option<String> = None;
        let mut date: Option<String> = None;
        for (dk, di) in fi.child_items().unwrap() {
            let full_dk = format!("{}.{}", gk, dk);
            match dk.as_str() {
                TOML_PAR_DATE => {
                    if str_par(di, dk, &gk, msgs) {
                        let fmt_str = di.value().as_str().unwrap();
                        if let Err(evar) = validate_date_format(&fmt_str) {
                            msgs.push(coalyxw!(W_CFG_INV_DTFMT_SPEC, di.line_nr(),
                                             evar, full_dk.to_string()));
                            continue
                        }
                        date = Some(fmt_str);
                    }
                },
                TOML_PAR_TIME => {
                    if str_par(di, dk, &gk, msgs) {
                        let fmt_str = di.value().as_str().unwrap();
                        if let Err(evar) = validate_time_format(&fmt_str) {
                            msgs.push(coalyxw!(W_CFG_INV_DTFMT_SPEC, di.line_nr(),
                                             evar, full_dk.to_string()));
                            continue
                        }
                        time = Some(fmt_str);
                    }
                },
                TOML_PAR_TIMESTAMP => {
                    if str_par(di, dk, &gk, msgs) {
                        let fmt_str = di.value().as_str().unwrap();
                        if let Err(evar) = validate_timestamp_format(&fmt_str) {
                            msgs.push(coalyxw!(W_CFG_INV_DTFMT_SPEC, di.line_nr(),
                                             evar, full_dk.to_string()));
                            continue
                        }
                        tstamp = Some(fmt_str);
                    }
                },
                _ => msgs.push(coalyxw!(W_CFG_INV_DFMT_ATTR, di.line_nr(),
                                      dk.to_string(), fk.to_string()))
            }
        }
        res_table.insert(fk, DateTimeFormatDesc::new(fk, date, time, tstamp));
    }
    res_table
}

/// Reads record levels.
/// 
/// # Arguments
/// * `lvls_item` - the TOML array containing the levels, or a single string item
/// * `key` - key of the array or string item, for error messages only
/// * `parent_key` - the full TOML key of the parent item, for error messages only
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// a bit mask with all record levels or'ed
fn read_levels_array(lvls_item: &TomlValueItem, key: &str, parent_key: &str,
                     msgs: &mut Vec<CoalyException>)  -> Option<u32> {
    match lvls_item.value() {
        TomlValue::String(s) => {
            if let Ok(lvl_id) = RecordLevelId::from_str(s) { return Some(lvl_id as u32) }
            msgs.push(coalyxw!(W_CFG_INV_LVL_REF, lvls_item.line_nr(),
                             s.to_string(), format!("{}.{}", parent_key, key)));
            None
        },
        TomlValue::Array(_) => {
            let mut bit_mask: u32 = 0;
            let mut defined_lvls = HashSet::<RecordLevelId>::new();
            for item in lvls_item.child_values().unwrap() {
                if ! str_par(item, key, parent_key, msgs) { continue }
                let lvl_name = item.value().as_str().unwrap();
                if let Ok(lvl_id) = RecordLevelId::from_str(&lvl_name) {
                    if defined_lvls.contains(&lvl_id) {
                        msgs.push(coalyxw!(W_CFG_DUP_LVL, item.line_nr(),
                                         lvl_name, format!("{}.{}", parent_key, key)));
                        continue
                    }
                    bit_mask |= lvl_id as u32;
                    defined_lvls.insert(lvl_id);
                    continue
                }
                msgs.push(coalyxw!(W_CFG_INV_LVL_REF, item.line_nr(),
                                 lvl_name, format!("{}.{}", parent_key, key)));
            }
            Some(bit_mask)
        },
        _ => {
            let full_name = format!("{}.{}", parent_key, key);
            msgs.push(coalyxw!(W_CFG_KEY_NOT_AN_ARRAY, lvls_item.line_nr(), full_name));
            None
        }
    }
}

/// Reads a TOML array containing record triggers.
/// 
/// # Arguments
/// * `trgs_item` - the TOML array containing the triggers
/// * `key` - the full TOML key of the array item, for error messages only
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// a bit mask with all record levels or'ed
fn read_rec_triggers_array(trgs_item: &TomlValueItem, key: &str, parent_key: &str,
                           msgs: &mut Vec<CoalyException>)  -> Option<u32> {
    match trgs_item.value() {
        TomlValue::String(s) => {
            if let Ok(trg_id) = RecordTrigger::from_str(s) { return Some(trg_id as u32) }
            msgs.push(coalyxw!(W_CFG_INV_TRG, trgs_item.line_nr(),
                             s.to_string(), format!("{}.{}", parent_key, key)));
            None
        },
        TomlValue::Array(_) => {
            let mut bit_mask: u32 = 0;
            for item in trgs_item.child_values().unwrap() {
                if ! str_par(item, key, parent_key, msgs) { continue }
                let trg_name = item.value().as_str().unwrap();
                if let Ok(trg_id) = RecordTrigger::from_str(&trg_name) {
                    let trg_bits = trg_id as u32;
                    if bit_mask & trg_bits != 0 {
                        msgs.push(coalyxw!(W_CFG_DUP_TRG, item.line_nr(),
                                         trg_name, format!("{}.{}", parent_key, key)));
                        continue
                    }
                    bit_mask |= trg_bits;
                    continue
                }
                msgs.push(coalyxw!(W_CFG_INV_TRG, item.line_nr(), trg_name,
                                 format!("{}.{}", parent_key, key)));
            }
            Some(bit_mask)
        },
        _ => {
            let full_name = format!("{}.{}", parent_key, key);
            msgs.push(coalyxw!(W_CFG_KEY_NOT_AN_ARRAY, trgs_item.line_nr(), full_name));
            None
        }
    }
}

/// Reads a TOML array containing buffer flush reasons.
/// 
/// # Arguments
/// * `flush_item` - the TOML array containing the levels
/// * `key` - the full TOML key of the array item, for error messages only
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// a bit mask with all flush reasons or'ed
fn read_flush_array(flush_item: &TomlValueItem, key: &str, parent_key: &str,
                    msgs: &mut Vec<CoalyException>)  -> Option<u32> {
    match flush_item.value() {
        TomlValue::String(s) => {
            if let Ok(cond) = BufferFlushCondition::from_str(s) { return Some(cond as u32) }
            msgs.push(coalyxw!(W_CFG_INV_BUF_FLUSH_CONDITION, flush_item.line_nr(),
                             s.to_string(), format!("{}.{}", parent_key, key)));
            None
        },
        TomlValue::Array(_) => {
            let mut bit_mask: u32 = 0;
            let mut defined_conds = HashSet::<BufferFlushCondition>::new();
            for item in flush_item.child_values().unwrap() {
                if ! str_par(item, key, parent_key, msgs) { continue }
                let cond_name = item.value().as_str().unwrap();
                if let Ok(cond) = BufferFlushCondition::from_str(&cond_name) {
                    if defined_conds.contains(&cond) {
                        msgs.push(coalyxw!(W_CFG_DUP_BUF_FLUSH_CONDITION, item.line_nr(), cond_name,
                                         parent_key.to_string()));
                        continue
                    }
                    bit_mask |= cond as u32;
                    defined_conds.insert(cond);
                    continue
                }
                msgs.push(coalyxw!(W_CFG_INV_BUF_FLUSH_CONDITION, item.line_nr(),
                                 cond_name, parent_key.to_string()));
            }
            Some(bit_mask)
        },
        _ => {
            let full_name = format!("{}.{}", parent_key, key);
            msgs.push(coalyxw!(W_CFG_KEY_NOT_AN_ARRAY, flush_item.line_nr(), full_name));
            None
        }
    }
}

/// Reads rollover and buffer policies from custom configuration.
/// 
/// # Arguments
/// * `policies_item` - the value item for the policies in the custom TOML document
/// * `buffer_policies` - the hash map that shall receive the custom buffer policies
/// * `rollover_policies` - the hash map that shall receive the custom rollover policies
/// * `msgs` - the array, where error messages shall be stored
fn read_buffer_policies(buffers_item: &TomlValueItem,
                        msgs: &mut Vec<CoalyException>) -> Option<BufferPolicyMap> {
    if not_table_item(buffers_item, TOML_GRP_BUFFER, Some(TOML_GRP_POLICIES), msgs) { return None }
    let mut bpols = BufferPolicyMap::default();
    let bpkey = format!("{}.{}", TOML_GRP_POLICIES, TOML_GRP_BUFFER);
    let mut cont_size: Option<usize> = None;
    let mut index_size: Option<usize> = None;
    let mut max_rec_len: Option<usize> = None;
    let mut flush_events: u32 = 0;
    for (key, pol_item) in buffers_item.child_items().unwrap() {
        if not_table_item(pol_item, key, Some(&bpkey), msgs) { continue }
        let polkey = format!("{}.{}", bpkey, key);
        for (attr_key, attr_item) in pol_item.child_items().unwrap() {
            match attr_key.as_str() {
                TOML_PAR_FLUSH => {
                    flush_events = read_flush_array(attr_item, attr_key, &polkey, msgs).unwrap_or(0);
                },
                TOML_PAR_CONTENT_SIZE => {
                    if let Some(cs) = size_par(attr_item, attr_key, &polkey,
                                               MIN_BUFFER_CONT_SIZE, MAX_BUFFER_CONT_SIZE,
                                               DEF_BUFFER_CONT_SIZE, msgs) {
                        cont_size = Some(cs);
                        continue;
                    }
                    cont_size = Some(DEF_BUFFER_CONT_SIZE);
                },
                TOML_PAR_INDEX_SIZE => {
                    if let Some(is) = size_par(attr_item, attr_key, &polkey,
                                               MIN_BUFFER_INDEX_SIZE, MAX_BUFFER_INDEX_SIZE,
                                               DEF_BUFFER_INDEX_SIZE, msgs) {
                        index_size = Some(is);
                    }
                },
                TOML_PAR_MAX_REC_LEN => {
                    if int_par(attr_item, attr_key, &polkey,
                               MIN_MAX_REC_LEN, MAX_MAX_REC_LEN, DEF_MAX_REC_LEN, msgs) {
                        max_rec_len = Some(attr_item.value().as_integer().unwrap() as usize);
                        continue;
                    }
                },
                _ => {
                    msgs.push(coalyxw!(W_CFG_INV_BUFFER_ATTR, attr_item.line_nr(),
                                     attr_key.to_string(), key.to_string()));
                }
            }
        }
        if flush_events == 0 {
            msgs.push(coalyxw!(W_CFG_INV_OR_MISSING_BUF_FLUSH_SPEC,
                             pol_item.line_nr(), key.to_string()));
            continue
        }
        if cont_size.is_none() {
            msgs.push(coalyxw!(W_CFG_MISSING_BUF_CONT_SIZE, pol_item.line_nr(), key.to_string()));
            continue
        }
        if index_size.is_none() {
            // default index size is content size / 32, assuming an average record length of 32
            let ind_sz = cont_size.unwrap() >> 5;
            msgs.push(coalyxw!(W_CFG_MISSING_BUF_INDEX_SIZE, pol_item.line_nr(),
                             key.to_string(), ind_sz.to_string()));
            index_size = Some(ind_sz);
        }
        if let Some(mrl) = max_rec_len {
            if mrl > cont_size.unwrap() {
                let bs = cont_size.unwrap();
                msgs.push(coalyxw!(W_CFG_RECLEN_EXCEEDS_SIZE, pol_item.line_nr(),
                                 key.to_string(), bs.to_string()));
                max_rec_len = Some(bs);
            }
        } else {
            max_rec_len = Some(DEF_MAX_REC_LEN as usize);
        }
        let pol_spec = BufferPolicy::new(key, cont_size.unwrap(), index_size.unwrap(),
                                         flush_events, max_rec_len.unwrap());
        bpols.insert(key, pol_spec);
   }
    Some(bpols)
}

/// Reads rollover policies from custom configuration.
/// 
/// # Arguments
/// * `rollover_item` - the value item for the policies in the custom TOML document
/// * `msgs` - the array, where error messages shall be stored
fn read_rollover_policies(rollover_item: &TomlValueItem,
                          msgs: &mut Vec<CoalyException>) -> Option<RolloverPolicyMap> {
    if not_table_item(rollover_item, TOML_GRP_BUFFER, Some(TOML_GRP_POLICIES), msgs) { return None }
    let mut rpols = RolloverPolicyMap::default();
    let rpkey = format!("{}.{}", TOML_GRP_POLICIES, TOML_GRP_ROLLOVER);
    for (key, pol_item) in rollover_item.child_items().unwrap() {
        if not_table_item(pol_item, key, Some(&rpkey), msgs) { continue }
        let polkey = format!("{}.{}", rpkey, key);
        let mut compr_algo: Option<CompressionAlgorithm> = None;
        let mut keep_count: Option<u32> = None;
        let mut cond: Option<RolloverCondition> = None;
        let mut cond_specified = false;
        for (attr_key, attr_item) in pol_item.child_items().unwrap() {
            match attr_key.as_str() {
                TOML_PAR_COMPRESSION => {
                    let mut ca_str = String::from("");
                    if str_par(attr_item, attr_key, &polkey, msgs) {
                        ca_str = attr_item.value().as_str().unwrap();
                        if let Ok(ca) = CompressionAlgorithm::from_str(&ca_str) {
                            #[cfg(not(feature="compression"))]
                            if ca != CompressionAlgorithm::None {
                                msgs.push(coalyxw!(W_CFG_COMPR_NOT_SUPPORTED, attr_item.line_nr()));
                                continue;
                            }
                            compr_algo = Some(ca);
                            continue
                        }
                    }
                    msgs.push(coalyxw!(W_CFG_INV_COMPR_ALGO, attr_item.line_nr(), ca_str,
                                    format!("{}", CompressionAlgorithm::default())));
                    compr_algo = Some(CompressionAlgorithm::default());
                },
                TOML_PAR_KEEP => {
                    if int_par(attr_item, attr_key, &polkey,
                               MIN_KEEP_COUNT, MAX_KEEP_COUNT, DEFAULT_KEEP_COUNT, msgs) {
                        keep_count = Some(attr_item.value().as_integer().unwrap() as u32);
                        continue;
                    }
                    keep_count = Some(DEFAULT_KEEP_COUNT as u32);
                },
                TOML_PAR_CONDITION => {
                    cond_specified = true;
                    if str_par(attr_item, attr_key, &polkey, msgs) {
                        let trg_str = attr_item.value().as_str().unwrap();
                        match RolloverCondition::from_str(&trg_str) {
                            Ok(trg) => cond = Some(trg),
                            Err(ex) => {
                                msgs.push(coalyxw!(W_CFG_INV_ROLLOVER_COND, attr_item.line_nr(),
                                                 key.to_string(), ex.localized_message()));
                            }
                        }
                        continue
                    }
                },
                _ => {
                    msgs.push(coalyxw!(W_CFG_INV_ROLLOVER_ATTR, attr_item.line_nr(),
                                     attr_key.to_string(), key.to_string()));
                }
            }
        }
        if cond.is_none() {
            // valid condition is mandatory
            if ! cond_specified {
                msgs.push(coalyxw!(W_CFG_MISSING_ROVR_COND, pol_item.line_nr(), key.to_string()));
            }
            continue
        }
        let cond = cond.unwrap();
        match cond {
            RolloverCondition::Never => {
                if compr_algo.is_some() || keep_count.is_some() {
                    msgs.push(coalyxw!(W_CFG_MEANINGLESS_ROVR_ATTR, pol_item.line_nr()));
                }
                compr_algo = Some(CompressionAlgorithm::default());
                keep_count = Some(0);
            },
            _ => {
                if compr_algo.is_none() { compr_algo = Some(CompressionAlgorithm::default()); }
                if keep_count.is_none() {
                    msgs.push(coalyxw!(W_CFG_MISSING_KEEP_COUNT, pol_item.line_nr(), key.to_string(),
                                     DEFAULT_KEEP_COUNT.to_string()));
                    keep_count = Some(DEFAULT_KEEP_COUNT as u32);
                }
            }
        }
        let pol_spec = RolloverPolicy::new(key, cond,
                                           keep_count.unwrap(), compr_algo.unwrap());
        rpols.insert(key, pol_spec);
    }
    Some(rpols)
}

/// Reads all application IDs from a TOML array.
/// 
/// # Arguments
/// * `app_ids_item` - the array item for the application IDs
/// * `res_key` - the full name of the parent TOML item
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// a vector holding the application IDs read
pub(crate) fn read_app_ids(app_ids_item: &TomlValueItem,
                           parent_key: &str,
                           msgs: &mut Vec<CoalyException>) -> Vec<u32> {
    let mut result = Vec::new();
    if let Some(app_ids) = app_ids_item.child_values() {
        for app_id in app_ids {
            if int_par(app_id, TOML_PAR_APP_IDS, parent_key,
                       0, u32::MAX as usize, 0, msgs) {
                result.push(app_id.value().as_integer().unwrap() as u32);
            }
        }
    } else {
        let full_key = format!("{}.{}", parent_key, TOML_PAR_APP_IDS);
        msgs.push(coalyxw!(W_CFG_KEY_NOT_AN_ARRAY, app_ids_item.line_nr(), full_key));
    }
    result
}

/// Checks whether the specified TOML value item holds a string value.
/// Appends an exception to the given exception array, if not.
/// 
/// # Arguments
/// * `item` - the TOML value item
/// * `key` - the pure name of the value item
/// * `parent_key` - the full key of the item's parent
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// **true** if the value item holds a string value; otherwise **false**
pub(crate) fn str_par(item: &TomlValueItem, key: &str,
                      parent_key: &str,
                      msgs: &mut Vec<CoalyException>) -> bool {
    if matches!(item.value(), TomlValue::String(_)) { return true }
    let full_name = format!("{}.{}", parent_key, key);
    msgs.push(coalyxw!(W_CFG_KEY_NOT_A_STRING, item.line_nr(), full_name));
    false
}

/// Checks whether the specified TOML value item holds a number value.
/// Appends an exception to the given exception array, if not.
/// 
/// # Arguments
/// * `item` - the TOML value item
/// * `key` - the pure name of the value item
/// * `parent_key` - the full key of the item's parent
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// **true** if the value item holds a string value; otherwise **false**
pub(crate) fn int_par(item: &TomlValueItem, key: &str, parent_key: &str,
                      min_val: usize, max_val: usize, default_val: usize,
                      msgs: &mut Vec<CoalyException>) -> bool {
    let full_key = format!("{}.{}", parent_key, key);
    if let Some(int_item) = item.value().as_integer() {
        if !(min_val..=max_val).contains(&(int_item as usize)) {
            msgs.push(coalyxw!(W_CFG_NUM_REQUIRED, item.line_nr(), full_key,
                             min_val.to_string(), max_val.to_string(), default_val.to_string()));
            return false
        }
        return true
    }
    msgs.push(coalyxw!(W_CFG_NUM_REQUIRED, item.line_nr(), full_key, min_val.to_string(),
                     max_val.to_string(), default_val.to_string()));
    false
}

/// Checks whether the specified TOML value item holds a size value.
/// Size values are optionally scaled numbers, in integer or string format.
/// Appends an exception to the given exception array, if not.
/// 
/// # Arguments
/// * `item` - the TOML value item
/// * `key` - the pure name of the value item
/// * `parent_key` - the full key of the item's parent
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// The number value, if the value item contains a valid size; otherwise **None**
pub(crate) fn size_par(item: &TomlValueItem, key: &str, parent_key: &str,
                       min_val: usize, max_val: usize, default_val: usize,
                       msgs: &mut Vec<CoalyException>) -> Option<usize> {
    let full_key = format!("{}.{}", parent_key, key);
    if let Some(str_item) = item.value().as_str() {
        let num_pat = Regex::new("^[0-9]+[kKmMgG]{0,1}$").unwrap();
        if ! num_pat.is_match(&str_item) {
            msgs.push(coalyxw!(W_CFG_INV_SIZE_SPEC, item.line_nr(), str_item,
                             full_key, default_val.to_string()));
            return None
        }
        if str_item.len() > max_val.to_string().len() {
            msgs.push(coalyxw!(W_CFG_NUM_REQUIRED, item.line_nr(), full_key,
                             min_val.to_string(), max_val.to_string(), default_val.to_string()));
            return None
        }
        let mut num: usize = 0;
        for ch in str_item.chars() {
            match ch {
                '0' ..= '9' => {
                    num *= 10;
                    num += char::to_digit(ch, 10).unwrap() as usize;
                },
                'k' | 'K' => num *= 1024,
                'm' | 'M' => num *= 1024 * 1024,
                'g' | 'G' => num *= 1024 * 1024 * 1024,
                _ => ()
            }
        }
        if !(min_val..=max_val).contains(&num) {
            msgs.push(coalyxw!(W_CFG_NUM_REQUIRED, item.line_nr(), full_key,
                             min_val.to_string(), max_val.to_string(), default_val.to_string()));
            return None
        }
        return Some(num)
    }
    msgs.push(coalyxw!(W_CFG_KEY_NOT_A_STRING, item.line_nr(), full_key));
    None
}

/// Checks whether the specified TOML value item holds a table value.
/// Appends an exception to the given exception array, if not.
/// 
/// # Arguments
/// * `item` - the TOML value item
/// * `key` - the pure name of the value item
/// * `parent_key` - the full key of the item's parent
/// * `msgs` - the array, where error messages shall be stored
/// 
/// # Return values
/// **true** if the value item doesn't hold a table; otherwise **false**
pub(crate) fn not_table_item(item: &TomlValueItem, key: &str, parent_key: Option<&str>,
                             msgs: &mut Vec<CoalyException>) -> bool {
    if matches!(item.value(), TomlValue::Table(_)) { return false }
    let full_name = if parent_key.is_some() {
        format!("{}.{}", parent_key.unwrap(), key) } else { key.to_string() };
    msgs.push(coalyxw!(W_CFG_KEY_NOT_A_TABLE, item.line_nr(), full_name));
    true
}

/// Returns all environment variable names in the given format string.
fn merge_env_vars(fmt_str: &str, result: &mut HashSet<String>) {
    for var_name in Regex::new(ENV_VAR_PATTERN).unwrap().captures_iter(fmt_str) {
        result.insert(var_name.get(1).unwrap().as_str().to_string());
    }
}

/// Replaces all placeholder variables in a path.
fn prepare_path(path_spec: &str,
                default_path: &str,
                cfg: &Configuration,
                orig_info: &OriginatorInfo,
                err_code: &'static str) -> Result<String, CoalyException> {
    // eventually replace placeholder variables in path specification
    let mut path_name = path_spec.to_string();
    let var_app_id = format!("${}", VAR_NAME_APP_ID);
    path_name = path_name.replace(&var_app_id, &cfg.system_properties().application_id_str());
    let var_app_name = format!("${}", VAR_NAME_APP_NAME);
    path_name = path_name.replace(&var_app_name, &cfg.system_properties().application_name());
    let var_proc_id = format!("${}", VAR_NAME_PROCESS_ID);
    path_name = path_name.replace(&var_proc_id, &orig_info.process_id());
    let var_proc_name = format!("${}", VAR_NAME_PROCESS_NAME);
    path_name = path_name.replace(&var_proc_name, orig_info.process_name());
    let var_env = format!("${}[", VAR_NAME_ENV);
    if path_name.contains(&var_env) {
        let env_pat = Regex::new(ENV_VAR_PATTERN).unwrap();
        for enva in env_pat.captures_iter(&path_name.clone()) {
            let enva_name = enva.get(1).unwrap().as_str();
            if let Ok(enva_val) = std::env::var(enva_name) {
                path_name = path_name.replace(enva_name, &enva_val);
            } else {
                return Err(coalyxw!(err_code, path_spec.to_string(), default_path.to_string()))
            }
        }
        path_name = path_name.replace(&var_env, "");
        path_name = path_name.replace("]", "");
    }
    // path must be absolute
    let path = Path::new(&path_name);
    if ! path.is_absolute() {
        return Err(coalyxw!(err_code, path_name, default_path.to_string()))
    }
    // create path, if it doesn't exist
    if ! path.exists() {
        if let Err(_) = create_dir_all(&path) {
            return Err(coalyxw!(err_code, path_name, default_path.to_string()))
        }
    }
    if ! path.is_dir() {
        return Err(coalyxw!(err_code, path_name, default_path.to_string()))
    }
    if let Ok(meta) = path.metadata() {
        if meta.permissions().readonly() {
            return Err(coalyxw!(err_code, path_name, default_path.to_string()))
        }
    } else {
        return Err(coalyxw!(err_code, path_name, default_path.to_string()))
    }
    Ok(path_name)
}

// TOML keys for logical groups in the custom configuration file.
// Logical groups are formed by TOML tables or arrays of tables.
const TOML_GRP_BUFFER: &str = "buffer";
const TOML_GRP_DATETIME: &str = "datetime";
const TOML_GRP_FORMATS: &str = "formats";
const TOML_GRP_LEVELS: &str = "levels";
const TOML_GRP_MODE: &str = "mode";
const TOML_GRP_MODES: &str = "modes";
const TOML_GRP_OUTPUT: &str = "output";
const TOML_GRP_POLICIES: &str = "policies";
const TOML_GRP_RESOURCES: &str = "resources";
const TOML_GRP_ROLLOVER: &str = "rollover";
const TOML_GRP_SYSTEM: &str = "system";
#[cfg(feature="net")]
const TOML_GRP_SERVER: &str = "server";

// TOML keys for single parameters in the custom configuration file.
const TOML_PAR_APP_ID: &str = "app_id";
const TOML_PAR_APP_IDS: &str = "app_ids";
const TOML_PAR_APP_NAME: &str = "app_name";
const TOML_PAR_BUFFER: &str = "buffer";
const TOML_PAR_BUFFERED: &str = "buffered";
const TOML_PAR_CHG_STACK_SIZE: &str = "change_stack_size";
const TOML_PAR_COMPRESSION: &str = "compression";
const TOML_PAR_CONDITION: &str = "condition";
const TOML_PAR_CONTENT_SIZE: &str = "content_size";
const TOML_PAR_DATE: &str = "date";
const TOML_PAR_DATETIME_FORMAT: &str = "datetime_format";
const TOML_PAR_ENABLED: &str = "enabled";
const TOML_PAR_FALLBACK_PATH: &str = "fallback_path";
const TOML_PAR_FLUSH: &str = "flush";
const TOML_PAR_ID: &str = "id";
const TOML_PAR_INDEX_SIZE: &str = "index_size";
const TOML_PAR_ITEMS: &str = "items";
const TOML_PAR_KEEP: &str = "keep";
const TOML_PAR_KIND: &str = "kind";
const TOML_PAR_LEVELS: &str = "levels";
const TOML_PAR_LOCAL_URL: &str = "local_url";
const TOML_PAR_MAX_REC_LEN: &str = "max_record_length";
const TOML_PAR_NAME: &str = "name";
const TOML_PAR_OUTPUT_FORMAT: &str = "output_format";
const TOML_PAR_OUTPUT_PATH: &str = "output_path";
const TOML_PAR_REMOTE_URL: &str = "remote_url";
const TOML_PAR_ROLLOVER: &str = "rollover";
const TOML_PAR_SCOPE: &str = "scope";
const TOML_PAR_SIZE: &str = "size";
const TOML_PAR_TIME: &str = "time";
const TOML_PAR_TIMESTAMP: &str = "timestamp";
const TOML_PAR_TRIGGER: &str = "trigger";
const TOML_PAR_TRIGGERS: &str = "triggers";
const TOML_PAR_VALUE: &str = "value";
const TOML_PAR_VERSION: &str = "version";
#[cfg(feature="net")]
const TOML_PAR_FACILITY: &str = "facility";

const ENV_VAR_PATTERN: &str = r"\$Env\[(.*?)\]";

#[cfg(feature="net")]
const DEFAULT_SYSLOG_URL: &str = "file:/dev/log";

#[cfg(test)]
mod test {
    use crate::errorhandling::COALY_MSG_TABLE;
    use crate::util::originator_info;
    use crate::util::tests::run_unit_tests;
    use std::env;
    use std::fs::read_to_string;
    use super::configuration;

    /// Unit test function for Coaly configuration tests.
    fn run_config_test(success_expected: bool,
                       proj_root_dir: &str,
                       input_fn: &str,
                       ref_fn: &str) -> Option<String> {
        let test_name = &input_fn[input_fn.rfind('/').unwrap()+1 ..];
        let test_name = &test_name[0 .. test_name.find('.').unwrap()];
        #[cfg(not(feature="net"))]
        if test_name.starts_with('n') { return None }
        #[cfg(not(feature="compression"))]
        if test_name.starts_with('x') { return None }
        #[cfg(feature="compression")]
        if test_name.starts_with('c') { return None }
        let block_index = if test_name.starts_with('s') || test_name.starts_with('f') {1} else {2};
        let oinfo = originator_info();
        match read_to_string(ref_fn) {
            Ok(expected_result) => {
                let fallback_path = std::env::var("COALY_FALLBACK_PATH").unwrap();
                let ro_path = format!("{}/readonly", std::env::var("TESTING_ROOT").unwrap());
                let sys_tmp_dir = std::env::temp_dir().to_string_lossy().to_string();
                let expected_result = expected_result.replace("%fallbackpath", &fallback_path);
                let expected_result = expected_result.replace("%readonlypath", &ro_path);
                let expected_result = expected_result.replace("%inputfile", input_fn);
                let expected_result = expected_result.replace("%projroot", proj_root_dir);
                let expected_result = expected_result.replace("%systmp", &sys_tmp_dir);
                let config = configuration(&oinfo, Some(input_fn));
                let mut actual_result = match test_name.chars().nth(block_index).unwrap() {
                    '1' => format!("{:?}", config.system_properties()),
                    '2' => format!("{:?}", config.date_time_formats()),
                    '3' => format!("{:?}", config.output_formats),
                    '4' => format!("{:?}", config.buffer_policies),
                    '5' => format!("{:?}", config.rollover_policies),
                    '6' => format!("{:?}", config.resources()),
                    '7' => format!("{:?}", config.mode_changes()),
                    #[cfg(feature="net")]
                    '8' => if config.server_properties().is_none() { String::from("-") }
                           else { format!("{:?}", config.server_properties().as_ref().unwrap()) },
                    _ => format!("{:?}", config)
                };
                if config.messages().is_empty() {
                    // Config file parsed without errors or warnings
                    if ! success_expected {
                        return Some(String::from("Expected failure, but test succeeded"))
                    }
                    assert_eq!(expected_result, actual_result, "{}", test_name);
                    None
                } else {
                    if success_expected {
                        return Some(String::from("Expected success, but got exceptions"))
                    }
                    for m in config.messages() {
                        let ex_msg = m.evaluate(&COALY_MSG_TABLE);
                        actual_result.push_str(&format!("\n{}", ex_msg));
                    }
                    assert_eq!(expected_result, actual_result, "{}", test_name);
                    None
                }
            },
            Err(_) => Some(format!("Could not read reference file {}", ref_fn))
        }
    }

    #[test]
    fn config_tests() {
        let test_lang = "en";
        let proj_root = env::var("COALY_PROJ_ROOT").unwrap();
        // Success tests
        if let Some(err_msg) = run_unit_tests(&proj_root, "config", true, ".toml", ".txt",
                                              test_lang, run_config_test) {
            panic!("Config success tests failed: {}", &err_msg)
        }
        // Failure tests
        if let Some(err_msg) = run_unit_tests(&proj_root, "config", false, ".toml", ".txt",
                                              test_lang, run_config_test) {
            panic!("Coaly failure tests failed: {}", &err_msg)
        }
    }
}
