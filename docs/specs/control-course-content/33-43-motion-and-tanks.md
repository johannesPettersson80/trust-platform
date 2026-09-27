# Lesson explanations: 33–43

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 33. Position control

**Position control** uses feedback to move an object to a desired position.

For a rotating shaft, the actuator produces torque. Torque changes velocity,
and velocity changes angle. A shaft can therefore still be moving when its
angle first reaches the target.

The physical relationships are:

$$
J\dot\omega=u-B\omega-\tau_L,\qquad \dot\theta=\omega.
$$

- $\theta$: shaft angle, in radians.
- $\omega$: angular velocity, in rad/s.
- $J$: inertia, describing resistance to acceleration.
- $u$: applied torque, in N·m.
- $B\omega$: friction torque.
- $\tau_L$: resisting load torque.

Proportional position control requests $u_r=K_p(r-\theta)$, where $r$ is the
desired angle and $K_p$ is torque per radian of error.

At $K_p=16$ and error 0.1 rad, the request is 1.6 N·m. When error reaches zero,
P requests zero torque—but that does not instantly remove existing velocity.

In Control Studio we compare position gains on the same shaft and inspect
velocity at the first target crossing. Stronger P may accelerate the shaft
more without supplying enough braking.

The next lesson adds a correction based on velocity.

## 34. PD control

**Proportional-derivative control**, or PD, combines position correction with
a contribution that responds to motion.

P tells us how far the shaft is from its target. Velocity tells us how much
motion needs braking. Using measured velocity:

$$
u_r=K_p(r-\theta)-K_d\omega.
$$

$u_r$ is requested torque, $r-\theta$ is angle error, and $\omega$ is measured
angular velocity. $K_d$ has units N·m·s/rad. A positive velocity produces
negative derivative torque, opposing that motion.

Substitution into the shaft equation gives:

$$
J\ddot\theta+(B+K_d)\dot\theta+K_p\theta=K_pr.
$$

Derivative feedback adds to the physical damping coefficient $B$.
With $K_d=7$ and $\omega=0.2$ rad/s, braking torque is −1.4 N·m.

Differentiating error instead would also respond to changes in the target.
An abrupt target step can then cause **derivative kick** even before the shaft
moves. Derivative on measurement avoids that target-induced term.

In Control Studio we hold P fixed and compare the motion with and without D.
We inspect overshoot, velocity, settling and torque.

At rest, D contributes no torque. The next lesson adds holding torque for a load.

## 35. PID load rejection

**PID control** combines proportional correction, accumulated-error correction
and derivative damping.

PD can position and brake the shaft, but holding a weight requires torque
even when angle error and velocity are zero. Integral action can retain that torque.

For a weight lifted by a drum:

$$
\tau_L=mgr.
$$

$m$ is mass in kg, $g$ gravitational acceleration in m/s² and $r$ drum radius
in metres. With 2 kg and a 0.1 m drum, $\tau_L=1.962$ N·m.

At rest, PD must supply this through P, so:

$$
e_{\mathrm{steady}}=\frac{\tau_L}{K_p}.
$$

For $K_p=16$, the remaining error is 0.122625 rad.

Adding integral torque gives:

$$
I_{\mathrm{new}}=I_{\mathrm{old}}+K_i e\Delta t.
$$

$I$ is retained torque contribution, $K_i$ is in N·m/(rad·s), and
$\Delta t$ is elapsed seconds. At zero error and velocity, I can remain
at 1.962 N·m to balance the load.

In Control Studio we compare PD and PID under the same drum load, retaining
torque limits and anti-windup. We check remaining position error and holding
torque, then examine the measurement noise affecting derivative action.

## 36. Derivative filtering

**Derivative filtering** reduces rapid noise-driven changes in the motion
estimate used for derivative control.

When velocity is calculated from angle samples:

$$
\omega_{\mathrm{raw},k}=
\frac{\theta_{m,k}-\theta_{m,k-1}}{\Delta t},
$$

even a small angle error can become a large velocity error if divided by
a short interval. A 0.002 rad difference over 0.02 s represents 0.1 rad/s.
At $K_d=7$, that produces 0.7 N·m of derivative-torque change.

A first-order filter gives:

$$
\omega_{f,k}=\omega_{f,k-1}
+\frac{\Delta t}{T_f+\Delta t}
(\omega_{\mathrm{raw},k}-\omega_{f,k-1}),
\qquad D_k=-K_d\omega_{f,k}.
$$

