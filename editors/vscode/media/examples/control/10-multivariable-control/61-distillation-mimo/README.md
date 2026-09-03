# Measure distillation interaction and select an RGA pairing

Course position: Lesson 61 of 84. Previous lesson: [Valve stiction diagnosis](../../09-industrial-control-structures/60-valve-stiction/README.md). Next lesson: [Distillation decoupling comparison](../62-distillation-decoupling/README.md).

**Lesson objective:** Measure all four local input-output gains, calculate the Relative Gain Array, and select a defensible decentralized pairing before designing controllers.

**Evidence class:** Simulation evidence. This lesson operates a simulated distillation column; it is not approved for a physical column.

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

The Relative Gain Array ($\boldsymbol{\Lambda}$) compares the open-loop gains with their closed-loop effective gains: each path's gain once ideal feedback holds the other output fixed at its target. Consider a small change in input 1 while output 2 is held by an ideal companion loop. That change alone would move output 2 by $g_{21}$ times itself; to cancel that movement, the companion loop must move input 2 by $-g_{21}/g_{22}$ times the same change. That companion movement then adds $g_{12}$ times $-g_{21}/g_{22}$ to output 1. The closed-loop effective gain from input 1 to output 1 is therefore the open-loop gain plus this correction, $g_{11}-g_{12}g_{21}/g_{22}$, and the relative gain is the open-loop gain divided by that effective gain:

$\lambda_{11}=\dfrac{g_{11}}{g_{11}-g_{12}g_{21}/g_{22}}=\dfrac{g_{11}g_{22}}{g_{11}g_{22}-g_{12}g_{21}}$

The denominator $g_{11}g_{22}-g_{12}g_{21}$ recurs for every entry of $\boldsymbol{\Lambda}$; it is called the **determinant ($\det(G)$)** of the gain matrix. Physically, the determinant tests whether the two inputs create genuinely different output directions: a zero determinant means both columns of $G$ point along the same output direction, so no combination of the two inputs can separate their effects. The **inverse ($G^{-1}$)** answers the reverse physical question, which input change would produce a desired output change, $u=G^{-1}y$, and it exists only when the determinant is nonzero:

$\det(G)=g_{11}g_{22}-g_{12}g_{21}$

$G^{-1}=\det(G)^{-1}\begin{bmatrix}g_{22}&-g_{12}\\-g_{21}&g_{11}\end{bmatrix}$

Repeating the derivation above for every input-output pair is exactly what this matrix formula does in one step: first invert the gain matrix. Then transpose that inverse by exchanging its rows and columns. Finally use element-by-element multiplication, rather than ordinary matrix multiplication, to combine it with $G$:

$\boldsymbol{\Lambda}=\mathbf{G}\circ\left(\mathbf{G}^{-1}\right)^{T}$

Ordinary matrix multiplication combines rows with columns: entry $(i,j)$ of a product is the sum of row-$i$ entries of the first matrix times the matching column-$j$ entries of the second — for example $\begin{bmatrix}1&2\\3&4\end{bmatrix}\begin{bmatrix}5&6\\7&8\end{bmatrix}$ has top-left entry $1(5)+2(7)=19$, not $1(5)=5$. Element-by-element multiplication instead pairs each entry only with the entry already in the same row and column, with no cross terms, so $\lambda_{11}=g_{11}\times\left(G^{-1}\right)^{T}_{11}=g_{11}g_{22}/\det(G)$ reproduces the ratio derived above directly.

Each row and column should sum to one. A relative gain near +1 usually indicates a pairing that changes little when the other loop closes. Large magnitudes indicate strong interaction. A negative relative gain warns that the effective feedback direction reverses. The calculation is invalid for a singular matrix and untrustworthy when a path is unsettled, poorly excited, missing, or measured at another operating point.

## Prediction

In words, the overhead response to reflux takes 20 seconds to reach about 63.2 percent of its settled change. The overhead response to reboiler steam takes 35 seconds. The bottoms response to reflux takes 40 seconds, and the bottoms response to reboiler steam takes 30 seconds. More reflux improves overhead light-product purity but reduces bottoms heavy-product purity. More reboiler steam improves bottoms heavy-product purity but reduces overhead light-product purity in this declared local model.

Lesson 61 uses the final settled changes for pairing. It still retains these unequal transient paths because Lesson 62 will test whether a steady decoupler is sufficient or whether measured dynamic compensation is justified.

The predicted settled gain matrix is:

$\mathbf{G}=\begin{bmatrix}0.8&-0.4\\-0.3&0.7\end{bmatrix}$

These four gains and four response times are **declared teaching values** for this simulation: they are configured directly in the plant model, not calculated from column geometry, transcribed from a vendor data sheet, or fitted to a measured experiment. A learner obtaining them from a real column would run the same bounded, independently restored step used later in this lesson's own experiment — move one valve at a time, wait for both compositions to settle, and calculate each gain from the measured change, exactly as the Executable reference result below does from the shipped test.

