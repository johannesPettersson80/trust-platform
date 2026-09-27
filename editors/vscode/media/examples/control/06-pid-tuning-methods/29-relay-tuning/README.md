# 20. Relay tuning

You will switch the electric heater between two bounded commands, measure a repeatable temperature cycle, and use an empirical relay rule to propose a PI candidate.

## The problem

In the step-response experiment, we changed the applied heater command once
and held it. We used the temperature response to estimate a process model:
its gain, time constant and delay. We then validated that model and used it
to calculate PI settings.

Relay tuning takes another route. We repeatedly switch heater power between
a higher and a lower value as temperature crosses the switching thresholds.
We measure how large the temperature oscillation is and how long each cycle
takes, then calculate approximate PI settings without first fitting that
step-response model. This gives us an alternative candidate to compare;
the experiment alone does not establish which controller is better.

## What we need

To compare another tuning route, we need a PI candidate obtained directly
from a controlled repeating response rather than from the fitted step model.
We must establish a repeatable cycle and extract meaningful measurements
before treating its calculated settings as a candidate.

## The new idea

**Relay tuning** uses a temporary two-level controller to obtain a repeating
response for calculating tuning parameters. Higher power is selected when
temperature is low and lower power when it is high. The retained state between
thresholds uses the hysteresis rule already taught. We measure complete cycles
after the startup response has settled.

Use the same heater as the physical-model and identification lessons: $C=50\ \mathrm{kJ/°C}$, $H=10\ \mathrm{kW/°C}$ to 20 °C, 200 kW full power, nominal gain $K=.2$ °C/percentage point, one-second delivered-power delay, and $dt=.1$ s. Start at SP=35 °C, PV=35 °C, applied command 75%, integral output 75%, proportional output 0%, command history 75% and $D=0$. Keep PI inactive during the relay test and hold its integral state at 75%.

Before starting, choose relay amplitude $d$ between 5 and 10 percentage points and justify it against the repeatable sensor-noise bound of ±.02 °C and the switching band. Use bias 75%, high command $75+d$, low command $75-d$, and illustrative thresholds 35.1 °C and 34.9 °C. The .02 °C noise bound concerns repeatable fluctuations during the cycle; keep it distinct from the earlier absolute measurement-error bound and record any retained sensor dynamics. The relay state starts High and is retained between thresholds. Record a maximum 60 s, sample interval .1 s, temperature bounds 33–37 °C, command bounds 0–100%, invalid-data abort rule, normal restore command 75%, and a full-reset rule.

The relay output is approximately a square wave of amplitude $d$ around the bias. The heater smooths that abrupt input, so we approximate the repeating temperature by a sine wave at the cycle frequency. To calculate the matching sine component of the command, multiply the square wave by a unit sine and integrate over one cycle, then divide by the integral of the squared unit sine. The latter integral is π; symmetry lets us use twice the positive half-cycle. Thus the command’s fundamental sine component has amplitude

$$
\frac{2}{\pi}\int_0^\pi d\sin(\phi)\,d\phi
=\frac{4d}{\pi}.
$$

Here $\phi$ spans one half-cycle from 0 to $\pi$; the other sine components, called higher harmonics, repeat at multiples of the main cycle frequency and are attenuated by the heater's thermal dynamics, so the fundamental is a useful approximation when the measured temperature cycle is near sinusoidal. If $a$ is half the measured peak-to-peak temperature amplitude in °C, estimate an equivalent relay gain. With a nearly sinusoidal temperature cycle and small switching hysteresis, this approximates the proportional gain at which the loop would sustain oscillation, called the ultimate gain:

$$
K_u\approx\frac{4d}{\pi a}
$$

in percentage points/°C. This is not the physical heater gain $K=.2$; it is an empirical cycle-based estimate under the relay assumptions.

Use the Ziegler–Nichols-style empirical PI rule

$$
K_p=0.45K_u,\qquad T_i=\frac{P_u}{1.2},\qquad K_i=\frac{K_p}{T_i},
$$

where $P_u$ is the full cycle period in seconds and $T_i$ is the integral time in seconds. The resulting $K_i$ has units percentage points/(°C·s). Finite hysteresis changes the switching phase; asymmetric heating/loss or saturation biases $a$ and $P_u$, and a nonrepeatable cycle makes the candidate unusable.

## Prediction

