# Plant-side load disturbance

Observe integral action reject a plant-side output bias. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Observe integral action reject a plant-side output bias.

## Predict

Predict the sign and duration of the MV correction after the bias event.

## Configure

Use the nominal paper loop and schedule output bias 0.2 at 10 s in the plant event system. The authoritative equations and limits are in Specification 36 Section 6.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance paper_pi_rejects_a_plant_side_load_disturbance -- --exact`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

Stable; within 2% by 20 s; post-event IAE ≤ 0.35. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

The disturbance belongs to the plant; modifying the PID measurement would falsify the experiment.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Compare rejection with Ki = 0 and explain the resulting offset.
