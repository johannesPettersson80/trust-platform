# Hold a two-stream blending ratio

Course position: Lesson 55 of 84. Previous lesson: [Reactor cascade](../54-reactor-cascade/README.md). Next lesson: [Reactor split-range](../56-reactor-split-range/README.md).

**Lesson objective:** Configure a blending ratio station, make additive demand follow a valid production-flow change immediately, use downstream composition feedback only as trim, and remove ratio authority safely when wild-flow data are invalid or below the controllable range.

**Evidence class:** Simulation evidence. These results do not authorize work on a physical skid.

## The problem

A liquid blending skid combines a base liquid with an additive. Base flow is
set elsewhere by the production schedule, so it is the **wild stream**. The
additive valve supplies the **controlled stream**. Both flows are measured in
litres per minute. A downstream analyser reports litres of additive per litre
of base liquid after mixing and analyser lag.

The normal point is 100 litres per minute of base liquid, 30 litres per minute
of additive, and a measured flow ratio of 0.30. The additive valve can request
0 through 60 litres per minute. Two equivalent local simulated plants compare
composition-feedback-only control with ratio demand plus identical feedback
trim. Both execute every 0.1 seconds.

Lesson 54 used cascade because a faster jacket variable sat inside the reactor
temperature path. A blender has no equivalent nested variable. Its quality
depends on two material flows remaining proportional while production changes
one of them independently.

A fixed additive setpoint becomes wrong as soon as base flow changes.
Composition feedback can eventually correct it, but only after the new mixture
reaches the analyser. Ratio control uses the measured base flow immediately,
before downstream ratio error appears.

## What we need

We need additive flow to follow valid production flow immediately while downstream composition feedback supplies only the slower trim. Ratio authority must be removed safely when the wild-flow measurement is invalid or below the controllable range.

## The new idea

### Material relationship

Measured base-liquid flow is the **measured wild-stream flow ($q_{\mathrm{w}}$)**, in litres
per minute. The calculated additive command is the **additive-flow request
($q_{\mathrm{a}}$)**, also in litres per minute. The desired additive-to-base proportion is
the **ratio setpoint ($R_{\mathrm{SP}}$)**. The analyser's proportion is the **measured blend
ratio ($R_{\mathrm{PV}}$)**. The difference between desired and measured proportion is the
**ratio error ($e_{R}$)**. The analyser controller contributes **feedback trim
($q_{\mathrm{trim}}$)** in litres per minute.

After those quantities are defined, the ratio station is:

$q_{\mathrm{a}} = R_{\mathrm{SP}} \cdot q_{\mathrm{w}} + q_{\mathrm{trim}}$

$e_{R} = R_{\mathrm{SP}} - R_{\mathrm{PV}}$

For a ratio setpoint of 0.30 and wild flow of 100 L/min, untrimmed additive
demand is 30 L/min. At 120 L/min wild flow, it becomes 36 L/min in the same
controller scan.

This flow ratio is not the additive fraction of the final combined stream. If
the ratio is $R_{\mathrm{SP}}$, that fraction would be:

$\text{additive fraction} = R_{\mathrm{SP}} / (1 + R_{\mathrm{SP}})$

The denominator is total flow: one base-flow unit plus $R_{\mathrm{SP}}$
additive-flow units. Thus a flow ratio of 0.30 means an additive fraction
$0.30/1.30=0.2308$, or 23.08 percent of combined flow, not 30 percent.

Confusing those two definitions changes product formulation. Engineering units,
meter zero, range, and scaling must therefore be checked before enabling the
ratio calculation.

### Why retain analyser feedback?

The multiplication is measured-disturbance feedforward. It assumes correct
meter scaling and a known material relationship, but does not measure actual
product quality. A downstream analyser PI supplies trim for modest meter bias,
concentration variation, and local-model error. It shares the valve limits and
anti-windup path with ratio demand.

