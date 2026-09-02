# Design and qualify active-suspension LQR/LQG

Course position: Lesson 67 of 84. Prerequisite: `control-weigh-feeder-kalman`.
Next lesson: `control-building-hvac-mpc`.

**Equipment:** Reduced body-bounce mode of an active road-vehicle suspension.

**Lesson objective:** Derive and compare pole-placement and four LQR designs on common physical measures, then qualify balanced position-only LQG under nominal noise and declared faults.

**Evidence class:** Simulation evidence, not physical-plant qualification evidence.

## The problem

The equipment in this lesson is Reduced body-bounce mode of an active road-vehicle suspension.

The equipment is the locally linear body-bounce mode of one corner of an active road-vehicle suspension. Suspension deflection ($z$) says how far the body moves relative to its local rest position, in metres. Body vertical velocity ($v$) says how quickly that deflection changes, in metres per second. Hydraulic force ($u$) is the bidirectional actuator command, in kilonewtons. Road-equivalent vertical acceleration disturbance ($d$) represents the bump force reaching the body mode, in metres per squared second.

Velocity changes deflection. Spring and damper action oppose body motion, while hydraulic force and the road disturbance change vertical velocity. In equations:

$\dot{z}=v$

$\dot{v}=-4z-1.2v+0.5u+d$

The state vector ($x$) contains deflection and velocity: $x=[z,v]^T$. State matrix ($A$) describes the unforced spring and damper. Actuator matrix ($B$) describes hydraulic-force influence. Disturbance matrix ($E$) describes road influence:

$\mathbf{A}=\begin{bmatrix}0&1\\-4&-1.2\end{bmatrix},\qquad \mathbf{B}=\begin{bmatrix}0\\0.5\end{bmatrix},\qquad \mathbf{E}=\begin{bmatrix}0\\1\end{bmatrix}$

Requested and applied force are separate. Applied hydraulic force is limited to -5 through +5 kilonewtons. The local model is valid only while absolute deflection is at most 0.08 metres, absolute velocity at most 0.50 metres per second, and absolute road acceleration at most 2 metres per squared second. The controller runs every 0.1 seconds.

![Active-suspension LQR and LQG signal path](figures/suspension-lqr-lqg-signal-path.svg)
Two equivalent simulated copies receive the same applied force and road event. One supplies clean validation state. The other supplies a position measurement with deterministic 0.002-metre standard-deviation noise. Its velocity is retained as **Simulation truth** for validation, but position-only LQG does not use it.

This is a reduced educational body-mode model. It omits tyre and wheel motion, hydraulic dynamics, delay, pitch, roll, structural modes, nonlinear geometry, friction, leakage, sensor dynamics, and real road spectra.

Lesson 65 showed that pole placement can assign a stabilizing pole set, but the chosen poles did not state how the engineer valued several state errors relative to actuator effort. Lesson 66 showed how a model and covariance can reconstruct an unmeasured state without pretending that the estimate is truth.

Active suspension needs both decisions. Many gains stabilize this local model. We need a reproducible reason for preferring one tradeoff, and we need a way to use that gain when only noisy displacement—not velocity—is measured.

## What we need

We need to turn explicit priorities for body motion, suspension travel, and hydraulic effort into comparable feedback designs. A position-only estimate must then preserve the balanced design under declared sensor noise and faults without being called universally optimal.

## The new idea

A state penalty assigns cost to unwanted deflection or velocity. An input penalty assigns cost to hydraulic force. State-weight matrix ($Q$) contains the state penalties. Input-weight matrix ($R$) contains the force penalty.

Quadratic performance cost ($J$) accumulates both kinds of penalty over time:

$J=\int_{0}^{\infty} \left(\mathbf{x}^{T}\mathbf{Q}\mathbf{x} +\mathbf{u}^{T}\mathbf{R}\mathbf{u}\right)dt$

Larger state weight asks the design to suppress that state more strongly. Larger input weight asks it to spend less actuator effort. A weight is not a physical limit, and a low cost is not permission to violate a limit.

### Units and normalization

Raw numerical weights depend on engineering units. A value written in millimetres is numerically one thousand times the same displacement written in metres, so reusing an unchanged weight would change the design.

Normalize deflection by 0.05 metres, velocity by 0.25 metres per second, and force by 5 kilonewtons before expressing a preference. Transform the normalized preference back into dimensional Q and R before solving. Unit conversion without the matching weight transformation is a configuration error.

### From cost to feedback

Riccati matrix (P) relates the present state to future optimal cost—the least cost achievable by the declared unconstrained linear problem. First confirm that actuator force can stabilize every unstable state. The continuous algebraic Riccati equation then finds P:

