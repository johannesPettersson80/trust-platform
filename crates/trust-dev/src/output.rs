//! What a `trust-dev` command prints on the standard output (specification 22, "trust-dev
//! output"), by the rule of `trust-runtime`'s (specification 11, "CLI entrypoint").
//!
//! A reader that stops reading and closes the pipe, as `head` does, is not an error: the rest of the
//! output is dropped, and the command carries on to its own end and exit status. A listing then
//! completes, and a change such as a commit or a service's installation completes instead of
//! stopping halfway. Rust ignores SIGPIPE, so a write to a closed pipe is an error, which
//! `println!` turns into a panic. Every other write error, such as a full disk, is still that panic.
//!
//! `outln!` and `out!` take the arguments of `println!` and `print!`. `main.rs` declares this module
//! first, so that every module after it has them.

use std::fmt;
use std::io::{self, ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once the reader has closed the pipe: nothing is written after it.
static CLOSED: AtomicBool = AtomicBool::new(false);

/// Prints `args` on the standard output, with a line end after them when `line` is set.
pub(crate) fn print(args: fmt::Arguments<'_>, line: bool) {
    write_to(&mut io::stdout().lock(), &CLOSED, args, line);
}

fn write_to(out: &mut impl Write, closed: &AtomicBool, args: fmt::Arguments<'_>, line: bool) {
    if closed.load(Ordering::Relaxed) {
        return;
    }
    let written = out
        .write_fmt(args)
        .and_then(|()| if line { out.write_all(b"\n") } else { Ok(()) });
    match written {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::BrokenPipe => {
            closed.store(true, Ordering::Relaxed)
        }
        Err(error) => panic!("failed printing to stdout: {error}"),
    }
}

/// `println!`, except that a reader that closed the pipe ends the output quietly (the module's
/// documentation).
macro_rules! outln {
    () => {
        $crate::output::print(format_args!(""), true)
    };
    ($($arg:tt)*) => {
        $crate::output::print(format_args!($($arg)*), true)
    };
}

/// `print!`, except that a reader that closed the pipe ends the output quietly (the module's
/// documentation).
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::output::print(format_args!($($arg)*), false)
    };
}

#[cfg(test)]
#[path = "output/tests.rs"]
mod tests;
