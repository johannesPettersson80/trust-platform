# Nonlinear actuator

Separate saturation, slew, lag, and deadband effects. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Separate saturation, slew, lag, and deadband effects.

## Predict

Predict each imperfection alone before enabling the declared combination.

## Configure

Use first-order plant with actuator range 0…1.2, slew 1/s, deadband 0.02, lag 0.2 s; PI Kp 1, Ki 0.5. The authoritative equations and limits are in Specification 36 Section 11.4.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_benchmark_handles_nonlinear_actuator_dynamics -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Finite; MV in range; overshoot ≤ 10%; settling ≤ 15 s; final error ≤ 3%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Actuator behavior is part of the plant boundary and must remain visible in evidence.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Run four saved revisions enabling one imperfection at a time, then compare the combined result.
