# 71. State observers

You will estimate the hidden upstream level deviation from tank 2, then assess whether the estimate is accurate enough to support state feedback.

## The problem

The controller needs both levels, but the proposed installation measures only
tank 2. A model can predict the hidden upstream level; disagreement between
predicted and measured downstream level can then correct that estimate.

## What we need

We need a usable estimate of the upstream level when only downstream level
is measured. It must use the model and measurement history rather than rely
on direct differentiation of noisy readings, and its error must be assessed.

## The new idea

A **state observer** estimates internal states using a dynamic model and
corrections from available measurements. It carries an estimate forward over
time instead of reconstructing it only from a noisy measurement derivative.

Use A=[[-.04,0],[.04,-.04]] s−1, B=[2,0]T m−2, C=[0,1], and dt=1 s. State deviations are metres and pump deviation is m³/s. Physical levels remain .05–.45 m and flow 0–.020 m³/s. Start the true state at x=[.02,0], observer state at xhat=[0,0], and hold q=.010 m³/s so v=0.

The true state is x; the observer estimate is xhat; measurement y=Cx is tank-2 deviation. Their error is e=x−xhat. Predict the estimate from the model, then add measured-minus-predicted output. This difference is called the **innovation**:

$$
\dot{\hat x}=A\hat x+Bv+L(y-C\hat x).
$$

The innovation is metres and L has units s−1, so the correction is metres/s. Subtracting the observer from the true equation gives e'=(A−LC)e. This error equation assumes the same linear model describes plant and observer. Its poles determine how estimation error decays; the nonlinear tank comparison also includes local-model error.

For L=[l1,l2]T:

$$
A-LC=\begin{bmatrix}-.04&-l_1\\.04&-.04-l_2\end{bmatrix}.
$$

Its characteristic polynomial is:

$$
s^2+(.08+l_2)s+(.0016+.04l_2+.04l_1).
$$

Choose poles −.16 and −.20 s⁻¹, faster than the controller’s −.08 and −.12 s⁻¹ modes, whose polynomial is s²+.36s+.032. Matching gives l2=.28 s−1 and .0016+.04(.28)+.04l1=.032, so l1=.48 s−1. Thus L=[.48,.28]T s−1.

Use the old state in the sampled update: first calculate predicted derivative from xhat and v, then innovation from the current measurement and $C\hat x_k$ at the same timestamp, then update xhat with dt=1. A nonzero innovation is essential: on the first step with x=[.02,0],xhat=0,y=0, innovation is zero because tank 2 has not yet moved; after the state propagates, the downstream measurement becomes nonzero and corrects both estimate entries.

## Prediction

Calculate A−LC and its polynomial. For the local one-second Euler model, the first innovation is zero and the next is .0008 m: the initial estimate remains zero for the first interval while tank 2 rises. Predict the first two innovations for true x=[.02,0], xhat=0, v=0. Explain why observer confidence can be low when xhat lies outside physical bounds; never clamp truth or hide that estimate condition.

## Implementation

Open src/Main.st. Declare StateHat : ARRAY[1..2] OF REAL with index 1 upstream and 2 downstream, plus L, A, B, C, innovation, measured h2, and dt. Write the two derivatives `Rate1 := -0.04 * StateHat[1] + 2.0 * PumpDeviation + 0.48 * Innovation;` and `Rate2 := 0.04 * StateHat[1] - 0.04 * StateHat[2] + 0.28 * Innovation;`. Calculate both next estimates from the old estimates, then store them together. Keep measured h2 read-only, reject invalid/stale data, and flag an estimate outside −.20 to +.20 m deviations, corresponding to .05–.45 m absolute levels without clamping the physical plant.

## Experiment in Control Studio

1. Restart the physical tanks at h1=.27 m,h2=.25 m, q=.010, and observer deviations [0,0]. Hold the applied flow for 60 s from **Confirm and start**.
2. Run a clean record first. Require both estimate errors |x1−xhat1| and |x2−xhat2|≤.002 m at every sample from 30–60 s.
3. Reuse the same open-loop physical trajectory and add the deterministic h2 noise sequence [−.005,+.005,+.005,−.005] m repeatedly. Calculate RMSE and peak estimate error; do not assume noise improves or worsens the result without measurement.
4. Stop for physical level/flow bounds, invalid or stale h2, or stopped time. Command zero pump flow and stop the simulator on an abort. At normal completion restore nominal flow, then reset physical and observer state before every repeat.

## Evidence and decision

Report pole matching, clean error windows, noisy RMSE/peak, innovations, estimate-bound flags, and reset evidence. The true physical state is evaluation truth from the start, not an observer input. The observer may be mathematically correct while noisy estimates are temporarily outside the physical range.

## Conclusion

The observer combines a two-tank model with tank-2 innovation to estimate both deviations. Its gains come from the desired error polynomial, and its usefulness is decided by clean and noisy error records.

## Reason for the next lesson

We chose correction gains from desired error-decay rates. Faster correction
can also transmit more sensor noise. [Kalman filtering](../71-kalman-filter/README.md)
calculates correction strength from model and measurement uncertainty instead.
