//! REPL loop: print the prompt, read a line, tokenize, and dispatch.

use std::io::{self, Write};

use crate::dispatch::dispatch;
use crate::parser::tokenize;

pub fn run() -> ! {
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        print!("$ ");
        // stdout is line-buffered; the prompt has no trailing newline, so it
        // won't reach the terminal until we flush explicitly.
        let _ = io::stdout().flush();

        line.clear();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                println!();
                std::process::exit(0);
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!("0-shell: {err}");
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

        if let Err(err) = dispatch(&argv) {
            eprintln!("{err}");
        }
    }
}
