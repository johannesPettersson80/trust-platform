# 33. Position control

You will command motor torque proportional to position error to move a rigid shaft to an angle setpoint. You will calculate its initial acceleration and explain why it can still be moving when position error reaches zero.

## The problem

The valve investigation showed how actual position can differ from a request. We now isolate the motion of a healthy motor using a rigid shaft; this is not a model of the faulty valve’s full mechanism. Unlike the heater’s stored thermal energy, a rotating shaft stores kinetic energy. Once the shaft has angular velocity, it keeps moving until a torque changes that velocity. A position controller may reduce its torque to zero exactly when the shaft reaches its target. If the shaft is still moving at that instant, it passes the target.

We need a physical vocabulary before changing gains. Position tells us where the shaft is. Angular velocity tells us how fast its angle is changing. Angular acceleration tells us how quickly that velocity changes. A larger position correction creates greater acceleration; angle then evolves as the resulting velocity accumulates.

## What we need

We need a rule that turns position error into motor torque, together with
a physical prediction of how that torque changes velocity and angle. This
will let us judge whether position correction alone gives acceptable motion.

## The new idea

**Position control** uses feedback to move an object to a desired position.
For a shaft, the actuator supplies torque, torque changes velocity, and
velocity changes angle. That sequence explains why reaching the target angle
does not instantly stop existing motion.

Use the same direction as positive for angle, velocity, and motor torque. Define radians and torque, derive the rigid-shaft balance, and compare two proportional gains. The supplied teaching case is J=1 kg·m², B=1 N·m·s/rad, no external load, torque limited to −20 through +20 N·m, and controller update interval dt=.02 s. These are supplied model quantities. Obtain inertia from mass distribution or an acceleration test with known torque; estimate viscous friction from the torque required at steady measured speeds.

An angle is measured in radians: one radian subtends an arc whose length equals the radius, and one full turn is 2π radians. Torque is turning effort. A 1 N force perpendicular to a 1 m lever arm produces 1 N·m. Let r be the requested angle, θ the measured angle, and e=r−θ the position error. Proportional feedback requests:

$$
\tau_P=K_p e.
$$

Kp is commanded rotational stiffness in N·m/rad. At r=1 and θ=0, Kp=4 requests 4 N·m and Kp=16 requests 16 N·m. At θ=1.2, e=−.2, so they request −.8 and −3.2 N·m. The sign reverses after the target, pulling back toward it. The torque limit clips a request beyond ±20; a clipped torque is different from the unclipped equation.

Angular velocity is ω=dθ/dt in rad/s and acceleration is α=dω/dt=d²θ/dt² in rad/s². The shaft's inertia J measures resistance to angular acceleration: Newton's rotational law says net torque equals Jα. Viscous friction is a useful linear approximation for losses in bearings and air; it opposes velocity with τB=−Bω. The balance is:

$$
J\ddot\theta=\tau_P-B\dot\theta.
$$

Substitute P feedback and rearrange:

$$
J\ddot\theta+B\dot\theta+K_p\theta=K_pr.
$$

At the initial rest state friction is zero. Therefore α=τP/J: 4/1=4 rad/s² for Kp=4 and 16/1=16 rad/s² for Kp=16. The high-gain shaft accelerates four times as strongly; it does not move four times as far in one scan.

To predict the swing frequency, temporarily ignore friction and hold r fixed. Let z=θ−r. Then J z''+Kp z=0. Try z=A cos(ωn t), where A is angular amplitude in radians and ωn is the oscillation's angular frequency in rad/s. Differentiating twice gives z''=−ωn²z, so (Kp−Jωn²)z=0. A nonzero motion therefore has:

$$
\omega_n=\sqrt{K_p/J}.
$$

With J=1, the natural frequencies are 2 rad/s for Kp=4 and 4 rad/s for Kp=16. This is a frequency of the ideal undamped oscillation, not a claim that settling time is simply its reciprocal. Oscillation frequency describes progress through a swing; it is different from the shaft's instantaneous angular velocity. Friction and the sampled controller affect the recorded response. At θ=r, P torque is zero but ω may not be; that is the central reason position-only feedback can cross the target.

