# 72. Kalman filtering

You will use a Kalman filter to decide how much to trust a two-tank model prediction versus a noisy tank-2 measurement, then compare estimated state error with known simulation truth.

## The problem

The controller may need both tank levels even though only tank 2 is measured. A model predicts how the unmeasured level evolves, while a sensor gives partial and noisy information. The filter combines them from declared uncertainty rather than choosing whichever number looks smoother.

## What we need

We need to decide how strongly a measurement should correct the model's
state prediction when both are uncertain. The estimate must balance their
uncertainties, rather than trust one completely or prefer whichever trace
looks smoother.

## The new idea

A **Kalman filter is a recursive state-estimation algorithm**. It predicts
the state and its uncertainty, then corrects both using a new measurement.
"Recursive" means retaining the latest estimate and uncertainty for the
next update rather than recalculating the entire measurement history.

We need it because the model can be imperfect and the sensor noisy. The
filter calculates how strongly to use the measurement from their relative
uncertainties. It estimates the tank levels; it does not itself choose pump flow.

An error is the difference between an estimate and the unknown true value. Its expected value is the average error over possible repeated outcomes. Variance is the expected squared deviation from that mean, and standard deviation is the square root of variance. Covariance is the expected product of two errors after subtracting each error’s mean. When both means are zero, it is the expected product of the errors themselves. It records whether state errors tend to move together.

For one directly measured quantity, the correction is:

$$
\hat x^+=\hat x^-+K(z-\hat x^-),\qquad K=\frac{P^-}{P^-+R}.
$$

- $\hat x^-$: predicted value before the measurement.
- $z$: measured value; $z-\hat x^-$ is their disagreement, or innovation.
- $P^-$: prediction-error variance; $R$: measurement-error variance.
- $K$: the Kalman gain, determining correction strength.
- $\hat x^+$: corrected estimate.

For independent zero-mean errors, the corrected variance is
$(1-K)^2P^-+K^2R$. Differentiate with respect to K and set the derivative
to zero: $-2(1-K)P^-+2KR=0$, giving the gain above. Independent means
knowing one error does not change the probability of the other.

With prediction .010 m, measurement .012 m, $P^-=4\times10^{-6}\ \mathrm{m^2}$
and $R=25\times10^{-6}\ \mathrm{m^2}$, K=4/29 and
$\hat x^+=.010+(4/29)(.012-.010)=.010275862$ m.
The prediction is more certain in this example, so the estimate moves only
partway toward the sensor reading. Greater prediction uncertainty would
give the measurement more influence.

Use one-second updates and deviation state $x=[x_1,x_2]^T$ in metres:

$$
A_d=\begin{bmatrix}.96&0\\.04&.96\end{bmatrix},\qquad
B_d=\begin{bmatrix}2\\0\end{bmatrix},\qquad
C=\begin{bmatrix}0&1\end{bmatrix}.
$$

The filter needs uncertainty assumptions as well as the physical model.
Use process-error covariance $Q_w=\operatorname{diag}(10^{-6},10^{-6})\ \mathrm{m^2}$
added per one-second prediction, and sensor-error variance
$R_n=25\times10^{-6}\ \mathrm{m^2}$. Using the uncertainty meanings above,
these values are supplied assumptions, not measured plant facts. The nominal
operating point is .25 m with $q=.010\ \mathrm{m^3/s}$; keep levels in
.05–.45 m and flow in 0–.020 $\mathrm{m^3/s}$. True levels are available
for evaluation only.

For two tanks, $\hat x$ holds both level estimates and P their error covariance.
Its diagonal entries are the two error variances; off-diagonal entries describe
how their errors vary together. $A_d,B_d,C$ are the sampled model above,
v is the previous applied pump-flow deviation, $Q_w$ the process uncertainty
added this update, and $R_n$ the downstream sensor variance. The prediction is

$$
\hat x^- = A_d\hat x+B_dv,\qquad
P^-=A_dP A_d^T+Q_w.
$$

The covariance formula follows from prediction error $A_de+w$, where e is
the previous state-estimation error and w the uncertain process increment.
Expanding its expected outer product removes the cross terms when w is
independent of prior error and has zero mean. The outer product of a column
e and row $e^T$ contains every pairwise error product. This propagates both
error sizes and their correlation. Superscripts − and + mean before and
after the current measurement; I is the two-by-two identity matrix.
For prior estimate $\hat x=[.02,0]^T$, $P=10^{-4}I$, $v=0$:

$$
\hat x^-=[.0192,.0008]^T,\qquad
P^-=\begin{bmatrix}.00009316&.00000384\\.00000384&.00009332\end{bmatrix}.
$$

With $y=.0028$ m, predicted measurement $C\hat x^-=.0008$, innovation $\nu=.002$ m, and

