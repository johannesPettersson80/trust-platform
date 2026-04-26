# PLCopen Deviations Log

This file tracks known, intentional deviations/extensions from strict PLCopen profile behavior.

## 2026-04-11 - PLCopen motion profile extensions and stricter choices

- Area: PLCopen Motion library profile
- PLCopen reference: PLCopen Motion Control Part 1 v2.0 and Part 4 v2.0 RFC
- Deviation:
  - `MC_AXIS_STATUS` and `MC_GROUP_STATUS` are published as truST extension enum types; PLCopen itself standardizes the status surface primarily through boolean outputs on `MC_ReadStatus` and `MC_GroupReadStatus`.
  - `MC_Constants` is published as a truST ST helper FB for standardized Part 1 parameter IDs and the stable public `mcERR_*` namespace; PLCopen defines the IDs and classic `ErrorID : WORD` surface, but not this convenience carrier. In the current runtime, callers invoke `MC_Constants()` before reading members because the outputs are assigned in the FB body.
  - The single-axis, synchronization, and coordinated-motion library packages use file-scope `VAR_GLOBAL` blocks in their `*_globals.st` sources as internal shared-state carriers for the motion kernels. That placement is a truST-specific implementation device, is not part of the public PLCopen surface, and is intentionally omitted from the compliance matrix.
  - `MC_CAM_REF` is published in the current ST profile as a fixed 8-point struct carrier (`MasterPosition0..7` / `SlavePosition0..7`) rather than as an opaque vendor reference or dynamically sized table object.
  - `MC_SYNC_MODE` exposes the PLCopen-compatible public literals `mcShortest`, `mcCatchUp`, and `mcSlowDown`, but the current deterministic synchronization kernel maps them to the same alignment path until a later profile differentiates the physical catch-up/slow-down behavior.
  - In coordinated-motion group contexts, legacy `mcBlending*` values follow the truST error path and return `mcERR_NotSupported` until a future profile explicitly documents a mapping, even though Part 4 also permits treating them as `mcBuffered`.
  - The initial truST profile rejects override factors greater than `1.0` for `MC_SetOverride` and `MC_GroupSetOverride`, even though PLCopen allows vendor-specific behavior above `1.0`.
  - `MC_COORD_SYSTEM` uses prefixed public ST literals (`mcACS`, `mcMCS`, `mcWCS`, `mcPCS`, `mcFCS`, `mcTCS`) rather than the bare coordinate-system spellings shown in the Part 4 tables.
  - `MC_AXES_POS_REF` uses an array field (`Axes : ARRAY[...] OF REAL`) rather than the per-axis scalar example shown in Part 4 RFC section 4.2.
  - Recommended `MC_CONFIG_DATA` / `MC_TURN_INFO` field names follow truST house-style casing rather than the exact casing used in the Part 4 examples; ST identifiers remain case-insensitive.
- Impact:
  - Public truST source exposes a slightly richer or stricter profile than the bare PLCopen FB surface in these areas.
- Mitigation:
  - The extension/deviation points are documented in the motion spec, recorded in the compliance matrix, and locked by dedicated tests before support is claimed.

## 2026-04-12 - Homing-kernel deterministic simplifications

- Area: PLCopen Part 5 homing execution in the deterministic ST fixture kernel
- PLCopen reference: PLCopen Motion Control Part 5 v2.0
- Deviation:
  - The homing library package uses the same file-scope `VAR_GLOBAL` shared-state carrier pattern in `plcopen_motion_homing_globals.st` as the other motion-library packages; this remains an internal truST implementation device and is not part of the public PLCopen surface.
  - `MC_StepBlock.DetectionVelocityTime` is modeled as a consecutive-scan confirmation rule in the deterministic ST kernel: `TIME#0ms` completes as soon as the block condition is met, while a nonzero value requires the low-velocity/torque block condition on one additional active scan before completion.
- Impact:
  - The public FB surface remains PLCopen-shaped, but the deterministic test kernel expresses physical dwell-time behavior as scan-count confirmation instead of wall-clock timing.
- Mitigation:
  - The simplification is recorded here, reflected in the compliance matrix notes, and locked by dedicated Phase D ST tests.

## 2026-04-26 - OOP facade binding and unsupported command objects

- Area: PLCopen Motion OOP facade
- PLCopen reference: PLCopen "Application Examples for Motion Control - Porting into OOP" v1.0
- Deviation:
  - `MC_OopAxis.Bind(AxisId, InternalIndex)` is a truST-specific binding method. PLCopen's OOP example deliberately removes `AXIS_REF` from method signatures and expects vendors to add identification/binding in a vendor-specific way.
  - Unsupported OOP methods return command objects with `Error = TRUE` and `ErrorId = mcERR_NotSupported` instead of being omitted from the interface.
  - `itfContinousAxisCommand` is exposed as a compatibility alias for the PLCopen example spelling while `itfContinuousAxisCommand` is the canonical truST spelling.
- Impact:
  - OOP applications can bind object axes and compile against the full PLCopen OOP method surface, but unsupported methods must be checked through the returned command object status.
- Mitigation:
  - The OOP package guide documents the binding method and unsupported method behavior, and ST unit tests cover interface dispatch, command properties, classic-state delegation, and unsupported command-object returns.
