# Lesson explanations: 55–66

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 61. Trajectory feedforward

A **motion profile** plans a move's angle $r(t)$ with its rate $\dot r$ and
acceleration $\ddot r$. The **trapezoidal profile** accelerates at $A$ to the
maximum rate $V$, cruises and brakes at $A$; a move of $D$ takes $D/V + V/A$, or
$2\sqrt{D/A}$ when $D < V^2/A$. The drum axis obeys $J\ddot\theta + B\dot\theta = u$,
so the torque with which it follows the plan is the **trajectory feedforward**
$u_{ff} = J\ddot r + B\dot r$. Control Studio designs it from the model
$n/(a s^2 + b s)$ as $K_v = b/n$ and $K_a = a/n$ (Specification 35 §17.16), which
for $1/(s(Js + B))$ are $B$ and $J$. The PID compares the planned angle with the
measured one; its derivative acts on that error, which has no jumps, because on
the measurement alone it would subtract $K_d\dot\theta$ during every move.

With Lesson 42's PID and a 1 rad move at 1 rad/s and 2 rad/s², the PID alone
lags the plan by up to 0.09 rad; with the feedforward the lag stays below
0.03 rad. Control Studio's Validate checks that following error on the model and
on an axis with 20 % more inertia and friction, where the feedback corrects what
the feedforward's model gets wrong; with the feedforward set to zero the check
fails.

An unmeasured jacket disturbance still reaches the vessel before
vessel-temperature feedback can respond adequately.

## 62. Feedforward control

**Feedforward control** calculates an action from a measured disturbance
or reference before output error is needed to trigger that action.

Feedback waits until an unmeasured heat extraction changes water temperature.
If we measure the extraction itself, we can request replacement heat earlier.

For this heater:

$$
u_{\mathrm{ff}}=\frac{D_m}{a},\qquad u_r=P+I+u_{\mathrm{ff}}.
$$

$D_m$ is measured extra heat extraction in kW, $a$ is delivered kW per
command point, and $u_{\mathrm{ff}}$ is the additional command.
$P$ and $I$ remain the feedback contributions.

A 30 kW extraction with $a=2$ kW/point requires 15 extra output points.
From the 75% hot holding command, that predicts 90%.

Feedback is still needed because the disturbance measurement, heat conversion
or delivery timing may be imperfect. Feedforward predicts compensation;
it does not eliminate the physical delay before heat arrives.

In Control Studio we compare PI alone with PI plus feedforward for the same
extraction. We inspect action timing and temperature excursion, then repeat
with a declared measurement or delay error.

Next we use an intermediate measurement when the disturbance itself is not measured.

## 63. Cascade control

**Cascade control** uses two nested feedback loops. The outer controller
sets the target for a faster inner controller.

In a jacketed vessel, an unexpected heat loss in the jacket changes jacket
temperature before vessel temperature. The inner loop reacts to that earlier
measurement instead of waiting for a large vessel error.

The thermal balances are:

$$
C_j\dot T_j=Q-H_{jv}(T_j-T_v)-D_j,
$$
$$
C_v\dot T_v=H_{jv}(T_j-T_v)-H_v(T_v-T_a)-D_v.
$$

$T_j,T_v,T_a$ are jacket, vessel and ambient temperatures.
$C_j,C_v$ are thermal capacities; $H_{jv},H_v$ heat-transfer coefficients.
$Q$ is applied heating and $D_j,D_v$ extra extractions.

The outer vessel controller outputs a jacket-temperature setpoint (20–50 °C).
The inner jacket controller outputs heater power (0–100 %). Those outputs have
different units and must not be connected as if they were interchangeable.

The project holds two jacketed vessels at rest at 35 °C, jacket 38 °C, 15 %.
The first is held by two `Control.PID` blocks in cascade, each with its own
Auto/Manual switch and manual output, both starting in Manual. The second keeps
the single-loop PI of Lesson 62 ($K_p = 2.5$ %/°C, $K_i = 0.1$ %/(°C·s)).

