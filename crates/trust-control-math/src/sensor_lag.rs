//! The sensor-lag fit of a held step (N2.2, N2.4, N3.2; `fitSensorLagStep` in `sensorLagStepIdentification.ts`).
//!
//! The held response is `y = y₀ + Δu · K · S(t − t_step + shift)`, where `S` is the second-order-with-dead-time step
//! response of [`second_order_dead_time_step`]: a first time constant `τ`, a second time constant `τ₂` standing for the
//! sensor's lag, and a dead time `L`. The search runs over grids of the delay, the two time constants, the baseline `y₀`
//! and the gain `K`. The gain grid is the range the baseline and final windows allow within the measurement bound `b`
//! (N2.4). For each shape the search keeps the least-squares candidate, and it counts every grid point whose baseline
//! and gain keep each measured sample within `b` of the model (the measurement-consistent set). Which physical process
//! or sensor owns which lag is not decided here: the two time constants can swap places with the same response.

use crate::identify::step_response::{gain_bounds, lattice, StepResponseWindow};
use crate::identify::ParameterInterval;

/// The tolerance of the search: in seconds for times, and in output units for values (`EPSILON`).
const EPSILON: f64 = 1e-9;
/// The most work a search may do, counted in samples evaluated (`MAX_WORK`).
const MAXIMUM_WORK: f64 = 500_000_000.0;
/// The most delay, time-constant and sensor shapes a search may hold.
const MAXIMUM_SHAPES: f64 = 1_000_000.0;
/// The most baseline grid points a search may hold.
const MAXIMUM_BASELINES: f64 = 100_000.0;
/// The most shape-by-baseline pairs a search may hold.
const MAXIMUM_SHAPE_BASELINE_PAIRS: f64 = 20_000_000.0;

/// A search range: the smallest value, the largest, and the spacing of the lattice between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchRange {
    /// The smallest value of the range.
    pub minimum: f64,
    /// The largest value of the range.
    pub maximum: f64,
    /// The spacing of the lattice.
    pub step: f64,
}

/// The grids and the measurement assumptions of a sensor-lag search.
#[derive(Debug, Clone, PartialEq)]
pub struct SensorLagSearch {
    /// The dead-time grid, in seconds.
    pub delay: SearchRange,
    /// The grid of the first time constant `τ`, in seconds.
    pub time_constant: SearchRange,
    /// The grid of the second time constant `τ₂` (the sensor lag), in seconds.
    pub sensor_time_constant: SearchRange,
    /// The measurement bound `b`: the largest difference the model may show from a measured output, in output units.
    pub measurement_bound: f64,
    /// The spacing of the baseline grid, in output units.
    pub baseline_step: f64,
    /// The spacing of the gain grid, in output units per input unit.
    pub gain_step: f64,
    /// The timestamp shift in seconds, a sensitivity study that moves the step time of the model (zero is nominal).
    pub timestamp_shift_seconds: f64,
}

/// One held sample: its time and its measured output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldSample {
    /// The sample time in seconds.
    pub time_seconds: f64,
    /// The measured output.
    pub output: f64,
}

/// The held step the fit is measured against: its baseline and final windows, its input change and its step time.
#[derive(Debug, Clone, Copy)]
pub struct StepReference<'a> {
    /// The steady baseline window (N2.6).
    pub baseline: &'a StepResponseWindow,
    /// The final window of the held response (N2.4).
    pub final_window: &'a StepResponseWindow,
    /// The fitted input change `Δu`.
    pub input_change: f64,
    /// The time of the step in seconds.
    pub step_time_seconds: f64,
}

/// One fitted candidate: its baseline, its gain, its three shape parameters and the sum of its squared residuals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorLagCandidate {
    /// The baseline output `y₀`.
    pub baseline: f64,
    /// The gain `K`.
    pub gain: f64,
    /// The dead time `L` in seconds.
    pub delay_seconds: f64,
    /// The first time constant `τ` in seconds.
    pub time_constant_seconds: f64,
    /// The second time constant `τ₂` in seconds.
    pub second_time_constant_seconds: f64,
    /// The sum of the squared residuals over the held samples.
    pub squared_error: f64,
}

