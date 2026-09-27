# Lesson explanations: 22–32

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 22. Setpoint weighting

**Setpoint weighting** changes how strongly the proportional term responds to
the target while retaining its response to the measured value.

A target step can cause an unnecessarily large power jump. Reducing the whole
proportional gain would also weaken correction of a measurement change.
Instead, we weight the target separately:

$$
P=K_p[\beta(SP-T_0)-(PV-T_0)].
$$

$P$ is proportional output, $K_p$ is proportional gain, SP and PV are desired
and measured temperature, and $T_0=35$ °C is the fixed operating-point reference.
The dimensionless weight $\beta$ controls only the target contribution.

With $K_p=2$ and a 3 °C target step, $\beta=1$ gives a 6-point P change;
$\beta=0.5$ gives 3 points. A measured-temperature change still has the
same proportional coefficient.

Integral action continues to use the full error $SP-PV$, so it can remove
steady error. Away from the reference, the weighted P contribution need not
be zero when SP equals PV; the integral supplies the remaining holding output.

In Control Studio we compare both weights on separate target and disturbance
trials. We judge reduced command jump against slower target response.
Measurement noise still affects the feedback term, motivating filtering.

## 23. Measurement filtering

A **measurement filter** reduces rapid fluctuations in a measured signal.
We use it when sensor noise causes unnecessary controller output movement.

A first-order low-pass filter lets slower changes through more readily than
faster ones. It moves the filtered value toward the raw measurement:

$$
y_{f,\mathrm{new}}=y_{f,\mathrm{old}}+
\alpha(y_r-y_{f,\mathrm{old}}),
\qquad
\alpha=\frac{\Delta t}{T_f+\Delta t}.
$$

- $y_r$: current raw measurement.
- $y_f$: filtered measurement, in the same units.
- $\Delta t$: update interval, in seconds.
- $T_f$: filter time, in seconds.
- $\alpha$: fraction of the difference applied this update.

This follows from a backward-difference approximation to
$T_f\dot y_f+y_f=y_r$.

With $T_f=1$ s and $\Delta t=0.1$ s, $\alpha=1/11$.
A raw change from 35 to 36 °C moves the filtered value first to 35.0909 °C.

The cost is delayed recognition of real change. The filter cannot distinguish
noise from a genuine rapid temperature movement just by knowing it is rapid.

In Control Studio we compare bypass and filtering with the same noise, then
a real heat disturbance. We inspect command activity and temperature recovery.
Next we examine whether measurements are acquired often enough in the first place.

## 24. Sampling and timing

**Sampling** measures a signal at separate instants instead of continuously.
The controller then calculates an output that is usually held until its next update.

A program can run quickly while repeatedly reading an old sensor sample.
Sensor acquisition time, controller update time and applied-command time
therefore describe different events.

State changes depend on actual elapsed time:

$$
\Delta I=K_i e\Delta t.
$$

$I$ is integral output, $K_i$ is integral gain, $e$ is error and
$\Delta t$ is elapsed time since the previous accepted update.
For $K_i=0.5$ and a constant 1 °C error, 0.1 s adds 0.05 point;
0.2 s adds 0.10 point.

**Aliasing** occurs when different continuous changes look the same at the
sample times. Sampling a sine wave once per complete cycle can make it appear
constant. Filtering afterward cannot reconstruct information never acquired.

In Control Studio we inspect acquisition and update timestamps, compare
sampling periods and introduce a delayed update. We distinguish missing
measurement information from incorrect elapsed-time arithmetic.

We must also initialize the stored state and time baseline correctly before
the first automatic calculation.

## 25. Controller initialization

**Controller initialization** sets the stored values needed for the first
automatic calculation to agree with the current operating condition.

A PI controller retains its integral contribution between updates. If a warm
heater is already receiving holding power, starting with zero integral can
abruptly remove that power.

For the current proportional contribution $P$ and applied output $u_a$:

$$
I_{\mathrm{initial}}=u_a-P.
$$

$I$ and $P$ are output contributions on the same command scale as $u_a$.
The equation makes the reconstructed request $P+I$ equal the current output.
At the 35 °C hot reset, $P=0$ and $u_a=75\%$, so initialize $I=75\%$.