In Control Studio the learner sets the cascade up with **Configure control
loop**, which writes one cascade loop. Control Studio identifies both
temperatures from one heater step with both PIDs in Manual, designs the inner
PI first and the outer PI through the closed inner loop, validates a 2 °C
setpoint step against the fixed requirements (within ±0.2 °C in 30 s, steady
error at most 0.3 °C) and applies both controllers. The project's
**Jacket-loss test** then takes 30 kW from both jackets, so the applied cascade
is compared with the single loop on the same unmeasured disturbance.

Native test: `lesson56` in `control_curriculum_lessons_55_57`.

The next production requirement is different: maintaining a proportion between material flows.

## 64. Ratio control

**Ratio control** keeps one process quantity in a specified proportion to another.

When carrier flow changes, a fixed additive flow would change the mixture.
We therefore make the additive-flow target follow measured carrier flow:

$$
q_{a,\mathrm{sp}}=R q_c.
$$

$q_c$ is carrier flow, $q_{a,\mathrm{sp}}$ the additive target in the same
flow units, and $R$ the required additive-to-carrier ratio.

For $R=0.1$ and carrier flow 100 L/min, the additive target is 10 L/min.
That is a 10% additive/carrier ratio, but additive is only
$10/(100+10)=9.09\%$ of the combined mixture.

A separate feedback controller makes the additive pump follow this moving
target. Pump dynamics mean actual flow cannot change instantly.

In Control Studio we change carrier flow and compare additive target with
actual flow. We also stop the carrier and observe residual additive delivery.
Zero command does not mean flow disappears at the same instant, and ratio
is undefined when its denominator is zero.

Next we return to the vessel and handle both heating and cooling.

## 65. Split-range control

**Split-range control** maps different regions of one controller output
to different actuators.

A vessel may require heating at one time and cooling at another.
One signed heat-demand signal can represent both:

$$
Q=2u_h-u_c.
$$

$Q$ is net applied heat in kW, $u_h$ heater command and $u_c$ cooler command
in percent. Here the heater supplies 2 kW per point and the cooler removes
1 kW per point.

For positive demand, heating supplies $u_h=Q/2$ and cooling is zero.
For negative demand, cooling supplies $u_c=-Q$ and heating is zero,
subject to their limits.

A 30 kW heating request needs 15% heater output.
A −30 kW demand needs 30% cooler output.

The unequal command scales matter. Adding their percentages would not give
net heat. Any neutral band and service-exclusion rule must be explicit.

In Control Studio we compare heating-demand and excess-heat cases, inspecting
both actuator commands, net heat and temperature. Tracking uses actual net
heat so the controller remains consistent with service limits.

Next we protect a separate jacket-temperature constraint.

## 66. Override control

**Override control** gives a limiting controller authority over the normal
controller when a constraint needs protection.

The vessel controller may request more heating while the jacket is already
too hot. A second controller reduces the allowed heating request.

For this heating constraint:

$$
u_r=\min(u_{\mathrm{normal}},u_{\mathrm{limit}}).
$$

$u_{\mathrm{normal}}$ is the normal vessel request and $u_{\mathrm{limit}}$
the jacket-limiting request. The minimum selects less heating.
Requests of 60% and 40% therefore produce 40%.

The selector direction depends on the physical constraint; minimum selection
is not correct for every override application.

The inactive controller must track the selected output. Otherwise its
integral can accumulate a request that causes a jump when it becomes active.

The project holds three heater-only jacketed vessels: the vessel PI alone
($K_p = 2.5$ %/°C, $K_i = 0.1$ %/(°C·s)), the override with tracking and the
override without tracking. The jacket-limit PI uses $K_p = 5$ %/°C and
$K_i = 10$ %/(°C·s), Lesson 19's model-based gains for the jacket alone
(0.2 °C/%, 0.5 s, $\lambda = 0.5$ s), with its setpoint 41.8 °C. The project
test requests 40 °C, which would need a 44 °C jacket, and 37 °C from 40 s.

