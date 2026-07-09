//! Test helper for generated truST verification case files.
//!
//! This crate is intentionally small and test-facing. It consumes committed
//! `verification/cases/**` files, records what a harness observed for each
//! case, and writes a machine-readable artifact for later `prove.py` checks.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const HELPER_VERSION: &str = "verification-cases v1";
const TRUST_VERIFY_TEST_ID: &str = "TRUST_VERIFY_TEST_ID";
const TRUST_VERIFY_RUN_ID: &str = "TRUST_VERIFY_RUN_ID";
const TRUST_VERIFY_CASE_FILE_DIGEST: &str = "TRUST_VERIFY_CASE_FILE_DIGEST";
const TRUST_VERIFY_ARTIFACT_DIR: &str = "TRUST_VERIFY_ARTIFACT_DIR";

#[macro_export]
macro_rules! run_case_file {
    ($config:expr, $probe:expr, $runner:expr $(,)?) => {
        $crate::run_case_file(&$config, $probe, $runner)
    };
}

#[derive(Clone, Debug)]
pub struct RunConfig {
    pub test_id: String,
    pub case_file: PathBuf,
    pub case_file_digest: String,
    pub artifact_dir: PathBuf,
}

impl RunConfig {
    #[must_use]
    pub fn new(
        test_id: impl Into<String>,
        case_file: impl Into<PathBuf>,
        case_file_digest: impl Into<String>,
    ) -> Self {
        Self {
            test_id: test_id.into(),
            case_file: case_file.into(),
            case_file_digest: case_file_digest.into(),
            artifact_dir: default_artifact_dir(),
        }
    }

    #[must_use]
    pub fn with_artifact_dir(mut self, artifact_dir: impl Into<PathBuf>) -> Self {
        self.artifact_dir = artifact_dir.into();
        self
    }
}

fn default_artifact_dir() -> PathBuf {
    workspace_root().join("target/gate-artifacts/cases")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            Path::to_path_buf,
        )
}

pub trait StateProbe {
    type Error;

