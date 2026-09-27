# 46. Characteristic equations

You will derive the characteristic equation of a complete feedback loop, including its retained controller state. You will use it to explain the heater mode hidden by setpoint-transfer cancellation and how adding integral action changes the shaft’s dynamics.

## The problem

The preceding lesson found only one pole in the reduced PI-heater setpoint transfer, although a disturbance still exposed two dynamic factors. Water temperature and the controller's integral are both stored quantities. To predict a response to an initial-state change, we must account for both rather than infer the whole system from one simplified input-output expression.

The shaft has the same need: feedback signs and a retained integral contribution change the equations governing its motion. We need to derive those complete equations before choosing gains.

## What we need

We need the natural dynamics of the complete connected controller and plant,
including retained controller states. A cancellation in one input-output
expression must not hide a mode relevant to another input or initial condition.

## The new idea

A **characteristic equation** determines the natural modes of a complete
linear dynamic system. Its roots tell us which exponential rates can occur
without a continuing external input. We derive it from the connected plant
and controller states, rather than infer all internal behavior from one
input-output transfer that may contain a cancellation.

Use the no-delay heater calculation from the preceding lesson first, then the same shaft with inertia $J=1$ kg·m², physical damping $B=1$ N·m·s/rad, proportional gain $K_p$, derivative gain $K_d$, torque limit ±20 and controller update $dt=.02$ s. Derive the closed-loop equation, insert a trial exponential, interpret real and complex roots, and test one known candidate. The continuous equation is a model prediction; sampling, saturation and startup limits remain observable implementation effects.

For the heater, use deviations $x=T-35$ in °C and $i=I-75$ in command points. Keep setpoint at 35 °C, $K_p=2.5$, $K_i=.5$, filtering bypassed and no disturbance. Omit delivery delay for this model calculation only. The physical balance and controller are

$$
5\dot x+x=.2v,\qquad v=-2.5x+i,\qquad\dot i=-.5x.
$$

Substitution gives two coupled state rates:

$$
\dot x=-.3x+.04i,\qquad\dot i=-.5x.
$$

Differentiate the first and use the second to eliminate i:

$$
\ddot x=-.3\dot x+.04\dot i
=-.3\dot x-.02x.
$$

Trying $x=Ae^{st}$ gives the **characteristic equation**, whose roots describe the connected system's natural modes:

$$
s^2+.3s+.02=(s+.1)(s+.2)=0.
$$

Both −.1 and −.2 s⁻¹ remain in the complete dynamics. The setpoint-transfer cancellation removed a factor from that particular zero-initial-state response, not from these state equations.

For example, raise the initial water temperature by .1 °C while keeping the integral at 75%, so x(0)=.1 and i(0)=0. The initial rate is −.03 °C/s. Write $x=a e^{-.1t}+b e^{-.2t}$; the initial conditions give $a+b=.1$ and $-.1a-.2b=-.03$, hence

$$
x(t)=-.1e^{-.1t}+.2e^{-.2t}\ \mathrm{°C}.
$$

This prediction contains both modes. At 5 s it is approximately .012923 °C; at 10 s it is −.009721 °C. The temporary negative deviation occurs because the earlier above-setpoint temperature made control error negative and reduced the integral. The no-delay approximation is essential here: this formula is not the original heater's one-second-delayed physical trace.

Now apply the same reasoning to the shaft. Let $\theta$ be shaft angle in radians and $r$ the target. A local linear shaft model with applied torque $u$ is

$$
J\ddot\theta+B\dot\theta=u.
$$

For feedback about target $r$, use $u=K_p(r-\theta)-K_d\dot\theta$. Substitution gives

$$
J\ddot\theta+(B+K_d)\dot\theta+K_p\theta=K_pr.
$$