/// The measurement-consistent set of a search: how many baseline-and-gain grid points keep every sample within the
/// measurement bound, and the intervals those points span for each parameter (N2.4). Each interval is `None` when no
/// point contributed to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SensorLagConsistency {
    /// The number of consistent baseline grid points, summed over the consistent gains and shapes.
    pub count: usize,
    /// The interval of the consistent gains.
    pub gain: Option<ParameterInterval>,
    /// The interval of the consistent dead times, in seconds.
    pub delay_seconds: Option<ParameterInterval>,
    /// The interval of the consistent first time constants, in seconds.
    pub time_constant_seconds: Option<ParameterInterval>,
    /// The interval of the consistent second time constants, in seconds.
    pub second_time_constant_seconds: Option<ParameterInterval>,
    /// The interval of the consistent baselines.
    pub baseline: Option<ParameterInterval>,
}

/// The result of a sensor-lag fit.
#[derive(Debug, Clone, PartialEq)]
pub struct SensorLagFit {
    /// The least-squares candidate over the whole grid.
    pub best: SensorLagCandidate,
    /// The measurement-consistent set.
    pub consistency: SensorLagConsistency,
    /// The model's prediction at each held sample, in the order of the held samples.
    pub predicted: Vec<f64>,
    /// Whether the best baseline or gain lies on the edge of its grid.
    pub amplitude_on_boundary: bool,
    /// Whether the best dead time (when positive), first time constant or second time constant lies on the edge of its grid.
    pub shape_on_boundary: bool,
}

/// Why a sensor-lag fit is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SensorLagError {
    /// The shapes, the work or the baseline grid of the search exceed their limits.
    #[error("the sensor-lag search is too large")]
    SearchTooLarge,
    /// A lattice of the search has more than the most points, or no finite size.
    #[error("a search grid is too large")]
    GridTooLarge,
    /// No baseline and gain lie within the qualified windows, so the search has no candidate.
    #[error("no grid point lies within the qualified windows")]
    NoCandidate,
}

/// The second-order-with-dead-time step response `S(t)` at `elapsed_seconds` after the step (`lowOrderStepResponse`
/// with `second_order_dead_time`, N2.2). It is zero before the dead time `L`, and afterwards
///
/// ```text
/// S(t) = 1 − (τ₁·e^(−t/τ₁) − τ₂·e^(−t/τ₂)) / (τ₁ − τ₂),        t = elapsed − L
/// ```
///
/// with the limit `S(t) = 1 − (1 + t/τ)·e^(−t/τ)` when the two time constants agree to within `10⁻¹²`.
#[must_use]
pub fn second_order_dead_time_step(
    elapsed_seconds: f64,
    time_constant_seconds: f64,
    second_time_constant_seconds: f64,
    delay_seconds: f64,
) -> f64 {
    let time = elapsed_seconds - delay_seconds;
    if time < 0.0 {
        return 0.0;
    }
    let (first, second) = (time_constant_seconds, second_time_constant_seconds);
    if (first - second).abs() <= 1e-12 {
        let normalized = time / first;
        return 1.0 - (1.0 + normalized) * (-normalized).exp();
    }
    1.0 - (first * (-time / first).exp() - second * (-time / second).exp()) / (first - second)
}

