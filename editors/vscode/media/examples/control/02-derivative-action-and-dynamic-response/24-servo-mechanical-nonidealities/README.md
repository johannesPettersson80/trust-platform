# Diagnose mechanical nonidealities before retuning a servo

Course position: Lesson 24 of 84. Previous lesson: [Filtered derivative for an encoder servo](../23-servo-filtered-derivative/README.md). Next lesson: [Qualify a digital PID realization on the positioning table](../25-servo-digital-realization/README.md).

**Lesson objective:** Distinguish an intentional trajectory limit, a drive
constraint, friction, command deadband, backlash, and stiction from synchronized
evidence before deciding whether to change PID or repair the mechanism.

**Evidence class:** Simulation evidence. This simulation cannot prove a
physical defect, safe dither, wear, or a maintenance interval.

## The problem

The equipment is the geared industrial positioning table used since Lesson
19, now driven through a gearbox that adds backlash and reversing between
+0.5 and -0.5 rad. Lesson 23 showed a noisy torque trace can come from the
measurement path, at a measurable cost. This lesson keeps the same PID and
asks a wider question: when a real axis moves badly, is the cause the
reference, the drive, or the mechanism?

We first establish an ideal reference case, then introduce trajectory
profiling, a torque constraint, Coulomb friction, command deadband,
backlash, and stiction one at a time, with signals synchronized so a
student can identify cause from a signature instead of raising PID gains at
every large error — which can increase current, heating, impacts, and wear
without removing the defect.

The drive limit is now ±2.5 N·m, not the ±10 N·m of Lessons 19 through 23,
because the added gearbox is rated for less torque at the motor shaft. This,
the Coulomb friction (0.8 N·m), deadband (0.2 N·m), stiction breakaway
(0.5 N·m), slip jump (0.1 N·m), and slew rate (20 N·m/s) below are
configured teaching values, each measurable on a real axis from a nameplate,
torque test, or command ramp.

## What we need

We need separate command, drive, and mechanism signals so a learner can tell a
controller request from torque limiting and from lost mechanical motion.

## The new idea

The **target position ($SP_{\mathrm{target}}$)** is the desired final table angle in radians
(rad). The **profiled position ($SP_{\mathrm{profile}}$)** is the acceleration- and
velocity-limited position command sent to PID, also in radians. The **load
position ($PV_{\mathrm{load}}$)** is the load-side encoder angle in radians.

The **trajectory velocity limit ($v_{\mathrm{profile}}$)** bounds the rate of
change of the profiled command, $dSP_{\mathrm{profile}}/dt$, in radians per
second (rad/s), to 0.5 rad/s. The **trajectory acceleration limit
($a_{\mathrm{profile}}$)** bounds the rate of change of that velocity, in
radians per second squared (rad/s²), to 1.0 rad/s². These limits
deliberately shape what is requested. They are neither drive saturation nor
gearbox damage.

The **requested motor torque ($T_{\mathrm{requested}}$)** is the controller's calculation in
newton metres (N m). The **applied motor torque ($T_{\mathrm{applied}}$)** is the simulated
drive output after amplitude, slew, deadband, and stiction behavior, measured in
newton metres. Their difference is diagnostic evidence.

The **rotational inertia** $J$ describes resistance to angular acceleration
in kg·m². Newton's rotational law, the same balance Lesson 19 wrote as
$J\ddot\theta+B\dot\theta=\tau_{\mathrm{applied}}$, says the sum of torques
equals inertia times **angular acceleration ($\alpha$)**, $\alpha=\ddot\theta$
in rad/s²:

$\sum T=J\alpha.$

Solving for acceleration gives $\alpha=\sum T/J$. At zero angle and zero
speed, gravity $4\sin(\theta)$ and viscous friction $B\dot\theta$ are both
zero, so if the 2.5 N·m drive limit were the only torque, the initial
acceleration would be $a_{\mathrm{ideal}}=T_{\mathrm{limit}}/J
=2.5/1.0=2.5\,\mathrm{rad/s^2}$ — in code terms, `a_ideal = T_limit / J = 2.5
N m / 1.0 kg m^2 = 2.5 rad/s^2`. Away from that instant, gravity and friction
must be subtracted; this is a local upper-bound calculation, not the
acceleration of the whole move.

**Coulomb friction** opposes motion with nearly constant magnitude, set by
the **Coulomb friction magnitude ($T_c$)**. Its direction uses the **sign
function ($\operatorname{sign}(\cdot)$)**, which returns $+1$ for positive
velocity and $-1$ for negative velocity, in a simplified moving model:

$T_f=-T_c\operatorname{sign}(\dot\theta).$

The sign changes when velocity reverses. The controller must request extra
torque in either direction, but motor and load can still move together.

**Command deadband** means a small command change near the accepted command
is ignored: requested torque can change while applied torque stays
unchanged, a command-path effect, not lost mechanical travel.

The **backlash width ($w_{\mathrm{backlash}}$)** is the total reversal travel through
mechanical clearance, measured in radians. After direction reverses, motor-side
motion crosses the gap before $PV_{\mathrm{load}}$ moves. This kinematic model cannot
reproduce tooth impact or compliance.

**Stiction**, or static friction, holds the actuator until demand exceeds a
breakaway band. Motion can then begin with a slip jump. The hold is tied to
being stuck; backlash is tied specifically to taking up clearance after a
direction reversal. Similar delayed PV traces therefore need torque and state
signals to distinguish them.

### Deriving the motion profile

The profile is deliberately slower than $a_{\mathrm{ideal}}$. The **cycle
time ($\Delta t$)** is 0.02 s, so its maximum velocity change per scan is:

$|\Delta v_p|_{\mathrm{max}}=a_{\mathrm{profile}}\Delta t =1.0(0.02)=0.02\,\mathrm{rad/s},$

in code terms `Δv_max = a_profile × dt = 1.0 rad/s^2 × 0.02 s = 0.02 rad/s`.
This is why the program clamps each scan's velocity change to ±0.02 rad/s.

At maximum profile speed, its maximum position change per scan is:

$|\Delta SP_{\mathrm{profile}}|_{\mathrm{max}} =v_{\mathrm{profile}}\Delta t=0.5(0.02)=0.01\,\mathrm{rad},$

in code terms `Δposition_max = v_profile × dt = 0.5 rad/s × 0.02 s = 0.01
rad`.

To stop at the target without exceeding acceleration, use the
constant-acceleration relation below, which eliminates time $t$ between
$v_f=v_i+at$ and $\Delta\theta=v_it+\tfrac12at^2$:

$v_f^2=v_i^2+2a\Delta\theta.$

At the target $v_f=0$. With remaining distance
$d=|SP_{\mathrm{target}}-SP_{\mathrm{profile}}|$ and braking acceleration
opposite the motion, solving for the largest current speed that can still stop
gives

$v_{\mathrm{stop}}=\sqrt{2a_{\mathrm{profile}}d}.$

The program chooses the smaller of this stopping speed and the 0.5 rad/s
cruise limit. Far from the target it cruises; near the target the square-root
bound starts deceleration.

### Measures and decision rules

Aligning every case at the 10 s reversal, **reversal delay** is the elapsed
time from the commanded reversal to the first sample where load motion in
the new direction reaches 0.020 rad. **Peak following error** is

$e_{\mathrm{peak}}=\max_k|SP_{\mathrm{profile},k}-PV_{\mathrm{load},k}|.$

The **requested-to-applied torque gap** is

$g_T=\max_k|T_{\mathrm{requested},k}-T_{\mathrm{applied},k}|,$

near zero when the drive follows the request and growing when amplitude,
slew, deadband, or stiction prevents that. **Torque total variation** sums
absolute successive changes so oscillation cannot cancel:

$TV(T)=\sum_{k=1}^{N}|T_k-T_{k-1}|,$

in accumulated N·m, not instantaneous torque. Each actuator flag's
**active time** is $t_{\mathrm{active}}=N_{\mathrm{active}}\Delta t$; a
duration of zero is evidence too, since backlash should not be renamed
deadband merely because both can delay motion.

A run counts as **settled** only if, over its final two seconds, load
position stays within a 0.010 rad range and applied torque stays within a
0.050 N·m range; otherwise report **Not settled** rather than trust one
final sample. Report final-window position error only for a settled run.

## Prediction

The configured $w_{\mathrm{backlash}}$ is 0.05 rad. Predict that a reversal
can consume 0.05 rad of motor-side travel before the load follows. The time is
not fixed: at constant speed $|v|$, the simple estimate is
$t_{\mathrm{gap}}=0.05/|v|$, so it depends on the reversal trajectory.

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

