# Compare direct, static, and dynamic distillation control

**Equipment:** Continuous binary distillation column.

Course position: Lesson 62 of 84. Prerequisite: measured distillation interaction and Relative Gain Array pairing. Next lesson: multivariable robustness and architecture selection on the same column.

This project operates local nominal and perturbed simulations. It does not operate a physical distillation column.

**Lesson objective:** Commission the paired PI loops and select direct, static, or realizable dynamic decoupling from equivalent nominal and model-mismatch evidence.

## The problem

The equipment in this lesson is Continuous binary distillation column.

This is the same local binary distillation column measured in Lesson 61. Reflux and reboiler steam regulate overhead and bottoms product quality, but each actuator affects both products.

The overhead purity setpoint ($SP_{1}$) is the desired overhead light-product purity deviation, in composition percentage points. Its process value ($PV_{1}$) is the measured deviation in the same unit. The bottoms purity setpoint ($SP_{2}$) and its process value ($PV_{2}$) describe bottoms heavy-product purity deviation.

The virtual loop corrections (v1 and v2) are the two feedback controllers' requested actions before cross compensation. The requested reflux and reboiler changes are the physical commands calculated by the selected architecture, in valve percentage points. The applied changes are those commands after the -10 through +10 percentage-point actuator limits. Requested and applied values remain separate throughout the experiment.

Lesson 61 selected the diagonal pairing, but its relative gains were 1.2727 rather than one. It also measured unequal 20, 35, 40, and 30-second path response times. Correct pairing avoids the negative effective gains of the cross pairing; it does not remove interaction. We now have measured reasons to test decoupling rather than adding it because it sounds more advanced.

## What we need

We need to reduce cross-coupling between the two commissioned purity loops without creating a fragile or unrealizable correction. Direct, steady inverse, and dynamic alternatives must face the same nominal and model-mismatch events.

## The new idea

### Two decentralized feedback loops

Decentralized control uses two ordinary feedback controllers on the same coupled plant. The first controller compares SP1 with PV1. The second compares SP2 with PV2. The overhead error (e1) and bottoms error (e2) are, in words, desired purity deviation minus measured purity deviation:

$e_1=SP_1-PV_1$

$e_2=SP_2-PV_2$

Both are parallel, positional `Control.PID` instances with derivative action disabled, so they operate as proportional-integral controllers. They use a 0.1-second cycle interval, explicit initialization, finite virtual-output limits, rate limits, tracking time, and combined anti-windup. Their tuning never changes between architectures.

### Static decoupling

A static decoupler is a steady linear map from the two virtual corrections to the two physical actuator changes. Divide each measured matrix row by its diagonal gain to preserve the direct loop gains. The normalized interaction matrix ($M$) is:

$\mathbf{M}=\begin{bmatrix}1&-0.5\\-0.428571&1\end{bmatrix}$

The physical-input vector ($u$) contains reflux and reboiler changes. The virtual-correction vector ($v$) contains v1 and v2. The static mapping is:

$\mathbf{u}=\mathbf{M}^{-1}\mathbf{v}$

With the Lesson 61 steady gain matrix ($G$), the predicted product is:

$\mathbf{G}\mathbf{M}^{-1}=\begin{bmatrix}0.8&0\\0&0.7\end{bmatrix}$

The off-diagonal actuator command is feedforward from the other controller's demand. Feedback remains active to remove residual error. Matrix inversion is rejected when the measured matrix is missing, invalid, singular, or poorly conditioned.

### Why dynamic compensation may be needed

Static cancellation uses final gains. It can cancel the final cross effect while leaving a transient because direct and cross contributions arrive at different rates.

A dynamic decoupler shapes each cross command with a causal transfer function. The Laplace frequency variable ($s$) describes how the relationship changes with frequency; s is the Laplace frequency variable, not seconds. The measured path ratios require:

$D_{12}(s)=0.5\frac{20s+1}{35s+1}$

$D_{21}(s)=0.428571\frac{30s+1}{40s+1}$

Both ratios are proper and causal because the numerator order is no greater than the denominator order. Each is implemented as a direct part plus a first-order filtered part:

$D_{12}(s)=0.285714+\frac{0.214286}{35s+1}$

$D_{21}(s)=0.321429+\frac{0.107143}{40s+1}$

