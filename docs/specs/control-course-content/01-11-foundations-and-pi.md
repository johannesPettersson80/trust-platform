# Lesson explanations: 1–11

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 1. Control-loop elements

**Control-loop elements** are the parts connecting a desired value,
measurement, control decision, actuator and physical process. Together they
let a control system influence a measured quantity toward its desired value.

In our cold room:

- The **controlled variable** is room temperature.
- The **setpoint**, SP, is the temperature we want: 2 °C.
- The **process value**, PV, is the temperature reported by the sensor.
- The **command** tells the refrigeration system to turn OFF or ON.
- The **actuator** is the refrigeration equipment that removes heat.
- The **plant** is the room and its contents.
- A **disturbance** is an outside influence, such as heat entering through an open door.

The command can change immediately, but temperature changes gradually because
the room and its contents store thermal energy. Asking for cooling is not the
same as instantly reaching the target temperature.

First we run a fixed sequence in Control Studio: refrigeration ON for 20 seconds,
then OFF, with the door open from 60 to 90 seconds. We compare command and
temperature to establish which action causes cooling.

This sequence does not react when the room becomes too warm. The next lesson
makes the measurement determine the command.

## 2. ON/OFF control

An ON/OFF controller uses a measurement to choose between two actuator states.
This is **feedback**: we measure the result of our action and use it to choose
the next action.

For the cold room, the rule is:

$$
u =
\begin{cases}
100, & PV \ge SP,\\
0, & PV < SP.
\end{cases}
$$

Here $u$ is the refrigeration command on the 0–100% scale, PV is measured
temperature and SP is the 2 °C target. At exactly the target, this rule chooses ON.

When temperature is too high, refrigeration removes heat. When temperature
falls below the target, refrigeration stops and incoming heat warms the room.
An ST comparison and IF/ELSE decision repeat this rule each update.

But readings of 2.01, 1.99 and 2.01 °C produce ON, OFF and ON. Small changes near
one threshold can therefore cause rapid repeated switching, called **chatter**.

In Control Studio we observe automatic recovery after the door opens, then
inspect the command near the threshold. The next lesson reduces unnecessary
switching and adds minimum operating times.

## 3. Hysteresis and dwell

**Hysteresis** uses different thresholds for switching ON and OFF. Between
them, the controller remembers its previous state.

For our cold room:

- At or above 2.5 °C, request ON.
- At or below 1.5 °C, request OFF.
- Between those temperatures, keep the previous request.

A small movement around 2 °C no longer reverses the command. The distance
between the thresholds is the hysteresis width: $2.5-1.5=1$ °C.

**Dwell time** is a separate rule: the actuator must remain in its current
applied state for a minimum time before a normal reversal. We use 8 seconds
for this teaching example.

If refrigeration has been ON for 6 seconds when temperature reaches 1.5 °C,
hysteresis requests OFF, but dwell postpones that change for another 2 seconds.
A stop or abort still forces OFF immediately.

In Control Studio we compare requested and applied states. Hysteresis explains
why the request is retained; dwell explains why an allowed request may be delayed.
We then check temperature variation and switching times together, rather than
assuming fewer switches automatically mean satisfactory control.

## 4. Cold-room commissioning

**Cold-room commissioning** checks whether the complete controller meets the refrigeration application's
requirements. Knowing that its individual rules work does not establish that
their combination works under changing conditions.

For this simulation, we combine feedback, hysteresis and dwell and require:

- Temperature within 1–3 °C from 40 seconds onward.
- Every normally completed ON and OFF interval at least 8 seconds.
- Applied command always exactly OFF or ON.
- A stop that removes refrigeration and a deliberate restart sequence.

An applied-state duration is:

$$
t_{\mathrm{duration}}=t_{\mathrm{next\ transition}}-t_{\mathrm{entry}}.
$$

Both times are measured in seconds from the same experiment clock. An interval
of 7.9 seconds fails an 8-second requirement. An interval interrupted by an
abort is recorded separately.

