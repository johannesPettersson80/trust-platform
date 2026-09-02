# Supervise adaptive pressure control on a polymer extruder

Course position: Lesson 71 of 84. Prerequisite: nonlinear robot-joint control. Next lesson: robust control of a flexible airframe.

**Lesson objective:** Select fixed, recipe-scheduled or supervised adaptive PI from common extruder evidence and prove that adaptation freezes safely when information or pressure quality is lost.

## The problem

The equipment is a single-screw polymer-extruder die-pressure loop. Its motor moves polymer through a heated barrel and die. Screw-speed command changes material flow and therefore die pressure.

Die-pressure setpoint ($p_{\mathrm{sp}}$) is the desired pressure. Die pressure ($p$) is the pressure measured at the die. Both use bar. Requested screw speed ($u_{\mathrm{requested}}$) is the controller calculation. Applied screw speed ($u_{\mathrm{applied}}$) is the bounded command presented to the simulated motor. Both use percent and are limited from 0 through 100 percent.

The controller executes every 100 milliseconds. The plant advances in 20-millisecond substeps. Resin grade A has nominal pressure gain 1.0 bar/%; resin grade B has nominal pressure gain 0.75 bar/%. The known recipe changes from grade A to grade B at 60 seconds.

Moisture, melt temperature, wear, and blend variation can change pressure gain even within one named recipe. The simulator represents that unmeasured viscosity effect as a continuous multiplier from 0.85 through 1.15 over a 120-second cycle. The effective gain therefore moves from 0.85 through 1.15 bar/% in grade A and from 0.6375 through 0.8625 bar/% in grade B. The controller receives resin grade, measured pressure, and its own applied command. It never receives effective Simulation gain.

A bounded identification probe ($u_{\mathrm{probe}}$) alternates by 10 percentage points around the ordinary controller output. This deliberate movement is applied to all three comparison cases. It is not free and it is not hidden as noise. Physical probing would require an independently reviewed production experiment and explicit approval immediately before it runs. This lesson operates only Simulation equipment.

Every result in this lesson is Simulation evidence. It is not physical-extruder evidence and shall not be presented as approval to excite or control one.

Lesson 70 showed that a few validated controller designs can be scheduled from a measured robot angle. It also showed that an unmeasured payload error remains after nominal model cancellation.

The extruder has a similar split. Resin grade is known, so switching between two reviewed PI designs is reasonable. Effective viscosity is not measured as a scheduling variable and drifts within each grade. Adding schedule points without knowing which point the plant currently occupies does not solve that problem.

We therefore need to estimate a small, explicitly bounded plant model online. The lesson is not trying to prove that adaptation is always better. It asks when an online estimate contains enough trustworthy information to be used and what the controller must do when that evidence disappears.

## What we need

We need die-pressure control that can use known recipe changes and cautiously learn an unmeasured material drift without adapting on bad or uninformative data. Loss of excitation or pressure quality must freeze learning and preserve safe authority.

## The new idea

Screw speed creates an equilibrium pressure through a material-dependent gain. Stored and compressible material prevents pressure from reaching that equilibrium instantly.

Pressure time constant ($\tau_{p}$) states how quickly die pressure approaches its equilibrium and uses seconds. Effective process gain ($K$) states how many bar of equilibrium pressure one percent of applied screw speed produces and uses bar/%. Pressure bias ($B$) represents equilibrium pressure not explained by screw speed and uses bar. Applied screw speed ($u_{\mathrm{applied}}$) uses percent.

After those quantities are defined, the first-order pressure balance is:

$\tau_{p} \cdot dp/dt=K \cdot u_{\mathrm{applied}}+B-p$

This lesson uses a known pressure time constant of 5 seconds. It estimates effective gain and pressure bias. Estimating both matters: if bias were silently fixed at zero, a sensor or operating-point offset could be misreported as gain.

Online parameter estimation means updating a model from synchronized measured input and output while the plant runs. It is not direct access to plant truth.

