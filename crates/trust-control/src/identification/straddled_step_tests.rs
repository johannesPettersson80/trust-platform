// The step of a recording whose samples average several executions (numerical annex N2.2; specification 35, A4.2): where
// the step falls inside a sample must not decide the model (G95, the CODESYS toolkit's S13). The plant is first order
// with gain 1.5 and a 4 s time constant, run at 10 ms executions, ten averaged into each 0.1 s sample. The Euler
// recursion's steady state is exactly 1.5 times the input, and the 30 s hold is 7.5 time constants, which leaves less
// than 0.06 % of the response unsettled: a fit that places the step right recovers the gain to better than 0.5 %.

use trust_control_math::identify::SampleQuality;

use super::*;

/// The execution period, in seconds.
const EXECUTION: f64 = 0.01;
/// The executions one sample averages.
const EXECUTIONS_PER_SAMPLE: u32 = 10;
/// The executions of a 40 s recording.
const EXECUTIONS: u32 = 4_000;
/// The plant's gain.
const GAIN: f64 = 1.5;
/// The plant's time constant, in seconds.
const TAU: f64 = 4.0;
/// The largest relative error of a fitted gain on these noise-free recordings.
const GAIN_TOLERANCE: f64 = 0.005;

/// A recording of the plant, the input stepped from 0 to 10 at `step_at` and back to 0 at `restore_at` (seconds). Open
/// loop, the recorded input is the step itself; in the closed loop a PI controller (gain 2, reset time 4 s) follows a
/// setpoint stepped so, and the recorded input is the controller's output, as Identify records a setpoint step.
fn averaged_recording(
    step_at: f64,
    restore_at: f64,
    closed_loop: bool,
) -> Vec<IdentificationSample> {
    let (mut pv, mut integral) = (0.0_f64, 0.0_f64);
    let mut sums = (0.0, 0.0, 0.0);
    let mut samples = Vec::new();
    for execution in 0..EXECUTIONS {
        let time = f64::from(execution) * EXECUTION;
        let target = if time >= step_at && time < restore_at {
            10.0
        } else {
            0.0
        };
        let input = if closed_loop {
            let error = target - pv;
            integral += error * EXECUTION / TAU;
            (2.0 * (error + integral)).clamp(-100.0, 100.0)
        } else {
            target
        };
        pv += EXECUTION * (GAIN * input - pv) / TAU;
        sums = (sums.0 + time, sums.1 + input, sums.2 + pv);
        if (execution + 1) % EXECUTIONS_PER_SAMPLE == 0 {
            let count = f64::from(EXECUTIONS_PER_SAMPLE);
            samples.push(IdentificationSample {
                time_seconds: sums.0 / count,
                input: sums.1 / count,
                output: sums.2 / count,
                quality: SampleQuality::Good,
                input_unit: None,
                output_unit: None,
                clipped: None,
                saturated: None,
            });
            sums = (0.0, 0.0, 0.0);
        }
    }
    samples
}

/// The time of an edge at `fraction` of the sample that starts at `sample_start` seconds.
fn edge(sample_start: f64, fraction: f64) -> f64 {
    sample_start + fraction * f64::from(EXECUTIONS_PER_SAMPLE) * EXECUTION
}

/// The single-run fit of a simulated recording.
fn fit(
    samples: &[IdentificationSample],
    closed_loop: bool,
) -> Result<IdentificationResult, Refusal> {
    let options = IdentifyPlantOptions {
        declared_process: None,
        closed_loop,
        pulse: false,
    };
    identify_plant(
        samples,
        &[],
        DataSource::Simulation,
        None,
        DEFAULT_MAXIMUM_NORMALIZED_ERROR,
        &options,
    )
}

fn assert_gain(result: &IdentificationResult, case: &str) {
    let gain = result.recommended.gain;
    assert!(
        ((gain - GAIN) / GAIN).abs() <= GAIN_TOLERANCE,
        "{case}: gain {gain}, the plant's is {GAIN}"
    );
}

/// The step's edge at 0, 50, 55, 75 and 95 % of a sample, the restoration at 50 % of the sample at 32 s, open loop and
/// closed loop. Every fit recovers the gain 1.5 within 0.5 %, and every fraction gives the same kind of model. Wrong
/// behaviour caught: the step placed at the larger of its two partial changes, so that past the middle of a sample the
/// straddling sample joins the samples before the step and biases the operating point; today's placement fitted
/// 1.527 open loop and 1.587 closed loop at 55 %.
#[test]
fn where_a_step_falls_inside_a_sample_does_not_change_the_model() {
    for closed_loop in [false, true] {
        let kinds: Vec<LowOrderModelKind> = [0.0, 0.5, 0.55, 0.75, 0.95]
            .into_iter()
            .map(|fraction| {
                let samples = averaged_recording(edge(2.0, fraction), edge(32.0, 0.5), closed_loop);
                let result = fit(&samples, closed_loop).expect("the recording is identified");
                assert_gain(
                    &result,
                    &format!("closed loop {closed_loop}, edge at {fraction} of a sample"),
                );
                result.recommended.kind
            })
            .collect();
        assert!(
            kinds.iter().all(|kind| *kind == kinds[0]),
            "closed loop {closed_loop}: one kind of model at every fraction, got {kinds:?}"
        );
    }
}

/// The toolkit's case: the restoration at 35 % of the sample at 39 s, ten samples before the recording ends, splits
/// into 6.5 and 3.5. A step at 50 % (5 and 5) or 55 % (4.5 and 5.5) splits into smaller parts than 6.5; whole, both the
/// step and the restoration are 10, and the step comes first. Every case is identified with the gain 1.5. Wrong
/// behaviour caught: the restoration's partial 6.5 taken as the step, with nine samples after it, which refused the
/// recording with "A bounded input step with pre- and post-step data is required." at 50 % and 55 % while 75 % and
/// 95 % fitted, as the toolkit reported.
#[test]
fn a_step_is_found_although_its_restoration_straddles_less_evenly() {
    for fraction in [0.5, 0.55, 0.75, 0.95] {
        let samples = averaged_recording(edge(2.0, fraction), edge(39.0, 0.35), false);
        let result = fit(&samples, false).unwrap_or_else(|refusal| {
            panic!("edge at {fraction} of a sample: refused with {refusal:?}")
        });
        assert_gain(&result, &format!("edge at {fraction} of a sample"));
    }
}
