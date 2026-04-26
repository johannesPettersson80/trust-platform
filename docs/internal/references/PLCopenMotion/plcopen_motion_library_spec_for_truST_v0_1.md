# PLCopen Motion Library for truST - Specification Draft v0.2

## 1. Purpose

This document defines a practical implementation profile for a PLCopen-based motion library targeting **truST**.

The primary design goal is:

- **normative public API:** classic PLCopen function blocks and classic PLCopen-compatible data types
- **second public API:** a shipped OOP single-axis facade for projects that prefer axis objects, interfaces, properties, and returned command objects
- **implementation freedom:** internal command-kernel, adapter, and planner architecture may be vendor-specific

The specification is organized so the team can implement the library in phases:

- **Phase A**: Single-axis core (PLCopen Part 1 profile)
- **Phase B**: Axis synchronization (cam/gear/phasing subset)
- **Phase C**: Coordinated motion groups (selected PLCopen Part 4 profile)
- **Phase D**: Homing extensions (selected PLCopen Part 5 profile)
- **Phase E**: OOP facade inspired by the PLCopen OOP example

This document does **not** currently claim full PLCopen certification coverage across all published motion-control parts. It defines the truST-selected profile and records what is required, deferred, or explicitly not targeted.

## 2. Source hierarchy

### 2.1 Normative sources for this draft

The normative behavior and naming rules in this draft are based on:

1. **PLCopen Motion Control Part 1, Version 2.0, Published (March 17, 2011)**  
   Single-axis motion, synchronization, base FB semantics, parameter numbers. This is the merged document formerly published as Part 1 and Part 2; the later multi-axis/synchronization sections in that merged publication remain normative for the truST Phase B profile.
2. **PLCopen Motion Control Part 4, Version 2.0 Release for Comments (dated November 18, 2025)**  
   Coordinated motion, transforms, group state model, command acceptance, transition model.
3. **PLCopen Motion Control Part 5, Version 2.0, Published (November 16, 2011)**  
   Homing toolkit and step-function-block behavior.

truST SHALL treat these exact revisions as the pinned baseline for this draft. Later PLCopen revisions SHALL NOT silently change this specification until the document is revised.

### 2.2 Informative sources

The following documents are informative and are used for examples, architecture hints, or future planning:

1. PLCopen Motion Control Part 3, Version 2.0 (application examples and UDFB patterns)
2. PLCopen "Application Examples for Motion Control - Porting into OOP", Version 1.0
3. PLCopen Motion Control Part 6, Version 2.0 (reviewed for scoping only; explicitly not targeted in Phases A-E)
4. truST platform documentation for runtime, debugger, PLCopen XML import, and configuration

The shipped Phase E single-axis facade uses the PLCopen OOP example as an interface and architecture source. The classic FB sources above remain the normative behavioral source for axis state transitions, command completion, parameters, and readback.

## 3. Public API stance

### 3.1 Normative public API

The **normative public contract SHALL be classic PLCopen function blocks and classic PLCopen-compatible public type names**.

Reasons:

- PLCopen Part 1, Part 4, and Part 5 standardize the FB interfaces and their externally observable behavior.
- PLCopen XML interoperability, ST portability, and vendor-example portability are strongest with the classic FB surface.
- Motion users typically expect scan-oriented `Execute`, `Enable`, `Busy`, `Done`, `Error`, `CommandAborted`, `Active`, and `Valid` semantics rather than command-object-only APIs.

### 3.2 Internal implementation freedom

Internally, truST MAY implement the library using:

- command kernels
- adapters
- planners
- interpolators
- transform pipelines
- OO helper abstractions

These internal design choices SHALL NOT change the normative classic FB behavior.

### 3.3 Shipped OOP facade

truST now ships an OOP single-axis facade because the runtime supports the required OO constructs (`INTERFACE`, `METHOD`, `PROPERTY`, `EXTENDS`, interface references, and command objects that remain valid across scan-like test cycles) well enough for the selected package scope.

That OOP facade is:

- **public** as a second API surface
- **normative for its documented single-axis package behavior**
- **derived from** the classic FB behavior rather than replacing it
- **not** the primary PLCopen certification surface

## 4. Compliance and scope model

### 4.1 PLCopen classification

The library SHALL preserve the PLCopen Basic / Extended / Vendor-specific distinction used by the selected source set. Part 4 v2.0 RFC Appendix 1 uses the explicit `B` / `E` / `V` compliance-statement columns; Part 1 and Part 5 primarily use `B` / `E`, and truST maps vendor-defined additions to `V` where needed.

- **B** = Basic in the source document
- **E** = Extended in the source document
- **V** = Vendor-specific in the source document or in truST

### 4.2 truST scope matrix

| Scope item | Primary source | truST v0.2 status | Notes |
| --- | --- | --- | --- |
| Single-axis core FBs | Part 1 v2.0 | Normative, targeted in Phase A | First public release surface |
| Axis synchronization FBs | Part 1 v2.0 | Normative, targeted in Phase B | Cam/gear first, phasing/combine later |
| Coordinated motion group FBs | Part 4 v2.0 RFC dated November 18, 2025 | Normative for the selected truST profile, targeted in Phase C | truST does not yet target the full Part 4 surface in one step |
| Homing toolkit FBs | Part 5 v2.0 | Normative for the selected truST profile, targeted in Phase D | `MC_Home` remains in Phase A |
| OOP facade | OOP Examples v1.0 | Shipped single-axis facade in Phase E | Second API over the classic FB behavior; never the primary compliance surface |
| Fluid power extensions | Part 6 v2.0 | Explicitly not targeted in Phases A-E | Revisit only if a concrete hydraulic/pneumatic use case appears |

### 4.3 Machine-readable compliance matrix

truST SHALL maintain a machine-readable compliance matrix with one row per public FB, enum, and public data type. Each row SHALL contain at least:

- `PublicName`
- `SourcePart`
- `SourceClass` (`B`, `E`, `V`)
- `truSTPhase`
- `truSTStatus` (`Required`, `Implemented`, `Deferred`, `Optional`, `NotTargeted`)
- `Notes`

Recommended profile columns:

- `Profile_A_AxisCore`
- `Profile_B_Sync`
- `Profile_C_GroupCore`
- `Profile_D_Homing`
- `Profile_E_OOPFacade`
- `PinnedSourceRevision`

If truST ships a placeholder for a deferred public FB or enum combination, that placeholder SHALL fail deterministically with `mcERR_NotSupported` rather than silently behaving differently. Deferred public names MAY also remain absent until implemented; the compliance matrix SHALL make that explicit.

## 5. Library package structure in truST

The recommended package layout is:

```text
PlcOpenMC/
  Types/
    MC_BaseTypes.st
    MC_Enums.st
    MC_AxisTypes.st
    MC_GroupTypes.st
    MC_ErrorTypes.st
  Core/
    _MC_AxisKernel.st
    _MC_GroupKernel.st
    _MC_CommandQueue.st
    _MC_ProfilePlanner.st
    _MC_Interpolator.st
    _MC_Kinematics.st
    _MC_Transforms.st
    _MC_ErrorMap.st
  FB_Axis/
    MC_Power.st
    MC_Home.st
    MC_Stop.st
    MC_Halt.st
    MC_MoveAbsolute.st
    ...
  FB_Sync/
    MC_CamTableSelect.st
    MC_CamIn.st
    MC_CamOut.st
    MC_GearIn.st
    ...
  FB_Group/
    MC_AddAxisToGroup.st
    MC_GroupEnable.st
    MC_MoveLinearAbsolute.st
    ...
  FB_Homing/
    MC_StepAbsoluteSwitch.st
    ...
  Adapters/
    MC_DriveAdapter_IF.st
    MC_SimAxisAdapter.st
    MC_SimGroupAdapter.st
  Tests/
    Test_AxisSemantics.st
    Test_Buffering.st
    Test_GroupSemantics.st
    Test_Kinematics.st
```

Internal POUs whose names begin with `_MC_` are **not public API**.

## 6. truST-specific implementation constraints

### 6.1 Project portability

The library SHOULD be written in a portable IEC 61131-3 ST subset suitable for PLCopen XML import/export. One public POU per PLCopen FB is recommended.

