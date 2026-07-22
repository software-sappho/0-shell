//! `cat` builtin: stream each named file's bytes to stdout, unmodified.

use std::fs::{self, File};
use std::io::{self, Read, Write};

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin = stdin.lock();
    let mut stdout = stdout.lock();
    run_with_io(args, &mut stdin, &mut stdout)
}

fn run_with_io(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), ShellError> {
    if args.is_empty() {
        return copy_reader(stdin, stdout, "");
    }
    for arg in args {
        cat_one(arg, stdout)?;
    }
    Ok(())
}

fn cat_one(path: &str, stdout: &mut dyn Write) -> Result<(), ShellError> {
    // Check directories up front: on Unix, `File::open` of a directory often
    // succeeds and only the subsequent read returns EISDIR. Real `cat`
    // reports "Is a directory" either way; detecting via metadata keeps the
    // message portable (Windows included).
    let meta = fs::metadata(path).map_err(|source| to_shell_error(path, source))?;
    if meta.is_dir() {
        return Err(directory_error(path));
    }

    let mut file = File::open(path).map_err(|source| to_shell_error(path, source))?;
    copy_reader(&mut file, stdout, path)
}

fn copy_reader(
    reader: &mut dyn Read,
    stdout: &mut dyn Write,
    path: &str,
) -> Result<(), ShellError> {
    io::copy(reader, stdout)
        .and_then(|_| stdout.flush())
        .map_err(|source| to_shell_error(path, source))?;
    Ok(())
}

fn directory_error(path: &str) -> ShellError {
    ShellError::Io {
        cmd: "cat",
        path: path.to_string(),
        source: io::Error::other("Is a directory"),
    }
}

// Same approach as `cd`: pin the audit-checked reason strings so they match
// coreutils byte-for-byte regardless of OS strerror wording.
fn to_shell_error(path: &str, source: io::Error) -> ShellError {
    let exact_text = match source.kind() {
        io::ErrorKind::NotFound => Some("No such file or directory"),
        io::ErrorKind::PermissionDenied => Some("Permission denied"),
        _ => None,
    };

    let source = match exact_text {
        Some(text) => io::Error::new(source.kind(), text),
        None => source,
    };

    ShellError::Io {
        cmd: "cat",
        path: path.to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    fn scratch(label: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("0-shell-cat-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");
        base
    }

    #[test]
    fn directory_message_matches_coreutils() {
        assert_eq!(
            directory_error("foo").to_string(),
            "cat: foo: Is a directory"
        );
    }

    #[test]
    fn not_found_error_message() {
        let err = to_shell_error("/tmp/missing-xyz", io::Error::from(io::ErrorKind::NotFound));
        assert_eq!(
            err.to_string(),
            "cat: /tmp/missing-xyz: No such file or directory"
        );
    }

    #[test]
    fn permission_denied_error_message() {
        let err = to_shell_error(
            "/root/secret",
            io::Error::from(io::ErrorKind::PermissionDenied),
        );
        assert_eq!(err.to_string(), "cat: /root/secret: Permission denied");
    }

    #[test]
    fn streams_file_bytes_unmodified() {
        let base = scratch("bytes");
        let file = base.join("doc.txt");
        // Include a missing trailing newline and a non-UTF-8 byte so we prove
        // we don't reinterpret or "fix" the contents.
        let contents: &[u8] = b"hello\xffworld";
        fs::write(&file, contents).expect("write fixture");

        let mut out = Vec::new();
        let mut stdin = io::empty();
        let path = file.to_str().expect("utf8 path");
        assert!(run_with_io(&argv(&[path]), &mut stdin, &mut out).is_ok());
        assert_eq!(out, contents);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn concatenates_multiple_operands() {
        let base = scratch("multi");
        let a = base.join("a.txt");
        let b = base.join("b.txt");
        fs::write(&a, b"one").expect("write a");
        fs::write(&b, b"two").expect("write b");

        let mut out = Vec::new();
        let mut stdin = io::empty();
        let args = argv(&[
            a.to_str().expect("utf8 path"),
            b.to_str().expect("utf8 path"),
        ]);
        assert!(run_with_io(&args, &mut stdin, &mut out).is_ok());
        assert_eq!(out, b"onetwo");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn directory_operand_is_an_error() {
        let base = scratch("dir");
        let mut out = Vec::new();
        let mut stdin = io::empty();
        let path = base.to_str().expect("utf8 path").to_string();
        let err = run_with_io(std::slice::from_ref(&path), &mut stdin, &mut out).unwrap_err();
        assert_eq!(err.to_string(), format!("cat: {path}: Is a directory"));
        assert!(out.is_empty());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_file_is_an_error() {
        let missing =
            std::env::temp_dir().join(format!("0-shell-cat-missing-{}", std::process::id()));
        let _ = fs::remove_file(&missing);
        let path = missing.to_str().expect("utf8 path").to_string();

        let mut out = Vec::new();
        let mut stdin = io::empty();
        let err = run_with_io(std::slice::from_ref(&path), &mut stdin, &mut out).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("cat: {path}: No such file or directory")
        );
    }

    #[test]
    fn empty_args_copies_stdin() {
        let mut stdin = &b"from stdin"[..];
        let mut out = Vec::new();
        assert!(run_with_io(&[], &mut stdin, &mut out).is_ok());
        assert_eq!(out, b"from stdin");
    }
}
