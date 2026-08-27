# Severe deterministic scan jitter

Verify PID.Update uses each actual cycle interval. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Verify PID.Update uses each actual cycle interval.

## Predict

Predict the error caused by substituting the 100 ms mean for every scan.

## Configure

Use the paper loop with repeated 20/180/50/150/100 ms PLC intervals and fixed 50 ms plant steps. The authoritative equations and limits are in Specification 36 Section 12.3.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_advanced_benchmark_uses_actual_cycle_time_under_severe_jitter -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Finite; |PV| ≤ 10; overshoot ≤ 20%; settling ≤ 8 s; final error ≤ 2%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Actual CycleTime is part of every saved sample and controller update.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Change the sequence while preserving the mean and compare responses.
