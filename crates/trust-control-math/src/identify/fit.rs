//! Low-order step-response identification (N2.1, N2.2, N3.1, N3.2): the least-squares grid fit of one form to a record
//! (`fit_form`), the free-running prediction of a fitted form on a record (`predict`), and the validated fit, which fits
//! each form on the estimation record and scores it on the validation record (`identify_low_order_models`). Every path
//! fits through `fit_form`. The choice of the recommended form is a product rule (N3.3) and is made by `trust-control`.

use super::response::{delay_into, step_amplitude, LowOrderShape};
use super::score::final_tenth_start;
use super::simulate::{decay_factors, respond, unit_response, Dynamics, Lag};
use super::single_run::largest_input_change;
use super::{
    count_f64, LowOrderIdentificationOptions, LowOrderModelFit, LowOrderModelKind,
    LowOrderRejection, LowOrderUncertainty, ParameterInterval, SampleQuality, TimeDomainSample,
    UncertaintyMethod,
};
use crate::sampling::statistics::mean;
use crate::sampling::{largest, median_spacing, smallest};

/// The points of the time-constant grid of a first-order form (N2.2: 45 points).
const FIRST_ORDER_POINTS: usize = 45;
/// The points of the time-constant grid of each lag of a second-order form (N2.2: 28 points).
const SECOND_ORDER_POINTS: usize = 28;
/// The delay grid spans from zero to this share of the fitted span `H` (N2.2: `0.3·H`).
const DELAY_SPAN_SHARE: f64 = 0.3;
/// The delay grid has at most this many intervals, so its step is at least `0.3·H / 50` (N2.2).
const DELAY_INTERVALS: f64 = 50.0;
/// The time-constant grid runs from the sample interval to this multiple of `H` (N2.2: `10·H`).
const TIME_CONSTANT_SPAN: f64 = 10.0;
/// A time constant is refined over this many points spaced evenly on a logarithmic scale (N2.2).
const REFINEMENT_POINTS: usize = 10;
/// The refinement passes around the best point (N2.2): the first between the best grid point's neighbouring grid values,
/// each further one between the best point's neighbouring values of the pass before.
const REFINEMENT_PASSES: usize = 3;
/// The tolerance, in sample intervals, with which a delay counts as a whole number of them.
const WHOLE_SAMPLE_TOLERANCE: f64 = 1e-9;
/// The fewest samples before the step of a validated record (`stepIndex < 3` is refused).
const MINIMUM_SAMPLES_BEFORE_STEP: usize = 3;
/// The fewest samples after the step of a validated record (`stepIndex >= length − 10` is refused).
const MINIMUM_SAMPLES_AFTER_STEP: usize = 10;
/// The fewest samples a fit may be given (`minimumSamples < 4` is refused).
const MINIMUM_FIT_SAMPLES: usize = 4;
/// The smallest denominator of a least-squares gain: the simulated response must have more energy than this.
const MINIMUM_ENERGY: f64 = f64::EPSILON;

/// A record prepared for the least-squares fit (N1.3, N2.2): the samples from the start of the steady baseline window to the
/// last fitted sample, the baseline means, and the quantities the grids are built from.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRecord {
    /// The sample times in seconds, strictly increasing.
    pub times: Vec<f64>,
    /// The plant input deviation `dₖ = uₖ − u₀` at each sample, from the baseline mean `u₀` (N2.2).
    pub deviations: Vec<f64>,
    /// The measured output at each sample.
    pub outputs: Vec<f64>,
    /// The index of the first hold sample; the samples before it are the steady baseline (N1.3, N2.2).
    pub hold_start: usize,
    /// The baseline mean of the measured output, `y₀` (N1.3).
    pub output0: f64,
    /// The span of the measured output over the record (N1.1), above zero: the scale of every normalized error (N3.2).
    pub span: f64,
    /// The sample interval `Ts` of the fit: the median spacing of the times (N2.2).
    pub sample_interval_seconds: f64,
    /// The fitted span `H`, from the first hold sample to the last sample (N2.2).
    pub horizon_seconds: f64,
}

/// The free-running prediction of a form with a gain on a record (N3.1, N3.2): the unit-gain response to the recorded
/// input, the residuals `yₖ − (y₀ + K·sₖ)` of every sample, and the normalized errors over the hold.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    /// The unit-gain response `sₖ` of the form to the recorded input at each sample.
    pub unit_response: Vec<f64>,
    /// The residuals `yₖ − (y₀ + K·sₖ)` at every sample of the record.
    pub residuals: Vec<f64>,
    /// The normalized error over the whole hold: `√(mean of eₖ² over the hold) / span` (N3.2).
    pub normalized_error: f64,
    /// The normalized error over the final `max(3, ⌈0.1·N⌉)` samples of the hold, `N` being the hold's length (N3.2).
    pub final_normalized_error: f64,
}

