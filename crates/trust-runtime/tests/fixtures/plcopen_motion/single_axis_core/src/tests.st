TEST_PROGRAM plcopen_motion_single_axis_axis_ref_fields_resolve
VAR
    Axis : AXIS_REF;
END_VAR

Axis.AxisId := UDINT#7;
Axis.InternalIndex := UINT#3;

ASSERT_EQUAL(UDINT#7, Axis.AxisId);
ASSERT_EQUAL(UINT#3, Axis.InternalIndex);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_buffer_mode_literals_resolve
VAR
    BufferModeAborting : MC_BUFFER_MODE;
    BufferModeBuffered : MC_BUFFER_MODE;
    BufferModeBlendingLow : MC_BUFFER_MODE;
    BufferModeBlendingPrevious : MC_BUFFER_MODE;
    BufferModeBlendingNext : MC_BUFFER_MODE;
    BufferModeBlendingHigh : MC_BUFFER_MODE;
END_VAR

BufferModeAborting := MC_BUFFER_MODE#mcAborting;
BufferModeBuffered := MC_BUFFER_MODE#mcBuffered;
BufferModeBlendingLow := MC_BUFFER_MODE#mcBlendingLow;
BufferModeBlendingPrevious := MC_BUFFER_MODE#mcBlendingPrevious;
BufferModeBlendingNext := MC_BUFFER_MODE#mcBlendingNext;
BufferModeBlendingHigh := MC_BUFFER_MODE#mcBlendingHigh;

ASSERT_TRUE(BufferModeAborting = MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(BufferModeBuffered = MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(BufferModeBlendingLow = MC_BUFFER_MODE#mcBlendingLow);
ASSERT_TRUE(BufferModeBlendingPrevious = MC_BUFFER_MODE#mcBlendingPrevious);
ASSERT_TRUE(BufferModeBlendingNext = MC_BUFFER_MODE#mcBlendingNext);
ASSERT_TRUE(BufferModeBlendingHigh = MC_BUFFER_MODE#mcBlendingHigh);
ASSERT_TRUE(BufferModeAborting <> BufferModeBuffered);
ASSERT_TRUE(BufferModeBlendingLow <> BufferModeBlendingHigh);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_direction_literals_resolve
VAR
    DirectionPositive : MC_DIRECTION;
    DirectionShortest : MC_DIRECTION;
    DirectionNegative : MC_DIRECTION;
    DirectionCurrent : MC_DIRECTION;
END_VAR

DirectionPositive := MC_DIRECTION#mcPositiveDirection;
DirectionShortest := MC_DIRECTION#mcShortestWay;
DirectionNegative := MC_DIRECTION#mcNegativeDirection;
DirectionCurrent := MC_DIRECTION#mcCurrentDirection;

ASSERT_TRUE(DirectionPositive = MC_DIRECTION#mcPositiveDirection);
ASSERT_TRUE(DirectionShortest = MC_DIRECTION#mcShortestWay);
ASSERT_TRUE(DirectionNegative = MC_DIRECTION#mcNegativeDirection);
ASSERT_TRUE(DirectionCurrent = MC_DIRECTION#mcCurrentDirection);
ASSERT_TRUE(DirectionPositive <> DirectionNegative);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_execution_mode_literals_resolve
VAR
    ExecutionModeImmediate : MC_EXECUTION_MODE;
    ExecutionModeDelayed : MC_EXECUTION_MODE;
    ExecutionModeQueued : MC_EXECUTION_MODE;
END_VAR

ExecutionModeImmediate := MC_EXECUTION_MODE#mcImmediately;
ExecutionModeDelayed := MC_EXECUTION_MODE#mcDelayed;
ExecutionModeQueued := MC_EXECUTION_MODE#mcQueued;

ASSERT_TRUE(ExecutionModeImmediate = MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(ExecutionModeDelayed = MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(ExecutionModeQueued = MC_EXECUTION_MODE#mcQueued);
ASSERT_TRUE(ExecutionModeImmediate <> ExecutionModeQueued);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_source_literals_resolve
VAR
    SourceCommanded : MC_SOURCE;
    SourceSet : MC_SOURCE;
    SourceActual : MC_SOURCE;
END_VAR

SourceCommanded := MC_SOURCE#mcCommandedValue;
SourceSet := MC_SOURCE#mcSetValue;
SourceActual := MC_SOURCE#mcActualValue;

ASSERT_TRUE(SourceCommanded = MC_SOURCE#mcCommandedValue);
ASSERT_TRUE(SourceSet = MC_SOURCE#mcSetValue);
ASSERT_TRUE(SourceActual = MC_SOURCE#mcActualValue);
ASSERT_TRUE(SourceCommanded <> SourceActual);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_axis_status_literals_resolve
VAR
    AxisStatusErrorStop : MC_AXIS_STATUS;
    AxisStatusDisabled : MC_AXIS_STATUS;
    AxisStatusStandstill : MC_AXIS_STATUS;
    AxisStatusHoming : MC_AXIS_STATUS;
    AxisStatusStopping : MC_AXIS_STATUS;
    AxisStatusDiscreteMotion : MC_AXIS_STATUS;
    AxisStatusContinuousMotion : MC_AXIS_STATUS;
    AxisStatusSynchronizedMotion : MC_AXIS_STATUS;
END_VAR

AxisStatusErrorStop := MC_AXIS_STATUS#mcErrorStop;
AxisStatusDisabled := MC_AXIS_STATUS#mcDisabled;
AxisStatusStandstill := MC_AXIS_STATUS#mcStandstill;
AxisStatusHoming := MC_AXIS_STATUS#mcHoming;
AxisStatusStopping := MC_AXIS_STATUS#mcStopping;
AxisStatusDiscreteMotion := MC_AXIS_STATUS#mcDiscreteMotion;
AxisStatusContinuousMotion := MC_AXIS_STATUS#mcContinuousMotion;
AxisStatusSynchronizedMotion := MC_AXIS_STATUS#mcSynchronizedMotion;

ASSERT_TRUE(AxisStatusErrorStop = MC_AXIS_STATUS#mcErrorStop);
ASSERT_TRUE(AxisStatusDisabled = MC_AXIS_STATUS#mcDisabled);
ASSERT_TRUE(AxisStatusStandstill = MC_AXIS_STATUS#mcStandstill);
ASSERT_TRUE(AxisStatusHoming = MC_AXIS_STATUS#mcHoming);
ASSERT_TRUE(AxisStatusStopping = MC_AXIS_STATUS#mcStopping);
ASSERT_TRUE(AxisStatusDiscreteMotion = MC_AXIS_STATUS#mcDiscreteMotion);
ASSERT_TRUE(AxisStatusContinuousMotion = MC_AXIS_STATUS#mcContinuousMotion);
ASSERT_TRUE(AxisStatusSynchronizedMotion = MC_AXIS_STATUS#mcSynchronizedMotion);
ASSERT_TRUE(AxisStatusStandstill <> AxisStatusHoming);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_core_command_fbs_resolve
VAR
    Axis : AXIS_REF;
    Power : MC_Power;
    Home : MC_Home;
    Stop : MC_Stop;
    Halt : MC_Halt;
    Reset : MC_Reset;
END_VAR

Axis.AxisId := UDINT#1;
Axis.InternalIndex := UINT#1;
Power(Axis := Axis, Enable := FALSE);
Home(Axis := Axis, Execute := FALSE, Position := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
Stop(Axis := Axis, Execute := FALSE, Deceleration := REAL#1.0, Jerk := REAL#1.0);
Halt(Axis := Axis, Execute := FALSE, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
Reset(Axis := Axis, Execute := FALSE);

ASSERT_FALSE(Power.Status);
ASSERT_FALSE(Power.Valid);
ASSERT_FALSE(Home.Done);
ASSERT_FALSE(Home.Active);
ASSERT_FALSE(Stop.Done);
ASSERT_FALSE(Stop.CommandAborted);
ASSERT_FALSE(Halt.Done);
ASSERT_FALSE(Halt.Active);
ASSERT_FALSE(Reset.Done);
ASSERT_TRUE(Power.ErrorID = WORD#16#0000);
ASSERT_TRUE(Home.ErrorID = WORD#16#0000);
ASSERT_TRUE(Stop.ErrorID = WORD#16#0000);
ASSERT_TRUE(Halt.ErrorID = WORD#16#0000);
ASSERT_TRUE(Reset.ErrorID = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_motion_fbs_resolve
VAR
    Axis : AXIS_REF;
    MoveAbsolute : MC_MoveAbsolute;
    MoveRelative : MC_MoveRelative;
    MoveAdditive : MC_MoveAdditive;
    MoveVelocity : MC_MoveVelocity;
    MoveContinuousAbsolute : MC_MoveContinuousAbsolute;
    MoveContinuousRelative : MC_MoveContinuousRelative;
END_VAR

Axis.AxisId := UDINT#2;
Axis.InternalIndex := UINT#2;
MoveAbsolute(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#100.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveRelative(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Distance := REAL#25.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveAdditive(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Distance := REAL#10.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveVelocity(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveContinuousAbsolute(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#100.0,
    EndVelocity := REAL#5.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveContinuousRelative(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Distance := REAL#20.0,
    EndVelocity := REAL#5.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);

ASSERT_FALSE(MoveAbsolute.Done);
ASSERT_FALSE(MoveRelative.Done);
ASSERT_FALSE(MoveAdditive.Done);
ASSERT_FALSE(MoveVelocity.InVelocity);
ASSERT_FALSE(MoveContinuousAbsolute.InEndVelocity);
ASSERT_FALSE(MoveContinuousRelative.InEndVelocity);
ASSERT_TRUE(MoveAbsolute.ErrorID = WORD#16#0000);
ASSERT_TRUE(MoveRelative.ErrorID = WORD#16#0000);
ASSERT_TRUE(MoveAdditive.ErrorID = WORD#16#0000);
ASSERT_TRUE(MoveVelocity.ErrorID = WORD#16#0000);
ASSERT_TRUE(MoveContinuousAbsolute.ErrorID = WORD#16#0000);
ASSERT_TRUE(MoveContinuousRelative.ErrorID = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_administrative_readback_fbs_resolve
VAR
    Axis : AXIS_REF;
    SetPosition : MC_SetPosition;
    SetOverride : MC_SetOverride;
    ReadActualPosition : MC_ReadActualPosition;
    ReadActualVelocity : MC_ReadActualVelocity;
    ReadActualTorque : MC_ReadActualTorque;
    ReadStatus : MC_ReadStatus;
    ReadMotionState : MC_ReadMotionState;
    ReadAxisInfo : MC_ReadAxisInfo;
    ReadAxisError : MC_ReadAxisError;
END_VAR

Axis.AxisId := UDINT#3;
Axis.InternalIndex := UINT#3;
SetPosition(
    Axis := Axis,
    Execute := FALSE,
    Position := REAL#0.0,
    Relative := FALSE,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
SetOverride(
    Axis := Axis,
    Enable := FALSE,
    VelFactor := REAL#1.0,
    AccFactor := REAL#1.0,
    JerkFactor := REAL#1.0
);
ReadActualPosition(Axis := Axis, Enable := FALSE);
ReadActualVelocity(Axis := Axis, Enable := FALSE);
ReadActualTorque(Axis := Axis, Enable := FALSE);
ReadStatus(Axis := Axis, Enable := FALSE);
ReadMotionState(Axis := Axis, Enable := FALSE, Source := MC_SOURCE#mcActualValue);
ReadAxisInfo(Axis := Axis, Enable := FALSE);
ReadAxisError(Axis := Axis, Enable := FALSE);

ASSERT_FALSE(SetPosition.Done);
ASSERT_FALSE(SetOverride.Enabled);
ASSERT_FALSE(ReadActualPosition.Valid);
ASSERT_FALSE(ReadActualVelocity.Valid);
ASSERT_FALSE(ReadActualTorque.Valid);
ASSERT_FALSE(ReadStatus.Valid);
ASSERT_FALSE(ReadMotionState.Valid);
ASSERT_FALSE(ReadAxisInfo.Valid);
ASSERT_FALSE(ReadAxisError.Valid);
ASSERT_TRUE(ReadActualPosition.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadActualVelocity.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadActualTorque.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadStatus.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadMotionState.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadAxisInfo.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadAxisError.AxisErrorID = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_parameter_fbs_resolve
VAR
    Axis : AXIS_REF;
    Constants : MC_Constants;
    ReadParameter : MC_ReadParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
    WriteParameter : MC_WriteParameter;
    WriteBoolParameter : MC_WriteBoolParameter;
END_VAR

Axis.AxisId := UDINT#4;
Axis.InternalIndex := UINT#4;
Constants();
ReadParameter(Axis := Axis, Enable := FALSE, ParameterNumber := Constants.PN_CommandedPosition);
ReadBoolParameter(Axis := Axis, Enable := FALSE, ParameterNumber := Constants.PN_EnableLimitPos);
WriteParameter(
    Axis := Axis,
    Execute := FALSE,
    ParameterNumber := Constants.PN_MaxVelocityAppl,
    Value := REAL#12.5,
    ExecutionMode := MC_EXECUTION_MODE#mcQueued
);
WriteBoolParameter(
    Axis := Axis,
    Execute := FALSE,
    ParameterNumber := Constants.PN_EnableLimitPos,
    Value := FALSE,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);

ASSERT_FALSE(ReadParameter.Valid);
ASSERT_FALSE(ReadBoolParameter.Valid);
ASSERT_FALSE(WriteParameter.Done);
ASSERT_FALSE(WriteBoolParameter.Done);
ASSERT_TRUE(ReadParameter.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadBoolParameter.ErrorID = WORD#16#0000);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0000);
ASSERT_TRUE(WriteBoolParameter.ErrorID = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_parameter_number_constants_resolve
VAR
    Constants : MC_Constants;
END_VAR

Constants();
ASSERT_EQUAL(INT#1, Constants.PN_CommandedPosition);
ASSERT_EQUAL(INT#2, Constants.PN_SWLimitPos);
ASSERT_EQUAL(INT#3, Constants.PN_SWLimitNeg);
ASSERT_EQUAL(INT#4, Constants.PN_EnableLimitPos);
ASSERT_EQUAL(INT#5, Constants.PN_EnableLimitNeg);
ASSERT_EQUAL(INT#6, Constants.PN_EnablePosLagMonitoring);
ASSERT_EQUAL(INT#7, Constants.PN_MaxPositionLag);
ASSERT_EQUAL(INT#8, Constants.PN_MaxVelocitySystem);
ASSERT_EQUAL(INT#9, Constants.PN_MaxVelocityAppl);
ASSERT_EQUAL(INT#10, Constants.PN_ActualVelocity);
ASSERT_EQUAL(INT#11, Constants.PN_CommandedVelocity);
ASSERT_EQUAL(INT#12, Constants.PN_MaxAccelerationSystem);
ASSERT_EQUAL(INT#13, Constants.PN_MaxAccelerationAppl);
ASSERT_EQUAL(INT#14, Constants.PN_MaxDecelerationSystem);
ASSERT_EQUAL(INT#15, Constants.PN_MaxDecelerationAppl);
ASSERT_EQUAL(INT#16, Constants.PN_MaxJerkSystem);
ASSERT_EQUAL(INT#17, Constants.PN_MaxJerkAppl);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_error_constants_resolve
VAR
    Constants : MC_Constants;
    ErrorCodes : ARRAY[1..13] OF WORD;
    I : INT;
    J : INT;
END_VAR

Constants();
ASSERT_EQUAL(WORD#16#0000, Constants.mcERR_None);
ASSERT_EQUAL(WORD#16#0001, Constants.mcERR_InvalidParameter);
ASSERT_EQUAL(WORD#16#0100, Constants.mcERR_InvalidState);
ASSERT_EQUAL(WORD#16#0101, Constants.mcERR_AxisGrouped);
ASSERT_EQUAL(WORD#16#0102, Constants.mcERR_GroupDisabled);
ASSERT_EQUAL(WORD#16#0103, Constants.mcERR_GroupNotReady);
ASSERT_EQUAL(WORD#16#0104, Constants.mcERR_NotHomed);
ASSERT_EQUAL(WORD#16#0105, Constants.mcERR_NotPowered);
ASSERT_EQUAL(WORD#16#0200, Constants.mcERR_BackendFault);
ASSERT_EQUAL(WORD#16#0300, Constants.mcERR_KinematicNoSolution);
ASSERT_EQUAL(WORD#16#0301, Constants.mcERR_KinematicSingularity);
ASSERT_EQUAL(WORD#16#0400, Constants.mcERR_QueueFull);
ASSERT_EQUAL(WORD#16#0500, Constants.mcERR_NotSupported);

ErrorCodes[1] := Constants.mcERR_None;
ErrorCodes[2] := Constants.mcERR_InvalidParameter;
ErrorCodes[3] := Constants.mcERR_InvalidState;
ErrorCodes[4] := Constants.mcERR_AxisGrouped;
ErrorCodes[5] := Constants.mcERR_GroupDisabled;
ErrorCodes[6] := Constants.mcERR_GroupNotReady;
ErrorCodes[7] := Constants.mcERR_NotHomed;
ErrorCodes[8] := Constants.mcERR_NotPowered;
ErrorCodes[9] := Constants.mcERR_BackendFault;
ErrorCodes[10] := Constants.mcERR_KinematicNoSolution;
ErrorCodes[11] := Constants.mcERR_KinematicSingularity;
ErrorCodes[12] := Constants.mcERR_QueueFull;
ErrorCodes[13] := Constants.mcERR_NotSupported;

FOR I := 1 TO 13 DO
    FOR J := I + 1 TO 13 DO
        ASSERT_TRUE(ErrorCodes[I] <> ErrorCodes[J]);
    END_FOR
END_FOR
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_mc_power_status_tracks_stage_not_enable
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Power : MC_Power;
    ReadStatus : MC_ReadStatus;
END_VAR

Axis.AxisId := UDINT#101;
Axis.InternalIndex := UINT#5;
Seed(Axis := Axis, ResetDefaults := TRUE);
Power(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(Power.Valid);
ASSERT_TRUE(Power.Status);
ASSERT_FALSE(Power.Error);
Power(Axis := Axis, Enable := FALSE);
ASSERT_FALSE(Power.Valid);
ASSERT_TRUE(Power.Status);
Seed(Axis := Axis, ApplyPowerStage := TRUE, PowerStage := FALSE);
Power(Axis := Axis, Enable := FALSE);
ASSERT_FALSE(Power.Status);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Disabled);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_read_status_and_reset_state_paths
VAR
    AxisStop : AXIS_REF;
    AxisResetOff : AXIS_REF;
    AxisResetOn : AXIS_REF;
    AxisResetNoop : AXIS_REF;
    SeedStop : TRUST_MOTION_TestAxisSeed;
    SeedResetOff : TRUST_MOTION_TestAxisSeed;
    SeedResetOn : TRUST_MOTION_TestAxisSeed;
    SeedResetNoop : TRUST_MOTION_TestAxisSeed;
    Stop : MC_Stop;
    ResetOff : MC_Reset;
    ResetOn : MC_Reset;
    ResetNoop : MC_Reset;
    ReadStatusStop : MC_ReadStatus;
    ReadStatusResetOff : MC_ReadStatus;
    ReadStatusResetOn : MC_ReadStatus;
    ReadStatusResetNoop : MC_ReadStatus;
END_VAR

AxisStop.AxisId := UDINT#102;
AxisStop.InternalIndex := UINT#6;
SeedStop(Axis := AxisStop, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcDiscreteMotion, ApplyPowerStage := TRUE, PowerStage := TRUE);
Stop(Axis := AxisStop, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ReadStatusStop(Axis := AxisStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusStop.Stopping);
Stop(Axis := AxisStop, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ReadStatusStop(Axis := AxisStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusStop.Stopping);
Stop(Axis := AxisStop, Execute := FALSE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ASSERT_TRUE(Stop.Done);
ReadStatusStop(Axis := AxisStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusStop.Standstill);

AxisResetOff.AxisId := UDINT#103;
AxisResetOff.InternalIndex := UINT#7;
SeedResetOff(Axis := AxisResetOff, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcErrorStop, ApplyPowerStage := TRUE, PowerStage := FALSE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
ResetOff(Axis := AxisResetOff, Execute := TRUE);
ResetOff(Axis := AxisResetOff, Execute := FALSE);
ReadStatusResetOff(Axis := AxisResetOff, Enable := TRUE);
ASSERT_TRUE(ReadStatusResetOff.Disabled);
ASSERT_TRUE(ResetOff.Done);

AxisResetOn.AxisId := UDINT#104;
AxisResetOn.InternalIndex := UINT#8;
SeedResetOn(Axis := AxisResetOn, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcErrorStop, ApplyPowerStage := TRUE, PowerStage := TRUE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
ResetOn(Axis := AxisResetOn, Execute := TRUE);
ResetOn(Axis := AxisResetOn, Execute := FALSE);
ReadStatusResetOn(Axis := AxisResetOn, Enable := TRUE);
ASSERT_TRUE(ReadStatusResetOn.Standstill);
ASSERT_TRUE(ResetOn.Done);

AxisResetNoop.AxisId := UDINT#105;
AxisResetNoop.InternalIndex := UINT#9;
SeedResetNoop(Axis := AxisResetNoop, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcHoming);
ResetNoop(Axis := AxisResetNoop, Execute := TRUE);
ResetNoop(Axis := AxisResetNoop, Execute := FALSE);
ReadStatusResetNoop(Axis := AxisResetNoop, Enable := TRUE);
ASSERT_TRUE(ReadStatusResetNoop.Homing);
ASSERT_TRUE(ResetNoop.Done);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_administrative_fbs_preserve_state
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    SetPosition : MC_SetPosition;
    SetOverride : MC_SetOverride;
    ReadActualPosition : MC_ReadActualPosition;
    ReadParameter : MC_ReadParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
    WriteParameter : MC_WriteParameter;
    WriteBoolParameter : MC_WriteBoolParameter;
    ReadStatus : MC_ReadStatus;
    Constants : MC_Constants;
END_VAR

Axis.AxisId := UDINT#106;
Axis.InternalIndex := UINT#10;
Constants();
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcSynchronizedMotion,
    ApplyActuals := TRUE,
    ActualPosition := REAL#10.0,
    SetPosition := REAL#10.0,
    CommandedPosition := REAL#10.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
SetPosition(Axis := Axis, Execute := TRUE, Position := REAL#25.0, Relative := FALSE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SetPosition(Axis := Axis, Execute := FALSE, Position := REAL#25.0, Relative := FALSE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SetOverride(Axis := Axis, Enable := TRUE, VelFactor := REAL#1.0, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteBoolParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.SynchronizedMotion);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_grouped_axis_motion_rejection_and_readonly_allowance
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveAbsolute : MC_MoveAbsolute;
    Home : MC_Home;
    ReadActualPosition : MC_ReadActualPosition;
    ReadParameter : MC_ReadParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
    ReadStatus : MC_ReadStatus;
    Constants : MC_Constants;
END_VAR

Axis.AxisId := UDINT#107;
Axis.InternalIndex := UINT#11;
Constants();
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyGrouped := TRUE,
    Grouped := TRUE,
    ApplyActuals := TRUE,
    ActualPosition := REAL#33.0,
    SetPosition := REAL#33.0,
    CommandedPosition := REAL#33.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
MoveAbsolute(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#100.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveAbsolute.Error);
ASSERT_TRUE(MoveAbsolute.ErrorID = WORD#16#0101);
Home(Axis := Axis, Execute := TRUE, Position := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(Home.Error);
ASSERT_TRUE(Home.ErrorID = WORD#16#0101);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_TRUE(ReadActualPosition.Valid);
ASSERT_TRUE(ReadParameter.Valid);
ASSERT_TRUE(ReadBoolParameter.Valid);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Standstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_home_backend_fault_enters_errorstop
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Home : MC_Home;
    ReadStatus : MC_ReadStatus;
END_VAR

Axis.AxisId := UDINT#108;
Axis.InternalIndex := UINT#12;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyPowerStage := TRUE, PowerStage := TRUE);
Home(Axis := Axis, Execute := TRUE, Position := REAL#12.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Homing);
Seed(Axis := Axis, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
Home(Axis := Axis, Execute := TRUE, Position := REAL#12.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(Home.Error);
ASSERT_TRUE(Home.ErrorID = WORD#16#0200);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.ErrorStop);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_read_motion_state_source_and_flags
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    ReadMotionState : MC_ReadMotionState;
END_VAR

Axis.AxisId := UDINT#109;
Axis.InternalIndex := UINT#13;
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyActuals := TRUE,
    ActualPosition := REAL#0.0,
    SetPosition := REAL#0.0,
    CommandedPosition := REAL#0.0,
    ActualVelocity := REAL#0.0 - REAL#2.0,
    SetVelocity := REAL#1.5,
    CommandedVelocity := REAL#3.0,
    ActualTorque := REAL#0.0,
    ApplyMotionFlags := TRUE,
    ConstantVelocity := TRUE,
    Accelerating := FALSE,
    Decelerating := FALSE
);
ReadMotionState(Axis := Axis, Enable := TRUE, Source := MC_SOURCE#mcActualValue);
ASSERT_TRUE(ReadMotionState.Valid);
ASSERT_TRUE(ReadMotionState.ConstantVelocity);
ASSERT_TRUE(ReadMotionState.DirectionNegative);
ASSERT_FALSE(ReadMotionState.DirectionPositive);
ReadMotionState(Axis := Axis, Enable := TRUE, Source := MC_SOURCE#mcSetValue);
ASSERT_TRUE(ReadMotionState.DirectionPositive);
ReadMotionState(Axis := Axis, Enable := TRUE, Source := MC_SOURCE#mcCommandedValue);
ASSERT_TRUE(ReadMotionState.DirectionPositive);
Seed(Axis := Axis, ApplyMotionFlags := TRUE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := TRUE);
ReadMotionState(Axis := Axis, Enable := TRUE, Source := MC_SOURCE#mcActualValue);
ASSERT_TRUE(ReadMotionState.Decelerating);
ASSERT_FALSE(ReadMotionState.ConstantVelocity);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_read_axis_info_and_axis_error
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    ReadAxisInfo : MC_ReadAxisInfo;
    ReadAxisError : MC_ReadAxisError;
END_VAR

Axis.AxisId := UDINT#110;
Axis.InternalIndex := UINT#14;
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyPowerStage := TRUE,
    PowerStage := TRUE,
    ApplyAxisError := TRUE,
    AxisErrorID := WORD#16#0200,
    ApplyAxisInfo := TRUE,
    HomeAbsSwitch := TRUE,
    LimitSwitchPos := TRUE,
    LimitSwitchNeg := FALSE,
    Simulation := TRUE,
    CommunicationReady := TRUE,
    ReadyForPowerOn := TRUE,
    IsHomed := TRUE,
    AxisWarning := TRUE
);
ReadAxisInfo(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadAxisInfo.Valid);
ASSERT_TRUE(ReadAxisInfo.HomeAbsSwitch);
ASSERT_TRUE(ReadAxisInfo.LimitSwitchPos);
ASSERT_FALSE(ReadAxisInfo.LimitSwitchNeg);
ASSERT_TRUE(ReadAxisInfo.Simulation);
ASSERT_TRUE(ReadAxisInfo.CommunicationReady);
ASSERT_TRUE(ReadAxisInfo.ReadyForPowerOn);
ASSERT_TRUE(ReadAxisInfo.PowerOn);
ASSERT_TRUE(ReadAxisInfo.IsHomed);
ASSERT_TRUE(ReadAxisInfo.AxisWarning);
ReadAxisError(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadAxisError.Valid);
ASSERT_TRUE(ReadAxisError.ErrorID = WORD#16#0000);
ASSERT_TRUE(ReadAxisError.AxisErrorID = WORD#16#0200);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_set_position_updates_actual_and_commanded_values
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    SetPosition : MC_SetPosition;
    ReadActualPosition : MC_ReadActualPosition;
    ReadParameter : MC_ReadParameter;
    ReadStatus : MC_ReadStatus;
    Constants : MC_Constants;
END_VAR

Axis.AxisId := UDINT#111;
Axis.InternalIndex := UINT#15;
Constants();
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyActuals := TRUE,
    ActualPosition := REAL#10.0,
    SetPosition := REAL#10.0,
    CommandedPosition := REAL#10.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
SetPosition(Axis := Axis, Execute := TRUE, Position := REAL#25.0, Relative := FALSE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
SetPosition(Axis := Axis, Execute := FALSE, Position := REAL#25.0, Relative := FALSE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadActualPosition.Position = REAL#25.0);
ASSERT_TRUE(ReadParameter.Value = REAL#25.0);
SetPosition(Axis := Axis, Execute := TRUE, Position := REAL#5.0, Relative := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
SetPosition(Axis := Axis, Execute := FALSE, Position := REAL#5.0, Relative := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadActualPosition.Position = REAL#30.0);
ASSERT_TRUE(ReadParameter.Value = REAL#30.0);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Standstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_set_override_validates_factors_and_vel_zero_behavior
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    SetOverride : MC_SetOverride;
    ReadStatus : MC_ReadStatus;
END_VAR

Axis.AxisId := UDINT#112;
Axis.InternalIndex := UINT#16;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcContinuousMotion);
SetOverride(Axis := Axis, Enable := TRUE, VelFactor := REAL#0.0, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(SetOverride.Enabled);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Stopping);
ASSERT_FALSE(ReadStatus.Standstill);
SetOverride(Axis := Axis, Enable := TRUE, VelFactor := REAL#0.0 - REAL#1.0, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(SetOverride.Error);
ASSERT_TRUE(SetOverride.ErrorID = WORD#16#0001);
SetOverride(Axis := Axis, Enable := TRUE, VelFactor := REAL#1.0, AccFactor := REAL#0.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(SetOverride.Error);
ASSERT_TRUE(SetOverride.ErrorID = WORD#16#0001);
SetOverride(Axis := Axis, Enable := TRUE, VelFactor := REAL#1.0, AccFactor := REAL#1.0, JerkFactor := REAL#0.0);
ASSERT_TRUE(SetOverride.Error);
ASSERT_TRUE(SetOverride.ErrorID = WORD#16#0001);
SetOverride(Axis := Axis, Enable := TRUE, VelFactor := REAL#1.5, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(SetOverride.Error);
ASSERT_TRUE(SetOverride.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_actual_value_readbacks_follow_seeded_values
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    ReadActualPosition : MC_ReadActualPosition;
    ReadActualVelocity : MC_ReadActualVelocity;
    ReadActualTorque : MC_ReadActualTorque;
END_VAR

Axis.AxisId := UDINT#113;
Axis.InternalIndex := UINT#17;
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyActuals := TRUE,
    ActualPosition := REAL#123.0,
    SetPosition := REAL#120.0,
    CommandedPosition := REAL#125.0,
    ActualVelocity := REAL#7.5,
    SetVelocity := REAL#8.0,
    CommandedVelocity := REAL#9.0,
    ActualTorque := REAL#2.5
);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadActualVelocity(Axis := Axis, Enable := TRUE);
ReadActualTorque(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadActualPosition.Position = REAL#123.0);
ASSERT_TRUE(ReadActualVelocity.Velocity = REAL#7.5);
ASSERT_TRUE(ReadActualTorque.Torque = REAL#2.5);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_parameter_plane_numeric_and_bool_roundtrip
VAR
    Axis : AXIS_REF;
    Constants : MC_Constants;
    WriteParameter : MC_WriteParameter;
    ReadParameter : MC_ReadParameter;
    WriteBoolParameter : MC_WriteBoolParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
END_VAR

Axis.AxisId := UDINT#114;
Axis.InternalIndex := UINT#18;
Constants();
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_CommandedPosition, Value := REAL#41.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_CommandedPosition, Value := REAL#41.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitNeg, Value := REAL#0.0 - REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitNeg, Value := REAL#0.0 - REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxPositionLag, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxPositionLag, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxVelocitySystem, Value := REAL#110.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxVelocitySystem, Value := REAL#110.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#55.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#55.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_ActualVelocity, Value := REAL#5.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_ActualVelocity, Value := REAL#5.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_CommandedVelocity, Value := REAL#6.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_CommandedVelocity, Value := REAL#6.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxAccelerationSystem, Value := REAL#21.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxAccelerationSystem, Value := REAL#21.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#22.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#22.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxDecelerationSystem, Value := REAL#23.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxDecelerationSystem, Value := REAL#23.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxDecelerationAppl, Value := REAL#24.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxDecelerationAppl, Value := REAL#24.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxJerkSystem, Value := REAL#25.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxJerkSystem, Value := REAL#25.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxJerkAppl, Value := REAL#26.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxJerkAppl, Value := REAL#26.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);

ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#41.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_SWLimitPos);
ASSERT_TRUE(ReadParameter.Value = REAL#90.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_SWLimitNeg);
ASSERT_TRUE(ReadParameter.Value = REAL#0.0 - REAL#90.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxPositionLag);
ASSERT_TRUE(ReadParameter.Value = REAL#12.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxVelocitySystem);
ASSERT_TRUE(ReadParameter.Value = REAL#110.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl);
ASSERT_TRUE(ReadParameter.Value = REAL#55.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_ActualVelocity);
ASSERT_TRUE(ReadParameter.Value = REAL#5.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedVelocity);
ASSERT_TRUE(ReadParameter.Value = REAL#6.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxAccelerationSystem);
ASSERT_TRUE(ReadParameter.Value = REAL#21.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxAccelerationAppl);
ASSERT_TRUE(ReadParameter.Value = REAL#22.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxDecelerationSystem);
ASSERT_TRUE(ReadParameter.Value = REAL#23.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxDecelerationAppl);
ASSERT_TRUE(ReadParameter.Value = REAL#24.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxJerkSystem);
ASSERT_TRUE(ReadParameter.Value = REAL#25.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxJerkAppl);
ASSERT_TRUE(ReadParameter.Value = REAL#26.0);

WriteBoolParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitNeg, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteBoolParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitNeg, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteBoolParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_EnablePosLagMonitoring, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_EnablePosLagMonitoring, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolParameter(Axis := Axis, Execute := TRUE, ParameterNumber := INT#1000, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteBoolParameter(Axis := Axis, Execute := FALSE, ParameterNumber := INT#1000, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_TRUE(ReadBoolParameter.Value);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitNeg);
ASSERT_TRUE(ReadBoolParameter.Value);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnablePosLagMonitoring);
ASSERT_TRUE(ReadBoolParameter.Value);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := INT#1000);
ASSERT_TRUE(ReadBoolParameter.Value);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_parameter_plane_rejections_and_mcDelayed
VAR
    Axis : AXIS_REF;
    Constants : MC_Constants;
    WriteParameter : MC_WriteParameter;
    WriteBoolParameter : MC_WriteBoolParameter;
    ReadParameter : MC_ReadParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
    SetPosition : MC_SetPosition;
END_VAR

Axis.AxisId := UDINT#115;
Axis.InternalIndex := UINT#19;
Constants();
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_TRUE(ReadParameter.Error);
ASSERT_TRUE(ReadParameter.ErrorID = WORD#16#0001);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadBoolParameter.Error);
ASSERT_TRUE(ReadBoolParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#0.0 - REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#0.0 - REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_CommandedPosition, Value := REAL#10.0, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0500);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_CommandedPosition, Value := REAL#10.0, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
WriteBoolParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(WriteBoolParameter.Error);
ASSERT_TRUE(WriteBoolParameter.ErrorID = WORD#16#0500);
WriteBoolParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
SetPosition(Axis := Axis, Execute := TRUE, Position := REAL#5.0, Relative := FALSE, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(SetPosition.Error);
ASSERT_TRUE(SetPosition.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_execute_edge_and_continuous_update_semantics
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveAbsolute : MC_MoveAbsolute;
    ReadParameter : MC_ReadParameter;
    Constants : MC_Constants;
END_VAR

Axis.AxisId := UDINT#116;
Axis.InternalIndex := UINT#20;
Constants();
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyActuals := TRUE,
    ActualPosition := REAL#0.0,
    SetPosition := REAL#0.0,
    CommandedPosition := REAL#0.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
MoveAbsolute(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#50.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveAbsolute.Busy);
ASSERT_TRUE(MoveAbsolute.Active);
ASSERT_FALSE(MoveAbsolute.Done);
ASSERT_FALSE(MoveAbsolute.Error);
ASSERT_FALSE(MoveAbsolute.CommandAborted);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Valid);
ASSERT_TRUE(ReadParameter.Value = REAL#50.0);
MoveAbsolute(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#75.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#50.0);
MoveAbsolute(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := TRUE,
    Position := REAL#80.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#80.0);
MoveAbsolute(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := TRUE,
    Position := REAL#80.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveAbsolute.Done);
ASSERT_FALSE(MoveAbsolute.Busy);
ASSERT_FALSE(MoveAbsolute.Active);
ASSERT_FALSE(MoveAbsolute.Error);
ASSERT_FALSE(MoveAbsolute.CommandAborted);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#80.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_enable_style_valid_error_exclusivity
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    ReadParameter : MC_ReadParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
    Constants : MC_Constants;
END_VAR

Axis.AxisId := UDINT#117;
Axis.InternalIndex := UINT#21;
Constants();
Seed(Axis := Axis, ResetDefaults := TRUE);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Valid);
ASSERT_FALSE(ReadParameter.Error);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_FALSE(ReadParameter.Valid);
ASSERT_TRUE(ReadParameter.Error);
ASSERT_TRUE(ReadParameter.ErrorID = WORD#16#0001);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_TRUE(ReadBoolParameter.Valid);
ASSERT_FALSE(ReadBoolParameter.Error);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_FALSE(ReadBoolParameter.Valid);
ASSERT_TRUE(ReadBoolParameter.Error);
ASSERT_TRUE(ReadBoolParameter.ErrorID = WORD#16#0001);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_relative_and_additive_distinction
VAR
    AxisRelative : AXIS_REF;
    AxisAdditive : AXIS_REF;
    SeedRelative : TRUST_MOTION_TestAxisSeed;
    SeedAdditive : TRUST_MOTION_TestAxisSeed;
    MoveRelative : MC_MoveRelative;
    MoveAdditive : MC_MoveAdditive;
    ReadActualPositionRelative : MC_ReadActualPosition;
    ReadActualPositionAdditive : MC_ReadActualPosition;
    ReadParameterRelative : MC_ReadParameter;
    ReadParameterAdditive : MC_ReadParameter;
    Constants : MC_Constants;
END_VAR

Constants();
AxisRelative.AxisId := UDINT#118;
AxisRelative.InternalIndex := UINT#22;
SeedRelative(
    Axis := AxisRelative,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyActuals := TRUE,
    ActualPosition := REAL#10.0,
    SetPosition := REAL#10.0,
    CommandedPosition := REAL#20.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
MoveRelative(
    Axis := AxisRelative,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := REAL#5.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveRelative(
    Axis := AxisRelative,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Distance := REAL#5.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadActualPositionRelative(Axis := AxisRelative, Enable := TRUE);
ReadParameterRelative(Axis := AxisRelative, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadActualPositionRelative.Position = REAL#15.0);
ASSERT_TRUE(ReadParameterRelative.Value = REAL#15.0);

AxisAdditive.AxisId := UDINT#119;
AxisAdditive.InternalIndex := UINT#23;
SeedAdditive(
    Axis := AxisAdditive,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyActuals := TRUE,
    ActualPosition := REAL#10.0,
    SetPosition := REAL#10.0,
    CommandedPosition := REAL#20.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
MoveAdditive(
    Axis := AxisAdditive,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := REAL#5.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveAdditive(
    Axis := AxisAdditive,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Distance := REAL#5.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadActualPositionAdditive(Axis := AxisAdditive, Enable := TRUE);
ReadParameterAdditive(Axis := AxisAdditive, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadActualPositionAdditive.Position = REAL#25.0);
ASSERT_TRUE(ReadParameterAdditive.Value = REAL#25.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_move_velocity_direction_and_state
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveVelocity : MC_MoveVelocity;
    ReadStatus : MC_ReadStatus;
    ReadMotionState : MC_ReadMotionState;
END_VAR

Axis.AxisId := UDINT#120;
Axis.InternalIndex := UINT#24;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveVelocity(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcNegativeDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveVelocity.Busy);
ASSERT_TRUE(MoveVelocity.Active);
ASSERT_TRUE(MoveVelocity.InVelocity);
ASSERT_FALSE(MoveVelocity.Error);
ASSERT_FALSE(MoveVelocity.CommandAborted);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.ContinuousMotion);
ReadMotionState(Axis := Axis, Enable := TRUE, Source := MC_SOURCE#mcActualValue);
ASSERT_TRUE(ReadMotionState.DirectionNegative);
ASSERT_TRUE(ReadMotionState.ConstantVelocity);
MoveVelocity(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcNegativeDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_FALSE(MoveVelocity.Busy);
ASSERT_FALSE(MoveVelocity.Active);
ASSERT_FALSE(MoveVelocity.InVelocity);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Standstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_home_halt_stop_and_continuous_end_velocity_behaviors
VAR
    AxisHome : AXIS_REF;
    AxisHalt : AXIS_REF;
    AxisStop : AXIS_REF;
    AxisContinuous : AXIS_REF;
    SeedHome : TRUST_MOTION_TestAxisSeed;
    SeedHalt : TRUST_MOTION_TestAxisSeed;
    SeedStop : TRUST_MOTION_TestAxisSeed;
    SeedContinuous : TRUST_MOTION_TestAxisSeed;
    Home : MC_Home;
    Halt : MC_Halt;
    Stop : MC_Stop;
    MoveContinuousAbsolute : MC_MoveContinuousAbsolute;
    ReadStatusHome : MC_ReadStatus;
    ReadStatusHalt : MC_ReadStatus;
    ReadStatusStop : MC_ReadStatus;
    ReadStatusContinuous : MC_ReadStatus;
    ReadAxisInfo : MC_ReadAxisInfo;
    ReadActualVelocity : MC_ReadActualVelocity;
END_VAR

AxisHome.AxisId := UDINT#121;
AxisHome.InternalIndex := UINT#25;
SeedHome(Axis := AxisHome, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyPowerStage := TRUE, PowerStage := TRUE);
Home(Axis := AxisHome, Execute := TRUE, Position := REAL#12.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(Home.Busy);
ASSERT_TRUE(Home.Active);
ReadStatusHome(Axis := AxisHome, Enable := TRUE);
ASSERT_TRUE(ReadStatusHome.Homing);
Home(Axis := AxisHome, Execute := FALSE, Position := REAL#12.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(Home.Done);
ReadStatusHome(Axis := AxisHome, Enable := TRUE);
ASSERT_TRUE(ReadStatusHome.Standstill);
ReadAxisInfo(Axis := AxisHome, Enable := TRUE);
ASSERT_TRUE(ReadAxisInfo.IsHomed);

AxisHalt.AxisId := UDINT#122;
AxisHalt.InternalIndex := UINT#26;
SeedHalt(Axis := AxisHalt, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcContinuousMotion, ApplyPowerStage := TRUE, PowerStage := TRUE);
Halt(Axis := AxisHalt, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Halt.Busy);
ASSERT_TRUE(Halt.Active);
ReadStatusHalt(Axis := AxisHalt, Enable := TRUE);
ASSERT_TRUE(ReadStatusHalt.DiscreteMotion);
ASSERT_FALSE(ReadStatusHalt.Stopping);
Halt(Axis := AxisHalt, Execute := FALSE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Halt.Done);
ReadStatusHalt(Axis := AxisHalt, Enable := TRUE);
ASSERT_TRUE(ReadStatusHalt.Standstill);

AxisStop.AxisId := UDINT#123;
AxisStop.InternalIndex := UINT#27;
SeedStop(Axis := AxisStop, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcContinuousMotion, ApplyPowerStage := TRUE, PowerStage := TRUE);
Stop(Axis := AxisStop, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ASSERT_TRUE(Stop.Busy);
ASSERT_TRUE(Stop.Done);
ReadStatusStop(Axis := AxisStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusStop.Stopping);
Stop(Axis := AxisStop, Execute := FALSE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ASSERT_TRUE(Stop.Done);
ReadStatusStop(Axis := AxisStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusStop.Standstill);

AxisContinuous.AxisId := UDINT#124;
AxisContinuous.InternalIndex := UINT#28;
SeedContinuous(Axis := AxisContinuous, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveContinuousAbsolute(
    Axis := AxisContinuous,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#40.0,
    EndVelocity := REAL#3.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveContinuousAbsolute.Busy);
ASSERT_TRUE(MoveContinuousAbsolute.Active);
ASSERT_TRUE(MoveContinuousAbsolute.InEndVelocity);
ReadStatusContinuous(Axis := AxisContinuous, Enable := TRUE);
ASSERT_TRUE(ReadStatusContinuous.ContinuousMotion);
ReadActualVelocity(Axis := AxisContinuous, Enable := TRUE);
ASSERT_TRUE(ReadActualVelocity.Velocity = REAL#3.0);
MoveContinuousAbsolute(
    Axis := AxisContinuous,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#40.0,
    EndVelocity := REAL#3.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_FALSE(MoveContinuousAbsolute.InEndVelocity);
ReadStatusContinuous(Axis := AxisContinuous, Enable := TRUE);
ASSERT_TRUE(ReadStatusContinuous.Standstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_move_absolute_final_position_semantics
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveAbsolute : MC_MoveAbsolute;
    ReadActualPosition : MC_ReadActualPosition;
END_VAR

Axis.AxisId := UDINT#125;
Axis.InternalIndex := UINT#29;
Seed(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyActuals := TRUE,
    ActualPosition := REAL#10.0,
    SetPosition := REAL#10.0,
    CommandedPosition := REAL#10.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
MoveAbsolute(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#42.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveAbsolute.Busy);
ASSERT_TRUE(MoveAbsolute.Active);
ASSERT_FALSE(MoveAbsolute.Done);
MoveAbsolute(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#42.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveAbsolute.Done);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadActualPosition.Position = REAL#42.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_home_from_nonstandstill_entry_state
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Home : MC_Home;
    ReadStatus : MC_ReadStatus;
END_VAR

Axis.AxisId := UDINT#126;
Axis.InternalIndex := UINT#30;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcDiscreteMotion, ApplyPowerStage := TRUE, PowerStage := TRUE);
Home(Axis := Axis, Execute := TRUE, Position := REAL#5.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(Home.Busy);
ASSERT_TRUE(Home.Active);
ASSERT_FALSE(Home.Error);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Homing);
Home(Axis := Axis, Execute := FALSE, Position := REAL#5.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(Home.Done);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Standstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_dynamic_max_parameter_rejections
VAR
    Axis : AXIS_REF;
    Constants : MC_Constants;
    WriteParameter : MC_WriteParameter;
END_VAR

Axis.AxisId := UDINT#127;
Axis.InternalIndex := UINT#31;
Constants();
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#0.0 - REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#0.0 - REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxDecelerationAppl, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxDecelerationAppl, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxJerkAppl, Value := REAL#0.0 - REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameter.Error);
ASSERT_TRUE(WriteParameter.ErrorID = WORD#16#0001);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxJerkAppl, Value := REAL#0.0 - REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
END_TEST_PROGRAM
