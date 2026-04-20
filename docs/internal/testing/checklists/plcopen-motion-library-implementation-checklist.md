# PLCopen Motion Library Implementation Checklist (Spec-Mapped)

Date opened: 2026-04-11
Status: In Progress
Owner: Johannes + Codex
Primary spec:
- `docs/internal/references/PLCopenMotion/plcopen_motion_library_spec_for_truST_v0_1.md`

Primary evidence artifact:
- `docs/internal/testing/evidence/plcopen-motion-library-implementation-<date>.md`

Purpose:
- Implement the PLCopen motion library exactly as specified in the truST motion spec, with the classic PLCopen FB layer as the normative public contract.
- Keep the implementation tests-first, using truST's built-in ST unit-test system as the primary semantic lock for observable PLCopen behavior.
- Prevent drift between the motion spec, standard-library catalog, runtime kernel, diagnostics/hover/completion, fixture projects, and future user-facing docs.
- Keep phase claims honest: only mark a PLCopen phase supported when its FBs, types, semantics, and evidence are all complete.

Status legend:
- `Not Started`
- `In Progress`
- `Blocked`
- `Done`

Evidence rule:
- No item may be marked `[x]` without reproducible evidence in the linked evidence artifact.
- No phase may be marked complete until its tests-first items, implementation items, targeted verification items, and compliance-matrix updates are all complete.
- No "PLCopen Part 1/4/5 support" claim is allowed unless the per-FB compliance matrix and corresponding deterministic tests are green.

Execution cadence:
1. Add or extend ST library unit tests first with `TEST_PROGRAM` / `TEST_FUNCTION_BLOCK` and `ASSERT_*`.
2. Implement the minimum Structured Text library code needed to satisfy the failing tests.
3. Run only the targeted library tests for the active phase and record evidence.
4. Update spec-adjacent docs and the compliance matrix in the same change set.
5. Reserve `just fmt`, `just clippy`, `just test`, and `just test-all` for milestone and final gates only.

Scope rule:
- Harness-only discovery smoke tests, Rust-side motion scaffolding, and empty future-phase placeholder projects are not part of the motion-library implementation checklist and SHALL NOT block or precede the ST library work.

Library location rule:
- The reusable PLCopen motion library source of truth lives under `libraries/plcopen_motion/*`.
- The fixture projects under `crates/trust-runtime/tests/fixtures/plcopen_motion/*` are conformance consumers/tests only.

Current acceptance note:
- After extracting the library source into `libraries/plcopen_motion/*`, the targeted motion suites were rerun against the package-based layout.
- Full-workspace gates remain a separate final acceptance pass and are not claimed complete in this targeted-only relocation pass.

Runtime base note:
- Runtime prerequisites for Phase A1+ landed via `feature/codesys-twincat-parity` (file-scope `VAR_GLOBAL`, `NAMESPACE`-scoped `VAR_GLOBAL`, `PROGRAM`-scoped `VAR_GLOBAL` acceptance, bare-global-access deviation, `VAR_STAT`, and the `TEST_PROGRAM` + `CONFIGURATION` test-mode fix). ST-only motion work proceeds on that base, and the current library packages use file-scope `VAR_GLOBAL` carriers in `libraries/plcopen_motion/*/src/*_globals.st` without exposing those carriers as public PLCopen surface.

## Global Execution Rules

- [x] `GR-01` ST conformance tests exist before implementation lands for every new public FB, type, enum, or semantics change where the behavior is externally observable. Spec: Sections `1`, `4`, `28`, `30`.
- [x] `GR-02` The PLCopen PDFs plus the truST motion spec remain the only normative behavior sources; no vendor implementation is treated as authoritative. Spec: Sections `2`, `3`, `4`.
- [x] `GR-03` The public API remains FB-first; optional OO work may not block or redefine Phases A-D. Spec: Sections `3`, `19.6`, `29`, `31`.
- [x] `GR-04` Classic public FB contracts use `ErrorID : WORD`; the optional OO facade may use `MC_ERROR`, but `MC_ERROR` does not replace classic `ErrorID : WORD` in the FB surface. Spec: Sections `7`, `12`, `29`.
- [x] `GR-05` Every PLCopen ambiguity resolved during motion implementation is recorded in `docs/PLCOPEN_DECISIONS.md`, and every truST-specific PLCopen behavior choice or extension is recorded in `docs/PLCOPEN_DEVIATIONS.md`. Spec: Sections `2`, `6`, `10`, `12`.
- [x] `GR-06` Implementation stays SOLID, KISS, and DRY: one shared axis command lifecycle, one shared group command lifecycle, one queue model, and no divergent copy-pasted per-FB queue/state machines. Spec: Sections `20`, `21`, `30`.
- [x] `GR-07` The compliance matrix stays current with each landed type/FB and uses the exact spec-defined columns and status set: `PublicName`, `SourcePart`, `SourceClass`, `truSTPhase`, `truSTStatus`, `Notes`, with `truSTStatus` in `{Required, Implemented, Deferred, Optional, NotTargeted}`. Optional columns such as `PinnedSourceRevision` are allowed where the spec calls for them. Spec: Section `4`.
- [x] `GR-07A` Deferred public names must have their chosen absent-vs-placeholder path recorded in the compliance matrix before the corresponding negative tests are written. Spec: Section `4`.
- [x] `GR-08` The exact PLCopen source revisions remain pinned until the spec is intentionally updated: Part 1 v2.0 published 2011-03-17, Part 4 v2.0 RFC dated 2025-11-18, and Part 5 v2.0 published 2011-11-16. Spec: Section `2`.
- [x] `GR-09` Full-workspace gates are not used as a substitute for phase-local semantic tests; targeted tests must fail first and pass before broader gates run. Spec: Sections `28`, `30`.
- [x] `GR-10` No claim is made for Part 6 fluid power or the OO facade until those scopes are explicitly implemented, tested, and documented. Spec: Sections `19.6`, `19.7`, `29`.

## Phase A0: Public Catalog, Types, Enums, And Classic Error Surface

Status: Done

Tooling follow-ups `A0-06` and `A0-10` are intentionally recorded as post-library IDE/LSP follow-up scope; they do not block the ST-only motion-library definition of done.

### Tests First

- [x] `A0-01` Add semantic tests proving all required Part 1 single-axis FB names are discoverable in ST source and resolve as callable standard-library symbols. Spec: Sections `7`, `8`, `19.1`.
- [x] `A0-02` Add semantic tests proving the single-axis public enum set and required members resolve exactly as specified, including `MC_BUFFER_MODE`, `MC_DIRECTION`, `MC_EXECUTION_MODE`, `MC_SOURCE`, and the documented truST extension `MC_AXIS_STATUS`. Coordinated-motion enum families are locked in Phase C. Spec: Section `8`.
- [x] `A0-03` Add semantic tests proving the single-axis public type names resolve exactly as specified, including `AXIS_REF` and any other single-axis-only public reference types. Coordinated-motion reference types are locked in Phase C. Spec: Sections `7`, `8`.
- [x] `A0-03A` Add semantic tests proving the `AXIS_REF` public field names are stable and resolve exactly as specified. Spec: Section `7`.
- [x] `A0-04` Add negative tests proving the classic FB surface does not expose `MC_ERROR` in place of `WORD` `ErrorID`, and does not define invented public types such as `MC_PAYLOAD_REF`. Spec: Sections `7`, `12`, `26`, `27`.
- [x] `A0-05` Add tests proving the standard parameter-number constants map to the exact Part 1 parameter IDs defined in the spec. Spec: Sections `8`, `19.1`, `28.1`.
- [x] `A0-05A` Add semantic tests proving every mandatory `mcERR_*` public constant resolves as a stable `WORD` value and does not overlap another mandatory public error constant. Spec: Section `12`.
- [x] `A0-05B` Record the initial single-axis motion-profile decisions in `docs/PLCOPEN_DECISIONS.md` before behavior-lock tests expand further, including grouped-axis rejection, zero-dynamic fallback, `MC_Reset` outside `ErrorStop`, vendor BOOL parameters, and the initial `MC_Power` / `MC_SetOverride` profile choices. Spec: Sections `8`, `9`, `10`, `12`, `15`.
- [x] `A0-06` [Tooling follow-up] Record the future IDE/LSP hover, completion, and signature-help coverage scope for representative single-axis symbols such as `MC_Power`, `MC_MoveAbsolute`, `MC_ReadStatus`, `AXIS_REF`, and `MC_BUFFER_MODE`, without making it a blocker for the ST-only library shipment. Spec: Sections `7`, `8`, `19.1`.
- [x] `A0-07` Add negative semantic tests proving unsupported legacy or mismatched public names such as `mcTransitionVelZero`, `mcTransitionVelNext`, and bare diagram labels such as `GroupStandby` are not accepted as ST public literals. Note in the evidence/docstring that the `mcTransitionVel*` names are plausible user mistakes because Part 4 prose and tables are inconsistent there; truST locks the canonical public names to the table-defined `mcTV*` literals. Spec: Sections `8`, `17`.