The lesson keeps untrimmed ratio demand, PI trim, and total requested additive
flow separate; no distinct applied-flow signal is bound because this ideal
simulated valve receives its requested flow directly. A large persistent trim
is evidence of a meter, formulation, or actuator problem; it must not be
hidden as successful control.

### Low-flow singularity

Measured ratio requires division by wild flow. Near zero, small noise or offset
can create an arbitrarily large ratio. The required additive command can also
fall below the dosing valve's useful range. This **low-flow singularity** is a
physical validity problem, not a tuning problem.

The reviewed minimum wild flow is 10 L/min: a **configured teaching threshold**
chosen to stay well clear of the low-flow singularity, safely below the 80
L/min lower edge of the local model's valid production range. A learner
deriving a minimum for a physical skid would instead use the wild-flow
meter's specified turndown (rangeability) and the largest ratio-measurement
uncertainty the process can tolerate, choosing the flow below which the
meter's absolute error makes the reported ratio untrustworthy.

The reviewed maximum of 160 L/min is calculated from the additive valve's
capacity: at ratio setpoint 0.30, the valve saturates when
$q_{\mathrm{w}} = 60 / 0.30 = 200$ L/min. Stopping ratio authority at 160
L/min keeps untrimmed demand at $0.30 \times 160 = 48$ L/min, leaving 12
L/min — 20 percent of valve range — of headroom for trim and transients
before saturation.

Bad-quality, nonfinite, or stale
data, or measured flow outside 10..160 L/min, removes ratio authority. This
simulation then tracks additive demand to 0 L/min. No blend-quality claim is
made until valid flow and the operating state are restored.

### Local mixing model

Measured ratio minus 0.30 is the **blend-ratio deviation ($x_{R}$)**. Applied
additive flow minus 30 L/min is the **additive-flow deviation ($u_{\mathrm{a}}$)**. Actual
wild flow minus 100 L/min is the **wild-flow deviation ($u_{\mathrm{w}}$)**.

The analyser sees mixed product after a two-second lag. The physical ratio is
$R=q_{\mathrm{a}}/q_{\mathrm{w}}$. A **partial derivative ($\partial R/\partial q$)**
is the local slope of $R$ with respect to one flow while the other flow is
held fixed — how much the ratio changes per unit change in that one flow
alone, near the operating point. Because the additive and wild flows can each
change independently, their separate linearized effects on $R$ may simply be
added (superposition): at $q_{\mathrm{a}}=30$ and
$q_{\mathrm{w}}=100$, the local slopes are
$\partial R/\partial q_{\mathrm{a}}=1/100=0.01$ and
$\partial R/\partial q_{\mathrm{w}}=-30/100^2=-0.003$. Keeping only these
first-order changes gives:

$2 \cdot dx_{R}/dt = -x_{R} + 0.01 \cdot u_{\mathrm{a}} - 0.003 \cdot u_{\mathrm{w}}$

This is a local model for 80 through 130 L/min production flow. It is not valid
near zero flow, so the low-flow run proves authority fallback only and must not
be presented as composition performance.

## Prediction

For the nominal production step from 100 to 120 L/min, calculate additive
demand before running:

$q_{\mathrm{a}} = 0.30 \cdot 120\text{ L/min} = 36\text{ L/min}$

Feedback-only remains near 30 L/min until the analyser moves. Its additive
deviation is zero and its wild-flow deviation is 20, so the local model
predicts $x_R=0.01(0)-0.003(20)=-0.06$ and a ratio of $0.30-0.06=0.24$.
Ratio demand changes from 30 to 36 L/min immediately; then
$0.01(6)-0.003(20)=0$, so the two input effects cancel and predict 0.30.

In a separate run, actual wild flow remains 120 L/min while the meter reads
125 L/min, a +5 L/min bias. Untrimmed demand becomes
$0.30\times125=37.5$ L/min. The physical stream still needs 36 L/min, so
predict that analyser feedback trim approaches $36-37.5=-1.5$ L/min and
applied demand returns near 36
L/min. For invalid quality or 5 L/min measured wild flow, predict loss of ratio
authority and a tracked zero additive request.