    /// Capture the current observable state around a case execution.
    ///
    /// # Errors
    ///
    /// Returns an error when the harness cannot observe the state required for
    /// the artifact.
    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub process_image_hash: Option<String>,
    pub retain_hash: Option<String>,
    pub target: Option<serde_json::Value>,
    pub siblings: BTreeMap<String, serde_json::Value>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaseFile {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub area: String,
    pub owner: String,
    pub status: String,
    pub invariant: String,
    pub generator: String,
    pub generator_digest: String,
    pub source_digest: String,
    pub last_reviewed: String,
    #[serde(rename = "case")]
    pub cases: Vec<CaseRecord>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CaseRecord {
    pub id: String,
    pub family: String,
    pub input: BTreeMap<String, toml::Value>,
    pub state: Option<String>,
    pub spec_gap_ref: Option<String>,
    pub expect: Option<toml::Value>,
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

#[derive(Debug, Error)]
pub enum CaseRunError {
    #[error("failed to read case file {path}: {source}")]
    ReadCaseFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse case file {path}: {source}")]
    ParseCaseFile {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("case file digest mismatch: expected {expected}, actual {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("unsupported case file schema_version {actual}; expected 1")]
    UnsupportedSchemaVersion { actual: u32 },
    #[error("incomplete TRUST_VERIFY environment; missing {missing}")]
    IncompleteTrustVerifyStamp { missing: String },
    #[error("TRUST_VERIFY environment variable {name} is not valid UTF-8")]
    InvalidTrustVerifyStamp { name: &'static str },
    #[error("TRUST_VERIFY {name} mismatch: expected {expected}, actual {actual}")]
    TrustVerifyStampMismatch {
        name: &'static str,
        expected: String,
        actual: String,
    },
    #[error("case {case_id} has neither state = blocked nor expect")]
    InvalidRunnableState { case_id: String },
    #[error("state probe failed for case {case_id}: {message}")]
    Probe { case_id: String, message: String },
    #[error("case runner failed for case {case_id}: {message}")]
    Runner { case_id: String, message: String },
    #[error("failed to write case artifact {path}: {source}")]
    WriteArtifact {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize case artifact: {0}")]
    SerializeArtifact(#[from] serde_json::Error),
}

/// Run every case in `config.case_file` and emit a JSON case artifact.
///
/// Blocked cases are recorded without invoking `runner`. Runnable cases are
/// wrapped with before/after [`StateProbe`] snapshots.
///
/// # Errors
///
/// Returns an error if the case file cannot be read or parsed, the configured
/// digest does not match the file, the probe or runner fails, or the artifact
/// cannot be written.
pub fn run_case_file<P, E, F>(
    config: &RunConfig,
    probe: &mut P,
    mut runner: F,
) -> Result<CaseRunArtifact, CaseRunError>
where
    P: StateProbe,
    P::Error: Display,
    E: Display,
    F: FnMut(&CaseRecord, &mut P) -> Result<CaseExecution, E>,
{
    let contents = fs::read(&config.case_file).map_err(|source| CaseRunError::ReadCaseFile {
        path: config.case_file.clone(),
        source,
    })?;
    let digest = digest_bytes(&contents);
    if config.case_file_digest != digest {
        return Err(CaseRunError::DigestMismatch {
            expected: config.case_file_digest.clone(),
            actual: digest,
        });
    }
    let case_file: CaseFile =
        toml::from_slice(&contents).map_err(|source| CaseRunError::ParseCaseFile {
            path: config.case_file.clone(),
            source,
        })?;
    if case_file.schema_version != 1 {
        return Err(CaseRunError::UnsupportedSchemaVersion {
            actual: case_file.schema_version,
        });
    }
    let stamp = read_trust_verify_stamp(config, &digest)?;

    let mut entries = Vec::with_capacity(case_file.cases.len());
    for case in &case_file.cases {
        entries.push(run_one_case(case, probe, &mut runner)?);
    }

    let artifact = CaseRunArtifact {
        schema_version: 1,
        test_id: config.test_id.clone(),
        case_file: config.case_file.to_string_lossy().into_owned(),
        case_file_digest: digest,
        helper_version: HELPER_VERSION.to_string(),
        trust_verify_test_id: stamp.test_id,
        trust_verify_run_id: stamp.run_id,
        trust_verify_case_file_digest: stamp.case_file_digest,
        trust_verify_artifact_dir: stamp.artifact_dir,
        cases: entries,
    };
    write_artifact(config, &artifact)?;
    Ok(artifact)
}

/// Compute the SHA-256 digest string for a case file.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn case_file_digest(path: impl AsRef<Path>) -> Result<String, CaseRunError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| CaseRunError::ReadCaseFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(digest_bytes(&bytes))
}

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

#[derive(Default)]
struct TrustVerifyStamp {
    test_id: Option<String>,
    run_id: Option<String>,
    case_file_digest: Option<String>,
    artifact_dir: Option<String>,
}

fn read_trust_verify_stamp(
    config: &RunConfig,
    case_file_digest: &str,
) -> Result<TrustVerifyStamp, CaseRunError> {
    let test_id = read_stamp_env(TRUST_VERIFY_TEST_ID)?;
    let run_id = read_stamp_env(TRUST_VERIFY_RUN_ID)?;
    let digest = read_stamp_env(TRUST_VERIFY_CASE_FILE_DIGEST)?;
    let artifact_dir = read_stamp_env(TRUST_VERIFY_ARTIFACT_DIR)?;

    let present = [
        test_id.is_some(),
        run_id.is_some(),
        digest.is_some(),
        artifact_dir.is_some(),
    ];
    if present.iter().all(|present| !present) {
        return Ok(TrustVerifyStamp::default());
    }
    if present.iter().any(|present| !present) {
        let mut missing = Vec::new();
        if test_id.is_none() {
            missing.push(TRUST_VERIFY_TEST_ID);
        }
        if run_id.is_none() {
            missing.push(TRUST_VERIFY_RUN_ID);
        }
        if digest.is_none() {
            missing.push(TRUST_VERIFY_CASE_FILE_DIGEST);
        }
        if artifact_dir.is_none() {
            missing.push(TRUST_VERIFY_ARTIFACT_DIR);
        }
        return Err(CaseRunError::IncompleteTrustVerifyStamp {
            missing: missing.join(", "),
        });
    }

    let test_id = required_stamp_value(test_id, TRUST_VERIFY_TEST_ID)?;
    let run_id = required_stamp_value(run_id, TRUST_VERIFY_RUN_ID)?;
    let digest = required_stamp_value(digest, TRUST_VERIFY_CASE_FILE_DIGEST)?;
    let artifact_dir = required_stamp_value(artifact_dir, TRUST_VERIFY_ARTIFACT_DIR)?;

    require_stamp_match(TRUST_VERIFY_TEST_ID, &config.test_id, &test_id)?;
    require_stamp_match(TRUST_VERIFY_CASE_FILE_DIGEST, case_file_digest, &digest)?;
    let expected_artifact_dir = config.artifact_dir.to_string_lossy();
    require_stamp_match(
        TRUST_VERIFY_ARTIFACT_DIR,
        expected_artifact_dir.as_ref(),
        &artifact_dir,
    )?;

    Ok(TrustVerifyStamp {
        test_id: Some(test_id),
        run_id: Some(run_id),
        case_file_digest: Some(digest),
        artifact_dir: Some(artifact_dir),
    })
}

fn read_stamp_env(name: &'static str) -> Result<Option<String>, CaseRunError> {
    std::env::var_os(name)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| CaseRunError::InvalidTrustVerifyStamp { name })
        })
        .transpose()
}

fn require_stamp_match(
    name: &'static str,
    expected: &str,
    actual: &str,
) -> Result<(), CaseRunError> {
    if expected == actual {
        return Ok(());
    }
    Err(CaseRunError::TrustVerifyStampMismatch {
        name,
        expected: expected.to_string(),
        actual: actual.to_string(),
    })
}

fn required_stamp_value(value: Option<String>, name: &'static str) -> Result<String, CaseRunError> {
    value.ok_or_else(|| CaseRunError::IncompleteTrustVerifyStamp {
        missing: name.to_string(),
    })
}

fn run_one_case<P, E, F>(
    case: &CaseRecord,
    probe: &mut P,
    runner: &mut F,
) -> Result<CaseArtifactEntry, CaseRunError>
where
    P: StateProbe,
    P::Error: Display,
    E: Display,
    F: FnMut(&CaseRecord, &mut P) -> Result<CaseExecution, E>,
{
    if case.is_blocked() {
        return Ok(CaseArtifactEntry {
            id: case.id.clone(),
            family: case.family.clone(),
            result: CaseResult::Blocked,
            spec_gap_ref: case.spec_gap_ref.clone(),
            observed_error: None,
            observed_status: None,
            state_delta: Some("not_applicable".to_string()),
            before: None,
            after: None,
        });
    }
    if case.expect.is_none() {
        return Err(CaseRunError::InvalidRunnableState {
            case_id: case.id.clone(),
        });
    }
    let before = probe.snapshot().map_err(|error| CaseRunError::Probe {
        case_id: case.id.clone(),
        message: error.to_string(),
    })?;
    let execution = runner(case, probe).map_err(|error| CaseRunError::Runner {
        case_id: case.id.clone(),
        message: error.to_string(),
    })?;
    let after = probe.snapshot().map_err(|error| CaseRunError::Probe {
        case_id: case.id.clone(),
        message: error.to_string(),
    })?;
    let state_delta = if before == after {
        "unchanged"
    } else {
        "changed"
    };
    Ok(CaseArtifactEntry {
        id: case.id.clone(),
        family: case.family.clone(),
        result: execution.result,
        spec_gap_ref: None,
        observed_error: execution.observed_error,
        observed_status: execution.observed_status,
        state_delta: Some(state_delta.to_string()),
        before: Some(before),
        after: Some(after),
    })
}

fn write_artifact(config: &RunConfig, artifact: &CaseRunArtifact) -> Result<(), CaseRunError> {
    fs::create_dir_all(&config.artifact_dir).map_err(|source| CaseRunError::WriteArtifact {
        path: config.artifact_dir.clone(),
        source,
    })?;
    let path = config.artifact_dir.join(format!("{}.json", config.test_id));
    let bytes = serde_json::to_vec_pretty(artifact)?;
    fs::write(&path, bytes).map_err(|source| CaseRunError::WriteArtifact { path, source })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use crate::{run_case_file, CaseExecution, CaseResult, RunConfig, StateProbe, StateSnapshot};

    #[derive(Default)]
    struct Probe {
        step: u64,
    }

    impl StateProbe for Probe {
        type Error = String;

        fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
            self.step += 1;
            let mut siblings = BTreeMap::new();
            siblings.insert("sibling".to_string(), json!(self.step));
            Ok(StateSnapshot {
                process_image_hash: Some(format!("pi-{}", self.step)),
                retain_hash: Some("retain-stable".to_string()),
                target: Some(json!(self.step)),
                siblings,
                diagnostics: vec![format!("diag-{}", self.step)],
            })
        }
    }

    #[test]
    fn blocked_cases_are_recorded_without_executing_the_runner() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("blocked");
        let case_file = dir.join("blocked.toml");
        fs::write(
            &case_file,
            r#"schema_version = 1
id = "CASES_BLOCKED"
title = "Blocked cases"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV_BLOCKED"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-09"

[[case]]
id = "CASE_BLOCKED"
family = "above_max"
input = { source_partition = { above = 5 }, value = 6 }
state = "blocked"
spec_gap_ref = "SPEC_GAP_VALUE"
"#,
        )
        .unwrap();

        let digest = crate::case_file_digest(&case_file).unwrap();
        let mut probe = Probe::default();
        let config = RunConfig::new("TEST_BLOCKED", &case_file, digest)
            .with_artifact_dir(dir.join("artifacts"));
        let artifact = run_case_file!(config, &mut probe, |_, _| -> Result<_, String> {
            panic!("blocked cases must not execute");
        })
        .unwrap();

        assert_eq!(artifact.cases.len(), 1);
        assert_eq!(artifact.cases[0].result, CaseResult::Blocked);
        assert_eq!(
            artifact.cases[0].spec_gap_ref.as_deref(),
            Some("SPEC_GAP_VALUE")
        );
        assert_eq!(
            artifact.cases[0].state_delta.as_deref(),
            Some("not_applicable")
        );
        assert!(dir.join("artifacts/TEST_BLOCKED.json").exists());
    }

    #[test]
    fn runnable_cases_capture_snapshots_and_state_delta() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("runnable");
        let case_file = dir.join("runnable.toml");
        fs::write(
            &case_file,
            r#"schema_version = 1
id = "CASES_RUNNABLE"
title = "Runnable cases"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV_RUNNABLE"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-09"

[[case]]
id = "CASE_RUNNABLE"
family = "happy_path"
input = { source_partition = { equals = "RUN" }, scenario = "RUN" }
expect = { outcome = "accept_value", oracle_ref = "SPEC_RUNTIME#case" }
"#,
        )
        .unwrap();
        let digest = crate::case_file_digest(&case_file).unwrap();

        let mut probe = Probe::default();
        let config = RunConfig::new("TEST_RUNNABLE", &case_file, digest)
            .with_artifact_dir(dir.join("artifacts"));
        let artifact = run_case_file(&config, &mut probe, |case, _probe| -> Result<_, String> {
            assert_eq!(case.id, "CASE_RUNNABLE");
            Ok(CaseExecution {
                result: CaseResult::Passed,
                observed_error: None,
                observed_status: Some("ok".to_string()),
            })
        })
        .unwrap();

        let entry = &artifact.cases[0];
        assert_eq!(entry.result, CaseResult::Passed);
        assert_eq!(entry.observed_status.as_deref(), Some("ok"));
        assert_eq!(entry.state_delta.as_deref(), Some("changed"));
        assert!(entry.before.is_some());
        assert!(entry.after.is_some());
        let written = fs::read_to_string(dir.join("artifacts/TEST_RUNNABLE.json")).unwrap();
        assert!(written.contains("\"case_file_digest\""));
        assert!(written.contains("\"helper_version\""));
    }

    #[test]
    fn expected_case_file_digest_is_enforced_before_execution() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("digest");
        let case_file = dir.join("digest.toml");
        fs::write(
            &case_file,
            r#"schema_version = 1
id = "CASES_DIGEST"
title = "Digest cases"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV_DIGEST"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-09"

[[case]]
id = "CASE_BLOCKED"
family = "above_max"
input = { source_partition = { above = 5 }, value = 6 }
state = "blocked"
spec_gap_ref = "SPEC_GAP_VALUE"
"#,
        )
        .unwrap();

        let mut probe = Probe::default();
        let config = RunConfig::new("TEST_DIGEST", &case_file, "sha256:not-the-file")
            .with_artifact_dir(dir.join("artifacts"));
        let error = run_case_file(&config, &mut probe, |_, _| -> Result<_, String> {
            panic!("digest mismatch must stop before execution");
        })
        .unwrap_err();

        assert!(error.to_string().contains("case file digest mismatch"));
        assert!(!dir.join("artifacts/TEST_DIGEST.json").exists());
    }

