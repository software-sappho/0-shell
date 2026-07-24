# SH-016 — Audit dry-run

_Last run: 2026-07-22 (WSL2 Linux, `rustc 1.97.1`)_

Walked the mandatory deliverable checklist (D1–D14) against a live `0-shell`
binary, with stdout diffs vs bash/coreutils where the audit compares output.
Crash-hunt pass included.

**Result: green — 28 / 28 automated checks passed. No follow-up tickets.**

## How to re-run

On Linux or WSL, from the repo root:

```sh
cargo build
./scripts/audit-dry-run.sh
```

The script feeds commands into `target/debug/0-shell`, captures stdout/stderr
separately (prompt stays on stderr), and compares byte-for-byte with
`bash` / `cat` / expected listings.

## Checklist

| ID | Check | Result |
|----|-------|--------|
| D1 | Binary builds, runs, exits cleanly on EOF | PASS |
| D2 | `$ ` prompt on stderr | PASS |
| D3/D5 | `echo` quoted + unquoted; matches bash stdout | PASS |
| D4 | `Command '<name>' not found` | PASS |
| D6 | `pwd` | PASS |
| D7 | `cd` after `mkdir` | PASS |
| D8 | `mkdir` creates siblings; duplicate → `File exists` | PASS |
| D9 | `cat` byte-identical to coreutils; dir → `Is a directory` | PASS |
| D10 | Plain `ls` hides dots, sorts | PASS |
| D11 | `ls -a` / `-F` / `-la` (`total`, mode) | PASS |
| D12 | `cp` into directory keeps basename | PASS |
| D13 | `mv` nests directory into directory | PASS |
| D14 | `rm` refuses dirs; `rm -r` removes tree | PASS |
| Crash-hunt | Bad flags, missing operands, unknown cmds, long input; recovers | PASS |

## Notes

- Prompt and EOF cosmetic newline go to **stderr**, so stdout diffs stay clean
  (see [TICKET-TRACKER.md](./TICKET-TRACKER.md) §5 note for D15/SH-016).
- Full `ls -l` owner/group/mtime parity depends on `/etc/passwd`, `/etc/group`,
  and local timezone — exercised structurally here (`total`, mode string).
  Re-run on the audit host before the formal review for byte-level `ls -l` diffs
  against GNU `ls`.
- Windows host `cargo test` still has 4 pre-existing failures in
  `mkdir`/`error` tests that assert Unix strerror text via raw OS error codes;
  Linux/WSL is the audit target and is green.