Choose $d$ and calculate high/low commands. Predict whether the commands remain within 0–100% and whether a ±.02 °C noise signal can cross a .2 °C-wide switching band. Before evaluating data, require at least three complete cycles after discarding the first two, with each retained period and amplitude within 10% of their respective means. Predict whether your amplitude and 60 s duration can produce those observations.

As a conditional arithmetic example only, suppose a cycle has upper 35.4 °C, lower 34.6 °C, full period $P_u=5$ s, and $d=10$ points. Then $a=.4$ °C,

$$
K_u=\frac{40}{\pi(0.4)}\approx31.831\ \mathrm{points/°C},
$$

$$
K_p\approx14.324,\quad T_i=4.1667\ \mathrm s,\quad
K_i\approx3.43775\ \mathrm{points/(°C\cdot s)}.
$$

Use your own accepted cycle measurements for the actual candidate. From 75% and a 3 °C setpoint step, the first proportional-plus-integral request with $dt=.1$ s would be approximately $75+14.324(3)+3.43775(3)(.1)=119.003$% before limiting. The applied output would be 100%, so both target and disturbance trials must inspect saturation and anti-windup.

## Implementation

Use a retained BOOL RelayHigh. On each valid sample, set `RelayHigh := FALSE` at or above 35.1 °C and `RelayHigh := TRUE` at or below 34.9 °C; retain the previous state inside the band. Command $75+d$% in High and $75-d$% in Low. Guard the known conditions with IF/AND checks for validity, bounds and elapsed duration. A `BOOL` and the `IF`/`ELSIF` decision from hysteresis implement this rule.

Accumulate only complete cycles after discarding the first two. Store scalar extrema, switch times, period and amplitude; require positive values before calculating $K_u$, $T_i$ and $K_i$. If a fault occurs, command zero heat and abort rather than restoring a nonzero command. On normal completion, restore 75%, disable relay mode and requalify the normal controller before any PI trial.

## Experiment in Control Studio

1. Confirm PV=35 °C, SP=35 °C, applied and delayed command 75%, integral output 75%, PI inactive, $D=0$, noise bound ±.02 °C and thresholds 34.9/35.1 °C. Choose and record $d\in[5,10]$ with justification.
2. In **Control Studio → Test**, select **Observe without excitation**, maximum duration 60 s, and record from **Confirm and start** at time 0. Display true/model temperature if available, PV, RelayHigh, thresholds, requested/applied command, validity and cycle counters.
3. Ignore the first two transient cycles. Accept at least three later complete cycles only when each period and amplitude is within 10% of the accepted mean. Record every accepted scalar amplitude and period.
4. Abort on PV outside 33–37 °C, applied command outside 0–100%, invalid data or unresponsive signals. On normal completion restore 75% and requalify; on fault command zero heat. Select **Abort test** for an early stop and stop the simulator if it is unresponsive. Fully reset temperature, I and the 1 s command history before a PI trial or another amplitude.

## Evidence and decision

Report $d$, bias, thresholds, upper/lower commands, sample interval, accepted cycle table, mean $a$, mean $P_u$, and the 10% consistency checks. Calculate $K_u,K_p,T_i,K_i$ with units and guards. If no three consistent cycles exist, report no relay candidate.

Trial the candidate from a full 35 °C/75% reset under the same fixed Lesson 9 requirements: 35→38 °C peak ≤39 °C and $|error|\le.2$ °C from 30–60 s, and, in a separately reset test, 30 kW disturbance minimum ≥33 °C and the same error window; 90% is the nominal final-output prediction for each, not a separate pass criterion. Keep selected anti-windup and limits. A relay estimate is not a physical stability proof and cannot override infeasibility or failed requirements.

## Conclusion

A bounded heater relay can provide an empirical $K_u$ and $P_u$ when its temperature cycle is repeatable, its hysteresis and amplitude are recorded, and its limits are respected. The fundamental-component derivation explains the $4d/\pi$ estimate; the resulting PI values remain candidates requiring matched target and disturbance tests.

## Reason for the next lesson

The relay experiment can provide another set of PI gains when its cycle is repeatable. Neither those gains nor the model-derived gains are suitable merely because their calculations are complete.

[Tuning comparison](../31-tuning-method-comparison/README.md) compares the available candidates using the same heater, initial states, temperature requirements and output limits. If the relay did not produce a valid cycle, retain that outcome rather than inventing a candidate.
