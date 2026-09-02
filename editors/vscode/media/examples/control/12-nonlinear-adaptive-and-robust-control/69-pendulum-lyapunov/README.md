# Certify a nonlinear inverted pendulum locally

Course position: Lesson 69 of 84. Prerequisite: constrained building-HVAC model predictive control. Next lesson: nonlinear robot-joint control.

**Lesson objective:** Evaluate a local Lyapunov certificate on a nonlinear pendulum, map phase-plane trajectories, and reject global claims that are supported only by simulation.

## The problem

The equipment in this lesson is Motor-driven inverted-pendulum laboratory rig.

The equipment is a rigid pendulum driven by a torque motor at its pivot. The absolute pendulum angle ($\theta$) is measured in radians. The angular rate ($\omega$) states how quickly that angle changes, in radians per second. The motor produces requested and applied torque in newton metres.

The pendulum is upright when its absolute angle is pi radians. Its upright equilibrium is therefore pi rad, zero angular rate, and zero motor torque. Angle error ($\alpha$) means the measured absolute angle minus the upright angle setpoint. Positive and negative error identify opposite sides of upright.

![Inverted-pendulum feedback and Lyapunov-certificate path](figures/pendulum-lyapunov-signal-path.svg)
The nonlinear simulator uses a rigid-joint model with inertia 1 kilogram square metre and gravity-torque coefficient 1 newton metre. Friction is zero so the lesson isolates gravity, feedback, saturation, and the certificate. The plant advances with 10-millisecond Runge–Kutta substeps while the controller executes every 100 milliseconds. Applied torque is limited to -2 through +2 newton metres. These configured dynamics and every result in this lesson are Simulation evidence.

Lesson 68 predicted a coupled building with a linear model over a declared operating range. Local linear reasoning is useful, but it does not establish what happens far from that operating point.

One successful inverted-pendulum recovery has the same limitation. It does not prove that every nearby release converges, that the motion remains inside the assumed region, or that the argument survives motor saturation. This lesson adds a mathematical certificate and then tests its boundary without changing the controller.

## What we need

We need a mathematical certificate that proves local upright stability for declared pendulum states and a separate simulation record outside that certified region. Successful trajectories must not be promoted into a global proof.

## The new idea

A state is the smallest set of quantities needed to predict future motion. Here the state consists of angle error and angular rate. An equilibrium is a state that remains constant under its equilibrium input. At the upright equilibrium both state components and the required motor torque are zero.

A phase plane is a graph with angle error on its horizontal axis and angular rate on its vertical axis. Each initial state starts at one point. As the pendulum moves, its state draws a trajectory through the plane. Several trajectories reveal useful behavior, but a finite number of simulated trajectories cannot prove the boundary of every trajectory that was not run.

Local stability is a claim about a declared neighbourhood around an equilibrium. Global stability is a claim over the complete admissible state space. A region of attraction is the set of initial states known to converge to the equilibrium. This lesson certifies a local subset of that region; it makes no global claim.

The physical nonlinear equations come before the certificate. The absolute angle changes at the angular rate. Motor torque and gravity change the angular rate:

$\\dot\theta=\omega$

$\\dot\omega=\tau_{\mathrm{applied}}-\sin\left(\theta\right)$

Because the absolute angle equals pi plus the angle error, the sine of absolute angle is the negative sine of angle error. The error dynamics are therefore:

$\\dot\alpha=\omega$

$\\dot\omega=\tau_{\mathrm{applied}}+\sin\left(\alpha\right)$

The local feedback requests four newton metres per radian of angle correction and two newton metre seconds per radian of rate damping:

$\tau_{\mathrm{requested}}=-4 \cdot \alpha-2 \cdot \omega$

Near upright, the sine of a small angle error is approximately the angle error.
The resulting local closed-loop matrix is:

$\mathbf{A}_{cl}=\begin{bmatrix}0&1\\-3&-2\end{bmatrix}$

A Lyapunov function is a positive energy-like number that is zero at equilibrium and positive elsewhere. If it continually decreases away from equilibrium, the state cannot climb out through a fixed level boundary. The symmetric matrix used here is:

$\mathbf{P}=\begin{bmatrix}\frac{4}{3}&\frac{1}{6}\\\frac{1}{6}&\frac{1}{3}\end{bmatrix}$

Its first diagonal value is positive and its determinant is `5/12`, which is also positive. Those two facts make the matrix positive definite. The Lyapunov value ($V$) is:

$V=0.5 \cdot ((4/3) \cdot \alpha^{2}+(1/3) \cdot \alpha \cdot \omega+(1/3) \cdot \omega^{2})$

The actual pendulum still contains the sine function. The nonlinear remainder
($r(\alpha)$) is the difference between that sine and its local linear
approximation: $r(\alpha)=\sin(\alpha)-\alpha$. Including this remainder gives
the continuous Lyapunov derivative ($\dot V$):

$\dot V=-0.5 \cdot (\alpha^{2}+\omega^{2})+(\alpha/6+\omega/3) \cdot (\sin\left(\alpha\right)-\alpha)$