$\mathbf{A}^{T}\mathbf{P}+\mathbf{P}\mathbf{A} -\mathbf{P}\mathbf{B}\mathbf{R}^{-1}\mathbf{B}^{T}\mathbf{P} +\mathbf{Q}=0$

Linear-quadratic regulator gain ($K$) converts state into requested force:

$\mathbf{K}=\mathbf{R}^{-1}\mathbf{B}^{T}\mathbf{P}$

$u_{\mathrm{requested}}=-\mathbf{K}\mathbf{x}$

This solution is optimal for the declared linear model, quadratic cost, complete state, and unconstrained calculation. Saturation breaks the unconstrained optimum. Two candidates designed with different Q and R also have different private cost definitions, so their private J values cannot rank one another. Compare them using the same physical measures.

The independent Riccati calculation gives:

| Design | Q diagonal | R | Gain K | Intended emphasis |
|---|---:|---:|---:|---|
| Body-motion | $[20,20]$ | 1 | $[1.16515139,3.11548779]$ | vertical velocity |
| Suspension-travel | $[120,2]$ | 1 | $[5.56465997,3.07892689]$ | deflection |
| Balanced | $[40,4]$ | 1 | $[2.19803903,1.90722139]$ | both states |
| Effort-saving | $[40,4]$ | 5 | $[0.48528137,0.51566896]$ | hydraulic effort |

Pole-placement baseline gain $[13,6]$ is stable but does not expose a cost preference. It is not an LQR candidate.

### From full state to position-only LQG

The controller samples every 0.1 seconds. Sampled state-transition matrix ($\mathbf{A}_{\mathrm{d}}$) advances state without a new input. Sampled actuator matrix ($\mathbf{B}_{\mathrm{d}}$) advances state due to the previous applied force. Sampled disturbance matrix ($\mathbf{E}_{\mathrm{d}}$) advances state due to the previous road disturbance:

$\mathbf{A}_d=\begin{bmatrix}0.98084004&0.09360616\\-0.37442462&0.86851265\end{bmatrix}$

$\mathbf{B}_d=\begin{bmatrix}0.00239499\\0.04680308\end{bmatrix}$

$\mathbf{E}_d=\begin{bmatrix}0.00478999\\0.09360616\end{bmatrix}$

Measured position ($y$) is the noisy sensor sample. Predicted state estimate ($x_{\mathrm{prior}}$) is the state expected from the previous corrected estimate and applied inputs. Innovation ($\nu$) is measured position minus predicted position. Estimator gain ($L$) says how much of that surprise corrects deflection and velocity.

Process-noise covariance ($W$) declares model uncertainty. Measurement-noise covariance ($V$) declares position-sensor uncertainty. For $W=diag(1e-7,1e-4)$ and $V=4e-6$, the independent discrete Riccati calculation gives $L=[0.58488259,2.24682847]^T$.

$\hat{\mathbf{x}}_{k\mid k-1} =\mathbf{A}_d\hat{\mathbf{x}}_{k-1\mid k-1} +\mathbf{B}_d u_{k-1}+\mathbf{E}_d d_{k-1}$

$\nu_k=y_k-\begin{bmatrix}1&0\end{bmatrix}\hat{\mathbf{x}}_{k\mid k-1}$

$\hat{\mathbf{x}}_{k\mid k} =\hat{\mathbf{x}}_{k\mid k-1}+\mathbf{L}\nu_k$

The balanced LQR gain then uses corrected estimated state instead of measured full state. That combination is linear-quadratic-Gaussian control, or LQG.

The separation principle says regulator and estimator poles can be designed separately for the exact nominal linear assumptions. It does not guarantee robustness to wrong dynamics, wrong covariance, delay, saturation, outliers, or an unsafe actuator.

## Prediction

Before running, use the common worked state of 0.03 metres deflection and zero velocity. Requested force is minus the first gain multiplied by 0.03:

| Design | Predicted requested force (kN) |
|---|---:|
| Pole placement | -0.39000 |
| Body-motion LQR | -0.03495 |
| Suspension-travel LQR | -0.16694 |
| Balanced LQR | -0.06594 |
| Effort-saving LQR | -0.01456 |

Predict that suspension-travel emphasis reduces deflection relative to effort-saving control, while effort-saving control consumes less force energy. Predict that the body-motion design reduces velocity relative to effort-saving control. Finally, predict that nominal LQG approaches balanced full-state behavior only while the estimator's model and noise assumptions remain credible.

## Implementation

The shipped Structured Text selects the declared gains, distinguishes requested from applied force, and bounds both hydraulic and road inputs. In position-only mode, force is calculated from estimated state:

