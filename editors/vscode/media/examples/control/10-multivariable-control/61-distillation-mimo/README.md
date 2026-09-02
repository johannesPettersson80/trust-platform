# Measure distillation interaction and select an RGA pairing

**Equipment:** Continuous binary distillation column.

Course position: Lesson 61 of 84. Prerequisite: valve characterization and stiction diagnosis. Next lesson: decentralized proportional-integral control and decoupling on the same column.

This project operates a simulated column. It does not operate a physical distillation plant.

**Lesson objective:** Measure all four local input-output gains, calculate the Relative Gain Array, and select a defensible decentralized pairing before designing controllers.

## The problem

A continuous binary distillation column separates a feed into overhead and bottoms products near one qualified production operating point. Reflux returns condensed liquid at the top. Reboiler steam supplies heat at the bottom.

The overhead light-product purity deviation ($y_{1}$) is the difference between overhead purity and its normal target, in composition percentage points. The bottoms heavy-product purity deviation ($y_{2}$) is the corresponding difference for the bottoms product. The reflux-valve change ($u_{1}$) is a requested change from its normal position, in valve percentage points. The reboiler-steam-valve change ($u_{2}$) uses the same unit.

Both requested changes are limited to -10 through +10 percentage points. The experiment uses +5 percentage points. The outputs must remain between -6 and +6 composition percentage points. These are local simulation limits, not permission to test a physical column.

Lesson 60 showed how independent position feedback localizes a problem in a final control element. In this lesson both final elements are healthy, yet reflux and reboiler steam each affect both product compositions. Tag names and the largest direct response do not reveal how one path changes after the other output is held by feedback. We must measure interaction before choosing loop pairings.

## What we need

We need to measure how each distillation input affects both product purities and use those four gains to choose which input should control which output. The calculated pairing must agree with the observed interaction before any loop is designed.

## The new idea

### Four paths form one gain matrix

The steady-state gain from input j to output i is the settled output change divided by an independently applied input change while the other input remains fixed. It is written gij only after those physical changes have been defined. Its unit here is composition percentage points per valve percentage point.

The first index identifies the output row. The second index identifies the input column. The four gains form the steady-state gain matrix ($G$):

$\mathbf{G}=\begin{bmatrix}g_{11}&g_{12}\\g_{21}&g_{22}\end{bmatrix}$

For example, the gain from reflux-valve change to overhead purity deviation is g11. The gain from reboiler-steam-valve change to overhead purity deviation is g12.

### Why open-loop gains are not enough

Loop interaction means that one controller's action changes the plant seen by the other controller. An open-loop gain is measured while the other input is fixed. After the other output is regulated, its controller moves that input and changes the first path's effective gain. In a strongly interacting plant, the effective gain can become much larger or even reverse sign.

The Relative Gain Array ($\boldsymbol{\Lambda}$) compares the open-loop gains with their closed-loop effective gains when the other output is held by ideal feedback. First invert the gain matrix. Then transpose that inverse by exchanging its rows and columns. Finally use element-by-element multiplication, rather than ordinary matrix multiplication:

$\boldsymbol{\Lambda}=\mathbf{G}\circ\left(\mathbf{G}^{-1}\right)^{T}$

Each row and column should sum to one. A relative gain near +1 usually indicates a pairing that changes little when the other loop closes. Large magnitudes indicate strong interaction. A negative relative gain warns that the effective feedback direction reverses. The calculation is invalid for a singular matrix and untrustworthy when a path is unsettled, poorly excited, missing, or measured at another operating point.

## Prediction

In words, the overhead response to reflux takes 20 seconds to reach about 63.2 percent of its settled change. The overhead response to reboiler steam takes 35 seconds. The bottoms response to reflux takes 40 seconds, and the bottoms response to reboiler steam takes 30 seconds. More reflux improves overhead light-product purity but reduces bottoms heavy-product purity. More reboiler steam improves bottoms heavy-product purity but reduces overhead light-product purity in this declared local model.

Lesson 61 uses the final settled changes for pairing. It still retains these unequal transient paths because Lesson 62 will test whether a steady decoupler is sufficient or whether measured dynamic compensation is justified.

The predicted settled gain matrix is:

