#![allow(clippy::float_cmp)] // The tests compare exact values on purpose.

use super::*;

fn shaft() -> Realization {
    Realization {
        a: vec![vec![0.0, 1.0], vec![0.0, -8.0]],
        b: vec![0.0, 1.0],
        c: vec![1.0, 0.0],
        d: 0.0,
    }
}

#[test]
fn a_proportional_gain_closes_the_shaft_to_its_expected_poles() {
    // Ported from control-studio-general-feedback.test.ts: the shaft with the proportional gain 12 closes to
    // s² + 8s + 12 = (s + 2)(s + 6), so the poles are −2 and −6.
    let controller = Realization::from_transfer(&[12.0], &[1.0]).expect("a constant controller");
    let mut poles: Vec<f64> = closed_loop_poles(&shaft(), &controller)
        .expect("the closed loop")
        .iter()
        .map(|pole| pole.real)
        .collect();
    poles.sort_by(f64::total_cmp);
    assert!((poles[0] + 6.0).abs() < 1e-9);
    assert!((poles[1] + 2.0).abs() < 1e-9);
}

#[test]
fn a_loop_with_no_states_has_no_poles() {
    let constant = Realization {
        a: Vec::new(),
        b: Vec::new(),
        c: Vec::new(),
        d: 2.0,
    };
    assert_eq!(
        closed_loop_poles(&constant, &constant).expect("no states"),
        Vec::new()
    );
}

#[test]
fn an_algebraic_loop_with_a_unit_loop_gain_is_singular() {
    assert!(!algebraic_loop_is_singular(0.0, 0.0));
    assert!(algebraic_loop_is_singular(2.0, -0.5), "1 + 2·(−0.5) = 0");
    assert!(
        algebraic_loop_is_singular(f64::INFINITY, 1.0),
        "a loop gain that is not finite"
    );
}

#[test]
fn a_transfer_function_realization_reads_its_normalised_coefficients() {
    // 2/(s + 4): A = [−4], B = [1], C = [2], D = [0].
    let realization = Realization::from_transfer(&[2.0], &[1.0, 4.0]).expect("a realization");
    assert_eq!(realization.a, vec![vec![-4.0]]);
    assert_eq!(realization.b, vec![1.0]);
    assert_eq!(realization.c, vec![2.0]);
    assert_eq!(realization.d, 0.0);
}
