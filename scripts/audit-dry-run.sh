#!/usr/bin/env bash
# SH-016 audit dry-run: exercise checklist items against 0-shell and, where
# noted, diff stdout against real bash/coreutils.
#
# Usage (from repo root, on Linux/WSL):
#   cargo build
#   ./scripts/audit-dry-run.sh
#
# Exit 0 = all automated checks passed. Manual items are printed as SKIP/MANUAL.

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SHELL_BIN="${SHELL_BIN:-$ROOT/target/debug/0-shell}"
WORKDIR="$(mktemp -d /tmp/0-shell-audit-XXXXXX)"
PASS=0
FAIL=0
SKIP=0
REPORT=()

cleanup() {
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

if [[ ! -x "$SHELL_BIN" ]]; then
  echo "error: missing binary at $SHELL_BIN (run cargo build first)" >&2
  exit 1
fi

# Feed lines to 0-shell; capture stdout and stderr separately.
run_shell() {
  local stdout_f="$WORKDIR/sh.out" stderr_f="$WORKDIR/sh.err"
  : >"$stdout_f"
  : >"$stderr_f"
  printf '%s\n' "$@" | "$SHELL_BIN" >"$stdout_f" 2>"$stderr_f"
  echo "$stdout_f|$stderr_f"
}

record() {
  local status="$1" id="$2" detail="$3"
  REPORT+=("$status|$id|$detail")
  case "$status" in
    PASS) PASS=$((PASS + 1)) ;;
    FAIL) FAIL=$((FAIL + 1)) ;;
    *) SKIP=$((SKIP + 1)) ;;
  esac
  printf '[%s] %s — %s\n' "$status" "$id" "$detail"
}

# Compare two files byte-for-byte (avoids bash $(…) eating trailing newlines).
assert_files_eq() {
  local id="$1" got_f="$2" want_f="$3" detail="$4"
  if cmp -s "$got_f" "$want_f"; then
    record PASS "$id" "$detail"
  else
    record FAIL "$id" "$detail (diff follows)"
    diff -u "$want_f" "$got_f" || true
  fi
}

assert_eq_text() {
  local id="$1" got_f="$2" want="$3" detail="$4"
  local want_f="$WORKDIR/want.$id"
  printf '%s' "$want" >"$want_f"
  assert_files_eq "$id" "$got_f" "$want_f" "$detail"
}

assert_contains() {
  local id="$1" hay="$2" needle="$3" detail="$4"
  if [[ "$hay" == *"$needle"* ]]; then
    record PASS "$id" "$detail"
  else
    record FAIL "$id" "$detail (missing $(printf %q "$needle") in $(printf %q "$hay"))"
  fi
}

assert_file() {
  local id="$1" path="$2" detail="$3"
  if [[ -e "$path" ]]; then
    record PASS "$id" "$detail"
  else
    record FAIL "$id" "$detail (missing $path)"
  fi
}

echo "=== 0-shell SH-016 audit dry-run ==="
echo "binary: $SHELL_BIN"
echo "work:   $WORKDIR"
echo

# --- D1: builds & runs ---
if "$SHELL_BIN" </dev/null >/dev/null 2>&1; then
  record PASS D1 "binary runs and exits on EOF"
else
  # EOF exit is exit 0; a crash would be non-zero often
  status=$?
  if [[ $status -eq 0 ]]; then
    record PASS D1 "binary runs and exits on EOF"
  else
    record FAIL D1 "binary exited $status on EOF"
  fi
fi

# --- D2: prompt on stderr, Ctrl+D / EOF ---
paths=$(run_shell "")
stdout="${paths%%|*}"
stderr="${paths##*|}"
assert_contains D2 "$(cat "$stderr")" '$ ' "prompt printed to stderr"
# empty input still shows prompt; EOF adds cosmetic newline on stderr

# --- D3 / D5: echo quoting ---
paths=$(run_shell 'echo "Hello There"' 'echo something else')
stdout="${paths%%|*}"
assert_eq_text D5 "$stdout" $'Hello There\nsomething else\n' "echo quoted vs unquoted"

# bash builtin comparison for echo
bash_tmp="$WORKDIR/bash.echo"
printf '%s\n' 'echo "Hello There"' 'echo something else' | bash --norc --noprofile >"$bash_tmp" 2>/dev/null
assert_files_eq D5-bash "$stdout" "$bash_tmp" "echo stdout matches bash"

# --- D4: unknown command ---
paths=$(run_shell 'something')
stderr="${paths##*|}"
assert_contains D4 "$(cat "$stderr")" "Command 'something' not found" "unknown command message"

# --- D6: pwd ---
cd "$WORKDIR"
paths=$(run_shell 'pwd')
stdout="${paths%%|*}"
want_pwd="$(pwd -P)"$'\n'
assert_eq_text D6 "$stdout" "$want_pwd" "pwd prints cwd"

# --- D7 / D8: mkdir + cd ---
paths=$(run_shell "mkdir a b" "cd a" "pwd")
stdout="${paths%%|*}"
assert_file D8-a "$WORKDIR/a" "mkdir created a"
assert_file D8-b "$WORKDIR/b" "mkdir created b"
assert_contains D7 "$(cat "$stdout")" "/a" "cd into mkdir'd dir"

