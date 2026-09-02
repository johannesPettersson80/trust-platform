# Select scheduled or model-linearised robot-joint control

Course position: Lesson 70 of 84. Prerequisite: local Lyapunov analysis of the inverted pendulum. Next lesson: supervised adaptive extruder control.

**Lesson objective:** Select a robot-joint architecture from common nominal and payload-mismatch evidence and explain what scheduling and nonlinear cancellation each require.

## The problem

The equipment in this lesson is Gravity-loaded industrial robot joint.

The equipment is one rigid industrial robot arm axis driven by a bidirectional torque motor. Joint angle setpoint ($q_{\mathrm{sp}}$) is the demanded arm angle. Joint angle ($q$) is the encoder measurement. Angular rate ($\dot q$) states how quickly the joint moves. Angles use radians, rate uses radians per second, and requested and applied motor torque use newton metres.

The simulated arm has inertia 1 kilogram square metre, nominal gravity-torque coefficient 9.81 newton metres, and viscous coefficient 0.2 newton metre seconds per radian. The nonlinear plant advances every 5 milliseconds. The controller executes every 20 milliseconds. Applied torque is limited to -30 through +30 newton metres. The validated range is -1.2 through +1.2 radians and -4 through +4 radians per second.

The nominal and heavy-payload plants use the same controller-facing input and measurements. The heavy case increases actual gravity coefficient to 11.772 newton metres while the controller estimate remains 9.81. Actual payload truth is never supplied to the controller. Every result is Simulation evidence.

Lesson 69 proved a small local region around one upright equilibrium. A production robot cannot remain near one angle. As its arm rotates, gravity torque and the slope of gravity torque change continuously. A single controller tuned near zero angle therefore experiences different effective dynamics at large angles.

The engineering decision is not simply “use nonlinear control.” We need to compare two distinct responses to the limitation. Gain scheduling interpolates controller parameters that were validated at several operating points. Feedback linearisation trusts a nonlinear model enough to cancel its known terms. This lesson exposes what each choice gains and what it risks.

## What we need

We need joint control that remains credible as gravity torque and payload change across a wide angular range. Fixed, operating-point-adjusted, and model-cancelling alternatives must be compared with the same tracking, torque, constraint, and model-trust evidence.

## The new idea

Motor torque accelerates inertia after gravity and viscous friction have been overcome. After those physical effects are named, the joint balance is:

$J \cdot \ddot q+b \cdot \dot q+G \cdot \sin\left(q\right)=\tau_{\mathrm{applied}}$

Inertia ($J$) uses kilogram square metres. Viscous coefficient ($b$) uses newton metre seconds per radian. Gravity-torque coefficient ($G$) and applied torque use newton metres. Joint acceleration ($\ddot q$) uses radians per second squared.

A local model is a linear approximation around one operating angle (q0). The local gravity stiffness is $G \cdot \cos\left(q_{0}\right)$. It changes from its largest value near zero angle to a smaller value as the arm moves toward 1.2 radians. A fixed proportional gain therefore does not produce identical local closed-loop stiffness everywhere.

Choose desired natural frequency ($\omega_{n}$) 5 rad/s and damping ratio ($\zeta$) 0.8. Natural frequency describes the intended response speed. Damping ratio describes how strongly oscillation is suppressed. The local proportional and derivative gains are:

$K_p(q_{0})=J \cdot \omega_{n}^{2}-G \cdot \cos\left(q_{0}\right)$

$K_d=2 \cdot \zeta \cdot \omega_{n} \cdot J-b$

At absolute operating angles 0, 0.6, and 1.2 rad, the proportional gains are 15.190, 16.904, and 21.445 N m/rad. The derivative gain is 7.800 N m s/rad.

Gain scheduling means selecting or interpolating parameters from validated local controller designs using a measurable scheduling variable. Here the scheduling variable is absolute setpoint angle, clamped to 1.2 radians. Linear interpolation connects the three proportional-gain points. Integral gain remains 12 N m/(rad s), and derivative gain remains 7.8 N m s/rad.

