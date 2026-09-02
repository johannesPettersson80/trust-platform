# Match excitation to the plant

Course position: Lesson 38 of 84. Previous lesson: [Safe heat-exchanger
experiment](../37-safe-heat-exchanger-experiment/README.md). Next lesson: model
structure selection.

**Lesson objective:** Select and validate informative excitation for a slow
thermal process and a lightly damped mechanical process.

**Equipment:** Slow water heater and resonant flexible-drive servo test stand.

Every result in this lesson is simulation evidence, not physical-plant
evidence.

## The problem

The equipment in this lesson is Slow water heater and resonant flexible-drive servo test stand.

The water heater has a one-sided 0-to-100-percent heating actuator. It operates
at 20 percent and 24 degrees Celsius. Near that point, 1 percentage point of
extra heat eventually raises temperature by 0.2 degrees Celsius. Its time
constant is 5 seconds and its dead time is 1 second.

The flexible drive has a bidirectional torque actuator limited to plus or minus
0.2 newton metres. Shaft position is measured in radians. Its lightly damped
flexible mode has natural frequency 6 radians per second and damping ratio
0.08. Both plants use 100-millisecond sampling and deterministic measurement
noise.

Lesson 37 used a step to expose the slow gain, delay, and lag of a heat
exchanger. A step concentrates most input variation at low frequency. It can
leave a flexible shaft ringing without deliberately separating its resonance,
while a long thermal step can consume unnecessary operating range. The
excitation must match the dynamics we need to observe.

## What we need

We need to choose an input shape whose duration and frequency content expose the dynamics relevant to the intended model without leaving the safe operating envelope. A slow thermal process and a resonant mechanical process require different evidence.

## The new idea

**Excitation** is a deliberate bounded input variation used to make plant
dynamics visible above normal measurement variation. **Amplitude** is the
largest intended departure from the baseline input. **Frequency content** is
the range of change rates present in that input.

A **step** is one sustained input change. Its strong low-frequency content is
useful for gain, delay, and dominant lag. A **pulse** is a step followed by
restoration. It spends less time away from the operating point but can end
before a slow response becomes clear. A **doublet** uses equal positive and
negative pulses. Its approximately zero net input area reduces lasting
displacement and reveals directional differences.

**Relay excitation** is feedback-dependent switching between two levels to
create an oscillation. It requires the limit-cycle safeguards introduced in
Lesson 43, so this open-loop lesson explains it and then proves that selection
is rejected. A timed square wave is not renamed relay feedback.

A **pseudo-random binary sequence (PRBS)** is a deterministic pattern of two
input levels. The irregular switching spreads information across a chosen
frequency band. **Dwell time** is how long each binary level is held. A slow
plant needs longer dwell than a fast plant; switching faster does not create
useful information when the plant cannot respond.

A **chirp** is a sinusoidal input whose frequency changes continuously from a
declared start to a declared end. Its changing frequency maps how strongly and
how late the plant responds across a band.

**Persistent excitation** means that the input contains enough independent
temporal content for every parameter in the chosen model structure. It does
not mean merely collecting more samples.

The expected output change caused by the experiment is the signal amplitude
($A_{\mathrm{signal}}$). The declared root-mean-square measurement variation is the noise
amplitude ($A_{\mathrm{noise}}$). Their dimensionless ratio is the signal-to-noise ratio
($SNR$):

$SNR = A_{\mathrm{signal}} / A_{\mathrm{noise}}$

For the heater, 5 percentage points predict 1 degree Celsius. With 0.02 degree
Celsius noise amplitude, the predicted SNR is 50. High SNR does not make a
clipped or out-of-region experiment valid.

The heater PRBS dwell is 2.5 seconds, half its 5-second thermal lag. The servo
PRBS dwell is 0.3 second, exactly three 100-millisecond task scans. The servo
chirp sweeps from 0.2 to 2 hertz in 20 seconds. It crosses the 0.95-hertz
flexible mode implied by 6 radians per second.

## Prediction

Before running, classify each method by whether it has sustained offset,
returns to baseline, changes sign, has approximately zero signed input area,
and covers low or broad frequency content.

Predict that the heater step gives the clearest final gain while the fast chirp
is attenuated by thermal lag. Predict that the servo doublet has nearly zero
signed torque area and that the chirp's largest position response occurs near
0.95 hertz. Predict relay reason code 4 and no plant movement.

## Implementation

There is no feedback controller and no fitted model. The actual Structured Text
generates the waveform selected by the learner:

```st
CASE MethodSelection OF
    1: (* bounded step *)
        IF (ElapsedSeconds >= LREAL#5.0) AND
           (ElapsedSeconds < LREAL#25.0) THEN
            NormalizedExcitation := LREAL#1.0;
        END_IF;
    6: (* bounded chirp *)
        ChirpPhase := ChirpPhase +
            LREAL#6.283185307179586 * InstantFrequencyHz * LREAL#0.1;
        NormalizedExcitation := SIN(ChirpPhase);
END_CASE;
```

