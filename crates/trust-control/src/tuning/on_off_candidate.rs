//! The candidate of a switching (ON/OFF) controller (specification 35, C3 and B4.1): the three settings that Tune designs, unapplied. The
//! switching law has no gains and shows no margins (C0, C3), so its candidate holds the settings alone, by their symbols.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A candidate of a switching controller, unapplied (B4.1, C3): the value of each setting that Apply writes, by its symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OnOffCandidate {
    /// Always false: a candidate is never applied by Tune (N8.9).
    pub applied: bool,
    /// The hysteresis and the two minimum times, by symbol. Apply writes each one and no other setting.
    pub values: BTreeMap<String, f64>,
}

impl OnOffCandidate {
    /// Whether every value is finite, as a stored candidate must be (B10.4).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.values.values().all(|value| value.is_finite())
    }
}
