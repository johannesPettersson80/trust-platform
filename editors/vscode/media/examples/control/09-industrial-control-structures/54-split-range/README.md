# 58. Split-range control

You will convert a signed heat demand into separate heater and cooler commands. You will calculate the commands from actuator capacity and test how a neutral region affects temperature control when demand changes from heating to cooling.

## The problem

Return from the mixing operation to the jacketed vessel studied in cascade control. Its heater can add heat, but its heater cannot remove excess heat. If another process delivers heat into the jacket, setting the heater to zero may be insufficient to maintain vessel temperature. Add a cooler to investigate this new requirement.

Use the same jacket and vessel as [cascade control](../52-cascade/README.md). The cooler can remove 100 kW at 100% command over the stated jacket-temperature range. Treat this as a supplied actuator calibration. Heating still delivers 200 kW at 100%; no extra actuator lag is introduced in this example.

## What we need

We need one temperature-control demand to select and scale either heating
or cooling. The resulting actuator commands must deliver the intended net
heat while handling the transition between the two services.

## The new idea

**Split-range control** maps different regions of one controller output to
different actuators. Here positive net heat demand selects the heater and
negative demand selects the cooler. We limit the requested heat to the
available range, then convert it using the two actuator capacities.

Let $Q$ be net heat delivered to the jacket in kW: positive means heating and negative means cooling. With heater command $u_h$ and cooler command $u_c$ in percent,

$$
Q=2u_h-u_c,
$$
$$
5\dot T_j=Q-10(T_j-T_v)-D_j,
$$
$$
50\dot T_v=10(T_j-T_v)-2(T_v-20).
$$

The capacities allow $-100\le Q\le200$ kW. Start at $T_v=35$ °C, $T_j=38$ °C, $Q=30$ kW, heater 15% and cooler 0%. Keep the cascade lesson’s setpoint and jacket-extraction tests and requirements. Add a separate 60 s excess-heat test: setpoint 35 °C and $D_j=-60$ kW at time zero. Require vessel peak at most 36 °C and error within ±.2 °C from 30–60 s. Negative extraction here means 60 kW enters the jacket from the added heat source.

Without a neutral region, the command mapping is:

$$
u_h=\max(Q_r,0)/2,\qquad u_c=\max(-Q_r,0).
$$

A request of +40 kW means heater 20%, cooler 0%. A request of −40 kW means heater 0%, cooler 40%. Equal command percentages would not represent equal power because the capacities differ.

A **neutral region** turns both services off for $|Q_r|\le b$, where b is in kW. Outside it, use the same capacity conversion. Compare $b=0$ with $b=2$ kW. The 2 kW choice avoids small heating/cooling reversals, but discards small useful heat requests and introduces a jump at the boundary. It can therefore increase temperature cycling.

At 35/38 °C, 30 kW must pass from jacket to vessel. With 60 kW arriving from the new source, the controller must remove 30 kW: $Q=-30$ kW, cooler 30%. A heater-only controller cannot meet this balance.

## Prediction

Calculate both service commands for $Q_r=40,-40,1.5,-1.5$ kW with each b. Explain why the last two requests disappear with b=2. Predict the initial jacket slope after $D_j=-60$: $60/5=12$ °C/s before the controller changes delivered net heat.

Predict whether a neutral region matters at the settled −30 kW demand or mainly while crossing zero. State which temperature and command observations could reject it.

## Implementation

Retain the outer cascade gains and its integral initialized at 38 °C. Convert the inner controller from heater percent to kW by multiplying its gains and stored output by 2: $K_p=10$ kW/°C, $K_i=20$ kW/(°C·s), initial integral 30 kW. Use .1 s updates and 1 s tracking time.

Write `IF`/`ELSIF` branches for positive demand above b, negative demand below −b, and the neutral region. Set both output variables in every branch. Calculate actual net heat from the two limited service commands, then track the inner integral with $(dt/T_t)(Q_{applied}-Q_{requested})$. This correction must include the neutral region as well as the end limits. Inspect the four prediction cases before enabling temperature control.

## Experiment in Control Studio

1. Start **Simulator** and display both temperatures, outer jacket setpoint, inner net-heat request, heater/cooler commands, applied net kW and integral state.
2. Reset to the full 35/38 °C, 15%/0% state. Use **Test** to run each of the three specified 60 s events with b=0, then with b=2, resetting all physical and controller states before every record. Events start at **Confirm and start**.
3. Record vessel peak or minimum, the 30–60 s error, time in the neutral region, service reversals, and any request that differs from delivered net heat. Keep gains and requirements unchanged.
4. Abort if $T_j$ leaves 20–55 °C, $T_v$ leaves 30–40 °C, either applied command leaves 0–100%, both services become active, or data or clock become invalid. Set both service commands to zero and stop the simulator if needed. The external heat source still warms the jacket until it is also removed; cancel it and reset before another run.

## Evidence and decision

First check service exclusivity, power conversion and all temperature requirements. Then compare reversal counts. Fewer reversals justify the neutral region only if the temperature cost is acceptable. If neither mapping qualifies, retain that result rather than enlarging the temperature limits after the tests.

## Conclusion

Split-range control expresses the temperature controller’s demand in physical heat units and converts it to the appropriate actuator. Capacity, neutral-region behavior and tracking determine what heat actually reaches the jacket.

## Reason for the next lesson

The split-range map decides which actuator should supply a signed heat demand. It does not decide when a jacket-temperature constraint must take priority over the vessel’s normal demand. We need a selection rule and a way to keep an unselected controller’s state ready for a later transfer.

[Override control](../55-override-control/README.md) isolates that constraint decision on the heater-only vessel baseline.
