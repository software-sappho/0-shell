//! Tokenizer: split a line into argv, honoring quotes and `$VAR` expansion.
//!
//! Quotes are token-joining, not token-delimiting: a quoted run adjacent to
//! unquoted text stays part of the same token (matching bash), and closing a
//! quote never forces a token boundary on its own.
//!
//! `$VAR` / `${VAR}` / `$?` expand outside single quotes (and inside double
//! quotes). Unknown names expand to the empty string.

use std::env;

use crate::error::ShellError;

pub fn tokenize(line: &str) -> Result<Vec<String>, ShellError> {
    tokenize_with_status(line, 0)
}

pub fn tokenize_with_status(line: &str, last_status: i32) -> Result<Vec<String>, ShellError> {
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut current: Option<String> = None;
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        if in_single {
            if c == '\'' {
                in_single = false;
                i += 1;
            } else {
                current.get_or_insert_with(String::new).push(c);
                i += 1;
            }
        } else if c == '$' && (in_double || !in_single) {
            // Expand in unquoted text and inside double quotes.
            let buf = current.get_or_insert_with(String::new);
            i = expand_dollar(&chars, i, last_status, buf);
        } else if in_double {
            if c == '"' {
                in_double = false;
                i += 1;
            } else {
                current.get_or_insert_with(String::new).push(c);
                i += 1;
            }
        } else if c == '\'' {
            in_single = true;
            current.get_or_insert_with(String::new);
            i += 1;
        } else if c == '"' {
            in_double = true;
            current.get_or_insert_with(String::new);
            i += 1;
        } else if c == ' ' || c == '\t' {
            if let Some(token) = current.take() {
                tokens.push(token);
            }
            i += 1;
        } else {
            current.get_or_insert_with(String::new).push(c);
            i += 1;
        }
    }

    if in_double {
        return Err(ShellError::Parse(
            "unterminated quote: unmatched \"".to_string(),
        ));
    }
    if in_single {
        return Err(ShellError::Parse(
            "unterminated quote: unmatched '".to_string(),
        ));
    }

    if let Some(token) = current.take() {
        tokens.push(token);
    }

    Ok(tokens)
}

/// Expand starting at `chars[i] == '$'`. Returns the next index to read.
fn expand_dollar(chars: &[char], i: usize, last_status: i32, out: &mut String) -> usize {
    debug_assert_eq!(chars.get(i), Some(&'$'));
    let next = i + 1;
    if next >= chars.len() {
        out.push('$');
        return next;
    }

    match chars[next] {
        '?' => {
            out.push_str(&last_status.to_string());
            next + 1
        }
        '{' => {
            let mut j = next + 1;
            while j < chars.len() && chars[j] != '}' {
                j += 1;
            }
            if j >= chars.len() {
                // Unclosed `${` — treat `$` literally and continue.
                out.push('$');
                return next;
            }
            let name: String = chars[next + 1..j].iter().collect();
            out.push_str(&lookup_var(&name, last_status));
            j + 1
        }
        c if is_var_start(c) => {
            let mut j = next + 1;
            while j < chars.len() && is_var_cont(chars[j]) {
                j += 1;
            }
            let name: String = chars[next..j].iter().collect();
            out.push_str(&lookup_var(&name, last_status));
            j
        }
        _ => {
            out.push('$');
            next
        }
    }
}

