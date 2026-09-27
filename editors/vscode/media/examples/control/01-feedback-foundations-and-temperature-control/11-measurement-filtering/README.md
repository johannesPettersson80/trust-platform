# 23. Measurement filtering

You will use a measurement filter to reduce heater-command movement caused by sensor noise. You will measure the extra response delay it introduces and decide whether the noise reduction justifies that delay.

## The problem

Lesson 22 preserves the full proportional response to a measured temperature change. That is useful when the water really becomes colder, but an electrical sensor can alternate above and below the true temperature while the water is steady. The PI controller then changes its requested heater power for a signal change that does not represent stored thermal energy. Reducing that activity is useful only if the controller still detects a real temperature change quickly enough.

## What we need

We need to reduce controller-output movement caused by measurement noise
while still detecting real temperature changes quickly enough. The choice
must account for both noise reduction and delayed feedback.

## The new idea

A **measurement filter** reduces rapid fluctuations in a measured signal.
This first-order **low-pass filter** lets slower changes through more readily
than faster ones by moving its stored value gradually toward the new reading.
It also delays recognition of a real temperature change, so smoothing has a cost.

Let $T_r(t)$ be the raw measured temperature and $T_f(t)$ the filtered temperature. The filter is a first-order lag with filter time $T_{\mathrm{filter}}>0$:

$$
T_{\mathrm{filter}}\frac{dT_f}{dt}+T_f=T_r.
$$

This lag says the filtered estimate moves toward the raw measurement at a rate set by $T_{\mathrm{filter}}$. It is a signal-processing approximation, not a heat-balance law. It assumes rapid measurement variation is less useful than slower thermal change.

For an update interval $\Delta t>0$, approximate the derivative by a backward difference and use the current filtered value on the lag term:

$$
T_{\mathrm{filter}}\frac{T_{f,\mathrm{new}}-T_{f,\mathrm{old}}}{\Delta t}+T_{f,\mathrm{new}}=T_{r,\mathrm{new}}.
$$

Collecting the new filtered value gives

$$
T_{f,\mathrm{new}}
=T_{f,\mathrm{old}}+
\frac{\Delta t}{T_{\mathrm{filter}}+\Delta t}
(T_{r,\mathrm{new}}-T_{f,\mathrm{old}}).
$$

The dimensionless weight is $\alpha=\Delta t/(T_{\mathrm{filter}}+\Delta t)$. At $T_{\mathrm{filter}}=1$ s and $\Delta t=0.1$ s, $\alpha=1/11$. If raw temperature changes from 35 to 36 °C for one sample, filtered temperature becomes $35+1/11=35.090909$ °C.

For a persistent one-degree step, the residual after $n$ samples is $(10/11)^n$. The first sample count at which 90% of the step has appeared is

$$
n=\left\lceil\frac{\ln(0.1)}{\ln(10/11)}\right\rceil=25,
$$

The ceiling brackets mean round up to the next whole update: 25 updates, or 2.5 s at the 0.1 s interval. This is a useful delay calculation, not a claim about a full noise spectrum.

Keep the selected Lesson 21 PI gains, the Lesson 22 proportional setpoint weight, integral state, 0–100% limits, anti-windup and requested/applied-power distinction. The reset condition is true physical temperature 35 °C, manual applied power 75%, integral output 75%, and each filter state initialized from a valid 35 °C measurement. Compare filter-time candidates $T_{\mathrm{filter}}=0$ s (bypass), $0.2$ s and $1.0$ s.

Current project boundary: `src/Main.st` fixes `FilterTime` at 1.0 s and
`CycleTime` at 0.1 s. It exposes one filtered signal and no noise waveform or
filter-time candidate control. The bypass, 0.2 s and alternating-noise cases
below require those missing Studio inputs; ST helper calculations do not create
their recorded evidence.
The simulation configuration starts the water and sensor models at 20 °C with
zero delayed input, while the teaching reset below assumes 35 °C and 75%.
Record that initial-state mismatch before using any trace as the specified hot
operating point.

Requirements are fixed before the experiment. A noise candidate must reduce measured request activity in the declared noise window while also meeting the same target and disturbance requirements used for the selected controller: for a 35→38 °C step, peak ≤39 °C and $|error|\le0.2$ °C from 30–60 s; and, in a separate test, for a 30 kW added load at 35 °C, minimum ≥33 °C and $|error|\le0.2$ °C from 30–60 s. Use the simulator's actual water temperature for peak, minimum and settling; the controller's filtered reading cannot independently establish these requirements. The 90% equilibrium output is a nominal prediction, not another acceptance condition. A candidate that is quieter but fails those requirements does not qualify.

## Prediction

