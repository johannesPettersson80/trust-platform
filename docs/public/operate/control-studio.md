# Control Studio

Control Studio identifies a process, tunes a controller for it, checks the
result on your actual ST program and applies it to the running controller. It
works on any truST project: the loop's names, units and ranges come from your
program and your setup, never from built-in assumptions.

It has three pages: **Identify**, **Tune** and **Validate**. **Apply** is the
last action on Validate.

## Set up a loop

Open **Control Studio** from the truST sidebar. A project without a loop offers
**Configure control loop** (also in the command palette) and example projects.

The setup form reads the compiled project, including its library
dependencies. For each `Control.PID` it fills in what the program's own calls
say: the parameter record passed to `Configure`, and the setpoint,
measurement, manual output and output of `Update`. When the program has
several PIDs, choosing one fills in its own signals; fields you have chosen
are kept. The form recognises a cascade when one PID's output feeds another
PID's setpoint, and fills in both controllers.

1. Choose the **Structure**: **One PID loop**, **Cascade**, **PID group**
   (when the program has several PIDs), **ON/OFF**, **State feedback**,
   **Own P, PI or PID**, **Other law** or **Identify only, no controller**. **Step-by-step setup…**
   covers the rest: recording a project's own experiment, an ON/OFF controller
   with a BOOL output, a law with reset or derivative times, a derivative filter or other retained states,
   and imported controller mappings.
2. Confirm or choose each signal. The output and the setpoint may be program
   variables without an I/O address. The measurement is an `%I` input, or a
   variable the program only reads: validation's simulated plant writes it, so a
   value the program computes itself is refused (choose the measurement it is
   computed from). The manual output needs an `%M` address, because
   identification writes it through the process image.
3. Enter each signal's unit, minimum and maximum; the program does not know
   them, and tuning results are in these units.
4. Choose the **Auto/Manual signal**, what TRUE means, and the **Manual
   output**. Control Studio writes new settings only while the controller is
   confirmed in Manual, which makes application bumpless.
5. Set the **Identification step**, the change of the output during
   identification. It defaults to a tenth of the output range. A model is only
   trusted in the range it was measured in, so the step must move the
   measurement across the range your tests will use.
6. Set the **Identification duration**: long enough for the measurement to show
   the response you want to control, typically three to five of its time
   constants. It has no default, because it depends on the process.
7. Validation starts your program from the values it seeds: the setpoint, the
   manual output, the mode, the parameter record and the PID's internal state.
   If the program writes one of them itself, for example in startup code, or
   calls the PID's `Initialize`, the form names that write and asks for the
   **Startup-done flag**. Validation starts with that flag TRUE, so the startup
   code does not replace the values being checked.

**Save loop** writes `control-loops/<name>.json`, one file per loop, and opens
Control Studio on it. Save is unavailable until the form is complete; the line
below it names the first missing item.

## Identify

Choose the plant inputs and measured outputs, then the **Method**:
**Automatic**, **Step response**, **Pulse response**, **Setpoint step**,
**Feedback relay**, **Pseudo-random binary sequence (PRBS)**, **Multisine**,
**Recorded-data fit**, **Supplied plant model**, or **Project experiment** when
the project declares its own test. **Limits…** sets the permitted input and
output ranges and the longest experiment.

**Identify plant** runs the experiment on the live plant and fits a model.
A method that steps the plant input switches the loop to Manual for the step.
A loop it finds in Auto is first read until it has settled, the measured value
and the controller's output both steady, so the step starts from the output at
which the controller holds the loop; the wait shows its progress, writes
nothing and can be cancelled. **Open recording…** fits a saved recording
instead. The model shows its
equation and stays **Provisional** until Validate checks it against a fresh
recording.

## Tune