For a fixed setpoint r, define z=θ−r. Its derivatives equal those of θ, so subtracting the equilibrium gives $J\ddot z+(B+K_d)\dot z+K_pz=0$. Try $z=Ae^{st}$, where $s$ has units s⁻¹. Its derivatives are $sAe^{st}$ and $s^2Ae^{st}$. Dividing by the nonzero trial amplitude gives the characteristic equation

$$
Js^2+(B+K_d)s+K_p=0.
$$

For $J=1,B=1,K_p=16,K_d=7$, this becomes $s^2+8s+16=0=(s+4)^2$, so both roots are $-4\ \mathrm{s^{-1}}$. The continuous model predicts a repeated decaying rate. Repeated roots can produce a term proportional to $t e^{-4t}$, which still decays; it is not an undamped oscillation.

For a general quadratic, roots may be real or a complex conjugate pair. Negative real parts mean the exponential envelope decays. A nonzero imaginary part means position swings with angular frequency given by its magnitude. If coefficients or signs are changed, the root interpretation changes with them. The derivation does not assume that a digital saturated implementation has the same roots.

The PID holding controller adds another retained quantity. With constant setpoint, no external load and integral torque w, the equations are

$$
J\ddot z+(B+K_d)\dot z+K_pz=w,\qquad\dot w=-K_i z.
$$

Differentiate the first equation and substitute the second:

$$
J\dddot z+(B+K_d)\ddot z+K_p\dot z+K_i z=0.
$$

Its characteristic equation is therefore

$$
Js^3+(B+K_d)s^2+K_p s+K_i=0.
$$

For J=1, B=1, Kp=16 and Kd=7, this is $s^3+8s^2+16s+K_i=0$. The stored integral has added a third mode. The P/PD experiments below disable integral action and use the earlier quadratic; the cubic belongs to the controller with its integral state retained.

## Prediction

For the heater, calculate the initial temperature and integral rates at x=.1 and i=0, verify the two factors, and check the 5 s and 10 s values. Explain why its reduced setpoint transfer alone cannot supply this initial-state response. For the shaft, explain why adding w changes the degree from two to three.

Before running, calculate the characteristic coefficients for $K_p=4,K_d=7$ and $K_p=16,K_d=7$. Confirm that the latter has roots $-4,-4\ \mathrm{s^{-1}}$. Predict the direction of initial torque for a positive position error, the qualitative decay of the known candidate, and why a ±20 torque limit can invalidate the continuous prediction.

Also predict what a sampled trace can reveal: it can show shrinking peaks and settling, but it cannot by itself prove the exact continuous roots. If the initial request exceeds the torque limit, the linear transient prediction is incomplete. That alone does not make the final position infeasible: the unloaded shaft needs zero holding torque at rest.

## Implementation

For the heater model-only check, write the two scalar rates from the same old state:

```st
TemperatureRateCPerS := -0.3 * TemperatureDeviationC
    + 0.04 * IntegralDeviationPct;
IntegralRatePctPerS := -0.5 * TemperatureDeviationC;
```

At the prescribed .1 °C and zero integral-deviation state, these rates are −.03 °C/s and −.05 point/s. If you advance this model, compute both next states before storing either. These calculation channels do not command the heater.

In the shaft project, calculate the characteristic coefficients from the selected $K_p,K_d,J,B$ and expose them for inspection. Do not implement a transform or general root solver. A compact calculation is sufficient:

```st
DampingRatePerS := (FrictionNmSPerRad + KdNmSPerRad) / InertiaKgM2;
StiffnessRatePerS2 := KpNmPerRad / InertiaKgM2;
```

The first coefficient has units s⁻¹ and the second s⁻². For the PID cubic normalized by J, also calculate Ki/J, whose units are s⁻³. Keep it associated with the retained integral-torque state. For Kp=4 and Kd=7, the roots are −4±√12, approximately −0.53590 and −7.46410 s⁻¹. For Kp=16 they coincide at −4 s⁻¹.

The model update should continue to calculate position, velocity and applied torque at $dt=.02$ s. Keep requested and applied torque separate, report saturation, and retain the initial state. A coefficient display is a prediction aid, not a replacement for the running model.

