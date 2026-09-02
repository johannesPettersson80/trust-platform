# Diagnose mechanical nonidealities before retuning a servo

Course position: Lesson 24 of 84. Previous lesson:
[Filtered derivative for an encoder servo](../23-servo-filtered-derivative/README.md).
Next lesson: Digital motion realization (Lesson 25).

**Lesson objective:** Distinguish an intentional trajectory limit, a drive
constraint, friction, command deadband, backlash, and stiction from synchronized
evidence before deciding whether to change PID or repair the mechanism.

This simulation cannot prove a physical defect, safe dither, wear, or a
maintenance interval.

**Equipment:** Geared industrial positioning table.

## The problem

The equipment in this lesson is Geared industrial positioning table.

A geared positioning table moves between plus and minus 0.5 radians. Its PID is
unchanged. A motion profile and declared drive and transmission limits are now
enabled one at a time after an ideal baseline.

Filtering cannot explain every irregular move. Torque limits, an intentional
profile, friction, clearance, and sticking produce different evidence. Raising
gains before identifying the cause can increase heating and wear.

Ask which mechanism explains the signature and whether it needs control,
compensation, or maintenance.

## What we need

We need synchronized evidence that separates intentional trajectory shaping, drive limits, friction, deadband, backlash, and stiction. Only then can the engineer decide whether the remedy belongs in the controller, command path, or mechanism.

## The new idea

The **target position ($SP_{\mathrm{target}}$)** is the desired final table angle in radians
(rad). The **profiled position ($SP_{\mathrm{profile}}$)** is the acceleration- and
velocity-limited position command sent to PID, also in radians. The **load
position ($PV_{\mathrm{load}}$)** is the load-side encoder angle in radians.

The **profiled velocity ($v$)** is the rate at which $SP_{\mathrm{profile}}$ changes, measured
in radians per second (rad/s). The **trajectory velocity limit ($v_{\mathrm{profile}}$)** is
its allowed absolute maximum, 0.5 rad/s. The **trajectory acceleration limit
($a_{\mathrm{profile}}$)** is the allowed rate of change of v, measured in radians per
second squared (rad/s^{2}); it is 1.0 rad/s^{2}. These command-shaping limits are
neither gearbox defects nor drive-torque limits.

The **requested motor torque ($T_{\mathrm{requested}}$)** is the controller's calculation in
newton metres (N m). The **applied motor torque ($T_{\mathrm{applied}}$)** is the simulated
drive output after amplitude, slew, deadband, and stiction behavior, measured in
newton metres. Their difference is diagnostic evidence.

The **rotational inertia ($J$)** describes resistance to angular acceleration in
kilogram square metres (kg m^{2}). The **drive torque limit ($T_{\mathrm{limit}}$)** is the
largest magnitude the drive can apply, 2.5 N m. Ignoring gravity and friction at
the zero-angle starting point, the **ideal angular acceleration ($a_{\mathrm{ideal}}$)** is:

$a_{\mathrm{ideal}} = T_{\mathrm{limit}} / J$

**Coulomb friction** opposes motion with nearly constant torque. It changes sign
with velocity and appears as extra requested torque while moving.

**Command deadband** means a small command change is ignored. Requested torque
changes while the accepted drive command temporarily stays fixed.

The **backlash width ($w_{\mathrm{backlash}}$)** is the total reversal travel through
mechanical clearance, measured in radians. After direction reverses, motor-side
motion crosses the gap before $PV_{\mathrm{load}}$ moves. This kinematic model cannot
reproduce tooth impact or compliance.

**Stiction** holds actuator motion until the requested change exceeds a
breakaway band. Motion can then begin with a slip jump. That hold-then-release
signature differs from the reversal-only clearance of backlash.

## Prediction

At zero angle and speed, the maximum ideal acceleration is:

$a_{\mathrm{ideal}} = T_{\mathrm{limit}} / J = 2.5\,\mathrm{N\,m} / 1.0\,\mathrm{kg\,m^2} = 2.5\,\mathrm{rad/s^2}$

The profile is deliberately slower. The **cycle time ($\Delta t$)** is 0.02 s. Its
maximum velocity change per scan is:

$\Delta v_{\mathrm{max}} = a_{\mathrm{profile}} \times dt = 1.0\,\mathrm{rad/s^2} \times 0.02\,\mathrm{s} = 0.02\,\mathrm{rad/s}$

At maximum profile speed, its maximum position change per scan is:

$\Delta position_{\mathrm{max}} = v_{\mathrm{profile}} \times dt = 0.5 rad/s \times 0.02 s = 0.01 rad$

The configured $w_{\mathrm{backlash}}$ is 0.05 rad. Predict that a reversal can consume
0.05 rad of ideal transmission travel between engaged flanks before the load
again follows. Do not predict a fixed time without also considering speed.

## Implementation

The real Structured Text first calculates a stopping speed, limits the profile
change per scan, and then gives $SP_{\mathrm{profile}}$ to PID:

```st
StoppingVelocity := SQRT(LREAL#2.0 * LREAL#1.0 * ABS(PositionRemaining));
VelocityChange := DesiredVelocity - ProfiledVelocity;
VelocityChange := MAX(MIN(VelocityChange, LREAL#0.02), LREAL#-0.02);
ProfiledVelocity := ProfiledVelocity + VelocityChange;
ProfiledPosition := ProfiledPosition + ProfiledVelocity * LREAL#0.02;

Input.Setpoint := ProfiledPosition;
Input.ProcessValue := LoadPosition;
RequestedTorque := Controller.Update(Input := Input);
```

