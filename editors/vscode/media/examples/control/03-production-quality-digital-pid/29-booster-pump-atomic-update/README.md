# Atomic PID updates and signal ordering on a booster pump

Course position: Lesson 29 of 84. Previous lesson: [Bumpless authority
transfer](../28-booster-pump-bumpless-transfer/README.md). Next lesson: Invalid
data and safe behavior.

**Lesson objective:** Apply a complete PID configuration as one revision and
distinguish the valid internal feedforward path from an invalid post-limit sum.

This lesson uses simulation. Its results are not physical-plant evidence.

**Equipment:** Variable-speed booster pump with recirculation valve.

## The problem

The equipment in this lesson is Variable-speed booster pump with recirculation valve.

An online booster pump maintains discharge pressure by moving a recirculation
valve. An engineer may prepare new tuning while the old tuning is running. A
measured disturbance may also supply an additional valve contribution.

The valve accepts 0–100 percent and can move at only a declared rate. Pressure
is measured in bar. The controller must never expose a partly updated tuning or
let any contribution bypass the valve constraints.

Lesson 28 aligned controller state when authority changed. That good state can
still be lost in one scan if the controller sees some old parameters and some
new parameters. It can also be bypassed if feedforward is added after the
controller has already applied anti-windup and limits.

## What we need

We need each scan to use one complete, coherent parameter revision and one constraint-aware path for every output contribution. A half-applied configuration or a feedforward sum added after limits must be impossible.

## The new idea

The **candidate configuration** is the complete proposed set of PID parameters.
The **active configuration** is the complete record the controller uses for the
current scan. A **configuration revision** identifies one whole active record.
A **coherent snapshot** contains either the complete old configuration or the
complete new configuration, never a mixture. A **scan boundary** separates one
controller update from the next.

The **feedback contribution ($\Delta MV_{\mathrm{feedback}}$)** is the output change caused
by setpoint and pressure differences, measured in percentage points. The
**feedforward contribution ($\Delta MV_{\mathrm{feedforward}}$)** is an output change derived
from a measured disturbance, also in percentage points. Lesson 53 teaches how
to design it; here its known value exposes ordering.

The **nominal requested output ($MV_{\mathrm{requested}}$)** is the controller result before
constraints. It is private controller state. The lesson therefore publishes an
**unconstrained target diagnostic**: an independent calculation of the raw
PI/feedforward target before anti-windup and limits. It is evidence only and
cannot command the plant. The **applied output ($MV_{\mathrm{applied}}$)** is the valve
command after anti-windup, amplitude limits, and rate limits. The **previous
applied output ($MV_{\mathrm{previous\,applied}}$)** anchors the next allowed rate interval.

An atomic update performs one record assignment and one `Configure` call before
the next update. The order inside the authoritative path is:

1. Read one coherent parameter snapshot.
2. Calculate feedback and feedforward contributions.
3. Apply anti-windup and output constraints.
4. Retain and publish the applied output.

Feedforward enters `PIDInput.FeedForward` before anti-windup and limits. Adding
it after `Control.PID` returns would bypass both the constraints and the state
used by the next scan.

## Prediction

The previous applied valve output is 40 percent. The old proportional gain is
5 percent per bar and the old integral gain is 1 percent per bar-second. The
candidate values are 15 and 3. The sample interval is 0.1 seconds.

For a 1 bar setpoint increase with unchanged pressure, the proportional output
increment is 15 percentage points. The integral output increment is the
integral gain multiplied by sample interval and error:

$\Delta MV_{\mathrm{integral}} = 3 \%/(bar \cdot s) \cdot 0.1 s \cdot 1 bar = 0.3 percentage points$

The coherent new revision therefore predicts **15.3 percentage points** and a
55.3 percent output. A mixed revision with new proportional gain and old
integral gain predicts **15.1 percentage points** and 55.1 percent.

At zero error, a 20 percent feedforward contribution is requested. A 20
percent-per-second rate limit over 0.1 seconds permits a 2-point move from 40,
so $MV_{\mathrm{applied}}$ predicts **42.000 percent**. Adding feedforward after the
returned 40 percent would instead produce an invalid **60.000 percent** sum.

## Implementation

The production path uses one `Control.PID`. Candidate fields become one active
record before the controller is configured:

