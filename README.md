# 0-shell

`0-shell` is a minimalist Unix shell written in Rust with no external
dependencies: it implements its own REPL, argument parser, and a small set
of builtin commands (`echo`, `cd`, `pwd`, `ls`, `cat`, `cp`, `rm`, `mv`,
`mkdir`, `exit`) directly on top of `std::fs` / `std::io`, without ever
invoking an external binary.

## Build

```sh
cargo build
```

## Run

```sh
./target/debug/0-shell
```

> **Package name vs. binary name**: Cargo package names can't start with a
> digit, so the package in `Cargo.toml` is named `zero-shell`. A `[[bin]]`
> section retargets the compiled output to `0-shell`, so the binary you run
> is `./target/debug/0-shell` (or `./target/release/0-shell` for a release
> build), not `zero-shell`.

## Commands

| Command | Status |
|---------|--------|
| `echo`  | Not yet implemented |
| `cd`    | Not yet implemented |
| `pwd`   | Not yet implemented |
| `ls`    | Not yet implemented |
| `cat`   | Not yet implemented |
| `cp`    | Not yet implemented |
| `rm`    | Not yet implemented |
| `mv`    | Not yet implemented |
| `mkdir` | Not yet implemented |
| `exit`  | Not yet implemented |