### 6.2 Numeric types

Public PLCopen-compatible FB interfaces SHALL use `REAL` where the PLCopen tables use `REAL`.

Internal planning, interpolation, transform, and kinematic calculations SHALL use `LREAL`.

This preserves API familiarity while improving numeric robustness for:

- long paths
- kinematic transforms
- circular interpolation
- blend calculations
- accumulated time-step integration

### 6.3 Runtime model

The implementation SHALL assume execution in a cyclic task with deterministic sample time `Ts`.

All motion planning SHALL be performed relative to the configured task interval. The motion kernel SHALL be agnostic to the scan period by reading `Ts` from one central configuration constant.

### 6.4 PLCopen XML interop

The build and verification workflow SHOULD preserve PLCopen XML round-trip friendliness. Public type names and FB names SHOULD remain exactly aligned with PLCopen naming unless a shortened-name profile is explicitly enabled.

## 7. Public reference and derived types

### 7.1 `AXIS_REF`

Per PLCopen Part 1, `AXIS_REF` is implementation dependent. For truST, the public type SHALL be a lightweight handle to a registered axis kernel.

Recommended definition:

```iecst
TYPE AXIS_REF :
STRUCT
    AxisId         : UDINT;   // Stable public handle
    InternalIndex  : UINT;    // Kernel registry index
END_STRUCT
END_TYPE
```

The content of `AXIS_REF` SHALL be treated as opaque outside the motion library.

### 7.2 `AXES_GROUP_REF`

The coordinated-motion equivalent SHALL also be a lightweight handle.

```iecst
TYPE AXES_GROUP_REF :
STRUCT
    GroupId        : UDINT;
    InternalIndex  : UINT;
END_STRUCT
END_TYPE
```

### 7.3 Additional public types required by the selected profiles

The following names SHALL exist as public types when the corresponding profile is enabled:

- `AXIS_ID` and `AXES_GROUP_ID`
  Vendor-specific public ID aliases used by the coordinated-motion membership and readback FBs. Recommended implementation for truST: `UDINT`.
- `MC_COMMAND_ID`  
  Vendor-specific identifier for buffered motions and queued administrative commands. Recommended implementation for truST: `UINT`, with `0` meaning "not yet accepted".
- `IDENT_IN_GROUP_REF`  
  Vendor-specific identifier used by the Part 4 group-membership and group-configuration FBs. Recommended implementation for truST: `STRING[63]`, carrying the stable member/kinematic name used inside the axes group.
- `MC_GROUP_PARAMETER`  
  Part 4 enum type used by `MC_GroupReadParameter` and `MC_GroupWriteParameter`. The initial standardized members are defined in Section `8.5`.
- `MC_TRANSITION_PARAMETER`  
  Vendor-specific additional transition/blending parameter used by the Part 4 group-motion FBs. Recommended implementation for truST: `REAL`.
- `MC_KIN_REF`  
  Vendor-specific kinematic-model reference used by `MC_SetKinTransform` and `MC_ReadKinTransform`. The current truST profile represents `MC_KIN_REF` as `UINT`.
- `MC_COORD_REF`  
  Vendor-specific coordinate-transform type. truST SHOULD use the same field names as `MC_CART_REF`.
- `MC_CAM_ID` and `MC_CAM_REF`  
  Vendor-specific identifiers/data references used by camming FBs in Phase B. The current truST profile represents `MC_CAM_ID` as `UINT`. The current public ST publication path for `MC_CAM_REF` is a fixed 8-point struct carrier with fields `CamId`, `NumberOfPairs`, `IsAbsolute`, `MasterPosition0..7`, and `SlavePosition0..7`.
- `MC_GROUP_SWLIMITS`  
  Vendor-specific grouped software-limit type for `MC_GroupReadSWLimits` and `MC_GroupWriteSWLimits`.
- `MC_PATH_REF` and `MC_PATH_DATA_REF`  
  Vendor-specific path description and prepared-path types for deferred path-motion features.
- `MC_DHParameters` and `MC_JointInfo`  
  Vendor-specific data returned by deferred kinematic introspection FBs.
- `MC_REF_SIGNAL_REF`  
  Vendor-specific reference-signal type used by the Part 5 homing-step FBs.

Recommended grouped software-limit helper structure:

```iecst
TYPE MC_SWLIMIT :
STRUCT
    SWLimitPos : REAL;
    SWLimitNeg : REAL;
END_STRUCT
END_TYPE
```

`MC_GROUP_SWLIMITS` SHOULD then be represented as:

```iecst
TYPE MC_GROUP_SWLIMITS : ARRAY[1..MC_MAX_GROUP_AXES] OF MC_SWLIMIT; END_TYPE
```

### 7.4 Classic `ErrorID`

For the classic FB API, all public FBs SHALL expose `ErrorID : WORD` exactly as the PLCopen classic documents do.

truST MAY define named public error constants such as `mcERR_InvalidParameter`, `mcERR_NotSupported`, and `mcERR_AxisGrouped`, but these constants SHALL map to stable `WORD` values.

The name `MC_ERROR` is reserved for the shipped OO facade only and SHALL NOT replace classic `ErrorID : WORD`.

## 8. Public enums and canonical data types

### 8.1 Core enums

The following enums SHALL be public in Phase A:

```iecst
TYPE MC_BUFFER_MODE : (
    mcAborting,
    mcBuffered,
    mcBlendingLow,
    mcBlendingPrevious,
    mcBlendingNext,
    mcBlendingHigh
); END_TYPE

TYPE MC_DIRECTION : (
    mcPositiveDirection,
    mcShortestWay,
    mcNegativeDirection,
    mcCurrentDirection
); END_TYPE

TYPE MC_EXECUTION_MODE : (
    mcImmediately,
    mcDelayed,
    mcQueued
); END_TYPE

TYPE MC_SOURCE : (
    mcCommandedValue,
    mcSetValue,
    mcActualValue
); END_TYPE
```

Notes:

- truST adopts the Part 4 three-value `MC_EXECUTION_MODE` enum.
- Part 1 single-axis administrative FBs define only the `{mcImmediately, mcQueued}` subset and do not define semantics for `mcDelayed`.
- In the initial Phase A profile, single-axis FBs that expose `ExecutionMode` SHALL reject `mcDelayed` with `mcERR_NotSupported` until a later profile explicitly enables it.
- Individual FBs MAY support only a subset of enum values; unsupported values SHALL return `mcERR_NotSupported` or the corresponding PLCopen-compatible FB error.

The following additional synchronization enums SHALL be public in Phase B:

```iecst
TYPE MC_START_MODE : (
    mcAbsolute,
    mcRelative,
    mcRampIn
); END_TYPE

TYPE MC_SYNC_MODE : (
    mcShortest,
    mcCatchUp,
    mcSlowDown
); END_TYPE
```

### 8.2 Coordinated-motion enums

The following enums SHALL be public in the initial Phase C core profile:

```iecst
TYPE MC_COORD_SYSTEM : (
    mcACS,
    mcMCS,
    mcWCS,
    mcPCS,
    mcFCS,
    mcTCS
); END_TYPE

TYPE MC_DYNAMICS_MODE : (
    mcAbsolute,
    mcPercentage
); END_TYPE

TYPE MC_TRANSITION_MODE : (
    mcTMNone,
    mcTMStartVelocity,
    mcTMConstantVelocity,
    mcTMCornerDistance,
    mcTMMaxCornerDeviation
); END_TYPE

TYPE MC_TRANSITION_VELOCITY : (
    mcTVZero,
    mcTVLow,
    mcTVPrevious,
    mcTVNext,
    mcTVHigh
); END_TYPE

TYPE MC_TRANSITION_REFERENCE : (
    mcStartPoint,
    mcEndPoint
); END_TYPE

TYPE MC_COMMAND_STATE : (
    mcAccepted,
    mcActive
); END_TYPE
```

The following additional Part 4 enum families SHALL remain reserved in the compliance matrix and become public only when their first consuming FBs enter scope:

```iecst
TYPE MC_CIRC_MODE : (
    mcBorder,
    mcCenter,
    mcRadius
); END_TYPE

TYPE MC_CIRC_PATHCHOICE : (
    mcClockwise,
    mcCounterClockwise
); END_TYPE

TYPE MC_ORIENTATION_MODE : (
    mcLinear,
    mcJointInterpolated,
    mcFixed,
    mcPathBased
); END_TYPE

TYPE MC_TOOL_SOURCE : (
    mcActive,
    mcSelected
); END_TYPE
```

### 8.3 Axis and group status enums

The classic PLCopen compatibility surface for status remains the boolean outputs of `MC_ReadStatus` and `MC_GroupReadStatus`.

truST SHALL expose enum-based status internally and MAY additionally publish the following enums as documented truST extension types:

```iecst
TYPE MC_AXIS_STATUS : (
    mcErrorStop,
    mcDisabled,
    mcStandstill,
    mcHoming,
    mcStopping,
    mcDiscreteMotion,
    mcContinuousMotion,
    mcSynchronizedMotion
); END_TYPE

TYPE MC_GROUP_STATUS : (
    mcGroupErrorStop,
    mcGroupDisabled,
    mcGroupStandby,
    mcGroupHoming,
    mcGroupStopping,
    mcGroupMoving
); END_TYPE
```

In truST ST source, public enum literals retain the `mc`, `mcTM`, and `mcTV` prefixes consistently even where Part 4 permits omission when values are enum-qualified. For `MC_COORD_SYSTEM`, Part 4 writes the table values in bare form (`ACS`, `MCS`, `WCS`, `PCS`, `FCS`, `TCS`); truST deliberately publishes the corresponding ST literals as `mcACS`, `mcMCS`, `mcWCS`, `mcPCS`, `mcFCS`, and `mcTCS` for naming consistency. `GroupStandby` is the Part 4 state-diagram/state-label spelling; `mcGroupStandby` is the corresponding truST ST literal. If a future OO facade prefers an alias such as `mcStandstill`, that alias SHALL be documented as an OO mapping and SHALL NOT replace the classic Part 4 public naming.

### 8.4 Canonical position, transform, tool, and payload-related structs

Recommended `MC_CART_REF` and `MC_COORD_REF`:

```iecst
TYPE MC_CART_REF :
STRUCT
    X  : REAL;
    Y  : REAL;
    Z  : REAL;
    RX : REAL;
    RY : REAL;
    RZ : REAL;
END_STRUCT
END_TYPE

TYPE MC_COORD_REF :
STRUCT
    X  : REAL;
    Y  : REAL;
    Z  : REAL;
    RX : REAL;
    RY : REAL;
    RZ : REAL;
END_STRUCT
END_TYPE
```

Recommended `MC_POS_REF` family:

```iecst
TYPE MC_CONFIG_DATA :
STRUCT
    ConfigValid : BOOL;
    Shoulder    : BOOL;
    Elbow       : BOOL;
    Wrist       : BOOL;
END_STRUCT
END_TYPE

TYPE MC_TURN_INFO :
STRUCT
    ATurns : ARRAY[1..MC_MAX_ROTARY_AXES] OF SINT;
END_STRUCT
END_TYPE

TYPE MC_CART_POS_REF :
STRUCT
    Tcp           : MC_CART_REF;
    Cfg           : MC_CONFIG_DATA;
    TurnInfo      : MC_TURN_INFO;
    AuxiliaryAxes : ARRAY[1..MC_MAX_AUX_AXES] OF REAL;
END_STRUCT
END_TYPE

TYPE MC_AXES_POS_REF :
STRUCT
    Axes : ARRAY[1..MC_MAX_GROUP_AXES] OF REAL;
END_STRUCT
END_TYPE

TYPE MC_POS_REF :
STRUCT
    C : MC_CART_POS_REF;
    A : MC_AXES_POS_REF;
END_STRUCT
END_TYPE
```

`MC_DISTANCE_REF` SHALL be an alias or equivalent structure for relative moves.

Part 4 v2.0 RFC section 4.2 illustrates `MC_AXES_POS_REF` with per-axis scalar fields and uses lowercase-leading example field names such as `cfg`, `turnInfo`, `shoulder`, and `aTurns`. truST adopts the array shape above plus consistent house-style casing for the recommended representation; these structures remain vendor-specific and ST identifier resolution is case-insensitive.

Recommended rigid-body and tool structures:

```iecst
TYPE MC_RIGID_BODY_DYNAMIC_REF :
STRUCT
    CenterOfGravity : MC_CART_REF;
    Mass            : REAL;
    IX              : REAL;
    IY              : REAL;
    IZ              : REAL;
END_STRUCT
END_TYPE

TYPE MC_TOOL_REF :
STRUCT
    ToolFrame        : MC_COORD_REF;
    RigidBodyDynamic : MC_RIGID_BODY_DYNAMIC_REF;
END_STRUCT
END_TYPE
```

Payload data in Part 4 SHALL be modeled via payload numbers plus `MC_RIGID_BODY_DYNAMIC_REF`. truST SHALL NOT introduce `MC_PAYLOAD_REF` as if it were a PLCopen Part 4 public type.

### 8.5 Standard Part 4 group parameters

The initial Phase C profile SHALL expose the standardized Part 4 group-parameter enum:

```iecst
TYPE MC_GROUP_PARAMETER : (
    mcDynamicsMode,
    mcTransitionReferencePoint
); END_TYPE
```

The initial standardized parameter table is:

| Parameter name | Datatype | B/E | R/W |
| --- | --- | --- | --- |
| `mcDynamicsMode` | `MC_DYNAMICS_MODE` | B | E |
| `mcTransitionReferencePoint` | `MC_TRANSITION_REFERENCE` | B | E |

Vendors MAY extend `MC_GROUP_PARAMETER`, but vendor-defined parameter names SHALL NOT start with `mc`.

### 8.6 Standard Part 1 parameter numbers

For `MC_ReadParameter`, `MC_ReadBoolParameter`, `MC_WriteParameter`, and `MC_WriteBoolParameter`, truST SHALL preserve the standard Part 1 parameter numbers:

| PN | Name | Datatype | B/E | R/W |
| --- | --- | --- | --- | --- |
| 1 | `CommandedPosition` | `REAL` | B | R |
| 2 | `SWLimitPos` | `REAL` | E | R/W |
| 3 | `SWLimitNeg` | `REAL` | E | R/W |
| 4 | `EnableLimitPos` | `BOOL` | E | R/W |
| 5 | `EnableLimitNeg` | `BOOL` | E | R/W |
| 6 | `EnablePosLagMonitoring` | `BOOL` | E | R/W |
| 7 | `MaxPositionLag` | `REAL` | E | R/W |
| 8 | `MaxVelocitySystem` | `REAL` | E | R |
| 9 | `MaxVelocityAppl` | `REAL` | B | R/W |
| 10 | `ActualVelocity` | `REAL` | B | R |
| 11 | `CommandedVelocity` | `REAL` | B | R |
| 12 | `MaxAccelerationSystem` | `REAL` | E | R |
| 13 | `MaxAccelerationAppl` | `REAL` | E | R/W |
| 14 | `MaxDecelerationSystem` | `REAL` | E | R |
| 15 | `MaxDecelerationAppl` | `REAL` | E | R/W |
| 16 | `MaxJerkSystem` | `REAL` | E | R |
| 17 | `MaxJerkAppl` | `REAL` | E | R/W |

The public ST library SHALL publish these standardized parameter numbers through `FUNCTION_BLOCK MC_Constants`, using the stable member names `PN_CommandedPosition`, `PN_SWLimitPos`, `PN_SWLimitNeg`, `PN_EnableLimitPos`, `PN_EnableLimitNeg`, `PN_EnablePosLagMonitoring`, `PN_MaxPositionLag`, `PN_MaxVelocitySystem`, `PN_MaxVelocityAppl`, `PN_ActualVelocity`, `PN_CommandedVelocity`, `PN_MaxAccelerationSystem`, `PN_MaxAccelerationAppl`, `PN_MaxDecelerationSystem`, `PN_MaxDecelerationAppl`, `PN_MaxJerkSystem`, and `PN_MaxJerkAppl`.

