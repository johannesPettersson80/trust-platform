# Initialize a booster-pump controller before Auto

Course position: Lesson 27 of 84. Previous lesson:
[PID realization forms](../26-pid-forms/README.md). Next lesson: Bumpless transfer
and external tracking.

**Lesson objective:** Initialize controller and filter histories from the
running booster pump, distinguish Disabled, Manual, Auto, and Track authority,
and block Auto until startup state is qualified.

This lesson uses simulation. Its results are not physical-plant evidence.

**Equipment:** Variable-speed booster pump with recirculation valve.

## The problem

The equipment in this lesson is Variable-speed booster pump with recirculation valve.

A variable-speed booster pump is already running when its pressure controller
starts. A recirculation valve is 40 percent open and discharge pressure is
6 bar. Starting software does not make those physical values zero.

The station's declared Disabled action commands 30 percent recirculation. A
physical startup additionally requires reviewed piping, independent valve
feedback, pump protection, and explicit approval.

Lesson 26 showed that a digital controller retains output and previous signals.
If those histories or the pressure filter start at arbitrary zero values, the
first controller request can disagree sharply with the already-running valve.

## What we need

We need every retained controller and filter history to match the already-running pump before automatic authority is allowed. Disabled, Manual, Auto, and Track must each have one explicit owner of the applied valve command.

## The new idea

The **discharge-pressure setpoint ($SP$)** is the desired pump discharge pressure
in bar. The **raw discharge pressure ($PV_{\mathrm{raw}}$)** is the value received directly
from the pressure sensor in bar. The **filtered discharge pressure
($PV_{\mathrm{filtered}}$)** is the pressure supplied to the controller after the low-pass
filter, also in bar.

The **actual valve position ($MV_{\mathrm{actual}}$)** is independent position feedback from
the recirculation valve, measured in percent. The **requested valve command
($MV_{\mathrm{requested}}$)** is the controller result in percent. The **applied valve
command ($MV_{\mathrm{applied}}$)** is the value accepted by the simulated actuator after
its limits. These three values shall not be assumed equal.

The **nominal controller output ($u_{\mathrm{nominal}}$)** is the controller's retained
output before final amplitude and rate limits. The **previous setpoint
($SP_{\mathrm{previous}}$)** and **previous process value ($PV_{\mathrm{previous}}$)** are retained signal
histories used by the incremental controller.

**Initialization** copies known present conditions into these histories. It
performs no control calculation, integration, differentiation, limiting, or new
actuator command.

`Control.PID` implements three modes. **Manual** uses the supplied manual
output. **Auto** performs the feedback calculation. **Track** first aligns the
retained nominal output with an external tracking value and then executes the
normal nonmanual branch.

**Disabled** is station-level supervision and is not a `PIDMode`. Disabled does
not call `Control.PID`; the station applies its declared safe command. This
keeps plant safety policy outside the paper-compatible three-value enum.

The **filter-settled indication** becomes true only after filtered and raw
pressure differ by no more than 0.01 bar for three consecutive 100 ms updates.
Until initialization is complete and this indication is true, a request for
Auto remains under Manual authority at the actual valve position.

The simulator's local equilibrium says that discharge pressure rises by
0.1 bar for every percentage point of valve position, above a 2 bar base:

$PV = 2 bar + 0.1 bar/\% \times MV$

This is a bounded first-order teaching model, not a pump curve.

## Prediction

At 20 percent valve position, predicted equilibrium pressure is 4 bar. At
40 percent it is 6 bar, and at 70 percent it is 9 bar. Set SP equal to the
corresponding PV in each startup case.

With measured-state initialization, error and signal differences are zero and
retained output equals $MV_{\mathrm{actual}}$. The first permitted Auto request should
therefore be 20, 40, or 70 percent respectively.

With zero histories at the 40 percent case, the two-stage filter's first output
is only 0.198 bar. The rate-limited controller starts at 10 percent rather than
the physical 40 percent. Predict a material startup mismatch.

## Implementation

The conforming startup initializes the real product filter and controller:

```st
PressureFilter.Initialize(Input := RawPressure);
InitialState.Setpoint := Setpoint;
InitialState.ProcessValue := RawPressure;
InitialState.NominalOutput := ActualValvePosition;
InitialState.SaturatedOutput := ActualValvePosition;
Controller.Initialize(State := InitialState);
```