Controller interval ($\Delta t$) is 0.1 seconds. Previous pressure ($p_{\mathrm{previous}}$) is the preceding pressure sample. Reconstructed equilibrium-pressure observation ($z$) is the pressure target implied by the observed change over one interval. In words, take the previous pressure and add the measured pressure rate multiplied by the known time constant. Then write:

$z=p_{\mathrm{previous}}+\tau_{p} \cdot (p-p_{\mathrm{previous}})/\Delta t$

Previous applied screw speed ($u_{\mathrm{previous}}$) is the command that produced that pressure change. Estimated gain parameter ($\theta_{K}$) represents process gain. Estimated scaled bias parameter ($\theta_{B}$) represents pressure bias divided by 100. Both sides are scaled by 100 to keep PLC arithmetic well conditioned:

$z/100=\theta_{K} \cdot (u_{\mathrm{previous}}/100)+\theta_{B}$

Estimated process gain ($\hat{K}$) equals $\theta_{K}$. Estimated pressure bias ($\hat{B}$) equals 100 times $\theta_{B}$.

Recursive least squares (RLS) is an online estimator that corrects parameters from prediction error and an uncertainty matrix. Regressor vector ($\phi$) contains previous applied speed divided by 100 and a constant one. Scaled observation ($y_{\mathrm{id}}$) is z divided by 100. Innovation ($\varepsilon$) is the measured observation minus the model prediction. Covariance matrix (P) represents remaining parameter uncertainty. Forgetting factor ($\lambda$) states how much older evidence is retained; it is 0.98 here. At a 100-millisecond update interval, that corresponds to an effective evidence horizon of about 5 seconds, short enough to follow the declared slow material drift without treating one sample as truth.

The estimator first calculates estimator gain vector ($L$), then corrects parameter vector ($\theta$), then updates covariance:

$L=P \cdot \phi/(\lambda+\phi^T \cdot P \cdot \phi)$

$\theta_{\mathrm{next}}=\theta+L \cdot \varepsilon$

$P_{\mathrm{next}}=(P-L \cdot \phi^T \cdot P)/\lambda$

A forgetting factor below one allows slow tracking, but it can also let parameters wander when the data contain no new information. Forgetting is not evidence.

Persistent excitation means the measured input has enough independent variation to distinguish gain from bias. A constant screw speed cannot distinguish gain from bias: many identical input rows still describe only one direction in the two-parameter problem.

The estimator retains a two-by-two information matrix ($R$). Each healthy unsaturated sample adds the outer product of the regressor while older information is multiplied by 0.98:

$R_{\mathrm{next}}=0.98 \cdot R+\phi \cdot \phi^T$

Information determinant ($D_{R}$) measures whether the two regressor directions span useful area rather than collapsing onto one line:

$D_{R}=R_{11} \cdot R_{22}-R_{12}^{2}$

Persistent excitation is accepted only when the information determinant is at least 0.10. The common probe alternates by +10 and -10 percentage points every 2 seconds to create enough bounded variation to distinguish both parameters. The resulting pressure and actuator cost remain visible.

Persistent excitation is a data condition, not permission to keep experimenting. The bounded probe window must also be explicitly authorized before an estimate may update. Otherwise, a controller-gain change can move screw speed, the estimator can mistake that self-induced motion for fresh experimental evidence, and adaptation can sustain its own apparent excitation. Removing probe authorization therefore freezes estimation immediately even if old information keeps the determinant above 0.10 for a while.

Parameter projection means restricting a candidate estimate to a reviewed physical set. Estimated gain is limited to 0.5 through 1.5 bar/%. Estimated bias is limited to -20 through +20 bar. ProjectionActive reports when a candidate reaches a boundary; clipping is not presented as successful identification. An estimate sitting exactly on either boundary is not qualified for adaptive controller authority.

