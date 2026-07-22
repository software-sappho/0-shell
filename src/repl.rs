//! REPL loop: print the prompt, read a line, tokenize, and dispatch.

use std::io::{self, Write};

use crate::dispatch::{dispatch, ControlFlow};
use crate::parser::tokenize;

pub fn run() -> ! {
    let stdin = io::stdin();
    let mut line = String::new();
    let mut consecutive_read_errors = 0u32;

    loop {
        // The prompt is cosmetic terminal output, not command output, so it
        // goes to stderr — that keeps stdout clean for piping and for the
        // SH-016 audit diffs against real bash.
        {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(b"$ ");
            let _ = stderr.flush();
        }

        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(b"\n");
                std::process::exit(0);
            }
            Ok(_) => {
                consecutive_read_errors = 0;
            }
            Err(err) => {
                consecutive_read_errors += 1;
                eprintln!("0-shell: {err}");
                if consecutive_read_errors >= 3 {
                    eprintln!("0-shell: too many consecutive read errors, exiting");
                    std::process::exit(1);
                }
                continue;
            }
        }

        let trimmed = line.strip_suffix('\n').unwrap_or(&line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);

        if trimmed.trim().is_empty() {
            continue;
        }

        let argv = match tokenize(trimmed) {
            Ok(argv) => argv,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };

        if argv.is_empty() {
            continue;
        }

        match dispatch(&argv) {
            ControlFlow::Exit(code) => std::process::exit(code),
            ControlFlow::Continue(Ok(())) => {}
            ControlFlow::Continue(Err(err)) => eprintln!("{err}"),
        }
    }
}