Fixed and scheduled PID use the same integral gain. Integral time ($T_i$) is proportional gain divided by integral gain. It therefore ranges from about 1.27 seconds at zero angle to 1.79 seconds at 1.2 radians. This gives integral action enough authority to build the required gravity-holding torque during each plateau while leaving scheduled proportional gain as the only difference between those two records.

Interpolation does not prove stability between the validated points or during a schedule transition. Those conditions must be tested. Scheduling is also not adaptation: these gains do not learn or change from performance data.

Feedback linearisation means using the nonlinear model to cancel known plant terms before applying desired linear error dynamics. The nominal model compensation ($\tau_{\mathrm{model}}$) uses estimated gravity coefficient ($\hat{G}$) and estimated viscous coefficient ($\hat{b}$):

$\tau_{\mathrm{model}}=\hat{G} \cdot \sin\left(q\right)+\hat{b} \cdot \dot q$

With estimated inertia ($\hat{J}$), the complete request is:

$\tau_{\mathrm{requested}}=\tau_{\mathrm{model}}+\hat{J} \cdot (24 \cdot (q_{sp}-q)-8 \cdot \dot q)$

The proportional coefficient is deliberately 24 rather than 25 N m/rad. A
1.2-radian step then initially requests 28.8 N m from feedback, leaving 1.2
N m below the motor's 30 N m limit for model compensation and numerical
margin. Designing exactly on an actuator boundary would make the nominal
architecture saturate before its claimed linear dynamics could be evaluated.

Exact nominal coefficients cancel gravity and viscous friction, leaving the desired linear error response. Feedback is still necessary because initial error, disturbances, sensor error, and model error remain.

Input effectiveness is the coefficient that maps actuator command into state acceleration. It is `1/J_hat` here. Feedback linearisation is eligible because this value stays finite, positive, and bounded throughout the declared range. On a different plant, dividing by input effectiveness that approaches zero would create a singular command. That architecture must be rejected rather than numerically clipped and called successful.

Residual dynamics are the tracking dynamics left after imperfect cancellation. A 20 percent heavy payload creates coefficient error because actual gravity is 11.772 while the controller still cancels 9.81. The resulting residual must remain visible in the comparison.

All three architectures use the shared Control.PID implementation. Fixed and scheduled forms retain integral action. The feedback-linearised form uses model compensation with proportional-derivative feedback and no integral action, specifically so constant model mismatch remains observable.

## Prediction

At measured joint angle 0.5 rad and zero rate, nominal gravity compensation is:

$\tau_{\mathrm{model}}=9.81 \cdot \sin\left(0.5\right)=4.703 N m$

At zero setpoint, scheduled proportional gain is 15.190 N m/rad. At 1.2-radian setpoint, it is 21.445 N m/rad.

When measured angle equals the 1.2-radian setpoint and rate is zero, nominal feedback error is zero. Exact feedback linearisation still requests the holding torque:

The implemented model-torque term is recorded as `tau_model=` before the
feedback correction is added, so the learner can distinguish cancellation from
ordinary error feedback.

$\tau_{\mathrm{model}}=9.81 \cdot \sin\left(1.2\right)=9.143 N m$

With heavy actual gravity but unchanged nominal compensation, the missing torque is:

$\tau_{\mathrm{residual}}=(11.772-9.81) \cdot \sin\left(1.2\right)=1.829 N m$

The no-integral feedback term needs roughly $1.829/24=0.076 rad$ of error to balance that residual. Predict a visible load-dependent offset for feedback linearisation. Predict that scheduled PID integral action eventually removes the constant residual, although it may move more slowly.

## Implementation

The shipped program has one shared Control.PID instance. The scheduling law and model term remain visible around it:

