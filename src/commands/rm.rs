//! `rm` builtin: remove files; `-r` removes directories depth-first.

use std::fs;
use std::io;
use std::path::Path;

use crate::error::ShellError;

#[derive(Debug, Default, Clone, Copy)]
struct Flags {
    /// `-r` / `-R`: remove directories and their contents recursively.
    recursive: bool,
}

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let (flags, targets) = parse_args(args)?;
    if targets.is_empty() {
        return Err(ShellError::Usage("rm: missing operand".to_string()));
    }
    for target in targets {
        remove_one(target, flags)?;
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<(Flags, Vec<&str>), ShellError> {
    let mut flags = Flags::default();
    let mut targets = Vec::new();
    let mut parsing_flags = true;

    for arg in args {
        if parsing_flags && arg.as_str() == "--" {
            parsing_flags = false;
            continue;
        }
        if parsing_flags && arg.starts_with('-') && arg.as_str() != "-" {
            apply_option(arg, &mut flags)?;
        } else {
            targets.push(arg.as_str());
        }
    }

    Ok((flags, targets))
}

fn apply_option(arg: &str, flags: &mut Flags) -> Result<(), ShellError> {
    if arg.starts_with("--") {
        return Err(ShellError::Usage(format!(
            "rm: unrecognized option '{arg}'"
        )));
    }

    for c in arg.chars().skip(1) {
        match c {
            'r' | 'R' => flags.recursive = true,
            _ => {
                return Err(ShellError::Usage(format!("rm: invalid option -- '{c}'")));
            }
        }
    }
    Ok(())
}

fn remove_one(path: &str, flags: Flags) -> Result<(), ShellError> {
    let p = Path::new(path);
    // Use symlink metadata so a symlink-to-directory is treated as a link
    // (removable without `-r`), matching GNU rm.
    let meta = fs::symlink_metadata(p).map_err(|source| remove_error(path, source))?;

    if meta.is_dir() {
        if !flags.recursive {
            return Err(ShellError::Io {
                cmd: "rm",
                path: format!("cannot remove '{path}'"),
                source: io::Error::other("Is a directory"),
            });
        }
        remove_dir_depth_first(p)?;
    } else {
        fs::remove_file(p).map_err(|source| remove_error(path, source))?;
    }
    Ok(())
}

/// Walk children first, then remove the directory itself (audit: `rm -r`).
fn remove_dir_depth_first(path: &Path) -> Result<(), ShellError> {
    let entries =
        fs::read_dir(path).map_err(|source| remove_error(&path.display().to_string(), source))?;

    for entry in entries {
        let entry = entry.map_err(|source| remove_error(&path.display().to_string(), source))?;
        let child = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| remove_error(&child.display().to_string(), source))?;

        if file_type.is_dir() {
            remove_dir_depth_first(&child)?;
        } else {
            fs::remove_file(&child)
                .map_err(|source| remove_error(&child.display().to_string(), source))?;
        }
    }

    fs::remove_dir(path).map_err(|source| remove_error(&path.display().to_string(), source))
}

fn remove_error(path: &str, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "rm",
        path: format!("cannot remove '{path}'"),
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
    use std::path::PathBuf;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    fn scratch(label: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("0-shell-rm-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");
        base
    }

    #[test]
    fn missing_operand_is_usage_error() {
        assert_eq!(run(&[]).unwrap_err().to_string(), "rm: missing operand");
    }

    #[test]
    fn removes_a_file() {
        let base = scratch("file");
        let file = base.join("doc.txt");
        fs::write(&file, b"x").unwrap();

        run(&argv(&[file.to_str().unwrap()])).unwrap();
        assert!(!file.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn directory_without_r_is_an_error() {
        let base = scratch("dir");
        let dir = base.join("folder");
        fs::create_dir(&dir).unwrap();

        let err = run(&argv(&[dir.to_str().unwrap()])).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "rm: cannot remove '{}': Is a directory",
                dir.to_str().unwrap()
            )
        );
        assert!(dir.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn recursive_removes_nested_tree() {
        // Audit-shaped case: `rm -r new_folder1`.
        let base = scratch("tree");
        let root = base.join("new_folder1");
        let nested = root.join("sub");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("a.txt"), b"1").unwrap();
        fs::write(nested.join("b.txt"), b"2").unwrap();

        run(&argv(&["-r", root.to_str().unwrap()])).unwrap();
        assert!(!root.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn capital_r_is_accepted() {
        let base = scratch("R");
        let dir = base.join("d");
        fs::create_dir(&dir).unwrap();
        run(&argv(&["-R", dir.to_str().unwrap()])).unwrap();
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_path_uses_cannot_remove() {
        let base = scratch("missing");
        let missing = base.join("nope");
        let err = run(&argv(&[missing.to_str().unwrap()])).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "rm: cannot remove '{}': No such file or directory",
                missing.to_str().unwrap()
            )
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn removes_multiple_files() {
        let base = scratch("multi");
        let a = base.join("a");
        let b = base.join("b");
        fs::write(&a, b"").unwrap();
        fs::write(&b, b"").unwrap();

        run(&argv(&[a.to_str().unwrap(), b.to_str().unwrap()])).unwrap();
        assert!(!a.exists());
        assert!(!b.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn invalid_option_is_usage_error() {
        let err = parse_args(&argv(&["-z"])).unwrap_err();
        assert_eq!(err.to_string(), "rm: invalid option -- 'z'");
    }

    #[test]
    fn double_dash_ends_flag_parsing() {
        let base = scratch("ddash");
        let weird = base.join("-r");
        fs::write(&weird, b"x").unwrap();
        run(&argv(&["--", weird.to_str().unwrap()])).unwrap();
        assert!(!weird.exists());
        let _ = fs::remove_dir_all(&base);
    }
}
