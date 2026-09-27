# Lesson explanations: 76–82

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 76. Lyapunov stability

**Lyapunov stability analysis** uses a function of the system state to establish
whether motion remains near an equilibrium.

A successful simulation shows one trajectory. A Lyapunov argument can establish
behavior for every initial state in a specified region.

For the upright arm, choose:

$$
V=\frac12J\omega^2+\frac12K_p\phi^2+G(\cos\phi-1).
$$

$\phi$ is angle from upright, $\omega$ angular velocity, $J$ inertia,
$K_p$ position gain and $G$ the gravity-torque coefficient.
$V$ combines kinetic energy and the potential shaped by gravity and feedback.

Using the arm dynamics with derivative braking gives:

$$
\dot V=-(B+K_d)\omega^2.
$$

$B$ is physical friction and $K_d$ velocity-feedback gain.
Their positive values make $V$ nonincreasing.

For the taught gains, $V$ is zero at upright rest and positive elsewhere.
Within a suitable level set, it bounds angle and speed while keeping torque
within the actuator limit. The only motion that can remain indefinitely
with $\dot V=0$ in this region is upright rest, establishing convergence there.

A nonpositive derivative alone is not always sufficient to establish convergence;
the state and invariant-motion conditions matter.

In Control Studio we compare angle, speed, torque and $V$ for initial states
inside the justified region. Next we compensate gravity when holding a nonzero angle.

## 77. Feedback linearization

**Feedback linearization** uses a nonlinear model-based control action to
produce simpler, linear closed-loop dynamics under the model's assumptions.

For the arm, gravity torque changes with angle. PD alone needs position error
to provide holding torque at a nonzero angle. We can calculate an opposing
gravity term directly.

The plant and command are:

$$
J\ddot\phi+B\dot\phi=\tau+G\sin\phi,
$$
$$
\tau=K_p(r-\phi)-K_d\dot\phi-\hat G\sin\phi.
$$

$\phi$ is arm angle, $r$ desired angle, $J$ inertia, $B$ friction,
and $\tau$ applied torque. $G$ is actual gravity coefficient;
$\hat G$ is the value used by the controller.

Substitution gives:

$$
J\ddot\phi+(B+K_d)\dot\phi+K_p\phi
=K_pr+(G-\hat G)\sin\phi.
$$

If $\hat G=G$, the nonlinear gravity term cancels in this model.
An incorrect coefficient leaves a residual; a wrong sign can worsen the response.
Torque limits also prevent arbitrary compensation.

In Control Studio we compare no compensation, imperfect compensation
and nominal compensation for the same nonzero target. We inspect error,
motion and torque.

Next we estimate parameter changes when fixed model values become inadequate.

## 78. Adaptive control

**Adaptive control** updates controller parameters using information learned
from the operating process.

Ordinary PI can remove steady error after heater heat loss changes.
Adaptation has a different purpose: revise the model and tuning so the
transient response remains suitable.

For the heater:

$$
C\dot T=a u_{\mathrm{delayed}}-H(T-T_a)-D.
$$

If thermal capacity $C$, power conversion $a$ and disturbance $D=0$ are known,
define $x=T-T_a$ and $y=a u_{\mathrm{delayed}}-C\dot T$.
Then $y=Hx$.

Choosing the heat-loss coefficient that minimizes squared balance errors gives:

$$
\hat H=\frac{\sum_i x_i y_i}{\sum_i x_i^2}.
$$

$i$ identifies observations and the denominator must be nonzero.
This is a **least-squares estimate**: positive and negative residuals
cannot cancel in the minimized squared-error sum.

For $(x,y)=(15,120)$ and $(16,128)$, the estimate is 8 kW/°C.

Unknown extraction or sensor bias can be confused with changed heat loss.
We therefore qualify the data and proposed estimate before adopting new gains.

In Control Studio we compare fixed and revised tuning at the same changed
heat-loss condition, and also reject an unsuitable estimation window.
Next we design for uncertainty that cannot reliably be identified away.

## 79. Robust control

**Robust control** designs for a stated range of possible process models,
rather than assuming one nominal model is exact.

We use it when uncertainty is known well enough to bound but cannot be
reliably estimated away during operation.