/// A form fitted to a record by the least-squares grid search of N2.2, with its free-running prediction on that record.
#[derive(Debug, Clone, PartialEq)]
pub struct FormFit {
    /// The model form.
    pub kind: LowOrderModelKind,
    /// The least-squares gain `K`, or the rate gain `K′` of an integrating form (N2.2).
    pub gain: f64,
    /// The dynamic shape of the best candidate: its time constants, and its dead time for a form with one.
    pub shape: LowOrderShape,
    /// Whether the best grid point lies on an edge of its grid, a search-boundary finding that N2.4 keeps: the longest
    /// delay, or either end of a time-constant grid. A best delay of zero is one-sided and is not one (N2.4).
    pub on_search_boundary: bool,
    /// The free-running prediction on the fitted record, with its normalized errors (N3.1, N3.2).
    pub prediction: Prediction,
}

/// Identifies every family of `options.allowed_kinds` from the estimation record and checks each one on the validation
/// record (N2, N3.2, N3.4).
///
/// The estimation record fixes each family's gain and dynamics by [`fit_form`]. Each fitted form is then simulated on the
/// validation record under its recorded input, from that record's own steady baseline, and scored there: its normalized
/// error over the whole hold and over the final set of the hold (N3.2), each divided by the span of the validation output.
/// The result holds every family that could be fitted, once each, in the order of `allowed_kinds`; the caller chooses the
/// recommended form among them (N3.3).
///
/// The two records must each hold a step: a good sample at every instant, strictly increasing times, an input span of at
/// least `minimum_input_span`, three samples before and ten after the largest input change, and an input change of at
/// least `minimum_input_span`. The output of each record must change, because an error is normalised by its span.
///
/// # Errors
/// Returns the [`LowOrderRejection`] that names why the identification is refused: non-finite input or options, too few
/// samples, no excitation, no step, or an output that does not change.
pub fn identify_low_order_models(
    estimation: &[TimeDomainSample],
    validation: &[TimeDomainSample],
    options: &LowOrderIdentificationOptions,
) -> Result<Vec<LowOrderModelFit>, LowOrderRejection> {
    if !options_are_valid(options) {
        return Err(LowOrderRejection::NonfiniteInput);
    }
    let estimation_record = step_record(estimation, options)?;
    let validation_record = step_record(validation, options)?;
    let mut attempted: Vec<LowOrderModelKind> = Vec::new();
    let mut models = Vec::new();
    for &kind in &options.allowed_kinds {
        if attempted.contains(&kind) {
            continue;
        }
        attempted.push(kind);
        if let Some(form) = fit_form(&estimation_record, kind) {
            models.push(validated_fit(
                form,
                &validation_record,
                estimation_record.sample_interval_seconds,
            ));
        }
    }
    Ok(models)
}

/// Whether the options are usable: at least four samples, a positive finite minimum input span.
fn options_are_valid(options: &LowOrderIdentificationOptions) -> bool {
    options.minimum_samples >= MINIMUM_FIT_SAMPLES
        && options.minimum_input_span.is_finite()
        && options.minimum_input_span > 0.0
}

/// Checks a validated record and prepares it for the fit: its samples, its step (the largest input change, with enough
/// samples on either side) and the step amplitude `Δu = ū_hold − u₀` (N2.2).
fn step_record(
    samples: &[TimeDomainSample],
    options: &LowOrderIdentificationOptions,
) -> Result<PreparedRecord, LowOrderRejection> {
    if samples.len() < options.minimum_samples {
        return Err(LowOrderRejection::InsufficientData);
    }
    let ordered = samples.iter().enumerate().all(|(index, sample)| {
        sample.quality == SampleQuality::Good
            && sample.time_seconds.is_finite()
            && sample.input.is_finite()
            && sample.output.is_finite()
            && (index == 0 || sample.time_seconds > samples[index - 1].time_seconds)
    });
    if !ordered {
        return Err(LowOrderRejection::NonfiniteInput);
    }
    let times: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    let inputs: Vec<f64> = samples.iter().map(|sample| sample.input).collect();
    let outputs: Vec<f64> = samples.iter().map(|sample| sample.output).collect();
    if span(&inputs) < options.minimum_input_span {
        return Err(LowOrderRejection::PoorExcitation);
    }
    let step_index = largest_input_change(&inputs)
        .filter(|&index| {
            index >= MINIMUM_SAMPLES_BEFORE_STEP
                && index + MINIMUM_SAMPLES_AFTER_STEP < samples.len()
        })
        .ok_or(LowOrderRejection::MissingStep)?;
    let input_change = step_amplitude(mean(&inputs[..step_index]), &inputs[step_index..]);
    if input_change.abs() < options.minimum_input_span {
        return Err(LowOrderRejection::PoorExcitation);
    }
    prepare_record(&times, &inputs, &outputs, step_index)
}

