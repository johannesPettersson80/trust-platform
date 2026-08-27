# Delay-dominant process

Recognize the limit of aggressive PI tuning when delay dominates. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Recognize the limit of aggressive PI tuning when delay dominates.

## Predict

Predict why larger gains cannot remove a two-second information delay.

## Configure

Use exp(-2s)/(1+s), Kp=Ki=1/6, 100 ms, 60 s. The authoritative equations and limits are in Specification 36 Section 11.3.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_benchmark_tracks_a_delay_dominant_process -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Stable; overshoot ≤ 20%; settling ≤ 25 s; final error ≤ 2%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Robust tuning must respect delay; predictor control is a separate future component.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Increase PI gains until validation rejects the candidate and identify the limiting metric.
