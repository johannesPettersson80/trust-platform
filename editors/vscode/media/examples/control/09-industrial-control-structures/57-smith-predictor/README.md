# 60. Smith prediction

You will learn how a Smith predictor uses a heater model to account for power-delivery delay inside the feedback loop. You will test whether it improves temperature response and when model error requires a return to ordinary PI feedback.

## The problem

Return from the jacketed vessel to the original single-store heater: C=50 kJ/°C, H=10 kW/°C and a 1 s command-delivery delay. The constraint selector did not account for that delay within the control calculation. The heater sensor measures current water temperature; it does not report an old temperature. The one-second delay is between applied heater command and delivered heater power. A predictor can estimate what the temperature would be if the current applied command acted without that delay, but it cannot know an unmeasured disturbance before it affects PV.

## What we need

We need to account for known power-delivery delay in the feedback calculation,
while retaining correction from the actual temperature measurement. The
method must also let us recognize when its prediction is no longer suitable.

## The new idea

A **Smith predictor** uses a process model to account for a known input delay
inside the feedback calculation. It combines a model's delay-free response
with correction from the actual measurement:

$$
y_c=T_0+(y_m-T_L).
$$

$T_0$ is model temperature without delivery delay, $T_L$ model temperature
with that delay, $y_m$ measured temperature, and $y_c$ the temperature signal
used by the controller. If the delayed model agrees with measurement, the
correction is zero and the controller uses $T_0$. If they disagree, their
difference corrects the prediction. All four quantities are in °C.

This does not make heat arrive sooner. We must model both paths and test
whether their prediction remains adequate for the actual plant.

Use the heater $C=50\ \mathrm{kJ/°C}$, $H=10\ \mathrm{kW/°C}$, 200 kW full power, $K=.2$ °C/point, $\tau=5$ s and nominal $L=1$ s. Start at SP=PV=35 °C, applied and delayed command 75%, integral 75%, proportional output 0%, $dt=.1$ s, $\beta=1$, filter bypass, selected PI $K_p=5,K_i=1$, and a full command history. Compare PI-only and predictor modes for both 35→38 °C and 30 kW at SP=35 °C, each with 60 s duration and the fixed requirements: setpoint-test peak at most 39 °C and error within ±.2 °C from 30–60 s; disturbance-test minimum at least 33 °C and the same late error band.

The delay-free model from current applied command $u$ is

$$
50\dot T_0=2u-10(T_0-20),
$$

while a second model includes the nominal one-second delay:

$$
50\dot T_L=2u(t-1)-10(T_L-20).
$$

Both model states start at 35 °C. The exact held-input update for either state is

$$
T_{\mathrm{next}}=20+.2u+
(T_{\mathrm{old}}-(20+.2u))e^{-dt/5}.
$$

For $dt=.1$ s, a delay-free 75→85% model changes after one update from 35 to $35+2(1-e^{-.02})=35.03960265$ °C. The physical/delayed state remains 35 °C for its first second if the delivered history is still 75%. This difference is delivery timing, not stale sensor reporting.

Let measured PV be $y_m$. The Smith correction is $r=y_m-T_L$, and selected controller temperature is $y_c=T_0+r$. When the model is correct and there is no unmeasured disturbance, $r=0$ and $y_c=T_0$. This generally differs from the physical temperature during a transient: it is the model’s hypothetical response without delivery delay. A pure $r=0$ match occurs only with no unmeasured disturbance or model mismatch; an immediate $D$ changes the physical PV before the predictor can know it. A residual threshold of .5 °C can select measured-feedback fallback while PV remains healthy.

## Prediction

Predict the one-update delay-free value 35.03960265 °C and the delayed value 35 °C. Predict why a 30 kW load disturbance is visible first through PV, and why a 1.2 s actual delay with a frozen 1 s model produces a nonzero correction even with a healthy sensor. Predict that invalid model history or PV uses different fallbacks: predictor/history invalid uses measured PI bumplessly; invalid PV follows Lesson 28 zero heat.

## Implementation

Maintain only the two scalar model states and the supplied applied-command/delay state; do not introduce a new command-history feature. Update $T_0$ from current applied command and $T_L$ from applied-command history delayed by the fixed model delay of 1 s. In the mismatch experiment, do not substitute the actual 1.2 s delivery history into the nominal model. Calculate correction, threshold and selected controller temperature. Write the two model updates and a Boolean selection in ST. At each controller time, first advance both models over the interval just elapsed using commands actually held during that interval. Compare their values with PV at that same timestamp; then calculate the new PI command and retain its applied value for the next interval. Do not compare current PV with a future model state. Latch measured-feedback fallback until the next full reset, preventing repeated switching at the residual threshold. Predictor fallback preloads $I=u_a-P_{\mathrm{new}}$ at the transfer boundary; skip ordinary integration only for that transfer, then resume .1 s integration and applied-output tracking with $T_t=1$ s; PV invalid takes the Lesson 28 fault path.

Keep requested/applied command, physical PV, both model states, correction, predictor validity and fallback reason visible. Use the selected PI gains and ordinary limit/tracking order.

## Experiment in Control Studio

1. Reset full hot state and verify both model states, measured PV, command history and integral. Run four nominal records: PI-only and predictor for 35→38 °C, and PI-only and predictor for 30 kW at SP=35 °C.
2. Repeat the four records with actual delivery delay 1.2 s while the predictor remains frozen at 1 s. Keep each run 60 s and use the same target/load event at **Confirm and start**.
3. Display PV, physical applied command, $T_0,T_L$, correction, selected controller temperature, fallback, P/I, request and applied output. Trigger measured-feedback fallback only when healthy-PV residual exceeds .5 °C or predictor/history is invalid.
4. Stop if physical/PV temperature leaves 0–80 °C, command leaves 0–100%, data/clock is invalid, or monitor fails. At normal end cancel events, command zero heat, restore and fully reset before every record.

## Evidence and decision

Assess each architecture and timing case against peak, minimum and late-error requirements. The physical plant retains the delay declared for that record, either 1 or 1.2 s. Do not claim a fault was identified when a healthy load disturbance merely triggered residual fallback. Predictor and PI traces must be separate closed-loop records with full resets.

## Conclusion

Smith prediction compensates command delivery delay by comparing a current-command model with a delayed-command model and correcting it with current measured PV. The correction cannot anticipate an unseen disturbance, and explicit residual/history/PV fallbacks define when model-based control stops being credible.

## Reason for the next lesson

The predictor adds another structure whose usefulness depends on its physical assumptions and comparison results. Knowing how each structure works does not establish which complete arrangement meets an application’s temperature, actuator, constraint and lifecycle requirements.

[Vessel commissioning](../../14-integrated-capstones/83-reactor-control/README.md) returns to the taught jacketed vessel and asks you to select and compare suitable arrangements. Keep the single-store predictor example’s parameters separate from the jacket model; do not add a predictor or cooling service without a requirement for it.
