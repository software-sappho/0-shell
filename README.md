# 0-shell

A minimalist Unix shell written in Rust. It implements its own REPL, argument
parser, and builtins (`echo`, `cd`, `pwd`, `ls`, `cat`, `cp`, `rm`, `mv`,
`mkdir`, `exit`) on top of `std::fs` / `std::io` — **never** by shelling out
to `sh`, `bash`, or coreutils.

> Instant audit failure: calling `std::process::Command`, `exec*`, or any
> external binary from this project.

## Requirements

- Rust 1.70+ (edition 2021)
- Unix-like host for a full audit match (Linux / WSL recommended). Windows
  builds for development, but error strings and `ls -l` owner/mtime details
  target Linux.

## Build

```sh
cargo build
# or
cargo build --release
```

Package name in `Cargo.toml` is `zero-shell` (Cargo forbids a leading digit).
A `[[bin]]` section names the binary `0-shell`.

## Run

```sh
./target/debug/0-shell
# or
./target/release/0-shell
```

You get a `$ ` prompt (on stderr). Type a command and press Enter. Ctrl+D
(EOF) exits with status 0.

### Quick smoke test

```sh
printf 'echo hello\npwd\nexit\n' | ./target/debug/0-shell
```

### Audit dry-run

On Linux/WSL, after `cargo build`:

```sh
./scripts/audit-dry-run.sh
```

Results from the last green run: [AUDIT-DRY-RUN.md](./AUDIT-DRY-RUN.md).

## Supported commands

| Command | Behaviour |
|---------|-----------|
| `echo [args…]` | Join args with a single space, trailing newline. No `-n` / `-e` flags. |
| `pwd` | Print the current working directory. |
| `cd [dir]` | Change directory. Bare `cd` → `$HOME`. Supports `~` / `~/…`. |
| `mkdir <dir>…` | Create each directory (not parents). Multi-component relative paths are truncated at the first `/` (e.g. `mkdir foo/bar` creates `foo`). |
| `cat <file>…` | Stream file bytes to stdout unmodified. Concatenates multiple operands. No operands → copy stdin. |
| `ls [opts] [path…]` | List directory entries. See flags below. |
| `cp <src> <dst>` | Copy a file. If `dst` is a directory, copy into it preserving the basename. Directories require `-r` (not implemented — refused). |
| `mv <src> <dst>` | Rename/move a file or directory. If `dst` is a directory, move into it. Cross-device moves fall back to copy+delete. |
| `rm [opts] <path>…` | Remove files. Directories need `-r` / `-R`. |
| `exit [code]` | Exit the shell (optional status 0–255). |

Unknown commands print exactly:

```text
Command '<name>' not found
```

Errors go to stderr as `<cmd>: <path>: <reason>` (or a command-specific usage line) and return to the prompt — the shell does not panic on bad user input.

### `ls` flags

| Flag | Effect |
|------|--------|
| *(none)* | One name per line; hide dotfiles; sort by name. |
| `-a` | Include `.`, `..`, and other dotfiles. |
| `-F` | Append `/` (dir), `*` (executable), `@` (symlink). |
| `-l` | Long format: `total`, mode, nlink, owner, group, size, mtime, name. |
| Combined | e.g. `-la`, `-l -a -F`. |

`--` ends option parsing. Owner/group names come from parsing `/etc/passwd` and `/etc/group` (no `getpwuid` shell-out).

### `rm` flags

| Flag | Effect |
|------|--------|
| `-r` / `-R` | Recursive depth-first removal of directories. |

## Quoting

The tokenizer splits on whitespace and groups `"…"` / `'…'`:

```text
$ echo "Hello There"
Hello There
$ echo something else
something else
```

Unterminated quotes produce a parse error and return to the prompt.

## Known deviations from GNU coreutils / bash

These are intentional scope limits or small behavioural differences:

| Area | Deviation |
|------|-----------|
| `echo` | No `-n` / `-e`; a leading `-n` is printed as a normal argument. |
| `mkdir` | No `-p`. A relative path with `/` only creates the first component. |
| `cp` | No `-r` / `-R`; directory sources are refused. Exactly two operands. |
| `mv` | Exactly two operands (no multi-source form). |
| `rm` | No `-f` / `-i`; only `-r` / `-R`. |
| `ls` | Default listing is one-per-line (like non-TTY GNU `ls`), not columnar. No `-R`, colours (see bonuses), or ACL `+` in the mode string. |
| `cd` | No `cd -` (previous directory). `~user` is not expanded. |
| Prompt | Fixed `$ ` (cwd in the prompt is a bonus). |
| Signals | Ctrl+C is not specially handled yet (bonus). |
| Env / `$VAR` | Not expanded at parse time yet (bonus). |
| Chaining | No `;`, pipes, or redirections yet (bonus / backlog). |

Where the audit compares terminal output (`echo`, `cat`, `pwd`, plain `ls`), this shell aims for byte-for-byte parity with bash/coreutils under `LANG=C`.

## Project layout

```text
src/
  main.rs          Entry point → REPL
  repl.rs          Prompt, read line, dispatch
  parser.rs        Tokenizer (quotes)
  dispatch.rs      Builtin table
  error.rs         ShellError
  commands/        echo, cd, pwd, ls, cat, cp, mv, rm, mkdir, exit
scripts/
  audit-dry-run.sh Automated checklist vs bash
```

Board / tracker: [BOARD.md](./BOARD.md), [TICKET-TRACKER.md](./TICKET-TRACKER.md),
[DEPENDENCIES.md](./DEPENDENCIES.md).

## Bonus features (optional)

Not required for the mandatory audit. Do not start these until SH-016 is green
(already done).

| ID | Feature | Ticket |
|----|---------|--------|
| B1 | Ctrl+C (SIGINT) without exiting | SH-018 |
| B2 | Current directory in the prompt | SH-019 |
| B3 | Command history | SH-020 |
| B4 | Environment variables (`$HOME`, `$PATH`, …) | SH-021 |
| B5 | Colorized `ls` / errors | SH-022 |
| B6 | `help` command | SH-023 |
| B7 | Command chaining with `;` | SH-024 |
| B8 | Completion / pipes / redirection | SH-025–SH-027 (backlog) |

## License

Project coursework — follow your school’s submission rules.
