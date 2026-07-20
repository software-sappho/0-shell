//! `cp` builtin stub.

use crate::error::ShellError;

pub fn run(_args: &[String]) -> Result<(), ShellError> {
    Ok(())
}
