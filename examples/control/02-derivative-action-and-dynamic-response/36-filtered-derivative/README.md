# 36. Derivative filtering

You will estimate shaft velocity from sampled angle, filter the derivative contribution, and measure both torque-activity reduction and lost braking under unchanged motion requirements.

This extends PD damping using the [measurement-filter calculation](../../01-feedback-foundations-and-temperature-control/11-measurement-filtering/README.md) and [sampling concepts](../../01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling/README.md).

## The problem

Derivative torque multiplies measured velocity by $K_d$. Encoder quantization or electrical noise in angle can therefore create large alternating torque requests even when the shaft barely moves. Filtering can reduce that activity, but its lag can delay braking. The filter decision must be made from torque activity and motion requirements together.

## What we need

We need to reduce torque changes caused by noisy velocity estimates without
delaying the braking so much that motion requirements fail. Both noise
activity and the response to real motion must guide the choice.

## The new idea

**Derivative filtering** reduces rapid noise-driven changes in the motion
estimate used for D control. A small angle error divided by a short time
interval can become a large apparent velocity. Smoothing that estimate reduces
torque activity, but it also delays useful braking.

From sampled measured angle $\theta_m$, form a scalar velocity estimate after the first valid sample:

$$
\omega_{\mathrm{raw},k}=\frac{\theta_{m,k}-\theta_{m,k-1}}{dt}.
$$

The P term uses measured angle, $P=K_p(SP-\theta_m)$; ground-truth angle is used only for the declared motion requirements. The derivative filter is a first-order lag:

$$
T_{\mathrm{filter}}\frac{d\omega_f}{dt}+\omega_f=\omega_{\mathrm{raw}}.
$$

Backward Euler with the current filtered value on the lag term gives

$$
\omega_{f,k}=\omega_{f,k-1}
+\frac{dt}{T_{\mathrm{filter}}+dt}
(\omega_{\mathrm{raw},k}-\omega_{f,k-1}).
$$

Thus $\alpha=.5$ for $T_{\mathrm{filter}}=.02$ s and $\alpha=1/11$ for .2 s. At rest, a change in angle-measurement error from −0.002 to +0.002 rad over 0.02 s gives a raw velocity error of +0.2 rad/s. With $K_d=7$, that raw error produces −1.4 N·m of D torque. For an isolated +0.2 rad/s sample with the filter state initially zero, the first filtered values are 0.1 and 0.0181818 rad/s, giving −0.7 and −0.127273 N·m. These are first-update values, not the steady amplitudes of an alternating sequence. For a real 1 rad/s velocity, raw D is −7 N·m; the .2 s filter's first D is −.63636 N·m. Its remaining fraction after n updates is $(10/11)^n$. Requiring this to be at most 0.1 gives n≥ln(0.1)/ln(10/11), so round up to 25 updates, or 0.5 s, showing the lost braking cost.

Initialize previous angle from the first valid measurement and filtered velocity to zero at rest. Do not manufacture a first derivative from an invented pose history.

Use the same unloaded shaft as Lessons 33–35: $J=1$, $B=1$, $K_p=16$, $K_d=7$, torque limits ±20 N·m, $dt=.02$ s, SP 0→1 at experiment time 0. The PD controller has no integral action here: use the unloaded comparison to isolate derivative noise, while retaining the PID holding design for the following complete-update and commissioning tests. Each candidate receives the same additive angle-noise sequence added to its own simulated plant angle; do not replay one complete angle trajectory as if it were closed-loop evidence.

Choose filter candidates $T_{\mathrm{filter}}=0$ s (bypass), .02 s and .2 s. Requirements are fixed before testing: true-angle peak ≤1.1 rad, $|true\ error|\le.02$ rad from 4–8 s, and applied torque within ±20 N·m. Compare derivative activity as the sum of absolute consecutive D-torque changes over 4–8 s, in N·m, plus applied-torque activity. This is command activity, not energy or wear.

## Prediction

Calculate the raw and first filtered responses to the isolated velocity-error sample and D torques above. Predict that the longer filter reduces activity more but delays a true velocity step. Predict which candidate can meet both the activity benefit and the true-angle peak/error requirements.

## Implementation

Use scalar REAL values for measured angle, previous angle, raw/filtered velocity, $P$, $D$, request, applied torque and elapsed time. Use a BOOL for validity. On the first valid sample, initialize previous angle and skip D. On later valid samples, require positive reasonable time, calculate raw velocity, update filter unless bypassed, then calculate $D=-K_d\omega_f$. Sum P and D, clamp ±20, and expose raw/filtered signals. Invalid time or data stops this demonstration and commands zero torque.

## Experiment in Control Studio

Use **Identify → Tune → Validate**. On **Identify**, select the project-owned test, open its compact parameter editor and choose `TrialKind` 0 (bypass), 1 ($T_f=.02$ s) or 2 ($T_f=.2$ s). Press **Review project test**, then **Confirm and start**. Reset the project between candidates so every run uses the same angle-noise generator, gains and zero initial state.

Keep the measured response on Identify, then use **Tune** and **Validate** to inspect the retained controller result and applicable requirements. Display measured angle, controller-facing angle, velocity, raw/filtered signals, P, D, requested/applied torque and elapsed time; compute D and applied-torque activity over 4–8 s. Abort on angle outside ±2 rad, velocity outside ±10 rad/s, applied torque outside ±20 N·m, invalid data/time, or an unresponsive monitor. Command zero torque, allow unloaded coasting, and reset before another candidate.

## Evidence and decision

A filter qualifies only if it reduces the declared D/applied activity and still meets peak, error-window and torque limits. Compare each candidate's own closed-loop true-angle trace; a replayed angle trajectory is not equivalent evidence. Report the raw and filtered calculation alongside the motion result. A quieter D trace can be unacceptable if delayed braking increases true-angle peak.

## Conclusion

Filtering derivative feedback follows a derived lag update. The longer filter suppresses sampled angle-noise torque but delays real braking, so activity and motion requirements must be assessed together.

## Reason for the next lesson

We can calculate a filtered velocity and assess its noise and damping tradeoff. The complete PID controller must now combine that retained filter state with the integral, output limits and tracking in one correctly ordered update.

[Discrete PID](../../03-production-quality-digital-pid/37-discrete-integral-derivative/README.md) makes that complete calculation explicit, using the actual elapsed interval and the previous stored values.