/// Prepares a record for the fit (N1.3, N2.2): the baseline means over the samples before `hold_start`, the plant input
/// deviations, the output span, the sample interval and the fitted span.
///
/// # Errors
/// Returns [`LowOrderRejection::InsufficientData`] when the series differ in length, when `hold_start` leaves no baseline
/// sample, or when fewer than two hold samples remain; [`LowOrderRejection::NonfiniteInput`] when a value is not finite or a
/// time does not increase; and [`LowOrderRejection::FlatOutput`] when the measured output does not change, so that no error
/// has a span to be divided by (N3.2).
pub fn prepare_record(
    times: &[f64],
    inputs: &[f64],
    outputs: &[f64],
    hold_start: usize,
) -> Result<PreparedRecord, LowOrderRejection> {
    let count = times.len();
    if inputs.len() != count || outputs.len() != count || hold_start < 1 || count < hold_start + 2 {
        return Err(LowOrderRejection::InsufficientData);
    }
    let finite = times
        .iter()
        .chain(inputs)
        .chain(outputs)
        .all(|value| value.is_finite());
    let increasing = times.windows(2).all(|pair| pair[1] > pair[0]);
    if !finite || !increasing {
        return Err(LowOrderRejection::NonfiniteInput);
    }
    let span = span(outputs);
    if span <= 0.0 {
        return Err(LowOrderRejection::FlatOutput);
    }
    let baseline_input = mean(&inputs[..hold_start]);
    Ok(PreparedRecord {
        times: times.to_vec(),
        deviations: inputs.iter().map(|input| input - baseline_input).collect(),
        outputs: outputs.to_vec(),
        hold_start,
        output0: mean(&outputs[..hold_start]),
        span,
        sample_interval_seconds: median_spacing(times),
        horizon_seconds: times[count - 1] - times[hold_start],
    })
}

/// The free-running prediction of a form with gain `K` on a record (N3.1, N3.2): the unit-gain response to the recorded
/// input from the steady baseline, the residuals `yₖ − (y₀ + K·sₖ)`, and the normalized errors over the hold. The
/// validation record is scored this way with the gain of its estimation record.
#[must_use]
pub fn predict(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    shape: LowOrderShape,
    gain: f64,
) -> Prediction {
    let response = unit_response(kind, shape, &record.times, &record.deviations);
    let residuals: Vec<f64> = record
        .outputs
        .iter()
        .zip(&response)
        .map(|(output, unit)| output - (record.output0 + gain * unit))
        .collect();
    let hold = &residuals[record.hold_start..];
    let final_start = final_tenth_start(hold.len());
    let normalized_error = root_mean_square(hold) / record.span;
    let final_normalized_error = root_mean_square(&hold[final_start..]) / record.span;
    Prediction {
        unit_response: response,
        residuals,
        normalized_error,
        final_normalized_error,
    }
}

/// Fits one form to a record by the least-squares grid search of N2.2, with its prediction on the record.
///
/// For every candidate shape on the grids, the simulated unit-gain response `sₖ` to the recorded deviations is taken, the
/// gain `K = Σ(yₖ − y₀)·sₖ / Σsₖ²` and the error `E = Σ(yₖ − y₀ − K·sₖ)²` are computed over the hold samples, and the
/// candidate with the smallest `E` wins; of equal errors, the first in grid order (delay, then time constants, each
/// ascending). The best grid point is then refined (N2.2), and the refined point replaces it only when its error is smaller.
/// The result is `None` for a form whose candidates all have no finite gain.
#[must_use]
pub fn fit_form(record: &PreparedRecord, kind: LowOrderModelKind) -> Option<FormFit> {
    let grids = Grids::new(record, kind);
    let mut buffers = Buffers::new(record.times.len());
    let grid_best = grid_search(record, kind, &grids, &mut buffers)?;
    let refined = refine(record, kind, &grids, grid_best, &mut buffers);
    let best = match refined {
        Some(candidate) if candidate.squared_error < grid_best.candidate.squared_error => candidate,
        _ => grid_best.candidate,
    };
    // N2.4: a best delay of zero is one-sided, so it is no search boundary; only the grid point's own edge rule applies.
    Some(FormFit {
        kind,
        gain: best.gain,
        shape: best.shape,
        on_search_boundary: grid_best.on_edge,
        prediction: predict(record, kind, best.shape, best.gain),
    })
}

