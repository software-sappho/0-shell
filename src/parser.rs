//! Tokenizer stub. Implemented in SH-003.

use crate::error::ShellError;

pub fn tokenize(_line: &str) -> Result<Vec<String>, ShellError> {
    Ok(vec![])
}
