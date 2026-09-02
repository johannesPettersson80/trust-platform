# Decide whether heat-exchanger data can identify a model

Course position: Lesson 40 of 84. Previous lesson:
[dryer model selection](../39-dryer-model-selection/README.md). Next lesson:
[independent model validation](../41-heat-exchanger-validation-uncertainty/README.md).

This produces **Simulation evidence**, not physical-plant approval.

**Lesson objective:** Compare weak open-loop, qualified open-loop, and ordinary feedback records and retain a deterministic qualification decision for each.

## The problem

A plate heat exchanger transfers energy from hot water into a product stream. The hot-water valve is the actuator. Its requested and applied positions are measured in percent. The outlet-temperature sensor is the process measurement in degrees Celsius.

The lesson runs three independent copies of the same plate heat exchanger. Each starts at 20% applied valve position and 50.0 °C outlet temperature. Each has a local steady gain of 0.5 °C per percentage point, a 20 s time constant, a 3 s dead time, and 0.15 °C root-mean-square measurement noise. They use the same physical parameters, distinct deterministic noise streams, and a 100 ms task interval.

The valves are limited to 0–100%. Physical testing additionally requires
reviewed hazards, protections, procedure, and immediate approval.

Lesson 39 assumed its record distinguished candidate parameters. A fitting
program can still return plausible numbers from nearly constant or feedback-
dependent input; numbers alone do not prove sufficient information.

## Decide what the data can support before fitting

### Information in a record

**Identifiability** means that the available input and output data, under stated assumptions, can distinguish the parameters being estimated. An **identifiable parameter** is one for which different plausible values produce measurably different predictions. An **unidentifiable parameter** is one for which several values or model structures remain indistinguishable inside the measurement noise and experiment bandwidth.

**Input span** is the maximum applied input minus the minimum applied input, measured here in percentage points. **Expected response amplitude ($A_{\mathrm{response}}$)** is the predicted repeatable temperature change in degrees Celsius. **Measurement-noise amplitude ($A_{\mathrm{noise}}$)** is the declared ordinary sensor variation in degrees Celsius. The **response-to-noise ratio ($SNR$)** compares those two amplitudes:

\[
SNR=\frac{A_{\mathrm{response}}}{A_{\mathrm{noise}}}.
\]

For this lesson, data qualification requires SNR at least 5, input span at least 5 percentage points, valid finite samples, no clipping or saturation, and a response window of at least 63 seconds. The 63 seconds cover the 3-second dead time plus three 20-second time constants.

Persistent excitation means that the applied input contains enough independent variation for the selected model order. **Collinearity** means that explanatory signals change in nearly the same pattern, so their separate effects cannot be determined reliably. Fast sampling adds rows, not independent information.

### Why ordinary feedback data are different

An **exogenous excitation** is a reviewed input change selected independently of current measurement noise. An **endogenous input** is partly produced in response to the measured output, as occurs in feedback.

**Measurement noise ($n$)** is the sensor variation added to the underlying outlet temperature. **Controller-used temperature ($PV_{\mathrm{used}}$)** is the exact noisy temperature sample used to calculate the next valve request. **Valve deviation ($\Delta MV$)** is applied valve position minus its 20-percent baseline.

The third experiment uses 2 percentage points of valve request per degree Celsius of temperature error:

\[
\Delta MV=2(50-PV_{\mathrm{used}}).
\]

If $PV_{\mathrm{used}}=PV_{\mathrm{true}}+n$, substitution puts the term
$-2n$ directly into valve deviation. Positive measurement noise therefore
causes a negative valve change. The **correlation coefficient** makes that
signed association measurable:

\[
\rho_{xy}=\frac{\sum_k(x_k-\bar x)(y_k-\bar y)}
{\sqrt{\sum_k(x_k-\bar x)^2\sum_k(y_k-\bar y)^2}}.
\]

It ranges from minus one to plus one. A magnitude near one means strong
association, not causation.

Here the controller itself creates strong negative correlation between controller-used temperature and valve deviation. An ordinary open-loop estimator that assumes input is independent of measurement noise is biased on this record. Closed-loop identification requires a method and excitation designed for that dependency.

### Deterministic qualification decisions

Before fitting, reason 1 means insufficient response-to-noise ratio or input span; reason 2 means missing independent excitation or disallowed closed-loop correlation; reason 3 means invalid data or duration; and reason 0 means every required check passed. Invalid data include clipping, saturation, gaps, drift, bad quality, and nonfinite samples.

A rejected record remains available as evidence, but it has no accepted model or parameter estimate.

## Prediction

The poor applied-input step is 0.2 percentage points. Multiplying it by the local gain predicts a 0.1 degree Celsius (0.1 °C) response. Dividing that response by 0.15 °C noise predicts SNR 0.67. It must fail with reason 1.

The qualified 10-percentage-point step predicts 5 °C of equilibrium movement
and $SNR=5/0.15=33.33$. At 63 seconds, 3 seconds of delay leave 60 seconds, or
three time constants, of response. Thus the predicted movement is
$5(1-e^{-3})=4.751$ °C. This is a dynamic-window check, not a claim that the
noisy response has fully settled. It should pass with reason 0 if all other
checks remain valid.

The ordinary feedback record has no independent excitation. The implemented equation should produce a correlation magnitude of at least 0.95 between controller-used temperature and valve deviation. The ordinary open-loop route must reject it with reason 2.

## Implementation

The actual Structured Text keeps requested and applied commands separate. It directly exposes the simple feedback law rather than hiding it in a PID block:

```st
ClosedMeasurementUsed := ClosedTemperature;

IF ExperimentEnabled THEN
    PoorRequested := LREAL#20.2;
    QualifiedRequested := LREAL#30.0;
    ClosedRequested := 20.0 + 2.0 * (50.0 - ClosedMeasurementUsed);
ELSE
    PoorRequested := LREAL#20.0;
    QualifiedRequested := LREAL#20.0;
    ClosedRequested := LREAL#20.0;
END_IF;
```

Each requested value then passes through its own 0–100% valve clamp to produce the applied plant input. No model parameter estimate, candidate fit, score, or accepted answer exists in the program.

## Experiment in Control Studio

Control Studio records all three synchronized copies. Test writes one reviewed
Boolean, `P1.ExperimentEnabled`, for 70.1 seconds. The project owns the weak
20.2% request, qualified 30% request, feedback law, and restoration of every
valve to 20%. Native evidence owns the qualification arithmetic and reason
codes; no model is fitted from a rejected record. The deterministic Simulation
seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Poor outlet temperature (PV, degC) and Qualified outlet temperature (PV, degC); Poor applied valve position (MV, %) and Qualified applied valve position (MV, %), including units, limits, mode, and quality. The initial operating point is Poor applied valve position = 20 %, Qualified applied valve position = 20 %, Feedback applied valve position = 20 %, Poor outlet temperature = 50 degC, Qualified outlet temperature = 50 degC, Feedback outlet temperature = 50 degC. The unresolved baseline problem is: Finite fitted values can look credible even when weak excitation or feedback correlation makes their interpretation invalid.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Retain every record but allow the ordinary open-loop identification route only when amplitude, duration, quality, and independence checks pass.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **70.1**. The only writable signal is `P1.ExperimentEnabled`; it starts false and must restore false. Confirm all requested/applied valves = 20% and all temperatures near 50 °C.
   - **Decision before action:** Before selecting **Review test**, record the predicted SNRs 0.67 and 33.33 and the expected negative feedback correlation. Confirm all valves remain within 0–100% and temperatures within 40–60 °C.
   - **Successful evidence:** Select **Review test**, verify Boolean authority, timeout, false restoration, and stop conditions, then **Confirm and start**. Keep **Abort test** available. Observe the three distinct requested/applied records and all valves returning to 20%.
   - **Recovery:** Abort for stale, invalid, nonfinite, clipped, saturated, gapped, or out-of-range data. Confirm authority false and all requested/applied valves 20% before retrying.
   - **Record:** Retain terminal state, restoration, all input/output samples, controller-used temperature, timing, quality, seed, and the pre-run qualification predictions.

There is no shared Identify stage. Native executable evidence calculates input
span, SNR, duration, and feedback correlation, then assigns reasons 1, 0, and
2. The rejected records remain evidence but never become accepted fits.

## Evidence and decision

### Measures and comparison

Compare the three synchronized records. Only the qualified open-loop record
has sufficient independent excitation.

![Weak, qualified, and feedback-created identification records](figures/poor-excitation-comparison.svg)

[Retained records](figures/poor-excitation-comparison-trace.csv) ·
[Qualification decisions](figures/poor-excitation-comparison-summary.csv) ·
[Reproducible records](figures/poor-excitation-evidence.json)

Define every decision measure before reading the result:

| Measure | Calculation and source | Unit and window | Validity and preferred result |
|---|---|---|---|
| Applied-input span | Maximum applied valve minus minimum applied valve | percentage points, complete record | finite and unclipped; at least 5 |
| Output span | Maximum measured temperature minus minimum measured temperature | °C, complete record | valid quality; larger than ordinary noise |
| Expected response | Local gain times applied-input span | °C, before the run | inside operating region |
| Response-to-noise ratio | Expected response divided by 0.15 °C | dimensionless, before fitting | at least 5 |
| Observation duration | Last timestamp minus step timestamp | s, post-step record | at least 63 s |
| Late slope | Linear temperature slope | °C/s, final 10 s | near zero before an equilibrium claim |
| Feedback correlation | Correlation of controller-used temperature and valve deviation | dimensionless, enabled record | magnitude below the method's declared independence limit |

Also retain gaps, clipping, saturation, drift, quality, exogenous-input status, and the estimator assumption. Report parameter-sensitivity spread across noise-consistent resamples only after basic qualification passes.

### Validation

The executable acceptance result is:

| Record | Applied-input span | Predicted response | Predicted SNR | Independence | Decision |
|---|---:|---:|---:|---|---|
| Poor open loop | 0.2 percentage points | 0.1 °C | 0.67 | Exogenous | Rejected, reason 1 |
| Qualified open loop | 10.0 percentage points | 5.0 °C equilibrium; about 4.75 °C at 63 s | 33.33 | Exogenous | Qualified, reason 0 after 63 s |
| Ordinary feedback | Measured from record | Not used by open-loop route | Not sufficient alone | Endogenous; correlation magnitude at least 0.95 | Rejected, reason 2 |

No rejected path may create a model, expose tuning, or discard its raw evidence. A finite fit returned from a rejected record is not an accepted result.

## Conclusion

The poor record fails because its entire expected response is smaller than ordinary sensor noise. More samples of essentially the same weak change cannot make gain, time constant, and delay distinguishable.

The feedback record fails because the controller makes valve input respond to measured temperature and noise. This violates the open-loop estimator's independence assumption. Fit percentage, faster sampling, or model complexity cannot repair missing independent information.

The qualified step passes because its amplitude, duration, operating region, sample quality, and independence match the declared estimator assumptions.

## Reason for the next lesson

The correct engineering outcome may be **Data inadequate**, with retained samples and a precise reason, rather than a model. On another plant, improve the experiment or choose a method qualified for the way the data were collected. Never tune from evidence that cannot identify the required parameters.

Lesson 41 starts with data that pass these gates and asks a different question: does the fitted model predict independent amplitudes, disturbances, and operating points, and how uncertain is that prediction?
