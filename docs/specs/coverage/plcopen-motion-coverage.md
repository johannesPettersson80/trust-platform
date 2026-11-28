# PLCopen Motion Coverage

This table summarizes the PLCopen Motion profile currently shipped by truST.
The per-symbol source of truth remains
`docs/internal/references/PLCopenMotion/plcopen_motion_compliance_matrix.yaml`.

## Summary

| Scope | Implemented | Deferred | Notes |
| --- | ---: | ---: | --- |
| Part 1 single-axis | 31 | 12 | Classic public surface through the deferred-feature guard |
| Part 1 synchronization | 10 | 3 | Cam/gear subset shipped; phasing/combine deferred |
| Part 4 coordinated motion | 67 | 30 | Phase C core shipped; optional C.1 and later path/tool/payload subsets deferred |
| Part 5 homing | 11 | 3 | Core homing toolkit shipped; passive/flying subset deferred |
| OOP facade | 6 interface/object families | profile/profile-sync method stubs | Single-axis axis/command object facade shipped over the classic kernel |

## Shipped FB Families

| PLCopen area | Implemented FBs |
| --- | --- |
| Single-axis | `MC_Power`, `MC_Home`, `MC_Stop`, `MC_Halt`, `MC_MoveAbsolute`, `MC_MoveRelative`, `MC_MoveAdditive`, `MC_MoveVelocity`, `MC_MoveContinuousAbsolute`, `MC_MoveContinuousRelative`, `MC_SetPosition`, `MC_SetOverride`, `MC_ReadActualPosition`, `MC_ReadActualVelocity`, `MC_ReadActualTorque`, `MC_ReadStatus`, `MC_ReadMotionState`, `MC_ReadAxisInfo`, `MC_ReadAxisError`, `MC_Reset`, `MC_ReadParameter`, `MC_ReadBoolParameter`, `MC_WriteParameter`, `MC_WriteBoolParameter` |
| Synchronization | `MC_CamTableSelect`, `MC_CamIn`, `MC_CamOut`, `MC_GearIn`, `MC_GearOut`, `MC_GearInPos` |
| Coordinated motion core | `MC_AddAxisToGroup`, `MC_RemoveAxisFromGroup`, `MC_UngroupAllAxes`, `MC_GroupReadConfiguration`, `MC_ReadAxisGroupInfo`, `MC_GroupEnable`, `MC_GroupDisable`, `MC_GroupPower`, `MC_GroupReadStatus`, `MC_GroupReadError`, `MC_GroupReset`, `MC_GroupReadPosition`, `MC_GroupReadVelocity`, `MC_GroupReadAcceleration`, `MC_GroupReadMotionState`, `MC_GroupReadParameter`, `MC_GroupWriteParameter`, `MC_GroupReadSWLimits`, `MC_GroupWriteSWLimits`, `MC_SetKinTransform`, `MC_SetCartesianTransform`, `MC_SetCoordinateTransform`, `MC_ReadKinTransform`, `MC_ReadCartesianTransform`, `MC_ReadCoordinateTransform`, `MC_GroupSetPosition`, `MC_MoveLinearAbsolute`, `MC_MoveLinearRelative`, `MC_MoveDirectAbsolute`, `MC_MoveDirectRelative`, `MC_GroupHome`, `MC_GroupStop`, `MC_GroupHalt`, `MC_GroupWaitTime`, `MC_GroupSetOverride`, `MC_TransformPosition`, `MC_GroupReadCommandInfo`, `MC_GroupWriteReferenceDynamics`, `MC_GroupReadReferenceDynamics`, `MC_GroupWriteDefaultDynamics`, `MC_GroupReadDefaultDynamics` |
| Homing core | `MC_StepAbsoluteSwitch`, `MC_StepLimitSwitch`, `MC_StepBlock`, `MC_StepReferencePulse`, `MC_StepDistanceCoded`, `MC_HomeDirect`, `MC_HomeAbsolute`, `MC_FinishHoming` |
| OOP single-axis facade | `itfCommand`, `itfAxisCommand`, `itfContinuousAxisCommand`, `itfContinousAxisCommand`, `itfSynchronizedAxisCommand`, `itfSynchronizedCommand`, `itfCamTable`, `itfAxis`, `MC_OopCommand`, `MC_OopAxisCommand`, `MC_OopContinuousAxisCommand`, `MC_OopSynchronizedAxisCommand`, `MC_OopAxis` |

## Deferred Surface Groups

