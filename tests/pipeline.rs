//! Integration tests for SH-026 piping.
//!
//! These spawn the `0-shell` binary in a subprocess so we never `fork` inside
//! the multi-threaded unit-test harness (which can deadlock on stdio locks).

use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
    // Prefer Cargo's path to the built binary when available.
    let path = option_env!("CARGO_BIN_EXE_0-shell")
        .or(option_env!("CARGO_BIN_EXE_0_shell"))
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}/target/debug/0-shell", env!("CARGO_MANIFEST_DIR")));
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn run_script(script: &str) -> (String, String) {
    let mut child = bin().spawn().expect("spawn 0-shell");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(script.as_bytes()).expect("write script");
    }
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn echo_piped_to_cat_matches_bash() {
    let (ours, _) = run_script("echo hello | cat\nexit\n");
    assert_eq!(ours, "hello\n");
}

#[test]
fn multi_stage_pipeline() {
    let (ours, _) = run_script("echo a b | cat | cat\nexit\n");
    assert_eq!(ours, "a b\n");
}

#[test]
fn pipe_inside_quotes_is_literal() {
    let (ours, _) = run_script("echo \"x|y\" | cat\nexit\n");
    assert_eq!(ours, "x|y\n");
}

#[test]
fn pipeline_status_is_last_stage() {
    let (ours, _) = run_script("echo hi | nosuchbuiltin99\necho $?\nexit\n");
    assert_eq!(ours, "127\n");
}

#[test]
fn exit_in_pipeline_does_not_kill_shell() {
    let (ours, _) = run_script("echo hi | exit 7\necho $?\nexit\n");
    assert_eq!(ours, "7\n");
}

#[test]
fn pipeline_then_semicolon() {
    let (ours, _) = run_script("echo a | cat; echo after\nexit\n");
    assert_eq!(ours, "a\nafter\n");
}

#[test]
fn empty_pipe_stage_is_syntax_error() {
    let (ours, err) = run_script("echo a | | cat\necho $?\nexit\n");
    assert!(
        err.contains("syntax error near unexpected token `|'"),
        "stderr was: {err:?}"
    );
    assert_eq!(ours, "2\n");
}

#[test]
fn large_pipeline_does_not_deadlock() {
    let path = std::env::temp_dir().join(format!("0-shell-pipe-large-{}.txt", std::process::id()));
    let chunk = "line\n".repeat(20_000);
    std::fs::write(&path, &chunk).expect("write fixture");
    let script = format!("cat {} | cat | cat\nexit\n", path.display());
    let (ours, _) = run_script(&script);
    let _ = std::fs::remove_file(&path);
    assert_eq!(ours, chunk);
}
