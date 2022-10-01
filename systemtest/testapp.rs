// ---------------------------------------------------------------------------------------------
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
// ---------------------------------------------------------------------------------------------

use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use chamaeleon::{App, TestConfig};

pub mod chamaeleon;

/// Main function to run a system test.
/// Requires command line arguments:
/// - test configuration file name including full path
/// - directory containing Coaly configuration files including full path
/// - footprint root directory for system tests
/// - output root directory for system tests
pub fn main() {
    let mut test_cfg_file_path: Option<PathBuf> = None;
    let mut coaly_cfg_path: Option<PathBuf> = None;
    let mut footprint_root_dir: Option<PathBuf> = None;
    let mut output_root_dir: Option<PathBuf> = None;
    for (i, arg) in std::env::args_os().skip(1).enumerate() {
        let str_arg = Path::new(&arg).to_path_buf();
        match i {
            1 => test_cfg_file_path = Some(str_arg),
            2 => coaly_cfg_path = Some(str_arg),
            3 => footprint_root_dir = Some(str_arg),
            4 => output_root_dir = Some(str_arg),
            _ => if i > 4 { println!("Argument #{} ({}) ignored", i, arg.to_string_lossy()); }
        }
    }
    if test_cfg_file_path.is_none() {
        println!("Missing arguments <test config file>, <Coaly config dir>, <Footprint root dir>, <Output root dir>");
        std::process::exit(1);
    }
    let test_cfg_file_path = test_cfg_file_path.unwrap();
    if ! test_cfg_file_path.is_file() {
        println!("Test config file {} not found", test_cfg_file_path.to_string_lossy());
        std::process::exit(1);
    }
    let test_name = test_cfg_file_path.file_stem().unwrap().to_string_lossy().to_string();
    if coaly_cfg_path.is_none() {
        println!("Missing arguments <Coaly config dir>, <Footprint root dir>, <Output root dir>");
        std::process::exit(1);
    }
    let coaly_cfg_path = coaly_cfg_path.unwrap();
    if ! coaly_cfg_path.is_dir() {
        println!("Coaly config dir {} not found", coaly_cfg_path.to_string_lossy());
        std::process::exit(1);
    }
    if footprint_root_dir.is_none() {
        println!("Missing arguments <Footprint root dir>, <Output root dir>");
        std::process::exit(1);
    }
    let footprint_path = footprint_root_dir.unwrap().join(&test_name);
    if ! footprint_path.exists() {
        println!("Footprint dir {} not found", footprint_path.to_string_lossy());
        std::process::exit(1);
    }
    if output_root_dir.is_none() {
        println!("Missing argument <Output root dir>");
        std::process::exit(1);
    }
    let out_path = output_root_dir.unwrap().join(&test_name);
    if out_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&out_path) {
            println!("Failed to delete output dir for test {}: {}", &test_name, e);
            std::process::exit(1);
        }
    }
    if let Err(e) = std::fs::create_dir_all(&out_path) {
        println!("Failed to create output dir for test {}: {}", &test_name, e);
        std::process::exit(1);
    }
    std::env::set_var("COALY_TEST_NAME", &test_name);
    std::env::set_var("COALY_OUTPUT_PATH", out_path.to_string_lossy().to_string());
    let cfg = TestConfig::from_file(&test_cfg_file_path);
    if let Err(e) = cfg {
        println!("Could not parse test configuration file {}: {}",
                 test_cfg_file_path.to_string_lossy(), e);
        std::process::exit(1);
    }
    let cfg = cfg.unwrap();
    let sort_pattern = cfg.file_sort_pattern();
    let fp_pattern = cfg.footprint_pattern.to_string();
    let app = App::new(&test_name, cfg);
    if let Err(e) = app.run(&coaly_cfg_path) {
        println!("{}", e);
        std::process::exit(1);
    }
    let expected_output = read_footprints(&footprint_path, &sort_pattern);
    let actual_output = read_outputs(&out_path, &fp_pattern, &sort_pattern);
    let no_of_expected_outputs = expected_output.len();
    let no_of_actual_outputs = actual_output.len();
    if no_of_expected_outputs != no_of_actual_outputs {
        println!("Number of expected output files {} does not match actual ({})",
                 no_of_expected_outputs, no_of_actual_outputs);
        std::process::exit(1);
    }
    for (i, actual) in actual_output.iter().enumerate() {
        let expected = expected_output.iter().nth(i).unwrap();
        if actual.1.len() != expected.1.len() {
            println!("Number of lines ({}) in output file {} does not match expectation ({})",
                     actual.1.len(), actual.0, expected.1.len());
            std::process::exit(1);
        }
        for (line_nr, actual_line) in actual.1.iter().enumerate() {
            let expected_line = expected.1.get(line_nr).unwrap();
            if actual_line != expected_line {
                println!("Output file {}, line {} ({}) does not match expectation ({})",
                         &actual.0, line_nr+1, actual_line, expected_line);
                std::process::exit(1);
            }
        }
    }
    println!("System test {} succeeded", &test_name);
}

fn read_footprints(dir: &PathBuf, sort_pattern: &str) -> BTreeMap<String, Vec<String>> {
    let mut footprints = BTreeMap::<String, Vec<String>>::new();
    let sort_pattern = Regex::new(sort_pattern).unwrap();
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let sort_key = sort_pattern.captures(&file_name).unwrap().get(1).unwrap().as_str();
        let sort_key = format!("[{}]-{}", sort_key, &file_name);
        let file_contents = std::fs::read_to_string(&entry.path()).unwrap();
        let file_lines = file_contents.split("\n").map(|x| x.to_string()).collect();
        footprints.insert(sort_key, file_lines);
    }
    footprints
}

fn read_outputs(dir: &PathBuf, pattern: &str, sort_pattern: &str) -> BTreeMap<String, Vec<String>> {
    let mut outputs = BTreeMap::<String, Vec<String>>::new();
    let sort_pattern = Regex::new(sort_pattern).unwrap();
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let sort_key = sort_pattern.captures(&file_name).unwrap().get(1).unwrap().as_str();
        let sort_key = format!("[{}]-{}", sort_key, &file_name);
        let file_contents = std::fs::read_to_string(&entry.path()).unwrap();
        let mut filtered_lines = Vec::<String>::new();
        let filter_pattern = Regex::new(pattern).unwrap();
        for line in file_contents.split("\n") {
            if line.is_empty() { continue; }
            if filter_pattern.is_match(line) {
                let mut filtered_line = String::with_capacity(line.len());
                for cap in filter_pattern.captures_iter(line) {
                    for subcap in cap.iter().skip(1) {
                        if subcap.is_none() { continue; }
                        filtered_line.push_str(subcap.unwrap().as_str());
                    }
                }
                filtered_lines.push(filtered_line);
            } else {
                filtered_lines.push(line.to_string());
            }
        }
        outputs.insert(sort_key, filtered_lines);
    }
    outputs
}