$\mathbf{G}=\begin{bmatrix}0.8&-0.4\\-0.3&0.7\end{bmatrix}$

A +5-percentage-point reflux move should change overhead and bottoms purity by +4.00 and -1.50 composition-percentage-point respectively. In a separately restored run, a +5-percentage-point reboiler move should change them by -2.00 and +3.50 composition-percentage-point.

The determinant is 0.44, so the matrix can be inverted. The predicted Relative Gain Array is:

$\boldsymbol{\Lambda}=\begin{bmatrix}1.2727&-0.2727\\-0.2727&1.2727\end{bmatrix}$

Record the prediction before testing. The diagonal pairing—overhead purity with reflux and bottoms purity with reboiler steam—is expected to be eligible but still interacting. The cross pairing is expected to be rejected because both relative gains are negative.

## Implementation

This is an open-loop interaction-identification lesson and shall not hide a centralized controller in the experiment. The shipped Structured Text keeps Manual authority while automatic mode is false, accepts independent requests, exposes the limited applied inputs, and returns both inputs to zero if automatic mode is selected:

```st
IF NOT AutoMode THEN
    RefluxApplied := LIMIT(LREAL#-10.0, RefluxRequested, LREAL#10.0);
    ReboilerApplied := LIMIT(LREAL#-10.0, ReboilerRequested, LREAL#10.0);
ELSE
    RefluxApplied := LREAL#0.0;
    ReboilerApplied := LREAL#0.0;
END_IF;
```

The Structured Text does not contain the gain matrix, Relative Gain Array, determinant, or pairing verdict. Control Studio calculates them from the two retained experiments.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Execute one restored step per input, estimate all four gains, and form the Relative Gain Array from measured records. The declared event lasts 200 seconds. It restores Reflux requested change to 0 pp afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Overhead light-product purity deviation (PV, pp) and Bottoms heavy-product purity deviation (PV, pp); Reflux requested change (MV, pp) and Reboiler steam requested change (MV, pp), including units, limits, mode, and quality. The initial operating point is Reflux requested change = 0 pp, Reboiler steam requested change = 0 pp, Overhead light-product purity deviation = 0 pp, Bottoms heavy-product purity deviation = 0 pp. The unresolved baseline problem is: Tag names and isolated direct gains do not show how closing the second loop changes the effective gain and feedback direction. Control Studio Monitor must show Simulation mode, Manual authority, and the declared 0.1-second sample interval before either input step. Control Studio Monitor must show Simulation mode, Manual authority, and the declared 0.1-second sample interval before either input step. Change only reflux in one fresh run, restore it, then in a new run change only reboiler steam. Control Studio Monitor must show Simulation mode, Manual authority, and the declared 0.1-second sample interval before either input step. Retain a zero-input baseline for 20 seconds. Change only reflux and hold it for 180 seconds; restore it, then in a new run change only reboiler steam for 180 seconds. Use the final 10 seconds only when each composition varies by no more than 0.02 composition percentage points. If an input-output path is missing or invalid, report the gain and pairing as Unconfirmed. This is simulation only; physical-column tests require explicit approval.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept the diagonal overhead-to-reflux and bottoms-to-reboiler pairing only from four valid measured gains and reject the negative-relative-gain cross pairing.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Manual output step**, **Amplitude** = **5 pp**, and **Maximum duration (s)** = **200**. The affected signal is **Reflux requested change**; it starts at 0 pp and must restore to 0 pp.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Reflux requested change -10 pp to 10 pp; Reboiler steam requested change -10 pp to 10 pp; Overhead light-product purity deviation -6 pp to 6 pp; Bottoms heavy-product purity deviation -6 pp to 6 pp.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Reflux requested change to 0 pp, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Baseline check:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **Open-loop interaction testing and Relative Gain Array pairing** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **Open-loop interaction testing and Relative Gain Array pairing** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Implementation** and **Evidence and decision**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.

4. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Baseline check:** **Design method** and **Generate candidate** are disabled because **Open-loop interaction testing and Relative Gain Array pairing** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Implementation**, then compare the declared alternatives under **Evidence and decision** before accepting **Open-loop interaction testing and Relative Gain Array pairing**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.

5. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: g11_overhead_from_reflux == 0.8 pp/pp (tolerance 0.01 pp/pp); g21_bottoms_from_reflux == -0.3 pp/pp (tolerance 0.01 pp/pp); g12_overhead_from_reboiler == -0.4 pp/pp (tolerance 0.01 pp/pp); g22_bottoms_from_reboiler == 0.7 pp/pp (tolerance 0.01 pp/pp).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

Define every measure before reading the result:

- **Input change** is final-window applied-input mean minus baseline-window applied-input mean, in valve percentage points.
- **Output change** is final-window composition mean minus baseline-window composition mean, in composition percentage points.
- **Steady-state gain** is output change divided by the independently excited input change, in composition percentage points per valve percentage point. Reject the calculation when the absolute input change is below 4.95 percentage points.
- **Determinant** is the signed scalar calculated from the four measured gains. A value too close to zero rejects matrix inversion.
- **Relative gain** is one element of the Relative Gain Array calculated from the qualified measured matrix. Retain four elements and every row and column sum.

Classify settling, excitation, signal quality, limits, completeness, and restoration before comparing pairings.

### Validation

The executable reference test must identify all four gains within 0.01 composition percentage points per valve percentage point of the prediction. Each 63.2-percent response time must be within 1.0 second of its prediction; this tolerance accounts for estimating settled change from the finite 180-second record rather than an infinite-time asymptote. Each non-excited input must remain fixed. Every Relative Gain Array row and column must sum to one within 0.001.

Accept the diagonal pairing only if both diagonal relative gains are positive, both cross relative gains are negative, the determinant remains separated from zero, and all four paths come from valid settled records. If any path is missing or invalid, report the pairing as `Unconfirmed`; never fill a missing measurement from configured simulator coefficients.

### Executable reference result

The native lesson test compiled the shipped Structured Text, ran both independently restored steps, and obtained this measured matrix:

$\mathbf{G}_{\mathrm{measured}} =\begin{bmatrix}0.7999&-0.3973\\-0.2962&0.6979\end{bmatrix}$

Its determinant was 0.4406. The Relative Gain Array calculated independently from those measured values was:

$\boldsymbol{\Lambda}_{\mathrm{measured}} =\begin{bmatrix}1.2671&-0.2671\\-0.2671&1.2671\end{bmatrix}$

Both records met the settling rule, each non-excited input remained fixed, and every gain stayed within the declared 0.01 tolerance. The measured 63.2-percent response times were 20.0, 34.6, 39.2 and 29.9 seconds, all inside the finite-record tolerance. Control Studio should display the gains as 0.80, -0.40, -0.30 and 0.70 and the relative gains as 1.27 and -0.27; the retained calculation keeps the additional digits shown above. The diagonal pairing qualified and the cross pairing was rejected.

### Retained evidence

The two plots show the independently restored input steps. Each manipulated
input moves both product compositions, so the pairing decision comes from the
measured four-path response rather than from loop names.

![Restored distillation interaction tests](figures/distillation-rga-evidence.svg)

[Complete trace](figures/distillation-rga-evidence-trace.csv) ·
[Decision summary](figures/distillation-rga-evidence-summary.csv)

## Conclusion

The signs follow the declared local material and energy response: each manipulated input improves its nearby product quality while changing the other product in the opposite direction. When one product is held by feedback, that controller moves its input and changes the gain available to the other loop.

The positive diagonal relative gains support the diagonal pairing, but their value above one shows meaningful interaction. The negative cross values warn that closing the companion loop reverses the cross pairing's effective direction. The Relative Gain Array selects a pairing; it does not remove interaction, specify controller gains, represent dynamics, or prove robust stability.

## Reason for the next lesson

Input-output pairing is an engineering decision based on qualified multivariable evidence, not a naming convention. Repeat all four path measurements when operating point, feed, product targets, pressure, inventory, analyzers, or actuators change materially.

This evidence is simulation only. Physical-column tests require an independently reviewed topology, production and safety limits, perturbation and dwell, product disposition, abort and restoration procedures, and explicit approval immediately before action.

The accepted diagonal pairing still interacts. Lesson 62 therefore introduces decentralized proportional-integral control and tests whether static or dynamic decoupling is justified under equal events and model mismatch.
