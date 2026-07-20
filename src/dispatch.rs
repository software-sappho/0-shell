//! Command dispatch table stub. Implemented in SH-004.

use crate::error::ShellError;

pub fn dispatch(_argv: &[String]) -> Result<(), ShellError> {
    Ok(())
}
