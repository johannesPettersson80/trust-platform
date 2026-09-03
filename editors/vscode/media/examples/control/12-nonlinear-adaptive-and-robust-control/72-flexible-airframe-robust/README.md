# Qualify robust load alleviation on a flexible-wing test rig

Course position: Lesson 72 of 84. Previous lesson: [Supervise adaptive pressure control on a polymer extruder](../71-extruder-adaptive/README.md). Next lesson: [Qualify network ownership at a remote pumping station](../../13-networked-fault-aware-and-diagnostic-control/73-networked-pumping/README.md).

**Lesson objective:** Reject an aggressive nominal controller, select a filtered-PD candidate from common worst-case evidence, and state the exact boundary of the resulting robust claim.

**Evidence class:** Simulation evidence, not physical-plant qualification evidence.

## The problem

The equipment is a ground vibration-test flexible-wing load-alleviation rig. A controlled surface moves against a gust-induced bending mode so engineers can study load alleviation before considering a physical or flight implementation.

The wing-root bending response ($y$) is the accelerometer-derived bending signal in g. The desired bending response or setpoint ($r$) is zero g. The control-surface command ($u$) is the controller's requested surface position in percent. The applied control-surface command ($u_{\mathrm{applied}}$) is the position after the physical limit of -15 through +15 percent. The gust-equivalent load ($d$) is an independently generated aerodynamic-load input in g.

Positive surface position increases positive measured bending. Feedback must therefore command a negative surface position when the measured bending is positive. The controller runs every 20 milliseconds, while each simulated plant advances in 5-millisecond substeps.

Two independently restored plant copies receive the same gust. One uses the aggressive nominal controller and the other uses the robust candidate. Their requested and applied control-surface positions remain separate, so saturation cannot be mistaken for controller performance.

The previous lesson estimated a few slowly changing extruder parameters. It updated them only while an authorized experiment supplied enough information. That approach is not suitable for every uncertainty.

A flexible wing can vary in resonant frequency, damping and control-surface effectiveness. It can also contain residual dynamics that the chosen model does not describe individually. Making an adaptive estimator move faster would not prove that those uncertain resonances are safe.

The baseline controller was designed from one nominal model. Its large proportional gain looks attractive because it rejects low-frequency bending strongly. However, the controller also runs through a sampled one-cycle command-to-next-measurement path. In low-damping cases, that delay consumes the remaining damping. The nominal sampled loop becomes marginal or unstable inside the declared model set. We therefore need a design that evaluates the worst requirement over every declared uncertainty case.

## What we need

We need one load-alleviation controller whose worst-case evidence remains acceptable across the declared flexible-wing model set, not merely at the nominal plant. The claim must stop at the tested uncertainty and cannot stand in for flight or certification evidence.

## The new idea

### Two different descriptions of uncertainty

The nominal plant ($P_{0}$) is the best single linear model at the reviewed operating condition.

Structured uncertainty means uncertainty in named physical parameters. Here those parameters are modal frequency, damping and surface effectiveness. Because they have physical names and ranges, we can construct explicit plant cases.

Unstructured uncertainty means a bounded residual whose internal dynamics are not described one by one. Multiplicative uncertainty says that a local plant response may differ by a bounded fraction at each frequency. The local grid plant ($P_i$) is one structured case. The residual-uncertainty weight ($W_{\Delta}$) is the allowed fractional error. The **H-infinity norm** ($H_\infty$ norm) of a stable scalar transfer is its greatest magnitude over all frequencies; a maximum is used, rather than an average, because an average could hide one badly behaved frequency. The normalized unknown residual ($\Delta$) may contain any stable dynamics whose $H_\infty$ norm is no greater than one.

Only after those quantities are defined do we write their relationship:

$P=P_{i} \cdot (1+W_{\Delta} \cdot \Delta)$

This relationship does not say that every real wing lies inside the envelope. The envelope needs independent identification and validation evidence for a physical article.

### Flexible-mode model

Surface motion and gust load accelerate one flexible bending mode. Structural stiffness returns the wing toward zero, while damping removes energy.

The natural angular frequency ($\omega_{n}$) is the undamped modal frequency in radians per second. The damping ratio ($\zeta$) is the dimensionless fraction describing modal energy loss. The control effectiveness ($g_{u}$) is a dimensionless scale on the installed surface-to-bending path. The Laplace frequency variable ($s$) is measured in radians per second.

Dividing the modal balance $\ddot y+2\zeta\omega_n\dot y+\omega_n^2y=0.1g_u\omega_n^2u+\omega_n^2d$ by its characteristic operator gives the two transfer paths. The surface-to-bending plant for structured case i is:

