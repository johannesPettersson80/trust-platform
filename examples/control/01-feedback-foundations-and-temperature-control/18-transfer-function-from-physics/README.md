# 44. Laplace transforms

You will use a Laplace transform to express the heater energy balance in a form that can be combined with feedback. You will calculate how proportional control changes response speed and steady-state temperature error.

## The problem

We have connected plant and controller equations by repeated substitution and differentiation. A common algebraic representation will make those connections easier to calculate and inspect. Return first to the heater, whose physical response is already familiar.

The [heater energy balance](../15-water-heater-energy-balance/README.md) supplied the physics. Here the question is narrower: with the same 35 °C operating point, how does proportional feedback change the temperature response to a 1 °C setpoint step, and why does a fixed 75% holding bias still leave error?

## What we need

We need a common way to express and connect plant and controller dynamics
without repeatedly solving each differential equation from scratch. It must
preserve initial-state effects and let us recover a physical response.

## The new idea

A **Laplace transform** converts a time-dependent signal into a representation
where differential equations can be handled algebraically. We use it to connect
plant and controller equations and calculate their response without solving
each connected differential equation from scratch. We will transform the
familiar heater balance, then connect the result back to temperature versus time.

Use hot SP=PV=35 °C, D=0, applied output 75%, $I=75\%$ held constant, $\beta=1$, and filter bypassed. Define deviations $x=T-35$ °C, $v=u-75$ percentage points, and $r=SP-35$ °C. Temporarily omit the 1 s delivered-power delay for the transform derivation, then restore it in the complete model.

The deviation balance is

$$
5\dot x+x=0.2v,
$$

from $50\dot x=2v-10x$. Define the one-sided transform

$$
X(s)=\int_0^\infty x(t)e^{-st}\,dt,
$$

and similarly $V(s)$ and $R(s)$. Begin with s as a positive real weight large enough for the integral to converge; st is dimensionless, so s has units s⁻¹. The next lesson extends s to complex values to describe oscillation. Constants and sums pass through this integral, which is why the differential equation becomes an algebraic equation. Integration by parts gives

$$
\mathcal L\{\dot x\}=[xe^{-st}]_0^\infty+sX(s)=-x(0)+sX(s),
$$

when the weighted boundary at infinity vanishes. Therefore

$$
5(sX-x(0))+X=0.2V.
$$

A **transfer function** is the ratio of output transform to input transform
when initial state deviations are zero. It describes that input-output
relationship. For $x(0)=0$, the heater transfer function is

$$
G(s)=\frac{X}{V}=\frac{0.2}{5s+1}.
$$

For $x(0)\ne0$, $X=(0.2V+5x(0))/(5s+1)$; the initial stored thermal deviation contributes a separate response even when the input does not change. If $v=0$ and $x(0)=1$ °C, temperature decays immediately even though no command changes, because heat loss acts on the stored deviation.

With P feedback and fixed I/bias, $v=K_p(r-x)$. Substitution gives

$$
X=G K_p(R-X),\qquad (1+GK_p)X=GK_pR,
$$

so

$$
H(s)=\frac{X}{R}=\frac{0.2K_p}{5s+1+0.2K_p}.
$$

The closed-loop steady gain is $K_{cl}=0.2K_p/(1+0.2K_p)$, and the time constant is $\tau_{cl}=5/(1+0.2K_p)$. For $K_p=2$, $K_{cl}=2/7=0.285714$ and $\tau_{cl}=25/7=3.57143$ s. For $K_p=5$, $K_{cl}=0.5$ and $\tau_{cl}=2.5$ s.

For a 1 °C step, the no-delay prediction is $x(t)=K_{cl}(1-e^{-t/\tau_{cl}})$. Thus final temperatures are 35.285714 °C and 35.5 °C, not 36 °C. P leaves errors 0.714286 °C and 0.5 °C because the 75% holding bias is fixed and no integral correction is allowed. One time constant reaches 63.2% of the final temperature change, not 63.2% of the 1 °C setpoint step.

For a delay L and zero input-deviation history before time zero, the delayed signal is zero until L. Change integration variable to ξ=t−L:

$$
\int_L^\infty v(t-L)e^{-st}\,dt
=e^{-sL}\int_0^\infty v(\xi)e^{-s\xi}\,d\xi=e^{-sL}V(s).
$$

Thus $G_d(s)=0.2e^{-sL}/(5s+1)$, with L=1 s. Repeating the feedback algebra gives denominator $5s+1+0.2K_pe^{-sL}$. Delay changes the complete feedback transient and can affect stability; it is not merely a horizontal shift of the no-delay curve.

Use the same transform rule on the familiar shaft equation $\ddot\theta+\dot\theta+4\theta=4r$. Applying the derivative rule twice gives $\mathcal L\{\ddot\theta\}=s^2\Theta-s\theta(0)-\dot\theta(0)$. At zero initial angle and velocity,

