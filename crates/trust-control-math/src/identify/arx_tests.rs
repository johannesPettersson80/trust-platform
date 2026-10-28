//! Tests of the multichannel ARX fit (specification 35, C7 and C8; numerical annex N10). Each record is simulated from a known
//! difference equation that starts at rest, and its first excitation sample is zero, so the first row is the operating point at
//! the origin and the data are exact. The fit must then recover the coefficients of the equation.

use super::*;

/// The tolerance of the recovered coefficients: the records are exact, so only rounding remains.
const TOLERANCE: f64 = 1e-6;

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "{actual} is not {expected}"
    );
}

/// A deterministic excitation in `[-1, 1)`: a linear congruential sequence whose first sample is zero, so the record starts at
/// rest.
#[allow(clippy::cast_precision_loss)]
fn excitation(seed: u64, length: usize) -> Vec<f64> {
    let mut state = seed;
    (0..length)
        .map(|index| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            if index == 0 {
                return 0.0;
            }
            // The top 53 bits give a uniform value in [0, 1).
            let uniform = (state >> 11) as f64 / (1_u64 << 53) as f64;
            2.0 * uniform - 1.0
        })
        .collect()
}

/// The rows `[u1, u2, y]` of `y(k+1) = a1·y(k) + a2·y(k−1) + b10·u1(k) + b11·u1(k−1) + b20·u2(k) + b21·u2(k−1)`, from rest. The
/// coefficients are `[a1, a2, b10, b11, b20, b21]`.
#[allow(clippy::needless_range_loop)] // A difference equation reads its earlier samples by index.
fn two_input_record(first: &[f64], second: &[f64], coefficients: [f64; 6]) -> Vec<Vec<f64>> {
    let [a1, a2, b10, b11, b20, b21] = coefficients;
    let mut output = vec![0.0; first.len()];
    for k in 1..first.len() - 1 {
        output[k + 1] = a1 * output[k]
            + a2 * output[k - 1]
            + b10 * first[k]
            + b11 * first[k - 1]
            + b20 * second[k]
            + b21 * second[k - 1];
    }
    (0..first.len())
        .map(|k| vec![first[k], second[k], output[k]])
        .collect()
}

/// The rows `[u, y1, y2]` of `y1(k+1) = 0.5·y1(k) + 0.2·y2(k) + 0.3·u(k)` and `y2(k+1) = −0.1·y1(k) + 0.7·y2(k) + 0.4·u(k)`, from
/// rest.
#[allow(clippy::needless_range_loop)] // A difference equation reads its earlier samples by index.
fn two_output_record(input: &[f64]) -> Vec<Vec<f64>> {
    let mut first = vec![0.0; input.len()];
    let mut second = vec![0.0; input.len()];
    for k in 0..input.len() - 1 {
        first[k + 1] = 0.5 * first[k] + 0.2 * second[k] + 0.3 * input[k];
        second[k + 1] = -0.1 * first[k] + 0.7 * second[k] + 0.4 * input[k];
    }
    (0..input.len())
        .map(|k| vec![input[k], first[k], second[k]])
        .collect()
}

/// Rows of `width` independent excitations, for the cap test.
fn random_record(rows: usize, width: usize, seed: u64) -> Vec<Vec<f64>> {
    let columns: Vec<Vec<f64>> = (0..width)
        .map(|column| excitation(seed + u64::try_from(column).unwrap_or_default(), rows))
        .collect();
    (0..rows)
        .map(|row| columns.iter().map(|column| column[row]).collect())
        .collect()
}

#[test]
fn a_two_input_order_two_record_is_recovered_from_its_difference_equation() {
    // y(k+1) = 1.2·y(k) − 0.4·y(k−1) + 0.5·u1(k) + 0.3·u1(k−1) − 0.2·u2(k) + 0.1·u2(k−1): a stable pair of poles at 0.6 ± 0.2i.
    let coefficients = [1.2, -0.4, 0.5, 0.3, -0.2, 0.1];
    let estimation = two_input_record(&excitation(7, 240), &excitation(11, 240), coefficients);
    let comparison = two_input_record(&excitation(23, 240), &excitation(29, 240), coefficients);
    let identified =
        fit(&estimation, &comparison, 2, 2, 0.05).expect("an exact order-two record is identified");
    assert_eq!(identified.order, 2);
    let model = &identified.model;
    assert_eq!((model.states(), model.inputs(), model.outputs()), (4, 2, 1));
    // The output lags come first in the state, then the input lag one: the companion form holds the coefficients.
    assert_close(model.a[0][0], 1.2);
    assert_close(model.a[0][1], -0.4);
    assert_close(model.a[0][2], 0.3);
    assert_close(model.a[0][3], 0.1);
    assert_close(model.b[0][0], 0.5);
    assert_close(model.b[0][1], -0.2);
    // The input history takes the current inputs one step later, and the output reads the current output.
    assert_close(model.b[2][0], 1.0);
    assert_close(model.b[2][1], 0.0);
    assert_close(model.b[3][0], 0.0);
    assert_close(model.b[3][1], 1.0);
    assert_close(model.c[0][0], 1.0);
    assert_close(model.d[0][0], 0.0);
    assert_close(model.d[0][1], 0.0);
    // The first order cannot hold a second-order record, so the second order is the first adequate one.
    assert!(matches!(
        identified.comparisons[0].outcome,
        Outcome::Evaluated {
            adequate: false,
            ..
        }
    ));
    // The free run on the comparison record reproduces its output from the first predicted sample, which is sample two.
    assert_eq!(identified.predictions[0].len(), comparison.len() - 2);
    for (index, predicted) in identified.predictions[0].iter().enumerate() {
        assert_close(*predicted, comparison[index + 2][2]);
    }
}