$P_{i}(s)=0.1 \cdot g_{u} \cdot \omega_{n}^{2}/(s^{2}+2 \cdot \zeta \cdot \omega_{n} \cdot s+\omega_{n}^{2})$

Its input is surface position in percent and its output is bending in g. The gust-to-bending plant is:

$P_{\mathrm{d},i}(s)=\omega_{n}^{2}/(s^{2}+2 \cdot \zeta \cdot \omega_{n} \cdot s+\omega_{n}^{2})$

Its input and output are both in g.

The nominal model uses 4.0 rad/s natural angular frequency, 0.08 damping ratio and 1.0 control effectiveness. The structured grid contains 27 cases: natural angular frequency is 3.2, 4.0 or 4.8 rad/s; damping ratio is 0.03, 0.08 or 0.15; and control effectiveness is 0.8, 1.0 or 1.2. These points are executable evidence. A finite grid alone does not prove every intermediate plant.

### The three closed-loop paths

The controller response ($K$) converts measured bending error into requested surface position. The loop transfer ($L$) is the plant response multiplied by the controller response.

The sensitivity ($S$) is the closed-loop path that exposes low-frequency error and disturbance rejection. The complementary sensitivity ($T$) is the path that exposes resonance and multiplicative model uncertainty. The control sensitivity ($KS$) is the path from an error-producing input to control-surface effort.

After defining their meanings, the relationships are:

$L=P_{i} \cdot K$

$S=1/(1+L)$

$T=L/(1+L)$

$KS=K/(1+L)$

These relationships follow by closing the negative-feedback loop. Substitute $u=K(r-y)$ into the plant equation and collect all terms containing $y$; the common denominator is $1+P_iK=1+L$. The remaining numerators define $S$, $T$, and $KS$, so the three paths expose different consequences of the same loop.

Improving one path can worsen another. Large loop gain reduces low-frequency sensitivity but can amplify uncertain resonance and demand excessive surface motion.

### Weights turn preferences into requirements

A weighting function is a frequency-dependent engineering limit. It is not decoration for a Bode plot.

The performance weight ($W_{p}$) requires useful low-frequency disturbance rejection. The residual-uncertainty weight ($W_{\Delta}$) represents an independently supplied 15-percent low-frequency residual that rises toward 75 percent at high frequency. The control-effort weight ($W_{u}$) scales surface demand into the common dimensionless decision.

The declared weights are:

$W_{p}(s)=1.5/(1+s/1)$

$W_{\Delta}(s)=0.15 \cdot (1+s/2)/(1+s/10)$

$W_{u}=0.005\,\text{g}/\%$

At zero frequency, $W_p(0)=1.5$, so requiring $|W_pS|<1$ demands $|S|<1/1.5\approx0.667$ at low frequency — at least 33 percent gust attenuation. $W_p$'s 1 rad/s corner is a **declared requirement**, set below the grid's slowest modal frequency (3.2 rad/s) so the low-frequency demand relaxes before the flexible mode itself. $W_\Delta(0)=0.15$ matches the stated 15-percent low-frequency residual directly; its 2 and 10 rad/s corners are likewise declared, shaping the rise toward 75 percent so that uncertainty is treated as largest near and above the 3.2-4.8 rad/s modal band, where an unmodelled residual is least trustworthy. $W_u=0.005\,\text{g}/\%$ is a declared control-effort price, small enough that surface effort is a soft tie-breaker rather than the dominant term, chosen so it does not mask the performance and robustness requirements it is weighed against. Each weighted path must remain below one. An average is not allowed to hide one failed frequency or plant.

### The mixed-sensitivity score and the small-gain condition

The mixed-sensitivity score ($\gamma$) is the worst of the three weighted paths over every declared plant and frequency:

$\gamma=\max\left(\lVert W_{p} \cdot S \rVert_\infty,\lVert W_{\Delta} \cdot T \rVert_\infty,\lVert W_{u} \cdot KS \rVert_\infty\right)$

The small-gain robust stability condition says that a stable local loop remains stable for every stable normalized residual inside the declared multiplicative envelope when the weighted complementary-sensitivity peak is below one. Because $\gamma$ is the worst of three *separately* evaluated peaks, meeting $\gamma<1$ proves nominal performance on every grid plant plus robust stability — it does not by itself prove robust performance (acceptable performance under the worst-case combination of residual and disturbance together). The standard single-loop robust-performance test instead sums two of those same peaks, $|W_pS|+|W_\Delta T|<1$; for the selected design (Evidence and decision, below) this sum is $0.8333+0.7714=1.6047$, which fails. This lesson therefore selects on nominal performance across the grid plus robust stability, and states plainly that strict robust performance is not established by this design.

