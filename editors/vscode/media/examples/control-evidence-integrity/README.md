# Process-image evidence integrity

See why typed process-image ranges matter to trustworthy evidence. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

See why typed process-image ranges matter to trustworthy evidence.

## Predict

Predict which bytes two LREAL values at %IL0 and %IL1 share.

## Configure

Use the safe runnable mapping %IL0 and %IL8; reproduce the rejected %IL0/%IL1 mapping only in validation. The authoritative equations and limits are in Specification 36 Section 13.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

Run the bounded experiment in Control Studio. Abort if values leave the declared range. Native expected-result command: `cargo test -p trust-runtime --lib simulation_process_image_ranges_detect_lreal_overlap`.

## Identify

Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.

## Tune

Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.

## Validate

The overlap is rejected; the corrected mapping runs with independent values. Save the result so its samples, model, controller, metrics, and conclusion can be reopened.

## Explain

A plausible trace is not evidence if signals alias the same bytes.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Repeat with REAL and WORD values and calculate each safe next address.
