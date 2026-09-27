# 35. PID load rejection

You will use integral action in PID control to reject a constant load disturbance. You will calculate the torque needed to hold a hanging mass and explain why PD needs a position error while PID can remove it.

This uses the [PI accumulation](../../01-feedback-foundations-and-temperature-control/05-water-heater-pi/README.md) and [anti-windup](../../01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup/README.md) ideas with the shaft dynamics just taught.

## The problem

The unloaded shaft could remain at its setpoint with zero motor torque. We now attach a hanging mass that requires torque even at rest.

Use a rigid shaft with $J=1$ kg·m², viscous damping $B=1$ N·m·s/rad, $K_p=16$ N·m/rad, $K_d=7$ N·m·s/rad, $K_i=4$ N·m/(rad·s), and 0.02 s updates. A 2 kg mass hangs from a constant-radius 0.1 m drum. Positive rotation lifts it; the cable remains taut.

## What we need

We need to supply the load's holding torque even when position error and
velocity are zero. The correction must remove the steady position error
while retaining the damping already provided by PD.

## The new idea

**PID control** combines proportional correction, accumulated-error correction
and derivative damping. We already used I to retain heater power at zero
temperature error. Here it retains torque against the constant load:

$$
u_r=K_p(SP-\theta)+I-K_d\omega.
$$

$u_r$ is requested torque in N·m, $SP-\theta$ the angle error in radians,
$\omega$ measured velocity in rad/s, and $I$ the retained torque contribution.

Here m is mass, g=9.81 m/s² is gravitational acceleration and R is drum radius. The mass weighs $mg=2(9.81)=19.62$ N. Its opposing torque is $\tau_L=mgR=19.62(0.1)=1.962$ N·m. Reflected inertia $mR^2=0.02$ kg·m² is neglected relative to J=1, an explicit 2% approximation. Applied torque is limited to ±20 N·m.

Requirements are fixed before testing:

| Test | Requirement |
| --- | --- |
| Unloaded SP step | From 0 to 1 rad, 8 s; peak ≤1.1 rad; $\lvert e\rvert\le0.02$ rad from 4–8 s |
| Loaded hold | Start at angle 1 rad, velocity 0, SP=1, I=0, no initial load; apply 1.962 N·m at time zero for 20 s; $\lvert e\rvert\le0.02$ rad from 12–20 s |

Run PD and PID as separate resets. Under load, zero motor torque lets the mass descend; this is not a safe physical hold.

At rest, PD derivative torque is zero. PD balance requires $K_pe=\tau_L$, so $e=1.962/16=0.122625$ rad and $\theta=1-0.122625=0.877375$ rad. PID stores the load torque in I. Its update is

$$
I_{new}=I_{old}+K_i e\Delta t.
$$

For $e=0.1$ rad and $\Delta t=0.02$ s, $\Delta I=4(0.1)(0.02)=0.008$ N·m. At exact target and zero velocity, P and D are zero, so steady I must be 1.962 N·m. Use torque anti-windup with tracking time $T_t=0.2$ s, so the correction factor $\Delta t/T_t=0.1$, and preserve that choice for Lessons 36–39.

## Prediction

Calculate PD’s loaded angle, PID’s required I, and the 0.008 N·m update. Predict unloaded peak/settling and loaded late error. Predict what happens when applied torque reaches ±20 N·m and when zero torque is commanded under load.

## Implementation

Write scalar P, I, D, trial-I, requested-torque, limited-torque, and tracking calculations. Read actual angle and velocity; do not overwrite plant measurements. Use $u_P=K_p(SP-\theta)$, $u_D=-K_d\omega$, and $u=u_P+I+u_D$, form the request with the trial integral, then limit it to ±20 N·m. Write `IntegralNm := TrialIntegralNm + 0.1 * (AppliedTorqueNm - RequestedTorqueNm);`. Including D in the complete request preserves the same tracking derivation: the desired integral is applied torque minus P minus D. Initialize I separately for each test; do not reuse unloaded state.

## Experiment in Control Studio

Use the actual three-page flow **Identify → Tune → Validate**. On **Identify**, choose the project-owned test, inspect the declared shaft signals, open the compact parameter editor and select `TrialKind` 0–3: unloaded PD, unloaded PID, loaded PD or loaded PID. Press **Review project test**, then **Confirm and start**. Each choice is a separate reset; the project writes the load and setpoint event through normal PLC I/O.

After each recording, keep the measured response visible on Identify, then use **Tune** to inspect the selected controller and **Validate** to review the applicable requirements. Do not treat page navigation or a retained recording as a new run. Compare angle, velocity, P/I/D, requested/applied torque, load and elapsed time with the stated peak/error windows. Stop on angle outside ±2 rad, velocity outside ±10 rad/s, applied torque outside ±20 N·m, invalid measurements/clock, or an unresponsive simulator. Applied torque at ±20 is an expected limit, not an automatic abort. On stop, command zero torque, cancel the virtual load before reset, and stop the simulator if necessary. Do not claim physical safe holding from zero-torque shutdown.

## Evidence and decision

Compare PD and PID separately against both windows, torque limits, and state initialization. The calculation predicts PD near 0.877375 rad and PID near 1 rad with I=1.962 N·m. Keep the 2% inertia approximation explicit and report saturation or invalid-data stops as evidence boundaries.

## Conclusion

PD can hold a constant drum load by accepting position error. PID accumulates the required 1.962 N·m and can remove that error when the torque is available.

## Reason for the next lesson

The load comparison used a clean velocity signal. If velocity is estimated
from noisy angle samples, D can command braking for motion the shaft did not
make. [Derivative filtering](../36-filtered-derivative/README.md) investigates
that noise and braking tradeoff, first on the unloaded shaft to isolate it.
