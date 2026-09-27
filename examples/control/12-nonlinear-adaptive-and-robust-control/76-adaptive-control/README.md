# 78. Adaptive control

You will estimate a change in the heater’s heat-loss coefficient and use it to revise PI tuning. You will decide whether the data support that change and test whether the revised gains improve control.

## The problem

A model-based controller can become poorly tuned when physical parameters
change. We now return to the heater and ask whether new observations can
justify updating its heat-loss model and tuning, rather than assuming the
old values remain suitable.

Use $50\dot T=2u(t-1)-H(T-20)-D$, with 2 kW/point and 1 s delay. If H changes from 10 to 8 kW/°C, both holding power and thermal response time change. Ordinary PI can already adjust its integral to restore the setpoint. Adaptation has a different purpose: update the model and tuning when the physical parameters change. D must be known zero for the simple estimator; otherwise it is confounded with heat loss.

## What we need

We need to identify a supported change in process parameters and revise
the tuning accordingly. The update must use informative, trustworthy data
and be rejected when the observations cannot distinguish the proposed change.

## The new idea

**Adaptive control** updates controller parameters using information learned
from the operating process. Ordinary PI can remove steady error after heat
loss changes; adaptation revises the model and tuning so the transient response
remains suitable. Here we estimate heat loss from a completed experiment,
qualify the result, then update the controller coherently.

Use $x=T-20$ and $y=2u_{delayed}-50\dot T$. With D=0, $y=Hx$. Keep C=50 kJ/°C and the 2 kW/point conversion known. Accept a proposed H only in 7–13 kW/°C and only with valid data, $x$-span ≥1 °C, residual max ≤5 kW, and two disjoint half-windows agreeing within 0.5 kW/°C. Reject otherwise rather than changing gains automatically.

Choose H to minimize the squared heat-balance residuals $J(H)=\sum_i(y_i-Hx_i)^2$. Differentiating gives $dJ/dH=-2\sum_i x_i(y_i-Hx_i)$. Setting that derivative to zero gives the **least-squares estimate**

$$
\hat H=\frac{\sum x_i y_i}{\sum x_i^2},
$$

provided the denominator is nonzero. A changing temperature is not required when H is the only unknown and C is known: at steady T=35, u=75, D=0, $x=15$, $y=150$, so H=10. The nonzero denominator requires temperature above ambient, not necessarily a temperature change. The additional one-degree span is an experimental quality requirement: multiple operating conditions help expose a constant unknown heat load or derivative error. Sensor bias can still confound this inference; diagnose it separately. For H=8, x=15 and y=120; x=16 and y=128 give $\hat H=(15\cdot120+16\cdot128)/(15^2+16^2)=3848/481=8$ kW/°C.

At λ=10 s, model-based gains are $K_p=C/(2\lambda)=2.5$ points/°C and $K_i=H/(2\lambda)$: .5 for H=10 and .4 for H=8. The 35 °C holding command is $15H/2$: 75% at H=10 and 60% at H=8. Reset H=8 to 35 °C, 60% output and matching I/history; do not reuse the H=10 75% record.

## Prediction

Calculate H for the points above, gains and holding commands. Predict residual signs if D=10 is wrongly included, and why central derivative windows should exclude the 1 s delay edge and record edges.

## Implementation

Process the saved experiment record after it finishes. A centered difference uses a later sample and cannot be available at its centre time during live operation. For each centre time t in that record, estimate the slope from $(T(t+.5)-T(t-.5))/1$ s, using the .1 s samples. Exclude windows that cross the command-delivery step or either record end. Compute y from the applied command delayed by 1 s at that centre time. Write ST sums `SumXY := SumXY + TemperatureAboveAmbientC * BalancePowerKw;` and `SumXX := SumXX + TemperatureAboveAmbientC * TemperatureAboveAmbientC;`, then divide only for a positive denominator. Reset the sums for each new window and evaluate residual and split-window checks. Publish H, bounds and validity. At one atomic parameter-update boundary from Lesson 27, align I to the new P/output; perform exactly one ordinary integral update for each valid elapsed interval, including an interval when the gain record changes. The new Ki scales future increments; it does not rescale the entire stored integral.

## Experiment in Control Studio

1. For H=10, reset T=SP=35 °C, applied command and integral 75%, and the full delayed history 75%. For H=8, use the same temperature with command, integral and history 60%. Set D=0 and disable PI during identification.
2. At **Confirm and start**, apply a +10-point manual step for 30 s: 75→85% for H=10, or 60→70% for H=8. These use the same relative input change but produce separate temperature records. At normal end restore that case’s holding command.
3. Estimate H from the valid derivative windows. Divide the usable time interval into two nonoverlapping halves and compare their estimates. Require the declared H range, whole-record temperature span, residual and half-window agreement. Reject a failed estimate and retain existing gains.
4. For each supported H, compare the existing PI gains 2.5/.5 with the gains based on the accepted estimate. Freeze each candidate and run both standard 60 s tests from full H-specific resets: SP35→38 with peak≤39 °C and error within ±.2 °C from 30–60 s; separate D=30 kW at SP35 with minimum≥33 °C and the same late error band. Use dt=.1 s, beta=1, filter bypass and tracking time 1 s.
5. Abort if actual temperature leaves 0–80 °C, applied command leaves 0–100%, measurements or time become invalid, or the monitor fails. Reaching 100% is ordinary saturation to record. Command zero heat on abort, cancel disturbances, stop the simulator if needed, and reset before each repeat.

## Evidence and decision

Require D=0, residual and split-window agreement, excitation span, and valid timestamps. Keep the supplied physical H distinct from the learner’s estimate. Compare both controllers on the same physical H before attributing an improvement to adaptation. Update gains and integral state atomically only after qualification.

## Conclusion

Adaptive control is qualified estimation followed by one coherent controller revision. Heat-loss uncertainty changes holding power and integral gain while preserving the same physical heater model.

## Reason for the next lesson

Adaptation needs informative, trustworthy data. If the current parameter
cannot be estimated reliably but a plausible range is known,
[robust control](../77-robust-control/README.md) compares fixed controllers
across that uncertainty rather than assuming adaptation can remove it.
