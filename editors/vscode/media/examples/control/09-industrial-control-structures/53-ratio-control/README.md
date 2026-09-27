# 57. Ratio control

You will control additive flow in proportion to measured carrier flow. You will account for pump lag during flow changes and calculate how much additive still enters after the carrier stops.

## The problem

The jacket controller generated an inner setpoint from temperature feedback. In this separate mixing operation, the required setpoint must instead follow measured carrier flow. A carrier stream is supplied at $q_c$ L/min and an additive pump must deliver $q_a=R q_c$. A fixed additive command fails when carrier flow changes. A command estimate is also insufficient because pump lag means requested and measured additive flow differ.

## What we need

We need additive flow to change with measured carrier flow so the mixture
keeps its required proportion as throughput changes. The decision must also
account for pump lag and what happens when carrier flow stops.

## The new idea

**Ratio control** keeps one process quantity in a specified proportion to
another. When carrier flow changes, fixed additive flow would change the
mixture. We make the additive target follow measured carrier flow, then use
feedback to make the pump follow that target.

Use carrier flow measured at a mixing junction, with transport delay neglected in this example. The required volume ratio is $R=.1$, so at $q_c=100$ L/min the additive target is 10 L/min. Ten litres of additive with 100 litres of carrier make 110 litres of mixture, assuming volumes add. Thus the additive fraction of the mixture is $10/110=1/11$, about 9.09%; the required additive-to-carrier ratio is 10%.

A constant pump command produces a settled flow proportional to speed. A step in command takes time to change motor speed and flow. The supplied calibration represents that response by $\tau_a\dot q_a+q_a=.2u$, with $\tau_a=1$ s. The calibrated additive pump is $q_{a,ss}=.2u$ L/min for command $u$ from 0–100%, with first-order time constant 1 s and $dt=.1$ s. These are supplied illustrative calibration values, not learner measurements. Use $K_p=10$ percentage points/(L/min), $K_i=10$ percentage points/((L/min)·s), integral state 50%, and tracking time $T_t=.5$ s. Requirements are fixed before testing: steady ratio within ±.005 after 10 s through 60 s, additive flow 0–20 L/min, carrier flow 0–200 L/min. Run separate reset cases for carrier 100→150 L/min over 60 s and carrier 100→0 L/min over 10 s.

$$
q_{a,sp}=R q_c.
$$

At carrier 150 L/min, target additive flow is 15 L/min. With error $e=q_{a,sp}-q_a$, use the same update order as the PI lessons: update the trial integral, add proportional output, then limit. At the first sample, $e=15-10=5$ L/min, $I_{trial}=50+10(5)(.1)=55$%, and $P=10(5)=50$ points. The unlimited request is 105%; the applied command is 100%. The steady command for 15 L/min is $u=75$%.

When carrier falls below 5 L/min or its measurement is invalid, command additive pump 0% within one update and do not evaluate $q_a/q_c$. The command stops the request, not the physical flow instantly. From $q_a(0)=10$ L/min and $\tau=1$ s,

$$
q_a(t)=10e^{-t}\ \mathrm{L/min}.
$$

Convert L/min to L/s before integrating. The tail volume is $\int_0^\infty (10/60)e^{-t/(1\,\mathrm{s})}dt=10/60=.166667$ L. If detection and command take .1 s, the first .1 s contributes approximately $10(.1)/60=.016667$ L, for nominal extra dose .183333 L. Use allowed tail .20 L and $q_a<.1$ L/min by 5 s. Zero command cannot make tail volume zero without physical isolation.

## Prediction

Calculate targets at carrier 100 and 150 L/min, the first 150-step request/limit, and the steady 75% command. Predict that ratio is undefined during the carrier-stop interval. Calculate tail flow at 1 s and 5 s and the .183333 L detection-plus-tail dose.

## Implementation

Read carrier and additive flow validity before calculating the quotient. If both flow sensors are valid and $q_c\ge5$, set $q_{a,sp}=R q_c$, calculate PI using the retained integral, limit command, and update additive-flow model. Accumulate tail dose as

$$
V_{\mathrm{tail},k+1}=V_{\mathrm{tail},k}
+\frac{q_{a,k}}{60}dt
$$

with volume in litres. If either flow sensor is invalid, or carrier flow is below 5 L/min, set pump command to zero within one update, inhibit ordinary PI and ratio calculations, track the zero applied command; on explicit re-enable preload $I=u_{applied}-P$ from the valid current measurements before resuming integration, and mark ratio not evaluated. Restart requires valid flow sensors and explicit re-enable; use the parameter/mode and invalid-data lessons rather than inventing a new valve model.

## Experiment in Control Studio

1. Reset carrier 100 L/min, additive measured flow 10 L/min, $R=.1$, integral 50%, applied command 50%, valid sensors and $dt=.1$ s. Display carrier/additive targets and measurements, ratio, error, request/applied command, tail volume and validity.
2. Run the carrier 100→150 step for 60 s from **Confirm and start**. Check the first limited request, then steady ratio after 10 s through 60 s. Keep flows within declared bounds.
3. Fully reset and run carrier 100→0 for 10 s. At carrier below 5 verify zero command within one update, ratio not evaluated, tail volume, and $q_a<.1$ L/min by 5 s.
4. In separately reset 10 s tests, first mark the carrier reading invalid at 0 s, then mark only the additive reading invalid. Physical carrier flow remains 100 L/min. Verify zero additive command within .1 s, no ratio calculation, and re-enable blocked until both sensors are valid. When the additive sensor is invalid, use the simulator’s true flow only to evaluate the test’s tail volume; that hidden value is not available to the controller. The planned sensor fault tests the shutdown logic; abort if that logic fails. Stop for invalid timestamps, additive outside 0–20 L/min, carrier outside 0–200 L/min, tail above .20 L, or unresponsive monitor. Command the additive pump to zero; flow will decay. Restore sensors and reset before repeating.

## Evidence and decision

Assess steady ratio tolerance separately from the carrier-step transient and stop-tail dose. Compare measured flows, not commands. A first request above 100% is a predicted saturation event, not proof that PI is wrong; it affects how quickly the ratio reaches tolerance. The carrier-stop run cannot claim ratio accuracy while denominator is below 5 L/min.

Report tail integral with units and compare it with .20 L. Because actuator output decays physically after a zero command, an isolation valve or other hardware boundary would be required to guarantee zero additional dose; that requirement would need a different actuator arrangement.

## Conclusion

Ratio control computes additive target from measured carrier flow and uses feedback to correct pump lag. Guarding the denominator prevents meaningless ratios, while tail-volume accounting makes the physical consequence of a zero command explicit.

## Reason for the next lesson

The dosing calculation now defines how one flow must follow another, including the physical tail after a stop. Return to the jacketed vessel for a separate unmet requirement: if incoming process heat exceeds passive heat loss, the vessel needs active cooling as well as heating. One positive heater command cannot express both actions.

[Split-range control](../54-split-range/README.md) maps a signed heat demand to separate heater and cooler commands using their stated capacities.
