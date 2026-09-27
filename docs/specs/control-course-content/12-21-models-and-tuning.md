# Lesson explanations: 12–21

Part of [the course specification](../control-course-teaching-specification.md).
These are the explanations to use in the mapped lessons.

## 12. Process dynamics

**Process dynamics** describe how a physical process responds over time to
changes in its applied input.

Our heater temperature depends on delivered heating, stored energy and heat
loss. In automatic control, PI continually changes the input too. The resulting
temperature trace therefore describes the complete feedback loop, not just
the heater.

A model of the heater itself should tell us:

- How much a held power change alters the final temperature.
- How quickly the temperature approaches that value.
- How long the changed power takes to start affecting the water.

The first relationship is the **process gain**:

$$
K=\frac{\Delta T_{\mathrm{steady}}}{\Delta u}.
$$

Here $\Delta T_{\mathrm{steady}}$ is the settled temperature change in °C and
$\Delta u$ is the applied-command change in percentage points.

For the supplied heater relationship, increasing power from 75% to 85%
changes equilibrium from 35 to 37 °C. Thus $K=2/10=0.2$ °C per point.

In Control Studio we compare a held-input response with a PI-controlled response,
looking at applied power as well as temperature. This requires a real held-input
record: a program that only calculates an equilibrium number while holding75%
is insufficient. From the supplied35°C/75% hot state and one-second75% command
history, the prepared project applies85% for30s when Test enables its declared
experiment authority. While the learner prepares the first test, hold75% so
the35°C state and delayed command history remain valid. Once a trial has run
or a fault has latched, disabling must remove heating, not restore75%. PI remains inactive. The learner uses Review test,
Start recording, Confirm and start, Stop recording and Save result to obtain
the actual applied-input/PV trace. Normal completion or abort removes heating;
a simulator restart restores the full hot initial state. Supervise33–39°C and
0–100% applied-output bounds. The ST program applies the prepared input; it
must not present a computed equilibrium as measured evidence.

Compare this measured held-input response with the Lesson11 PI response in
its owning project's saved results. Calculate process gain from the observed
approximately2°C rise and10-point input change, identify the delay before
movement, and describe the gradual approach. Do not estimate heater time
constant from the closed-loop settling time. This supplied demonstration is
not yet a student-designed identification experiment; lesson14 teaches choosing
and qualifying that experiment. The next lesson derives the heater model from
physical energy flows.

## 13. Energy balance

An **energy balance** states that energy stored in a body changes by the
energy entering minus the energy leaving.

For the heater, water stores energy, electric power adds it and heat loss
removes it. With constant thermal capacity:

$$
C\frac{dT}{dt}=a\,u(t-L)-H(T-T_a)-D.
$$

- $T$ and $T_a$: water and ambient temperature, in °C.
- $C$: thermal capacity, in kJ/°C.
- $a$: delivered power per command point, in kW/point.
- $u$: applied heater command; $L$: delivery delay, in seconds.
- $H$: heat-loss coefficient, in kW/°C.
- $D$: additional heat extraction, in kW.

The left side is the rate of stored-energy change; one kJ/s equals one kW.
The three right-hand terms are delivered heat, ordinary loss and extra loss.

With $C=50$, $a=2$, $H=10$, $T_a=20$ and $L=1$, equilibrium at 75% is 35 °C.
The process gain is $K=a/H=0.2$ °C/point and time constant is $\tau=C/H=5$ s.

A held change from 75% to 85% gives, after the delay:

$$
T(t)=35+2\left(1-e^{-(t-1)/5}\right).
$$

After one time constant, 63.2% of the final change has occurred.

We compare this prediction with the Control Studio trace. These supplied
coefficients explain the model; the next lessons measure what coefficients
an actual response supports.

## 14. Identification experiments

An **identification experiment** deliberately changes a process input to
obtain measurements from which we can estimate a model.

For a **step test**, we change applied heater power once and hold the new
value. We observe how far and how quickly temperature moves. PI is inactive
so it does not keep changing the input we are trying to study.

First we need a steady starting condition. Otherwise existing temperature
drift could be mistaken for a response to our step.

A simple drift check is:

$$
\text{window slope}=\frac{T_{\mathrm{last}}-T_{\mathrm{first}}}
{t_{\mathrm{last}}-t_{\mathrm{first}}}.
$$

Temperatures are in °C and times in seconds. A rise of 0.06 °C over five seconds
gives 0.012 °C/s, exceeding this experiment's 0.01 °C/s baseline limit.

From the 35 °C/75% operating point, a 10-point step predicts a 2 °C final rise.
That is distinguishable from the stated ±0.05 °C measurement uncertainty.
A 30-second hold includes the one-second delay and almost six time constants.

