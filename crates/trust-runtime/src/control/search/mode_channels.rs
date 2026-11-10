//! The enumeration mode signals of an isolated run (protocol annex E8.3; specification 35 A6.1): a
//! run writes and records a loop's mode as Control Studio reads it, 1 in Manual and 0 in Auto, and
//! an enumeration's two named members stand for those numbers. A BOOL mode needs no translation:
//! its value is the number already.

use serde::{Deserialize, Serialize};

use super::program::IsolatedProgram;

/// The most mode signals one run translates: the loop's own and its further controllers'.
const MAX_MODE_CHANNELS: usize = 9;

/// An enumeration mode signal of a run: the member that means Manual and the member that means
/// Auto.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeChannel {
    pub symbol: String,
    pub manual_value: String,
    pub auto_value: String,
}

/// Checks the run's mode signals: at most nine, each symbol once (without regard to case), and two
/// different members, none of them blank.
pub(super) fn validate(channels: &[ModeChannel]) -> Result<(), String> {
    if channels.len() > MAX_MODE_CHANNELS {
        return Err(format!(
            "at most {MAX_MODE_CHANNELS} enumeration mode signals are supported"
        ));
    }
    let mut symbols = std::collections::HashSet::new();
    for channel in channels {
        if channel.symbol.trim().is_empty()
            || channel.manual_value.trim().is_empty()
            || channel.auto_value.trim().is_empty()
            || channel
                .manual_value
                .eq_ignore_ascii_case(&channel.auto_value)
            || !symbols.insert(channel.symbol.to_ascii_uppercase())
        {
            return Err(format!(
                "invalid enumeration mode signal {}: a symbol once, with two different members",
                channel.symbol
            ));
        }
    }
    Ok(())
}

/// The mode signal `symbol` names, compared without regard to case.
fn channel<'a>(channels: &'a [ModeChannel], symbol: &str) -> Option<&'a ModeChannel> {
    channels
        .iter()
        .find(|channel| channel.symbol.eq_ignore_ascii_case(symbol))
}

/// Writes `value` into `symbol`: a mode signal's 1 as its Manual member and its 0 as its Auto
/// member, and any other symbol as [`IsolatedProgram::assign`] writes it.
///
/// # Errors
///
/// A mode signal given a number other than 0 or 1, and the errors of the write.
#[allow(clippy::float_cmp)] // The mode's numbers are exactly 1 and 0.
pub(super) fn assign(
    program: &mut IsolatedProgram,
    channels: &[ModeChannel],
    symbol: &str,
    value: f64,
) -> Result<(), String> {
    let Some(channel) = channel(channels, symbol) else {
        return program.assign(symbol, value);
    };
    let member = if value == 1.0 {
        &channel.manual_value
    } else if value == 0.0 {
        &channel.auto_value
    } else {
        return Err(format!(
            "mode {symbol} takes 1 for Manual or 0 for Auto, not {value}"
        ));
    };
    program.assign_member(symbol, member)
}

/// The number `symbol` holds: a mode signal 1 in its Manual member and 0 in its Auto member, and
/// any other symbol as [`IsolatedProgram::read`] reads it.
///
/// # Errors
///
/// A mode signal in neither member, which leaves the run in neither mode, and the errors of the
/// read.
pub(super) fn read(
    program: &mut IsolatedProgram,
    channels: &[ModeChannel],
    symbol: &str,
) -> Result<f64, String> {
    let Some(channel) = channel(channels, symbol) else {
        return program.read(symbol);
    };
    let member = program.member(symbol)?;
    if member.eq_ignore_ascii_case(&channel.manual_value) {
        Ok(1.0)
    } else if member.eq_ignore_ascii_case(&channel.auto_value) {
        Ok(0.0)
    } else {
        Err(format!(
            "mode {symbol} is {member}, neither Manual nor Auto"
        ))
    }
}
