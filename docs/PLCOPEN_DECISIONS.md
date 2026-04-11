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
- Reason:
  - These choices turn PLCopen implementation-defined, profile-scoped, or source-inconsistent areas into deterministic, testable truST behavior before implementation begins.
