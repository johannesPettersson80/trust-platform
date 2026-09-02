# Classify physical plant responses before modelling

Course position: Lesson 32 of 84. Previous lesson: [Controller
selection](../../04-controller-selection-and-plant-models/31-controller-selection/README.md). Next lesson:
Conservation-law models.

**Lesson objective:** Apply one normalized bounded experiment to four physical
modules and classify each response from measured signatures without fitting a
model.

This shared experiment is simulation evidence. Each physical plant would need
its own approved step, limits, and abort procedure.

## The problem

The equipment in this lesson is Modular skid with heated vessel, buffer tank, conveyor oven, and flexible drive.

**Equipment:** Modular skid with heated vessel, buffer tank, conveyor oven, and
flexible drive.

A modular training skid contains a heated vessel, buffer tank, long conveyor
oven, and flexible mechanical drive. The first seconds of their trends can all
look like “a slow rise,” but the mechanisms are different.

The configured model names remain hidden until the learner commits a
classification.

Lesson 31 could choose controllers because the plant classes were already
known. On unfamiliar equipment, choosing PI or fitting a stable first-order
model from a short trend can mistake delay, integration, or oscillation for an
ordinary slow process.

## What we need

We need a bounded observation that distinguishes equilibrium, continuing drift, delay, and oscillation before any model is fitted. The record must last long enough that a slow or delayed process is not mistaken for a simple low-gain response.

## The new idea

The **operating point** is the steady or defined starting condition. A
**manipulated-input change** is the actuator move imposed by the experiment. A
**process-output change** is the resulting sensor movement. **Control direction**
states whether positive input moves output positively or negatively.

Plants respond dynamically because they store quantities. A tank stores mass,
a heated vessel stores energy, and a flexible drive stores momentum and elastic
energy. We therefore examine **stored mass**, **stored energy**, and **stored
momentum** before naming a curve shape.

A **time constant** describes how quickly a self-regulating response approaches
its steady value. A **continuing slope** means output still changes per second.
**Integration** occurs when unequal rates accumulate in a stored quantity. A
**response delay** is an interval after input changes but before output begins
to move.

The **natural frequency** describes the unforced exchange rate of stored energy.
**Damping** removes energy from that motion. **Resonance** is strong response
near a lightly damped natural frequency. The **observation window** is the time
range used to decide; a window can be too short to reveal the signature.

A self-regulating plant approaches a finite equilibrium and its late slope
approaches zero. An integrating plant retains a nonzero slope under constant
net input. A delayed plant shows no output response initially. An oscillatory
plant crosses its eventual value as stored energy exchanges between states.

## Prediction

One **normalized experiment command** moves from 0 to 1. Structured Text maps
it into a 20-percentage-point heater command, a 10-percentage-point tank
net-inflow trim, a 20-point oven command, and a one-newton-metre drive torque.
The plots retain those physical commands and PV units.

The heated vessel starts at 50 °C. Its gain is 0.2 °C per percent and its time
constant is 3 seconds. The 20-point step predicts **54 degrees** at equilibrium
and **53.93 degrees** after four time constants.

The tank starts at 50 cm. Its integrating gain is 0.1 cm per percent-second, so
10 percent produces 1 cm/s and predicts **62 centimetres after 12 seconds**.

The oven starts at 80 °C. It waits **four seconds**, then approaches 85 °C with
a 2-second time constant. The prediction at 12 seconds is **84.91 degrees**.

The drive has unit static gain, natural frequency 1 rad/s, and damping ratio
0.2. Its predicted first peak has about **52.7 percent overshoot** above the
eventual 1 rad position.

<details>
<summary>Reveal the plant identities</summary>

- The response that approaches a new equilibrium is the heated vessel.
- The response that continues ramping is the integrating tank.
- The response that begins only after a quiet interval is the delayed oven.
- The response with decaying oscillation is the flexible drive.

Reveal this list only after recording the four predictions and classifications.
</details>

## Implementation

There is no feedback controller under test. The actual Structured Text clamps
the approved experiment and converts it into physical commands:

