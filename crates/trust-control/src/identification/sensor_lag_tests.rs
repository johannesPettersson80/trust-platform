//! Tests of the sensor-lag rule: the declared default sensor-time grid, the order and conditions of the warnings, and the
//! sentence of each refusal of the search. The grid search itself is tested in `trust_control_math::sensor_lag`.

#![allow(clippy::float_cmp)] // The default grid is compared with the values it is declared with.

use trust_control_math::sensor_lag::SensorLagError;

use super::{refusal_sentence, warnings, DEFAULT_SENSOR_TIME_CONSTANT};
use crate::identification::sensor_lag_texts::{
    GRID_BOUNDARY_WARNING, GRID_TOO_LARGE, HYPOTHESIS_WARNING, NO_CANDIDATE,
    NO_CONSISTENT_POINT_WARNING, SEARCH_BOUNDARY_WARNING, SEARCH_TOO_LARGE,
    TIMESTAMP_SHIFT_WARNING,
};

#[test]
fn the_default_sensor_grid_runs_from_0_1_to_10_seconds_in_steps_of_0_1() {
    assert_eq!(DEFAULT_SENSOR_TIME_CONSTANT.minimum, 0.1);
    assert_eq!(DEFAULT_SENSOR_TIME_CONSTANT.maximum, 10.0);
    assert_eq!(DEFAULT_SENSOR_TIME_CONSTANT.step, 0.1);
}

#[test]
fn warnings_come_in_the_declared_order_and_follow_their_conditions() {
    assert_eq!(
        warnings(false, false, 5, 0.0),
        vec![HYPOTHESIS_WARNING.to_string()]
    );
    assert_eq!(
        warnings(true, false, 3, 0.0),
        vec![
            HYPOTHESIS_WARNING.to_string(),
            SEARCH_BOUNDARY_WARNING.to_string()
        ]
    );
    let all: Vec<String> = [
        HYPOTHESIS_WARNING,
        SEARCH_BOUNDARY_WARNING,
        GRID_BOUNDARY_WARNING,
        NO_CONSISTENT_POINT_WARNING,
        TIMESTAMP_SHIFT_WARNING,
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();
    assert_eq!(warnings(true, true, 0, 0.1), all);
}

#[test]
fn each_refusal_of_the_search_has_its_sentence() {
    assert_eq!(
        refusal_sentence(SensorLagError::SearchTooLarge),
        SEARCH_TOO_LARGE
    );
    assert_eq!(
        refusal_sentence(SensorLagError::GridTooLarge),
        GRID_TOO_LARGE
    );
    assert_eq!(refusal_sentence(SensorLagError::NoCandidate), NO_CANDIDATE);
}
