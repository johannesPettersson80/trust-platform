# 73. LQR control

You will balance two-tank level error against pump effort by calculating one-step feedback and then a finite-horizon LQR gain for the same sampled plant.

## The problem

A pump correction that removes level error quickly may move the pump farther from its nominal flow. A gentler correction uses less actuator effort but may leave level deviations longer. LQR makes this physical tradeoff explicit instead of choosing a gain from an unexplained formula.

## What we need

We need to choose how much pump correction is worthwhile to reduce the
level errors. The decision must express the tradeoff between recovery and
actuator effort and use the model to calculate suitable feedback gains.

## The new idea

A **linear-quadratic regulator**, or **LQR**, calculates feedback that minimizes
a chosen cost for a linear model. The cost expresses the tradeoff between
correcting level errors and using pump effort. Its weights are design priorities,
not the sensor and model uncertainties used by the Kalman filter.

Use deviations x=[x1,x2]T in metres and pump deviation v in m³/s around h=.25 m and q=.010 m³/s. The one-second model is:

Ad=[[.96,0],[.04,.96]], Bd=[2,0]T.

Dividing a level deviation by .05 m and squaring gives coefficient $1/.05^2=400$ m⁻². Doing the same for a pump deviation scale .005 m³/s gives $R=1/.005^2=40000$ s²/m⁶. Thus $Q=400I$, where I has diagonal entries one and off-diagonal entries zero, and each cost contribution is dimensionless. Compare R=160000 as a four-times effort penalty. Keep absolute flow 0–.020 m³/s and levels .05–.45 m.

The next sampled state is xnext=Ad x+Bd v. A one-step cost is:

$$
J(v)=Rv^2+x_{next}^TQx_{next}.
$$

Here J is total cost, v the pump correction, Q the state-error weights and
R the input-effort weight. The transpose makes each state term a weighted
sum of squared deviations. We choose v to make this cost as small as possible.

Transpose turns a column into a row; each matrix product is a row-by-column sum. Differentiating the scalar v gives:

$$
\frac{dJ}{dv}=2Rv+2B_d^TQ(A_dx+B_dv).
$$

Here $R+B_d^TQB_d$ is a positive scalar: R>0 and the squared-state penalty is nonnegative. Its superscript −1 means taking its reciprocal, not inverting an unexplained matrix. Setting the derivative to zero gives v=−K1x, where K1=(R+BdTQBd)^−1BdTQAd. For Q=400I and R=40000, K1=[.01846153846,0] m²/s. For x=[.02,0], Adx=[.0192,.0008], so v=−.000369230769 and q=.009630769 m³/s. The no-input cost is .147712 and the one-step minimum is .1420406154. x2 does not enter this one-step gain because the pump changes x1 immediately; it enters through future steps. Applying the model twice gives $x_{2,k+2}=.0768x_{1,k}+.9216x_{2,k}+.08v_k$. The present pump correction can therefore reduce downstream error two samples later, which gives the longer-horizon design a reason to use both levels.

For a finite horizon of 20 one-second intervals, minimize $J=\sum_{k=0}^{19}(x_k^TQx_k+Rv_k^2)+x_{20}^TQx_{20}$. The final term penalizes level deviations left at the end. Write future cost as $x^TPx$: P tells the next decision how expensive a remaining state will be.

Suppose the best achievable cost after the next state is already known as $x_{next}^TP_{next}x_{next}$. The current decision then minimizes

$$
J_{remaining}(x,v)=x^TQx+Rv^2+(A_dx+B_dv)^TP_{next}(A_dx+B_dv).
$$

The first term charges for the current level deviations, the second for this pump correction, and the last for the remaining consequences of the next state. At the final state there are no further pump decisions, so $P_{20}=Q$. Solve the last decision first, then use its minimized cost to solve the preceding decision. Working backward makes the future cost available before each earlier decision is calculated; it does not run the physical tanks backward.

For the first backward step, use $P_{20}=400I$ and $R=40000$. The products needed by the same scalar minimization are

$$
B_d^TP_{20}B_d=1600,\qquad B_d^TP_{20}A_d=[768,0],
$$
$$
A_d^TP_{20}A_d=\begin{bmatrix}369.28&15.36\\15.36&368.64\end{bmatrix}.
$$