/// The grids of a form's search (N2.2): the delays, the time constants and their decay factors on the record's intervals.
struct Grids {
    /// The delays in seconds, from zero, in ascending order; only zero for a form without a dead time.
    delays: Vec<f64>,
    /// The time constants in seconds, log-spaced from the sample interval to `10·H`; empty for a form without one.
    time_constants: Vec<f64>,
    /// The decay factors of each time constant on the record's intervals, one vector per time constant.
    decays: Vec<Vec<f64>>,
}

impl Grids {
    /// The grids of a form on a record (N2.2).
    fn new(record: &PreparedRecord, kind: LowOrderModelKind) -> Self {
        let delays = if has_dead_time(kind) {
            delay_grid(record)
        } else {
            vec![0.0]
        };
        let count = match kind {
            LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
                FIRST_ORDER_POINTS
            }
            LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
                SECOND_ORDER_POINTS
            }
            LowOrderModelKind::StaticGain | LowOrderModelKind::Integrating => 0,
        };
        let time_constants = if count == 0 {
            Vec::new()
        } else {
            logspace(
                record.sample_interval_seconds,
                TIME_CONSTANT_SPAN * record.horizon_seconds,
                count,
            )
        };
        let decay_per_interval = time_constants
            .iter()
            .map(|&tau| decay_factors(&record.times, tau))
            .collect();
        Self {
            delays,
            time_constants,
            decays: decay_per_interval,
        }
    }
}

/// The working vectors of a search, allocated once for the form: the delayed deviations and the simulated response.
struct Buffers {
    /// The plant input deviations delayed by the candidate's dead time, one per sample.
    delayed: Vec<f64>,
    /// The unit-gain response of the candidate, one per sample.
    response: Vec<f64>,
}

impl Buffers {
    /// Buffers for a record of `count` samples.
    fn new(count: usize) -> Self {
        Self {
            delayed: vec![0.0; count],
            response: vec![0.0; count],
        }
    }
}

/// A candidate of the search: its shape, its least-squares gain and the squared error it leaves over the hold (N2.2).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Candidate {
    /// The time constants and dead time of the candidate.
    shape: LowOrderShape,
    /// The least-squares gain, finite for every candidate that is offered.
    gain: f64,
    /// The squared error `E` over the hold samples.
    squared_error: f64,
}

/// The best point of the grid search with the indices that place it on the grids, for its refinement.
#[derive(Debug, Clone, Copy)]
struct GridBest {
    /// The best candidate.
    candidate: Candidate,
    /// The index of its delay on the delay grid.
    delay_index: usize,
    /// The index of its first time constant on the time-constant grid.
    first_index: usize,
    /// The index of its second time constant on the time-constant grid, for a second-order form.
    second_index: usize,
    /// Whether the point lies on the edge of its grid: the longest delay, or the first or last time constant.
    on_edge: bool,
}

/// The position of a grid point: its delay, its time-constant indices and whether it lies on the edge of the grid.
#[derive(Debug, Clone, Copy)]
struct GridPoint {
    /// The index of the delay on the delay grid.
    delay_index: usize,
    /// The index of the first time constant (the larger one of a second-order pair).
    first_index: usize,
    /// The index of the second time constant (the smaller one of a second-order pair).
    second_index: usize,
    /// Whether the point lies on the edge of its grid.
    on_edge: bool,
}

/// One delay of the grid search: its index and value on the delay grid, and whether it is the longest delay of a grid that
/// has more than one, a search boundary that N2.4 keeps.
#[derive(Debug, Clone, Copy)]
struct DelayPass {
    /// The index of the delay on the delay grid.
    index: usize,
    /// The delay in seconds.
    delay: f64,
    /// Whether the delay is the longest of its grid, which lies on the edge of the grid.
    on_edge: bool,
}

/// The best grid point of a form (N2.2): the candidate with the smallest error, the first in grid order on a tie. `None`
/// when no grid point has a finite gain.
fn grid_search(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    grids: &Grids,
    buffers: &mut Buffers,
) -> Option<GridBest> {
    let mut best: Option<GridBest> = None;
    let delay_count = grids.delays.len();
    // The longest delay of a dead-time grid is a search boundary when the grid has more than one delay.
    let delay_is_bounded = has_dead_time(kind) && delay_count > 1;
    for (index, &delay) in grids.delays.iter().enumerate() {
        delay_into(
            &record.times,
            &record.deviations,
            delay,
            &mut buffers.delayed,
        );
        let pass = DelayPass {
            index,
            delay,
            on_edge: delay_is_bounded && index + 1 == delay_count,
        };
        match kind {
            LowOrderModelKind::StaticGain => {
                let candidate = candidate(
                    record,
                    Dynamics::Static,
                    shape_of(kind, delay, None, None),
                    buffers,
                );
                offer_grid(&mut best, candidate, single_point(pass));
            }
            LowOrderModelKind::Integrating => {
                let candidate = candidate(
                    record,
                    Dynamics::Integrating,
                    shape_of(kind, delay, None, None),
                    buffers,
                );
                offer_grid(&mut best, candidate, single_point(pass));
            }
            LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
                first_order_pass(record, kind, grids, pass, buffers, &mut best);
            }
            LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
                second_order_pass(record, kind, grids, pass, buffers, &mut best);
            }
        }
    }
    best
}

