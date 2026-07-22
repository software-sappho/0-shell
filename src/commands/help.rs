//! `help` builtin: list builtins, or show detail for one command.

use std::io::{self, Write};

use crate::error::ShellError;

#[derive(Debug, Clone, Copy)]
struct BuiltinHelp {
    name: &'static str,
    usage: &'static str,
    summary: &'static str,
    detail: &'static str,
}

const BUILTINS: &[BuiltinHelp] = &[
    BuiltinHelp {
        name: "cat",
        usage: "cat [FILE...]",
        summary: "Concatenate files to standard output",
        detail: "\
Stream each FILE's bytes to stdout unmodified. Multiple operands are
concatenated in order. With no FILE, copy stdin to stdout.
Directories are refused with 'Is a directory'.",
    },
    BuiltinHelp {
        name: "cd",
        usage: "cd [DIR]",
        summary: "Change the current working directory",
        detail: "\
With no DIR, change to $HOME. Supports ~ and ~/... expansion.
Relative and absolute paths are accepted.",
    },
    BuiltinHelp {
        name: "cp",
        usage: "cp SOURCE DEST",
        summary: "Copy a file",
        detail: "\
Copy SOURCE to DEST. If DEST is an existing directory, copy into it
preserving the basename. Directory sources require -r (not implemented).",
    },
    BuiltinHelp {
        name: "echo",
        usage: "echo [ARG...]",
        summary: "Write arguments to standard output",
        detail: "\
Join ARGs with a single space and append a trailing newline.
Leading dashes are ordinary text (no -n / -e flags).",
    },
    BuiltinHelp {
        name: "exit",
        usage: "exit [N]",
        summary: "Exit the shell",
        detail: "\
Terminate the shell. Optional N is an exit status from 0 to 255.
With no N, exit with status 0.",
    },
    BuiltinHelp {
        name: "help",
        usage: "help [COMMAND]",
        summary: "Display help about builtins",
        detail: "\
With no COMMAND, list every builtin with a one-line summary.
With COMMAND, print usage and a short description for that builtin.",
    },
    BuiltinHelp {
        name: "ls",
        usage: "ls [-aFl] [FILE...]",
        summary: "List directory contents",
        detail: "\
List FILEs (default: current directory), one name per line, sorted.
Dotfiles are hidden unless -a is given.

Flags:
  -a  include ., .., and other dotfiles
  -F  append / (dir), * (executable), @ (symlink)
  -l  long format (mode, owner, size, mtime, …)
  Combined forms such as -la are accepted.
  -- ends option parsing.

On a TTY, directories are blue, executables green, and symlinks cyan.",
    },
    BuiltinHelp {
        name: "mkdir",
        usage: "mkdir DIRECTORY...",
        summary: "Create directories",
        detail: "\
Create each DIRECTORY. Parent directories are not created.
Multi-component relative paths are truncated at the first /.",
    },
    BuiltinHelp {
        name: "mv",
        usage: "mv SOURCE DEST",
        summary: "Move or rename a file or directory",
        detail: "\
Rename SOURCE to DEST, or move into DEST if DEST is a directory.
Cross-device moves fall back to copy then delete.",
    },
    BuiltinHelp {
        name: "pwd",
        usage: "pwd",
        summary: "Print the current working directory",
        detail: "\
Write the absolute path of the current working directory to stdout.",
    },
    BuiltinHelp {
        name: "rm",
        usage: "rm [-rR] FILE...",
        summary: "Remove files or directories",
        detail: "\
Remove each FILE. Directories require -r or -R (depth-first recursive).
Without -r, removing a directory is an error.",
    },
];

pub fn run(args: &[String]) -> Result<(), ShellError> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    run_with_writer(args, &mut out)
}

fn run_with_writer(args: &[String], out: &mut dyn Write) -> Result<(), ShellError> {
    let text = match args.first().map(String::as_str) {
        None => summary_text(),
        Some(name) => match find(name) {
            Some(entry) => detail_text(entry),
            None => {
                return Err(ShellError::Usage(format!(
                    "help: no help topics match `{name}'. Try `help help'."
                )));
            }
        },
    };

    out.write_all(text.as_bytes())
        .map_err(|source| ShellError::Io {
            cmd: "help",
            path: String::new(),
            source,
        })
}

fn find(name: &str) -> Option<&'static BuiltinHelp> {
    BUILTINS.iter().find(|b| b.name == name)
}

fn summary_text() -> String {
    let name_w = BUILTINS.iter().map(|b| b.usage.len()).max().unwrap_or(1);

    let mut out = String::from("0-shell builtins:\n");
    for b in BUILTINS {
        out.push_str(&format!(
            "  {usage:<name_w$}  {summary}\n",
            usage = b.usage,
            summary = b.summary,
            name_w = name_w,
        ));
    }
    out.push_str("\nType `help name' for more information about each builtin.\n");
    out.push_str("Commands on one line may be chained with `;` (failures do not stop the rest).\n");
    out
}

fn detail_text(entry: &BuiltinHelp) -> String {
    let mut out = format!("{}: {}\n", entry.name, entry.usage);
    for line in entry.detail.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    fn help_out(args: &[String]) -> String {
        let mut out = Vec::new();
        run_with_writer(args, &mut out).expect("help should succeed");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn bare_help_lists_every_builtin() {
        let text = help_out(&[]);
        assert!(text.starts_with("0-shell builtins:\n"));
        for name in [
            "cat", "cd", "cp", "echo", "exit", "help", "ls", "mkdir", "mv", "pwd", "rm",
        ] {
            assert!(text.contains(name), "summary should mention {name}: {text}");
        }
        assert!(text.contains("Type `help name'"));
        assert!(text.contains("chained with `;`"));
    }

    #[test]
    fn help_ls_shows_flags() {
        let text = help_out(&argv(&["ls"]));
        assert!(text.starts_with("ls: ls [-aFl]"));
        assert!(text.contains("-a"));
        assert!(text.contains("-F"));
        assert!(text.contains("-l"));
    }

    #[test]
    fn help_help_documents_itself() {
        let text = help_out(&argv(&["help"]));
        assert!(text.starts_with("help: help [COMMAND]"));
        assert!(text.contains("one-line summary"));
    }

    #[test]
    fn unknown_topic_is_a_usage_error() {
        let mut out = Vec::new();
        let err = run_with_writer(&argv(&["nope"]), &mut out).unwrap_err();
        assert_eq!(
            err.to_string(),
            "help: no help topics match `nope'. Try `help help'."
        );
        assert!(out.is_empty());
    }

    #[test]
    fn builtins_table_is_sorted_by_name() {
        let names: Vec<&str> = BUILTINS.iter().map(|b| b.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }
}