Use $K_p=2$ percentage points/°C and $K_i=0.5$ percentage points/(°C·s) only for the following arithmetic illustration; do not replace the selected Lesson 21 PI gains in the experiment. Starting with integral output 75% and setpoint 35 °C, a raw sample of 36 °C gives an isolated proportional contribution of $-2$ points. The filtered sample gives $-0.181818$ points, so proportional suppression is $1.818182$ points.

The integral increments also differ if the controller integrates filtered error: raw error $-1$ °C gives $-0.05$ points in 0.1 s, while filtered error $-0.090909$ °C gives $-0.004545$ points. The corresponding illustrative full requests from an integral state of 75% are 72.95% and 74.813636%. The calculation starts each controller state at I=75% and holds SP fixed, so the setpoint weight does not affect this example.

Predict that a persistent physical change eventually passes through, while the $T_{\mathrm{filter}}=1$ s candidate reaches 90% after 2.5 s. Predict that a smoother request is not automatically better if it violates the fixed peak, minimum or settling requirements.

## Implementation

In `src/Main.st`, initialize the filtered temperature from the first valid raw measurement. On each valid update, use the configured elapsed time and filter time:

```st
Alpha := ElapsedS / (FilterTimeS + ElapsedS);
FilteredTemperatureC := FilteredTemperatureC
    + Alpha * (RawTemperatureC - FilteredTemperatureC);
```

Calculate proportional action from the weighted setpoint path and filtered measurement, and update the integral state from $SP-T_f$. Retain the selected PI gains, Lesson 8 limiting/tracking behavior, and the requested/applied-power separation. A bypass candidate with $T_{\mathrm{filter}}=0$ uses raw PV in a separate branch; reject negative values, but do not reject the permitted zero bypass. Do not use the filter to hide invalid data.

## Experiment in Control Studio

1. Start Simulator and open Control Studio → Monitor → Response. Display true simulated temperature, raw PV, filtered PV, selected filter time, requested and applied power, integral state, saturation/tracking status, and the 0.1 s elapsed interval.
2. Confirm true temperature 35 °C, manual applied power 75%, integral output 75%, setpoint 35 °C, valid raw PV 35 °C, and the project filter state initialized to 35 °C. The current project supplies no Control Studio noise-source or filter-time selector; record those missing controls instead of claiming the alternating-noise candidate comparison.
3. A shadow comparison of bypass, 0.2 s and 1.0 s filters would require a Studio analysis workspace and candidate controls that the current project does not expose. Record that missing capability; an ST-derived request table is not a substitute for this named Studio operation. If the workspace becomes available, hold the actual heater manually at 75%, calculate complete PI requests with separate shadow integral states, and record request range plus sum of absolute sample-to-sample changes. This isolates measurement activity; it is not closed-loop performance evidence.
4. The requested closed-loop target and disturbance comparison requires candidate and noise controls that the current project does not expose. Do not substitute its one fixed 1.0 s scripted run or an ST-derived request table for those Studio trials. If the controls become available, use the same reset, event clock, noise and requirements described here.
5. For a persistent physical change, measure the time from the true simulated temperature's 0.2 °C drop to the raw and filtered signals crossing the same 0.2 °C threshold. Mark the raw and filtered crossing as the first of three consecutive samples at or below 34.8 °C. Compare these times with the true-temperature crossing, and record “not reached” if a signal never meets the rule. A noisy early crossing is not evidence that the water cooled before the physical event.
6. Abort if true temperature leaves 0–80 °C, applied power leaves 0–100%, data becomes invalid/stale, or signals stop updating. At stop disable automatic control, command zero heat, wait for restoration, and reset the whole simulator before every repeat.

## Evidence and decision

Check the isolated 1 °C calculation against the $1.818182$-point proportional reduction and requests 72.95% and 74.813636%. The 10 s shadow noise experiment uses ±0.2 °C, so compare its own recorded request activity rather than expecting those one-degree values. Report request range and total movement separately. Do not call shadow activity a heater-response result because applied power was held manually.

For each closed-loop candidate, assess noise activity, setpoint peak/error, disturbance minimum/error, and true-temperature detection delay. Use the same event clock, noise, initial filter state and controller gains. A filter qualifies only if it reduces the declared noise activity and meets the fixed response requirements. Report the observed detection delay as a cost and use the actual-temperature requirements to decide whether the filtered controller still responds adequately.

## Conclusion

Filtering can reduce noise-driven command movement, but it also delays real
changes. Choose it from both observations: quieter output and acceptable
temperature response with the same controller and requirements.

## Reason for the next lesson

The filter and integral update use elapsed time, while the heater and sensor continue changing between controller updates. Lesson 24 examines missed samples, held commands and what a sampled controller cannot observe.

[Elapsed time and sampling](../12-elapsed-time-and-sampling/README.md) continues that timing analysis.