/// The grid point of a form without time constants at one delay.
fn single_point(pass: DelayPass) -> GridPoint {
    GridPoint {
        delay_index: pass.index,
        first_index: 0,
        second_index: 0,
        on_edge: pass.on_edge,
    }
}

/// The first-order grid points at one delay (N2.2): every time constant of the grid, in ascending order.
fn first_order_pass(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    grids: &Grids,
    pass: DelayPass,
    buffers: &mut Buffers,
    best: &mut Option<GridBest>,
) {
    let count = grids.time_constants.len();
    for (first_index, (&tau, decay)) in grids.time_constants.iter().zip(&grids.decays).enumerate() {
        let lag = Lag {
            time_constant_seconds: tau,
            decay: decay.as_slice(),
        };
        let candidate = candidate(
            record,
            Dynamics::FirstOrder(lag),
            shape_of(kind, pass.delay, Some(tau), None),
            buffers,
        );
        let on_edge = pass.on_edge || at_grid_end(first_index, count);
        offer_grid(
            best,
            candidate,
            GridPoint {
                delay_index: pass.index,
                first_index,
                second_index: 0,
                on_edge,
            },
        );
    }
}

/// The second-order grid points at one delay (N2.2): every pair of time constants with the first not below the second.
fn second_order_pass(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    grids: &Grids,
    pass: DelayPass,
    buffers: &mut Buffers,
    best: &mut Option<GridBest>,
) {
    let count = grids.time_constants.len();
    let entries: Vec<(f64, &Vec<f64>)> = grids
        .time_constants
        .iter()
        .copied()
        .zip(&grids.decays)
        .collect();
    for (second_index, &(second_tau, second_decay)) in entries.iter().enumerate() {
        for (first_index, &(first_tau, first_decay)) in
            entries.iter().enumerate().skip(second_index)
        {
            let dynamics = Dynamics::SecondOrder {
                first: Lag {
                    time_constant_seconds: first_tau,
                    decay: first_decay.as_slice(),
                },
                second: Lag {
                    time_constant_seconds: second_tau,
                    decay: second_decay.as_slice(),
                },
            };
            let candidate = candidate(
                record,
                dynamics,
                shape_of(kind, pass.delay, Some(first_tau), Some(second_tau)),
                buffers,
            );
            let on_edge =
                pass.on_edge || at_grid_end(first_index, count) || at_grid_end(second_index, count);
            offer_grid(
                best,
                candidate,
                GridPoint {
                    delay_index: pass.index,
                    first_index,
                    second_index,
                    on_edge,
                },
            );
        }
    }
}

/// Refines the best grid point (N2.2) in [`REFINEMENT_PASSES`] passes. Each pass searches the delay in whole sample
/// intervals between the best point's two neighbouring delays, together with each time constant on [`REFINEMENT_POINTS`]
/// log-spaced values between its two neighbouring values: on the grids for the first pass, on the values of the pass before
/// for each further one. The result is the refined candidate with the smallest error, or `None` when the form has no
/// refinement.
fn refine(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    grids: &Grids,
    grid_best: GridBest,
    buffers: &mut Buffers,
) -> Option<Candidate> {
    if kind == LowOrderModelKind::StaticGain {
        return None;
    }
    let second_order = matches!(
        kind,
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime
    );
    let mut axes = Axes {
        delays: grids.delays.clone(),
        first: grids.time_constants.clone(),
        second: if second_order {
            grids.time_constants.clone()
        } else {
            Vec::new()
        },
    };
    let mut point = grid_best;
    let mut best: Option<Candidate> = None;
    for _ in 0..REFINEMENT_PASSES {
        let delays = refined_delays(record, kind, &axes.delays, point.delay_index);
        let first = RefinedSet::new(record, &axes.first, point.first_index, kind);
        let second = if second_order {
            RefinedSet::new(record, &axes.second, point.second_index, kind)
        } else {
            RefinedSet::empty()
        };
        let mut pass_best: Option<GridBest> = None;
        for (delay_index, &delay) in delays.iter().enumerate() {
            delay_into(
                &record.times,
                &record.deviations,
                delay,
                &mut buffers.delayed,
            );
            refine_delay(
                record,
                kind,
                (delay_index, delay),
                (&first, &second),
                buffers,
                &mut pass_best,
            );
        }
        let Some(found) = pass_best else {
            break;
        };
        if best.is_none_or(|current| found.candidate.squared_error < current.squared_error) {
            best = Some(found.candidate);
        }
        axes = Axes {
            delays,
            first: first.values,
            second: second.values,
        };
        point = found;
    }
    best
}

