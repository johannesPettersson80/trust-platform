# 14. Identification experiments

You will choose a heater-command step and test duration that allow you to estimate process gain, delay and time constant while respecting temperature and power limits.

## The problem

Lesson 13 derived the heater balance:

$$
C\frac{dT}{dt}=2u(t-1)-10(T-20)-D.
$$

For this teaching case, C=50 kJ/°C, full heater power is 200 kW, each command percentage point delivers 2 kW, heat loss is H=10 kW/°C toward 20 °C, the delivered heat has a distinct 1 s delay, and the nominal time constant is τ=C/H=5 s. These are supplied engineering quantities for planning. They are not measurements from the experiment you are about to run.

A step that is too small can be hidden by measurement resolution. A step that is too large can move temperature outside the exercise range. A hold that is too short can end before the delayed response and approach are visible. You must balance those risks before changing the manual command.

## What we need

We need a measured response that reveals the heater's dynamics clearly
enough to estimate a model. We must choose an input change and observation
duration that separate the response from noise and existing drift while
keeping the experiment within its limits.

## The new idea

An **identification experiment** deliberately changes a process input to obtain
measurements from which we can estimate a model. Here we use a **step test**:
change applied heater power once, hold it, and observe how far and how quickly
temperature moves. PI stays inactive so it does not keep changing that input.

We first need a steady starting condition. Otherwise an existing temperature
drift could be mistaken for the effect of our step.

Qualify the baseline before choosing the step. Use a five-second pre-step window, from −5 s through 0 s where zero is the command-step time. Require every valid PV sample to remain within 35±.05 °C, the maximum minus minimum range to be at most .05 °C, and the finite-window slope to be at most .01 °C/s in magnitude. Define that slope as:

$$
\text{window slope}=\frac{T_{last}-T_{first}}{t_{last}-t_{first}}.
$$

The range checks short-term spread; the slope checks drift. Both are needed. A record from 35.00 °C to 35.06 °C over 5 s has slope .012 °C/s and fails even though its change is small. If the reading is valid and within 33–39 °C but the baseline still drifts, keep the manual command at 75% and wait or reset; do not attribute later motion to the step.

Choose a positive command step of 5–15 percentage points, staying within 0–100%. The model predicts the final temperature from T_eq=20+.2u when D=0. A 5-point step from 75% predicts 36 °C, a 10-point step predicts 37 °C, and a 15-point step predicts 38 °C. The worked 10-point choice therefore has a predicted steady change of 2 °C. Immediately after its one-second delivery delay, the initial model rate at 35 °C is approximately 2(10)/50=.4 °C/s. The command change corresponds to 20 kW of extra heat. With an assumed measurement-error bound of ±0.05 °C, a 2 °C final change is easy to distinguish; a 5-point change gives only 1 °C and may be safer but less informative.

Choose a hold of 20–40 s. The worked choice is 30 s: after the 1 s delay, 29 s remain, corresponding to 29/5=5.8 time constants. The nominal model completes more than 99% of its temperature change in that interval. A shorter 20 s hold reaches the delay plus nearly four time constants and uses less exposure. A longer 40 s hold reveals more of the final value but keeps the changed command applied longer. Your selected duration and amplitude must be justified by this risk/information tradeoff.

## Prediction

Before opening the experiment controls, write down your chosen amplitude and duration and answer:

1. What applied command will result from your chosen 5–15 point increase?
2. What final PV does T_eq=20+.2u predict for that command?
3. What is the predicted initial rate after the 1 s delay for a 10-point step?
4. Does your predicted change exceed the assumed ±0.05 °C measurement uncertainty by enough to identify a direction?
5. Why does your hold cover the 1 s delay and several 5 s time constants?
6. What observation would make you reject the record as uninformative even if no hard limit was reached?

The predictions are planning calculations. They do not replace the measured PV or establish that the supplied C, H, delay, or τ are true for other equipment.

## Implementation

Open `src/Main.st` and identify the experiment's command, duration, measurement-validity and limit inputs.

Write only the concise calculation and checks needed by the existing interface. The planning values `SelectedStepPoints`, `ProposedCommandPct`, and `PredictedFinalC` are `REAL` in `Main.st`; the observed `TemperatureC` PV is `LREAL`, and experiment timestamps come from the Control Studio record. Calculate:

```st
ProposedCommandPct := 75.0 + SelectedStepPoints;
PredictedFinalC := 20.0 + 0.2 * ProposedCommandPct;
```

Use IF and AND, which were introduced in the early lessons, to express the checks:

- proposed command is between 0 and 100%;
- PV is between 33 and 39 °C;
- baseline range is at most .05 °C;
- baseline slope magnitude is at most .01 °C/s; and
- the measurement is valid.

Keep the proposed command and measured PV distinct from the actual applied input. The provided interface records the applied command and PV every .1 s; inspect that record to verify which input the heater received. This project owns the open-loop step; it has no PI state or writable disturbance control.

