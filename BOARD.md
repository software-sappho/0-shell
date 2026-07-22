# Board

_Last updated: 2026-07-22_

> **Dependency map:** see [DEPENDENCIES.md](./DEPENDENCIES.md) for which tickets block which.
> **Full tracker:** see [TICKET-TRACKER.md](./TICKET-TRACKER.md) for acceptance detail and coverage.
> **Branch naming:** `ticket/SH-XXX-short-title` — see [../docs/BRANCHING.md](../docs/BRANCHING.md)

**Status:** Foundation phase (SH-001–SH-004) and the navigation stack (SH-005–SH-007) are done. `mkdir` (SH-008) is done. SH-016 (audit dry-run) still needs all of SH-009–SH-015 first. Phase 2 is open — every remaining ticket is unassigned and unclaimed.

8 Done, 16 To Do, 0 In Progress, 0 In Review, 3 Backlog.

**Do not start Phase 4 (bonus) until every Phase 2 ticket is ✅ and SH-016 passes.**

---

## Backlog

| ID | Title | Priority | Epic | Assignee |
|----|-------|----------|------|----------|
| [SH-025](./SH-025-bonus-autocompletion.md) | Auto-completion (bonus) | P3 | bonus | — |
| [SH-026](./SH-026-bonus-piping.md) | Piping `\|` (bonus) | P3 | bonus | — |
| [SH-027](./SH-027-bonus-redirection.md) | Redirection `>` `<` (bonus) | P3 | bonus | — |

## To Do

| ID | Title | Priority | Epic | Assignee |
|----|-------|----------|------|----------|
| [SH-009](./SH-009-cat.md) | `cat` | P1 | fs-read | — |
| [SH-010](./SH-010-ls-plain.md) | `ls` plain listing | P1 | fs-read | — |
| [SH-011](./SH-011-ls-flags.md) | `ls -a` / `-F` flags | P1 | fs-read | — |
| [SH-012](./SH-012-ls-long.md) | `ls -l` long format | P1 | fs-read | — |
| [SH-013](./SH-013-cp.md) | `cp` | P1 | fs-write | — |
| [SH-014](./SH-014-mv.md) | `mv` | P1 | fs-write | — |
| [SH-015](./SH-015-rm.md) | `rm` / `rm -r` | P1 | fs-write | — |
| [SH-016](./SH-016-audit-dry-run.md) | Audit dry-run vs real bash | P1 | qa | — |
| [SH-017](./SH-017-readme.md) | README / usage docs | P1 | docs | — |
| [SH-018](./SH-018-bonus-sigint.md) | Ctrl+C handling (bonus) | P2 | bonus | — |
| [SH-019](./SH-019-bonus-prompt-cwd.md) | Current dir in prompt (bonus) | P2 | bonus | — |
| [SH-020](./SH-020-bonus-history.md) | Command history (bonus) | P2 | bonus | — |
| [SH-021](./SH-021-bonus-env-vars.md) | Environment variables (bonus) | P2 | bonus | — |
| [SH-022](./SH-022-bonus-colors.md) | Colorized output (bonus) | P2 | bonus | — |
| [SH-023](./SH-023-bonus-help.md) | `help` command (bonus) | P2 | bonus | — |
| [SH-024](./SH-024-bonus-chaining.md) | Command chaining `;` (bonus) | P2 | bonus | — |

## In Progress

| ID | Title | Priority | Epic | Assignee |
|----|-------|----------|------|----------|
| _empty_ | | | | |

## In Review

| ID | Title | Priority | Epic | Assignee |
|----|-------|----------|------|----------|
| _empty_ | | | | |

## Done

| ID | Title | Priority | Epic | Assignee |
|----|-------|----------|------|----------|
| [SH-001](./SH-001-project-setup.md) | Cargo project & repo structure | P0 | foundation | — |
| [SH-002](./SH-002-repl-loop.md) | REPL loop, `$ ` prompt, Ctrl+D exit | P0 | foundation | — |
| [SH-003](./SH-003-tokenizer.md) | Tokenizer — quoted & unquoted args | P0 | foundation | — |
| [SH-004](./SH-004-dispatch-errors.md) | Dispatch table & error model | P0 | foundation | — |
| [SH-005](./SH-005-echo.md) | `echo` | P1 | foundation | — |
| [SH-006](./SH-006-pwd-exit.md) | `pwd` & `exit` | P1 | navigation | — |
| [SH-007](./SH-007-cd.md) | `cd` (bare → `$HOME`, rel & abs) | P1 | navigation | — |
| [SH-008](./SH-008-mkdir.md) | `mkdir` | P1 | fs-write | — |

---

## Definition of Done

A ticket moves to **In Review** only when all of these hold:

1. `cargo build` is warning-free and `cargo fmt` / `cargo clippy` are clean.
2. No external binary is invoked — no `std::process::Command`, no `exec*`, no shelling out.
3. Output was diffed against real `bash` for every case the audit checks.
4. Bad input (missing operand, nonexistent path, permission denied, directory-where-file-expected) prints an error and returns to the prompt — no panic, no unwind.
5. The relevant audit-checklist item in [TICKET-TRACKER.md](./TICKET-TRACKER.md) §3 is demonstrably answerable "yes".