Control Studio executes the checked-in combined diagnostic case. Test writes
one reviewed Boolean authority, $\mathtt{P1.ExperimentEnabled}$, for 20.02
seconds. The project owns the complete motion: safe Manual zero first, +0.5 rad
in Auto, reversal to -0.5 rad at 10 seconds, and restoration to target 0 rad,
Manual mode, and zero request when authority returns false.

Fresh native runtimes own the ideal baseline and six one-effect cases.
That separation matters: changing simulator configuration during one UI run
would not be the controlled comparison reported in the evidence table. The
deterministic Simulation seed is 20260828.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait for **Simulation**, **Running**, current samples, and good quality. Confirm target and profiled position = 0 rad, profiled velocity = 0 rad/s, load position = 0 rad, requested torque = 0 N·m, Manual mode, and experiment authority false. Add target, profiled position/velocity, load position, motor velocity, requested torque, mode, and authority to **Signals**; applied torque and actuator-state flags have no bound PLC symbol here and appear only as the plant-evidence panel's Applied input and Active effects text, not as Response traces.
   - **Decision before action:** Write the calculated 0.02 rad/s velocity increment, 0.01 rad position increment, and expected reversal signatures. Commit to classifying the evidence before proposing PID changes.
   - **Successful evidence:** The plot advances, values are finite and current, quality is good, and **Start recording** is enabled. Select **Start recording**.
   - **Recovery:** If samples are stale, unavailable, nonfinite, or poor quality, stop the Simulator, correct the displayed runtime or binding fault, restart, and repeat the baseline check.
   - **Record:** Note every initial signal above and your predicted command-, drive-, and mechanism-path signatures.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **20.02**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). The project owns both target changes and their timing.
   - **Decision before action:** Confirm the plan writes only that Boolean and restores it false. Require load position within ±4 rad, motor velocity within ±20 rad/s, and applied torque within ±2.5 N·m. Predict which trace changes first at the 10-second reversal.
   - **Successful evidence:** Select **Review test**, verify authority, timeout, stop conditions, and false restoration, then select **Confirm and start**. Keep **Abort test** available. Response must show the profiled move to +0.5 rad, the -0.5 rad reversal, and the requested-torque trace, followed by complete safe restoration; the plant-evidence panel shows the applied/state separation.
   - **Recovery:** Select **Abort test** for nonfinite, stale, or out-of-limit data; confirm authority false, target/profile state reset to 0 rad, Manual mode, and zero requested torque before retrying.
   - **Record:** Retain the synchronized reversal traces, terminal state, restoration, seed, reversal delay, peak following error, torque gap and variation, state durations, and settling disposition.

There is no separate disabled Validate stage. The native test owns the
one-effect matrix; Monitor and Test expose the checked-in combined case.

## Evidence and decision

Every run must keep $SP_{\mathrm{profile}}$ within ±0.5 rad/s and successive
velocity changes within ±0.02 rad/s, proving the 1.0 rad/s² acceleration
bound. In the isolated torque case, $T_{\mathrm{applied}}$ shall stay within
±1.0 N m; in the combined checked-in case, within ±2.5 N m; other cases use
±10 N m. Values must remain finite, and only a settled run reports
final-window position error.

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

![Mechanical limits can look like poor controller tuning](figures/mechanical-nonidealities.svg)

Each panel aligns SP_profile, PV_load, and torque at the 10 s reversal so
the reversal-delay gap and torque-gap width can be compared by eye across
cases before trusting the table. (Configured Simulation evidence.)

[Retained samples](figures/mechanical-nonidealities-trace.csv) ·
[Decision summary](figures/mechanical-nonidealities-summary.csv)

“Settled” does not mean acceptable: the torque-limited case settled with a
0.250 rad error because the actuator lacked torque. These are simulation
results, and the samples let every movement, active-time, and limit result
be recalculated.

## Conclusion

A rounded $SP_{\mathrm{profile}}$ with bounded $v_{\mathrm{profile}}$
identifies intentional profiling; requested/applied separation with
saturation or slew flags, a drive constraint; extra near-constant torque
during motion, Coulomb friction; a held command, deadband; travel lost only
after reversal, backlash; hold then breakaway, stiction. Effects can
coexist, so change one factor and diagnose from synchronized evidence, not
controller error alone. Profiles protect a healthy mechanism; retuning shall
not be presented as mechanical repair, and dither, friction compensation, or
excessive clearance and wear require separate safety and maintenance review.

## Reason for the next lesson

This lesson assumed the discrete controller itself was faithful to its
continuous design. Lesson 25 keeps this positioning table and tests whether
that assumption holds as sample time and numerical realization change.
