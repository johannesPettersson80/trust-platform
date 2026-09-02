# Select the minimum sufficient controller

Course position: Lesson 31 of 84. Previous lesson: [Invalid data and safe
recovery](../booster-pump-invalid-data/README.md). Next lesson: Plant response
classes.

**Lesson objective:** Select and defend ON/OFF, P, PI, or PID for four different
real loops and prove each choice on executable plants.

## The problem

The equipment in this lesson is Cold room, recirculating water heater, buffer tank, and loaded positioning table.

**Equipment:** Cold room, recirculating water heater, buffer tank, and loaded
positioning table.

This workshop runs a cold-storage room, recirculating water heater, buffer
tank, and loaded positioning table together. They do not interact. Their
different physics and actuators let us ask the same engineering question four
times: what mechanism is actually needed?

The results are simulation evidence, not physical commissioning evidence.

Earlier lessons developed ON/OFF, P, PI, PD, and PID individually. Selecting
PID everywhere would ignore the binary compressor and add tuning parameters,
dynamic state, and noise sensitivity where simpler control already meets the
requirement.

## What we need

We need to choose the simplest controller whose physical actuation and dynamic mechanisms can satisfy each loop's requirements. Eligibility must come from executable evidence rather than a habit of applying the most complex controller everywhere.

## The new idea

**Controlled-variable accuracy** states how close the measured result must be
to its target. An **acceptable band** permits the value to move inside explicit
limits. **Permanent load error** is an offset that remains after a sustained
disturbance.

A **self-regulating plant** reaches an equilibrium after a fixed input. An
**integrating plant** keeps changing while net input is nonzero. A **damping
requirement** limits oscillation in a plant that stores mechanical energy.

**Actuator resolution** is the smallest output change equipment can realize.
An actuator may be binary or continuous, and one-sided or bidirectional. A
heater is continuous and one-sided; motor torque is continuous and
bidirectional; this compressor is binary.

A **servo objective** concerns setpoint tracking. A **regulatory objective**
concerns disturbance rejection. **Noise sensitivity** describes how sensor
variation creates output activity. Constraints and safety requirements can
make an otherwise attractive structure ineligible.

The **minimum sufficient controller** is the least complex structure that
satisfies every declared requirement with margin. Simplicity never excuses a
failed requirement. Added mechanisms must answer measured needs:

- Integral action is justified by required removal of permanent error.
- Derivative action is justified by a demonstrated damping need.
- Plant integration can supply setpoint integration, but an unknown constant
  load can still create error.

## Prediction

The cold-storage room has a binary compressor and permits 1.5–2.5 °C. ON/OFF
with hysteresis and five-second dwell directly matches that actuator.

The water heater is self-regulating and must return to 30 °C after a constant
heat-loss change. Its gain is 0.2 °C per percent from a 20 °C baseline, so it
needs **50 percent** output to hold **30 degrees** initially. PI can retain that
output at zero error; P cannot, and derivative has no demonstrated purpose.

The buffer tank integrates net flow. Its known equilibrium pump bias is 20
percent. Plant gain is 0.2 centimetres per percent-second and proportional gain
is 2 percent per centimetre, so the predicted closed-loop time constant is:

$tank response time = 1 / (0.2 \cdot 2) = 2.5 seconds$

P control is sufficient for this setpoint case. An unknown changed outflow or
an exact regulatory requirement can justify PI; that boundary is not hidden.

The positioning table must oppose a **four newton-metre** constant load. PID is
selected because derivative provides damping while integral action gives zero
final position error. With proportional gain 20 newton-metres per radian, PD
would require **0.2 radian** error to create that torque.

## Implementation

The structure codes are ON/OFF 1, P 2, PI 3, PD 4, and PID 5. This project
publishes 1, 3, 2, and 5 for the four loops. It is **not an automatic
structure-selection algorithm**; the codes record decisions made from the
requirements above.

The heater, tank, and table each use one real `Control.PID` with only the
selected terms enabled. The cold room uses an explicit binary law:

```st
IF ColdRoomTemperature >= ColdRoomSetpoint + LREAL#0.5 THEN
    CompressorOn := TRUE;
ELSIF ColdRoomTemperature <= ColdRoomSetpoint - LREAL#0.5 THEN
    CompressorOn := FALSE;
END_IF;
```

