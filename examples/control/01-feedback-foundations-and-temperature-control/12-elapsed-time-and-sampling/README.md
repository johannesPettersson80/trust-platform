# 24. Sampling and timing

You will calculate integral and filter updates from the elapsed interval, then decide which heater and sensor changes a sampled controller can miss between readings.

## The problem

The water has thermal inertia, so its temperature changes gradually. An electrical sensor can vary much faster because of pickup or quantization. Suppose a repeatable noise cycle has period 0.1 s and the controller samples every 0.1 s at the same phase: the raw value may be 35.0 °C at every sample even though it reaches 35.2 °C between samples. Equal samples do not prove that the analogue input was constant.

This is **aliasing**: distinct continuous-time signals produce the same sampled sequence. A software filter applied after acquisition cannot reconstruct variations that were never sampled. Faster acquisition or filtering before sampling can address that information loss. Timing arithmetic is a separate issue: when a controller update is late, the state update must use the actual elapsed interval.

## What we need

We need to use the actual elapsed interval for state updates and distinguish
when a measurement was acquired, when control was calculated and when output
was applied. We also need to recognize changes the sampling cannot observe.

## The new idea

**Sampling** measures a signal at separate instants rather than continuously.
**Controller timing** determines when calculations and output updates occur
and how much elapsed time their state changes represent. The output is
usually held between updates; a fast calculation does not refresh an old sample.

For a constant rate $r$, change over an interval is

$$
\mathrm{change}=r\,\Delta t.
$$

For the isolated arithmetic example, use $SP=36$ °C, $PV=35$ °C, $K_i=0.5$ percentage points/(°C·s), and no output limit. With a 0.1 s interval,

$$
\Delta I=K_i(SP-PV)\Delta t=0.5(1)(0.1)=0.05\ \text{points}.
$$

With a 0.2 s interval and the same fixed error calculation,

$$
\Delta I=0.5(1)(0.2)=0.10\ \text{points}.
$$

This is a calculation of a fixed input, not a claimed live heater trace. A filter update uses the same accepted elapsed interval:

$$
\alpha=\frac{\Delta t}{T_{\mathrm{filter}}+\Delta t},\qquad
T_{f,\mathrm{new}}=T_{f,\mathrm{old}}+\alpha(T_{r,\mathrm{new}}-T_{f,\mathrm{old}}).
$$

The acquisition timestamp says when a sensor value was obtained. The controller-update timestamp says when code accepts it and updates state. The applied-command timestamp records when the limited command is written to the actuator. Delivered heat is a separate physical quantity, delayed here by 1 s. A request can be held at the controller while an older command is still moving through the one-second heater delay; these are different events.

For this exercise, a fresh sample and its controller update are synchronous. The numerical value may be unchanged and still be fresh. Elapsed time is measured from the previous accepted controller update, not repeatedly from an unchanged acquisition timestamp. At startup, initialize the time baseline once. On a valid interval, calculate it once and advance the baseline once; never double-count or leave an old accepted timestamp as if that were recovery.

The exercise uses a maximum accepted gap of 0.25 s. A gap beyond that stops this demonstration and requires a reset; full invalid-data recovery belongs to Lesson 28.

## Prediction

Predict the isolated integral increments $0.05$ and $0.10$ points above. Predict that a fixed-0.1 s implementation undercounts the 0.2 s interval. Predict that a controller request can remain unchanged while delivered heater power changes later because of the one-second delay.

For the sampling illustration, a 0.1 s noise cycle can return to 35.0 °C at each sample while reaching 35.2 °C between samples. The sampled record cannot distinguish that signal from a constant 35.0 °C signal. A short 30 kW heat-removal pulse lasting 0.05 s changes stored energy by about $30(0.05)=1.5$ kJ and temperature by approximately $1.5/50=0.03$ °C. It may be below measurement resolution, but it does not exactly restore temperature between samples. State what faster acquisition or an event signal would be needed to establish it.

## Implementation

In `src/Main.st`, run this fragment while the experiment is active. Initialize `AutomaticEnabled` to TRUE only when starting from the complete reset below. Convert the timestamp difference to seconds and guard one update:

