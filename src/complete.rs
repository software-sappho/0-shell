//! Tab completion for builtin names and filesystem paths.

use std::fs;
use std::path::PathBuf;

use crate::dispatch::BUILTIN_NAMES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apply {
    /// Nothing to do (no matches).
    None,
    /// Replace the whole line (word extended / uniquely completed).
    Replace(String),
    /// Line unchanged at the common prefix; show these matches.
    List { line: String, matches: Vec<String> },
}

/// Attempt to complete the word at the end of `line`.
pub fn apply(line: &str) -> Apply {
    let (start, word) = word_to_complete(line);
    let matches = if is_command_position(line, start) && is_command_word(word) {
        builtin_matches(word)
    } else {
        path_matches(word)
    };

    if matches.is_empty() {
        return Apply::None;
    }

    let common = common_prefix(&matches);
    let completed = if matches.len() == 1 {
        matches[0].clone()
    } else {
        common.clone()
    };

    let mut new_line = String::with_capacity(start + completed.len());
    new_line.push_str(&line[..start]);
    new_line.push_str(&completed);

    if matches.len() > 1 && completed == word {
        Apply::List {
            line: new_line,
            matches,
        }
    } else {
        Apply::Replace(new_line)
    }
}

fn word_to_complete(line: &str) -> (usize, &str) {
    let mut start = 0usize;
    for (i, ch) in line.char_indices() {
        if ch == ' ' || ch == '\t' || ch == ';' {
            start = i + ch.len_utf8();
        }
    }
    // After `;` there may be spaces before the next command word.
    let rest = &line[start..];
    let skip = rest.len() - rest.trim_start().len();
    start += skip;
    (start, &line[start..])
}

/// First word of a `;`-segment (only whitespace between the prior `;` and it).
fn is_command_position(line: &str, word_start: usize) -> bool {
    let before = &line[..word_start];
    match before.rfind(';') {
        Some(idx) => before[idx + 1..].chars().all(|c| c.is_whitespace()),
        None => before.chars().all(|c| c.is_whitespace()),
    }
}

/// Builtin-name completion (not a path-looking token).
fn is_command_word(word: &str) -> bool {
    !word.contains('/') && !word.contains('\\') && !word.starts_with('.') && !word.starts_with('~')
}

fn builtin_matches(prefix: &str) -> Vec<String> {
    let mut out: Vec<String> = BUILTIN_NAMES
        .iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| format!("{name} "))
        .collect();
    out.sort();
    out
}

fn path_matches(partial: &str) -> Vec<String> {
    let (dir_display, file_prefix) = split_dir_and_file(partial);
    let dir_fs = expand_tilde_for_fs(&dir_display);

    let read = match fs::read_dir(&dir_fs) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let show_hidden = file_prefix.starts_with('.');
    let mut out = Vec::new();

    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if !name.starts_with(file_prefix) {
            continue;
        }

        let mut rendered = String::new();
        rendered.push_str(&dir_display);
        rendered.push_str(&name);

        let is_dir = entry
            .file_type()
            .map(|ft| ft.is_dir())
            .unwrap_or_else(|_| entry.path().is_dir());
        if is_dir {
            rendered.push('/');
        } else {
            rendered.push(' ');
        }
        out.push(rendered);
    }

    out.sort();
    out
}

/// Split `partial` into the directory prefix the user typed and the file prefix.
///
/// Examples: `"src/ma"` → `("src/", "ma")`, `"ma"` → `("", "ma")`,
/// `"~/Do"` → `("~/", "Do")`, `"src/"` → `("src/", "")`.
fn split_dir_and_file(partial: &str) -> (String, &str) {
    let sep = partial
        .char_indices()
        .rev()
        .find(|(_, ch)| *ch == '/' || *ch == '\\')
        .map(|(i, _)| i);
    match sep {
        Some(idx) => {
            let dir = partial[..=idx].to_string();
            let file = &partial[idx + 1..];
            (dir, file)
        }
        None => (String::new(), partial),
    }
}

fn expand_tilde_for_fs(dir_display: &str) -> PathBuf {
    if dir_display.is_empty() {
        return PathBuf::from(".");
    }
    if dir_display == "~/" || dir_display == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = dir_display.strip_prefix("~/") {
        let mut home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        if !rest.is_empty() {
            home.push(rest);
        }
        return home;
    }
    PathBuf::from(dir_display)
}

fn home_dir() -> Option<PathBuf> {
    env_home().map(PathBuf::from)
}

