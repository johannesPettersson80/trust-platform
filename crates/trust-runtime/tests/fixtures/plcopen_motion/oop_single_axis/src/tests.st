TEST_PROGRAM plcopen_motion_oop_bind_returns_success
VAR
    Axis : MC_OopAxis;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#200, InternalIndex := UINT#1);

ASSERT_TRUE(Result = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_axis_assigns_to_interface
VAR
    Axis : MC_OopAxis;
    AxisObject : itfAxis;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#200, InternalIndex := UINT#1);
AxisObject := Axis;

ASSERT_TRUE(Result = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_command_interface_properties_dispatch
VAR
    Command : MC_OopCommand;
    CommandObject : itfCommand;
    Updated : BOOL;
END_VAR

Updated := Command.SetStatus(
    Done := TRUE,
    Busy := FALSE,
    Active := FALSE,
    CommandAborted := FALSE,
    Error := FALSE,
    ErrorId := WORD#16#0000
);
CommandObject := Command;

ASSERT_TRUE(Updated);
ASSERT_TRUE(CommandObject.Done);
ASSERT_FALSE(CommandObject.Error);
ASSERT_TRUE(CommandObject.ErrorId = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_command_concrete_properties_dispatch
VAR
    Command : MC_OopCommand;
    Updated : BOOL;
END_VAR

Updated := Command.SetStatus(
    Done := TRUE,
    Busy := FALSE,
    Active := FALSE,
    CommandAborted := FALSE,
    Error := FALSE,
    ErrorId := WORD#16#0000
);

ASSERT_TRUE(Updated);
ASSERT_TRUE(Command.Done);
ASSERT_FALSE(Command.Error);
ASSERT_TRUE(Command.ErrorId = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_concrete_power_returns_command
VAR
    Axis : MC_OopAxis;
    PowerCommand : itfCommand;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#200, InternalIndex := UINT#1);
PowerCommand := Axis.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);

ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(PowerCommand.Done);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_concrete_power_call_does_not_error
VAR
    Axis : MC_OopAxis;
    PowerCommand : itfCommand;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#200, InternalIndex := UINT#1);
PowerCommand := Axis.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);

ASSERT_TRUE(Result = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_refresh_after_bind_returns_success
VAR
    Axis : MC_OopAxis;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#200, InternalIndex := UINT#1);
ASSERT_TRUE(Result = WORD#16#0000);
Result := Axis.Refresh();
ASSERT_TRUE(Result = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_interface_power_returns_command
VAR
    Axis : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#200, InternalIndex := UINT#1);
AxisObject := Axis;
PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);

ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(PowerCommand.Done);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_axis_interface_and_power_command
VAR
    Axis : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#201, InternalIndex := UINT#1);
AxisObject := Axis;
PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);

ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(PowerCommand.Done);
ASSERT_FALSE(PowerCommand.Busy);
ASSERT_FALSE(PowerCommand.Error);
ASSERT_TRUE(PowerCommand.ErrorId = WORD#16#0000);
ASSERT_TRUE(AxisObject.PowerOn);
ASSERT_TRUE(AxisObject.Status = MC_AXIS_STATUS#mcStandstill);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_move_absolute_completes_after_release
VAR
    Axis : MC_OopAxis;
    MoveCommand : itfAxisCommand;
    PowerCommand : itfCommand;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#202, InternalIndex := UINT#2);
ASSERT_TRUE(Result = WORD#16#0000);
PowerCommand := Axis.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
ASSERT_TRUE(PowerCommand.Done);

MoveCommand := Axis.MoveAbsolute(
    Position := REAL#125.0,
    Velocity := REAL#20.0,
    Acceleration := REAL#10.0,
    Deceleration := REAL#10.0,
    Jerk := REAL#1.0,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);

ASSERT_TRUE(MoveCommand.Busy);
ASSERT_TRUE(MoveCommand.Active);
ASSERT_FALSE(MoveCommand.Done);
ASSERT_TRUE(Axis.Status = MC_AXIS_STATUS#mcDiscreteMotion);

Result := Axis.Release();
ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(MoveCommand.Done);
ASSERT_FALSE(MoveCommand.Busy);
ASSERT_FALSE(MoveCommand.Active);
ASSERT_TRUE(Axis.Status = MC_AXIS_STATUS#mcStandstill);
ASSERT_TRUE(Axis.ActualPosition = REAL#125.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_home_setposition_and_reset_use_classic_state
VAR
    RawAxis : AXIS_REF;
    Axis : MC_OopAxis;
    Seed : TRUST_MOTION_TestAxisSeed;
    HomeCommand : itfAxisCommand;
    SetPositionCommand : itfCommand;
    Result : MC_ERROR;
END_VAR

RawAxis.AxisId := UDINT#203;
RawAxis.InternalIndex := UINT#3;
Seed(Axis := RawAxis, ResetDefaults := TRUE, ApplyPowerStage := TRUE, PowerStage := TRUE, ApplyState := TRUE, State := MC_AXIS_STATUS#mcStandstill);
Result := Axis.Bind(AxisId := UDINT#203, InternalIndex := UINT#3);
ASSERT_TRUE(Result = WORD#16#0000);

HomeCommand := Axis.Home(Position := REAL#10.0, BufferMode := MC_BUFFER_MODE#mcAborting);
ASSERT_TRUE(HomeCommand.Busy);
Result := Axis.Release();
ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(HomeCommand.Done);
ASSERT_TRUE(Axis.IsHomed);
ASSERT_TRUE(Axis.ActualPosition = REAL#10.0);

SetPositionCommand := Axis.SetPosition(
    Position := REAL#42.0,
    Relative := FALSE,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(SetPositionCommand.Done);
ASSERT_TRUE(Axis.ActualPosition = REAL#42.0);

Seed(Axis := RawAxis, ApplyState := TRUE, State := MC_AXIS_STATUS#mcErrorStop, ApplyAxisError := TRUE, AxisErrorID := WORD#16#0200);
Result := Axis.Reset();
ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(Axis.Status = MC_AXIS_STATUS#mcStandstill);
ASSERT_TRUE(Axis.ErrorId = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_readback_properties_refresh_from_classic_axis
VAR
    RawAxis : AXIS_REF;
    Axis : MC_OopAxis;
    Seed : TRUST_MOTION_TestAxisSeed;
    Result : MC_ERROR;
END_VAR

RawAxis.AxisId := UDINT#204;
RawAxis.InternalIndex := UINT#4;
Seed(
    Axis := RawAxis,
    ResetDefaults := TRUE,
    ApplyState := TRUE,
    State := MC_AXIS_STATUS#mcContinuousMotion,
    ApplyActuals := TRUE,
    ActualPosition := REAL#12.5,
    ActualVelocity := REAL#-3.25,
    ActualTorque := REAL#1.5,
    ApplyAxisInfo := TRUE,
    CommunicationReady := TRUE,
    ReadyForPowerOn := TRUE,
    PowerStage := TRUE,
    IsHomed := TRUE,
    ApplyMotionFlags := TRUE,
    ConstantVelocity := TRUE
);
Result := Axis.Bind(AxisId := UDINT#204, InternalIndex := UINT#4);
ASSERT_TRUE(Result = WORD#16#0000);
Result := Axis.Refresh();

ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(Axis.ActualPosition = REAL#12.5);
ASSERT_TRUE(Axis.ActualVelocity = REAL#-3.25);
ASSERT_TRUE(Axis.ActualTorque = REAL#1.5);
ASSERT_TRUE(Axis.Direction = MC_AXIS_DIRECTION#mcDirectionNegative);
ASSERT_TRUE(Axis.MotionStatus = MC_MOTION_STATUS#mcConstantVelocity);
ASSERT_TRUE(Axis.Status = MC_AXIS_STATUS#mcContinuousMotion);
ASSERT_TRUE(Axis.CommunicationReady);
ASSERT_TRUE(Axis.ReadyForPowerOn);
ASSERT_TRUE(Axis.IsHomed);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_velocity_and_parameter_methods
VAR
    Axis : MC_OopAxis;
    Constants : MC_Constants;
    PowerCommand : itfCommand;
    VelocityCommand : itfContinuousAxisCommand;
    Result : MC_ERROR;
    ParameterValue : REAL;
END_VAR

Constants();
Result := Axis.Bind(AxisId := UDINT#205, InternalIndex := UINT#5);
ASSERT_TRUE(Result = WORD#16#0000);
PowerCommand := Axis.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
ASSERT_TRUE(PowerCommand.Done);

VelocityCommand := Axis.MoveVelocity(
    Velocity := REAL#15.0,
    Acceleration := REAL#5.0,
    Deceleration := REAL#5.0,
    Jerk := REAL#1.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);

ASSERT_TRUE(VelocityCommand.InVelocity);
ASSERT_TRUE(VelocityCommand.Active);
ASSERT_TRUE(Axis.Status = MC_AXIS_STATUS#mcContinuousMotion);

Result := Axis.WriteParameter(ParameterNumber := Constants.PN_MaxVelocityAppl, Value := REAL#75.0);
ASSERT_TRUE(Result = WORD#16#0000);
Result := Axis.ReadParameter(ParameterNumber := Constants.PN_MaxVelocityAppl, Value => ParameterValue);
ASSERT_TRUE(Result = WORD#16#0000);
ASSERT_TRUE(ParameterValue = REAL#75.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_oop_unsupported_methods_return_not_supported
VAR
    Axis : MC_OopAxis;
    ProfileCommand : itfCommand;
    MoveCommand : itfAxisCommand;
    Result : MC_ERROR;
END_VAR

Result := Axis.Bind(AxisId := UDINT#206, InternalIndex := UINT#6);
ASSERT_TRUE(Result = WORD#16#0000);

ProfileCommand := Axis.PositionProfile(
    TimeScale := REAL#1.0,
    PositionScale := REAL#1.0,
    Offset := REAL#0.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ASSERT_TRUE(ProfileCommand.Error);
ASSERT_TRUE(ProfileCommand.ErrorId = WORD#16#0500);

MoveCommand := Axis.MoveSuperimposed(
    Distance := REAL#1.0,
    VelocityDiff := REAL#1.0,
    Acceleration := REAL#1.0,
    Deceleration := REAL#1.0,
    Jerk := REAL#1.0
);
ASSERT_TRUE(MoveCommand.Error);
ASSERT_TRUE(MoveCommand.ErrorId = WORD#16#0500);
END_TEST_PROGRAM
