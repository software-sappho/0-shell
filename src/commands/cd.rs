//! `cd` builtin: change the current working directory.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let target = resolve_target(args)?;

    env::set_current_dir(&target).map_err(|source| to_shell_error(&target, source))
}

fn resolve_target(args: &[String]) -> Result<PathBuf, ShellError> {
    match args {
        [] => home_dir(),
        [only] => resolve_one(only),
        _ => Err(ShellError::Usage("cd: too many arguments".to_string())),
    }
}

fn resolve_one(arg: &str) -> Result<PathBuf, ShellError> {
    if arg == "~" || arg.starts_with("~/") {
        home_dir().map(|home| expand_tilde(arg, &home))
    } else {
        Ok(PathBuf::from(arg))
    }
}

fn home_dir() -> Result<PathBuf, ShellError> {
    env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| ShellError::Usage("cd: HOME not set".to_string()))
}

// Only expands an exact `~` or a `~/...` prefix, matching the ticket's
// scope; `~user` (another user's home) falls through as a literal path.
fn expand_tilde(arg: &str, home: &Path) -> PathBuf {
    if arg == "~" {
        home.to_path_buf()
    } else if let Some(rest) = arg.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(arg)
    }
}

// The Display text shown to the user comes entirely from `source`'s own
// Display (see ShellError::Io in error.rs), so we rebuild `source` with an
// exact, portable message for the error kinds the audit checks rather than
// trusting the OS's strerror() wording to match bash's coreutils text
// byte-for-byte. Anything else falls through to the OS-provided text.
fn to_shell_error(path: &Path, source: io::Error) -> ShellError {
    let exact_text = match source.kind() {
        io::ErrorKind::NotFound => Some("No such file or directory"),
        io::ErrorKind::PermissionDenied => Some("Permission denied"),
        io::ErrorKind::NotADirectory => Some("Not a directory"),
        _ => None,
    };

    let source = match exact_text {
        Some(text) => io::Error::new(source.kind(), text),
        None => source,
    };

    ShellError::Io {
        cmd: "cd",
        path: path.display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn too_many_arguments_is_a_usage_error() {
        let err = resolve_target(&argv(&["one", "two"])).unwrap_err();
        assert_eq!(err.to_string(), "cd: too many arguments");
    }

    #[test]
    fn plain_relative_path_is_unchanged() {
        assert_eq!(
            resolve_target(&argv(&["relative/dir"])).unwrap(),
            PathBuf::from("relative/dir")
        );
    }

    #[test]
    fn plain_absolute_path_is_unchanged() {
        assert_eq!(
            resolve_target(&argv(&["/abs/dir"])).unwrap(),
            PathBuf::from("/abs/dir")
        );
    }

    #[test]
    fn dash_is_a_literal_path_not_a_flag() {
        assert_eq!(resolve_target(&argv(&["-"])).unwrap(), PathBuf::from("-"));
    }

    #[test]
    fn tilde_alone_expands_to_home() {
        let home = PathBuf::from("/home/tester");
        assert_eq!(expand_tilde("~", &home), PathBuf::from("/home/tester"));
    }

    #[test]
    fn tilde_slash_expands_to_home_subpath() {
        let home = PathBuf::from("/home/tester");
        assert_eq!(
            expand_tilde("~/projects", &home),
            PathBuf::from("/home/tester/projects")
        );
    }

    #[test]
    fn tilde_username_is_not_expanded() {
        // `~foo` (another user's home) is out of scope; it falls through to
        // a literal relative path, same as real cd would attempt if it
        // couldn't resolve the user.
        let home = PathBuf::from("/home/tester");
        assert_eq!(expand_tilde("~foo", &home), PathBuf::from("~foo"));
    }

    #[test]
    fn not_found_error_message() {
        let err = to_shell_error(
            Path::new("/tmp/not-a-dir-xyz"),
            io::Error::from(io::ErrorKind::NotFound),
        );
        assert_eq!(
            err.to_string(),
            "cd: /tmp/not-a-dir-xyz: No such file or directory"
        );
    }

    #[test]
    fn permission_denied_error_message() {
        let err = to_shell_error(
            Path::new("/root"),
            io::Error::from(io::ErrorKind::PermissionDenied),
        );
        assert_eq!(err.to_string(), "cd: /root: Permission denied");
    }

    #[test]
    fn not_a_directory_error_message() {
        let err = to_shell_error(
            Path::new("/etc/hosts"),
            io::Error::from(io::ErrorKind::NotADirectory),
        );
        assert_eq!(err.to_string(), "cd: /etc/hosts: Not a directory");
    }

    #[test]
    fn other_error_falls_back_to_os_text_without_suffix() {
        let err = to_shell_error(Path::new("/tmp/loop"), io::Error::from_raw_os_error(40)); // ELOOP
        let rendered = err.to_string();
        assert!(rendered.starts_with("cd: /tmp/loop: "));
        assert!(!rendered.contains("os error"));
    }
}
