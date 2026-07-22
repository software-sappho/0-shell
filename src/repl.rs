//! REPL loop: print the prompt, read a line, tokenize, and dispatch.

use std::env;
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::color;
use crate::dispatch::{dispatch, ControlFlow};
use crate::history::{self, History};
use crate::parser::{self, tokenize_with_status};
use crate::signals;
use crate::tty;

#[cfg(unix)]
use crate::complete::{self, Apply};

pub fn run() -> ! {
    signals::install_sigint_handler();

    let mut hist = History::new();
    let hist_path = history::default_history_path();
    if let Some(ref path) = hist_path {
        let _ = hist.load_from_path(path);
    }

    let mut line = String::new();
    let mut consecutive_read_errors = 0u32;
    let mut last_status = 0i32;

    loop {
        // The prompt is cosmetic terminal output, not command output, so it
        // goes to stderr — that keeps stdout clean for piping and for the
        // SH-016 audit diffs against real bash.
        let prompt = format_prompt();
        {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(prompt.as_bytes());
            let _ = stderr.flush();
        }

        line.clear();
        match read_line_with_history(&mut line, &hist, &prompt) {
            Ok(0) => {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(b"\n");
                std::process::exit(0);
            }
            Ok(_) => {
                consecutive_read_errors = 0;
                if signals::take_interrupted() {
                    let mut stderr = io::stderr().lock();
                    let _ = stderr.write_all(b"\n");
                    continue;
                }
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {
                let _ = signals::take_interrupted();
                consecutive_read_errors = 0;
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(b"\n");
                continue;
            }
            Err(err) => {
                consecutive_read_errors += 1;
                print_err(format!("0-shell: {err}"));
                if consecutive_read_errors >= 3 {
                    print_err("0-shell: too many consecutive read errors, exiting");
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

        hist.push(trimmed.to_string());
        if let Some(ref path) = hist_path {
            let _ = hist.append_to_path(path, trimmed);
        }

        if let Some(code) = run_line(trimmed, &mut last_status) {
            std::process::exit(code);
        }
    }
}

/// Run one input line, possibly containing `;`-chained commands.
///
/// Each segment is tokenized and dispatched in order. A failing command does
/// not abort the rest. Returns `Some(code)` when `exit` requests termination.
fn run_line(line: &str, last_status: &mut i32) -> Option<i32> {
    for segment in parser::split_commands(line) {
        if segment.is_empty() {
            continue;
        }

        let argv = match tokenize_with_status(segment, *last_status) {
            Ok(argv) => argv,
            Err(err) => {
                print_err(err.to_string());
                *last_status = 1;
                continue;
            }
        };

        if argv.is_empty() {
            continue;
        }

        match dispatch(&argv) {
            ControlFlow::Exit(code) => return Some(code),
            ControlFlow::Continue(Ok(())) => {
                *last_status = 0;
            }
            ControlFlow::Continue(Err(err)) => {
                print_err(err.to_string());
                *last_status = match &err {
                    crate::error::ShellError::NotFound(_) => 127,
                    _ => 1,
                };
            }
        }
    }
    None
}

/// Errors go to stderr; red when stderr is a TTY.
fn print_err(message: impl AsRef<str>) {
    eprintln!(
        "{}",
        color::paint_error(message.as_ref(), tty::stderr_is_tty())
    );
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

fn read_line_with_history(buf: &mut String, hist: &History, prompt: &str) -> io::Result<usize> {
    #[cfg(unix)]
    {
        if tty::stdin_is_tty() {
            return read_line_raw(buf, hist, prompt);
        }
        return unix_read_line_cooked(buf);
    }
    #[cfg(not(unix))]
    {
        let _ = (hist, prompt);
        io::stdin().read_line(buf)
    }
}

#[cfg(unix)]
fn unix_read_line_cooked(buf: &mut String) -> io::Result<usize> {
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
                if signals::interrupted() {
                    return Err(err);
                }
                continue;
            }
            return Err(err);
        }
        if n == 0 {
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

/// Interactive editor: printable chars, backspace, Tab completion, ↑/↓ history,
/// Enter, Ctrl+D.
#[cfg(unix)]
fn read_line_raw(buf: &mut String, hist: &History, prompt: &str) -> io::Result<usize> {
    let _raw = match tty::RawMode::enter() {
        Ok(guard) => guard,
        Err(_) => return unix_read_line_cooked(buf),
    };

    buf.clear();
    let mut draft = String::new();
    // None = editing draft; Some(i) = showing hist[i]
    let mut cursor: Option<usize> = None;

    unsafe extern "C" {
        fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    }

    let mut byte = [0u8; 1];
    loop {
        let n = unsafe { read(0, byte.as_mut_ptr(), 1) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == ErrorKind::Interrupted {
                if signals::interrupted() {
                    return Err(err);
                }
                continue;
            }
            return Err(err);
        }
        if n == 0 {
            // Ctrl+D / EOF
            if draft.is_empty() && cursor.is_none() {
                return Ok(0);
            }
            // Ignore EOF mid-line (bash keeps the line).
            continue;
        }

        match byte[0] {
            b'\n' | b'\r' => {
                let final_line = match cursor {
                    Some(i) => hist.get(i).unwrap_or("").to_string(),
                    None => draft.clone(),
                };
                redraw_line(prompt, &final_line)?;
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_all(b"\n");
                let _ = stderr.flush();
                *buf = final_line;
                buf.push('\n');
                return Ok(buf.len());
            }
            b'\t' => {
                ensure_draft(&mut draft, &mut cursor, hist);
                match complete::apply(&draft) {
                    Apply::None => {
                        let mut stderr = io::stderr().lock();
                        let _ = stderr.write_all(b"\x07");
                        let _ = stderr.flush();
                    }
                    Apply::Replace(new_line) => {
                        draft = new_line;
                        redraw_line(prompt, &draft)?;
                    }
                    Apply::List { line, matches } => {
                        draft = line;
                        show_completions(prompt, &draft, &matches)?;
                    }
                }
            }
            0x7f | 0x08 => {
                // Backspace
                ensure_draft(&mut draft, &mut cursor, hist);
                let _ = draft.pop();
                redraw_line(prompt, &draft)?;
            }
            0x03 => {
                // Ctrl+C in raw mode (if ISIG is off); treat like SIGINT.
                return Err(io::Error::from(ErrorKind::Interrupted));
            }
            0x04 => {
                // Ctrl+D
                if draft.is_empty() && cursor.is_none() {
                    return Ok(0);
                }
            }
            0x1b => {
                // Escape sequence — read CSI if present.
                if !read_exact_byte(&mut byte)? {
                    continue;
                }
                if byte[0] != b'[' {
                    continue;
                }
                if !read_exact_byte(&mut byte)? {
                    continue;
                }
                match byte[0] {
                    b'A' => {
                        // Up — older
                        if hist.is_empty() {
                            continue;
                        }
                        let next = match cursor {
                            None => hist.len() - 1,
                            Some(0) => 0,
                            Some(i) => i - 1,
                        };
                        cursor = Some(next);
                        let shown = hist.get(next).unwrap_or("").to_string();
                        redraw_line(prompt, &shown)?;
                    }
                    b'B' => {
                        // Down — newer / back to draft
                        match cursor {
                            None => {}
                            Some(i) if i + 1 >= hist.len() => {
                                cursor = None;
                                redraw_line(prompt, &draft)?;
                            }
                            Some(i) => {
                                cursor = Some(i + 1);
                                let shown = hist.get(i + 1).unwrap_or("").to_string();
                                redraw_line(prompt, &shown)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            c if c >= 0x20 && c != 0x7f => {
                ensure_draft(&mut draft, &mut cursor, hist);
                draft.push(c as char);
                redraw_line(prompt, &draft)?;
            }
            _ => {}
        }
    }
}

#[cfg(unix)]
fn show_completions(prompt: &str, draft: &str, matches: &[String]) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(b"\n")?;
    for (i, m) in matches.iter().enumerate() {
        if i > 0 {
            stderr.write_all(b"  ")?;
        }
        // Drop the synthetic trailing space used for "word finished".
        let display = m.trim_end_matches(' ');
        stderr.write_all(display.as_bytes())?;
    }
    stderr.write_all(b"\n")?;
    stderr.write_all(prompt.as_bytes())?;
    stderr.write_all(draft.as_bytes())?;
    stderr.write_all(b"\x1b[K")?;
    stderr.flush()?;
    Ok(())
}

#[cfg(unix)]
fn ensure_draft(draft: &mut String, cursor: &mut Option<usize>, hist: &History) {
    if let Some(i) = cursor.take() {
        *draft = hist.get(i).unwrap_or("").to_string();
    }
}

#[cfg(unix)]
fn read_exact_byte(byte: &mut [u8; 1]) -> io::Result<bool> {
    unsafe extern "C" {
        fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    }
    loop {
        let n = unsafe { read(0, byte.as_mut_ptr(), 1) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == ErrorKind::Interrupted {
                if signals::interrupted() {
                    return Err(err);
                }
                continue;
            }
            return Err(err);
        }
        return Ok(n == 1);
    }
}

#[cfg(unix)]
fn redraw_line(prompt: &str, content: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    // CR to column 0, reprint prompt+content, clear to end of line.
    stderr.write_all(b"\r")?;
    stderr.write_all(prompt.as_bytes())?;
    stderr.write_all(content.as_bytes())?;
    stderr.write_all(b"\x1b[K")?;
    stderr.flush()?;
    Ok(())
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

    #[test]
    fn chaining_continues_after_a_failed_command() {
        let mut status = 0;
        let missing =
            std::env::temp_dir().join(format!("0-shell-chain-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);
        let path = missing.to_str().expect("utf8");
        let line = format!("cd {path}; echo after");
        assert_eq!(run_line(&line, &mut status), None);
        assert_eq!(status, 0, "echo after should have succeeded");
    }

    #[test]
    fn exit_in_a_chain_stops_later_commands() {
        let mut status = 0;
        assert_eq!(
            run_line("echo before; exit 9; echo after", &mut status),
            Some(9)
        );
    }

    #[test]
    fn failed_command_sets_status_used_by_next_segment() {
        let mut status = 0;
        assert_eq!(run_line("nosuchbuiltin42; echo ok", &mut status), None);
        assert_eq!(status, 0);

        assert_eq!(run_line("nosuchbuiltin42", &mut status), None);
        assert_eq!(status, 127);
        // Next segment would expand $? from that failure.
        assert_eq!(
            tokenize_with_status("echo $?", status).unwrap(),
            vec!["echo", "127"]
        );
    }
}
