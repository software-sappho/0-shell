> Legend: 🔴 Blocked · 🟡 To Do · 🟢 In Progress · 🔵 In Review · ✅ Done · ⬜ Backlog
>
> **Sources**: individual tickets (`SH-*.md`) · [BOARD.md](./BOARD.md) · [DEPENDENCIES.md](./DEPENDENCIES.md) · [TEAM.md](../TEAM.md)

---

# sh Ticket Tracker

Last refreshed: 2026-07-22 (SH-017 done — README / usage docs)

> **Board vs tracker**: [BOARD.md](./BOARD.md) is the live sprint board (who is on what). This file is the full requirements-style tracker: every ticket, deps, acceptance summary, and coverage by epic.

---

## 1) Scope Contract

This tracker covers the mandatory 0-shell deliverable and optional bonuses.

Execution order (see [DEPENDENCIES.md](./DEPENDENCIES.md)):

1. Bootstrap (`SH-001`)
2. Shell skeleton — REPL, parser, dispatch (`SH-002`–`SH-004`)
3. Parallel command implementation (`SH-005`–`SH-015`)
4. Audit dry-run & docs (`SH-016`, `SH-017`)
5. Bonus features (`SH-018`–`SH-024`, optional)

This repository delivers a **Rust** minimalist Unix shell. Every command is implemented from scratch on `std::fs` / `std::io` / raw syscalls. **No external binaries, no `Command::new`, no shelling out to `sh`/`bash`/coreutils** — this is an instant audit failure.

Byte-for-byte output parity with real `bash` is required wherever the audit compares terminal output (`echo`, `cat`, `pwd`, `ls`).

---

## 2) Team Assignment

The foundation phase (SH-001–SH-004) is complete. Phase 2 onward is unassigned
and open to claim — no ticket has an owner yet. Whoever picks up a ticket
should update its Assignee cell in the tables below.

---

## 3) Epics and Deliverable IDs

### Epics

| Epic | Scope |
|------|--------|
| `foundation` | Cargo setup, REPL loop, parser, dispatch, error model |
| `navigation` | `cd`, `pwd`, `exit` |
| `fs-read` | `ls`, `cat` |
| `fs-write` | `mkdir`, `cp`, `mv`, `rm` |
| `qa` | Audit checklist verification against real bash |
| `docs` | README / usage notes |
| `bonus` | Ctrl+C, prompt cwd, history, env vars, colors, help, chaining |

### Mandatory deliverables (audit checklist)

| ID | Deliverable | Tickets |
|----|-------------|---------|
| D1 | Cargo project builds & runs, Rust only | SH-001 |
| D2 | `$ ` prompt, executes only on Enter, Ctrl+D exits | SH-002 |
| D3 | Argument parsing — quoted and unquoted | SH-003 |
| D4 | Dispatch + `Command '<name>' not found` + never panics | SH-004 |
| D5 | `echo` matches bash for quoted and unquoted args | SH-005 |
| D6 | `pwd` and `exit` | SH-006 |
| D7 | `cd` — bare → `$HOME`, relative and absolute paths | SH-007 |
| D8 | `mkdir` | SH-008 |
| D9 | `cat` matches real `cat` exactly | SH-009 |
| D10 | `ls` plain listing | SH-010 |
| D11 | `ls -l -a -F` combined | SH-011, SH-012 |
| D12 | `cp` file into another directory | SH-013 |
| D13 | `mv`, incl. moving a directory into a directory | SH-014 |
| D14 | `rm` and `rm -r` | SH-015 |
| D15 | Full audit checklist verified end-to-end | SH-016 |

### Bonus deliverables

| ID | Deliverable | Tickets |
|----|-------------|---------|
| B1 | `Ctrl+C` (SIGINT) handled without exiting | SH-018 |
| B2 | Current directory in prompt | SH-019 |
| B3 | Command history | SH-020 |
| B4 | Environment variable support (`$HOME`, `$PATH`) | SH-021 |
| B5 | Colorized output | SH-022 |
| B6 | Custom `help` command | SH-023 |
| B7 | Command chaining with `;` | SH-024 |
| B8 | Auto-completion, piping, redirection | SH-025–SH-027 (backlog) |

