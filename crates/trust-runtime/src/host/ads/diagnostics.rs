use std::collections::BTreeMap;

use serde_json::Value as JsonValue;

/// Version of the ADS onboarding diagnostics JSON contract.
pub const ADS_DIAGNOSTICS_SCHEMA_VERSION: u32 = 2;

/// Ordered JSON object used for deterministic evidence and action payloads.
pub type DiagnosticMap = BTreeMap<String, JsonValue>;

mod report;
pub use report::*;

mod evidence;
pub use evidence::*;

mod failures;
pub use failures::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_failures;
