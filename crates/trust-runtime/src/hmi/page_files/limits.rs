//! Display-only alarm thresholds for bars and gauges (specification 39 §5.9).
use serde::{Deserialize, Serialize};

/// Manual fallback thresholds, separate from the program's alarm state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", try_from = "RawLimits")]
pub struct HmiLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Lower outer threshold, drawn in the High-priority colour.
    pub low_low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Lower normal-range threshold, drawn in the Medium-priority colour.
    pub low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Upper normal-range threshold, drawn in the Medium-priority colour.
    pub high: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Upper outer threshold, drawn in the High-priority colour.
    pub high_high: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct RawLimits {
    low_low: Option<f64>,
    low: Option<f64>,
    high: Option<f64>,
    high_high: Option<f64>,
}

impl TryFrom<RawLimits> for HmiLimits {
    type Error = &'static str;

    fn try_from(raw: RawLimits) -> Result<Self, Self::Error> {
        let mut previous = None;
        for value in [raw.low_low, raw.low, raw.high, raw.high_high]
            .into_iter()
            .flatten()
        {
            if !value.is_finite() || previous.is_some_and(|previous| value < previous) {
                return Err("limits must be finite and ordered");
            }
            previous = Some(value);
        }
        Ok(Self {
            low_low: raw.low_low,
            low: raw.low,
            high: raw.high,
            high_high: raw.high_high,
        })
    }
}
