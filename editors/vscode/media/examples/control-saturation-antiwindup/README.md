# Saturation and anti-windup

Compare BackCalculation and Combined state recovery after saturation. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Compare BackCalculation and Combined state recovery after saturation.

## Predict

Predict which internal state must be corrected when the setpoint returns to zero.

## Configure

Use paper Section 6.4 process, Kp 2.83, Ki 2.83/3.25, Kd 2.83·0.23, limits ±3.5, tracking 3.25 s. The authoritative equations and limits are in Specification 36 Section 7.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance paper_pid_recovers_from_saturation_and_respects_rate_limits -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Output remains within limits; leaves saturation; PV returns within 0.05 by 50 s. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

Anti-windup changes state handling at a limit, not the unconstrained PID equation.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Change tracking time and explain the recovery/noise tradeoff.
