# Bounded plant variation

Validate one fixed controller over the four declared FOPDT cases. Every result in this project is simulation evidence; it is never physical-plant evidence.

SP means setpoint, PV means process value, MV means manipulated variable (controller output), and FOPDT means first-order plus dead time.

## Objective

Validate one fixed controller over the four declared FOPDT cases.

## Predict

Predict which gain/time/delay combination is most demanding.

## Configure

Run the exact four gain, time-constant, and delay combinations without
retuning:

| Case | Gain | Time constant | Dead time |
| --- | ---: | ---: | ---: |
| Low gain, fast, short delay | 0.8 | 0.8 s | 0.4 s |
| High gain, slow, long delay | 1.2 | 1.2 s | 0.6 s |
| High gain, fast, long delay | 1.2 | 0.8 s | 0.6 s |
| Low gain, slow, short delay | 0.8 | 1.2 s | 0.4 s |

For each run, edit only `gain`, `time_constant_s`, and `dead_time_s` in
`simulation.toml`, restart the simulator, and save a separate Control Studio
result. Do not retune between cases. The authoritative equations and limits
are in Specification 36 Section 4.

## Monitor

Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.

## Test

For each configuration, start recording at zero, run a unit setpoint step for
30 s, save the result, and restart before the next case. Abort if values leave
the declared range. Native expected-result command: `cargo test -p trust-runtime --test control_pid_performance paper_pi_meets_nominal_robustness_and_sampling_gates -- --exact`.

## Identify

Do not fit one model across all four runs. Each configured FOPDT model is exact
simulation evidence; use Identify only to compare one run with its configured
model, and keep the four saved results separate.

## Tune

Skip retuning. This lesson deliberately holds one controller fixed so the four
responses measure robustness to the declared plant variation.

## Validate

Control Studio's general requirement list is supporting evidence for each
saved run, not the four-case verdict. The native command above evaluates all
four exact 30 s cases together: each run is stable; overshoot ≤ 20%; settling
≤ 8 s; final error ≤ 2%.

## Explain

Passing proves only the declared uncertainty box, not universal robustness.

## Export

Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.

## Questions

1. Which observed signal first showed the limiting behavior?
2. Which acceptance metric decided the result?
3. What does this experiment not prove?

## Advanced exercise

Expand one bound by 10%, rerun, and state whether the original claim still holds.
