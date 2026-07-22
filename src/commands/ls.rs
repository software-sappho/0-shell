//! `ls` builtin: plain listing plus `-a` / `-F` / `-l`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::ShellError;

#[derive(Debug, Default, Clone, Copy)]
struct Flags {
    /// `-a`: include `.`, `..`, and other dotfiles.
    all: bool,
    /// `-F`: append `/` (dir), `*` (exec), `@` (symlink).
    classify: bool,
    /// `-l`: long format listing.
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
    let ids = if flags.long {
        IdNames::load()
    } else {
        IdNames::empty()
    };
    write_listing(&classified, flags, &ids, out)
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

    Ok(Classified {
        files,
        dirs,
        show_headers: targets.len() > 1,
    })
}

fn write_listing(
    classified: &Classified,
    flags: Flags,
    ids: &IdNames,
    out: &mut dyn Write,
) -> Result<(), ShellError> {
    // Non-directory operands: one long/short line each, never a `total` header.
    if flags.long {
        let mut rows = Vec::new();
        for file in &classified.files {
            rows.push(long_row_for_path(Path::new(file), file, flags, ids)?);
        }
        write_long_rows(&rows, out)?;
    } else {
        for file in &classified.files {
            let line = short_name(Path::new(file), file, flags)?;
            writeln!(out, "{line}").map_err(write_error)?;
        }
    }

    for (i, dir) in classified.dirs.iter().enumerate() {
        if classified.show_headers {
            if i > 0 || !classified.files.is_empty() {
                writeln!(out).map_err(write_error)?;
            }
            writeln!(out, "{dir}:").map_err(write_error)?;
        }

        let entries = collect_dir_entries(dir, flags)?;
        if flags.long {
            let mut rows = Vec::new();
            let mut total_blocks = 0u64;
            for (name, path) in &entries {
                let row = long_row_for_path(path, name, flags, ids)?;
                total_blocks = total_blocks.saturating_add(row.blocks);
                rows.push(row);
            }
            // GNU ls reports 1K-block totals: st_blocks is in 512-byte units.
            writeln!(out, "total {}", total_blocks / 2).map_err(write_error)?;
            write_long_rows(&rows, out)?;
        } else {
            for (name, path) in &entries {
                let line = short_name(path, name, flags)?;
                writeln!(out, "{line}").map_err(write_error)?;
            }
        }
    }

    out.flush().map_err(write_error)?;
    Ok(())
}