In Control Studio we observe both requests, which one is selected,
jacket temperature and vessel error. We assess the stated 42 °C jacket limit
without pretending the controller can satisfy physically incompatible demands.

Native test: `lesson59` in `control_curriculum_lessons_58_60`.

Override handles authority and constraints. The next lesson addresses
a separate problem: delayed heat delivery.

## 67. Smith prediction

A **Smith predictor** uses a process model to compensate for a known input
delay within the feedback calculation.

Ordinary heater feedback sees the delayed response. The predictor combines
what the model says should happen without delay with a correction from
the actual measurement:

$$
y_{\mathrm{pred}}=y_{m,0}+(y-y_{m,L})=y+c,\qquad c=y_{m,0}-y_{m,L}.
$$

- $y$: measured process output.
- $y_{m,0}$: model output without the delivery delay.
- $y_{m,L}$: model output including the delivery delay.
- $y_{\mathrm{pred}}$: signal used by the controller for feedback.

For $y_{m,0}=36$, $y_{m,L}=35.4$ and $y=35.2$ °C,
$y_{\mathrm{pred}}=35.8$ °C. Both models are the first-order heater $K/(Ts+1)$,
one driven by the applied command $u$ and one by the command $\theta$ earlier,
so $T\dot c = K(u-u_\theta)-c$, updated exactly over each 0.1 s step.

This does not make heat arrive sooner, nor correct unmeasured loads faster.
Performance depends on the model and delay being adequate.

The project holds the heater of Lessons 12–30 with a 1 s delivery delay and one
`Control.PID` fed $y+c$. It starts in Manual at 75 %, with the slow PI of
Lesson 19 and model gain 0.

In Control Studio the learner sets the loop up with **Configure control loop**
as a Smith predictor, binding the controller, its Auto/Manual switch and manual
output, and the model gain, time constant, delay and correction variables.
Control Studio identifies a first-order model with delay from a heater step,
designs the PI for the delay-free model, validates a 0.5 °C step against the
fixed requirements and applies the design. With the **Manual** tuning method the
learner then validates the same design with a wrong model delay and with model
gain 0, the same gains on the measured temperature.

Native test: `lesson60` in `control_curriculum_lessons_58_60`.

Next we select complete structures for the vessel's actual requirements.

## 68. Vessel commissioning

**Vessel commissioning** checks whether a chosen combination of control
structures meets the vessel's performance, constraint and operating requirements.

Cascade, split-range and override solve different problems. We do not add
them all simply because they are available.

For the taught jacketed vessel, we first decide whether the application needs:

- Earlier correction of jacket disturbances.
- Cooling as well as heating.
- A maximum jacket temperature.
- The taught startup, transfer and fault-recovery behavior.

The heat balance establishes required capacity. At vessel 35 °C and jacket
38 °C, a 10 kW/°C transfer coefficient gives
$10(38-35)=30$ kW flowing into the vessel. The example heater supplies that
with 15% command.

That equilibrium calculation does not establish transient performance.

The project holds five jacketed vessels: the single loop and the cascade on a
35 → 37 °C setpoint test, the same on a 30 kW jacket loss, and a lifecycle
vessel on the cascade. The single loop uses $K_p = 2.5$ %/°C and
$K_i = 0.1$ %/(°C·s); the cascade uses Lesson 63's Control Studio design, inner
$K_p = 25.25$ %/°C and $K_i = 13.59$ %/(°C·s), outer $K_p = 0.8717$ and
$K_i = 0.21$ per second. The lifecycle vessel starts in Manual at 15 %,
switches to Auto at 10 s and has a failed sensor from 48 to 50 s.

In Control Studio one project test runs all five vessels with the same plant,
reset and requirements. We inspect vessel and jacket temperatures, service
commands and controller states.

