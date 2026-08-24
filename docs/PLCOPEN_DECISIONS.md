# PLCopen Decisions Log

This file tracks implementation decisions made where PLCopen profiles or source documents leave room for interpretation.

## 2026-04-11 - PLCopen motion profile choices

- Area: PLCopen Motion Parts 1 / 4 / 5 profile behavior
- PLCopen context: PLCopen Motion Control Part 1 v2.0 sections 2.4.1, 3.1, 3.3, 3.18, 3.28, 3.31; Part 4 v2.0 RFC sections 2.2, 3.1, 9.1, 9.9, 9.19.1, 11.3.1; and Part 5 v2.0 as the pinned homing-extension baseline
- Decision:
  - The pinned normative source revisions for this profile are Part 1 v2.0 published `2011-03-17`, Part 4 v2.0 RFC dated `2025-11-18`, and Part 5 v2.0 published `2011-11-16`. The Part 4 RFC is treated as normative for the selected coordinated-motion profile until truST intentionally updates to a later Part 4 revision.
  - Single-axis motion commands issued to grouped axes are rejected with the public constant `mcERR_AxisGrouped`.
  - Zero-valued `Acceleration`, `Deceleration`, and `Jerk` inputs use the configured axis/group maximum values.
  - Writes less than or equal to `0` for `MaxVelocityAppl`, `MaxAccelerationAppl`, `MaxDecelerationAppl`, and `MaxJerkAppl` are rejected with `mcERR_InvalidParameter`; this parameter-plane rule is distinct from the zero-valued FB-input fallback.
  - `MC_Reset` outside `ErrorStop` is accepted as a deterministic no-op command: no axis-state change, and completion without error if the backend accepts the request.
  - Vendor-specific BOOL parameter numbers greater than `999` may be accepted by `MC_ReadBoolParameter` / `MC_WriteBoolParameter` when truST declares them as BOOL in the parameter registry.
  - The initial Phase A profile does not target `MC_Power.EnablePositive` or `MC_Power.EnableNegative`, and these inputs are omitted from the initial public Phase A signature.
  - `MC_SetOverride` remains the classic enable-style FB in the initial Phase A profile, with `Enabled` as its status output; the initial truST profile also adopts the stricter documented choice of rejecting override factors greater than `1.0`.
  - Single-axis FBs in the initial Phase A profile reject `MC_EXECUTION_MODE = mcDelayed` with `mcERR_NotSupported`.
  - `MC_Stop` follows the Part 1 v2.0 FB tables in the selected truST profile and does not expose a public `Active` output; the conflicting Part 1 section 2.4.1 wording is treated as a source inconsistency rather than as a signature requirement.
  - The initial Phase C minimum coordinate-system subset is `ACS`, `MCS`, and `PCS`.
  - `IDENT_IN_GROUP_REF` is represented in the public truST profile as `STRING[63]`, carrying the stable member/kinematic name used inside the axes group.
  - The initial standardized `MC_GROUP_PARAMETER` surface includes `mcDynamicsMode` and `mcTransitionReferencePoint`.
  - `MC_GroupPower.Enable = FALSE` causing power loss during active group motion is treated as a power-failure path into `GroupErrorStop`.
  - `ContinuousUpdate` changes to a buffered FB that has been accepted but is not yet `Active` are ignored until a new `Execute` edge submits an updated command.
  - truST retains `mc`, `mcTM`, and `mcTV` prefixes consistently in public ST enum literals even where Part 4 allows omission when enum-qualified.
  - For `MC_COORD_SYSTEM`, truST publishes the public ST literals as `mcACS`, `mcMCS`, `mcWCS`, `mcPCS`, `mcFCS`, and `mcTCS` even though the Part 4 coordinate-system table writes these values in bare form.
  - The initial ST publication path for standardized Part 1 parameter IDs and the public `mcERR_*` namespace is `MC_Constants`, which exposes stable accessible members with those names.
  - The initial public `mcERR_*` mapping is pinned to the spec's stable namespace table: `mcERR_None=16#0000`, `mcERR_InvalidParameter=16#0001`, `mcERR_InvalidState=16#0100`, `mcERR_AxisGrouped=16#0101`, `mcERR_GroupDisabled=16#0102`, `mcERR_GroupNotReady=16#0103`, `mcERR_NotHomed=16#0104`, `mcERR_NotPowered=16#0105`, `mcERR_BackendFault=16#0200`, `mcERR_KinematicNoSolution=16#0300`, `mcERR_KinematicSingularity=16#0301`, `mcERR_QueueFull=16#0400`, `mcERR_NotSupported=16#0500`.
  - Phase C compliance-matrix rows track the pinned Part 4 RFC revision dated `2025-11-18`; deferred DH/joint-introspection names must be re-verified when that Part 4 source revision changes.
  - When single-axis motion FBs consume the Part 1 software-limit parameter plane, enabled `SWLimitPos` / `SWLimitNeg` values clamp accepted position targets for `MC_Home`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveContinuousAbsolute`, and `MC_MoveContinuousRelative`; disabled limit flags leave the target unclamped.
  - The current Phase A deferred single-axis FB set uses the `absent` path only; none of those deferred names ship as runtime placeholders in the initial release.
  - The initial Phase B synchronization enum surface includes `MC_START_MODE = {mcAbsolute, mcRelative, mcRampIn}` and `MC_SYNC_MODE = {mcShortest, mcCatchUp, mcSlowDown}`.
  - The initial Phase B synchronization public types use `MC_CAM_ID = UINT` and publish `MC_CAM_REF` as a fixed 8-point ST struct with `MasterPosition0..7` / `SlavePosition0..7` fields.
  - In the current Phase B profile, `MC_CamTableSelect` rejects `MC_EXECUTION_MODE = mcDelayed` with `mcERR_NotSupported`.
  - The current Phase B deferred synchronization FB set (`MC_PhasingAbsolute`, `MC_PhasingRelative`, `MC_CombineAxes`) uses the `absent` path only; none of those names ship as runtime placeholders in the current profile.
  - The initial Phase C public kinematic reference type uses `MC_KIN_REF = UINT`.
- Reason:
  - These choices turn PLCopen implementation-defined, profile-scoped, or source-inconsistent areas into deterministic, testable truST behavior before implementation begins.

## 2026-04-12 - Coordinated-motion scope pin and homing guard decisions

- Area: PLCopen Motion Parts 4 / 5 shipped scope
- PLCopen context: PLCopen Motion Control Part 4 v2.0 RFC sections 19.3, 19.4 and Part 5 v2.0 section 19.5 as pinned by the truST motion spec
- Decision:
  - The current shipped coordinated-motion profile is the Phase C core subset only; the optional Phase C.1 tracking/synchronization FBs (`MC_SetDynCoordTransform`, `MC_TrackConveyorBelt`, `MC_SyncAxisToGroup`, `MC_SyncGroupToAxis`, `MC_TrackRotaryTable`) remain deferred on the absent path until a later scope expansion explicitly selects them.
  - The current shipped Phase D homing profile includes `MC_StepAbsoluteSwitch`, `MC_StepLimitSwitch`, `MC_StepBlock`, `MC_StepReferencePulse`, `MC_StepDistanceCoded`, `MC_HomeDirect`, `MC_HomeAbsolute`, and `MC_FinishHoming`.
  - The passive/flying Part 5 homing FBs (`MC_StepReferenceFlyingSwitch`, `MC_StepReferenceFlyingRefPulse`, `MC_AbortPassiveHoming`) remain deferred on the absent path in the current shipped profile.
- Reason:
  - These decisions keep the public PLCopen surface aligned with the ST fixtures that are actually implemented and tested, while preserving the reserved names explicitly in the compliance matrix.

## 2026-04-26 - PLCopen Motion OOP facade scope

- Area: PLCopen Motion OOP application example surface
- PLCopen context: PLCopen "Application Examples for Motion Control - Porting into OOP" v1.0 sections 2, 3, 5, 6, 7, and 8; PLCopen public download catalog entries for `OOP Motion Control Library` and `PLCopen OOP Motion Control Library`
- Decision:
  - truST ships `libraries/plcopen_motion/oop` as a second public motion package, while the classic PLCopen FB packages remain the primary compliance surface and behavior source of truth.
  - The shipped OOP subset includes the PLCopen command interfaces, `itfAxis`, concrete command objects, and `MC_OopAxis`.
  - `MC_OopAxis` adapts OOP method/property calls to the existing classic single-axis package instead of duplicating axis state, parameter storage, or queue logic.
  - The PLCopen OOP document intentionally omits `AXIS_REF`; truST therefore adds the vendor-specific `MC_OopAxis.Bind(AxisId, InternalIndex)` method to connect an OOP axis object to a simulated or configured axis slot.
  - The misspelled PLCopen example name `itfContinousAxisCommand` is published as a compatibility alias extending the corrected `itfContinuousAxisCommand`.
  - OOP profile, probe, digital-cam, torque/superimposed, and synchronization methods that are not implemented by the current OOP facade remain present and return deterministic command objects with `mcERR_NotSupported`.
- Reason:
  - This matches the PLCopen OOP guidance that a standardized interface defines the user-facing axis surface, while implementation remains vendor-specific and may coexist with procedural FBs.

## 2026-07-26 - Phase A deterministic single-axis profile choices

- Area: PLCopen Motion Part 1 Phase A single-axis behavior
- PLCopen context: Part 1 v2.0 sections 2.4.1, 2.4.2, 2.4.3, 3.3, 3.18,
  3.20, and Table 5
- Decision:
  - `AXIS_REF.AxisId` and `AXIS_REF.InternalIndex` are supported public handle
    fields that may be initialized before FB use but not mutated while work is
    active or queued. The Phase A kernel uses `InternalIndex` directly and
    requires a configured slot in `0..31`; it does not silently normalize an
    out-of-range value. PLCopen makes the fields' content implementation
    dependent and requires support for any exposed elements.
  - The current single-axis kernel provides one active slot and exactly two
    buffered FIFO slots. A third buffered request fails with
    `mcERR_QueueFull` without mutating accepted work.
  - `PN_TRUST_VendorBool0 = 1000` is registered by truST as a per-axis
    application metadata bit with default `FALSE`, no motion side effect, and
    immediate or queued read/write behavior.
  - Standard read-only parameter numbers may be initialized through
    `MC_WriteParameter` only while the axis is `Disabled` with no active or
    queued work, or through the internal test-seeding boundary. Public writes
    outside that phase fail with `mcERR_InvalidState`.
  - An aborting `MC_Home` accepted from `DiscreteMotion` enters `Homing` and
    completes in `Standstill` after aborting the current owner and clearing
    buffered work.
  - The deterministic verification helpers are not PLCopen public FBs and do
    not widen the compliance surface.
- Reason:
  - These implementation-dependent choices make the selected software-only
    Phase A profile deterministic without presenting truST-only behavior as
    normative PLCopen behavior.

## 2026-07-27 - ST-complete interchange profile choices

- Area: PLCopen TC6 ST interchange and vendor migration artifacts
- PLCopen context: TC6 namespace `http://www.plcopen.org/xml/tc6_0200`;
  graphical-network semantics and native vendor package formats are outside
  the selected profile
