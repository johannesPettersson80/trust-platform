# Control Studio numerical annex

This annex is the numerical contract of the VS Code Control Studio specification.
It defines the mathematics for identification, tuning, analysis and validation.
Part A of the specification governs. This annex never relaxes a Part A rule. It
cites Part A rules by number and restates them only where a formula needs them.
The words MUST, MUST NOT, SHOULD and MAY are normative.

## N0. Scope and conventions

**N0.1 Scope.** This annex defines the formulas, domains, thresholds and order of
tests that Control Studio uses to identify plant models, tune controllers,
analyze stability and margins, and evaluate validation runs. It does not define
controller structures (Part C), stage behavior (Part B), runtime operations (the
runtime protocol annex) or page layout (the page specifications).

**N0.2 Authorities.** The formula authorities are:

- S. Skogestad, "Simple analytic rules for model reduction and PID controller
  tuning", *Journal of Process Control* 13 (2003), 291-309,
  doi:10.1016/S0959-1524(02)00062-8, for SIMC;
- K. J. Åström and T. Hägglund, "Revisiting the Ziegler-Nichols step response
  method for PID control", *Journal of Process Control* 14 (2004), 635-650,
  doi:10.1016/j.jprocont.2004.01.002, for AMIGO and the relay method.

**N0.3 Units.** Each channel carries the unit declared by its loop definition
(B1.4). A static gain `K` has output unit per input unit. A rate gain `Kprime`
has output unit per input unit per second. Times are process seconds (A3). The
annex never converts units. A model whose input or output units do not match the
loop's channels is refused.

**N0.4 Symbols.**

| Symbol | Meaning |
|---|---|
| `K` | Static gain of a self-regulating form |
| `Kprime` | Rate gain of an integrating form |
| `T` | Time constant, in seconds |
| `L` | Delay (dead time), in seconds. The two-point reduction of N5.15 writes this delay as θ; it is the same quantity |
| `Kp`, `Ki`, `Kd` | Controller gains of N0.5 |
| `Ti`, `Td` | Integral and derivative times, in seconds |
| `λ` | Closed-loop time constant of the SIMC formulas (N5.4) |
| `Δt` | Controller task period (actual, not nominal) |
| `Ts` | Sample interval of a discrete model |
| `ω` | Angular frequency, in rad/s |
| `ωN` | Nyquist frequency, `π / Ts` rad/s |
| `e` | Error, `e = r − y`: setpoint minus measured output |
| `n` | Baseline noise estimate (N1.3) |
| `span` | Measured output span (N1.1) |

**N0.5 Control.PID convention.** Every gain in this annex is a gain of the
parallel `Control.PID` form:

```text
u = Kp·e + Ki·∫e dt + Kd·(de/dt),    Ki = Kp / Ti,    Kd = Kp · Td
```

Formula results are ideal (ISA time-constant) PI values. They are translated to the
parallel gains explicitly. No series, incremental, positional, vendor or per-scan
gain conversion is permitted. The exact PI, PD and PID form, timing, derivative filter,
weights, limits, tracking and anti-windup are those of Specification 32.

**N0.6 Continuous and discrete.** Transfer functions, the SIMC and AMIGO formulas,
the integrating rules and the Routh-Hurwitz test are continuous-time, in `s`. A
discrete model is a state-space model with `Ts > 0`. Its frequency response is
evaluated at `exp(jωTs)` (N4.3). Gains are not adjusted for a nominal scan
interval. The controller's integral action uses the actual task interval `Δt`.

## N1. Data preparation

**N1.1 Qualification metrics.** Before any fit, the qualification summary MUST
hold:

- raw and usable sample counts, and the retained duration;
- the median sample interval and the maximum interval deviation;
- the detected gaps (A4.3, A4.4) and the rejected-quality samples;
- the samples reported as clipped or saturated;
- the baseline drift;
- the input span and the output span;
- a time-domain signal-to-noise estimate, the ratio of the response size to `σ` (N2.6);
- the excitation class;
- whether independent validation data is available.

The span of a signal is its maximum minus its minimum over the samples used.

Coherence MUST be recorded as `Not evaluated for time-domain excitation` for
observation and step records. It MUST NOT be derived from a step response.
Suitability for a same-run preview and qualification for controller design are
separate states.

**N1.2 Sample times and resampling (A4.3).** A fit that simulates a model under
the recorded plant input runs on the recorded sample times: the input is held
between samples, each interval `Δt_k` is discretized exactly (for a first-order lag,
`a_k = exp(−Δt_k / T)`), and a delayed input at `t_k − L` is the latest recorded
input at or before that time. An analysis that needs an even grid (frequency
estimation, discrete-time model fitting, the model-check comparison of N3) resamples
the record once: the grid step `Δ` is the median spacing of the recorded times, or
the record period (A4.2); plant inputs are held (zero-order hold) and measured
outputs are linearly interpolated. In both cases an interval longer than `1.5 Δ` is
a gap, and an analysis that needs continuous data refuses the recording and names
the gap's time and length. Isolated simulations sample an exact grid and are not
resampled (A4.6).

**N1.3 Baseline noise.** Let `Wb` be the steady baseline window of B3.4. The
baseline noise estimate is half the peak-to-peak range of the measured output over
that window:

```text
n = ( max(y over Wb) − min(y over Wb) ) / 2
```

The baseline's RMS noise estimate `σ` is taken from the first differences of the
measured output over the same window, so a slow drift does not count as noise:

```text
σ = sqrt( mean over Wb of (y_k − y_(k−1))² / 2 )
```

`n` sets a relay's switching band and the response allowance of Identify's own mode
changes; `σ` sets the signal-to-noise check (N2.6). The Auto-entry allowance of Apply,
Validate's initialization case and the return to Auto after a failed Apply uses its own
measure of the noise (specification 35, A6.4: three standard deviations about the mean,
times √k for averaged samples). The baseline mean of the measured output is `y0`, and the baseline mean of
the plant input is `u0`. Both are taken over `Wb`.

**N1.4 Block averaging.** A fit uses at most 5,000 points per channel (A4.5). A
resampled record with `N > 5,000` points is block-averaged before fitting:

- the block length `b` is the smallest whole number of consecutive samples for
  which `ceil(N / b) ≤ 5,000`;
- the blocks partition the record in time order; the last block holds the
  remaining samples;
- the input value of a block is the mean of its plant input samples; its output
  value is the mean of its measured output samples; its time is the mean of its
  sample times;
- a block that contains a bad, clipped or saturated sample is bad. A bad block is
  excluded from the fit and its reason is kept (B3.9). Raw samples are never
  deleted.

All channels of a fit share one block partition. The block interval is the time
resolution of the fit: a delay cannot be resolved more finely than one block
interval. Block averaging applies to fits. The free-running check of N3 runs on
the full resampled grid, within the limit of 100,000 samples per channel (A7).

## N2. Model forms and fitting

**N2.1 Forms, in order.** Identify fits the forms of B3.5 in this order, from the
simplest:

1. static gain: `y − y0 = K·(u − u0)`;
2. first order: `K / (T·s + 1)`;
3. first order with delay: `K·exp(−L·s) / (T·s + 1)` (FOPDT);
4. integrating with delay: `Kprime·exp(−L·s) / s`;
5. second order: `K / (a2·s² + a1·s + 1)`, with `a2 > 0` and `a1 > 0` for a
   stable form;
6. second order with delay: the form of item 5 multiplied by `exp(−L·s)`
   (SOPDT);
7. state models, up to the maximum order of N2.7.

The model that is selected is the simplest form that is acceptable under N3, except
for the resolved delay of N3.3.

**N2.2 Joint gain and dynamics estimation.** A recorded response is fitted for its
gain and its dynamics together, by the same least-squares grid fit on every path:
the held step of a single run, the estimation record of an independent pair
(N3.4), and any other excitation (N2.6). The fit does not need a steady final
window to fix the gain. The time a measured output first crosses a threshold
never sets a fitted delay or time constant (N2.5).