```st
SchedulingVariable := MIN(ABS(JointSetpoint), LREAL#1.2);

IF ActiveArchitecture = INT#1 THEN
    ActiveProportionalGain := ScheduledProportionalGain;
    Parameters.IntegralGain := LREAL#12.0;
    Input.FeedForward := LREAL#0.0;
ELSIF ActiveArchitecture = INT#2 THEN
    ActiveProportionalGain := LREAL#24.0 * EstimatedInertia;
    Parameters.IntegralGain := LREAL#0.0;
    Input.FeedForward := ModelCompensation;
END_IF;

Controller.Configure(Parameters := Parameters);
TorqueRequested := Controller.Update(Input := Input);
TorqueApplied := LIMIT(LREAL#-30.0, TorqueRequested, LREAL#30.0);
```

ControllerMode requests fixed PID, scheduled PID, or feedback linearisation. ActiveArchitecture states what actually runs. ModelEligible requires bounded positive inertia and bounded gravity and friction estimates. An ineligible model requested for cancellation activates explicit scheduled fallback. FixedProportionalGain, ScheduledProportionalGain, and ActiveProportionalGain show exactly what is used. GravityCompensation contains only the nominal sine-gravity term; ModelCompensation adds nominal viscous friction. Neither uses actual payload truth. TorqueRequested and TorqueApplied remain separate, with TorqueSaturated showing any limit action.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run scheduled PID and eligible feedback linearisation separately under the same nominal and mismatch conditions. The declared event lasts 31 seconds. It restores Joint angle setpoint to 0 rad afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Joint angle setpoint (SP, rad); Joint angle (PV, rad); Requested motor torque (MV, N m) and Applied motor torque (MV, N m), including units, limits, mode, and quality. The initial operating point is Joint angle setpoint = 0 rad, Joint angle = 0 rad, Joint angular rate = 0 rad/s, Applied motor torque = 0 N m. The unresolved baseline problem is: A tuning derived at zero angle has different effective stiffness and transient behavior as gravity curvature changes across the arm range.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Select scheduled PID when validated local designs and integral load rejection are stronger evidence than the cancellation model; select feedback linearisation only while model and input-effectiveness eligibility remain qualified.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **1.2 rad**, and **Maximum duration (s)** = **31**. The affected signal is **Joint angle setpoint**; it starts at 0 rad and must restore to 0 rad.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Applied motor torque -30 N m to 30 N m; Joint angle -1.5 rad to 1.5 rad; Joint angular rate -4 rad/s to 4 rad/s.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Joint angle setpoint to 0 rad, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Comparison of fixed PID, gain-scheduled PID, and eligible feedback linearisation** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Comparison of fixed PID, gain-scheduled PID, and eligible feedback linearisation** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Comparison of fixed PID, gain-scheduled PID, and eligible feedback linearisation** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Comparison of fixed PID, gain-scheduled PID, and eligible feedback linearisation**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: maximum_applied_torque <= 30 N m (tolerance 0.000001 N m); scheduled_heavy_payload_final_error <= 0.02 rad (tolerance 0.0001 rad); nominal_linearised_segment_settling <= 4 s (tolerance 0.02 s); invalid_model_fallback == 1 (dimensionless) (tolerance 0 (dimensionless)).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Segment integrated absolute angle error is the sum of absolute setpoint-minus-angle error multiplied by the 20-millisecond interval, in radian-seconds. Overshoot is the greatest movement beyond the commanded target divided by the magnitude of that segment's commanded change, in percent.

Settling time is valid only when absolute angle error is at most 0.02 rad and absolute angular rate is at most 0.05 rad/s continuously for the final 0.5 seconds of the plateau. An ineligible segment reports `Not settled` instead of a final-error ranking.

Applied-torque total variation is the sum of absolute consecutive applied-torque changes, in newton metres. Also retain peak requested and applied torque, saturation duration, and final-window mean absolute error over the final 0.5 seconds of each plateau. Requested and applied torque, nominal and heavy payload, requested and active architecture, model eligibility, and fallback state remain distinct in every table and plot.

### Validation

The active scheduled gain and nominal model compensation must match independent equations to `1e-9`. Applied torque must remain within -30 through +30 N m. On the nominal plant, scheduled PID should make wide-angle segment behavior more consistent than the fixed zero-angle tuning, while eligible feedback linearisation must meet every segment tolerance without saturation.

