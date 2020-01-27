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

use std::collections::HashMap;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use isla_lib::concrete::Sbits;
use isla_lib::executor;
use isla_lib::executor::Frame;
use isla_lib::init::initialize_letbindings;
use isla_lib::memory::Memory;
use isla_lib::ir::*;
use isla_lib::litmus::Litmus;
use isla_lib::smt::Checkpoint;

mod opts;
use opts::CommonOpts;

fn main() {
    let code = isla_main();
    unsafe { isla_lib::smt::finalize_solver() };
    exit(code)
}

fn isla_main() -> i32 {
    let mut opts = opts::common_opts();
    opts.reqopt("l", "litmus", "load a litmus file", "<file>");

    let (matches, arch) = opts::parse(&opts);
    let CommonOpts { num_threads, mut arch, symtab, initial_registers, isa_config } =
        opts::parse_with_arch(&opts, &matches, &arch);

    insert_primops(&mut arch, AssertionMode::Optimistic);

    let register_state = initial_register_state(&arch, initial_registers);
    let letbindings = Mutex::new(HashMap::new());
    let shared_state = Arc::new(SharedState::new(symtab, &arch));

    initialize_letbindings(&arch, &shared_state, &register_state, &letbindings);

    let litmus = match Litmus::from_file(matches.opt_str("litmus").unwrap(), &isa_config) {
        Ok(litmus) => litmus,
        Err(e) => {
            eprintln!("{}", e);
            return 1
        }
    };

    let mut memory = Memory::new();
    memory.add_concrete_region(isa_config.thread_base .. isa_config.thread_top, HashMap::new());

    litmus.log_info(0);
    memory.log_info(0);

    0
}