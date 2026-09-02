# Place bearing modes and observe motor load

Course position: Lesson 65 of 84. Prerequisite:
`control-two-tank-observability`. Next lesson: `control-weigh-feeder-kalman`.

**Equipment:** Active magnetic-bearing training rig and loaded conveyor motor.

**Lesson objective:** Calculate and execute bearing feedback and motor observer gains, validate their modes and limits, and expose observer bias under declared plant-model mismatch.

## The problem

The equipment in this lesson is Active magnetic-bearing training rig and loaded conveyor motor.

This lesson contains two linked real-world benches. The active magnetic-bearing rig measures rotor displacement in millimetres and rotor velocity in millimetres per second. Its bidirectional actuator is coil-current deviation in amperes. The local model is valid only within -0.5 to +0.5 millimetres and -2 to +2 millimetres per second. Applied current is limited to -2 to +2 amperes.

The loaded conveyor motor measures shaft speed with an encoder in radians per second. Drive torque is measured in newton metres and limited to -3 to +3 newton metres. Physical load torque is not instrumented. The simulator exposes it as a **Simulation-only truth channel** so you can check the observer; the controller never reads that channel.

Both models run every 0.1 seconds. They omit coil electrical dynamics, nonlinear magnetic force, drive delay, shaft flexibility, nonlinear friction, and encoder quantisation. Zero coil current is the safe output but cannot levitate a physical rotor, so real equipment also needs independent touchdown bearings.

![State-feedback and observer signal paths](figures/state-feedback-observer-signal-path.svg)
The first loop uses two measured states. The second uses one measured output and a model to reconstruct an unmeasured state.

Lesson 64 proved that installed actuators and sensors can move and reveal the required model directions. That does not choose the closed-loop dynamics.

Position-only feedback can change the magnetic bearing's stiffness, but it cannot independently choose displacement and velocity behavior. Measured velocity is therefore added to place both modes. The motor then creates the next limitation: its load state can be inferred from speed history but is not measured. Assuming zero load leaves a speed error, which creates the reason for an observer.

## What we need

We need feedback from the bearing's dynamic state and a separate estimate of conveyor speed and load where direct measurement is unavailable. Measured state, estimated state, limits, and model-mismatch bias must remain visibly distinct.

## The new idea

### Bearing state feedback

The bearing state vector ($\mathbf{x}_{\mathrm{b}}$) contains rotor displacement and rotor velocity. The bearing input ($\mathbf{u}_{\mathrm{b}}$) is applied coil-current deviation in amperes. The bearing state matrix ($\mathbf{A}_{\mathrm{b}}$) describes the open-loop motion, and the bearing input matrix ($\mathbf{B}_{\mathrm{b}}$) describes current-to-acceleration action:

$\mathbf{A}_b=\begin{bmatrix}0&1\\4&0\end{bmatrix}$

$\mathbf{B}_b=\begin{bmatrix}0\\1\end{bmatrix}$

Velocity changes displacement. Near the chosen centre point, displacement produces an acceleration away from centre, so the uncontrolled model is unstable. One ampere produces one millimetre per second squared of acceleration in this scaled training model.

The feedback-gain row ($\mathbf{K}_{\mathrm{b}}$) contains the displacement and velocity weights used to oppose motion. The reference gain ($\mathbf{N}_{\mathrm{b}}$) scales a constant requested displacement so the model reaches that displacement at equilibrium. The bearing reference ($\mathbf{r}_{\mathrm{b}}$) is the requested displacement. Only after defining these quantities do we write:

$u_{b,\mathrm{requested}}=N_b r_b-\mathbf{K}_b\mathbf{x}_b$

Applied bearing current is the requested value limited to -2 through +2 amperes.

Position-only gain 5 produces $s^{2} + 1 = 0$. Its poles are `+j` and `-j`, so the ideal response oscillates and does not settle. Desired poles at -3 and -4 per second produce $s^{2} + 7 \cdot s + 12$. Matching this polynomial to the model gives:

$\mathbf{K}_b=\begin{bmatrix}16&7\end{bmatrix}$

$N_b=12$

From an initial +0.1-millimetre displacement at zero reference, the first requested current is $-16 \cdot 0.1 = -1.6 A$. From centred conditions with a +0.1-millimetre reference, it is $12 \cdot 0.1 = +1.2 A$. Both predictions are inside the current limit.

### Motor state observation

The motor state vector ($\mathbf{x}_{m}$) contains shaft speed and opposing load torque. The measured motor output ($\mathbf{y}_{m}$) is encoder speed. The motor state matrix ($\mathbf{A}_{m}$), input matrix ($\mathbf{B}_{m}$), and output matrix ($\mathbf{C}_{m}$) are:

$\mathbf{A}_m=\begin{bmatrix}-1&-1\\0&0\end{bmatrix}$

$\mathbf{B}_m=\begin{bmatrix}1\\0\end{bmatrix}$

$\mathbf{C}_m=\begin{bmatrix}1&0\end{bmatrix}$

