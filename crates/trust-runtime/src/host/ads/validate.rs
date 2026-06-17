use std::fmt;

use trust_ads_core::SymbolSnapshot;

use super::contracts::AdsClientConfig;
use super::generate::{generate_ads_interface, AdsInterfaceError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsOfflineValidationReport {
    pub point_count: usize,
    pub generated_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdsValidationError {
    Interface(AdsInterfaceError),
    GeneratedSourceMismatch {
        expected: String,
        actual: String,
        first_difference: Option<GeneratedSourceDifference>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSourceDifference {
    pub line: usize,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for AdsValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interface(error) => write!(f, "{error}"),
            Self::GeneratedSourceMismatch {
                first_difference: Some(diff),
                ..
            } => write!(
                f,
                "generated ADS ST source does not match ads.toml + snapshot; first difference at line {}: expected {:?}, got {:?}",
                diff.line, diff.expected, diff.actual
            ),
            Self::GeneratedSourceMismatch {
                first_difference: None,
                ..
            } => write!(
                f,
                "generated ADS ST source does not match ads.toml + snapshot; source lengths differ"
            ),
        }
    }
}

impl std::error::Error for AdsValidationError {}

impl From<AdsInterfaceError> for AdsValidationError {
    fn from(value: AdsInterfaceError) -> Self {
        Self::Interface(value)
    }
}

pub fn validate_ads_interface_offline(
    config: &AdsClientConfig,
    snapshots: &[SymbolSnapshot],
    generated_source: &str,
) -> Result<AdsOfflineValidationReport, AdsValidationError> {
    let expected = generate_ads_interface(config, snapshots)?;
    if expected.source != generated_source {
        return Err(AdsValidationError::GeneratedSourceMismatch {
            first_difference: first_difference(expected.source.as_str(), generated_source),
            expected: expected.source,
            actual: generated_source.to_string(),
        });
    }
    Ok(AdsOfflineValidationReport {
        point_count: expected.point_count,
        generated_bytes: generated_source.len(),
    })
}

fn first_difference(expected: &str, actual: &str) -> Option<GeneratedSourceDifference> {
    for (index, (expected_line, actual_line)) in expected.lines().zip(actual.lines()).enumerate() {
        if expected_line != actual_line {
            return Some(GeneratedSourceDifference {
                line: index + 1,
                expected: expected_line.to_string(),
                actual: actual_line.to_string(),
            });
        }
    }

    let expected_count = expected.lines().count();
    let actual_count = actual.lines().count();
    if expected_count == actual_count {
        return None;
    }
    Some(GeneratedSourceDifference {
        line: expected_count.min(actual_count) + 1,
        expected: expected.lines().nth(actual_count).unwrap_or("").to_string(),
        actual: actual.lines().nth(expected_count).unwrap_or("").to_string(),
    })
}
