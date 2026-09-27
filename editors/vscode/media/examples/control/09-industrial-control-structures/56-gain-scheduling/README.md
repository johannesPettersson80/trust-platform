# 43. Gain scheduling

You will use gain scheduling to adjust PI tuning as tank level changes. You will calculate the settings from the local flow-to-level model and test whether the schedule meets the same requirements at low, middle and high levels.

## The problem

We linearized the tank near one water level, but it does not respond the same
way at every level. At higher levels, another centimetre adds less extra
drainage than it does at lower levels. The same small increase in inlet flow
therefore produces a larger, slower level change. Gains chosen for one
operating level may give a different response elsewhere.

## What we need

We need to adjust tuning for the tank's changing response as operating
level changes, without manually retuning at each level or discarding the
integral's stored holding flow. The adjustment must be justified by the
known level-dependent dynamics.

## The new idea

**Gain scheduling** changes controller gains according to a known operating
condition. Here we use measured water level to calculate suitable gains from
the local tank model. Another implementation could select them from a prepared
table. We do this while the controller runs instead of manually retuning at
every level.

In this example only integral gain changes; proportional gain stays fixed.
That follows from our model and chosen response time, not a general rule for
every gain schedule. We retain the integral's accumulated holding flow when
updating its gain, then compare responses at low, middle and high levels.

Use $A=.5\ \mathrm{m^2}$, $c=.020\ \mathrm{m^{5/2}/s}$, and operating points $h_0=.16,.25,.36\ \mathrm m$, with balanced inlet flows $q_0=.008,.010,.012\ \mathrm{m^3/s}$. Use $dt=1$ s throughout this tank branch, inlet limits 0–.020 $\mathrm{m^3/s}$, and controlled operating window .05–.45 m. Start each case with $SP=PV=h_0$, applied inlet $q_0$, integral state equal to $q_0$, and proportional output zero.

Requirements are fixed before comparing candidates. At each of the three starting levels, step its setpoint by +.02 m at experiment time 0 and run 60 s. The peak must be no more than new setpoint +.01 m, error must remain within ±.005 m from 40–60 s, and inlet flow must remain 0–.020 $\mathrm{m^3/s}$. Run six cases: fixed $K_i=.002$ and scheduled $K_i(h)$ at each starting level. Compare primary control accuracy and the sum of absolute consecutive flow changes; do not call that sum pump life.

From Lesson 42, deviations near an operating level satisfy

$$
A\dot x+g x=v,\qquad
g(h)=\frac{c}{2\sqrt h},
$$

where $x=h-h_0$ is metres and $v=q_{in}-q_0$ is $\mathrm{m^3/s}$. Choose a desired local response time λ=10 s. First hold g fixed at one operating point. With e=SP−h and a fixed setpoint, the desired decay $\dot e=-e/\lambda$ means $\dot x=e/\lambda$. Substitute into the local balance:

$$
v=\frac{A}{\lambda}e+gx.
$$

Differentiate with fixed A, g and λ, then substitute the desired derivative again:

$$
\dot v=\frac{A}{\lambda}\dot e+\frac{g}{\lambda}e.
$$

For a PI command, the derivative is $\dot v=K_p\dot e+K_i e$. Matching coefficients gives

$$
K_p=\frac{A}{\lambda}=0.05\ \mathrm{m^2/s},\qquad K_i=\frac{g}{\lambda}.
$$

The derivation is local. Recomputing g from the changing level creates a nonlinear scheduled controller whose performance still needs comparison. Implement the integral as accumulated flow: each update adds the current Ki times error and elapsed time. Multiplying all past error by the latest Ki would change previously accumulated flow and implement a different controller.

The local reference values are:

| Level $h$ | $q_0=c\sqrt h$ ($\mathrm{m^3/s}$) | $g$ ($\mathrm{m^2/s}$) | $K_p$ ($\mathrm{m^2/s}$) | $K_i$ ($\mathrm{m^2/s^2}$) | $\tau=A/g$ (s) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| .16 m | .008 | .0250000 | .050 | .0025000 | 20 |
| .25 m | .010 | .0200000 | .050 | .0020000 | 25 |
| .36 m | .012 | .0166667 | .050 | .0016667 | 30 |

