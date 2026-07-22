//! REPL loop: print the prompt, read a line, tokenize, and dispatch.

use std::env;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

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
            let _ = stderr.write_all(format_prompt().as_bytes());
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

/// `~/projects/0-shell $ ` — `$HOME` collapsed to `~`, refreshed every loop
/// so it tracks `cd`.
fn format_prompt() -> String {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("?"));
    let home = env::var_os("HOME").map(PathBuf::from);
    format!("{} $ ", display_cwd(&cwd, home.as_deref()))
}

fn display_cwd(cwd: &Path, home: Option<&Path>) -> String {
    let cwd_s = cwd.to_string_lossy();
    match home {
        Some(home) if !home.as_os_str().is_empty() => {
            let home_s = home.to_string_lossy();
            if cwd_s.as_ref() == home_s.as_ref() {
                "~".to_string()
            } else if let Some(rest) = cwd_s.strip_prefix(home_s.as_ref()) {
                // Only collapse when HOME is a path prefix followed by a separator
                // (or exact match above). Avoid turning `/home/me2` into `~2`
                // when HOME is `/home/me`.
                if let Some(stripped) = rest.strip_prefix('/') {
                    format!("~/{stripped}")
                } else if cfg!(windows) {
                    if let Some(stripped) = rest.strip_prefix('\\') {
                        format!("~/{stripped}")
                    } else {
                        cwd_s.into_owned()
                    }
                } else {
                    cwd_s.into_owned()
                }
            } else {
                cwd_s.into_owned()
            }
        }
        _ => cwd_s.into_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_alone_becomes_tilde() {
        let home = Path::new("/home/tester");
        assert_eq!(display_cwd(home, Some(home)), "~");
    }

    #[test]
    fn home_subdir_uses_tilde_slash() {
        let home = Path::new("/home/tester");
        let cwd = Path::new("/home/tester/projects/0-shell");
        assert_eq!(display_cwd(cwd, Some(home)), "~/projects/0-shell");
    }

    #[test]
    fn outside_home_is_absolute() {
        let home = Path::new("/home/tester");
        let cwd = Path::new("/tmp");
        assert_eq!(display_cwd(cwd, Some(home)), "/tmp");
    }

    #[test]
    fn similar_prefix_is_not_collapsed() {
        let home = Path::new("/home/me");
        let cwd = Path::new("/home/me2/docs");
        assert_eq!(display_cwd(cwd, Some(home)), "/home/me2/docs");
    }

    #[test]
    fn missing_home_prints_absolute() {
        let cwd = Path::new("/var/log");
        assert_eq!(display_cwd(cwd, None), "/var/log");
    }

    #[test]
    fn prompt_ends_with_dollar_space() {
        let prompt = format!("{} $ ", display_cwd(Path::new("/tmp"), None));
        assert!(prompt.ends_with(" $ "));
        assert_eq!(prompt, "/tmp $ ");
    }
}