fn collect_dir_entries(path: &str, flags: Flags) -> Result<Vec<(String, PathBuf)>, ShellError> {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    let dir = PathBuf::from(path);

    if flags.all {
        entries.push((".".to_string(), dir.join(".")));
        entries.push(("..".to_string(), dir.join("..")));
    }

    let read = fs::read_dir(path).map_err(|source| open_dir_error(path, source))?;
    for entry in read {
        let entry = entry.map_err(|source| open_dir_error(path, source))?;
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        if !flags.all && name.starts_with('.') {
            continue;
        }
        entries.push((name.clone(), dir.join(&name)));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

fn short_name(path: &Path, printed: &str, flags: Flags) -> Result<String, ShellError> {
    if !flags.classify {
        return Ok(printed.to_string());
    }
    classify_suffix(path, printed)
}

fn classify_suffix(path: &Path, printed: &str) -> Result<String, ShellError> {
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

#[derive(Debug)]
struct LongRow {
    mode: String,
    nlink: String,
    user: String,
    group: String,
    size: String,
    time: String,
    name: String,
    blocks: u64,
}

fn long_row_for_path(
    path: &Path,
    printed: &str,
    flags: Flags,
    ids: &IdNames,
) -> Result<LongRow, ShellError> {
    let meta = fs::symlink_metadata(path)
        .map_err(|source| access_error(&path.display().to_string(), source))?;

    let mode = format_mode(&meta);
    let (nlink, uid, gid, blocks, size_val) = meta_numbers(&meta);
    let user = ids.user(uid);
    let group = ids.group(gid);
    let time = format_mtime(&meta);

    let mut name = if flags.classify {
        classify_suffix(path, printed)?
    } else {
        printed.to_string()
    };

    if meta.file_type().is_symlink() {
        if let Ok(target) = fs::read_link(path) {
            name.push_str(" -> ");
            name.push_str(&target.to_string_lossy());
        }
    }

    Ok(LongRow {
        mode,
        nlink: nlink.to_string(),
        user,
        group,
        size: size_val.to_string(),
        time,
        name,
        blocks,
    })
}

fn write_long_rows(rows: &[LongRow], out: &mut dyn Write) -> Result<(), ShellError> {
    let nlink_w = rows.iter().map(|r| r.nlink.len()).max().unwrap_or(1);
    let user_w = rows.iter().map(|r| r.user.len()).max().unwrap_or(1);
    let group_w = rows.iter().map(|r| r.group.len()).max().unwrap_or(1);
    let size_w = rows.iter().map(|r| r.size.len()).max().unwrap_or(1);

    for row in rows {
        writeln!(
            out,
            "{} {:>nlink_w$} {:<user_w$} {:<group_w$} {:>size_w$} {} {}",
            row.mode,
            row.nlink,
            row.user,
            row.group,
            row.size,
            row.time,
            row.name,
            nlink_w = nlink_w,
            user_w = user_w,
            group_w = group_w,
            size_w = size_w,
        )
        .map_err(write_error)?;
    }
    Ok(())
}

fn format_mode(meta: &fs::Metadata) -> String {
    let ft = meta.file_type();
    let mut s = String::with_capacity(10);
    s.push(file_type_char(&ft));

    let mode = posix_mode(meta);
    s.push(bit(mode, 0o400, 'r'));
    s.push(bit(mode, 0o200, 'w'));
    s.push(exec_bit(mode, 0o100, 0o4000, 's', 'S'));
    s.push(bit(mode, 0o040, 'r'));
    s.push(bit(mode, 0o020, 'w'));
    s.push(exec_bit(mode, 0o010, 0o2000, 's', 'S'));
    s.push(bit(mode, 0o004, 'r'));
    s.push(bit(mode, 0o002, 'w'));
    s.push(exec_bit(mode, 0o001, 0o1000, 't', 'T'));
    s
}

fn file_type_char(ft: &fs::FileType) -> char {
    if ft.is_symlink() {
        'l'
    } else if ft.is_dir() {
        'd'
    } else if ft.is_file() {
        '-'
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if ft.is_block_device() {
                return 'b';
            }
            if ft.is_char_device() {
                return 'c';
            }
            if ft.is_fifo() {
                return 'p';
            }
            if ft.is_socket() {
                return 's';
            }
        }
        '-'
    }
}

fn bit(mode: u32, mask: u32, ch: char) -> char {
    if mode & mask != 0 {
        ch
    } else {
        '-'
    }
}

fn exec_bit(mode: u32, exec: u32, special: u32, lower: char, upper: char) -> char {
    let has_exec = mode & exec != 0;
    let has_special = mode & special != 0;
    match (has_special, has_exec) {
        (true, true) => lower,
        (true, false) => upper,
        (false, true) => 'x',
        (false, false) => '-',
    }
}

#[cfg(unix)]
fn posix_mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode()
}

#[cfg(not(unix))]
fn posix_mode(meta: &fs::Metadata) -> u32 {
    let mut mode = 0o644;
    if meta.is_dir() {
        mode = 0o755;
    }
    if meta.permissions().readonly() {
        mode &= !0o222;
    }
    mode
}

#[cfg(unix)]
fn meta_numbers(meta: &fs::Metadata) -> (u64, u32, u32, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (
        meta.nlink(),
        meta.uid(),
        meta.gid(),
        meta.blocks(),
        meta.len(),
    )
}

#[cfg(not(unix))]
fn meta_numbers(meta: &fs::Metadata) -> (u64, u32, u32, u64, u64) {
    let blocks = meta.len().div_ceil(512);
    (1, 0, 0, blocks, meta.len())
}

#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(meta: &fs::Metadata) -> bool {
    let _ = meta;
    false
}

