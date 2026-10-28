//! The name of the selected target that Control Studio shows (specification 35, B9.2): "Simulation", "Simulation ×10",
//! "Simulation ×7 of ×10" or "Physical". The runtime words the speed in its status report (`simulation_speed_text`, computed by
//! one function in the runtime), and the engine names the target from those words. The target is simulated only when the
//! runtime's status says so (A2.4).

use crate::recording::EvidenceSource;
use crate::runtime::RuntimeStatus;

/// The name of the selected target, from the runtime's status report (B9.2). "Physical" unless simulation is enabled and every
/// attached I/O is simulated. A simulation is named by the speed words the runtime gives, and "Simulation" alone when it gives none.
#[must_use]
pub fn target_label(status: &RuntimeStatus) -> String {
    if status.source() != EvidenceSource::SimulatedPlant {
        return "Physical".to_string();
    }
    if status.simulation_speed_text.is_empty() {
        "Simulation".to_string()
    } else {
        format!("Simulation {}", status.simulation_speed_text)
    }
}

#[cfg(test)]
#[path = "target_label_tests.rs"]
mod tests;
