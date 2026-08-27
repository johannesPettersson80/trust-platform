# Sampling interval sensitivity

Compare the same plant and tuning at 50, 100, and 200 ms. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Compare the same plant and tuning at 50, 100, and 200 ms.

## Predict

Predict which run departs furthest from the continuous-time expectation.

## Configure

Run the paper FOPDT and PI with each actual task and plant interval; pass that same interval to PID.Update. The authoritative equations and limits are in Specification 36 Section 5.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance paper_pi_meets_nominal_robustness_and_sampling_gates -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Every run remains stable, overshoot ≤ 20%, settling ≤ 8 s, final error ≤ 2%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

The actual interval is controller input data; changing simulator timing alone is not a valid comparison.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Add a 300 ms case, declare your expected failure mode, then validate it without changing gains.