```st
ActiveParameters := CandidateParameters;
Controller.Configure(Parameters := ActiveParameters);

Input.FeedForward := FeedForward;
ValveCommand := Controller.Update(Input := Input);
```

`RejectedExternalSum` and the mixed sequential update are calculation-only
simulation diagnostics. Neither can drive the plant.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Restore, assign the complete candidate record and call Configure once before the controller update. Restore again and pass the prescribed feedforward through PIDInput rather than summing it after the returned output. The declared event lasts 10 seconds. It restores Discharge-pressure setpoint to 6 bar afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Discharge-pressure setpoint (SP, bar); Discharge pressure (PV, bar); Applied valve command (MV, %), including units, limits, mode, and quality. The initial operating point is Discharge-pressure setpoint = 6 bar, Discharge pressure = 6 bar, Applied valve command = 40 %, Feedforward contribution = 0 %. The unresolved baseline problem is: A sequential update exposes new proportional gain with old integral gain and produces a 15.1 percentage-point first increment that belongs to neither complete configuration.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Assign the complete candidate record, call Configure once, and pass feedforward through PIDInput so the controller's anti-windup and output constraints remain authoritative.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **10**. The observed lesson signal is **Discharge-pressure setpoint**. Control Studio observes the project's existing timed sequence and does not write a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to 20 percent prescribed feedforward contribution in the ordering comparison. Confirm the live values are current and the reviewed plan stays inside Discharge pressure 0 bar to 12 bar; Applied valve command 0 % to 100 %.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: atomic_first_update_increment <= 15.300000001 % (tolerance 1e-9 %); mixed_first_update_increment >= 15.099999999 % (tolerance 1e-9 %); constraint_aware_feedforward_output <= 42.000000001 % (tolerance 1e-9 %); rejected_external_sum >= 59.999999999 % (tolerance 1e-9 %).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

**Configuration coherence** is true only when all active fields match either
the complete old revision or the complete new revision at every update.

The **first-update increment** compares the applied output immediately before
the update with the applied output immediately after it. Its calculation is
after minus before, in percentage points.

The **external-sum limit violation** is the rejected post-controller sum minus
the maximum currently allowed output, in percentage points. Smaller is better;
zero means no violation.

The **applied-output rate** is the difference between successive applied
outputs divided by measured cycle time, in percent per second. Its absolute
value must not exceed the configured directional rate.

| Case | Active gains | First/applied output | Engineering state |
| --- | --- | ---: | --- |
| Atomic revision | $K_p$ 15, $K_i$ 3 | 55.300% | Coherent revision 1 |
| Mixed diagnostic | $K_p$ 15, $K_i$ 1 | 55.100% | Invalid mixed record |
| Internal feedforward | $K_p$ 5, $K_i$ 1 | 42.000% | Rate limited correctly |
| Rejected external sum | Not a controller path | 60.000% | Bypasses limit |

The independent unconstrained target diagnostic remains 60.000 percent during
six consecutive rate intervals. Each interval lasts 0.1 seconds and starts from
the previous applied output. Its lower and upper bounds are calculated from that
applied value, not from the unconstrained target. The applied sequence advances
from 42.000 to 52.000 percent without leaving those bounds, and the rate-limited
state remains true on all six scans.

![Every rate interval starts at the prior applied output](figures/atomic-update.svg)

[Retained run](figures/atomic-update-run.csv) ·
[Plot data](figures/atomic-update-trace.csv) ·
[Decision summary](figures/atomic-update-summary.csv)

### Validation

Accept the production update only when every controller scan sees a complete
old or complete new record. The mixed case must be labelled invalid, not
treated as another tuning result. Require the internal feedforward path to
return 42.000 percent on its first scan and remain within 20 percent per second
for all six consecutive scans. Each rate interval must use the previous applied
output as its starting point. The 60.000 percent external sum must remain
disconnected from the plant.

## Conclusion

The equations are not the entire controller. The scan that chooses parameters
and the point where contributions enter are part of its behavior. A mixed
record creates a controller that nobody selected. A post-limit sum creates a
command that anti-windup and rate limiting never observed.

One atomic record and one constraint-aware output path keep the calculation,
retained state, and physical valve command consistent.

## Reason for the next lesson

Use this pattern for online tuning, gain scheduling, selector stations, and
multiloop structures. Atomicity does not prove that values are finite, fresh,
safe, or timely. Lesson 30 addresses invalid data and deterministic recovery.