$$
S=CP^-C^T+R_n=.00011832\ \mathrm{m^2},
$$

the two-entry Kalman gain is denoted L here, distinct from the state-feedback
gain K used by the controller:

$$
L=P^-C^T/S=[.03245436,.78870859]^T.
$$

The update is $\hat x^+=\hat x^-+L\nu=[.019264909,.002377417]^T$ m. Covariance uses $P^+=(I-LC)P^-$. Since $C=[0,1]$, $LC$ has zero first column and gain entries as its second column; the scalar entries are $P^+_{11}=P^-_{11}-L_1P^-_{21}$, $P^+_{12}=P^-_{12}-L_1P^-_{22}$, $P^+_{21}=(1-L_2)P^-_{21}$ and $P^+_{22}=(1-L_2)P^-_{22}$. Numerically they are $.0000930353753$, $.0000008113590$, $.0000008113590$ and $.0000197177147$ m². Calculate every new entry from the old predicted matrix before storing it. Larger $R_n$ makes $S$ larger and gain smaller, so the estimator trusts the model more.

With a correct linear model, Gaussian initial uncertainty and independent
zero-mean Gaussian process and measurement noise of known covariance, the
Kalman estimate minimizes expected squared state error. Gaussian means a
bell-shaped distribution; the standard noise assumptions also require no
correlation between different updates. Our deterministic noise experiment
demonstrates the calculation and tradeoff, not that optimality result.

## Prediction

Calculate the scalar blend, prediction, covariance, innovation, $S$, gain and updated state above. Predict why covariance is not actual error, why process $Q_w$ is not an effort cost, and why a repeated deterministic noise pattern can test arithmetic without proving Gaussian-optimal performance.

## Implementation

Implement one complete prediction/correction with retained estimate and
covariance. Use scalar ST fields for the four covariance entries, two state
entries, $A_d,B_d,C,Q_w,R_n$, innovation, its variance $S$, and gain.
For example, write `InnovationM := MeasuredLevel2M - 0.25 - PredictedState2M;`
and `UpdatedState1M := PredictedState1M + Gain1 * InnovationM;`, then the
second-state correction. Compute all new covariance entries before overwriting
the predicted matrix. On invalid quality or stale timestamp, freeze state
and stop/recover using Lesson 28's principles with timing limits appropriate
to one-second tank updates; do not copy the heater's shorter gap limit or
clamp levels to hide a bad estimate.

## Experiment in Control Studio

1. Reset physical tanks to h1=.27 m and h2=.25 m with $q=.010$ m³/s, giving true deviation state $[.02,0]^T$. Initialize estimate zero, covariance $10^{-4}I$, and hold absolute pump flow $q=.010$ m³/s, so its deviation $v=0$. Run 60 s from **Confirm and start** at 1 s updates.
2. Add the same deterministic tank-2 measurement noise sequence $[- .005,+.005,+.005,-.005]$ m at every sample to two separate records, one with $R_n=25\times10^{-6}$ and one with $R_n=100\times10^{-6}$. The sequence has zero mean and mean square $25\times10^{-6}$, but is correlated and is not a Gaussian optimality proof.
3. Display measured level, estimate, covariance entries, innovation, gain, held flow and validity. Keep true state visible only as evaluation truth, never as estimator input.
4. For each state error $e_k=x_k-\hat x_k$, calculate $\mathrm{RMSE}=\sqrt{\sum e_k^2/N}$ across the N samples of the chosen window, in metres. Record maximum absolute state error, and metrics over 0–10 s and 30–60 s windows. Compare estimation speed and noise cost.
5. Stop for level outside .05–.45 m, flow outside 0–.020, invalid/stale measurement or stopped clock. Freeze the estimator, command zero pump flow and stop the simulator on an abort; gravity outflow continues until stopped. Restore and fully reset before the other covariance choice.

## Evidence and decision

The arithmetic record must reproduce the stated vector update. The experiment must retain the exact input/noise sequence, reset, clock, $Q_w/R_n$ assumptions and RMSE units. Compare both covariance choices on the same open-loop plant record. Covariance describes expected uncertainty under the assumptions; RMSE uses hidden truth and evaluates actual error. Do not claim the hidden truth was available to the estimator.

## Conclusion

The Kalman update is an uncertainty-weighted compromise between a two-tank model and one noisy level measurement. Definitions of mean, variance, standard deviation and covariance explain the weighting, while the complete covariance arithmetic and matched deterministic records make the estimate auditable.

## Reason for the next lesson

Estimating the levels does not decide how much pump correction is worthwhile.
Fast recovery can require larger pump changes, while gentler commands may
leave errors longer. [LQR control](../69-lqr/README.md) expresses that tradeoff
as a cost and calculates feedback from it. Its weights are design priorities,
not the measurement uncertainty values used by this filter.
