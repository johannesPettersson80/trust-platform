# 52. Loop shaping

You will use lead and lag compensation to change the heater loop’s response at different frequencies. You will decide whether a proposed change improves temperature control without excessive command movement from measurement noise or actuator saturation.

## The problem

The heater model is G_u(s)=.2 exp(−s)/(5s+1), with command in percentage points and temperature in °C. The preceding lesson showed how feedback routes disturbances and noise. We now need a specific controller change to modify those responses. A controller element can preserve its steady, zero-frequency (DC) command while changing how different frequencies are amplified or delayed. The lead/lag choice must be judged with the actual shaped command and measurement noise, not by a phase number alone.

## What we need

We need a controller change that improves the identified frequency-response
tradeoff while preserving temperature and output requirements. Its effects
on noise and actuator demand must be assessed alongside any response benefit.

## The new idea

**Loop shaping** designs controller dynamics to obtain useful gain and phase
behavior across frequencies. We change the controller for the response or
noise requirement identified in the preceding lesson, then assess the entire
loop rather than one attractive point on a plot.

Use the common hot reset: T=35 °C, SP=35 °C, applied command=75%, I=75%, P=0, delayed command history=75%, dt=.1 s, beta=1, and filter bypassed. The worked PI values are Kp=5 point/°C and Ki=1 point/(°C s); retain learner gains when available and label 5/1 as the worked case.

Shape controller command deviation w=(P+I)−75 with:

$$
F(s)=\frac{1+sT_z}{1+sT_p},\qquad T_p=2\ \mathrm s.
$$

$T_z$ and $T_p$ are time constants. Choose $T_z=3$ s or 10 s for comparison.

The baseline is F=1. Requirements are fixed before choosing Tz: both standard 60 s trials (SP35→38: peak PV≤39 and |error|≤.2 from 30–60; separate SP35,D30: minimum PV≥33 and same error); command 0–100; plus a separate noise trial with D=0,SP35, measurement noise n=.05 sin(ωn t) °C, with ωn=1 rad/s, 60 s, and applied-command amplitude over the three complete periods from t=6P_n to t=9P_n, where P_n=2π s (t≈37.70–56.55 s) no greater than .5 percentage point.

With $T_z>T_p$, the element above is **lead compensation**: it adds positive
phase over a frequency range but also increases high-frequency gain. Reversing
the ordering gives **lag compensation**, which contributes negative phase
and reduces high-frequency gain relative to its unchanged steady gain.

The pole is −1/Tp and zero is −1/Tz. DC gain is one. At high frequency the magnitude tends to a=Tz/Tp: a=1.5 for Tz=3 and a=5 for Tz=10. For complex s=jω:

$$
|F(j\omega)|=\frac{\sqrt{1+(\omega T_z)^2}}{\sqrt{1+(\omega T_p)^2}},
\qquad \angle F=\arctan(\omega T_z)-\arctan(\omega T_p).
$$

For Tz=10,Tp=2, at ω=.1 rad/s magnitude is sqrt(2)/sqrt(1.04)=1.38675 and phase is 45°−atan(.2)=33.6901°. At ω=1 magnitude is sqrt(101)/sqrt(5)=4.49444 and phase is atan(10)−atan(2)=20.8545°. The loop transfer is $L=C F G_u$, where $C=K_p+K_i/s$. Extra phase at one frequency does not guarantee more margin because loop crossover can move. Define crossover by |L(jωc)|=1 and phase margin as the angular separation of L(jωc) from −180°; a full Nyquist check belongs to Lesson 53.

The noise-to-command path also changes. With zero setpoint deviation, $y=G_u v$ and $v=-CF(y+n)$, so $v/n=-CF/(1+CFG_u)$. A .05 °C noise amplitude meets the .5-point command limit only if this path’s magnitude is at most 10 points/°C. Evaluate this at 1 rad/s for the selected PI gains; phase lead alone cannot answer the noise question.

A lag has reciprocal ordering: Tz<Tp, so it preserves DC gain but attenuates high frequency and contributes negative phase. This lesson uses a lead choice for the worked comparison; lag is a design alternative, not a mandatory replacement.

