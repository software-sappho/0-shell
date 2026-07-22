//! `mv` builtin: rename/move a file or directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    match args {
        [] => Err(ShellError::Usage("mv: missing file operand".to_string())),
        [only] => Err(ShellError::Usage(format!(
            "mv: missing destination file operand after '{only}'"
        ))),
        [src, dst] => move_one(src, dst),
        _ => Err(ShellError::Usage("mv: too many arguments".to_string())),
    }
}

fn move_one(src: &str, dst: &str) -> Result<(), ShellError> {
    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    let src_meta = fs::metadata(src_path).map_err(|source| stat_error(src, source))?;
    let final_dst = resolve_destination(src_path, dst_path, src_meta.is_dir())?;

    match fs::rename(src_path, &final_dst) {
        Ok(()) => Ok(()),
        Err(err) if is_cross_device(&err) => {
            move_across_devices(src_path, &final_dst, src_meta.is_dir())
        }
        Err(err) => Err(rename_error(src, &final_dst, err)),
    }
}

/// If `dst` is an existing directory, move into it under the source basename
/// (audit: `mv new_folder2 new_folder1` → `new_folder1/new_folder2`).
fn resolve_destination(src: &Path, dst: &Path, src_is_dir: bool) -> Result<PathBuf, ShellError> {
    match fs::metadata(dst) {
        Ok(meta) if meta.is_dir() => {
            let Some(name) = src.file_name() else {
                return Err(ShellError::Usage(format!(
                    "mv: cannot move '{}': invalid source path",
                    src.display()
                )));
            };
            Ok(dst.join(name))
        }
        Ok(meta) if src_is_dir && meta.is_file() => Err(ShellError::Usage(format!(
            "mv: cannot overwrite non-directory '{}' with directory '{}'",
            dst.display(),
            src.display()
        ))),
        Ok(_) | Err(_) => Ok(dst.to_path_buf()),
    }
}

fn move_across_devices(src: &Path, dst: &Path, src_is_dir: bool) -> Result<(), ShellError> {
    if src_is_dir {
        copy_dir_recursive(src, dst)?;
        fs::remove_dir_all(src).map_err(|source| remove_error(src, source))?;
    } else {
        fs::copy(src, dst).map_err(|source| create_error(dst, source))?;
        fs::remove_file(src).map_err(|source| remove_error(src, source))?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), ShellError> {
    fs::create_dir(dst).map_err(|source| create_error(dst, source))?;
    if let Ok(perms) = fs::metadata(src).map(|m| m.permissions()) {
        let _ = fs::set_permissions(dst, perms);
    }

    for entry in
        fs::read_dir(src).map_err(|source| stat_error(&src.display().to_string(), source))?
    {
        let entry = entry.map_err(|source| stat_error(&src.display().to_string(), source))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|source| stat_error(&from.display().to_string(), source))?;

        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|source| create_error(&to, source))?;
        }
    }
    Ok(())
}

fn is_cross_device(err: &io::Error) -> bool {
    if err.kind() == io::ErrorKind::CrossesDevices {
        return true;
    }
    // Linux EXDEV = 18; Windows ERROR_NOT_SAME_DEVICE = 17.
    matches!(err.raw_os_error(), Some(17) | Some(18))
}

fn stat_error(path: &str, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "mv",
        path: format!("cannot stat '{path}'"),
        source: pin_reason(source),
    }
}

fn rename_error(src: &str, dst: &Path, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "mv",
        path: format!("cannot move '{src}' to '{}'", dst.display()),
        source: pin_reason(source),
    }
}

fn create_error(dst: &Path, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "mv",
        path: format!("cannot create '{}'", dst.display()),
        source: pin_reason(source),
    }
}

fn remove_error(path: &Path, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "mv",
        path: format!("cannot remove '{}'", path.display()),
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
            std::env::temp_dir().join(format!("0-shell-mv-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");
        base
    }

    #[test]
    fn missing_operands_are_usage_errors() {
        assert_eq!(
            run(&[]).unwrap_err().to_string(),
            "mv: missing file operand"
        );
        assert_eq!(
            run(&argv(&["only"])).unwrap_err().to_string(),
            "mv: missing destination file operand after 'only'"
        );
    }

    #[test]
    fn renames_file_within_same_directory() {
        let base = scratch("file");
        let src = base.join("a.txt");
        let dst = base.join("b.txt");
        fs::write(&src, b"payload").unwrap();

        run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap();

        assert!(!src.exists());
        assert_eq!(fs::read(&dst).unwrap(), b"payload");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn moves_directory_into_directory() {
        // Audit case: `mv new_folder2 new_folder1` nests the directory.
        let base = scratch("nest");
        let folder1 = base.join("new_folder1");
        let folder2 = base.join("new_folder2");
        fs::create_dir(&folder1).unwrap();
        fs::create_dir(&folder2).unwrap();
        fs::write(folder2.join("doc.txt"), b"x").unwrap();

        run(&argv(&[
            folder2.to_str().unwrap(),
            folder1.to_str().unwrap(),
        ]))
        .unwrap();

        let nested = folder1.join("new_folder2");
        assert!(nested.is_dir());
        assert!(nested.join("doc.txt").is_file());
        assert!(!folder2.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn moves_file_into_existing_directory() {
        let base = scratch("into-dir");
        let src = base.join("doc.txt");
        let dir = base.join("dest");
        fs::write(&src, b"hi").unwrap();
        fs::create_dir(&dir).unwrap();

        run(&argv(&[src.to_str().unwrap(), dir.to_str().unwrap()])).unwrap();

        assert!(!src.exists());
        assert_eq!(fs::read(dir.join("doc.txt")).unwrap(), b"hi");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn renames_directory_to_new_name() {
        let base = scratch("rename-dir");
        let src = base.join("old_name");
        let dst = base.join("new_name");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("f"), b"1").unwrap();

        run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap();

        assert!(!src.exists());
        assert!(dst.join("f").is_file());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_source_uses_cannot_stat() {
        let base = scratch("missing");
        let src = base.join("nope");
        let dst = base.join("out");
        let err = run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "mv: cannot stat '{}': No such file or directory",
                src.to_str().unwrap()
            )
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn refuses_directory_onto_file() {
        let base = scratch("dir-onto-file");
        let src = base.join("dir");
        let dst = base.join("file");
        fs::create_dir(&src).unwrap();
        fs::write(&dst, b"x").unwrap();

        let err = run(&argv(&[src.to_str().unwrap(), dst.to_str().unwrap()])).unwrap_err();
        assert!(
            err.to_string().contains("cannot overwrite non-directory"),
            "got {err}"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn cross_device_fallback_copies_and_removes_file() {
        let base = scratch("exdev-file");
        let src = base.join("src.txt");
        let dst = base.join("dst.txt");
        fs::write(&src, b"across").unwrap();

        // Simulate EXDEV by calling the fallback path directly.
        move_across_devices(&src, &dst, false).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read(&dst).unwrap(), b"across");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn cross_device_fallback_copies_and_removes_directory() {
        let base = scratch("exdev-dir");
        let src = base.join("src_dir");
        let dst = base.join("dst_dir");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("inner"), b"nested").unwrap();

        move_across_devices(&src, &dst, true).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read(dst.join("inner")).unwrap(), b"nested");
        let _ = fs::remove_dir_all(&base);
    }
}