An inverse requiring prediction, an improper transfer function, uncertain unstable-zero cancellation, excessive high-frequency gain, or unavailable fresh signals is not eligible. Dynamic decoupling adds model, noise, and timing dependence.

The signal path is:

![Distillation decoupling signal path](figures/distillation-decoupling-signal-path.svg)
## Prediction

For an overhead-only setpoint increase, direct control should move bottoms purity through the reflux-to-bottoms path. Static decoupling should remove the final cross offset but retain a transient. Dynamic decoupling should reduce that nominal transient by matching the unequal path dynamics. The bottoms-only experiment should show the corresponding behavior in the other direction.

When the cross gains or response times change, model mismatch should make exact dynamic cancellation degrade. A more elaborate design is accepted only if that degradation remains within the declared requirements.

## Implementation

The shipped program contains two real `Control.PID` instances, one `Control.StaticDecoupler2x2`, and two stateful `Control.LowPass` instances. One architecture selector chooses direct, static, or dynamic mapping. It does not change the PI settings, plant event, timing, limits, or random seed.

The dynamic implementation is short enough to trace from the preceding equations:

```st
RefluxRequested := VirtualCommand.First
    + LREAL#0.285714285714286 * VirtualCommand.Second
    + LREAL#0.214285714285714 * FilteredBottomsCorrection;
ReboilerRequested := VirtualCommand.Second
    + LREAL#0.321428571428571 * VirtualCommand.First
    + LREAL#0.107142857142857 * FilteredOverheadCorrection;
```

