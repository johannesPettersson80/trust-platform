//! GPIO (specification 11, "comm.discover" and "Sources"): the lines that the GPIO chips of the
//! runtime's computer expose. They are listed only inside a runtime: `comm.discover` and the
//! search check that before they ask.

use serde_json::json;

use crate::io::GpioLine;

use super::super::model::{DeviceKey, GpioIdentity, Identity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{find, number, text, FindSink};
use super::report::{sanitize_id, CandidateSink, Deadline, DiscoverCandidate};

/// Lists the GPIO lines of this computer.
pub(crate) fn discover(deadline: Deadline, sink: &mut dyn CandidateSink) {
    if deadline.passed() {
        return;
    }
    match crate::io::discover_gpio_lines() {
        Ok(lines) if lines.is_empty() => {
            sink.warn("No GPIO chip lines were exposed by this runtime host.".to_string());
        }
        Ok(lines) => report(lines, sink),
        Err(error) => sink.warn(format!("GPIO discovery did not complete: {error}")),
    }
}

/// Reports one already-listed set of GPIO lines (specification 11, "Sources" and "Identity"): each
/// line's chip, the chip's label, its offset within the chip, its name, its consumer, whether it is
/// in use, and its direction. A line the operating system lists is `confirmed` (specification 11,
/// "Confidence"). A line from the sysfs fallback carries a warning that says so.
pub(crate) fn report(lines: Vec<GpioLine>, sink: &mut dyn CandidateSink) {
    for line in lines {
        let id = format!("gpio:{}:{}", sanitize_id(line.chip.as_str()), line.line);
        let label = line.label.as_ref().map_or_else(
            || format!("{} line {}", line.chip, line.line),
            |label| format!("{label} · line {}", line.line),
        );
        let warnings = if line.from_sysfs {
            vec![
                "This GPIO line was listed through the sysfs fallback; the kernel exposes no GPIO character device.".to_string(),
            ]
        } else {
            Vec::new()
        };
        sink.found(DiscoverCandidate {
            id,
            label,
            source: "gpio_sysfs",
            confidence: "confirmed",
            params: json!({
                "chip": line.chip,
                "label": line.label,
                "line": line.line,
                "name": line.name,
                "consumer": line.consumer,
                "used": line.used,
                "direction": line.direction,
            }),
            warnings,
        });
    }
}

/// GPIO in a search: the lines of the runtime's computer.
pub(super) fn search(sink: &SourceSink) {
    let mut found = FindSink::new(sink, None, gpio_find);
    discover(found.deadline(), &mut found);
}

fn gpio_find(candidate: DiscoverCandidate) -> ProtocolFind {
    let chip = text(&candidate.params, "chip").unwrap_or_default();
    let line = number(&candidate.params, "line");
    let identity = Identity::Gpio(GpioIdentity {
        chip: chip.clone(),
        chip_label: text(&candidate.params, "label"),
        line,
        direction: text(&candidate.params, "direction"),
    });
    find(SourceId::Gpio, candidate, identity).keyed(
        format!("{chip}:{line}"),
        vec![DeviceKey::Local(format!("gpio:{chip}"))],
    )
}

#[cfg(test)]
#[path = "gpio_tests.rs"]
mod tests;
