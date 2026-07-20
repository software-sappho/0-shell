// 0-shell must never invoke an external binary: no std::process::Command,
// no exec*/system/fork-exec, no shelling out to sh/bash/coreutils, ever.
// Every builtin is implemented directly on std::fs / std::io.

// Scaffolding stage (SH-001): parser/dispatch/commands are not wired
// together yet — that happens in SH-002/003/004. Allow dead_code until then.
#![allow(dead_code)]

mod commands;
mod dispatch;
mod error;
mod parser;
mod repl;

fn main() {
    repl::run();
}
