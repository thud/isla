// MIT License
//
// Copyright (c) 2019 Alasdair Armstrong
//
// Permission is hereby granted, free of charge, to any person
// obtaining a copy of this software and associated documentation
// files (the "Software"), to deal in the Software without
// restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
// BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
// ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::exit;
use toml::Value;

use crate::ast::SharedState;
use crate::zencode;

fn find_tool_path<P>(program: P) -> Result<PathBuf, String>
where
    P: AsRef<Path>,
{
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .filter_map(|dir| {
                let full_path = dir.join(&program);
                if full_path.is_file() {
                    Some(full_path)
                } else {
                    None
                }
            })
            .next()
    }).ok_or(format!("Tool {} not found in $PATH", program.as_ref().display()))
}

fn get_tool_path(config: &Value, tool: &str) -> Result<PathBuf, String> {
    match config.get(tool) {
	Some(Value::String(program)) => find_tool_path(program),
	_ => Err(format!("Configuration option {} must be specified", tool)),
    }
}

/// Get the program counter from the ISA config, and map it to the
/// correct register identifer in the shared state.
fn get_program_counter(config: &Value, shared_state: &SharedState) -> Result<u32, String> {
    match config.get("pc") {
        Some(Value::String(register)) => {
            let register = zencode::encode(&register);
            match shared_state.symtab.get(&register) {
                Some(symbol) => Ok(symbol),
                None => Err(format!("Register {} does not exist in supplied architecture", register)),
            }
        }
        _ => Err(format!("Configuration file must specify the program counter via `pc = \"REGISTER_NAME\"`")),
    }
}
 
#[derive(Debug)]
pub struct ISAConfig {
    pub pc: u32,
    pub assembler: PathBuf,
    pub objdump: PathBuf,
}

pub fn load_config<P>(path: P, shared_state: &SharedState) -> Result<ISAConfig, String>
where
    P: AsRef<Path>,
{
    let mut contents = String::new();
    match File::open(&path) {
        Ok(mut handle) => match handle.read_to_string(&mut contents) {
	    Ok(_) => (),
	    Err(e) =>
		return Err(format!("Unexpected failure while reading config")),
	},
        Err(e) =>
            return Err(format!("Error when loading config '{}': {}", path.as_ref().display(), e)),
    };
    let config = match contents.parse::<Value>() {
        Ok(config) => config,
        Err(e) =>
            return Err(format!("Error when parsing config '{}': {}", path.as_ref().display(), e)),
    };

    Ok(ISAConfig {
	pc: get_program_counter(&config, shared_state)?,
	assembler: get_tool_path(&config, "assembler")?,
	objdump: get_tool_path(&config, "objdump")?,
    })
}