When the single-axis motion FBs consume this parameter plane, enabled software limits clamp accepted position targets rather than rejecting the command outright. In the initial Phase A profile this clamp applies to `MC_Home`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveContinuousAbsolute`, and `MC_MoveContinuousRelative`; disabling the corresponding BOOL limit parameter removes the clamp.

For the standardized Part 1 parameter range, `MC_ReadBoolParameter` and `MC_WriteBoolParameter` SHALL accept `PN = 4`, `PN = 5`, and `PN = 6`.

Vendor-specific parameter numbers greater than `999` MAY also be BOOL-typed. When truST exposes such vendor-specific BOOL parameters in its parameter registry, `MC_ReadBoolParameter` and `MC_WriteBoolParameter` SHALL accept them as well.

### 8.7 Phase D homing enums and references

The following additional public types SHALL exist when the Phase D homing profile is enabled:

```iecst
TYPE MC_HOME_DIRECTION : (
    mcPositiveDirection,
    mcNegativeDirection,
    mcSwitchPositive,
    mcSwitchNegative
); END_TYPE

TYPE MC_SWITCH_MODE : (
    mcOn,
    mcOff,
    mcRisingEdge,
    mcFallingEdge,
    mcEdgeSwitchPositive,
    mcEdgeSwitchNegative
); END_TYPE
```

Notes:

- `MC_HOME_DIRECTION` is distinct from the Phase A `MC_DIRECTION` enum and SHALL NOT be collapsed into it.
- `MC_REF_SIGNAL_REF` is a vendor-specific public type required by the Part 5 step-homing FBs and SHALL resolve when the Phase D profile is enabled.

## 9. Units and scaling

The library SHALL follow the PLCopen technical-unit model:

- position in `[u]`
- velocity in `[u/s]`
- acceleration in `[u/s^2]`
- jerk in `[u/s^3]`

For group dynamics, the implementation SHALL also support percentage-based dynamics where configured by the group parameter `mcDynamicsMode`, in line with Part 4.

The following configuration SHALL exist per axis/group:

```iecst
TYPE MC_DYNAMIC_LIMITS :
STRUCT
    MaxVelocity     : LREAL;
    MaxAcceleration : LREAL;
    MaxDeceleration : LREAL;
    MaxJerk         : LREAL;
END_STRUCT
END_TYPE
```

For `MC_SetOverride` and `MC_GroupSetOverride`, the standard factor range is `0.0 .. 1.0`.

The initial truST profile SHALL apply the following rules:

- values above `1.0` are vendor-specific in PLCopen and SHALL be rejected unless a future profile explicitly enables them and records that choice in the PLCopen decision/deviation log
- values below `0.0` are invalid and SHALL be rejected
- `AccFactor = 0.0` and `JerkFactor = 0.0` are invalid and SHALL be rejected
- `VelFactor = 0.0` SHALL stop the axis or axes group without forcing the final state to `Standstill` or `GroupStandby`

## 10. Execution semantics

### 10.1 General input rules

For all FBs, the implementation SHALL preserve the Part 1 general rules:

- `Execute` commands act on the rising edge.
- `Enable` commands are level-sensitive.
- If an FB input is left open, the previous invocation value of that FB instance is used; on the first invocation, the IEC 61131-3 initial value is used.
- `Acceleration`, `Deceleration`, and `Jerk` are always positive quantities.
- `Velocity`, `Position`, and `Distance` may be positive or negative where the applicable FB permits.

truST-specific choice for zero dynamic inputs:

- If `Acceleration = 0`, `Deceleration = 0`, or `Jerk = 0`, truST SHALL use the configured axis/group maximum value rather than treating zero as an immediate FB error.

### 10.2 `ContinuousUpdate`

Where Part 1 defines `ContinuousUpdate`, the implementation SHALL:

- latch the initial command on the rising edge of `Execute`
- apply changed input values only while `ContinuousUpdate = TRUE` and the FB is still `Busy`
- keep relative distances referenced to the original command-start condition
- treat `ContinuousUpdate` as affecting the ongoing movement only; it is not a retrigger of `Execute`

Additional truST rule:

- if a buffered FB has been accepted but is not yet `Active`, changes made while it is still queued SHALL NOT alter the accepted queued command; updated values become relevant only after a new `Execute`

### 10.3 Level-triggered `Enable`

For all read/status/monitor FBs with `Enable`, outputs SHALL remain continuously refreshable while `Enable = TRUE`.

### 10.4 `MC_EXECUTION_MODE`

For administrative/group FBs that expose `ExecutionMode`, the meanings SHALL be:

- `mcImmediately`: functionality becomes valid immediately and may influence on-going motion without itself becoming a motion command
- `mcDelayed`: functionality becomes valid when the current motion command completes
- `mcQueued`: functionality becomes valid when all previous commands in front of it are done; this applies to buffered motion commands and queued administrative commands

### 10.5 Parameter-plane writes to dynamic maxima

The initial truST profile distinguishes between zero-valued FB motion inputs and writes to the parameter plane.

For writes through `MC_WriteParameter` to the application dynamic-limit parameters `MaxVelocityAppl`, `MaxAccelerationAppl`, `MaxDecelerationAppl`, and `MaxJerkAppl`, the initial truST profile SHALL reject values less than or equal to `0` with `mcERR_InvalidParameter`.

## 11. Output semantics

### 11.1 Execute-style FBs

For FBs with `Execute`, the implementation SHALL preserve PLCopen timing and exclusivity rules:

- `Busy`, `Done`, `Error`, and `CommandAborted` are mutually exclusive.
- `Active`, `Done`, `Error`, and `CommandAborted` are mutually exclusive on execute-style FBs that expose `Active`.
- `Busy` is set on the rising edge of `Execute`.
- `Busy` resets when one of `Done`, `Error`, or `CommandAborted` is set.
- `Done` is not set when a motion is interrupted before reaching its final goal.
- `Done`, `Error`, `ErrorID`, and `CommandAborted` SHALL remain observable for at least one scan of the calling task even if `Execute` has already gone low.
- For buffered FBs, `Active` goes TRUE only when the FB actually takes control of the axis or axes group.

### 11.2 Enable-style FBs

For FBs with `Enable`:

- `Valid` and `Error` SHALL be mutually exclusive.
- outputs SHALL reset on the falling edge of `Enable` as soon as practical
- `Enabled` outputs, where defined, SHALL reflect the level-triggered model rather than execute-edge behavior
- `MC_Power.Status` is not part of the falling-edge reset rule that applies to `Valid`, `Enabled`, `Busy`, `Error`, and `ErrorID`; it SHALL reflect the effective power-stage state independently of the `Enable` edge.

### 11.3 In-state outputs

`InVelocity`, `InEndVelocity`, `InGear`, `InTorque`, and `InSync` SHALL reflect the **internal setpoint**, not the actual measured value.

As long as the FB is `Active`, an `Inxxx` output:

- is set when the set value equals the commanded value
- is reset if the set value later diverges from the commanded value
- remains updated even if `Execute = FALSE`, as long as `Active = TRUE` and `Busy = TRUE`
- is reset when `CommandAborted` occurs

### 11.4 `Active` concurrency exceptions

For one axis, only one FB can normally be `Active` at a time. The classic Part 1 exceptions are FBs intended to work in parallel, including:

- `MC_MoveSuperimposed`
- `MC_HaltSuperimposed`
- `MC_PhasingAbsolute`
- `MC_PhasingRelative`

### 11.5 `MC_Stop` signature note

truST follows the Part 1 v2.0 `MC_Stop` FB tables and does not expose a public `Active` output on `MC_Stop`.

The Part 1 section 2.4.1 wording about simultaneous `Active = TRUE` and `Done = TRUE` for `MC_Stop` is treated as a textual inconsistency in that source and SHALL NOT change the selected truST public signature.

## 12. Error model

### 12.1 Error categories

The implementation SHALL distinguish at least:

1. **FB instance errors**
2. **Axis or group state errors**
3. **Backend/drive/communication errors**
4. **Kinematic/transform errors**
5. **Unsupported-profile errors**

### 12.2 Resulting state changes

Not every FB instance error SHALL force the axis/group into `ErrorStop` / `GroupErrorStop`.

Typical examples:

- parameter out of range -> FB `Error = TRUE`, no mandatory axis state change
- drive fault -> axis `ErrorStop`
- invalid inverse kinematic solution -> group `GroupErrorStop`
- unsupported feature -> FB `Error = TRUE`, no mandatory axis/group state change

### 12.3 Public error-code namespace

The library SHALL define a stable `WORD` error-code namespace with ranges such as:

- `16#0001..16#00FF` generic FB errors
- `16#0100..16#01FF` axis-state violations
- `16#0200..16#02FF` backend/drive errors
- `16#0300..16#03FF` kinematic/transform errors
- `16#0400..16#04FF` queue/buffer errors
- `16#0500..16#05FF` unsupported feature/profile errors

