//! Command dispatch: route argv to a builtin handler.

use crate::commands;
use crate::error::ShellError;

/// What the REPL loop should do after a dispatched command runs.
///
/// `exit` is the only builtin that can request loop termination, and it
/// needs to carry a numeric code out — a plain `Result<(), ShellError>`
/// can't express that, so dispatch() returns this instead. Only the REPL
/// loop acts on `Exit` by calling `std::process::exit`; dispatch() itself
/// never exits the process, which keeps it testable.
#[derive(Debug)]
pub enum ControlFlow {
    Exit(i32),
    Continue(Result<(), ShellError>),
}

pub fn dispatch(argv: &[String]) -> ControlFlow {
    let Some(name) = argv.first() else {
        return ControlFlow::Continue(Ok(()));
    };

    match name.as_str() {
        "echo" => ControlFlow::Continue(commands::echo::run(&argv[1..])),
        "cd" => ControlFlow::Continue(commands::cd::run(&argv[1..])),
        "pwd" => ControlFlow::Continue(commands::pwd::run(&argv[1..])),
        "ls" => ControlFlow::Continue(commands::ls::run(&argv[1..])),
        "cat" => ControlFlow::Continue(commands::cat::run(&argv[1..])),
        "cp" => ControlFlow::Continue(commands::cp::run(&argv[1..])),
        "rm" => ControlFlow::Continue(commands::rm::run(&argv[1..])),
        "mv" => ControlFlow::Continue(commands::mv::run(&argv[1..])),
        "mkdir" => ControlFlow::Continue(commands::mkdir::run(&argv[1..])),
        "exit" => match commands::exit::parse_code(&argv[1..]) {
            Ok(code) => ControlFlow::Exit(code),
            Err(err) => ControlFlow::Continue(Err(err)),
        },
        _ => ControlFlow::Continue(Err(ShellError::NotFound(name.clone()))),
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
        let ControlFlow::Continue(Err(err)) = dispatch(&argv(&["something"])) else {
            panic!("expected ControlFlow::Continue(Err(NotFound))");
        };
        assert!(matches!(&err, ShellError::NotFound(name) if name == "something"));
        assert_eq!(err.to_string(), "Command 'something' not found");
    }

    #[test]
    fn empty_argv_is_ok() {
        assert!(matches!(dispatch(&[]), ControlFlow::Continue(Ok(()))));
    }

    #[test]
    fn ls_dispatches() {
        assert!(matches!(
            dispatch(&argv(&["ls"])),
            ControlFlow::Continue(Ok(()))
        ));
    }

    #[test]
    fn echo_with_args_dispatches() {
        assert!(matches!(
            dispatch(&argv(&["echo", "hi"])),
            ControlFlow::Continue(Ok(()))
        ));
    }

    #[test]
    fn command_names_are_case_sensitive() {
        assert!(matches!(
            dispatch(&argv(&["LS"])),
            ControlFlow::Continue(Err(ShellError::NotFound(_)))
        ));
    }

    // `cd` is deliberately excluded from this loop: unlike the other
    // builtins, a real `cd` mutates the whole process's working directory,
    // which is global state shared by every concurrently-running test
    // thread. See `cd_dispatches_and_restores_cwd` below for a dispatch-level
    // test of `cd` that saves and restores the directory safely.
    #[test]
    fn every_non_exit_builtin_dispatches() {
        for name in ["echo", "pwd", "ls", "cat", "cp", "rm", "mv", "mkdir"] {
            assert!(
                matches!(dispatch(&argv(&[name])), ControlFlow::Continue(Ok(()))),
                "expected {name} to dispatch to a builtin"
            );
        }
    }

    #[test]
    fn cd_dispatches_and_restores_cwd() {
        struct RestoreCwd(std::path::PathBuf);
        impl Drop for RestoreCwd {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }

        let original = std::env::current_dir().expect("cwd must be readable in tests");
        let _restore = RestoreCwd(original);

        let target = std::env::temp_dir();
        let target = target.to_str().expect("temp dir must be valid UTF-8");
        assert!(matches!(
            dispatch(&argv(&["cd", target])),
            ControlFlow::Continue(Ok(()))
        ));
    }

    // "A full exit run": exercise dispatch() end to end for `exit`, the same
    // way the REPL loop would, and confirm it produces the ControlFlow::Exit
    // the loop needs to break correctly — without ever calling
    // std::process::exit, so the test process keeps running.
    #[test]
    fn bare_exit_requests_exit_code_zero() {
        assert!(matches!(dispatch(&argv(&["exit"])), ControlFlow::Exit(0)));
    }

    #[test]
    fn exit_with_numeric_arg_requests_that_code() {
        assert!(matches!(
            dispatch(&argv(&["exit", "44"])),
            ControlFlow::Exit(44)
        ));
    }

    #[test]
    fn exit_with_bad_arg_does_not_request_exit() {
        assert!(matches!(
            dispatch(&argv(&["exit", "hello"])),
            ControlFlow::Continue(Err(ShellError::Usage(_)))
        ));
    }

    #[test]
    fn exit_with_too_many_args_does_not_request_exit() {
        assert!(matches!(
            dispatch(&argv(&["exit", "1", "2", "3"])),
            ControlFlow::Continue(Err(ShellError::Usage(_)))
        ));
    }
}
