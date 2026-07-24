//! Apply `<` / `>` / `>>` by reopening stdin/stdout via `dup2` (SH-027).
//!
//! A [`Guard`] restores the previous fds on drop so the REPL keeps its
//! terminal after a redirected builtin runs in-process.

use std::fs::OpenOptions;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, IntoRawFd, RawFd};

use crate::error::ShellError;
use crate::parser::Redirections;

/// Saved stdin/stdout descriptors; restores them when dropped.
pub struct Guard {
    #[cfg(unix)]
    saved_in: Option<RawFd>,
    #[cfg(unix)]
    saved_out: Option<RawFd>,
}

impl Guard {
    /// Open redirect targets and `dup2` them onto fds 0/1.
    ///
    /// Files for `>` / `>>` are created before the command runs (bash).
    pub fn apply(redirs: &Redirections) -> Result<Self, ShellError> {
        #[cfg(unix)]
        {
            unix::apply(redirs)
        }
        #[cfg(not(unix))]
        {
            let _ = redirs;
            Err(ShellError::Usage(
                "redirection is only supported on Unix".to_string(),
            ))
        }
    }
}

#[cfg(unix)]
impl Drop for Guard {
    fn drop(&mut self) {
        unix::restore(self.saved_in.take(), self.saved_out.take());
    }
}

#[cfg(not(unix))]
impl Drop for Guard {
    fn drop(&mut self) {}
}

/// Apply redirects permanently in the current process (forked pipeline stage).
pub fn apply_in_child(redirs: &Redirections) -> Result<(), ShellError> {
    #[cfg(unix)]
    {
        unix::apply_norestore(redirs)
    }
    #[cfg(not(unix))]
    {
        let _ = redirs;
        Err(ShellError::Usage(
            "redirection is only supported on Unix".to_string(),
        ))
    }
}

#[cfg(unix)]
mod unix {
    use super::*;

    unsafe extern "C" {
        fn dup(oldfd: i32) -> i32;
        fn dup2(oldfd: i32, newfd: i32) -> i32;
        fn close(fd: i32) -> i32;
    }

    pub fn apply(redirs: &Redirections) -> Result<Guard, ShellError> {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();

        let mut saved_in = None;
        let mut saved_out = None;

        if let Some(path) = redirs.stdin.as_deref() {
            let file = OpenOptions::new()
                .read(true)
                .open(path)
                .map_err(|source| redir_open_error(path, source))?;
            let fd = file.as_raw_fd();
            let saved = unsafe { dup(0) };
            if saved < 0 {
                return Err(ShellError::from(io::Error::last_os_error()));
            }
            if unsafe { dup2(fd, 0) } < 0 {
                unsafe {
                    let _ = close(saved);
                }
                return Err(ShellError::from(io::Error::last_os_error()));
            }
            drop(file);
            saved_in = Some(saved);
        }

        if let Some(out) = redirs.stdout.as_ref() {
            let path = out.path();
            let mut opts = OpenOptions::new();
            opts.write(true).create(true);
            if out.append() {
                opts.append(true);
            } else {
                opts.truncate(true);
            }
            let file = opts
                .open(path)
                .map_err(|source| redir_open_error(path, source))?;
            let fd = file.as_raw_fd();
            let saved = unsafe { dup(1) };
            if saved < 0 {
                restore(saved_in.take(), None);
                return Err(ShellError::from(io::Error::last_os_error()));
            }
            if unsafe { dup2(fd, 1) } < 0 {
                unsafe {
                    let _ = close(saved);
                }
                restore(saved_in.take(), None);
                return Err(ShellError::from(io::Error::last_os_error()));
            }
            drop(file);
            saved_out = Some(saved);
        }

        Ok(Guard {
            saved_in,
            saved_out,
        })
    }

    pub fn restore(saved_in: Option<RawFd>, saved_out: Option<RawFd>) {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
        unsafe {
            if let Some(fd) = saved_out {
                let _ = dup2(fd, 1);
                let _ = close(fd);
            }
            if let Some(fd) = saved_in {
                let _ = dup2(fd, 0);
                let _ = close(fd);
            }
        }
    }

    pub fn apply_norestore(redirs: &Redirections) -> Result<(), ShellError> {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();

        if let Some(path) = redirs.stdin.as_deref() {
            let file = OpenOptions::new()
                .read(true)
                .open(path)
                .map_err(|source| redir_open_error(path, source))?;
            let fd = file.into_raw_fd();
            if unsafe { dup2(fd, 0) } < 0 {
                unsafe {
                    let _ = close(fd);
                }
                return Err(ShellError::from(io::Error::last_os_error()));
            }
            unsafe {
                let _ = close(fd);
            }
        }

        if let Some(out) = redirs.stdout.as_ref() {
            let path = out.path();
            let mut opts = OpenOptions::new();
            opts.write(true).create(true);
            if out.append() {
                opts.append(true);
            } else {
                opts.truncate(true);
            }
            let file = opts
                .open(path)
                .map_err(|source| redir_open_error(path, source))?;
            let fd = file.into_raw_fd();
            if unsafe { dup2(fd, 1) } < 0 {
                unsafe {
                    let _ = close(fd);
                }
                return Err(ShellError::from(io::Error::last_os_error()));
            }
            unsafe {
                let _ = close(fd);
            }
        }

        Ok(())
    }

    fn redir_open_error(path: &str, source: io::Error) -> ShellError {
        let exact = match source.kind() {
            io::ErrorKind::NotFound => Some("No such file or directory"),
            io::ErrorKind::PermissionDenied => Some("Permission denied"),
            io::ErrorKind::IsADirectory => Some("Is a directory"),
            _ => None,
        };
        let source = match exact {
            Some(text) => io::Error::new(source.kind(), text),
            None => source,
        };
        ShellError::Io {
            cmd: "0-shell",
            path: path.to_string(),
            source,
        }
    }
}