/// Fits the sensor-lag model to a held step and keeps its measurement-consistent set (`fitSensorLagStep`).
///
/// `hold` holds the samples from the step to the end of the hold, in time order. `pre_step_outputs` holds the outputs of
/// the samples of the baseline window that come before the step; they bound the baseline (N2.6).
///
/// # Errors
/// Returns [`SensorLagError::SearchTooLarge`] when the shapes, the baseline grid or the work exceed their limits,
/// [`SensorLagError::GridTooLarge`] when a lattice is too large or has no finite size, and
/// [`SensorLagError::NoCandidate`] when no baseline and gain lie within the qualified windows.
pub fn fit_sensor_lag_step(
    hold: &[HeldSample],
    pre_step_outputs: &[f64],
    reference: &StepReference<'_>,
    search: &SensorLagSearch,
) -> Result<SensorLagFit, SensorLagError> {
    let bound = search.measurement_bound;
    let origin = reference.baseline.mean - bound;
    let baseline_count = ((2.0 * bound + EPSILON) / search.baseline_step).floor() + 1.0;
    let shapes = grid_size(search.delay)
        * grid_size(search.time_constant)
        * grid_size(search.sensor_time_constant);
    let hold_count = count_f64(hold.len());
    if !shapes.is_finite()
        || shapes > MAXIMUM_SHAPES
        || shapes * hold_count > MAXIMUM_WORK
        || baseline_count > MAXIMUM_BASELINES
        || shapes * baseline_count > MAXIMUM_SHAPE_BASELINE_PAIRS
    {
        return Err(SensorLagError::SearchTooLarge);
    }
    let gain_range = gain_bounds(
        reference.baseline,
        reference.final_window,
        reference.input_change,
        bound,
    );
    let grids = Grids {
        gains: grid(gain_range.minimum, gain_range.maximum, search.gain_step)?,
        baselines: grid(
            origin,
            reference.baseline.mean + bound,
            search.baseline_step,
        )?,
        delays: grid(
            search.delay.minimum,
            search.delay.maximum,
            search.delay.step,
        )?,
        time_constants: grid(
            search.time_constant.minimum,
            search.time_constant.maximum,
            search.time_constant.step,
        )?,
        sensors: grid(
            search.sensor_time_constant.minimum,
            search.sensor_time_constant.maximum,
            search.sensor_time_constant.step,
        )?,
    };
    if shapes * count_f64(grids.gains.len()) > MAXIMUM_WORK {
        return Err(SensorLagError::SearchTooLarge);
    }
    let problem = Problem::new(hold, pre_step_outputs, reference, search);
    let mut accumulator = Accumulator {
        best: None,
        consistency: SensorLagConsistency::default(),
        work: 0.0,
    };
    let mut responses = vec![0.0; hold.len()];
    for &delay_seconds in &grids.delays {
        for &time_constant_seconds in &grids.time_constants {
            for &second_time_constant_seconds in &grids.sensors {
                let shape = Shape {
                    delay_seconds,
                    time_constant_seconds,
                    second_time_constant_seconds,
                };
                accumulator.shape(&problem, &grids, shape, &mut responses)?;
            }
        }
    }
    let best = accumulator.best.ok_or(SensorLagError::NoCandidate)?;
    let predicted: Vec<f64> = hold
        .iter()
        .map(|sample| problem.predict(&best, sample.time_seconds))
        .collect();
    let squared_error = hold
        .iter()
        .zip(&predicted)
        .fold(0.0, |sum, (sample, prediction)| {
            let residual = sample.output - prediction;
            sum + residual * residual
        });
    let amplitude_on_boundary = near(best.baseline, grids.baselines[0])
        || near(best.baseline, grids.baselines[grids.baselines.len() - 1])
        || near(best.gain, grids.gains[0])
        || near(best.gain, grids.gains[grids.gains.len() - 1]);
    let shape_on_boundary = on_grid_edge(best.delay_seconds, search.delay, true)
        || on_grid_edge(best.time_constant_seconds, search.time_constant, false)
        || on_grid_edge(
            best.second_time_constant_seconds,
            search.sensor_time_constant,
            false,
        );
    Ok(SensorLagFit {
        best: SensorLagCandidate {
            squared_error,
            ..best
        },
        consistency: accumulator.consistency,
        predicted,
        amplitude_on_boundary,
        shape_on_boundary,
    })
}

/// One shape of the search: a dead time and the two time constants.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_field_names)] // Every field is a time, and its name says which one.
struct Shape {
    delay_seconds: f64,
    time_constant_seconds: f64,
    second_time_constant_seconds: f64,
}

