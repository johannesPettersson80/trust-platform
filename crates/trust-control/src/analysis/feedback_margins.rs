//! The Nyquist count, its report, the closed-loop poles' result and the classical margins of a general feedback loop
//! (N7.8, N7.10, N7.11): the helpers that `general_feedback` and `feedback_declared` share. The loop itself is in
//! `general_feedback`; the mathematics is in `trust_control_math`.

use std::f64::consts::PI;

use trust_control_math::classical::{
    classical_stability_margins, ClassicalError, GainMargin, Sweep, Transfer,
};
use trust_control_math::linalg::ComplexValue;
use trust_control_math::nyquist::{nyquist_stability, Nyquist};
use trust_control_math::polynomial::convolve;

use crate::analysis::general_feedback::{
    ClosedLoop, FeedbackDefinition, Margins, NyquistReport, NyquistSummary,
};
use crate::analysis::{limits, texts};
use crate::model::GeneralModel;

/// The Nyquist count of a transfer function on the grid, between its first and last frequencies (N7.10).
pub(crate) fn contour_count_for(
    transfer: &Transfer,
    frequencies: &[f64],
) -> Result<Nyquist, ClassicalError> {
    let first = frequencies[0];
    let last = frequencies[frequencies.len() - 1];
    let sweep = Sweep {
        minimum_rad_per_second: first,
        maximum_rad_per_second: last,
        points_per_decade: nyquist_points((last / first).log10()),
    };
    nyquist_stability(transfer, sweep)
}

/// The points per decade of a Nyquist contour: `⌊511 / decades⌋`, clamped to the allowed range.
fn nyquist_points(decades: f64) -> u32 {
    let budget = (limits::MARGIN_POINT_BUDGET / decades).floor();
    let clamped = budget.clamp(
        f64::from(limits::NYQUIST_MINIMUM_POINTS_PER_DECADE),
        f64::from(limits::NYQUIST_MAXIMUM_POINTS_PER_DECADE),
    );
    // The clamp keeps the value between ten and one hundred, so the conversion is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let points = clamped as u32;
    points
}

/// The report of a Nyquist count.
pub(crate) fn report(outcome: Result<Nyquist, ClassicalError>) -> NyquistReport {
    match outcome {
        Ok(nyquist) => NyquistReport::Qualified {
            summary: NyquistSummary {
                encirclements_counterclockwise: nyquist.encirclements_counterclockwise,
                open_loop_right_half_plane_poles: nyquist.open_loop_right_half_plane_poles,
                closed_loop_right_half_plane_poles: nyquist.closed_loop_right_half_plane_poles,
                closed_loop_stable: nyquist.closed_loop_stable,
                minimum_distance_to_minus_one: nyquist.minimum_distance_to_minus_one,
                endpoint_magnitude: nyquist.endpoint_magnitude,
            },
        },
        Err(error) => NyquistReport::Rejected {
            reason: texts::classical_reason(error).to_string(),
        },
    }
}

/// The closed-loop result of a set of poles: sorted, and stable when every pole lies in the stable region
/// (N7.11).
pub(crate) fn closed_loop_result(
    mut poles: Vec<ComplexValue>,
    plant: &GeneralModel,
    scope: String,
) -> ClosedLoop {
    poles.sort_by(|left, right| {
        left.real
            .partial_cmp(&right.real)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left.imaginary
                    .partial_cmp(&right.imaginary)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let stable = poles.iter().all(|pole| match plant {
        GeneralModel::DiscreteStateSpace { .. } => pole.real.hypot(pole.imaginary) < 1.0 - 1e-10,
        _ => pole.real < -1e-10,
    });
    ClosedLoop::Qualified {
        poles,
        stable,
        scope,
    }
}
/// The classical margins of the loop, estimated on the grid (N7.8): the first crossovers, with the range and
/// the pure delay checked first.
pub(crate) fn estimate_margins(definition: &FeedbackDefinition, frequencies: &[f64]) -> Margins {
    let unavailable = |reason: &str| Margins::Unavailable {
        reason: reason.to_string(),
    };
    let GeneralModel::TransferFunction { identity, transfer } = &definition.plant else {
        return unavailable(texts::MARGIN_NEEDS_TRANSFER);
    };
    let controller_transfer = match &definition.controller {
        GeneralModel::TransferFunction { identity, transfer } => Some((identity, transfer)),
        _ => None,
    };
    let (numerator, denominator, controller_delay) =
        match (&definition.declared_controller, controller_transfer) {
            (Some(declared), _) => (
                declared.numerator.clone(),
                declared.denominator.clone(),
                0.0,
            ),
            (None, Some((controller_identity, controller_transfer))) => (
                controller_transfer.numerator.clone(),
                controller_transfer.denominator.clone(),
                controller_identity.delay_seconds,
            ),
            (None, None) => return unavailable(texts::MARGIN_NEEDS_TRANSFER),
        };
    let delay = identity.delay_seconds + controller_delay;
    let first = frequencies[0];
    let last = frequencies[frequencies.len() - 1];
    let decades = (last / first).log10();
    let points = (limits::MARGIN_POINT_BUDGET / decades).floor().min(100.0);
    if !decades.is_finite() || decades <= 0.0 || points < 10.0 {
        return unavailable(texts::MARGIN_RANGE_UNSUPPORTED);
    }
    let largest_interval = last * (1.0 - 10f64.powf(-1.0 / points));
    if largest_interval * delay > PI / 2.0 {
        return unavailable(texts::MARGIN_DELAY_TOO_FAST);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let points_per_decade = points as u32;
    let loop_transfer = Transfer {
        numerator: convolve(&transfer.numerator, &numerator),
        denominator: convolve(&transfer.denominator, &denominator),
        delay_seconds: delay,
    };
    let sweep = Sweep {
        minimum_rad_per_second: first,
        maximum_rad_per_second: last,
        points_per_decade,
    };
    match classical_stability_margins(&loop_transfer, sweep) {
        Ok(margins)
            if margins.gain_crossover_rad_per_second.is_finite()
                && margins.phase_margin_degrees.is_finite() =>
        {
            Margins::Estimated {
                gain_crossover_rad_per_second: margins.gain_crossover_rad_per_second,
                phase_margin_degrees: margins.phase_margin_degrees,
                gain_margin_db: match margins.gain_margin {
                    GainMargin::Decibels(value) if value.is_finite() => Some(value),
                    _ => None,
                },
                scope: texts::MARGIN_SCOPE.to_string(),
            }
        }
        _ => unavailable(texts::MARGIN_NO_CROSSOVER),
    }
}