    #[test]
    fn schema_version_mismatch_is_rejected_before_execution() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("schema");
        let case_file = dir.join("schema.toml");
        fs::write(
            &case_file,
            r#"schema_version = 2
id = "CASES_SCHEMA"
title = "Schema cases"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV_SCHEMA"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-09"

[[case]]
id = "CASE_BLOCKED"
family = "above_max"
input = { source_partition = { above = 5 }, value = 6 }
state = "blocked"
spec_gap_ref = "SPEC_GAP_VALUE"
"#,
        )
        .unwrap();
        let digest = crate::case_file_digest(&case_file).unwrap();

        let mut probe = Probe::default();
        let config = RunConfig::new("TEST_SCHEMA", &case_file, digest)
            .with_artifact_dir(dir.join("artifacts"));
        let error = run_case_file(&config, &mut probe, |_, _| -> Result<_, String> {
            panic!("schema mismatch must stop before execution");
        })
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("unsupported case file schema_version"));
        assert!(!dir.join("artifacts/TEST_SCHEMA.json").exists());
    }

    #[test]
    fn default_artifact_dir_is_workspace_target_gate_artifacts() {
        let digest = "sha256:dummy";
        let config = RunConfig::new(
            "TEST_DEFAULT_DIR",
            "verification/cases/example.toml",
            digest,
        );

        assert!(config.artifact_dir.is_absolute());
        assert!(config.artifact_dir.ends_with("target/gate-artifacts/cases"));
        assert!(config.artifact_dir.starts_with(
            env!("CARGO_MANIFEST_DIR").trim_end_matches("/crates/verification-cases")
        ));
    }

    #[test]
    fn trust_verify_env_stamps_are_recorded_in_artifact() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("verify_stamp");
        let case_file = write_blocked_case_file(&dir, "stamp.toml");
        let digest = crate::case_file_digest(&case_file).unwrap();
        let artifact_dir = dir.join("artifacts");

        std::env::set_var("TRUST_VERIFY_TEST_ID", "TEST_STAMP");
        std::env::set_var("TRUST_VERIFY_RUN_ID", "run-123");
        std::env::set_var("TRUST_VERIFY_CASE_FILE_DIGEST", &digest);
        std::env::set_var(
            "TRUST_VERIFY_ARTIFACT_DIR",
            artifact_dir.to_string_lossy().as_ref(),
        );

        let mut probe = Probe::default();
        let config = RunConfig::new("TEST_STAMP", &case_file, digest.clone())
            .with_artifact_dir(&artifact_dir);
        let artifact = run_case_file(&config, &mut probe, |_, _| -> Result<_, String> {
            panic!("blocked cases must not execute");
        })
        .unwrap();

        assert_eq!(artifact.trust_verify_test_id.as_deref(), Some("TEST_STAMP"));
        assert_eq!(artifact.trust_verify_run_id.as_deref(), Some("run-123"));
        assert_eq!(
            artifact.trust_verify_case_file_digest.as_deref(),
            Some(digest.as_str())
        );
        assert_eq!(
            artifact.trust_verify_artifact_dir.as_deref(),
            Some(artifact_dir.to_string_lossy().as_ref())
        );

        let written = fs::read_to_string(artifact_dir.join("TEST_STAMP.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(json["trust_verify_test_id"], "TEST_STAMP");
        assert_eq!(json["trust_verify_run_id"], "run-123");
        assert_eq!(json["trust_verify_case_file_digest"], digest);
        assert_eq!(
            json["trust_verify_artifact_dir"],
            artifact_dir.to_string_lossy().as_ref()
        );
    }

    #[test]
    fn partial_trust_verify_env_stamps_fail_before_execution() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("verify_partial");
        let case_file = write_runnable_case_file(&dir, "partial.toml");
        let digest = crate::case_file_digest(&case_file).unwrap();

        std::env::set_var("TRUST_VERIFY_TEST_ID", "TEST_PARTIAL");

        assert_stamp_failure_before_execution(
            &dir,
            "TEST_PARTIAL",
            &case_file,
            digest,
            "incomplete TRUST_VERIFY",
        );
    }

    #[test]
    fn mismatched_trust_verify_test_id_fails_before_execution() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("verify_test_id_mismatch");
        let case_file = write_runnable_case_file(&dir, "test_id_mismatch.toml");
        let digest = crate::case_file_digest(&case_file).unwrap();
        let artifact_dir = dir.join("artifacts");

        std::env::set_var("TRUST_VERIFY_TEST_ID", "OTHER_TEST");
        std::env::set_var("TRUST_VERIFY_RUN_ID", "run-456");
        std::env::set_var("TRUST_VERIFY_CASE_FILE_DIGEST", &digest);
        std::env::set_var(
            "TRUST_VERIFY_ARTIFACT_DIR",
            artifact_dir.to_string_lossy().as_ref(),
        );

        assert_stamp_failure_before_execution(
            &dir,
            "TEST_MISMATCH",
            &case_file,
            digest,
            "TRUST_VERIFY_TEST_ID mismatch",
        );
    }

    #[test]
    fn mismatched_trust_verify_case_file_digest_fails_before_execution() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("verify_digest_mismatch");
        let case_file = write_runnable_case_file(&dir, "digest_mismatch.toml");
        let digest = crate::case_file_digest(&case_file).unwrap();
        let artifact_dir = dir.join("artifacts");

        std::env::set_var("TRUST_VERIFY_TEST_ID", "TEST_MISMATCH");
        std::env::set_var("TRUST_VERIFY_RUN_ID", "run-456");
        std::env::set_var("TRUST_VERIFY_CASE_FILE_DIGEST", "sha256:wrong");
        std::env::set_var(
            "TRUST_VERIFY_ARTIFACT_DIR",
            artifact_dir.to_string_lossy().as_ref(),
        );

        assert_stamp_failure_before_execution(
            &dir,
            "TEST_MISMATCH",
            &case_file,
            digest,
            "TRUST_VERIFY_CASE_FILE_DIGEST mismatch",
        );
    }

    #[test]
    fn mismatched_trust_verify_artifact_dir_fails_before_execution() {
        let _env = lock_trust_verify_env();
        let dir = temp_dir("verify_artifact_dir_mismatch");
        let case_file = write_runnable_case_file(&dir, "artifact_dir_mismatch.toml");
        let digest = crate::case_file_digest(&case_file).unwrap();

        std::env::set_var("TRUST_VERIFY_TEST_ID", "TEST_MISMATCH");
        std::env::set_var("TRUST_VERIFY_RUN_ID", "run-456");
        std::env::set_var("TRUST_VERIFY_CASE_FILE_DIGEST", &digest);
        std::env::set_var(
            "TRUST_VERIFY_ARTIFACT_DIR",
            dir.join("different-artifacts").to_string_lossy().as_ref(),
        );

        assert_stamp_failure_before_execution(
            &dir,
            "TEST_MISMATCH",
            &case_file,
            digest,
            "TRUST_VERIFY_ARTIFACT_DIR mismatch",
        );
    }

    fn temp_dir(label: &str) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "verification_cases_{label}_{}_{}",
            std::process::id(),
            nanos + u128::from(id)
        ));
        if Path::new(&path).exists() {
            fs::remove_dir_all(&path).unwrap();
        }
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_blocked_case_file(dir: &Path, name: &str) -> PathBuf {
        let case_file = dir.join(name);
        fs::write(
            &case_file,
            r#"schema_version = 1
id = "CASES_BLOCKED"
title = "Blocked cases"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV_BLOCKED"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-09"

[[case]]
id = "CASE_BLOCKED"
family = "above_max"
input = { source_partition = { above = 5 }, value = 6 }
state = "blocked"
spec_gap_ref = "SPEC_GAP_VALUE"
"#,
        )
        .unwrap();
        case_file
    }

    fn write_runnable_case_file(dir: &Path, name: &str) -> PathBuf {
        let case_file = dir.join(name);
        fs::write(
            &case_file,
            r#"schema_version = 1
id = "CASES_RUNNABLE"
title = "Runnable cases"
area = "bytecode_vm"
owner = "verification"
status = "planned"
invariant = "INV_RUNNABLE"
generator = "gen_cases.py v1"
generator_digest = "sha256:generator"
source_digest = "sha256:source"
last_reviewed = "2026-07-09"

[[case]]
id = "CASE_RUNNABLE"
family = "happy_path"
input = { source_partition = { equals = "RUN" }, scenario = "RUN" }
expect = { outcome = "accept_value", oracle_ref = "SPEC_RUNTIME#case" }
"#,
        )
        .unwrap();
        case_file
    }

    fn assert_stamp_failure_before_execution(
        dir: &Path,
        test_id: &str,
        case_file: &Path,
        digest: String,
        expected_error: &str,
    ) {
        let mut probe = Probe::default();
        let mut runner_called = false;
        let config =
            RunConfig::new(test_id, case_file, digest).with_artifact_dir(dir.join("artifacts"));
        let error = run_case_file(&config, &mut probe, |_, _| -> Result<_, String> {
            runner_called = true;
            Ok(CaseExecution {
                result: CaseResult::Passed,
                observed_error: None,
                observed_status: Some("should-not-run".to_string()),
            })
        })
        .unwrap_err();

        assert!(error.to_string().contains(expected_error));
        assert_eq!(probe.step, 0);
        assert!(!runner_called);
        assert!(!dir.join(format!("artifacts/{test_id}.json")).exists());
    }

    struct TrustVerifyEnvGuard {
        _guard: MutexGuard<'static, ()>,
    }

    impl Drop for TrustVerifyEnvGuard {
        fn drop(&mut self) {
            clear_trust_verify_env();
        }
    }

    fn lock_trust_verify_env() -> TrustVerifyEnvGuard {
        let guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clear_trust_verify_env();
        TrustVerifyEnvGuard { _guard: guard }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_trust_verify_env() {
        for name in [
            "TRUST_VERIFY_TEST_ID",
            "TRUST_VERIFY_RUN_ID",
            "TRUST_VERIFY_CASE_FILE_DIGEST",
            "TRUST_VERIFY_ARTIFACT_DIR",
        ] {
            std::env::remove_var(name);
        }
    }
}