The filters update continuously so a selected dynamic path does not begin with stale state. Structured Text does not contain cross-peak measurements, integral-error results, or a winning architecture.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Select static and then dynamic mapping without changing either PI tuning, plant event, timing, limits, or metric windows. The declared event lasts 220 seconds. It restores Overhead purity setpoint to 0 pp afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Overhead purity setpoint (SP, pp) and Bottoms purity setpoint (SP, pp); Overhead light-product purity deviation (PV, pp) and Bottoms heavy-product purity deviation (PV, pp); Requested reflux change (MV, pp) and Requested reboiler steam change (MV, pp), including units, limits, mode, and quality. The initial operating point is Overhead purity setpoint = 0 pp, Bottoms purity setpoint = 0 pp, Applied reflux change = 0 pp, Applied reboiler steam change = 0 pp, Applied feed-composition disturbance = 0 pp. The unresolved baseline problem is: The RGA-selected diagonal PI loops remain coupled because each physical actuator still moves both products. Control Studio Monitor must show Simulation mode and Auto authority, with requested and applied inputs visible separately. Control Studio Monitor must show Simulation mode and Auto authority, with requested and applied inputs visible separately. Execute the overhead-only event from 0 to +2 and restore it before starting the bottoms-only event. Control Studio Monitor must show Simulation mode and Auto authority, with requested and applied inputs visible separately. Execute the overhead-only event from 0 to +2 and restore it before starting the bottoms-only event. Repeat both events under the feed-composition disturbance and the perturbed interaction model.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use the simplest architecture whose nominal and perturbed executed evidence satisfies the declared cross-response, primary-loop, effort, limit, and restoration requirements.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **2 pp**, and **Maximum duration (s)** = **220**. The affected signal is **Overhead purity setpoint**; it starts at 0 pp and must restore to 0 pp.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Overhead purity setpoint -4 pp to 4 pp; Bottoms purity setpoint -4 pp to 4 pp; Applied reflux change -10 pp to 10 pp; Applied reboiler steam change -10 pp to 10 pp; Overhead light-product purity deviation -6 pp to 6 pp; Bottoms heavy-product purity deviation -6 pp to 6 pp.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Overhead purity setpoint to 0 pp, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Decentralized PI with qualified static and dynamic decoupling** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Decentralized PI with qualified static and dynamic decoupling** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Decentralized PI with qualified static and dynamic decoupling** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Decentralized PI with qualified static and dynamic decoupling**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: final_window_combined_absolute_error <= 0.05 pp (tolerance 0.001 pp); dynamic_to_static_nominal_cross_peak_ratio < 1 (dimensionless) (tolerance 0.001 (dimensionless)); nominal_cross_peak <= 0.05 pp (tolerance 0.001 pp); nominal_servo_combined_iae <= 100 pp*s (tolerance 0.001 pp*s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Define the measures before inspecting results:

- **Primary-loop integral absolute error** is the sum of absolute setpoint-minus-process-value error over the event window, multiplied by the 0.1-second sample interval, in composition-percentage-point seconds.
- **Cross-output peak excursion** is the largest absolute unintended product deviation during a one-product setpoint test, in composition percentage points.
- **Final-window absolute error** is the mean combined absolute error over the final 10 seconds and is reported only for a valid settled record.
- **Requested and applied input total variation** is the sum of absolute changes between consecutive command samples, in valve percentage points. The two values are reported separately.
- **Saturation duration** is elapsed time at a physical input limit, in seconds.

For each servo record, cross peak must not exceed 0.05 composition percentage points, combined integral absolute error must not exceed 100 composition-percentage-point seconds, and combined applied travel must not exceed 6 valve percentage points. Feed-disturbance combined error must not exceed 40 composition-percentage-point seconds.

On the perturbed model, select dynamic decoupling only if it materially improves the result: it must lower cross peak by at least 20 percent relative to static, increase combined integral absolute error by no more than 5 percent, increase applied travel by no more than 25 percent, and remain otherwise eligible.

### Validation

The native executable produced these nominal results:

| Event and measure | Direct | Static | Dynamic |
|---|---:|---:|---:|
| Overhead step: cross peak | 0.185 | 0.043 | 0.00005 |
| Overhead step: combined error integral | 73.16 | 50.26 | 55.44 |
| Bottoms step: cross peak | 0.244 | 0.122 | 0.00016 |
| Bottoms step: combined error integral | 99.99 | 75.45 | 79.50 |
| Feed disturbance: combined error integral | 22.59 | 31.30 | 38.50 |

Cross peaks use composition percentage points. Error integrals use composition-percentage-point seconds. Dynamic compensation is the only nominal architecture meeting the 0.05 cross-peak requirement in both directions. It does not beat static on combined servo error, and direct control rejects this particular feed disturbance best. All three nevertheless meet the separate declared error and effort limits.

On the perturbed overhead test, direct, static, and dynamic cross peaks were 0.1568, 0.0678, and 0.0594 composition percentage points. Static therefore retained a substantial improvement over direct control. Static and dynamic combined error integrals were 54.77 and 64.00 composition-percentage-point seconds, and applied travel was 4.29 and 4.07 percentage points. Dynamic reduced cross peak by only 12.3 percent and increased combined error by 16.8 percent. It therefore failed the predeclared material-improvement rule. Static decoupling is the selected fallback for the declared uncertain model; the nominal dynamic result is retained as a qualified local result, not generalized.

### Retained evidence

The upper plot verifies that all three architectures control the requested
overhead change. The lower plot isolates the unwanted bottoms-composition
movement that decoupling is intended to reduce. The retained trace also keeps
the manipulated variables and the perturbed-model comparison.

![Direct, static, and dynamic decoupling](figures/distillation-decoupling-evidence.svg)

[Complete trace](figures/distillation-decoupling-evidence-trace.csv) ·
[Decision summary](figures/distillation-decoupling-evidence-summary.csv)

## Conclusion

Direct control lets each controller correction travel through both physical paths. Static inversion adds an opposing command that cancels the final cross contribution, but unequal arrival times leave a transient. The dynamic direct-and-filtered terms align those nominal contributions and nearly remove the cross peak.

Model mismatch changes the gain and arrival time of the real cross path while the feedforward compensation remains nominal. The cancellation then becomes incomplete. Feedback eventually trims residual error, but it cannot make the inverse model correct. This is why excellent nominal dynamic cancellation does not by itself justify the more complex architecture.

## Reason for the next lesson

Use decoupling only when a qualified interaction model and common executed evidence show that it passes a requirement the simpler architecture misses. Static compensation addresses settled interaction. Eligible dynamic compensation can address transient interaction, but carries more model and timing risk.

This is simulation evidence. Physical-column application requires independently reviewed models, topology, product and safety limits, product disposition, abort and restoration procedures, and explicit approval immediately before action.

Lesson 63 asks why these successful single-loop and scenario checks still do not prove multivariable robustness for the coupled architecture.