/// The values a refinement pass searches around its best point: the delays and the time constants of each lag, ascending.
struct Axes {
    /// The delays in seconds.
    delays: Vec<f64>,
    /// The values of the first time constant in seconds.
    first: Vec<f64>,
    /// The values of the second time constant in seconds; empty for a form without one.
    second: Vec<f64>,
}

/// The refinement values of one time constant on the record: the values and their decay factors.
struct RefinedSet {
    /// The refined time constants in seconds.
    values: Vec<f64>,
    /// The decay factors of each refined time constant on the record's intervals.
    decays: Vec<Vec<f64>>,
}

impl RefinedSet {
    /// The refinement of the time constant at `index` of the grid, with its decay factors on the record.
    fn new(
        record: &PreparedRecord,
        time_constants: &[f64],
        index: usize,
        kind: LowOrderModelKind,
    ) -> Self {
        let values = refined_values(time_constants, index, kind);
        let decays = values
            .iter()
            .map(|&tau| decay_factors(&record.times, tau))
            .collect();
        Self { values, decays }
    }

    /// The refinement of a form without this time constant: no values.
    fn empty() -> Self {
        Self {
            values: Vec::new(),
            decays: Vec::new(),
        }
    }
}

/// The refined candidates at one delay (N2.2), each simulated with its refined time constants and offered with its place on
/// the pass's values, the first in their order winning a tie.
fn refine_delay(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    (delay_index, delay): (usize, f64),
    (first, second): (&RefinedSet, &RefinedSet),
    buffers: &mut Buffers,
    best: &mut Option<GridBest>,
) {
    let place = |first_index: usize, second_index: usize| GridPoint {
        delay_index,
        first_index,
        second_index,
        on_edge: false,
    };
    match kind {
        LowOrderModelKind::StaticGain => {}
        LowOrderModelKind::Integrating => {
            let candidate = candidate(
                record,
                Dynamics::Integrating,
                shape_of(kind, delay, None, None),
                buffers,
            );
            offer_grid(best, candidate, place(0, 0));
        }
        LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
            for (first_index, (&tau, decay)) in first.values.iter().zip(&first.decays).enumerate() {
                let lag = Lag {
                    time_constant_seconds: tau,
                    decay: decay.as_slice(),
                };
                let candidate = candidate(
                    record,
                    Dynamics::FirstOrder(lag),
                    shape_of(kind, delay, Some(tau), None),
                    buffers,
                );
                offer_grid(best, candidate, place(first_index, 0));
            }
        }
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
            for (first_index, (&first_tau, first_decay)) in
                first.values.iter().zip(&first.decays).enumerate()
            {
                for (second_index, (&second_tau, second_decay)) in
                    second.values.iter().zip(&second.decays).enumerate()
                {
                    let dynamics = Dynamics::SecondOrder {
                        first: Lag {
                            time_constant_seconds: first_tau,
                            decay: first_decay.as_slice(),
                        },
                        second: Lag {
                            time_constant_seconds: second_tau,
                            decay: second_decay.as_slice(),
                        },
                    };
                    let candidate = candidate(
                        record,
                        dynamics,
                        shape_of(kind, delay, Some(first_tau), Some(second_tau)),
                        buffers,
                    );
                    offer_grid(best, candidate, place(first_index, second_index));
                }
            }
        }
    }
}

/// The delays searched by a refinement pass: the whole sample intervals between the delays next to the best one of a
/// dead-time form, and zero for a form without a dead time.
fn refined_delays(
    record: &PreparedRecord,
    kind: LowOrderModelKind,
    delays: &[f64],
    delay_index: usize,
) -> Vec<f64> {
    if !has_dead_time(kind) {
        return vec![0.0];
    }
    let (lower, upper) = neighbours(delays, delay_index);
    whole_sample_delays(record.sample_interval_seconds, lower, upper)
}

