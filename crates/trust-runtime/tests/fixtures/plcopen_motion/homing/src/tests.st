TEST_PROGRAM plcopen_motion_homing_public_types_resolve
VAR
    DirectionPositive : MC_HOME_DIRECTION;
    DirectionNegative : MC_HOME_DIRECTION;
    SwitchModeOn : MC_SWITCH_MODE;
    SwitchModeEdge : MC_SWITCH_MODE;
    ReferenceSignal : MC_REF_SIGNAL_REF;
END_VAR
DirectionPositive := MC_HOME_DIRECTION#mcPositiveDirection;
DirectionNegative := MC_HOME_DIRECTION#mcNegativeDirection;
SwitchModeOn := MC_SWITCH_MODE#mcOn;
SwitchModeEdge := MC_SWITCH_MODE#mcEdgeSwitchPositive;
ReferenceSignal.Signal := TRUE;
ReferenceSignal.PositionStamp := REAL#12.5;
ReferenceSignal.MarkerCode := UINT#7;
ASSERT_TRUE(DirectionPositive <> DirectionNegative);
ASSERT_TRUE(SwitchModeOn <> SwitchModeEdge);
ASSERT_TRUE(ReferenceSignal.Signal);
ASSERT_TRUE(ReferenceSignal.PositionStamp = REAL#12.5);
ASSERT_TRUE(ReferenceSignal.MarkerCode = UINT#7);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_simulated_signals_can_be_seeded
VAR
    Axis : AXIS_REF;
    SeedAxis : TRUST_MOTION_TestAxisSeed;
    SeedSignal : TRUST_HOME_TestSignalSeed;
    ProbeSignal : TRUST_HOME_TestSignalProbe;
    ReadAxisInfo : MC_ReadAxisInfo;
END_VAR
Axis.AxisId := UDINT#201;
Axis.InternalIndex := UINT#1;
SeedAxis(
    Axis := Axis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcStandstill,
    ApplyGrouped := FALSE,
    Grouped := FALSE,
    ApplyPowerStage := FALSE,
    PowerStage := FALSE,
    ApplyAxisError := FALSE,
    AxisErrorID := WORD#16#0000,
    ApplyActuals := FALSE,
    ActualPosition := REAL#0.0,
    SetPosition := REAL#0.0,
    CommandedPosition := REAL#0.0,
    ActualVelocity := REAL#0.0,
    SetVelocity := REAL#0.0,
    CommandedVelocity := REAL#0.0,
    ActualTorque := REAL#0.0,
    ApplyAxisInfo := TRUE,
    HomeAbsSwitch := TRUE,
    LimitSwitchPos := FALSE,
    LimitSwitchNeg := TRUE,
    Simulation := TRUE,
    CommunicationReady := TRUE,
    ReadyForPowerOn := TRUE,
    IsHomed := FALSE,
    AxisWarning := FALSE,
    ApplyMotionFlags := FALSE,
    ConstantVelocity := FALSE,
    Accelerating := FALSE,
    Decelerating := FALSE
);
SeedSignal(
    Axis := Axis,
    ApplyReferenceSignal := TRUE,
    ReferenceSignalActive := TRUE,
    ReferenceSignalPosition := REAL#88.0,
    ReferenceSignalCode := UINT#3,
    ApplyAbsoluteEncoder := TRUE,
    AbsoluteEncoderPosition := REAL#144.0,
    ApplyDistanceCoded := TRUE,
    DistanceCodedReady := TRUE,
    DistanceCodedPosition := REAL#233.0
);
ProbeSignal(Axis := Axis);
ReadAxisInfo(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(ReadAxisInfo.HomeAbsSwitch);
ASSERT_TRUE(ReadAxisInfo.LimitSwitchNeg);
ASSERT_TRUE(ReadAxisInfo.Simulation);
ASSERT_TRUE(ProbeSignal.ReferenceSignalActive);
ASSERT_TRUE(ProbeSignal.ReferenceSignalPosition = REAL#88.0);
ASSERT_TRUE(ProbeSignal.AbsoluteEncoderPosition = REAL#144.0);
ASSERT_TRUE(ProbeSignal.DistanceCodedReady);
ASSERT_TRUE(ProbeSignal.DistanceCodedPosition = REAL#233.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_state_probe_helper_smoke
VAR
    Axis : AXIS_REF;
    SeedAxis : TRUST_MOTION_TestAxisSeed;
    ProbeState : TRUST_HOME_TestStateProbe;
END_VAR
Axis.AxisId := UDINT#2140;
Axis.InternalIndex := UINT#15;
SeedAxis(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := TRUE, AxisErrorID := WORD#16#1234, ApplyActuals := TRUE, ActualPosition := REAL#12.0, SetPosition := REAL#12.0, CommandedPosition := REAL#12.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := TRUE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := TRUE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
ProbeState(Axis := Axis);
ASSERT_TRUE(ProbeState.Standstill);
ASSERT_TRUE(ProbeState.IsHomed);
ASSERT_TRUE(ProbeState.Position = REAL#12.0);
ASSERT_TRUE(ProbeState.AxisErrorID = WORD#16#1234);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_step_absolute_and_limit_switch_behaviors
VAR
    AxisAbsolute : AXIS_REF;
    AxisLimit : AXIS_REF;
    SeedAxisAbsolute : TRUST_MOTION_TestAxisSeed;
    SeedAxisLimit : TRUST_MOTION_TestAxisSeed;
    AbsoluteSwitch : MC_StepAbsoluteSwitch;
    LimitSwitch : MC_StepLimitSwitch;
    ReadStatusAbsolute : MC_ReadStatus;
    ReadStatusLimit : MC_ReadStatus;
    ReadPositionAbsolute : MC_ReadActualPosition;
    ReadPositionLimit : MC_ReadActualPosition;
    ReferenceSignal : MC_REF_SIGNAL_REF;
END_VAR
AxisAbsolute.AxisId := UDINT#202;
AxisAbsolute.InternalIndex := UINT#2;
SeedAxisAbsolute(Axis := AxisAbsolute, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
ReferenceSignal.Signal := FALSE;
AbsoluteSwitch(Axis := AxisAbsolute, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, SwitchMode := MC_SWITCH_MODE#mcOn, ReferenceSignal := ReferenceSignal, Velocity := REAL#2.0, SetPosition := REAL#10.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ASSERT_TRUE(AbsoluteSwitch.Busy);
ASSERT_TRUE(AbsoluteSwitch.Active);
ASSERT_TRUE(ReadStatusAbsolute.Homing);
ReferenceSignal.Signal := TRUE;
AbsoluteSwitch(Axis := AxisAbsolute, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, SwitchMode := MC_SWITCH_MODE#mcOn, ReferenceSignal := ReferenceSignal, Velocity := REAL#2.0, SetPosition := REAL#10.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ReadPositionAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ASSERT_TRUE(AbsoluteSwitch.Done);
ASSERT_TRUE(ReadStatusAbsolute.Homing);
ASSERT_TRUE(ReadPositionAbsolute.Position = REAL#10.0);

AxisLimit.AxisId := UDINT#203;
AxisLimit.InternalIndex := UINT#3;
SeedAxisLimit(Axis := AxisLimit, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := TRUE, HomeAbsSwitch := FALSE, LimitSwitchPos := TRUE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
LimitSwitch(Axis := AxisLimit, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, LimitSwitchMode := MC_SWITCH_MODE#mcOn, Velocity := REAL#1.0, SetPosition := REAL#20.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
LimitSwitch(Axis := AxisLimit, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, LimitSwitchMode := MC_SWITCH_MODE#mcOn, Velocity := REAL#1.0, SetPosition := REAL#20.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ReadStatusLimit(Axis := AxisLimit, Enable := TRUE);
ReadPositionLimit(Axis := AxisLimit, Enable := TRUE);
ASSERT_TRUE(LimitSwitch.Done);
ASSERT_TRUE(ReadStatusLimit.Homing);
ASSERT_TRUE(ReadPositionLimit.Position = REAL#20.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_step_block_behavior
VAR
    AxisBlock : AXIS_REF;
    SeedAxisBlock : TRUST_MOTION_TestAxisSeed;
    StepBlock : MC_StepBlock;
    ReadStatusBlock : MC_ReadStatus;
    ReadPositionBlock : MC_ReadActualPosition;
END_VAR
AxisBlock.AxisId := UDINT#204;
AxisBlock.InternalIndex := UINT#4;
SeedAxisBlock(Axis := AxisBlock, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.1, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#5.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepBlock(Axis := AxisBlock, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#30.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#0ms, TorqueLimit := REAL#4.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
SeedAxisBlock(Axis := AxisBlock, ResetDefaults := FALSE, ApplyState := FALSE, State := MC_AXIS_STATUS#mcDisabled, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.1, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#5.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepBlock(Axis := AxisBlock, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#30.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#0ms, TorqueLimit := REAL#4.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusBlock(Axis := AxisBlock, Enable := TRUE);
ReadPositionBlock(Axis := AxisBlock, Enable := TRUE);
ASSERT_TRUE(StepBlock.Done);
ASSERT_TRUE(ReadStatusBlock.Homing);
ASSERT_TRUE(ReadPositionBlock.Position = REAL#30.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_step_block_detection_time_behavior
VAR
    AxisBlockTimed : AXIS_REF;
    SeedAxisBlockTimed : TRUST_MOTION_TestAxisSeed;
    StepBlockTimed : MC_StepBlock;
    ReadStatusBlockTimed : MC_ReadStatus;
    ReadPositionBlockTimed : MC_ReadActualPosition;
END_VAR
AxisBlockTimed.AxisId := UDINT#214;
AxisBlockTimed.InternalIndex := UINT#14;
SeedAxisBlockTimed(Axis := AxisBlockTimed, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.1, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#5.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepBlockTimed(Axis := AxisBlockTimed, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#31.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#100ms, TorqueLimit := REAL#4.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
SeedAxisBlockTimed(Axis := AxisBlockTimed, ResetDefaults := FALSE, ApplyState := FALSE, State := MC_AXIS_STATUS#mcDisabled, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.1, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#5.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepBlockTimed(Axis := AxisBlockTimed, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#31.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#100ms, TorqueLimit := REAL#4.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(StepBlockTimed.Busy);
ASSERT_TRUE(NOT StepBlockTimed.Done);
SeedAxisBlockTimed(Axis := AxisBlockTimed, ResetDefaults := FALSE, ApplyState := FALSE, State := MC_AXIS_STATUS#mcDisabled, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.1, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#5.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepBlockTimed(Axis := AxisBlockTimed, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#31.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#100ms, TorqueLimit := REAL#4.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusBlockTimed(Axis := AxisBlockTimed, Enable := TRUE);
ReadPositionBlockTimed(Axis := AxisBlockTimed, Enable := TRUE);
ASSERT_TRUE(StepBlockTimed.Done);
ASSERT_TRUE(ReadStatusBlockTimed.Homing);
ASSERT_TRUE(ReadPositionBlockTimed.Position = REAL#31.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_reference_pulse_behavior
VAR
    AxisPulse : AXIS_REF;
    SeedAxisPulse : TRUST_MOTION_TestAxisSeed;
    StepPulse : MC_StepReferencePulse;
    ReadStatusPulse : MC_ReadStatus;
    ReadPositionPulse : MC_ReadActualPosition;
    ReferenceSignal : MC_REF_SIGNAL_REF;
END_VAR
AxisPulse.AxisId := UDINT#205;
AxisPulse.InternalIndex := UINT#5;
SeedAxisPulse(Axis := AxisPulse, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
ReferenceSignal.Signal := FALSE;
StepPulse(Axis := AxisPulse, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, ReferenceSignal := ReferenceSignal, Velocity := REAL#3.0, SetPosition := REAL#40.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ReferenceSignal.Signal := TRUE;
StepPulse(Axis := AxisPulse, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, ReferenceSignal := ReferenceSignal, Velocity := REAL#3.0, SetPosition := REAL#40.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ReadStatusPulse(Axis := AxisPulse, Enable := TRUE);
ReadPositionPulse(Axis := AxisPulse, Enable := TRUE);
ASSERT_TRUE(StepPulse.Done);
ASSERT_TRUE(ReadStatusPulse.Homing);
ASSERT_TRUE(ReadPositionPulse.Position = REAL#40.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_distance_coded_and_limit_errors
VAR
    AxisCoded : AXIS_REF;
    AxisTimeout : AXIS_REF;
    AxisBlockTimeout : AXIS_REF;
    SeedAxisCoded : TRUST_MOTION_TestAxisSeed;
    SeedAxisTimeout : TRUST_MOTION_TestAxisSeed;
    SeedAxisBlockTimeout : TRUST_MOTION_TestAxisSeed;
    SeedSignal : TRUST_HOME_TestSignalSeed;
    StepCoded : MC_StepDistanceCoded;
    StepAbsoluteTimeout : MC_StepAbsoluteSwitch;
    StepBlockTimeout : MC_StepBlock;
    ReadPositionCoded : MC_ReadActualPosition;
    ReadStatusCoded : MC_ReadStatus;
    ReferenceSignal : MC_REF_SIGNAL_REF;
END_VAR
AxisCoded.AxisId := UDINT#206;
AxisCoded.InternalIndex := UINT#6;
SeedAxisCoded(Axis := AxisCoded, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepCoded(Axis := AxisCoded, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, Velocity := REAL#1.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
SeedSignal(Axis := AxisCoded, ApplyReferenceSignal := FALSE, ReferenceSignalActive := FALSE, ReferenceSignalPosition := REAL#0.0, ReferenceSignalCode := UINT#0, ApplyAbsoluteEncoder := FALSE, AbsoluteEncoderPosition := REAL#0.0, ApplyDistanceCoded := TRUE, DistanceCodedReady := TRUE, DistanceCodedPosition := REAL#77.0);
StepCoded(Axis := AxisCoded, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, Velocity := REAL#1.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadPositionCoded(Axis := AxisCoded, Enable := TRUE);
ReadStatusCoded(Axis := AxisCoded, Enable := TRUE);
ASSERT_TRUE(StepCoded.Done);
ASSERT_TRUE(ReadStatusCoded.Homing);
ASSERT_TRUE(ReadPositionCoded.Position = REAL#77.0);

AxisTimeout.AxisId := UDINT#207;
AxisTimeout.InternalIndex := UINT#7;
SeedAxisTimeout(Axis := AxisTimeout, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
ReferenceSignal.Signal := FALSE;
StepAbsoluteTimeout(Axis := AxisTimeout, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, SwitchMode := MC_SWITCH_MODE#mcOn, ReferenceSignal := ReferenceSignal, Velocity := REAL#2.0, SetPosition := REAL#5.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#100ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
StepAbsoluteTimeout(Axis := AxisTimeout, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, SwitchMode := MC_SWITCH_MODE#mcOn, ReferenceSignal := ReferenceSignal, Velocity := REAL#2.0, SetPosition := REAL#5.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#100ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(StepAbsoluteTimeout.Error);

AxisBlockTimeout.AxisId := UDINT#208;
AxisBlockTimeout.InternalIndex := UINT#8;
SeedAxisBlockTimeout(Axis := AxisBlockTimeout, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#1.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#1.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
StepBlockTimeout(Axis := AxisBlockTimeout, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#9.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#0ms, TorqueLimit := REAL#3.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#5.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
StepBlockTimeout(Axis := AxisBlockTimeout, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcNegativeDirection, Velocity := REAL#1.0, SetPosition := REAL#9.0, DetectionVelocityLimit := REAL#0.2, DetectionVelocityTime := TIME#0ms, TorqueLimit := REAL#3.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#5.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(StepBlockTimeout.Error);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_direct_absolute_finish_and_generic_home_behaviors
VAR
    AxisDirect : AXIS_REF;
    AxisAbsolute : AXIS_REF;
    AxisFinish : AXIS_REF;
    AxisGenericHome : AXIS_REF;
    SeedDirect : TRUST_MOTION_TestAxisSeed;
    SeedAbsolute : TRUST_MOTION_TestAxisSeed;
    SeedFinish : TRUST_MOTION_TestAxisSeed;
    SeedGeneric : TRUST_MOTION_TestAxisSeed;
    SeedSignal : TRUST_HOME_TestSignalSeed;
    HomeDirect : MC_HomeDirect;
    HomeAbsolute : MC_HomeAbsolute;
    FinishHoming : MC_FinishHoming;
    GenericHome : MC_Home;
    ReadStatusDirect : MC_ReadStatus;
    ReadStatusAbsolute : MC_ReadStatus;
    ReadStatusFinish : MC_ReadStatus;
    ReadStatusGeneric : MC_ReadStatus;
    ReadInfoDirect : MC_ReadAxisInfo;
    ReadInfoAbsolute : MC_ReadAxisInfo;
    ReadInfoFinish : MC_ReadAxisInfo;
    ReadInfoGeneric : MC_ReadAxisInfo;
    ReadPositionDirect : MC_ReadActualPosition;
    ReadPositionAbsolute : MC_ReadActualPosition;
    ReadPositionFinish : MC_ReadActualPosition;
    ReadPositionGeneric : MC_ReadActualPosition;
END_VAR
AxisDirect.AxisId := UDINT#209;
AxisDirect.InternalIndex := UINT#9;
SeedDirect(Axis := AxisDirect, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcHoming, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
HomeDirect(Axis := AxisDirect, Execute := TRUE, SetPosition := REAL#42.0, BufferMode := MC_BUFFER_MODE#mcAborting);
HomeDirect(Axis := AxisDirect, Execute := TRUE, SetPosition := REAL#42.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusDirect(Axis := AxisDirect, Enable := TRUE);
ReadInfoDirect(Axis := AxisDirect, Enable := TRUE);
ReadPositionDirect(Axis := AxisDirect, Enable := TRUE);
ASSERT_TRUE(HomeDirect.Done);
ASSERT_TRUE(ReadStatusDirect.Standstill);
ASSERT_TRUE(ReadInfoDirect.IsHomed);
ASSERT_TRUE(ReadPositionDirect.Position = REAL#42.0);

AxisAbsolute.AxisId := UDINT#210;
AxisAbsolute.InternalIndex := UINT#10;
SeedAbsolute(Axis := AxisAbsolute, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#5.0, SetPosition := REAL#5.0, CommandedPosition := REAL#5.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
SeedSignal(Axis := AxisAbsolute, ApplyReferenceSignal := FALSE, ReferenceSignalActive := FALSE, ReferenceSignalPosition := REAL#0.0, ReferenceSignalCode := UINT#0, ApplyAbsoluteEncoder := TRUE, AbsoluteEncoderPosition := REAL#84.0, ApplyDistanceCoded := FALSE, DistanceCodedReady := FALSE, DistanceCodedPosition := REAL#0.0);
HomeAbsolute(Axis := AxisAbsolute, Execute := TRUE, BufferMode := MC_BUFFER_MODE#mcBuffered);
HomeAbsolute(Axis := AxisAbsolute, Execute := TRUE, BufferMode := MC_BUFFER_MODE#mcBuffered);
ReadStatusAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ReadInfoAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ReadPositionAbsolute(Axis := AxisAbsolute, Enable := TRUE);
ASSERT_TRUE(HomeAbsolute.Done);
ASSERT_TRUE(ReadStatusAbsolute.Standstill);
ASSERT_TRUE(ReadInfoAbsolute.IsHomed);
ASSERT_TRUE(ReadPositionAbsolute.Position = REAL#84.0);

AxisFinish.AxisId := UDINT#211;
AxisFinish.InternalIndex := UINT#11;
SeedFinish(Axis := AxisFinish, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcHoming, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := TRUE, ActualPosition := REAL#10.0, SetPosition := REAL#10.0, CommandedPosition := REAL#10.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
FinishHoming(Axis := AxisFinish, Execute := TRUE, Distance := REAL#5.0, Velocity := REAL#2.0, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
FinishHoming(Axis := AxisFinish, Execute := TRUE, Distance := REAL#5.0, Velocity := REAL#2.0, Acceleration := REAL#1.0, Deceleration := REAL#1.0, Jerk := REAL#1.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusFinish(Axis := AxisFinish, Enable := TRUE);
ReadInfoFinish(Axis := AxisFinish, Enable := TRUE);
ReadPositionFinish(Axis := AxisFinish, Enable := TRUE);
ASSERT_TRUE(FinishHoming.Done);
ASSERT_TRUE(ReadStatusFinish.Standstill);
ASSERT_TRUE(ReadInfoFinish.IsHomed);
ASSERT_TRUE(ReadPositionFinish.Position = REAL#15.0);

AxisGenericHome.AxisId := UDINT#212;
AxisGenericHome.InternalIndex := UINT#12;
SeedGeneric(Axis := AxisGenericHome, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
GenericHome(Axis := AxisGenericHome, Execute := TRUE, Position := REAL#11.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusGeneric(Axis := AxisGenericHome, Enable := TRUE);
ASSERT_TRUE(GenericHome.Busy);
ASSERT_TRUE(GenericHome.Active);
ASSERT_TRUE(ReadStatusGeneric.Homing);
GenericHome(Axis := AxisGenericHome, Execute := FALSE, Position := REAL#11.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatusGeneric(Axis := AxisGenericHome, Enable := TRUE);
ReadInfoGeneric(Axis := AxisGenericHome, Enable := TRUE);
ReadPositionGeneric(Axis := AxisGenericHome, Enable := TRUE);
ASSERT_TRUE(GenericHome.Done);
ASSERT_TRUE(ReadStatusGeneric.Standstill);
ASSERT_TRUE(ReadInfoGeneric.IsHomed);
ASSERT_TRUE(ReadPositionGeneric.Position = REAL#11.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_homing_multi_step_sequence_finishes_in_work_area
VAR
    Axis : AXIS_REF;
    SeedAxis : TRUST_MOTION_TestAxisSeed;
    StepAbsoluteSwitch : MC_StepAbsoluteSwitch;
    StepReferencePulse : MC_StepReferencePulse;
    FinishHoming : MC_FinishHoming;
    ReadStatus : MC_ReadStatus;
    ReadInfo : MC_ReadAxisInfo;
    ReadPosition : MC_ReadActualPosition;
    ReferenceSignal : MC_REF_SIGNAL_REF;
END_VAR
Axis.AxisId := UDINT#213;
Axis.InternalIndex := UINT#13;
SeedAxis(Axis := Axis, ResetDefaults := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill, ApplyGrouped := FALSE, Grouped := FALSE, ApplyPowerStage := FALSE, PowerStage := FALSE, ApplyAxisError := FALSE, AxisErrorID := WORD#16#0000, ApplyActuals := FALSE, ActualPosition := REAL#0.0, SetPosition := REAL#0.0, CommandedPosition := REAL#0.0, ActualVelocity := REAL#0.0, SetVelocity := REAL#0.0, CommandedVelocity := REAL#0.0, ActualTorque := REAL#0.0, ApplyAxisInfo := FALSE, HomeAbsSwitch := FALSE, LimitSwitchPos := FALSE, LimitSwitchNeg := FALSE, Simulation := FALSE, CommunicationReady := TRUE, ReadyForPowerOn := TRUE, IsHomed := FALSE, AxisWarning := FALSE, ApplyMotionFlags := FALSE, ConstantVelocity := FALSE, Accelerating := FALSE, Decelerating := FALSE);
ReferenceSignal.Signal := FALSE;
StepAbsoluteSwitch(Axis := Axis, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, SwitchMode := MC_SWITCH_MODE#mcOn, ReferenceSignal := ReferenceSignal, Velocity := REAL#1.0, SetPosition := REAL#100.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReferenceSignal.Signal := TRUE;
StepAbsoluteSwitch(Axis := Axis, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, SwitchMode := MC_SWITCH_MODE#mcOn, ReferenceSignal := ReferenceSignal, Velocity := REAL#1.0, SetPosition := REAL#100.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatus(Axis := Axis, Enable := TRUE);
ReadPosition(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(StepAbsoluteSwitch.Done);
ASSERT_TRUE(ReadStatus.Homing);
ASSERT_TRUE(ReadPosition.Position = REAL#100.0);
ReferenceSignal.Signal := FALSE;
StepReferencePulse(Axis := Axis, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, ReferenceSignal := ReferenceSignal, Velocity := REAL#0.5, SetPosition := REAL#101.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ReferenceSignal.Signal := TRUE;
StepReferencePulse(Axis := Axis, Execute := TRUE, Direction := MC_HOME_DIRECTION#mcPositiveDirection, ReferenceSignal := ReferenceSignal, Velocity := REAL#0.5, SetPosition := REAL#101.0, TorqueLimit := REAL#0.0, TimeLimit := TIME#0ms, DistanceLimit := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
ReadStatus(Axis := Axis, Enable := TRUE);
ReadPosition(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(StepReferencePulse.Done);
ASSERT_TRUE(ReadStatus.Homing);
ASSERT_TRUE(ReadPosition.Position = REAL#101.0);
FinishHoming(Axis := Axis, Execute := TRUE, Distance := REAL#5.0, Velocity := REAL#1.0, Acceleration := REAL#0.5, Deceleration := REAL#0.5, Jerk := REAL#0.25, BufferMode := MC_BUFFER_MODE#mcAborting);
FinishHoming(Axis := Axis, Execute := TRUE, Distance := REAL#5.0, Velocity := REAL#1.0, Acceleration := REAL#0.5, Deceleration := REAL#0.5, Jerk := REAL#0.25, BufferMode := MC_BUFFER_MODE#mcAborting);
ReadStatus(Axis := Axis, Enable := TRUE);
ReadInfo(Axis := Axis, Enable := TRUE);
ReadPosition(Axis := Axis, Enable := TRUE);
ASSERT_TRUE(FinishHoming.Done);
ASSERT_TRUE(ReadStatus.Standstill);
ASSERT_TRUE(ReadInfo.IsHomed);
ASSERT_TRUE(ReadPosition.Position = REAL#106.0);
END_TEST_PROGRAM
