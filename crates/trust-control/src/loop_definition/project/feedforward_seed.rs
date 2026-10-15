//! The `feedforward-pid` seed of a program's own law, decoded (specification 35, C8): the PI's three variables, the feedforward gain's
//! variable and the lead and lag variables when the program has both. Its rules are those of `validFeedforwardSeed`: the family is PI,
//! the lead and the lag come together, and no two of the variables are the same.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::has_repeat;
use crate::tuning::project_seed::{FeedforwardPidSeed, LeadLagSymbols, ParallelPidSeed};
use crate::tuning::structure::Structure;

/// The fields of a `feedforward-pid` seed other than its kind (C8). Unknown fields are refused. The PI's integral and derivative
/// variables and the feedforward gain's variable are required; the lead and lag variables are optional together.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FeedforwardFields {
    family: Structure,
    proportional_symbol: String,
    integral_symbol: String,
    derivative_symbol: String,
    gain_symbol: String,
    #[serde(default)]
    lead_symbol: Option<String>,
    #[serde(default)]
    lag_symbol: Option<String>,
}

/// A feedforward seed (C8, `validFeedforwardSeed`): the family is PI, the PI's three variables and the feedforward gain's are named, the
/// lead and lag variables are both named or both absent, and no two variables are the same.
pub(super) fn feedforward_seed(object: &Map<String, Value>) -> Result<FeedforwardPidSeed, String> {
    let mut fields = object.clone();
    fields.remove("kind");
    let fields: FeedforwardFields = crate::json::decode_value(Value::Object(fields))
        .map_err(|error| format!("controller.project.seed: {error}"))?;
    if fields.family != Structure::PI {
        return Err("controller.project.seed of a feedforward must be of family PI".to_owned());
    }
    let lead_lag = match (fields.lead_symbol, fields.lag_symbol) {
        (Some(lead_symbol), Some(lag_symbol)) => Some(LeadLagSymbols {
            lead_symbol,
            lag_symbol,
        }),
        (None, None) => None,
        _ => {
            return Err(
                "controller.project.seed names a feedforward lead or lag without the other"
                    .to_owned(),
            )
        }
    };
    let seed = FeedforwardPidSeed {
        pid: ParallelPidSeed {
            family: Structure::PI,
            proportional_symbol: fields.proportional_symbol,
            integral_symbol: Some(fields.integral_symbol),
            derivative_symbol: Some(fields.derivative_symbol),
        },
        gain_symbol: fields.gain_symbol,
        lead_lag,
    };
    if has_repeat(&seed.variables()) {
        return Err("controller.project.seed names one variable for two gains".to_owned());
    }
    Ok(seed)
}
