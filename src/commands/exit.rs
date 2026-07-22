//! `exit` builtin: validate the exit code argument.
//!
//! `exit` can't actually terminate the process through this file's `run`
//! function alone — its signature is `Result<(), ShellError>`, which has no
//! way to carry a numeric exit code out. Instead, `parse_code` below computes
//! the validated code and dispatch() turns that into `ControlFlow::Exit`,
//! which the REPL loop (the only place allowed to call `std::process::exit`)
//! acts on. That keeps the exit path testable: a test can call `parse_code`
//! or `dispatch()` and inspect the result without killing the test process.

use crate::error::ShellError;

pub fn run(args: &[String]) -> Result<(), ShellError> {
    parse_code(args).map(|_code| ())
}

/// Parse `exit`'s arguments into a process exit code (0..=255, matching
/// dash/bash's own numeric-argument range).
pub fn parse_code(args: &[String]) -> Result<i32, ShellError> {
    match args {
        [] => Ok(0),
        [only] => only
            .parse::<u8>()
            .map(i32::from)
            .map_err(|_| ShellError::Usage(format!("exit: {only}: numeric argument required"))),
        _ => Err(ShellError::Usage("exit: too many arguments".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn bare_exit_is_code_zero() {
        assert_eq!(parse_code(&[]).unwrap(), 0);
    }

    #[test]
    fn numeric_arg_is_the_code() {
        assert_eq!(parse_code(&argv(&["44"])).unwrap(), 44);
    }

    #[test]
    fn boundary_values_are_accepted() {
        assert_eq!(parse_code(&argv(&["0"])).unwrap(), 0);
        assert_eq!(parse_code(&argv(&["255"])).unwrap(), 255);
    }

    #[test]
    fn out_of_range_is_a_usage_error() {
        let err = parse_code(&argv(&["123456"])).unwrap_err();
        assert_eq!(err.to_string(), "exit: 123456: numeric argument required");
    }

    #[test]
    fn non_numeric_is_a_usage_error() {
        let err = parse_code(&argv(&["hello"])).unwrap_err();
        assert_eq!(err.to_string(), "exit: hello: numeric argument required");
    }

    #[test]
    fn multiple_args_is_a_usage_error() {
        let err = parse_code(&argv(&["1", "2", "3"])).unwrap_err();
        assert_eq!(err.to_string(), "exit: too many arguments");
    }

    #[test]
    fn run_validates_without_exiting() {
        assert!(run(&[]).is_ok());
        assert!(run(&argv(&["44"])).is_ok());
        assert!(run(&argv(&["hello"])).is_err());
    }
}
