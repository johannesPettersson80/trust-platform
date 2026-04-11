# PLCopen Deviations Log

This file tracks known, intentional deviations/extensions from strict PLCopen profile behavior.

## 2026-04-11 - PLCopen motion profile extensions and stricter choices

- Area: PLCopen Motion library profile
- PLCopen reference: PLCopen Motion Control Part 1 v2.0 and Part 4 v2.0 RFC
- Deviation:
  - `MC_AXIS_STATUS` and `MC_GROUP_STATUS` are published as truST extension enum types; PLCopen itself standardizes the status surface primarily through boolean outputs on `MC_ReadStatus` and `MC_GroupReadStatus`.
  - `MC_Constants` is published as a truST ST helper FB for standardized Part 1 parameter IDs and the stable public `mcERR_*` namespace; PLCopen defines the IDs and classic `ErrorID : WORD` surface, but not this convenience carrier. In the current runtime, callers invoke `MC_Constants()` before reading members because the outputs are assigned in the FB body.
  - The single-axis ST fixture uses a file-scope `VAR_GLOBAL` block as an internal shared-state carrier for the motion kernel. That placement is a truST-specific implementation device, is not part of the public PLCopen surface, and is intentionally omitted from the compliance matrix.
  - In coordinated-motion group contexts, legacy `mcBlending*` values follow the truST error path and return `mcERR_NotSupported` until a future profile explicitly documents a mapping, even though Part 4 also permits treating them as `mcBuffered`.
  - The initial truST profile rejects override factors greater than `1.0` for `MC_SetOverride` and `MC_GroupSetOverride`, even though PLCopen allows vendor-specific behavior above `1.0`.
  - `MC_COORD_SYSTEM` uses prefixed public ST literals (`mcACS`, `mcMCS`, `mcWCS`, `mcPCS`, `mcFCS`, `mcTCS`) rather than the bare coordinate-system spellings shown in the Part 4 tables.
  - `MC_AXES_POS_REF` uses an array field (`Axes : ARRAY[...] OF REAL`) rather than the per-axis scalar example shown in Part 4 RFC section 4.2.
  - Recommended `MC_CONFIG_DATA` / `MC_TURN_INFO` field names follow truST house-style casing rather than the exact casing used in the Part 4 examples; ST identifiers remain case-insensitive.
- Impact:
  - Public truST source exposes a slightly richer or stricter profile than the bare PLCopen FB surface in these areas.
- Mitigation:
  - The extension/deviation points are documented in the motion spec, recorded in the compliance matrix, and locked by dedicated tests before support is claimed.
