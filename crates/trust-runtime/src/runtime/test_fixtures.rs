//! Runtimes for unit tests of host services (simulation, Control Studio) that need process-image
//! bindings rather than a particular program.

use super::{Runtime, RuntimeBuilder};

/// A runtime without programs.
pub(crate) fn empty_runtime() -> Runtime {
    RuntimeBuilder::new()
        .build()
        .expect("admit an empty runtime")
}

/// A runtime whose program declares a located variable of each `(address, type)`, so that the
/// process image binds those addresses with those types.
pub(crate) fn bound_runtime(bindings: &[(&str, &str)]) -> Runtime {
    let vars: String = bindings
        .iter()
        .enumerate()
        .map(|(index, (address, type_name))| {
            format!("    Bound{index} AT {address} : {type_name};\n")
        })
        .collect();
    crate::harness::CompileSession::from_source(format!(
        "PROGRAM Main\nVAR\n{vars}END_VAR\nEND_PROGRAM\n"
    ))
    .build_runtime()
    .expect("compile the test bindings")
}
