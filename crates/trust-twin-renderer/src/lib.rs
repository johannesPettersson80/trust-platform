#![forbid(unsafe_code)]

const CONTRACT_VERSION: u32 = 1;

#[must_use]
pub fn trust_twin_renderer_contract_version() -> u32 {
    CONTRACT_VERSION
}

#[must_use]
pub fn trust_twin_scale_to_unit(value: f32, min: f32, max: f32) -> f32 {
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || max <= min {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

#[must_use]
pub fn trust_twin_bool_to_visibility(value: u32) -> u32 {
    u32::from(value != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_version_is_stable_for_p5_webview_loader() {
        assert_eq!(trust_twin_renderer_contract_version(), 1);
    }

    #[test]
    fn scale_to_unit_clamps_and_rejects_invalid_ranges() {
        assert_close(trust_twin_scale_to_unit(50.0, 0.0, 100.0), 0.5);
        assert_close(trust_twin_scale_to_unit(-1.0, 0.0, 100.0), 0.0);
        assert_close(trust_twin_scale_to_unit(150.0, 0.0, 100.0), 1.0);
        assert_close(trust_twin_scale_to_unit(10.0, 100.0, 0.0), 0.0);
    }

    #[test]
    fn bool_to_visibility_is_explicit() {
        assert_eq!(trust_twin_bool_to_visibility(0), 0);
        assert_eq!(trust_twin_bool_to_visibility(1), 1);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "expected {expected}, got {actual}",
        );
    }
}