**Robust stability** means the loop is stable for every allowed model.
**Robust performance** additionally means it meets the specified response
and actuator requirements throughout that uncertainty set.

Capacity must be checked first:

$$
u_{\mathrm{hold}}=\frac{H(T-T_a)+D}{a}.
$$

$H$ is heat-loss coefficient, $T-T_a$ temperature above ambient,
$D$ extra extraction and $a$ power per command point.

At $H=12$, ambient 20 °C and $a=2$, holding 36 °C needs 96%.
Holding 38 °C would need 108%, so no tuning can meet that requirement
with the available heater.

In Control Studio we keep each candidate fixed while varying the declared
heat-loss and delivery-delay cases. We compare the same temperature requirements,
using a physically correct reset for each case.

Passing a finite grid demonstrates those tested cases. A claim covering every
intermediate value also needs a justified bound or stability argument.
Next we address uncertainty in command delivery over a network.

## 80. Networked control

**Networked control** exchanges measurements or commands through a communication
link whose delay, ordering and availability affect operation.

A fresh calculation is not useful if an old packet arrives afterward and
overwrites it. A lost connection must not leave an old remote command
applied indefinitely.

We track:

- **Sequence number:** which command is newer within a connection.
- **Connection identity:** which established session a command belongs to.
- **Age:** time since the command was sent or the measurement acquired.
- **Hold time:** how long the last accepted command has remained active.

With a shared clock:

$$
\mathrm{age}=t_{\mathrm{now}}-t_{\mathrm{sent}}.
$$

Unsynchronized equipment clocks require an offset and uncertainty treatment
before this subtraction can establish freshness.

The example accepts only newer commands in the current connection with age
at most 0.2 s. After more than 0.3 s without an accepted command, it transfers
to local control, provided local measurement is valid.

In Control Studio we introduce dropped, delayed, reordered and old-session
packets. We observe accepted commands, held output, fallback and deliberate
recovery. Network delay is separate from the heater's physical delivery delay.

Next we handle measurements that arrive correctly but are themselves faulty.

## 81. Fault-aware control

**Fault-aware control** changes its behavior when measurements or components
can no longer support the normal control decision.

Two temperature sensors can fail differently. One may be clearly invalid
or stale, allowing use of the other. Both may instead be fresh and plausible
but disagree, which does not tell us which one is correct.

For two readings:

$$
d=|T_1-T_2|.
$$

$d$ is disagreement in °C. With each sensor known within ±0.1 °C,
their errors alone can produce a difference of 0.2 °C.

The example detects disagreement above 0.4 °C sustained for 0.2 s.
At 0.1-second updates, first detection at 5.0 s reaches that duration at
5.2 s. Separate short disagreement periods do not accumulate into one
continuous event.

An unresolved disagreement or loss of both valid sensors latches a fault
and removes heat. A latched fault requires the specified fresh-data and
deliberate recovery conditions; it does not disappear merely because one
displayed value looks normal.

In Control Studio we test invalid, stale and disagreeing sensors separately,
observing measurement selection, timing, output and recovery.

The final assessment combines the taught methods into a justified complete design.

## 82. Independent design

**Independent control design** means choosing and integrating suitable methods
for a physical problem, then defending that choice with calculations and evidence.

The task is not to include every technique from the course.
It is to explain which requirements need which methods.

We select a taught plant and two meaningful alternatives. Before testing,
we declare targets, disturbances, actuator limits, measurement assumptions,
uncertainty and operating-state requirements.

A physical feasibility calculation comes first. For a heater:

$$
u_{\mathrm{hold}}=\frac{H(T-T_a)+D}{a}.
$$

The heat-loss coefficient $H$, temperature difference, disturbance $D$ and
power conversion $a$ determine required holding output. A shaft or tank
instead uses its corresponding torque or flow balance.

We then write the ST integrations using understood units, retained state,
initialization, limits and recovery rules.

In Control Studio we run matched tests for both alternatives. We compare
every requirement, not just the most attractive trace, and recommend one
or explain why neither qualifies.

The recommendation states which conditions were demonstrated and which remain
untested. Completing this simulation design is distinct from verifying the
implementation on physical equipment.
