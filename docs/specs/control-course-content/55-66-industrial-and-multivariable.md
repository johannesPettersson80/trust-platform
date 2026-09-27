# Lesson explanations: 55–66

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 55. Feedforward control

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

## 56. Cascade control

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

The outer vessel controller outputs a jacket-temperature setpoint.
The inner jacket controller outputs heater power. Those outputs have different
units and must not be connected as if they were interchangeable.

We first tune and verify the faster inner loop, then close the outer loop.

In Control Studio we compare ordinary vessel PI and cascade under the same
jacket disturbance. We inspect both temperatures and the inner setpoint.

The next production requirement is different: maintaining a proportion between material flows.

## 57. Ratio control

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

## 58. Split-range control

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

## 59. Override control

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

In Control Studio we observe both requests, which one is selected,
jacket temperature and vessel error. We assess the stated 42 °C jacket limit
without pretending the controller can satisfy physically incompatible demands.

Override handles authority and constraints. The next lesson addresses
a separate problem: delayed heat delivery.

## 60. Smith prediction

A **Smith predictor** uses a process model to compensate for a known input
delay within the feedback calculation.

Ordinary heater feedback sees the delayed response. The predictor combines
what the model says should happen without delay with a correction from
the actual measurement:

$$
y_{\mathrm{pred}}=y_{m,0}+(y-y_{m,L}).
$$

- $y$: measured process output.
- $y_{m,0}$: model output without the delivery delay.
- $y_{m,L}$: model output including the delivery delay.
- $y_{\mathrm{pred}}$: signal used by the controller for feedback.

All terms use the same temperature reference.

If the delayed model matches measurement, their difference is zero and
the controller uses the delay-free prediction. If they disagree, the difference
corrects that prediction.

For $y_{m,0}=36$, $y_{m,L}=35.4$ and $y=35.2$ °C,
$y_{\mathrm{pred}}=35.8$ °C.

This does not make heat arrive sooner. Performance depends on the model and
delay being adequate.

In Control Studio we compare ordinary PI and predictor operation, then
introduce a declared model or delay mismatch and test fallback.
Next we select complete structures for the vessel's actual requirements.

## 61. Vessel commissioning

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

In Control Studio we compare two meaningful taught arrangements using the
same plant, reset and requirements. We inspect vessel and jacket temperatures,
service commands and controller states.

We recommend an arrangement only for the conditions it has met.
Next we study two coupled controlled temperatures, where each actuator
influences both outputs and controller pairing becomes a new design question.

## 62. Multivariable process paths

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

## 63. Relative gain array

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

## 64. Decentralized PI

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
limits and applied-output tracking.

Separate controllers do not make the physical process independent.
When heater 1 warms vessel 1, heat also reaches vessel 2.
PI 2 reduces its heater only after its measured temperature begins to rise.

In Control Studio we change one target while requiring the other temperature
to remain near its original value. We repeat for the other target and
separate load disturbances, inspecting both temperatures and outputs.

Both steady errors may eventually vanish while the unchanged vessel has
an unacceptable transient excursion. Next we coordinate the inputs
to reduce that cross effect earlier.

## 65. Static decoupling

**Static decoupling** coordinates several actuator commands to cancel
predicted steady cross effects.

Separate PI loops wait for the other output to move before correcting it.
A decoupler uses the gain matrix to command compensating action immediately.

Let $v$ contain the PI controllers' virtual requests and $u$ the physical
heater-command deviations:

$$
u=Mv,\qquad
M=\begin{bmatrix}1&-4/9\\-4/9&1\end{bmatrix}.
$$

For the taught gain matrix, $KM=(1/9)I$, where $I$ is the identity matrix.
The off-diagonal products become zero, so each virtual request has only
its intended steady output effect.

A virtual request of $[5,0]^T$ produces physical changes
$[5,-2.2222]^T$ points. The second heater decreases to compensate for
heat arriving from the first vessel.

The physical 75% holding commands are added after this deviation mapping.
When physical outputs clip, tracking corrections must be converted back
through $M^{-1}$ to the virtual controller states.

In Control Studio we compare paired PI and decoupled PI on the same cases.
Steady cancellation does not guarantee perfect transient cancellation.
Next we test errors in the assumed coupling.

## 66. Multivariable design selection

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

In Control Studio we keep the paired and decoupled controllers fixed,
then test both target directions and both load locations at all three
coupling values. We compare cross excursion, recovery and output limits.

Retuning every case would change the question. We are selecting a design
that handles uncertainty, not finding a different nominal controller each time.

Next we describe internal storage explicitly with state-space models.
