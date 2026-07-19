//! Same-run environment stamp validation for case artifacts.

use crate::{CaseRunError, RunConfig};

const TRUST_VERIFY_TEST_ID: &str = "TRUST_VERIFY_TEST_ID";
const TRUST_VERIFY_RUN_ID: &str = "TRUST_VERIFY_RUN_ID";
const TRUST_VERIFY_CASE_FILE_DIGEST: &str = "TRUST_VERIFY_CASE_FILE_DIGEST";
const TRUST_VERIFY_ARTIFACT_DIR: &str = "TRUST_VERIFY_ARTIFACT_DIR";

#[derive(Default)]
pub(crate) struct TrustVerifyStamp {
    pub(crate) test_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) case_file_digest: Option<String>,
    pub(crate) artifact_dir: Option<String>,
}

pub(crate) fn read_trust_verify_stamp(
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