Native test: `lesson61` in `control_curriculum_lessons_61_63`.

We recommend an arrangement only for the conditions it has met.
Next we study two coupled controlled temperatures, where each actuator
influences both outputs and controller pairing becomes a new design question.

## 69. Multivariable process paths

A **multivariable process** has several inputs and outputs.
A process path is the effect of one input on one output.

In two thermally coupled vessels, heater 1 changes both temperatures,
and so does heater 2. We need all four relationships before choosing
independent feedback pairs.

Near the common operating point:

$$
\begin{bmatrix}x_1\\x_2\end{bmatrix}
=
\begin{bmatrix}K_{11}&K_{12}\\K_{21}&K_{22}\end{bmatrix}
\begin{bmatrix}v_1\\v_2\end{bmatrix}.
$$

$x_i$ are steady temperature deviations and $v_i$ heater-command deviations.
$K_{ij}$ is the gain from input $j$ to output $i$, in °C per command point.

The matrix simply abbreviates two scalar sums, such as
$x_1=K_{11}v_1+K_{12}v_2$.

For the taught coupled balances:

$$
K=\begin{bmatrix}9/65&4/65\\4/65&9/65\end{bmatrix}.
$$

A 5-point heater-1 change predicts temperature rises of
0.6923 and 0.3077 °C.

In Control Studio we step heater 1 while holding heater 2, reset, then step
heater 2. Two experiments reveal four paths.

Next we examine how those gains change when the other output is actively held.

## 70. Relative gain array

The **relative gain array**, or RGA, is a table of dimensionless ratios
comparing input-output gains with other inputs fixed against the gains
with other outputs held fixed. We use it to judge controller pairings.

First measure a path with the other input held fixed. Then determine its
gain while the other output is held fixed by compensating action.
The ratio of these gains is the relative gain.

For the two-vessel example, the isolated heater-1 gain is $9/65$ °C/point.
Holding temperature 2 fixed changes the effective gain to $1/9$.
Their ratio is:

$$
\lambda_{11}=\frac{9/65}{1/9}=\frac{81}{65}\approx1.246.
$$

For the full steady gain matrix:

$$
\Lambda=K\circ(K^{-1})^T.
$$

$K^{-1}$ converts output changes back to required input changes.
The superscript $T$ exchanges rows and columns.
The circle means corresponding-element multiplication, not matrix multiplication.

A relative gain near one means the pairing changes little when the other
loops are closed. Negative values warn of a direction reversal under
the other-output constraint.

In Control Studio we compare fixed-input and compensated-output experiments.
RGA guides pairing, but it does not establish acceptable transient interaction.
Next we operate the paired PI loops together.

## 71. Decentralized PI

**Decentralized PI control** uses a separate PI controller for each
selected input-output pair.

For the coupled vessels, PI 1 uses temperature 1 to command heater 1,
while PI 2 uses temperature 2 to command heater 2.

Each controller has its own error and integral:

$$
e_i=SP_i-T_i,\qquad
I_{i,\mathrm{new}}=I_{i,\mathrm{old}}+K_{I,i} e_i\Delta t.
$$

The subscript $i$ identifies the loop; $K_{I,i}$ is that loop's integral gain.
Each complete request also contains its own proportional contribution,
limits and applied-output tracking. Each member is designed on its own path
with the gain it has when the other loop is closed, $K_{11}/\lambda_{11}$.

Separate controllers do not make the physical process independent.
When heater 1 warms vessel 1, heat also reaches vessel 2.
PI 2 reduces its heater only after its measured temperature begins to rise.

The project holds the coupled vessels at 35 °C and 75 % with a heat load on
each vessel, one `Control.PID` per vessel and one Auto/Manual switch for both.

In Control Studio the learner sets the two controllers up as a **PID group**.
Control Studio identifies both vessels with a multisine on each heater, designs
both PIs from the group model, listing each member's own path, relative gain and
gain with the other loops closed, validates both setpoints together and each
alone against the fixed requirements, and applies the design. A load trial uses
Studio's **Input disturbance**. The unchanged vessel's excursion in the
one-setpoint trials is judged against the ±0.2 °C requirement.

