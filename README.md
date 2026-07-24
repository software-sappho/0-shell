# 0-shell

A minimalist Unix shell written in Rust. It implements its own REPL, argument
parser, and builtins (`echo`, `cd`, `pwd`, `ls`, `cat`, `cp`, `rm`, `mv`,
`mkdir`, `help`, `exit`) on top of `std::fs` / `std::io` — **never** by
shelling out to `sh`, `bash`, or coreutils.

> Instant audit failure: calling `std::process::Command`, `exec*`, or any
> external binary from this project. `fork` + `pipe` + `dup2` are used only so
> the same in-process builtins can run on both ends of a pipeline or with
> file redirection.

Subject / checklist: [docs/requirements.md](./docs/requirements.md),
[docs/audit.md](./docs/audit.md).

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

```sh
cargo test   # unit + integration tests
cargo fmt
```

## Run

```sh
./target/debug/0-shell
# or
./target/release/0-shell
```

You get a prompt like `~/0-shell $ ` on stderr (`$HOME` collapsed to `~`).
Type a command and press Enter. Ctrl+D (EOF) exits with status 0. Ctrl+C
cancels the current line and reprints the prompt (does not exit).

### Quick smoke test

```sh
printf 'echo hello\npwd\nexit\n' | ./target/debug/0-shell
```

### Audit dry-run

On Linux/WSL, after `cargo build`:

```sh
./scripts/audit-dry-run.sh
```

Results from the last green run: [docs/AUDIT-DRY-RUN.md](./docs/AUDIT-DRY-RUN.md).

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
| `help [cmd]` | List builtins, or show detail for one command. |
| `exit [code]` | Exit the shell (optional status 0–255). |

Unknown commands print exactly:

```text
Command '<name>' not found
```

Errors go to stderr as `<cmd>: <path>: <reason>` (or a command-specific usage line) and return to the prompt — the shell does not panic on bad user input. When stderr is a TTY, error lines are printed in red.

### `ls` flags

| Flag | Effect |
|------|--------|
| *(none)* | One name per line; hide dotfiles; sort by name. |
| `-a` | Include `.`, `..`, and other dotfiles. |
| `-F` | Append `/` (dir), `*` (executable), `@` (symlink). |
| `-l` | Long format: `total`, mode, nlink, owner, group, size, mtime, name. |
| Combined | e.g. `-la`, `-l -a -F`. |
| Colors | On a TTY stdout: directories blue, executables green, symlinks cyan. Disabled when piped/redirected. |

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

## Shell features

| Feature | Behaviour |
|---------|-----------|
| Env vars | `$VAR`, `${VAR}`, and `$?` expand outside single quotes (and inside double quotes). |
| Chaining | Unquoted `;` runs commands left to right; a failure does not abort later ones. |
| Piping | Unquoted `\|` connects builtins with `pipe` + `fork` (in-process on both ends). |
| Redirection | Unquoted `<` / `>` / `>>` reopen stdin/stdout onto files via `dup2`. |
| History | ↑/↓ recall in a TTY (raw mode); persisted to `~/.0shell_history`. |
| Completion | Tab completes builtins (command position) and paths in a TTY. |
| Prompt | `~/path $ ` with `$HOME` → `~`, updates after `cd`. |
| Signals | Ctrl+C cancels the line and reprints the prompt. |

Examples:

```text
$ echo hello | cat
hello
$ echo hi > out.txt
$ cat < out.txt
hi
$ echo a; echo b
a
b
$ echo $HOME
/home/…
```

## Known deviations from GNU coreutils / bash

These are intentional scope limits or small behavioural differences:

| Area | Deviation |
|------|-----------|
| `echo` | No `-n` / `-e`; a leading `-n` is printed as a normal argument. |
| `mkdir` | No `-p`. A relative path with `/` only creates the first component. |
| `cp` | No `-r` / `-R`; directory sources are refused. Exactly two operands. |
| `mv` | Exactly two operands (no multi-source form). |
| `rm` | No `-f` / `-i`; only `-r` / `-R`. |
| `ls` | Default listing is one-per-line (like non-TTY GNU `ls`), not columnar. No `-R` or ACL `+` in the mode string. |
| `cd` | No `cd -` (previous directory). `~user` is not expanded. |
| Piping | No `\|\|` / `\|&` / `pipefail`. |
| Redirection | No `2>` / `&>` / heredocs. |
| Prompt | Interactive prompt includes the cwd (bonus), not bare `$ ` alone. |

Where the audit compares terminal output (`echo`, `cat`, `pwd`, plain `ls`), this shell aims for byte-for-byte parity with bash/coreutils under `LANG=C`.

## Project layout

```text
src/
  main.rs          Entry point → REPL
  repl.rs          Prompt, history-aware line editor, dispatch
  history.rs       In-memory history ring + `~/.0shell_history`
  complete.rs      Tab completion (builtins + paths)
  tty.rs           Raw mode (termios) for interactive input
  signals.rs       SIGINT handler (Ctrl+C)
  parser.rs        Tokenizer (quotes), `;` / `|` splitting, `<`/`>`/`>>`
  pipeline.rs      Pipe execution (`pipe` + `fork` + builtins)
  redir.rs         File redirection (`dup2` onto stdin/stdout)
  dispatch.rs      Builtin table
  error.rs         ShellError
  color.rs         ANSI helpers for `ls` / errors
  commands/        echo, cd, pwd, ls, cat, cp, mv, rm, mkdir, help, exit
scripts/
  audit-dry-run.sh Automated checklist vs bash
docs/
  requirements.md  Subject brief
  audit.md         Audit questionnaire (answered)
  AUDIT-DRY-RUN.md Last automated dry-run results
  BOARD.md         Sprint board
  TICKET-TRACKER.md Ticket coverage
  DEPENDENCIES.md  Ticket dependency map
tests/
  pipeline.rs      Integration tests for `|`
  redir.rs         Integration tests for `<` / `>` / `>>`
```

Board / tracker: [docs/BOARD.md](./docs/BOARD.md),
[docs/TICKET-TRACKER.md](./docs/TICKET-TRACKER.md),
[docs/DEPENDENCIES.md](./docs/DEPENDENCIES.md).

## Bonus features

| ID | Feature | Status |
|----|---------|--------|
| B1 | Ctrl+C (SIGINT) without exiting | ✅ |
| B2 | Current directory in the prompt | ✅ |
| B3 | Command history | ✅ |
| B4 | Environment variables (`$HOME`, `$PATH`, …) | ✅ |
| B5 | Colorized `ls` / errors | ✅ |
| B6 | `help` command | ✅ |
| B7 | Command chaining with `;` | ✅ |
| B8 | Tab completion, pipes, redirection | ✅ |

## License

Project coursework — follow your school’s submission rules.