## Implementation

The shipped program contains two real `Control.PID` controllers with identical
PI trim tuning: proportional gain 50 L/min per unit ratio error and integral
gain 10 L/min per unit ratio error per second. These are **configured
teaching gains**, chosen for a bounded, well-damped trim response; a learner
tuning a physical analyser-trim loop would instead apply Lesson 44's
model-qualified SIMC procedure to the identified two-second mixing/analyser
lag. The only nominal difference between the two controllers is feedforward,
and both the demand calculation and the feedforward path are guarded by the
same ratio-authority signal:

```st
RatioAuthority := WildFlowValid
    AND MeasuredWildFlow >= LREAL#10.0
    AND MeasuredWildFlow <= LREAL#160.0;
IF RatioAuthority THEN
    RatioDemand := RatioSetpoint * MeasuredWildFlow;
ELSE
    RatioDemand := LREAL#0.0;
END_IF;
(* setpoint/process-value assignment and the second authority guard, shown
   next, run later in the same scan *)
IF RatioAuthority THEN
    RatioInput.FeedForward := RatioDemand;
    RatioInput.Mode := PIDMode#Auto;
ELSE
    (* Near zero flow, a reported ratio is physically ill-conditioned. Track
       the reviewed safe request instead of integrating analyser noise. *)
    RatioInput.Setpoint := RatioControlledRatio;
    RatioInput.FeedForward := LREAL#0.0;
    RatioInput.Mode := PIDMode#Track;
    RatioInput.TrackingOutput := LREAL#0.0;
END_IF;
RatioAdditiveRequest := RatioController.Update(Input := RatioInput);
RatioTrim := RatioAdditiveRequest - RatioDemand;
```

When authority is unavailable, the ratio controller uses Track mode with a
zero safe request. The program contains no timed production event, calculated
performance metric, or preferred-controller flag.

## Experiment in Control Studio

The project owns the 100-to-120-L/min event at 30 seconds, both blender paths,
the 90-second observation, and restoration to 100 L/min. Control Studio may
write only `P1.ExperimentEnabled`; it must not inject another flow step. The
90.1-second timeout lets project restoration finish first. Fresh native cases
own meter bias, invalid quality, and low flow. The deterministic Simulation
seed is 20260829.

### Control Studio actions

1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm Blend-ratio setpoint (SP, dimensionless); Feedback-only blend ratio (PV, dimensionless); Feedback-only additive request (MV, L/min), including units, limits, mode, and quality. The initial operating point is Blend-ratio setpoint = 0.3 (dimensionless), Actual wild-stream flow = 100 L/min, Measured wild-stream flow = 100 L/min, Untrimmed ratio demand = 30 L/min, Composition feedback trim = 0 L/min, Feedback-only blend ratio = 0.3 (dimensionless), Ratio-controlled blend ratio = 0.3 (dimensionless), Feedback-only additive request = 30 L/min. The unresolved baseline problem is: Downstream composition feedback cannot move additive flow because mixing lag has not yet produced a measured ratio error. In Control Studio Monitor retain the normal-flow boundary at 30.0 s and the restored boundary at 90.0 s.
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: Use ratio authority only with valid scaled wild-flow data above the reviewed minimum, and retain analyser PI as bounded trim rather than a substitute for meter validity.
   - **Successful evidence:** The Response plot advances and the displayed values are finite and current. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.

2. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Baseline check:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **90.1**. The reviewed authority is **Experiment authority** (`P1.ExperimentEnabled`). The observed signal is **Actual wild-stream flow**. It does not inject another flow step.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside Feedback-only additive request 0 L/min to 60 L/min; Ratio-controlled additive request 0 L/min to 60 L/min; Feedback-only blend ratio 0.1 (dimensionless) to 0.5 (dimensionless); Ratio-controlled blend ratio 0.1 (dimensionless) to 0.5 (dimensionless).
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore Actual wild-stream flow to 100 L/min, wait for current data, and review a new test.
   - **Record:** Compare immediate demand, downstream ratio, trim, and additive movement with the prediction. Retain the terminal state, restoration result, and deterministic seed **20260829**. The native lesson test owns meter-bias and fallback cases, metrics, and verdicts; disabled Identify, Tune, or Validate controls are not evidence.