$$
(s^2+s+4)\Theta=4R,\qquad
\frac{\Theta}{R}=\frac{4}{s^2+s+4}.
$$

Here $\Theta$ and R are transforms of angle and its setpoint. The position-control experiment showed a decaying oscillation. The denominator is now quadratic, so the first-order heater shortcut does not directly give its oscillation period and decay rate.

A second algebra exercise uses the same heater with PI gains $K_p=2.5$, $K_i=.5$, an initial 75% integral contribution and zero state deviations. Temporarily omit delivery delay for this calculation only. If $i=I-75$, transforming $\dot i=K_i e$ gives $s\mathcal I=K_iE$, so the controller's command-deviation transfer is $C(s)=2.5+.5/s$. With $G(s)=.2/(5s+1)$, the same feedback substitution yields

$$
\frac{X}{R}=\frac{CG}{1+CG}
=\frac{.5(s+.2)}{5(s+.1)(s+.2)}
=\frac{.1}{s+.1}.
$$

For a disturbance, let d be extra heating expressed in command-equivalent percentage points: a 30 kW extraction means d=−15 points. Write its transform as $U_d$. At zero setpoint deviation, $X=G[-CX+U_d]$, hence

$$
\frac{X}{U_d}=\frac{G}{1+CG}
=\frac{.2s}{(5s+1)(s+.1)}.
$$

Both expressions describe the same heater and PI settings, but the factor $(s+.2)$ cancels from the setpoint expression and remains in the disturbance expression. The algebra has not removed stored water energy. What do those numerator and denominator factors tell us about the response? Keep that question and the shaft's oscillation question for the next lesson. These are model calculations; the live P experiment below retains its fixed integral and physical 1 s delay.

## Prediction

Calculate $K_{cl}$, $\tau_{cl}$, final temperature and remaining error for $K_p=2$ and 5. Predict the 63.2% crossing and explain why the actual delayed trace may differ in both onset and shape. Predict the contribution of $x(0)=1$ °C with zero command deviation. Check the shaft transformation and the two PI feedback expressions algebraically; identify which common factor cancels and which remains.

## Implementation

Write scalar ST calculations for $K_{cl}$, $\tau_{cl}$, predicted final temperature, and predicted 63.2% level. Keep $I=75\%$ fixed, do not update it, and bypass filtering. Write `ClosedLoopGain := 0.2 * KpPctPerC / (1.0 + 0.2 * KpPctPerC);` and `ClosedLoopTimeS := 5.0 / (1.0 + 0.2 * KpPctPerC);`. Calculate final temperature as 35 plus ClosedLoopGain times the setpoint change. Check both gains and keep these prediction channels separate from measured PV.

## Experiment in Control Studio

1. Reset to SP=PV=35 °C, applied power 75%, I=75%, P=0, D=0, filter bypassed, and delayed history 75%. Set time zero at **Confirm and start**.
2. Before each run, declare the 30 s duration, final-temperature tolerance ±0.05 °C around the correct P prediction, comparison of 63.2% crossings, and peak/output limits.
3. In **Control Studio → Test**, review and start a 35→36 °C step with $K_p=2$. Display SP, PV, error, P, fixed I, requested/applied output, delayed command and elapsed time.
4. Fully reset the same hot state and repeat with $K_p=5$. Do not compare runs with different delay history or initial conditions.
5. After the P records, inspect the separate shaft and PI algebra calculations above. Retain their expressions beside the familiar smooth heater response and oscillating shaft response; do not label the no-delay PI calculation as a recorded P trial.
6. Stop on PV outside 0–80 °C, applied output outside 0–100%, invalid data/clock, or unresponsive signals. Command zero heat and restart before repeating.

## Evidence and decision

Compare each final PV with 35.285714 °C or 35.5 °C, using the declared ±0.05 °C tolerance. Compare 63.2% crossings to each no-delay prediction, but do not claim the setpoint was fully reached. Record delay, peak, output limits and the shape difference caused by $e^{-sL}$. A delayed trace is not established by a simple horizontal shift if feedback acts during the delay.

## Conclusion

Laplace algebra carries the thermal initial condition into a plant transfer function. P feedback shortens the modeled time constant but, with fixed 75% bias and no integral update, leaves a predictable steady error. Restored transport delay changes the feedback dynamics themselves.

## Reason for the next lesson

For the simple heater expression we could read one time constant directly. The shaft expression $4/(s^2+s+4)$ describes a decaying oscillation, but how do we read both its period and decay rate? For the PI heater, why does $(s+.2)$ disappear from the setpoint expression while remaining in the disturbance expression?

[Poles and zeros](../../02-derivative-action-and-dynamic-response/19-poles-zeros-and-response/README.md) answers these questions by finding and interpreting the numerator and denominator roots of the expressions we have just derived.