A +5-percentage-point reflux move should change overhead and bottoms purity by +4.00 and -1.50 composition-percentage-point respectively. In a separately restored run, a +5-percentage-point reboiler move should change them by -2.00 and +3.50 composition-percentage-point.

The determinant is 0.44, so the matrix can be inverted. The predicted Relative Gain Array is:

$\boldsymbol{\Lambda}=\begin{bmatrix}1.2727&-0.2727\\-0.2727&1.2727\end{bmatrix}$

Here $\det(G)=0.8(0.7)-(-0.4)(-0.3)=0.44$. Therefore
$G^{-1}=0.44^{-1}\begin{bmatrix}0.7&0.4\\0.3&0.8\end{bmatrix}$.
After transposing, elementwise multiplication gives
$\lambda_{11}=0.8(0.7)/0.44=1.2727$ and
$\lambda_{12}=(-0.4)(0.3)/0.44=-0.2727$; symmetry gives the second row.

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

This procedure observes both independently excited input windows under project
control. The project owns the zero baselines, the reflux step, the restored
separation, the reboiler step, and zero restoration. Control Studio writes only
one Boolean authority signal. The complete observation lasts 400.1 seconds and
uses deterministic Simulation seed 20260829.

The native lesson test uses two independently restored runs so that only one input moves at a time. Each run starts with a zero-input baseline for 20 seconds. The first run then continues to change only reflux and hold it for 180 seconds. A new run starts with its own zero-input baseline for 20 seconds, then continues to change only reboiler steam and hold it for 180 seconds. The interactive Test below reproduces both windows in one continuous 400.1-second session for convenience.

### Control Studio actions

1. **Monitor** — Select target **Simulator**, choose **Run**, then open **Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm both purity deviations and both requested/applied input changes, including units, limits, Manual mode, quality, and the 0.1-second interval. The initial point is zero for all four quantities. Retain a 20-second zero baseline, the 20-to-200-second reflux-only window, the 200-to-220-second restored window, and the 220-to-400-second reboiler-only window. Use a mean over the final 10 seconds only when each composition varies by no more than 0.02 composition percentage points. If a path is missing or invalid, report the matrix and pairing as Unconfirmed.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Accept the diagonal overhead-to-reflux and bottoms-to-reboiler pairing only from four valid measured gains and reject the negative-relative-gain cross pairing, because a diagonal relative gain near one means that pairing changes little when the other loop closes, while a negative cross relative gain means closing the other loop would reverse that pairing's effective direction.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial reflux and reboiler requested and applied changes, both purity deviations, Manual mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **400.1**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). The project excites only reflux from 20 to 200 seconds, restores both inputs for 20 seconds, then excites only reboiler steam from 220 to 400 seconds.
   - **Decision before action:** Before selecting **Review test**, predict all four response signs and confirm inputs can remain within -10 through +10 percentage points and purity deviations within -6 through +6 points, because a saturated input or an excursion past those limits would leave the local linear gain model this lesson depends on.
   - **Successful evidence:** Select **Review test**, verify zero external excitation, authority, duration, restoration, and stop conditions, then select **Confirm and start**. Response must show that the non-excited input remains zero in each window. At termination authority clears and both requested inputs return to zero. Keep **Abort test** visible.
   - **Recovery:** Select **Abort test** if the test is rejected, stale, unsafe, unavailable, failed, or aborted. Read the reason, confirm authority is false and both inputs restored to zero, wait for current data, and review a new observation.
   - **Record:** Calculate each settled change and compare it with your prediction. Retain terminal state,
     restoration, and seed **20260829**. The native lesson test owns the qualified gain matrix and
     RGA verdict; this observation does not create generic Identify, Tune, or Validate evidence. The
     inherited limitation under test is: Tag names and isolated direct gains do not show how closing the second loop changes the effective gain and feedback direction. The decision this run
     supports is: Accept the diagonal overhead-to-reflux and bottoms-to-reboiler pairing only from four valid measured gains and reject the negative-relative-gain cross pairing.
## Evidence and decision

Define every measure before reading the result:

- **Input change** is final-window applied-input mean minus baseline-window applied-input mean, in valve percentage points.
- **Output change** is final-window composition mean minus baseline-window composition mean, in composition percentage points.
- **Steady-state gain** is output change divided by the independently excited input change, in composition percentage points per valve percentage point. Reject the calculation when the absolute input change is below 4.95 percentage points.
- **Determinant** is the signed scalar calculated from the four measured gains. A value too close to zero rejects matrix inversion.
- **Relative gain** is one element of the Relative Gain Array calculated from the qualified measured matrix. Retain four elements and every row and column sum.

Classify settling, excitation, signal quality, limits, completeness, and restoration before comparing pairings.

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