In Control Studio we run the controller from 4 °C with the door open at
60–90 seconds, then repeat from the same reset with the door closing at
105 seconds. The requirements stay unchanged.

This establishes whether the taught cold-room controller is satisfactory.
The next application uses a different actuator: an electric heater with
continuously adjustable power.

## 5. Proportional control

**Proportional control** makes the correcting command proportional to the
current difference between the desired and measured value.

For the heater, larger temperature shortfall should produce more heating.
We want zero correction at zero error and the same extra correction for every
additional degree. That gives a straight-line rule:

$$
e=SP-PV,\qquad u_r=K_p e.
$$

- $e$: temperature error, in °C.
- $SP$ and $PV$: desired and measured temperature, in °C.
- $K_p$: proportional gain, in command percentage points per °C.
- $u_r$: requested heater command, on the percent scale.

With SP = 35 °C, PV = 20 °C and $K_p=2$:

$$
u_r=2(35-20)=30\%.
$$

As the water warms, the error and heating command decrease. But at zero error,
this rule requests zero heat even though the hot water still loses energy.
The water therefore settles below target, where a remaining error produces
the necessary holding power. This remaining difference is **steady-state error**.

In Control Studio we observe that error with the same heater and fixed gain.
The next lesson adds a contribution that can remain when current error is zero.

## 6. PI control

**Proportional-integral control**, or PI, adds a stored contribution based on
accumulated error to the immediate proportional correction.

P alone needed an error to keep heating the water. Integral action increases
the stored heating contribution while the water remains below target.
When the error reaches zero, that contribution remains.

The sampled calculation is:

$$
\Delta I=K_i e\Delta t,\qquad
I_{\mathrm{new}}=I_{\mathrm{old}}+\Delta I,\qquad
u_r=K_p e+I_{\mathrm{new}}.
$$

Here $I$ is retained output contribution in percent, $K_i$ is integral gain in
points/(°C·s), and $\Delta t$ is elapsed time in seconds. Multiplying error by
time gives a small area under the error curve; adding those areas approximates
its integral.

With error 15 °C, $K_i=0.5$ and $\Delta t=0.1$ s, the increment is 0.75 point.
Starting from zero, I becomes 0.75, then 1.50 if the error remains unchanged.
Those values are retained program state, not heat stored in the water.

At zero error with I = 75%, the next increment is zero and the heater still
receives 75%.

We repeat the P experiment in Control Studio with integral action added.
We check that the same steady error is removed, then investigate what happens
when the requested heating exceeds available power.

## 7. Saturation and windup

**Actuator saturation** occurs when requested output reaches a physical or
configured limit. A heater cannot supply more than its maximum power.

The controller can still calculate a larger request:

$$
u_a=\min(100,\max(0,u_r)).
$$

Here $u_r$ is requested output and $u_a$ is applied output. The min/max operations
limit the applied command to 0–100%. A 125% request therefore applies 100%,
leaving 25 percentage points of demand undelivered.

**Integral windup** occurs when the integral keeps accumulating demand while
the actuator cannot follow it. Positive error still adds $K_i e\Delta t$,
even though further demand produces no additional heat.

Our supplied heater relationship is $T_{\mathrm{eq}}=20+0.2u_a$ °C.
At 100%, equilibrium is 40 °C, so a 45 °C target is unreachable.

In Control Studio we start at 35 °C with 75% holding output, request 45 °C
for 10 seconds, then return to 35 °C. We inspect the integral and requested
versus applied output during the following 60 seconds.

Returning the target does not reset accumulated demand. The next lesson
corrects that stored excess.

## 8. Anti-windup

**Anti-windup** prevents unusable integral demand from causing excessive
recovery delay when an actuator is limited.

Back-calculation anti-windup corrects integral state using the difference
between applied and requested output:

$$
I^*=I_{\mathrm{old}}+K_i e\Delta t,\qquad
u_r=P+I^*,
$$

$$
I_{\mathrm{next}}=I^*+\frac{\Delta t}{T_t}(u_a-u_r).
$$

