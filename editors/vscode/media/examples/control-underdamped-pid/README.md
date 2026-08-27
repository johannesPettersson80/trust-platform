# Underdamped second-order PID

Use derivative action to damp an oscillatory process. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Use derivative action to damp an oscillatory process.

## Predict

Predict the PI response before enabling derivative action.

## Configure

Use wn 1 rad/s, damping 0.2, Kp 1.5, Ki 0.5, Kd 1, 50 ms, 30 s. The authoritative equations and limits are in Specification 36 Section 11.2.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_benchmark_damps_an_underdamped_second_order_process -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Stable; overshoot ≤ 20%; settling ≤ 15 s; final error ≤ 2%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Derivative action can improve damping while increasing measurement-noise sensitivity.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Add identical noise to PI and PID runs and compare MV increment RMS.