$\theta_m$ is measured angle, $\omega_f$ filtered velocity, $T_f$ filter time,
and $D$ derivative torque. Previous values are retained between updates.

The filter reduces rapid fluctuations but also delays recognition of real
motion. Too much filtering can weaken useful braking.

In Control Studio we compare the same angle-noise and target records with
filtering bypassed and enabled. We assess torque activity together with
overshoot and settling.

Next we put all PID terms into one consistent sampled update.

## 37. Discrete PID

A **discrete PID controller** calculates its output at separate update times,
using stored values from previous updates.

The calculation order matters. For measured angle $\theta_m$, target $r$
and elapsed time $\Delta t$:

$$
e_k=r_k-\theta_{m,k},\qquad P_k=K_pe_k,
$$
$$
I_k^*=I_{k-1}+K_ie_k\Delta t,
$$
$$
u_{r,k}=P_k+I_k^*-K_d\omega_{f,k},
\qquad
u_{a,k}=\operatorname{clamp}(u_{r,k},-20,20),
$$
$$
I_k=I_k^*+\frac{\Delta t}{T_t}(u_{a,k}-u_{r,k}).
$$

$k$ identifies the current update; $I^*$ is the trial integral before tracking.
$u_r$ and $u_a$ are requested and applied torque. The clamp enforces ±20 N·m.
$T_t$ is tracking time and $\omega_f$ is the filtered velocity from Lesson 36.

For error 0.08 rad, $K_p=16$, $K_i=4$, $\Delta t=0.02$ s and old I = 1.962,
P is 1.28 N·m and trial I is 1.9684 N·m. If unfiltered velocity is 1 rad/s
and $K_d=7$, requested torque is −3.7516 N·m.

In Control Studio we inspect consecutive updates and compare one with this
arithmetic. Next we derive the alternative incremental implementation.

## 38. Positional and incremental PID

A **positional PID** calculates the complete output request each update.
An **incremental PID** calculates a change and adds it to the previous request.

They can describe the same controller, but only if their timing, retained state
and correction rules agree.

Subtracting consecutive positional requests with fixed gains gives:

$$
\Delta u_k=K_p\Delta e_k+K_i e_k\Delta t
-K_d\Delta\omega_{f,k}+\gamma_{k-1}.
$$

$\Delta e$ and $\Delta\omega_f$ mean current minus previous values.
$\gamma_{k-1}$ is the previous update's after-request anti-windup correction.
The current request is $u_{r,k}=u_{r,k-1}+\Delta u_k$.

Retaining clipped applied output instead of the previous unlimited request
changes this recurrence unless a corresponding tracking policy is included.

For previous error 0.10, new error 0.096, velocity 0.20→0.22,
$K_p=16$, $K_i=4$, $K_d=7$, and $\Delta t=0.02$:

$$
\Delta u=-0.064+0.00768-0.14=-0.19632\ \mathrm{N\,m}.
$$

A previous request of 1.2 becomes 1.00368 N·m, matching the positional calculation.

In Control Studio we compare both forms under identical inputs, limits and
mode changes. Matching their arithmetic is distinct from commissioning the
complete servo.

## 39. Servo commissioning

**Servo commissioning** checks whether the complete motion controller meets
positioning, load and operating-state requirements.

The controller must do more than reproduce the PID equation. Its measurement
handling, torque limits, initialization, transfer and fault response must
work together.

We reuse the same shaft and drum load. The load calculation
$\tau_L=mgr=1.962$ N·m establishes the holding torque; it does not establish
how quickly or accurately the controller recovers after the load is applied.

In Control Studio we test:

- Position changes and settling.
- Rejection of the taught constant load.
- Requested and applied torque at the limits.
- Startup and manual-to-automatic transfer.
- Invalid measurement handling and deliberate recovery.

Each case uses its declared reset and unchanged requirements. We inspect
angle, velocity, torque and controller state together.

We recommend the integration only for the conditions its evidence supports.
This is a taught simulation assessment; physical equipment commissioning
requires separate verification.

The next application is a tank. Its actuator changes flow, and flow changes
stored volume, so we derive that physical relationship before designing its control.

## 40. Volume balance

A **volume balance** calculates how stored liquid changes from inflow
minus outflow.

For incompressible water in a tank of constant area:

$$
A\frac{dh}{dt}=q_{\mathrm{in}}-q_{\mathrm{out}}.
$$

