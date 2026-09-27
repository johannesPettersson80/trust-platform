# 37. Discrete PID

You will translate PID action into one ST update using elapsed time and successive angle measurements. You will calculate the P, I and D torque contributions and identify which values must persist between updates.

## The problem

The shaft controller runs every .02 s. Integral action adds an area over that interval, while derivative action estimates velocity from sampled angle. A display refresh is not the controller clock. A correct implementation must retain the previous angle, previous timestamp and integral state, initialize them deliberately, and distinguish requested from applied torque.

## What we need

We need one correctly ordered controller update that combines P, I,
filtered D, limits and tracking using the same elapsed interval. Each
calculation must use the appropriate current or retained previous value.

## The new idea

A **discrete PID controller** calculates its output at separate update times
using values retained from previous updates. We combine the terms already
taught in one order: calculate P and trial I, update the velocity filter,
form the request, limit it, then correct the retained I for the next update.

Use $J=1$, $B=1$, $K_p=16$, $K_d=7$, $K_i=4$ when integral action is enabled, $dt=.02$ s, and torque limits ±20 N·m. Use scalar state only: previous angle, previous update time, integral torque and the previous filtered velocity. The trial integral is a calculation for this update; the corrected integral is the value retained afterward. Each retained value has a separate physical or timing role in this update.

Let $e_k=SP_k-\theta_{m,k}$. Proportional torque is

$$
P_k=K_p e_k.
$$

Let $I_{k-1}$ be the retained integral after the preceding tracking correction. The trial contribution after adding the current rectangular error area is

$$
I_{trial,k}=I_{k-1}+K_i e_k\,dt.
$$

If $e_k=0.08$ rad, $K_i=4$, and $dt=.02$ s, the increment is $4(.08)(.02)=.0064\ \mathrm{N\,m}$ with Ki in N·m/(rad·s). The derivative uses the average angle slope over the sampled interval, not an always-current continuous velocity:

$$
\omega_k=\frac{\theta_{m,k}-\theta_{m,k-1}}{t_k-t_{k-1}},\qquad D_k=-K_d\omega_k.
$$

With filtering bypassed, $\omega_k=1$ gives $D_k=-7\ \mathrm{N\,m}$. A smaller interval makes a fixed angle difference imply a larger slope; it also changes which noise differences are visible.

As a complete hypothetical update, use $SP=.50$, $\theta_m=.42$, previous measured angle 0.40 rad, previous integral $I_{k-1}=1.962$ N·m, $K_p=16$, $K_i=4$, $K_d=7$, $dt=.02$, and the derivative filter bypassed, giving $\omega=(0.42-0.40)/0.02=1$ rad/s. Then $e=.08$, $P=1.28$, $\Delta I=.0064$, $I_{trial,k}=1.9684$, $D=-7$, request $u=-3.7516\ \mathrm{N\,m}$, and applied torque is the same if it is within ±20. These are controller inputs for arithmetic, not a claimed physical run.

Now keep those same arithmetic inputs but use the .2 s derivative filter, whose previous filtered velocity is .2 rad/s. The raw slope is still 1 rad/s. With the preceding lesson's $\alpha=dt/(T_f+dt)=1/11$,

$$
\omega_{f,k}=.2+\frac{1}{11}(1-.2)=.272727\ \mathrm{rad/s},
\qquad D_k=-7(.272727)=-1.909091\ \mathrm{N\,m}.
$$

P remains 1.28 N·m and the trial integral remains 1.9684 N·m, so the complete request is $1.28+1.9684-1.909091=1.339309$ N·m. It is within ±20 N·m, so tracking adds zero and the retained integral equals the trial value. This example states the previous filter value explicitly; it is not the reset state for the live shaft experiment. The different torque shows the damping delay introduced by filtering the same measured slope.

The last two operations are the output limit and retained-state correction:

$$
u_{a,k}=\operatorname{clamp}(u_{r,k},-20,20),\qquad
I_k=I_{trial,k}+\frac{dt}{T_t}(u_{a,k}-u_{r,k}).
$$

$u_r$ and $u_a$ are requested and applied torque in N·m. The clamp restricts
the applied value to the stated bounds. Tracking time $T_t=0.2$ s is retained
from Lesson 35. The corrected $I_k$ becomes the next update's old integral.

## Prediction

Calculate both complete updates above, including which old filter value produces the filtered torque. Predict the signs when target is above measured angle and velocity is positive: P and the integral increment are positive while D opposes positive motion. The accumulated I also depends on its previous value. Predict that using two angle samples .04 s apart with the same angle difference gives an average velocity 0.5 rad/s, not a numerical “fix” for the sensor. Predict that a nonadvancing or backward timestamp must stop the demonstration rather than update state.

## Implementation

Use scalar REAL variables for angle, setpoint, error, P, I, D, request, applied torque and elapsed time. Initialize the previous angle from the first valid measurement and skip derivative differencing until the next sample. Require $0<dt\le0.05$ s for this timing exercise. On valid updates, calculate error and raw velocity from the current and previous measured angles. Calculate the filtered velocity from its previous stored value using the selected filter time, or use raw velocity for bypass. Calculate P, the trial integral and D from that selected velocity, form the complete request, clamp to ±20 N·m, and apply `IntegralNm := TrialIntegralNm + (ElapsedS / 0.2) * (AppliedTorqueNm - RequestedTorqueNm);`. This is the torque tracking rule from the load lesson. On invalid data or time, stop this demonstration and command zero torque; full fault recovery is Lesson 28.

After forming the request, limiting it and correcting the integral, retain the current measurement, timestamp, filtered velocity and corrected integral for the next update. Never replace the previous angle before taking its difference. Do not overwrite measured angle from source code. Expose the previous angle, interval, each term, unlimited request and applied torque so the learner can inspect one scan. Retain whether I was enabled; P/PD trials do not silently accumulate I.

## Experiment in Control Studio

Use **Identify → Tune → Validate**. On **Identify**, select the project-owned test and review the declared position signals. The project records the complete sampled update, including previous angle, interval, P, trial I, filtered D, requested torque, applied torque and corrected retained I. Press **Review project test**, then **Confirm and start**.

Use the compact project parameter editor to repeat the same reset with the declared sample interval (.02 s, then .04 s). Keep the prescribed .40/.42 rad arithmetic check separate from the plant record, and use **Tune** and **Validate** only to inspect the retained recording and requirements. Stop on angle outside ±2 rad, velocity outside ±10 rad/s, applied torque outside ±20 N·m, invalid data/time or monitor failure. Command zero torque, allow the unloaded shaft to coast, and reset before another case; under a constant load, cancel the virtual load before reset.

## Evidence and decision

The bypass arithmetic must produce trial and retained I=1.9684 N·m, D=−7 N·m and request/applied torque −3.7516 N·m. The filtered arithmetic uses the same measurements and I, but the stated old filter state gives D=−1.909091 N·m and request/applied torque 1.339309 N·m. The live motion record separately assesses peak and error; it does not turn one matching scan into full performance evidence. Check that invalid state does not update.

Use the same recorded measured inputs only for an algebra shadow comparison of alternative calculations. If claiming motion performance, use separate plant resets and the full sensor/controller path. Report saturation and whether I was enabled.

## Conclusion

Sampled integral action is a rectangular error area, and sampled derivative action is an average angle slope multiplied by $-K_d$. Their units, retained state, timestamps and initialization determine the controller actually running.

## Reason for the next lesson

Consecutive complete-output equations can be subtracted to form an incremental PID update, but only when previous output and retained states represent the same controller.

[Positional and incremental PID](../38-positional-incremental-pid/README.md) derives that equivalence.