/// The refinement values of one time constant: [`REFINEMENT_POINTS`] values log-spaced between the values next to the
/// best one on the grid or on the pass before. A form without a time constant has none.
fn refined_values(time_constants: &[f64], index: usize, kind: LowOrderModelKind) -> Vec<f64> {
    match kind {
        LowOrderModelKind::FirstOrder
        | LowOrderModelKind::FirstOrderDeadTime
        | LowOrderModelKind::SecondOrder
        | LowOrderModelKind::SecondOrderDeadTime => {
            let (lower, upper) = neighbours(time_constants, index);
            logspace(lower, upper, REFINEMENT_POINTS)
        }
        LowOrderModelKind::StaticGain | LowOrderModelKind::Integrating => Vec::new(),
    }
}

/// The shape of a candidate: its time constants, and its dead time for a form that has one (zero delay included).
fn shape_of(
    kind: LowOrderModelKind,
    delay_seconds: f64,
    first_tau: Option<f64>,
    second_tau: Option<f64>,
) -> LowOrderShape {
    LowOrderShape {
        time_constant_seconds: first_tau,
        second_time_constant_seconds: second_tau,
        dead_time_seconds: has_dead_time(kind).then_some(delay_seconds),
    }
}

/// Simulates one candidate on the record into the buffers (the delayed deviations are already in place) and returns its
/// least-squares gain and squared error over the hold.
fn candidate(
    record: &PreparedRecord,
    dynamics: Dynamics<'_>,
    shape: LowOrderShape,
    buffers: &mut Buffers,
) -> Candidate {
    respond(
        dynamics,
        &record.times,
        &buffers.delayed,
        &mut buffers.response,
    );
    let (gain, squared_error) = evaluate(record, &buffers.response);
    Candidate {
        shape,
        gain,
        squared_error,
    }
}

/// The least-squares gain `K = Σ(yₖ − y₀)·sₖ / Σsₖ²` and the squared error `E = Σ(yₖ − y₀ − K·sₖ)²` of a response over the
/// hold samples (N2.2). The gain is `NaN` and the error infinite when the response has no energy.
fn evaluate(record: &PreparedRecord, response: &[f64]) -> (f64, f64) {
    let outputs = &record.outputs[record.hold_start..];
    let units = &response[record.hold_start..];
    let numerator: f64 = outputs
        .iter()
        .zip(units)
        .map(|(output, unit)| (output - record.output0) * unit)
        .sum();
    let energy: f64 = units.iter().map(|unit| unit * unit).sum();
    if energy <= MINIMUM_ENERGY {
        return (f64::NAN, f64::INFINITY);
    }
    let gain = numerator / energy;
    let squared_error: f64 = outputs
        .iter()
        .zip(units)
        .map(|(output, unit)| {
            let residual = output - record.output0 - gain * unit;
            residual * residual
        })
        .sum();
    (gain, squared_error)
}

/// Keeps a grid candidate when its gain is finite and its error is strictly smaller than the best so far, so the first
/// candidate of the grid order wins a tie.
fn offer_grid(best: &mut Option<GridBest>, candidate: Candidate, point: GridPoint) {
    let improves = candidate.gain.is_finite()
        && best.is_none_or(|current| candidate.squared_error < current.candidate.squared_error);
    if improves {
        *best = Some(GridBest {
            candidate,
            delay_index: point.delay_index,
            first_index: point.first_index,
            second_index: point.second_index,
            on_edge: point.on_edge,
        });
    }
}

/// Whether a dead time family has a dead time: the delayed first and second order forms and the integrator (N2.1).
fn has_dead_time(kind: LowOrderModelKind) -> bool {
    matches!(
        kind,
        LowOrderModelKind::FirstOrderDeadTime
            | LowOrderModelKind::Integrating
            | LowOrderModelKind::SecondOrderDeadTime
    )
}

/// Whether an index lies at either end of a grid of `count` values.
fn at_grid_end(index: usize, count: usize) -> bool {
    index == 0 || index + 1 == count
}

/// The delay grid of N2.2: from zero to `0.3·H` in equal steps of `max(Ts, 0.3·H / 50)`, so at most 51 values.
fn delay_grid(record: &PreparedRecord) -> Vec<f64> {
    let span = DELAY_SPAN_SHARE * record.horizon_seconds;
    let step = (span / DELAY_INTERVALS).max(record.sample_interval_seconds);
    multiples(step, 0.0, (span / step + WHOLE_SAMPLE_TOLERANCE).floor())
}

/// The delays that are whole numbers of sample intervals within `[lower, upper]` (N2.2 refinement). Their interval is at
/// least two sample intervals in every grid, so the list is not empty; `lower` stands in when no whole interval lies in it.
fn whole_sample_delays(sample_interval_seconds: f64, lower: f64, upper: f64) -> Vec<f64> {
    let first = (lower / sample_interval_seconds - WHOLE_SAMPLE_TOLERANCE)
        .ceil()
        .max(0.0);
    let last = (upper / sample_interval_seconds + WHOLE_SAMPLE_TOLERANCE).floor();
    let delays = multiples(sample_interval_seconds, first, last);
    if delays.is_empty() {
        vec![lower]
    } else {
        delays
    }
}