Speed naturally decays, positive load reduces acceleration, drive torque increases acceleration, and load remains constant during a nominal record.

The estimated state ($\hat{\mathbf{x}}$) is the observer's speed and load prediction. Innovation ($\nu$) is measured encoder speed minus estimated speed. Observer gain ($\mathbf L$) states how strongly that innovation corrects the two state predictions. Applied motor torque ($u_{m,\mathrm{applied}}$) is the torque that actually entered the plant. The observer is:

$\dot{\hat{\mathbf{x}}} =\mathbf{A}_m\hat{\mathbf{x}}+\mathbf{B}_m u_{m,\mathrm{applied}} +\mathbf{L}\nu$

$\nu=y_m-\mathbf{C}_m\hat{\mathbf{x}}$

Desired observer-error poles at -4 and -5 per second give
$\mathbf{L}=\begin{bmatrix}8&-20\end{bmatrix}^{T}$. The load correction is
negative because positive physical load reduces speed. Forward Euler at 0.1
seconds maps the two error poles to 0.6 and 0.5, inside the discrete unit circle.

The motor speed-feedback gain ($K_m$) is 2 newton metres per radian per second. The motor reference gain ($N_m$) is 3 newton metres per radian per second. The motor reference ($r_m$) is desired speed, estimated speed ($\hat{\omega}$) is the observer's speed state, and estimated load ($\hat d$) is its load state:

$u_{m,\mathrm{requested}} =N_m r_m-K_m\hat{\omega}+\hat{d}$

At a 1-radian-per-second reference with 0.5 newton metres of load, assuming zero load predicts 0.8333 radians per second. A converged nominal observer predicts 1.0000 radians per second.

The separation principle says feedback and observer-error poles can be selected separately for an exact linear model without active limits. It does not prove robustness, state truth, noise performance, or saturated behavior.

## Prediction

Before running, calculate the position-only poles, $K_{\mathrm{b}}$, $N_{\mathrm{b}}$, $L$, both continuous and discrete observer-error poles, and both motor equilibria.

Predict that position-only bearing feedback is **Not settled**, while state feedback restores the initial displacement and tracks the reference. Predict that nominal load estimation removes the motor bias. For a motor whose real damping is 1.2 while the observer retains 1.0, predict good speed regulation but a load estimate about 0.2 newton metres above physical load near 1 radian per second.

## Implementation

The shipped Structured Text calculates both designs directly:

```st
BearingCurrentRequested := LREAL#12.0 * BearingSetpoint
    - LREAL#16.0 * BearingDisplacement
    - LREAL#7.0 * BearingVelocity;
BearingCurrentApplied := LIMIT(LREAL#-2.0, BearingCurrentRequested, LREAL#2.0);

MotorInnovation := MotorSpeed - EstimatedMotorSpeed;
EstimatedMotorSpeed := EstimatedMotorSpeed + LREAL#0.1 * (
    -EstimatedMotorSpeed - EstimatedLoadTorque + PreviousMotorTorque
    + LREAL#8.0 * MotorInnovation
);
EstimatedLoadTorque := EstimatedLoadTorque
    + LREAL#0.1 * (LREAL#-20.0 * MotorInnovation);
MotorTorqueRequested := LREAL#3.0 * MotorSetpoint
    - LREAL#2.0 * EstimatedMotorSpeed + EstimatedLoadTorque;
MotorTorqueApplied := LIMIT(LREAL#-3.0, MotorTorqueRequested, LREAL#3.0);
```