The certified sublevel set contains every state for which $V\le \rho=0.025$. Its boundary lies within 0.2 radian angle error, its feedback request remains below the 2-newton-metre limit, and the continuous derivative is negative everywhere except the equilibrium. A decreasing value cannot cross outward through the fixed boundary. The sublevel set is therefore an invariant local subset of the region of attraction for this declared nonlinear model and unsaturated feedback.

Those limits can be checked rather than assumed. For a fixed angle error, the smallest possible Lyapunov value occurs at angular rate $\omega=-\alpha/2$. This gives $V\ge (5/8) \cdot \alpha^{2}$; setting $V=0.025$ gives $|\alpha|\le 0.2 rad$. For a fixed angular rate, the smallest value occurs at $\alpha=-\omega/8$. This gives $V\ge (5/32) \cdot \omega^{2}$ and therefore $|\omega|\le 0.4 rad/s$.

The greatest magnitude of the feedback request anywhere on this ellipsoidal boundary is $\sqrt{0.96}=0.980 N m$, safely below the 2-newton-metre limit. Finally, inside the angle bound, $|\sin\left(\alpha\right)-\alpha|\le |\alpha|^{3}/6$. Applying that bound to the nonlinear remainder gives $\dot V\le -(0.5-1/450) \cdot \alpha^{2}-(0.5-1/900) \cdot \omega^{2}$, which is strictly negative for every nonzero state in the sublevel set. These calculations are the reason the boundary is certified; the trajectory plot is a separate executable check.

That statement does not prove global stability. It also does not cover a different model, gains, torque limit, sample interval, stale measurement, physical rig, or state outside the sublevel set.

## Prediction

Start with angle error 0.1 rad and angular rate zero. The feedback requests:

$\tau_{\mathrm{requested}}=-4 \cdot 0.1-2 \cdot 0=-0.4 N m$

The Lyapunov value is:

$V=0.5 \cdot (4/3) \cdot 0.1^{2}=0.0066667$

The nonlinear remainder makes the continuous derivative:

$\dot V=-0.5 \cdot 0.1^{2}+(0.1/6) \cdot (\sin\left(0.1\right)-0.1)=-0.0050028 1/s$

The value is below 0.025, the derivative is negative, and requested torque is inside the actuator limit. Predict that this release is certified.

Now use angle error 0.25 rad and zero rate. Its value is $V=0.0416667$, which is above 0.025. Predict that it begins outside the certificate. It may still recover in simulation, but that outcome cannot retroactively certify its initial state.

## Implementation

The shipped Structured Text uses the measured nonlinear plant state. It does not integrate a private linear pendulum:

```st
AngleError := AbsoluteAngle - AngleSetpoint;
TorqueRequested := -LREAL#4.0 * AngleError - LREAL#2.0 * AngularRate;
TorqueApplied := LIMIT(LREAL#-2.0, TorqueRequested, LREAL#2.0);

LyapunovValue := LREAL#0.5 * (
    (LREAL#4.0 / LREAL#3.0) * AngleError * AngleError
    + (LREAL#1.0 / LREAL#3.0) * AngleError * AngularRate
    + (LREAL#1.0 / LREAL#3.0) * AngularRate * AngularRate);

InsideCertifiedSet := LyapunovValue <= CertificateLevel;
```

