# Open-loop unstable plant

Distinguish eventual convergence from an acceptable unstable-plant transient. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Distinguish eventual convergence from an acceptable unstable-plant transient.

## Predict

Predict the uncontrolled trajectory and the required feedback sign.

## Configure

Use A=0.2, B=C=1, D=0 with PI Kp 3, Ki 1 at 50 ms for 30 s. The authoritative equations and limits are in Specification 36 Section 12.1.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_advanced_benchmark_stabilizes_an_open_loop_unstable_plant -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Finite; |PV| ≤ 10; overshoot ≤ 20%; settling ≤ 15 s; final error ≤ 2%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Stabilization is necessary but peak, settling, and actuator constraints still decide acceptance.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Compare three PI tunings and reject any that merely converge with an unacceptable transient.
