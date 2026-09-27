# 62. Multivariable process paths

You will measure how each heater changes both vessel temperatures, derive the four steady paths, and decide whether the coupling evidence is sufficient for pairing.

## The problem

The jacketed-vessel assessment had one primary controlled temperature. This new process requires two vessel temperatures to be controlled independently, while the vessels exchange heat through a conducting wall. Each has its own heater and temperature measurement. We want to determine whether each heater can be treated as affecting only its own vessel. Use the following supplied teaching-model values. Each vessel has $C=50$ kJ/°C, loss $H=10$ kW/°C to 20 °C, and a 200 kW heater, so one command point contributes 2 kW. The wall transfers 8 kW per degree of temperature difference, so its coupling coefficient is $H_c=8$ kW/°C. Positive $u_i$ heats vessel i; no additional transport delay is assumed. The balances are

$$
50\dot T_1=2u_1-10(T_1-20)-8(T_1-T_2),
$$
$$
50\dot T_2=2u_2-10(T_2-20)+8(T_1-T_2).
$$

The coupling terms have opposite signs because heat leaving one vessel enters the other. The second vessel makes interaction observable: changing one heater can affect two measured temperatures.

## What we need

We need to know how each heater affects each vessel temperature, including
the cross effects. Those relationships must be measured and understood
before deciding which actuator should regulate each output.

## The new idea

A **multivariable process** has several inputs and outputs. A **process path**
is one input's effect on one output. Here either heater changes both vessel
temperatures, so we need four paths before choosing independent feedback pairs.

With $x_i=T_i-35$, $v_i=u_i-75$, steady balance gives

$$
18x_1-8x_2=2v_1,\qquad -8x_1+18x_2=2v_2.
$$

For $v_2=0$, $x_2=(4/9)x_1$, so $x_1=(9/65)v_1$ and $x_2=(4/65)v_1$. A +5-point heater step therefore predicts $T_1=35.6923077$ °C and $T_2=35.3076923$ °C. A step of heater 2 swaps the outputs; both changes remain positive. Collect the four gains in a matrix:

$$
\begin{bmatrix}x_1\\x_2\end{bmatrix}=K\begin{bmatrix}v_1\\v_2\end{bmatrix},\qquad K=\begin{bmatrix}9/65&4/65\\4/65&9/65\end{bmatrix}.
$$

A matrix row abbreviates one equation: $x_1=(9/65)v_1+(4/65)v_2$, and similarly for $x_2$. The diagonal entries describe each heater’s own vessel; the other entries describe cross effects. Every entry has units °C per percentage point.

Adding the two dynamic balances cancels internal heat transfer. For the mean temperature deviation $m=(x_1+x_2)/2$, $50\dot m+10m=v_1+v_2$. Its time constant is 5 s. The temperature difference decays faster, with time constant $50/(10+2H_c)=50/26=1.923$ s. A 30 s hold covers six slow time constants; the last 5 s therefore sample near equilibrium.

## Prediction

Calculate all four gains, the +5-point final temperatures, and why the common slow mode justifies 30 s. Predict the cross-temperature sign and the effect of an incomplete final window.

## Implementation

Write the two temperature derivative calculations in ST using old model temperatures. For example, `TransferKw := 8.0 * (ModelTemperature1C - ModelTemperature2C);`. Subtract this term from vessel 1’s balance and add it to vessel 2’s. Calculate both next temperatures before storing either. Expose temperatures, commands, coupling flow and applied values. Keep the local 2×2 gain rows explicit; no matrix library is required. PI remains off during identification.

## Experiment in Control Studio

Start at both $T=35$ °C and commands 75%. Use PI off/manual steps, 0.1 s samples, 30 s per step, and a 5 s final-window average. Restore 75% normally; abort commands zero heat. Declare before testing a ±0.02 °C model-comparison tolerance, T 33–37 °C and command 0–100% bounds.

1. Reset both vessels to 35 °C/75%, PI off, D=0, and stable history.
2. At **Confirm and start**, step heater 1 to 80% for 30 s; retain the final 5 s average. Reset fully, then step heater 2 to 80% for 30 s.
3. Display both temperatures and applied commands. Stop on bounds, invalid data, or unresponsive execution; restore 75%, zero heat on abort, and restart.

## Evidence and decision

Compare the four observed final temperature changes with the predicted changes within ±.02 °C. Divide each change by its applied five-point step to estimate the corresponding gain. Keep applied histories and units. A mismatch is evidence to inspect coefficients or reset, not permission to call the plant identified.

## Conclusion

The two-vessel energy balance produces four positive steady paths because shared transfer conserves heat while changing both outputs.

## Reason for the next lesson

We measured each path while holding the other heater command fixed. Once the
other feedback loop acts to hold its temperature, it changes that command and
therefore changes the gain seen by the first loop.

[Relative gain array](../62-relative-gain-array/README.md) compares those two
steady situations before choosing the controller pairings.