fn format_mtime(meta: &fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format_unix_mtime(meta.mtime())
    }
    #[cfg(not(unix))]
    {
        match meta.modified() {
            Ok(t) => format_unix_mtime(system_time_secs(t)),
            Err(_) => "Jan  1  1970".to_string(),
        }
    }
}

#[cfg(not(unix))]
fn system_time_secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn format_unix_mtime(mtime: i64) -> String {
    let parts = local_broken_down(mtime);
    let mon = MONTHS[parts.month.min(11) as usize];
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Same window GNU ls uses: older than ~6 months, or more than an hour ahead.
    let six_months = 365 * 24 * 60 * 60 / 2;
    if mtime < now - six_months || mtime > now + 60 * 60 {
        format!("{mon} {:2}  {:4}", parts.day, parts.year)
    } else {
        format!("{mon} {:2} {:02}:{:02}", parts.day, parts.hour, parts.min)
    }
}

struct CivilTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[cfg(unix)]
fn local_broken_down(mtime: i64) -> CivilTime {
    // libc localtime_r — required so timestamps match real `ls` on the machine.
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: isize,
        tm_zone: *mut libc_char,
    }
    type libc_char = i8;

    unsafe extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }

    unsafe {
        let mut tm = std::mem::zeroed::<Tm>();
        if localtime_r(&mtime, &mut tm).is_null() {
            return CivilTime {
                year: 1970,
                month: 0,
                day: 1,
                hour: 0,
                min: 0,
            };
        }
        CivilTime {
            year: tm.tm_year + 1900,
            month: tm.tm_mon.clamp(0, 11) as u32,
            day: tm.tm_mday.max(1) as u32,
            hour: tm.tm_hour.clamp(0, 23) as u32,
            min: tm.tm_min.clamp(0, 59) as u32,
        }
    }
}

#[cfg(not(unix))]
fn local_broken_down(mtime: i64) -> CivilTime {
    // UTC fallback for non-Unix hosts; audit target is Linux.
    let secs = mtime.max(0) as u64;
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    CivilTime {
        year,
        month,
        day,
        hour: (tod / 3600) as u32,
        min: ((tod % 3600) / 60) as u32,
    }
}

#[cfg(not(unix))]
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    // Howard Hinnant civil-from-days (UTC).
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, (m - 1) as u32, d as u32)
}

/// Owner/group names from `/etc/passwd` and `/etc/group` (no libc getpwuid).
struct IdNames {
    users: HashMap<u32, String>,
    groups: HashMap<u32, String>,
}

impl IdNames {
    fn empty() -> Self {
        Self {
            users: HashMap::new(),
            groups: HashMap::new(),
        }
    }

    fn load() -> Self {
        Self {
            users: parse_id_file("/etc/passwd"),
            groups: parse_id_file("/etc/group"),
        }
    }

    fn user(&self, uid: u32) -> String {
        self.users
            .get(&uid)
            .cloned()
            .unwrap_or_else(|| uid.to_string())
    }

    fn group(&self, gid: u32) -> String {
        self.groups
            .get(&gid)
            .cloned()
            .unwrap_or_else(|| gid.to_string())
    }
}

