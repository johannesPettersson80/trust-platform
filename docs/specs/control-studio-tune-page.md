# Control Studio — Tune page

User-directed UI specification, 13 September 2026.
Implementation and rendered acceptance are pending.

## Scope and authority

Tune is the second page of **Identify → Tune → Validate**. Controller
recommendation and selection belong on Tune, not on a separate page.

This document is the layout authority for Tune. The
[shared UI clarity contract](35-vscode-control-studio.md#shared-trust-uiux-contract--mandatory)
applies to every label, selector, field, result and interaction.
[Specification 35](35-vscode-control-studio.md) and the
[complete workflow specification](control-studio-complete-workflow-specification.md)
own controller compatibility, recommendation and tuning algorithms. Reuse the
shared truST components and theme, consistently with the
[Identify page](control-studio-identify-page.md).

## Exact page contents

Show these elements, in this order:

1. **Identify / Tune / Validate** navigation, with Tune selected.
2. **Controller** selector, identifying the actual ST instance or controller
   group and showing its recommended configuration by default. Show the small
   **Channels…** entry beside it only for multiple channels.
3. **Tuning method** selector, defaulting to **Automatic**.
4. **Tune controller** button, with a small **Requirements…** entry on that row.
5. The resulting **controller type and settings**, in plain text.
6. The **predicted response plot**.

Do not add other permanent controls, headings, descriptive paragraphs or
information panels to this page.

Use the [shared automatic flow](control-studio-complete-workflow-specification.md#three-page-automatic-flow)
for continuation, the small editor pattern, autosave and reopening.

## Control requirements

**Requirements…** opens one compact editor with **Requirement / Signal / Limit**
rows and units. Prefill the project's declared requirements. Typical rows are
maximum settling time, maximum overshoot and maximum steady-state error; show
the criteria applicable to this task rather than an unrelated catalogue.
Existing declared disturbance, actuator, rate and state constraints remain part
of the same requirement set and must not disappear when editing these rows.

Recommendation, Tune and Validate use this one saved requirement set. For
multiple outputs, identify the channel on each row. Use the same names, values
and units in Validate's results table; do not offer a second limits editor there.
Where a criterion depends on a tolerance band or measurement window, keep that
value in the same row's edit form instead of guessing how to interpret the limit.

On **Tune controller**, automatically fill missing test settings from the selected
model, its identification recording and the configured operating/actuator limits.
Do not open an empty Requirements editor. Preserve every project-declared or
manually entered value, the accepted controller and pinned settings. Calculate
per-output settings for explicit multichannel mappings; never infer a mapping.

Generated values are **automatic test criteria**, not inferred production
requirements. In Requirements, label generated rows with a compact “Automatic”
marker; retain the ordinary editable values. In Validate, identify generated
error/time checks as automatic, alongside the unchanged declared requirements.
No new permanent panel, selector, confirmation or navigation step is introduced.

Use a conservative model timescale, resolved delay and sample period to choose
a settling target. Choose the test change within the declared operating range,
actuator headroom and identified operating region; account for finite slew
rates. Use available baseline noise to avoid an unmeasurable test and define a
relative settling/error tolerance. Noise estimates describe the recording, not
sensor accuracy or process acceptance. Choose sufficient observation time after
the deadline within runtime duration/sample limits. If essential model, mapping
or operating-limit information is missing, identify that specific prerequisite.
Never widen a configured limit or weaken an existing requirement to make a pass.

Store the values and their automatic origin in the shared study. Freeze them
before Validate. Retuning after model/limit changes recalculates only generated
values; edited values remain fixed. Validation never adjusts its own criteria
after observing the response. All generated checks and every declared mandatory
requirement must pass before the existing explicit Apply action becomes eligible.

The initial automatic policy uses the shared modal analysis, delay and sample
period for its timescale. The settling deadline includes six times that
scale, plant delay and a slew allowance; the observation duration extends beyond
it. Test changes use at most ten percent of the available directional range,
including the recorded region where available. A qualified baseline supplies a
three-standard-deviation noise estimate; changes smaller than twenty times that
estimate require review. Default bands/errors use two/four percent of the test
change with that noise floor. Generated values use readable engineering precision
without enlarging a test step. These are initial test-design choices; independent
model and controller checks determine whether they work. Explicit values are
never rounded or relaxed. Runtime caps remain600s and12000samples, with the
existing channel-sample budget enforced by the verification planner.

The PID paper (*A Practical Guide to PID Controller Implementation*, v3, §1)
explicitly excludes tuning from its scope. Its §6 examples use Lambda PI with
lambda equal to the plant time constant; §§3.1,3.5,3.9 and5.1 emphasize actuator
limits, anti-windup, noise and coherent initialization/parameters. Automatic
setup is a truST engineering policy using the existing model/tuning services;
it changes neither the paper-based Control.PID equations nor its runtime state.

## Controller recommendation and selection

Use the identified plant model, the control requirements and the supported
controllers available in the project to recommend a compatible controller.
Read this context from the existing workflow; do not repeat signal selection
or model setup on Tune.

The **Controller** selector must identify the actual controller instance in the
loaded ST program, not just a controller family or library type. Use the
configured loop's instance when one is already selected; allow the user to
choose another compatible instance. Each choice must retain its exact program
instance identity, compiled type and declared parameter/state adapter.

When the project contains multiple PID instances, list each separately by its
full qualified ST instance name in this same selector. For example:

```text
Controller    [ Main.HeaterPID       ▾ ]
                Main.HeaterPID
                Main.PressurePID
                Main.FlowPID
```

Select the PID controlling the plant whose input/output signals were chosen
on Identify. Preselect the configured loop's bound instance when available.
Otherwise show **Select controller** and require the user to choose; never
guess from instance order, a similar name or the fact that its type is PID.
Check the selected instance against the loop's declared signal bindings.

When tuning an individual PID, Tune, Validate and Apply target that selected
instance. Other PID instances must not receive its parameters or state updates.
An explicitly selected multichannel group is the separate case below.

Show the instance and recommendation together in the existing **Controller**
selector when Tune opens. For example:

```text
Controller    [ Main.TemperaturePID · PI — recommended ▾ ]
```

Here `Main.TemperaturePID` is an illustrative instance of `Control.PID`; `PI`
is its recommended configuration. Dropdown choices must identify the instance
and compatible configuration, so changing a selection never leaves the write
target ambiguous. This uses the same selector, not an additional page.

Once the actual instance is selected, preselect its recommended configuration
unless the engineer has already chosen a configuration. A recommendation must
not resolve an ambiguous instance selection. Do not leave the selector showing
only Automatic when the instance and recommendation are available. Do not add
a separate recommendation card or page.

The user can choose a compatible alternative from this dropdown. Preserve an
existing explicit choice; automatic recommendation or tuning must not silently
replace it. Mark only the actual recommendation as recommended.

For the primary PID journey, use the existing **Control.PID**, configured as
P, PI, PD or PID. Do not replace it with a handwritten controller. Other
supported controller families retain their existing compatibility rules.

## Multichannel controller mapping

A MIMO plant may use one supported multivariable controller instance or an
explicit group of compatible PID instances. The **Controller** selector offers
the project's declared compatible groups as single entries, identified by
their actual member instances. Selecting several plant signals must not
silently select every PID in the project or treat them as interchangeable.

**Channels…** opens the compact mapping editor only for a multichannel target.
Prefill declared bindings. List the selected target's command and feedback
channels with the corresponding **Plant signal** selector, using the signals
chosen on Identify. For a PID group, show the owning PID's qualified instance
name on each row. This is the same mapping for tuning, validation and Apply.

Every selected plant input/output must have its required corresponding mapping.
Preserve declared coupling/decoupling and channel order. Missing, ambiguous or
incompatible mappings pause the flow and identify the affected row; never pair
channels by name similarity or silently discard a channel. A SISO loop does
not show Channels or a mapping table.

Validate the whole selected group together. Apply and restoration require a
supported coherent transaction for its complete parameter/state set; sequential
independent PID writes must not be presented as one successful group update.
If the group has no compatible application adapter, explain that in the action
area and leave Apply unavailable. Unselected controllers remain untouched.

## Tuning controls

**Tuning method** offers Automatic and the supported manual choices for the
selected controller. Automatic chooses the applicable tuning method. An
explicit method selection uses that method or reports why it cannot be used.

Carry the selected instance and its declared parameter/state mapping through
tuning, validation and Apply. The mapping identifies which controller settings
correspond to each tuned value; never guess writable variables from names such
as `Kp` or `PID`. If the mapping is absent or incompatible, report that in the
existing action area and leave application unavailable. Follow the existing
[controller binding contract](control-studio-controller-bindings.md).

**Tune controller** calculates settings for the selected controller and stops
here with its candidate and predicted response. The Validate tab only navigates;
**Validate controller** explicitly starts validation there. In manual
tuning, make the relevant controller settings editable in the same settings
area. Do not introduce a separate settings dashboard.

Preserve settings the engineer has explicitly pinned. Opening Tune, selecting
a controller or method, editing settings and calculating a result must not
change the running controller. **Apply** belongs on Validate.

While tuning runs, the action becomes **Abort**, with one short progress
message. If tuning fails or a prerequisite is missing, show one short, specific
message in that area. Do not turn this into a permanent status panel.

## Result and plot

Show the proposed controller type and its actual calculated settings. For
example:

```text
PI controller · Control.PID

Proportional gain (Kp) = 5
Integral gain (Ki) = 1
```

These values are illustrative. Display the declared parameter units alongside
the real values. Show the parameters relevant to the actual
selected controller; do not force every controller into a PID parameter form.
Before settings exist, omit the result rather than showing placeholder cards.

The plot is labelled **Predicted response**. Compare the current and tuned
controller under the same conditions, using a small **Tuned / Current** legend
and axes with signal units and time. Do not label predictions as measured
validation. For multiple outputs, keep each response associated with its
corresponding channel.

## Layout

Example after tuning:

```text
 Identify                 Tune                 Validate
                          ━━━━

 Controller          [ Main.TemperaturePID · PI — recommended ▾ ]
 Tuning method       [ Automatic                               ▾ ]

 [ Tune controller ]                         Requirements…


 PI controller · Control.PID

 Proportional gain (Kp) = 5
 Integral gain (Ki) = 1


 Predicted response                  ── Tuned  ╌╌ Current
 ┌─────────────────────────────────────────────────────┐
 │                         ╌╌╌╌                        │
 │                     ╌╌╌      ╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌   │
 │                  ────────────────────────────────   │
 │              ────                                   │
 │          ────                                       │
 │ ─────────                                           │
 │                                                     │
 └─────────────────────────────────────────────────────┘
                         Time (s)
```

There is **no bottom status strip**: no Simulation, Live, Running, runtime
identity, scan counters, connection badges or separate SP/PV/MV value strip.
Do not add recommendation reports, method-evidence tables, scorecards,
revision bookkeeping, Help/Details panels or a plot-action toolbar to this
agreed layout.

## Documentation checkpoint

- [x] **Start:** read the purpose contract and owning specifications in
  `trust-platform-connected-workflow`, branch
  `feat/control-studio-connected-workflow`, HEAD
  `f5ba9d02bd66441a894865f10070a27ccefa5eac`. This advances part 3, Control
  Studio, for the selected project and its configured plant. The primary PID
  target remains the existing `Control.PID`.
- [x] **Finish:** record the agreed Tune layout and controller recommendation
  placement; check references and consistency. This is documentation only:
  no controller, plant or UI was executed. Implementation and real rendered
  review remain outstanding under the [purpose contract](control-platform-purpose.md).
- [x] **Multiple-PID clarification — Start:** retain the same documentation-only
  scope and verified checkout; specify how the selected plant maps to one
  actual PID instance when the project contains several.
- [x] **Multiple-PID clarification — Finish:** record distinct instance names,
  bound-instance preselection, explicit choice when unbound and writes limited
  to that instance. Check the Tune/Validate wording and references; no runtime
  parameters changed and no UI or controller was executed.

## Explicit validation boundary — 14 September 2026

Tune may use a provisional model fitted from the Identify recording. It keeps
that qualification and the accepted controller/pinned settings. Tuning ends on
Tune with its predicted response; **Validate controller** on Validate starts the
independent model check and actual-ST controller campaign. No runtime parameter
application is authorized by a provisional fit or tuning result.
