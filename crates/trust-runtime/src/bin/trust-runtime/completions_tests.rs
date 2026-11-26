//! `trust-runtime completions` (checklist G322): it writes the completion script of a shell, a
//! reader that closes the pipe early ends the output without an error, and another write error is
//! the command's.

use std::io::{self, ErrorKind, Write};

use clap_complete::Shell;

use super::write_completions;

/// A writer that takes `room` bytes and then fails with `kind`, as a pipe whose reader has closed
/// it fails with `BrokenPipe`.
struct Closing {
    taken: Vec<u8>,
    room: usize,
    kind: ErrorKind,
}

impl Write for Closing {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.taken.len() >= self.room {
            return Err(io::Error::from(self.kind));
        }
        let take = bytes.len().min(self.room - self.taken.len());
        self.taken.extend_from_slice(&bytes[..take]);
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn the_script_is_written_whole() {
    // The bash script defines the completion function of `trust-runtime` and names its commands.
    // Catches a script that is not written or names another program.
    let mut out = Vec::new();
    write_completions(Shell::Bash, &mut out).expect("the script is written");
    let script = String::from_utf8(out).expect("text");
    assert!(
        script.starts_with("_trust__runtime() {"),
        "{}",
        &script[..60]
    );
    assert!(script.contains("completions"));
}

#[test]
fn a_reader_that_closes_the_pipe_early_ends_the_script_without_an_error() {
    // `trust-runtime completions bash | head -3`: the pipe takes the start of the script, and its
    // reader closes it before the end. Catches the panic that clap_complete raised writing into the
    // closed pipe ('failed to write completion file: ... Broken pipe', 26 November 2026).
    let mut pipe = Closing {
        taken: Vec::new(),
        room: 64,
        kind: ErrorKind::BrokenPipe,
    };
    write_completions(Shell::Bash, &mut pipe).expect("a closed pipe is the end of the output");
    assert!(pipe.taken.starts_with(b"_trust__runtime() {"));
}

#[test]
fn another_write_error_is_the_commands() {
    // A full disk is no reader that stopped: the command fails with it. Catches an error that the
    // command would swallow as it swallows a closed pipe.
    let mut disk = Closing {
        taken: Vec::new(),
        room: 64,
        kind: ErrorKind::StorageFull,
    };
    let error = write_completions(Shell::Bash, &mut disk).expect_err("the write fails");
    assert_eq!(error.kind(), ErrorKind::StorageFull);
}