fn env_home() -> Option<std::ffi::OsString> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
}

fn common_prefix(items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut prefix = items[0].as_str();
    for item in &items[1..] {
        prefix = shared_prefix(prefix, item);
        if prefix.is_empty() {
            break;
        }
    }
    // Don't leave a trailing space/slash from a partial unique finalize in the
    // common-prefix path — those suffixes only apply to unique matches.
    // For multi-match, strip a trailing space that isn't shared as path content.
    let mut out = prefix.to_string();
    if items.len() > 1 {
        while out.ends_with(' ') {
            out.pop();
        }
    }
    out
}

fn shared_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let mut end = 0;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca != cb {
            break;
        }
        end += ca.len_utf8();
    }
    &a[..end]
}

/// Normalize a path for tests / display helpers (strip `.` components).
#[cfg(test)]
fn simplify(path: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(label: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("0-shell-complete-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn completes_unique_builtin_with_trailing_space() {
        assert_eq!(apply("ec"), Apply::Replace("echo ".into()));
        assert_eq!(apply("  pw"), Apply::Replace("  pwd ".into()));
    }

    #[test]
    fn completes_common_builtin_prefix() {
        // cat / cd / cp share "c"
        match apply("c") {
            Apply::Replace(s) => assert_eq!(s, "c"),
            Apply::List { matches, .. } => {
                assert!(matches.iter().any(|m| m.starts_with("cat")));
                assert!(matches.iter().any(|m| m.starts_with("cd")));
            }
            Apply::None => panic!("expected matches for c"),
        }
    }

    #[test]
    fn lists_when_already_at_common_prefix() {
        // "e" → echo / exit; common prefix "e"
        match apply("e") {
            Apply::List { matches, line } => {
                assert_eq!(line, "e");
                assert!(matches.iter().any(|m| m == "echo "));
                assert!(matches.iter().any(|m| m == "exit "));
            }
            Apply::Replace(s) => {
                // If common prefix grew past "e" that would be fine too.
                assert!(s.starts_with('e'));
            }
            Apply::None => panic!("expected matches"),
        }
    }

    #[test]
    fn command_position_after_semicolon() {
        assert_eq!(apply("pwd; ec"), Apply::Replace("pwd; echo ".into()));
    }

    #[test]
    fn path_argument_not_treated_as_builtin() {
        // "echo e…" should complete a file, not the `echo` builtin.
        let base = scratch("arg");
        fs::write(base.join("example.txt"), b"").unwrap();
        let base_s = unixish(&base);

        match apply(&format!("echo {base_s}/e")) {
            Apply::Replace(s) => assert!(s.contains("example"), "got {s}"),
            Apply::List { matches, .. } => {
                assert!(matches.iter().any(|m| m.contains("example")));
            }
            Apply::None => panic!("expected path match"),
        }

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn unique_directory_gets_trailing_slash() {
        let base = scratch("dir");
        fs::create_dir(base.join("srcdir")).unwrap();
        let base_s = unixish(&base);

        assert_eq!(
            apply(&format!("ls {base_s}/src")),
            Apply::Replace(format!("ls {base_s}/srcdir/"))
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn unique_file_gets_trailing_space() {
        let base = scratch("file");
        fs::write(base.join("readme.md"), b"").unwrap();
        let base_s = unixish(&base);

        assert_eq!(
            apply(&format!("cat {base_s}/read")),
            Apply::Replace(format!("cat {base_s}/readme.md "))
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn nested_path_prefix() {
        let base = scratch("nest");
        fs::create_dir_all(base.join("a").join("b")).unwrap();
        fs::write(base.join("a").join("bee.txt"), b"").unwrap();
        let base_s = unixish(&base);

        match apply(&format!("cat {base_s}/a/b")) {
            Apply::Replace(s) | Apply::List { line: s, .. } => {
                assert!(s.contains("/a/b"), "got {s}");
            }
            Apply::None => panic!("expected nested matches"),
        }

        let _ = fs::remove_dir_all(&base);
    }

    fn unixish(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn no_match_returns_none() {
        assert_eq!(apply("zzznope"), Apply::None);
    }

    #[test]
    fn word_boundaries_ignore_earlier_tokens() {
        let (start, word) = word_to_complete("echo hello");
        assert_eq!(&"echo hello"[start..], "hello");
        assert_eq!(word, "hello");
    }

    #[test]
    fn simplify_smoke() {
        assert_eq!(
            simplify(std::path::Path::new("./foo")),
            PathBuf::from("foo")
        );
    }
}
