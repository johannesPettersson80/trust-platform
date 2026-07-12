//! Hand-authored state-machine trace provenance for case files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CaseFile, CaseRecord, CaseRunError};

pub const GENERATED_DECISION_TABLE_V1: &str = "generated_decision_table_v1";
pub const HAND_AUTHORED_STATE_MACHINE_V1: &str = "hand_authored_state_machine_v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceStep {
    pub expected: BTreeMap<String, toml::Value>,
    pub oracle_ref: String,
    pub sequence: u32,
    pub stimulus: BTreeMap<String, toml::Value>,
}

pub(crate) fn validate_case_file_provenance(
    case_file: &CaseFile,
) -> Result<(String, Option<String>), CaseRunError> {
    let kind = case_file
        .case_provenance_kind
        .as_deref()
        .unwrap_or(GENERATED_DECISION_TABLE_V1);
    match kind {
        GENERATED_DECISION_TABLE_V1 => validate_generated(case_file, kind),
        HAND_AUTHORED_STATE_MACHINE_V1 => validate_hand_authored(case_file, kind),
        other => Err(CaseRunError::InvalidCaseProvenance {
            message: format!("unknown case_provenance_kind {other:?}"),
        }),
    }
}

fn validate_generated(
    case_file: &CaseFile,
    kind: &str,
) -> Result<(String, Option<String>), CaseRunError> {
    if case_file.generator.as_deref() != Some("gen_cases.py v1")
        || case_file.generator_digest.is_none()
    {
        return Err(CaseRunError::InvalidCaseProvenance {
            message: "generated_decision_table_v1 requires gen_cases.py v1 and generator_digest"
                .to_string(),
        });
    }
    if case_file.trace_definition_digest.is_some()
        || case_file.cases.iter().any(|case| case.trace.is_some())
    {
        return Err(CaseRunError::InvalidCaseProvenance {
            message: "generated_decision_table_v1 forbids trace definitions".to_string(),
        });
    }
    Ok((kind.to_string(), None))
}

fn validate_hand_authored(
    case_file: &CaseFile,
    kind: &str,
) -> Result<(String, Option<String>), CaseRunError> {
    if case_file.generator.is_some() || case_file.generator_digest.is_some() {
        return Err(CaseRunError::InvalidCaseProvenance {
            message: "hand_authored_state_machine_v1 forbids generator provenance".to_string(),
        });
    }
    validate_trace_shapes(&case_file.cases)?;
    let expected = trace_definition_digest(&case_file.cases)?;
    if case_file.trace_definition_digest.as_deref() != Some(expected.as_str()) {
        return Err(CaseRunError::InvalidCaseProvenance {
            message: format!(
                "trace_definition_digest mismatch: expected {expected}, actual {:?}",
                case_file.trace_definition_digest
            ),
        });
    }
    Ok((kind.to_string(), Some(expected)))
}

fn validate_trace_shapes(cases: &[CaseRecord]) -> Result<(), CaseRunError> {
    for case in cases {
        if case.is_blocked() {
            if case.trace.is_some() {
                return Err(CaseRunError::InvalidCaseProvenance {
                    message: format!("blocked hand-authored case {} forbids a trace", case.id),
                });
            }
            continue;
        }
        let trace = case
            .trace
            .as_ref()
            .ok_or_else(|| CaseRunError::InvalidCaseProvenance {
                message: format!("runnable hand-authored case {} requires a trace", case.id),
            })?;
        if trace.is_empty() {
            return Err(CaseRunError::InvalidCaseProvenance {
                message: format!(
                    "runnable hand-authored case {} requires a non-empty trace",
                    case.id
                ),
            });
        }
        for (index, step) in trace.iter().enumerate() {
            if usize::try_from(step.sequence).ok() != Some(index) {
                return Err(CaseRunError::InvalidCaseProvenance {
                    message: format!(
                        "case {} trace sequence must be contiguous from zero",
                        case.id
                    ),
                });
            }
            if step.stimulus.is_empty() || step.expected.is_empty() || step.oracle_ref.is_empty() {
                return Err(CaseRunError::InvalidCaseProvenance {
                    message: format!(
                        "case {} trace steps require stimulus, expected, and oracle_ref",
                        case.id
                    ),
                });
            }
            if step.stimulus.values().any(contains_toml_float)
                || step.expected.values().any(contains_toml_float)
            {
                return Err(CaseRunError::InvalidCaseProvenance {
                    message: format!(
                        "case {} trace values must not contain TOML floats; use integer units for cross-language digest parity",
                        case.id
                    ),
                });
            }
            if !step.stimulus.values().all(is_canonical_trace_value)
                || !step.expected.values().all(is_canonical_trace_value)
            {
                return Err(CaseRunError::InvalidCaseProvenance {
                    message: format!("case {} trace values must be canonical JSON", case.id),
                });
            }
        }
    }
    Ok(())
}

fn is_canonical_trace_value(value: &toml::Value) -> bool {
    match value {
        toml::Value::String(_) | toml::Value::Integer(_) | toml::Value::Boolean(_) => true,
        toml::Value::Float(_) | toml::Value::Datetime(_) => false,
        toml::Value::Array(values) => values.iter().all(is_canonical_trace_value),
        toml::Value::Table(values) => values.values().all(is_canonical_trace_value),
    }
}

fn contains_toml_float(value: &toml::Value) -> bool {
    match value {
        toml::Value::Float(_) => true,
        toml::Value::Array(values) => values.iter().any(contains_toml_float),
        toml::Value::Table(values) => values.values().any(contains_toml_float),
        _ => false,
    }
}

