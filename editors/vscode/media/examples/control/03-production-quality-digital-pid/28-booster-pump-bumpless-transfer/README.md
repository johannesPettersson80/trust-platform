# Bumpless authority transfer on a booster pump

Course position: Lesson 28 of 84. Previous lesson:
[Booster-pump initialization and modes](../27-booster-pump-modes/README.md). Next
lesson: Atomic parameter updates and signal ordering.

**Lesson objective:** Transfer valve authority between an external station,
Auto, and Manual without an avoidable command step and identify boundaries
where exact bumpless transfer is infeasible.

This lesson uses simulation. Its results are not physical-plant evidence.

**Equipment:** Variable-speed booster pump with external valve authority.

## The problem

The equipment in this lesson is Variable-speed booster pump with external valve authority.

The booster-pump pressure controller was initialized correctly in Lesson 27.
An external station can nevertheless take valve authority and move the valve
while the pressure controller is inactive. The controller must remain aligned
if it may take authority again.

Correct startup state becomes stale when another authority changes the physical
output. We now compare continuous tracking with an inactive controller that
receives no applied-output information.

## What we need

We need an inactive controller to follow the valve position applied by another authority so its internal output is current when control returns. The transfer must be judged from the actual command step, including cases where exact matching is physically impossible.

## The new idea

The **authority owner** is the station currently permitted to command the
recirculation valve. The **external applied valve output
($MV_{\mathrm{external\,applied}}$)** is the externally commanded and physically accepted
valve position in percent.

The **tracking input ($MV_{\mathrm{track}}$)** is the independently observed applied output
supplied to the inactive pressure controller, in percent. The **retained nominal
output ($u_{\mathrm{nominal}}$)** is the output held as controller state before final limits.
The **last applied output ($MV_{\mathrm{last}}$)** is the physical valve value immediately
before ownership changes. The **transition instant ($t_{\mathrm{transfer}}$)** is the exact
controller update at which the new authority begins.

**Bumpless transfer** means that the first requested output from the new
authority equals the last physically applied output within a declared
tolerance. It does not mean pressure is already at SP or that later feedback
increments must be zero.

With integral action, `Track` writes $MV_{\mathrm{track}}$ into the retained nominal output
before executing the incremental controller update. If SP and PV do not change
at that update, Auto can continue from the aligned value. An **inactive without
tracking** controller receives no update and retains stale state.

For Auto-to-Manual transfer, the external manual station must adopt the current
applied output before taking authority. `Control.PID` cannot make an unrelated
manual command bumpless after the supervisor has selected it.

There are boundaries. In P or PD positional control, tracking has no aligning
effect in the paper-compatible controller. Exact transfer is also infeasible
when applied-output feedback is stale, the desired value is outside limits, a
rate constraint is binding, or SP or PV changes without being included in the
prediction.

## Prediction

Start with controller and valve at 40 percent, SP at 6 bar, and PV at 6 bar.
External authority moves the valve to 60 percent while SP and PV are held for
the exact transition update.

With Track, $u_{\mathrm{nominal}}$ becomes 60 percent, so the first Auto request predicts
60 percent and a zero-point step. Without Track, $u_{\mathrm{nominal}}$ remains 40 percent,
so Auto predicts a 20 percentage-point step from $MV_{\mathrm{last}} = 60\%$.

For Auto-to-Manual at 40 percent, a manual station still set to 10 percent
predicts a 30-point step. If it first adopts 40 percent, the predicted step is
zero.

## Implementation

The same single product controller receives the external applied value in
Track, then changes to Auto:

```st
Input.Mode := PIDMode#Track;
Input.TrackingOutput := TrackingOutput;
ValveCommand := Controller.Update(Input := Input);
```

The simulation-only untracked baseline applies `ExternalOutput` while
deliberately not calling the controller. It is not another control law.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Restore, call Track with the independently observed applied output throughout the same external move, then change only authority to Auto. Repeat Auto-to-Manual with unrelated and pre-aligned manual values. The declared event lasts 10 seconds. It restores External applied valve output to 40 % afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Discharge-pressure setpoint (SP, bar); Discharge pressure (PV, bar); Requested valve command (MV, %), including units, limits, mode, and quality. The initial operating point is Discharge pressure = 6 bar, Requested valve command = 40 %, External applied valve output = 40 %. The unresolved baseline problem is: The inactive controller remains at 40 percent while external authority moves the valve to 60 percent, creating a 20 percentage-point step when Auto takes authority.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Feed independently observed applied output to Track throughout external authority, then change only authority to Auto.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **10**. The observed lesson signal is **External applied valve output**. Control Studio observes the project's existing timed sequence and does not write a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Tracked and untracked external 40-to-60 percent changes. Confirm the live values are current and the reviewed plan stays inside Discharge pressure 0 bar to 12 bar; Requested valve command 0 % to 100 %; External applied valve output 0 % to 100 %.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: tracked_external_to_auto_output_step <= 0.001 % (tolerance 1e-9 %); untracked_external_to_auto_output_step >= 20 % (tolerance 1e-9 %); unaligned_auto_to_manual_output_step >= 30 % (tolerance 1e-9 %); aligned_auto_to_manual_output_step <= 0.001 % (tolerance 1e-9 %).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

The **authority-transition output step** is the absolute difference between the
first requested output from the new authority and the last applied output
before the transition, measured in percentage points.

The **retained-state alignment error** is $u_{\mathrm{nominal}}$ minus
$MV_{\mathrm{external\,applied}}$ immediately before transfer, measured in percentage
points. A zero PV error does not prove this state is aligned.

| Transition | Last applied | First new request | Output step |
| --- | ---: | ---: | ---: |
| Tracked external → Auto | 60.000% | 60.000% | 0.000 pp |
| Untracked external → Auto | 60.000% | 40.000% | 20.000 pp |
| Unaligned Auto → Manual | 40.000% | 10.000% | 30.000 pp |
| Aligned Auto → Manual | 40.000% | 40.000% | 0.000 pp |
| Stale 40% tracking while actual output is 60% → Auto | 60.000% | 40.000% | 20.000 pp |

![Tracking aligns authority transfer](figures/bumpless-transfer.svg)

[Retained run](figures/bumpless-transfer-run.csv) ·
[Plot data](figures/bumpless-transfer-trace.csv) ·
[Decision summary](figures/bumpless-transfer-summary.csv)

### Validation

Require the tracked and aligned steps to be at most 0.001 percentage points.
The diagnostic untracked and unaligned cases must expose at least 20 and
30 percentage points respectively. Reject the comparison if tracking quality
is invalid or an amplitude or rate limit masks the unrestricted request.

After the transition measurement, release the held PV and assess pressure
recovery separately; subsequent feedback action is not a transfer bump. In the
retained simulation, post-transition recovery reaches 6.266 bar after
10 seconds.

## Conclusion

The controller that may own the valve next needs the applied value from the
station that owns it now. Track provides that state alignment for PI and PID.
Likewise, the manual station owns its command and must align it before taking
authority.

Bumpless transfer is therefore a contract between controller and supervisor,
not a property the PID can guarantee in isolation. Stale tracking makes an exact
transfer infeasible. If tracking says 40 percent while independently measured
actuator output is 60 percent, the controller is missing 20 percentage points
of the physical state it must match.

## Reason for the next lesson

Apply the same tracking contract before selectors, cascade handover, parallel
gain schedules, and redundant controllers. Record infeasible limits rather than
promising a zero step. Lesson 29 tests atomic configuration and signal ordering
inside each authoritative update.
