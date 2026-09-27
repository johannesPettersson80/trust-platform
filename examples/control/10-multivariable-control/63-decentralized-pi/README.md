# 64. Decentralized PI

You will commission two paired PI loops on the coupled heater and decide whether each loop meets changed and unchanged-vessel requirements under setpoint changes and load disturbances.

## The problem

Pairing each heater with its own temperature gives two PI loops. They still interact because heat crosses the wall between vessels. We must test both together. For a 2 °C setpoint change in vessel 1 with vessel 2 held at 35 °C, substitute $x_1=2$, $x_2=0$ into $18x_1-8x_2=2v_1$ and $-8x_1+18x_2=2v_2$. They give $2v_1=36$ and $2v_2=-16$, hence $v_1=18$, $v_2=-8$, so powers are 93% and 67%. These commands come from the plant physics, not from PI gains. Each controller must counter the other vessel’s coupling.

## What we need

We need to establish whether the selected paired PI loops regulate both
temperatures acceptably when they run together. A target change or disturbance
in one vessel must also be judged by its effect on the other vessel.

## The new idea

**Decentralized PI control** uses a separate PI controller for each selected
measurement–actuator pair. Each loop has its own error, integral state and
applied-output tracking. Separate controllers do not make the physical
process independent: heat still crosses the wall.

For loop i, the error remains $e_i=SP_i-T_i$.

Use dt=0.1 s, $T_t=1$ s, $\beta=1$, no filter, and fully reset both loops to 35/35 °C, 75/75%, I=75% each. If no plant-specific gains are available, choose initial $K_p=5$ points/°C and $K_i=1$ point/(°C·s) per loop as provisional starting gains. They come from applying the earlier model-based rule to each uncoupled heater with K=.2 °C/point, time constant 5 s and response choice 5 s; coupling makes that approximation incomplete. Any retuning multiplies one gain by a selected factor from 0.8 to 1.2, at most three gain sets, testing all four events for each set and retaining the chosen gains unchanged for decoupling.

Requirements are fixed before tests: each direction’s 35→37 °C, 60 s test has changed-vessel peak ≤37.5 °C, absolute error ≤0.2 °C from 30–60 s, unchanged vessel within ±0.2 °C throughout, and outputs 0–100%. Separately apply D=10 kW to each vessel at SP=35 °C for 60 s; loaded vessel minimum ≥34 °C, absolute error ≤0.2 °C from 30–60 s, other vessel within ±0.2 °C, outputs bounded.

For the provisional gains 5 and 1, a vessel-1 target step from 35 to 37 °C
gives P1=10 points and trial I1=75.2%, so heater 1 first requests 85.2%.
Vessel 2 still has zero error and requests 75%. Only after transferred heat
raises T2 does its PI reduce heater-2 power. The unchanged vessel can therefore
have an unacceptable excursion even when both steady errors eventually vanish.

A separate 10 kW extraction increases the loaded vessel's nominal holding
requirement from 75% to 80%; the other remains 75%. These steady balances
do not predict the entire interacting transient.

## Prediction

Calculate 93/67% for the 2 °C vessel-1 target using the balance, and 80/75% for a 10 kW single-vessel load. Predict the initial direction of both requests and which unchanged-vessel requirement is most sensitive to pairing.

## Implementation

Write two scalar PI instances with explicit P, trial I, request, limit, tracking and applied output. Initialize each from applied 75%; preserve requested/applied distinction and revision. Write one controller-instance call per vessel and use distinct retained integral states; inspect a single scan to confirm that changing one measurement changes only its paired PI calculation. The plant then carries that output’s effect to both temperatures.

## Experiment in Control Studio

1. Reset both vessels and states. Run vessel-1 target step for 60 s from **Confirm and start**, then full reset and vessel-2 target step.
2. Full reset and run D=10 kW on vessel 1 at fixed SP=35; reset and repeat D on vessel 2. Display temperatures, errors, P/I, requests/applied outputs and loads.
3. Stop on T outside 0–80 °C, output outside 0–100%, invalid data or unresponsive signals. Command zero heat on an abort, cancel both disturbances, and stop the simulator if needed. At normal completion restore both nominal commands, then restart and reset each case.

## Evidence and decision

Assess every requirement separately, including both directions and both load tests. Retain the chosen gains and any bounded gain comparison. A pass in one direction does not qualify the pairing; freeze only a complete evidence set for Lesson 65.

## Conclusion

Decentralized PI must be judged as two interacting loops. Plant balance predicts steady commands, while matched target/load tests reveal transient interaction and feasibility.

## Reason for the next lesson

Each independent PI corrects the other vessel’s influence after its own temperature has moved. A known cross effect could instead be countered by coordinating the heater requests when the first request changes. The steady gain matrix supplies a starting calculation for that coordination.

[Static decoupling](../64-decoupling/README.md) derives the map and tests whether its transient cost is acceptable under these same requirements.