The provided baseline collector retains 51 samples, spanning 5 s at the configured
100 ms period, and publishes range, slope and `BaselineQualified`. It does not
assume zero range from nominal model values. Review the actual recorded
timestamps as well. An early confirmation is rejected without moving above 75%;
disable the request, qualify the baseline, then confirm again.

`SelectedStepPoints` and `HoldSeconds` are bounded `REAL` parameters. Restart,
inspect and apply their reviewed values while the experiment is disabled,
then wait for baseline qualification. The prepared 30 s hold returns to 75% on
normal completion. `SafetyAbort` latches bad quality or PV outside 33–39 °C and
commands zero. Quality code 1 means good in this simulated interface; other
codes invalidate the measurement. Valid PV alone does not clear the latch.

## Experiment in Control Studio

Keep the common hot initial condition:

- PV = 35 °C;
- applied manual command = 75%;
- preceding 1 s command history = 75%;
- measurement quality code = 1;
- experiment authority disabled.

The command is bounded by 0–100%. For this exercise, PV has a hard acceptable range of 33–39 °C and the measurement is invalid whenever the interface marks it invalid or stops supplying a usable timestamped value. A normal experiment must begin only after a qualified baseline. A hard safety violation or invalid measurement stops the experiment; it does not automatically restore a command.

1. Stop and restart Simulator. Open **Control Studio → Identify** and choose **Project experiment** under **Method**. In **Signals**, select `P1.TemperatureC`, `P1.AppliedPowerPct`, `P1.MeasurementQuality`, `P1.SafetyAbort` and `P1.ExperimentEnabled`. Confirm 35 °C, 75%, quality 1, abort 0 and authority 0 in the live plot. The native plant includes the preceding 1 s input history at 75%.
2. Expand **Declared experiment settings** and press **Inspect experiment parameters**. Set **Selected Step Points** to your chosen 5–15 points and **Hold Seconds** to your chosen 20–40 s. Press **Review parameter values**, check the values, then **Apply reviewed values**. The worked choice is 10 points/30 s; 12 points/25 s is another permitted choice. Close the settings disclosure after checking the confirmed values.
3. In **Signals**, select `P1.BaselineQualified`, `P1.BaselineRangeC` and `P1.BaselineSlopeCPerS`. Wait for Qualified=1, Range≤0.05 °C and slope magnitude≤0.01 °C/s. These values come from the program's actual 51-sample window. If the baseline drifts, keep 75% and wait or restart; do not start from that drift. Select temperature and applied power again for the experiment.
4. Press **Review project test**. The prepared 45.1 s maximum contains the 5 s baseline, any permitted hold up to 40 s and restoration. Press **Confirm and start**. The recorder first keeps the experiment disabled at 75%; after 5 s, the project checks qualification again and applies your step for **Hold Seconds**. A shorter hold returns to 75% before recording ends; the extra samples document that return.
5. Treat the observed applied-input step as response time zero. Compare its actual amplitude and duration with your choice. Proposal and Prediction are planning values, not measured temperature. The five seconds immediately preceding the step must meet the baseline range and slope conditions.
6. Press **Abort test** if PV leaves 33–39 °C, applied input leaves 0–100%, quality becomes invalid, or timestamps stop advancing. A measurement fault latches Safety abort and commands 0%; valid feedback does not clear it. An ordinary operator abort disables the experiment and restores 75% while measurements remain valid. Stop Simulator if it is unresponsive.
7. After normal completion, inspect the retained temperature and applied-input traces, including the return to 75%. Right-click the response plot and choose **Save recording…** to keep the complete JSON record. For the next lesson, repeat with **12 points and a 40 s hold** and save that record; the longer hold improves the final-value estimate, and its step amplitude must differ from the next project’s 10-point validation step. The same action is available as **Control Studio: Save recording…** in the command palette. Closing and reopening Control Studio retains the plotted record and your signal selection. Restart Simulator and reapply your reviewed parameter values before repeating; a fault requires this fresh start and baseline qualification.

## Evidence and decision

Accept the record as informative only when all of these are true:

- the common hot initial state was confirmed;
- the five-second baseline passed both range and slope rules;
- the applied, rather than merely requested, command changed by the selected amount;
- command and PV samples are timestamped at the .1 s experiment interval;
- no hard PV, command, validity, or timestamp fault occurred; and
- the hold contains data after the 1 s delay and long enough to judge the approach.

Then compare the measured direction, delay, rise, and final trend with the prediction. Do not insert the nominal 36, 37, or 38 °C final value into the evidence if the record differs. A record can be safe but uninformative when the PV remains within baseline noise, the command step is not applied, or the hold ends before the delayed response can be distinguished. Record that decision and redesign amplitude or duration rather than silently extending the record after seeing it.

## Conclusion

A useful step test changes one known input from a qualified steady condition.
The model helps us choose a bounded amplitude and duration; the actual
applied-command and temperature record provides the data for identification.

## Reason for the next lesson

We now have an informative applied-command and temperature record, but it
does not label the model parameters for us.
[Step-response identification](../23-step-response-identification/README.md)
uses that record to estimate gain, delay and time constant with uncertainty.