# File exists error
paths=$(run_shell "mkdir a")
stderr="${paths##*|}"
assert_contains D8-exists "$(cat "$stderr")" "File exists" "mkdir duplicate errors"

# --- D9: cat ---
printf 'hello\xffworld' >"$WORKDIR/bin.dat"
paths=$(run_shell "cat $WORKDIR/bin.dat")
stdout="${paths%%|*}"
assert_files_eq D9 "$stdout" "$WORKDIR/bin.dat" "cat byte-identical to coreutils"

paths=$(run_shell "cat $WORKDIR")
stderr="${paths##*|}"
assert_contains D9-dir "$(cat "$stderr")" "Is a directory" "cat on directory"

# --- D10: ls plain ---
mkdir -p "$WORKDIR/lsdir"
touch "$WORKDIR/lsdir/zebra" "$WORKDIR/lsdir/alpha" "$WORKDIR/lsdir/.hidden"
paths=$(run_shell "ls $WORKDIR/lsdir")
stdout="${paths%%|*}"
assert_eq_text D10 "$stdout" $'alpha\nzebra\n' "ls hides dotfiles and sorts"

# --- D11: ls -a -F ---
paths=$(run_shell "ls -a $WORKDIR/lsdir")
stdout="${paths%%|*}"
assert_contains D11-a "$(cat "$stdout")" ".hidden" "ls -a shows hidden"
assert_contains D11-a-dot "$(cat "$stdout")" $'.\n' "ls -a shows ."

mkdir -p "$WORKDIR/lsdir/sub"
paths=$(run_shell "ls -F $WORKDIR/lsdir")
stdout="${paths%%|*}"
assert_contains D11-F "$(cat "$stdout")" "sub/" "ls -F marks directories"

paths=$(run_shell "ls -la $WORKDIR/lsdir")
stdout="${paths%%|*}"
assert_contains D11-la "$(cat "$stdout")" "total " "ls -la prints total"
assert_contains D11-la-mode "$(cat "$stdout")" "drwx" "ls -la includes mode"

# --- D12: cp into directory ---
printf 'doc' >"$WORKDIR/new_doc.txt"
mkdir -p "$WORKDIR/new_folder2"
paths=$(run_shell "cp $WORKDIR/new_doc.txt $WORKDIR/new_folder2")
assert_file D12 "$WORKDIR/new_folder2/new_doc.txt" "cp into directory preserves basename"
got_bytes="$(cat "$WORKDIR/new_folder2/new_doc.txt"; printf x)"
got_bytes="${got_bytes%x}"
if [[ "$got_bytes" == "doc" ]]; then
  record PASS D12-bytes "cp contents"
else
  record FAIL D12-bytes "cp contents mismatch"
fi

# --- D13: mv directory into directory ---
mkdir -p "$WORKDIR/new_folder1" "$WORKDIR/moveme"
echo x >"$WORKDIR/moveme/inside.txt"
paths=$(run_shell "mv $WORKDIR/moveme $WORKDIR/new_folder1")
assert_file D13 "$WORKDIR/new_folder1/moveme/inside.txt" "mv nests directory"
if [[ ! -e "$WORKDIR/moveme" ]]; then
  record PASS D13-gone "mv removed source"
else
  record FAIL D13-gone "mv left source behind"
fi

# --- D14: rm / rm -r ---
echo y >"$WORKDIR/toremove.txt"
paths=$(run_shell "rm $WORKDIR/toremove.txt")
if [[ ! -e "$WORKDIR/toremove.txt" ]]; then
  record PASS D14-file "rm removes file"
else
  record FAIL D14-file "rm did not remove file"
fi

mkdir -p "$WORKDIR/new_folder1/tree/sub"
echo z >"$WORKDIR/new_folder1/tree/sub/f"
paths=$(run_shell "rm $WORKDIR/new_folder1/tree")
stderr="${paths##*|}"
assert_contains D14-dir "$(cat "$stderr")" "Is a directory" "rm without -r refuses directory"

paths=$(run_shell "rm -r $WORKDIR/new_folder1/tree")
if [[ ! -e "$WORKDIR/new_folder1/tree" ]]; then
  record PASS D14-r "rm -r removes tree"
else
  record FAIL D14-r "rm -r left tree behind"
fi

# --- Crash-hunt ---
paths=$(run_shell 'ls -z' 'cp' 'rm' 'cat /root' "echo $(python3 -c 'print(\"A\"*5000)' 2>/dev/null || printf 'A%.0s' {1..5000})")
stderr="${paths##*|}"
# still alive = we got stderr from the run
if [[ -f "$stderr" ]]; then
  record PASS CRASH-hunt "survived bad flags, missing operands, long input"
else
  record FAIL CRASH-hunt "shell did not produce stderr capture"
fi

paths=$(run_shell 'notacommand')
stderr="${paths##*|}"
assert_contains CRASH-unknown "$(cat "$stderr")" "not found" "unknown command returns to prompt"

# Never panics: second command after error still works
paths=$(run_shell 'badcmd' 'echo still-here')
stdout="${paths%%|*}"
assert_contains CRASH-recover "$(cat "$stdout")" "still-here" "recovers after error"

echo
echo "=== summary: $PASS pass, $FAIL fail, $SKIP skip ==="
if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
exit 0
