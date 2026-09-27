# 56. Cascade control

You will use two PI controllers to regulate a jacketed vessel. You will explain why measuring jacket temperature allows an inner loop to react to a disturbance before the vessel-temperature loop can detect it, then test whether this improves temperature control.

## The problem

The electric-heater example has one thermal store. Here, an electric heater warms a jacket surrounding the vessel. Heat passes from the jacket into the liquid, so jacket temperature can change before the liquid temperature does.

Let $T_j$ and $T_v$ be jacket and vessel temperatures in °C. The supplied physical model uses jacket heat capacity 5 kJ/°C, vessel heat capacity 50 kJ/°C, jacket-to-vessel heat conductance 10 kW/°C, and vessel heat loss 2 kW/°C to a 20 °C environment. A 0–100% heater command $u$ delivers $2u$ kW. No additional transport delay is assumed. This jacketed-vessel model has no nominal cold-water throughflow; its supplied 2 kW/°C environmental loss differs from the earlier heater’s combined 10 kW/°C loss. Keep those two teaching cases distinct.

## What we need

We need to react to an unmeasured jacket disturbance before it strongly
changes vessel temperature. An intermediate measurement must provide an
earlier correction while the vessel's final temperature remains the objective.

## The new idea

**Cascade control** uses two nested feedback loops: the outer controller sets
the target for a faster inner controller. Here the outer PI compares vessel
temperature with its setpoint and requests a jacket temperature. The inner
PI compares measured jacket temperature with that request and commands heater
power. Only the inner controller writes the heater command.

Use the energy-balance method already taught:

$$
5\dot T_j=2u-10(T_j-T_v)-D_j,
$$
$$
50\dot T_v=10(T_j-T_v)-2(T_v-20)-D_v.
$$

Positive $D_j$ and $D_v$, in kW, remove heat from the jacket and vessel respectively. At $T_v=35$, $T_j=38$ and zero disturbances, 30 kW passes through the jacket and leaves the vessel. The holding command is therefore 15%.

Compare single-loop PI with cascade under these fixed requirements:

| Separate 60 s test | Vessel requirement |
| --- | --- |
| Setpoint 35→37 °C, no disturbance | Peak at most 38 °C; error within ±.2 °C from 30–60 s |
| Setpoint 35 °C, jacket extraction $D_j=30$ kW | Minimum at least 34 °C; error within ±.2 °C from 30–60 s |

Use .1 s controller updates, jacket setpoint limits 20–50 °C, applied power 0–100%, and simulation bounds $T_j=20$–55 °C and $T_v=30$–40 °C.

Immediately after a 30 kW jacket disturbance, the jacket cools at $-30/5=-6$ °C/s. The vessel initially has zero temperature slope because its heat balance has not yet changed. The inner loop can therefore begin correcting the jacket while the vessel error is still small. A disturbance applied directly to the vessel would not provide this advantage.

The inner loop must respond sufficiently faster than the outer loop. Holding vessel temperature approximately fixed gives inner gain $.2$ °C/point and time constant $.5$ s. With an ideal fast inner loop, the outer relation is $50\dot T_v=10T_j-12T_v+40$, giving gain $10/12$ and time constant $50/12$ s.

Using [model-based PI tuning](../../06-pid-tuning-methods/30-model-based-pi-tuning/README.md), $K_p=\tau/(K\lambda)$ and $K_i=1/(K\lambda)$. Choose inner response time $.5$ s and outer response time 10 s. This gives inner gains 5 points/°C and 10 points/(°C·s), and outer gains .5 °C/°C and .12/s. These approximations provide starting gains; the experiment must check the separation of response speeds.

For a single-loop comparison, neglecting jacket storage gives $50\dot T_v=2u-2(T_v-20)$. Its gain is 1 °C/point and time constant 25 s; the same 10 s tuning choice gives $K_p=2.5$, $K_i=.1$. This is a specified baseline, not the best possible single-loop tuning.

## Prediction

Calculate the initial temperature slopes after the jacket disturbance. Explain which measurement changes first. At restored temperatures 35/38 °C with $D_j=30$, calculate the 60 kW heating requirement and 30% holding command.

For a one-degree inner setpoint step, calculate the first request: $I_{trial}=15+10(1)(.1)=16$%, $P=5$ points, so the request is 21%.

## Implementation

Write the two PI calls using the stateful interface and tracking taught in [controller modes](../../03-production-quality-digital-pid/39-controller-mode-initialization/README.md). Initialize the outer integral at 38 °C, which already includes the absolute jacket setpoint, and the inner integral at 15%. The single-loop comparison initializes its integral at 15%. Give the outer controller Celsius output names and bounds; its requested jacket setpoint is not heater power in percent.

Calculate the outer request, limit it to 20–50 °C, then use that selected value as the inner setpoint. Apply each integral correction as $I_{next}=I_{trial}+(dt/T_t)(selected-request)$ with $T_t=1$ s. The outer correction is in °C; the inner correction is in heater percentage points. Inspect both with a deliberately limited request before running.

## Experiment in Control Studio

1. Start **Simulator** and display both temperatures, both setpoints, P/I contributions, requested and selected jacket temperature, requested/applied heater power, and disturbances.
2. First disable the outer loop and test the inner loop alone: reset to 35/38 °C and 15%, step jacket setpoint 38→39 °C at **Confirm and start**, and record 10 s. Measure the time to reach 38.632 °C, 63.2% of this one-degree change.
3. Fully reset temperatures and controller states. Run each architecture for the setpoint test, then reset each again for the disturbance test. All events begin at **Confirm and start**; only one event occurs per 60 s record.
4. In the cascade setpoint record, measure the vessel’s time to reach 36.264 °C, 63.2% of its two-degree change. Compare normalized response times: the inner response should be distinctly faster. If it is not, revise the proposed separation before claiming a cascade benefit.
5. Abort if physical temperatures or applied output leave their stated bounds, data become invalid, or the monitor stops updating. Command zero heat and stop the simulator if needed. At normal completion also cancel disturbances and reset the entire plant and both integrals before repeating.

## Evidence and decision

Assess every temperature requirement and both response times. Earlier jacket correction is useful only if vessel performance meets the requirements without unacceptable output behavior. Compare the disturbed final command with the calculated 30% after settling.

A failed baseline does not prove that every single-loop tuning would fail. If neither candidate meets the requirements, retain that result and identify whether inner-loop speed, tuning, or available heating limits the comparison.

## Conclusion

Cascade uses an intermediate measurement to correct disturbances before they strongly affect the controlled vessel. It requires a sufficiently fast inner loop, correctly scaled controller outputs, and a comparison against the vessel’s actual requirements.

## Reason for the next lesson

Cascade calculates an inner setpoint from the outer temperature error. A different production operation needs another relationship: as carrier flow changes in a mixing line, additive flow must change in a fixed proportion. Holding the additive-flow setpoint constant would change the mixture ratio.

[Ratio control](../53-ratio-control/README.md) introduces that dosing requirement and calculates the dependent flow setpoint from the measured carrier flow.