### Implementation

- [x] `A0-08` Add the Structured Text motion-library definitions for all required single-axis FBs, public types, enums, and parameter/error constants required by the spec. Spec: Sections `7`, `8`, `19.1`.
- [x] `A0-09` Implement the classic public FB surface so every classic motion FB exposes `ErrorID : WORD` and not an OO-only error type. Spec: Sections `7`, `12`.
- [x] `A0-10` [Tooling follow-up] Record the future hover/docstring scope for the primary single-axis FBs and types, including the `MC_Constants()` call-before-read idiom, without making it a blocker for the ST-only library shipment. Spec: Sections `2`, `3`, `7`, `8`.
- [x] `A0-11` Keep the compliance-matrix rows and the current public ST motion surface aligned exactly so the landed single-axis names and documented scope do not drift. Spec: Sections `3`, `4`, `30`.

### Targeted Verification

- [x] `A0-12` Run focused single-axis ST library surface tests covering only the new public motion symbols and record exact commands/results. Spec: Sections `7`, `8`, `19.1`.
- [x] `A0-13` Run the single-axis library project in normal mode to prove symbol resolution and test execution work end to end. Spec: Sections `19.1`, `28`.

## Phase A1: Axis Kernel Contract And Axis State Engine

Status: Done

### Tests First

- [x] `A1-01` Add ST tests locking the classic axis state set and state transitions: `ErrorStop`, `Disabled`, `Stopping`, `Homing`, `Standstill`, `DiscreteMotion`, `ContinuousMotion`, and `SynchronizedMotion`. Spec: Sections `8`, `13`, `28.1`.
- [x] `A1-02` Add tests proving `MC_Stop` holds the axis in `Stopping` while `Execute = TRUE` and only permits `Standstill` after `Execute` falls. Spec: Sections `11`, `13`, `28.1`.
- [x] `A1-03` Add tests proving `MC_Reset` follows the two-path reset rule: `ErrorStop -> Disabled` when power is off, and `ErrorStop -> Standstill` when power is enabled and active. Spec: Sections `13`, `19.1`, `28.1`.
- [x] `A1-03A` Add tests proving `MC_Reset` outside `ErrorStop` follows the documented deterministic no-op policy and does not silently change axis state. Spec: Sections `12`, `13`, `19.1`, `28.1`.
- [x] `A1-04` Add tests proving implemented no-state-change FBs do not silently move the axis state machine. Spec: Sections `13`, `19.1`, `28.1`.
- [x] `A1-05` Add fault-injection tests proving the axis state engine and the classic `Error`, `ErrorID`, and `CommandAborted` outputs stay coherent when backend errors occur mid-command. Spec: Sections `11`, `12`, `13`, `28.1`.
- [x] `A1-06` Add tests proving the chosen grouped-axis restriction rejects single-axis motion commands on grouped axes. Spec: Section `15`.
- [x] `A1-06A` Add positive tests proving read-only administrative FBs remain legal on grouped axes, including representative readback and parameter-read cases. Spec: Section `15`.

### Implementation

- [x] `A1-07` Implement the axis kernel contract described by the motion spec with one authoritative state engine used by all Phase A FBs. Spec: Sections `20`, `30`.
- [x] `A1-08` Implement deterministic mapping from backend status/error inputs into classic PLCopen axis states and `WORD` error codes. Spec: Sections `12`, `13`, `20`.
- [x] `A1-09` Ensure the axis kernel exposes enough internal observability for conformance tests without leaking non-standard public API. Spec: Sections `6`, `20`, `28`.
- [x] `A1-09A` Verify the earlier grouped-axis decision-log entry still matches the implemented axis-kernel behavior and public `mcERR_AxisGrouped` naming before Phase A support is claimed. Spec: Sections `12`, `15`.

### Targeted Verification

- [x] `A1-10` Run only the new axis-state conformance tests and record exact evidence. Spec: Sections `13`, `20`, `28`.

## Phase A2: Power, Reset, Status, Parameter Plane, And Administrative Readback

Status: Done

### Tests First

- [x] `A2-01` Add ST tests for `MC_Power` proving `Enable`, `Status`, `Valid`, `Error`, and `ErrorID` behavior matches the chosen classic contract, and that `Status` is not treated as a generic `Valid` alias. Spec: Sections `11`, `19.1`, `28.1`.
- [x] `A2-01A` Add semantic/profile-scope tests proving the initial single-axis `MC_Power` signature does not expose `EnablePositive` / `EnableNegative` unless a later profile explicitly enables them in both the compliance matrix and the public catalog. Spec: Sections `4`, `19.1`, `28.1`.
- [x] `A2-01B` Add ST tests proving `MC_Power.Status` is not reset merely by the falling edge of `Enable` and instead continues to reflect the effective power-stage state until that state actually changes. Spec: Sections `11`, `19.1`, `28.1`.
- [x] `A2-02` Add ST tests for `MC_ReadStatus` proving all eight boolean state outputs reflect the axis state model consistently. Spec: Sections `11`, `13`, `19.1`.
- [x] `A2-03` Add ST tests for `MC_ReadMotionState` proving state readback remains coherent across idle, moving, stopping, homing, and faulted conditions. Spec: Sections `13`, `19.1`.
- [x] `A2-03A` Add ST tests for `MC_ReadMotionState` proving `Source : MC_SOURCE` selects the documented commanded/set/actual value source. Spec: Sections `8`, `19.1`, `28.1`.
- [x] `A2-03B` Add ST tests for `MC_ReadMotionState` proving `ConstantVelocity`, `Accelerating`, `Decelerating`, `DirectionPositive`, and `DirectionNegative` behave as documented under scripted motion scenarios. Spec: Sections `11`, `19.1`, `28.1`.
- [x] `A2-04` Add ST tests for `MC_ReadAxisInfo` proving the nine documented Part 1 outputs `HomeAbsSwitch`, `LimitSwitchPos`, `LimitSwitchNeg`, `Simulation`, `CommunicationReady`, `ReadyForPowerOn`, `PowerOn`, `IsHomed`, and `AxisWarning` reflect their underlying axis signals correctly. Spec: Sections `19.1`, `28.1`.
- [x] `A2-05` Add ST tests for `MC_ReadAxisError` proving axis-level errors remain readable separately from the per-FB `ErrorID` of the current command instance. Spec: Sections `12`, `19.1`.
- [x] `A2-06` Add ST tests for `MC_SetPosition` proving state effects, readback behavior, and commanded-vs-actual consequences are deterministic and documented. Spec: Sections `13`, `19.1`, `28.1`.
- [x] `A2-07` Add ST tests for `MC_SetOverride` proving enable-style semantics, `Enabled` behavior, standard `0.0 .. 1.0` factor handling, rejection of negative values, rejection of `AccFactor = 0.0` and `JerkFactor = 0.0`, the special `VelFactor = 0.0` stop-without-`Standstill` behavior, and the documented truST behavior for vendor-specific values above `1.0`. Spec: Sections `9`, `19.1`.
- [x] `A2-08` Add ST tests for `MC_Reset` at the FB-output level in addition to the axis-state tests from `A1`. Spec: Sections `12`, `13`, `19.1`.
- [x] `A2-09` Add ST tests for `MC_ReadActualPosition`, `MC_ReadActualVelocity`, and `MC_ReadActualTorque` proving the actual-value readbacks are stable across idle, move, stop, and fault conditions. Spec: Sections `9`, `19.1`.
- [x] `A2-10` Add ST tests for `MC_ReadParameter` and `MC_WriteParameter` covering all standardized parameter IDs, invalid-ID rejection, and representative read/write cases. Spec: Sections `8`, `12`, `19.1`, `28.1`.
- [x] `A2-11` Add ST tests for `MC_ReadBoolParameter` and `MC_WriteBoolParameter` proving `PN = 4`, `PN = 5`, and `PN = 6` are the accepted standardized BOOL parameter numbers, that numeric-only standardized parameter numbers are rejected explicitly, and that any declared vendor-specific BOOL parameters are accepted consistently when the profile exposes them. Spec: Sections `8`, `12`, `19.1`, `28.1`.
- [x] `A2-12` Add ST tests proving numeric parameter FBs reject BOOL-only parameter numbers, or return the exact documented PLCopen-compatible error path if the implementation chooses that behavior. Spec: Sections `8`, `12`, `19.1`, `28.1`.
- [x] `A2-13` Add ST tests for `MC_WriteParameter` and `MC_WriteBoolParameter` covering `ExecutionMode = mcImmediately` and `ExecutionMode = mcQueued`. Spec: Sections `10`, `19.1`, `28.1`.
- [x] `A2-13A` Add negative ST tests proving single-axis FBs in the initial Phase A profile reject `ExecutionMode = mcDelayed` with `mcERR_NotSupported`. Spec: Sections `8`, `10`, `19.1`, `28.1`.
- [x] `A2-14` Add parameter-plane tests proving software-limit values and enable flags are readable, writable, and exposed through one authoritative axis parameter service before any motion FB consumes them. Spec: Sections `8`, `19.1`, `20`.
- [x] `A2-15` Add tests proving unit scaling and commanded-vs-actual value semantics remain internally consistent across readback and parameter access. Spec: Sections `8`, `9`, `19.1`.