- Decision:
  - The public profile identity is `trust-st-complete-v1`, limited to the
    documented ST, data-type, project-model, and CODESYS metadata subset.
  - Imported project sources are written under `src/`; a data-type-only import
    writes `src/plcopen_data_types.st`.
  - FBD, LD, SFC, and unknown executable bodies fail closed with the documented
    `PLCO215` through `PLCO218` diagnostics rather than being scraped as ST.
  - Opaque vendor `addData` is preserved for re-injection but is not
    semantically interpreted.
  - The reviewed Siemens, Rockwell, Schneider/CODESYS, Mitsubishi, and OpenPLC
    symbol shims are migration transforms whose applications are reported;
    they are not vendor-runtime equivalence claims.
  - The public compatibility guide owns the exact shim catalogue, token
    positions, ecosystem-detection precedence, deterministic migration-score
    formulas, `PLCO101` through `PLCO103` structural diagnostics, embedded
    source-map tolerance, opaque-extension wrapper, and migration-report JSON
    serialization. These are truST migration-product choices, not normative
    PLCopen scoring or vendor-runtime semantics.
  - Allen-Bradley, Siemens, and Schneider export adapters emit the documented
    XML, sidecar report, and optional Siemens SCL bundle. They do not emit
    native vendor project packages. The public target ID, suffix, label, and
    serialized-enum table is a stable truST adapter contract.
  - PLCopen POU-kind aliases and their XML/declaration/terminator projection,
    plus native-vendor-parity as the default global-variable import mode, are
    explicit truST interchange choices; strict IEC adapter reshaping requires
    an explicit option.
- Reason:
  - TC6 supplies the interchange container, but the supported subset, source
    layout, vendor aliases, reports, and adapter artifacts are product/profile
    choices that must remain explicit and testable without being presented as
    IEC deviations or full vendor compatibility.
