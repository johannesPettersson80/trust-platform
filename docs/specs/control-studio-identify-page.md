# Control Studio — Identify page

User-directed UI specification, 13 September 2026.
Implementation and rendered acceptance are pending.

## Scope and authority

The automatic workflow has three pages: **Identify → Tune → Validate**.
Apply is an action on Validate, not a fourth page. Monitor and Test are not
separate pages in this automatic flow. This document specifies the Identify
page; it does not add requirements to the Tune or Validate page contents.

This document is the layout authority for Identify. The
[shared UI clarity contract](35-vscode-control-studio.md#shared-trust-uiux-contract--mandatory)
applies to every label, selector, field, result and interaction.
[Specification 35](35-vscode-control-studio.md) and the
[complete workflow specification](control-studio-complete-workflow-specification.md)
own identification algorithms, supported model families and runtime operations.
Reuse the shared truST components and theme.

## Single experiment and provisional model — 14 September 2026

One press of **Identify plant** runs one live acquisition campaign for the selected
method (including its required multichannel input intervals), then fits a model
from that recording. Do not automatically start a second independent recording.
Keep the measured experiment and its time origin visible when fitting completes.
Label the model **Provisional** compactly beside its model type. The same-record
fit is not independent validation and must not be stored as validated evidence.
The provisional model can be used in Tune, but cannot authorize Apply.

The user starts the independent model check explicitly with **Validate controller**
on Validate. The model coefficients remain frozen during that check.

## Exact page contents

Show these elements, in this order:

1. **Identify / Tune / Validate** navigation, with Identify selected.
2. **Plant inputs** signal selector.
3. **Measured outputs** signal selector.
4. **Method** selector, defaulting to **Automatic**.
5. **Identify plant**, **Open recording…** and **Reset** buttons on one
   row, with a small **Limits…** entry. Identify plant remains the primary action
   with the same label after a result. Use the Tune tab to change pages.
6. The identified **model type and plant-model equation**, once available, with
   one plain line of its key numbers and units (for example "Gain 0.21 °C per %,
   time constant 75 min, delay 12 s"). Durations are shown in s below 2 min, in
   min below 2 h and in h from 2 h, each with the engineering-number formatter of
   [Specification 35 B9.5](35-vscode-control-studio.md#b95-numbers).
7. The **response plot**. Its traces are named by role (Measurement, Output), with
   the symbol on hover; without data it shows one line instead of empty axes.

Simplification, 8 October 2026: the user asked for a design that serves both a
beginner control engineer and a professor ("do what you think is best, think from a
users experience"). The rules of [Specification 35 B9.9](35-vscode-control-studio.md#b99-simple-by-default-complete-one-click-away)
apply to this page and take precedence over earlier wording here.


Do not add other permanent controls, headings, descriptive paragraphs or
information panels to this page.

Use the [shared automatic flow](control-studio-complete-workflow-specification.md#three-page-automatic-flow)
for continuation, the small editor pattern, autosave and reopening.

## Signals

- Plant inputs are the signals actually driving the plant, such as applied
  heater power. Measured outputs are the plant responses, such as temperature.
- Both selectors allow one or multiple signals. Preselect available configured
  loop bindings and allow the user to change the selection.
- Show signal names and units in the selectors. Use a checkbox dropdown for
  multiple selections. Apply changes immediately, close on outside click, and
  provide no Done/Cancel buttons (user correction, 14 September 2026).
- A SISO model uses one input and one output. A MIMO model uses the selected
  multiple inputs and outputs. These are channel arrangements, not model types.
- Capture synchronized timing automatically; do not add a timestamp field.
- For an opened recording, these selectors choose signals from the recording.
- Use the configured setpoint binding when closed-loop identification needs
  one. Only if that binding is missing, show a **Setpoint** signal selector
  beneath Measured outputs. Omit it for methods that do not need it. Capture
  the applied plant input as well as setpoint and measured output; setpoint
  must not substitute for the actual plant input.

## Experiment limits

**Limits…** opens a compact editor containing the selected plant input ranges,
permitted measured-output ranges and maximum experiment duration. Display
signal names and units. Prefill the project's declared experiment limits;
existing actuator/rate constraints remain enforced.

If a required limit is missing, opening Identify alone does not interrupt the
user. Attempting identification opens this editor and identifies the missing
field before any excitation. Do not infer safe operating bounds from the full
numeric range of a PLC type. Automatic identification chooses its amplitude
and timing within these bounds; it must not enlarge them automatically.

The same bounds apply to all live acquisitions in the identification campaign.
An imported recording does not require permission to excite the plant; preserve
and evaluate its recorded conditions instead of applying new live-test limits
retroactively to its data.

## Identification controls

**Method** offers Automatic and the supported manual identification methods.
Automatic chooses the applicable identification method for the selected plant
and signals. A manual selection uses the method the user chose. This selector
is for the identification method, not a list of SISO/MIMO configurations.

**Identify plant** starts live identification using the selected signals and
method. Acquisition, fitting and model checking belong to that operation; do
not expose separate Record, Check recording, Fit or Accept model buttons. Those buttons belong to the automatic method only:
Recorded-data fit is the manual method, with steps of its own (spec 35, B3.3), and the engine names the one button the page shows
at each step. On a loop the engine serves, the page sends its form as the text the user entered, after a short pause in typing, and
shows the engine's stage, labels, gates and status; a refusal shows under the form field the engine names (B3.3).
Once the model qualifies, remain on Identify with its model and response plot
visible. The **Tune** tab navigates to Tune without starting tuning or opening
an editor. Press **Identify plant** again to run another identification with the
selected signals, method and limits. Do not rename it or add a continuation button.

**Reset** starts a new working identification study. Clear the displayed chart,
identified model, retained working acquisition results and dependent tuning and
validation results, including the saved working draft. Reopening must stay at
this fresh state. Keep the configured signals and experiment limits; do not
write controller parameters, change the plant, or erase separately saved files.
Require any running or unconfirmed operation to finish or be cancelled and
recovered before resetting its evidence. This supersedes the earlier display-only
Reset chart behavior, following the user's 14 September 2026 correction.
Selecting a method configures the next experiment without starting it. Pressing
Identify plant always executes a new experiment with that selection; previous
recordings must not substitute for this explicit live action.

**Open recording…** opens an existing recording for identification using the
same signal selection, method and result presentation. This path uses the
recorded data and must not start live excitation. Once its selected channels
are known and the model qualifies, remain on the same model review state.

While identification runs, replace the active identification action with
**Abort** and show one short progress message. If it fails, show one short,
specific failure message in the same area. These are temporary operation
states, not a permanent status panel or footer.

## Identified model and plot

Show the model type in plain text, followed by its equation with the actual
identified coefficients. For example:

**First-order model with delay**

\[
G(s)=\frac{0.2\,e^{-s}}{5s+1}
\]

The numbers above are illustrative. Display the model that was actually
identified, including the appropriate equations or matrices for a multichannel
or state-space result. Do not substitute a scalar example for a MIMO model.

The response plot shows the experiment's actual applied plant inputs together
with the measured outputs and, when available, model predictions. Use signal
names and units in the compact legend/axes. Keep different units on aligned
plots sharing the same time range, so the applied step and resulting response
can be read together. Show the recorded actuator behavior, including rate
limits; do not draw an ideal step in place of measured input data.

Identify operates on the connected running plant through the runtime's existing
experiment and recording services. In simulation mode, this is the running native
simulated plant connected to the actual ST controller through ordinary I/O.
Plot real incoming samples continuously before and during acquisition. The chart
must grow as the experiment happens; an isolated accelerated batch run followed
by a complete graph is not this live acquisition path. Overlay the fitted model
only after its required measured data is available. Keep the recorded input and
output synchronized and retain their actual timestamps and channel identities.
An imported recording remains a separate explicit data source.

Reset clears the old study and displayed history, then immediately resumes live
observation of the selected signals. It must not freeze an empty recorded view.
These requirements follow the user's explicit live-data correction on 14 September
2026 and supersede the earlier batch-return limitation.

Before identification produces a result, show the selected plant inputs and
measured outputs from the existing runtime observation stream, with current
values and units in the plot legend. Keep the axes visible while waiting for
readings; unavailable or stale readings must not appear as current values.
Once identification completes, show the retained measured/model comparison.

Before identification produces a result, omit the model name and equation.
Do not fill that space with placeholder reports or explanatory paragraphs.

Reopening a study shows its identification again, as the engine answers it (A8.6):
the model type, equation and key numbers, the response plot with the applied input,
and the requirements the model is tuned to. Reopening starts no experiment.

## Layout

Example after identification:

```text
 Identify                 Tune                 Validate
 ━━━━━━━━

 Plant inputs        [ Heater power (%)             ▾ ]
 Measured outputs    [ Temperature (°C)             ▾ ]

 Method              [ Automatic                   ▾ ]

 [ Identify plant ] [ Open recording… ] [ Reset ] Limits…


 First-order model with delay

                 0.2 · e⁻ˢ
         G(s) = ───────────
                   5s + 1


 Temperature (°C)                  ── Measured  ╌╌ Model
 ┌─────────────────────────────────────────────────────┐
 │                                                     │
 │                                ──────────────────   │
 │                         ───────                     │
 │                    ─────                            │
 │                ────                                 │
 │ ───────────────                                     │
 │                                                     │
 └─────────────────────────────────────────────────────┘
                         Time (s)
```

There is **no bottom status strip**. In particular, do not add Simulation,
Live, Running, runtime identity, scan counters, connection badges or a separate
SP/PV/MV value strip. Do not add qualification cards, sample-count summaries,
signal/noise scores, revision bookkeeping, Help/Details panels or a plot-action
toolbar to this agreed layout.

## Documentation checkpoint

- [x] **Start:** read the purpose contract and owning Control Studio specs in
  `trust-platform-connected-workflow`, branch
  `feat/control-studio-connected-workflow`, HEAD
  `f5ba9d02bd66441a894865f10070a27ccefa5eac`. This advances part 3, Control
  Studio. The intended target is the selected real project and its configured
  plant; the primary PID journey continues to use the existing `Control.PID`.
- [x] **Finish:** record the user's agreed layout and exclusions, and check the
  document's references and consistency. This is documentation only: no
  controller, plant or UI was executed. Implementation and real rendered review
  remain outstanding under the [purpose contract](control-platform-purpose.md).