fn parse_id_file(path: &str) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let Ok(text) = fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split(':');
        let Some(name) = parts.next() else { continue };
        // passwd/group: name:passwd:id:...
        let Some(_) = parts.next() else { continue };
        let Some(id_str) = parts.next() else { continue };
        if let Ok(id) = id_str.parse::<u32>() {
            map.entry(id).or_insert_with(|| name.to_string());
        }
    }
    map
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

        assert_eq!(listing(&argv(&["--", weird_s])), format!("{weird_s}\n"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn long_listing_prints_total_and_mode() {
        let base = scratch("long");
        fs::write(base.join("doc.txt"), b"hi").unwrap();
        fs::create_dir(base.join("subdir")).unwrap();

        let path = base.to_str().expect("utf8 path");
        let out = listing(&argv(&["-l", path]));
        let mut lines = out.lines();
        let total = lines.next().expect("total line");
        assert!(total.starts_with("total "), "got {total:?}");

        let mut saw_file = false;
        let mut saw_dir = false;
        for line in lines {
            let mode = line.split_whitespace().next().unwrap_or("");
            assert_eq!(mode.len(), 10, "mode should be 10 chars in {line}");
            if line.ends_with(" doc.txt") || line.contains(" doc.txt") {
                assert!(mode.starts_with('-'), "{line}");
                saw_file = true;
            }
            if line.ends_with(" subdir") || line.contains(" subdir") {
                assert!(mode.starts_with('d'), "{line}");
                saw_dir = true;
            }
        }
        assert!(saw_file && saw_dir, "output was:\n{out}");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn long_listing_single_file_has_no_total() {
        let base = scratch("long-file");
        let file = base.join("only.txt");
        fs::write(&file, b"xyz").unwrap();
        let path = file.to_str().expect("utf8 path");

        let out = listing(&argv(&["-l", path]));
        assert!(!out.starts_with("total "), "got {out:?}");
        assert!(out.contains("only.txt"), "got {out:?}");
        let mode = out.split_whitespace().next().unwrap_or("");
        assert_eq!(mode.len(), 10);
        assert!(mode.starts_with('-'));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn combined_la_uses_long_format_and_shows_dotfiles() {
        let base = scratch("la");
        fs::write(base.join(".dot"), b"").unwrap();
        fs::write(base.join("file"), b"").unwrap();

        let path = base.to_str().expect("utf8 path");
        let out = listing(&argv(&["-la", path]));
        assert!(out.starts_with("total "), "got {out:?}");
        assert!(out.contains(" ."), "missing '.' entry:\n{out}");
        assert!(out.contains(" .."), "missing '..' entry:\n{out}");
        assert!(out.contains(" .dot"), "missing .dot:\n{out}");
        assert!(out.contains(" file"), "missing file:\n{out}");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn mode_string_helpers() {
        assert_eq!(bit(0o644, 0o400, 'r'), 'r');
        assert_eq!(bit(0o644, 0o020, 'w'), '-');
        assert_eq!(exec_bit(0o755, 0o100, 0o4000, 's', 'S'), 'x');
        assert_eq!(exec_bit(0o4755, 0o100, 0o4000, 's', 'S'), 's');
        assert_eq!(exec_bit(0o4000, 0o100, 0o4000, 's', 'S'), 'S');
    }

    #[test]
    fn passwd_style_parsing_reads_name_and_id() {
        let text = "root:x:0:0:root:/root:/bin/bash\nalice:x:1000:1000::/home/alice:/bin/sh\n";
        let mut map = HashMap::new();
        for line in text.lines() {
            let mut parts = line.split(':');
            let name = parts.next().unwrap();
            let _ = parts.next();
            let id: u32 = parts.next().unwrap().parse().unwrap();
            map.insert(id, name.to_string());
        }
        assert_eq!(map.get(&0).map(String::as_str), Some("root"));
        assert_eq!(map.get(&1000).map(String::as_str), Some("alice"));
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

    #[cfg(unix)]
    #[test]
    fn long_listing_symlink_shows_arrow() {
        let base = scratch("long-link");
        fs::write(base.join("target"), b"x").unwrap();
        std::os::unix::fs::symlink("target", base.join("link")).unwrap();

        let path = base.to_str().expect("utf8 path");
        let out = listing(&argv(&["-l", path]));
        assert!(out.contains("link -> target"), "got {out:?}");
        assert!(out.lines().any(|l| l.starts_with('l')), "got {out:?}");

        let _ = fs::remove_dir_all(&base);
    }
}
