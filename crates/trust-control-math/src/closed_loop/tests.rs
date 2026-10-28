#![allow(clippy::float_cmp)] // Zero delay, saturation and zero load are exact by construction, so they are compared exactly.

use std::convert::Infallible;

use super::*;

/// A proportional law on the error: `command = gain·(setpoint − measurement)`.
struct Proportional {
    gain: f64,
}

impl DiscreteLaw for Proportional {
    type Refusal = Infallible;

    fn command(
        &mut self,
        setpoint: f64,
        measurement: f64,
        _period: f64,
    ) -> Result<f64, Infallible> {
        Ok(self.gain * (setpoint - measurement))
    }
}

/// A proportional-integral law on the error, with the integral taken over the actual period.
struct ProportionalIntegral {
    proportional: f64,
    integral: f64,
    accumulated_error: f64,
}

impl DiscreteLaw for ProportionalIntegral {
    type Refusal = Infallible;

    fn command(&mut self, setpoint: f64, measurement: f64, period: f64) -> Result<f64, Infallible> {
        let error = setpoint - measurement;
        self.accumulated_error += error * period;
        Ok(self.proportional * error + self.integral * self.accumulated_error)
    }
}

/// A law that ignores the measurement, and records the measurements it receives.
struct Recorder {
    command: f64,
    measurements: Vec<f64>,
}

impl DiscreteLaw for Recorder {
    type Refusal = Infallible;

    fn command(
        &mut self,
        _setpoint: f64,
        measurement: f64,
        _period: f64,
    ) -> Result<f64, Infallible> {
        self.measurements.push(measurement);
        Ok(self.command)
    }
}

fn plant(dynamics: SampledDynamics, gain: f64, delay_seconds: f64) -> SampledPlant {
    SampledPlant {
        dynamics,
        gain,
        delay_seconds,
        operating_input: 0.0,
        operating_output: 0.0,
    }
}

fn run(period_seconds: f64, periods: usize, setpoint: f64) -> ClosedLoopRun {
    ClosedLoopRun {
        period_seconds,
        periods,
        setpoint,
        measurement_filter_seconds: 0.0,
        load: 0.0,
        command_minimum: -1.0e9,
        command_maximum: 1.0e9,
        maximum_delay_periods: 1_000_000,
    }
}

#[test]
fn proportional_control_of_a_first_order_plant_reaches_k_kp_over_one_plus_k_kp() {
    // y = K·Kp·(r − y) in steady state, so y = K·Kp/(1 + K·Kp)·r: with K = 2, Kp = 3, r = 1 it is 6/7.
    let first_order = plant(
        SampledDynamics::FirstOrder {
            time_constant_seconds: 1.0,
        },
        2.0,
        0.0,
    );
    let mut law = Proportional { gain: 3.0 };
    let samples =
        simulate(&first_order, &mut law, &run(0.01, 6000, 1.0)).expect("the run is valid");
    let final_value = samples.last().expect("the run has samples").process_value;
    assert!(
        (final_value - 6.0 / 7.0).abs() <= 1e-9,
        "final value {final_value}"
    );
}

#[test]
fn proportional_integral_control_of_a_first_order_plant_has_no_steady_error() {
    let first_order = plant(
        SampledDynamics::FirstOrder {
            time_constant_seconds: 1.0,
        },
        1.0,
        0.0,
    );
    let mut law = ProportionalIntegral {
        proportional: 1.0,
        integral: 1.0,
        accumulated_error: 0.0,
    };
    let samples =
        simulate(&first_order, &mut law, &run(0.01, 6000, 1.0)).expect("the run is valid");
    let final_value = samples.last().expect("the run has samples").process_value;
    assert!(
        (final_value - 1.0).abs() <= 1e-6,
        "final value {final_value}"
    );
}

#[test]
fn saturation_holds_the_command_at_its_limit() {
    let first_order = plant(
        SampledDynamics::FirstOrder {
            time_constant_seconds: 1.0,
        },
        1.0,
        0.0,
    );
    let mut law = Proportional { gain: 100.0 };
    let mut settings = run(0.01, 500, 1.0);
    settings.command_minimum = -1.0;
    settings.command_maximum = 1.0;
    let samples = simulate(&first_order, &mut law, &settings).expect("the run is valid");
    assert_eq!(
        samples[0].command, 1.0,
        "the first command is 100 · 1, held at the limit"
    );
    assert!(samples
        .iter()
        .all(|sample| sample.command <= 1.0 && sample.command >= -1.0));
}

#[test]
fn a_zero_delay_plant_takes_the_command_of_its_own_period() {
    // Integrator with gain 1, period 0.1, P = 1, setpoint 1: the first command is 1, so the output after
    // the first period is 0.1 at once; the second command is 1 − 0.1 and the output becomes 0.19.
    let integrator = plant(SampledDynamics::Integrating, 1.0, 0.0);
    let mut law = Proportional { gain: 1.0 };
    let samples = simulate(&integrator, &mut law, &run(0.1, 3, 1.0)).expect("the run is valid");
    assert!(
        (samples[0].process_value - 0.1).abs() <= 1e-12,
        "{}",
        samples[0].process_value
    );
    assert!(
        (samples[1].process_value - 0.19).abs() <= 1e-12,
        "{}",
        samples[1].process_value
    );
}