The ordinary output carries $T_{\mathrm{requested}}$; the simulator produces $T_{\mathrm{applied}}$ and
the actuator-state flags.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Restore the baseline for each run and add only trajectory profiling, the isolated 1 N m torque limit, Coulomb friction, command deadband, backlash, or stiction. Finish with the checked-in combined case and classify it before reading its configuration. The declared event lasts 20 seconds. It restores Target position to 0 rad afterward. The deterministic Simulation seed is 20260828.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Target position (SP, rad); Load position (PV, rad); Requested motor torque (MV, N m), including units, limits, mode, and quality. The initial operating point is PV = 0 rad, MV = 0 N m. The unresolved baseline problem is: The ideal servo response does not explain delayed reversal, requested-to-applied torque separation, or actuator-state flags seen after real drive and transmission effects are introduced.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Classify each synchronized signature before choosing trajectory redesign, constraint-aware control, characterized compensation, inspection, or mechanical maintenance; do not retune PID merely because error increased.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **-1 rad**, and **Maximum duration (s)** = **20**. The affected signal is **Target position**; it starts at 0.5 rad and must restore to 0 rad. Run six one-effect cases from a fresh ideal baseline before the checked-in combined case.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Load position -4 rad to 4 rad; Motor-side velocity -20 rad/s to 20 rad/s; Applied motor torque -2.5 N m to 2.5 N m.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Target position to 0 rad, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260828**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: nonideal_minus_ideal_reversal_delay >= 0.04 s (tolerance 0.02 s); maximum_requested_to_applied_torque_gap >= 0.5 N m (tolerance 0.01 N m); profiled_velocity_absolute_maximum <= 0.5 rad/s (tolerance 0.000001 rad/s); profiled_acceleration_absolute_maximum <= 1 rad/s2 (tolerance 0.000001 rad/s2).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Align traces at the 10 s reversal and use equal axes. **Reversal delay** is the
elapsed time from the commanded reversal to the first sample where load motion
in the new direction reaches 0.020 rad. Calculate that delay, peak absolute
$SP_{\mathrm{profile}}$ minus $PV_{\mathrm{load}}$, requested-to-applied torque gap, and
requested and applied torque total variation. Total variation is the sum of
absolute successive changes and is measured in accumulated N m, not
instantaneous torque.

The **requested-to-applied torque gap** is the absolute value of
$T_{\mathrm{requested}}$ minus $T_{\mathrm{applied}}$ at each sample; report its
maximum over the declared run.

For each actuator flag, calculate active time as its active sample count times
$\Delta t$. A flag duration of zero is evidence too: backlash should not be renamed
deadband merely because both can delay motion.

![Mechanical limits can look like poor controller tuning](figures/mechanical-nonidealities.svg)

[Retained samples](figures/mechanical-nonidealities-trace.csv) ·
[Decision summary](figures/mechanical-nonidealities-summary.csv)

The samples let every movement, active-time, and limit result be recalculated.

### Validation

$SP_{\mathrm{profile}}$ shall stay within ±0.5 rad/s and successive velocity changes shall
stay within ±0.02 rad/s, proving the 1.0 rad/s^{2} acceleration bound. In the
isolated torque case, $T_{\mathrm{applied}}$ shall stay within ±1.0 N m. In the combined
checked-in case, $T_{\mathrm{applied}}$ shall stay within ±2.5 N m. Other cases use ±10 N m.
Values must remain finite.

Report final-window position error only if both $PV_{\mathrm{load}}$ and $T_{\mathrm{applied}}$ satisfy
declared settling spans over the final two seconds. Otherwise report **Not
settled**. Never use the last oscillation sample as steady-state evidence.

The retained isolated cases produced these signatures:

| Case | Reversal delay | Peak following error | Settled? | Distinguishing evidence |
| --- | ---: | ---: | :---: | --- |
| Unprofiled ideal | 0.06 s | 0.974 rad | No | abrupt reference produces high following error |
| Trajectory profile | 0.52 s | 0.326 rad | No | bounded velocity and acceleration |
| Torque constraint | 1.88 s | 0.611 rad | Yes | 14.88 s at the amplitude limit |
| Coulomb friction | 0.88 s | 0.425 rad | No | additional motion-opposing torque |
| Command deadband | 0.56 s | 0.334 rad | No | deadband state active for 19.12 s |
| Backlash | 0.68 s | 0.339 rad | No | reversal-dependent lost travel |
| Stiction | 0.82 s | 0.346 rad | No | stiction state active for 19.40 s |
| Combined | 1.34 s | 0.487 rad | No | overlapping actuator and transmission states |

“Settled” does not mean acceptable: the torque-limited case settled with a
0.250 rad error because the actuator lacked torque. These are simulation results.

## Conclusion

A rounded $SP_{\mathrm{profile}}$ with bounded v identifies intentional motion profiling.
Requested/applied separation plus saturation or slew flags identifies a drive
constraint. Extra nearly constant torque during motion suggests Coulomb
friction. A held accepted command identifies command deadband. Lost travel only
after reversal suggests backlash. Hold followed by breakaway suggests stiction.

Effects can coexist. Change one factor and diagnose from synchronized evidence,
not controller error alone.

## Reason for the next lesson

Profiles protect healthy mechanisms. Dither or friction compensation requires
separate safety, wear, and process-quality review.

Retuning shall not be presented as mechanical repair. Excessive clearance,
wear, or sticking requires inspection or maintenance. Lesson 25 keeps this
positioning table and tests whether the discrete controller remains faithful as
sample time and numerical realization change.
