# Measurement noise and filtering

Compare raw PV with a separate second-order LowPass using one seed. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Compare raw PV with a separate second-order LowPass using one seed.

## Predict

Predict how filtering changes MV activity and apparent response delay.

## Configure

Use two identical paper loops and deterministic measurement noise; filter only one PV with tau 0.1 s. The authoritative equations and limits are in Specification 36 Section 8.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance separate_low_pass_reduces_seeded_measurement_noise_activity -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Both finite and within 5%; filtered MV increment RMS is lower. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Measurement filtering stays a separate visible block; PID.Update has no hidden filter.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Increase filter time constant and find where quieter MV no longer justifies added delay.