On the heavy plant, scheduled PID integral action must keep final-window mean absolute error at or below 0.02 rad on both nonzero plateaus. The unchanged no-integral feedback-linearised controller must expose a materially larger residual rather than using actual plant truth to hide it. Invalid estimated inertia must select scheduled fallback. Disabled mode must apply zero torque.

The shipped native execution produced the following common-trajectory results.
Values are rounded for reading; the test retains full precision.

| Plant and architecture | Integrated absolute error | Applied-torque travel | Peak applied torque | Saturation |
|---|---:|---:|---:|---:|
| Nominal, fixed PID | 3.028 rad s | 173.2 N m | 18.52 N m | 0.00 s |
| Nominal, scheduled PID | 3.028 rad s | 208.4 N m | 26.02 N m | 0.00 s |
| Nominal, feedback linearisation | 1.631 rad s | 264.2 N m | 28.80 N m | 0.00 s |
| Heavy payload, fixed PID | 3.614 rad s | 177.1 N m | 18.52 N m | 0.00 s |
| Heavy payload, scheduled PID | 3.616 rad s | 211.4 N m | 26.02 N m | 0.00 s |
| Heavy payload, feedback linearisation | 2.917 rad s | 264.3 N m | 28.80 N m | 0.00 s |

On the nominal nonzero plateaus, fixed PID settles in 3.14 and 3.22 seconds,
scheduled PID in 4.66 and 4.76 seconds, and feedback linearisation in 1.02
seconds for both directions. Across all four plateaus, the settling-time spread
is 0.92 seconds for fixed PID, 0.70 seconds for scheduled PID, and 0.02 seconds
for feedback linearisation. Scheduling makes behavior somewhat more consistent,
but it does not improve total integrated error and it increases torque travel.

On the heavy-payload nonzero plateaus, scheduled PID's final-window mean
absolute errors are 0.00189 and 0.00207 rad. Feedback linearisation retains
0.07380 rad on both plateaus, agreeing closely with the predicted 0.076 rad
model-mismatch offset. The fixed and scheduled return-to-zero segments are
still `Not settled` after five seconds on the heavy plant, so their final
samples are not ranked as steady-state evidence. Nominal feedback-linearised
overshoot remains below 0.38 percent; no architecture saturates in these
records.

The evidence therefore selects feedback linearisation for the qualified
nominal model when speed matters and model eligibility is continuously
enforced. It selects scheduled PID when rejecting an uncertain constant
payload torque is more important than nominal speed. Source equations or a
single attractive trace cannot make that decision.

![Tracking error and motor-torque movement for fixed, scheduled, and feedback-linearised control](figures/robot-joint-nonlinear-evidence.svg)

[Complete trace](figures/robot-joint-nonlinear-evidence-trace.csv) · [Decision summary](figures/robot-joint-nonlinear-evidence-summary.csv)

## Conclusion

The fixed controller changes behavior because actual local gravity stiffness changes while its gain does not. Scheduled PID changes only its proportional gain according to the bounded setpoint schedule; integral feedback still supplies the unknown holding torque. Feedback linearisation instead supplies the nominal holding torque immediately and asks feedback to regulate the remaining error.

With the nominal model, cancellation can produce simple and fast dynamics. With the heavy payload, the missing gravity torque becomes residual dynamics and creates the predicted offset because the selected linearised controller deliberately has no integral action. Scheduled PID can integrate that constant residual away. This does not mean scheduling is universally better or that model-based control is unsafe. It means the faster cancellation architecture requires stronger evidence that its model stays accurate enough for the operating envelope.

## Reason for the next lesson

Choose gain scheduling when a finite operating envelope and several local designs can be validated without trusting complete nonlinear cancellation. Choose feedback linearisation when the nonlinear model and input effectiveness are qualified and a bounded fallback exists. Physical robot operation also requires payload identification, collision and travel limits, guarding, independent stop functions, explicit approval immediately before motion, and rollback. This lesson operates no physical robot.

Lesson 71 introduces constrained adaptation for an extruder whose material parameters drift continuously rather than occupying a small set of validated operating regions.