We also initialize the measurement filter from a fresh reading and establish
the update-time baseline. Automatic output ownership starts only after
these steps succeed.

In ST, one persistent function-block instance retains the controller state.
The calling program selects which output actually drives the actuator.

In Control Studio we compare unaligned and correctly initialized starts.
We inspect the first command as well as temperature. Later manual changes
require ongoing alignment, which is the purpose of bumpless transfer.

## 26. Bumpless transfer

**Bumpless transfer** changes control ownership without an unintended jump
in the actuator command.

Initialization handled the first automatic start. But an operator may later
change the output in manual mode while the automatic controller is inactive.
Its stored integral must follow that actual output.

For the current proportional contribution $P$:

$$
I=u_a-P.
$$

Here $u_a$ is applied manual output and $I$ is the integral contribution needed
to reproduce it. If $P=4$ points and manual output is 60%, $I=56\%$ gives
$P+I=60\%$. A stale integral of 75% would instead request 79%.

On the transfer update, we align state and reconstruct the current output
without also adding an ordinary integral increment. Normal PI updates
resume afterward.

This prevents a jump caused by changing mode. It does not stop the controller
making necessary corrections on later updates.

In Control Studio we change manual power, return to automatic and inspect
the command around the transfer. Temperature recovery is assessed separately.
Next we handle another source of unintended jumps: changes to controller parameters.

## 27. Parameter updates

A **parameter update** changes the settings used by a running controller.
An **atomic update** adopts one complete validated set at a single update
boundary, so the controller never uses an accidental mixture of old and new values.

Changing a gain can also change the output even when the measurement has not moved.
To preserve the current request when proportional action changes:

$$
I_{\mathrm{new}}=I_{\mathrm{old}}+P_{\mathrm{old}}-P_{\mathrm{new}}.
$$

$P_{\mathrm{old}}$ and $P_{\mathrm{new}}$ are calculated from the same input
snapshot using the two parameter sets.

For a 2 °C error, changing $K_p$ from 2 to 3 changes P from 4 to 6 points.
Changing I from 71 to 69% keeps the total request at 75%.

We validate the whole candidate before adopting it. Invalid gains, timing
values or inconsistent output limits reject the candidate. A deliberately
lowered output ceiling may legitimately reduce the command; that is different
from an accidental state jump.

In Control Studio we apply valid and invalid updates and inspect the active
revision, contributions and applied output. Correct parameters still cannot
make an invalid or stale measurement usable.

## 28. Measurement validity

**Measurement validity** means deciding whether a reading is suitable for
the control calculation.

A value may be numerically plausible but too old. Another may be fresh but
outside the sensor's range or marked invalid by its source. We check those
conditions before updating controller state.

Measurement age is:

$$
a=t_{\mathrm{now}}-t_{\mathrm{acquisition}}.
$$

$a$ is age in seconds. A reading is fresh enough only when
$0\le a\le a_{\max}$, where $a_{\max}$ is the configured maximum age.
For a 0.5 s limit, a 0.6 s old reading fails even if it still says 35 °C.

The heater additionally requires a finite reading within 0–80 °C, valid source
quality and a usable controller interval.

A **latched fault** remains active until its recovery conditions are met.
In this example, invalid data removes heat, freezes integral/filter state
and clears stale manual demand. Recovery counts genuinely fresh acquisitions
and requires deliberate re-enabling.

In Control Studio we test invalid values and stale acquisition separately,
then inspect the recovery sequence. Next we verify these mechanisms in the
complete heater controller.

## 29. Heater commissioning

**Heater commissioning** checks the integrated controller against both
temperature-performance and operating-state requirements.

We have taught tuning, filtering, anti-windup, initialization, transfer,
parameter updates and measurement validity separately. The complete ST
controller must combine them with one clear owner of the applied output.

A capacity check comes first:

$$
u_{\mathrm{hold}}=\frac{H(T-T_a)+D}{a}.
$$

