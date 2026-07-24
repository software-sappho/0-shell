#### General

###### Was the project written in Rust?

**Yes.** Cargo package `zero-shell` (edition 2021); binary name `0-shell`. Sources under `src/` are Rust only.

###### Are the commands mentioned in the subject implemented from scratch, without calling any external binaries?

**Yes.** Builtins use `std::fs` / `std::io` and raw Unix calls (`fork`, `pipe`, `dup2`, termios, etc.) only as needed for the REPL, pipelines, and redirection. There is no `std::process::Command`, no `exec*`, and no shelling out to `sh` / `bash` / coreutils. Pipeline stages and redirects still run the same in-process builtins.

The use of external binaries or system calls that spawn them is strictly forbidden, as the project requires implementing all functionality from scratch without relying on external programs.

#### Functional

##### Open a terminal and run the project.

###### Can you confirm that the project runs and displays a unix shell?

**Yes.** `cargo build` then `./target/debug/0-shell` starts an interactive REPL that reads lines, runs builtins, and returns to the prompt until `exit` or Ctrl+D.

##### Open a terminal and run the project.

###### Can you confirm that this interpreter displays at least a simple `$` and waits for you to type a command?

**Yes.** The prompt ends with `$ ` (bonus form: `~/path $ ` on stderr) and blocks on line input until Enter or EOF.

##### Try to run a command of your choice.

###### Can you confirm that the interpreter only validates the command if you press enter?

**Yes.** Characters are buffered (cooked or raw line editor); tokenization and dispatch run only after Enter (`\n` / `\r`).

##### Try to run the command `exit`.

###### Can you confirm that the interpreter terminates properly and gives back the parent's shell?

**Yes.** `exit` (optional status 0–255) ends the process cleanly; Ctrl+D on an empty line also exits with status 0. Control returns to the parent shell.

##### Try to run the command `echo "something"`. Do the same in your computer terminal.

###### Can you confirm that the displayed message of the project is exactly the same as the computer terminal?

**Yes.** Quotes are stripped by the tokenizer; stdout is `something\n`, matching bash’s `echo "something"`.

##### Try to run the command `echo something else` (without double quotes). Do the same in your computer terminal.

###### Can you confirm that the displayed message of the project is exactly the same as the computer terminal?

**Yes.** Args are joined with a single space and a trailing newline → `something else\n`, matching bash.

##### Try to run the command `pwd`.

###### Can you confirm that the interpreter displayed the current path?

**Yes.** Prints `std::env::current_dir()` followed by a newline.

##### Try to open the project and create a parent folder with two children folders using the command `mkdir`. Then enter the parent folder and do `pwd`.

###### Can you confirm that the interpreter displayed the current path?

**Yes.** Example: `mkdir parent`, `cd parent`, `mkdir child1`, `mkdir child2`, then `pwd` prints the absolute path of `parent`. (No `mkdir -p`; create the parent first, then children.)

##### Try to enter a directory of your choice by using the command `cd dir/of/your/choice`.

###### Can you confirm that the interpreter took you to the correct path? Use `pwd` to confirm.

**Yes.** Relative and absolute paths work via `env::set_current_dir`; `pwd` reflects the new cwd. The prompt cwd updates as well.

##### Try to run only the command `cd`.

###### Can you confirm that the interpreter took you to the users home folder? Use `pwd` to confirm.

**Yes.** Bare `cd` changes to `$HOME` (also supports `~` / `~/…`).

##### Try to run the command `ls` in a directory at your choice. Do the same in your computer terminal.

###### Can you confirm that the output is similar in the project and in your computer terminal?

**Yes.** Plain `ls` lists non-dot names, sorted, one per line (same as non-TTY GNU `ls`). Names match; layout may differ from a columnar TTY GNU `ls`.

##### Try to run the command `ls -l -a -F` in a directory at your choice. Do the same in your computer terminal.

###### Can you confirm that the output is similar in the project and in your computer terminal?

**Yes.** Combined flags work (`-l`, `-a`, `-F`, and forms like `-la`). Long format includes `total`, mode, nlink, owner/group (from `/etc/passwd` + `/etc/group`), size, mtime, and name; `-a` shows dot entries; `-F` appends `/`, `*`, `@`. Output is comparable to coreutils under `LANG=C` (no ACL `+`).

##### Try to run the commands `mkdir new_folder1` and `mkdir new_folder2` in a directory of your choice.

###### Can you confirm that the directories `new_folder1` and `new_folder2` were created?

**Yes.** Two separate `mkdir` calls create two sibling directories.

##### Create a document inside the `new_folder1` called `new_doc.txt` with some random text inside (`echo "some random text" > new_folder1/new_doc.txt`) . Try to run the command `cp new_doc.txt ../new_folder2` to copy the document to the folder `new_folder2`.

###### Can you confirm that the document `new_doc.txt` is inside the `new_folder2`?

**Yes.** With cwd inside `new_folder1`, `cp new_doc.txt ../new_folder2` copies into that directory preserving the basename.

##### Try to run the command `cat new_folder1/new_doc.txt`. Do the same in your computer terminal.

###### Can you confirm that the output is the same in the project and in your computer terminal?

**Yes.** `cat` streams file bytes unmodified (byte-identical to coreutils `cat` for normal files). Use the real filename (`new_doc.txt` if that is what was created).

##### Try to run the commands `mv new_folder2 new_folder1` to move the directory `new_folder2` inside of the directory `new_folder1`.

###### Can you confirm that the directory `new_folder2` is inside of the directory `new_folder1`?

**Yes.** When the destination is an existing directory, `mv` nests the source into it (`new_folder1/new_folder2`).

##### Try to run the command `rm -r new_folder1` to remove what was created above.

###### Can you confirm that the directory `new_folder1` was removed?

**Yes.** `rm -r` / `rm -R` removes the tree depth-first; without `-r`, removing a directory errors with `Is a directory`.

#### Bonus

###### +Did the student handle `Ctrl + C` gracefully?

**Yes.** SIGINT cancels the current line and reprints the prompt; the shell does not exit (`src/signals.rs`, REPL handling).

###### +Did the student add auto-completion when writing a command?

**Yes.** Tab completion for builtins (command position) and paths in a TTY (`src/complete.rs`, raw-mode line editor).

###### +Did the student implement piping?

**Yes.** Unquoted `|` runs builtins via `pipe` + `fork` with in-process dispatch on each stage (`src/pipeline.rs`). Example: `echo hello | cat`.

###### +Did the student add colors to the errors or directories?

**Yes.** On a TTY: directories blue, executables green, symlinks cyan in `ls`; errors in red on stderr. Colors are suppressed when not a TTY (`src/color.rs`).

###### +Did the student implement redirection?

**Yes.** Unquoted `<`, `>`, and `>>` reopen stdin/stdout onto files with `dup2` (`src/redir.rs`). Works alone and with pipelines (e.g. `echo hi > out.txt`, `cat < out.txt`, `echo a | cat > out.txt`).

###### +Did the student display the current directory in the prompt?

**Yes.** Prompt is `~/path $ ` with `$HOME` collapsed to `~`, refreshed after `cd` (`format_prompt` in `src/repl.rs`).

###### +Did the student add any other features or commands to the project?

**Yes.** Additional bonuses beyond the checklist items above:

- Command history (↑/↓) with persistence to `~/.0shell_history`
- Environment variable expansion (`$VAR`, `${VAR}`, `$?`)
- Command chaining with `;`
- Custom `help` / `help <cmd>` builtin
- Append redirection `>>` (in addition to `>` / `<`)