Estimator confidence uses covariance trace, which is covariance element $P_{11}$ plus covariance element $P_{22}$. An estimate becomes qualified only after at least 20 informative updates reduce covariance trace to 10.0 or less while data and parameter bounds remain valid. A small covariance trace does not prove the model structure is correct; it only qualifies this declared estimator under this experiment.

Supervised adaptation is the state machine around RLS. Adaptation may update only when it is requested, control is enabled, pressure data are healthy, the previous applied output is not saturated, persistent excitation is present, and the bounded probe window is authorized. Loss of excitation authorization freezes the last qualified estimate. Bad pressure quality freezes estimation and applies the zero-speed safe output. Before qualification, an adaptive request uses recipe-scheduled PI instead of an unqualified estimate.

Indirect adaptive control means estimating a plant parameter and deriving ordinary controller settings from it. Closed-loop design time ($\lambda_{\mathrm{c}}$) is the desired model-based response time and is 3 seconds. Design gain ($K_{\mathrm{design}}$) is fixed at 1.0 for fixed PI, selected as 1.0 or 0.75 for scheduled PI, and taken from qualified projected $K_{\mathrm{hat}}$ for adaptive PI. Proportional gain ($K_p$) uses %/bar. Integral gain ($K_i$) uses %/(bar*s).

$K_p=\tau_{p}/(K_{\mathrm{design}} \cdot \lambda_{\mathrm{c}})$

$K_i=1/(K_{\mathrm{design}} \cdot \lambda_{\mathrm{c}})$

Fixed PI, scheduled PI, and adaptive PI all use the same shared Control.PID function block, zero derivative gain, combined anti-windup, the same probe, and the same applied-speed limits.

## Prediction

For grade A, fixed and scheduled PI both use design gain 1.0 bar/%. They calculate:

$K_p=5/(1.0 \cdot 3)=1.667 \%/bar$

$K_i=1/(1.0 \cdot 3)=0.333 \%/(bar \cdot s)$

For grade B, scheduled PI uses design gain 0.75 bar/%:

$K_p=5/(0.75 \cdot 3)=2.222 \%/bar$

$K_i=1/(0.75 \cdot 3)=0.444 \%/(bar \cdot s)$

If a qualified adaptive estimate is 0.65 bar/%, adaptive PI calculates 2.564 %/bar and 0.513 %/(bar*s). If an update or forced test requests 2.3 bar/%, projection retains 1.5 bar/% and reports the boundary. Predict that removing probe authorization freezes $K_{\mathrm{hat}}$ and $B_{\mathrm{hat}}$ immediately. On the separate constant-grade plant, the information determinant then decays below 0.10 because no deliberate or naturally varying excitation remains.

## Implementation

The shipped Structured Text contains one Control.PID instance. The estimator and supervisor remain visible around it:

```st
InformationDeterminant := Information11 * Information22
    - Information12 * Information12;
PersistentExcitation := InformationDeterminant >= LREAL#0.10;
AdaptationActive := Enabled
    AND AdaptationRequested
    AND DataHealthy
    AND NOT PreviousSaturated
    AND PersistentExcitation;
IF NOT ProbeEnabled THEN
    AdaptationActive := FALSE;
END_IF;

IF AdaptationActive THEN
    Denominator := LREAL#0.98 + Phi1 * PTimesPhi1 + PTimesPhi2;
    EstimatorGain1 := PTimesPhi1 / Denominator;
    EstimatorGain2 := PTimesPhi2 / Denominator;
    CandidateGain := EstimatedGain + EstimatorGain1 * Innovation;
    CandidateBiasScaled := BiasScaled + EstimatorGain2 * Innovation;
END_IF;

ActiveProportionalGain := LREAL#5.0 / (DesignGain * LREAL#3.0);
ActiveIntegralGain := LREAL#1.0 / (DesignGain * LREAL#3.0);
Controller.Configure(Parameters := Parameters);
ScrewSpeedRequested := Controller.Update(Input := Input);
ScrewSpeedApplied := LIMIT(LREAL#0.0, ScrewSpeedRequested, LREAL#100.0);
```

