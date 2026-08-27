# Nominal paper FOPDT

Reproduce the paper PI response and identify SP, PV, and MV. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Reproduce the paper PI response and identify SP, PV, and MV.

## Predict

Predict a stable, low-overshoot response with zero steady-state offset.

## Configure

Use gain 1, time constant 1 s, dead time 0.5 s, Kp = Ki = 0.667, and 100 ms. The authoritative equations and limits are in Specification 36 Section 3.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Start recording at the zero operating point, then run a unit setpoint step for
20 s. Abort if values leave the declared range. Native expected-result command:
`cargo test -p trust-runtime --test control_pid_performance paper_pi_meets_nominal_robustness_and_sampling_gates -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Use Control Studio to review the recorded response, limits, model prediction,
and stability margins. Its general requirement list is supporting engineering
evidence, not the exact lesson verdict. The native command above evaluates the
declared 20 s unit-step contract: overshoot ≤ 20%; settling ≤ 4 s; final error
ratio ≤ 0.0001; IAE ≤ 1.4. Save the result so its samples, model, controller,
metrics, and conclusion can be reopened.

## Explain

Lambda is the desired closed-loop time constant: increasing it asks for a
slower, more robust response, while decreasing it asks for a faster response
that usually requires more actuator movement. Lambda PI exposes this choice
without changing the PID equation.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Change Lambda through the tuning workflow and explain the MV cost of a faster response.
