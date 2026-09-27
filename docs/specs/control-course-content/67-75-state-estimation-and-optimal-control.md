# Lesson explanations: 67–75

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 67. State-space models

A **state-space model** describes how internal quantities change with inputs
and how measurements depend on those quantities.

A **state** contains the information needed, together with future input,
to predict future behavior under the model.

For two cascading tanks, both levels are states. Knowing only downstream
level does not tell us how much water is still stored upstream.

From the volume balances, the local equations are:

$$
\dot x_1=-0.04x_1+2v,\qquad
\dot x_2=0.04x_1-0.04x_2.
$$

$x_1,x_2$ are level deviations in metres from 0.25 m, and $v$ is pump-flow
deviation in m³/s from 0.010 m³/s.

A matrix is a compact way to write these same scalar equations:

$$
\dot x=Ax+Bv,\qquad y=Cx,
$$

$$
A=\begin{bmatrix}-.04&0\\.04&-.04\end{bmatrix},
\quad B=\begin{bmatrix}2\\0\end{bmatrix},
\quad C=\begin{bmatrix}0&1\end{bmatrix}.
$$

$x$ is the column of two states. Each matrix row forms one weighted sum.
$y=x_2$ is the downstream measurement, which is only part of the state.

At $x=[.02,0]^T$ and $v=0$, the level rates are
$[-.0008,.0008]^T$ m/s.

In Control Studio we compare predicted and physical levels after a pump change.
Next we ask what one pump can control and what one sensor can reveal.

## 68. Controllability

**Controllability** is the property that inputs can move a model's state
between arbitrary states over time.

For our local two-tank model, one pump acts directly on tank 1.
Tank 2 responds later through tank 1. We need to know whether these effects
provide independent ways to influence the two state components.

The controllability matrix is:

$$
\mathcal C=[B,\ AB].
$$

$B$ describes direct input influence and $AB$ how that influence propagates
through the dynamics. For the taught model:

$$
B=\begin{bmatrix}2\\0\end{bmatrix},
\qquad AB=\begin{bmatrix}-.08\\.08\end{bmatrix}.
$$

The columns are not multiples of each other. They span two independent
directions, so the two-state model is controllable. The 2×2 determinant
check gives $2(.08)-0(-.08)=.16\ne0$.

This is an unconstrained local-model statement. It does not remove pump
or level limits. It also does not mean one pump can maintain arbitrary
unequal steady levels: the downstream equilibrium still requires equal levels.

In Control Studio we use bounded pump pulses and observe direct upstream
and delayed downstream changes. Next we ask whether downstream measurement
history can distinguish the hidden upstream state.

## 69. Observability

**Observability** is the ability to determine a model's state from its
measured output over time when the input is known.

The downstream sensor measures $y=x_2$. Upstream level influences how that
downstream level changes:

$$
\dot y=0.04x_1-0.04y,
\qquad
x_1=y+25\dot y.
$$

Levels are in metres and $\dot y$ is in m/s. The factor 25 has units of seconds.

This relationship shows why a history of downstream level contains information
about upstream level. For $y=0$ and $\dot y=.0008$ m/s, it gives $x_1=.02$ m.

The corresponding observability matrix is:

$$
\mathcal O=\begin{bmatrix}C\\CA\end{bmatrix}
=\begin{bmatrix}0&1\\.04&-.04\end{bmatrix}.
$$

Its independent rows provide two independent equations for the two states.

But theoretical observability does not guarantee accurate reconstruction
from noisy data. A .010 m noise difference over one second contributes
.25 m through the derivative term.

In Control Studio we compare noiseless and noisy reconstruction, using
upstream truth only to assess error.

We next design the controller with full-state information as a reference,
then introduce an observer to estimate the missing state more practically.

## 70. State feedback

**State feedback** calculates control action from the system's state variables,
rather than from just one output error.

For the two tanks, using both levels lets the pump respond to upstream stored
water before all of it reaches the downstream tank.

The control law is:

$$
v=-Kx=-k_1x_1-k_2x_2.
$$