RequestedArchitecture selects fixed, scheduled, or adaptive PI. ActiveArchitecture reports what actually controls the plant. AdaptationRequested controls estimator updates separately from controller selection. AdaptationActive, EstimateFrozen, PersistentExcitation, EstimateQualified, FallbackActive, ProjectionActive, and FrozenReason explain every supervision decision. The program contains no actual effective gain and no built-in performance verdict.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run recipe-scheduled and supervised-adaptive PI under identical plant, setpoint, probe and timing conditions. The declared event lasts 120 seconds. It restores Die-pressure setpoint to 0 bar afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Die-pressure setpoint (SP, bar); Die pressure (PV, bar); Requested screw speed (MV, %) and Applied screw speed (MV, %), including units, limits, mode, and quality. The initial operating point is Die-pressure setpoint = 40 bar, Die pressure = 40 bar, Applied screw speed = 40 %. The unresolved baseline problem is: Fixed PI cannot preserve the same response when known resin grade and unmeasured within-grade pressure gain both change.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use a qualified frozen estimate only while its model, information, bounds and data remain valid; otherwise preserve scheduled fallback or safe output.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **10 bar**, and **Maximum duration (s)** = **120**. The affected signal is **Die-pressure setpoint**; it starts at 40 bar and must restore to 0 bar.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Applied screw speed 0 % to 100 %; Die pressure 0 bar to 90 bar; Estimated process gain 0.5 bar/% to 1.5 bar/%; Estimated pressure bias -20 bar to 20 bar.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Die-pressure setpoint to 0 bar, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Two-parameter recursive least squares with persistent-excitation, projection, confidence, fallback and recovery supervision** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Two-parameter recursive least squares with persistent-excitation, projection, confidence, fallback and recovery supervision** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Two-parameter recursive least squares with persistent-excitation, projection, confidence, fallback and recovery supervision** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Two-parameter recursive least squares with persistent-excitation, projection, confidence, fallback and recovery supervision**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: scheduled_grade_b_iae < 999 bar*s (tolerance 0.001 bar*s); adaptive_gain_rmse <= 0.08 bar/% (tolerance 0.0001 bar/%); frozen_estimate_change <= 1e-12 bar/% (tolerance 0 bar/%); bad_quality_safe_output == 0 % (tolerance 0 %).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Pressure integrated absolute error is the sum of absolute setpoint-minus-pressure error multiplied by the 0.1-second interval, in bar seconds. Compare the declared 60-through-110-second grade-B window under identical probe conditions.

Settling time is valid only after absolute pressure error remains at most 0.5 bar continuously for 5 seconds. A case that does not satisfy this rule reports `Not settled` rather than using its last sample as steady state.

Applied-speed total variation is the sum of absolute changes between consecutive $u_{\mathrm{applied}}$ samples, in percentage points. It exposes the cost of probing and changing controller gains. Also retain peak requested and applied speed, saturation duration, projection count, informative-update count, frozen duration, covariance trace, $D_{R}$, architecture transitions, and exact parameter change during frozen windows.

Estimate root-mean-square error compares $K_{\mathrm{hat}}$ with effective gain reconstructed independently from Simulation configuration over qualified samples. This metric is available only to the native Simulation oracle. A real extruder does not reveal true effective gain, so a field workflow must use independent validation data instead.

### Validation

The shipped native test must match every RLS state and active PI gain against an independent oracle. Estimated gain must remain within 0.5 through 1.5 bar/% and estimated bias within -20 through +20 bar. Covariance and all commands must remain finite.

Under the common record, scheduled PI must improve the grade-B transition over fixed PI. After qualification, adaptive $K_{\mathrm{hat}}$ must have Simulation-only root-mean-square error no greater than 0.08 bar/% and adaptive grade-B pressure error must not exceed scheduled PI error. Any additional probe or adaptation movement remains part of the decision.

