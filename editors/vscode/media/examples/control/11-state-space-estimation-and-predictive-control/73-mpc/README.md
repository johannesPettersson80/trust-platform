# 75. Model predictive control

You will use model predictive control (MPC) to allocate a limited power supply between two heaters. You will predict future temperatures, reject command plans that exceed the supply, and choose a feasible plan before applying its first command.

## The problem

Each vessel needs temperature control, but the two heaters share one 200 kW supply. An independent PI loop can request power without knowing what the other loop needs. Each heater is 200 kW at 100%, but the shared supply permits $u_1+u_2\le100$ points. Use $C=50$ kJ/°C, $H=10$ kW/°C, ambient 20 °C, 1 s input delay and 1 s Euler planning interval. Start T1=T2=28 °C, previous commands 40/40, and initial setpoints 28/28 °C. The test changes SP1 to 31 °C and leaves SP2 at 28 °C. These heaters have no thermal coupling; only their supply is shared.

## What we need

We need to choose both heater commands while accounting for their shared
power supply and predicted temperature limits. The decision must look ahead
before applying a command and revise the plan as new measurements arrive.

## The new idea

**Model predictive control**, or **MPC**, predicts future behavior for possible
input plans, chooses the best feasible plan, applies its first command, then
repeats the decision using new observations. "Feasible" means satisfying the
declared constraints; the cost selects among those feasible alternatives.

Here we plan both heater commands together because they share a supply.
The **prediction horizon** is the three future updates considered in each
decision. We do not apply the whole plan blindly: we replan after the next measurement.

Use a three-step teaching horizon. Candidate commands lie on a 5-point grid, each 0–100, with pair sum ≤100: 231 possible pairs. Hold one selected pair for the next three updates (move blocking), compare those 231 constant-pair plans, and apply only the first pair. The one-second delay means the first predicted temperature depends on the previous command. A three-second horizon includes two temperature changes caused by the newly selected pair. Restricting a whole plan to one pair is called **move blocking**; it keeps this first planning exercise small.

The energy balance divided by 50 kJ/°C gives a temperature rate. Multiplying that rate by the one-second planning interval gives the Euler prediction below; the previous applied command represents the delivery delay. The rollout is

$$
T_{next}=T+\frac{2u_{previous}-10(T-20)-D}{50}.
$$

Reject a plan if either command leaves 0–100%, their sum exceeds 100%, a predicted temperature leaves 26–33 °C, or tank 2’s predicted temperature leaves 28±.2 °C. Score each remaining three-step plan with

$$
J=\sum_{k=1}^{3}[(T_{1,k}-SP_1)^2+(T_{2,k}-SP_2)^2]
+.002[(u_1-u_{1,previous})^2+(u_2-u_{2,previous})^2].
$$

The command penalty appears once because later commands in this constant-pair plan do not change. The first term has °C²; the second uses the stated normalized design weight 0.002 °C²/point². For first pair 40/40, the temperature cost is 27. For 55/40, predictions are 28,28.6,29.08 for vessel 1 and 28 for vessel 2, giving (9+5.76+3.6864+0.45=18.8964). For 60/40, predictions 28,28.8,29.44 give (9+4.84+2.4336+0.8=17.0736). Sequence 60/50 is rejected because its sum is 110.

This enumeration is a finite move-blocked teaching controller, not a global optimum. If no plan satisfies the prediction constraints, report planning failure and select the defined fallback; a valid plan need not reach its setpoint within three seconds. At equilibrium, 31 °C needs $u_1=5(31-20)=55$%, while 28 °C needs 40%, total 95%. Adding a 30 kW load to vessel 2 increases its requirement by 15 points: the total becomes 110%. That is an infeasible setpoint pair, not a failed optimizer.

## Prediction

Calculate the first rollout values and costs above. Predict why a 30 kW load on vessel 2 cannot meet both 31/28 targets under the 100-point supply. With priority assigned to vessel 2 at 28 °C, its nominal need is 55 points, leaving 45 for vessel 1, whose equilibrium is 29 °C; both targets cannot be met.

## Implementation

Write one scalar prediction for each temperature, then repeat it three times
using the candidate pair. Calculate cost and a Boolean feasibility result.
Initialize the selected candidate as absent; when a feasible candidate has
lower cost, retain its index and cost. The enumeration repeats this calculation
over the grid; inspect the three worked plans yourself. Apply the first pair,
retain the actual applied commands and replan. Invalid PV commands both
outputs zero; use Lesson 28's recovery principles with timing suited to these
one-second updates. Keep model revision and fallback visible.

While MPC owns the outputs, keep both inactive fallback PI states tracking their actual allocated heater commands. On takeover with valid PV, preload each integral as $I_i=u_{i,applied}-P_i$ using its current proportional term, and skip ordinary integration on that transfer update. Both initial PI requests then match the previously applied commands. Resume ordinary integration and allocation tracking on subsequent updates; the fallback gives vessel 2 priority as specified below.

## Experiment in Control Studio

1. Reset T1=T2=28 °C, previous/applied commands 40/40, initial setpoints 28/28 and D=0.
2. Declare 60 s requirements: T1 peak ≤32 °C, |T1−31|≤0.2 °C from 30–60 s, T2 within ±0.2 °C, command sum ≤100 and individual bounds. For the comparison, use paired PI gains $K_p=2.5$ points/°C, $K_i=.5$ points/(°C·s), initial integrals 40%, one-second updates, no filter and tracking time 1 s. After limiting each PI request to 0–100%, scale both by $a=\min(1,100/(u_1+u_2))$ if their sum is positive; if zero, apply two zeros. Correct each integral using its allocated applied command. Both arrangements face the same supply and temperature requirements.
3. Start at **Confirm and start**, step SP1 to31 °C and apply the first selected move each 1 s, and display rollouts, cost, selected plan, temperatures and commands.
4. Separately reset and repeat the setpoint event with a simultaneous 30 kW load on vessel 2. Declare priority for vessel 2 before this 60 s test: use local PI with vessel 2 allocated first, leaving only the unused supply for vessel 1. Track both integrals from their allocations. Require the same total-power bound and vessel-2 late ±.2 °C band; report that vessel 1 cannot maintain 31 °C. This priority fallback also handles planning failure with valid sensors.
5. Abort if true temperatures leave 26–33 °C, an applied command leaves 0–100%, the total exceeds 100%, data become invalid, or the monitor stops updating. Invalid PV commands both heaters to zero; cancel events, stop the simulator if needed and reset both temperature, controller and one-second command-history states before repeating.

## Evidence and decision

Check arithmetic, selected finite plan, applied command and measured prediction error. Compare MPC and allocated PI against identical requirements. Do not claim optimality beyond the enumerated move-blocked set.

## Conclusion

MPC here is transparent finite planning: roll out a delayed model, reject shared-supply violations, score temperature and movement, apply one move, and replan.

## Reason for the next lesson

Feasible predictions and successful finite runs do not establish stability
for every starting condition in a nonlinear operating region.
[Lyapunov stability](../../12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability/README.md)
uses an upright arm to develop an energy-based regional argument, then
compares that argument with simulated motion.
