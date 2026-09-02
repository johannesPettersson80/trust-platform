# Reveal hidden stored water with state space

Course position: Lesson 36 of 84. Previous lesson: [Poles and plant
responses](../35-pole-zero-plant-classes/README.md). Next lesson: safe experiment
design.

**Lesson objective:** Build an executable state-space model of two
interconnected water tanks, predict both internal levels, and demonstrate why
one downstream input-output reading cannot reveal all stored water.

This lesson produces simulation evidence, not physical-plant evidence.

## The problem

The equipment in this lesson is Two interconnected water tanks with inlet pump and two level transmitters.

**Equipment:** Two interconnected water tanks with inlet pump and two level
transmitters.

An inlet pump feeds an upstream tank. Water then flows into a downstream tank
and finally to a drain. Both tanks have level transmitters, although the
hidden-state comparison initially reveals only downstream level.

The model uses metres for state calculations, centimetres for learner-facing
level plots, and cubic metres per second for pump flow. Tank areas and linear
flow coefficients remain constant in the declared operating region.

Lesson 35 used poles and zeros to predict external input-output behavior. That
view assumes an initial internal condition. Two installations can have the same
pump command and downstream-level reading now but different upstream stored
water and therefore a different future downstream response.

## What we need

We need both stored water levels to remain explicit because the same current downstream measurement can hide different upstream inventories and future responses. The model must advance those internal states and map them to the measured outputs.

## The new idea

A **state variable** is a physical quantity whose present value, together with
future input, is required to predict future behavior. Define upstream level
($h_{1}$) and downstream level ($h_{2}$), both in metres. Their ordered collection is the
**state vector ($x$)**.

Define pump inlet flow ($u$) in cubic metres per second. The measured upstream
and downstream levels form the **output vector ($y$)**. A vector is an ordered
collection of values. A matrix is a table of coefficients; its row meanings,
column meanings, and units must be declared.

Both tank areas, A1 and A2, are **1 square metre**. The inter-tank flow
coefficient (k12) is **0.5 square metres per second**. The downstream outlet
coefficient (k2) is **0.2 square metres per second**. First say the balances in
words:

- upstream accumulation equals pump inflow minus inter-tank outflow;
- downstream accumulation equals inter-tank inflow minus drain outflow.

The equations are then:

$A_{1} \cdot dh_{1}/dt = u - k_{12} \cdot h_{1}$

$A_{2} \cdot dh_{2}/dt = k_{12} \cdot h_{1} - k_{2} \cdot h_{2}$

The **state equation** advances internal state. The **output equation** maps
state and any direct input effect into measurements. With the state order
already declared, write:

$dx/dt = A \cdot x + B \cdot u$

$y = C \cdot x + D \cdot u$

The state matrix ($A$), input matrix ($B$), and output matrix ($C$) are:

$\mathbf{A}=\begin{bmatrix}-0.5&0\\0.5&-0.2\end{bmatrix}\,\mathrm{s}^{-1},\qquad \mathbf{B}=\begin{bmatrix}1\\0\end{bmatrix}\,\mathrm{m}^{-2},\qquad \mathbf{C}=\begin{bmatrix}1&0\\0&1\end{bmatrix}$

The output matrix is the two-by-two identity, and the
direct matrix ($D$) is zero. The first row advances upstream level. The second row
advances downstream level. The two output rows publish upstream and downstream
level respectively.

An **equilibrium** is a constant state under constant input and therefore means
zero accumulation. For 0.1 cubic metres per second, solving the balances predicts **20
centimetres upstream** and **50 centimetres downstream**.

An **eigenvalue** is a natural state-space mode. This triangular state matrix
has eigenvalues `-0.5` and `-0.2 1/s`; these are also poles of the
zero-initial-deviation transfer functions. Those transfer functions do not
contain the plant's current internal state.

## Prediction

For the inlet-step run, predict both levels rising toward 20 cm and 50 cm.

For the hidden-state comparison, the **Empty** plant begins with state
$x = [0.0, 0.0] m$. The **Upstream charged** plant begins with
$x = [0.2, 0.0] m$. Both have the same initial downstream reading and zero
input. Empty should remain at zero. Upstream charged should transfer stored
water downstream, producing about **10.9 centimetres** peak downstream level
near **3.05 seconds**, then drain.

Commit this prediction before revealing either upstream level.

## Implementation

There is no controller under test. The actual Structured Text clamps the one
active pump request and deliberately holds both hidden-state inputs equal:

```st
IF RequestedPumpFlow < LREAL#0.0 THEN
    MainPumpFlow := LREAL#0.0;
ELSIF RequestedPumpFlow > LREAL#0.2 THEN
    MainPumpFlow := LREAL#0.2;
ELSE
    MainPumpFlow := RequestedPumpFlow;
END_IF;

EmptyCasePumpFlow := LREAL#0.0;
ChargedCasePumpFlow := LREAL#0.0;
```

The simulator executes the same two-state matrices in all three cases. There is
no observer, estimator, state feedback, optimal controller, or MPC.

## Experiment in Control Studio

This procedure tests one declared lesson change under controlled conditions. Run the full-state inlet step, restore, compare empty and upstream-charged zero-input cases, then reveal internal states. The declared event lasts 300 seconds. It restores Requested pump flow to 0 m3/s afterward. The deterministic Simulation seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Main upstream level (PV, m) and Main downstream level (PV, m); Main pump flow (MV, m3/s), including units, limits, mode, and quality. The initial operating point is Requested pump flow = 0.1 m3/s, Main pump flow = 0.1 m3/s, Main upstream level = 0 m, Main downstream level = 0 m, Empty-case downstream level = 0 m, Charged-case upstream level = 0.2 m, Charged-case downstream level = 0 m. The unresolved baseline problem is: Identical current downstream readings and inputs do not reveal different upstream stored-water states.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use a state-space representation when internal stored quantities affect future behavior and must remain explicit.
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Set **Experiment** = **Manual output step**, **Amplitude** = **0.1 m3/s**, and **Maximum duration (s)** = **300**. The affected signal is **Requested pump flow**; it starts at 0 m3/s and must restore to 0 m3/s.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Main upstream level 0 m to 1 m; Main downstream level 0 m to 1 m; Charged-case downstream level 0 m to 1 m. Use the run to expose hidden state only; do not Identify, Tune, or Apply.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Requested pump flow to 0 m3/s, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **20260829**.

3. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Baseline check:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: main_upstream_equilibrium >= 0.1998 m (tolerance 0.0002 m); main_downstream_equilibrium >= 0.4997 m (tolerance 0.0003 m); charged_downstream_peak >= 0.1065 m (tolerance 0.002 m); charged_peak_time >= 2.9 s (tolerance 0.2 s).
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.

## Evidence and decision

### Measures and comparison

**Upstream equilibrium error** and **downstream equilibrium error** are absolute
measured-minus-predicted differences in centimetres after both late slopes meet
the declared settling limit.

**Hidden-state downstream separation** is the absolute difference between the
two downstream levels at the same timestamp. **Charged-case peak level** is the
maximum retained downstream sample; **peak time** is its timestamp.

Matrix-to-balance consistency requires every executable sign, coefficient,
row, column, and unit to agree with the physical balances. A plausible curve is
not sufficient evidence.

The first figure deliberately shows only the two identical current downstream
measurements. It contains no future trace and no answer. Write your prediction
and reason in the editable record before opening the reveal.

![Downstream-only hidden-state prediction](figures/two-tank-hidden-prediction.svg)

[Blank pre-reveal prediction record](figures/two-tank-hidden-prediction-summary.csv)

<details>
<summary>Reveal the upstream states</summary>

![Identical downstream readings can hide different stored-water states](figures/two-tank-state-space.svg)

The retained result shows that Case A remains at zero while Case B rises to a
10.857 cm downstream peak because its initially hidden upstream level was
20 cm. Compare this result with the prediction you wrote before opening the
reveal.

</details>

[Retained run](figures/two-tank-state-space-run.csv) ·
[Plot data](figures/two-tank-state-space-trace.csv) ·
[Decision summary](figures/two-tank-state-space-summary.csv)

### Validation

The native executable run measured:

| Evidence | Result |
|---|---:|
| Main upstream equilibrium | 20.000 cm |
| Main downstream equilibrium | 50.000 cm |
| Hidden downstream initial readings | 0.000 / 0.000 cm |
| Charged downstream peak | 10.857 cm |
| Charged downstream peak time | 3.10 s |

The empty case remains zero and the charged case eventually drains. The model
is invalid if states are transposed, units differ, a flow sign is wrong, or the
simulator matrices differ from the lesson—even if one output looks reasonable.

## Conclusion

The state vector preserves both stored-water quantities. A downstream-only
transfer relation describes how pump deviation affects downstream level under
its initial-state assumption. It does not reveal water already stored upstream.
State space separates internal evolution from measurement selection and extends
naturally to multiple inputs and outputs.

## Reason for the next lesson

State space exposes internal dynamics needed later for controllability,
observability, estimation, optimal control, and MPC. It does not automatically
make an unmeasured state known and does not replace model validation. Lesson 37
uses this need for trustworthy models to design a safe, informative experiment
before identification.
