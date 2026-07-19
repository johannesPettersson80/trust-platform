//! Case-file and artifact data models.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{StateSnapshot, TraceStep};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseFile {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub area: String,
    pub owner: String,
    pub status: String,
    pub invariant: String,
    pub case_provenance_kind: Option<String>,
    pub generator: Option<String>,
    pub generator_digest: Option<String>,
    pub trace_definition_digest: Option<String>,
    pub source_digest: String,
    pub last_reviewed: String,
    #[serde(rename = "case")]
    pub cases: Vec<CaseRecord>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRecord {
    pub id: String,
    pub family: String,
    pub input: BTreeMap<String, toml::Value>,
    pub state: Option<String>,
    pub spec_gap_ref: Option<String>,
    pub expect: Option<toml::Value>,
    pub trace: Option<Vec<TraceStep>>,
}

impl CaseRecord {
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.state.as_deref() == Some("blocked")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseResult {
    Passed,
    Failed,
    Skipped,
    Blocked,
}

#[derive(Clone, Debug)]
pub struct CaseExecution {
    pub result: CaseResult,
    pub observed_error: Option<String>,
    pub observed_status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseRunArtifact {
    pub schema_version: u32,
    pub test_id: String,
    pub case_file: String,
    pub case_file_digest: String,
    pub helper_version: String,
    pub case_provenance_kind: String,
    pub trace_definition_digest: Option<String>,
    pub trust_verify_test_id: Option<String>,
    pub trust_verify_run_id: Option<String>,
    pub trust_verify_case_file_digest: Option<String>,
    pub trust_verify_artifact_dir: Option<String>,
    pub cases: Vec<CaseArtifactEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseArtifactEntry {
    pub id: String,
    pub family: String,
    pub result: CaseResult,
    pub spec_gap_ref: Option<String>,
    pub observed_error: Option<String>,
    pub observed_status: Option<String>,
    pub state_delta: Option<String>,
    pub before: Option<StateSnapshot>,
    pub after: Option<StateSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::CaseFile;

    #[test]
    fn case_file_rejects_unknown_root_and_case_fields() {
        let root_error = toml::from_str::<CaseFile>(
            r#"schema_version = 1
id = "CASES_UNKNOWN_ROOT"
title = "Unknown root"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-12"
unreviewed_root = true

[[case]]
id = "CASE"
family = "happy_path"
input = { scenario = "RUN" }
state = "blocked"
spec_gap_ref = "SPEC_GAP"
"#,
        )
        .unwrap_err();
        assert!(root_error
            .to_string()
            .contains("unknown field `unreviewed_root`"));

        let case_error = toml::from_str::<CaseFile>(
            r#"schema_version = 1
id = "CASES_UNKNOWN_CASE"
title = "Unknown case"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-12"

[[case]]
id = "CASE"
family = "happy_path"
input = { scenario = "RUN" }
state = "blocked"
spec_gap_ref = "SPEC_GAP"
unreviewed_case = true
"#,
        )
        .unwrap_err();
        assert!(case_error
            .to_string()
            .contains("unknown field `unreviewed_case`"));
    }
}
