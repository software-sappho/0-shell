//! `ls` builtin: directory listing with `-a` / `-F` (and `-l` parsed for SH-012).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::ShellError;

#[derive(Debug, Default, Clone, Copy)]
struct Flags {
    /// `-a`: include `.`, `..`, and other dotfiles.
    all: bool,
    /// `-F`: append `/` (dir), `*` (exec), `@` (symlink).
    classify: bool,
    /// `-l`: accepted here so combined forms like `-la` parse; long format is SH-012.
    #[allow(dead_code)]
    long: bool,
}

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    run_with_writer(args, &mut out)
}

fn run_with_writer(args: &[String], out: &mut dyn Write) -> Result<(), ShellError> {
    let (flags, targets) = parse_args(args)?;
    let targets: Vec<&str> = if targets.is_empty() {
        vec!["."]
    } else {
        targets
    };

    let classified = classify_targets(&targets)?;
    write_listing(&classified, flags, out)
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
            "ls: unrecognized option '{arg}'"
        )));
    }

    for c in arg.chars().skip(1) {
        match c {
            'a' => flags.all = true,
            'F' => flags.classify = true,
            'l' => flags.long = true,
            _ => {
                return Err(ShellError::Usage(format!("ls: invalid option -- '{c}'")));
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Classified {
    files: Vec<String>,
    dirs: Vec<String>,
    show_headers: bool,
}

fn classify_targets(targets: &[&str]) -> Result<Classified, ShellError> {
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

    let show_headers = targets.len() > 1;

    Ok(Classified {
        files,
        dirs,
        show_headers,
    })
}

fn write_listing(
    classified: &Classified,
    flags: Flags,
    out: &mut dyn Write,
) -> Result<(), ShellError> {
    for file in &classified.files {
        let line = format_path_entry(Path::new(file), file, flags)?;
        writeln!(out, "{line}").map_err(write_error)?;
    }

    for (i, dir) in classified.dirs.iter().enumerate() {
        if classified.show_headers {
            if i > 0 || !classified.files.is_empty() {
                writeln!(out).map_err(write_error)?;
            }
            writeln!(out, "{dir}:").map_err(write_error)?;
        }

        for name in list_dir_entries(dir, flags)? {
            writeln!(out, "{name}").map_err(write_error)?;
        }
    }

    out.flush().map_err(write_error)?;
    Ok(())
}

fn list_dir_entries(path: &str, flags: Flags) -> Result<Vec<String>, ShellError> {
    let mut entries: Vec<(String, String)> = Vec::new();
    let dir = PathBuf::from(path);

    if flags.all {
        // `read_dir` never yields `.` / `..`; inject them when `-a` is set.
        entries.push(display_pair(&dir, ".", flags)?);
        entries.push(display_pair(&dir, "..", flags)?);
    }

    let read = fs::read_dir(path).map_err(|source| open_dir_error(path, source))?;
    for entry in read {
        let entry = entry.map_err(|source| open_dir_error(path, source))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !flags.all && name.starts_with('.') {
            continue;
        }
        entries.push(display_pair(&dir, &name, flags)?);
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries.into_iter().map(|(_, display)| display).collect())
}

fn display_pair(dir: &Path, name: &str, flags: Flags) -> Result<(String, String), ShellError> {
    let display = format_path_entry(&dir.join(name), name, flags)?;
    Ok((name.to_string(), display))
}

fn format_path_entry(path: &Path, printed: &str, flags: Flags) -> Result<String, ShellError> {
    if !flags.classify {
        return Ok(printed.to_string());
    }

    // Prefer symlink metadata so a link-to-dir gets `@`, not `/`.
    let meta = fs::symlink_metadata(path)
        .map_err(|source| access_error(&path.display().to_string(), source))?;
    let ft = meta.file_type();

    let mut out = printed.to_string();
    if ft.is_symlink() {
        out.push('@');
    } else if ft.is_dir() {
        out.push('/');
    } else if ft.is_file() && is_executable(&meta) {
        out.push('*');
    }
    Ok(out)
}

#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(meta: &fs::Metadata) -> bool {
    // Windows has no POSIX exec bits; treat nothing as executable for `-F`.
    let _ = meta;
    false
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

    #[test]
    fn dash_a_shows_dot_dotdot_and_hidden() {
        let base = scratch("all");
        fs::write(base.join("visible"), b"").unwrap();
        fs::write(base.join(".secret"), b"").unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&["-a", path])), ".\n..\n.secret\nvisible\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn combined_la_enables_all_without_treating_l_as_path() {
        let base = scratch("la");
        fs::write(base.join(".dot"), b"").unwrap();
        fs::write(base.join("file"), b"").unwrap();

        let path = base.to_str().expect("utf8 path");
        // `-l` is parsed (for SH-012) but short listing still applies for now.
        assert_eq!(listing(&argv(&["-la", path])), ".\n..\n.dot\nfile\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dash_f_appends_slash_for_directories() {
        let base = scratch("classify-dir");
        fs::create_dir(base.join("subdir")).unwrap();
        fs::write(base.join("plain"), b"").unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&["-F", path])), "plain\nsubdir/\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dash_f_on_file_operand_leaves_plain_file_unchanged() {
        let base = scratch("classify-file");
        let file = base.join("doc.txt");
        fs::write(&file, b"x").unwrap();

        let path = file.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&["-F", path])), format!("{path}\n"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn separate_flags_combine() {
        let base = scratch("af");
        fs::create_dir(base.join("d")).unwrap();
        fs::write(base.join(".h"), b"").unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&["-a", "-F", path])), "./\n../\n.h\nd/\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn invalid_option_is_a_usage_error() {
        let err = parse_args(&argv(&["-z"])).unwrap_err();
        assert_eq!(err.to_string(), "ls: invalid option -- 'z'");
    }

    #[test]
    fn double_dash_ends_flag_parsing() {
        let base = scratch("ddash");
        let weird = base.join("-z");
        fs::write(&weird, b"").unwrap();
        let weird_s = weird.to_str().expect("utf8 path");

        // Without `--`, `-z` is an invalid option. After `--` it is a path.
        assert_eq!(listing(&argv(&["--", weird_s])), format!("{weird_s}\n"));

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn dash_f_appends_star_for_executable() {
        use std::os::unix::fs::PermissionsExt;

        let base = scratch("exec");
        let bin = base.join("run");
        fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms).unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&["-F", path])), "run*\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn dash_f_appends_at_for_symlink() {
        let base = scratch("link");
        let target = base.join("target");
        fs::write(&target, b"x").unwrap();
        std::os::unix::fs::symlink("target", base.join("link")).unwrap();

        let path = base.to_str().expect("utf8 path");
        assert_eq!(listing(&argv(&["-F", path])), "link@\ntarget\n");

        let _ = fs::remove_dir_all(&base);
    }
}