#[test]
fn a_one_period_delay_holds_the_first_command_back_one_period() {
    // Delay 0.1 s at period 0.1 s is one whole period: the first command reaches the plant in the second period.
    let delayed_integrator = plant(SampledDynamics::Integrating, 1.0, 0.1);
    let mut law = Proportional { gain: 1.0 };
    let samples =
        simulate(&delayed_integrator, &mut law, &run(0.1, 3, 1.0)).expect("the run is valid");
    assert_eq!(samples[0].process_value, 0.0);
    assert!(
        (samples[1].process_value - 0.1).abs() <= 1e-12,
        "{}",
        samples[1].process_value
    );
    assert!(
        (samples[2].process_value - 0.2).abs() <= 1e-12,
        "{}",
        samples[2].process_value
    );
}

#[test]
fn a_constant_load_enters_the_plant_before_the_delay() {
    // Delay 0.2 s at period 0.1 s is two periods. With no command and a load of 0.5, the load reaches the
    // integrator in the third period, so the output is zero, zero, then 0.1 · 0.5.
    let delayed_integrator = plant(SampledDynamics::Integrating, 1.0, 0.2);
    let mut law = Constant(0.0);
    let mut settings = run(0.1, 3, 0.0);
    settings.load = 0.5;
    let samples = simulate(&delayed_integrator, &mut law, &settings).expect("the run is valid");
    assert_eq!(samples[0].process_value, 0.0);
    assert_eq!(samples[1].process_value, 0.0);
    assert!(
        (samples[2].process_value - 0.05).abs() <= 1e-12,
        "{}",
        samples[2].process_value
    );
}

#[test]
fn the_measurement_filter_follows_its_first_order_update() {
    // Filter time 0.1 s at period 0.1 s gives α = 0.5. The measurement of a unit step seen through the
    // filter is 0, then 0.5, then 0.75, because the static plant follows the command at once.
    let static_gain = plant(SampledDynamics::Static, 1.0, 0.0);
    let mut law = Recorder {
        command: 1.0,
        measurements: Vec::new(),
    };
    let mut settings = run(0.1, 3, 0.0);
    settings.measurement_filter_seconds = 0.1;
    simulate(&static_gain, &mut law, &settings).expect("the run is valid");
    assert!(law.measurements[0].abs() <= 1e-12);
    assert!(
        (law.measurements[1] - 0.5).abs() <= 1e-12,
        "{}",
        law.measurements[1]
    );
    assert!(
        (law.measurements[2] - 0.75).abs() <= 1e-12,
        "{}",
        law.measurements[2]
    );
}

#[test]
fn an_integrating_lag_is_exact_for_a_held_input() {
    // K/(s(s + 1)) with a unit command held: the output is t − 1 + e^(−t), exact at every period.
    let lagged = plant(
        SampledDynamics::IntegratingWithLag {
            time_constant_seconds: 1.0,
        },
        1.0,
        0.0,
    );
    let mut law = Constant(1.0);
    let samples = simulate(&lagged, &mut law, &run(0.1, 20, 0.0)).expect("the run is valid");
    let at_two_seconds = 1.0 + (-2.0_f64).exp();
    assert!(
        (samples[19].process_value - at_two_seconds).abs() <= 1e-12,
        "{}",
        samples[19].process_value
    );
}

#[test]
fn a_second_order_plant_settles_at_its_gain_under_a_held_command() {
    let second_order = plant(
        SampledDynamics::SecondOrder {
            first_time_constant_seconds: 1.0,
            second_time_constant_seconds: 0.5,
        },
        2.0,
        0.0,
    );
    let mut law = Constant(1.0);
    let samples =
        simulate(&second_order, &mut law, &run(0.01, 5000, 0.0)).expect("the run is valid");
    assert!(
        (samples[4999].process_value - 2.0).abs() <= 1e-6,
        "{}",
        samples[4999].process_value
    );
}

#[test]
fn a_law_that_returns_a_nonfinite_command_stops_the_run() {
    let static_gain = plant(SampledDynamics::Static, 1.0, 0.0);
    let mut law = Constant(f64::NAN);
    let result = simulate(&static_gain, &mut law, &run(0.1, 3, 0.0));
    assert!(matches!(result, Err(ClosedLoopError::NonFinite)));
}

#[test]
fn runs_that_cannot_be_simulated_are_refused() {
    let static_gain = plant(SampledDynamics::Static, 1.0, 0.0);
    let mut law = Constant(0.0);
    assert!(matches!(
        simulate(&static_gain, &mut law, &run(0.0, 3, 0.0)),
        Err(ClosedLoopError::Run(RunRefusal::Period))
    ));
    assert!(matches!(
        simulate(&static_gain, &mut law, &run(0.1, 0, 0.0)),
        Err(ClosedLoopError::Run(RunRefusal::NoPeriods))
    ));
    let negative_delay = plant(SampledDynamics::Static, 1.0, -0.1);
    assert!(matches!(
        simulate(&negative_delay, &mut law, &run(0.1, 3, 0.0)),
        Err(ClosedLoopError::Run(RunRefusal::Delay))
    ));
    let no_lag = plant(
        SampledDynamics::FirstOrder {
            time_constant_seconds: 0.0,
        },
        1.0,
        0.0,
    );
    assert!(matches!(
        simulate(&no_lag, &mut law, &run(0.1, 3, 0.0)),
        Err(ClosedLoopError::Run(RunRefusal::TimeConstant))
    ));
}

/// A constant law, for the held-command cases.
struct Constant(f64);

impl DiscreteLaw for Constant {
    type Refusal = Infallible;

    fn command(
        &mut self,
        _setpoint: f64,
        _measurement: f64,
        _period: f64,
    ) -> Result<f64, Infallible> {
        Ok(self.0)
    }
}