/// The lattices of the search, in the order `fitSensorLagStep` builds them.
struct Grids {
    gains: Vec<f64>,
    baselines: Vec<f64>,
    delays: Vec<f64>,
    time_constants: Vec<f64>,
    sensors: Vec<f64>,
}

/// What every shape of the search reads: the held samples, their centred outputs and the step's constants.
struct Problem<'a> {
    hold: &'a [HeldSample],
    centered: Vec<f64>,
    sum_y: f64,
    sum_y2: f64,
    baseline_mean: f64,
    final_mean: f64,
    input_change: f64,
    step_time_seconds: f64,
    timestamp_shift_seconds: f64,
    measurement_bound: f64,
    origin: f64,
    baseline_step: f64,
    gain_step: f64,
    baseline_lower: f64,
    baseline_upper: f64,
}

/// The sums over the held samples of one shape's responses `φₖ`: `Σφ`, `Σφ²` and `Σφ·(y − y₀)`.
#[derive(Debug, Clone, Copy, Default)]
struct Sums {
    phi: f64,
    phi2: f64,
    phi_y: f64,
}

/// What the search accumulates over the shapes: the best candidate, the consistent set and the work done.
struct Accumulator {
    best: Option<SensorLagCandidate>,
    consistency: SensorLagConsistency,
    work: f64,
}

impl<'a> Problem<'a> {
    fn new(
        hold: &'a [HeldSample],
        pre_step_outputs: &[f64],
        reference: &StepReference<'_>,
        search: &SensorLagSearch,
    ) -> Self {
        let bound = search.measurement_bound;
        let baseline_mean = reference.baseline.mean;
        let origin = baseline_mean - bound;
        let centered: Vec<f64> = hold
            .iter()
            .map(|sample| sample.output - baseline_mean)
            .collect();
        let sum_y = centered.iter().fold(0.0, |sum, value| sum + value);
        let sum_y2 = centered.iter().fold(0.0, |sum, value| sum + value * value);
        let baseline_lower = pre_step_outputs
            .iter()
            .fold(origin, |lower, &output| lower.max(output - bound));
        let baseline_upper = pre_step_outputs
            .iter()
            .fold(baseline_mean + bound, |upper, &output| {
                upper.min(output + bound)
            });
        Self {
            hold,
            centered,
            sum_y,
            sum_y2,
            baseline_mean,
            final_mean: reference.final_window.mean,
            input_change: reference.input_change,
            step_time_seconds: reference.step_time_seconds,
            timestamp_shift_seconds: search.timestamp_shift_seconds,
            measurement_bound: bound,
            origin,
            baseline_step: search.baseline_step,
            gain_step: search.gain_step,
            baseline_lower,
            baseline_upper,
        }
    }

    /// The time since the step at which the model is read for a sample (with the timestamp shift).
    fn elapsed(&self, time_seconds: f64) -> f64 {
        time_seconds - self.step_time_seconds + self.timestamp_shift_seconds
    }

    /// Writes the responses `φₖ = Δu·S(elapsed)` of one shape into `responses` and returns their sums.
    fn responses(&self, shape: Shape, responses: &mut [f64]) -> Sums {
        let mut sums = Sums::default();
        for (index, sample) in self.hold.iter().enumerate() {
            let phi = self.input_change
                * second_order_dead_time_step(
                    self.elapsed(sample.time_seconds),
                    shape.time_constant_seconds,
                    shape.second_time_constant_seconds,
                    shape.delay_seconds,
                );
            responses[index] = phi;
            sums.phi += phi;
            sums.phi2 += phi * phi;
            sums.phi_y += phi * self.centered[index];
        }
        sums
    }

    /// The model's output at a time for a candidate.
    fn predict(&self, candidate: &SensorLagCandidate, time_seconds: f64) -> f64 {
        candidate.baseline
            + self.input_change
                * candidate.gain
                * second_order_dead_time_step(
                    self.elapsed(time_seconds),
                    candidate.time_constant_seconds,
                    candidate.second_time_constant_seconds,
                    candidate.delay_seconds,
                )
    }
}

