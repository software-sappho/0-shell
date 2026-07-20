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
            ShellError::NotFound(name) => write!(f, "{name}: command not found"),
            ShellError::Io { cmd, path, source } => write!(f, "{cmd}: {path}: {source}"),
            ShellError::Usage(msg) => write!(f, "{msg}"),
            ShellError::Parse(msg) => write!(f, "{msg}"),
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