The shipped code also enforces the five-second dwell condition.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run ON/OFF, PI, biased P, and PID on the four executable plants under their declared events, then change one worksheet requirement at a time without silently changing the plant. The declared event lasts 120 seconds. It restores Cold-room temperature to 4 degC afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Cold-room temperature setpoint (SP, degC) and Heater temperature setpoint (SP, degC); Cold-room temperature (PV, degC) and Heater temperature (PV, degC); Compressor command (MV, %) and Heater power (MV, %), including units, limits, mode, and quality. The initial operating point is Cold-room temperature = 4 degC, Compressor command = 0 %, Heater temperature = 20 degC, Heater power = 0 %, Tank level = 50 cm, Tank inlet-pump command = 20 %, Table position = 0 rad, Table motor torque = 4 N m. The unresolved baseline problem is: PID cannot be realized by the binary compressor and adds unjustified dynamic state or noise sensitivity to loops whose requirements are met by simpler structures.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Require every candidate to pass all declared requirements, then choose the eligible structure with the fewest justified dynamic mechanisms.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **120**. The observed lesson signal is **Cold-room temperature**. Control Studio observes the project's existing timed sequence and does not write a second process event.
   - **Decision before action:** Before selecting **Review test**, predict the response to Heater ambient-equivalent bias changes from 20 to 17 degC at 60 s. Confirm the live values are current and the reviewed plan stays inside Cold-room temperature -10 degC to 20 degC; Compressor command 0 % to 100 %; Heater temperature 0 degC to 60 degC; Heater power 0 % to 100 %; Tank level 0 cm to 100 cm; Tank inlet-pump command 0 % to 100 %.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: cold_room_lower_band_tolerance >= 1.35 degC (tolerance 0.01 degC); cold_room_upper_band_tolerance <= 2.65 degC (tolerance 0.01 degC); heater_final_window_error <= 0.1 degC (tolerance 0.01 degC); tank_final_window_error <= 0.05 cm (tolerance 0.01 cm).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

**Requirement coverage** is the count of declared requirements passed. A
candidate is eligible only if it passes all of them.

**Complexity** counts enabled dynamic mechanisms and retained states. Use it
only to choose among eligible candidates; never use it to excuse failure.

The candidate matrix records all five structures for every plant. It rejects
structurally ineligible choices before tuning and then uses the executed run to
qualify the one selected structure.

[Candidate requirements and rejection matrix](figures/controller-selection-candidates.csv)

For the cold room, measure band-violation time after pull-down, transitions per
hour, and dwell compliance. For the heater and tank, first qualify settling,
then use final-window absolute error and output total variation. For the table,
also measure overshoot and torque total variation.

| Plant | Selection | Executable result | Reason |
| --- | --- | --- | --- |
| Cold room | ON/OFF | 1.492–2.502 °C | Binary actuator and acceptable band |
| Water heater | PI | 0.0159 °C final-window error | Zero load error, no damping need |
| Buffer tank | P | 0.0000 cm final-window error | Plant integrates the declared setpoint case |
| Loaded table | PID | 0.0000 rad final-window error | Damping plus zero constant-load error |

The plot expresses each process value relative to its own setpoint and each
actuator relative to its declared range. It supports comparison without
pretending the physical units are interchangeable. The retained run preserves
every physical value and unit.

![Minimum sufficient structures meet four different requirements](figures/controller-selection.svg)

[Retained run](figures/controller-selection-run.csv) ·
[Plot data](figures/controller-selection-trace.csv) ·
[Decision summary](figures/controller-selection-summary.csv)

### Validation

Require compressor commands of exactly 0 or 100 percent and at least five
seconds between transitions. After pull-down, allow only 0.02 °C numerical
tolerance outside the 1.5–2.5 °C thresholds. Require heater error no greater
than 0.10 °C, tank error no greater than 0.05 cm, and table error no greater
than 0.02 rad in the final 20 seconds. Reject any nonfinite or limited-state
result that invalidates its measure.

## Conclusion

ON/OFF, P, and PI are not incomplete versions of PID. They are correct choices
when actuator capability, plant dynamics, and requirements do not justify more
mechanisms. The loaded table selects PID because its evidence requires all
three terms, not because PID is considered inherently better.

## Reason for the next lesson

Apply the same worksheet to another loop, but do not assume its plant class.
Lesson 32 next teaches plant response classes through equal open-loop tests
before fitting models or selecting a controller.
