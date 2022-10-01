// ---------------------------------------------------------------------------------------------
// Coaly - context aware logging and tracing system for Rust.
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

//! Highly configurable application for integration and system test.

use std::path::PathBuf;

pub mod app;
pub use app::App;
pub mod config;
pub use config::TestConfig;

/// Runs a chamaeleon application.
///
/// # Arguments
/// * `test_name` - the name of the test
/// * `test_cfg_file` - the test configuration file
/// * `arx_cfg_dir` - the directory containing Ariadne configuration files
/// * `output_root` - the root directory for the application's output, the actual output directory
///                   is a subdir named like the test's name
pub fn run_app(test_name: &str,
               test_cfg_file: &PathBuf,
               arx_cfg_dir: &PathBuf,
               output_root: &PathBuf) -> Result<(), String> {
    println!("Running testapp {}", test_name);
    let cfg = TestConfig::from_file(&test_cfg_file)?;
    let app = App::new(test_name, cfg);
    let out_path = output_root.join(test_name);
    if out_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&out_path) {
            return Err(format!("Failed to delete output dir for test {}: {}", &test_name, e))
        }
    }
    if let Err(e) = std::fs::create_dir_all(&out_path) {
        return Err(format!("Failed to create output dir for test {}: {}", &test_name, e))
    }
    std::env::set_var("ARX_TEST_NAME", &test_name);
    std::env::set_var("ARX_OUTPUT_PATH", out_path.to_string_lossy().to_string());
    if let Err(e) = app.run(arx_cfg_dir) {
        panic!("Test {} failed: {}", &test_name, e);
    }
    Ok(())
}
