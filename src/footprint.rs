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

use crossbeam::queue::SegQueue;
use sha2::{Digest, Sha256};
use std::process::exit;
use std::sync::Arc;
use std::time::Instant;

use isla_axiomatic::litmus::assemble_instruction;
use isla_lib::concrete::{bitvector64::B64, BV};
use isla_lib::executor;
use isla_lib::executor::LocalFrame;
use isla_lib::init::{initialize_architecture, Initialized};
use isla_lib::ir::*;
use isla_lib::simplify::write_events;
use isla_lib::smt::Event;

mod opts;
use opts::CommonOpts;

fn main() {
    let code = isla_main();
    unsafe { isla_lib::smt::finalize_solver() };
    exit(code)
}

fn isla_main() -> i32 {
    let mut opts = opts::common_opts();
    opts.reqopt("i", "instruction", "display footprint of instruction", "<instruction>");
    opts.optopt("e", "endianness", "instruction encoding endianness (little default)", "big/little");
    opts.optflag("x", "hex", "parse instruction as hexadecimal opcode, rather than assembly");

    let mut hasher = Sha256::new();
    let (matches, arch) = opts::parse(&mut hasher, &opts);
    let CommonOpts { num_threads, mut arch, symtab, isa_config } =
        opts::parse_with_arch(&mut hasher, &opts, &matches, &arch);

    let Initialized { regs, lets, shared_state } =
        initialize_architecture(&mut arch, symtab, &isa_config, AssertionMode::Optimistic);

    let little_endian = match matches.opt_str("endianness").as_ref().map(String::as_str) {
        Some("little") | None => true,
        Some("big") => false,
        Some(_) => {
            eprintln!("--endianness argument must be one of either `big` or `little`");
            exit(1)
        }
    };

    let instruction = matches.opt_str("instruction").unwrap();

    let opcode = if matches.opt_present("hex") {
        match u32::from_str_radix(&instruction, 16) {
            Ok(opcode) => opcode.to_le_bytes(),
            Err(e) => {
                eprintln!("Could not parse instruction: {}", e);
                exit(1)
            }
        }
    } else {
        match assemble_instruction(&instruction, &isa_config) {
            Ok(bytes) => {
                let mut opcode: [u8; 4] = Default::default();
                opcode.copy_from_slice(&bytes);
                opcode
            }
            Err(msg) => {
                eprintln!("{}", msg);
                return 1;
            }
        }
    };

    let opcode = B64::from_u32(if little_endian { u32::from_le_bytes(opcode) } else { u32::from_be_bytes(opcode) });
    eprintln!("opcode: {:#010x}", opcode.bits);

    let function_id = shared_state.symtab.lookup("zisla_footprint");
    let (args, _, instrs) = shared_state.functions.get(&function_id).unwrap();
    let task = LocalFrame::new(args, Some(&[Val::Bits(opcode)]), instrs).add_lets(&lets).add_regs(&regs).task(0);

    let queue = Arc::new(SegQueue::new());

    let now = Instant::now();
    executor::start_multi(num_threads, None, vec![task], &shared_state, queue.clone(), &executor::trace_collector);
    eprintln!("Execution took: {}ms", now.elapsed().as_millis());

    loop {
        match queue.pop() {
            Ok(Ok((_, mut events))) => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                let events: Vec<Event<B64>> = events.drain(..).rev().collect();
                write_events(&mut handle, &events, &shared_state.symtab);
            }
            // Error during execution
            Ok(Err(msg)) => {
                eprintln!("{}", msg);
                break 1;
            }
            // Empty queue
            Err(_) => break 0,
        }
    }
}
