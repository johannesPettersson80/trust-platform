//! The scenario records of Verify and their structural contract (specification 35, B5 and C1; `TypeScript`
//! `scenarioContract.ts`). A scenario starts from its own initial values, writes timed events, applies its effects and
//! checks its own requirements. The campaign that runs the scenarios is ported with the campaign; the native effects and
//! the compatibility of the requirements with the plant are checked there.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identification::connected_record::RuntimeVerificationResult;
use crate::validation::engineer_scenario_effects::EngineerScenarioEffect;
use crate::validation::run_validation::RunRequirement;

/// The most scenarios one campaign runs (`validEngineerScenarios`).
const MAXIMUM_SCENARIOS: usize = 24;
/// The most timed events one scenario writes (`validEngineerScenarios`).
const MAXIMUM_EVENTS: usize = 12_000;
/// The most requirements one scenario checks; a scenario checks at least one (`validEngineerScenarios`).
const MAXIMUM_REQUIREMENTS: usize = 64;
/// The identity of the nominal run, which no scenario may reuse (`validEngineerScenarios`).
const TARGET_RESPONSE: &str = "target-response";

/// One scenario of a verification campaign (`TypeScript` `EngineerScenario`): the values it starts from, the timed events
/// it writes, the effects it applies and the requirements it checks. The stored form is `camelCase`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineerScenario {
    /// The scenario's identity: unique within its campaign, and not `target-response`.
    pub id: String,
    /// The plant model the scenario runs on, when it is not the selected one; the campaign decodes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<Value>,
    /// The operating outputs of that model, when the scenario names them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_outputs: Option<Vec<f64>>,
    /// The effects the scenario applies to the plant: disturbances, noise and parameter variations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<Vec<EngineerScenarioEffect>>,
    /// The values the run starts from, by symbol.
    pub initial_values: BTreeMap<String, f64>,
    /// The timed writes of the run.
    pub events: Vec<ScenarioEvent>,
    /// The requirements the scenario checks.
    pub requirements: Vec<RunRequirement>,
}

/// One timed write of a scenario (an entry of `TypeScript` `events`): a value written to a symbol at a time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioEvent {
    /// The time of the write, in ms from the start of the run; never negative.
    pub at_ms: f64,
    /// The variable written.
    pub symbol: String,
    /// The value written.
    pub value: f64,
}

/// The outcome of one scenario's run (`TypeScript` `EngineerScenarioResult`): the scenario's identity and the connected
/// record of its run, with the evaluation `E` that the campaign made of it.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult<E> {
    /// The scenario's identity.
    pub id: String,
    /// The connected record of the run.
    pub result: RuntimeVerificationResult<E>,
}

/// The declared check that ends last (`TypeScript` `latestScenarioCheck`): the validation run must reach its window's end.
#[derive(Debug, Clone, PartialEq)]
pub struct LatestCheck {
    /// The scenario that holds the check.
    pub scenario: String,
    /// The check's requirement identity.
    pub requirement: String,
    /// The end of the check's window, in seconds.
    pub end_seconds: f64,
}

/// Whether the campaign's scenarios have the structure Verify runs (`TypeScript` `validEngineerScenarios`): at most 24
/// scenarios with distinct identities, none of them `target-response`; finite initial values and event values; named
/// symbols; event times from zero; at most 12,000 events; and from one to 64 requirements each. The effects and the
/// compatibility of the requirements with the plant are checked by the campaign.
#[must_use]
pub fn valid_engineer_scenarios(scenarios: &[EngineerScenario]) -> bool {
    let identities: BTreeSet<&str> = scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect();
    scenarios.len() <= MAXIMUM_SCENARIOS
        && identities.len() == scenarios.len()
        && scenarios.iter().all(valid_scenario)
}

/// One scenario's own structure, as `validEngineerScenarios` checks it for each scenario.
fn valid_scenario(scenario: &EngineerScenario) -> bool {
    !scenario.id.is_empty()
        && scenario.id != TARGET_RESPONSE
        && scenario
            .initial_values
            .iter()
            .all(|(symbol, value)| !symbol.is_empty() && value.is_finite())
        && scenario.events.len() <= MAXIMUM_EVENTS
        && scenario.events.iter().all(valid_event)
        && (1..=MAXIMUM_REQUIREMENTS).contains(&scenario.requirements.len())
}

/// One timed write's own structure: a time from zero, a named symbol and a finite value.
fn valid_event(event: &ScenarioEvent) -> bool {
    event.at_ms.is_finite()
        && event.at_ms >= 0.0
        && !event.symbol.is_empty()
        && event.value.is_finite()
}

/// The declared check that ends last (`TypeScript` `latestScenarioCheck`): the requirement with the greatest finite window
/// end, scanning the scenarios and their requirements in order, so an earlier check keeps a tie.
#[must_use]
pub fn latest_scenario_check(scenarios: &[EngineerScenario]) -> Option<LatestCheck> {
    let mut latest: Option<LatestCheck> = None;
    for scenario in scenarios {
        for requirement in &scenario.requirements {
            let end = requirement.window.end;
            if end.is_finite() && latest.as_ref().is_none_or(|known| end > known.end_seconds) {
                latest = Some(LatestCheck {
                    scenario: scenario.id.clone(),
                    requirement: requirement.id.clone(),
                    end_seconds: end,
                });
            }
        }
    }
    latest
}

#[cfg(test)]
#[path = "scenario_contract_tests.rs"]
mod tests;