### Implementation

- [x] `A2-16` Implement `MC_Power`, `MC_Reset`, `MC_ReadStatus`, `MC_ReadMotionState`, `MC_ReadAxisInfo`, `MC_ReadAxisError`, `MC_SetPosition`, and `MC_SetOverride` on top of the shared axis kernel. Spec: Sections `13`, `19.1`, `20`.
- [x] `A2-17` Implement `MC_ReadActualPosition`, `MC_ReadActualVelocity`, `MC_ReadActualTorque`, `MC_ReadParameter`, `MC_WriteParameter`, `MC_ReadBoolParameter`, and `MC_WriteBoolParameter` on top of one authoritative axis parameter/value service. Spec: Sections `8`, `9`, `19.1`, `20`.
- [x] `A2-18` Ensure parameter read/write validation, `ExecutionMode`, and error mapping reuse one shared path rather than bespoke per-FB logic. Spec: Sections `10`, `12`, `20`, `30`.
- [x] `A2-19` Ensure all single-axis administrative FBs use consistent execute/enable edge handling, output latching, and error mapping. Spec: Sections `10`, `11`, `12`, `20`.

### Targeted Verification

- [x] `A2-20` Run only the single-axis administrative, readback, and parameter-plane fixture tests. Spec: Sections `19.1`, `28`.

## Phase A3: Queue, Buffering, Execute Semantics, And Motion Prerequisites

Status: Done

### Tests First

- [x] `A3-01` Add tests proving execute-edge latching for execute-style FBs and stable enable semantics for enable-style FBs. Spec: Sections `10`, `28.1`.
- [x] `A3-02` Add tests proving missing inputs reuse the previous invocation's value and that first invocation falls back to the IEC initial value. Spec: Sections `10`, `28.1`.
- [x] `A3-03` Add tests proving `Busy`, `Done`, `Error`, and `CommandAborted` follow the required exclusivity rules. `Active` and its special cases are tested separately. Spec: Sections `11`, `28.1`.
- [x] `A3-03A` Add tests proving `Active`, `Done`, `Error`, and `CommandAborted` are mutually exclusive for non-`MC_Stop` execute-style FBs. Spec: Sections `11`, `28.1`.
- [x] `A3-03B` Add tests proving `Valid` and `Error` are mutually exclusive on enable-style FBs. Spec: Sections `11`, `28.1`.
- [x] `A3-04` Add tests proving `Active` only becomes true when a buffered command actually takes ownership, not merely when it is queued. Spec: Sections `11`, `16`, `28.1`.
- [x] `A3-05` Add tests proving queued buffered commands are ordered deterministically and are aborted or errored correctly when the active motion faults. Spec: Sections `12`, `16`, `28.1`.
- [x] `A3-06` Add tests proving queued buffered commands are cleaned up when the axis enters `ErrorStop`. Spec: Sections `12`, `16`, `28.1`.
- [x] `A3-07` Add tests proving `ContinuousUpdate` latches the initial command on the rising edge of `Execute`. Spec: Sections `10`, `28.1`.
- [x] `A3-08` Add tests proving changed inputs are applied only while `ContinuousUpdate = TRUE` and the FB is still `Busy`. Spec: Sections `10`, `28.1`.
- [x] `A3-09` Add tests proving relative distances under `ContinuousUpdate` remain referenced to the original command-start condition. Spec: Sections `10`, `28.1`.
- [x] `A3-10` Add tests proving `ContinuousUpdate` is not a retrigger of `Execute`. Spec: Sections `10`, `28.1`.
- [x] `A3-11` Add tests proving `ContinuousUpdate` changes are ignored while a buffered command is still queued and become live only once the command is active. Spec: Sections `10`, `16`, `28.1`.
- [x] `A3-12` Add tests proving queued commands retain their requested buffer-mode metadata deterministically and that unsupported queue/buffer combinations are rejected by the shared queue layer before execution begins. Spec: Sections `16`, `19.1`, `28.1`.

### Implementation

- [x] `A3-13` Implement one shared axis command queue and ownership-transfer model for all single-axis execute-style FBs and queued administrative writes. Spec: Sections `11`, `16`, `20`.
- [x] `A3-14` Implement shared lifecycle/output cleanup helpers so latching, abort, buffered activation, and queue cleanup behavior is consistent across the library. Spec: Sections `10`, `11`, `12`, `16`, `20`.
- [x] `A3-15` Ensure the parameter-plane `ExecutionMode = mcQueued` semantics reuse the same queue and ownership-transfer model as motion FBs. Spec: Sections `10`, `16`, `20`.

### Targeted Verification

- [x] `A3-16` Run only the execute, queue, buffering, and `ContinuousUpdate` semantic tests and record evidence separately from planner-specific motion tests. Spec: Sections `10`, `11`, `16`, `28`.

## Phase A4: Core Motion FBs

Status: Done

### Tests First

