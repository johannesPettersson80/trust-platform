# Actuator rate limit

See how actuator protection bounds achievable loop speed. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

See how actuator protection bounds achievable loop speed.

## Predict

Sketch the fastest possible MV ramp before running the loop.

## Configure

Use nominal process and tuning with output rate limits of ±1 unit/s. The authoritative equations and limits are in Specification 36 Section 7.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance paper_pid_recovers_from_saturation_and_respects_rate_limits -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Every MV increment respects 1 unit/s within 1e-10; final error ≤ 2% by 30 s. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

A controller cannot demand a response faster than the actuator can deliver.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Compare ±0.5 and ±2 unit/s while keeping the same tuning.
