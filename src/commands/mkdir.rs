//! `mkdir` builtin: create each named directory.

use std::fs;
use std::io;

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    for arg in args {
        create_one(arg)?;
    }
    Ok(())
}

fn create_one(arg: &str) -> Result<(), ShellError> {
    let path = effective_path(arg);
    fs::create_dir(path).map_err(|source| to_shell_error(path, source))
}

// A single argument containing '/' only creates its first path component and
// stops there — e.g. `mkdir foo/bar` creates just `foo`. A leading '/'
// (an absolute path) is passed through whole rather than truncated, since
// truncating there would leave an empty, meaningless prefix.
fn effective_path(arg: &str) -> &str {
    match arg.find('/') {
        Some(0) | None => arg,
        Some(idx) => &arg[..idx],
    }
}

fn to_shell_error(path: &str, source: io::Error) -> ShellError {
    if source.kind() == io::ErrorKind::AlreadyExists {
        ShellError::Io {
            cmd: "mkdir",
            path: format!("cannot create directory '{path}'"),
            source,
        }
    } else {
        ShellError::Io {
            cmd: "mkdir",
            path: path.to_string(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn truncates_relative_multi_component_path_at_first_slash() {
        assert_eq!(effective_path("foo/bar"), "foo");
    }

    #[test]
    fn plain_name_is_unchanged() {
        assert_eq!(effective_path("foo"), "foo");
    }

    #[test]
    fn leading_slash_is_not_truncated() {
        assert_eq!(effective_path("/root/nope"), "/root/nope");
    }

    #[test]
    fn empty_args_does_nothing() {
        assert!(run(&[]).is_ok());
    }

    #[test]
    fn already_exists_message() {
        let source = io::Error::from_raw_os_error(17); // EEXIST
        let err = to_shell_error("foo", source);
        assert_eq!(
            err.to_string(),
            "mkdir: cannot create directory 'foo': File exists"
        );
    }

    #[test]
    fn other_error_uses_generic_cmd_path_reason_format() {
        let source = io::Error::from_raw_os_error(13); // EACCES
        let err = to_shell_error("/root/nope", source);
        assert_eq!(err.to_string(), "mkdir: /root/nope: Permission denied");
    }

    #[test]
    fn creates_two_sibling_directories_not_nested() {
        let base = std::env::temp_dir().join(format!(
            "0-shell-mkdir-test-siblings-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");

        let first = base.join("first");
        let second = base.join("second");
        let args = argv(&[
            first.to_str().expect("utf8 path"),
            second.to_str().expect("utf8 path"),
        ]);
        assert!(run(&args).is_ok());
        assert!(first.is_dir());
        assert!(second.is_dir());
        assert!(!first.join("second").exists());
        assert!(!second.join("first").exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn second_create_of_same_dir_is_already_exists() {
        let base =
            std::env::temp_dir().join(format!("0-shell-mkdir-test-exists-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");

        let target = base.join("foo");
        let target_str = target.to_str().expect("utf8 path").to_string();
        assert!(run(std::slice::from_ref(&target_str)).is_ok());
        let err = run(&[target_str]).unwrap_err();
        assert!(err.to_string().contains("File exists"));

        let _ = fs::remove_dir_all(&base);
    }
}