Requested and applied excitation remain separate. The program rejects unsafe
amplitude, insufficient duration, an invalid chirp band, relay feedback, or an
ineligible plant-method pairing before either applied input moves.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Compare pulse, doublet, plant-specific PRBS, and chirp while holding each plant's initial state, amplitude, duration, noise, limits, and seed constant. The declared event lasts 30 seconds. It restores Servo applied torque to 0 N m afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Heater temperature (PV, degC) and Servo shaft position (PV, rad); Heater requested command (MV, %) and Heater applied command (MV, %), including units, limits, mode, and quality. The initial operating point is Plant selection = 0 (dimensionless), Method selection = 1 (dimensionless), Heater temperature = 24 degC, Heater requested command = 20 %, Heater applied command = 20 %, Servo shaft position = 0 rad, Servo requested torque = 0 N m, Servo applied torque = 0 N m. The unresolved baseline problem is: One sustained low-frequency input does not expose every time scale or resonant mode efficiently.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Choose waveform, amplitude, duration, dwell, and frequency band from the plant dynamics and intended model parameters.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **multisine**, **Amplitude** = **0.1 N m**, and **Maximum duration (s)** = **30**. The affected signal is **Servo applied torque**; it starts at 0 N m and must restore to 0 N m.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Heater applied command 0 % to 100 %; Servo applied torque -0.2 N m to 0.2 N m; Servo shaft position -0.35 rad to 0.35 rad. Compare the declared excitation records only; do not Identify, Tune, or Apply.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Servo applied torque to 0 N m, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: heater_step_output_span >= 0.9 degC (tolerance 0.02 degC); heater_chirp_output_span <= 0.6 degC (tolerance 0.02 degC); servo_doublet_signed_area <= 0.01 N m s (tolerance 0.0001 N m s); servo_chirp_peak_frequency >= 0.8 Hz (tolerance 0.15 Hz).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Compare the slow heater and flexible drive separately. The same waveform name
does not imply the same dwell time or useful frequency content on both plants.

![Heater excitation comparison](figures/excitation-heater-comparison.svg)

[Heater samples](figures/excitation-heater-comparison-trace.csv) ·
[Heater decisions](figures/excitation-heater-comparison-summary.csv)

![Flexible-drive excitation comparison](figures/excitation-servo-comparison.svg)

[Servo samples](figures/excitation-servo-comparison-trace.csv) ·
[Servo decisions](figures/excitation-servo-comparison-summary.csv) ·
[All seven executed comparisons and the rejected relay](figures/excitation-methods-evidence.json)

**Input total variation** is the sum of absolute successive applied-input
changes. Its unit is percentage points for the heater and newton metres for the
servo. **Signed input area** is the sampled sum of input deviation multiplied
by sample interval, in percentage-point-seconds or newton-metre-seconds.

**Output span** is maximum minus minimum measured output, in degrees Celsius or
radians. **Return error** is the absolute final applied-input deviation from
baseline. **Heater information ratio** is temperature span divided by declared
temperature-noise amplitude. **Servo resonance peak frequency** is the chirp
frequency at the largest qualified shaft-position amplitude.

Each measure retains its signal, unit, sample interval, window, and
qualification state. A rejected or clipped run is `Not qualified`.

### Validation

The focused executable simulation measured:

| Evidence | Result |
|---|---:|
| Heater step output span | 1.065 degrees Celsius |
| Heater chirp output span | 0.274 degrees Celsius |
| Heater PRBS internal transitions | 5 at 2.5-second dwell |
| Servo PRBS internal transitions | 45 at 0.3-second dwell |
| Servo doublet signed torque area | 0.0000 newton-metre-seconds |
| Servo chirp peak | 1.091 hertz at 0.180 radians |
| Relay request | Rejected with reason code 4 and zero input departure |

The heater step is within 0.10 degree Celsius of its 1-degree prediction, while
the chirp span is less than 60 percent of the step span. The doublet area is
within 0.01 newton-metre-seconds of zero. The servo peak is within 0.15 hertz
of 0.95 hertz. Torque remains inside plus or minus 0.2 newton metres and
position inside plus or minus 0.35 radians.

## Conclusion

The heater behaves as a low-pass process: it follows sustained low-frequency
input but attenuates rapid reversals. The servo stores energy in its flexible
mode, so an input crossing that frequency produces a larger response. PRBS
dwell and chirp sweep band are therefore engineering choices, not cosmetic
settings. Useful information depends on plant response, SNR, limits, and model
validity together—not on waveform complexity.

## Reason for the next lesson

Excitation must match the parameters, time scales, frequency band,
directionality, constraints, and evidence quality of the plant and candidate
model. No waveform is universally best.

Lesson 39 compares model structures because informative excitation still does
not decide whether a plant is first-order, delayed, second-order, or
integrating.