## Prediction

Calculate pole/zero locations, high-frequency gains, and both frequency results for Tz=10. Predict which lead has greater noise-to-command gain and whether its phase benefit can move crossover unfavorably. Predict which candidate may fail the .5-point noise requirement while still passing a temperature trial.

## Implementation

Open src/Main.st. Implement the lead with scalar state z:

$$
T_p\dot z+z=w,\qquad v=aw+(1-a)z,\qquad u_{request}=75+v.
$$

Forward Euler uses z_next=z+dt/Tp*(w−z), with z initialized to zero. Calculate the current request from the old z and current w, then store z_next for the next update. This realizes F(s)=a+(1−a)/(1+sTp). For the worked gains 5 and 1, the first SP step has e=3, I_trial=75.3, P=15, w=15.3. With z=0, a=1.5 requests 97.95%; a=5 requests 151.5% before the 0–100 limit.

Tracking must account for shaping: if the applied command is ua and u_request is the shaped request, update I_next=I_trial+(dt/Tt)(ua−u_request)/a with Tt=1 s. The factor a is required because changing I changes v by a times the unfiltered input. Do not track against the unshaped request. Keep z, I, w, request, applied command, and measurement distinct.

The prepared program exposes symbolic `ZeroTimeS` and `PoleTimeS`. Keep
`PoleTimeS=2`; set `ZeroTimeS` to2 for the unshaped baseline,3 or10 for the
lead choices. `TrialKind` at `%MW80` selects0 for the target step,1 for heater load extraction, or2 for the declared measurement-noise
trial.
Restart for every60s trial. The physical probe/filter is bypassed; `NoiseC`
is added only in trial2. `RequestedPowerPct` is the shaped request before
limiting, and `IntegralPct` includes the tracking correction divided by a.
Set `SafetyAbort` for zero heat and zero load/noise; normal completion restores75%.
Tune/Validate candidate selection and assessment remain subject to the
workflow verification below.

## Experiment in Control Studio

**Workflow verification remains open.** The executable project now uses the
specified physical case and Control Studio stages are accessible. The complete
lesson-specific candidate selection and validation interactions have not yet
been verified with these actual-runtime records. Keep the records; ST
calculations do not substitute for the required Control Studio analysis.

1. Restart and confirm T35,SP35,applied75,I75,P0,delayed history75,D0,valid data,dt=.1,z=0.
2. Freeze the two standard trials and the noise trial requirements before selecting baseline, Tz=3, or Tz=10.
3. Use Control Studio Tune to select one scalar candidate, Test to run the appropriate 60 s trial, and Validate to compare retained records. Run every candidate from a full reset, and complete all three trials for every candidate.
4. For the noise trial apply SP35,D0 and the declared n=.05 sin(ωn t) measurement, with ωn=1 rad/s; measure applied-command amplitude over t=6P_n to t=9P_n (37.70–56.55 s). Keep physical T separate from noisy measurement.
5. All times begin at **Confirm and start**. Stop if actual water temperature leaves 0–80 °C, applied command leaves 0–100%, data or clock become invalid, or a fault occurs. Reaching 100% is saturation and is recorded separately. Abort to zero output, cancel D/noise, and reset all states before repeating.

## Evidence and decision

Report temperature peak/error, load minimum/error, noise command amplitude, saturation, crossover screen, and state reset. Tz=10 may be unqualified despite more phase lead if its high-frequency command amplification violates noise activity or output limits. Retain the baseline when no shaped candidate meets all requirements. A nominal phase calculation is not a full stability margin.

## Conclusion

Lead/lag shaping changes dynamic magnitude and phase while preserving or changing high-frequency gain according to pole/zero ordering. The state implementation and scaled tracking correction determine the actual bounded heater command.

## Reason for the next lesson

The lead/lag calculation and finite tests compare response and command movement, but added phase at one frequency does not establish stability of the complete loop. The unity-gain crossing may move, and the delivery delay keeps adding phase lag as frequency increases.

[Nyquist stability](../47-nyquist-stability/README.md) examines the complete directed frequency curve, then returns to these shaped PI candidates instead of treating a single phase number as a stability conclusion.
