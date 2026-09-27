# 59. Override control

You will let a jacket-temperature controller limit the vessel controller’s heating request. You will track both integral states through this selection and distinguish successful constraint control from an infeasible vessel setpoint.

## The problem

Split-range mapping decides which service delivers a requested heat demand. To isolate the separate constraint-selection problem, return to the heater-only vessel baseline. The vessel needs heat, but the jacket must remain at or below 42 °C in this exercise. A vessel controller that sees only its own temperature cannot enforce that jacket requirement. An override uses the second measurement to reduce heating when necessary.

Use the same heater-only jacketed vessel as [cascade control](../52-cascade/README.md), including its balances, heat capacities and 2 kW per heater percentage point. This investigation compares two controllers that request heater power directly; it does not require the split-range cooler.

## What we need

We need the jacket-temperature constraint to take priority when normal
vessel demand requests too much heating. Controllers that are not selected
must remain ready to take over without an unintended command jump.

## The new idea

**Override control** gives a limiting controller authority over the normal
controller when a constraint needs protection. Here the vessel may request
more heat while the jacket is already too hot. We select the request that
protects the jacket and keep both controller states ready for a later transfer.

Both controllers request heating in percent. More heating raises both temperatures. The **low selector** therefore chooses the smaller request:

$$
u_{selected}=\min(u_{vessel},u_{jacket}),
$$

then limits the applied command to 0–100%. When jacket temperature approaches its setpoint, its error decreases and its controller can request less heat than the vessel controller. The selector then gives the jacket controller authority.

Both integrators must account for the selected applied output. Otherwise an unselected controller can accumulate a request that cannot reach the heater. Use the same tracking correction for each: $I_{next}=I_{trial}+(dt/T_t)(u_{applied}-u_{request})$, with $T_t=1$ s. Each request uses its own error and integral; each correction uses the common applied output.

The temperature constraint can make a setpoint impossible. With no vessel disturbance, equilibrium requires

$$
T_j=T_v+.2(T_v-20)=1.2T_v-4.
$$

A 37 °C vessel needs a 40.4 °C jacket and is feasible below 42 °C. A 40 °C vessel would need a 44 °C jacket. With $T_j\le42$, the highest maintainable vessel temperature is $(42+4)/1.2=38.333$ °C. No tuning can satisfy both a 40 °C setpoint and that jacket limit.

## Prediction

For vessel request 30% and jacket request 24%, identify the selected output and the sign of each tracking correction. Explain why reducing a request that is not selected still matters for the later transfer.

Calculate the jacket temperature required at vessel setpoints 35, 37 and 40 °C. Predict which requirement must remain unmet in the 40 °C case.

## Implementation

Write two PI calculations and one `MIN` selector. `MIN(a,b)` returns the smaller argument. Apply the output limit once after selection, then correct both integrators using their own unlimited requests. Display which request was smaller; do not infer selector authority from vessel error alone.

At the 35/38 °C, 15% starting state, initialize vessel integral to 15%. The jacket error is $41.5-38=3.5$ °C and P is 35 points. Initialize its integral to $15-35=-20$%, making its initial complete request 15%. Negative integral state is legitimate: only the complete applied command is limited to 0–100%. Use .1 s updates, valid measurements and the initialization rules already taught.

## Experiment in Control Studio

The normal vessel PI has $K_p=2.5$ points/°C and $K_i=.1$ points/(°C·s). A second PI regulates jacket temperature toward 41.5 °C, leaving .5 °C between its setpoint and the 42 °C requirement. Its supplied starting gains are 10 points/°C and 2 points/(°C·s). They are design choices to test, not a guarantee that overshoot stays below .5 °C.

Keep the two 60 s vessel tests from cascade control: setpoint 35→37 °C with peak at most 38 °C, and separate jacket extraction 30 kW at vessel setpoint 35 °C with minimum at least 34 °C. Both require vessel error within ±.2 °C from 30–60 s. Add the jacket maximum 42 °C to both tests before comparing designs.

1. Display both temperatures, both errors, both unlimited requests and integral states, selected request, applied power and selector authority.
2. Compare ordinary vessel PI and override in separate full resets for both 60 s tests. All events start at **Confirm and start**; initial temperatures are 35/38 °C, power 15%, disturbances zero, and integrals as specified above.
3. In a separate override-only 60 s test, request vessel 40 °C for the first 20 s, then return to 35 °C. This is an intentionally infeasible demand. Require jacket at most 42 °C and vessel within 35±.2 °C from 50–60 s after the return. Inspect how selection and tracking affect recovery.
4. Abort when jacket temperature exceeds 42 °C, vessel temperature leaves 30–40 °C, applied power leaves 0–100%, data become invalid, or time stops. Record a jacket exceedance as a failed requirement before commanding zero heat. Cancel disturbances, stop the simulator if needed, and reset both plant and controllers before repeating.

## Evidence and decision

Assess jacket maximum and vessel performance separately. Passing the jacket requirement does not establish vessel tracking. In the 40 °C interval, persistent vessel error follows from the heat balance; evaluate constraint handling and subsequent recovery instead of claiming that tuning should remove the error.

Compare selected-output continuity and both integral states as authority changes. If the jacket limit is exceeded, the proposed override has not qualified; it is not an independent protective device merely because it is called an override.

## Conclusion

Override control gives a measured constraint authority over the normal heating request. Tracking prepares both controllers for selection changes, while the heat balance determines whether the desired vessel temperature is compatible with the jacket limit.

## Reason for the next lesson

The selector gives a measured constraint priority over normal heating demand. It does not compensate for a command whose physical heating effect arrives late. Return now to the single-store electric heater with its known delivery delay; the jacket model itself did not include that extra transport delay.

[Smith prediction](../57-smith-predictor/README.md) uses a validated heater model to account for that timing inside the feedback arrangement while retaining measurement correction.
