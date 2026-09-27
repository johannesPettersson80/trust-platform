# 48. Routh–Hurwitz stability

Find which proportional gains make a shaft's small position disturbances die
away. Let Control Studio derive the stable range, then test three gains on the
actual ST controller and simulated shaft.

## The problem

[Root locus](../45-root-locus/README.md) showed the poles at selected gains.
Checking a few points does not prove which values between or beyond them are
stable. We need the complete stable range, including its boundaries.

We use the same physical shaft, with inertia and friction both numerically one.
This introductory example removes the previous lesson's velocity feedback and
uses proportional position control alone.

## What we need

We need a way to derive stability conditions from the characteristic polynomial,
without solving its roots at every possible gain. We also need to distinguish a
stable model from a boundary case that merely happens to remain still in one test.

## The new idea

The **Routh–Hurwitz stability criterion** is a mathematical test for whether a linear control system is stable. It uses the coefficients of the system’s characteristic polynomial without calculating every pole individually.

For this lesson, **stable means asymptotically stable: small disturbances die away**.

A pole’s real part determines whether its corresponding natural response decays or grows:

- **Negative real part:** the response decays.
- **Positive real part:** the response grows.
- **Zero real part:** the response does not necessarily decay.

We arrange the polynomial’s coefficients in a **Routh table** and inspect its first column. When the ordinary nonzero-pivot rules apply, **each sign change down that column indicates one right-half-plane pole—an unstable pole**.

With no poles on the imaginary axis, no sign changes means all poles are in the left half-plane.

Start with a general second-order characteristic equation:

$$
a_2s^2+a_1s+a_0=0.
$$

Here:

- $s$ is the complex variable describing the system’s natural responses.
- $a_2$ is the coefficient of $s^2$.
- $a_1$ is the coefficient of $s$.
- $a_0$ is the constant term.

Place alternating coefficients in the first two rows:

$$
\begin{array}{c|cc}
s^2 & a_2 & a_0\\
s^1 & a_1 & 0
\end{array}
$$

The first entry of the next row is calculated from the two rows above:

$$
\frac{a_1a_0-a_2\cdot0}{a_1}=a_0,
\qquad a_1\ne0.
$$

The complete table is therefore:

$$
\begin{array}{c|cc}
s^2 & a_2 & a_0\\
s^1 & a_1 & 0\\
s^0 & a_0 &
\end{array}
$$

With $a_2>0$, the second-order system is asymptotically stable when:

$$
\boxed{a_1>0\quad\text{and}\quad a_0>0.}
$$

These conditions make every entry in the first column positive.

Our shaft has inertia $J=1\ \mathrm{kg\,m^2}$ and viscous friction $B=1\ \mathrm{N\,m\,s/rad}$. Its motion satisfies:

$$
J\ddot{\theta}+B\dot{\theta}=u.
$$

Here $\theta$ is angle, $\dot{\theta}$ is angular velocity, $\ddot{\theta}$ is angular acceleration, and $u$ is applied torque.

The proportional controller commands:

$$
u=K_p(r-\theta),
$$

where $r$ is the target angle and $K_p$ has units $\mathrm{N\,m/rad}$.

Substituting the controller into the shaft equation gives:

$$
J\ddot{\theta}+B\dot{\theta}+K_p\theta=K_pr.
$$

With $J=B=1$, the characteristic equation is:

$$
s^2+s+K_p=0.
$$

Its Routh table is:

$$
\begin{array}{c|cc}
s^2 & 1 & K_p\\
s^1 & 1 & 0\\
s^0 & K_p &
\end{array}
$$

The first column is $1,1,K_p$. Therefore:

$$
\boxed{K_p>0}
$$

is the stable gain range for this continuous-time linear model.

| Gain | First-column signs | Result |
|---|---|---|
| $K_p=2$ | $+,+,+$ | Stable |
| $K_p=5$ | $+,+,+$ | Stable |
| $K_p=-1$ | $+,+,-$ | One unstable pole |

At $K_p=0$, the final entry is zero. Examine this boundary separately:

$$
s^2+s=s(s+1)=0.
$$

The poles are $0$ and $-1$. A stationary shaft displaced from its target can remain displaced because the controller applies no correcting torque. Its error does not disappear, so this case is **not asymptotically stable**.

For higher-order systems, the table has more rows. A zero first-column entry or an entire zero row requires special handling; we cannot simply count signs through those cases.

**The criterion tells us which gains give stability. It does not tell us which stable gain produces less oscillation or faster settling.**

## Prediction

For a 0→0.01 rad target step from rest:

| Gain $K_p$ | Routh prediction | Expected shaft behavior |
| ---: | --- | --- |
| $1$ | First column $1,1,1$; stable | Moves toward the target; oscillation decays. Initial torque is $1\times0.01=0.01$ N·m. |
| $0$ | Origin pole; boundary | Torque remains zero. Angle stays at zero and the 0.01 rad error remains. |
| $-1$ | First column $1,1,-1$; one unstable pole | Initial torque is −0.01 N·m, so the shaft moves away from its positive target. |

A finite simulation cannot prove every positive gain works in the sampled,
limited implementation. The continuous stable interval comes from the
coefficient inequalities, not from these three trials.

## Implementation

The prepared [ST program](src/Main.st) calculates proportional torque every
20 ms. The [simulated plant](simulation.toml) integrates the shaft's angle and
velocity, starting at zero. There is no integral or velocity-feedback term.

`P1.ExperimentEnabled` starts the ST-owned 0.01 rad step. `P1.Kp` is the
reviewable gain; the configured demonstration range is −1 to 4 N·m/rad.
That bounded configuration is separate from the mathematical interval $K_p>0$.
Applied torque is limited to ±20 N·m.

The program latches `SafetyAbort` if angle reaches ±0.2 rad or velocity reaches
±2 rad/s, or its input/gain checks fail. It then reports Manual and commands
zero torque. These are controller stop thresholds: the simulated shaft can
continue coasting under friction after torque is removed. Restart the simulator
to reset angle, velocity, the latch and source-default gain before each trial.

## Experiment in Control Studio

1. Open this project and start **Simulator**. Open **Control Studio** for
   **Routh-Hurwitz stability**. Confirm zero angle, zero torque and Manual mode.
2. In **Tune**, find **Routh–Hurwitz stability**. Enter `1` in **Parameter value**
   and click **Calculate Routh–Hurwitz**. Read the polynomial, coefficients,
   three table rows, first-column signs and stable interval. Expand
   **How the table is calculated** and match its numerical row calculation to
   the formula above.
3. Calculate `0` and `-1` too. Explain the origin-pole message at zero and the
   one unstable pole at −1. Each calculation leaves the runtime unchanged.
4. Return to `1` and calculate again. Click **Use selected analysis values**,
   then **Review candidate**. Check `P1.Kp`, its unit and the reviewed value.
   Click **Apply reviewed values** and wait for confirmation that the value was
   observed after ST execution.
5. In **Test**, leave the maximum duration at **10 s** and click **Review test**.
   Read the activation and restoration. The prepared ST program owns the
   0.01 rad step; the observation experiment does not inject another step.
6. In **Test**, click **Confirm and start**. Test automatically records the
   reviewed observation. Observe target, angle and applied torque in the
   response; velocity is also a declared recorded signal. Use **Abort test**
   if you need to stop early. For the negative-gain trial, expect the program's
   own threshold to remove torque before the ten-second observation ends.
7. After Test reaches terminal restoration, confirm Manual and zero applied torque.
   In **Validate**, click **Save result**. Under **Saved results**, select its
   revision and click **Open** to inspect the retained response. This saves
   the response, not the Routh table; recalculate the table when needed.
8. Stop and restart Simulator for a fresh zero state. Repeat the calculation,
   review/application and experiment for `0`, then `-1`. Apply each gain
   **after** the restart, because restart restores the source default `Kp=1`.

## Evidence and decision

For each trial, retain the selected gain, Routh signs, boundary interpretation
and saved response revision. Compare angle, velocity, applied torque and Manual
state with your prediction.

At $K_p=1$, check that the final automatic-mode angle is within 0.001 rad of
the 0.01 rad target and that the oscillation is decaying. At zero, explain why
no motion and zero torque leave the command error uncorrected. At −1, identify
the growing negative displacement, the change to Manual and zero torque, and
any subsequent coast. Do not interpret the stopped tail as a stable
negative-gain closed loop: its controller has been disabled.

If observation disagrees with the prediction, first check the applied gain,
reset state, units, clipping, sample time and stop conditions. The theorem
applies to the declared continuous linear model.

## Conclusion

For this shaft under proportional position control, every positive $K_p$
makes the continuous model asymptotically stable. Zero is an excluded boundary;
negative gain produces a growing mode. Control Studio derives that range from
the polynomial, and the actual ST trials demonstrate the three distinct cases.

## Reason for the next lesson

Stability tells us whether natural disturbances die away. It does not tell us
how strongly the system responds to a continuing periodic command or disturbance.
[Periodic excitation](../../05-experiments-identification-and-validation/22-excitation-for-dynamics/README.md)
returns to the heater to investigate that frequency-dependent response.