The observer uses torque applied during the preceding scan because that input produced the newly measured motor state. The program contains no pole calculator, settling verdict, accepted-design label, PID instance, or use of `MotorLoadTruth` in its calculations.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Use measured bearing state feedback and the nominal speed/load observer without changing the declared plants or limits. The declared event lasts 20 seconds. It restores Bearing displacement to 0 mm afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Bearing displacement reference (SP, mm) and Motor speed reference (SP, rad/s); Bearing displacement (PV, mm) and Measured motor speed (PV, rad/s); Requested bearing current (MV, A) and Requested motor torque (MV, N m), including units, limits, mode, and quality. The initial operating point is Bearing displacement reference = 0 mm, Motor speed reference = 1 rad/s, Applied bearing current = 0 A, Applied motor torque = 0 N m, Simulation truth motor load torque = 0.5 N m. The unresolved baseline problem is: Position-only bearing feedback does not settle, and a motor controller that assumes zero load retains speed error.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept the nominal gains only after executed mode, estimation, limit, and mismatch evidence agrees with the reviewed model.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Observe without excitation**, **Amplitude** = **0 mm**, and **Maximum duration (s)** = **20**. The affected signal is **Bearing displacement**; it starts at 0 mm and must restore to 0 mm.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Bearing displacement -0.5 mm to 0.5 mm; Bearing velocity -2 mm/s to 2 mm/s; Applied bearing current -2 A to 2 A; Measured motor speed -2 rad/s to 2 rad/s; Applied motor torque -3 N m to 3 N m.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Bearing displacement to 0 mm, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **State-feedback pole placement, reference gain, and Luenberger observation** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **State-feedback pole placement, reference gain, and Luenberger observation** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **State-feedback pole placement, reference gain, and Luenberger observation** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **State-feedback pole placement, reference gain, and Luenberger observation**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: bearing_state_feedback_settling_time <= 4 s (tolerance 0.1 s); bearing_final_displacement_error <= 0.002 mm (tolerance 0.0001 mm); zero_load_baseline_speed_error >= 0.1 rad/s (tolerance 0.001 rad/s); nominal_observer_speed_error <= 0.02 rad/s (tolerance 0.001 rad/s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Bearing settling time is the first time after which absolute displacement error is at most 0.002 millimetres and absolute velocity is at most 0.005 millimetres per second for two continuous seconds. A response that never meets both conditions is **Not settled**. Final bearing error is the mean absolute displacement error over the final two seconds and is reported only for a settled record.

Motor final speed error is the final five-second mean absolute difference between reference and encoder speed. Nominal load-estimation error is the final five-second mean absolute difference between estimated load and Simulation truth. Estimation settling time requires speed-estimation error at most 0.01 radians per second and load-estimation error at most 0.02 newton metres for two continuous seconds.

For each record also retain peak displacement or speed, peak innovation, peak requested and applied output, time at the applied limit, and cumulative applied-output movement calculated as the sum of absolute changes between consecutive samples.

### Validation

The position-only bearing record must be **Not settled**. State-feedback regulation and reference tracking must settle within 4 seconds, finish within 0.002 millimetres, and avoid sustained current limiting.

The zero-load motor baseline must retain at least 0.10 radians per second final speed error. The nominal observer must settle its estimates within 3 seconds, finish within 0.02 radians per second speed error and 0.02 newton metres load error, and avoid sustained torque limiting. The damping-mismatch record must remain bounded while retaining at least 0.10 newton metres load-estimation error.

The shipped executable lesson produced this Simulation evidence:

| Record | Eligibility and response | Final error | Applied-output evidence |
|---|---|---:|---:|
| Bearing, position only | Not settled | Not valid for a steady-state metric | Bounded inside the declared local range |
| Bearing, state regulation | Settled in 1.70 s; peak displacement 0.1000 mm | 0.00000 mm | Peak 1.6000 A; cumulative movement 1.7298 A |
| Bearing, +0.1 mm reference | Settled in 1.70 s | 0.00000 mm | No sustained limiting |
| Motor, zero-load assumption | Valid bounded baseline | 0.1667 rad/s speed error | Inside -3..+3 N m |
| Motor, nominal observer | Estimates settled in 2.00 s | 0.0000 rad/s speed error; 0.0000 N m load error | Peak 3.0000 N m; cumulative movement 2.2239 N m |
| Motor, damping mismatch | Bounded output regulation | 0.0000 rad/s speed error; 0.2000 N m physical-load error | Same reviewed torque limit |

The nominal innovation peak was 0.2379 radians per second. The mismatch row is intentionally not accepted as physical load measurement: its good speed result and wrong load estimate are the lesson's required warning.

The comparison table is populated only from the shipped executable records. Simulation evidence must never be presented as physical-bearing, motor, encoder, or load-torque evidence.

### Retained evidence

![Bearing state feedback and loaded-motor observer comparison](figures/magnetic-bearing-state-feedback-evidence.svg)

The figure shows the two decisions made in this lesson: full-state feedback stabilizes the bearing, and the observer improves load rejection only inside its model qualification. The [trace CSV](figures/magnetic-bearing-state-feedback-evidence-trace.csv) retains measured states, estimates, innovation, and requested/applied commands. The [summary CSV](figures/magnetic-bearing-state-feedback-evidence-summary.csv) records settling, error, effort, and mismatch results.

## Conclusion

Velocity feedback changes the damping term of the bearing polynomial, which is why position gain alone cannot place both modes. The motor innovation corrects both estimated speed and estimated load; its negative load-gain sign follows the physical convention that positive load reduces speed.

With the nominal model, the observer error follows the selected -4 and -5 modes. With extra real damping, the encoder cannot distinguish that missing damping from extra opposing load inside the observer's simpler model. Speed can still regulate while the reported load is wrong. The observer reconstructs model states, not truth, and its output is not a sensor.

## Reason for the next lesson

Use state feedback only after actuator and state feasibility are qualified. Use an observer only after observability, model validity, discretisation, limits, noise, and mismatch have been checked. A successful output trace does not prove an estimated physical quantity is accurate.

All results here are Simulation evidence. A physical magnetic-bearing or motor experiment needs independently reviewed topology, calibrated scaling, mechanical containment, current and overspeed protection, explicit approval immediately before action, and verified abort and restoration.

Lesson 66 introduces the Kalman filter because a deterministic observer does not express how process uncertainty and sensor uncertainty should be balanced.
