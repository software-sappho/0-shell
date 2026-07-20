// 0-shell must never invoke an external binary: no std::process::Command,
// no exec*/system/fork-exec, no shelling out to sh/bash/coreutils, ever.
// Every builtin is implemented directly on std::fs / std::io.

// Scaffolding stage: command builtins are still stubs, so ShellError::Usage
// has no constructor yet. Allow dead_code until the command tickets land.
#![allow(dead_code)]

mod commands;
mod dispatch;
mod error;
mod parser;
mod repl;

fn main() {
    repl::run();
}