Choose the **Controller** configuration (P, PI, PD or PID of the same
`Control.PID`; the recommended one is marked) and the **Tuning method**:
**Automatic**, **Simple internal model control (SIMC)**, **Approximate
M-constrained integral gain optimization (AMIGO)**, the **Relay rule** after a
relay experiment, or **Manual**. The same list offers the analysis tools: root
locus, Routh–Hurwitz, poles and zeros, frequency responses, loop response
(Bode, Nyquist, sensitivity), lead/lag design, initial-state response,
controllability and observability, the relative gain array and measured MIMO
paths.

**Requirements…** holds the test change, allowed error, settling band, settling
time and test duration. Missing values are derived from the model.
**Tune controller** shows the predicted response of the tuned and the current
settings. With the **Automatic** method, Tune takes the most robust response
whose prediction meets the requirements; when none can, it names the
requirement the fastest response misses and by how much. For an ON/OFF or state-feedback law in your program, the prediction
runs your ST with both settings on the identified model. Nothing is applied. If a requested test change leaves the range the
model was identified in, Tune names that range and the largest change it
supports.

## Validate and apply

**Validate controller** first checks the model against a fresh recording. It
then runs your actual ST program with the tuned settings against a simulated
plant built from the model, including the scenarios the project declares. The
table lists each requirement with its measured value, limit and **Pass**,
**Fail** or **Inconclusive**. Passing generated checks of ranges, controller
states and the controller transfer are counted in one row; every failure keeps
its own row. **Measured response** plots the tuned and current settings.

**Apply** is available only when every row passes. It starts from the
operating point the settings were validated at. If the process has not yet
returned there, Apply waits for it, showing the measurement's validated and
current value, and continues on its own; **Abort** ends the wait without
changing anything. Apply then writes the validated settings to the running
controller in Manual, enters Auto bumplessly, steps the setpoint and checks the
response, then reports **Applied**. If the check fails, the previous settings
are restored and the failure and the recovery are reported. **Restore previous
settings** undoes an applied change.

A state-feedback law has no integrator to make the switch to Auto bumpless, so
its first output in Auto responds to how far the process still is from the
operating point. Apply allows exactly that response to the deviation it
accepts as settled, unless you set an **Auto-entry tolerance** in
**Requirements…**.

## Cascades

A cascade is saved as one loop for both controllers and tuned together.
**Identify** puts the inner controller in Manual, steps the plant input and
records both measurements; give it a duration long enough for the outer
measurement to settle. **Tune** designs the inner controller first, for a
fifth of the outer settling time unless you declare inner requirements, then
predicts on the model how the outer measurement follows an inner setpoint step
through the closed inner loop and designs the outer controller on that
response for your requirements. **Validate** runs both controllers in Auto on
the actual ST. **Apply** switches both to Auto in one step, the outer output
starting at the inner measurement so nothing bumps, and **Restore previous
settings** returns both to their previous settings and to Manual.

A cascade written earlier as two linked loop files is still tuned one loop at a
time: the inner loop with the outer controller in Manual, then the outer loop
through the applied inner loop.

## Feedforward from a measured disturbance

When a disturbance can be measured, such as a heat extraction or an inflow,
the program can add a feedforward to the PID's `FeedForward` input so that the
correction starts with the disturbance instead of with the error. Choose
**Feedforward**: the PID's fields as for one loop, then the measured
disturbance, the variable Identify steps to move it (in a simulation usually
the simulated load itself), its identification step and the program's
feedforward gain variable. If the program filters the disturbance through a
lead/lag, also choose its lead time, lag time and the filtered disturbance it
keeps; the program computes
`FeedForward = K_ff·(z + (T_lead/T_lag)·(d − z))` with `z` the disturbance
through the lag.

**Identify** steps the controller output and then the disturbance and fits one
model from both. **Tune** designs the PI on the output's path and the
feedforward from both paths: `K_ff = −K_d/K_u` cancels the disturbance in steady
state, the lead time is the output path's time constant and the lag time the
disturbance path's (plus any delay the disturbance takes longer). It shows both
path models and, on the model, the largest deviation after a disturbance step
with the PI alone and with the feedforward. **Validate** adds a step of the
measured disturbance with the setpoint unchanged. **Apply** starts the PID's
feedforward state at `K_ff` times the measured disturbance, so switching to Auto
does not bump.

