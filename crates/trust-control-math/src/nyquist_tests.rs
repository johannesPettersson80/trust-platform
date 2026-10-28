#![allow(clippy::float_cmp)] // The tests compare exact values on purpose.

use super::*;

fn sweep() -> Sweep {
    Sweep {
        minimum_rad_per_second: 1e-4,
        maximum_rad_per_second: 1e4,
        points_per_decade: 80,
    }
}

#[test]
fn a_pole_on_the_imaginary_axis_is_refused_for_the_contour() {
    // Ported from control-studio-numerics.test.ts: 2/(s² + s) has a pole at the origin.
    let transfer = Transfer {
        numerator: vec![2.0],
        denominator: vec![1.0, 1.0, 0.0],
        delay_seconds: 0.0,
    };
    assert_eq!(
        nyquist_stability(&transfer, sweep()),
        Err(ClassicalError::PoleOnContour)
    );
}

#[test]
fn a_stable_open_loop_has_no_closed_loop_right_half_plane_poles() {
    // Ported from control-studio-numerics.test.ts: 2/(s + 1).
    let transfer = Transfer {
        numerator: vec![2.0],
        denominator: vec![1.0, 1.0],
        delay_seconds: 0.0,
    };
    let nyquist = nyquist_stability(&transfer, sweep()).expect("a Nyquist count");
    assert_eq!(nyquist.open_loop_right_half_plane_poles, 0);
    assert!(nyquist.closed_loop_stable);
    assert_eq!(nyquist.closed_loop_right_half_plane_poles, 0);
    assert_eq!(nyquist.encirclements_counterclockwise, 0);
    assert!(nyquist.endpoint_magnitude <= 0.02);
    assert_eq!(
        nyquist.contour.len() % 2,
        1,
        "the mirror image, the origin and the curve: an odd count"
    );
}

#[test]
fn an_open_loop_pole_in_the_right_half_plane_stays_in_the_closed_loop_when_it_is_not_encircled() {
    // 0.5/(s − 1): the open-loop pole at +1 is in the right half-plane. The curve is a circle of radius 0.25
    // about −0.25 that does not enclose −1, so N = 0 and the closed loop keeps P = 1 right-half-plane pole.
    let transfer = Transfer {
        numerator: vec![0.5],
        denominator: vec![1.0, -1.0],
        delay_seconds: 0.0,
    };
    let nyquist = nyquist_stability(&transfer, sweep()).expect("a Nyquist count");
    assert_eq!(nyquist.open_loop_right_half_plane_poles, 1);
    assert_eq!(nyquist.encirclements_counterclockwise, 0);
    assert_eq!(nyquist.closed_loop_right_half_plane_poles, 1);
    assert!(!nyquist.closed_loop_stable);
}