This lesson calculates a qualified numerical bound, not an exact continuous-frequency proof. The oracle evaluates 20,000 logarithmically spaced frequencies from 0.01 through 100 rad/s. It retains each peak's frequency and plant. The 20-millisecond sample interval has Nyquist angular frequency $\pi/0.02=157.08$ rad/s; higher frequencies cannot be represented uniquely by these samples and are outside the claim.

### The controller that is actually sampled

The baseline is positional proportional control with proportional gain 40 %/g — the naive design an engineer reaches for first because high gain rejects low-frequency bending strongly (The problem, above); it has no other provenance because its purpose here is to be tested against the declared uncertainty and rejected, not to be trusted.

The robust candidates use proportional gain ($K_p$) in %/g, derivative gain ($K_d$) in %·s/g and measurement-filter time constant ($\tau_f$) in seconds. The finite candidate set is $(8,3,0.05)$, $(10,2,0.05)$, $(10,3,0.05)$ and $(12,3,0.05)$ for $(K_p,K_d,\tau_f)$ — a small **declared design search** around one filter time constant fast enough to pass the flexible mode largely unfiltered (0.05 s, versus the mode's roughly 1.3-2.0 s period) while still attenuating high-frequency sensor content, with proportional gain lowered from the rejected 40 to a range the sampled-pole test can still pass and derivative gain added for damping; it is not derived from a single closed-form design equation the way Lesson 70's scheduled gains were.

The frequency calculation uses the actual 20 ms sampled loop: exact zero-order-hold discretization of each plant, the one-cycle command-to-next-measurement relationship, and the same first-order filter and backward measurement difference used by the Structured Text. It rejects a candidate if a sampled closed-loop pole is on or outside the unit circle.

The implemented robust controller uses `Control.LowPass` followed by derivative-on-measurement `Control.PID`. Integral gain is zero. Derivative setpoint weight is zero, so a setpoint change cannot create derivative kick. Requested output is calculated before the -15 through +15 percent physical limit; applied output is the limited value.

## Prediction

Consider zero frequency on the weakest plant, whose control effectiveness is 0.8. The selected candidate's proportional gain is 10 %/g. The plant's static surface gain is 0.1 g/% multiplied by 0.8.

The low-frequency loop gain is therefore:

$L=0.1 \cdot 0.8 \cdot 10=0.8$

The low-frequency sensitivity is:

$S=1/(1+0.8)=0.556$

The performance weight is approximately 1.5 at this frequency, giving:

$|W_{p} \cdot S|=1.5 \cdot 0.556=0.833$

Predict that this weak-control low-frequency case sets, or nearly sets, the selected design's mixed-sensitivity score.

Also predict that the aggressive nominal proportional controller fails at least one weighted bound and has at least one sampled pole outside the unit circle. Candidate (10,3,0.05) should keep all three weighted peaks below one and every declared sampled pole inside the unit circle.

There is a separate continuous local-stability check. **Normalizing** means folding the plant's own 0.1 g/% static surface gain into the controller coefficients so the loop can be written with one shared plant gain of 1: multiplying candidate (10,3,0.05)'s proportional gain 10 %/g and derivative gain 3 %s/g by 0.1 g/% gives normalized coefficients 1.0 and 0.3. The selected continuous filtered-PD approximation therefore has proportional coefficient 1.0, derivative coefficient 0.3 and filter time constant 0.05 s.

Closing the loop with plant $g_u\omega_n^2/(s^2+2\zeta\omega_ns+\omega_n^2)$, controller $(1.0+0.3s)$, and filter $1/(1+0.05s)$ gives a cubic closed-loop characteristic polynomial $a_3s^3+a_2s^2+a_1s+a_0$ with:

$a_3=0.05,\quad a_2=1+0.1\zeta\omega_n,\quad a_1=2\zeta\omega_n+0.05\omega_n^2+0.3g_u\omega_n^2,\quad a_0=\omega_n^2(1+g_u)$

The cubic Routh-Hurwitz condition for all three roots to stay in the left half-plane is $a_2a_1>a_3a_0$, so it must hold that $a_2a_1-a_3a_0>0$. Expanding that difference term by term gives:

$a_2a_1-a_3a_0=2 \cdot \zeta \cdot \omega_{n}+0.25 \cdot g_{u} \cdot \omega_{n}^{2}+0.2 \cdot \zeta^{2} \cdot \omega_{n}^{2}+0.005 \cdot \zeta \cdot \omega_{n}^{3}+0.03 \cdot g_{u} \cdot \zeta \cdot \omega_{n}^{3}$

Every term in this difference is a product of positive physical parameters ($\zeta$, $\omega_n$, $g_u$ all positive by their declared ranges), so the sum is always positive and the condition holds for every case in the grid. This predicts continuous local stability, but small-gain protects stability against the *unstructured* residual $\Delta$ specifically — a stable local loop can still lose stability if the true plant's unmodelled dynamics exceed the declared $W_\Delta$ envelope, which is what the weighted complementary-sensitivity peak below one is checking. This continuous result does not replace the sampled-pole test or the residual-uncertainty test.

## Implementation

The shipped program runs the nominal and robust controllers on separate equivalent plant copies:

```st
FilteredRobustBending := RobustMeasurementFilter.Update(
    Input := RobustBending,
    CycleTime := LREAL#0.02
);

NominalRequested := LREAL#-40.0 * NominalBending;
RobustRequested := LREAL#-10.0 * FilteredRobustBending
    - LREAL#3.0
        * (FilteredRobustBending - LastFilteredBending) / LREAL#0.02;

NominalApplied := NominalController.Update(Input := NominalInput);
RobustApplied := RobustController.Update(Input := RobustInput);
```

`NominalController` is the positional P baseline. `RobustMeasurementFilter` and `RobustController` implement the selected sampled filtered-PD law. `ControllerEnabled` and `DataHealthy` gate both applied commands. `GustLoad` is copied into both independently bound plant inputs. `NominalBending` and `RobustBending` are separate plant outputs. `NominalRequested`, `NominalApplied`, `RobustRequested`, `RobustApplied`, `NominalSaturated` and `RobustSaturated` expose calculation and physical application without embedding any performance verdict in the PLC program.

## Experiment in Control Studio

This procedure observes the aggressive baseline and selected robust controller on equivalent nominal plants under one project-owned gust. Removing authority, invalid inputs, or a safety abort restores zero gust and zero surface commands. The candidate search, 27-case grid, and quality fault remain native test records. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Wing-root bending setpoint (SP, g); Nominal-loop wing-root bending (PV, g) and Robust-loop wing-root bending (PV, g); Nominal requested surface position (MV, %) and Nominal applied surface position (MV, %), including units, limits, mode, and quality. The initial operating point is Wing-root bending setpoint = 0 g, Nominal-loop wing-root bending = 0 g, Robust-loop wing-root bending = 0 g. The unresolved baseline problem is: Aggressive nominal proportional control ignores the sampled delay and loses damping or stability over the declared flexible-mode set.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept only a candidate whose three weighted paths and sampled poles pass at every declared structured case; retain all peak locations and time-domain costs.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **25.1**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). The project applies the shared +0.10-g gust from 5 through 6 seconds.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Nominal applied surface position -15 % to 15 %; Robust applied surface position -15 % to 15 %; Nominal-loop wing-root bending -1 g to 1 g; Robust-loop wing-root bending -1 g to 1 g.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, remove experiment authority, verify zero gust and both zero applied surfaces, wait for current data, and review a new test.
   - **Record:** Stop recording only after restoration. Retain both bending traces, requested/applied surfaces, saturation, terminal state, restoration result, and seed **20260829**. Use native evidence for frequency qualification, uncertainty corners, and faults; do not claim this visible run executed them.

