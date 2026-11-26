//! What a command prints on the standard output (specification 11, "CLI entrypoint").
//!
//! A reader that stops reading and closes the pipe, as `head` does, is not an error: the rest of the
//! output is dropped, and the command carries on to its own end and exit status. A listing then
//! completes, a change such as a deploy completes instead of stopping halfway, and a running runtime
//! keeps running while its log lines go nowhere. Rust ignores SIGPIPE, so a write to a closed pipe is
//! an error, which `println!` turns into a panic. Every other write error, such as a full disk, is
//! still that panic.
//!
//! `outln!` and `out!` take the arguments of `println!` and `print!`. The binary's root declares this
//! module first, so that every module after it has them.

use std::fmt;
use std::io::{self, ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once the reader has closed the pipe: nothing is written after it.
static CLOSED: AtomicBool = AtomicBool::new(false);

/// Prints `args` on the standard output, with a line end after them when `line` is set.
pub(crate) fn print(args: fmt::Arguments<'_>, line: bool) {
    write_to(&mut io::stdout().lock(), &CLOSED, args, line);
}

/// Flushes the standard output, as a prompt does before it reads the answer. A closed pipe is not an
/// error here either.
pub(crate) fn flush() -> io::Result<()> {
    flush_to(&mut io::stdout().lock(), &CLOSED)
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

fn flush_to(out: &mut impl Write, closed: &AtomicBool) -> io::Result<()> {
    if closed.load(Ordering::Relaxed) {
        return Ok(());
    }
    match out.flush() {
        Err(error) if error.kind() == ErrorKind::BrokenPipe => {
            closed.store(true, Ordering::Relaxed);
            Ok(())
        }
        flushed => flushed,
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