## Smith predictor

A delay limits how fast any controller can act. With a Smith predictor the PID
reads the measurement plus a correction the program computes: the delay-free
model's response to how much the output changed over the delay,
`T·dc/dt = K·(u − u_θ) − c`. The program keeps the output's history over the
delay, refilled with the output it is applying, before the controller's update,
while the controller is not in Auto and on its first scan, so the correction is
zero whenever the output has been steady. Choose **Smith predictor**: the PID's fields as for one loop, with the
process measurement as the measurement, then the program's variables for the
model gain, time constant and delay and the correction.

**Tune** reduces the model to first order with delay, writes that gain, time
constant and delay into the program and designs the PI for the process without
its delay, to settle the delay sooner than your settling time: with a correct
model the loop responds as the delay-free loop, delayed. A settling time no
longer than the delay is refused, because no controller acts sooner.

## Gain schedules

When the process's gain or speed changes with its operating point, the program
can interpolate the PID's gains between the gains of a few operating points,
for example by the measured level. Choose **Gain schedule**: the PID's fields
as for one loop, then two to four operating points, each with a name and the
program's proportional and integral gain variables for it. Setup writes one
loop per point. For each point, bring the process to it in Manual, then
identify, tune, validate and apply that point's loop; validation runs with the
program's schedule active, so the gains it interpolates as the process moves
are the ones checked.

## Several PIDs and PID groups

When a program has several independent PIDs, set each up as its own loop:
choosing the controller fills in its signals. When the loops interact, so that
each output moves several measurements, you can either tune them one after the
other or as a group:

- **One after the other** (sequential loop closing): tune the first loop with
  the others in Manual and apply it, then identify and tune the next loop with
  the first in Auto. Each model then includes the loops already closed.
- **PID group**: include the interacting PIDs in one group. The members share
  one Auto/Manual signal and are identified, validated and applied together.
  Tune designs one PI per member from the group model: for its own path, with
  the gain that path has when the other loops are closed (the path gain divided
  by its relative gain). A pairing whose relative gain is not positive is
  refused, with the pairing to use instead.
- **PID group with a static decoupler**: when the program passes the members'
  outputs `v` through a decoupler, `u = D·v` in Auto (and `u = v` in Manual),
  tick **The program applies a static decoupler** and choose its variables
  `D_ij`. Each member's output is then the plant input it drives, not the
  controller's own output. Tune calculates `D = K⁻¹·diag(K)` from the model's
  steady-state gains, so each member moves only its own measurement in steady
  state, designs each PI through it, and Apply writes the gains and `D`
  together, starting each controller at its share of the present plant inputs.

## ON/OFF controllers

An ON/OFF controller is switching logic in your own program. Choose **ON/OFF**
and the instance that runs it, then its measurement, setpoint, output,
Auto/Manual signal and manual output. The output is a number that switches
between its minimum and maximum, for example 0 and 100 %.

Tune designs three settings, each a variable of your program:

- the **Hysteresis**, in the measurement's unit. **Hysteresis holds** says
  whether the variable is half the switching band (the output switches at
  setpoint ± hysteresis) or the whole band;
- the **Minimum ON time** and **Minimum OFF time**, in seconds.

Enter the **Equipment minimum ON time** and **OFF time** when the switched
equipment has them, for example a compressor's restart protection. Tune never
proposes shorter times. Leave them blank when the equipment has none.

The **Output-on state** is the BOOL that holds whether the output is on, and
**Time in current state** the variable that counts the seconds since the last
switch, when the program has one. Validation starts them from the plant's
initial state.

