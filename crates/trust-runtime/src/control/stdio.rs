//! The control link of a process that its parent drives over its standard input and output: the
//! simulator of a machine test (specification 11 §6.8.1, "Simulator for machine tests").

use std::io::{BufRead, Write};

use super::{prepare_request_line, ControlState};

/// Answers the control requests read from `input`, one JSON object per line, with one response
/// line each on `output`, as the control endpoint answers them (the runtime protocol annex, E0.1),
/// for a client on this computer: the process that holds the input. A blank line is skipped. It
/// ends at the end of the input; the runtime answers on after a `shutdown`, as every runtime does
/// (specification 34, STATE-08).
///
/// # Errors
///
/// The input cannot be read or the output written.
pub fn serve_lines(
    input: impl BufRead,
    mut output: impl Write,
    state: &ControlState,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Some(prepared) = prepare_request_line(&line, state, None) else {
            continue;
        };
        output.write_all(prepared.response().as_bytes())?;
        output.write_all(b"\n")?;
        output.flush()?;
        prepared.complete(state);
    }
    Ok(())
}