- $A$: tank cross-sectional area, in m².
- $h$: water level, in metres.
- $q_{\mathrm{in}}$ and $q_{\mathrm{out}}$: flows, in m³/s.

Over an interval, net flow adds volume
$\Delta V=(q_{\mathrm{in}}-q_{\mathrm{out}})\Delta t$.
Because $\Delta V=A\Delta h$, dividing by area gives the level change.

With area 0.5 m², inflow 0.012 and outflow 0.010 m³/s, level rises at
0.004 m/s. Ten seconds adds 20 litres and raises level from 0.25 to 0.29 m.

Returning inflow to 0.010 m³/s balances the outlet and holds 0.29 m.
It does not restore the original 0.25 m because the added water remains stored.

In Control Studio we predict the level slope, apply the bounded flow change
and compare the trace. Next we replace constant outflow with gravity drainage.

## 41. Gravity outflow

**Gravity outflow** is drainage driven by the height of liquid above an outlet.

A taller water column creates greater pressure. For an open tank, gravitational
energy per unit mass is $gh$. Equating it to velocity energy $v^2/2$ gives
outlet velocity $v=\sqrt{2gh}$.

After accounting for outlet area and discharge losses:

$$
q_{\mathrm{out}}=c\sqrt h,
\qquad
A\dot h=q_{\mathrm{in}}-c\sqrt h.
$$

$h$ is level in metres, $A$ is tank area and $c$ combines outlet geometry,
gravity and discharge coefficient. Its units are m$^{5/2}$/s.

With $c=0.020$, level 0.25 m gives outflow 0.010 m³/s.
If inflow increases to 0.011, the equilibrium is:

$$
h_{\mathrm{eq}}=\left(\frac{q_{\mathrm{in}}}{c}\right)^2
=\left(\frac{0.011}{0.020}\right)^2=0.3025\ \mathrm m.
$$

As level rises, drainage increases until it balances inflow.
The square-root relationship is nonlinear: doubling level does not double flow.

In Control Studio we compare this response with the previous constant-outflow
model. Next we approximate the curved relationship near one operating level.

## 42. Linearization

**Linearization** replaces a nonlinear relationship near an operating point
with a straight-line approximation having the same value and slope there.

It gives a simpler model for small changes. Its accuracy generally decreases
as we move farther from that operating point.

For gravity outflow $q=c\sqrt h$, the slope at level $h_0$ is:

$$
g=\frac{c}{2\sqrt{h_0}},
\qquad
q_{\mathrm{out}}\approx q_0+g(h-h_0).
$$

$q_0$ is outflow at $h_0$. With deviations $x=h-h_0$ and
$v=q_{\mathrm{in}}-q_0$, the volume balance becomes:

$$
A\dot x=v-gx.
$$

The local gain is $K=1/g$ and time constant is $\tau=A/g$.
At $h_0=0.25$ m, $c=0.020$ and $A=0.5$, these are
$g=0.020$ m²/s, $K=50$ s/m² and $\tau=25$ s.

A 0.001 m³/s inflow increase predicts equilibrium 0.3000 m;
the nonlinear model gives 0.3025 m.

In Control Studio we compare small-step predictions and observations,
then calculate how the error grows for a larger departure. The next lesson
adjusts controller gains when the operating level changes.

## 43. Gain scheduling

**Gain scheduling** changes controller gains according to a known operating
condition, such as measured water level.

We linearized the tank near one level, but its response changes across the
operating range. At higher levels, another centimetre adds less extra drainage
than it does at lower levels. The same small increase in inlet flow therefore
produces a larger, slower level change.

Instead of manually retuning at every level, we calculate suitable gains from
the local model or select them from a prepared table.

For this tank and a chosen local response time $\lambda$:

$$
K_p=\frac{A}{\lambda},
\qquad
K_i(h)=\frac{c}{2\lambda\sqrt h}.
$$

$A$ is tank area, $c$ its outlet coefficient, $h$ measured level and
$\lambda$ desired response time in seconds. These follow by applying the
model-based PI rule to local gain $1/g$ and time constant $A/g$.

With $\lambda=10$ s, $K_p=0.05$ m²/s at every level.
$K_i$ decreases from 0.0025 at 0.16 m to 0.001667 m²/s² at 0.36 m.

Only integral gain changes here. We retain its accumulated holding flow;
we do not recalculate all past accumulation with the new gain.

In Control Studio we compare fixed and scheduled PI at low, middle and high
levels. We check whether scheduling gives more consistent acceptable responses.
