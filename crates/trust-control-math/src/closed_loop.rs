//! The closed-loop time response of a discrete law in feedback with a sampled linear plant (B4.5 and
//! B9.6 of the Control Studio specification, and the simulation of `engineeringPlots.ts`).
//!
//! Each period the law gives one command from the setpoint and the measurement at the period's start.
//! The measurement passes a first-order filter `y_f ← y_f + α·(y − y_f)` with `α = T/(T_f + T)` first;
//! the command is saturated; a constant load is added to it; and the sum reaches the plant after its
//! transport delay, rounded to whole periods. The plant is one of the sampled forms of
//! [`SampledDynamics`], each advanced over one period with its input held. A zero-delay plant takes the
//! command of its own period at once, so the loop adds no period of delay of its own.
//!
//! The law is any [`DiscreteLaw`]. The Control.PID law and the product rules are in `trust-control`.

use std::collections::VecDeque;

/// The sampled plant forms (B9.6). Each is advanced over one period with its delayed input held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampledDynamics {
    /// No dynamics: the output is at `K` times the input change at once.
    Static,
    /// `K/(τs + 1)`, exact for a held input: `y ← y_t + (y − y_t)·e^(−T/τ)` with `y_t = K·Δu`.
    FirstOrder {
        /// The time constant `τ` in seconds, positive.
        time_constant_seconds: f64,
    },
    /// `K/((τ₁s + 1)(τ₂s + 1))`: the first lag and then the second, each advanced by forward Euler.
    SecondOrder {
        /// The first time constant `τ₁` in seconds, positive.
        first_time_constant_seconds: f64,
        /// The second time constant `τ₂` in seconds, positive.
        second_time_constant_seconds: f64,
    },
    /// `K/s`: the output integrates `K` times the input change, `y ← y + T·K·Δu`.
    Integrating,
    /// `K/(s(τs + 1))`, exact for a held input: the rate follows `K·Δu` through a lag `τ`, and the output
    /// integrates the rate.
    IntegratingWithLag {
        /// The lag time constant `τ` in seconds, positive.
        time_constant_seconds: f64,
    },
}

/// A sampled plant: its form, its static gain `K`, its input delay and its operating point. Inputs and
/// outputs are deviations from the operating point in the model, and the operating point is added back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampledPlant {
    /// The form of the plant.
    pub dynamics: SampledDynamics,
    /// The static gain `K` (the rate gain of an integrating form).
    pub gain: f64,
    /// The pure input delay `L` in seconds, zero or more.
    pub delay_seconds: f64,
    /// The operating input `u₀`.
    pub operating_input: f64,
    /// The operating output `y₀`, from which the response starts.
    pub operating_output: f64,
}

/// The settings of one run of the loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosedLoopRun {
    /// The actual period of the law in seconds.
    pub period_seconds: f64,
    /// The number of periods simulated.
    pub periods: usize,
    /// The setpoint, held from the first period.
    pub setpoint: f64,
    /// The first-order measurement filter time `T_f` in seconds; zero bypasses the filter.
    pub measurement_filter_seconds: f64,
    /// A constant load added to the command before the transport delay.
    pub load: f64,
    /// The lowest command the loop applies.
    pub command_minimum: f64,
    /// The highest command the loop applies.
    pub command_maximum: f64,
    /// The longest delay in periods that the run keeps; a longer delay is cut to this many periods.
    pub maximum_delay_periods: usize,
}

/// One period of a run: the output at the end of the period and the command applied in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosedLoopSample {
    /// The plant output at the end of the period.
    pub process_value: f64,
    /// The command of the period after saturation.
    pub command: f64,
}

/// A discrete control law: it gives one command per period.
pub trait DiscreteLaw {
    /// The refusal that the law gives when it cannot compute a command.
    type Refusal;

    /// The command of one period, from the setpoint and the measurement at its start. `period_seconds`
    /// is the actual interval of the period.
    ///
    /// # Errors
    /// Returns the law's refusal when it cannot compute the command for this period.
    fn command(
        &mut self,
        setpoint: f64,
        measurement: f64,
        period_seconds: f64,
    ) -> Result<f64, Self::Refusal>;
}

/// Why a run or a plant cannot be simulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunRefusal {
    /// The sample period is not finite and positive.
    #[error("the sample period is not finite and positive")]
    Period,
    /// The run has no periods.
    #[error("the run has no periods")]
    NoPeriods,
    /// A setting or a plant value is not finite.
    #[error("a run or plant value is not finite")]
    Nonfinite,
    /// The measurement filter time is negative.
    #[error("the measurement filter time is negative")]
    Filter,
    /// The delay is negative.
    #[error("the delay is negative")]
    Delay,
    /// A time constant is not positive.
    #[error("a time constant is not positive")]
    TimeConstant,
    /// The command minimum is above its maximum.
    #[error("the command minimum is above its maximum")]
    Saturation,
}

/// Why a run fails: a run or plant refusal, the law's refusal, or a value that is not finite during the run.
#[derive(Debug, Clone, PartialEq)]
pub enum ClosedLoopError<Refusal> {
    /// The run or the plant cannot be simulated.
    Run(RunRefusal),
    /// The law refused to compute a command.
    Law(Refusal),
    /// The plant output or the command became not finite during the run.
    NonFinite,
}

