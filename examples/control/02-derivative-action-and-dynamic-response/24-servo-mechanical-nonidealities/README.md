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
measurement path, at a cost. This lesson keeps the same PID and asks a
wider question: when a real axis moves badly, is the cause the reference,
the drive, or the mechanism?

We first establish an ideal reference, then introduce trajectory
profiling, a torque constraint, Coulomb friction, command deadband,
backlash, and stiction one at a time, with signals synchronized so a
student can identify cause from a signature instead of raising PID gains at
every large error — which increases current, heating, impacts, and wear
without removing the defect.

The drive limit is now ±2.5 N·m, not the ±10 N·m of Lessons 19 through 23,
because the added gearbox is rated for less torque at the motor shaft. This,
the Coulomb friction (0.8 N·m), deadband (0.2 N·m), stiction breakaway
(0.5 N·m), slip jump (0.1 N·m), and slew rate (20 N·m/s) below are
configured teaching values, each measurable on a real axis from a nameplate,
torque test, or command ramp.

## What we need

We need separate command, drive, and mechanism signals so a learner can tell
a request from torque limiting and lost mechanical motion.

## The new idea

The **target position ($SP_{\mathrm{target}}$)** is the desired final table angle in radians
(rad). The **profiled position ($SP_{\mathrm{profile}}$)** is the acceleration- and
velocity-limited position command sent to PID, also in radians. The **load
position ($PV_{\mathrm{load}}$)** is the load-side encoder angle in radians.

The **trajectory velocity limit ($v_{\mathrm{profile}}$)** bounds
$dSP_{\mathrm{profile}}/dt$, in radians per second (rad/s), to 0.5 rad/s.
The **trajectory acceleration limit ($a_{\mathrm{profile}}$)** bounds that
rate's own rate of change, in radians per second squared (rad/s²), to 1.0
rad/s² — a deliberate shaping of the request, neither drive saturation nor
gearbox damage.

The **requested motor torque ($T_{\mathrm{requested}}$)** is the controller's
calculation in newton metres (N m); the **applied motor torque
($T_{\mathrm{applied}}$)** is the simulated drive output after amplitude,
slew, deadband, and stiction, also in newton metres. Their difference is
diagnostic evidence.

The **rotational inertia** $J$ describes resistance to angular acceleration
in kg·m². Newton's rotational law, the same balance Lesson 19 wrote as
$J\ddot\theta+B\dot\theta=\tau_{\mathrm{applied}}$, says the sum of torques
equals inertia times **angular acceleration ($\alpha$)**, $\alpha=\ddot\theta$
in rad/s²:

$\sum T=J\alpha.$

Solving gives $\alpha=\sum T/J$. At zero angle and speed, gravity and
friction are both zero, so if the 2.5 N·m drive limit were the only torque,
$a_{\mathrm{ideal}}=T_{\mathrm{limit}}/J=2.5/1.0=2.5\,\mathrm{rad/s^2}$ —
`a_ideal = T_limit / J = 2.5 N m / 1.0 kg m^2 = 2.5 rad/s^2` — a local upper
bound, not the whole move's acceleration.

**Coulomb friction** opposes motion with nearly constant magnitude, the
**Coulomb friction magnitude ($T_c$)**, in a direction set by the **sign
function ($\operatorname{sign}(\cdot)$)**: $+1$ for positive velocity, $-1$
for negative, in this simplified moving model:

$T_f=-T_c\operatorname{sign}(\dot\theta).$

The sign changes when velocity reverses. The controller must request extra
torque in either direction, but motor and load can still move together.

**Command deadband** means a small command change near the accepted command
is ignored: requested torque can change while applied torque stays
unchanged, a command-path effect, not lost mechanical travel.

The **backlash width ($w_{\mathrm{backlash}}$)** is the total reversal travel through
mechanical clearance, measured in radians. After direction reverses, motor-side
motion crosses the gap before $PV_{\mathrm{load}}$ moves; this kinematic
model omits tooth impact and compliance.

**Stiction**, or static friction, holds the actuator until demand exceeds a
breakaway band, then motion begins with a slip jump — unlike backlash,
which is tied to clearance after reversal, not to being stuck. Similar
delayed PV traces need torque and state signals to tell apart.

### Deriving the motion profile

The profile is deliberately slower than $a_{\mathrm{ideal}}$. The **cycle
time ($\Delta t$)** is 0.02 s, so its maximum velocity change per scan is:

$|\Delta v_p|_{\mathrm{max}}=a_{\mathrm{profile}}\Delta t =1.0(0.02)=0.02\,\mathrm{rad/s},$

— `Δv_max = a_profile × dt = 1.0 rad/s^2 × 0.02 s = 0.02 rad/s` — clamping
each scan's velocity change to ±0.02 rad/s.

At maximum profile speed, its maximum position change per scan is:

$|\Delta SP_{\mathrm{profile}}|_{\mathrm{max}} =v_{\mathrm{profile}}\Delta t=0.5(0.02)=0.01\,\mathrm{rad},$

in code terms `Δposition_max = v_profile × dt = 0.5 rad/s × 0.02 s = 0.01
rad`.

To stop at the target without exceeding acceleration, use the
constant-acceleration relation below, which eliminates time $t$ between
$v_f=v_i+at$ and $\Delta\theta=v_it+\tfrac12at^2$:

$v_f^2=v_i^2+2a\Delta\theta.$

At the target $v_f=0$; with remaining distance
$d=|SP_{\mathrm{target}}-SP_{\mathrm{profile}}|$, solving for the largest
speed that can still stop gives

$v_{\mathrm{stop}}=\sqrt{2a_{\mathrm{profile}}d}.$

The program cruises at 0.5 rad/s far from the target and takes the smaller
of that and $v_{\mathrm{stop}}$ near it.

### Measures and decision rules

Aligning cases at the 10 s reversal, **reversal delay** is the elapsed
time from the commanded reversal to the first sample where load motion in
the new direction reaches 0.020 rad. **Peak following error** is

$e_{\mathrm{peak}}=\max_k|SP_{\mathrm{profile},k}-PV_{\mathrm{load},k}|.$

The **requested-to-applied torque gap** is the largest absolute difference,
$T_{\mathrm{requested}}$ minus $T_{\mathrm{applied}}$:

$g_T=\max_k|T_{\mathrm{requested},k}-T_{\mathrm{applied},k}|,$

near zero unless amplitude, slew, deadband, or stiction prevents the drive
from following. **Torque total variation** sums absolute successive changes
so oscillation cannot cancel:

$TV(T)=\sum_{k=1}^{N}|T_k-T_{k-1}|,$

in accumulated N·m, not instantaneous torque. Each actuator flag's
**active time** is $t_{\mathrm{active}}=N_{\mathrm{active}}\Delta t$; a
duration of zero is evidence too, since backlash should not be renamed
deadband merely because both can delay motion.

A run counts as **settled** only if, over its final two seconds, load
position stays within 0.010 rad and applied torque within 0.050 N·m;
otherwise report **Not settled** and skip final-window position error
rather than trust one sample.

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
one reviewed Boolean authority, $\mathtt{P1.ExperimentEnabled}$, for 20.02 s.
The project owns the complete motion: safe Manual zero, +0.5 rad in Auto,
reversal to -0.5 rad at 10 s, and restoration to target 0 rad, Manual mode,
zero request, when authority returns false.

Fresh native runtimes own the ideal baseline and six one-effect cases,
since changing simulator configuration mid-run would not be the controlled
comparison in the evidence table. Seed 20260828 is deterministic.

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