When probe authorization is removed, $K_{\mathrm{hat}}$ and $B_{\mathrm{hat}}$ must stop changing immediately within 1e-12. The constant-grade record must then demonstrate loss of persistent excitation. Bad pressure quality must preserve the estimates and apply exactly zero speed. Restored healthy authorized excitation must resume bounded updates. A 2.3 bar/% candidate must project to 1.5 bar/%. Disabled output must be zero.

The focused native Simulation run produced:

| Architecture | Grade-B pressure IAE, 60–110 s (bar*s) | Applied-speed movement, 0–110 s (percentage points) | Settling after the 60-bar command | Saturation (s) |
| --- | ---: | ---: | --- | ---: |
| Fixed PI | 158.457 | 1430.5 | Not settled | 2.8 |
| Recipe-scheduled PI | 134.651 | 1457.9 | Not settled | 2.9 |
| Supervised adaptive PI | 109.792 | 1468.4 | Not settled | 3.0 |

The adaptive estimate's Simulation-only gain root-mean-square error was 0.07350 bar/%, inside the declared 0.08 bar/% limit. The grade transition caused 63 projected samples; an estimate on the 0.5 bar/% gain or 20 bar bias boundary was marked unqualified, so recipe-scheduled PI retained authority until the estimate returned inside its reviewed region.

On the separate constant-grade record, removing probe authorization froze both estimates on the first unauthorized sample. The information determinant subsequently fell from 23.543 at 60 seconds to 0.060 at 89.9 seconds, below the 0.10 eligibility threshold. Unhealthy pressure from 90 through 95 seconds applied exactly zero screw speed, and restored healthy authorized excitation resumed bounded updates by 105 seconds.

The lower adaptive IAE is useful, but it is not a complete win: none of the three probed responses met the five-second settling rule, and adaptive PI used the most applied-speed movement and saturation time. A moving estimate, small covariance number, or attractive pressure trace alone cannot qualify adaptive authority.

![Grade-B pressure error and screw-speed movement for fixed, scheduled, and supervised adaptive control](figures/extruder-adaptive-evidence.svg)

[Complete trace](figures/extruder-adaptive-evidence-trace.csv) · [Decision summary](figures/extruder-adaptive-evidence-summary.csv)

## Conclusion

The known resin-grade transition changes scheduled PI gains because grade identity is a reviewed measurable variable. The unmeasured viscosity multiplier appears instead as model innovation. Probe variation creates independent regressor directions, which raises information determinant. Only then may RLS convert innovation into a bounded gain-and-bias correction.

Once a qualified $K_{\mathrm{hat}}$ becomes $K_{\mathrm{design}}$, the same model-based equations adjust ordinary PI gains. This is indirect adaptation, not a new mysterious controller. Its benefit must be compared with probe-driven screw movement, estimator lag, projection, and model assumptions.

When probe authorization is removed, continuing to update would let controller-induced motion, forgetting, and noise masquerade as experimental evidence. Freezing immediately preserves the last evidenced controller. The later determinant decay independently shows that the constant-grade record no longer distinguishes gain from bias. Bad pressure quality is more severe: neither adaptive nor fallback feedback may safely act on a corrupt pressure value, so the lesson applies zero speed.

## Reason for the next lesson

Use supervised adaptive control only when the model structure, excitation, forgetting, projection bounds, confidence rule, fallback, safe state, restoration, and operating envelope are independently validated. Parameter motion is not proof of learning. Adaptation without information or constraints is a hazard, not an improvement.

A physical extruder also requires reviewed pressure, motor torque, melt temperature, heater, surge, die, product-quality, mechanical, and personnel limits. It requires explicit approval immediately before excitation and a tested rollback. Simulation evidence here does not authorize that action.

Lesson 72 addresses uncertainty that cannot be represented safely as a few slowly drifting parameters. It introduces robust design and worst-case evidence rather than responding with faster unsupported adaptation.
