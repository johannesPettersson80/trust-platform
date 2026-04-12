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
Seed(Axis := Axis, ResetDefaults := FALSE, ApplyPowerStage := TRUE, PowerStage := FALSE);
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
Seed(Axis := Axis, ResetDefaults := FALSE, ApplyState := FALSE, ApplyPowerStage := FALSE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
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
Seed(Axis := Axis, ResetDefaults := FALSE, ApplyActuals := FALSE, ApplyMotionFlags := TRUE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := TRUE);
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

TEST_PROGRAM plcopen_motion_single_axis_numeric_parameter_position_and_limit_roundtrip
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Constants : MC_Constants;
    WriteParameter : MC_WriteParameter;
    ReadParameter : MC_ReadParameter;
END_VAR

Axis.AxisId := UDINT#114;
Axis.InternalIndex := UINT#18;
Seed(Axis := Axis, ResetDefaults := TRUE);
Constants();
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_CommandedPosition, Value := REAL#41.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_CommandedPosition, Value := REAL#41.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitNeg, Value := REAL#0.0 - REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitNeg, Value := REAL#0.0 - REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxPositionLag, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxPositionLag, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#41.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_SWLimitPos);
ASSERT_TRUE(ReadParameter.Value = REAL#90.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_SWLimitNeg);
ASSERT_TRUE(ReadParameter.Value = REAL#0.0 - REAL#90.0);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxPositionLag);
ASSERT_TRUE(ReadParameter.Value = REAL#12.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_numeric_parameter_dynamics_roundtrip
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Constants : MC_Constants;
    WriteParameter : MC_WriteParameter;
    ReadParameter : MC_ReadParameter;
END_VAR

Axis.AxisId := UDINT#115;
Axis.InternalIndex := UINT#19;
Seed(Axis := Axis, ResetDefaults := TRUE);
Constants();
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
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_bool_parameter_roundtrip
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Constants : MC_Constants;
    WriteBoolParameter : MC_WriteBoolParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
END_VAR

Axis.AxisId := UDINT#116;
Axis.InternalIndex := UINT#20;
Seed(Axis := Axis, ResetDefaults := TRUE);
Constants();
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

TEST_PROGRAM plcopen_motion_single_axis_missing_inputs_reuse_previous_invocation_values
VAR
    AxisFresh : AXIS_REF;
    AxisReuse : AXIS_REF;
    SeedFresh : TRUST_MOTION_TestAxisSeed;
    SeedReuse : TRUST_MOTION_TestAxisSeed;
    MoveFresh : MC_MoveAbsolute;
    MoveReuse : MC_MoveAbsolute;
    ReadFresh : MC_ReadParameter;
    ReadReuse : MC_ReadParameter;
    Constants : MC_Constants;
END_VAR

Constants();

AxisFresh.AxisId := UDINT#1161;
AxisFresh.InternalIndex := UINT#20;
SeedFresh(
    Axis := AxisFresh,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill
);
MoveFresh(
    Axis := AxisFresh,
    Execute := TRUE,
    Position := REAL#25.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadFresh(Axis := AxisFresh, Enable := TRUE, ParameterNumber := Constants.PN_CommandedVelocity);
ASSERT_TRUE(ReadFresh.Valid);
ASSERT_TRUE(ReadFresh.Value = REAL#0.0);

AxisReuse.AxisId := UDINT#1162;
AxisReuse.InternalIndex := UINT#21;
SeedReuse(
    Axis := AxisReuse,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill
);
MoveReuse(
    Axis := AxisReuse,
    Execute := TRUE,
    Position := REAL#40.0,
    Velocity := REAL#7.5,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveReuse(
    Axis := AxisReuse,
    Execute := FALSE,
    Position := REAL#40.0,
    Velocity := REAL#7.5,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveReuse(
    Axis := AxisReuse,
    Execute := TRUE,
    Position := REAL#60.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadReuse(Axis := AxisReuse, Enable := TRUE, ParameterNumber := Constants.PN_CommandedVelocity);
ASSERT_TRUE(ReadReuse.Valid);
ASSERT_TRUE(ReadReuse.Value = REAL#7.5);
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

TEST_PROGRAM plcopen_motion_single_axis_sign_rules_for_motion_inputs
VAR
    AxisPositionNegative : AXIS_REF;
    AxisDistanceNegative : AXIS_REF;
    AxisVelocityNegative : AXIS_REF;
    AxisNegativeAcceleration : AXIS_REF;
    AxisNegativeDeceleration : AXIS_REF;
    AxisNegativeJerk : AXIS_REF;
    SeedPositionNegative : TRUST_MOTION_TestAxisSeed;
    SeedDistanceNegative : TRUST_MOTION_TestAxisSeed;
    SeedVelocityNegative : TRUST_MOTION_TestAxisSeed;
    SeedNegativeAcceleration : TRUST_MOTION_TestAxisSeed;
    SeedNegativeDeceleration : TRUST_MOTION_TestAxisSeed;
    SeedNegativeJerk : TRUST_MOTION_TestAxisSeed;
    MoveAbsoluteNegativePosition : MC_MoveAbsolute;
    MoveRelativeNegativeDistance : MC_MoveRelative;
    MoveVelocityNegativeVelocity : MC_MoveVelocity;
    MoveAbsoluteNegativeAcceleration : MC_MoveAbsolute;
    HaltNegativeDeceleration : MC_Halt;
    MoveContinuousNegativeJerk : MC_MoveContinuousAbsolute;
    ReadPositionNegative : MC_ReadActualPosition;
    ReadDistanceNegative : MC_ReadActualPosition;
    ReadVelocityNegative : MC_ReadMotionState;
END_VAR

AxisPositionNegative.AxisId := UDINT#126;
AxisPositionNegative.InternalIndex := UINT#29;
SeedPositionNegative(Axis := AxisPositionNegative, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveAbsoluteNegativePosition(
    Axis := AxisPositionNegative,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#0.0 - REAL#12.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveAbsoluteNegativePosition(
    Axis := AxisPositionNegative,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#0.0 - REAL#12.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_FALSE(MoveAbsoluteNegativePosition.Error);
ReadPositionNegative(Axis := AxisPositionNegative, Enable := TRUE);
ASSERT_TRUE(ReadPositionNegative.Position = REAL#0.0 - REAL#12.0);

AxisDistanceNegative.AxisId := UDINT#127;
AxisDistanceNegative.InternalIndex := UINT#30;
SeedDistanceNegative(
    Axis := AxisDistanceNegative,
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
MoveRelativeNegativeDistance(
    Axis := AxisDistanceNegative,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := REAL#0.0 - REAL#4.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveRelativeNegativeDistance(
    Axis := AxisDistanceNegative,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Distance := REAL#0.0 - REAL#4.0,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_FALSE(MoveRelativeNegativeDistance.Error);
ReadDistanceNegative(Axis := AxisDistanceNegative, Enable := TRUE);
ASSERT_TRUE(ReadDistanceNegative.Position = REAL#6.0);

AxisVelocityNegative.AxisId := UDINT#128;
AxisVelocityNegative.InternalIndex := UINT#31;
SeedVelocityNegative(Axis := AxisVelocityNegative, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveVelocityNegativeVelocity(
    Axis := AxisVelocityNegative,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Velocity := REAL#0.0 - REAL#3.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_FALSE(MoveVelocityNegativeVelocity.Error);
ReadVelocityNegative(Axis := AxisVelocityNegative, Enable := TRUE, Source := MC_SOURCE#mcActualValue);
ASSERT_TRUE(ReadVelocityNegative.DirectionNegative);

AxisNegativeAcceleration.AxisId := UDINT#129;
AxisNegativeAcceleration.InternalIndex := UINT#24;
SeedNegativeAcceleration(Axis := AxisNegativeAcceleration, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveAbsoluteNegativeAcceleration(
    Axis := AxisNegativeAcceleration,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#5.0,
    Velocity := REAL#4.0,
    Acceleration := REAL#0.0 - REAL#1.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveAbsoluteNegativeAcceleration.Error);
ASSERT_TRUE(MoveAbsoluteNegativeAcceleration.ErrorID = WORD#16#0001);

AxisNegativeDeceleration.AxisId := UDINT#130;
AxisNegativeDeceleration.InternalIndex := UINT#25;
SeedNegativeDeceleration(Axis := AxisNegativeDeceleration, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcContinuousMotion);
HaltNegativeDeceleration(
    Axis := AxisNegativeDeceleration,
    Execute := TRUE,
    Deceleration := REAL#0.0 - REAL#1.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(HaltNegativeDeceleration.Error);
ASSERT_TRUE(HaltNegativeDeceleration.ErrorID = WORD#16#0001);

AxisNegativeJerk.AxisId := UDINT#131;
AxisNegativeJerk.InternalIndex := UINT#26;
SeedNegativeJerk(Axis := AxisNegativeJerk, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveContinuousNegativeJerk(
    Axis := AxisNegativeJerk,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    EndVelocity := REAL#2.0,
    Velocity := REAL#6.0,
    Acceleration := REAL#3.0,
    Deceleration := REAL#3.0,
    Jerk := REAL#0.0 - REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveContinuousNegativeJerk.Error);
ASSERT_TRUE(MoveContinuousNegativeJerk.ErrorID = WORD#16#0001);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_zero_dynamics_use_configured_axis_maxima
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    Constants : MC_Constants;
    WriteParameter : MC_WriteParameter;
    MoveAbsolute : MC_MoveAbsolute;
    Probe : TRUST_MOTION_TestAxisProbe;
END_VAR

Axis.AxisId := UDINT#132;
Axis.InternalIndex := UINT#27;
Constants();
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxAccelerationAppl, Value := REAL#12.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxDecelerationAppl, Value := REAL#13.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxDecelerationAppl, Value := REAL#13.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := TRUE, ParameterNumber := Constants.PN_MaxJerkAppl, Value := REAL#14.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameter(Axis := Axis, Execute := FALSE, ParameterNumber := Constants.PN_MaxJerkAppl, Value := REAL#14.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
MoveAbsolute(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#0.0,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_FALSE(MoveAbsolute.Error);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.ResolvedAcceleration = REAL#12.0);
ASSERT_TRUE(Probe.ResolvedDeceleration = REAL#13.0);
ASSERT_TRUE(Probe.ResolvedJerk = REAL#14.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_buffered_activation_and_fifo_queue
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveActive : MC_MoveAbsolute;
    MoveBuffered1 : MC_MoveAbsolute;
    MoveBuffered2 : MC_MoveAbsolute;
    MoveQueueFull : MC_MoveAbsolute;
    MoveUnsupported : MC_MoveAbsolute;
    ReadParameter : MC_ReadParameter;
    Probe : TRUST_MOTION_TestAxisQueueProbe;
    Constants : MC_Constants;
END_VAR

Axis.AxisId := UDINT#133;
Axis.InternalIndex := UINT#28;
Constants();
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActive(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#10.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveActive.Busy);
ASSERT_TRUE(MoveActive.Active);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#0);
ASSERT_TRUE(Probe.ActiveCommandID <> UINT#0);

MoveBuffered1(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveBuffered1.Busy);
ASSERT_FALSE(MoveBuffered1.Active);
ASSERT_FALSE(MoveBuffered1.Error);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#1);
ASSERT_TRUE(Probe.Queue0CommandID <> UINT#0);
ASSERT_TRUE(Probe.Queue0BufferMode = MC_BUFFER_MODE#mcBuffered);

MoveBuffered2(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveBuffered2.Busy);
ASSERT_FALSE(MoveBuffered2.Active);
ASSERT_FALSE(MoveBuffered2.Error);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#2);
ASSERT_TRUE(Probe.Queue1CommandID <> UINT#0);
ASSERT_TRUE(Probe.Queue1BufferMode = MC_BUFFER_MODE#mcBuffered);

MoveQueueFull(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#40.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveQueueFull.Error);
ASSERT_TRUE(MoveQueueFull.ErrorID = WORD#16#0400);

MoveUnsupported(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#50.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBlendingLow
);
ASSERT_TRUE(MoveUnsupported.Error);
ASSERT_TRUE(MoveUnsupported.ErrorID = WORD#16#0500);

MoveActive(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#10.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveActive.Done);
MoveBuffered1(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveBuffered1.Active);
ASSERT_TRUE(MoveBuffered1.Busy);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#20.0);

MoveBuffered1(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveBuffered1.Done);
MoveBuffered2(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveBuffered2.Active);
ASSERT_TRUE(MoveBuffered2.Busy);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadParameter.Value = REAL#30.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_active_fault_drives_errorstop_and_clears_queue
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveActive : MC_MoveAbsolute;
    MoveBuffered1 : MC_MoveAbsolute;
    MoveBuffered2 : MC_MoveAbsolute;
    ReadStatus : MC_ReadStatus;
    Probe : TRUST_MOTION_TestAxisQueueProbe;
END_VAR

Axis.AxisId := UDINT#134;
Axis.InternalIndex := UINT#29;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActive(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#10.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveBuffered1(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveBuffered2(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
Seed(Axis := Axis, ResetDefaults := FALSE, ApplyState := FALSE, ApplyPowerStage := FALSE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
MoveActive(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#10.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveActive.Error);
ASSERT_TRUE(MoveActive.ErrorID = WORD#16#0200);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.ErrorStop);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#0);
ASSERT_TRUE(Probe.ActiveCommandID = UINT#0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_buffered_commands_report_commandaborted_after_active_fault
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveActive : MC_MoveAbsolute;
    MoveBuffered1 : MC_MoveAbsolute;
    MoveBuffered2 : MC_MoveAbsolute;
END_VAR

Axis.AxisId := UDINT#137;
Axis.InternalIndex := UINT#29;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActive(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#10.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveBuffered1(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveBuffered2(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
Seed(Axis := Axis, ResetDefaults := FALSE, ApplyState := FALSE, ApplyPowerStage := FALSE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
MoveActive(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#10.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveBuffered1(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveBuffered2(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveBuffered1.CommandAborted);
ASSERT_TRUE(MoveBuffered2.CommandAborted);
ASSERT_FALSE(MoveBuffered1.Busy);
ASSERT_FALSE(MoveBuffered2.Busy);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_move_relative_continuous_update_uses_command_start_reference
VAR
    AxisRelative : AXIS_REF;
    SeedRelative : TRUST_MOTION_TestAxisSeed;
    MoveRelative : MC_MoveRelative;
    ReadRelative : MC_ReadParameter;
    ReadActualRelative : MC_ReadActualPosition;
    Constants : MC_Constants;
END_VAR

Constants();
AxisRelative.AxisId := UDINT#135;
AxisRelative.InternalIndex := UINT#30;
SeedRelative(
    Axis := AxisRelative,
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
MoveRelative(
    Axis := AxisRelative,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := REAL#5.0,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveRelative(
    Axis := AxisRelative,
    Execute := TRUE,
    ContinuousUpdate := TRUE,
    Distance := REAL#8.0,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadRelative(Axis := AxisRelative, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadRelative.Value = REAL#18.0);
MoveRelative(
    Axis := AxisRelative,
    Execute := FALSE,
    ContinuousUpdate := TRUE,
    Distance := REAL#8.0,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadActualRelative(Axis := AxisRelative, Enable := TRUE);
ASSERT_TRUE(ReadActualRelative.Position = REAL#18.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_buffered_queued_updates_are_ignored_until_new_execute
VAR
    AxisQueued : AXIS_REF;
    SeedQueued : TRUST_MOTION_TestAxisSeed;
    MoveActive : MC_MoveAbsolute;
    MoveQueued : MC_MoveAbsolute;
    ReadQueued : MC_ReadParameter;
    Constants : MC_Constants;
END_VAR

Constants();
AxisQueued.AxisId := UDINT#136;
AxisQueued.InternalIndex := UINT#31;
SeedQueued(Axis := AxisQueued, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActive(
    Axis := AxisQueued,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveQueued(
    Axis := AxisQueued,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#30.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveQueued(
    Axis := AxisQueued,
    Execute := TRUE,
    ContinuousUpdate := TRUE,
    Position := REAL#45.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_FALSE(MoveQueued.Active);
MoveActive(
    Axis := AxisQueued,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveQueued(
    Axis := AxisQueued,
    Execute := TRUE,
    ContinuousUpdate := TRUE,
    Position := REAL#45.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
ASSERT_TRUE(MoveQueued.Active);
ReadQueued(Axis := AxisQueued, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadQueued.Value = REAL#30.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_queued_parameter_writes_follow_motion_queue_order
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveActive : MC_MoveAbsolute;
    WriteQueuedReal : MC_WriteParameter;
    WriteQueuedBool : MC_WriteBoolParameter;
    ReadParameter : MC_ReadParameter;
    ReadBoolParameter : MC_ReadBoolParameter;
    Probe : TRUST_MOTION_TestAxisQueueProbe;
    Constants : MC_Constants;
END_VAR

Constants();
Axis.AxisId := UDINT#138;
Axis.InternalIndex := UINT#27;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActive(
    Axis := Axis,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
WriteQueuedReal(
    Axis := Axis,
    Execute := TRUE,
    ParameterNumber := Constants.PN_MaxVelocityAppl,
    Value := REAL#44.0,
    ExecutionMode := MC_EXECUTION_MODE#mcQueued
);
ASSERT_TRUE(WriteQueuedReal.Busy);
ASSERT_FALSE(WriteQueuedReal.Done);
ASSERT_FALSE(WriteQueuedReal.Error);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#1);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl);
ASSERT_TRUE(ReadParameter.Value = REAL#100.0);
WriteQueuedBool(
    Axis := Axis,
    Execute := TRUE,
    ParameterNumber := Constants.PN_EnableLimitPos,
    Value := TRUE,
    ExecutionMode := MC_EXECUTION_MODE#mcQueued
);
ASSERT_TRUE(WriteQueuedBool.Busy);
ASSERT_FALSE(WriteQueuedBool.Done);
ASSERT_FALSE(WriteQueuedBool.Error);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#2);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_FALSE(ReadBoolParameter.Value);
MoveActive(
    Axis := Axis,
    Execute := FALSE,
    ContinuousUpdate := FALSE,
    Position := REAL#20.0,
    Velocity := REAL#5.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(MoveActive.Done);
WriteQueuedReal(
    Axis := Axis,
    Execute := TRUE,
    ParameterNumber := Constants.PN_MaxVelocityAppl,
    Value := REAL#44.0,
    ExecutionMode := MC_EXECUTION_MODE#mcQueued
);
ASSERT_TRUE(WriteQueuedReal.Done);
ASSERT_FALSE(WriteQueuedReal.Busy);
ASSERT_FALSE(WriteQueuedReal.Error);
ReadParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl);
ASSERT_TRUE(ReadParameter.Value = REAL#44.0);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#1);
WriteQueuedBool(
    Axis := Axis,
    Execute := TRUE,
    ParameterNumber := Constants.PN_EnableLimitPos,
    Value := TRUE,
    ExecutionMode := MC_EXECUTION_MODE#mcQueued
);
ASSERT_TRUE(WriteQueuedBool.Done);
ASSERT_FALSE(WriteQueuedBool.Busy);
ASSERT_FALSE(WriteQueuedBool.Error);
ReadBoolParameter(Axis := Axis, Enable := TRUE, ParameterNumber := Constants.PN_EnableLimitPos);
ASSERT_TRUE(ReadBoolParameter.Value);
Probe(Axis := Axis);
ASSERT_TRUE(Probe.QueueDepth = UINT#0);
ASSERT_TRUE(Probe.ActiveCommandID = UINT#0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_buffer_mode_support_rules_by_fb_family
VAR
    AxisHomeBuffered : AXIS_REF;
    AxisHomeUnsupported : AXIS_REF;
    AxisHaltUnsupported : AXIS_REF;
    AxisAdditiveUnsupported : AXIS_REF;
    AxisVelocityBuffered : AXIS_REF;
    AxisVelocityUnsupported : AXIS_REF;
    AxisContinuousBuffered : AXIS_REF;
    AxisContinuousUnsupported : AXIS_REF;
    SeedHomeBuffered : TRUST_MOTION_TestAxisSeed;
    SeedHomeUnsupported : TRUST_MOTION_TestAxisSeed;
    SeedHaltUnsupported : TRUST_MOTION_TestAxisSeed;
    SeedAdditiveUnsupported : TRUST_MOTION_TestAxisSeed;
    SeedVelocityBuffered : TRUST_MOTION_TestAxisSeed;
    SeedVelocityUnsupported : TRUST_MOTION_TestAxisSeed;
    SeedContinuousBuffered : TRUST_MOTION_TestAxisSeed;
    SeedContinuousUnsupported : TRUST_MOTION_TestAxisSeed;
    HomeBuffered : MC_Home;
    HomeUnsupported : MC_Home;
    HaltUnsupported : MC_Halt;
    MoveAdditiveUnsupported : MC_MoveAdditive;
    MoveVelocityBuffered : MC_MoveVelocity;
    MoveVelocityUnsupported : MC_MoveVelocity;
    MoveContinuousAbsoluteBuffered : MC_MoveContinuousAbsolute;
    MoveContinuousRelativeUnsupported : MC_MoveContinuousRelative;
END_VAR

AxisHomeBuffered.AxisId := UDINT#139;
AxisHomeBuffered.InternalIndex := UINT#18;
SeedHomeBuffered(Axis := AxisHomeBuffered, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
HomeBuffered(Axis := AxisHomeBuffered, Execute := TRUE, Position := REAL#5.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(HomeBuffered.Busy);
ASSERT_TRUE(HomeBuffered.Active);
ASSERT_FALSE(HomeBuffered.Error);

AxisHomeUnsupported.AxisId := UDINT#140;
AxisHomeUnsupported.InternalIndex := UINT#19;
SeedHomeUnsupported(Axis := AxisHomeUnsupported, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
HomeUnsupported(Axis := AxisHomeUnsupported, Execute := TRUE, Position := REAL#5.0, BufferMode := MC_BUFFER_MODE#mcBlendingLow);
ASSERT_TRUE(HomeUnsupported.Error);
ASSERT_TRUE(HomeUnsupported.ErrorID = WORD#16#0500);

AxisHaltUnsupported.AxisId := UDINT#141;
AxisHaltUnsupported.InternalIndex := UINT#20;
SeedHaltUnsupported(Axis := AxisHaltUnsupported, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcContinuousMotion);
HaltUnsupported(Axis := AxisHaltUnsupported, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBlendingLow);
ASSERT_TRUE(HaltUnsupported.Error);
ASSERT_TRUE(HaltUnsupported.ErrorID = WORD#16#0500);

AxisAdditiveUnsupported.AxisId := UDINT#142;
AxisAdditiveUnsupported.InternalIndex := UINT#21;
SeedAdditiveUnsupported(
    Axis := AxisAdditiveUnsupported,
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
MoveAdditiveUnsupported(Axis := AxisAdditiveUnsupported, Execute := TRUE, ContinuousUpdate := FALSE, Distance := REAL#5.0, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBlendingLow);
ASSERT_TRUE(MoveAdditiveUnsupported.Error);
ASSERT_TRUE(MoveAdditiveUnsupported.ErrorID = WORD#16#0500);

AxisVelocityBuffered.AxisId := UDINT#143;
AxisVelocityBuffered.InternalIndex := UINT#22;
SeedVelocityBuffered(Axis := AxisVelocityBuffered, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveVelocityBuffered(Axis := AxisVelocityBuffered, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveVelocityBuffered.Busy);
ASSERT_TRUE(MoveVelocityBuffered.Active);
ASSERT_TRUE(MoveVelocityBuffered.InVelocity);
ASSERT_FALSE(MoveVelocityBuffered.Error);

AxisVelocityUnsupported.AxisId := UDINT#144;
AxisVelocityUnsupported.InternalIndex := UINT#23;
SeedVelocityUnsupported(Axis := AxisVelocityUnsupported, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveVelocityUnsupported(Axis := AxisVelocityUnsupported, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcBlendingLow);
ASSERT_TRUE(MoveVelocityUnsupported.Error);
ASSERT_TRUE(MoveVelocityUnsupported.ErrorID = WORD#16#0500);

AxisContinuousBuffered.AxisId := UDINT#145;
AxisContinuousBuffered.InternalIndex := UINT#24;
SeedContinuousBuffered(Axis := AxisContinuousBuffered, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveContinuousAbsoluteBuffered(Axis := AxisContinuousBuffered, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#30.0, EndVelocity := REAL#3.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveContinuousAbsoluteBuffered.Busy);
ASSERT_TRUE(MoveContinuousAbsoluteBuffered.Active);
ASSERT_TRUE(MoveContinuousAbsoluteBuffered.InEndVelocity);
ASSERT_FALSE(MoveContinuousAbsoluteBuffered.Error);

AxisContinuousUnsupported.AxisId := UDINT#146;
AxisContinuousUnsupported.InternalIndex := UINT#25;
SeedContinuousUnsupported(Axis := AxisContinuousUnsupported, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveContinuousRelativeUnsupported(Axis := AxisContinuousUnsupported, Execute := TRUE, ContinuousUpdate := FALSE, Distance := REAL#5.0, EndVelocity := REAL#3.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBlendingLow);
ASSERT_TRUE(MoveContinuousRelativeUnsupported.Error);
ASSERT_TRUE(MoveContinuousRelativeUnsupported.ErrorID = WORD#16#0500);
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

TEST_PROGRAM plcopen_motion_single_axis_continuous_update_changes_target_and_end_velocity
VAR
    AxisAbsolute : AXIS_REF;
    AxisRelative : AXIS_REF;
    SeedAbsolute : TRUST_MOTION_TestAxisSeed;
    SeedRelative : TRUST_MOTION_TestAxisSeed;
    MoveContinuousAbsolute : MC_MoveContinuousAbsolute;
    MoveContinuousRelative : MC_MoveContinuousRelative;
    ReadAbsolute : MC_ReadParameter;
    ReadRelative : MC_ReadParameter;
    ReadVelocityAbsolute : MC_ReadActualVelocity;
    ReadVelocityRelative : MC_ReadActualVelocity;
    Constants : MC_Constants;
END_VAR

Constants();
AxisAbsolute.AxisId := UDINT#147;
AxisAbsolute.InternalIndex := UINT#26;
SeedAbsolute(
    Axis := AxisAbsolute,
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
MoveContinuousAbsolute(Axis := AxisAbsolute, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#40.0, EndVelocity := REAL#3.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveContinuousAbsolute(Axis := AxisAbsolute, Execute := TRUE, ContinuousUpdate := TRUE, Position := REAL#55.0, EndVelocity := REAL#4.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveContinuousAbsolute.InEndVelocity);
ReadAbsolute(Axis := AxisAbsolute, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadAbsolute.Value = REAL#55.0);
ReadVelocityAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ASSERT_TRUE(ReadVelocityAbsolute.Velocity = REAL#4.0);

AxisRelative.AxisId := UDINT#148;
AxisRelative.InternalIndex := UINT#27;
SeedRelative(
    Axis := AxisRelative,
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
MoveContinuousRelative(Axis := AxisRelative, Execute := TRUE, ContinuousUpdate := FALSE, Distance := REAL#5.0, EndVelocity := REAL#2.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveContinuousRelative(Axis := AxisRelative, Execute := TRUE, ContinuousUpdate := TRUE, Distance := REAL#8.0, EndVelocity := REAL#4.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveContinuousRelative.InEndVelocity);
ReadRelative(Axis := AxisRelative, Enable := TRUE, ParameterNumber := Constants.PN_CommandedPosition);
ASSERT_TRUE(ReadRelative.Value = REAL#18.0);
ReadVelocityRelative(Axis := AxisRelative, Enable := TRUE);
ASSERT_TRUE(ReadVelocityRelative.Velocity = REAL#4.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_continuous_commandaborted_resets_in_end_velocity
VAR
    Axis : AXIS_REF;
    Seed : TRUST_MOTION_TestAxisSeed;
    MoveContinuous : MC_MoveContinuousAbsolute;
    MoveAbort : MC_MoveAbsolute;
    ReadStatus : MC_ReadStatus;
END_VAR

Axis.AxisId := UDINT#149;
Axis.InternalIndex := UINT#28;
Seed(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveContinuous(Axis := Axis, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#30.0, EndVelocity := REAL#3.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveContinuous.InEndVelocity);
MoveAbort(Axis := Axis, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#12.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbort.Active);
MoveContinuous(Axis := Axis, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#30.0, EndVelocity := REAL#3.0, Velocity := REAL#8.0, Acceleration := REAL#4.0, Deceleration := REAL#4.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveContinuous.CommandAborted);
ASSERT_FALSE(MoveContinuous.InEndVelocity);
ASSERT_FALSE(MoveContinuous.Busy);
ASSERT_FALSE(MoveContinuous.Active);
ReadStatus(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadStatus.DiscreteMotion);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_buffered_home_halt_and_additive_take_ownership_after_active_motion
VAR
    AxisHome : AXIS_REF;
    AxisHalt : AXIS_REF;
    AxisAdditive : AXIS_REF;
    SeedHome : TRUST_MOTION_TestAxisSeed;
    SeedHalt : TRUST_MOTION_TestAxisSeed;
    SeedAdditive : TRUST_MOTION_TestAxisSeed;
    MoveActiveHome : MC_MoveAbsolute;
    MoveActiveHalt : MC_MoveAbsolute;
    MoveActiveAdditive : MC_MoveAbsolute;
    Home : MC_Home;
    Halt : MC_Halt;
    MoveAdditive : MC_MoveAdditive;
    ReadStatusHome : MC_ReadStatus;
    ReadStatusHalt : MC_ReadStatus;
    ReadAxisInfoHome : MC_ReadAxisInfo;
    ReadActualAdditive : MC_ReadActualPosition;
END_VAR

AxisHome.AxisId := UDINT#150;
AxisHome.InternalIndex := UINT#29;
SeedHome(Axis := AxisHome, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActiveHome(Axis := AxisHome, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#15.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
Home(Axis := AxisHome, Execute := TRUE, Position := REAL#7.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Home.Busy);
ASSERT_FALSE(Home.Active);
MoveActiveHome(Axis := AxisHome, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#15.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
Home(Axis := AxisHome, Execute := TRUE, Position := REAL#7.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Home.Active);
ReadStatusHome(Axis := AxisHome, Enable := TRUE);
ASSERT_TRUE(ReadStatusHome.Homing);
Home(Axis := AxisHome, Execute := FALSE, Position := REAL#7.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Home.Done);
ReadAxisInfoHome(Axis := AxisHome, Enable := TRUE);
ASSERT_TRUE(ReadAxisInfoHome.IsHomed);

AxisHalt.AxisId := UDINT#151;
AxisHalt.InternalIndex := UINT#30;
SeedHalt(Axis := AxisHalt, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActiveHalt(Axis := AxisHalt, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#20.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
Halt(Axis := AxisHalt, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Halt.Busy);
ASSERT_FALSE(Halt.Active);
MoveActiveHalt(Axis := AxisHalt, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#20.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
Halt(Axis := AxisHalt, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Halt.Active);
ReadStatusHalt(Axis := AxisHalt, Enable := TRUE);
ASSERT_TRUE(ReadStatusHalt.DiscreteMotion);
Halt(Axis := AxisHalt, Execute := FALSE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(Halt.Done);
ReadStatusHalt(Axis := AxisHalt, Enable := TRUE);
ASSERT_TRUE(ReadStatusHalt.Standstill);

AxisAdditive.AxisId := UDINT#152;
AxisAdditive.InternalIndex := UINT#31;
SeedAdditive(
    Axis := AxisAdditive,
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
MoveActiveAdditive(Axis := AxisAdditive, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#15.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveAdditive(Axis := AxisAdditive, Execute := TRUE, ContinuousUpdate := FALSE, Distance := REAL#5.0, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveAdditive.Busy);
ASSERT_FALSE(MoveAdditive.Active);
MoveActiveAdditive(Axis := AxisAdditive, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#15.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveAdditive(Axis := AxisAdditive, Execute := TRUE, ContinuousUpdate := FALSE, Distance := REAL#5.0, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveAdditive.Active);
MoveAdditive(Axis := AxisAdditive, Execute := FALSE, ContinuousUpdate := FALSE, Distance := REAL#5.0, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveAdditive.Done);
ReadActualAdditive(Axis := AxisAdditive, Enable := TRUE);
ASSERT_TRUE(ReadActualAdditive.Position = REAL#20.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_move_velocity_participates_in_abort_and_buffer_queue
VAR
    AxisAbort : AXIS_REF;
    AxisBuffered : AXIS_REF;
    SeedAbort : TRUST_MOTION_TestAxisSeed;
    SeedBuffered : TRUST_MOTION_TestAxisSeed;
    MoveVelocityAbort : MC_MoveVelocity;
    MoveAbort : MC_MoveAbsolute;
    MoveActiveBuffered : MC_MoveAbsolute;
    MoveVelocityBuffered : MC_MoveVelocity;
    ReadStatusBuffered : MC_ReadStatus;
END_VAR

AxisAbort.AxisId := UDINT#153;
AxisAbort.InternalIndex := UINT#18;
SeedAbort(Axis := AxisAbort, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveVelocityAbort(Axis := AxisAbort, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveVelocityAbort.Active);
ASSERT_TRUE(MoveVelocityAbort.InVelocity);
MoveAbort(Axis := AxisAbort, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#12.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbort.Active);
MoveVelocityAbort(Axis := AxisAbort, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveVelocityAbort.CommandAborted);
ASSERT_FALSE(MoveVelocityAbort.InVelocity);
ASSERT_FALSE(MoveVelocityAbort.Active);

AxisBuffered.AxisId := UDINT#154;
AxisBuffered.InternalIndex := UINT#19;
SeedBuffered(Axis := AxisBuffered, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
MoveActiveBuffered(Axis := AxisBuffered, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#16.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveVelocityBuffered(Axis := AxisBuffered, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveVelocityBuffered.Busy);
ASSERT_FALSE(MoveVelocityBuffered.Active);
MoveActiveBuffered(Axis := AxisBuffered, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#16.0, Velocity := REAL#5.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveVelocityBuffered(Axis := AxisBuffered, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(MoveVelocityBuffered.Active);
ASSERT_TRUE(MoveVelocityBuffered.InVelocity);
ReadStatusBuffered(Axis := AxisBuffered, Enable := TRUE);
ASSERT_TRUE(ReadStatusBuffered.ContinuousMotion);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_single_axis_software_limits_clamp_position_target_commands
VAR
    AxisAbsoluteClamped : AXIS_REF;
    AxisRelativeClamped : AXIS_REF;
    AxisLimitDisabled : AXIS_REF;
    SeedAbsolute : TRUST_MOTION_TestAxisSeed;
    SeedRelative : TRUST_MOTION_TestAxisSeed;
    SeedDisabled : TRUST_MOTION_TestAxisSeed;
    Constants : MC_Constants;
    WriteParameterAbsolute : MC_WriteParameter;
    WriteBoolAbsolute : MC_WriteBoolParameter;
    MoveAbsolute : MC_MoveAbsolute;
    ReadActualAbsolute : MC_ReadActualPosition;
    WriteParameterRelative : MC_WriteParameter;
    WriteBoolRelative : MC_WriteBoolParameter;
    MoveRelative : MC_MoveRelative;
    ReadActualRelative : MC_ReadActualPosition;
    WriteParameterDisabled : MC_WriteParameter;
    MoveAbsoluteDisabled : MC_MoveAbsolute;
    ReadActualDisabled : MC_ReadActualPosition;
END_VAR

Constants();
AxisAbsoluteClamped.AxisId := UDINT#156;
AxisAbsoluteClamped.InternalIndex := UINT#14;
SeedAbsolute(
    Axis := AxisAbsoluteClamped,
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
WriteParameterAbsolute(Axis := AxisAbsoluteClamped, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#25.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameterAbsolute(Axis := AxisAbsoluteClamped, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#25.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolAbsolute(Axis := AxisAbsoluteClamped, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolAbsolute(Axis := AxisAbsoluteClamped, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
MoveAbsolute(Axis := AxisAbsoluteClamped, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#40.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsolute.Active);
ASSERT_FALSE(MoveAbsolute.Error);
MoveAbsolute(Axis := AxisAbsoluteClamped, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#40.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsolute.Done);
ReadActualAbsolute(Axis := AxisAbsoluteClamped, Enable := TRUE);
ASSERT_TRUE(ReadActualAbsolute.Position = REAL#25.0);

AxisRelativeClamped.AxisId := UDINT#157;
AxisRelativeClamped.InternalIndex := UINT#15;
SeedRelative(
    Axis := AxisRelativeClamped,
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
WriteParameterRelative(Axis := AxisRelativeClamped, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitNeg, Value := REAL#0.0 - REAL#5.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameterRelative(Axis := AxisRelativeClamped, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitNeg, Value := REAL#0.0 - REAL#5.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolRelative(Axis := AxisRelativeClamped, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitNeg, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolRelative(Axis := AxisRelativeClamped, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitNeg, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
MoveRelative(Axis := AxisRelativeClamped, Execute := TRUE, ContinuousUpdate := FALSE, Distance := REAL#0.0 - REAL#30.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveRelative.Active);
ASSERT_FALSE(MoveRelative.Error);
MoveRelative(Axis := AxisRelativeClamped, Execute := FALSE, ContinuousUpdate := FALSE, Distance := REAL#0.0 - REAL#30.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveRelative.Done);
ReadActualRelative(Axis := AxisRelativeClamped, Enable := TRUE);
ASSERT_TRUE(ReadActualRelative.Position = REAL#0.0 - REAL#5.0);

AxisLimitDisabled.AxisId := UDINT#158;
AxisLimitDisabled.InternalIndex := UINT#16;
SeedDisabled(
    Axis := AxisLimitDisabled,
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
WriteParameterDisabled(Axis := AxisLimitDisabled, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#25.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameterDisabled(Axis := AxisLimitDisabled, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#25.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
MoveAbsoluteDisabled(Axis := AxisLimitDisabled, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#40.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsoluteDisabled.Active);
ASSERT_FALSE(MoveAbsoluteDisabled.Error);
MoveAbsoluteDisabled(Axis := AxisLimitDisabled, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#40.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsoluteDisabled.Done);
ReadActualDisabled(Axis := AxisLimitDisabled, Enable := TRUE);
ASSERT_TRUE(ReadActualDisabled.Position = REAL#40.0);
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

TEST_PROGRAM plcopen_motion_single_axis_end_to_end_conformance_scenario
VAR
    AxisPowerMotion : AXIS_REF;
    AxisHaltStop : AXIS_REF;
    AxisFaultReset : AXIS_REF;
    SeedPowerMotion : TRUST_MOTION_TestAxisSeed;
    SeedHaltStop : TRUST_MOTION_TestAxisSeed;
    SeedFaultReset : TRUST_MOTION_TestAxisSeed;
    Constants : MC_Constants;
    PowerPowerMotion : MC_Power;
    WriteParameterPowerMotion : MC_WriteParameter;
    WriteBoolPowerMotion : MC_WriteBoolParameter;
    MoveAbsolutePowerMotion : MC_MoveAbsolute;
    ReadStatusPowerMotion : MC_ReadStatus;
    ReadActualPositionPowerMotion : MC_ReadActualPosition;
    MoveVelocityHalt : MC_MoveVelocity;
    MoveVelocityStop : MC_MoveVelocity;
    HaltHaltStop : MC_Halt;
    StopHaltStop : MC_Stop;
    ReadStatusHaltStop : MC_ReadStatus;
    MoveAbsoluteFaultReset : MC_MoveAbsolute;
    ReadStatusFaultReset : MC_ReadStatus;
    ReadAxisErrorFaultReset : MC_ReadAxisError;
    ResetFaultReset : MC_Reset;
END_VAR

Constants();

AxisPowerMotion.AxisId := UDINT#159;
AxisPowerMotion.InternalIndex := UINT#17;
SeedPowerMotion(
    Axis := AxisPowerMotion,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcDisabled,
    ApplyPowerStage := TRUE,
    PowerStage := FALSE,
    ApplyActuals := TRUE,
    ActualPosition := REAL#10.0,
    SetPosition := REAL#10.0,
    CommandedPosition := REAL#10.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0
);
PowerPowerMotion(Axis := AxisPowerMotion, Enable := TRUE);
ASSERT_TRUE(PowerPowerMotion.Status);
ReadStatusPowerMotion(Axis := AxisPowerMotion, Enable := TRUE);
ASSERT_TRUE(ReadStatusPowerMotion.Standstill);
WriteParameterPowerMotion(Axis := AxisPowerMotion, Execute := TRUE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#8.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameterPowerMotion.Done);
WriteParameterPowerMotion(Axis := AxisPowerMotion, Execute := FALSE, ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#8.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameterPowerMotion(Axis := AxisPowerMotion, Execute := TRUE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#30.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteParameterPowerMotion.Done);
WriteParameterPowerMotion(Axis := AxisPowerMotion, Execute := FALSE, ParameterNumber := Constants.PN_SWLimitPos, Value := REAL#30.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolPowerMotion(Axis := AxisPowerMotion, Execute := TRUE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteBoolPowerMotion.Done);
WriteBoolPowerMotion(Axis := AxisPowerMotion, Execute := FALSE, ParameterNumber := Constants.PN_EnableLimitPos, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
MoveAbsolutePowerMotion(Axis := AxisPowerMotion, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#40.0, Velocity := REAL#8.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsolutePowerMotion.Active);
MoveAbsolutePowerMotion(Axis := AxisPowerMotion, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#40.0, Velocity := REAL#8.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsolutePowerMotion.Done);
ReadActualPositionPowerMotion(Axis := AxisPowerMotion, Enable := TRUE);
ASSERT_TRUE(ReadActualPositionPowerMotion.Position = REAL#30.0);

AxisHaltStop.AxisId := UDINT#160;
AxisHaltStop.InternalIndex := UINT#18;
SeedHaltStop(Axis := AxisHaltStop, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyPowerStage := TRUE, PowerStage := TRUE);
MoveVelocityHalt(Axis := AxisHaltStop, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#4.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveVelocityHalt.InVelocity);
HaltHaltStop(Axis := AxisHaltStop, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(HaltHaltStop.Active);
HaltHaltStop(Axis := AxisHaltStop, Execute := FALSE, Deceleration := REAL#2.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(HaltHaltStop.Done);
ReadStatusHaltStop(Axis := AxisHaltStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusHaltStop.Standstill);
MoveVelocityStop(Axis := AxisHaltStop, Execute := TRUE, ContinuousUpdate := FALSE, Velocity := REAL#3.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveVelocityStop.InVelocity);
StopHaltStop(Axis := AxisHaltStop, Execute := TRUE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ASSERT_TRUE(StopHaltStop.Busy);
ASSERT_TRUE(StopHaltStop.Done);
ReadStatusHaltStop(Axis := AxisHaltStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusHaltStop.Stopping);
StopHaltStop(Axis := AxisHaltStop, Execute := FALSE, Deceleration := REAL#2.0, Jerk := REAL#1.0);
ReadStatusHaltStop(Axis := AxisHaltStop, Enable := TRUE);
ASSERT_TRUE(ReadStatusHaltStop.Standstill);

AxisFaultReset.AxisId := UDINT#161;
AxisFaultReset.InternalIndex := UINT#19;
SeedFaultReset(Axis := AxisFaultReset, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyPowerStage := TRUE, PowerStage := TRUE);
MoveAbsoluteFaultReset(Axis := AxisFaultReset, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#22.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsoluteFaultReset.Active);
SeedFaultReset(Axis := AxisFaultReset, ResetDefaults := FALSE, ApplyState := FALSE, ApplyPowerStage := FALSE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
MoveAbsoluteFaultReset(Axis := AxisFaultReset, Execute := TRUE, ContinuousUpdate := FALSE, Position := REAL#22.0, Velocity := REAL#6.0, Acceleration := REAL#2.0, Deceleration := REAL#2.0, Jerk := REAL#1.0, Direction := MC_DIRECTION#mcCurrentDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(MoveAbsoluteFaultReset.Error);
ASSERT_TRUE(MoveAbsoluteFaultReset.ErrorID = WORD#16#0200);
ReadStatusFaultReset(Axis := AxisFaultReset, Enable := TRUE);
ASSERT_TRUE(ReadStatusFaultReset.ErrorStop);
ReadAxisErrorFaultReset(Axis := AxisFaultReset, Enable := TRUE);
ASSERT_TRUE(ReadAxisErrorFaultReset.Valid);
ASSERT_TRUE(ReadAxisErrorFaultReset.AxisErrorID = WORD#16#0200);
ResetFaultReset(Axis := AxisFaultReset, Execute := TRUE);
ASSERT_TRUE(ResetFaultReset.Done);
ResetFaultReset(Axis := AxisFaultReset, Execute := FALSE);
ReadStatusFaultReset(Axis := AxisFaultReset, Enable := TRUE);
ASSERT_TRUE(ReadStatusFaultReset.Standstill);
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