/// The values `k·step` for the whole numbers `k` from `first` to `last`, both included, in ascending order.
fn multiples(step: f64, first: f64, last: f64) -> Vec<f64> {
    let mut values = Vec::new();
    let mut index = first;
    while index <= last {
        values.push(index * step);
        index += 1.0;
    }
    values
}

/// The values log-spaced from `minimum` to `maximum`, both included: `minimum·(maximum/minimum)^(i/(count − 1))` for
/// `i = 0, …, count − 1` (N2.2). A count below two gives `minimum` alone.
fn logspace(minimum: f64, maximum: f64, count: usize) -> Vec<f64> {
    if count < 2 {
        return vec![minimum];
    }
    let ratio = maximum / minimum;
    (0..count)
        .map(|index| minimum * ratio.powf(count_f64(index) / count_f64(count - 1)))
        .collect()
}

/// The values on each side of a grid index, the grid value below and the grid value above it, clamped to the grid's ends
/// (N2.2 refinement). An empty grid gives zeros, which no caller reaches.
fn neighbours(values: &[f64], index: usize) -> (f64, f64) {
    let at = values.get(index).copied().unwrap_or(0.0);
    let below = values.get(index.saturating_sub(1)).copied().unwrap_or(at);
    let above = values.get(index + 1).copied().unwrap_or(at);
    (below, above)
}

/// The root-mean-square of values, `√(Σvᵢ² / n)`; zero for no values.
fn root_mean_square(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let squares: f64 = values.iter().map(|value| value * value).sum();
    (squares / count_f64(values.len())).sqrt()
}

/// The span of values: their largest minus their smallest (N1.1).
fn span(values: &[f64]) -> f64 {
    largest(values, f64::NEG_INFINITY) - smallest(values)
}

/// The validated fit of one form (N3.2, N3.4): its estimation prediction, and its prediction on the validation record under
/// the same shape and gain, with the uncertainty intervals of the validation residual (`lowOrderModel` in the `TypeScript`).
fn validated_fit(
    form: FormFit,
    validation: &PreparedRecord,
    sample_interval_seconds: f64,
) -> LowOrderModelFit {
    let FormFit {
        kind,
        gain,
        shape,
        on_search_boundary,
        prediction,
    } = form;
    let scored = predict(validation, kind, shape, gain);
    let uncertainty_fraction = (2.0 * scored.normalized_error).clamp(0.02, 0.5);
    let time_fraction = (3.0 * scored.normalized_error).clamp(0.08, 0.6);
    // A dead time of zero is not reported: the families without one have no dead-time parameter.
    let dead_time_seconds = shape.dead_time_seconds.filter(|delay| *delay > 0.0);
    let uncertainty = LowOrderUncertainty {
        method: UncertaintyMethod::GridResolutionAndValidationResidual,
        gain: symmetric_interval(gain, (gain.abs() * uncertainty_fraction).max(1e-12)),
        time_constant_seconds: shape
            .time_constant_seconds
            .map(|tau| positive_interval(tau, tau * time_fraction)),
        second_time_constant_seconds: shape
            .second_time_constant_seconds
            .map(|tau| positive_interval(tau, tau * time_fraction)),
        dead_time_seconds: dead_time_seconds.map(|delay| {
            positive_interval(delay, sample_interval_seconds.max(delay * time_fraction))
        }),
    };
    LowOrderModelFit {
        kind,
        gain,
        time_constant_seconds: shape.time_constant_seconds,
        second_time_constant_seconds: shape.second_time_constant_seconds,
        dead_time_seconds,
        estimation_residuals: prediction.residuals,
        validation_residuals: scored.residuals,
        normalized_estimation_error: prediction.normalized_error,
        normalized_validation_error: scored.normalized_error,
        final_normalized_validation_error: scored.final_normalized_error,
        sample_interval_seconds,
        uncertainty,
        on_search_boundary,
    }
}

/// The symmetric interval `value ± half_width` (`symmetricInterval`).
fn symmetric_interval(value: f64, half_width: f64) -> ParameterInterval {
    ParameterInterval {
        minimum: value - half_width,
        maximum: value + half_width,
    }
}

/// The interval `value ± half_width` whose minimum is not below zero (`positiveInterval`).
fn positive_interval(value: f64, half_width: f64) -> ParameterInterval {
    ParameterInterval {
        minimum: (value - half_width).max(0.0),
        maximum: value + half_width,
    }
}

#[cfg(test)]
mod tests;