impl Accumulator {
    /// Adds the work of one pass over the held samples, and refuses a search that has exceeded its work limit.
    fn add_work(&mut self, amount: f64) -> Result<(), SensorLagError> {
        self.work += amount;
        if self.work > MAXIMUM_WORK {
            return Err(SensorLagError::SearchTooLarge);
        }
        Ok(())
    }

    /// Searches one shape: the least-squares candidates over the baselines, then, when the shape can satisfy the bound, the
    /// measurement-consistent gains and baselines.
    fn shape(
        &mut self,
        problem: &Problem<'_>,
        grids: &Grids,
        shape: Shape,
        responses: &mut [f64],
    ) -> Result<(), SensorLagError> {
        let sums = problem.responses(shape, responses);
        self.add_work(count_f64(problem.hold.len()))?;
        let shape_error = self.best_over_baselines(problem, grids, shape, sums);
        let allowance =
            count_f64(problem.hold.len()) * problem.measurement_bound * problem.measurement_bound;
        // No point can satisfy every residual bound when even the least sum of squares exceeds N·b²; this discards no
        // point that the measurements allow.
        if shape_error > allowance + EPSILON {
            return Ok(());
        }
        self.consistent_gains(problem, grids, shape, responses)
    }

    /// The least-squares candidate of one shape over the baselines and the gains that the final window allows. For each
    /// baseline the gain objective is a convex quadratic, so the two grid gains around its optimum contain the discrete
    /// minimum. Returns the least squared error the shape reaches.
    fn best_over_baselines(
        &mut self,
        problem: &Problem<'_>,
        grids: &Grids,
        shape: Shape,
        sums: Sums,
    ) -> f64 {
        let bound = problem.measurement_bound;
        let gains = &grids.gains;
        let last_gain = count_f64(gains.len() - 1);
        let hold_count = count_f64(problem.hold.len());
        let mut shape_error = f64::INFINITY;
        for &baseline in &grids.baselines {
            let endpoints = [
                (problem.final_mean - bound - baseline) / problem.input_change,
                (problem.final_mean + bound - baseline) / problem.input_change,
            ];
            let low = endpoints[0].min(endpoints[1]);
            let high = endpoints[0].max(endpoints[1]);
            let first_f = ((low - gains[0] - EPSILON) / problem.gain_step)
                .ceil()
                .max(0.0);
            let last_f = ((high - gains[0] + EPSILON) / problem.gain_step)
                .floor()
                .min(last_gain);
            if first_f > last_f {
                continue;
            }
            let (first, last) = (to_index(first_f), to_index(last_f));
            let offset = baseline - problem.baseline_mean;
            let optimum = if sums.phi2 > 0.0 {
                (sums.phi_y - offset * sums.phi) / sums.phi2
            } else {
                gains[first]
            };
            let nearest = (optimum - gains[0]) / problem.gain_step;
            for index in [
                clamp_index(nearest.floor(), first, last),
                clamp_index(nearest.ceil(), first, last),
            ] {
                let gain = gains[index];
                let squared_error =
                    (problem.sum_y2 - 2.0 * offset * problem.sum_y - 2.0 * gain * sums.phi_y
                        + hold_count * offset * offset
                        + 2.0 * offset * gain * sums.phi
                        + gain * gain * sums.phi2)
                        .max(0.0);
                shape_error = shape_error.min(squared_error);
                let improves = match self.best {
                    None => true,
                    Some(best) => squared_error < best.squared_error - 1e-12,
                };
                if improves {
                    self.best = Some(SensorLagCandidate {
                        baseline,
                        gain,
                        delay_seconds: shape.delay_seconds,
                        time_constant_seconds: shape.time_constant_seconds,
                        second_time_constant_seconds: shape.second_time_constant_seconds,
                        squared_error,
                    });
                }
            }
        }
        shape_error
    }