---

## Phase 0 — Bootstrap

> **Goal**: Initialize the Cargo project and module layout.

| ID | Status | Ticket | Size | Deps | Coverage | Assignee |
|----|--------|--------|------|------|----------|----------|
| SH-001 | ✅ | **Cargo project & repo structure**: `Cargo.toml`, `src/main.rs`, `src/commands/` module stubs, `src/parser.rs`, `src/error.rs`, README stub, `cargo run` produces a running binary. | S | — | D1 | — |

---

## Phase 1 — Shell Skeleton

> **Goal**: A shell that loops, parses, and dispatches — with zero commands implemented yet.

| ID | Status | Ticket | Size | Deps | Coverage | Assignee |
|----|--------|--------|------|------|----------|----------|
| SH-002 | ✅ | **REPL loop**: print exactly `$ ` (with flush), block on `read_line`, execute only after Enter, `Ok(0)` from stdin (Ctrl+D) exits cleanly with status 0, empty/whitespace-only line reprints prompt. | M | SH-001 | D2 | — |
| SH-003 | ✅ | **Tokenizer**: split on whitespace, honour `"…"` and `'…'` grouping, handle unterminated quotes without panicking, return `Vec<String>`. Must make `echo "Hello There"` one arg and `echo something else` two. | M | SH-001 | D3 | — |
| SH-004 | ✅ | **Dispatch & error model**: `Builtin` enum/table mapping name → handler `fn(&[String]) -> Result<(), ShellError>`; unknown name prints exactly `Command '<name>' not found`; all errors print to stderr as `<cmd>: <path>: <reason>` and return to prompt. No `unwrap`/`expect` on user input paths. | M | SH-002, SH-003 | D4 | — |

---

## Phase 2 — Commands (parallel)

> **Goal**: All ten required commands, from scratch. Up to 3 people in parallel once SH-004 lands.

| ID | Status | Ticket | Size | Deps | Coverage | Assignee |
|----|--------|--------|------|------|----------|----------|
| SH-005 | ✅ | **echo**: join args with a single space + trailing `\n`; quotes already stripped by SH-003. Verify `echo "something!"` and `echo something else` byte-match bash. | S | SH-004 | D5 | — |
| SH-006 | ✅ | **pwd & exit**: `pwd` via `env::current_dir()`; `exit` terminates the process cleanly (optional numeric status arg) and returns control to the parent shell. | S | SH-004 | D6 | — |
| SH-007 | ✅ | **cd**: bare `cd` → `$HOME`; relative and absolute paths via `env::set_current_dir`; errors: `cd: <path>: No such file or directory`, `Not a directory`, `Permission denied`. Confirm with `pwd` after nested `mkdir`. | M | SH-004, SH-006 | D7 | — |
| SH-008 | ✅ | **mkdir**: `fs::create_dir` per operand, multiple operands in one call; error `mkdir: cannot create directory '<x>': File exists`. Two separate `mkdir` calls must yield two independent dirs. | S | SH-004 | D8 | — |
| SH-009 | ✅ | **cat**: stream file bytes to stdout unmodified (no added/stripped newline), multiple operands concatenated, directory operand → `cat: <x>: Is a directory`. Must byte-match real `cat`. | M | SH-004 | D9 | — |
| SH-010 | ✅ | **ls (plain)**: read dir entries, hide dotfiles, sort by name (bytewise, matching `ls` default), print columnized or one-per-line; bare `ls` and `ls <dir>` and `ls <file>`. | M | SH-004 | D10 | — |
| SH-011 | ✅ | **ls flags -a / -F**: flag parsing incl. combined forms (`-la`, `-l -a -F`); `-a` shows `.`/`..`/dotfiles; `-F` appends `/` dir, `*` exec, `@` symlink. | M | SH-010 | D11 | — |
| SH-012 | ✅ | **ls -l long format**: `total <blocks>`, mode string (`drwxr-xr-x`) from `MetadataExt::mode()`, link count, uid/gid → names via `/etc/passwd`+`/etc/group` parsing (no `getpwuid` shell-out), size, `Mon DD HH:MM` mtime, column alignment. | L | SH-010, SH-011 | D11 | — |
| SH-013 | ✅ | **cp**: `cp <src> <dst>`; if dst is an existing directory, copy into it preserving basename; preserve contents and mode; error on missing src / dir src without `-r`. Audit case: `cp new_doc.txt ../new_folder2`. | M | SH-004 | D12 | — |
| SH-014 | ✅ | **mv**: `fs::rename` fast path; fall back to copy+delete across filesystems; dst-is-directory → move into it. Audit case: `mv new_folder2 new_folder1` nests the directory. | M | SH-004, SH-013 | D13 | — |
| SH-015 | ✅ | **rm / rm -r**: file removal by default; `rm: cannot remove '<x>': Is a directory` without `-r`; `-r` walks depth-first and removes children before parents. Audit case: `rm -r new_folder1`. | M | SH-004 | D14 | — |

