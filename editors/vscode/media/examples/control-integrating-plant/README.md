# Integrating plant structure choice

Understand why P control can remove step offset on an integrating plant. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Understand why P control can remove step offset on an integrating plant.

## Predict

Predict the effect of adding unnecessary controller integral action.

## Configure

Use P(s)=1/s, Kp 0.8, Ki=Kd=0, 100 ms, 30 s. The authoritative equations and limits are in Specification 36 Section 11.1.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_benchmark_tracks_an_integrating_process -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Stable; overshoot ≤ 10%; settling ≤ 12 s; final error ≤ 1%. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

The plant already integrates; structure selection must consider plant dynamics.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Try a small Ki, record the deterioration or benefit, and justify whether PI is warranted.
