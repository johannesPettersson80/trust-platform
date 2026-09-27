# 82. Independent design

You will independently design a control solution for familiar equipment. You will write and compare two ST implementations, assess uncertainty and recovery, and recommend a design using requirements set before the experiments.

## The problem

A control recommendation must connect physical behavior, requirements, units, actuator limits, measurement validity and recovery. Copying a controller from another lesson can hide incompatible assumptions. The assessment is independent because the learner chooses the plant and the comparison, while the evidence rules remain explicit.

## What we need

We need to demonstrate an independently justified control solution for
a familiar application. That means choosing the necessary taught methods,
comparing meaningful alternatives under the same requirements, and stating
what the evidence does and does not support.

## The new idea

**Independent control design** means choosing and integrating suitable methods
for a physical problem, then defending the choice with calculations and
evidence. The task is not to include every technique from the course; explain
which requirement needs each method.

Choose one taught path: the electric heater, rigid shaft, two-vessel process, a cold-room controller from Lessons 1–3, or another plant with a completed lesson record. The chosen plant and units must come from that lesson; other capstones are not prerequisites. Choose two taught candidate controllers or integrations that are meaningful for the plant, such as P versus PI, fixed versus scheduled PI, paired versus decoupled control, or two controller arrangements with the required lifecycle behavior.

Before the experiment, write a decision rule but leave the outcome open. Define nominal setpoint/disturbance cases, one physical uncertainty parameter and range supported by the chosen lesson's record, actuator and state limits, sample interval, validity rule, stop/reset sequence, and metrics. A fault or mode case is required when Lessons 25–28 are prerequisites for the selected controller; cold-room candidates may instead use their taught threshold and dwell behavior.

Identify the controlled variable, measurement, actuator, stored state, model
parameters, command limits and fallback. Work at least one physical feasibility
calculation from the chosen plant, such as heater holding power or shaft load
torque, and explain every operation used in the ST integration.

The uncertainty must come from the chosen lesson's physical or identification record: for example heater heat loss or inlet disturbance, the taught shaft load, tank operating level or an identified outlet-coefficient range, coupled-vessel conductance, or a documented sensor bound. Do not invent a new physical mechanism. Test the nominal case and the stated alternative range with the same decision rule.

## Prediction

Before implementation, predict both candidates' first commands, physical response direction, likely limiting condition, and the evidence that would reject each. Predict how the chosen uncertainty changes the plant response or feasibility. Predict which metric will decide the recommendation, while allowing the run to disprove that prediction.

## Implementation

Write a concise integration using the chosen lesson's learned ST structures: named units, explicit mode/ownership where taught, retained state, validity, limits, requested/applied outputs and reset. Use one coherent candidate interface for both comparisons. Do not add a new technique merely to make the assessment advanced. If the plant lacks a valid measurement, use its declared fallback rather than inventing a PV.

## Experiment in Control Studio

1. Reset the selected plant and verify all physical, controller and model states separately. Confirm the common clock, initial measurement, applied output and validity.
2. Run the nominal setpoint and disturbance cases for candidate A, then fully reset and run the same cases for candidate B. Use durations justified by the plant time scale and record every event time from **Confirm and start**.
3. Repeat the same cases for the documented uncertainty value or range. If lifecycle behavior is in scope, run the declared mode/invalid-data case separately and restore before the next candidate.
4. Record setpoint, PV, error, requested/applied actuator, mode, validity, retained state, model residual and the selected performance metrics. Stop at the chosen physical/software bounds or stopped clock.
5. Restore the documented safe/manual state and fully reset between every comparison. Keep learner choices and actual outcomes separate.

## Evidence and decision

Build a requirement-by-requirement table after the runs: nominal performance, uncertainty performance, actuator limits, state/mode behavior, validity/fallback and model residual. A simulator result supports only the configured model and uncertainty range; it is not hardware proof. Include the worked calculation, candidate settings, reset states, event timings and any saturation.

Apply the predeclared decision rule without changing thresholds after seeing results. If both candidates qualify, choose using the stated effort/complexity or robustness metric. If neither qualifies, recommend redesign or bounded manual operation. A “not qualified” result is a legitimate engineering conclusion.

## Conclusion

An independent assessment is a traceable recommendation: one taught plant, two defensible candidates, one documented physical uncertainty, explicit lifecycle behavior where required, and evidence tied to a decision rule that was fixed before the outcomes were known.

## Reason for the next lesson

This assessment completes the independent path. The recommendation, its supporting comparisons and its stated limits are the final course outcome.