$v$ is pump-flow correction in m³/s, $x_1,x_2$ are level deviations in metres,
and the gains have units m²/s. Absolute flow is $q=q_0+v$.

Substituting into $\dot x=Ax+Bv$ gives:

$$
\dot x=(A-BK)x.
$$

We choose $K$ to obtain desired roots of
$\det(sI-A+BK)=0$, the connected system's characteristic equation.
This is **pole placement**.

For desired poles −.08 and −.12 s⁻¹, coefficient matching gives
$K=[.06,.04]$ m²/s.
At $x=[.02,0]^T$, correction is −.0012 m³/s, so flow is .0088 m³/s.

In Control Studio we compare this full-state reference with uncorrected
recovery. We check both levels and pump limits.

The installation may measure only downstream level. The next lesson estimates
the missing upstream state.

## 71. State observers

A **state observer** estimates internal states using a dynamic model and
corrections from available measurements.

The model predicts how both tank levels change. The downstream sensor tells
us how that prediction disagrees with the observed output. We use that
disagreement to correct the estimate:

$$
\dot{\hat x}=A\hat x+Bv+L(y-C\hat x).
$$

- $\hat x$: estimated state.
- $A,B,C$: the existing state, input and measurement models.
- $v$: known applied-input deviation.
- $y$: measured downstream deviation.
- $y-C\hat x$: measurement minus predicted output, called the **innovation**.
- $L$: observer gain, determining correction strength.

For the same exact linear plant and observer model, estimation error
$e=x-\hat x$ satisfies:

$$
\dot e=(A-LC)e.
$$

We can therefore choose $L$ to make estimation errors decay.
The taught pole choices give $L=[.48,.28]^T$ s⁻¹.

Stronger correction may remove initial estimation error faster but also
transmit more sensor noise.

In Control Studio we start with an incorrect estimate and observe convergence,
then repeat with noise. The next lesson calculates correction strength
from uncertainty instead of choosing it only through desired observer rates.

## 72. Kalman filtering

A **Kalman filter is a recursive state-estimation algorithm**. It predicts
a system's state and the uncertainty of that prediction, then corrects both
using new measurements.

We need it because a model can be imperfect and a sensor can be noisy.
The filter calculates how strongly to use the measurement from their
estimated uncertainties.

For one directly measured quantity:

$$
\hat x^+=\hat x^-+K(z-\hat x^-),
\qquad
K=\frac{P^-}{P^-+R}.
$$

- $\hat x^-$: predicted value before the measurement.
- $z$: measured value.
- $z-\hat x^-$: innovation, or disagreement.
- $P^-$: prediction-error variance.
- $R$: measurement-error variance.
- $K$: Kalman gain.
- $\hat x^+$: corrected estimate.

Variance is the mean squared departure of an error from its mean.
For a level, it has units of level squared.
For independent unbiased errors, corrected variance is
$(1-K)^2P^-+K^2R$. Differentiating and minimizing it gives the gain above.

With prediction 10 cm, measurement 12 cm, $P^-=4$ cm² and $R=1$ cm²,
$K=.8$ and the estimate becomes $10+.8(12-10)=11.6$ cm.

For the two-tank state, the complete update is:

$$
\hat x^-=A_d\hat x^+_{\mathrm{previous}}+B_dv_{\mathrm{previous}},
\qquad P^-=A_dP^+_{\mathrm{previous}}A_d^T+Q,
$$
$$
K=P^-C^T(CP^-C^T+R)^{-1},
\qquad
\hat x^+=\hat x^-+K(y-C\hat x^-),
\qquad P^+=(I-KC)P^-.
$$

$A_d,B_d,C$ are the sampled state, input and measurement models.
$P$ is error covariance: its diagonal contains state-error variances and
its other entries describe how those errors vary together.
$Q$ is process-noise covariance; $R$ measurement-noise covariance.
$T$ means transpose and $I$ is the identity matrix.

Under the standard correct linear-Gaussian assumptions, this gives the
minimum mean-square-error state estimate.

In Control Studio we compare predicted and corrected tank levels with known
simulation truth, then change the assumed sensor uncertainty.
The truth evaluates the filter; it is not an input to it.
A smoother estimate is not automatically a more accurate one.

