//! REPL loop: print the prompt, read a line, tokenize, and dispatch.

use std::io::{self, ErrorKind, Write};

use crate::dispatch::{dispatch, ControlFlow};
use crate::parser::tokenize;
use crate::signals;

pub fn run() -> ! {
    signals::install_sigint_handler();

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
        match read_line_interruptible(&mut line) {
            Ok(0) => {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(b"\n");
                std::process::exit(0);
            }
            Ok(_) => {
                consecutive_read_errors = 0;
                // Ctrl+C may have arrived just as Enter was pressed; drop the line.
                if signals::take_interrupted() {
                    let mut stderr = io::stderr().lock();
                    let _ = stderr.write_all(b"\n");
                    continue;
                }
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {
                // SIGINT during input: cancel the line, fresh prompt, stay alive.
                let _ = signals::take_interrupted();
                consecutive_read_errors = 0;
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(b"\n");
                continue;
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

/// Read one line from stdin. On Unix this uses raw `read(2)` so SIGINT can
/// surface as `ErrorKind::Interrupted` (Rust's `read_line` retries EINTR).
fn read_line_interruptible(buf: &mut String) -> io::Result<usize> {
    #[cfg(unix)]
    {
        unix_read_line(buf)
    }
    #[cfg(not(unix))]
    {
        io::stdin().read_line(buf)
    }
}

#[cfg(unix)]
fn unix_read_line(buf: &mut String) -> io::Result<usize> {
    buf.clear();
    let mut bytes = Vec::new();
    let mut tmp = [0u8; 1];

    unsafe extern "C" {
        fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    }

    loop {
        let n = unsafe { read(0, tmp.as_mut_ptr(), 1) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == ErrorKind::Interrupted {
                // Real Ctrl+C (flag set) vs spurious EINTR.
                if signals::interrupted() {
                    return Err(err);
                }
                continue;
            }
            return Err(err);
        }
        if n == 0 {
            // EOF
            if bytes.is_empty() {
                return Ok(0);
            }
            break;
        }

        bytes.push(tmp[0]);
        if tmp[0] == b'\n' {
            break;
        }
    }

    let n = bytes.len();
    buf.push_str(&String::from_utf8_lossy(&bytes));
    Ok(n)
}
