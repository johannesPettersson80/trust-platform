//! The controller structures of a parallel `Control.PID` record (numerical annex N0.5; specification 35, C0): P, PI, PD
//! and PID, and the gains a candidate writes for them. A structure is a fact about its gains, never a name.

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize};

/// Which terms a controller keeps (C0, "Families and zero terms"). A term the family does not use is written as an
/// explicit zero, so the structure follows from the gains that are not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Structure {
    /// Proportional only.
    P,
    /// Proportional and integral.
    PI,
    /// Proportional and derivative.
    PD,
    /// Proportional, integral and derivative.
    PID,
}

impl Structure {
    /// The structure a family word names (B4.4): `P`, `PI`, `PD` or `PID`. Any other word fixes none, `automatic` and the family of a
    /// state law or of a program included.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "P" => Some(Self::P),
            "PI" => Some(Self::PI),
            "PD" => Some(Self::PD),
            "PID" => Some(Self::PID),
            _ => None,
        }
    }

    /// The structure of a record whose integral and derivative gains are the given values (`candidateFromControlPid`
    /// of the TypeScript): a nonzero integral gain makes an integral term, a nonzero derivative gain a derivative term.
    #[must_use]
    pub fn of_gains(integral_gain: f64, derivative_gain: f64) -> Self {
        match (
            !is_exactly_zero(integral_gain),
            !is_exactly_zero(derivative_gain),
        ) {
            (true, true) => Self::PID,
            (true, false) => Self::PI,
            (false, true) => Self::PD,
            (false, false) => Self::P,
        }
    }

    /// Whether the structure has an integral term.
    #[must_use]
    pub fn has_integral(self) -> bool {
        matches!(self, Self::PI | Self::PID)
    }

    /// Whether the structure has a derivative term.
    #[must_use]
    pub fn has_derivative(self) -> bool {
        matches!(self, Self::PD | Self::PID)
    }

    /// Whether a law with the given terms offers the structure (C0, "Families and zero terms"): P always, PI where the law has an
    /// integral term, PD where it has a derivative term, and PID where it has both.
    #[must_use]
    pub fn offered_by_terms(self, integral_term: bool, derivative_term: bool) -> bool {
        match self {
            Self::P => true,
            Self::PI => integral_term,
            Self::PD => derivative_term,
            Self::PID => integral_term && derivative_term,
        }
    }
}

/// The parallel gains of a candidate, signed (N6.4): `u = Kp·e + Ki·∫e dt + Kd·de/dt` (N0.5). A term that the structure
/// does not use is zero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParallelGains {
    /// The proportional gain `Kp`.
    pub proportional_gain: f64,
    /// The integral gain `Ki = Kp / Ti`.
    pub integral_gain: f64,
    /// The derivative gain `Kd`.
    pub derivative_gain: f64,
}

impl ParallelGains {
    /// The structure these gains make.
    #[must_use]
    pub fn structure(self) -> Structure {
        Structure::of_gains(self.integral_gain, self.derivative_gain)
    }
}

/// Reads the optional structure of a request from the family word the page shows (B4.2, B4.4): `P`, `PI`, `PD` and `PID` fix that
/// structure, and any other word fixes none ([`Structure::of_word`]). The page sends the word of its Controller selector as it is.
///
/// # Errors
/// The field is present but is not a text.
pub fn structure_of_word<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Structure>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.and_then(|word| Structure::of_word(&word)))
}

/// The JSON Schema of the family word that fixes a structure (see [`structure_of_word`]): a text, or null. The generated type of such a
/// field is `string | null`.
#[must_use]
pub fn structure_word_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": ["string", "null"] })
}

/// Whether a value is exactly zero. The rules of Tune are about exact zero (a zero gain is refused, N5.7), so the
/// comparison is exact on purpose.
#[allow(clippy::float_cmp)] // Exact zero is the rule's meaning (N5.7, C0), not a tolerance.
#[must_use]
pub fn is_exactly_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::messages::TuneRequest;

    #[test]
    fn the_structure_follows_the_nonzero_terms() {
        assert_eq!(Structure::of_gains(0.0, 0.0), Structure::P);
        assert_eq!(Structure::of_gains(0.5, 0.0), Structure::PI);
        assert_eq!(Structure::of_gains(0.0, 0.5), Structure::PD);
        assert_eq!(Structure::of_gains(0.5, 0.5), Structure::PID);
    }

    #[test]
    fn a_family_word_fixes_a_structure_only_when_it_names_one() {
        for (word, structure) in [
            ("P", Structure::P),
            ("PI", Structure::PI),
            ("PD", Structure::PD),
            ("PID", Structure::PID),
        ] {
            assert_eq!(Structure::of_word(word), Some(structure));
        }
        for word in ["automatic", "LQR", "state-feedback", "pi", ""] {
            assert_eq!(
                Structure::of_word(word),
                None,
                "{word:?} names no structure"
            );
        }
    }

    /// A Tune request of the loop `l`, with the page's family word as its structure.
    fn request_with(structure: Option<Value>) -> Result<TuneRequest, serde_json::Error> {
        let mut request =
            json!({ "projectRoot": "/p", "runtimeId": "r", "loopId": "l", "storageFolder": "" });
        if let Some(word) = structure {
            request["structure"] = word;
        }
        serde_json::from_value(request)
    }

    #[test]
    fn a_tune_request_reads_the_family_the_page_shows_as_a_structure_or_none() {
        let structure =
            |word: Option<Value>| request_with(word).expect("the request decodes").structure;
        assert_eq!(structure(Some(json!("PI"))), Some(Structure::PI));
        assert_eq!(structure(Some(json!("PID"))), Some(Structure::PID));
        assert_eq!(
            structure(Some(json!("automatic"))),
            None,
            "Automatic fixes no structure"
        );
        assert_eq!(
            structure(Some(json!("LQR"))),
            None,
            "a state law's family is not a PID structure"
        );
        assert_eq!(structure(Some(Value::Null)), None);
        assert_eq!(structure(None), None, "an absent structure is none");
        assert!(
            request_with(Some(json!(5))).is_err(),
            "a family that is no text is refused"
        );
    }

    #[test]
    fn a_tune_request_writes_the_structure_it_fixes_and_reads_it_back() {
        let request = request_with(Some(json!("PD"))).expect("the request decodes");
        let written = serde_json::to_value(&request).expect("the request serializes");
        assert_eq!(written["structure"], json!("PD"));
        let read: TuneRequest = serde_json::from_value(written).expect("the request reads back");
        assert_eq!(read, request);
    }

    #[test]
    fn the_declaration_of_the_family_word_is_a_text_or_null() {
        let schema = schemars::SchemaGenerator::default().into_root_schema_for::<TuneRequest>();
        let schema = serde_json::to_value(&schema).expect("the schema is JSON");
        assert_eq!(
            schema["properties"]["structure"]["type"],
            json!(["string", "null"]),
            "the page sends any family word, so the generated type is a text"
        );
    }

    #[test]
    fn a_structure_names_the_terms_it_keeps() {
        assert!(Structure::PI.has_integral());
        assert!(!Structure::PI.has_derivative());
        assert!(Structure::PD.has_derivative());
        assert!(!Structure::PD.has_integral());
        assert!(!Structure::P.has_integral() && !Structure::P.has_derivative());
    }
}