```st
ElapsedS := CurrentUpdateTimeS - PreviousUpdateTimeS;
IF AutomaticEnabled AND MeasurementValid
    AND (ElapsedS > 0.0) AND (ElapsedS <= 0.25) THEN
    Alpha := ElapsedS / (FilterTimeS + ElapsedS);
    FilteredTemperatureC := FilteredTemperatureC
        + Alpha * (RawTemperatureC - FilteredTemperatureC);
    ErrorC := SetpointC - FilteredTemperatureC;
    IntegralIncrementPct := KiPctPerCs * ErrorC * ElapsedS;
    PreviousUpdateTimeS := CurrentUpdateTimeS;
ELSE
    AutomaticEnabled := FALSE;
    RequestedPowerPct := 0.0;
    AppliedPowerPct := 0.0;
END_IF;
```

Use the new filtered measurement in both the weighted proportional calculation and the integral error. Add `IntegralIncrementPct` to the trial integral, then form the request, limit it and apply the existing anti-windup correction. These operations run only in the valid-update branch. A zero filter time gives Alpha=1 and therefore bypasses filtering; reject a negative filter time when selecting the candidate.

The timestamp inputs are elapsed seconds on the same continuously advancing clock. Initialize `PreviousUpdateTimeS` from the current time before the first control interval. For a backward, invalid or overlong interval, this exercise stops automatic control and commands zero heat. A later valid interval does not re-enable the Boolean. Explicitly restart the experiment, reinitialize the time baseline and the other controller states, and then set `AutomaticEnabled` TRUE before restarting control, so the next update does not integrate over the stopped interval.

## Experiment in Control Studio

1. Start Simulator and open Control Studio → Monitor → Response. Display SP, PV, raw/filtered temperature, acquisition time, controller-update time, elapsed interval, integral increment, requested power, applied power and delayed applied power.
2. Reset to SP=35 °C, PV=35 °C, applied power 75%, integral output 75%, proportional output 0%, filter state 35 °C and delay history 75%. Use the selected Lesson 21 gains, Lesson 22 weight and Lesson 23 filter choice. The nominal update is 0.1 s.
3. In **Control Studio → Test**, select **Confirm and start** as time zero for the sampled-signal observation. The project task and simulator step are both 100 ms, and `src/Main.st` deliberately uses a 0.2 s elapsed interval at experiment cycle 10, which is 1.0 s after enablement. The same scan passes that interval to the existing `Control.PID`; the plant still holds the applied output between updates and retains its one-second delivery delay. Keep the project’s 2.1 s duration and reset between runs.
4. At the scripted 1.0 s event, use the published current error and the 0.2 s value to calculate $K_i e(0.2)$, using the stated constant-latest-error approximation for this comparison. Do not assume the error is 1 °C in the live trace. Distinguish the held request during the scripted interval from any separate delayed plant behavior; this project is a sampled-signal arithmetic demonstration, not a live missed-task injection.
5. Separately record the sampling illustration: a repeatable 0.1 s noise cycle sampled at the same phase, and the 0.05 s, 30 kW pulse as an input calculation rather than a claim that the plant exactly returns between readings. Record which analogue variation is unobservable.
6. Abort if temperature leaves 0–80 °C, applied command leaves 0–100%, data is invalid, timestamps move backward, or elapsed time exceeds 0.25 s. On stop, cancel the target event, disable automatic control and command zero heat; confirm zero applied command and fully reset before repeating. Stop the simulator if it is unresponsive.

## Evidence and decision

For the isolated calculation, verify 0.05 points at 0.1 s and 0.10 points at 0.2 s. The runnable project also executes the existing `Control.PID` over the same hot heater I/O: its 0.1 s task uses the scripted 0.2 s interval at the tenth update, while the plant retains the one-second delivery delay. For the live test, use the logged error at the scripted 1.0 s event and the $0.2$ s interval; a nominal-error substitution is not evidence. Check that the controller receives the same accepted elapsed interval used by the demonstration.

Keep the three time axes separate. A fresh unchanged sample is valid, the scripted 0.2 s interval is a controller timing event, and a delayed applied command is plant/actuator behavior. If the overlong-gap guard stops the run, report the stop and reset rather than treating the old state as recovered. The fixed target requirements assess the controller response; the noise-cycle record assesses sampling observability.

## Conclusion

Elapsed time belongs in every rate-based state update, while acquisition and actuator timing determine which measurements are available and when heating reaches the water. Aliasing can hide real analogue variation; a later software filter cannot reconstruct it. Correct timestamps and explicit gap handling prevent a late update from being undercounted or integrated twice.

## Reason for the next lesson

Timing correctness still leaves controller lifecycle questions: how should a stateful heater start when it is already hot and power is already applied?

[Controller modes and initialization](../../03-production-quality-digital-pid/39-controller-mode-initialization/README.md) introduces that boundary.
