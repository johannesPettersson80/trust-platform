//! Compiled programs for unit tests of host services (simulation, Control Studio) that need a
//! runtime or process-image bindings rather than a particular program; they are built with the
//! toolchain at `TRUST_TOOLCHAIN`, as every compiled program of a test is (specification 44 §14.2).

use super::CompiledRuntime;
use crate::harness::TestHarness;

/// A runtime whose one program does nothing.
pub(crate) fn empty_runtime() -> CompiledRuntime {
    TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .expect("build an empty program")
        .into_runtime()
}

/// A runtime whose program declares a located variable of each `(address, type)`, so that the
/// process image binds those addresses with those types.
pub(crate) fn bound_runtime(bindings: &[(&str, &str)]) -> CompiledRuntime {
    let vars: String = bindings
        .iter()
        .enumerate()
        .map(|(index, (address, type_name))| {
            format!("    Bound{index} AT {address} : {type_name};\n")
        })
        .collect();
    TestHarness::from_source(&format!("PROGRAM Main\nVAR\n{vars}END_VAR\nEND_PROGRAM\n"))
        .expect("compile the test bindings")
        .into_runtime()
}
