# 54. Sampled-data stability

You will learn how holding a motor-torque command between controller updates changes shaft motion. You will predict why a longer update interval, combined with computation delay, can make a stable PD design unstable.

## The problem

The preceding Nyquist calculation included heater transport delay but not sampled command holding. Return to the familiar shaft so that we can isolate digital timing: its physical inertia and friction stay unchanged while the controller-update interval changes.

Use $J=1$ kg·m², $B=1$ N·m·s/rad, $K_p=16$ N·m/rad, $K_d=7$ N·m·s/rad, no integral, no filter, measured velocity, and unloaded shaft. Between controller updates the old torque remains applied. The next request is calculated from an older measured angle and velocity, so a longer update interval can reverse the stability conclusion.

## What we need

We need to establish whether the controller remains stable with its actual
sampling, held commands and computation delay. The comparison must separate
those physical timing effects from numerical simulation error.

## The new idea

**Sampled-data stability** concerns a system whose physical process moves
continuously while the controller measures and updates at separate times.
Commands are held between updates, and computation may delay a new command
by another interval. We derive that actual timing model before deciding
whether its response decays.

For held torque $u_k$, the shaft equations are $J\dot\omega=u_k-B\omega$ and $\dot\theta=\omega$. With $x=\theta-r$, define

$$
a=e^{-Bh/J},\quad b=\frac{J}{B}(1-a),\quad c=\frac{h}{B}-\frac{J(1-a)}{B^2},\quad d=\frac{1-a}{B}.
$$

For a constant torque, the velocity approaches $u_k/B$ with time constant $J/B$:

$$
\omega(t_k+t)=u_k/B+(\omega_k-u_k/B)e^{-Bt/J}.
$$

At $t=h$ this gives the next velocity. Integrating this expression from 0 to h gives the angle change $b\omega_k+c u_k$. Thus b has units seconds, c has units rad/(N·m), and d has units (rad/s)/(N·m). Exact held updates are

$$
x_{k+1}=x_k+b\omega_k+c u_k,\qquad \omega_{k+1}=a\omega_k+d u_k.
$$

For J=B=1, $a=e^{-h}$, $b=d=1-e^{-h}$, and $c=h-1+e^{-h}$. The one-update computation delay is

$$
u_{k+1}=-K_p x_k-K_d\omega_k,
$$

while $u_k$ is held through the current interval. Collect x, ω and held torque u in that order as a column; the superscript T turns the written row $[x,\omega,u]$ into that column. Each matrix row below is just the weighted sum already written as a scalar update: the first uses $x+b\omega+cu$, the second $a\omega+du$, and the third $-16x-7\omega$. Its update rows are

$$
\begin{bmatrix}x_{k+1}\\\omega_{k+1}\\u_{k+1}\end{bmatrix}=\begin{bmatrix}1&b&c\\0&a&d\\-16&-7&0\end{bmatrix}\begin{bmatrix}x_k\\\omega_k\\u_k\end{bmatrix}.
$$

Assume a mode $x=Xz^k,\omega=Wz^k,u=Uz^k$. Substitution gives $(z-1)X=bW+cU$, $(z-a)W=dU$ and $zU=-K_pX-K_dW$. Multiply the first equation by $(z-a)$ and use the second to get $(z-1)(z-a)X=[bd+c(z-a)]U$. Substituting both expressions in the third and collecting terms gives

$$
z(z-1)(z-a)+K_d d(z-1)+K_p[c(z-a)+bd]=0.
$$

The multiplier z is dimensionless: $|z|<1$ decays, $|z|>1$ grows, and a negative z alternates sign between samples.

For $h=.02$, the polynomial is

$$
z^3-1.9801986733z^2+1.1219867331z-0.1354516352.
$$

Its signs at 0, .2, .8, .9 and 1 are respectively negative, positive, positive, negative and positive, establishing three disjoint real-root intervals ((0,.2),(.8,.9),(.9,1)); all roots lie inside the unit circle. For $h=.6$, the polynomial is

