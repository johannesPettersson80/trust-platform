use super::super::ControlState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Source {
    pub path: String,
    pub text: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Snapshot {
    pub project_revision: String,
    pub project_identity: String,
    pub sources: Vec<Source>,
    pub simulation: String,
    /// The live runtime's bound inputs when the run was requested, written into the isolated
    /// runtime before its first scan. Kept out of `project_revision`, which names the project and
    /// must not change with the plant between review and start; `process_inputs_revision` is
    /// their own digest.
    #[serde(default)]
    pub process_inputs: Vec<super::process_inputs::ProcessInput>,
    #[serde(default)]
    pub process_inputs_revision: String,
}
/// The refusal for an isolated simulation that would run on a physical target's host (A2.6).
pub(super) const PHYSICAL_HOST_REFUSAL: &str =
    "Isolated simulations do not run on a physical target's host; use a local analysis runtime.";

/// Admits an isolated simulation only when the runtime itself reports simulation enabled
/// (`status.active`) and simulation-only I/O, however it was started (A2.4, A2.6). An absent
/// native host answer is not simulated.
pub(super) fn admit_simulated_host(
    native: Option<&crate::simulation::SimulationRuntimeSnapshot>,
) -> Result<(), String> {
    match native {
        Some(native) if native.status.active && native.simulation_only_io == Some(true) => Ok(()),
        _ => Err(PHYSICAL_HOST_REFUSAL.into()),
    }
}

pub(super) fn digest(value: impl std::fmt::Debug) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(format!("{value:?}").as_bytes())
    )
}
pub(super) fn load(state: &ControlState) -> Result<Snapshot, String> {
    if !super::limits::available() {
        return Err(
            "bounded search requires a native process-memory-limit adapter; currently Linux only"
                .into(),
        );
    }
    let root = state
        .project_root
        .as_ref()
        .ok_or("active project path is unavailable")?;
    let (sender, receiver) = std::sync::mpsc::channel();
    state
        .resource
        .send_command(crate::scheduler::ResourceCommand::SimulationSnapshot { respond_to: sender })
        .map_err(|e| e.to_string())?;
    let native = receiver
        .recv_timeout(std::time::Duration::from_millis(250))
        .map_err(|_| "native simulation host status timed out")?;
    admit_simulated_host(native.as_ref())?;
    let files = state.sources.files();
    if files.is_empty()
        || files.len() > 64
        || files.iter().map(|file| file.text.len()).sum::<usize>() > 1_048_576
    {
        return Err("search requires 1 to 64 loaded source files totaling at most 1MiB".into());
    }
    let mut simulation = String::new();
    std::fs::File::open(root.join("simulation.toml"))
        .map_err(|e| format!("simulation.toml unavailable: {e}"))?
        .take(262_145)
        .read_to_string(&mut simulation)
        .map_err(|e| e.to_string())?;
    if simulation.len() > 262_144 {
        return Err("simulation configuration exceeds 256KiB".into());
    }
    let config = crate::simulation::SimulationConfig::from_toml_text(&simulation, "control search")
        .map_err(|e| e.to_string())?;
    if !config.enabled || config.plant_system.plants.is_empty() {
        return Err("search requires an enabled configured simulated plant".into());
    }
    let sources: Vec<_> = files
        .iter()
        .map(|file| Source {
            path: file.path.to_string_lossy().into_owned(),
            text: file.text.clone(),
        })
        .collect();
    let project_identity = root
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();
    let project_revision = digest((&project_identity, &sources, &simulation));
    let process_inputs = super::process_inputs::capture(
        state
            .io_snapshot
            .lock()
            .map_err(|_| "runtime I/O snapshot unavailable")?
            .as_ref(),
    );
    let process_inputs_revision = digest(&process_inputs);
    Ok(Snapshot {
        project_revision,
        project_identity,
        sources,
        simulation,
        process_inputs,
        process_inputs_revision,
    })
}