- [x] `A4-01` Add ST tests for `MC_Home` proving the actual Part 1 v2.0 signature and behavior: `Execute`, `Position`, `BufferMode`, `Busy/Active/Done/Error/CommandAborted`, buffered ownership semantics, the `Standstill -> Homing -> Standstill` path, and at least one documented non-standstill entry-state case. Spec: Sections `11`, `13`, `28.1`.
- [x] `A4-02` Add ST tests for `MC_Stop` proving the classic non-bufferable stop behavior, output latching, and that the selected truST public signature does not expose `Active`. Spec: Sections `11`, `16`, `19.1`, `28.1`.
- [x] `A4-03` Add ST tests for `MC_Halt` proving the halt semantics differ from stop semantics, move the axis through `DiscreteMotion`, and complete in `Standstill` instead of adopting `MC_Stop`'s blocking behavior. Spec: Sections `13`, `16`, `19.1`.
- [x] `A4-04` Add ST tests for `MC_MoveAbsolute` proving execute-edge capture, parameter use, `Busy/Active/Done` behavior, and correct final position semantics. Spec: Sections `10`, `11`, `19.1`, `28.1`.
- [x] `A4-05` Add ST tests for `MC_MoveRelative` and `MC_MoveAdditive` proving the difference between command-start-relative distance and commanded-position-additive distance. Spec: Sections `19.1`, `28.1`.
- [x] `A4-06` Add ST tests for `MC_MoveVelocity` proving `InVelocity` behavior, direction handling, and steady-state output semantics. Spec: Sections `11`, `19.1`, `28.1`.
- [x] `A4-07` Add ST tests for `MC_MoveContinuousAbsolute` and `MC_MoveContinuousRelative` proving continuous-update behavior, `InEndVelocity` setpoint-equality behavior, reset of `InEndVelocity` on `CommandAborted`, termination behavior, and state assignment into continuous motion. Spec: Sections `10`, `11`, `13`, `19.1`.
- [x] `A4-08` Add negative tests for sign rules proving velocity, position, and distance may be signed while acceleration, deceleration, and jerk remain positive-input quantities. Spec: Sections `9`, `10`, `28.1`.
- [x] `A4-09` Add tests for the zero-acceleration, zero-deceleration, and zero-jerk policy proving the implementation uses the configured axis maxima rather than inventing ad hoc behavior. Spec: Sections `10`, `28.1`.
- [x] `A4-09A` Add tests proving software-limit parameters and enable flags affect command acceptance and motion clamping exactly as documented once the motion FBs consume the parameter plane. Spec: Sections `8`, `19.1`, `20`.
- [x] `A4-09B` Add tests proving the minimum single-axis buffer support rules from spec `16.4` are enforced by FB family and reflected explicitly in the compliance matrix. Spec: Sections `16`, `19.1`, `28.1`.

### Implementation