$$
z^3-1.5488116361z^2+6.0881163609z-1.2078964313.
$$

For a monic cubic $(z-z_1)(z-z_2)(z-z_3)$, the constant term is $-z_1z_2z_3$. The product of this polynomial’s roots is therefore 1.2078964313, greater than one, so at least one root has magnitude greater than one. This is sampled-loop instability from held control and computation delay, not an Euler artifact.

Forward Euler’s scalar repeated-pole check $z_E=1-4h$ gives .92 at .02 s and −1.4 at .6 s. Keep it as a numerical-method comparison only; it does not replace the exact held dynamics above.

## Prediction

Calculate a, b, c and d for both h values. Use the polynomial signs and root-product argument to predict decay at .02 s and growth at .6 s. Explain why changing the plant integration error at the same time would prevent a fair timing comparison.

## Implementation

Write scalar old-state/next-state calculations for x, velocity, held torque and pending torque. At time $t_k$, retain the measured angle and velocity for the pending calculation. Apply the already available $u_k$ during $[t_k,t_{k+1})$. Compute the next plant state from that old state and held torque, and compute $u_{k+1}$ from the measurement at $t_k$, not from the newly calculated plant state. Store all next values together. This ordering creates the declared full-update delay; using the new measurement would describe a different controller. Keep requested, pending and applied torque separate. Do not introduce a mode framework or a coarse Euler plant.

## Experiment in Control Studio

Compare $h=0.02$ s and $h=0.6$ s with the same plant and one pending command. Use the exact held-command solution above, or retain the same sufficiently fine plant integration interval in both tests. Only controller sampling and torque holding change with h. Configure the expected period and timing guard together: use a maximum accepted gap of 1.25h for this experiment. A deliberate .6 s period must not be mistaken for an unexpected missed .02 s update. Use a target step from 0 to 0.01 rad at time zero, a 10 s maximum run, angle bounds ±0.2 rad, speed bounds ±2 rad/s, and applied torque bounds ±20 N·m. Torque clipping is bounded operation, not an out-of-range abort, but it ends the unsaturated linear-stability claim.

1. Reset angle=0, velocity=0, pending=0, applied=0, target=0 and unloaded shaft. At **Confirm and start**, apply target 0.01 rad at time zero.
2. Run h=.02 s and h=.6 s as separate fully reset 10 s cases. Keep fine physical integration, gains, target, limits and initial state identical.
3. Display angle, error, velocity, pending/applied torque, sample interval and clipping. Record successive error peaks and the first torque clip.
4. Stop on angle outside ±0.2 rad, speed outside ±2 rad/s, invalid measurement/clock, or unresponsive execution. If torque reaches ±20 N·m, mark the linear claim ended but retain bounded operation evidence.
5. On abort or completion, disable automatic output and clear pending torque before commanding zero, so a queued request cannot be applied after the stop. The unloaded shaft coasts and must be reset before repeating.

## Evidence and decision

Use the exact polynomial and measured unsaturated peaks together. The .02 s case has all roots inside the unit circle; the .6 s case has a root magnitude above one. Euler’s −1.4 warning alone is insufficient, while agreement with the exact delayed update supports a physical sampled-data explanation.

## Conclusion

Sampled-data stability depends on the held command and one-update computation delay. Exact held dynamics predict decay at .02 s and an unstable mode at .6 s for this PD shaft, while coarse Euler integration is a separate numerical question.

## Reason for the next lesson

Correct timing prevents an implementation from changing the dynamics unintentionally. It does not remove a limitation of ordinary feedback: an unmeasured heat-load change must first alter temperature before the controller can respond. If that load can be measured, its heating requirement could be calculated immediately.

[Feedforward control](../../09-industrial-control-structures/51-feedforward/README.md) returns to the familiar heater and derives that disturbance-based command.