Let `d_k = u_k − u0` be the plant input deviation at hold sample `k = 1, …, K`. The
hold runs from the step to the end of the hold (B3.4). The step amplitude is
`Δu = ū_hold − u0`, where `ū_hold` is the mean plant input over the hold. A fit
with `Δu = 0` is refused, and the refusal names the missing nonzero step (N2.6).

*Grids.* Let `H` be the fitted span, from the start of the excitation (the step of
a held step or of a setpoint step; the first change of any other excitation) to the
last fitted sample (the end of the hold for a held step), and `Ts` the fit's sample
interval: the median interval of the fitted samples, or the block interval after
block averaging (N1.4). The grids depend only on the record; no grid is capped by
a fixed time, so a slow process keeps its long delay.

- *Time constants:* 45 points spaced evenly on a logarithmic scale from `Ts` to
  `10·H` for a first-order form; 28 points over the same range for each time
  constant of a second-order form, with `T1 ≥ T2`. A time constant longer than
  the hold lets a partial response report its fraction (N2.3).
- *Delays:* from 0 to `0.3·H` in equal steps of `max(Ts, 0.3·H / 50)`, so at most
  51 points; only 0 for a form without delay.
- *Refinement:* around the best grid point, the delay is searched again on the
  whole multiples of `Ts` between its two neighbouring grid delays, together with
  each time constant on 10 points spaced evenly on a logarithmic scale between its
  two neighbouring grid values. The refinement runs three passes; each further
  pass searches the same way between the neighbours of the last pass's best point,
  so a time constant is resolved to about 0.2 %. The refined point replaces the
  grid point when its error `E` is smaller.

For each candidate dynamic shape `D` on these grids:

1. Simulate the unit-gain response `s_k` of `D` to the deviation `d_k`, starting
   from the steady baseline (zero deviation). The plant input is held between
   samples (N1.2).
2. Take the gain as the least-squares value:

   ```text
   K = Σk (y_k − y0)·s_k / Σk s_k²
   ```

3. Compute the error of the candidate:

   ```text
   E = Σk (y_k − y0 − K·s_k)²
   ```

The fit keeps the candidate with the smallest `E`; of equal errors, the first in
grid order (delay, then time constants, each ascending). Its fitted response is
`ŷ_k = y0 + K·s_k`. For an integrating form, `D` is the unit-rate integrator with
delay, and the same formulas give `Kprime`.

The baseline means `y0` and `u0` are taken over the steady baseline window (N1.3).
They are not fitted quantities. The baseline window MUST still be steady (B3.4).

**N2.3 Fraction reached and acceptance (A10.3).** The fraction reached is defined
for a held step whose form has a finite final change:

```text
f = (ȳ_end − y0) / (K · Δu)
```

`ȳ_end` is the mean measured output over the last 5 % of the hold samples (at least
three), so a single noisy sample does not decide `f`. `K · Δu` is the fitted final
change. With an independent validation record (N3.4), `f` is taken on the
estimation record, whose hold the gain is fitted from.

A partial response is accepted when both hold:

- the normalized error of N3.2 over the whole response is within the limit;
- `f ≥ 0.63`.

When the error is within the limit but `f < 0.63`, the fit is refused. The refusal
names `f` in percent and the suggested hold:

```text
H_suggested = L + 3 · Tdom
```

`L` is the fitted delay. `Tdom` is the dominant time constant of the fitted model:
the largest of `1 / |Re p|` over its poles `p`. For FOPDT, the fraction at `L + T` is
`1 − e^−1 = 0.632`, and at `L + 3T` it is `1 − e^−3 = 0.950`. The 63 % criterion
is therefore one time constant past the delay, and the suggested hold reaches 95 %.

The fraction is defined only for a held step with a finite final change. For any
other fit (an integrating form, a state model with an integrator, a pulse that has
returned to baseline, PRBS or multisine), the fraction is not defined. The
acceptance rule is then the error limit of N3 alone, and the report states that the
fraction is not defined.

**N2.3a Integrating or slow.** Over a short hold, the response of a slow
self-regulating process and of an integrating process both look like a ramp. When
the form selected for a held step is integrating, the first-order-with-delay form is
also fitted to the same data. If that fit meets the error limit with a time constant
`T` at least as long as the hold, the data cannot tell the two apart: the fit is
refused, naming the hold, and Identify suggests a longer hold, `L + 3·T` of that
first-order-with-delay fit (N2.3), or a setpoint step in Auto. A loop whose identification policy declares `process: "integrating"` skips
this test. A closed-loop fit (a setpoint step in Auto) is not subject to it: the
controlled output settles, and the plant input's final value tells the two forms
apart.

**N2.4 Uncertainty and final windows.** An automatic identification has no
configured measurement bound. It reports the resolution of its fit, labelled as
such: for a single run, a sample-resolution envelope (each time constant within
one sample interval of its value and not below one interval, the delay from its
value to one sample interval more, the gain between the gains the record's
endpoints and its measured change give); for an independent pair, the search
resolution together with the held-out residuals of the validation record. Neither
is a confidence interval.

Where an absolute measurement bound `b_m` is configured (a bounded step analysis,
whose grids are engineering inputs, N2.5), the parameter uncertainty is
grid-resolved. It is the set of grid points `(y0, K, L, T)` on the analysis's
grids that satisfy both:

- for every hold sample, `|y_k − ŷ_k| ≤ b_m`, where `b_m` is the configured
  absolute measurement bound;
- where a steady final window is declared, the steady value `y0 + K·Δu` lies within
  `b_m` of the mean of that window.

Rules for the set:

- An empty set is reported explicitly. It is never replaced by an interval derived
  from residuals.
- Exact interval intersection is permitted. Random sampling and silent coarsening
  are not.
- The intervals are grid-resolved approximations. They are not statistical
  confidence intervals or profile likelihood, and MUST NOT be labeled as such.
- One-sided zero-delay findings and search-boundary findings are retained. A best
  delay of zero is one-sided, because a delay is never negative: it is kept with
  the result and is not a search boundary, so it never refuses an estimate. A best
  point on any other edge of a grid is a search-boundary finding.
- A timestamp shift of up to one sample is a sensitivity study. It MUST NOT relax
  any missing-sample check.

Where a final window is needed:

- The gain is not fixed by a final window (N2.2).
- The final-window mean is a reported check, not an input. It is compared with the
  fitted steady value `y0 + K·Δu`, and the difference is reported.
- The final-window bounds of this set apply only where a steady final window is
  declared.
- The baseline window is always required, and it MUST be steady (B3.4).
- Integrating forms take no final window. Their rate gain and delay come from
  N2.2.

**N2.5 Onset and delay.** The detectable onset is a crossing of the measured
output. It is separate from the fitted delay `L`. The direction is taken from the
baseline mean `y0` to the final mean `yf`, the mean of the last 10 % of the hold samples (at least
three), independently of the sign of the input step.

- Falling response: onset is the first run of the declared number of consecutive
  samples that are all strictly below `y0 − h`.
- Rising response: onset is the first run of the declared number of consecutive
  samples that are all strictly above `y0 + h`.

Here `h` is the configured sustained-onset threshold. An isolated crossing does not
establish onset. If the measured output does not change, onset is unavailable. The
configured measurement bound, the onset threshold and its sample count, and the
grids of a bounded step analysis are engineering inputs; the grids of an automatic
identification follow from the record (N2.2). None of them is a plant coefficient.

**N2.6 Windows, closed-loop data and noise.**

- Window coverage includes every expected sampling instant inside the requested
  interval, including an instant exactly at its start. A fractional start between
  sampling instants is permitted. Each window has at least two samples.
- The final window, where declared, lies entirely inside the held step and before
  the restoration (B3.4).
