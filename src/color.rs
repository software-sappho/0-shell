//! ANSI color helpers for TTY output.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Dir,
    Exec,
    Symlink,
    File,
}

const RESET: &str = "\x1b[0m";
const BOLD_BLUE: &str = "\x1b[1;34m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const RED: &str = "\x1b[31m";

/// Wrap a file name in the usual `ls --color` style codes.
pub fn paint_name(name: &str, kind: NameKind, enabled: bool) -> String {
    if !enabled {
        return name.to_string();
    }
    match kind {
        NameKind::Dir => format!("{BOLD_BLUE}{name}{RESET}"),
        NameKind::Exec => format!("{BOLD_GREEN}{name}{RESET}"),
        NameKind::Symlink => format!("{BOLD_CYAN}{name}{RESET}"),
        NameKind::File => name.to_string(),
    }
}

/// Red error text when stderr is a TTY.
pub fn paint_error(message: &str, enabled: bool) -> String {
    if enabled {
        format!("{RED}{message}{RESET}")
    } else {
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_returns_plain_text() {
        assert_eq!(paint_name("dir", NameKind::Dir, false), "dir");
        assert_eq!(paint_error("boom", false), "boom");
    }

    #[test]
    fn enabled_wraps_with_ansi() {
        let dir = paint_name("d", NameKind::Dir, true);
        assert!(dir.contains("d"));
        assert!(dir.starts_with("\x1b["));
        assert!(dir.ends_with(RESET));

        let err = paint_error("nope", true);
        assert!(err.contains("nope"));
        assert!(err.contains(RED));
    }
}
