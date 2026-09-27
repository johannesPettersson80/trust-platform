# 39. Servo commissioning

You will commission the shaft’s position controller for both setpoint changes and a constant load disturbance. You will write the complete ST integration, test positioning and holding behavior, and check initialization, transfer, limits and measurement-fault recovery.

Enter after the motion lessons through [positional and incremental PID](../../03-production-quality-digital-pid/38-positional-incremental-pid/README.md) and the [controller lifecycle lessons](../../03-production-quality-digital-pid/39-controller-mode-initialization/README.md) through measurement recovery.

## The problem

The sampled PID calculations can agree while the complete servo still fails
its motion requirements or mishandles a change of operating state. We need
to assess positioning, load support and lifecycle behavior together on the
same mechanism.

Use $J=1$ kg·m², $B=1$ N·m·s/rad, 0.02 s updates, torque bound ±20 N·m, and a 2 kg mass on a 0.1 m drum. The load torque is 1.962 N·m. Reflected inertia $mR^2=0.02$ kg·m² is explicitly neglected as a 2% approximation. A zero-torque stop allows the mass to descend; no brake physics is supplied.

## What we need

We need to verify the complete servo under positioning, load and lifecycle
requirements. Matching the PID arithmetic does not show that startup,
transfer, limits and measurement recovery work together correctly.

## The new idea

**Servo commissioning** checks whether the complete motion controller meets
its positioning, load and operating-state requirements. Matching the PID
arithmetic is not enough: measurement handling, initialization, torque limits,
mode transfer and recovery must work together on the same shaft.

At initialization, use actual angle and velocity. With $P=K_p(SP-\theta)$ and $D=-K_d\omega$, align integral state to applied torque with $I=u_{applied}-P-D$. Do not carry $I=1.962$ from a different start: the loaded test begins with load zero and I=0, then applies the load at time zero. A preloaded alternative with load already applied and I=1.962 is a different explicitly stated start.

Use selected derivative filtering and the Lesson 35 anti-windup factor 0.1. Parameter changes to both P and D require an I adjustment to preserve the current request. A stale/invalid sample must stop state updates and enter the declared fault state; recovery needs fresh data and deliberate re-enable.

## Prediction

Predict unloaded peak and settling, loaded late error, first torque after the 1.962 N·m load, and the request change at a P/D revision. Predict which state and output are safe to claim when angle or velocity is invalid, and why zero-torque shutdown is not physical safe holding.

## Implementation

Write the integration with scalar P, I, D, trial torque, request, limit, tracking, mode, validity, timestamp and revision fields. Read actual angle/velocity. Choose timing thresholds suited to 0.02 s motion; for example, a 0.04 s sample-age limit and 0.05 s maximum controller-update interval are candidate values that must be justified and tested, not copied blindly. Keep planned fault injection separate from unexpected bound violations.

## Experiment in Control Studio

Write a coherent integration record with selected P/I/D gains, derivative filter from Lesson 36, positional/incremental realization from Lessons 37–38, tracking time $T_t=0.2$ s, limits, mode ownership, reset state, and validity policy. Prerequisites are [initialization](../../03-production-quality-digital-pid/39-controller-mode-initialization/README.md), [bumpless transfer](../../03-production-quality-digital-pid/40-bumpless-transfer/README.md), [atomic parameter update](../../03-production-quality-digital-pid/41-atomic-parameter-update/README.md), and [invalid/stale data](../../03-production-quality-digital-pid/42-invalid-stale-data/README.md).

Use the actual three-page flow **Identify → Tune → Validate**. On **Identify**, choose the project-owned test and select `TrialKind` 0 (unloaded move), 1 (loaded hold) or 2 (invalid-quality recovery) in the compact project parameter editor. Press **Review project test**, then **Confirm and start**. Reset before each case so the P, filtered PD, selected PID and coarse comparison use a new plant history; a candidate comparison in one recording does not replace separate case evidence.

Requirements are declared before any result:

| Test or function | Requirement |
| --- | --- |
| Unloaded motion | SP 0→1 rad, 8 s; peak ≤1.1 rad; $\lvert e\rvert\le0.02$ rad from 4–8 s |
| Loaded hold | Reset angle 1, velocity 0, SP 1, I 0, no initial load; apply 1.962 N·m at time zero for 20 s; $\lvert e\rvert\le0.02$ rad from 12–20 s |
| Lifecycle | Initialization, manual transfer, atomic P/D/I revision, stale/invalid measurement latch, fresh-data recovery, separate re-enable |

1. For `TrialKind` 0, reset angle 0, velocity 0, SP 0, load 0, I 0, previous-angle, previous-velocity and filter states, then run the unloaded 8 s test. Apply the motion requirements before recording.
2. For `TrialKind` 1, fully reset to angle 1, velocity 0, SP 1, load 0, I 0; the project applies 1.962 N·m at the run boundary and records the loaded hold. Display angle, velocity, P/I/D, request/applied torque, load and timing.
3. For `TrialKind` 2, run the planned invalid-quality interval and fresh-data recovery. Initialization, manual transfer, atomic P/D/I revision and deliberate re-enable remain separate lifecycle checks with their own reset and event clock. Review the measured recording on Identify, then use **Tune** and **Validate** to inspect the applicable requirements; page navigation does not start another case.
4. Stop on angle outside ±2 rad, velocity outside ±10 rad/s, applied torque outside ±20 N·m, invalid clock/data outside the planned fault case, or unresponsive signals. Command zero torque, cancel virtual load before reset, and stop the simulator if needed.

## Evidence and decision

Report each requirement with reset, event time, gains/revision, trace interval and units. Normal torque at ±20 N·m is permitted if requirements are met; saturation must remain visible. A lifecycle pass requires ownership, state alignment, atomic revision, fault latch, recovery freshness and separate re-enable. A zero-torque stop is simulation shutdown evidence only; physical safe holding would require a separately specified brake or actuator.

## Conclusion

Commissioning the rigid shaft requires one coherent P/I/D integration, both unloaded and loaded tests, and explicit lifecycle evidence. The load-start state, derivative term, parameter changes and timing validity must all be represented in the state alignment.

## Reason for the next lesson

The servo assessment connects torque, angular motion and controller state. The next application controls liquid level: its actuator changes inlet flow, while the level changes as water accumulates or drains. A torque-to-motion model cannot describe that process.

[Volume balance](../../01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation/README.md) derives the tank’s level equation from inflow, outflow and stored volume before choosing a level controller.