    /// The measurement-consistent set of one shape: for each gain, the baselines that keep every sample within the bound.
    /// Each gain adds the count of its consistent baselines and widens the intervals of the shape's parameters.
    fn consistent_gains(
        &mut self,
        problem: &Problem<'_>,
        grids: &Grids,
        shape: Shape,
        responses: &[f64],
    ) -> Result<(), SensorLagError> {
        let bound = problem.measurement_bound;
        let last_baseline = count_f64(grids.baselines.len() - 1);
        for &gain in &grids.gains {
            let mut lower = problem
                .baseline_lower
                .max(problem.final_mean - bound - gain * problem.input_change);
            let mut upper = problem
                .baseline_upper
                .min(problem.final_mean + bound - gain * problem.input_change);
            let mut index = 0;
            while index < problem.hold.len() && lower <= upper + EPSILON {
                let output = problem.hold[index].output;
                lower = lower.max(output - bound - gain * responses[index]);
                upper = upper.min(output + bound - gain * responses[index]);
                self.work += 1.0;
                index += 1;
            }
            if self.work > MAXIMUM_WORK {
                return Err(SensorLagError::SearchTooLarge);
            }
            let first_f = ((lower - problem.origin - EPSILON) / problem.baseline_step)
                .ceil()
                .max(0.0);
            let last_f = ((upper - problem.origin + EPSILON) / problem.baseline_step)
                .floor()
                .min(last_baseline);
            if first_f > last_f {
                continue;
            }
            let (first, last) = (to_index(first_f), to_index(last_f));
            self.consistency.count += last - first + 1;
            widen(&mut self.consistency.gain, gain);
            widen(&mut self.consistency.delay_seconds, shape.delay_seconds);
            widen(
                &mut self.consistency.time_constant_seconds,
                shape.time_constant_seconds,
            );
            widen(
                &mut self.consistency.second_time_constant_seconds,
                shape.second_time_constant_seconds,
            );
            widen(&mut self.consistency.baseline, grids.baselines[first]);
            widen(&mut self.consistency.baseline, grids.baselines[last]);
        }
        Ok(())
    }
}

/// A lattice of a search range, refused when it is too large (`lattice`).
fn grid(minimum: f64, maximum: f64, step: f64) -> Result<Vec<f64>, SensorLagError> {
    lattice(minimum, maximum, step).map_err(|_| SensorLagError::GridTooLarge)
}

/// The number of points of a search range, `round((maximum − minimum)/step) + 1` (`size` in `fitSensorLagStep`). The
/// rounding is JavaScript's `Math.round`, which rounds a half upward: `⌊x + ½⌋`.
fn grid_size(range: SearchRange) -> f64 {
    (((range.maximum - range.minimum) / range.step) + 0.5).floor() + 1.0
}

/// Whether a value lies on the edge of its search range. A dead time of zero is not a boundary when zero is allowed.
fn on_grid_edge(value: f64, range: SearchRange, zero_allowed: bool) -> bool {
    !(zero_allowed && value == 0.0)
        && ((value - range.maximum).abs() < EPSILON || (value - range.minimum).abs() < EPSILON)
}

/// Whether two values are equal to within the search's tolerance.
fn near(value: f64, other: f64) -> bool {
    (value - other).abs() < EPSILON
}

/// The index of a value clamped to `first..=last`, where the value is a lattice index held as a float.
fn clamp_index(value: f64, first: usize, last: usize) -> usize {
    to_index(value.max(count_f64(first)).min(count_f64(last)))
}

/// Widens an interval to include a value; the first value starts it.
fn widen(interval: &mut Option<ParameterInterval>, value: f64) {
    *interval = Some(match *interval {
        None => ParameterInterval {
            minimum: value,
            maximum: value,
        },
        Some(known) => ParameterInterval {
            minimum: known.minimum.min(value),
            maximum: known.maximum.max(value),
        },
    });
}

/// A count as a float. A count of samples or of grid points is far below 2⁵³, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

/// The index a non-negative, integral float holds. The callers keep the value within the lattice, so it is never negative.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_index(value: f64) -> usize {
    value as usize
}

#[cfg(test)]
#[path = "sensor_lag_tests.rs"]
mod tests;
