//! Command dispatch: route argv to a builtin handler.

use crate::commands;
use crate::error::ShellError;

pub fn dispatch(argv: &[String]) -> Result<(), ShellError> {
    let Some(name) = argv.first() else {
        return Ok(());
    };

    match name.as_str() {
        "echo" => commands::echo::run(&argv[1..]),
        "cd" => commands::cd::run(&argv[1..]),
        "pwd" => commands::pwd::run(&argv[1..]),
        "ls" => commands::ls::run(&argv[1..]),
        "cat" => commands::cat::run(&argv[1..]),
        "cp" => commands::cp::run(&argv[1..]),
        "rm" => commands::rm::run(&argv[1..]),
        "mv" => commands::mv::run(&argv[1..]),
        "mkdir" => commands::mkdir::run(&argv[1..]),
        "exit" => commands::exit::run(&argv[1..]),
        _ => Err(ShellError::NotFound(name.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn unknown_command_is_not_found() {
        let err = dispatch(&argv(&["something"])).unwrap_err();
        assert!(matches!(&err, ShellError::NotFound(name) if name == "something"));
        assert_eq!(err.to_string(), "Command 'something' not found");
    }

    #[test]
    fn empty_argv_is_ok() {
        assert!(dispatch(&[]).is_ok());
    }

    #[test]
    fn ls_dispatches() {
        assert!(dispatch(&argv(&["ls"])).is_ok());
    }

    #[test]
    fn echo_with_args_dispatches() {
        assert!(dispatch(&argv(&["echo", "hi"])).is_ok());
    }

    #[test]
    fn command_names_are_case_sensitive() {
        assert!(matches!(
            dispatch(&argv(&["LS"])),
            Err(ShellError::NotFound(_))
        ));
    }

    #[test]
    fn every_builtin_dispatches() {
        for name in [
            "echo", "cd", "pwd", "ls", "cat", "cp", "rm", "mv", "mkdir", "exit",
        ] {
            assert!(
                dispatch(&argv(&[name])).is_ok(),
                "expected {name} to dispatch to a builtin"
            );
        }
    }
}
