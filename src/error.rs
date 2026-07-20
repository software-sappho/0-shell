use std::fmt;

#[derive(Debug)]
pub enum ShellError {
    NotFound(String),
    Io {
        cmd: &'static str,
        path: String,
        source: std::io::Error,
    },
    Usage(String),
    Parse(String),
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::NotFound(name) => write!(f, "Command '{name}' not found"),
            ShellError::Io { cmd, path, source } => {
                write!(
                    f,
                    "{cmd}: {path}: {}",
                    strip_os_error_suffix(&source.to_string())
                )
            }
            ShellError::Usage(msg) => write!(f, "{msg}"),
            ShellError::Parse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ShellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShellError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ShellError {
    fn from(source: std::io::Error) -> Self {
        ShellError::Io {
            cmd: "shell",
            path: String::new(),
            source,
        }
    }
}

// std::io::Error's Display appends " (os error N)" when it wraps a raw OS
// errno (which is how most fs:: errors arrive). Coreutils output never shows
// that suffix, so strip it to keep our error lines byte-comparable to bash.
fn strip_os_error_suffix(message: &str) -> &str {
    match message.rfind(" (os error ") {
        Some(idx) => &message[..idx],
        None => message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn not_found_display_matches_audit_format() {
        let err = ShellError::NotFound("something".to_string());
        assert_eq!(err.to_string(), "Command 'something' not found");
    }

    #[test]
    fn io_display_strips_os_error_suffix() {
        let source = io::Error::from_raw_os_error(2); // ENOENT
        let err = ShellError::Io {
            cmd: "cd",
            path: "/nope".to_string(),
            source,
        };
        let rendered = err.to_string();
        assert_eq!(rendered, "cd: /nope: No such file or directory");
        assert!(!rendered.contains("os error"));
    }

    #[test]
    fn usage_and_parse_display_the_message_verbatim() {
        assert_eq!(
            ShellError::Usage("usage: cd [dir]".to_string()).to_string(),
            "usage: cd [dir]"
        );
        assert_eq!(
            ShellError::Parse("unterminated quote: unmatched \"".to_string()).to_string(),
            "unterminated quote: unmatched \""
        );
    }

    #[test]
    fn implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<ShellError>();
    }
}
