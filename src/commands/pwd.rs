//! `pwd` builtin: print the current working directory.

use std::env;
use std::io::{self, Write};
use std::path::Path;

use crate::error::ShellError;

pub fn run(_args: &[String]) -> Result<(), ShellError> {
    // Bare `pwd` ignores any arguments, same as real pwd.
    let cwd = env::current_dir().map_err(|source| ShellError::Io {
        cmd: "pwd",
        path: String::new(),
        source,
    })?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle
        .write_all(format_line(&cwd).as_bytes())
        .map_err(|source| ShellError::Io {
            cmd: "pwd",
            path: String::new(),
            source,
        })
}

// `Path::display()` replaces invalid UTF-8 with U+FFFD; that lossy fallback
// is the accepted behavior here rather than raw-byte handling of the path.
fn format_line(cwd: &Path) -> String {
    format!("{}\n", cwd.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn appends_trailing_newline() {
        let cwd = PathBuf::from("/tmp/0-shell-test/subdir");
        assert_eq!(format_line(&cwd), "/tmp/0-shell-test/subdir\n");
    }

    #[test]
    fn root_path() {
        assert_eq!(format_line(&PathBuf::from("/")), "/\n");
    }
}