| PLCopen area | Deferred public names |
| --- | --- |
| Single-axis | `MC_MoveSuperimposed`, `MC_HaltSuperimposed`, `MC_TorqueControl`, `MC_PositionProfile`, `MC_VelocityProfile`, `MC_AccelerationProfile`, `MC_ReadDigitalInput`, `MC_ReadDigitalOutput`, `MC_WriteDigitalOutput`, `MC_DigitalCamSwitch`, `MC_TouchProbe`, `MC_AbortTrigger` |
| Synchronization | `MC_PhasingAbsolute`, `MC_PhasingRelative`, `MC_CombineAxes` |
| Coordinated motion | `MC_CIRC_MODE`, `MC_CIRC_PATHCHOICE`, `MC_TOOL_SOURCE`, `MC_MoveCircularAbsolute`, `MC_MoveCircularRelative`, `MC_PathSelect`, `MC_MovePath`, `MC_GroupInterrupt`, `MC_GroupContinue`, `MC_ReadDHParameters`, `MC_ReadJointInfo`, `MC_GroupJog`, `MC_GroupJogVector`, `MC_GroupWriteJoggingDynamics`, `MC_GroupReadJoggingDynamics`, `MC_WriteToolData`, `MC_ReadToolData`, `MC_SelectTool`, `MC_ReadTool`, `MC_WritePayloadData`, `MC_ReadPayloadData`, `MC_SelectPayload`, `MC_ReadPayload`, `MC_ReadRigidBodyDynamic`, `MC_WriteRigidBodyDynamic`, `MC_SetDynCoordTransform`, `MC_TrackConveyorBelt`, `MC_SyncAxisToGroup`, `MC_SyncGroupToAxis`, `MC_TrackRotaryTable` |
| Homing | `MC_StepReferenceFlyingSwitch`, `MC_StepReferenceFlyingRefPulse`, `MC_AbortPassiveHoming` |
| OOP facade methods | OOP profile/probe/digital-cam/torque/superimposed/synchronization method behavior beyond the shipped single-axis facade. The names are present and return deterministic command objects with `mcERR_NotSupported`. |

## Notes

- Move dynamics: the single-axis kernel's virtual axis has no motion profile yet, so the velocity, accelerations and jerk change no timing (26 November 2026). The discrete moves `MC_MoveAbsolute`, `MC_MoveRelative` and `MC_MoveAdditive` stay `Busy` while `Execute` is TRUE and end with `Done` at the call that has `Execute` FALSE, at once and with the axis at the target; PLCopen Part 1 ends a discrete move when the axis reaches the target, whatever `Execute` does, so a program that holds `Execute` until `Done` waits for ever. The continuous moves `MC_MoveContinuousAbsolute` and `MC_MoveContinuousRelative` report `InEndVelocity` at the call that makes them active, before any travel, and at the call that has `Execute` FALSE end with the axis at their position, at once and at standstill; in PLCopen Part 1 `InEndVelocity` comes when the axis reaches the position, and the axis then goes on at `EndVelocity` until another block aborts the move. The OOP facade's `Release` takes `Execute` to FALSE, so it ends the active move, and an OOP continuous command's `Done` is its block's `InEndVelocity`.
- Power: the single-axis kernel's virtual axis is its own drive, so `MC_Power` switches its power stage on and off at once with `Enable` (28 November 2026). `Enable` FALSE takes the axis to Disabled from any state except ErrorStop, which it keeps with the power off until `MC_Reset`, and every on-going command, active or buffered, ends with `CommandAborted` (PLCopen Part 1 v2.0 §2.1, Figure 2 Note 2). A motion command issued while the axis is Disabled ends in `Error` with `mcERR_NotPowered` (16#0105), and one issued in ErrorStop with `mcERR_InvalidState` (16#0100), without moving the axis or changing its state (§2.1: in Disabled the FBs do not influence the movement; in ErrorStop no further motion command is accepted until a reset). The motion commands are `MC_Home`, `MC_Stop`, `MC_Halt`, the discrete, velocity and continuous moves and the homing blocks of Part 5; `MC_Power`, `MC_Reset`, `MC_SetPosition`, `MC_SetOverride`, the parameter writes and the readbacks are allowed in every state. The synchronization blocks do not consult the axis state yet (checklist G348d).
- `MC_EXECUTION_MODE` is published across the shipped profile; unsupported `mcDelayed` paths are rejected where the current profile does not enable them.
- The current shipped coordinated-motion profile is the Phase C core subset only; optional Phase C.1 tracking/synchronization remains deferred on the absent path.
- The current shipped homing profile excludes passive/flying homing FBs.
- The OOP package is a second API surface, not the primary PLCopen certification surface. `MC_OopAxis` delegates axis behavior to the classic single-axis package, and the PLCopen OOP methods outside that shipped subset return explicit unsupported command objects.