## Experiment in Control Studio

Before enabling the shaft, use a separate model-only calculation for the heater: initialize x=.1 °C and i=0, omit delay in both calculations, and compare the analytic prediction above with the two scalar state updates over 30 s. At .1 s model increments, require their temperature predictions to agree within .001 °C. This checks the derived initial-state response; it is not a live delayed-heater test.

1. Start **Simulator** with angle 0 rad, angular velocity 0 rad/s, setpoint 0 rad, zero applied torque and no load. Disable I and bypass derivative filtering; use the supplied velocity measurement to match the continuous-model question.
2. In **Control Studio → Monitor → Response**, display angle/error in rad, velocity in rad/s, requested/applied torque in N·m, Kp/Kd, and both calculated characteristic coefficients. Use J=1 kg·m², B=1 N·m·s/rad and the 0.02 s update.
3. Predict both root pairs before running. With Kd=7 fixed, run a setpoint step 0→1 rad for 8 s at Kp=4, then fully reset and repeat at Kp=16. **Test → Confirm and start** establishes each event's time zero.
4. Record whether the response decays, whether it oscillates and its time to enter and remain within ±0.02 rad of the setpoint. Compare this with the real-root predictions; do not look for peak spacing when there are no repeated peaks.
5. Abort for angle outside ±2 rad, velocity outside ±10 rad/s, applied torque outside ±20 N·m or unusable measurements. Disable control and command zero torque, allowing unloaded coasting; stop the simulator if unresponsive and reset fully before repeating.

## Evidence and decision

For the heater, verify the two initial rates, roots −.1 and −.2, and the two-mode initial-state prediction. The retained integral changes the subsequent temperature even when the setpoint does not move. For the PID shaft, identify the integral-torque state that produces the cubic term; do not silently apply its cubic to an I-disabled P/PD record.

For $K_p=16,K_d=7$, the algebra checks as $(s+4)^2$. Compare the measured response with a decaying repeated-root prediction while torque is unsaturated. For $K_p=4,K_d=7$, calculate roots from $s^2+8s+4=0$ and predict a slower pair of real decays; report the arithmetic and its units.

If requested torque clips at ±20, mark the corresponding interval as outside the linear equation and inspect applied torque separately. A stable continuous root does not guarantee a stable sampled saturated hardware loop, and a clipped transient does not by itself prove instability or an infeasible final angle. Use reset, limits and event timing in the evidence record.

The characteristic equation is formed after feedback signs are substituted. If derivative feedback is accidentally added instead of subtracted, the coefficient of $s$ changes and so does the root prediction. Check the physical sign first: positive velocity should make derivative feedback oppose motion. Also check that $K_p$ has units consistent with torque per radian and that $J$ and $B+K_d$ produce common units.

The repeated-root example shows why “two negative roots” is not a complete response description. The time multiplier $t$ changes transient shape even though the envelope decays. Use the running curve to assess settling and torque demand, then use roots to explain the trend rather than replacing evidence with a root label.

The characteristic polynomial is a compact statement of the feedback signs and physical coefficients. It does not include the actuator's torque limit, sample-and-hold behavior, sensor delay or mode transitions. Those boundaries belong in the experiment and can change which equation is active.

## Conclusion

The characteristic equation comes from the complete connected dynamics. The heater's temperature and integral state retain both modes even when one cancels from its setpoint transfer. The shaft's P/PD dynamics give a quadratic; retaining integral torque gives a cubic. The appropriate equation must be established before its roots can guide a controller decision.

## Reason for the next lesson

We can now derive the relevant mode equation for a chosen controller. Checking isolated gain values does not yet show how the modes move as a gain changes, or which choices can meet a desired decay rate.

[Root locus](../45-root-locus/README.md) follows those pole movements on the familiar shaft while varying one gain and holding the other controller and plant properties fixed.
