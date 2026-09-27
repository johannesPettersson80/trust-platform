# 51. Sensitivity functions

You will learn why stronger feedback can improve disturbance rejection while making the heater command more sensitive to measurement noise. Sensitivity functions will let you predict both effects and judge them separately.

## The problem

An extra extraction of heat immediately increases the water’s cooling rate, while sensor noise changes only the temperature value used by the controller. Both can move the request, but they enter at different points and have different units. A sensitivity calculation makes those paths explicit and shows why more loop gain can help load disturbance rejection while increasing command activity from measurement noise.

## What we need

We need to predict separately how feedback responds to a physical heat
disturbance and to noise in the measurement. We must account for their effects
on both actual temperature and actuator demand before choosing a controller change.

## The new idea

**Sensitivity functions** describe how feedback transmits or suppresses
different inputs and disturbances. A physical heat load acts on the water;
sensor noise changes the information used by the controller. We derive their
separate paths so we can predict physical temperature and actuator movement
instead of treating similar-looking measured fluctuations as the same event.

Use

$$
G_u(s)=\frac{K e^{-sL_d}}{\tau s+1}
=\frac{.2e^{-s}}{5s+1}\quad\mathrm{°C/percentage\ point},
$$

for applied command to physical temperature, with delivery delay $L_d=1$ s. An extra extraction $D$ is an immediate thermal input, so

$$
G_D(s)=-\frac{K/2}{\tau s+1}
=-\frac{.1}{5s+1}\quad\mathrm{°C/kW}.
$$

The command delay belongs to $G_u$; it does not belong in $G_D$. Use $C(s)=K_p+K_i/s=5+1/s$ for the illustrative analysis from Lesson 19, hot state 35 °C/75%, $\beta=1$, and filter bypass. Use temperature deviation $y=T_{water}-35$ in °C. The measured deviation is $y+n$; the physical and measured absolute temperatures are $35+y$ and $35+y+n$. The command deviation $u$ is measured from the 75% holding command.

Let $r$ be setpoint deviation, $n$ sensor noise in °C, and $D$ heat extraction in kW. With negative feedback,

$$
e=r-y-n,\qquad y=G_u C e+G_DD.
$$

Let $L=C G_u$ denote the dimensionless loop transfer, distinct from the physical delay $L_d$ in seconds. Substitution gives

$$
(1+L)y=Lr+G_DD-Ln.
$$

Define sensitivity and complementary sensitivity:

$$
S=\frac{1}{1+L},\qquad T=\frac{L}{1+L}.
$$

Then

$$
y=T r+S G_DD-Tn.
$$

The disturbance path is $S G_D$, because extraction enters the physical plant directly. The physical output's response to measurement noise is $-Tn$, but the controller request is

$$
u=C(r-y-n)=-C S n
$$

when $r=0$ and the noise path is isolated. Do not call the measured signal $y+n$ the physical temperature. Adding $S$ and $T$ gives 1 algebraically, but their complex magnitudes do not generally sum to 1. Large $|L|$ away from $-1$ makes $|S|$ small and $|T|$ near one; near $L=-1$, both fractions can become large. At very high frequency $T$ may become small while $C$ still moves the command through the noise path.

At $\omega=.2\ \mathrm{rad/s}$, $C=5-j5$, and

$$
G_u=.1(1-j)e^{-j.2},\qquad L=-j e^{-j.2}.
$$

Thus $|L|=1$, phase is $-101.459^\circ$, and

$$
1+L=(1-\sin .2)-j\cos .2
\approx .80133067-j.98006658.
$$

Its magnitude is $\sqrt{2-2\sin .2}\approx1.26596$, so $|S|=|T|\approx.78991$. Since $|G_D|=.1/\sqrt2=.0707107\ \mathrm{°C/kW}$, a 10 kW sinusoidal extraction gives physical-temperature amplitude about $10(.0707107)(.78991)=.5586$ °C. A .05 °C noise amplitude gives physical-temperature amplitude about $.05(.78991)=.0395$ °C and command amplitude $|C||S|(.05)=7.0711(.78991)(.05)\approx.2793$ percentage points.