The minimizing gain is therefore $[768,0]/41600$, the one-step gain already calculated. Substituting that minimizing command back into the cost gives

$$
P_{19}=400I+\begin{bmatrix}369.28&15.36\\15.36&368.64\end{bmatrix}
-\frac{1}{41600}\begin{bmatrix}768^2&0\\0&0\end{bmatrix}
=\begin{bmatrix}755.10153846&15.36\\15.36&768.64\end{bmatrix}.
$$

Thus $x^TP_{19}x=755.10153846x_1^2+30.72x_1x_2+768.64x_2^2$: upstream and downstream deviations jointly affect the remaining cost. At the next backward step, the second entry of $B_d^TP_{19}A_d$ is $2(15.36)(.96)=29.4912$, so the gain now uses downstream level too. Its future response matters even though the pump cannot change it in the first interval.

Starting with P20=Q, repeat this calculation using the **Riccati recursion** below. Work backward for k=19 to 0:

$$
K_k=(R+B_d^TP_{k+1}B_d)^{-1}B_d^TP_{k+1}A_d,
$$
$$
P_k=Q+A_d^TP_{k+1}A_d-A_d^TP_{k+1}B_dK_k.
$$

Here $P_k$ is a future-cost matrix, not the estimation-error covariance P
from Kalman filtering. The same letter is used for different physical meanings.

Repeating exactly 20 steps gives the first gain K0=[.07874026,.02387430] m²/s for R=40000 and K0=[.03460802,.00952351] for R=160000. At x=[.02,0], their first pump deviations are −.001574805 and −.000692160 m³/s, so absolute flows are .008425195 and .009307840 m³/s. At each new sample, start a fresh 20-step horizon and apply the same first gain K0. This is a receding finite-horizon design, not a claim that K0 is the infinite-horizon solution. The gains are unconstrained; clipping changes the planned cost.

## Prediction

Calculate the one-step K1 and first flow. Predict which 20-step design uses less pump effort, whether either can meet the late deviation requirement, and why the second state affects K0 even though it does not affect K1.

## Implementation

Open src/Main.st. Store the selected finite-horizon K0 as two REAL gains and calculate v=−K0[1]x1−K0[2]x2. Add q0, expose requested/applied flow, and apply the 0–.020 limit. The runtime applies the supplied gain; it does not implement a hidden optimizer. Keep physical measured levels, deviation state, and design cost signals separate and abort on out-of-bounds levels rather than clamping them.

## Experiment in Control Studio

1. Restart with physical h1=.27 m and h2=.25 m, giving x=[.02,0], q=.010 m³/s, controller disabled, and dt=1 s.
2. At **Confirm and start**, run a 60 s record with R=40000, then fully reset and run R=160000. Use the same plant, clock, reset, and requirements.
3. Require both deviations to satisfy |x1|≤.002 m and |x2|≤.002 m at every sample from 50 through 60 s. Keep q within 0–.020 and levels within .05–.45 m.
4. Display physical levels, deviations, requested/applied q, level cost, effort cost, and clipping. Abort if physical levels or applied flow leave their bounds, data become invalid, or time stops. Command zero pump flow and stop the simulator; record ordinary flow clipping separately.
5. Restore nominal q and both levels, then reset all controller state before any repeat.

## Evidence and decision

Retain Ad, Bd, Q, each R, horizon 20, terminal cost, K0, first flow, cost terms, and late deviation results. Compare level error and pump effort separately. Because R differs between designs, their differently weighted total costs cannot by themselves rank performance. Compare the same unweighted sums of squared level deviations and pump deviations, plus the fixed error requirement. Here effort means deviation from nominal flow; it is not a measured electrical-energy or wear cost. A finite-horizon gain with flow clipping is not an unconstrained optimality proof.

## Conclusion

Sampled LQR assigns a numerical cost to physical level error and pump movement. The one-step derivation explains the sign and units; the 20-step Riccati recursion explains why downstream state appears in the actual gain.

## Reason for the next lesson

The feedback calculation uses both state values. We must still establish
its behavior when those values come from a noisy, imperfect estimate.
[LQG control](../72-lqg/README.md) combines this selected gain with the
Kalman estimator and compares it with the full-state reference.
