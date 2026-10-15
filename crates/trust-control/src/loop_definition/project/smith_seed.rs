//! The `smith-pid` seed of a program's own law, decoded (specification 35, C9): the PI's three gain variables, the variables of the
//! model's gain, time constant and delay, and, when the program uses `Control.SmithPredictorFOPDT`, the block's four variables together.
//! Its rules are those of `validSmithSeed`: the family is PI, the block's four variables are all named or all absent, and no two of the
//! variables are the same.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::has_repeat;
use crate::tuning::project_seed::{ParallelPidSeed, SmithLibraryBlock, SmithPidSeed};
use crate::tuning::structure::Structure;

/// The fields of a `smith-pid` seed other than its kind (C9). Unknown fields are refused. The PI's three variables and the model's three
/// are required; the library block's four are optional together.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SmithFields {
    family: Structure,
    proportional_symbol: String,
    integral_symbol: String,
    derivative_symbol: String,
    gain_symbol: String,
    time_constant_symbol: String,
    delay_symbol: String,
    #[serde(default)]
    predictor_output_symbol: Option<String>,
    #[serde(default)]
    maximum_correction_symbol: Option<String>,
    #[serde(default)]
    operating_input_symbol: Option<String>,
    #[serde(default)]
    operating_output_symbol: Option<String>,
}

/// A Smith predictor's seed (C9, `validSmithSeed`): the family is PI, the library block's four variables are named together or not at
/// all, and no two of the variables are the same.
///
/// # Errors
/// The fields do not decode, the family is not PI, the library block is partly named, or two variables are the same.
pub(super) fn smith_seed(object: &Map<String, Value>) -> Result<SmithPidSeed, String> {
    let mut fields = object.clone();
    fields.remove("kind");
    let fields: SmithFields = crate::json::decode_value(Value::Object(fields))
        .map_err(|error| format!("controller.project.seed: {error}"))?;
    if fields.family != Structure::PI {
        return Err("controller.project.seed of a Smith predictor must be of family PI".to_owned());
    }
    let library_block = library_block(&fields)?;
    let seed = SmithPidSeed {
        pid: ParallelPidSeed {
            family: Structure::PI,
            proportional_symbol: fields.proportional_symbol,
            integral_symbol: Some(fields.integral_symbol),
            derivative_symbol: Some(fields.derivative_symbol),
        },
        gain_symbol: fields.gain_symbol,
        time_constant_symbol: fields.time_constant_symbol,
        delay_symbol: fields.delay_symbol,
        library_block,
    };
    if repeats(&seed) {
        return Err("controller.project.seed names one variable for two of its terms".to_owned());
    }
    Ok(seed)
}

/// Whether two of the seed's variables are the same, the library block's result and maximum included.
fn repeats(seed: &SmithPidSeed) -> bool {
    let mut names = seed.variables();
    if let Some(block) = &seed.library_block {
        names.push(block.output_symbol.as_str());
        names.push(block.maximum_correction_symbol.as_str());
    }
    has_repeat(&names)
}

/// The library block's four variables: all named, or none (C9).
///
/// # Errors
/// Some of the four variables are named and others are not.
fn library_block(fields: &SmithFields) -> Result<Option<SmithLibraryBlock>, String> {
    match (
        &fields.predictor_output_symbol,
        &fields.maximum_correction_symbol,
        &fields.operating_input_symbol,
        &fields.operating_output_symbol,
    ) {
        (None, None, None, None) => Ok(None),
        (Some(output), Some(maximum), Some(input), Some(output_point)) => {
            Ok(Some(SmithLibraryBlock {
                output_symbol: output.clone(),
                maximum_correction_symbol: maximum.clone(),
                operating_input_symbol: input.clone(),
                operating_output_symbol: output_point.clone(),
            }))
        }
        _ => Err(
            "controller.project.seed names some of the library block's variables without the others"
                .to_owned(),
        ),
    }
}