$H$ is heat-loss coefficient in kW/°C, $T-T_a$ is temperature above ambient,
$D$ is additional extraction in kW, and $a$ is heater kW per command point.
The result is holding command in percent-scale points.

At 35 °C, ambient 20 °C, $H=10$, $D=30$ and $a=2$, holding requires 90%.
A controller cannot overcome a requirement beyond available power.

In Control Studio we run the same target and disturbance trials, then
separate startup, manual-transfer, saturation, update and measurement-fault
cases. Every requirement needs its own observation and decision.

This is a simulation commissioning exercise, not physical equipment approval.
The next application replaces electric power with hot-water flow through
a valve whose motion introduces another actuator limitation.

## 30. Actuator tracking

**Actuator tracking** uses what an actuator has actually achieved to keep
controller state consistent with it.

A valve can receive a target inside 0–100% and still take many seconds to
reach it. Tracking only that target would miss the difference between
requested and actual flow-producing position.

For measured stem position $x$ and requested position $u_r$:

$$
I_{\mathrm{next}}=I^*+\frac{\Delta t}{T_t}(x-u_r).
$$

$I^*$ is the integral after ordinary accumulation, $\Delta t$ the update
interval and $T_t$ the tracking time. Position and output contributions use
the same percentage scale.

At a 0.5-point/s movement limit, a 75→85% target takes at least 20 seconds.
With current position 75%, request 80.1%, $\Delta t=0.1$ s and $T_t=1$ s,
the integral tracking correction is −0.51 point.

The valve's flow then carries heat into the vessel:

$$
Q=c_p q(T_{\mathrm{in}}-T),
$$

where $q$ is mass flow, $c_p$ specific heat and the temperature difference
determines energy delivered per unit mass.

In Control Studio we compare target tracking with measured-position tracking,
observing stem, flow, integral and temperature. Next we establish how position
and pressure determine the delivered flow.

## 31. Valve characteristics

A **valve characteristic** describes how valve opening relates to flow.
The installed relationship also depends on pressure drop across the valve.

Pressure supplies energy to move the fluid. For incompressible flow through
the teaching valve, the supplied calibration is:

$$
q=2z\sqrt{\frac{\Delta p}{100\ \mathrm{kPa}}}\ \mathrm{kg/s},
\qquad z=x/100.
$$

- $q$: mass flow.
- $x$: opening in percent; $z$: opening fraction.
- $\Delta p$: pressure drop across the valve.
- 2 kg/s: calibrated full-opening flow at 100 kPa.

The square root comes from pressure energy producing velocity-squared kinetic
energy. The proportional opening assumption belongs to this example;
a different valve trim can have another relationship.

At fixed 100 kPa, opening from 75 to 80% changes flow from 1.5 to 1.6 kg/s.
The local slope is $0.1/5=0.02$ kg/(s·point).

If pipe losses increase with flow, less supply pressure remains across the
valve, so the installed flow increase is smaller than the constant-pressure prediction.

In Control Studio we wait for stem and flow to settle, record position,
pressure and flow, then compare the two pressure arrangements.
Next we investigate commands that fail to move the stem normally.

## 32. Valve stiction

**Stiction** is static friction that holds an actuator still until the driving
force exceeds a breakaway threshold.

It differs from **command deadband**, where small requested changes are
deliberately ignored before reaching the actuator. We need different signals
to distinguish those mechanisms.

For the illustrative valve positioner:

$$
F=k_s(u_t-x).
$$

$F$ is drive force in newtons, $k_s$ is force per position-error point,
$u_t$ is accepted target and $x$ is measured stem position.

With $k_s=10$ N/point, stem at 75% and target 80.5%, force is 55 N.
That exceeds the example's 50 N static breakaway threshold.
Once moving, lower friction means the stem can move until the remaining
force falls to the stated 20 N moving-friction threshold.

A deadband case can leave the accepted target unchanged.
A stiction case can change the accepted target while the stem remains still.

In Control Studio we apply bounded increasing, decreasing and reversal probes.
We inspect request, accepted target, force, actual position and flow before
deciding whether to investigate the command path or mechanics.

The next lesson explains how a healthy motor controls position.