In Control Studio we qualify the baseline, apply the step, record actual
power and temperature, then restore the original power. We choose amplitude
and duration within the stated temperature and output limits.

The project experiment parameters are edited and reviewed in Test through the
existing compiled symbolic parameter binding. The native Test recording includes
five seconds at the 75% baseline before the project is enabled, followed by the
selected 20–40 s hold and a restoration observation. The published 30 s hold
therefore uses a 35.1 s maximum recording envelope. Time zero for the response
calculation is the observed applied-input step within that record.

The next lesson turns that record into estimated parameters.

## 15. Step-response identification

**Step-response identification** estimates a process model from its response
to a held input change.

For a delayed first-order model, we estimate three quantities:

- **Gain $K$:** final output change per input change.
- **Time constant $\tau$:** the scale of the gradual response.
- **Delay $L$:** when the changed input begins affecting the output.

After the delay, the model is:

$$
T(t)=T_0+K\Delta u\left(1-e^{-(t-L)/\tau}\right).
$$

$T_0$ is initial temperature and $\Delta u$ is the command step.
Before the delay, the prediction remains $T_0$.

If a 10-point step changes settled temperature from 35 to 37 °C,
$K=(37-35)/10=0.2$ °C/point. To estimate the timing, rearrange:

$$
\ln\left(\frac{T_f-T(t)}{T_f-T_0}\right)=-\frac{t-L}{\tau}.
$$

Here $T_f$ is final temperature; the logarithm undoes the exponential.
Values approximately −1 at 6 s and −2 at 11 s give slope −0.2 per second,
so $\tau\approx5$ s and $L\approx1$ s.

In Control Studio we use qualified initial/final windows and fit the complete
response, then compare the prediction with the measurements. Noise and sample
timing limit parameter precision. The first detectable rise is not automatically
the true transport delay.

A persistent shape mismatch leads next to reconsidering the model structure.

## 16. Model structure

**Model structure** is the choice of physical relationships and stored
quantities represented in a model. Estimating different coefficients cannot
always compensate for a missing physical mechanism.

Our simple heater model assumes the measured temperature immediately follows
the water temperature. A real probe may take time to warm. Then we need
separate water and sensor temperatures.

A probe energy balance gives:

$$
C_s\frac{dT_s}{dt}=G_s(T-T_s),
\qquad
\tau_s=\frac{C_s}{G_s}.
$$

- $T$: water temperature; $T_s$: sensor temperature.
- $C_s$: sensor thermal capacity, in J/°C.
- $G_s$: heat conductance into the sensor, in W/°C.
- $\tau_s$: sensor time constant, in seconds.

With $C_s=5$ and $G_s=2.5$, $\tau_s=2$ s. The sensor moves toward the water
temperature gradually instead of copying it instantly.

For the same heater step, at 6 seconds the predicted water rise is about
1.264 °C, while this sensor's rise is about 0.883 °C.

In Control Studio we compare the instantaneous-sensor and delayed-sensor
predictions against the same record. A separate water reference or probe
calibration is needed to establish that the extra lag really belongs to
the sensor.

The reusable project reads both temperatures from ordinary PLC inputs. The
native simulation owns the two balances and the one-second transport delay,
including the 35 °C / 75% initial operating point and the physical 0–100%
heater range. Controller ST does not emulate the sensor. The separate learner
state-update exercise remains an explained numerical model calculation.

The next lesson explains why different parameter assignments can sometimes
produce the same observed response.

## 17. Identifiability

**Identifiability** is the ability to distinguish different model parameter
values from the available inputs and measurements.

A good fit does not necessarily mean we have identified a unique model.
For the early part of a first-order response:

$$
\Delta T(t')\approx\frac{K}{\tau}\Delta u\,t',
\qquad t'=t-L.
$$

Here $K$ is process gain, $\tau$ is time constant, $\Delta u$ is the input
step and $t'$ is time since its delayed arrival. Early slope mainly tells us
the ratio $K/\tau$.

For a 10-point step, models $(K,\tau)=(0.2,5)$ and $(0.4,10)$ predict rises
of 0.363 and 0.381 °C after one second. With ±0.05 °C measurement uncertainty,
that difference may not distinguish them. A longer record can.

That is **practical ambiguity**: better observations can help.
**Structural ambiguity** remains even with perfect data. A five-second process
lag followed by a two-second sensor lag has the same input-output response
as those lags in the opposite order, with matched initial conditions and gain.

In Control Studio we compare competing predictions and choose a longer bounded
experiment or independent measurement that can separate them. More samples
of the same indistinguishable signal cannot solve every ambiguity.

Once parameters are sufficiently established, we test predictions on unused data.

## 18. Model validation

**Model validation** checks whether an unchanged model predicts new observations
well enough for its intended purpose.

The fitting record helped us choose parameters. Reusing only that record
would test how well we fitted, not how well we predict another experiment.

For each new measurement, calculate the **residual**:

$$
r_i=T_{\mathrm{measured},i}-T_{\mathrm{predicted},i}.
$$

The root mean square error is:

$$
\mathrm{RMSE}=\sqrt{\frac{1}{N}\sum_{i=1}^{N}r_i^2}.
$$

$i$ identifies a sample and $N$ is the number of samples. Residual and RMSE
both have units of °C. Squaring prevents positive and negative errors from
cancelling: residuals +0.1 and −0.1 °C have mean zero but RMSE 0.1 °C.

In Control Studio we freeze the model, run a different bounded input or load
case, and compare timing, shape and final value. The lesson uses predefined
limits for each, including residuals within ±0.2 °C and final-window
RMSE at most 0.1 °C.

We do not silently refit a failed validation record. We identify the mismatch
and decide whether the model supports the intended control-design decision.

The next lesson uses a sufficiently validated model to calculate PI settings.

## 19. Model-based PI tuning

**Model-based PI tuning** calculates proportional and integral settings from a process model
and a chosen response requirement.

Manual tuning tried gains experimentally. Now we use the validated heater
model to obtain a starting candidate, then still test that candidate.

For the ideal delay-free local model:

$$
\tau\dot y+y=Kv,\qquad e=r-y,
$$

$y$ is temperature deviation, $v$ is power-command deviation, $r$ is desired
temperature deviation and $e$ is error. Choose the desired rate $\dot y=e/\lambda$,
where $\lambda$ is the response time we want.

Substitution gives $v=(\tau e/\lambda+y)/K$. Differentiating and comparing with
the PI derivative $\dot v=K_p\dot e+K_i e$ gives:

$$
K_p=\frac{\tau}{K\lambda},
\qquad
K_i=\frac{1}{K\lambda}.
$$

$K_p$ has units points/°C and $K_i$ points/(°C·s).
With $K=0.2$, $\tau=5$ s and $\lambda=5$ s, the gains are 5 and 1.
Choosing $\lambda=10$ s halves both.

A shorter desired response asks for stronger action. Actual delay, uncertainty
and output limits can prevent that ideal response.

In Control Studio we apply the calculated settings and repeat the same target
and disturbance tests. The next lesson obtains an alternative candidate from
a different experiment: relay tuning.

## 20. Relay tuning

**Relay tuning** repeatedly switches the applied input between two levels
to produce an oscillation whose amplitude and period can be used to estimate
controller settings.

In the step-response experiment, we changed heater power once and held it.
We used the response to estimate process gain, time constant and delay, then
used that model to calculate PI settings.

Relay tuning takes another route. We switch to higher power when temperature
is low and lower power when it is high. We measure the repeating temperature
rise and fall and calculate approximate settings without first fitting that
step-response model.

The commonly used approximation is:

$$
K_u\approx\frac{4d}{\pi a}.
$$

- $d$: command half-amplitude, in percentage points.
- $a$: temperature half-amplitude, in °C.
- $K_u$: approximate proportional gain associated with sustained oscillation,
  in points/°C; it is not the plant's process gain.

The factor $4/\pi$ comes from the switching signal's sine component at the
cycle frequency. The approximation needs a nearly sinusoidal temperature
cycle and sufficiently small switching hysteresis.

One empirical PI rule uses measured cycle period $P_u$:

$$
K_p=0.45K_u,\qquad T_i=P_u/1.2,\qquad K_i=K_p/T_i.
$$

$T_i$ is integral time in seconds. These coefficients are an empirical tuning
rule, not a physical law.

In Control Studio we run the bounded relay test, restore PI, and test its
candidate against the same requirements. The next lesson compares all candidates.

## 21. Tuning comparison

**Tuning comparison** selects between controller settings using matched
experiments and requirements fixed before viewing the results.

We now have candidates from manual, model-based and relay tuning. The method
used to obtain a candidate does not establish that it is suitable.

First, each candidate must pass both the setpoint and disturbance requirements.
Only then do we compare preferences such as settling time and command activity.

Applied-command activity is:

$$
A=\sum_k|u_k-u_{k-1}|.
$$

$u_k$ is applied output at sample $k$ and $A$ is total movement in percentage
points. For commands 75%, 80%, 78%, activity is $5+2=7$ points.
It measures command movement, not electrical energy or actuator wear.

In Control Studio we use identical resets, events, output limits and measurement
windows. The stated selection rule first compares the worst settling time
across the two trials, then command activity among similarly fast candidates.

A failed temperature limit cannot be averaged away by a good result elsewhere.
If none passes, “none qualifies” is the correct outcome.

The next lesson addresses a more specific issue: reducing the proportional
command jump from a setpoint change without weakening measurement feedback.