#[test]
fn a_one_input_two_output_first_order_record_is_recovered() {
    let estimation = two_output_record(&excitation(3, 200));
    let comparison = two_output_record(&excitation(5, 200));
    let identified =
        fit(&estimation, &comparison, 1, 1, 0.05).expect("an exact order-one record is identified");
    assert_eq!(identified.order, 1);
    let model = &identified.model;
    assert_eq!((model.states(), model.inputs(), model.outputs()), (2, 1, 2));
    assert_close(model.a[0][0], 0.5);
    assert_close(model.a[0][1], 0.2);
    assert_close(model.a[1][0], -0.1);
    assert_close(model.a[1][1], 0.7);
    assert_close(model.b[0][0], 0.3);
    assert_close(model.b[1][0], 0.4);
    assert_close(model.c[0][0], 1.0);
    assert_close(model.c[0][1], 0.0);
    assert_close(model.c[1][0], 0.0);
    assert_close(model.c[1][1], 1.0);
    // Each output is predicted from the first sample on, and its predictions match its record.
    assert_eq!(identified.predictions.len(), 2);
    for (output, predictions) in identified.predictions.iter().enumerate() {
        assert_eq!(predictions.len(), comparison.len() - 1);
        for (index, predicted) in predictions.iter().enumerate() {
            assert_close(*predicted, comparison[index + 1][1 + output]);
        }
    }
}

#[test]
fn the_held_out_check_refuses_a_model_above_its_error_limit() {
    // Estimated on one system and compared on a second whose dynamics differ: no compared order predicts it within the limit.
    let estimation = two_input_record(
        &excitation(7, 240),
        &excitation(11, 240),
        [1.2, -0.4, 0.5, 0.3, -0.2, 0.1],
    );
    let comparison = two_input_record(
        &excitation(23, 240),
        &excitation(29, 240),
        [0.5, -0.2, 0.2, 0.0, -0.6, 0.0],
    );
    match fit(&estimation, &comparison, 2, 2, 0.05) {
        Err(Rejection::NoAdequateOrder { comparisons }) => {
            assert_eq!(comparisons.len(), 2);
            for compared in &comparisons {
                assert!(
                    matches!(
                        compared.outcome,
                        Outcome::Evaluated {
                            adequate: false,
                            ..
                        }
                    ),
                    "order {} is adequate on the second system",
                    compared.order
                );
            }
        }
        other => panic!("expected a refusal above the error limit, got {other:?}"),
    }
}

#[test]
fn the_limits_and_the_records_are_checked_before_any_fit() {
    let record = two_input_record(&excitation(7, 60), &excitation(11, 60), [0.0; 6]);
    assert!(matches!(
        fit(&record, &record, 2, 9, 0.05),
        Err(Rejection::InvalidLimit)
    ));
    assert!(matches!(
        fit(&record, &record, 2, 0, 0.05),
        Err(Rejection::InvalidLimit)
    ));
    assert!(matches!(
        fit(&record, &record, 2, 2, 1.0),
        Err(Rejection::InvalidLimit)
    ));
    assert!(matches!(
        fit(&record, &record, 2, 2, 0.0),
        Err(Rejection::InvalidLimit)
    ));
    assert!(matches!(
        fit(&record, &record, 0, 2, 0.05),
        Err(Rejection::InvalidRecords)
    ));
    let ragged = vec![vec![0.0, 0.0, 0.0], vec![0.0, 0.0]];
    assert!(matches!(
        fit(&ragged, &ragged, 2, 2, 0.05),
        Err(Rejection::InvalidRecords)
    ));
    let empty: Vec<Vec<f64>> = Vec::new();
    assert!(matches!(
        fit(&empty, &empty, 2, 2, 0.05),
        Err(Rejection::InvalidRecords)
    ));
    let nonfinite = vec![vec![0.0, 0.0, f64::NAN]];
    assert!(matches!(
        fit(&nonfinite, &nonfinite, 2, 2, 0.05),
        Err(Rejection::InvalidRecords)
    ));
}

#[test]
fn the_regressor_cap_stops_the_orders_at_sixty_four() {
    // Nine channels (five inputs and four outputs): order seven holds 63 regressors per output, and order eight would hold 72,
    // past the cap, so only orders one to seven are compared whatever the records say.
    let record = random_record(200, 9, 41);
    let comparisons = match fit(&record, &record, 5, 8, 0.05) {
        Ok(identified) => identified.comparisons,
        Err(Rejection::NoAdequateOrder { comparisons }) => comparisons,
        Err(other) => panic!("unexpected refusal {other:?}"),
    };
    assert_eq!(comparisons.len(), 7);
    assert_eq!(comparisons.last().map(|compared| compared.order), Some(7));
}
