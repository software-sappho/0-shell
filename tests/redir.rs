//! Integration tests for SH-027 redirection (`>`, `>>`, `<`).

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
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

fn scratch(label: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("0-shell-redir-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("scratch");
    base
}

#[test]
fn stdout_redirect_writes_file() {
    let dir = scratch("out");
    let file = dir.join("out.txt");
    let path = file.to_str().unwrap();
    let (ours, _) = run_script(&format!("echo hello > {path}\nexit\n"));
    assert_eq!(ours, "");
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stdout_redirect_truncates() {
    let dir = scratch("trunc");
    let file = dir.join("out.txt");
    fs::write(&file, "old contents\n").unwrap();
    let path = file.to_str().unwrap();
    let _ = run_script(&format!("echo new > {path}\nexit\n"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "new\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn append_redirect() {
    let dir = scratch("append");
    let file = dir.join("log.txt");
    let path = file.to_str().unwrap();
    let _ = run_script(&format!("echo one > {path}\necho two >> {path}\nexit\n"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "one\ntwo\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stdin_redirect() {
    let dir = scratch("in");
    let file = dir.join("in.txt");
    fs::write(&file, "from file").unwrap();
    let path = file.to_str().unwrap();
    let (ours, _) = run_script(&format!("cat < {path}\nexit\n"));
    assert_eq!(ours, "from file");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pipe_then_redirect() {
    let dir = scratch("pipeout");
    let file = dir.join("out.txt");
    let path = file.to_str().unwrap();
    let (ours, _) = run_script(&format!("echo a b | cat > {path}\nexit\n"));
    assert_eq!(ours, "");
    assert_eq!(fs::read_to_string(&file).unwrap(), "a b\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn redirect_then_still_interactive() {
    // After a redirect, the next command must still write to the real stdout.
    let dir = scratch("restore");
    let file = dir.join("out.txt");
    let path = file.to_str().unwrap();
    let (ours, _) = run_script(&format!("echo hidden > {path}\necho visible\nexit\n"));
    assert_eq!(ours, "visible\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), "hidden\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_input_file_is_error() {
    let missing =
        std::env::temp_dir().join(format!("0-shell-redir-missing-{}", std::process::id()));
    let _ = fs::remove_file(&missing);
    let path = missing.to_str().unwrap();
    let (ours, err) = run_script(&format!("cat < {path}\necho $?\nexit\n"));
    assert!(err.contains("No such file or directory"), "stderr={err:?}");
    assert_eq!(ours, "1\n");
}

#[test]
fn bare_redirect_creates_file() {
    let dir = scratch("bare");
    let file = dir.join("empty.txt");
    let path = file.to_str().unwrap();
    let (ours, _) = run_script(&format!("> {path}\necho $?\nexit\n"));
    assert_eq!(ours, "0\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), "");
    let _ = fs::remove_dir_all(&dir);
}