- $I^*$: integral after the ordinary error update.
- $P$: proportional contribution.
- $u_r$ and $u_a$: requested and applied output.
- $T_t$: tracking time, in seconds.
- $\Delta t/T_t$: fraction of the output discrepancy corrected this update.

For request 125%, applied 100%, $\Delta t=0.1$ s and $T_t=1$ s,
the correction is $0.1(100-125)=-2.5$ points.

At the upper limit it reduces I; at the lower limit it increases I.
When the actuator follows the request, the correction is zero.
It changes retained controller state, not the heater's capacity.

We repeat the same saturation-and-return experiment with and without tracking
in Control Studio. We compare the integral, time at the limit and temperature
recovery. Then we establish ordinary setpoint and disturbance requirements.

## 9. Setpoint and disturbance response

**Setpoint response** is the reaction to a changed desired value.
**Disturbance rejection** is the response to an unwanted physical influence
while the desired value stays fixed.

They enter different places in the loop. A setpoint step changes the
controller's error immediately. An unmeasured heat extraction must first
change water temperature before feedback sees an error.

At the hot reset, SP = PV = 35 °C and I = 75%. With $K_p=2$,
$K_i=0.5$ and a 0.1-second update, changing SP to 38 °C gives:

$$
u_r=75+2(3)+0.5(3)(0.1)=81.15\%.
$$

A 30 kW extraction initially leaves measured temperature and error unchanged.
The controller reacts as the water cools. At 2 kW per command point, replacing
that loss ultimately needs $30/2=15$ extra points.

In Control Studio we run these events separately for 60 seconds from identical
resets. The target trial requires peak temperature at most 39 °C; the load trial
requires minimum temperature at least 33 °C. Both require error within ±0.2 °C
through 30–60 seconds and output within 0–100%.

A satisfactory target response does not establish satisfactory disturbance
rejection. A failed requirement leads next to diagnosis.

## 10. Loop diagnosis

**Loop diagnosis** identifies the cause of unsatisfactory control by checking
evidence that distinguishes possible explanations.

A slow response could come from controller gains, an actuator that does not
deliver its request, an unreliable measurement or insufficient capacity.
Changing gains cannot solve all those problems.

Follow the actual sequence: requested command, applied command, delivered heat,
physical temperature and acquired measurement.

For example:

- Request above applied output suggests a restriction.
- Applied output without an immediate temperature change may be normal delivery delay.
- A repeated displayed value may be a steady temperature or an old sample.

Measurement age is:

$$
\mathrm{age}=t_{\mathrm{now}}-t_{\mathrm{acquired}}.
$$

It measures seconds since acquisition, not since the display refreshed.

Capacity also matters. Holding 35 °C with the extra 30 kW load needs 90% nominal
output. Holding 38 °C with that same load needs 105%, which is impossible.

In Control Studio we choose the signal that distinguishes competing causes,
then inspect it. Only after measurement, delivery and capacity problems are
excluded does manual retuning become the appropriate next action.

## 11. Manual PI tuning

**Manual tuning** is deliberate adjustment of controller gains through
repeatable experiments against unchanged requirements.

Changing $K_p$ changes the correction from the current error.
Changing $K_i$ changes how quickly the retained contribution accumulates.
The student chooses which effect is needed before changing a number.

For the same 3 °C error:

$$
\Delta P=\Delta K_p\,e,\qquad
\Delta(\text{integral increment})=\Delta K_i\,e\,\Delta t.
$$

Increasing $K_p$ from 2 to 2.4 adds $0.4(3)=1.2$ output points immediately.
Increasing $K_i$ from 0.5 to 0.6 instead adds $0.1(3)(0.1)=0.03$ point
on the first update, with further differences accumulating afterward.

More aggressive correction can improve speed but also increase overshoot
or time at an output limit. Stored heat and delivery delay make both effects
important.

In Control Studio we change one gain at a time and repeat both response tests
from Lesson 9. We keep or reject the setting using the same limits and metrics.

These experiments tell us how the tested controllers behave. They do not yet
separate the heater's own dynamics from the feedback response, which motivates
the modelling lessons.
