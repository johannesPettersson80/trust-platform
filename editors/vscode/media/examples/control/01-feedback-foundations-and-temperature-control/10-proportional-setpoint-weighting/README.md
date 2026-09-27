# 22. Setpoint weighting

You will reduce the proportional response to a deliberate setpoint change while preserving the response to a change in measured temperature. The same electric heater and the selected Lesson 21 gains remain the reference.

## The problem

The heater is hot and settled at 35 °C with about 75% output. Changing the setpoint to 38 °C changes the error immediately, even though the water temperature has not changed. Reducing $K_p$ would soften that request but would also weaken correction when measured temperature falls because of a disturbance. Setpoint weighting changes the setpoint path alone.

## What we need

We need to soften the proportional command change caused by a new target
without weakening its response to a measured-temperature change. Integral
action must still be able to remove steady error.

## The new idea

**Setpoint weighting** changes how strongly the proportional term responds to
the desired value while retaining its response to the measurement. We multiply
only the setpoint contribution by a weight; integral action still uses the
full temperature error.

Use deviations around the 35 °C operating point. Let $SP$ be setpoint, $PV$ measured temperature, and $u_P$ proportional request:

$$
u_P=K_p[\beta(SP-35)-(PV-35)].
$$

$\beta$ is dimensionless. With $\beta=1$, the setpoint and measurement have the ordinary proportional weights. With $\beta=0.5$, only the setpoint deviation is halved; the measured-temperature deviation keeps its full coefficient. The 35 °C anchor is an operating-point reference, not a new setpoint.

Integral action uses the true error $e=SP-PV$. With old state $I_{old}$,

$$
I_{trial}=I_{old}+K_i(SP-PV)\Delta t.
$$

The selected Lesson 21 controller also retains applied-output tracking; form the request using the trial integral, then limit the output and correct the retained state in the Lesson 8 order.

Compare proportional setpoint weights $\beta=1$ and $\beta=0.5$ using your selected Lesson 21 gains unchanged. Retain 0.1 s scans, the 1 s heat-delivery delay and 0–100% output limits. The gains Kp=2 and Ki=0.5 are used only for the worked arithmetic below. Requirements are fixed before testing:

| Test | Requirement through 60 s |
| --- | --- |
| Setpoint 35→38 °C | Peak ≤39 °C; $\vert SP-PV\vert\le0.2$ °C by 30 s and thereafter |
| Added load D=30 kW at SP=35 °C | Minimum PV ≥33 °C; $\vert SP-PV\vert\le0.2$ °C by 30 s and thereafter |

The nominal final output for either case is 90%; that is a prediction from the heater capacity, not a measured qualification rule. Assess the two requirements separately.

Current project boundary: `src/Main.st` fixes `SetpointWeight` at 0.5 and
scriptedly changes only the setpoint; it has no disturbance input or
Control Studio binding for selecting a second weight. The two-weight and
30 kW disturbance comparison below is therefore the required analysis, not a
claim that the present project can already run both cases. Record those missing
controls rather than changing the project silently.
The current simulation also starts its plant at 20 °C with zero delayed input,
while this lesson's teaching reset is 35 °C/75%; that reset mismatch must be
resolved before treating the scripted trace as the stated operating-point run.

At the hot reset $SP=PV=35$, $u_P=0$ for either $\beta$. For SP 35→38 with I=75%, the first request is $75+2[\beta(3)]+0.5(3)(0.1)$: 81.15% for $\beta=1$, 78.15% for $\beta=0.5$. For fixed SP=35 and PV=34.5 °C, both cases have P=1% and integral trial $75+0.5(0.5)(0.1)=75.025\%$, giving 76.025% before any tracking correction.

At steady SP=PV=38 °C, $\beta=0.5$ gives $u_P=2[0.5(3)-3]=-3\%$ while $\beta=1$ gives zero P. Thus a 90% nominal holding output requires I≈93% for $\beta=0.5$, versus I≈90% for $\beta=1$. Integral action still removes steady error, but the weighted proportional term is not zero at a setpoint different from its 35 °C anchor.

## Prediction

Calculate both first setpoint requests and the common 34.5 °C measurement response above. Predict why the lower setpoint kick may increase rise time or integral accumulation. Predict why an unmeasured 30 kW disturbance must first change PV, while a measured-temperature fall of 0.5 °C produces the same 1-point P correction for both weights.

## Implementation

Calculate setpoint and measurement deviations separately, then update integral state from the full error. Write `ProportionalPct := KpPctPerC * (Beta * (SetpointC - 35.0) - (MeasuredTemperatureC - 35.0));`. Add P and the trial integral, apply the output limit, and apply the existing tracking correction. Set $\beta$ while the controller is stopped; reset to SP=PV=35 °C, applied output 75%, I=75%, and P=0 before each comparison. Live changes during operation are examined in the later transfer and parameter-update lessons. The 1 s delay affects delivered thermal response, not the limited applied command itself.

## Experiment in Control Studio

1. Reset to the full heated state SP=PV=35 °C, applied output 75%, retained I=75%, P=0, and the selected gains. Initialize the preceding 1 s command history to 75%, set D=0 and begin with $\beta=1$.
2. Open **Control Studio → Monitor → Response** to display temperatures/errors in °C and controller contributions/commands in percent. In **Test**, review each sequence. Before each run, record the requirement table and confirm the 0.1 s scan, 1 s delivery delay, and 0–100% limit.
3. At **Confirm and start**, time zero, run the separate 60 s setpoint test 35→38 °C with no added load. Display SP, PV, error, P, I, requested/applied output, and $\beta$.
4. Restart to the identical full heated state. The current project has no D=30 kW input or binding, so do not claim that this disturbance trial ran; record the missing scenario control. Do not combine events.
5. The current project already forces `SetpointWeight := 0.5`; it does not expose a UI choice for $\beta=1$. Record that missing candidate control before claiming a two-weight comparison. Stop on PV outside 0–80 °C, applied command outside 0–100%, invalid data, or normal duration. At completion/abort command zero heat, cancel D, clear state, and restart before repeating. Select **Abort test** for an early stop and stop the simulator if it is unresponsive.

## Evidence and decision

For your actual gains, predict the first setpoint request as $75+3\beta K_p+0.3K_i$. The 81.15% and 78.15% values apply to the worked gains 2 and 0.5; tracking contributes zero while that request remains within the output bounds. For fixed SP35 and PV34.5 °C, both weights give the pre-tracking request $75+.5K_p+.05K_i$ using your gains. This is 76.025% only for the worked gains 2 and .5. Then assess peak/error for the setpoint test and minimum/error for the disturbance test separately.

A smaller setpoint-induced command step is useful only if the setpoint requirement still passes. It does not automatically improve the load test, and the 90% final output is a nominal prediction rather than measured proof. Report output movement and the physical cost of a slower setpoint response alongside each requirement.

## Conclusion

Setpoint weighting changes the proportional response to setpoint deviation while retaining full proportional response to measured-temperature deviation. Integral action still removes error, but at a non-anchor setpoint the weighted P term can be nonzero and the retained I must supply the remainder.

## Reason for the next lesson

Weighting softened the target's contribution, but measured temperature still
has its full feedback coefficient. Sensor noise can therefore move the
heater command even when the water barely changes.
[Measurement filtering](../11-measurement-filtering/README.md) reduces that
rapid measurement variation and tests the cost of delaying real changes.