## 73. LQR control

A **linear-quadratic regulator**, or LQR, calculates feedback that minimizes
a specified cost for a linear model.

We use it to express the tradeoff between correcting state errors and using
actuator effort. A faster tank recovery may require larger pump corrections;
a gentler command may leave errors longer.

The cost over a chosen horizon is:

$$
J=\sum_{k=0}^{N-1}(x_k^TQx_k+v_k^TRv_k)+x_N^TP_fx_N.
$$

- $x_k$: state deviation at update $k$.
- $v_k$: input correction.
- $Q$: weights for state errors.
- $R$: weights for input effort.
- $P_f$: terminal-state weight.
- $N$: number of predicted updates.

The superscript $T$ turns a column into a row, making each quadratic expression
a weighted sum of squared quantities.

For the tanks, dividing level error by .05 m gives weight
$1/.05^2=400$ m⁻². Dividing pump deviation by .005 m³/s gives
$1/.005^2=40000$ s²/m⁶. These are design scales, not sensor uncertainties.

Minimizing the cost gives a feedback law $v=-Kx$.
For one prediction step, differentiating its quadratic cost gives:

$$
K=(R+B_d^TQB_d)^{-1}B_d^TQA_d.
$$

$A_d,B_d$ are the sampled plant model. A longer horizon also accounts for
the later downstream effect of today's pump command.

In Control Studio we compare the taught 20-step designs with different effort
weights, inspecting level recovery and actual pump corrections separately.
Next we supply the states through a Kalman estimate.

## 74. LQG control

**Linear-quadratic-Gaussian control**, or LQG, combines LQR state feedback
with a Kalman state estimator.

LQR tells us how to use the state to choose control action.
The Kalman filter supplies an estimate when all states are not directly measured.

The combined command is:

$$
v_k=-K\hat x_k^+.
$$

$K$ is the LQR feedback gain and $\hat x_k^+$ is the current corrected
Kalman state estimate. For the tanks, absolute pump flow is
$q_k=.010+v_k$ m³/s.

Each update first predicts from the previous applied input, then corrects
with the new downstream measurement, then calculates the next pump request.
Using a requested input that was not actually applied would give the model
the wrong physical history.

“Linear” describes the model, “quadratic” the control cost, and “Gaussian”
the noise assumptions. Gaussian noise has a bell-shaped distribution.
Under the appropriate unconstrained assumptions, the estimator and feedback
designs can be calculated separately and combined.

In Control Studio we compare full-state LQR with estimated-state feedback
under the same noise and reset. We assess level and pump performance;
bounded actuators and nonlinear tanks do not automatically inherit the
ideal optimality result.

Next we explicitly plan around future resource constraints.

## 75. Model predictive control

**Model predictive control**, or MPC, predicts future behavior for possible
input plans, chooses the best feasible plan, applies its first command,
then repeats the calculation using new observations.

We use it when decisions must account for future consequences and constraints.
Here two heaters share a supply, so their independent requests can exceed
available power.

For each heater, a one-second prediction follows its energy balance:

$$
T_{\mathrm{next}}=T+
\frac{2u_{\mathrm{previous}}-10(T-20)-D}{50}.
$$

$T$ is temperature in °C, $u$ command in points, and $D$ extra extraction
in kW. The previous command represents one-second delivery delay;
50 is thermal capacity in kJ/°C.

The teaching controller considers command pairs satisfying:

$$
0\le u_1,u_2\le100,\qquad u_1+u_2\le100.
$$

It predicts three steps, rejects plans violating temperature or power limits,
and compares squared temperature errors plus a penalty for command changes.

At targets 31 and 28 °C, steady commands are 55% and 40%, totaling 95%.
An extra 30 kW load on heater 2 requires another 15 points, making those
targets infeasible with the shared supply.

In Control Studio we inspect candidate predictions, rejected plans,
selected commands and actual temperatures. This example searches a finite
command grid; it does not claim an unrestricted global optimum.

Next we ask what stability can be established beyond a finite set of successful runs.