---

## Phase 3 — QA & Docs

> **Goal**: Every audit question answered "yes" before the review.

| ID | Status | Ticket | Size | Deps | Coverage | Assignee |
|----|--------|--------|------|------|----------|----------|
| SH-016 | ✅ | **Audit dry-run**: walk the 12-point checklist side-by-side against a real terminal, diff outputs, log failures as follow-up tickets. Includes crash-hunt pass: bad flags, missing operands, `/root`, non-UTF-8 filenames, very long input. | M | SH-005–SH-015 | D15 | — |
| SH-017 | ✅ | **README / usage docs**: build & run instructions, supported commands and flags, known deviations from GNU coreutils, bonus feature list. | S | SH-005–SH-015 | — | — |

---

## Phase 4 — Bonus (optional)

> **Goal**: Extra credit. Do **not** start before Phase 3 is green.

| ID | Status | Ticket | Size | Deps | Coverage | Assignee |
|----|--------|--------|------|------|----------|----------|
| SH-018 | 🟡 | **Ctrl+C (SIGINT)**: install handler via `signal`/`sigaction`; cancel the current line, print a fresh prompt, never exit or unwind through the loop. | M | SH-002 | B1 | — |
| SH-019 | 🟡 | **Prompt with cwd**: `~/projects/0-shell $ ` — `$HOME` collapsed to `~`, updates after `cd`. | S | SH-007 | B2 | — |
| SH-020 | 🟡 | **Command history**: in-memory ring + ↑/↓ recall (raw mode), optional persistence to `~/.0shell_history`. | M | SH-002 | B3 | — |
| SH-021 | 🟡 | **Environment variables**: expand `$VAR` / `${VAR}` at parse time via `env::var`; support `$HOME`, `$PATH`, `$?`. | M | SH-003 | B4 | — |
| SH-022 | 🟡 | **Colorized output**: ANSI colors for directories/executables in `ls`, red for errors; suppress when stdout is not a TTY. | S | SH-010, SH-011 | B5 | — |
| SH-023 | 🟡 | **help command**: list every builtin with flags and a one-line description; `help <cmd>` for detail. | S | SH-004 | B6 | — |
| SH-024 | 🟡 | **Command chaining `;`**: split the line into sequential commands before tokenizing; each runs in order, a failure does not abort the rest. | M | SH-003, SH-004 | B7 | — |

---

## Phase 5 — Backlog (unscheduled)

| ID | Status | Ticket | Size | Deps | Coverage | Assignee |
|----|--------|--------|------|------|----------|----------|
| SH-025 | ⬜ | **Auto-completion**: Tab completion for command names and paths (needs raw-mode input from SH-020). | L | SH-020 | B8 | — |
| SH-026 | ⬜ | **Piping (`\|`)**: `fork` + `pipe` + in-process command execution on both ends. | L | SH-024 | B8 | — |
| SH-027 | ⬜ | **Redirection (`>`, `<`)**: reopen stdin/stdout onto files via `dup2`. | L | SH-026 | B8 | — |