```st
Innovation := PositionOnlyMeasurement - PredictedDeflection;
EstimatedDeflection := PredictedDeflection
    + LREAL#0.58488259 * Innovation;
EstimatedVelocity := PredictedVelocity
    + LREAL#2.24682847 * Innovation;

ForceRequested := -GainDeflection * FeedbackDeflection
    - GainVelocity * FeedbackVelocity;
ForceApplied := LIMIT(LREAL#-5.0, ForceRequested, LREAL#5.0);
```

Design modes are 0 pole placement, 1 body motion, 2 suspension travel, 3 balanced, 4 effort saving, and 5 balanced position-only LQG. An invalid selector or invalid local-range input sets `ControllerValid` false and applies zero force. Disabled mode also applies zero hydraulic force.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Execute four independently solved LQR gains, select from common physical measures, then combine the balanced gain with the position-only Kalman estimator. The declared event lasts 12 seconds. It restores Requested road-equivalent acceleration to 0 m/s2 afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Full-state suspension-deflection measurement (PV, m) and Full-state body-velocity measurement (PV, m/s); Requested hydraulic force (MV, kN) and Applied hydraulic force (MV, kN), including units, limits, mode, and quality. The initial operating point is Requested hydraulic force = 0 kN, Applied hydraulic force = 0 kN, Requested road-equivalent acceleration = 0 m/s2, Applied road-equivalent acceleration = 0 m/s2. The unresolved baseline problem is: Pole placement stabilizes the local mode but does not encode a reproducible preference among suspension travel, body motion and hydraulic effort. Use Control Studio Monitor before Tune/Design, then execute Test and finish in Validate on common physical measures.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Select a regulator only from common physical tradeoff measures, then accept position-only LQG only when estimator and abnormal-case evidence remain inside declared bounds.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **mv_pulse**, **Amplitude** = **0.5 m/s2**, and **Maximum duration (s)** = **12**. The affected signal is **Requested road-equivalent acceleration**; it starts at 0 m/s2 and must restore to 0 m/s2.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Applied hydraulic force -5 kN to 5 kN; Applied road-equivalent acceleration -2 m/s2 to 2 m/s2; Full-state suspension-deflection measurement -0.08 m to 0.08 m.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Requested road-equivalent acceleration to 0 m/s2, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Linear-quadratic regulation and position-only linear-quadratic-Gaussian control** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Linear-quadratic regulation and position-only linear-quadratic-Gaussian control** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Linear-quadratic regulation and position-only linear-quadratic-Gaussian control** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Linear-quadratic regulation and position-only linear-quadratic-Gaussian control**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: maximum_applied_hydraulic_force <= 5 kN (tolerance 0.001 kN); maximum_absolute_suspension_deflection <= 0.08 m (tolerance 0.0001 m); nominal_lqg_position_estimation_rms <= 0.01 m (tolerance 0.0001 m); nominal_lqg_velocity_estimation_rms <= 0.1 m/s (tolerance 0.001 m/s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Suspension-deflection root-mean-square value is the square root of mean squared deflection from 1 through 12 seconds, in metres. Body-velocity root-mean-square value uses the same calculation and window, in metres per second. Body-acceleration root-mean-square value is calculated from the declared state equation, in metres per squared second.

Hydraulic-force energy is $\sum\left(u_{\mathrm{applied}}^{2} \cdot 0.1\right)$ over 1 through 12 seconds, in squared-kilonewton seconds. Force movement is total variation, the sum of absolute applied-force changes, in kilonewtons. Also retain peak absolute force and saturation duration.

Return time begins after the road pulse. It is the first point for which the next continuous second remains within 0.002 metres deflection and 0.01 metres per second velocity. Report **Not returned** if that prerequisite is never met.

For LQG, position- and velocity-estimation root-mean-square errors compare estimated state with synchronized Simulation truth from 1 through 12 seconds. Innovation mean over the same window checks persistent position prediction bias. Retain source signals, units, window, sample count, mode, model, seed, and evidence class with every value.

### Validation

The native executable test must independently solve all four continuous Riccati designs and the discrete estimator Riccati equation before comparing gains. Every regulator pole and estimator pole must lie inside its applicable stability boundary. Every run must remain finite and inside force and local-model limits.

The suspension-travel candidate must reduce deflection RMS relative to effort-saving control. The body-motion candidate must reduce velocity RMS relative to effort-saving control. Effort-saving control must use less force energy than body-motion and suspension-travel control. Balanced control is eligible only when it avoids saturation and occupies a defensible common-measure tradeoff.

Nominal position-only LQG must keep position-estimation RMS within 0.01 metres, velocity-estimation RMS within 0.10 metres per second, and absolute innovation mean within 0.002 metres. Its deflection RMS may be no more than 1.5 times balanced full-state control, and its force energy no more than twice the balanced value. Force movement remains a separate actuator-wear measure. Doubled noise must not appear better than nominal in position-estimation error, and model mismatch must not appear better in velocity-estimation error.

The shipped native execution produced the following common-measure results. Every row used the same 0.5-metre-per-squared-second road pulse, initial state, interval, duration, force limit, and seed. All rows remained finite, unsaturated, and inside the local model range.

| Design | Deflection RMS ($m$) | Velocity RMS (m/s) | Acceleration RMS (m/s^{2}) | Force energy (kN^{2}·s) | Force movement (kN) | Peak force (kN) | Return time (s) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Pole placement | 0.0077 | 0.0197 | 0.0963 | 0.2638 | 1.991 | 0.825 | 1.6 |
| Body-motion LQR | 0.0145 | 0.0261 | 0.1005 | 0.0759 | 1.117 | 0.406 | 2.8 |
| Suspension-travel LQR | 0.0120 | 0.0264 | 0.1071 | 0.1218 | 1.383 | 0.544 | 2.3 |
| Balanced LQR | 0.0156 | 0.0305 | 0.1109 | 0.0503 | 0.911 | 0.335 | 3.5 |
| Effort-saving LQR | 0.0207 | 0.0383 | 0.1213 | 0.0054 | 0.312 | 0.099 | 5.5 |
| Balanced position-only LQG | 0.0156 | 0.0304 | 0.1104 | 0.0520 | 1.952 | 0.330 | 3.5 |

Pole placement suppresses this one nominal pulse most strongly, but spends more than five times the balanced force energy. Suspension-travel weighting reduces deflection relative to effort-saving control; body-motion weighting reduces velocity; effort-saving control minimizes force energy but returns most slowly. Balanced LQR is not “best” in every column. It is a measured compromise on the common tradeoff.

Nominal LQG position- and velocity-estimation RMS were 0.00127 metres and 0.00511 metres per second. Innovation mean was 0.000058 metres. Its motion and force energy closely reproduce balanced full-state control, but force movement rises from 0.911 to 1.952 kilonewtons because noisy position enters the estimate. That actuator-wear cost remains visible rather than being hidden by the LQG label.

Doubling position-noise standard deviation increased position- and velocity-estimation RMS to 0.00254 metres and 0.01021 metres per second. The spring/damping mismatch produced 0.00132 metres and 0.00590 metres per second, with innovation mean 0.000191 metres. Both are separately labelled abnormal **Simulation** evidence; neither changes the nominal qualification automatically. A Riccati solver success flag, a smooth plot, or a low private cost is not sufficient evidence.

### Retained evidence

![Active-suspension LQR response and force tradeoffs](figures/suspension-lqr-evidence.svg)

The figure compares the same road event under four unit-scaled costs. The [trace CSV](figures/suspension-lqr-evidence-trace.csv) also retains the pole-placement baseline, balanced LQG estimates, innovation, requested/applied force, saturation, doubled-noise case, and model mismatch. The [summary CSV](figures/suspension-lqr-evidence-summary.csv) records the comfort, travel, effort, peak, and return-time measures used in the design decision.

## Conclusion

The four LQR gains differ because their Q and R values express different preferences. The Riccati equation does not discover whether comfort, travel, or energy matters most; it solves the preference the engineer supplied. This is why unit scaling and common physical comparison measures are essential.

Hydraulic saturation matters because the Riccati result assumed unlimited force. Once requested and applied force differ, the unconstrained optimum no longer describes the executed system.

Position-only LQG predicts hidden velocity from the model, previous applied force, previous road disturbance, and noisy position. Estimation error therefore enters requested force. A smooth position estimate does not prove velocity is correct. Clean synchronized velocity exists here only for Simulation validation.

## Reason for the next lesson

LQR makes a multi-state-versus-effort preference explicit and reproducible, but optimal means optimal only for the declared model, cost, complete-state assumption, and unconstrained calculation. LQG combines that regulator with a covariance-qualified estimator; it does not remove model, noise, delay, or constraint risk.

Physical use requires an identified full-order vehicle and hydraulic model, calibrated sensing, structural and force limits, independent topology and functional-safety review, explicit approval immediately before physical testing, and rollback. This lesson operates no physical vehicle.

Lesson 68 introduces building-HVAC model-predictive-control because several future constraints and disturbances cannot be represented by one unconstrained instantaneous state-feedback calculation.