`TorqueRequested` is the controller calculation. `TorqueApplied` is the bounded plant input. `TorqueSaturated` states when they differ. `LyapunovDerivative` evaluates the continuous nonlinear formula. `LyapunovDelta` compares consecutive sampled values after `EvidenceInitialized` is true. `CertificateValid` also requires enabled control, sublevel membership, no saturation, and decreasing value away from equilibrium.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Evaluate the declared Lyapunov certificate over the complete inside and outside initial-state map without changing feedback. The declared event lasts 8 seconds. It restores Angle error to 0 rad afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Upright angle setpoint (SP, rad); Absolute pendulum angle (PV, rad); Requested motor torque (MV, N m) and Applied motor torque (MV, N m), including units, limits, mode, and quality. The initial operating point is Upright angle setpoint = 3.141592653589793 rad, Absolute pendulum angle = 3.241592653589793 rad, Pendulum angular rate = 0 rad/s, Applied motor torque = 0 N m. The unresolved baseline problem is: One successful nonlinear trajectory does not establish an invariant region or prove convergence for neighbouring initial states.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Call an initial state certified only when it lies in the declared sublevel set and all model, feedback, torque and decrease assumptions remain valid.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Observe without excitation**, **Amplitude** = **0 rad**, and **Maximum duration (s)** = **8**. The affected signal is **Angle error**; it starts at 0.1 rad and must restore to 0 rad.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Applied motor torque -2 N m to 2 N m; Absolute pendulum angle 0 rad to 6.2832 rad; Pendulum angular rate -10 rad/s to 10 rad/s.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Angle error to 0 rad, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Quadratic Lyapunov certificate for a declared nonlinear sublevel set** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Quadratic Lyapunov certificate for a declared nonlinear sublevel set** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Quadratic Lyapunov certificate for a declared nonlinear sublevel set** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Quadratic Lyapunov certificate for a declared nonlinear sublevel set**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: certified_sublevel_exit_count == 0 (dimensionless) (tolerance 0 (dimensionless)); certified_positive_lyapunov_change <= 0 (dimensionless) (tolerance 1e-12 (dimensionless)); certified_final_angle_error <= 0.01 rad (tolerance 0.000001 rad); certified_final_angular_rate <= 0.02 rad/s (tolerance 0.000001 rad/s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Initial Lyapunov value is the value at release. Maximum Lyapunov value is the largest value over the eight-second record. Maximum positive sampled change is the largest difference between consecutive values after evidence initialization. Maximum continuous derivative is the least-negative derivative observed away from equilibrium.

Settling time begins at release and ends only when absolute angle error is at most 0.01 rad and absolute angular rate is at most 0.02 rad/s for the declared final 0.5-second window. If that condition is not maintained, report `Not settled`.

Also retain peak requested and applied torque in newton metres, saturation duration in seconds, sublevel-exit count, and certificate-violation count. Requested and applied torque are separate measures. Every result retains its source signals, model, initial state, interval, gains, limit, matrix, certificate level, units, window, tolerance, and Simulation classification.

### Validation

An independent oracle must match the nonlinear ten-substep Runge–Kutta plant update and every Structured Text certificate calculation. Every non-equilibrium sample of every certified trajectory must retain $V\le 0.025$, have negative continuous derivative, have nonpositive sampled change after initialization, and remain unsaturated. After eight seconds, absolute angle error must be at most 0.01 rad and absolute angular rate at most 0.02 rad/s.

Both 0.25-radian releases must be marked outside the set at their first sample. Their later motion remains observational regardless of whether they enter the set. The 0.8-radian case must expose saturation and reject certificate validity. Disabled mode must apply zero torque.

The shipped native execution produced this Simulation evidence:

| Certified initial state $(\alpha, \omega)$ | Initial V | Peak applied torque | Settling time | Final absolute $(\alpha, \omega)$ |
| --- | ---: | ---: | ---: | ---: |
| `(+/-0.05,0)` | 0.001667 | 0.200 N m | 1.5 s | `(0.00001 rad,0.00001 rad/s)` |
| `(+/-0.10,0)` | 0.006667 | 0.400 N m | 2.5 s | `(0.00003 rad,0.00003 rad/s)` |
| $(+0.15,-0.15)$ and its sign reverse | 0.015000 | 0.300 N m | 2.7 s | at most `(0.00005 rad,0.00002 rad/s)` |
| `(0,+/-0.35)` | 0.020417 | 0.700 N m | 3.2 s | at most `(0.00003 rad,0.00014 rad/s)` |

Across all eight certified records, sublevel exits, certificate violations, saturation duration, and maximum positive sampled Lyapunov change were all zero. The independent oracle matched the simulator's nonlinear ten-substep Runge–Kutta update and the Structured Text values.

Both `(+/-0.25,0)` records began at $V=0.041667$, outside the certificate. Each later entered the certified set and ended near `(0.00006 rad,0.00007 rad/s)`. That is useful simulated recovery evidence, but their initial states remain uncertified. The $(0.8,0)$ rejection began at $V=0.426667$, requested as much as `3.200 N m`, was limited to `2.000 N m`, and accumulated `0.4 s` of saturation. It therefore cannot use the unsaturated certificate.

A plotted trajectory, source formula, or successful final state alone does not prove the declared certificate. Retained run and phase-portrait artifacts are still required before rendered acceptance.

![Initial Lyapunov values and peak applied torque for certified and observational releases](figures/pendulum-lyapunov-evidence.svg)

[Complete trace](figures/pendulum-lyapunov-evidence-trace.csv) · [Decision summary](figures/pendulum-lyapunov-evidence-summary.csv)

## Conclusion

For a certified release, the measured state produces a bounded feedback torque. The nonlinear plant moves, the positive Lyapunov value decreases, and the state remains inside the fixed sublevel boundary. Because the continuous derivative is negative everywhere away from equilibrium inside that boundary, this is more than one fortunate trace: the mathematical argument covers every state in the declared set under its assumptions.

The phase-plane runs still matter. They verify the executable model, sampling, controller, and calculation at representative states. They do not replace the proof or create a global claim. A recovered 0.25-radian trace shows useful behavior at one initial state; it does not prove all nearby untested states. Saturation changes the feedback law used in the derivative calculation, so the unsaturated certificate must fail closed when requested and applied torque differ.

## Reason for the next lesson

Use this local Lyapunov certificate only for the declared nonlinear model, upright equilibrium, feedback gains, torque limit, sampling assumptions, and $V\le 0.025$ sublevel set. Physical use requires an identified rig, guarding, an independent emergency stop, reviewed torque topology, explicit approval immediately before operation, and rollback. This lesson operates no physical pendulum.

Lesson 70 compares gain scheduling with feedback linearisation on a gravity-loaded robot joint. The need follows directly: the pendulum certificate is deliberately local, while the robot must work across a much wider nonlinear operating range.