The mode supervisor then invokes this one `Control.PID` instance or performs
the external Disabled action. There is no parallel controller path.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Restore, initialize from current pressure, setpoint, and actual valve feedback, wait for three settled-filter updates under Manual authority, then permit Auto. Repeat at the 20, 40, and 70 percent equilibria. The declared event lasts 10 seconds. It restores Actual valve position to 30 % afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Discharge-pressure setpoint (SP, bar); Raw discharge pressure (PV, bar); Requested valve command (MV, %), including units, limits, mode, and quality. The initial operating point is Raw discharge pressure = 6 bar, Requested valve command = 40 %, Actual valve position = 40 %, Applied valve command = 40 %. The unresolved baseline problem is: Arbitrary zero controller and filter histories produce a first request of 10 percent and filtered pressure of 0.198 bar on a pump already operating at 40 percent and 6 bar.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Copy current pressure, setpoint, and actual valve position into filter and controller histories atomically, retain Manual authority until the filter is qualified, then permit Auto.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **10**. The observed lesson signal is **Actual valve position**. Control Studio observes the project's existing timed sequence and does not write a second process event. Retain the zero-state startup comparison: it produces a 10.000 percent command step and a 0.198 bar pressure deviation before measured-state initialization is enabled.
   - **Decision before action:** Before selecting **Review test**, predict the response to Repeat at 20, 40, and 70 percent equilibrium valve positions. Confirm the live values are current and the reviewed plan stays inside Raw discharge pressure 0 bar to 12 bar; Filtered discharge pressure 0 bar to 12 bar; Requested valve command 0 % to 100 %; Applied valve command 0 % to 100 %.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: measured_initialization_first_update_valve_mismatch <= 1e-9 % (tolerance 1e-9 %); measured_initialization_filter_error <= 1e-9 bar (tolerance 1e-9 bar); zero_state_first_update_valve_mismatch >= 10 % (tolerance 0.001 %); equilibrium_pressure_drift <= 0.05 bar (tolerance 0.005 bar).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

The **first-update valve mismatch** is the absolute value of $MV_{\mathrm{requested}}$
minus $MV_{\mathrm{actual}}$ at the first permitted Auto update, measured in percentage
points. The **filter initialization error** is the absolute value of
$PV_{\mathrm{filtered}}$ minus $PV_{\mathrm{raw}}$ immediately after initialization, measured in bar.

| Initialization | Actual valve | First request | Filtered pressure | Result |
| --- | ---: | ---: | ---: | --- |
| Zero-state baseline | 40.0% | 10.000% | 0.198 bar | reject |
| Measured state | 20.0% | 20.000% | 4.000 bar | accept |
| Measured state | 40.0% | 40.000% | 6.000 bar | accept |
| Measured state | 70.0% | 70.000% | 9.000 bar | accept |

![Measured-state initialization prevents a startup bump](figures/booster-pump-startup.svg)

[Retained run](figures/booster-pump-startup-run.csv) ·
[Plot data](figures/booster-pump-startup-trace.csv) ·
[Decision summary](figures/booster-pump-startup-summary.csv)

If a valve limit or rate limit changes the request, report that state and do
not call the bounded result an initialization calculation.

### Validation

Require measured-state first-update mismatch and filter initialization error to
be at most `1e-9`. The deliberate zero-state mismatch must be at least
10 percentage points. After measured initialization at 40 percent, run the
real simulator for 10 seconds and require final pressure within 0.05 bar of
6 bar and valve request within 0.5 percentage points of 40 percent. Retain the
zero-state rejection, all three measured-state starts, Disabled, Manual, Track,
and the complete 10-second startup history so the result can be reproduced.

## Conclusion

The controller cannot infer the running valve position from its equations.
Likewise, a filter initialized to zero invents a pressure transient that the
plant never produced. Copying actual SP, PV, valve feedback, and filter input
creates histories that describe the physical starting condition.

Modes identify who owns the command. Initialization determines whether that
owner starts from truthful state. Both are observable controller behavior.

## Reason for the next lesson

Use independent actuator feedback and present measurements whenever a
controller joins an already-running process. Keep Auto blocked until state and
signal quality are qualified. Lesson 28 tests bumpless Manual, Track, and Auto
transitions after this safe startup foundation exists.
