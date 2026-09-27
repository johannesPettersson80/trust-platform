# 49. Periodic excitation

**Periodic excitation** repeatedly changes a process input at a known rate.
We will use a sinusoidal heater command to observe how temperature responds
at different rates, choosing bounded amplitude and frequency and collecting
complete cycles for amplitude and phase measurements.

## The problem

The previous stability calculation concerned natural modes. To investigate a continuing periodic input, return to the heater: its stored thermal energy makes it respond differently to slow and fast power changes. The heater is hot at 35 °C with 75% holding command. Use the energy storage, input conversion and heat loss established in [the heater model](../../01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md). Its local values are $C=50$ kJ/°C, $H=10$ kW/°C, $K=0.2$ °C/point, time constant 5 s, delay 1 s, and scan time 0.1 s. A closed-loop controller would change the input while measuring PV, so disable PI and apply a known manual excitation.

## What we need

We need controlled repeating inputs that reveal how the heater responds
at different rates. The resulting record must separate settled periodic
behavior from startup effects so amplitude and timing can be measured.

## The new idea

**Periodic excitation** repeatedly changes a process input at a known rate
to investigate how the process responds. A **period** is one complete cycle's
duration. Frequency is cycles per second; angular frequency expresses the
same repetition in radians per second. **Amplitude** is half the peak-to-peak
change: around 75%, amplitude 5 points gives a command from 70% to 80%.

Choose amplitude $d=5$–10 percentage points and angular frequency $\omega=0.1,0.2,$ or 0.4 rad/s. Use

$$
u(t)=75+d\sin(\omega t),\qquad P=\frac{2\pi}{\omega}.
$$

P is seconds per cycle and $f=\omega/(2\pi)$ is cycles/s (Hz). Run five cycles, discard the first two, and retain the final three. Total duration is $5P$, at most 314.159 s; for $\omega=0.2$, $P=31.4159$ s and duration 157.08 s. The 0.1 s samples provide at least 157 samples per fastest selected cycle.

The command is a deviation around a steady operating point, not a setpoint. A step or long hold reveals equilibrium; a pulse reveals a bounded application and return; a sine reveals response at a selected rate. A ramp tests tracking of a changing input, but a moving input is not a setpoint unless a controller is explicitly following it.

The first two cycles allow the initial thermal transient to decay. Even the fastest selected case has more than 31 s across two cycles, while the supplied 5 s thermal time scale is much shorter. This is a reason for discarding those cycles, not an arbitrary period choice. With no clipping, the temperature amplitude is qualitatively bounded by $K d$ in steady conditions; exact frequency dependence belongs to Lesson 50.

## Prediction

Predict which chosen frequency gives the largest temperature amplitude and why. For $d=5$, $K d=1$ °C is a static-gain amplitude bound. Predict the effect of the 1 s delay on peak timing. State why clipping, invalid samples, or a changed load prevents a clean sinusoidal comparison.

## Implementation

Write scalar ST variables for d, omega, elapsed time, and command. Generate `75.0 + AmplitudePct * SIN(OmegaRadPerS * ElapsedS)`, with the SIN argument in radians. Guard positive amplitude, allowed frequency, elapsed duration, and 0–100% applied output. Keep PI disabled and retain requested/applied command and PV history; no new state machine is needed.

## Experiment in Control Studio

1. Reset PV=35 °C, manual/applied command 75%, D=0, PI disabled, and thermal history at steady state.
2. Before **Confirm and start**, choose d and omega from the allowed values, calculate P and 5P, and declare bounds T=33–37 °C, output 0–100%, sample interval 0.1 s, and the five-cycle clock.
3. At **Confirm and start**, time zero, apply the manual sine for 5P. Display chosen wave, applied command, PV, elapsed time, validity and units.
4. Preserve all five cycles, mark the first two as transient, and use the final three for Lesson 50. Record actual applied amplitude and any phase/quality uncertainty.
5. At normal completion, return the manual command to 75%, retain the record, and restore the full starting state before another frequency. On invalid measurement, clipping, PV outside 33–37 °C, or unresponsive execution, abort, command 0% heat, stop the simulator if needed, and fully restore/requalify PV=35 °C and command 75% before repeating.

## Evidence and decision

Accept the record as a frequency point only when PI stayed off, the applied waveform was unclipped and bounded, five cycles were complete, and the final three cycles were usable. Keep the chosen d, omega, duration, prehistory, applied command, PV, sample time, and uncertainty. A noisy or distorted record is indeterminate for the pure-wave calculation; do not invent amplitude or phase.

## Conclusion

A bounded manual sine separates a known input from the process response. Frequency, amplitude, duration, thermal prehistory, and clipping limits determine whether the record can answer the next question.

## Reason for the next lesson

The retained cycles show the temperature repeating, but we still need
numerical measures of how much the process reduces the input variation and
how far its response lags. [Frequency response](../../07-classical-stability-and-frequency-design/46-bode-frequency-response/README.md)
calculates amplitude ratio and phase from the final three cycles of this record.