## Evidence and decision

Classify stability and eligibility before comparing performance. An unstable controller is not allowed to win because one finite time window happens to look quiet.

Maximum weighted sensitivity is the largest $\lvert W_p \cdot S\rvert$ over the declared plants and frequencies. Lower is better and values below one pass the declared performance weight.

Maximum weighted complementary sensitivity is the largest $\lvert W_\Delta \cdot T\rvert$. Lower is better and values below one pass the small-gain residual-uncertainty condition, provided the local loop and residual assumptions are valid.

Maximum weighted control sensitivity is the largest $\lvert W_u \cdot KS\rvert$. Lower is better and values below one pass the declared surface-effort weight.

Peak absolute bending is the maximum absolute wing-root bending response after gust onset, measured in g. Bending integrated absolute error is the sum of absolute bending multiplied by the 0.02-second interval from 5 through 25 seconds, measured in g*s.

Settling time is valid only when absolute bending remains at or below 0.005 g continuously for 2 seconds after the gust ends. Otherwise the result is `Not settled`.

Cumulative applied-surface movement is total variation: the sum of absolute changes between consecutive applied surface samples, measured in percentage points. Also retain peak requested and applied surface position and saturation duration. A requested position beyond 15 percent may be numerically valid while the physical actuator remains limited; the two signals are not interchangeable. Every one of the three weighted peaks, their frequencies and worst plants, the settled-case count, and the worst settling time reported below are retained in [`figures/flexible-airframe-robust-evidence-summary.csv`](figures/flexible-airframe-robust-evidence-summary.csv), which extends the gamma, pole-radius, IAE, peak, movement, and saturation columns.

