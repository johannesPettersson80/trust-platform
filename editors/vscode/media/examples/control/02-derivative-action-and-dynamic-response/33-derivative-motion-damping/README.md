# 34. PD control

You will learn how proportional-derivative (PD) control uses measured shaft velocity to add damping and reduce overshoot. You will calculate the braking torque and explain why derivative-on-measurement avoids a torque spike when the setpoint changes.

## The problem

Lesson 33 showed that P torque can become zero at the target while the shaft still has velocity. Inertia carries it through the target. The negative P torque after crossing eventually reverses the motion, so the shaft can oscillate. A damping term must oppose velocity directly.

## What we need

We need a correction that opposes the shaft's existing velocity, so reaching
the target with nonzero speed does not lead to excessive overshoot. That
braking must respond to motion rather than merely to a changed target.

## The new idea

**Proportional-derivative control**, or **PD**, combines position correction
with a contribution that responds to motion. P tells us how far the shaft
is from its target; measured velocity tells us how much motion needs braking.

Use measured angular velocity omega=dtheta/dt. Keep J=1 kg·m², B=1 N·m·s/rad, Kp=16 N·m/rad, Kd=7 N·m·s/rad, no load, dt=.02 s and torque limits ±20. Compare P and PD with identical resets and clock.

$$
\tau_P=K_p(r-\theta),\qquad \tau_D=-K_d\omega,\qquad
\tau=K_p(r-\theta)-K_d\omega.
$$

Here $r$ and $\theta$ are target and measured angle in radians, $\omega$
is velocity in rad/s and $\tau$ is torque in N·m. With $K_d=7$,
velocity +1 rad/s gives −7 N·m braking; velocity −1 gives +7; at rest D is zero.

Substitute this into Newton's balance J theta''=tau−B omega:

$$
J\ddot\theta+(B+K_d)\dot\theta+K_p\theta=K_pr.
$$

The physical friction B and controller damping Kd add because both oppose velocity. With these values B+Kd=8. Ignoring the constant target and writing z=theta−r gives Jz''+8z'+16z=0. The choice of damping can be explained by a response that approaches the setpoint without crossing it. For a step from rest, try the error shape $e(t)=r(1+at)e^{-at}$, where a>0 sets its decay rate. This has initial error r and initial velocity zero. Its derivatives are $e'=-ra^2te^{-at}$ and $e''=ra^2(at-1)e^{-at}$. Substituting into $Je''+(B+K_d)e'+K_pe=0$ requires $K_p=Ja^2$ and $B+K_d=2Ja$, giving

$$
a=\sqrt{K_p/J},\qquad B+K_d=2\sqrt{JK_p}.
$$

This is the critically damped choice. Here a=4 s⁻¹ and total damping is 8 N·m·s/rad, so Kd=7 supplies the damping beyond the physical B=1. The ideal step response from rest approaches the setpoint without oscillation; sampling and limits must still be checked.

There is a second problem in software. If D differentiates error, a setpoint step changes error by 1 rad in one .02 s update. With Kd=7, a derivative contribution based on error would request +7(1/.02)=+350 N·m, before the shaft has moved. That is a software target kick, not physical damping.

At a fixed setpoint, error derivative is −omega, so +Kd times error derivative and −Kd times measured velocity agree. A setpoint step adds a term +Kd times the setpoint derivative to the error-based version. Derivative-on-measurement uses only −Kd*omega, so SP changes P immediately but D waits for measured motion. This is why the derivative term does not create the 350 N·m target kick.

## Prediction

Calculate P and PD torque at theta=.5, omega=1: P=8 N·m, D=−7, total PD=1. Calculate the initial torque for both at theta=0,omega=0: 16 N·m. Predict which crosses less often, and decide whether critical-damping algebra guarantees the sampled, limited simulation meets the stated requirements.

## Implementation

Open src/Main.st. Declare target, angle, measured velocity, error, P torque, D torque, total request, and applied torque as REAL. Read the supplied angular-velocity measurement for this lesson. Write `DerivativeTorqueNm := -KdNmSPerRad * MeasuredVelocityRadPerS;`, then add it to the P request. Later sampled-control lessons explain how velocity can be estimated from angle samples. Sum P and D, clamp to ±20, and preserve the derivative state only if the selected filter needs it. Initialize all state at zero and reject invalid velocity.

## Experiment in Control Studio

1. Start **Simulator** at angle 0 rad, velocity 0 rad/s, setpoint 0 rad and torque 0 N·m, with no load and a 0.02 s update. Keep Kp=16 in both runs; use Kd=0 for P and Kd=7 for PD.
2. Before running, require peak angle ≤1.1 rad, absolute error ≤0.02 rad from 4 through 8 s, and applied torque within ±20 N·m. In **Control Studio → Monitor → Response**, display angle/setpoint/error in rad, velocity in rad/s, and P/D/requested/applied torque in N·m.
3. In **Test**, review the 8 s sequence. **Confirm and start** establishes time zero and steps the setpoint to 1 rad. Record the full response, then fully reset and repeat with the other Kd.
4. Check error differentiation separately with fixed synthetic angle 0 and a 0→1 rad setpoint change over 0.02 s. It adds +350 N·m of derivative request to the 16 N·m P request; measured-velocity damping adds zero because the shaft is at rest.
5. Abort if angle leaves ±2 rad, velocity leaves ±10 rad/s, applied torque leaves ±20 N·m or readings become invalid. Disable control and command zero torque; the unloaded shaft coasts under friction. Stop the simulator if unresponsive, and reset the full state before repeating.

## Evidence and decision

Compare the first crossing, peak, crossing count, and error window. The equation predicts that Kd=7 raises total damping to the critical value 8. A trace with fewer crossings supports the damping prediction, but only the predeclared peak/error/torque checks decide qualification. Record the actual measured velocity and D contribution; a quieter or slower trace without damping evidence is inconclusive.

## Conclusion

PD combines position stiffness with velocity damping. Derivative-on-measurement brakes actual shaft motion and avoids a target-step derivative kick. For this teaching case the chosen Kd makes B+Kd exactly critical in the continuous ideal model.

## Reason for the next lesson

A constant resisting load needs torque even at rest. PD can supply it through a remaining position error. [Holding a load with PID](../34-servo-pid-holding-torque/README.md) derives that balance and adds integral action to remove the error.