The proportional gain is constant because the chosen $\lambda$ and tank area are constant; only $K_i$ schedules with $g(h)$. The integral state is an absolute holding flow, not an added bias. For the first .02 m step at .16 m, $P=.05(.02)=.001\ \mathrm{m^3/s}$ and the first 1 s integral increment with $K_i=.0025$ is .00005, giving .00905 requested flow. At .25 m with fixed or scheduled $K_i=.002$, the first request is .01104.

## Prediction

Calculate $g,K_i,\tau$ at all three levels and predict which case has the largest integral gain. Predict the first requested flow at .16 m and .25 m. Predict whether scheduled $K_i$ should make the response more similar across levels, and whether it must reduce flow activity. A fixed controller that meets every requirement remains a valid selection.

## Implementation

Use the current valid measured level each scan to calculate $g=c/(2\sqrt h)$ and $K_i=g/\lambda$. Reject dry/negative or invalid level before evaluating the square root and dividing by it; at zero level the scheduled slope is singular. Keep $K_p=.05$, integral state in absolute flow units, requested/applied flow and limits visible.

Use the coherent parameter-update principle from the [parameter-record lesson](../../03-production-quality-digital-pid/41-atomic-parameter-update/README.md), with fields expressed in flow and level units, but this continuous schedule does not skip the ordinary integral update at each publication. At each valid 1 s interval, calculate the current scheduled $K_i$, run the normal PI integration exactly once, limit flow, and track any applied limitation using a fixed 5 s tracking time in both candidates. This gives a 1/5 correction of applied minus requested flow after each 1 s update. The timing guard must allow the 1 s interval; use a 1.5 s maximum gap for this exercise. No region interpolation or arbitrary hysteresis is needed for this continuous rule.

## Experiment in Control Studio

1. In **Control Studio → Monitor → Response**, display $SP,PV$, level, $g,K_i$, integral state, P contribution, requested/applied inlet flow, flow-change activity and validity. Confirm the selected operating point and reset state.
2. Run the fixed-$K_i=.002$ case and scheduled case separately at $h_0=.16$, each with SP step +0.02 m at **Test → Confirm and start**, which defines time zero, and duration 60 s. Use $dt=1$ s and identical reset.
3. Repeat both candidates from full resets at .25 m/.010 $\mathrm{m^3/s}$ and .36 m/.012 $\mathrm{m^3/s}$. Do not carry integral or applied flow between cases.
4. Record peak, 40–60 s error, final flow, saturation and sum of absolute consecutive flow changes. The learner chooses whether any observed accuracy/activity benefit justifies a continuously varying parameter.
5. Abort for invalid/dry level, level outside .05–.45 m, flow outside 0–.020 $\mathrm{m^3/s}$, or unresponsive signals. Set inlet flow to zero on abort so gravity drains the tank, stop Simulator before leaving the model domain, and fully reset before repeating.

## Evidence and decision

First verify the table and first-scan calculations. Then assess each of the six cases against peak, error window and flow limits. Compare flow activity separately from level accuracy; a lower sum of absolute changes is not a claim about pump wear. If both fixed and scheduled gains qualify, retaining the fixed value can be the simpler decision. If scheduling helps only some levels, state that supported domain.

An invalid sensor or dry-level stop is not a tuning failure. At zero level, the square-root outflow is defined but its slope $c/(2\sqrt h)$ is singular, so this gain schedule must remain within its declared positive-level range. Zero inlet allows gravity drainage after the inlet command is stopped. These simulations do not establish installed-tank performance without parameter identification and validation.

## Conclusion

Gain scheduling here varies the integral gain from the current gravity-outflow slope while keeping $K_p=A/\lambda$ constant. The schedule uses absolute holding flow, valid-level checks and ordinary integration at every 1 s update. Its benefit must be demonstrated across all three operating points under unchanged requirements and flow-activity reporting.

## Reason for the next lesson

We have obtained control relationships by substituting and differentiating the physical equations for each case. As plant dynamics, controller states and delays are connected, repeating that work becomes harder to follow. We need a common algebraic representation while preserving what each physical quantity means.

[Laplace transforms](../../01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics/README.md) returns first to the familiar heater, then the shaft, to express those same dynamics in a form we can connect and analyse systematically.