Tune sets the switching band from the settling band of the requirements.
Because the output switches, the process cycles around the setpoint: the
allowed error is the band that cycle must stay within after the settling time.

## State feedback

A state-feedback controller is a law in your own program that feeds several
measured outputs back to one plant input:
u = u₀ + N·(r − y₀) − Σ Kᵢ·(yᵢ − yᵢ₀). Choose **State feedback** and the
instance that runs it, the **Plant input**, the **Controlled output** with its
setpoint, and one to three further measured outputs. For each measured output
choose the variable that holds its gain, and for the controlled output also the
variable that holds the **Reference gain** N.

Studio identifies the plant in these measured coordinates: each measured output
is one state of the model, so each gain multiplies one measured signal. The law
works in deviations from its operating point, so identify the plant at the
operating point the program uses.

**Design** sets how Tune calculates the gains: **LQR**, which weights each
signal by the inverse square of its range, or **Pole placement**. LQG, whose
estimator has its own states, uses the step-by-step setup.

## Your own P, PI or PID

A P, PI or PID law your program computes itself, instead of a `Control.PID`, is
set up with **Own P, PI or PID**. The program computes, in parallel form,
u = Kp·e + I + D, where e is the control error, the integral term I, in the
output's unit, integrates Ki·e over time (I = u₀ + Ki·∫e dt), and D is Kd times
the rate of change of the error, or minus Kd times the rate of the measurement
when the derivative acts on the measurement. Choose the instance that computes
it, the **Law**, the measurement, setpoint and output, the Auto/Manual signal
and manual output, and for each gain the variable that holds it with the range
Tune may choose from. For PI and PID also choose the variable that holds the
**Integral term**; for PID the variable in which the program keeps the previous
error or measurement its derivative is taken from, and which of the two it is.

Tune designs the gains from the model as for `Control.PID` and offers only the
families your law has. Validation starts the integral term at the output of the
switch to Auto and the previous value at the error (zero) or measurement of
that moment, with the setpoint at the measurement, so the switch is bumpless;
the program should do the same when it switches, for example by holding I at
the manual output while in Manual.

## Other laws: coefficients tuned by search

A law Studio has no design formula for, such as an MPC, a ratio, split-range
break points, override limits or a gain schedule, is set up with **Other law**.
Choose the kind of law, the instance that computes it, its signals, one to four
coefficient variables with their unit and the values to try, and up to three
values the program keeps from scan to scan, each with what validation starts it
at: zero, the output or the measurement. Tune runs every combination of the
values, at most 64, on the actual ST against the identified model, keeps those
that meet every requirement and uses the one with the smallest error after the
settling time.

## Physical-plant boundary

A physical experiment requires an independently reviewed topology and a new,
immediate approval of the exact displayed action. Verify the signal, bounds,
timeout, stop conditions and restoration value immediately before approval.

Simulation results are not physical-plant evidence. Revalidate limits, failure
behaviour, safety functions and controller performance on the reviewed physical
topology before commissioning.

## Examples

Each example is a small project to open from the examples gallery and set up
with the form. Its README lists the values measured in Control Studio.

- [PID loop](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control-pid-commissioning): a
  `Control.PID` on a heater.
- [Cascade](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control-cascade-commissioning): two
  `Control.PID` blocks in cascade on a jacketed vessel.
- [Interacting loops](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control-two-zone-commissioning):
  two zones sharing a wall, tuned one after the other or as a PID group.
- [ON/OFF control](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control-onoff-commissioning): a
  cold room held by a switching compressor.
- [State feedback](https://github.com/johannesPettersson80/trust-platform/tree/main/examples/control-state-feedback-commissioning):
  two tanks in series under an LQR law.

## Related

- [Control System Engineering](../develop/control-system-engineering.md)
- [Simulation](simulation.md)
- [Safety And Commissioning](safety-and-commissioning.md)
- [Runtime UI And Control](runtime-ui-and-control.md)
- [Live Values](live-values.md)