## Prediction

Before enabling the shaft, calculate and explain:

1. The torque and acceleration at r=1, θ=0, ω=0 for Kp=4 and 16.
2. The torque for each gain at θ=1.2 rad.
3. The natural frequency sqrt(Kp/J) for each gain.
4. Why torque is zero at θ=1 while velocity can remain nonzero.
5. Which gain should reach the first target crossing sooner and which is more likely to overshoot.

Your decision rule is explicit before the run: compare peak angle with 1.1 rad and require absolute error at most .02 rad from 4 s through 8 s. Applied torque must stay within ±20 N·m. A faster first crossing does not qualify a gain if its later error fails.

## Implementation

Write the P torque calculation using `REAL` values for measured angle, setpoint, gain and torque:

```st
PositionErrorRad := SetpointRad - MeasuredAngleRad;
RequestedTorqueNm := KpNmPerRad * PositionErrorRad;
```

Apply the known limiting decision at −20 and +20 N·m. Check the initial and overshot positions from the prediction before running.

For a one-step plant prediction, use separate model states. Calculate acceleration from `(AppliedTorqueNm - FrictionNmSPerRad * ModelVelocityRadPerS) / InertiaKgM2`, then calculate both next states from the previous states:

```st
NextVelocityRadPerS := ModelVelocityRadPerS + AccelerationRadPerS2 * ElapsedS;
NextAngleRad := ModelAngleRad + ModelVelocityRadPerS * ElapsedS;
ModelVelocityRadPerS := NextVelocityRadPerS;
ModelAngleRad := NextAngleRad;
```

This is a forward-Euler approximation: each old derivative is held for the 0.02 s interval. It predicts the plant; it does not overwrite the measured angle. At the first update from rest, predicted velocity changes while this particular approximation leaves angle unchanged until the following update.

## Experiment in Control Studio

1. Start **Simulator** with angle 0 rad, angular velocity 0 rad/s, setpoint 0 rad, applied torque 0 N·m, automatic control disabled and no external load. Select Kp=4 N·m/rad for the first run.
2. In **Control Studio → Monitor → Response**, display setpoint/angle/error in rad, velocity in rad/s, requested/applied torque in N·m, gain and elapsed time in seconds.
3. In **Test**, review the 8 s sequence. **Confirm and start** establishes time zero, enables P control and steps the setpoint to 1 rad. Keep the gain unchanged throughout the run.
4. Record first crossing, peak angle, later crossings and error from 4 through 8 s. The duration tests both initial motion and the required settled behavior.
5. Disable control and command zero torque. With no load, friction slows the remaining motion; the shaft does not return to zero just because the command is zero. Restart at the complete initial state, select Kp=16 and repeat the same test.
6. Select **Abort test** if angle leaves −2 to 2 rad, velocity leaves −10 to 10 rad/s, applied torque leaves −20 to 20 N·m, or measurements stop updating. Confirm zero torque, or stop the simulator if unresponsive. Restart fully before another comparison.

## Evidence and decision

The direction prediction follows directly from e=r−θ: below target, e and torque are positive; above target, both are negative. The initial acceleration calculation explains why Kp=16 begins more strongly. The trial sinusoid derivation predicts a doubled natural frequency when Kp increases from 4 to 16, but friction and the sampled update determine the actual trace.

Compare each gain with the predeclared requirements. A gain qualifies only if peak ≤1.1 rad, absolute error ≤.02 rad at every sample from 4 through 8 s, and applied torque remains inside ±20 N·m. Preserve the common reset, clock, and signal validity. If neither qualifies, record that P-only stiffness is insufficient; do not manufacture a winner from a visually fast first movement.

## Conclusion

Proportional feedback behaves like a virtual spring: position error creates restoring torque. Inertia converts that torque into acceleration, and existing velocity persists through zero error. Increasing Kp can make the response faster while increasing the tendency to swing.

## Reason for the next lesson

The shaft needs a torque that opposes velocity itself. [Measured-motion damping](../33-derivative-motion-damping/README.md) adds velocity feedback, derives its effect on the rotational balance, and explains why differentiating the target would create a software kick.
