//! Pipeline execution via `pipe` + `fork` + in-process builtins (SH-026).
//!
//! Each stage runs the shell's own dispatch table after `dup2`-ing pipe ends
//! onto stdin/stdout. No `exec*`, no `std::process::Command`, no external
//! binaries — children `_exit` after the builtin returns.
//!
//! Call only from a single-threaded context (the REPL). Forking while other
//! threads hold Rust stdio locks can deadlock the child.

use std::io::{self, Write};

use crate::color;
use crate::dispatch::{dispatch, ControlFlow};
use crate::error::ShellError;
use crate::tty;

/// Run a multi-stage pipeline. Caller must pass at least two stages.
///
/// On success, returns the last stage's exit status (bash default — no
/// `pipefail`). Children print their own errors; the parent only reports
/// failures from `pipe`/`fork` setup. `exit` inside a pipeline only
/// terminates that forked stage.
pub fn run(stages: &[Vec<String>]) -> Result<i32, ShellError> {
    #[cfg(unix)]
    {
        unix::run(stages)
    }
    #[cfg(not(unix))]
    {
        let _ = stages;
        Err(ShellError::Usage(
            "pipelines are only supported on Unix".to_string(),
        ))
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    unsafe extern "C" {
        fn pipe(pipefd: *mut i32) -> i32;
        fn fork() -> i32;
        fn dup2(oldfd: i32, newfd: i32) -> i32;
        fn close(fd: i32) -> i32;
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        fn _exit(status: i32) -> !;
    }

    pub fn run(stages: &[Vec<String>]) -> Result<i32, ShellError> {
        debug_assert!(stages.len() >= 2);

        // Drop any buffered parent output before cloning the address space.
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();

        let mut pids: Vec<i32> = Vec::with_capacity(stages.len());
        let mut prev_read: Option<i32> = None;

        for (i, argv) in stages.iter().enumerate() {
            let is_last = i + 1 == stages.len();
            let mut fds = [0i32; 2];
            if !is_last {
                let rc = unsafe { pipe(fds.as_mut_ptr()) };
                if rc != 0 {
                    cleanup_partial(&pids, prev_read);
                    return Err(ShellError::from(io::Error::last_os_error()));
                }
            }

            let _ = io::stdout().flush();
            let _ = io::stderr().flush();

            let pid = unsafe { fork() };
            if pid < 0 {
                if !is_last {
                    unsafe {
                        let _ = close(fds[0]);
                        let _ = close(fds[1]);
                    }
                }
                cleanup_partial(&pids, prev_read);
                return Err(ShellError::from(io::Error::last_os_error()));
            }

            if pid == 0 {
                // Child: never return to the parent REPL / test harness.
                child_setup_stdio(prev_read, if is_last { None } else { Some(fds) });
                child_run_and_exit(argv);
            }

            // Parent: close ends we don't need so writers see EOF when done.
            pids.push(pid);
            if let Some(r) = prev_read {
                unsafe {
                    let _ = close(r);
                }
            }
            if !is_last {
                unsafe {
                    let _ = close(fds[1]);
                }
                prev_read = Some(fds[0]);
            } else {
                prev_read = None;
            }
        }

        Ok(wait_for_pipeline(&pids))
    }

    fn child_setup_stdio(prev_read: Option<i32>, new_pipe: Option<[i32; 2]>) {
        unsafe {
            if let Some(r) = prev_read {
                if dup2(r, 0) < 0 {
                    let _ = writeln!(
                        io::stderr(),
                        "0-shell: dup2: {}",
                        io::Error::last_os_error()
                    );
                    _exit(1);
                }
                let _ = close(r);
            }
            if let Some(fds) = new_pipe {
                let _ = close(fds[0]);
                if dup2(fds[1], 1) < 0 {
                    let _ = writeln!(
                        io::stderr(),
                        "0-shell: dup2: {}",
                        io::Error::last_os_error()
                    );
                    _exit(1);
                }
                let _ = close(fds[1]);
            }
        }
    }

    fn child_run_and_exit(argv: &[String]) -> ! {
        let code = if argv.is_empty() {
            0
        } else {
            match dispatch(argv) {
                ControlFlow::Exit(code) => code & 0xff,
                ControlFlow::Continue(Ok(())) => 0,
                ControlFlow::Continue(Err(err)) => {
                    let msg = err.to_string();
                    if !msg.is_empty() {
                        eprintln!("{}", color::paint_error(&msg, tty::stderr_is_tty()));
                    }
                    match err {
                        ShellError::NotFound(_) => 127,
                        _ => 1,
                    }
                }
            }
        };
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
        unsafe { _exit(code) }
    }

    fn wait_for_pipeline(pids: &[i32]) -> i32 {
        let mut last_status = 0i32;
        for (i, &pid) in pids.iter().enumerate() {
            let status = wait_one(pid);
            if i + 1 == pids.len() {
                last_status = status;
            }
        }
        last_status
    }

    fn wait_one(pid: i32) -> i32 {
        let mut status = 0i32;
        loop {
            let rc = unsafe { waitpid(pid, &mut status, 0) };
            if rc < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return 1;
            }
            break;
        }
        decode_wait_status(status)
    }

    fn decode_wait_status(status: i32) -> i32 {
        // Linux wait status layout (same as the W* macros in bits/waitstatus.h).
        if status & 0x7f == 0 {
            (status >> 8) & 0xff
        } else if (((status & 0x7f) + 1) as i8) >> 1 > 0 {
            128 + (status & 0x7f)
        } else {
            1
        }
    }

    fn cleanup_partial(pids: &[i32], prev_read: Option<i32>) {
        if let Some(r) = prev_read {
            unsafe {
                let _ = close(r);
            }
        }
        for &pid in pids {
            let mut status = 0;
            unsafe {
                let _ = waitpid(pid, &mut status, 0);
            }
        }
    }
}
