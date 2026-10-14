//! Tests of the measured frequency rows (N2.8): ascending order, and the phase unwrapped to within 180° of the previous tone.
//! The TypeScript contract is the comment of `measuredFrequencyRows.ts`; the TypeScript tests reach it through the
//! three-page analysis webview, which is not ported.

#![allow(clippy::float_cmp)] // The rows carry the exact values of the points they were made from.

use trust_control_math::frequency_identification::FrequencyResponsePoint;
use trust_control_math::linalg::ComplexValue;

use super::measured_frequency_rows;

fn point(frequency: f64, phase_degrees: f64) -> FrequencyResponsePoint {
    let phase_radians = phase_degrees * std::f64::consts::PI / 180.0;
    FrequencyResponsePoint {
        frequency_rad_per_second: frequency,
        response: ComplexValue {
            real: phase_radians.cos(),
            imaginary: phase_radians.sin(),
        },
        magnitude: 1.0,
        phase_radians,
        coherence: 0.99,
    }
}

#[test]
fn rows_are_sorted_by_frequency_and_unwrapped_from_the_lowest_tone() {
    // Principal phases 170°, −170° and 0°: the second is brought to 190° (within 180° of 170°), and the third to 360°
    // (within 180° of 190°).
    let rows = measured_frequency_rows(&[point(3.0, 0.0), point(1.0, 170.0), point(2.0, -170.0)]);
    let frequencies: Vec<f64> = rows.iter().map(|row| row.frequency).collect();
    assert_eq!(frequencies, vec![1.0, 2.0, 3.0]);
    let phases: Vec<f64> = rows.iter().map(|row| row.phase_degrees).collect();
    assert!((phases[0] - 170.0).abs() < 1e-9, "{phases:?}");
    assert!((phases[1] - 190.0).abs() < 1e-9, "{phases:?}");
    assert!((phases[2] - 360.0).abs() < 1e-9, "{phases:?}");
    assert!(rows
        .iter()
        .all(|row| row.magnitude == 1.0 && row.coherence == 0.99));
}

#[test]
fn a_jump_of_more_than_180_degrees_is_unwrapped_and_a_smaller_one_is_kept() {
    // A step of 179° stays; a step of 181° is taken the other way round, to −179°.
    let kept = measured_frequency_rows(&[point(1.0, 0.0), point(2.0, 179.0)]);
    assert!((kept[1].phase_degrees - 179.0).abs() < 1e-9);
    let unwrapped = measured_frequency_rows(&[point(1.0, 0.0), point(2.0, 181.0)]);
    assert!((unwrapped[1].phase_degrees + 179.0).abs() < 1e-9);
}
