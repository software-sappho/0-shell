// 0-shell must never invoke an external binary: no std::process::Command,
// no exec*, no shelling out to sh/bash/coreutils, ever. Every builtin is
// implemented directly on std::fs / std::io. `fork`+`pipe` (SH-026) and
// `dup2` redirections (SH-027) only run those same in-process builtins.

#![allow(dead_code)]

mod color;
mod commands;
mod complete;
mod dispatch;
mod error;
mod history;
mod parser;
mod pipeline;
mod redir;
mod repl;
mod signals;
mod tty;

fn main() {
    repl::run();
}
