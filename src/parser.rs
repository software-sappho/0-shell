//! Tokenizer: split a line into argv, honoring double and single quotes.
//!
//! Quotes are token-joining, not token-delimiting: a quoted run adjacent to
//! unquoted text stays part of the same token (matching bash), and closing a
//! quote never forces a token boundary on its own.

use crate::error::ShellError;

pub fn tokenize(line: &str) -> Result<Vec<String>, ShellError> {
    let mut tokens = Vec::new();
    let mut current: Option<String> = None;
    let mut in_single = false;
    let mut in_double = false;

    for c in line.chars() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                current.get_or_insert_with(String::new).push(c);
            }
        } else if in_double {
            if c == '"' {
                in_double = false;
            } else {
                current.get_or_insert_with(String::new).push(c);
            }
        } else if c == '\'' {
            in_single = true;
            current.get_or_insert_with(String::new);
        } else if c == '"' {
            in_double = true;
            current.get_or_insert_with(String::new);
        } else if c == ' ' || c == '\t' {
            if let Some(token) = current.take() {
                tokens.push(token);
            }
        } else {
            current.get_or_insert_with(String::new).push(c);
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
}