- A window error distinguishes missing coverage from too few samples.
- A record with no input change reports the missing nonzero step.

*Closed-loop data (A10.6, B3.3).* A setpoint step in Auto is fitted from the
recorded plant input to the measured output, like any other excitation (direct
identification). The assumption is that the excitation comes from the setpoint
change. Noise feedback through the controller can bias the result when the response
is small against the noise. The signal-to-noise check covers this case. A fit is refused
when its response size is below `20 · σ` (N1.3). For a held step the response size
is the fitted final change `|K·Δu|`; for an excitation fit it is the span of the
model's predicted output. The refusal gives both values and the next action
(B3.9). When `σ` is zero or the baseline has fewer than three samples, the check
is skipped.

**N2.7 State models.** A state model is fitted when no form of N2.1 items 1 to 6 is
acceptable. Its order is bounded by the identification policy's maximum order. The
default is 4, and the supported range is 1 to 8 (B1.4). Each order is scored by N3.
The state-model estimation method is the identification module's existing method.
This annex fixes the order bound and the acceptance rules only. A state model with
an integrator has no finite final change, so N2.3 gives no fraction for it.

**N2.8 Frequency-response fitting, PRBS and multisine.**

- *PRBS* MUST declare a register order, a nonzero seed, an amplitude, a clock
  interval and a sequence length.
- *Multisine* MUST retain every angular frequency, amplitude, phase, sample
  interval, the applied common scale and the peak bound. A tone at or above `ωN`
  is rejected.
- *Frequency response.* From the input and output spectra `Suu`, `Syu` and `Syy`,
  estimated on the same segments, the complex response and the magnitude-squared
  coherence at each tone are:

  ```text
  H(jω) = Syu(jω) / Suu(jω)
  γ²(ω) = |Syu(jω)|² / ( Suu(jω) · Syy(jω) )
  ```

  The estimator is the standard H1 form.

At every requested tone the fit MUST retain the input and output spectra, the
complex response, the magnitude, the phase, the segmentation and the coherence.
Tones are listed in ascending frequency. The phase is unwrapped from the lowest
tone, so each tone's phase lies within 180° of the previous tone's.

A tone whose coherence is below the minimum coherence (0.8 unless the analysis declares a higher minimum) is retained. It
disqualifies the response from transfer-function fitting. A transfer function
fitted from the retained tones MUST be scored on different retained tones or in a
separate excitation run (N3.4). The fitting tones alone are not validation evidence.

**N2.9 Relay qualification.** A relay experiment qualifies cycles by these rules.

- Each measured period starts at an observed low-to-high output transition.
- The recorded baseline, an initial partial high leg and the restoration are not
  complete periods.
- A complete period retains the declared relay output levels within IEC
  floating-point tolerance. A clipped or intermediate output value invalidates that
  period.
- At least three complete periods are required. They qualify only when the
  peak-to-peak amplitude spread and the period spread are each at most 10 % of
  their respective means. The comparison is inclusive, with floating-point
  tolerance. The spread is the maximum minus the minimum over the qualified
  periods.

A record outside either bound is unqualified. The report states which quantity
varies and SHOULD ask for a longer settled record or another permitted method. Periods
that are not repeatable are not averaged into any result.

For qualified periods, let `d` be half the difference of the declared high and low
relay output levels, and let `a` be half the peak-to-peak range of the measured
output, averaged over the periods. The relay quantities are:

```text
Ku = 4 · d / (π · a)     (ultimate gain, Åström and Hägglund 2004)
Pu = mean period of the qualified periods
```

A relay record is not a step record. The step rules of N2.2 to N2.5 do not apply
to it.

**N2.10 Sensor lag: a first-order process with a first-order sensor.** When an accepted
first-order-plus-dead-time fit of a held step leaves a systematic residual, the
structure comparison (lesson workflow WF-016) also fits a candidate with an
additional first-order lag, such as a sensor balance `Cs·dTs/dt = Gs·(T − Ts)`:
`G(s) = K·e^(−L·s) / ((τ·s + 1)·(τs·s + 1))`.

- *Fit.* On the same estimation recording, windows and measurement bound `b` as the
  first-order fit, the candidate is fitted over the grids of `L` and `τ` of that fit
  and a sensor grid of `τs` (default 0.1 s to 10 s in steps of 0.1 s), with the gain
  and the baseline on the lattices of the held-step fit (N2.2). The best candidate
  minimizes the squared residual over the hold.
- *Measurement-consistent set.* Every grid point whose prediction stays within `b`
  of every sample of the hold and of the baseline belongs to the set; the minimum
  and maximum of each parameter over the set are the candidate's uncertainty. An
  empty set is reported as such. A delay at the lower end of its grid is flagged.
- *Search size.* The search is refused, with the advice to narrow the justified
  bounds or widen the grid spacing, when the shapes (`L`, `τ`, `τs` combinations)
  exceed 1,000,000, the shapes times the hold's samples exceed 500,000,000, the
  baselines exceed 100,000, or the shapes times the baselines exceed 20,000,000.