Native test: `lesson64` in `control_curriculum_lessons_64_66`.

Both steady errors may eventually vanish while the unchanged vessel has
an unacceptable transient excursion. Next we coordinate the inputs
to reduce that cross effect earlier.

## 72. Static decoupling

**Static decoupling** coordinates several actuator commands to cancel
predicted steady cross effects.

Separate PI loops wait for the other output to move before correcting it.
A decoupler uses the gain matrix to command compensating action immediately.

Let $v$ contain the PI controllers' requests and $u$ the heater commands:

$$
u=Mv,\qquad
M=K^{-1}\tfrac{1}{9}I=\begin{bmatrix}1&-4/9\\-4/9&1\end{bmatrix}.
$$

For the taught gain matrix, $KM=(1/9)I$, where $I$ is the identity matrix:
each request moves only its own temperature, by the gain it had with the other
loop closed. Control Studio designs $D=K^{-1}\,\mathrm{diag}(K)=(81/65)M$: the
same directions, scaled so that each loop keeps its own path's gain, 9/65 °C
per point.

Control Studio applies $u=Dv$ in absolute terms and starts each decoupled
controller at $v=D^{-1}u$ (Specification 35 §17.15). With heaters at 75 % that
is $75(1+4/9)=108.3$ %, outside the controllers' 0–100 %. The lesson therefore
runs the vessels at 30 °C, where the heaters hold at 50 % and the controllers
start at 72.2 %.

The project holds the coupled vessels at 30 °C and 50 %, one `Control.PID` per
vessel, one Auto/Manual switch, and the decoupler variables $D_{11}\ldots D_{22}$;
in Auto the program writes $u=Dv$, limited to 0–100 %, and in Manual each
request goes straight to its heater.

In Control Studio the learner sets the group up with the decoupler ticked.
Control Studio identifies both vessels, designs both PIs and $D$ as one group,
validates both setpoints together and each alone, and applies the gains and
$D$ together. A load trial uses Studio's **Input disturbance**.
Steady cancellation does not guarantee perfect transient cancellation.

Native test: `lesson65` in `control_curriculum_lessons_64_66`.

Next we test errors in the assumed coupling.

## 73. Multivariable design selection

**Multivariable design selection** compares complete controller arrangements
under the same requirements and justified uncertainty.

Our decoupler used one nominal coupling value. If actual heat transfer differs,
its compensation can be too small or too large. We need to know whether
decoupling still helps across the expected range.

At steady state the two-vessel balance is:

$$
\begin{bmatrix}10+H_c&-H_c\\-H_c&10+H_c\end{bmatrix}
\begin{bmatrix}x_1\\x_2\end{bmatrix}
=2\begin{bmatrix}v_1\\v_2\end{bmatrix}.
$$

$H_c$ is coupling conductance in kW/°C, $x_i$ temperature deviations
and $v_i$ heater-command deviations from 75%.

For targets 37 and 35 °C, coupling values 6, 8 and 10 require holding
commands 91/69%, 93/67% and 95/65%, respectively.

The project holds three sets of coupled vessels, with walls of 6, 8 and
10 kW/°C, and runs both arrangements as Control Studio designed them for
8 kW/°C, frozen: Lesson 71's decentralized PI and Lesson 72's decoupled group,
its gains and decoupler $D$ acting on deviations from the operating point. The
declared experiment setting **Coupling Case** selects the set a run exercises.

In Control Studio we run the project test once per coupling and compare cross
excursion, recovery and output limits for both target directions and both load
locations.

Retuning every case would change the question. We are selecting a design
that handles uncertainty, not finding a different nominal controller each time.

Native test: `lesson66` in `control_curriculum_lessons_64_66`.

Next we describe internal storage explicitly with state-space models.