/// Simulates the closed loop over `run.periods` periods from the operating point, and returns one sample
/// per period.
///
/// # Errors
/// Returns [`ClosedLoopError::Run`] when the run or the plant is not usable, [`ClosedLoopError::Law`] when
/// the law refuses a period, and [`ClosedLoopError::NonFinite`] when the command or the plant output is not
/// finite.
pub fn simulate<L: DiscreteLaw>(
    plant: &SampledPlant,
    law: &mut L,
    run: &ClosedLoopRun,
) -> Result<Vec<ClosedLoopSample>, ClosedLoopError<L::Refusal>> {
    check_run(plant, run).map_err(ClosedLoopError::Run)?;
    let period = run.period_seconds;
    let filter_factor = period / (run.measurement_filter_seconds + period);
    let delay_periods = rounded_periods(plant.delay_seconds, period).min(run.maximum_delay_periods);
    let mut pending: VecDeque<f64> = VecDeque::from(vec![plant.operating_input; delay_periods]);
    let mut state = PlantState::at_operating_point(plant);
    let mut filtered = plant.operating_output;
    let mut samples = Vec::with_capacity(run.periods);
    for _ in 0..run.periods {
        filtered += filter_factor * (state.process_value - filtered);
        let raw = law
            .command(run.setpoint, filtered, period)
            .map_err(ClosedLoopError::Law)?;
        if !raw.is_finite() {
            return Err(ClosedLoopError::NonFinite);
        }
        let command = raw.max(run.command_minimum).min(run.command_maximum);
        let applied = command + run.load;
        pending.push_back(applied);
        let delayed = pending.pop_front().unwrap_or(applied);
        advance(plant, &mut state, delayed, period);
        if !(state.process_value.is_finite() && command.is_finite()) {
            return Err(ClosedLoopError::NonFinite);
        }
        samples.push(ClosedLoopSample {
            process_value: state.process_value,
            command,
        });
    }
    Ok(samples)
}

/// The plant's retained values: its output, and its second lag (or, for an integrating form, its rate).
#[derive(Debug, Clone, Copy)]
struct PlantState {
    process_value: f64,
    lag: f64,
}

impl PlantState {
    /// The state at the operating point: the output at `y₀`, and the lag at `y₀`, or at zero for an
    /// integrating form, whose lag holds the rate.
    fn at_operating_point(plant: &SampledPlant) -> Self {
        let lag = match plant.dynamics {
            SampledDynamics::Integrating | SampledDynamics::IntegratingWithLag { .. } => 0.0,
            _ => plant.operating_output,
        };
        Self {
            process_value: plant.operating_output,
            lag,
        }
    }
}

/// Advances the plant over one period with the delayed input held.
fn advance(plant: &SampledPlant, state: &mut PlantState, input: f64, period: f64) {
    let input_change = input - plant.operating_input;
    let target = plant.operating_output + plant.gain * input_change;
    match plant.dynamics {
        SampledDynamics::Static => state.process_value = target,
        SampledDynamics::FirstOrder {
            time_constant_seconds,
        } => {
            let decay = (-period / time_constant_seconds).exp();
            state.process_value = target + (state.process_value - target) * decay;
        }
        SampledDynamics::SecondOrder {
            first_time_constant_seconds,
            second_time_constant_seconds,
        } => {
            state.lag += period * (target - state.lag) / first_time_constant_seconds;
            state.process_value +=
                period * (state.lag - state.process_value) / second_time_constant_seconds;
        }
        SampledDynamics::Integrating => {
            state.process_value += period * plant.gain * input_change;
        }
        SampledDynamics::IntegratingWithLag {
            time_constant_seconds,
        } => {
            let rate = plant.gain * input_change;
            let decay = (-period / time_constant_seconds).exp();
            state.process_value +=
                rate * period + (state.lag - rate) * time_constant_seconds * (1.0 - decay);
            state.lag = rate + (state.lag - rate) * decay;
        }
    }
}

/// Checks the run and the plant before the first period.
fn check_run(plant: &SampledPlant, run: &ClosedLoopRun) -> Result<(), RunRefusal> {
    if !(run.period_seconds.is_finite() && run.period_seconds > 0.0) {
        return Err(RunRefusal::Period);
    }
    if run.periods == 0 {
        return Err(RunRefusal::NoPeriods);
    }
    let values = [
        run.setpoint,
        run.measurement_filter_seconds,
        run.load,
        run.command_minimum,
        run.command_maximum,
        plant.gain,
        plant.delay_seconds,
        plant.operating_input,
        plant.operating_output,
    ];
    if !values.iter().all(|value| value.is_finite()) {
        return Err(RunRefusal::Nonfinite);
    }
    if run.measurement_filter_seconds < 0.0 {
        return Err(RunRefusal::Filter);
    }
    if plant.delay_seconds < 0.0 {
        return Err(RunRefusal::Delay);
    }
    if run.command_minimum > run.command_maximum {
        return Err(RunRefusal::Saturation);
    }
    let time_constants = match plant.dynamics {
        SampledDynamics::Static | SampledDynamics::Integrating => Vec::new(),
        SampledDynamics::FirstOrder {
            time_constant_seconds,
        }
        | SampledDynamics::IntegratingWithLag {
            time_constant_seconds,
        } => vec![time_constant_seconds],
        SampledDynamics::SecondOrder {
            first_time_constant_seconds,
            second_time_constant_seconds,
        } => vec![first_time_constant_seconds, second_time_constant_seconds],
    };
    if time_constants
        .iter()
        .all(|time| time.is_finite() && *time > 0.0)
    {
        Ok(())
    } else {
        Err(RunRefusal::TimeConstant)
    }
}

/// The delay in whole periods: the delay over the period, rounded to the nearest whole number.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The delay is checked finite and not negative, and the caller caps the count.
fn rounded_periods(delay_seconds: f64, period_seconds: f64) -> usize {
    (delay_seconds / period_seconds).round() as usize
}

#[cfg(test)]
mod tests;
