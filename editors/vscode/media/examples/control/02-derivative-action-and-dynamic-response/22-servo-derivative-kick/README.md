# Derivative kick and setpoint weighting

Course position: Lesson 22 of 84. Previous lesson:
[Loaded position-table PID control](../../02-derivative-action-and-dynamic-response/21-underdamped-pid/README.md).
Next lesson: [Filtered derivative](../23-servo-filtered-derivative/README.md).

**Lesson objective:** Expose derivative kick, separate requested from applied
torque, and select derivative setpoint weighting that rejects command kick
without removing physical-motion damping.

This lesson uses simulation. Its results are not physical-axis evidence.

**Equipment:** Payload-loaded industrial positioning table.

## The problem

The equipment in this lesson is Payload-loaded industrial positioning table.

The payload-loaded positioning table keeps $K_p$ equal to 8 N m/rad, $K_i$ equal to
2 N m/(rad s), $K_d$ equal to 3 N m s/rad, and its constant 4 N m payload torque.
A commanded position step now changes instantaneously from 0 to 0.5 rad.

The controller's diagnostic request may reach plus or minus 100 N m so the kick
remains visible. The physical drive separately limits applied torque to plus or
minus 10 N m. A discontinuous command is for simulation diagnosis only, not an
approved physical-axis motion profile.

Lesson 21 used derivative action to retain damping while integral action removed
load error. If derivative acts on the full error, however, an instantaneous
setpoint change looks like enormous motion even though the table has not moved.
That mathematical event creates a torque kick. We need to choose what enters the
derivative path without hiding the real drive limit.

## What we need

We need the motion-damping term to react to physical movement without treating an instantaneous setpoint change as infinite motion. Requested and applied torque must remain separate so drive saturation cannot hide the command kick.

## The new idea

The **setpoint ($SP$)** is commanded position and the **process value ($PV$)** is
measured position, both in radians (rad). The **error ($e$)** is SP minus PV.

The **derivative setpoint weight ($c$)** is a dimensionless value from 0 to 1 that
chooses how much setpoint motion enters derivative action. The **weighted
derivative input ($x_{D}$)** is:

$x_{D} = c \times SP - PV$

The **elapsed cycle time ($\Delta t$)** is the time between updates, measured in seconds.
The **derivative input rate ($v_{D}$)** is the change in $x_{D}$ divided by $\Delta t$, measured
in radians per second.

$v_{D} = (x_{D},new - x_{D},old) / dt$

The **derivative gain ($K_d$)** converts that rate into the **derivative torque
($T_{D}$)**, measured in newton metres.

$T_{D} = K_d \times v_{D}$

When c equals 1, the complete SP step enters $x_{D}$. This is **derivative on
error**, and the command discontinuity causes **derivative kick**. When c equals
0, $x_{D}$ is minus PV. This is **derivative on measurement**: an SP step alone
causes no derivative torque, while actual table motion still produces the
opposing torque that adds damping. A weight of 0.5 retains half the command
contribution.

This separation between setpoint and measurement paths makes the controller a
**two-degree-of-freedom PID**: command response can be shaped without changing
the feedback response to physical PV motion.

The **requested motor torque ($T_{\mathrm{requested}}$)** is the PID result before the
physical drive clamp. The **applied motor torque ($T_{\mathrm{applied}}$)** is the bounded
plant input after that clamp.

$T_{\mathrm{applied}} = \operatorname{clamp}\left(T_{\mathrm{requested}}, -10 N m, 10 N m\right)$

The **first post-step request** is $T_{\mathrm{requested}}$ from the first controller update
after SP changes. It is measured in N m and reveals the kick before saturation
hides its size.

The **final-sample position error** is the absolute value of SP minus PV at
15 seconds. It is an endpoint tracking check, not steady-state evidence.

## Prediction

The 0.5-rad SP step occurs over one 0.02-second cycle. With no immediate PV
motion, its apparent rate at c equal to 1 is:

$v_{D} = 0.5 rad / 0.02 s = 25 rad/s$

The derivative torque request is therefore:

$T_{D} = 3 N m s/rad \times 25 rad/s = 75 N m$

At c equal to 0.5 the derivative contribution is 37.5 N m. At c equal to 0 it
is 0 N m. The proportional path also requests about 4 N m from the 0.5-rad
error, while the drive can apply only 10 N m. Predict a large hidden request at
c equal to 1 and a request near 4 N m at c equal to 0.

## Implementation

The executable exposes requested and applied torque separately:

```st
Parameters.DerivativeSetpointWeight := DerivativeSetpointWeight;
RequestedTorque := Controller.Update(Input := Input);
AppliedTorque := MAX(MIN(RequestedTorque, LREAL#10.0), LREAL#-10.0);
```

The first line changes only c. The final line represents the physical drive
limit outside the wide diagnostic controller range.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Restore identical conditions and repeat with c equal to 0.5 and then zero; change no gain, plant, event, timing, limit or seed. The declared event lasts 15 seconds. It restores Table-position setpoint to 0 rad afterward. The deterministic Simulation seed is 20260828.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Table-position setpoint (SP, rad); Measured table position (PV, rad); Applied motor torque (MV, N m), including units, limits, mode, and quality. The initial operating point is PV = 0 rad, MV = 4 N m. The unresolved baseline problem is: Differentiating the full setpoint step creates a requested torque above 70 N m even though the drive can apply only 10 N m.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Select c equal to zero only if the initial requested torque is at most 5 N m, applied torque remains bounded, and final position still tracks within 0.02 rad.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Setpoint step**, **Amplitude** = **0.5 rad**, and **Maximum duration (s)** = **15**. The affected signal is **Table-position setpoint**; it starts at 0 rad and must restore to 0 rad.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Measured table position -4 rad to 4 rad; Applied motor torque -10 N m to 10 N m.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Table-position setpoint to 0 rad, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260828**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: full_error_first_requested_torque >= 70 N m (tolerance 0.01 N m); intermediate_minus_measurement_first_request >= 30 N m (tolerance 0.01 N m); measurement_derivative_first_requested_torque <= 5 N m (tolerance 0.01 N m); applied_torque_absolute_maximum <= 10 N m (tolerance 0.01 N m).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Plot all requested-torque traces on one scale and all applied-torque traces on
another aligned scale. Keep the position responses visible. Report first
post-step torque to 0.01 N m and final position error to 0.001 rad.

The requested trace answers what the controller calculated. The applied trace
answers what the drive and plant actually received. Neither can replace the
other.

![Derivative setpoint handling removes the torque-command kick](figures/derivative-kick.svg)

[Retained samples](figures/derivative-kick-trace.csv) ·
[Decision summary](figures/derivative-kick-summary.csv)

### Validation

The c-equals-1 first request must be at least 70 N m. The c-equals-0.5 request
must exceed the c-equals-0 request by more than 30 N m. With c equal to 0, the
first request must be at most 5 N m and final position error at most 0.020 rad.
Every applied-torque sample must remain within plus or minus 10 N m.

These criteria prove command-kick handling only on the declared simulation.
They do not approve an abrupt command on physical machinery.

## Conclusion

At c equal to 1, differentiation turns the finite command jump divided by a
small cycle time into a large rate, even before the table moves. The drive clamp
hides most of that request from the plant but does not make the controller
calculation harmless or informative.

At c equal to 0, SP is absent from the derivative input. The position step still
enters P and I, while derivative action responds only after PV physically moves.
The damping learned in Lesson 20 remains, but the command discontinuity no
longer creates derivative torque.

## Reason for the next lesson

Derivative on measurement is a robust default for command-following loops.
Setpoint weighting can be chosen deliberately when partial command anticipation
is useful, but requested and applied outputs must remain visible.

The next lesson adds deterministic encoder noise. Derivative on measurement
cannot distinguish real motion from noisy measurement, which creates the reason
for a qualified filter.
