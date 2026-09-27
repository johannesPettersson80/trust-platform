# 53. Nyquist stability

You will use the complete Nyquist curve to assess feedback stability. You will first calculate a heater’s transport-delay limit, then apply the method to the shaped PI candidates from the preceding lesson.

## The problem

Loop shaping changed the heater’s magnitude and phase, but a favorable phase change at one frequency did not establish complete-loop stability. Begin with fixed-bias P control to make the delay limit easy to calculate, then return to the shaped PI loops. The P example teaches the method; it does not by itself qualify those PI candidates.

Use the same heater $K=0.2$ °C/point and $\tau=5$ s, hot PV/SP=35 °C, applied holding command 75%, fixed holding contribution 75% with integral updates off, $\beta=1$, and filtering bypassed. The P gain is $K_p=10$ percentage points/°C. Let $t_d$ denote transport delay, distinct from the loop transfer $L_{loop}(s)$:

$$
L_{loop}(s)=\frac{2e^{-s t_d}}{5s+1}.
$$

The factor 2 is $K_pK=10(0.2)$ and is dimensionless. Compare $t_d=1$ s with 6.1 s; a longer delay can represent slower delivery or a longer transport path, not changed heater capacitance or heat loss.

## What we need

We need to determine whether the proposed controller gives stable feedback
with the plant's dynamics and delivery delay. A favorable phase change at
one frequency is not enough; the decision must account for the complete loop.

## The new idea

The Nyquist stability criterion is a mathematical test for whether a feedback loop is stable. A stable loop makes small disturbances die away; an unstable loop can make them grow into increasing oscillations.

It works by examining gain and phase around the loop:

- Gain: how much the controller and plant amplify or reduce a signal.
- Phase: how far their response lags behind that signal.

Normally, negative feedback corrects a deviation. But enough phase lag can make the correction arrive in the wrong direction and reinforce the deviation instead.

A Nyquist plot shows the loop’s gain and phase as frequency changes. Each frequency produces one point on a complex-plane graph. The point −1 represents gain 1 and phase −180°: the signal returns equally large but reversed. The subtraction in negative feedback then reinforces it.

The criterion counts how the complete curve goes around −1:

$$
\boxed{Z=P+N}
$$

With clockwise encirclements counted positively:

- $P$: number of unstable poles before closing feedback.
- $N$: signed number of encirclements of −1.
- $Z$: number of unstable poles after closing feedback.

For closed-loop stability, we need $Z=0$ and no poles on the stability boundary.

We use it to check whether a proposed controller remains stable with the plant’s dynamics and delay—before trusting that controller on the process.

Nyquist uses the complete path $L_{loop}(j\omega)$ as $\omega$ runs from $-\infty$ to $+\infty$, together with the right-half-plane closure. The negative-frequency branch is the conjugate of the positive branch. Traverse the positive branch as frequency increases, then the conjugate branch in the corresponding return direction. Mark the direction and count clockwise encirclements of $-1$ as positive $N$.

**Apply the criterion to the heater.** The loop transfer is $L_{loop}(s)$, the dimensionless product of controller and plant.
Its closed-loop poles satisfy $1+L_{loop}(s)=0$. The P example has no
open-loop right-half-plane poles: its finite plant pole is −0.2 s⁻¹, so P=0.

For scalar plotting, write $c=\cos(\omega t_d)$, $q=\sin(\omega t_d)$. Then

$$
\operatorname{Re}L=\frac{2[c-5\omega q]}{1+25\omega^2},\qquad \operatorname{Im}L=-\frac{2[q+5\omega c]}{1+25\omega^2}.
$$

The magnitude is $2/\sqrt{1+25\omega^2}$, so unity gain occurs at $\omega_c=\sqrt3/5=0.346410$ rad/s. The phase is $-\arctan(5\omega)-\omega t_d$. At the crossover, the plant phase is −60°. The critical delay is

$$
t_{d,crit}=\frac{2\pi/3}{\omega_c}=6.0459979\text{ s}.
$$

At 6.1 s the unity-crossover phase is about −181.07°. The refined full-path count is $N=0,Z=0$ for 1 s and $N=2,Z=2$ for 6.1 s. For 6.1 s, the positive-frequency branch passes left of −1 at its negative-real crossing and the negative-frequency branch completes two clockwise encirclements. At the boundary the curve passes through −1, so asymptotic stability fails. A coarse plot or guessed winding count is not evidence.

Now return to the preceding lesson's worked PI gains $C=5+1/s$ and $F=(1+T_zs)/(1+2s)$, with $T_z=3$ or 10 s, the 1 s heater delay, beta=1 and no extra measurement filter. Their loop is

$$
L(s)=C F G_u=\frac{.2e^{-s}(1+T_zs)}{s(1+2s)}.
$$

The canceled thermal factor is stable at −.2 s⁻¹. This loop has an integrator pole at s=0, directly on the imaginary-axis path. The contour must avoid that pole: while moving upward, replace the segment from $-j\varepsilon$ to $+j\varepsilon$ with a small semicircle to its right. Here ε is a positive number taken toward zero. This excludes the origin from the enclosed right half-plane, so P=0. Because $L\approx.2/s$ near zero, that detour maps to a large clockwise semicircle on the right of the L plane. Include it when closing the two frequency branches.

Unity gain satisfies

$$
.04(1+T_z^2\omega^2)=\omega^2(1+4\omega^2),
$$

or, writing $y=\omega^2$,

$$
4y^2+(1-.04T_z^2)y-.04=0.
$$