## Prediction

Calculate the load and sensor-noise amplitudes above. Predict which path should be reduced by a large low-frequency loop gain and why the measurement-noise command can remain active even when physical temperature barely moves. Explain why the stated delay is included in the calculation, while clipping would invalidate its linear assumptions.

## Implementation

Expose scalar setpoint deviation, physical temperature, sensor noise, measured temperature, extraction $D$, requested/applied command, and the separate plant inputs. Write scalar ST calculations for the real and imaginary parts. For $1+L=a+jb$, multiply numerator and denominator by $a-jb$ to obtain $S=(a-jb)/(a^2+b^2)$. Calculate `Denominator := RealPart * RealPart + ImagPart * ImagPart;`, then `SReal := RealPart / Denominator;` and `SImag := -ImagPart / Denominator;`. Require a positive denominator. Use $T=1-S$ and compute each magnitude from its two components. Do not inject $D$ as command noise or inject $n$ into the physical energy balance.

## Experiment in Control Studio

1. Before starting, set the amplitude-comparison tolerance to 5%, use 0.1 s controller updates with anti-windup tracking time 1 s, and select the illustrative PI gains 5 and 1 with filtering bypassed. Reset hot state to physical temperature 35 °C, measured PV 35 °C, applied command 75%, integral output 75%, proportional output 0%, delayed command history 75%, and no setpoint step. Use $\omega=.2\ \mathrm{rad/s}$.
2. At **Confirm and start**, time zero, run a clean-measurement load test with $D(t)=10\sin(.2t)$ kW for five cycles, duration $10\pi/.2=157.08$ s. This is a signed deviation about the nominal 150 kW heat-loss balance; negative $D$ reduces extraction and remains bounded in this teaching case. Discard the first two cycles and average the final three.
3. Reset fully. Run a separate sensor-noise test with $n(t)=.05\sin(.2t)$ °C, $D=0$, no setpoint step, and the same five-cycle duration. Each run has its own plant state and delayed history.
4. Display true physical temperature separately from measured temperature, requested/applied command, $D$, $n$, and validity. Compare measured amplitudes with the worked $S G_D$, $-Tn$, and $-CSn$ predictions. Include amplitude measurement uncertainty when assessing the declared 5% tolerance.
5. Stop for physical temperature outside 33–37 °C, applied command outside 0–100%, invalid clock/data or an unresponsive monitor. On normal end set extraction and noise to zero, command zero heat, wait for restoration, and fully reset before the other test.

## Evidence and decision

Average only the final three complete cycles and report physical-temperature, measured-temperature and command amplitudes with units. If clipping occurs, record it and mark the linear estimate inapplicable rather than fitting $S$ or $T$ to a saturated trace. Compare the load response through $S G_D$, sensor response through $-Tn$, and command activity through $-CSn$; these are separate evidence claims.

The analysis assumes the illustrative controller, bypass filter and linear plant. It does not claim the same numerical values for a different PI revision, filter or delay. A successful result is a bounded declared response under this model, not a general noise or disturbance guarantee.

## Conclusion

Sensitivity $S$ scales the physical heat-load path, while complementary sensitivity $T$ scales the physical response to measurement noise. The controller command sees $-CSn$, so a quiet physical temperature does not imply a quiet actuator request. Keeping the three paths and their units separate makes the feedback tradeoff testable.

## Reason for the next lesson

The sensitivity calculations explain why disturbance rejection and noise-driven command activity can demand different behavior at different frequencies. They have not yet supplied a controller modification that achieves a useful tradeoff while respecting temperature and output requirements.

[Loop shaping](../49-loop-shaping/README.md) adds a derived dynamic element and assesses its benefit and cost on this same heater.