- *Adequacy.* Each structure is adequate when its largest absolute residual is at
  most the declared limit (default 0.2 in the output's unit) and the magnitude of
  its mean residual over the final window is at most the declared limit (default
  0.1).
- *Recommendation.* The first-order model when it is adequate; otherwise the
  additional lag when it is adequate and lowers the RMS residual by more than `b`;
  otherwise neither. Only the recommended structure can be chosen. The physical
  origin of the extra lag stays unresolved until independent evidence (a second
  sensor or a reference) attributes it; the result is provisional until the
  independent check of N3 passes.
- The comparison runs on the estimation recording, before the independent check,
  and only with the windows and measurement assumptions of the first-order
  reference.

## N3. Model acceptance and the independent model check

**N3.1 Free-running prediction.** A model is checked by free-running prediction.
The frozen model is simulated from the recorded initial state (the steady baseline),
driven by the recorded plant input only. It receives no measured output as feedback.
Its prediction `ŷ_k` is compared with the measured output `y_k` on the A4.3 grid.
The residual is `e_k = y_k − ŷ_k`.

**N3.2 Normalized error and limits.** The normalized error over a set `S` of
samples is

```text
e_n(S) = sqrt( mean over S of e_k² ) / span
```

where `span` is the measured output span (N1.1) over the samples of the fit: from
the start of the steady baseline window to the last fitted sample. The samples
checked are the response: from the start of the excitation to the last fitted
sample (N2.2); the steady baseline before it is not counted. Every low-order
fit passes two checks:

- `e_n` over the whole response is within the limit;
- `e_n` over the final 10 % of samples is within the limit. The final set holds
  `max(3, ceil(0.1 · N))` samples, where `N` is the number of samples checked.

The limit is the identification policy's `maximumNormalizedError`. Its default is
`0.15`.

**N3.3 Acceptance and selection.** A form is acceptable when all of these hold:

- both checks of N3.2 pass;
- for a held step with a finite final change, the fraction of N2.3 is at least 0.63;
- the sign rule of N6 holds.

Among acceptable forms, the simplest is selected (N2.1). A more complex form is not
ranked ahead of a simpler acceptable form only because its error is lower. A fit
percentage alone never selects a model.

One exception covers a resolved delay. When first order and first order with delay
are both acceptable, first order with delay is selected if both of these hold:

- its fitted delay `L` is longer than two sample intervals of the fit
  (`L > 2·Ts`; after block averaging, `Ts` is the block interval of N1.4);
- its normalized error over the whole response is below 98 % of first order's.

A resolved delay that improves the fit is evidence of dead time. Dropping it would
make a tuning more aggressive than the plant allows.

**N3.4 Independence.** A model used for tuning or design is fitted on an immutable
estimation record. It is scored on a distinct immutable validation record that
differs in excitation amplitude or sequence. Each residual vector is retained with
the record that produced it. Fitting validation samples, or choosing parameters from
them, is not independent validation. An accepted model keeps its estimation fit
when it is scored on a validation step of changed amplitude. A transfer function
fitted from frequency data is scored on different retained tones or in a separate
excitation run (N2.8).

**N3.5 Model check.** The model check of B5.2 applies the tests of N3.1 and N3.2 to
the frozen model on the validation record. A passing check makes the model
qualified. A failing check blocks Apply.

## N4. Supplied models, frequency analysis and structure

**N4.0 Analysis eligibility.** Each analysis needs the evidence below. An
analysis whose evidence is missing is refused, with the missing evidence named.

| Analysis | Required evidence |
|---|---|
| Live response | Current or retained synchronized samples |
| Recorded experiment | One retained bounded recording |
| Model fit and residuals | A fitted model and the exact estimation data |
| Prediction versus validation | A model and independent validation data (N3.4) |
| Pole-zero map, Bode and Nyquist | An accepted linear dynamic model |
| Root locus | An accepted SISO linear model and a declared controller structure |
| Margins and sensitivity | An accepted model and a current or candidate controller |
| Channel response matrix and RGA | An accepted multivariable gain or transfer matrix with declared channel order |
| Singular values | An accepted frequency-response matrix |
| Uncertainty envelope | Declared uncertainty or an accepted model set |
| Constraint trajectories | Declared constraints and a controller or model prediction that evaluates them |

**N4.1 Supplied models.** A supplied model declares its channels, units, operating
range and delay in seconds. A discrete model also declares its sample interval.
Matrices, polynomials, channel metadata and bounds are checked before any numerical
work.

- A transfer function is continuous. Its coefficients are in descending powers of
  `s`.
- A state-space model has matrices `A`, `B`, `C`, `D` in the declared state, input
  and output order. A continuous model is in continuous time. A discrete model has
  `Ts > 0`.
- The number of channels is independent of the number of states. The matrices need
  not be square.

**N4.2 Model stability.** A model pole is stable when:

- continuous: its real part is negative (strictly in the left half-plane);
- discrete: its magnitude is less than one (inside the unit circle).

A pole on the boundary is marginal, not stable. This is a property of the model, not
of a closed loop. Pole results exclude the dynamics of a pure delay.

**N4.3 Frequency response.** For every output and input pair, at every requested
positive angular frequency `ω`:

- continuous: `G(jω)`;
- discrete: `G(exp(jω·Ts))`, with every `ω` at or above `ωN = π / Ts` rejected;
- a delay `L` multiplies the response by `exp(−jω·L)`.

A singular frequency rejects the request. It never produces a nonfinite point. A
frequency request has 2 to 512 points, a positive increasing range, and at most
16,384 channel-frequency points in total.

Plotted phase is wrapped. Tone tables are unwrapped as N2.8 requires.

An integrating model has the response `Kprime / (jω)`: magnitude `|Kprime| / ω`,
phase −90° for `Kprime > 0` and +90° for `Kprime < 0`. This applies before delay and
controller effects are added. Treating a rate gain as a static gain in a frequency
calculation is a calculation defect. It MUST NOT produce margins.

**N4.4 Selected-channel feedback.** For a plant `G` and a controller `C`, both
selected from supplied channels and connected in negative feedback, the open-loop
response on the requested grid is

```text
L(jω) = G_selected(jω) · C_selected(jω)
```

The controller input units equal the plant output units, and the controller output
units equal the plant input units. Both models are continuous, or both are discrete
with the same sample interval. The other inputs stay at their declared operating
point. A selected-channel closure does not qualify an arbitrary full MIMO controller. A
selected-channel frequency response is a plant response. It is not an inferred
controller margin or a closed-loop stability result (N7).

**N4.5 Multichannel matrices and RGA.** Every matrix and prediction keeps the input
and output order, the channel names, the units and the direction. Singular values
state their convention: dimensional, normalized or decibel. RGA results keep their
numeric values and conditioning. A complex or singular case never receives a pairing
recommendation.

For a square, nonsingular steady-state gain matrix `K` (outputs by inputs), the
relative gain array is

```text
Λ = K ∘ (K⁻¹)ᵀ
```

where `∘` is the element-wise product. `Λ` has the size of `K`.

- A singular `K` is refused. The RGA is then unavailable. Rectangular frequency
  response is not affected.
- A pairing is refused when its diagonal element `λii` is not positive.
- Among pairings with positive diagonal elements, the pairing whose diagonal
  elements are closest to 1 is named. Closeness is the sum of `|λii − 1|` over the pairing's diagonal elements; the smallest sum wins.

**N4.6 State-space structure.** For a state-space model with `n` states, the
controllability and observability matrices are

```text
Mc = [ B, A·B, …, A^(n−1)·B ]
Mo = [ C ; C·A ; … ; C·A^(n−1) ]
```

Their ranks come from the existing numerical state-space module. Full rank is a
local, unconstrained property. It is not bounded reachability, reachability of
arbitrary steady targets, or useful estimation in noise. Continuous-time
stabilizability and detectability results are not reused for discrete models.

*Example (labeled as an example).* For `A = [[−0.04, 0], [0.04, −0.04]]`,
`B = [2, 0]ᵀ` and `C = [0, 1]`:

- `[B, AB] = [[2, −0.08], [0, 0.08]]` has rank 2;
- `[C ; CA] = [[0, 1], [0.04, −0.04]]` has rank 2.

If only `C` changes to `[1, 0]`, the controllability rank stays 2, and
`[C ; CA] = [[1, 0], [−0.04, 0]]` has rank 1.

**N4.7 Sampled-model routes.**

*(a) Exact zero-order-hold conversion.* A continuous model is converted at the actual
task period `Δt`:

```text
Ad = exp(A·Δt)
Bd = ( ∫ from 0 to Δt of exp(A·τ) dτ ) · B
```

This conversion is used for the state-feedback designs of N5.14.

*(b) One-state sampled route.* A discrete SISO model with one state has a positive
scalar transition `a`, sample period `T`, input gain `b`, output coefficient `c`
and zero direct feedthrough. Its exact held-input continuous equivalent is

```text
A = log(a) / T
B = b · A / (a − 1)        (B = b / T when a = 1)
transfer function = c · B / (s − A)
```

The output coefficient and the operating-point coordinates are unchanged.

- A sampled pole with `a ≤ 0`, a direct feedthrough, or a higher-order sampled
  realization is outside this route. It requires a supported sampled-model route,
  which this annex does not define.
- No hidden mode is cancelled to force a fit.
- The route is an analysis adapter. The selected model stays discrete, with its
  sample period, evidence and revision. Its validation runs on the original discrete
  model. The continuous equivalent does not certify the sampled closed loop.

*(c) State-space to transfer function.* For a continuous state-space model:

```text
G(s) = C · (s·I − A)⁻¹ · B + D
```

The calculation stays within the transfer-polynomial order bound. The complete
characteristic denominator is kept. No pole or zero is cancelled where the
cancellation could hide an unstable internal mode. A nonfinite or unresolved
conversion is rejected. The conversion is an analysis representation. The model's
identity, units, operating point and evidence do not change. Validation uses the
originally selected model and its state coordinates.

## N5. Tuning formulas

**N5.1 Common rules.** The formula methods of this section (SIMC, AMIGO and the
constrained comparison) accept a model that Identify has accepted under N3, or an
exact simulation model. Every method operates on the same model, the same units,
the same actual task interval, the same controller limits, the same validation
scenarios and the same objective. A comparison MUST NOT refit or rescale the model
for one method.

Every formula below is evaluated on gain magnitudes, and the sign of N6 is then
applied to each gain. A gain is refused only as N5.7 states.

**N5.2 SIMC PI for a self-regulating model.** Applies to a stable, monotone
first-order or first-order-plus-delay model `K·exp(−L·s) / (T·s + 1)` with
`|K| > 0`, `T > 0` and `L ≥ 0`:

```text
Kp = T / ( |K| · (λ + L) )
Ti = min( T, 4 · (λ + L) )
Ki = Kp / Ti
Kd = 0
```

A stable second-order model with real poles,
`K·exp(−θ·s) / ((τ1·s + 1)(τ2·s + 1))` with `τ1 ≥ τ2 > 0`, is first reduced by
SIMC's half rule (Skogestad 2003): `T = τ1 + τ2/2` and `L = θ + τ2/2`, and then
tuned as above. A second-order model with complex poles is refused for SIMC, with
the reason and the advice to choose Automatic or another method (N5.7).

**N5.3 SIMC PI for an integrating model.** Applies to an accepted integrating model
`Kprime·exp(−L·s) / s` with `|Kprime| > 0` and a resolved positive effective delay
`L > 0`. `Kprime` has controlled-variable units per manipulated-variable unit per
second. It MUST NOT be displayed or used as a self-regulating steady-state gain.

```text
Kp = 1 / ( |Kprime| · (λ + L) )
Ti = 4 · (λ + L)
Ki = Kp / Ti
Kd = 0
```

**N5.4 Closed-loop time constant `λ`.** The response choice sets `λ`:

- Faster: `λ = max(L, T/3)`;
- Balanced: twice the Faster value;
- More robust: four times the Faster value.

For an integrating model, the Faster value is its positive effective delay `L`.
Balanced and More robust use the factors two and four. An integrating model
without a resolved positive effective delay has no response-time scale. It returns
an unavailable reason and creates no candidate.

The three choices change only `λ`. They do not change limits or acceptance
requirements. The numeric `λ` used is shown in the advanced result and saved with
the candidate.

**N5.5 AMIGO PI.** Applies to an accepted stable, monotone FOPDT model with a
resolved positive dead time and `0 < L / T ≤ 1`:

```text
Kp = 0.15 / |K| + (0.35 − L·T / (L + T)²) · T / (|K| · L)
Ti = 0.35·L + 13·L·T² / (T² + 12·L·T + 7·L²)
Ki = Kp / Ti
Kd = 0
```

AMIGO has no free response-speed parameter. The response choice changes the
comparison objective and the ranking, not these coefficients. The result MUST say
so, and MUST NOT present a modified formula as AMIGO. AMIGO is not applicable to
integrating models.

**N5.6 Translation to Control.PID.** A PI result, with its sign from N6, is
translated to the parallel gains of N0.5:

```text
ProportionalGain = Kp
IntegralGain     = Kp / Ti
DerivativeGain   = 0
```

**N5.7 Refusals.** Each of the following returns a deterministic reason and creates
no candidate:

- a zero gain;
- nonfinite parameters;
- a nonpositive required time constant;
- an invalid delay;
- a model normalized error (N3.2) above `0.25`;
- an unsupported model structure;
- a failure of the method's own domain.

A negative gain is not refused by itself. Its sign is checked against the loop's
action (N6). Data that failed qualification remains an Identify failure and does
not reach Tune.

**N5.8 Proportional control for an integrating model.** The plant supplies integral
action, so P stays eligible for an accepted integrating model.

- With a resolved positive delay `L`, P uses
  `Kp = 1 / ( |Kprime| · (λ + L) )`, with the response choices of N5.4. Automatic
  prediction covers it as it covers PI, including the declared input disturbance.
  A P candidate whose prediction fails is refused, and the refusal gives the
  shortfall.
- For a delay-free integrating plant `G(s) = k / s`, the P candidate has the
  closed-loop pole `−λ` with `λ = |k| · |Kp| > 0`. In deviation coordinates
  `y' = k · (u − u0 + d)` with `u = Kp · (r − y)`, the constant error is
  `e∞ = (u0 − d) / Kp`. No controller bias is introduced.
- Target and input-disturbance scenarios are bounded separately:

  ```text
  E∞ = max( |u0|, |u0 − d| ) / |Kp|
  transient envelope = ( |ΔR| + E∞ ) · exp(−λ·t)
  ```

  Here `t` is the elapsed time after the change. A compatible gain is chosen that
  meets the declared error and settling allowances.
- Supplied and identified integrating models use this calculation and the actual-ST
  verification of B5.3. A prediction alone never authorizes Apply.

**N5.9 PD error for a stable first-order alternative.** For a stable first-order
model with proportional DC gain `K`, the steady error of the PD alternative is

```text
error = |ΔR + K·u0| / (1 + K·Kp)
```

`K` and `Kp` are signed (N6). For a loop consistent with its action, `K·Kp > 0`,
so the denominator is `1 + |K|·|Kp|`. A nonpositive closed-loop DC denominator is
rejected. So is an error above the declared error allowance minus the transient
margin. A PD controller is not rejected merely because it has no integrator. A
finite error requirement MAY be met. Dynamic requirements still need the actual-ST
verification of B5.3.

**N5.9a Proportional gain by settling time.** For a stable first-order model with
time constant `τ` and gain `K`, the smallest loop gain `q = K·Kp` that brings the
response into the settling band `b` by the settling time `Tset`, from a setpoint
change `ΔR` at the operating input `u0`, is

```text
q = τ · ln( max(|ΔR|, |K·u0|) / b ) / Tset − 1
```

A proportional design takes the largest of this threshold, the steady-error
threshold of N5.9 and zero.

**N5.10 Constrained comparison.** The constrained comparison is a deterministic,
bounded neighborhood around the qualified SIMC candidate. It evaluates:

- the formula PI of N5.2 or N5.3;
- a less aggressive PI with `0.8·Kp` and `1.25·Ti`. Its integral gain is
  `0.8·Kp / (1.25·Ti) = 0.64·Ki`;
- P: the formula PI with its integral action removed;
- PD: the formula PI with its integral action removed and with
  `Kd = Kp · min( max(L, 0.05·T), 0.25·T )`;
- PID: the formula PI with the same derivative term `Kd`.

The derivative rule uses the model time constant `T`. For an integrating model, PD
and PID are unavailable, and P remains eligible (N5.8). Every non-formula candidate
is labeled `Constrained comparison`, not SIMC or AMIGO. All candidates use the same
validation and the same ranking (N8.6). The comparison creates no candidate when
none passes the mandatory requirements. For other formula domains, unsupported P, PD
or PID candidates are unavailable rather than derived from an unqualified rule.

**N5.11 Automatic recommendation.** Automatic recommendation compares the P, PI, PD
and PID structures that the loop permits. It chooses the simplest candidate that
meets the declared mandatory requirements. A more complex structure is chosen only
when no simpler applicable design can meet the mandatory requirements with the
required evidence. An improvement beyond the mandatory requirements never displaces
a simpler qualifying design (N8.6). For an integrating model, a PI, PD or PID candidate
MUST justify its additional term by a material benefit.

**N5.12 Automatic design to requirements.** The Automatic method designs the
selected structure to the declared requirements. Let `Tset` be the declared
settling time (the settling-time deadline), `Δr` the setpoint change and `b` the
settling band.

*(a) Response choice.* For a formula with a response choice, Automatic uses the
choice nearest to the declared one whose predicted response satisfies both:

- from the settling-time deadline to the end of the test, the response stays within
  the allowed error of the target;
- from the same time, it stays within the settling band of its final value.

When two choices are equally near, the more robust one is used.

*(b) Input disturbance.* When the requirements declare an input disturbance, every
prediction covers it. From the operating point, with the setpoint unchanged, the
disturbance is added to the plant input at the start. From the deadline to the end
of the test, the output stays within the allowed error of the target and within the
settling band of its final value.

*(c) Pole placement.* When no choice meets (a) and (b), and the model is delay-free,
first or second order and without zeros, Automatic places the closed-loop poles:

- damping `ζ = 0.8`;
- the first placement has decay rate `δ1 = max(4, ln(|Δr| / b)) / Tset`. At this
  rate the response envelope reaches the band `b` from `Δr` by `Tset`, and it is
  never slower than the 2 % envelope;
- each following placement is 25 % faster than the previous one, `δk+1 = 1.25·δk`;
- at most eight placements are tried, and each stays below the sample-rate limit
  `ωh < 0.2`, where `ω = δk / ζ` is the natural frequency of the placed poles and
  `h` is the task period in seconds.

The controller's own zeros make the response faster or less damped than the
envelope. Each placement is therefore checked by its prediction. The first
placement whose gains are non-negative in the magnitude frame (N6), and whose
predicted response meets (a) and (b), is used.

If no placement qualifies, the structure is rejected. The result gives the deviation
of its fastest predicted response, the deadline and the limit it misses. Without a
model prediction, the declared choice is used and Validate decides.

SIMC or AMIGO, when selected explicitly, calculates its formula unchanged
(N5.2 to N5.5).

**N5.13 Equivalent representations.** Compatible continuous SISO state-space and
transfer-function representations feed the same controller calculations. The
conversion of N4.7(c) is used. Validation runs on the originally selected model and
its state coordinates.

**N5.14 Sampled state-feedback designs.** Pole placement and LQR use the existing
numerical design functions. A continuous model is first discretized at the actual
task period `Δt` (N4.7(a)).

- Explicit pole targets are discrete poles.
- LQR weights MAY be supplied. Automatic weights are normalized by each coordinate's
  declared operating span, as the design function defines.
- LQG requires supplied process and measurement noise covariances and zero direct
  measurement feedthrough. Its estimator model MUST be the selected sampled model.
- The sampled feedback law in deviation coordinates is `Δu = −K·Δx + Nr·Δr`, where
  `K` is the state-feedback gain and `Nr` the reference gain.

**N5.15 Two-point first-order-plus-delay reduction.** For a model with a finite
final change, let `t28` and `t63` be the times at which its step response reaches
28.3 % and 63.2 % of its final change. The reduction is

```text
τ = 1.5 · (t63 − t28)
L = t63 − τ
```

When `L < 0`, use `L = 0` and `τ = t63`. The gain `K` is the final change per unit
input step. The result is the first-order-plus-delay model
`K · exp(−L·s) / (τ·s + 1)`. Part C uses this reduction for the path models of a group
design.

**N5.16 Ranking.** Candidates are ranked by N8.6. Every formula result is an unapplied
candidate.

### N5.R Relay rule (Feedback relay identification)

A Feedback relay experiment (specification 35, B3.3) switches the plant input between two levels around the
setpoint and records the sustained oscillation of the measurement.

- **Cycles.** A cycle runs from one rising crossing of the relay switching level to the next. At least three
  complete cycles are required. The amplitude and the period of each cycle are measured, and each MUST lie within
  10 % of its mean over the cycles used; otherwise the relay record is not qualified and the reason names the
  quantity.
- **Ultimate gain and period.** With the relay amplitude `d` (half the distance between the two input levels) and
  the measurement's oscillation amplitude `a` (half its peak-to-peak), the describing-function estimate of the
  ultimate gain is `Ku = 4·d / (π·a)`, and the ultimate period `Pu` is the mean cycle period.
- **Relay rule PI.** `Kp = 0.45·Ku` and `Ti = Pu / 1.2` (Ziegler–Nichols), so `Ki = Kp / Ti`. The candidate is
  shown with Ku and Pu, carries the gain sign of the controller action (N6), and is unapplied until Validate passes.
- The rule is offered only for a model fitted from a qualified Feedback relay record of the same study.

## N6. Controller action and gain sign

**N6.1 Action.** Each loop declares its controller action (B1.4) as the direction in
which its output moves:

- `direct`: the output increases while the measurement is below the setpoint, as for
  heating. Let `s_a = +1`.
- `reverse`: the output increases while the measurement is above the setpoint, as for
  cooling. Let `s_a = −1`.

These stored codes keep their meaning from earlier loop files. Messages to the
engineer name the direction in words, never the codes, because instrument practice
uses the same two words the other way round.

**N6.2 Expected plant gain.** The plant gain has the sign `s_a`:

- `direct`: the measurement rises when the output rises. The gain is `K > 0` for a
  self-regulating form, or `Kprime > 0` for an integrating form.
- `reverse`: the measurement falls when the output rises. The gain is `K < 0`, or
  `Kprime < 0`.

**N6.3 Identify check.** Identify compares the sign of the identified gain with the
action. The gain is the static gain `K`, the rate gain `Kprime` of an integrating
form, or the DC gain of a state model. Its sign MUST equal `s_a`. A contradiction is
a failure (B3.9): the message gives the identified gain with its unit, says in words
which way the measurement moves when the output rises, and points to the loop
setup. A zero gain is refused (N5.7).

The same check applies to every model used for tuning, identified or supplied. Tune
refuses a model whose gain sign contradicts the action, with the same reason.

**N6.4 Gains.** Tune designs on gain magnitudes (N5) and gives each gain the sign
`s_a`:

```text
Kp = s_a · |Kp_design|
Ki = Kp / Ti
Kd = s_a · |Kd_design|
```

`Kp_design` and `Kd_design` are the magnitudes from the formulas of N5. `Ki` takes
the sign of `Kp`, because `Ti > 0`. Bounds on gains apply to their magnitudes.

**N6.5 Consistency.** For a loop whose gain sign matches its action (N6.3), the plant
gain and the proportional gain have the same sign, so `K·Kp > 0`. N5.9 depends on
this.

**N6.6 Control.PID.** Control.PID has no direction setting. A loop whose measurement
falls when its output rises uses negative gains. The error keeps the convention of
N0.4, `e = r − y`.

**N6.7 Other laws.** The sign convention of a state-feedback law or any other
non-PID law is given by its Part C template. The check of N6.3 still applies to the
model.

## N7. Stability and margins

**N7.1 Routh-Hurwitz scope.** The test applies to a declared, delay-free, scalar,
continuous-time negative-feedback loop. Let the plant be `Np / Dp` and the controller
`Nc / Dc`. The characteristic polynomial is

```text
Dp·Dc + Np·Nc
```

Uncancelled factors are kept. The selected parameter is set to the requested value,
and the other parameters stay fixed. A delay model is reported as unsupported for
this test. It MUST NOT be replaced by a rational approximation. The analysis changes
no ST and no controller parameter.

**N7.2 Routh table.** Let the polynomial have real, finite, descending coefficients
`a[n], …, a[0]`, with degree `1 ≤ n ≤ 16`.

1. The first row holds `a[n], a[n−2], …`. The second row holds `a[n−1], a[n−3], …`.
   Both rows are padded with zeros.
2. Each later row `C` is built from the two rows above it: `A` is the row two above
   and `B` is the row above. Its entries are

   ```text
   C[j] = ( B[0]·A[j+1] − A[0]·B[j+1] ) / B[0]
   ```

The table reports the original coefficients, every row with its power of `s`, the
operands and results of each calculation, the signs of the first column, the number
of sign changes and the interpretation.

**N7.3 Interpretation.**

- With nonzero, well-conditioned pivots and no roots on the imaginary axis, the sign
  changes in the first column count the right-half-plane poles.
- A zero constant coefficient establishes a pole at the origin. The polynomial is
  then not asymptotically stable.
- An all-zero row or a zero pivot MUST be reported and explained. An unresolved
  special case MUST NOT produce a definitive unstable-pole count or a stable verdict.
- Near-zero pivots, overflow and loss of numerical confidence MUST be reported as
  uncertainty. They MUST NOT be rounded into proof.
- Invalid, all-zero, nonfinite or excessive-degree (above 16) polynomials are
  rejected.

**N7.4 Parameter intervals.** For a characteristic polynomial that is affine in a
real parameter `k`, of degree one to sixteen in `s`, with a fixed nonzero leading
coefficient:

1. Normalize the leading coefficient to be positive.
2. Each coefficient has the form `ci(k) = αi·k + βi`. For degree one or two,
   stability requires every `ci(k) > 0`: each strict inequality is a half-line in
   `k`, the whole real axis, or the empty set (when `αi = 0`), and the interval is
   their intersection over the full real axis.
3. For degree three to sixteen, the stable set is computed exactly, in rational
   arithmetic from the coefficients as written: the Routh first-column conditions
   become polynomial inequalities in `k`; the real roots of those polynomials are
   isolated with Sturm sequences; the stable set is the union of the open intervals
   between consecutive roots on which every condition holds. A zero pivot is carried
   as a formal `ε` (N7.2). Each bound is reported as the root of its named polynomial
   with its isolating interval and decimal value. A computation whose numbers exceed
   32,768 bits gives an explicit unsupported result.
4. Report the open bounds, the empty and all-real cases, and the inequalities with
   their derivation.

The configured allowed domain of the parameter is a separate setting. It MUST NOT be
confused with the mathematical interval. No sampled sweep establishes an interval.
An unsupported degree, a parameter-dependent leading coefficient or an
ill-conditioned inequality gives an explicit unsupported or uncertain result. This
holds even where a table at selected values is possible. The method is not a general
symbolic algebra system. A wider solver needs its own specification and tests.

*Example (labeled as an example).* The shaft has `J = B = 1`, plant
`1 / (s(s + 1))` and controller gain `Kp`. Its polynomial is `s² + s + Kp`, with first
column `1, 1, Kp`. The interval is `Kp > 0`. At `Kp = −1` there is one
right-half-plane pole (the roots are `0.618` and `−1.618`). At `Kp = 0` there is a
pole at the origin and a pole at −1, so the loop is not asymptotically stable. This
result follows from coefficient operations. No branch depends on the plant's name.

**N7.5 Root locus from a model.** A root locus varies one named parameter. All other
parameters stay fixed. For each value of the swept parameter the characteristic
equation is recomputed, and each root is reported with its parameter value. Scaling
other parameters together with the swept one is incorrect. The result retains the
controller structure, the selected and fixed parameters, and the plane convention
(continuous `s`, or discrete `z`). Timing, filter and delay approximations MUST be
identified in the result. A root locus from a model needs an accepted SISO linear
model and a declared controller structure (N4.0).

*Example (labeled as an example).* For the shaft with `J = B = 1` and fixed
measured-velocity feedback `Kd = 7`, the polynomial is `s² + 8s + Kp`. `Kp = 12`
gives poles −2 and −6. `Kp = 16` gives −4 and −4. `Kp = 20` gives −4 + 2j and
−4 − 2j.

**N7.6 Declared root locus.** A declared root locus uses an explicit linear
representation supplied by the engineer. It is not inferred from custom ST.

- The plant is a transfer function with descending numerator and denominator
  coefficients, a delay in seconds, an input unit and an output unit, a source
  (`provided` or `simulation`) and a validation state (`unvalidated` or `validated`).
- The controller has a bounded parameter map. Each parameter has a value, a minimum,
  a maximum and a unit. Its polynomial coefficients are descending. Each coefficient
  is a finite number, or a parameter multiplied by an optional finite scale (default
  1). Every parameter that a coefficient names MUST exist.
- The swept parameter has finite values and a selected value. All other parameters
  stay fixed.
- The plant input and output units MUST match the bound MV and PV units. A mismatched
  model is rejected. No conversion is made.
- The analysis rejects unknown coefficients, a zero denominator, invalid bounds,
  nonfinite values, an empty sweep, a sweep of more than 1,001 values, and a selected
  value that is absent from the sweep.

The analysis needs no live controller and no acquired identification record. Its
result keeps the selected and fixed parameters, the provenance, the approximations
and the gain and pole coordinates. The result does not qualify a candidate for Apply
(B5).

**N7.7 Margins.** For the loop transfer function `L(jω)` of a candidate on the model:

- Gain margin: `GM = −20·log10|L(jω180)|` in dB, at the frequency `ω180` where the
  phase of `L` is −180°.
- Phase margin: `PM = 180° + ∠L(jωc)`, at the gain crossover `ωc`, where
  `|L(jωc)| = 1`.
- Delay margin: `DM = PM (in radians) / ωc`. It is the extra dead time the loop
  tolerates before it becomes unstable.
- Peak sensitivity: `Ms = max over ω of |1 / (1 + L(jω))|`. It is the largest factor
  by which feedback amplifies a disturbance at any frequency.

Rules:

- Margins are shown for every candidate whose law is a linear transfer function on
  the selected model: a `Control.PID`, or a program's P, PI, PD or PID law set up as a
  parallel PID seed. The derivative filter and the model's delay are included.
- A margin the loop does not have is unbounded. This includes the delay margin when
  there is no gain crossover. A margin is unbounded only when the loop has no such
  crossover over the whole frequency axis. An absence within an evaluated range is
  reported as not observed in range (N7.8). It is never reported as infinite.
- The sensitivity functions `S = 1 / (1 + L)` and `T = L / (1 + L)`, and the control
  sensitivity `C·S`, are retained where defined, with their range and peak values.
- Switching and other nonlinear laws show no margins.
- Margins are calculated on the model. They are only as valid as the model.

**N7.8 Bounded-range estimates.** Classical margins of a continuous transfer-function
pair MAY be estimated over a bounded frequency range. Such an estimate is limited by
the sampling and by the range. It is not a stability qualification.

- Where no phase crossover is found in the range, the margin is reported as not
  observed in range, which is unavailable. It is not reported as infinite.
- For a pure delay `L` and an adjacent frequency spacing `Δω`, the phase advance is
  `L·Δω`. If `L·Δω > π/2`, margin estimation is unavailable until the range is
  narrowed.
- The margins of a state-space model in a selected-channel loop are not supported.
- Over the sampled range, the estimate takes the first gain crossover and the first
  phase crossover in increasing frequency, interpolated between the adjacent
  samples. A loop with several crossovers reports the first; the full margins of
  N7.7 decide where they apply.

**N7.9 State-feedback margins.** For a sampled state-feedback law (N5.14) with one
plant input, the loop is broken at the plant input. Use the model sampled at the task
period `Δt`, with `z = exp(j·ω·Δt)`:

```text
L(z) = K · (z·I − A)⁻¹ · B
```

It is evaluated from `ω = 0` up to half the sampling frequency, `ω = π / Δt`. `K`
holds the gains on the fed-back quantities. The margins of N7.7 are those of this
loop. The delay margin is the extra dead time at the plant input that the loop
tolerates.

**N7.10 Nyquist evidence.** A sampled or truncated Nyquist curve does not establish a
complete encirclement count. The point −1 is shown as a reference only. An
encirclement result retains the contour, the unstable-pole count it uses, and any
closed-loop conclusion that the evidence supports.

**N7.11 Closed-loop stability of a delay-free loop.** For a selected-channel loop
without delay:

- The closed-loop poles come from the exact state-space interconnection and the
  eigenvalue solver. The controller's internal dynamics and all plant states stay in
  the interconnection.
- A proper continuous transfer function uses its canonical state-space realization.
  No pole or zero is cancelled silently.
- The algebraic loop is checked first. Let `Dp` and `Dc` be the direct feedthroughs of
  the plant and the controller. For a SISO loop the loop is `1 + Dp·Dc`; for matrices,
  `I + Dp·Dc`. A singular or ill-conditioned loop is rejected before the feedback
  analysis.
- A closed loop is stable when all its poles lie strictly in the left half-plane
  (continuous) or strictly inside the unit circle (discrete). A pole on the boundary is
  marginal, not stable.
- Nonlinear actuator limits, the sampling of a continuous model and undeclared filters
  are not represented.

**N7.12 Pure delay.** A positive pure delay is preserved exactly in the frequency
response. An exact closed-loop pole calculation is then unavailable. This is not
evidence of instability. No delay approximation is inserted silently.

**N7.13 Evidence limits.** A stability result from this annex is separate from a
measured response. A bounded trial does not prove an entire parameter interval, and
it does not override the assumptions of sampling, saturation or the model.

**N7.14 Lead and lag design at a crossover.** For a continuous plant `G` without
a pole on the imaginary axis at the target crossover `ωc > 0`:

- A lead compensator for a phase contribution `0° < φ < 85°` is
  `C(s) = Kc·(1 + T·s)/(1 + α·T·s)` with `α = (1 − sin φ)/(1 + sin φ)`,
  `T = 1/(ωc·√α)`, its zero at `1/T` and its pole at `1/(α·T)`. Its phase at `ωc`
  is `φ`.
- A lag compensator for a low-frequency gain ratio `r > 1` is
  `C(s) = Kc·(1 + s/z)/(1 + s/p)` with its zero at `z = ωc/10` and its pole at
  `p = z/r`, so both break points lie below the crossover and the low-frequency
  loop gain rises by `r`.
- In both, `Kc = 1/(|G(jωc)|·|C₁(jωc)|)`, where `C₁` is the compensator without
  `Kc`, so the compensated loop's magnitude at `ωc` is one. A non-finite gain, or an
  input outside these ranges, is refused with the input it concerns.
- The design is an unapplied analysis result: it reports the gain, the zero, the
  pole, the phase contribution and the loop magnitude at `ωc`.

## N8. Validation conditions and evaluation

**N8.1 Validation conditions record.** Each validation keeps one immutable
validation-conditions record. The current controller and the candidate run under that
one record. Separate records that look alike are not sufficient evidence. The record
contains:

- the complete plant and controller initial condition;
- each setpoint or load event, with its signal, amplitude and time;
- the exact sample-time sequence and the total duration;
- the disturbance trajectory;
- the noise generator, its amplitude and its deterministic seed;
- the output and rate limits;
- the model-variation cases included in the comparison.

Model-prediction validation reconstructs both responses from that record. Validation
from an isolated simulation, a simulated plant, a physical plant or an opened recording
keeps the measured event and the sample sequence (A9).
If a required condition is absent, or differs between the current and the candidate
run, the comparison is invalid, and its dependent requirements are Inconclusive. A
recording without a declared initial state, event sequence, seed and parameter identity
cannot be used as a matched experiment.

**N8.2 Scope.** Validation covers the applicable subset of:

- setpoint response;
- disturbance response;
- rise time, settling time, overshoot and steady-state error;
- manipulated-variable excursion, rate, saturation and recovery;
- noise response;
- gain and phase margins;
- bounded model or parameter variation.

**N8.3 Requirement measurements.** Let `y_k` be the samples of a channel over the
requirement window, at times `t_k`. Where a reference applies, `e_k = y_k − r_k`,
where `r_k` is a constant or a recorded reference.

- Maximum absolute error: `max_k |e_k|`. Mean absolute error: `mean_k |e_k|`.
- RMS error: `rms = sqrt( mean_k e_k² )`. It is unweighted, uses the same sample grid
  and the same quality checks, and is calculated by Control Studio, not in ST.
- Mean error: the arithmetic mean of `e_k` over the declared sample grid.
- Overshoot beyond a declared step target: `max(0, direction · (PV − target))`, where
  `direction` is the sign of the declared step. The step amplitude MUST be nonzero.
- Maximum adjacent-sample rate: `max over k of |y[k+1] − y[k]| / (t[k+1] − t[k])`.
- Saturation duration: the total time in saturation, with the signal held left over
  each adjacent sample interval.
- Switching count: the number of adjacent changes whose size strictly exceeds the
  declared minimum change threshold.
- Signal or state bounds: every value in the window lies within the declared bounds.
- Settling into a target band by a deadline, with a declared hold time: the arrival
  time is the first sample of the last stay inside the band, so that every observed
  sample from it through the window end lies in the band. It MUST be at or before
  the deadline, and the stay MUST last at least the declared hold duration. An
  earlier pass through the band that the response leaves again is not an arrival. The window MUST extend through the deadline plus
  the hold duration. Otherwise the requirement is invalid, because late settling
  cannot be fully observed.

Each requirement window starts and ends on the declared sample grid.

**N8.4 Coverage.** A requirement's coverage is complete when:

- every expected grid sample is present exactly once, in order, and the recording is
  complete over the window (A4.4);
- every channel of the requirement has a good, finite value at every sample.

An isolated simulation has an exact grid (A4.6). A live recording is evaluated on
the declared sample times: each takes the latest recorded sample at or before it,
never an interpolated value, so a requirement is judged only on measured values.
The recorded samples must reach both ends of the window, be ordered and
consecutive, and be good; an interval longer than 1.5 grid steps is a gap (A4.3),
and a gap that touches a requirement's window makes that requirement
Inconclusive, naming the gap's time and length.

Missing, forced, bad, duplicated, gapped or truncated coverage makes the requirement
Inconclusive. Inconclusive never produces a passing measurement. A fully observed
violation is Fail. Other requirements that are independently complete still Pass or
Fail.

**N8.5 Overall result.** The overall result of an evaluation is:

- Fail when any independently observed requirement fails;
- otherwise Inconclusive when any requirement is Inconclusive;
- otherwise Pass.

A requirement set with no requirements is Inconclusive. A required scenario that did
not run, or has no valid source data, is Inconclusive; missing evidence never passes
(B5.4). An invalid specification
rejects the evaluation. A Pass establishes only the declared observed conditions. It
never establishes general stability. Each requirement reports Pass, Fail or
Inconclusive, with its measured value, its limit, its unit, its source and its reason
(B5.4).

**N8.6 Comparison and ranking.**

- A comparison of current and candidate runs requires identical conditions, channel
  identities, units, requirement definitions and evidence class. A mismatch gives
  Inconclusive, and no ranking. Controller parameter values MAY differ between
  candidates. The parameter identity and every experiment condition MUST match.
- A failed or inconclusive candidate cannot rank.
- Among qualifying runs, an explicitly chosen measured requirement is minimized. No
  composite score is used, and no default ranking applies silently.
- Each attempted method keeps one of the states Feasible, Rejected or Unavailable. A
  feasible candidate is one whose mandatory validation requirements all pass.
  Rejected and unavailable candidates rank after every feasible candidate. They stay
  visible, with their reasons.
- Feasible candidates are ranked first by task-appropriate structural complexity. Among
  equally simple candidates, the declared response, effort and robustness objectives
  are applied under matched scenarios. Every raw result is retained.
- An improvement beyond the mandatory requirements never displaces a simpler
  qualifying design.
- No fixed equivalence band (such as 10 %) and no fixed method preference apply.
  Results that are numerically or evidentially indistinguishable are labeled
  equivalent, and they are presented in a deterministic order.
- The result retains the winning structure, method, limiting requirement, rationale,
  exact formula, `λ`, scores, rejected candidates and conventions.

**N8.7 Default noise-response limit.** The default model-predicted limit on the noise
response is

```text
limit = max( 1.2 · RMS_MV , 0.001 · span_out )
```

`RMS_MV` is the root-mean-square movement of the manipulated variable of the current
controller under the noise scenario. `span_out` is the configured span of the
controller output. The absolute floor prevents an almost motionless baseline from
turning a physically negligible increase into a rejection on relative terms alone.
The retained requirement MUST show the numeric limit and its unit. A project-specific
limit MAY replace the default only before the immutable validation plan runs.

**N8.8 Evidence classes.** Results from each source of A9 (model prediction, isolated
simulation, simulated plant, physical plant and opened recording) remain distinct. They
are never merged into one verdict.

**N8.9 Validation does not change the controller.** A validation result does not
write to the running controller.
