# Control Studio

Control Studio is the VS Code workspace for monitoring, testing, identifying,
tuning, and validating a control loop. It uses the same workflow for a
simulated plant and an approved physical plant, while always showing which
source produced the evidence.

Use [Simulation](simulation.md) to create and connect a simulated plant first.
Select target **Simulator**, choose **Run**, then open **Control Studio** from
the truST sidebar. A configured plant's contextual action opens the same
editor.

## Monitor before changing anything

Start in **Monitor**. Confirm these items before running a test:

- source: Simulation or Physical plant;
- runtime connection and data freshness;
- controller mode and active revision;
- SP, PV, and MV with their engineering units;
- output and rate limits, saturation, and signal quality.

The main plot remains visible throughout the workflow. Use its inspect, zoom,
pan, reset, PNG, CSV, and table actions when you need more detail. Unavailable
or stale data is shown as unavailable; it is not replaced with a plausible
value.

## Run a bounded test

1. Open **Test** and enter the requested amplitude and duration. A
   project-scripted lesson instead locks **Experiment** to **Observe without
   excitation** with **Amplitude** fixed at `0` and disabled: the runnable
   project owns the setpoint, disturbance, and timing, not Control Studio.
2. Choose **Review test**.
3. Check the affected signal, start and requested values, allowed range,
   timeout, stop conditions, and restoration value. For a project-scripted
   lesson the affected signal is its Boolean experiment-authority signal (for
   example `P1.ExperimentEnabled`), not a new excitation.
4. Choose **Confirm and start** only when the displayed plan is correct. For a
   project-scripted lesson this is the authority signal's false-to-true
   transition — the single write that grants the runnable project permission
   to run its own scripted sequence.
5. Watch SP, PV, MV, limits, and the plot while the test runs.
6. Choose **Abort test** whenever the response is unexpected.
7. Confirm the stop reason and restoration result. Restoration returns the
   authority signal to false and every requested value to its declared safe
   state.

Reviewing a plan does not start it. Approval applies only to the displayed
plan, and the live loop does not disappear behind the test controls.

## Identify the plant

Open **Identify** after recording a suitable response. Choose the model family
and run identification. Review the model structure, process gain, time
constants or delay, fit error, uncertainty, and qualification result.

Identification needs enough excitation to distinguish the response from noise.
If it reports insufficient excitation, the recording remains available. Use
the suggested bounded increase in amplitude or duration rather than accepting
an unreliable model.

## Generate a controller candidate

Open **Tune**, select the method and response objective, then generate a
candidate. The standard workflow offers a small objective choice such as
**More robust**, **Balanced**, or **Faster**. Exact Control.PID settings remain
available under advanced controls.

Compare the current and candidate time response, control effort, Bode
magnitude, Bode phase, gain margin, and phase margin. The candidate remains
unapplied.

Values are formatted for the decision being made. For example, a live value
might appear as `4.38 °C`, a model gain as `1.32372`, and a stability margin as
`19.08 dB`. The UI keeps useful significant digits without displaying raw
floating-point noise.

## Validate before applying

Open **Validate** and run the required scenarios. Review at least:

- settling time, overshoot, and steady-state error;
- MV excursion, rate, and saturation recovery;
- noise response;
- gain and phase margins; and
- stability across the declared bounded model variation.

Every requirement reports its status, value, limit, unit, evidence source, and
reason. Validation does not apply gains.

Choose **Review application** only after accepting the evidence. Verify the
controller structure, dimensioned gains, limits, and revision, then confirm
the separate apply action. Observe the bounded post-apply period. Use
**Rollback** to restore the exact previous revision if the result is not
acceptable.

## Use Automatic Tune

**Automatic Tune** sequences data qualification, Test, Identify, Tune, and
Validate:

1. Choose an objective.
2. Review the evidence source, bounded experiment, constraints, stop
   conditions, and restoration behavior.
3. Approve the exact plan.
4. Follow the visible stage progress.
5. Review the selected P, PI, PD, or PID structure and validation result.

The completed result is still unapplied. Automatic Tune never silently changes
the running controller.

Open **Method comparison** to inspect the qualified alternatives. Control
Studio retains SIMC, AMIGO, and bounded P/PI/PD/PID comparison attempts with
their structure, feasibility, score, limiting reason, and SIMC closed-loop time
(`lambda`). Every candidate is evaluated with the running loop's same units,
setpoint weights, bias, output and rate limits, anti-windup mode, model,
scenarios, and objective. Unsupported methods remain visible as unavailable;
failed candidates remain visible as rejected.

The response choice changes SIMC's declared robustness-speed parameter and the
common ranking objective. It does not modify the published AMIGO coefficients.
There is no universal winner: choose only a candidate that passes the required
stability, uncertainty, noise, and actuator constraints for the current loop.

## Recover from a failed workflow

Control Studio keeps failures attached to the stage that owns them:

- invalid configuration identifies the missing or conflicting field;
- identification failure explains whether excitation or data quality was
  insufficient;
- tuning failure retains the accepted model and suggests a more robust
  objective or revised limits;
- disconnection marks live data stale or unavailable; and
- restoration failure remains visible until the runtime confirms recovery.

Still-valid recordings, models, and unapplied candidates are retained. Do not
treat retained work as new live evidence after a disconnect; reconnect and
qualify current data first.

## Physical-plant boundary

A physical experiment requires an independently reviewed topology and a new,
immediate approval of the exact displayed action. Verify the signal, bounds,
timeout, stop conditions, and restoration value immediately before approval.

Simulation results are not physical-plant evidence. Revalidate limits,
failure behavior, safety functions, and controller performance on the reviewed
physical topology before commissioning.

## Related

- [Control System Engineering](../develop/control-system-engineering.md)
- [Simulation](simulation.md)
- [Safety And Commissioning](safety-and-commissioning.md)
- [Runtime UI And Control](runtime-ui-and-control.md)
- [Live Values](live-values.md)