fn is_var_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_var_cont(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn lookup_var(name: &str, last_status: i32) -> String {
    if name == "?" {
        return last_status.to_string();
    }
    env::var(name).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(line: &str) -> Vec<String> {
        tokenize(line).expect("expected successful tokenize")
    }

    #[test]
    fn double_quoted_group_is_one_token() {
        assert_eq!(tok(r#"echo "Hello There""#), vec!["echo", "Hello There"]);
    }

    #[test]
    fn unquoted_whitespace_splits_tokens() {
        assert_eq!(
            tok("echo something else"),
            vec!["echo", "something", "else"]
        );
    }

    #[test]
    fn double_quotes_are_stripped_from_output() {
        assert_eq!(tok(r#"echo "something!""#), vec!["echo", "something!"]);
    }

    #[test]
    fn single_quoted_group_is_one_token() {
        assert_eq!(tok("echo 'single quoted'"), vec!["echo", "single quoted"]);
    }

    #[test]
    fn quotes_join_adjacent_unquoted_text() {
        assert_eq!(tok(r#"ab"cd ef"gh"#), vec!["abcd efgh"]);
    }

    #[test]
    fn explicit_empty_quoted_string_is_a_real_token() {
        assert_eq!(tok(r#"echo """#), vec!["echo", ""]);
    }

    #[test]
    fn lone_empty_quoted_string_is_one_empty_token() {
        assert_eq!(tok("\"\""), vec![""]);
    }

    #[test]
    fn empty_line_yields_no_tokens() {
        assert_eq!(tok(""), Vec::<String>::new());
    }

    #[test]
    fn whitespace_only_line_yields_no_tokens() {
        assert_eq!(tok("   \t  "), Vec::<String>::new());
    }

    #[test]
    fn unterminated_double_quote_is_a_parse_error() {
        assert!(matches!(
            tokenize(r#"echo "unterminated"#),
            Err(ShellError::Parse(_))
        ));
    }

    #[test]
    fn unterminated_single_quote_is_a_parse_error() {
        assert!(matches!(
            tokenize("echo 'unterminated"),
            Err(ShellError::Parse(_))
        ));
    }

    #[test]
    fn single_quote_is_literal_inside_double_quotes() {
        assert_eq!(tok(r#"echo "it's fine""#), vec!["echo", "it's fine"]);
    }

    #[test]
    fn adjacent_quoted_runs_of_different_kinds_join() {
        assert_eq!(tok(r#"echo "a"'b'c"#), vec!["echo", "abc"]);
    }

    #[test]
    fn expands_dollar_var() {
        env::set_var("ZERO_SHELL_EXPAND_TEST", "/home/tester");
        assert_eq!(
            tokenize_with_status("echo $ZERO_SHELL_EXPAND_TEST", 0).unwrap(),
            vec!["echo", "/home/tester"]
        );
        env::remove_var("ZERO_SHELL_EXPAND_TEST");
    }

    #[test]
    fn expands_braced_var() {
        env::set_var("ZERO_SHELL_EXPAND_TEST", "/bin:/usr/bin");
        assert_eq!(
            tokenize_with_status("echo ${ZERO_SHELL_EXPAND_TEST}", 0).unwrap(),
            vec!["echo", "/bin:/usr/bin"]
        );
        env::remove_var("ZERO_SHELL_EXPAND_TEST");
    }

    #[test]
    fn expands_inside_double_quotes() {
        env::set_var("ZERO_SHELL_EXPAND_TEST", "/home/tester");
        assert_eq!(
            tokenize_with_status(r#"echo "dir=$ZERO_SHELL_EXPAND_TEST""#, 0).unwrap(),
            vec!["echo", "dir=/home/tester"]
        );
        env::remove_var("ZERO_SHELL_EXPAND_TEST");
    }

    #[test]
    fn no_expand_inside_single_quotes() {
        env::set_var("ZERO_SHELL_EXPAND_TEST", "/home/tester");
        assert_eq!(
            tokenize_with_status("echo '$ZERO_SHELL_EXPAND_TEST'", 0).unwrap(),
            vec!["echo", "$ZERO_SHELL_EXPAND_TEST"]
        );
        env::remove_var("ZERO_SHELL_EXPAND_TEST");
    }

    #[test]
    fn question_mark_is_last_status() {
        assert_eq!(
            tokenize_with_status("echo $?", 42).unwrap(),
            vec!["echo", "42"]
        );
        assert_eq!(
            tokenize_with_status("echo ${?}", 7).unwrap(),
            vec!["echo", "7"]
        );
    }

    #[test]
    fn unknown_var_expands_to_empty() {
        env::remove_var("ZERO_SHELL_NO_SUCH_VAR");
        assert_eq!(
            tokenize_with_status("echo[$ZERO_SHELL_NO_SUCH_VAR]", 0).unwrap(),
            vec!["echo[]"]
        );
    }

    #[test]
    fn lone_dollar_stays_literal() {
        assert_eq!(tok("echo $"), vec!["echo", "$"]);
        assert_eq!(tok("echo $."), vec!["echo", "$."]);
    }
}
