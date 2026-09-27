# 70. State feedback

You will use both tank levels to regulate them toward the operating point. You will choose how quickly small level deviations should decay, calculate the corresponding feedback gains, and check the resulting pump demand.

## The problem

One pump changes tank 1 directly and tank 2 through the cascade. A state-feedback law can use both deviations, but using the upstream level lets it respond before that water reaches tank 2. The remaining design question is how strongly each measured level should affect pump flow.

## What we need

We need a rule for choosing pump correction from both level deviations
so their response has the desired local decay. We must calculate the gains
and check the resulting demand against the pump's limits.

## The new idea

**State feedback** calculates control action from the system's state variables
rather than from only one output error. Here both level deviations influence
pump flow, so upstream stored water can affect the decision before it reaches
the downstream tank:

$$
v=-Kx=-k_1x_1-k_2x_2.
$$

$v$ is pump correction in m³/s, $x_1,x_2$ are level deviations in metres,
and $k_1,k_2$ have units m²/s. Absolute flow is $q=.010+v$.
We choose the gains to obtain desired roots of the connected characteristic
equation. This is **pole placement**.

Use x=[x1,x2]T in metres, v in m³/s, A=[[-.04,0],[.04,-.04]] s−1 and B=[2,0]T m−2. The fixed point is h1=h2=.25 m, q=.010 m³/s. Flow is bounded 0–.020 m³/s and levels .05–.45 m.

Without feedback, the local model has decay rates .04/s. Choose desired poles −.08 and −.12 s−1, two and three times that rate, so the desired polynomial is (s+.08)(s+.12)=s²+.20s+.0096. Let v=−k1x1−k2x2. Then:

$$
A-BK=\begin{bmatrix}-.04-2k_1&-2k_2\\.04&-.04\end{bmatrix}.
$$

A mode varies as $e^{st}$, where a negative real s means decay. Substituting that motion into $\dot x=(A-BK)x$ requires $(sI-(A-BK))x=0$. Here I is the identity matrix, with ones on its diagonal and zeros elsewhere. Nonzero modal states require a zero determinant. Expanding the two-by-two determinant gives:

$$
s^2+(.08+2k_1)s+(.0016+.08k_1+.08k_2).
$$

Matching coefficients gives .08+2k1=.20, so k1=.06 m²/s. The constant term then requires .0016+.08(.06)+.08k2=.0096, so k2=.04 m²/s. Thus K=[.06,.04]. For x=[.02,0], v=−.06(.02)=−.0012 m³/s and q=.0088 m³/s. The determinant is .0096 s⁻².

This is local pole placement around the fixed point. Pump clipping changes the intended poles. Equal levels remain the one-pump steady-state condition.

## Prediction

Calculate the coefficient matching, first pump request, and initial derivative for x=[.02,0]. Predict whether both deviations can meet .002 m by 50–60 s without flow clipping.

## Implementation

Open src/Main.st. Declare State : ARRAY[1..2] OF REAL, Gain : ARRAY[1..2] OF REAL, and named scalar flow values. Index 1 is upstream tank deviation and index 2 downstream. Calculate the two-term row product explicitly, add q0, clamp applied flow, and publish requested/applied flow. Keep nonlinear measured levels and local model state separate; do not clamp a level to hide an out-of-bounds condition.

## Experiment in Control Studio

1. Restart with h1=h2=.25 m, q=.010, state zero, controller disabled, and dt=1 s.
2. Apply x1=.02,x2=0 and enable feedback for 60 s from Confirm and start.
3. Require both absolute deviations ≤.002 m from 50–60 s, with q 0–.020 and levels .05–.45. Abort if physical levels or applied flow leave their stated bounds, data become invalid, or time stops. Command zero pump flow and stop the simulator before resetting. Reaching a flow limit is recorded as clipping, separately from an out-of-range output.
4. Restore nominal flow, disable, and fully reset before repeating. Compare measured nonlinear levels with the local model prediction.

## Evidence and decision

Retain the symbolic coefficient match, K values, first command, applied flow, and late error window. A pass supports this local design under its bounds; it is not a global nonlinear claim.

## Conclusion

Matching the two coefficients gives k1=.06 and k2=.04 m²/s and the desired local poles. The state law is useful only while the physical tank and pump remain within the declared local range.

## Reason for the next lesson

This reference controller uses both levels. The proposed installation may
measure only the downstream tank. A [state observer](../70-state-observer/README.md)
estimates the missing upstream level from the model and downstream measurement.
