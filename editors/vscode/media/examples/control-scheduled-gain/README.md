# Scheduled plant gain change

Observe one fixed controller across a declared gain transition. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Observe one fixed controller across a declared gain transition.

## Predict

Predict the immediate PV/MV response when gain rises from 0.6 to 1.4.

## Configure

Use FOPDT tau 1 s, delay 0.5 s; schedule native set_parameter gain=1.4 at 15 s; no retuning. The authoritative equations and limits are in Specification 36 Section 12.2.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Stop the simulator, open Control Studio, then restart and begin recording
immediately so simulation time zero is unambiguous. Apply a unit setpoint step
and run for 45 s; the configured gain change occurs at 15 s. Keep live SP, PV,
MV, limits, and Abort visible; abort if values leave the declared range. Native
expected-result command: `cargo test -p trust-runtime --test control_pid_performance pid_advanced_benchmark_recovers_from_a_scheduled_gain_change -- --exact`.

## Identify

Skip a single-model fit across the transition because the plant gain changes.
Use the exact configured pre-event gain `0.6` and post-event gain `1.4` as two
operating regions; fitting one stationary model would be misleading.

## Tune

Skip retuning. The lesson deliberately keeps the paper PI gains fixed across
the scheduled change.

## Validate

Control Studio's general requirement list is supporting evidence, not the
scheduled-event verdict. The native command uses the exact event time and
post-event window to require finite response; |PV| ≤ 2; final error ≤ 2%;
return within 15 s after the change; and post-event IAE ≤ 2. Save the observed
run with its scheduled-event plant context for comparison and export.

## Explain

This tests one transition and motivates, but does not implement, gain scheduling.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Create two operating regions and specify what evidence a future scheduler would need.