## Evidence and decision

For feedback-only and ratio-plus-trim, hold plant equations, initial state, PI
tuning, physical flow event, sample time, duration, limits, seed, and measure
windows equal. For meter bias and fallback, restore first and change only the
declared measurement property.

Define the measures before viewing results:

- **Event-window blend-ratio integral absolute error** adds absolute ratio error
  multiplied by 0.1 seconds from 30 through 60 s. Because ratio is
  dimensionless, the unit is seconds. Lower is better.
- **Peak ratio deviation** is the largest absolute departure from 0.30 over the
  same event window. Lower is better.
- **Cumulative applied-additive-flow movement** is total variation: the sum of
  absolute changes between consecutive applied-flow samples. Its unit is
  L/min of accumulated movement, not an instantaneous valve command.
- **Final-window mean absolute ratio error** is the mean absolute error from 80
  through 90 seconds. It is eligible only when ratio span is at most 0.001 and
  applied-flow span at most 0.02 L/min over that window.

Also retain the first-scan ratio demand, final PI trim, saturation duration,
quality and low-flow classification, authority, abort, and restoration state.

Classify validity before ranking performance. Report `Not settled` instead of a
final-window value when its span rules fail. Report `Invalid` for low-flow or
bad-quality composition evidence.

For the nominal event, require a 36 L/min ratio demand in the first scan, event
error below 35 percent of feedback-only, lower peak deviation, and every
additive request within 0 through 60 L/min. Under +5 L/min meter bias, require
final mean absolute ratio error below 0.002 and trim near -1.5 L/min without
saturation. Invalid or below-minimum wild flow must remove ratio authority and
track additive request to zero.

The measured event-window IAE is 0.527 s for feedback-only and 0.000 s for
ratio-plus-trim, which holds the ratio through the entire event window; peak
ratio deviation falls from 0.034 to 0.000 over the same comparison. The
ratio-plus-trim first-scan demand is 36.00 L/min, exactly as predicted. Under
the +5 L/min meter-bias run, final mean absolute ratio error settles to
0.0002, comfortably inside the 0.002 acceptance limit, and final PI trim
measures -1.48 L/min, matching the predicted -1.5 L/min correction.

### Retained evidence

The plot is generated by the executable blending experiment. Compare how quickly feedback-only and ratio-plus-trim control respond when the wild-stream flow changes; the additive panel also plots the ratio demand that drives the ratio-plus-trim request, shown as a third legended series.

![Ratio-control comparison from the blending simulation](figures/blending-ratio-evidence.svg)

[Complete trace](figures/blending-ratio-evidence-trace.csv) · [Decision summary](figures/blending-ratio-evidence-summary.csv)

## Conclusion

Ratio demand acts immediately because the material relationship changes as
soon as measured production flow changes. Feedback-only waits for the changed
mixture to traverse the blender and analyser. PI trim removes modest residual
error, including meter bias, but cannot make missing or badly scaled flow data
trustworthy.

A multiplication can remain numerically finite at low flow while the physical
ratio measurement and dosing range are unusable. That is why minimum-flow
authority is explicit rather than left for PI tuning to handle.

The conclusion applies when the wild stream is valid, correctly scaled, and
independently establishes production rate. It does not apply when both streams
must be optimized together or when the desired quantity is a different mass or
molar fraction without the required density or concentration compensation.

## Reason for the next lesson

You can now configure and qualify ratio control without confusing feedforward,
feedback trim, or meter validity. Lesson 56 addresses a different problem:
temperature must cross an operating point where one controller coordinates
heating and cooling final elements rather than proportional material streams.
