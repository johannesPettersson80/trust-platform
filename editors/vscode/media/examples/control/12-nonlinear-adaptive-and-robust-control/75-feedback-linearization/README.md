# 77. Feedback linearization

You will use feedback linearization to compensate the upright arm’s known gravity torque. You will calculate how this changes position control and test the remaining error when the gravity coefficient or measured angle is inaccurate.

## The problem

PD feedback alone must oppose gravity through position error. Exact feedback linearization adds the modeled gravity torque directly, which can improve tracking, but a wrong coefficient leaves a residual and a wrong sign makes the command worse. The cancellation is exact only for the stated arm model and measured angle.

## What we need

We need to oppose the arm's angle-dependent gravity at a nonzero target
without relying on a steady position error to produce holding torque.
The compensation must remain tied to the actual physical model and limits.

## The new idea

**Feedback linearization** uses a nonlinear model-based control action to
produce simpler linear closed-loop dynamics under the model's assumptions.
Here we calculate torque opposing the arm's angle-dependent gravity, then
let P and D shape the remaining motion. This changes the feedback law; it
does not replace the plant's sine relationship with a tangent approximation.

An arm’s weight produces torque equal to weight times its perpendicular lever arm. Measure angle φ from upright; positive torque increases φ, and gravity contributes $+mgl\sin\phi$. Keep the same arm and sign convention from Lesson 76. Use $m=2$ kg, $l=.2038736$ m, $G=mgl=4$ N·m, supplied total inertia $J=1$ kg·m², friction $B=1$ N·m·s/rad, $K_p=16$ N·m/rad and $K_d=7$ N·m·s/rad, torque limits ±20 N·m and $dt=.02$ s. Use $SP=.1$ rad at time 0, starting $\phi=0,\omega=0$, with $Ghat=0,3.5,4$. Requirements are peak $\le.12$ rad, $|SP-\phi|\le.01$ rad from 4–8 s, and applied torque within ±20 N·m.

Let r be the angle setpoint in radians, and let $\widehat G$ be the controller’s gravity coefficient. Use the nonlinear command

$$
\tau=K_p(r-\phi)-K_d\omega-\widehat G\sin\phi.
$$

This cancellation uses the sine relationship itself. Substituting into $J\ddot\phi+B\dot\phi= \tau+G\sin\phi$ gives

$$
J\ddot\phi+(B+K_d)\dot\phi+K_p\phi
=K_pr+(G-\widehat G)\sin\phi.
$$

When $\widehat G=G$, the nonlinear gravity term cancels exactly in this model. With coefficient error, the residual is $(G-\widehat G)\sin\phi$. The static zero-velocity estimate uses $\sin\phi\approx\phi$ only as a paper approximation: $\phi\approx16r/(16-G+\widehat G)$, giving approximately .13333 rad for $Ghat=0$, .1032258 for 3.5, and .1 for 4. These estimates explain why the no-compensation case can miss the .12 requirement; they are not the exact nonlinear trajectory.

## Prediction

Calculate $-4\sin\phi$ at $\phi=.1$ rad and the residual for $Ghat=3.5$. Predict that $Ghat=4$ removes the modeled gravity term, $Ghat=0$ leaves the destabilizing term to PD, and a reversed sign adds gravity instead of canceling it. Predict which candidate reaches the peak/error requirements without saturation.

## Implementation

Use measured angle in the proportional and gravity calculations, and measured velocity in the derivative calculation. True simulation angle is available only to evaluate performance. Write `GravityTorqueNm := -GravityCoefficientNm * SIN(MeasuredAngleRad);`, add it to the P and D outputs, and inspect the sign at positive and negative angles. Calculate gravity feedforward, P, D, unlimited request and applied torque separately. If measured angle, measured velocity or the controller clock is invalid, command zero torque and stop the demonstration; disabling only gravity feedforward leaves invalid PD feedback active. Keep $Ghat$ explicit and do not substitute the local tangent model for the sine.

## Experiment in Control Studio

1. Reset each candidate to $\phi=0,\omega=0$, $SP=0$, no retained state and $dt=.02$ s. At **Confirm and start** apply $SP=.1$ rad and record 8 s.
2. Run $Ghat=0,3.5,4$ as separate full-reset cases with identical model and limits. Display true angle, measured angle, error, gravity feedforward, P/D, request/applied torque and validity.
3. Repeat one selected candidate with additive angle noise $+.002,-.002$ rad alternating every .02 s. Use exact velocity input for this noise comparison rather than finite-difference noise; compare peak, 4–8 s error and command activity $\sum |u_k-u_{k-1}|$ in N·m over 4–8 s. This sum measures torque-command movement, not energy.
4. Stop for true angle outside ±.3 rad, velocity outside ±2 rad/s, applied torque outside ±20 N·m, invalid angle, velocity or clock, or an unresponsive monitor. Command zero torque and stop the simulator; gravity continues acting until it is stopped. Reset before each case.

## Evidence and decision

Assess peak and 4–8 s error separately from torque and noise activity. The true angle is the motion requirement; the controller uses measured angle. If a candidate saturates, mark the linearized comparison inapplicable over that interval. Exact cancellation in the nominal case does not prove robustness to coefficient error, sampling or noise.

## Conclusion

Exact feedback linearization cancels $G\sin\phi$ when the measured angle, sign convention and coefficient are correct. The residual $(G-\widehat G)\sin\phi$, actuator limits and measurement path determine whether the tracking requirements are actually met.

## Reason for the next lesson

The compensation depends on the model coefficient being correct. Parameters
can change during operation, making an old model or tuning unsuitable.
[Adaptive control](../76-adaptive-control/README.md) returns to the heater,
estimates a change in heat loss from observations and qualifies a gain update.
