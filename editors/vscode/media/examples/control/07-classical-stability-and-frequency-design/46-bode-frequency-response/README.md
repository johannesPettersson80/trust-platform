# 50. Frequency response

You will learn how frequency response describes the size and timing of a temperature change caused by repeated changes in heater power. You will measure amplitude ratio and phase lag, then use them to check the heater model.

## The problem

The preceding experiment retained repeating command and temperature traces. We now need numerical measures of how much temperature changed and how far its peaks lagged the applied input. An absolute 75% command and 35 °C temperature hide that small repeated variation. Define $v=u-75$ in percentage points and $y=PV-35$ in °C. The applied input has amplitude $A_u$ in points; the temperature has amplitude $A_y$ in °C. Their ratio and timing are the measured frequency response.

## What we need

We need numerical measures of how much the process changes the size and
timing of a repeating input. Those measures must let us compare different
frequencies and assess whether the model predicts the observed response.

## The new idea

**Frequency response** describes how a linear system changes the amplitude
and phase of a sinusoidal input at each frequency. After startup effects
decay, the output repeats at the same frequency but can be smaller and delayed.
The amplitude ratio measures the size change; phase measures the relative
position within the repeating cycle. We first derive one point for the heater,
then compare it with the measured cycles.

Let $\omega=2\pi/P$ rad/s. For a sinusoidal deviation, use $c(t)=\cos(\omega t)+j\sin(\omega t)$, where $j^2=-1$. Differentiating gives $dc/dt=j\omega c$. The heater model

$$
5\dot y+y=0.2v(t-1)
$$

has a repeating response of the same frequency. Write $v=V c(t)$ and $y=Y c(t)$; the real or imaginary part supplies the physical sine wave. A delay multiplies $c(t)$ by $e^{-j\omega}$, so substitution gives $(1+5j\omega)Y=.2e^{-j\omega}V$. Dividing gives the complex response

$$
G_u(j\omega)=\frac{Y}{V}=\frac{0.2e^{-j\omega}}{1+5j\omega}.
$$

The numerator 0.2 is °C/point, the delay is 1 s, and the time constant is 5 s. Its magnitude is

$$
M=\frac{0.2}{\sqrt{1+(5\omega)^2}}\quad\mathrm{°C/point},
$$

because the delay has unit magnitude. A complex number $a+jb$ has length $\sqrt{a^2+b^2}$ and angle measured from its real axis. The inverse tangent $\arctan(b/a)$ gives that angle here because $a>0$. The lag denominator has angle $\arctan(5\omega)$, and the delay adds $-\omega$ radians, so

$$
\phi=-\omega-\arctan(5\omega).
$$

At $\omega=0.2$ rad/s, $P=31.4159$ s, $M=0.141421$ °C/point, and $\phi=-0.985398$ rad ($-56.459^\circ$). With $A_u=5$ points, predicted $A_y=0.7071068$ °C and peak lag $\Delta t=-\phi/\omega=4.92699$ s. A decibel value needs a dimensionless reference; with 1 °C/point, $20\log_{10}(0.141421)=-16.9897$ dB. The base-10 logarithm is the exponent needed to obtain its argument: $\log_{10}(10)=1$. Decibels compress a wide range of amplitude ratios; the factor 20 makes a tenfold amplitude ratio 20 dB. A **Bode plot** places magnitude and phase on separate graphs against frequency, with equal horizontal distances for equal frequency ratios. Add the other frequency records from the previous lesson to see the trend.

Use the final three complete cycles from Lesson 49, with their actual applied command and PV, period, peak timing and uncertainty. Declare before comparison a 5% magnitude tolerance and 3° phase tolerance, with input-amplitude uncertainty at most 2%, output-amplitude uncertainty at most 2%, and peak-time uncertainty at most 0.1 s. A usable measured point can contradict the model; quality and model agreement are separate decisions.

## Prediction

Using the chosen Lesson 49 frequency, calculate $P$, model M, phase and expected $A_y=M A_u$. Predict how peak-to-peak amplitude gives $A_y=(\max y-\min y)/2$, and why the first two cycles are excluded. Predict how noisy or multi-harmonic waveforms affect peak timing.

## Implementation

Use scalar ST calculations: $M=A_y/A_u$ with $A_u>0$, $P$ from the selected wave, and $\phi_{deg}=-360\Delta t/P$. Preserve units, cycle window, extrema, peak-time uncertainty and actual applied input. Do not implement a transform solver. If a waveform is visibly non-sinusoidal, mark the point indeterminate or use a declared fundamental-fit measurement rule.

## Experiment in Control Studio

1. Open the retained Lesson 49 record and confirm PI was disabled, PV=35 °C, prehistory 75%, chosen d/omega, 0.1 s samples and actual applied command.
2. Confirm the five-cycle record and select only final cycles 3–5. Enter magnitude/phase tolerances and quality criteria before comparing with the model.
3. Display applied deviation v, measured deviation y, extrema, period, peak timestamps, measured M/phase, and model prediction. Record uncertainty and any clipping or drift.
4. Do not restore an actuator from this read-only analysis. If a new frequency point is needed, reuse Lesson 49’s bounded excitation and reset procedure.

## Evidence and decision

A quality pass requires three complete usable cycles, bounded unclipped applied input, stable period and declared peak uncertainty. A model-match pass separately requires the 5%/3° criteria. Report measured values even when the model fails; a discrepancy may identify gain, delay, lag, or operating-point error.

## Conclusion

Frequency response measures how a heater scales a repeated applied command and how far temperature peaks lag. Complex magnitude and angle provide the derivation, while the record supplies measured amplitude, period, and uncertainty.

## Reason for the next lesson

The measured gain and phase describe how the heater responds to an applied command with PI disabled. Once feedback is connected, an extra heat loss and a noisy sensor reading enter at different points and need not have the same effect on temperature or actuator movement.

[Sensitivity functions](../48-sensitivity-functions/README.md) derives those distinct closed-loop paths for the same heater.
