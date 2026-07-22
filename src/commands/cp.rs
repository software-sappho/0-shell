//! `cp` builtin: copy a file to a new path (no recursive directory copy yet).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    match args {
        [] => Err(ShellError::Usage("cp: missing file operand".to_string())),
        [only] => Err(ShellError::Usage(format!(
            "cp: missing destination file operand after '{only}'"
        ))),
        [src, dst] => copy_one(src, dst),
        _ => Err(ShellError::Usage("cp: too many arguments".to_string())),
    }
}

fn copy_one(src: &str, dst: &str) -> Result<(), ShellError> {
    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    let src_meta = fs::metadata(src_path).map_err(|source| stat_error(src, source))?;

    if src_meta.is_dir() {
        // No `-r` in this ticket — refuse directories the way GNU cp does.
        return Err(ShellError::Usage(format!(
            "cp: -r not specified; omitting directory '{src}'"
        )));
    }

    let final_dst = resolve_destination(src_path, dst_path)?;
    fs::copy(src_path, &final_dst).map_err(|source| copy_error(&final_dst, source))?;
    Ok(())
}

/// If `dst` is an existing directory, copy into it under the source basename.
fn resolve_destination(src: &Path, dst: &Path) -> Result<PathBuf, ShellError> {
    match fs::metadata(dst) {
        Ok(meta) if meta.is_dir() => {
            let Some(name) = src.file_name() else {
                return Err(ShellError::Usage(format!(
                    "cp: cannot copy '{}': invalid source path",
                    src.display()
                )));
            };
            Ok(dst.join(name))
        }
        Ok(_) | Err(_) => Ok(dst.to_path_buf()),
    }
}

fn stat_error(path: &str, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "cp",
        path: format!("cannot stat '{path}'"),
        source: pin_reason(source),
    }
}

fn copy_error(dst: &Path, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "cp",
        path: format!("cannot create regular file '{}'", dst.display()),
        source: pin_reason(source),
    }
}

fn pin_reason(source: io::Error) -> io::Error {
    let exact = match source.kind() {
        io::ErrorKind::NotFound => Some("No such file or directory"),
        io::ErrorKind::PermissionDenied => Some("Permission denied"),
        _ => None,
    };
    match exact {
        Some(text) => io::Error::new(source.kind(), text),
        None => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    fn scratch(label: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("0-shell-cp-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");
        base
    }

    #[test]
    fn missing_operands_are_usage_errors() {
        assert_eq!(
            run(&[]).unwrap_err().to_string(),
            "cp: missing file operand"
        );
        assert_eq!(
            run(&argv(&["only"])).unwrap_err().to_string(),
            "cp: missing destination file operand after 'only'"
        );
    }

    #[test]
    fn copies_file_contents_to_new_path() {
        let base = scratch("basic");
        let src = base.join("new_doc.txt");
        let dst = base.join("copy.txt");
        fs::write(&src, b"hello audit").unwrap();

        run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap();

        assert_eq!(fs::read(&dst).unwrap(), b"hello audit");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn copies_into_existing_directory_preserving_basename() {
        // Audit-shaped case: `cp new_doc.txt ../new_folder2` (dst is a dir).
        let base = scratch("into-dir");
        let src = base.join("new_doc.txt");
        let folder = base.join("new_folder2");
        fs::write(&src, b"payload").unwrap();
        fs::create_dir(&folder).unwrap();

        run(&argv(&[src.to_str().unwrap(), folder.to_str().unwrap()])).unwrap();

        let copied = folder.join("new_doc.txt");
        assert!(copied.is_file());
        assert_eq!(fs::read(copied).unwrap(), b"payload");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_source_uses_cannot_stat() {
        let base = scratch("missing");
        let src = base.join("nope.txt");
        let dst = base.join("out.txt");
        let err = run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "cp: cannot stat '{}': No such file or directory",
                src.to_str().unwrap()
            )
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn directory_source_is_omitted_without_recursive() {
        let base = scratch("dir-src");
        let src = base.join("folder");
        let dst = base.join("out");
        fs::create_dir(&src).unwrap();

        let err = run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "cp: -r not specified; omitting directory '{}'",
                src.to_str().unwrap()
            )
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn overwrites_existing_file_destination() {
        let base = scratch("overwrite");
        let src = base.join("a.txt");
        let dst = base.join("b.txt");
        fs::write(&src, b"new").unwrap();
        fs::write(&dst, b"old").unwrap();

        run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"new");
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn preserves_mode_bits() {
        use std::os::unix::fs::PermissionsExt;

        let base = scratch("mode");
        let src = base.join("script.sh");
        let dst = base.join("script.copy");
        fs::write(&src, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&src).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&src, perms).unwrap();

        run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap();
        let mode = fs::metadata(&dst).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        let _ = fs::remove_dir_all(&base);
    }
}