The native test independently calculates the exact sampled plant, filter and controller responses. It also matches every executed filter, requested command, applied command and saturation state against a separate scan oracle.

The sampled-frequency qualification produced:

| Controller | Maximum weighted sensitivity | Maximum weighted complementary sensitivity | Maximum weighted control sensitivity | Mixed-sensitivity score | Maximum pole radius | Verdict |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Nominal P | 343.053 | 1050.044 | 410.869 | 1050.044 | 1.008104 | Reject: unstable cases and failed bounds |
| Selected filtered PD | 0.8333 | 0.7714 | 0.3117 | 0.8333 | 0.976057 | Eligible on the declared sampled set |

The selected result agrees with the 0.833 low-frequency prediction. Its weighted-sensitivity peak occurs at 0.0100 rad/s on the 4.8 rad/s, 0.03-damping, 0.8-effectiveness plant. Its weighted complementary-sensitivity and control-sensitivity peaks occur at 10.0589 and 23.8105 rad/s on the 4.8 rad/s, 0.03-damping, 1.2-effectiveness plant. Retaining those locations shows which physical corner controls each requirement.

Across the 27 identical gust comparisons, the worst nominal and robust bending integrated absolute errors were 10.0757 and 0.0856 g*s. Worst peak bending was 1.6787 and 0.0790 g. Worst cumulative applied-surface movement was 1228.1 and 4.1 percentage points. Nominal saturation reached 9.30 seconds; the robust controller never saturated. Seventeen of 27 nominal cases were `Not settled`, while every robust case settled and the worst robust settling time was 2.12 seconds.

The separate quality record matched the controller scan oracle, applied exactly zero from both controllers while bending quality was bad, resumed bounded control after restoration, and applied zero after disable. These are Simulation results for the declared ground-rig models. A single favorable gamma, Bode curve or nominal trace still cannot qualify an undeclared plant.

![Worst-case bending error and peak response for the nominal and robust controllers](figures/flexible-airframe-robust-evidence.svg)

Both panels plot the worst case across all 27 structured plants, on a common log-free scale that makes the nominal controller's near-total loss of authority visible next to the robust design's small residual bending.

[Complete trace](figures/flexible-airframe-robust-evidence-trace.csv) · [Decision summary](figures/flexible-airframe-robust-evidence-summary.csv)

## Conclusion

Sensitivity explains low-frequency gust rejection. Complementary sensitivity explains why uncertain resonant dynamics matter. Control sensitivity explains surface effort. The weights state how much of each path is acceptable at each frequency.

The aggressive nominal controller increases low-frequency authority, but its sampled command arrives after the measurement used to calculate it. In a lightly damped flexible mode, that phase delay can turn apparent correction into added oscillation. The robust controller uses less proportional aggression and filtered derivative action to add damping. The filter prevents the derivative calculation from treating high-frequency measurement content as unlimited surface demand.

That improvement has a price. The robust controller can permit more low-frequency sensitivity than the nominal design intended, and derivative action can increase surface movement. That is why the decision retains all three weighted paths and time-domain actuator evidence instead of celebrating one scalar score.

An analytic continuous Routh-Hurwitz result, a sampled frequency-grid qualification and a flight-worthy guarantee are different evidence classes. The first proves a local polynomial property under its assumptions. The second tests the declared numerical sampled set and residual envelope. Neither proves nonlinear aeroelastic behavior, structural integrity, flutter clearance or certification.

## Reason for the next lesson

Use this robust design only inside the stated ground-rig model set, residual envelope, frequency range, sampling, sensor, actuator and operating condition. Undeclared uncertainty is not guaranteed. A model weight with no physical provenance is not safety evidence.

Lesson 73 adds communication latency, jitter, loss, reordering and stale data to a remote pumping station. Those effects become part of the loop dynamics, so a controller qualified on a local deterministic connection is not automatically qualified across a network.