Mandatory named constants:

- `mcERR_None`
- `mcERR_InvalidParameter`
- `mcERR_InvalidState`
- `mcERR_NotSupported`
- `mcERR_QueueFull`
- `mcERR_AxisGrouped`
- `mcERR_GroupDisabled`
- `mcERR_GroupNotReady`
- `mcERR_KinematicNoSolution`
- `mcERR_KinematicSingularity`
- `mcERR_BackendFault`
- `mcERR_NotHomed`
- `mcERR_NotPowered`

The public ST library SHALL publish these named values through `FUNCTION_BLOCK MC_Constants` with the following stable `WORD` assignments:

| Name | Value |
| --- | --- |
| `mcERR_None` | `16#0000` |
| `mcERR_InvalidParameter` | `16#0001` |
| `mcERR_InvalidState` | `16#0100` |
| `mcERR_AxisGrouped` | `16#0101` |
| `mcERR_GroupDisabled` | `16#0102` |
| `mcERR_GroupNotReady` | `16#0103` |
| `mcERR_NotHomed` | `16#0104` |
| `mcERR_NotPowered` | `16#0105` |
| `mcERR_BackendFault` | `16#0200` |
| `mcERR_KinematicNoSolution` | `16#0300` |
| `mcERR_KinematicSingularity` | `16#0301` |
| `mcERR_QueueFull` | `16#0400` |
| `mcERR_NotSupported` | `16#0500` |

Axis-level error status and group-level error status SHALL remain distinct from per-FB `ErrorID`; that separation is surfaced via `MC_ReadAxisError` and `MC_GroupReadError`.

## 13. Axis state model

The axis kernel SHALL implement the Part 1 state machine:

- `Disabled`
- `Standstill`
- `Homing`
- `Stopping`
- `DiscreteMotion`
- `ContinuousMotion`
- `SynchronizedMotion`
- `ErrorStop`

The axis kernel SHALL ensure that motion commands are taken sequentially.

Required state rules:

- `MC_Stop` holds the axis in `Stopping` while `Execute = TRUE`; transition to `Standstill` requires `MC_Stop.Done = TRUE` and `MC_Stop.Execute = FALSE`.
- `MC_Halt` moves the axis into `DiscreteMotion` until velocity is zero; when `MC_Halt.Done = TRUE`, the axis transitions to `Standstill`.
- `MC_Reset` SHALL leave `ErrorStop` through one of the classic paths:
  - `ErrorStop -> Disabled` if power is not enabled
  - `ErrorStop -> Standstill` if power is enabled and the axis is ready to stand still
- Outside `ErrorStop`, the initial truST profile treats `MC_Reset` as a deterministic no-op command: it SHALL not change the axis state and SHALL complete without error if the backend accepts the request. Part 1 explicitly leaves `MC_Reset` outside `ErrorStop` vendor-specific.
- `MC_Home`, when started in `Standstill`, SHALL complete in `Standstill`. The profile SHALL not imply that every allowed non-standstill entry state returns to `Standstill`; those entry-state-dependent cases SHALL be tested explicitly.
- Administrative/read/status FBs SHALL NOT, by themselves, change the motion state.
- `MC_MoveSuperimposed` only changes state from `Standstill` to `DiscreteMotion`; from other states it changes the commanded motion without changing the state name.
- `MC_GearOut` and `MC_CamOut`, when implemented, SHALL transfer the axis from `SynchronizedMotion` to `ContinuousMotion`; using them outside the synchronized state is an FB/state error.
- Passive/flying homing FBs, if implemented later, SHALL be callable without forcing the axis into the `Homing` state.

Examples of no-state-change FBs include:

- read/status FBs
- parameter read/write FBs
- `MC_SetPosition`
- `MC_SetOverride`
- `MC_AbortTrigger`
- `MC_TouchProbe`
- `MC_DigitalCamSwitch`
- `MC_CamTableSelect`
- `MC_PhasingAbsolute`
- `MC_PhasingRelative`
- `MC_HaltSuperimposed`

## 14. Group state model

The group kernel SHALL implement the Part 4 state machine:

- `GroupDisabled`
- `GroupStandby`
- `GroupHoming`
- `GroupMoving`
- `GroupStopping`
- `GroupErrorStop`

The implementation SHALL preserve these behaviors:

- group creation begins in `GroupDisabled`
- `MC_GroupEnable` moves the group toward `GroupStandby`
- `MC_GroupPower` does not, by itself, change the group state
- If `MC_GroupPower.Enable = FALSE` causes group power to be removed while the group is active, the initial truST profile SHALL treat that loss of power as a power-failure path into `GroupErrorStop`; it is not treated as a silent no-op during motion.
- moving FBs transition the group to `GroupMoving`
- `MC_GroupStop` transitions to `GroupStopping`
- `GroupStopping` returns to `GroupStandby` only when `Done = TRUE` and `Execute = FALSE`
- `MC_GroupHome` accepts only `mcAborting` and `mcBuffered`
- `MC_GroupReset` leaves `GroupErrorStop`

Required differences between halt/stop/interrupt behavior:

- `MC_GroupStop` is non-bufferable, path-oriented, blocks follow-on motion while active, and cancels synchronization/tracking relationships.
- `MC_GroupHalt` is the normal-operation controlled stop; it is bufferable/abortable by later motion and may return to `GroupMoving` instead of `GroupStandby` when the active coordinate system is a dynamic PCS.
- If `MC_GroupInterrupt` / `MC_GroupContinue` are implemented later, they SHALL preserve suspended motion context rather than report `CommandAborted` on the interrupted movement FB.

## 15. Mandatory implementation choice: single-axis commands on grouped axes

PLCopen Part 4 leaves the behavior of issuing single-axis commands to grouped axes open. truST SHALL choose one behavior explicitly.

**Chosen behavior for truST v0.2: option 1 - not allowed.**

Therefore:

- If an axis belongs to an enabled group and the group is not `GroupDisabled`, any single-axis motion FB issued to that axis SHALL return `mcERR_AxisGrouped`.
- Read-only administrative FBs remain allowed.
- Group-level motion SHALL be the only legal motion path while the axis is grouped.

This choice is made for deterministic semantics, simpler testing, and lower risk.

## 16. Buffering and command queue semantics

### 16.1 Single-axis profile

The Phase A axis profile SHALL implement the full Part 1 `MC_BUFFER_MODE` enum:

- `mcAborting`
- `mcBuffered`
- `mcBlendingLow`
- `mcBlendingPrevious`
- `mcBlendingNext`
- `mcBlendingHigh`

### 16.2 Queue model

Per axis or group, the kernel SHALL provide:

- exactly one active motion command
- zero or more buffered commands
- deterministic FIFO ordering among buffered commands

### 16.3 Buffered command cleanup

If the controlled axis or group enters an error-stop state, all queued/buffered commands SHALL be cleared and SHALL report an error condition compatible with PLCopen behavior.

### 16.4 Targeted support rules for the selected profile

The detailed per-FB support table SHALL live in the machine-readable compliance matrix. For the selected truST profile, the following minimum rules apply:

| FB family | Required support rule |
| --- | --- |
| `MC_Stop` / `MC_GroupStop` | No `BufferMode` input; implicitly aborting; stop-hold behavior while `Execute = TRUE` |
| `MC_Home` | `mcBuffered` supported where the Part 1 signature exposes `BufferMode` |
| `MC_GroupHome` | Only `mcAborting` and `mcBuffered` are accepted |
| `MC_Halt` / `MC_GroupHalt` | Buffered execution supported |
| `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveVelocity`, `MC_MoveContinuousAbsolute`, `MC_MoveContinuousRelative` | Buffered operation supported per Part 1 |
| Group linear/direct motion FBs | Use the Part 4 buffer/transition model described below |

