//! The seeds of a PID group's program law, decoded (specification 35, C7; `projectController.ts`, `validDecoupledSeed`). A
//! `decentralized-pid` seed names nothing beyond its family: each member's PI is designed on the group model, and the members'
//! gain variables come from the group's members. A `decoupled-pid` seed names the static decoupler's square matrix of program
//! variables `D[i][j]` (plant input `i`, member `j`), each variable once, at most eight by eight. Both seeds have the family PI.

use serde_json::{Map, Value};

use super::has_repeat;

/// The most rows, and the most columns, of a decoupler (`validDecoupledSeed`).
pub const MAXIMUM_DECOUPLER_SIZE: usize = 8;

/// The refusal of a group seed whose family is not PI or whose fields are not those of the seed.
const DECENTRALIZED_FIELDS: &str =
    "controller.project.seed of kind decentralized-pid holds only its family, PI.";
/// The refusal of a decoupler seed whose matrix is not square, or is empty, or is larger than eight by eight.
const DECOUPLER_SHAPE: &str =
    "controller.project.seed of kind decoupled-pid needs a square matrixSymbols of at most eight by eight.";
/// The refusal of a decoupler seed that names one variable twice.
const DECOUPLER_REPEAT: &str =
    "controller.project.seed names one variable for two decoupler entries.";

/// A `decoupled-pid` seed (C7): the static decoupler's variables, `D[i][j]` for plant input `i` and member `j`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoupledPidSeed {
    /// The variables by row (plant input) and column (member), each a bound parameter path.
    pub matrix_symbols: Vec<Vec<String>>,
}

impl DecoupledPidSeed {
    /// The decoupler's variables, row by row.
    #[must_use]
    pub fn variables(&self) -> Vec<&str> {
        self.matrix_symbols
            .iter()
            .flatten()
            .map(String::as_str)
            .collect()
    }
}

/// Reads a `decentralized-pid` seed: its only field besides the kind is the family PI.
///
/// # Errors
/// The family is not PI, or the seed holds another field.
pub(super) fn decentralized(object: &Map<String, Value>) -> Result<(), String> {
    if object.get("family").and_then(Value::as_str) == Some("PI") && object.len() == 2 {
        Ok(())
    } else {
        Err(DECENTRALIZED_FIELDS.to_owned())
    }
}

/// Reads a `decoupled-pid` seed: the family PI, and a square matrix of distinct, non-empty variables.
///
/// # Errors
/// The family is not PI, the matrix is not square or is too large, or one variable names two entries.
pub(super) fn decoupled(object: &Map<String, Value>) -> Result<DecoupledPidSeed, String> {
    if object.get("family").and_then(Value::as_str) != Some("PI") {
        return Err(DECOUPLER_SHAPE.to_owned());
    }
    let rows = object
        .get("matrixSymbols")
        .and_then(Value::as_array)
        .filter(|rows| (1..=MAXIMUM_DECOUPLER_SIZE).contains(&rows.len()))
        .ok_or_else(|| DECOUPLER_SHAPE.to_owned())?;
    let matrix_symbols = rows
        .iter()
        .map(|row| square_row(row, rows.len()))
        .collect::<Option<Vec<Vec<String>>>>()
        .ok_or_else(|| DECOUPLER_SHAPE.to_owned())?;
    let flat: Vec<&str> = matrix_symbols
        .iter()
        .flatten()
        .map(String::as_str)
        .collect();
    if has_repeat(&flat) {
        return Err(DECOUPLER_REPEAT.to_owned());
    }
    Ok(DecoupledPidSeed { matrix_symbols })
}

/// One row of a decoupler: `width` non-empty texts, or `None`.
fn square_row(row: &Value, width: usize) -> Option<Vec<String>> {
    let symbols = row.as_array().filter(|symbols| symbols.len() == width)?;
    symbols
        .iter()
        .map(|symbol| {
            symbol
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .map(str::to_owned)
        })
        .collect()
}
