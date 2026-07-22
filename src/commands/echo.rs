//! `echo` builtin: join args with a single space and a trailing newline.

use std::io::{self, Write};

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let line = format_line(args);

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle
        .write_all(line.as_bytes())
        .map_err(|source| ShellError::Io {
            cmd: "echo",
            path: String::new(),
            source,
        })
}

// No `-n`/`-e` flag handling: real bash's echo builtin flag behavior is
// shell-dependent and the audit never exercises it, so any leading "-n" here
// is just an ordinary argument to print, not a flag.
fn format_line(args: &[String]) -> String {
    let mut line = args.join(" ");
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn single_arg() {
        assert_eq!(format_line(&argv(&["something!"])), "something!\n");
    }

    #[test]
    fn two_args_joined_with_one_space() {
        assert_eq!(
            format_line(&argv(&["something", "else"])),
            "something else\n"
        );
    }

    #[test]
    fn no_args_is_just_a_newline() {
        assert_eq!(format_line(&[]), "\n");
    }

    #[test]
    fn multiple_args() {
        assert_eq!(format_line(&argv(&["a", "b", "c"])), "a b c\n");
    }

    #[test]
    fn leading_dash_n_is_not_a_flag() {
        assert_eq!(format_line(&argv(&["-n"])), "-n\n");
    }
}