- [x] `A4-10` Implement `MC_Home`, `MC_Stop`, `MC_Halt`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveVelocity`, `MC_MoveContinuousAbsolute`, and `MC_MoveContinuousRelative` using the shared axis lifecycle, queue, and planner hooks. Spec: Sections `10`, `11`, `13`, `16`, `19.1`, `20`.
- [x] `A4-11` Ensure the planner interface differentiates discrete-motion completion from continuous-motion steady-state so the classic `Done` vs `InVelocity` semantics remain correct. Spec: Sections `11`, `13`, `20`.

### Targeted Verification

- [x] `A4-12` Run only the single-axis motion fixture tests plus any directly related runtime tests for planning and cycle behavior. Spec: Sections `19.1`, `28`.

## Phase A5: Phase A Hardening And Integration Regression

Status: Done

### Tests First

- [x] `A5-01` Add tests proving writes less than or equal to `0` for `MaxVelocityAppl`, `MaxAccelerationAppl`, `MaxDecelerationAppl`, and `MaxJerkAppl` are rejected through the documented parameter-plane rule and do not silently invent fallback behavior. Spec: Sections `8`, `10`, `12`.
- [x] `A5-02` Add an end-to-end single-axis conformance scenario that exercises power, parameter setup, motion, stop/halt, fault, and reset through the ST fixture projects. Spec: Sections `10`, `11`, `12`, `13`, `19.1`, `28.1`.
- [x] `A5-03` Add a focused regression run against the existing standard-library ST test corpus for the areas most likely to be affected by the new motion kernel and catalog. Spec: Sections `20`, `28`, `30`.

### Implementation

- [x] `A5-04` Reconcile motion, parameter, and queue integration so dynamic fallback, limit lookup, and command admission go through one authoritative path. Spec: Sections `10`, `16`, `20`, `30`.
- [x] `A5-05` Keep the single-axis fixtures and shared helpers reusable for later synchronization, coordinated-motion, and homing phases instead of creating a one-off rollout-specific test harness. Spec: Sections `28`, `30`.

### Targeted Verification

- [x] `A5-06` Run the full single-axis targeted suite and update the compliance matrix rows for every shipped single-axis FB, enum, type, and the public `MC_Constants` carrier for standardized parameter/error names. Spec: Sections `4`, `12`, `19.1`, `28`.
- [x] `A5-07` Run the focused existing runtime regression suites selected in `A5-03` and record exact commands/results. Spec: Sections `20`, `28`, `30`.

## Phase A6: Phase A Deferred-Feature Guard

Status: Done

### Guard Rails

- [x] `A6-01` Keep `MC_MoveSuperimposed`, `MC_HaltSuperimposed`, `MC_TorqueControl`, `MC_PositionProfile`, `MC_VelocityProfile`, `MC_AccelerationProfile`, `MC_ReadDigitalInput`, `MC_ReadDigitalOutput`, `MC_WriteDigitalOutput`, `MC_DigitalCamSwitch`, `MC_TouchProbe`, and `MC_AbortTrigger` marked deferred or reserved until their own tests and matrix rows are implemented. Spec: Section `19.1`.
- [x] `A6-02` Ensure examples, docs, hover text, and future user-facing coverage tables do not imply those deferred single-axis items already ship. Spec: Sections `4`, `19.1`, `31`.
- [x] `A6-03A` Add semantic/analyzer negative tests for deferred single-axis public FBs whose compliance-matrix path is `absent`, proving the names are not present in the public catalog. Spec: Sections `4`, `12`, `19.1`.
- [x] `A6-03B` If any deferred single-axis public FBs use the compliance-matrix path `placeholder`, add runtime negative tests proving they fail deterministically with `mcERR_NotSupported`; otherwise record that Phase A uses the `absent` path only. Spec: Sections `4`, `12`, `19.1`.
- [x] `A6-04` Mirror the chosen absent-vs-placeholder path for each deferred single-axis public FB explicitly in the compliance matrix notes. Spec: Sections `4`, `19.1`.

## Phase B0: Synchronization Test Harness And Fixture Expansion

Status: Done

### Tests First

- [x] `B0-01` Extend the Phase B fixture project with deterministic master/slave axis test scenarios and table-driven cam test data. Spec: Sections `19.2`, `28.3`, `30`.
- [x] `B0-02` Add ST tests proving the simulated master/slave setup can produce deterministic phase relationships, sync start conditions, and cam-table activation timing. Spec: Sections `20`, `28`, `30`.

### Implementation

- [x] `B0-03` Add reusable sync test helpers for master position progression, slave follow assertions, and sync break/exit conditions. Spec: Sections `19.2`, `28.1`, `28.3`.

### Targeted Verification

- [x] `B0-04` Run only the new sync fixture tests and smoke tests. Spec: Sections `19.2`, `28`.

## Phase B1: Cam Table Selection And Cam Control

Status: Done

### Tests First

- [x] `B1-01` Add ST tests for `MC_CamTableSelect` proving selection timing, readiness semantics, the Part 1 v2.0 `ExecutionMode` behavior, and the documented cyclic-update behavior defined by the selected profile. Spec: Section `19.2`.
- [x] `B1-02` Add ST tests for `MC_CamIn` proving correct command start semantics, `StartMode`, `MasterValueSource`, master/slave coupling behavior, and the expected synchronized-motion transition. Spec: Sections `13`, `19.2`, `28.3`.
- [x] `B1-03` Add ST tests for `MC_CamOut` proving synchronized-motion exit behavior and the required transition away from synchronized motion without implying standstill. Spec: Sections `13`, `19.2`.
- [x] `B1-04` Add negative tests for invalid cam activation order or missing selected-cam conditions if the profile chooses to reject them at command time. Spec: Sections `12`, `19.2`.

### Implementation

- [x] `B1-05` Implement `MC_CamTableSelect`, `MC_CamIn`, and `MC_CamOut` on top of the shared axis kernel and sync helpers. Spec: Sections `19.2`, `20`.
- [x] `B1-06` Ensure `MC_CamOut` uses the same state engine as the rest of the axis layer so synchronized-motion exit is not hardcoded separately. Spec: Sections `13`, `20`, `30`.

### Targeted Verification

- [x] `B1-07` Run only the cam-focused ST fixture tests and related focused runtime tests. Spec: Sections `19.2`, `28.3`.

## Phase B2: Gear Control

Status: Done

### Tests First

- [x] `B2-01` Add ST tests for `MC_GearIn` proving coupling behavior, the Part 1 v2.0 `MasterValueSource` input semantics, and `InGear` output behavior. Spec: Sections `11`, `19.2`, `28.1`.
- [x] `B2-02` Add ST tests for `MC_GearInPos` proving synchronization-start semantics and the documented distinction from plain gear-in behavior. Spec: Section `19.2`.
- [x] `B2-03` Add ST tests for `MC_GearOut` proving it may only leave synchronized motion from a valid synchronized state and produces the correct exit state. Spec: Sections `13`, `19.2`.
- [x] `B2-04` Add negative tests for invalid gear-out from non-synchronized states and mismatched master/slave conditions. Spec: Sections `12`, `13`, `19.2`.

### Implementation

- [x] `B2-05` Implement `MC_GearIn`, `MC_GearInPos`, and `MC_GearOut` using shared sync primitives rather than separate one-off code paths. Spec: Sections `19.2`, `20`, `30`.

### Targeted Verification

- [x] `B2-06` Run only the gear-focused ST fixture tests and related focused runtime tests. Spec: Sections `19.2`, `28.1`, `28.3`.

## Phase B3: Synchronization Semantics, Buffering, And Exit Rules

Status: Done

### Tests First

- [x] `B3-01` Add tests proving `InGear` and any other implemented `Inxxx` outputs follow the classic setpoint-equality semantics instead of pulse semantics. Spec: Sections `11`, `28.1`.
- [x] `B3-02` Add tests proving sync FBs interact correctly with the shared buffer/abort model and do not bypass axis queue semantics. Spec: Sections `11`, `16`, `19.2`.
- [x] `B3-03` Add scenario tests based on PLCopen cam+gear examples to lock end-to-end sync behavior. Spec: Section `28.3`.

### Implementation

- [x] `B3-04` Reconcile sync FB outputs and abort behavior with the shared axis lifecycle helpers so sync commands do not carry custom latch rules. Spec: Sections `11`, `16`, `20`, `30`.

### Targeted Verification

- [x] `B3-05` Run the full Phase B targeted suite and update the compliance matrix rows for all Phase B FBs. Spec: Sections `4`, `19.2`, `28`.

## Phase B4: Synchronization Deferred-Feature Guard

Status: Done

### Guard Rails

- [x] `B4-01` Keep `MC_PhasingAbsolute`, `MC_PhasingRelative`, and `MC_CombineAxes` marked deferred or optional until their own tests and compliance-matrix rows are complete. Spec: Section `19.2`.
- [x] `B4-02` Ensure docs, completion lists, examples, and future user-facing coverage tables do not imply those deferred Phase B items already ship. Spec: Sections `4`, `19.2`, `31`.
- [x] `B4-03A` Add semantic/analyzer negative tests for deferred Phase B public FBs whose compliance-matrix path is `absent`, proving the names are not present in the public catalog. Spec: Sections `4`, `12`, `19.2`.
- [x] `B4-03B` Add runtime negative tests for deferred Phase B public FBs whose compliance-matrix path is `placeholder`, proving they fail deterministically with `mcERR_NotSupported`. Spec: Sections `4`, `12`, `19.2`.
- [x] `B4-04` Mirror the chosen absent-vs-placeholder path for each deferred Phase B public FB explicitly in the compliance matrix notes. Spec: Sections `4`, `19.2`.

## Phase C0: Group Foundation, Public Types, And Kinematic Test Bed

Status: Done

### Tests First

- [x] `C0-01` Extend the Phase C fixture project with deterministic multi-axis group scenarios and coordinate-system-aware expected values. Spec: Sections `7`, `8`, `19.3`, `28.2`, `30`.
- [x] `C0-02` Add semantic tests proving the required initial Phase C public types and enums resolve cleanly in ST source, including `AXES_GROUP_REF`, `IDENT_IN_GROUP_REF`, `MC_COMMAND_ID`, `MC_GROUP_PARAMETER`, `MC_TRANSITION_PARAMETER`, `MC_KIN_REF`, coordinate references, transform-related types, `MC_COORD_SYSTEM`, `MC_DYNAMICS_MODE`, `MC_TRANSITION_MODE`, `MC_TRANSITION_VELOCITY`, `MC_TRANSITION_REFERENCE`, `MC_COMMAND_STATE`, and the documented truST extension `MC_GROUP_STATUS`. Also prove the standardized `MC_GROUP_PARAMETER` members `mcDynamicsMode` and `mcTransitionReferencePoint` resolve exactly as specified. Spec: Sections `7`, `8`, `19.3`.
- [x] `C0-02A` Add semantic tests proving the `AXES_GROUP_REF` public field names are stable and resolve exactly as specified. Spec: Section `7`.
- [x] `C0-03` Add ST tests proving the simulated group setup can model axis membership, group state, commanded vs actual Cartesian values, and deterministic command acceptance. Spec: Sections `21`, `22`, `28.2`, `30`.
- [x] `C0-04` Add transform round-trip identity tests before group transform implementation begins. Spec: Sections `22`, `28.2`.

### Implementation

- [x] `C0-05` Implement the group kernel contract and fake group backend needed for deterministic Part 4 conformance tests. Spec: Sections `21`, `22`, `23`, `24`, `25`, `30`.
- [x] `C0-06` Register only the required initial Phase C public types, enums, and transform-related symbols in the standard-library catalog; keep later-only Part 4 enum families reserved in the compliance matrix until their first consuming FBs enter scope. Spec: Sections `4`, `7`, `8`, `19.3`.
- [x] `C0-07` Ensure the group kernel remains separate from the single-axis kernel while still reusing shared command-lifecycle infrastructure where behavior is identical. Spec: Sections `20`, `21`, `30`.

### Targeted Verification

- [x] `C0-08` Run only the Phase C type-resolution tests, transform identity tests, and fake-group-backend tests. Spec: Sections `19.3`, `22`, `28.2`.

## Phase C1: Group State, Membership, Admin, And Readback FBs

Status: Done

### Tests First

- [x] `C1-01` Add ST tests for `MC_AddAxisToGroup`, `MC_RemoveAxisFromGroup`, and `MC_UngroupAllAxes` proving group membership changes are deterministic and validated, including correct handling of `IdentInGroup : IDENT_IN_GROUP_REF`. Spec: Sections `19.3`, `21`.
- [x] `C1-02` Add ST tests for `MC_GroupEnable`, `MC_GroupDisable`, `MC_GroupPower`, and `MC_GroupReset` proving group administrative state handling is distinct from axis-power behavior, including the documented `MC_GroupPower.Enable = FALSE` power-loss policy. Spec: Sections `14`, `19.3`, `28.2`.
- [x] `C1-03` Add ST tests for `MC_GroupReadConfiguration`, `MC_ReadAxisGroupInfo`, `MC_GroupReadStatus`, and `MC_GroupReadError` proving group metadata and status/error readback are stable and queryable. Spec: Sections `14`, `19.3`.
- [x] `C1-04` Add ST tests for `MC_GroupReadPosition`, `MC_GroupReadVelocity`, and `MC_GroupReadAcceleration` proving coordinate-system-aware readback semantics. Spec: Sections `8`, `19.3`, `28.2`.
- [x] `C1-05` Add ST tests for `MC_GroupReadMotionState` proving `Tracking`, `InSync`, `InPosition`, `Standstill`, `ConstantVelocity`, `Accelerating`, `Decelerating`, and `ActiveCommandID` remain coherent across idle, moving, halted, stopped, and faulted conditions, and that the coordinate-system-relative outputs track the active coordinate system correctly. Spec: Sections `14`, `18`, `19.3`, `28.2`.
- [x] `C1-06` Add ST tests for `MC_GroupReadParameter`, `MC_GroupWriteParameter`, `MC_GroupReadSWLimits`, and `MC_GroupWriteSWLimits` covering representative valid and invalid cases, including `ParameterNumber : MC_GROUP_PARAMETER`. Spec: Sections `19.3`, `21`.
- [x] `C1-06A` Add ST tests proving `MC_GroupWriteParameter` / `MC_GroupReadParameter` round-trip `mcDynamicsMode` correctly and reject invalid writes according to the selected profile. Spec: Sections `8`, `9`, `19.3`, `28.2`.

### Implementation

- [x] `C1-07` Implement the required Phase C administrative/readback FBs on top of the shared group kernel. Spec: Sections `14`, `18`, `19.3`, `21`.
- [x] `C1-08` Ensure group administrative FBs do not accidentally use single-axis semantics for power, status, or reset where the Part 4 model differs. Spec: Sections `14`, `21`, `30`.

### Targeted Verification

- [x] `C1-09` Run only the group-admin and group-readback fixture suite plus focused runtime tests. Spec: Sections `19.3`, `28.2`.

## Phase C2: Transforms, Group Positioning, And Core Group Motion FBs

Status: Done

### Tests First

- [x] `C2-01` Add ST tests for `MC_SetKinTransform`, `MC_SetCartesianTransform`, `MC_SetCoordinateTransform`, and their corresponding read FBs proving transform round-trip behavior and validation rules. Spec: Sections `19.3`, `22`, `28.2`.
- [x] `C2-02` Add ST tests for `MC_GroupSetPosition` proving commanded group position updates are reflected consistently in subsequent readbacks. Spec: Sections `19.3`, `21`, `22`.
- [x] `C2-03` Add ST tests for `MC_MoveLinearAbsolute` and `MC_MoveLinearRelative` in at least `MCS` and one additional supported coordinate system. Spec: Sections `8`, `17`, `19.3`, `28.2`.
- [x] `C2-04` Add ST tests for `MC_MoveDirectAbsolute` and `MC_MoveDirectRelative` in `ACS` and `MCS` as required by the spec acceptance tests. Spec: Sections `8`, `19.3`, `28.2`.
- [x] `C2-05` Add ST tests for `MC_GroupHome` proving the allowed buffer-mode subset and correct state transition behavior. Spec: Sections `14`, `17`, `19.3`, `28.2`.
- [x] `C2-06` Add ST tests for `MC_GroupStop`, `MC_GroupHalt`, `MC_GroupWaitTime`, and `MC_GroupSetOverride` proving each FB preserves the distinct Part 4 group-motion semantics, and that `MC_GroupSetOverride` re-exercises the documented factor rules for `0.0 .. 1.0`, negative values, `AccFactor/JerkFactor = 0.0`, `VelFactor = 0.0`, and the selected truST handling for values above `1.0`. Spec: Sections `9`, `14`, `19.3`, `25`, `28.2`.
- [x] `C2-07` Add ST tests for `MC_TransformPosition` proving coordinate conversion behavior and deterministic validation of unsupported combinations. Spec: Sections `8`, `19.3`, `22`.

### Implementation

- [x] `C2-08` Implement the required transform, group-position, and core group-motion FBs on top of the shared group kernel and kinematic layer. Spec: Sections `19.3`, `21`, `22`, `25`.
- [x] `C2-09` Ensure linear vs direct motion semantics are represented explicitly in the planner/group command layer rather than inferred indirectly from coordinates alone. Spec: Sections `19.3`, `21`, `22`.

### Targeted Verification

- [x] `C2-10` Run only the transform and group-motion fixture tests plus focused runtime kinematics tests. Spec: Sections `19.3`, `22`, `28.2`.

## Phase C3: Blend Model, Dynamics, Command Acceptance, And Group Queue Semantics

Status: Done

### Tests First

- [x] `C3-01` Add ST tests proving the corrected enum split between `MC_BUFFER_MODE`, `MC_TRANSITION_MODE`, `MC_TRANSITION_VELOCITY`, and `MC_TRANSITION_REFERENCE`, including the compatibility-only treatment of legacy `mcBlending*` values in coordinated-motion contexts. Spec: Sections `8`, `17`.
- [x] `C3-02` Add ST tests proving the Phase C supported subset for blending behaves exactly as specified: `mcAborting`, `mcBuffered`, `mcTMNone`, `mcTMCornerDistance`, `mcTVZero`, `mcTVNext`, and `mcStartPoint`. Spec: Sections `17`, `19.3`.
- [x] `C3-02A` Add negative ST tests proving deferred Phase C transition modes and transition velocities return `mcERR_NotSupported` until implemented. Spec: Sections `12`, `17`, `19.3`.
- [x] `C3-02B` Add negative ST tests proving the documented truST error-path handling for legacy `mcBlending*` values in group contexts returns `mcERR_NotSupported`. Spec: Sections `12`, `17`, `19.3`.
- [x] `C3-03` Add ST tests for `MC_GroupWriteReferenceDynamics`, `MC_GroupReadReferenceDynamics`, `MC_GroupWriteDefaultDynamics`, and `MC_GroupReadDefaultDynamics` proving the reference/default dynamics planes are distinct and queryable. Spec: Sections `17`, `19.3`.
- [x] `C3-03A` Add ST tests proving `MC_GroupWriteParameter` / `MC_GroupReadParameter` round-trip `mcTransitionReferencePoint` correctly and that the chosen transition-reference-point policy is reflected in subsequent coordinated-motion command acceptance. Spec: Sections `8`, `17`, `19.3`, `28.2`.
- [x] `C3-04` Add ST tests for `MC_GroupReadCommandInfo` proving retained metadata behavior after `CommandAccepted` and during active/recent command lifetime, including `CommandState`, `ElapsedDuration`, `RemainingDuration`, `RemainingDistance`, `Progress`, `InfoID`, and `WarningID`. Spec: Sections `18`, `19.3`, `28.2`.
- [x] `C3-05` Add ST tests for the FBs that expose `CommandAccepted` and `CommandID`, proving handshake timing and retention behavior match the spec and do not assume those outputs exist on every coordinated-motion FB. Spec: Section `18`.
- [x] `C3-06` Add ST tests proving `MC_GroupStop` blocks subsequent commands until the required release condition and cancels active synchronization/tracking relationships. Spec: Sections `14`, `24`, `25`, `28.2`.
- [x] `C3-07` Add tests proving dynamic coordinate-system behavior for `PCS` follows the spec's dynamic-frame rule, including the `MC_GroupHalt` case that may end in `GroupMoving` instead of `GroupStandby`. Spec: Sections `8`, `14`, `23`, `28.2`.

### Implementation

- [x] `C3-08` Implement the Phase C dynamics, command-info, buffer/transition, and queue semantics using one authoritative group command scheduler. Spec: Sections `17`, `18`, `19.3`, `21`, `23`, `25`.
- [x] `C3-09` Ensure command-retention/query behavior is driven by the group scheduler or command store, not by transient FB output latches alone. Spec: Sections `18`, `21`, `30`.

### Targeted Verification

- [x] `C3-10` Run only the blend/dynamics/command-info fixture suite and focused runtime queue tests. Spec: Sections `17`, `18`, `19.3`, `28.2`.

## Phase C4: Tracking And Synchronization Subset (Optional C.1)

Status: Done

### Scope Guard

- [x] `C4-00` The current shipped profile does not select optional Phase C.1; keep it out of active implementation until a later scope expansion confirms the required robot/conveyor use cases. Spec: Sections `19.4`, `30`.

### Guard Rails

- [x] `C4-01` Keep `MC_SetDynCoordTransform`, `MC_TrackConveyorBelt`, `MC_SyncAxisToGroup`, and `MC_SyncGroupToAxis` documented as deferred absent-path names in the current shipped profile rather than starting partial implementation. Spec: Sections `19.4`, `23`, `24`.
- [x] `C4-02` Record that no conveyor-tracking or robot-follow scenario is in scope for the current shipped profile; do not ship placeholder examples or tests that imply otherwise. Spec: Sections `19.4`, `28.3`.
- [x] `C4-03` Keep circular synchronization-ownership semantics out of scope until C.1 is explicitly selected; current docs/examples may not imply the feature exists. Spec: Sections `24`, `30`.

### Implementation

- [x] `C4-04` Keep the Phase C core group scheduler untouched by the not-selected C.1 subset; do not add partial tracking/synchronization code on the current shipped path. Spec: Sections `19.4`, `23`, `24`, `30`.

### Targeted Verification

- [x] `C4-05` Verify the optional C.1 names remain deferred on the absent path in the compliance matrix, decisions log, and deferred-public-surface coverage; no C.1 positive fixtures ship in the current profile. Spec: Sections `19.4`, `28.2`, `28.3`.

## Phase C5: Coordinated Deferred-Feature Guard

Status: Done

### Guard Rails

- [x] `C5-01` Keep path moves, circular moves, interrupt/continue, jogging, DH/joint info, tool/payload data FBs, and the other deferred coordinated-motion FBs marked deferred until their own tests and matrix rows are complete. Spec: Sections `19.3`, `19.4`.
- [x] `C5-02` Ensure docs, completion lists, examples, and future user-facing guides do not imply those deferred coordinated-motion features already ship. Spec: Sections `4`, `19.3`, `31`.
- [x] `C5-03A` Add semantic/analyzer negative tests for deferred Phase C public FBs whose compliance-matrix path is `absent`, proving the names are not present in the public catalog. Spec: Sections `4`, `12`, `19.3`, `19.4`.
- [x] `C5-03B` Record that the current shipped Phase C deferred FB set uses the `absent` path only, so no runtime placeholder-negative tests are required in this profile slice. Spec: Sections `4`, `12`, `19.3`, `19.4`.
- [x] `C5-04` Mirror the chosen absent-vs-placeholder path for each deferred Phase C public FB explicitly in the compliance matrix notes. Spec: Sections `4`, `19.3`, `19.4`.

## Phase D0: Homing Toolkit Harness And Fixture Expansion

Status: Done

### Tests First

- [x] `D0-01` Extend the Phase D fixture project with deterministic homing scenarios, including simulated switch, block, distance-coded, reference-pulse, and limit conditions. Spec: Sections `19.5`, `28.1`, `30`.
- [x] `D0-02` Add ST tests proving the simulated axis setup can drive homing sensors, reference pulses, detection limits, torque limits, and distance/time limits deterministically. Spec: Sections `20`, `19.5`, `28.1`, `30`.
- [x] `D0-02A` Add semantic tests proving the Phase D public types and enums resolve cleanly in ST source, including `MC_HOME_DIRECTION`, `MC_SWITCH_MODE`, and `MC_REF_SIGNAL_REF`. Spec: Sections `7`, `8`, `19.5`.

### Implementation

- [x] `D0-03` Add reusable homing test helpers that assert scan-by-scan homing state, command progression, and error-limit handling without hiding PLCopen-visible behavior. Spec: Sections `13`, `19.5`, `28.1`.

### Targeted Verification

- [x] `D0-04` Run only the homing fixture tests and smoke tests. Spec: Sections `19.5`, `28`.

## Phase D1: Part 5 Step FBs And Direct Homing Commands

Status: Done

### Tests First

- [x] `D1-01` Add ST tests for `MC_StepAbsoluteSwitch` and `MC_StepLimitSwitch` proving state progression, completion behavior, and error handling. Spec: Sections `19.5`, `28.1`.
- [x] `D1-02` Add ST tests for `MC_StepBlock` proving the distinct detection-velocity-limit and detection-velocity-time behavior. Spec: Sections `19.5`, `28.1`.
- [x] `D1-03` Add ST tests for `MC_StepReferencePulse` and `MC_StepDistanceCoded` proving each FB's trigger model remains distinct. Spec: Sections `19.5`, `28.1`.
- [x] `D1-04` Add ST tests proving the implemented Part 5 step FBs leave the axis in `Homing` after they complete and do not silently transition back to `Standstill`. Spec: Sections `13`, `19.5`, `28.1`.
- [x] `D1-05` Add ST tests for `MC_HomeDirect`, `MC_HomeAbsolute`, and `MC_FinishHoming` proving they finalize homing correctly and transition `Homing -> Standstill` when started in `Homing`, while still remaining callable in the other documented states. Spec: Sections `13`, `19.5`, `28.1`.
- [x] `D1-06` Add tests proving torque, time, and distance limit semantics are enforced consistently across the implemented Part 5 step FBs. Spec: Sections `19.5`, `28.1`.

### Implementation

- [x] `D1-07` Implement the selected Part 5 step FBs and direct homing commands on top of one shared homing execution layer. Spec: Sections `19.5`, `20`, `30`.
- [x] `D1-08` Ensure the Part 5 step FBs reuse the existing axis state engine and error model instead of introducing a parallel homing state machine. Spec: Sections `12`, `13`, `19.5`, `20`, `30`.

### Targeted Verification

- [x] `D1-09` Run only the Part 5 step-FB fixture tests and focused runtime homing tests. Spec: Sections `19.5`, `28.1`.

## Phase D2: Homing Integration, Generic `MC_Home`, And Regression Closure

Status: Done

### Tests First

- [x] `D2-01` Add regression tests proving the generic Phase A `MC_Home` still behaves correctly after the Part 5 toolkit lands. Spec: Sections `13`, `19.1`, `19.5`.
- [x] `D2-02` Add tests proving implemented Part 5 step FBs do not accidentally force unsupported motion-state transitions beyond the documented behavior, including the rule that passive/flying homing FBs do not change the axis state when those FBs are later added. Spec: Sections `13`, `19.5`.
- [x] `D2-03` Add scenario tests for at least one multi-step homing procedure assembled from the implemented Part 5 FBs. Spec: Sections `19.5`, `28.3`.

### Implementation

- [x] `D2-04` Reconcile generic homing and custom homing-toolkit behavior so they share backend capabilities and consistent error mapping without collapsing into one undocumented execution path. Spec: Sections `12`, `13`, `19.1`, `19.5`, `20`.

### Targeted Verification

- [x] `D2-05` Run the full Phase D targeted suite and update the compliance matrix rows for every Part 5 FB in scope. Spec: Sections `4`, `19.5`, `28`.

## Phase D3: Deferred Passive/Flying Homing Guard

Status: Done

### Guard Rails

- [x] `D3-01` Keep `MC_StepReferenceFlyingSwitch`, `MC_StepReferenceFlyingRefPulse`, and `MC_AbortPassiveHoming` marked deferred until their own tests and matrix rows exist. Spec: Section `19.5`.
- [x] `D3-02` Ensure examples, docs, and completion surfaces do not imply passive/flying homing support before it is implemented. Spec: Sections `4`, `19.5`, `31`.
- [x] `D3-03A` Add semantic/analyzer negative tests for deferred Phase D public FBs whose compliance-matrix path is `absent`, proving the names are not present in the public catalog. Spec: Sections `4`, `12`, `19.5`.
- [x] `D3-03B` Record that the current shipped Phase D deferred FB set uses the `absent` path only, so no runtime placeholder-negative tests are required in this profile slice. Spec: Sections `4`, `12`, `19.5`.
- [x] `D3-04` Mirror the chosen absent-vs-placeholder path for each deferred Phase D public FB explicitly in the compliance matrix notes. Spec: Sections `4`, `19.5`.

## Phase E: Optional OO Facade

Status: Done

### Scope Guard

- [x] `E-01` The current shipped motion profile does not select the OO facade; keep the classic FB layer as the only normative public motion API until a later scope expansion explicitly chooses otherwise. Spec: Sections `3`, `19.6`, `29`, `31`.
- [x] `E-02` Record OO command lifetime/aliasing as future scope only; it is not a blocker for the shipped classic motion profile. Spec: Sections `29`, `30`.

### Guard Rails

- [x] `E-03` Do not ship OO API-shape tests or wrapper types in the current profile; the classic FB layer remains the only supported motion surface. Spec: Sections `29`, `31`.
- [x] `E-04` Do not ship `Abort()` / `Wait(Timeout, AbortOnTimeout)` mapping behavior in the current profile; no docs or examples may imply it exists. Spec: Section `29`.
- [x] `E-05` Do not ship OO command-reference lifetime behavior in the current profile; no docs or examples may imply it exists. Spec: Section `29`.

### Implementation

- [x] `E-06` Keep the OO facade unimplemented and non-blocking in the current shipped profile; the classic FB layer remains the source of truth. Spec: Sections `29`, `31`.

### Targeted Verification

- [x] `E-07` Verify the OO facade remains non-shipped in the compliance matrix, guide, and active docs so it cannot block classic conformance closure. Spec: Sections `29`, `31`.

## Documentation, Diagram, And Coverage Sync

Status: Done

- [x] `DOC-01` Update the motion spec and compliance matrix in the same change set as each completed phase so supported/deferred status remains truthful. Spec: Sections `4`, `19`, `31`.
- [x] `DOC-02` Update `docs/PLCOPEN_DECISIONS.md` for every PLCopen-standard ambiguity resolved during motion implementation. Spec: Sections `2`, `6`, `10`, `12`.
- [x] `DOC-03` Update `docs/PLCOPEN_DEVIATIONS.md` for every truST-specific behavior choice or non-standard PLCopen extension used by the motion library. Spec: Sections `2`, `6`, `10`, `12`.
- [x] `DOC-04` Treat architecture and diagram sync as required for Phase A closure and Phase C closure: update the PLCopen motion PlantUML diagram, regenerate outputs, refresh `docs/diagrams/manifest.json`, and update `docs/internal/testing/checklists/architecture-improvements.md`. Spec: Sections `20`, `21`, `22`, `30`.
- [x] `DOC-05` Update `docs/specs/10-runtime-semantics.md` when reusable runtime call-binding semantics or motion-runtime integration points materially change. Spec: Sections `28`, `30`.
- [x] `DOC-06` Create the user-facing guide and coverage table for the supported PLCopen motion FBs instead of exposing internal planning docs as public documentation. Spec: Sections `4`, `19`, `31`.
- [x] `DOC-07` Audit examples and tutorial material so they demonstrate only the FBs and semantics that are actually shipped in the current profile. Spec: Sections `19`, `31`.

## Definition Of Done

- [x] `DOD-01` The classic PLCopen FB layer is the only normative public motion API for the shipped scope. Spec: Sections `3`, `31`.
- [x] `DOD-02` Every shipped public FB/type/enum has a compliance-matrix row, deterministic tests, and recorded evidence. Spec: Sections `4`, `28`.
- [x] `DOD-03` Phase A support is not claimed until the entire Phase A checklist and deferred-feature guard are green. Spec: Sections `19.1`, `31`.
- [x] `DOD-04` Phase B support is not claimed until the entire Phase B checklist is green. Spec: Sections `19.2`, `31`.
- [x] `DOD-05` Phase C support is not claimed until the entire Phase C checklist for the selected profile is green. Spec: Sections `19.3`, `19.4`, `31`.
- [x] `DOD-06` Phase D support is not claimed until the entire Phase D checklist for the selected profile is green. Spec: Sections `19.5`, `31`.
- [x] `DOD-07` No deferred, reserved, or non-targeted feature is described as implemented anywhere in active docs, examples, or IDE surfaces. Spec: Sections `4`, `19`, `31`.
- [x] `DOD-08` The motion library's public behavior is locked by ST conformance tests first, then by focused runtime/kernel tests where needed. Spec: Sections `28`, `30`.
- [x] `DOD-09` The implementation remains SOLID, KISS, and DRY according to the shared-command-lifecycle architecture goal. Spec: Sections `20`, `21`, `30`.

## Validation Gates

Status: In Progress

- [x] `VG-01` For each active phase, run only the targeted ST fixture tests for that phase, and record exact commands/results in the evidence artifact. Spec: Sections `28`, `30`.
- [x] `VG-02` For the current ST-only shipped scope, record that the Phase A release-quality claim relies on the focused ST symbol/surface tests in this checklist; future IDE/LSP motion-symbol tests remain tracked separately as tooling follow-up scope. Spec: Sections `7`, `8`, `19.1`.
- [x] `VG-03` Before any release-quality Phase C claim, run the full coordinated-motion targeted suite including transform, blend, command-info, and group-stop/halt coverage. Spec: Sections `17`, `18`, `19.3`, `28.2`.
- [x] `VG-04` Before any release-quality Phase D claim, run the full homing targeted suite including limit/error cases. Spec: Sections `19.5`, `28.1`.
- [x] `VG-05` Run `just fmt` after the targeted phase work is complete for the current milestone. Spec: Sections `28`, `30`.
- [x] `VG-06` Run `just clippy` after the targeted phase work is complete for the current milestone. Spec: Sections `28`, `30`.
- [x] `VG-07` Run `just test` after the targeted phase work is complete for the current milestone. Spec: Sections `28`, `30`.
- [ ] `VG-08` Run `just test-all` only at the final full-workspace acceptance gate for the selected shipped scope. Pending after the package-relocation pass because only targeted motion validation was rerun. Spec: Sections `28`, `30`.
- [x] `VG-09` If runtime architecture or data flow changes, also run the required architecture/diagram drift checks before declaring completion. Spec: Sections `20`, `21`, `22`, `30`.

## Evidence Register

Status: In Progress

- [x] `EV-01` Create and maintain `docs/internal/testing/evidence/plcopen-motion-library-implementation-<date>.md`. Spec: Sections `4`, `28`, `30`.
- [x] `EV-02` Record the exact `trust-runtime test --project ...` commands and outputs used for each phase fixture. Spec: Sections `28`, `30`.
- [x] `EV-03` Record the exact focused ST fixture commands and outputs used for simulated backends, kernel semantics, queue logic, and kinematics. Spec: Sections `20`, `21`, `22`, `28`, `30`.
- [x] `EV-04` Record the exact ST fixture discovery and symbol/signature validation commands and outputs used to validate motion-library public names and signatures. Spec: Sections `7`, `8`, `19`.
- [ ] `EV-05` Record milestone and final gate outputs for `just fmt`, `just clippy`, `just test`, and `just test-all`. Pending after the package-relocation pass because `just test-all` was not rerun and is not claimed complete here. Spec: Sections `28`, `30`.
- [x] `EV-06` Link each completed checklist item to the evidence artifact entry that proves it. Spec: Sections `4`, `28`, `30`.
- [x] `EV-07` Ensure each dated evidence artifact records at least the commit SHA, timestamp, exact command, and enough expected/actual result detail to reproduce the claim. Spec: Sections `4`, `28`, `30`.