---

## 4) Critical Path

```
SH-001 → SH-002 → SH-004 → SH-010 → SH-011 → SH-012 → SH-016 → SH-017
```

With 3 people: SH-003 runs alongside SH-002; after SH-004 the eleven command tickets fan out three ways. The `ls` chain (SH-010 → SH-011 → SH-012) is the longest sequential run and the biggest audit risk — start it early.

Full dependency graph: [DEPENDENCIES.md](./DEPENDENCIES.md).

---

## 5) Deliverable Coverage Matrix

| Deliverable | Description | Tickets | Status |
|-------------|-------------|---------|--------|
| D1 | Project builds & runs | SH-001 | ✅ |
| D2 | REPL, prompt, Ctrl+D | SH-002 | ✅ |
| D3 | Argument parsing | SH-003 | ✅ |
| D4 | Dispatch & not-found message | SH-004 | ✅ |
| D5 | `echo` | SH-005 | ✅ |
| D6 | `pwd`, `exit` | SH-006 | ✅ |
| D7 | `cd` | SH-007 | ✅ |
| D8 | `mkdir` | SH-008 | ✅ |
| D9 | `cat` | SH-009 | ✅ |
| D10 | `ls` plain | SH-010 | ✅ |
| D11 | `ls -l -a -F` | SH-011 ✅, SH-012 ✅ | ✅ |
| D12 | `cp` | SH-013 | ✅ |
| D13 | `mv` | SH-014 | ✅ |
| D14 | `rm -r` | SH-015 | ✅ |
| D15 | Audit checklist verified | SH-016 | ✅ |
| B1 | Ctrl+C | SH-018 | 🟡 |
| B2 | Prompt cwd | SH-019 | 🟡 |
| B3 | History | SH-020 | 🟡 |
| B4 | Env vars | SH-021 | 🟡 |
| B5 | Colors | SH-022 | 🟡 |
| B6 | `help` | SH-023 | 🟡 |
| B7 | Chaining `;` | SH-024 | 🟡 |
| B8 | Completion / pipes / redirection | SH-025–027 | ⬜ |

> Note for D15/SH-016: the `$ ` prompt (and the cosmetic EOF newline) print to
> stderr, not stdout, so automated audit diffs can compare stdout against real
> bash with no filtering.

---

## 6) Immediate Next Work Queue

Mandatory path through docs is complete. Optional next work:

| Ticket | Notes |
|--------|-------|
| SH-018+ | Bonus track (Ctrl+C, prompt cwd, history, …) |

All of SH-001–SH-017 are ✅.

---

## Total: 24 scheduled tickets (+3 backlog)

| Status | Count |
|--------|-------|
| ✅ Done | 17 |
| 🔵 In Review | 0 |
| 🟢 In Progress | 0 |
| 🟡 To Do | 7 |
| ⬜ Backlog | 3 |

| Priority | Count |
|----------|-------|
| P0 | 4 (SH-001–SH-004) |
| P1 | 13 (SH-005–SH-017) |
| P2 | 7 (SH-018–SH-024 bonus) |
| P3 | 3 (SH-025–SH-027 backlog) |

| Size (estimate) | Count |
|-----------------|-------|
| S | 7 |
| M | 15 |
| L | 5 |

---

## Load balance note

The foundation was a serial gate (SH-001 → SH-002/SH-003 → SH-004) and it's now cleared. Eleven scheduled tickets remain (all bonus). The mandatory path including the audit dry-run and README is complete.

---

## How to update this file

1. Change status in the ticket file (`SH-XXX-*.md`) **and** [BOARD.md](./BOARD.md).
2. Mirror the status emoji in the phase tables above.
3. Bump **Last refreshed** at the top.
4. When a deliverable's tickets are all ✅, mark its row ✅ in the coverage matrix.
5. Any audit-checklist failure found in SH-016 becomes a new `SH-1XX` follow-up ticket, not a silent fix.