#[derive(Serialize)]
struct TraceDigestRow<'a> {
    id: &'a str,
    trace: &'a Option<Vec<TraceStep>>,
}

fn trace_definition_digest(cases: &[CaseRecord]) -> Result<String, CaseRunError> {
    let rows = cases
        .iter()
        .map(|case| TraceDigestRow {
            id: &case.id,
            trace: &case.trace,
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&rows)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{digest:x}"))
}

#[cfg(test)]
mod tests {
    use crate::{CaseFile, CaseRunArtifact};

    use super::{
        trace_definition_digest, validate_case_file_provenance, HAND_AUTHORED_STATE_MACHINE_V1,
    };

    #[test]
    fn hand_authored_trace_provenance_is_artifact_ready() {
        let case_file: CaseFile = toml::from_str(
            r#"schema_version = 1
id = "CASES_TRACE"
title = "Trace cases"
area = "compiler_iec"
owner = "verification"
status = "planned"
invariant = "INV_TRACE"
case_provenance_kind = "hand_authored_state_machine_v1"
trace_definition_digest = "sha256:f3e7af49fbfc47d695088cf3cf0e4b50059c7abc1b6e4f8df50df6cd5d6725ca"
source_digest = "sha256:source"
last_reviewed = "2026-07-12"

[[case]]
id = "CASE_TRACE"
family = "happy_path"
input = { scenario = "TRACE" }
expect = { outcome = "accept_value", oracle_ref = "SPEC_TRACE#state-machine" }
trace = [
  { sequence = 0, stimulus = { input = false }, expected = { output = false }, oracle_ref = "SPEC_TRACE#state-machine" },
]
"#,
        )
        .unwrap();

        let (kind, trace_digest) = validate_case_file_provenance(&case_file).unwrap();
        assert_eq!(kind, HAND_AUTHORED_STATE_MACHINE_V1);
        assert_eq!(case_file.cases[0].trace.as_ref().unwrap()[0].sequence, 0);

        let artifact = CaseRunArtifact {
            schema_version: 1,
            test_id: "TEST_TRACE".to_string(),
            case_file: "trace.toml".to_string(),
            case_file_digest: "sha256:file".to_string(),
            helper_version: "verification-cases v1".to_string(),
            case_provenance_kind: kind,
            trace_definition_digest: trace_digest,
            trust_verify_test_id: None,
            trust_verify_run_id: None,
            trust_verify_case_file_digest: None,
            trust_verify_artifact_dir: None,
            cases: vec![],
        };
        let written = serde_json::to_string(&artifact).unwrap();
        assert!(written.contains("hand_authored_state_machine_v1"));
        assert!(
            written.contains("f3e7af49fbfc47d695088cf3cf0e4b50059c7abc1b6e4f8df50df6cd5d6725ca")
        );

        let mut stale = case_file;
        stale.trace_definition_digest = Some(format!("sha256:{}", "0".repeat(64)));
        let error = validate_case_file_provenance(&stale).unwrap_err();
        assert!(error
            .to_string()
            .contains("trace_definition_digest mismatch"));
    }

    #[test]
    fn unicode_trace_digest_matches_metadata_validator_contract() {
        let case_file: CaseFile = toml::from_str(
            r#"schema_version = 1
id = "CASES_TRACE_UNICODE"
title = "Unicode trace"
area = "compiler_iec"
owner = "verification"
status = "planned"
invariant = "INV_TRACE"
case_provenance_kind = "hand_authored_state_machine_v1"
trace_definition_digest = "sha256:e9fc05d0b2987cdaaf7e429b785bcd9e6f35aef6895e8f75c7f2a25666a414cf"
source_digest = "sha256:source"
last_reviewed = "2026-07-12"

[[case]]
id = "TRACE_UNICODE"
family = "happy_path"
input = { scenario = "TRACE_UNICODE" }
expect = { outcome = "accept_value", oracle_ref = "SPEC_TIMER#state-machine" }
trace = [
  { sequence = 0, stimulus = { label = "räknare" }, expected = { status = "klar" }, oracle_ref = "SPEC_TIMER#state-machine" },
]
"#,
        )
        .unwrap();

        let (_, digest) = validate_case_file_provenance(&case_file).unwrap();
        assert_eq!(
            digest.as_deref(),
            Some("sha256:e9fc05d0b2987cdaaf7e429b785bcd9e6f35aef6895e8f75c7f2a25666a414cf")
        );
    }

    #[test]
    fn finite_toml_float_is_rejected_before_trace_digesting() {
        let mut case_file: CaseFile = toml::from_str(
            r#"schema_version = 1
id = "CASES_TRACE_FLOAT"
title = "Float trace"
area = "compiler_iec"
owner = "verification"
status = "planned"
invariant = "INV_TRACE"
case_provenance_kind = "hand_authored_state_machine_v1"
trace_definition_digest = "sha256:placeholder"
source_digest = "sha256:source"
last_reviewed = "2026-07-12"

[[case]]
id = "TRACE_FLOAT"
family = "happy_path"
input = { scenario = "TRACE_FLOAT" }
expect = { outcome = "accept_value", oracle_ref = "SPEC_TIMER#state-machine" }
trace = [
  { sequence = 0, stimulus = { nested = [{ epsilon = 1e-7 }] }, expected = { elapsed_ns = 1 }, oracle_ref = "SPEC_TIMER#state-machine" },
]
"#,
        )
        .unwrap();
        case_file.trace_definition_digest =
            Some(trace_definition_digest(&case_file.cases).unwrap());

        let error = validate_case_file_provenance(&case_file).unwrap_err();

        assert!(error.to_string().contains("must not contain TOML floats"));
    }
}
