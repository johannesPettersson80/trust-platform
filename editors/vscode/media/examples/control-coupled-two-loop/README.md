# Coupled two-loop process

Observe interaction between two independent PI loops. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Observe interaction between two independent PI loops.

## Predict

Predict the cross-response from a step in only loop 1.

## Configure

Use the declared 2×2 state-space matrices and two PI controllers with Kp 0.8, Ki 0.4 at 50 ms. The authoritative equations and limits are in Specification 36 Section 12.4.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_mimo_performance pid_advanced_benchmark_controls_a_coupled_two_loop_plant -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Finite; |PV| and |MV| ≤ 5; each final error ≤ 2%; final 5 s stays in band. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Independent loops can work for declared weak coupling; stronger coupling needs separate decoupling evidence.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Increase off-diagonal B terms and define the point where decoupling becomes necessary.
