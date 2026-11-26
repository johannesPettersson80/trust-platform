//! Shell completions generator.

use std::io::{self, ErrorKind, Write};

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;

/// Prints the completion script of `trust-runtime` for `shell` to the standard output.
pub fn run_completions(shell: Shell) -> anyhow::Result<()> {
    write_completions(shell, &mut io::stdout().lock())?;
    Ok(())
}

/// Writes the completion script of `trust-runtime` for `shell` to `out`. A reader that closes the
/// pipe before the script's end, as `head` does, ends the output without an error: it stopped
/// reading, and the script is no less written. Another write error is returned.
fn write_completions(shell: Shell, out: &mut impl Write) -> io::Result<()> {
    let mut script = Vec::new();
    generate(shell, &mut Cli::command(), "trust-runtime", &mut script);
    match out.write_all(&script).and_then(|()| out.flush()) {
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        written => written,
    }
}

#[cfg(test)]
#[path = "completions_tests.rs"]
mod tests;
