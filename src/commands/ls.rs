//! `ls` builtin: plain directory listing (no flags yet — see SH-011 / SH-012).

use std::fs;
use std::io::{self, Write};

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    run_with_writer(args, &mut out)
}

fn run_with_writer(args: &[String], out: &mut dyn Write) -> Result<(), ShellError> {
    // Bare `ls` lists the current directory, same as `ls .`.
    let targets: Vec<&str> = if args.is_empty() {
        vec!["."]
    } else {
        args.iter().map(String::as_str).collect()
    };

    let classified = classify_targets(&targets)?;
    write_listing(&classified, out)
}

#[derive(Debug)]
struct Classified {
    /// Non-directory operands, already sorted in operand order after name sort.
    files: Vec<String>,
    /// Directory operands to list the contents of.
    dirs: Vec<String>,
    /// Whether directory headers (`name:`) should be printed.
    show_headers: bool,
}

fn classify_targets(targets: &[&str]) -> Result<Classified, ShellError> {
    // GNU ls sorts operands by name first, then emits non-directories, then
    // directories. Keep that order so multi-arg listings stay familiar.
    let mut sorted: Vec<&str> = targets.to_vec();
    sorted.sort();

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for path in sorted {
        let meta = fs::metadata(path).map_err(|source| access_error(path, source))?;
        if meta.is_dir() {
            dirs.push(path.to_string());
        } else {
            files.push(path.to_string());
        }
    }

    // Headers appear when there is more than one operand overall, or when a
    // directory is listed alongside any files. A lone directory (or bare
    // `ls`) prints contents with no `dirname:` banner.
    let show_headers = targets.len() > 1;

    Ok(Classified {
        files,
        dirs,
        show_headers,
    })
}

fn write_listing(classified: &Classified, out: &mut dyn Write) -> Result<(), ShellError> {
    for file in &classified.files {
        writeln!(out, "{file}").map_err(write_error)?;
    }

    for (i, dir) in classified.dirs.iter().enumerate() {
        if classified.show_headers {
            if i > 0 || !classified.files.is_empty() {
                writeln!(out).map_err(write_error)?;
            }
            writeln!(out, "{dir}:").map_err(write_error)?;
        }

        let names = list_dir_names(dir)?;
        for name in names {
            writeln!(out, "{name}").map_err(write_error)?;
        }
    }

    out.flush().map_err(write_error)?;
    Ok(())
}

fn list_dir_names(path: &str) -> Result<Vec<String>, ShellError> {
    let mut names = Vec::new();

    let entries = fs::read_dir(path).map_err(|source| open_dir_error(path, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| open_dir_error(path, source))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Plain ls hides dotfiles, including `.` and `..`.
        if name.starts_with('.') {
            continue;
        }
        names.push(name.into_owned());
    }

    // Bytewise / lexicographic order on the UTF-8 name. For the ASCII names
    // the audit uses, this matches `ls` under `LANG=C`.
    names.sort();
    Ok(names)
}

fn access_error(path: &str, source: io::Error) -> ShellError {
    to_shell_error(
        &format!("cannot access '{path}'"),
        pin_reason(
            source,
            &[
                (io::ErrorKind::NotFound, "No such file or directory"),
                (io::ErrorKind::PermissionDenied, "Permission denied"),
            ],
        ),
    )
}

fn open_dir_error(path: &str, source: io::Error) -> ShellError {
    to_shell_error(
        &format!("cannot open directory '{path}'"),
        pin_reason(
            source,
            &[
                (io::ErrorKind::NotFound, "No such file or directory"),
                (io::ErrorKind::PermissionDenied, "Permission denied"),
            ],
        ),
    )
}

fn write_error(source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "ls",
        path: String::new(),
        source,
    }
}

fn to_shell_error(path: &str, source: io::Error) -> ShellError {
    ShellError::Io {
        cmd: "ls",
        path: path.to_string(),
        source,
    }
}

fn pin_reason(source: io::Error, pinned: &[(io::ErrorKind, &str)]) -> io::Error {
    for &(kind, text) in pinned {
        if source.kind() == kind {
            return io::Error::new(kind, text);
        }
    }
    source
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
            std::env::temp_dir().join(format!("0-shell-ls-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("failed to set up test scratch dir");
        base
    }

    fn listing(args: &[String]) -> String {
        let mut out = Vec::new();
        run_with_writer(args, &mut out).expect("ls should succeed");
        String::from_utf8(out).expect("utf8 output")
    }

    #[test]
    fn hides_dotfiles_and_sorts_names() {
        let base = scratch("plain");
        fs::write(base.join("zebra"), b"").unwrap();
        fs::write(base.join("alpha"), b"").unwrap();
        fs::write(base.join(".hidden"), b"").unwrap();
        fs::create_dir(base.join("mid")).unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&[path])), "alpha\nmid\nzebra\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn bare_ls_lists_current_directory() {
        struct RestoreCwd(PathBuf);
        impl Drop for RestoreCwd {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }

        let base = scratch("cwd");
        fs::write(base.join("only"), b"x").unwrap();

        let original = std::env::current_dir().unwrap();
        let _restore = RestoreCwd(original);
        std::env::set_current_dir(&base).unwrap();

        assert_eq!(listing(&[]), "only\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn single_file_operand_prints_the_path() {
        let base = scratch("file");
        let file = base.join("doc.txt");
        fs::write(&file, b"hi").unwrap();

        let path = file.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&[path])), format!("{path}\n"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn single_dir_has_no_header() {
        let base = scratch("one-dir");
        fs::write(base.join("a"), b"").unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&[path])), "a\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn multiple_dirs_print_headers_and_blank_line() {
        let base = scratch("multi");
        let a = base.join("a");
        let b = base.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("one"), b"").unwrap();
        fs::write(b.join("two"), b"").unwrap();

        let a_s = a.to_str().expect("utf8 path");
        let b_s = b.to_str().expect("utf8 path");
        // Operands are sorted, so order follows the path strings.
        let mut paths = [a_s, b_s];
        paths.sort();
        let expected = format!("{}:\none\n\n{}:\ntwo\n", paths[0], paths[1]);
        assert_eq!(listing(&argv(&[a_s, b_s])), expected);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_path_uses_cannot_access_message() {
        let missing =
            std::env::temp_dir().join(format!("0-shell-ls-missing-{}", std::process::id()));
        let _ = fs::remove_file(&missing);
        let path = missing.to_str().expect("utf8 path").to_string();

        let mut out = Vec::new();
        let err = run_with_writer(std::slice::from_ref(&path), &mut out).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("ls: cannot access '{path}': No such file or directory")
        );
    }

    #[test]
    fn empty_directory_prints_nothing() {
        let base = scratch("empty");
        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&[path])), "");
        let _ = fs::remove_dir_all(&base);
    }
}