## 17. Coordinated-motion blend model

Because Part 4 separates queueing from transition semantics, the coordinated-motion kernel SHALL internally normalize each command into:

```text
QueuePolicy
TransitionVelocity
TransitionMode
TransitionParameter
TransitionReferencePoint
```

### 17.1 Supported enum categories

The category split SHALL be:

- `MC_BUFFER_MODE`: `mcAborting`, `mcBuffered`; legacy `mcBlending*` values remain defined for Part 1 compatibility but are obsolete for new coordinated-motion implementations
- `MC_TRANSITION_MODE`: `mcTMNone`, `mcTMStartVelocity`, `mcTMConstantVelocity`, `mcTMCornerDistance`, `mcTMMaxCornerDeviation`
- `MC_TRANSITION_VELOCITY`: `mcTVZero`, `mcTVLow`, `mcTVPrevious`, `mcTVNext`, `mcTVHigh`
- `MC_TRANSITION_REFERENCE`: `mcStartPoint`, `mcEndPoint`

`MC_TRANSITION_VELOCITY` SHALL only be evaluated when `BufferMode` is `mcAborting` or `mcBuffered`.

### 17.2 Phase C supported subset

For the first coordinated-motion release, truST SHALL support at least:

- `MC_BUFFER_MODE`: `mcAborting`, `mcBuffered`
- `MC_TRANSITION_MODE`: `mcTMNone`, `mcTMCornerDistance`
- `MC_TRANSITION_VELOCITY`: `mcTVZero`, `mcTVNext`
- `MC_TRANSITION_REFERENCE`: `mcStartPoint`

Other transition values MAY be parsed but SHALL return `mcERR_NotSupported` until implemented. Part 4 permits legacy `mcBlending*` values in group contexts either to behave like `mcBuffered` or to raise a warning/error; the initial truST profile chooses the error path, and these values SHALL return `mcERR_NotSupported` until truST explicitly documents a mapping.

### 17.3 Full target set for later release

The longer-term target set is:

- transition velocities through `mcTVHigh`
- transition modes through `mcTMMaxCornerDeviation`
- transition reference points through `mcEndPoint`

Legacy `mcBlending*` values remain obsolete compatibility values, not a coordinated-motion target set.

When future TCS-based buffered motion is supported, buffered commands in `mcTCS` SHALL evaluate the TCS at the end of the previous commanded movement, in line with Part 4.

## 18. `CommandAccepted` and `CommandID`

### 18.1 Availability

`CommandAccepted` and `CommandID` SHALL be exposed only on the FBs for which the selected PLCopen source document defines them. truST SHALL NOT add them blindly to every coordinated-motion FB.

Examples:

- many queued or buffered Part 4 motion/administrative FBs expose `CommandAccepted`
- many queued or buffered Part 4 motion/administrative FBs expose `CommandID`
- some FBs expose `CommandAccepted` without `CommandID`
- some FBs, such as `MC_MovePath`, do not expose either in the current Part 4 draft

The compliance matrix SHALL record this per FB.

### 18.2 Semantics

- `CommandAccepted = TRUE` means the command has been accepted into the motion queue/backend.
- `CommandID = 0` means the command has not yet been accepted.
- `CommandID` is valid on the FB output while `CommandAccepted = TRUE`.
- After output handshake completion, the system MAY still retain the command identity for `MC_GroupReadCommandInfo` or `MC_GroupReadMotionState.ActiveCommandID`; the output signal lifetime and the retained-query lifetime are not identical.

### 18.3 `MC_COMMAND_STATE`

When `MC_GroupReadCommandInfo` is implemented, it SHALL expose:

- `mcAccepted`
- `mcActive`
- `ElapsedDuration`
- `RemainingDuration`
- `RemainingDistance`
- `Progress`
- `InfoID`
- `WarningID`

### 18.4 Recommended axis extension

truST MAY later offer `CommandAccepted` / `CommandID` as a vendor extension on selected single-axis FBs as well, because asynchronous backends can make this useful outside the group profile. This extension SHALL NOT change the classic PLCopen baseline signatures.

## 19. Public FB profiles

### 19.1 Phase A - Axis Core

Required FBs:

- `MC_Power`
- `MC_Home`
- `MC_Stop`
- `MC_Halt`
- `MC_MoveAbsolute`
- `MC_MoveRelative`
- `MC_MoveAdditive`
- `MC_MoveVelocity`
- `MC_MoveContinuousAbsolute`
- `MC_MoveContinuousRelative`
- `MC_SetPosition`
- `MC_SetOverride`
- `MC_ReadActualPosition`
- `MC_ReadActualVelocity`
- `MC_ReadActualTorque`
- `MC_ReadStatus`
- `MC_ReadMotionState`
- `MC_ReadAxisInfo`
- `MC_ReadAxisError`
- `MC_Reset`
- `MC_ReadParameter`
- `MC_ReadBoolParameter`
- `MC_WriteParameter`
- `MC_WriteBoolParameter`

Deferred but reserved:

- `MC_MoveSuperimposed`
- `MC_HaltSuperimposed`
- `MC_TorqueControl`
- `MC_PositionProfile`
- `MC_VelocityProfile`
- `MC_AccelerationProfile`
- `MC_ReadDigitalInput`
- `MC_ReadDigitalOutput`
- `MC_WriteDigitalOutput`
- `MC_DigitalCamSwitch`
- `MC_TouchProbe`
- `MC_AbortTrigger`

For the current Phase A release, every deferred single-axis FB above follows the `absent` path rather than a runtime placeholder; the compliance matrix SHALL record that explicitly for each row.

Phase A signature notes:

- `MC_Power` SHALL expose `Status`; `Status` is not the same signal as `Valid`.
- `MC_Power.EnablePositive` and `MC_Power.EnableNegative` are Part 1 Extended inputs but are not part of the initial Phase A public signature unless a later profile explicitly enables them and updates the compliance matrix.
- `MC_ReadMotionState` SHALL expose `Source : MC_SOURCE` plus the motion-state outputs `ConstantVelocity`, `Accelerating`, `Decelerating`, `DirectionPositive`, and `DirectionNegative`.
- `MC_Home` SHALL follow the Part 1 v2.0 classic signature for the selected profile unless truST later introduces a documented extension.
- `MC_Stop` SHALL follow the Part 1 v2.0 FB tables for the selected profile and therefore does not expose `Active` in the initial truST public signature.
- `MC_SetOverride` is an enable-style FB with `Enabled` semantics, not a simple execute/done write command.
- `MC_MoveRelative` and `MC_MoveAdditive` SHALL remain distinct:
  - `MC_MoveRelative` adds `Distance` to the position captured at command start.
  - `MC_MoveAdditive` adds to the currently commanded position.
- `MC_MoveVelocity` SHALL expose `InVelocity`.
- `MC_MoveContinuousAbsolute` and `MC_MoveContinuousRelative` SHALL expose `InEndVelocity`.

### 19.2 Phase B - Synchronization

Required FBs:

- `MC_CamTableSelect`
- `MC_CamIn`
- `MC_CamOut`
- `MC_GearIn`
- `MC_GearOut`
- `MC_GearInPos`

Optional in later synchronization releases:

- `MC_PhasingAbsolute`
- `MC_PhasingRelative`
- `MC_CombineAxes`

Phase B notes:

- `MC_CamTableSelect` SHALL prepare/select the cam data needed by `MC_CamIn`.
- In the current truST Phase B profile, `MC_CamTableSelect` rejects `MC_EXECUTION_MODE = mcDelayed` with `mcERR_NotSupported`.
- Where the Part 1 v2.0 signatures define them, v2.0 additions such as `MasterValueSource`, `StartMode`, or cyclic-update behavior SHALL be preserved.
- `MC_CamOut` and `MC_GearOut` leave synchronized motion; they do not imply standstill.
- The current Phase B deferred set (`MC_PhasingAbsolute`, `MC_PhasingRelative`, `MC_CombineAxes`) uses the `absent` path only; these names are intentionally not published as runtime placeholders in the current public ST surface.

### 19.3 Phase C - Group Core