```st
BoundedExperimentInput := MAX(MIN(ExperimentInput, LREAL#1.0), LREAL#0.0);
HeaterCommand := LREAL#20.0 + LREAL#20.0 * BoundedExperimentInput;
TankInflowTrim := LREAL#10.0 * BoundedExperimentInput;
OvenCommand := LREAL#20.0 + LREAL#20.0 * BoundedExperimentInput;
DriveTorque := LREAL#1.0 * BoundedExperimentInput;
```

Do not fit a model and do not tune a controller in this lesson.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run the full 12-second bounded step once, retain exact commands and PVs, classify each trace, and reveal labels only after committing the decision. The declared event lasts 12 seconds. It restores Normalized experiment command to 0 1 afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Vessel temperature (PV, degC) and Tank level (PV, cm); Vessel heater command (MV, %) and Tank net-inflow trim (MV, %), including units, limits, mode, and quality. The initial operating point is Normalized experiment command = 0 (dimensionless), Vessel heater command = 20 %, Vessel temperature = 50 degC, Tank net-inflow trim = 0 %, Tank level = 50 cm, Oven heater command = 20 %, Oven product temperature = 80 degC, Drive torque = 0 N m. The unresolved baseline problem is: A short view makes slow, delayed, and integrating traces appear compatible with the same low-gain stable process assumption.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Observe long enough to measure equilibrium tendency, continuing slope, response delay, overshoot, and crossings before fitting or tuning.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Manual output step**, **Amplitude** = **1 (dimensionless)**, and **Maximum duration (s)** = **12**. The affected signal is **Normalized experiment command**; it starts at 0 (dimensionless) and must restore to 0 (dimensionless).
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Vessel temperature 0 degC to 100 degC; Tank level 0 cm to 100 cm; Oven product temperature 0 degC to 150 degC; Drive position -5 rad to 5 rad. Keep the plant identities hidden until every response has been classified from evidence; reveal them only in the checked answer below.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Normalized experiment command to 0 (dimensionless), wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: vessel_final_temperature >= 53.83 degC (tolerance 0.01 degC); vessel_late_slope_magnitude <= 0.08 degC/s (tolerance 0.01 degC/s); tank_late_slope >= 0.97 cm/s (tolerance 0.01 cm/s); oven_response_delay >= 3.9 s (tolerance 0.1 s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

**Response delay** is time from the applied input step to the first PV change
larger than sensor resolution.

**Late slope** is PV change across the declared final window divided by that
window duration, in PV units per second. A steady value is eligible only when
the late slope is sufficiently close to zero.

**Overshoot** is peak PV beyond an eligible steady value divided by total steady
change, in percent. **Crossing count** counts sign changes around that steady
value after first arrival.

| Module | Evidence | Classification |
| --- | --- | --- |
| Heated vessel | 53.927 °C and near-zero late slope | Self-regulating |
| Buffer tank | 62.000 cm and continuing 1 cm/s slope | Integrating |
| Conveyor oven | 84.908 °C after 4.0 s delay | Delayed self-regulating |
| Flexible drive | 1.527 rad peak and four crossings | Oscillatory |

The plot normalizes each response by its declared physical change so response
shapes can be compared without mixing unlike units. Classification uses the
physical values retained in the run, not the normalized plot alone.

![Equal open-loop steps reveal four different response classes](figures/plant-response-classes.svg)

[Retained run](figures/plant-response-classes-run.csv) ·
[Plot data](figures/plant-response-classes-trace.csv) ·
[Decision summary](figures/plant-response-classes-summary.csv)

### Validation

Require vessel temperature within 0.10 °C of 53.93 °C and late-slope magnitude
below 0.08 °C/s. Require tank late slope within 0.03 cm/s of 1 cm/s. Require
oven response delay of at least 3.9 seconds and final value within 0.15 °C of
84.91 °C. Require drive peak at least 1.40 rad and at least two crossings.

Do not classify from final value alone. A measure is invalid if its input,
window, resolution, or expected steady value is not qualified.

## Conclusion

The vessel approaches balance between heat input and losses. The tank adds net
flow to stored mass. The oven hides heater action during transport. The drive
exchanges mechanical energy while damping removes it. Similar early slopes do
not imply the same dynamics.

## Reason for the next lesson

Real plants can combine these signatures. Classify only what the evidence
supports and extend the experiment safely when the window is ambiguous. Lesson
33 next derives the tank and heater behavior from conservation laws rather than
treating their curves as unexplained shapes.