The product of its two roots is −.01, so exactly one root is positive. Use that root to obtain $\omega_c=\sqrt y$. The loop phase is $-\pi/2-\omega+\arctan(T_z\omega)-\arctan(2\omega)$:

| Lead zero time | Crossover, rad/s | Loop phase at crossover |
| --- | ---: | ---: |
| Tz=3 s | .21923158 | −92.9040° |
| Tz=10 s | .87355845 | −116.7961° |

The complete curve gives more than these two phase values. The lead phase lies between zero and π/2. Before the sole unity-gain crossing, the complete phase is therefore at least $-\pi/2-\omega$, remains above −π for both candidates, and stays below zero. That part of the positive-frequency curve cannot cross the negative-real ray extending left from −1. After crossover, magnitude remains below one, so it cannot reach that ray either. The negative-frequency branch is its conjugate; the origin detour lies on the right, and the high-frequency closure approaches zero. Thus the complete curve makes no winding around −1: N=0 and Z=P+N=0 for both continuous linear models. For $\omega\ge2$, $|L|\le T_z/(10\omega)\le.5$, which bounds the remaining tail.

This conclusion uses the stated 5/1 PI gains and lead elements. Repeat the complete calculation for other selected controller or model parameters. It also does not qualify a clipped transient: the previous lesson's Tz=10 first request of 151.5% is outside the linear, unsaturated analysis, even though its continuous linear modes are stable.

## Prediction

Predict the path direction, P, N and Z for 1 s and 6.1 s. Predict why 6.1 s grows slowly: it is only slightly beyond the critical delay. Predict why an 8 s optional comparison has the same qualitative $N=2$ count but faster growth. Predict why a finite quiet time record cannot prove the continuous model stable. For both shaped loops, calculate the positive crossover root and phase, explain the origin detour, and use the entire curve argument to determine N, P and Z. Explain why stable modes do not guarantee a passing noise or saturation requirement.

## Implementation

For the fixed-bias P example, write scalar calculations for real part, imaginary part, magnitude, phase and crossover. Refine frequency points near 0.34–0.35 rad/s and include the high-frequency tail; for $|\omega|\ge2$, $|L|<0.2$, so the tail lies near the origin and cannot wind around −1. Construct the conjugate branch and closure explicitly. Do not implement a winding library or infer N from a coarse screenshot.

For the shaped-loop plot at $\omega>0$, scalar complex division from the sensitivity lesson is sufficient. Define $c=\cos\omega$, $q=\sin\omega$, numerator components $n_r=.2(c+T_z\omega q)$ and $n_i=.2(T_z\omega c-q)$, and denominator components $a=-2\omega^2$, $b=\omega$. Calculate

$$
\operatorname{Re}L=\frac{n_ra+n_ib}{a^2+b^2},\qquad
\operatorname{Im}L=\frac{n_ia-n_rb}{a^2+b^2}.
$$

Handle the origin detour separately; do not divide by zero or join the two infinite ends with an arbitrary straight line.

## Experiment in Control Studio

Before the time-domain demonstration, use the scalar model calculations to construct both shaped PI curves, their conjugate branches and the origin detour. Check the crossover values and tail bound above. Retain the complete-curve decision beside the same candidates’ temperature, noise and output records from loop shaping.

1. Reset hot PV/SP=35 °C, applied command/history 75%, P-only $K_p=10$, fixed 75% bias, integral updates off, $\beta=1$, filter bypassed, D=0, and 0.1 s updates.
2. Fill the entire command history for the selected delay with 75%. Before **Confirm and start**, declare the 180 s duration, SP step 35→35.1 °C at time zero, transport delay, T bounds 34–36 °C, output 0–100%, and reset. The SP step provides a visible perturbation; the continuous model is analyzed about its 35 °C balance.
3. Run $t_d=1$ s, then fully reset and run $t_d=6.1$ s. Keep every condition except the declared physical transport delay identical. Optionally repeat with 8 s for clearer growth.
4. Display PV, error, request/applied output, delay, and elapsed time. Analyze growth around each case’s equilibrium; for the no-load model, the 35.1 °C target with P-only bias predicts 35.066667 °C equilibrium, not exact SP.
5. Abort on PV outside 34–36 °C, applied output outside 0–100%, invalid data/clock, or unresponsive execution. Command zero heat, stop the simulator if needed, and fully reset before repeating.

## Evidence and decision

For the worked shaped PI candidates, the complete path gives N=0 and P=0, hence Z=0. Judge their preceding performance records separately: a stable continuous model can still fail a noise, peak or output requirement. The P-only delay runs below illustrate a different fixed controller and must not be relabelled as those shaped PI records.

Compare the directed model path and refined (N/P/Z) count with time traces. The 1 s record should illustrate bounded decay; 6.1 s may show only slow growth within 180 s, so absence of visible growth is not proof of stability. Sampling at 0.1 s also changes the exact sampled boundary; the Nyquist result applies to the continuous-delay model, not a general hardware guarantee.

## Conclusion

Nyquist stability requires a complete directed path, pole count, and signed encirclement count. For the P example, 1 s gives Z=0 and 6.1 s gives Z=2, with slow growth just beyond the critical delay. The complete curves of the stated shaped PI examples give Z=0, while their performance and saturation remain separate questions.

## Reason for the next lesson

Nyquist has answered a continuous-model stability question that included physical delivery delay. The implemented controller also samples measurements, holds commands and may apply a calculation one update later. Those timing effects need their own model.

[Sampled-data stability](../50-position-z-plane/README.md) returns to the already-studied shaft to isolate held torque and computation delay, keeping its physical dynamics unchanged.