Required FBs for the selected truST Part 4 profile:

- `MC_AddAxisToGroup`
- `MC_RemoveAxisFromGroup`
- `MC_UngroupAllAxes`
- `MC_GroupReadConfiguration`
- `MC_ReadAxisGroupInfo`
- `MC_GroupEnable`
- `MC_GroupDisable`
- `MC_GroupPower`
- `MC_GroupHome`
- `MC_SetKinTransform`
- `MC_SetCartesianTransform`
- `MC_SetCoordinateTransform`
- `MC_ReadKinTransform`
- `MC_ReadCartesianTransform`
- `MC_ReadCoordinateTransform`
- `MC_GroupSetPosition`
- `MC_GroupReadPosition`
- `MC_GroupReadVelocity`
- `MC_GroupReadAcceleration`
- `MC_GroupReadMotionState`
- `MC_GroupReadCommandInfo`
- `MC_GroupReadParameter`
- `MC_GroupWriteParameter`
- `MC_GroupWriteReferenceDynamics`
- `MC_GroupReadReferenceDynamics`
- `MC_GroupWriteDefaultDynamics`
- `MC_GroupReadDefaultDynamics`
- `MC_GroupReadStatus`
- `MC_GroupReadError`
- `MC_GroupReset`
- `MC_GroupReadSWLimits`
- `MC_GroupWriteSWLimits`
- `MC_GroupStop`
- `MC_GroupHalt`
- `MC_MoveLinearAbsolute`
- `MC_MoveLinearRelative`
- `MC_MoveDirectAbsolute`
- `MC_MoveDirectRelative`
- `MC_GroupWaitTime`
- `MC_GroupSetOverride`
- `MC_TransformPosition`

Deferred to later coordinated releases:

- `MC_MoveCircularAbsolute`
- `MC_MoveCircularRelative`
- `MC_PathSelect`
- `MC_MovePath`
- `MC_GroupInterrupt`
- `MC_GroupContinue`
- `MC_ReadDHParameters`
- `MC_ReadJointInfo`
- `MC_GroupJog`
- `MC_GroupJogVector`
- `MC_GroupWriteJoggingDynamics`
- `MC_GroupReadJoggingDynamics`
- `MC_WriteToolData`
- `MC_ReadToolData`
- `MC_SelectTool`
- `MC_ReadTool`
- `MC_WritePayloadData`
- `MC_ReadPayloadData`
- `MC_SelectPayload`
- `MC_ReadPayload`
- `MC_ReadRigidBodyDynamic`
- `MC_WriteRigidBodyDynamic`

Phase C notes:

- `MC_AddAxisToGroup`, `MC_RemoveAxisFromGroup`, `MC_GroupReadConfiguration`, and `MC_ReadAxisGroupInfo` SHALL preserve `IdentInGroup : IDENT_IN_GROUP_REF`.
- `MC_GroupHome` SHALL only accept `mcAborting` and `mcBuffered`.
- `MC_GroupStop` and `MC_GroupHalt` SHALL preserve the distinct Part 4 behaviors documented in Section 14.
- `MC_GroupReadMotionState` SHALL expose `Tracking`, `InSync`, `InPosition`, `Standstill`, `ConstantVelocity`, `Accelerating`, `Decelerating`, and `ActiveCommandID`; these outputs are defined relative to the active coordinate system.
- `MC_GroupReadCommandInfo` SHALL be the supported query path for retained command metadata; `CommandID` output lifetime alone is not sufficient. It SHALL expose `CommandState`, `ElapsedDuration`, `RemainingDuration`, `RemainingDistance`, `Progress`, `InfoID`, and `WarningID`.
- `MC_GroupReadParameter` and `MC_GroupWriteParameter` SHALL expose `ParameterNumber : MC_GROUP_PARAMETER`. In the initial standardized Phase C surface, that includes at least `mcDynamicsMode` and `mcTransitionReferencePoint`.

### 19.4 Phase C.1 - Tracking and synchronization subset

Required if robot/conveyor scenarios are needed.
The current shipped truST motion profile does not select this optional C.1 subset; its public FB names remain deferred on the absent path until a later scope expansion explicitly adopts them.

- `MC_SetDynCoordTransform`
- `MC_TrackConveyorBelt`
- `MC_SyncAxisToGroup`
- `MC_SyncGroupToAxis`

Deferred:

- `MC_TrackRotaryTable`
- full path-data synchronization variants

### 19.5 Phase D - Homing extensions

Required if custom homing procedures are needed:

- `MC_StepAbsoluteSwitch`
- `MC_StepLimitSwitch`
- `MC_StepBlock`
- `MC_StepReferencePulse`
- `MC_StepDistanceCoded`
- `MC_HomeDirect`
- `MC_HomeAbsolute`
- `MC_FinishHoming`

Deferred but reserved:

- `MC_StepReferenceFlyingSwitch`
- `MC_StepReferenceFlyingRefPulse`
- `MC_AbortPassiveHoming`

Phase D notes:

- The Phase D step FBs complement `MC_Home`; they do not replace the generic Part 1 homing FB.
- The Phase D public surface SHALL include the Part 5 homing-specific types `MC_HOME_DIRECTION`, `MC_SWITCH_MODE`, and `MC_REF_SIGNAL_REF`.
- Step FBs SHALL preserve the Part 5 error-limiting model using torque/time/distance limits where defined.
- In the current deterministic ST kernel, `MC_StepBlock.DetectionVelocityTime` is modeled as a consecutive-scan confirmation rule: `TIME#0ms` completes immediately when the block condition is met, while a nonzero value requires the same block condition on one additional active scan before completion.
- Passive/flying homing FBs remain deferred on the absent path in the current shipped profile, and when they are later implemented they SHALL not themselves trigger motion-state transitions.

### 19.6 Phase E - OOP facade

Phase E ships a single-axis OOP facade package at `libraries/plcopen_motion/oop`.

The classic FB layer remains the primary PLCopen compliance contract and the source of truth for axis behavior. The OOP package SHALL adapt method calls and property reads/writes to existing classic FB behavior instead of owning a separate motion state model.

The shipped Phase E package SHALL include:

- `itfCommand`
- `itfAxisCommand`
- `itfContinuousAxisCommand`
- `itfContinousAxisCommand` compatibility alias
- `itfSynchronizedAxisCommand`
- `itfSynchronizedCommand` compatibility alias
- `itfCamTable`
- `itfAxis`
- `MC_OopCommand`
- `MC_OopAxisCommand`
- `MC_OopContinuousAxisCommand`
- `MC_OopSynchronizedAxisCommand`
- `MC_OopAxis`

Because the PLCopen OOP example intentionally removes `AXIS_REF` from method signatures and expects vendors to add identification in a vendor-specific way, `MC_OopAxis` SHALL expose `Bind(AxisId, InternalIndex) : MC_ERROR` as the truST binding point.

OOP profile, probe, digital-cam, torque/superimposed, and synchronization methods that are outside the current shipped OOP behavior SHALL return command objects with `Error = TRUE` and `ErrorId = mcERR_NotSupported`.

### 19.7 Explicit non-target: Part 6 fluid power

The following Part 6 FBs are explicitly **not targeted** in Phases A-E:

- `MC_LoadControl`
- `MC_LimitLoad`
- `MC_LimitMotion`
- `MC_LoadSuperImposed`
- `MC_LoadProfile`

Rationale:

- they assume a force/load-centric control surface distinct from the first truST adapter profile
- they require additional transducer/control semantics not yet part of the selected axis/group kernel contract

## 20. Axis kernel contract

Each registered axis SHALL provide the following internal capabilities through the adapter layer:

```text
EnablePower
DisablePower
ResetFault
ReadActualPosition
ReadActualVelocity
ReadActualTorque
ReadStatusBits
ReadAxisError
ReadAxisInfo
ReadParameter
WriteParameter
CommandProfileStep
CommandStop
SetOverride
SetLogicalPosition
BeginHome
StepHome
```

The motion library SHALL not assume a specific hardware backend. The adapter MAY target:

- simulation runtime
- fieldbus-connected servo drive
- external motion controller
- software-only planner for tests

## 21. Group kernel contract

Each group SHALL maintain:

- ordered member-axis registry
- kinematic model handle
- transform chain state
- active coordinate-system context
- active motion planner state
- queue of administrative and motion commands
- active/default/reference dynamics
- command-acceptance and command-info bookkeeping
- transition-reference-point configuration
- tool/payload selection state if and when supported

## 22. Kinematics and transforms

### 22.1 Minimum kinematic support

Phase C SHALL support at least:

- identity / cartesian kinematics
- 2D or 3D cartesian gantry mapping

### 22.2 Extensible kinematic model API

The internal kinematic API SHALL support:

- forward kinematics: `ACS -> MCS`
- inverse kinematics: `MCS -> ACS`
- singularity detection
- joint-limit validation
- optional configuration / turn handling

### 22.3 Coordinate systems

The group kernel SHALL support the coordinate systems defined by Part 4 as the target set:

- `ACS`
- `MCS`
- `WCS`
- `PCS`
- `FCS`
- `TCS`

Phase C minimum required support:

- `ACS`
- `MCS`
- `PCS`

`WCS`, `FCS`, and `TCS` MAY be added in later coordinated releases.

### 22.4 Deferred kinematic introspection

`MC_ReadDHParameters` and `MC_ReadJointInfo` are deferred until truST targets richer serial-robot introspection. Their public type names SHALL still be reserved in the compliance matrix. Because the pinned Part 4 RFC records ongoing edits around `MC_ReadDHParameters`, those reserved names SHALL be re-verified against the pinned RFC before truST claims any serial-robot introspection support.


## 23. Dynamic coordinate system rule

The implementation SHALL adopt the Part 4 rule:

> an axes group stays in the coordinate system specified by the last motion command; if that is a dynamic PCS, the group follows that PCS

For truST this SHALL be modeled as a persistent `ActiveFrame` in the group kernel.

## 24. Tracking vs synchronization

The implementation SHALL preserve the distinction:

- **Synchronization** changes progression along a predefined path.
- **Tracking** geometrically couples the group path in space to another moving axis/group.

The library SHALL not collapse these into one public FB even if they share internal machinery.

## 25. Group stop behavior

`MC_GroupStop` SHALL preserve path-oriented stopping semantics:

- group transitions to `GroupStopping`
- no other motion FB may move the group while `GroupStopping` is active
- `Done` becomes true when group velocity reaches zero
- group remains in `GroupStopping` while `Execute = TRUE`
- synchronization/tracking relationships are cancelled by `MC_GroupStop`

## 26. Tool and payload support

For the first release:

- `MC_TOOL_REF` and `MC_RIGID_BODY_DYNAMIC_REF` SHALL be public types
- payloads SHALL be modeled by payload number plus rigid-body dynamic data
- there SHALL be no invented public `MC_PAYLOAD_REF` type in the classic PLCopen surface

Additional rules when tool/payload management is later implemented:

- `Tool 0` SHALL represent the flange / FCS baseline and SHALL NOT be writable as a normal user tool
- tool data SHALL contain the tool frame relative to the flange
- payload selection SHALL remain separate from tool selection

## 27. Naming rules

The public library SHALL use the full PLCopen names by default.

A compile-time optional `SHORT_NAMES` profile MAY expose aliases such as:

- `MC_MoveContRel`
- `MC_GroupRdStatus`
- `MC_SetCoordTrans`

Short-name aliases MUST NOT replace the canonical names.

## 28. Acceptance tests

The implementation SHALL ship with deterministic test cases using the truST runtime.

### 28.1 Mandatory semantic tests

- execute-edge latching
- enable/valid behavior
- missing-input rule
- sign-rule enforcement
- zero-acceleration/zero-deceleration/zero-jerk policy
- busy/done/error/commandaborted exclusivity
- active ownership transfer for buffered commands
- `InVelocity`, `InGear`, `InTorque`, and `InSync` setpoint behavior
- buffered command queue ordering
- buffer cleanup on error stop
- `MC_Stop` and `MC_GroupStop` hold-state behavior while `Execute = TRUE`
- `ContinuousUpdate` parameter update behavior
- `ContinuousUpdate` on queued buffered FBs
- axis/group reset behavior
- `MC_MoveRelative` vs `MC_MoveAdditive`
- Part 1 parameter number table coverage

### 28.2 Group and coordinated-motion tests

- `MC_GroupHome` buffer-mode restrictions
- `MC_GroupStop` vs `MC_GroupHalt`
- `CommandAccepted` / `CommandID` handshake behavior on the FBs that expose them
- `MC_GroupReadCommandInfo` retained metadata behavior
- `MC_GroupReadMotionState.ActiveCommandID`
- linear group move in `MCS`
- direct group move in `ACS` and `MCS`
- blend corner-distance case
- transform round-trip identity case
- dynamic `PCS` follow behavior

### 28.3 Scenario tests from PLCopen examples

The suite SHOULD include scenario-level regressions based on PLCopen examples:

- label machine
- warehousing example
- cam + gear synchronization example
- group stop/halt on-path interruption scenarios

## 29. OOP facade mapping

The shipped single-axis OOP facade SHALL follow these rules:

- classic FB execution remains the source of truth
- command objects wrap classic FB instances or kernel command IDs
- `Abort()` maps to the appropriate cancellation path
- `Update()` maps to `ContinuousUpdate` semantics where supported
- group coordinate objects own data plus coordinate-system metadata

The intended interface family is:

- `itfCommand`
- `itfAxisCommand`
- `itfContinuousAxisCommand`
- `itfSynchronizedAxisCommand`
- `itfCamTable`
- `itfAxis`
- `itfGroupCommand`
- `itfSynchronizedGroupCommand`
- `itfGroupPosition`
- `itfGroupVelocity`
- `itfGroupAcceleration`
- `itfPath`
- `itfGroup`

The shipped Phase E package includes the command and axis interfaces through `itfAxis`. Group interfaces remain a future expansion until truST ships a coordinated-motion OOP facade.

Recommended command base interface:

```iecst
INTERFACE itfCommand
    PROPERTY Done : BOOL
    PROPERTY Busy : BOOL
    PROPERTY Active : BOOL
    PROPERTY CommandAborted : BOOL
    PROPERTY Error : BOOL
    PROPERTY ErrorId : MC_ERROR
    METHOD Abort : MC_ERROR
    METHOD Wait : MC_ERROR
    VAR_INPUT
        Timeout : TIME;
        AbortOnTimeout : BOOL;
    END_VAR
END_INTERFACE
```

Command-object status, axis binding, interface dispatch, property reads, property assignments, and unsupported method results SHALL be locked by Structured Text unit tests. The classic FB layer remains the primary PLCopen certification surface, but the documented OOP single-axis package behavior is now a shipped public API.

## 30. Recommended implementation order

1. Build `AXIS_REF`, core enums, parameter-table support, error-code constants, and a simulated axis adapter.
2. Implement `MC_Power`, `MC_Reset`, parameter FBs, and read/status FBs.
3. Implement queue management and the Part 1 buffer modes.
4. Implement `MC_Stop`, `MC_Halt`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveVelocity`.
5. Add synchronization FBs.
6. Add `AXES_GROUP_REF`, group state machine, transform chain, and group readback/admin FBs.
7. Add cartesian group moves (`MoveLinear`, `MoveDirect`) with identity kinematics and the Part 4 transition model subset.
8. Add dynamic coordinate transforms and conveyor tracking if the product scope needs them.
9. Add custom homing step FBs if required.
10. Maintain the shipped single-axis OOP facade over the classic FB kernels; expand to group OOP only after the classic coordinated-motion package proves the needed behavior.

## 31. Final recommendation

For truST, the most robust first release is:

- **public API:** classic PLCopen FBs
- **internal architecture:** command-kernel + adapter model
- **group conflict policy:** single-axis motion on grouped axes is not allowed
- **numeric policy:** `REAL` outside, `LREAL` inside
- **coordinated-motion scope:** linear/direct moves first, circular/path/tool/payload/jogging later
- **OO support:** shipped single-axis facade as a second public API over the classic kernels
- **fluid power:** explicit non-goal for Phases A-E

This gives a library that is PLCopen-shaped, testable in truST, honest about scope, and still extensible toward broader Part 4 and Part 5 behavior later.
