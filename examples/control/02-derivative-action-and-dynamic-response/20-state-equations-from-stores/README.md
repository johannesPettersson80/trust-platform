# 67. State-space models

You will learn why predicting two connected tank levels requires two state variables. You will derive each level’s rate of change from inflow and outflow, then write the same physical relationships as a state-space model.

## The problem

Two open tanks cascade: tank 1 feeds tank 2. The upstream tank stores water before it reaches the downstream tank. One level alone therefore cannot describe both stored volumes. Use the [gravity outflow and local approximation](../../01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization/README.md) already established for a single tank.

## What we need

We need a model that keeps track of the water stored in both tanks and
separates internal states, applied input and available measurements.
A steady input-output gain does not describe those transient storage effects.

## The new idea

A **state-space model** describes how internal quantities change with inputs
and how measurements depend on those quantities. A **state** contains the
information needed, together with future input, to predict future behavior
under the model. Here both stored tank levels are needed: the downstream
measurement alone does not tell us how much water is still upstream.

Use tank cross-sectional area $A_t=.5$ m², h0=.25 m, q0=.010 m³/s, and c=.020 m^(5/2)/s. Tank 1 freely discharges into tank 2 without backpressure. Define x1=h1−.25, x2=h2−.25, and v=q−.010. The matrix $A$ below describes state dynamics; it is distinct from tank area $A_t$. Absolute flow is 0–.020 m³/s and level .05–.45 m.

The nonlinear balances are .5 h1'=q−c√h1 and .5 h2'=c√h1−c√h2. At h0, c√.25=.010. The outlet slope is c/(2√.25)=.020 m²/s. Linearizing gives x1'=−.04x1+2v and x2'=.04x1−.04x2.

A **state vector** is an ordered column of quantities needed to predict what happens next. Here $x=[x_1,x_2]^T$ contains both level deviations; the superscript T turns the written row into a column. The two equations can be written $\dot x=Ax+Bv$, where

$$
A=\begin{bmatrix}-.04&0\\.04&-.04\end{bmatrix}\ \mathrm{s^{-1}},\qquad B=\begin{bmatrix}2\\0\end{bmatrix}\ \mathrm{m^{-2}}.
$$

In ST, `State : ARRAY[1..2] OF LREAL;` declares two LREAL values. `State[1]` is the upstream deviation and `State[2]` the downstream deviation. A matrix row is a weighted sum: row 1 uses −.04 State[1]+2v; row 2 uses .04 State[1]−.04 State[2]. For x=[.02,0],v=0, derivatives are [−.0008,.0008] m/s; for v=.001, they are [.0012,.0008]. The one-second Euler illustration uses both old state entries. The measured nonlinear levels are read-only and are not replaced by the local model.

## Prediction

Calculate both nonlinear balances and local derivatives for x=[.02,0], v=0 and .001. Predict which level rises first and why equal steady levels are required.

## Implementation

Open `src/Main.st`. `State` and `NextState` hold the separate local prediction; `Tank1Level` and `Tank2Level` remain read-only simulator outputs. The two derivative functions use both old state entries. A one-second forecast is accepted on the next scan only after `TIME()` confirms that interval. The first frame initializes the prediction and does not claim an observed interval.

`FlowDeviationM3PerS` selects 0 or .001 for the first 20 s. The program then restores .010 m³/s for 100 s recovery. Invalid bounds or clock intervals latch a fault and command zero; an early cancellation also latches the zero-flow response. Reset the simulator and runtime before another trial. No measured or model level is clipped to disguise a fault.

## Experiment in Control Studio

1. Use a fresh synchronized run with physical h1=.27 m, h2=.25 m and q=.010 m³/s; the local deviations start at [.02,0]. In Tune, import `provided-model.json` and use **Analyze model** to inspect A, B, C, D and the declared state/input/output order.
2. Run separate full-reset cases with `FlowDeviationM3PerS` set to 0 and then .001. Observe the first 20 s and the following 100 s recovery.
3. Restore q=.010 after each case and record both measured levels, model levels, derivatives, and flow.
4. Stop for h .05–.45 violation, q 0–.020 violation, invalid data, or stopped clock. At normal completion, restore nominal flow and observe 100 s of recovery, four nominal 25 s time constants. For a fair repeat, restart the simulator and explicitly set the stated initial levels; restoring nominal flow does not instantly return levels.

**Current workflow gap.** The non-equilibrium initial state drifts in a running simulator while preparing a test. A restart followed by a later Confirm is not the specified start. Native execution now tests the synchronized first scan; the fresh isolated-run UI must reset both runtime and plant before this lesson can claim the complete hands-on journey. The program rejects an initial level mismatch rather than fabricating a hold.

## Evidence and decision

Before comparing, require matching initial derivative signs, initial magnitudes within 5%, and model level errors within .001 m through the 20 s trial. For example, the nonlinear upstream derivative at .27 m with nominal flow is $(.010-.020\sqrt{.27})/.5=-.00078461$ m/s, close to the local prediction −.0008 m/s. Failure calls for checking the approximation or its calculation. Preserve the distinction between illustrative one-second Euler prediction and physical measurement.

## Conclusion

Storage and the local outlet slope produce a two-state model whose rows retain the physical cascade meaning.

## Reason for the next lesson

The equations describe both levels, but we have only one pump.
[Controllability](../../11-state-space-estimation-and-predictive-control/66-controllability/README.md)
asks whether that input can influence both state components independently
over time, and distinguishes that question from holding arbitrary steady levels.
