TEST_PROGRAM plcopen_motion_coordinated_motion_public_types_and_enums_resolve
VAR
    Axis : AXIS_REF;
    Group : AXES_GROUP_REF;
    Ident : IDENT_IN_GROUP_REF;
    CommandID : MC_COMMAND_ID;
    GroupParameterDynamics : MC_GROUP_PARAMETER;
    GroupParameterTransitionReference : MC_GROUP_PARAMETER;
    TransitionParameter : MC_TRANSITION_PARAMETER;
    KinRef : MC_KIN_REF;
    CoordSystemACS : MC_COORD_SYSTEM;
    CoordSystemPCS : MC_COORD_SYSTEM;
    DynamicsAbsolute : MC_DYNAMICS_MODE;
    DynamicsPercentage : MC_DYNAMICS_MODE;
    TransitionModeNone : MC_TRANSITION_MODE;
    TransitionModeCorner : MC_TRANSITION_MODE;
    TransitionVelocityZero : MC_TRANSITION_VELOCITY;
    TransitionVelocityNext : MC_TRANSITION_VELOCITY;
    TransitionReferenceStart : MC_TRANSITION_REFERENCE;
    TransitionReferenceEnd : MC_TRANSITION_REFERENCE;
    CommandStateAccepted : MC_COMMAND_STATE;
    CommandStateActive : MC_COMMAND_STATE;
    OrientationLinear : MC_ORIENTATION_MODE;
    OrientationPathBased : MC_ORIENTATION_MODE;
    GroupStatusDisabled : MC_GROUP_STATUS;
    GroupStatusStandby : MC_GROUP_STATUS;
    Cart : MC_CART_REF;
    Coord : MC_COORD_REF;
    CartPos : MC_CART_POS_REF;
    AxesPos : MC_AXES_POS_REF;
    PositionRef : MC_POS_REF;
    DistanceRef : MC_DISTANCE_REF;
    Limits : MC_GROUP_SWLIMITS;
END_VAR

Axis.AxisId := UDINT#1;
Axis.InternalIndex := UINT#1;
Group.GroupId := UDINT#7;
Group.InternalIndex := UINT#3;
Ident := 'AxisA';
CommandID := UINT#42;
GroupParameterDynamics := MC_GROUP_PARAMETER#mcDynamicsMode;
GroupParameterTransitionReference := MC_GROUP_PARAMETER#mcTransitionReferencePoint;
TransitionParameter := REAL#2.5;
KinRef := UINT#8;
CoordSystemACS := MC_COORD_SYSTEM#mcACS;
CoordSystemPCS := MC_COORD_SYSTEM#mcPCS;
DynamicsAbsolute := MC_DYNAMICS_MODE#mcAbsolute;
DynamicsPercentage := MC_DYNAMICS_MODE#mcPercentage;
TransitionModeNone := MC_TRANSITION_MODE#mcTMNone;
TransitionModeCorner := MC_TRANSITION_MODE#mcTMCornerDistance;
TransitionVelocityZero := MC_TRANSITION_VELOCITY#mcTVZero;
TransitionVelocityNext := MC_TRANSITION_VELOCITY#mcTVNext;
TransitionReferenceStart := MC_TRANSITION_REFERENCE#mcStartPoint;
TransitionReferenceEnd := MC_TRANSITION_REFERENCE#mcEndPoint;
CommandStateAccepted := MC_COMMAND_STATE#mcAccepted;
CommandStateActive := MC_COMMAND_STATE#mcActive;
OrientationLinear := MC_ORIENTATION_MODE#mcLinear;
OrientationPathBased := MC_ORIENTATION_MODE#mcPathBased;
GroupStatusDisabled := MC_GROUP_STATUS#mcGroupDisabled;
GroupStatusStandby := MC_GROUP_STATUS#mcGroupStandby;
Cart.X := REAL#1.0;
Coord.Y := REAL#2.0;

ASSERT_TRUE(Axis.AxisId = UDINT#1);
ASSERT_TRUE(Group.InternalIndex = UINT#3);
ASSERT_TRUE(CommandID = UINT#42);
ASSERT_TRUE(GroupParameterDynamics <> GroupParameterTransitionReference);
ASSERT_TRUE(TransitionParameter = REAL#2.5);
ASSERT_TRUE(KinRef = UINT#8);
ASSERT_TRUE(CoordSystemACS <> CoordSystemPCS);
ASSERT_TRUE(DynamicsAbsolute <> DynamicsPercentage);
ASSERT_TRUE(TransitionModeNone <> TransitionModeCorner);
ASSERT_TRUE(TransitionVelocityZero <> TransitionVelocityNext);
ASSERT_TRUE(TransitionReferenceStart <> TransitionReferenceEnd);
ASSERT_TRUE(CommandStateAccepted <> CommandStateActive);
ASSERT_TRUE(OrientationLinear <> OrientationPathBased);
ASSERT_TRUE(GroupStatusDisabled <> GroupStatusStandby);
ASSERT_TRUE(Cart.X = REAL#1.0);
ASSERT_TRUE(Coord.Y = REAL#2.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_ref_fields_resolve
VAR
    Group : AXES_GROUP_REF;
END_VAR

Group.GroupId := UDINT#11;
Group.InternalIndex := UINT#2;

ASSERT_TRUE(Group.GroupId = UDINT#11);
ASSERT_TRUE(Group.InternalIndex = UINT#2);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_fake_group_backend_models_membership_and_readback
VAR
    Group : AXES_GROUP_REF;
    Axis1 : AXIS_REF;
    Axis2 : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    Probe : TRUST_GROUP_TestProbe;
    AcceptedCommandID : MC_COMMAND_ID;
    AddedAxis1 : BOOL;
    AddedAxis2 : BOOL;
END_VAR

Group.GroupId := UDINT#21;
Group.InternalIndex := UINT#1;
Axis1.AxisId := UDINT#101;
Axis1.InternalIndex := UINT#1;
Axis2.AxisId := UDINT#102;
Axis2.InternalIndex := UINT#2;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#10.0,
    CommandedY := REAL#20.0,
    CommandedZ := REAL#30.0,
    CommandedRX := REAL#1.0,
    CommandedRY := REAL#2.0,
    CommandedRZ := REAL#3.0,
    ActualX := REAL#9.5,
    ActualY := REAL#19.5,
    ActualZ := REAL#29.5,
    ActualRX := REAL#0.5,
    ActualRY := REAL#1.5,
    ActualRZ := REAL#2.5
);
AddedAxis1 := TRUST_GROUP_AddMember(Group := Group, Axis := Axis1, IdentInGroup := 'Axis1');
AddedAxis2 := TRUST_GROUP_AddMember(Group := Group, Axis := Axis2, IdentInGroup := 'Axis2');
ASSERT_TRUE(AddedAxis1);
ASSERT_TRUE(AddedAxis2);
AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := Group, CoordSystem := MC_COORD_SYSTEM#mcMCS);
Probe(Group := Group);

ASSERT_TRUE(Probe.MemberCount = UINT#2);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStandby);
ASSERT_TRUE(Probe.ActiveCoordSystem = MC_COORD_SYSTEM#mcMCS);
ASSERT_TRUE(Probe.CommandedX = REAL#10.0);
ASSERT_TRUE(Probe.CommandedY = REAL#20.0);
ASSERT_TRUE(Probe.CommandedZ = REAL#30.0);
ASSERT_TRUE(Probe.ActualX = REAL#9.5);
ASSERT_TRUE(Probe.ActualY = REAL#19.5);
ASSERT_TRUE(Probe.ActualZ = REAL#29.5);
ASSERT_TRUE(Probe.LastAcceptedCommandID = AcceptedCommandID);
ASSERT_TRUE(Probe.LastAcceptedCommandID = UINT#1);
ASSERT_TRUE(Probe.TransformIdentity);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_command_acceptance_is_deterministic
VAR
    Group : AXES_GROUP_REF;
    Axis1 : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    Probe : TRUST_GROUP_TestProbe;
    Command1 : MC_COMMAND_ID;
    Command2 : MC_COMMAND_ID;
    AddedAxis : BOOL;
END_VAR

Group.GroupId := UDINT#22;
Group.InternalIndex := UINT#2;
Axis1.AxisId := UDINT#103;
Axis1.InternalIndex := UINT#3;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddedAxis := TRUST_GROUP_AddMember(Group := Group, Axis := Axis1, IdentInGroup := 'Axis1');
ASSERT_TRUE(AddedAxis);
Command1 := TRUST_GROUP_RequestCommand(Group := Group, CoordSystem := MC_COORD_SYSTEM#mcMCS);
Command2 := TRUST_GROUP_RequestCommand(Group := Group, CoordSystem := MC_COORD_SYSTEM#mcPCS);
Probe(Group := Group);

ASSERT_TRUE(Command1 = UINT#1);
ASSERT_TRUE(Command2 = UINT#2);
ASSERT_TRUE(Probe.LastAcceptedCommandID = UINT#2);
ASSERT_TRUE(Probe.ActiveCoordSystem = MC_COORD_SYSTEM#mcPCS);
ASSERT_TRUE(Probe.LastAcceptedCoordSystem = MC_COORD_SYSTEM#mcPCS);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_transform_round_trip_identity
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
    Transform : TRUST_GROUP_TransformRoundTripProbe;
    InputPosition : MC_COORD_REF;
END_VAR

Group.GroupId := UDINT#23;
Group.InternalIndex := UINT#3;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcPCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
InputPosition.X := REAL#12.5;
InputPosition.Y := REAL#-3.0;
InputPosition.Z := REAL#8.25;
InputPosition.RX := REAL#1.0;
InputPosition.RY := REAL#2.0;
InputPosition.RZ := REAL#3.0;
Transform(Group := Group, InputPosition := InputPosition);

ASSERT_TRUE(Transform.IdentityApplied);
ASSERT_TRUE(Transform.X = REAL#12.5);
ASSERT_TRUE(Transform.Y = REAL#-3.0);
ASSERT_TRUE(Transform.Z = REAL#8.25);
ASSERT_TRUE(Transform.RX = REAL#1.0);
ASSERT_TRUE(Transform.RY = REAL#2.0);
ASSERT_TRUE(Transform.RZ = REAL#3.0);
END_TEST_PROGRAM


TEST_PROGRAM plcopen_motion_coordinated_motion_seed_probe_smoke
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
END_VAR

Group.GroupId := UDINT#99;
Group.InternalIndex := UINT#2;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_membership_admin_fbs
VAR
    Group : AXES_GROUP_REF;
    AxisA : AXIS_REF;
    AxisB : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxisA : MC_AddAxisToGroup;
    AddAxisADuplicate : MC_AddAxisToGroup;
    AddAxisB : MC_AddAxisToGroup;
    ReadConfig : MC_GroupReadConfiguration;
    ReadInfo : MC_ReadAxisGroupInfo;
    RemoveAxisA : MC_RemoveAxisFromGroup;
    UngroupAll : MC_UngroupAllAxes;
    Probe : TRUST_GROUP_TestProbe;
    IdentMatch : INT;
END_VAR

Group.GroupId := UDINT#31;
Group.InternalIndex := UINT#4;
AxisA.AxisId := UDINT#201;
AxisA.InternalIndex := UINT#5;
AxisB.AxisId := UDINT#202;
AxisB.InternalIndex := UINT#6;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupDisabled,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);

AddAxisA(AxesGroup := Group, Axis := AxisA, Execute := TRUE, IdentInGroup := 'AxisA');
Probe(Group := Group);
ASSERT_TRUE(AddAxisA.Done);
ASSERT_TRUE(NOT AddAxisA.Error);
ASSERT_TRUE(Probe.MemberCount = UINT#1);

AddAxisADuplicate(AxesGroup := Group, Axis := AxisA, Execute := TRUE, IdentInGroup := 'AxisA_Dup');
ASSERT_TRUE(AddAxisADuplicate.Error);
ASSERT_TRUE(AddAxisADuplicate.ErrorID = WORD#16#0001);

AddAxisB(AxesGroup := Group, Axis := AxisB, Execute := TRUE, IdentInGroup := 'AxisB');
Probe(Group := Group);
ASSERT_TRUE(AddAxisB.Done);
ASSERT_TRUE(Probe.MemberCount = UINT#2);

ReadConfig(AxesGroup := Group, Enable := TRUE, IdentInGroup := 'AxisB', CoordSystem := MC_COORD_SYSTEM#mcMCS);
ASSERT_TRUE(ReadConfig.Valid);
ASSERT_TRUE(ReadConfig.Axis.AxisId = UDINT#202);
ASSERT_TRUE(ReadConfig.Axis.InternalIndex = UINT#6);
ASSERT_TRUE(ReadConfig.AxisID = UDINT#202);

ReadInfo(Axis := AxisB, Enable := TRUE);
IdentMatch := FIND(IN1 := ReadInfo.IdentInGroup, IN2 := 'AxisB');
ASSERT_TRUE(ReadInfo.Valid);
ASSERT_TRUE(ReadInfo.AxesGroup.GroupId = UDINT#104);
ASSERT_TRUE(ReadInfo.AxesGroup.InternalIndex = Group.InternalIndex);
ASSERT_TRUE(ReadInfo.AxesGroupID = UDINT#104);
ASSERT_TRUE(IdentMatch = INT#1);

RemoveAxisA(AxesGroup := Group, Execute := TRUE, IdentInGroup := 'AxisA');
Probe(Group := Group);
ASSERT_TRUE(RemoveAxisA.Done);
ASSERT_TRUE(Probe.MemberCount = UINT#1);

UngroupAll(AxesGroup := Group, Execute := TRUE);
Probe(Group := Group);
ASSERT_TRUE(UngroupAll.Done);
ASSERT_TRUE(Probe.MemberCount = UINT#0);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupDisabled);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_admin_state_and_power_fbs
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
    EnableGroup : MC_GroupEnable;
    DisableGroup : MC_GroupDisable;
    GroupPower : MC_GroupPower;
    GroupReset : MC_GroupReset;
    Probe : TRUST_GROUP_TestProbe;
    PowerHelperApplied : BOOL;
END_VAR

Group.GroupId := UDINT#32;
Group.InternalIndex := UINT#5;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupDisabled,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);

EnableGroup(AxesGroup := Group, Execute := TRUE);
Probe(Group := Group);
ASSERT_TRUE(EnableGroup.Done);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStandby);

GroupPower(AxesGroup := Group, Enable := TRUE);
Probe(Group := Group);
ASSERT_TRUE(GroupPower.Valid);
ASSERT_TRUE(GroupPower.Status);
ASSERT_TRUE(Probe.PowerOn);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStandby);

DisableGroup(AxesGroup := Group, Execute := TRUE);
Probe(Group := Group);
ASSERT_TRUE(DisableGroup.Done);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupDisabled);

Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupMoving,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcPCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
PowerHelperApplied := TRUST_GROUP_SetPowerAndError(Group := Group, PowerOn := TRUE, GroupErrorID := WORD#16#0000);
ASSERT_TRUE(PowerHelperApplied);
GroupPower(AxesGroup := Group, Enable := FALSE);
Probe(Group := Group);
ASSERT_TRUE(NOT GroupPower.Status);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupErrorStop);
ASSERT_TRUE(Probe.GroupErrorID = WORD#16#0105);

GroupReset(AxesGroup := Group, Execute := TRUE);
Probe(Group := Group);
ASSERT_TRUE(GroupReset.Done);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStandby);
ASSERT_TRUE(Probe.GroupErrorID = WORD#16#0000);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_status_and_error_readback_fbs
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
    ReadStatus : MC_GroupReadStatus;
    ReadError : MC_GroupReadError;
    PowerHelperApplied : BOOL;
END_VAR

Group.GroupId := UDINT#33;
Group.InternalIndex := UINT#6;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupHoming,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
PowerHelperApplied := TRUST_GROUP_SetPowerAndError(Group := Group, PowerOn := TRUE, GroupErrorID := WORD#16#0201);
ASSERT_TRUE(PowerHelperApplied);

ReadStatus(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadStatus.Valid);
ASSERT_TRUE(ReadStatus.GroupHoming);
ASSERT_TRUE(NOT ReadStatus.GroupMoving);
ASSERT_TRUE(NOT ReadStatus.GroupDisabled);

ReadError(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadError.Valid);
ASSERT_TRUE(ReadError.GroupErrorID = WORD#16#0201);

ReadStatus(AxesGroup := Group, Enable := FALSE);
ASSERT_TRUE(NOT ReadStatus.Valid);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_readback_position_velocity_acceleration
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
    PositionCommandedMCS : MC_GroupReadPosition;
    PositionActualPCS : MC_GroupReadPosition;
    VelocityCommanded : MC_GroupReadVelocity;
    VelocityActual : MC_GroupReadVelocity;
    AccelCommanded : MC_GroupReadAcceleration;
    AccelActual : MC_GroupReadAcceleration;
    KinematicsApplied : BOOL;
END_VAR

Group.GroupId := UDINT#34;
Group.InternalIndex := UINT#7;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#10.0,
    CommandedY := REAL#20.0,
    CommandedZ := REAL#30.0,
    CommandedRX := REAL#1.0,
    CommandedRY := REAL#2.0,
    CommandedRZ := REAL#3.0,
    ActualX := REAL#9.5,
    ActualY := REAL#19.5,
    ActualZ := REAL#29.5,
    ActualRX := REAL#0.5,
    ActualRY := REAL#1.5,
    ActualRZ := REAL#2.5
);
KinematicsApplied := TRUST_GROUP_SetPathKinematics(
    Group := Group,
    CommandedPathVelocity := REAL#4.0,
    ActualPathVelocity := REAL#3.5,
    CommandedPathAcceleration := REAL#2.0,
    ActualPathAcceleration := REAL#1.5
);
ASSERT_TRUE(KinematicsApplied);

PositionCommandedMCS(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcMCS, Source := MC_SOURCE#mcCommandedValue);
PositionActualPCS(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcPCS, Source := MC_SOURCE#mcActualValue);
VelocityCommanded(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcMCS, Source := MC_SOURCE#mcCommandedValue);
VelocityActual(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcPCS, Source := MC_SOURCE#mcActualValue);
AccelCommanded(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcMCS, Source := MC_SOURCE#mcCommandedValue);
AccelActual(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcPCS, Source := MC_SOURCE#mcActualValue);

ASSERT_TRUE(PositionCommandedMCS.Valid);
ASSERT_TRUE(PositionCommandedMCS.Position.C.Tcp.X = REAL#10.0);
ASSERT_TRUE(PositionActualPCS.Position.C.Tcp.X = REAL#209.5);
ASSERT_TRUE(PositionActualPCS.Position.C.Tcp.Y = REAL#219.5);
ASSERT_TRUE(VelocityCommanded.PathVelocity = REAL#4.0);
ASSERT_TRUE(VelocityActual.PathVelocity = REAL#3.5);
ASSERT_TRUE(VelocityCommanded.Velocity[1] = REAL#4.0);
ASSERT_TRUE(AccelCommanded.PathAcceleration = REAL#2.0);
ASSERT_TRUE(AccelActual.PathAcceleration = REAL#1.5);
ASSERT_TRUE(AccelActual.Acceleration[1] = REAL#1.5);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_read_motion_state_fbs
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
    ReadState : MC_GroupReadMotionState;
    MotionFlagsApplied : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
END_VAR

Group.GroupId := UDINT#35;
Group.InternalIndex := UINT#0;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
MotionFlagsApplied := TRUST_GROUP_SetMotionFlags(
    Group := Group,
    Tracking := FALSE,
    InSync := TRUE,
    InPosition := TRUE,
    ConstantVelocity := TRUE,
    Accelerating := FALSE,
    Decelerating := FALSE
);
ASSERT_TRUE(MotionFlagsApplied);
ReadState(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadState.Valid);
ASSERT_TRUE(ReadState.Standstill);
ASSERT_TRUE(ReadState.InSync);
ASSERT_TRUE(ReadState.InPosition);
ASSERT_TRUE(ReadState.ActiveCommandID = UINT#0);

Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupMoving,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcPCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
MotionFlagsApplied := TRUST_GROUP_SetMotionFlags(
    Group := Group,
    Tracking := TRUE,
    InSync := FALSE,
    InPosition := FALSE,
    ConstantVelocity := FALSE,
    Accelerating := TRUE,
    Decelerating := FALSE
);
ASSERT_TRUE(MotionFlagsApplied);
AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := Group, CoordSystem := MC_COORD_SYSTEM#mcPCS);
ReadState(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadState.Valid);
ASSERT_TRUE(ReadState.Tracking);
ASSERT_TRUE(NOT ReadState.InSync);
ASSERT_TRUE(NOT ReadState.Standstill);
ASSERT_TRUE(ReadState.Accelerating);
ASSERT_TRUE(ReadState.ActiveCommandID = AcceptedCommandID);

Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupErrorStop,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcPCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
ReadState(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(NOT ReadState.Valid);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_parameter_fbs
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    WriteDynamics : MC_GroupWriteParameter;
    WriteDynamicsInvalid : MC_GroupWriteParameter;
    WriteDynamicsDelayed : MC_GroupWriteParameter;
    ReadDynamics : MC_GroupReadParameter;
END_VAR

Group.GroupId := UDINT#36;
Group.InternalIndex := UINT#1;
Axis.AxisId := UDINT#301;
Axis.InternalIndex := UINT#3;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'ParamAxis');
ASSERT_TRUE(AddAxis.Done);

WriteDynamics(AxesGroup := Group, Execute := TRUE, ParameterNumber := MC_GROUP_PARAMETER#mcDynamicsMode, Value := REAL#1.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ReadDynamics(AxesGroup := Group, Enable := TRUE, ParameterNumber := MC_GROUP_PARAMETER#mcDynamicsMode);
ASSERT_TRUE(WriteDynamics.Done);
ASSERT_TRUE(WriteDynamics.CommandAccepted);
ASSERT_TRUE(WriteDynamics.CommandID <> UINT#0);
ASSERT_TRUE(ReadDynamics.Valid);
ASSERT_TRUE(ReadDynamics.Value = REAL#1.0);

WriteDynamicsInvalid(AxesGroup := Group, Execute := TRUE, ParameterNumber := MC_GROUP_PARAMETER#mcDynamicsMode, Value := REAL#2.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ASSERT_TRUE(WriteDynamicsInvalid.Error);
ASSERT_TRUE(WriteDynamicsInvalid.ErrorID = WORD#16#0001);

WriteDynamicsDelayed(AxesGroup := Group, Execute := TRUE, ParameterNumber := MC_GROUP_PARAMETER#mcDynamicsMode, Value := REAL#0.0, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(WriteDynamicsDelayed.Error);
ASSERT_TRUE(WriteDynamicsDelayed.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_swlimit_fbs
VAR
    Group : AXES_GROUP_REF;
    Seed : TRUST_GROUP_TestSeed;
    LimitsIn : MC_GROUP_SWLIMITS;
    Limit1 : MC_SWLIMIT;
    Limit2 : MC_SWLIMIT;
    Limit3 : MC_SWLIMIT;
    Limit4 : MC_SWLIMIT;
    WriteLimits : MC_GroupWriteSWLimits;
    WriteLimitsDelayed : MC_GroupWriteSWLimits;
    ReadLimits : MC_GroupReadSWLimits;
END_VAR

Group.GroupId := UDINT#37;
Group.InternalIndex := UINT#2;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);

Limit1.SWLimitPos := REAL#10.0;
Limit1.SWLimitNeg := REAL#-10.0;
Limit2.SWLimitPos := REAL#20.0;
Limit2.SWLimitNeg := REAL#-20.0;
Limit3.SWLimitPos := REAL#30.0;
Limit3.SWLimitNeg := REAL#-30.0;
Limit4.SWLimitPos := REAL#40.0;
Limit4.SWLimitNeg := REAL#-40.0;
LimitsIn[1] := Limit1;
LimitsIn[2] := Limit2;
LimitsIn[3] := Limit3;
LimitsIn[4] := Limit4;
WriteLimits(AxesGroup := Group, Execute := TRUE, LimitValues := LimitsIn, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
ReadLimits(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(WriteLimits.Done);
ASSERT_TRUE(ReadLimits.Valid);
ASSERT_TRUE(ReadLimits.LimitValues[1].SWLimitPos = REAL#10.0);
ASSERT_TRUE(ReadLimits.LimitValues[2].SWLimitNeg = REAL#-20.0);
ASSERT_TRUE(ReadLimits.LimitValues[4].SWLimitPos = REAL#40.0);

WriteLimitsDelayed(AxesGroup := Group, Execute := TRUE, LimitValues := LimitsIn, ExecutionMode := MC_EXECUTION_MODE#mcDelayed);
ASSERT_TRUE(WriteLimitsDelayed.Error);
ASSERT_TRUE(WriteLimitsDelayed.ErrorID = WORD#16#0500);
END_TEST_PROGRAM


TEST_PROGRAM plcopen_motion_coordinated_motion_transform_admin_and_readback_fbs
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    SetKin : MC_SetKinTransform;
    ReadKin : MC_ReadKinTransform;
    SetCartesian : MC_SetCartesianTransform;
    SetCartesianUnsupported : MC_SetCartesianTransform;
    ReadCartesian : MC_ReadCartesianTransform;
    CoordTransformIn : MC_COORD_REF;
    SetCoordinate : MC_SetCoordinateTransform;
    ReadCoordinate : MC_ReadCoordinateTransform;
END_VAR

Group.GroupId := UDINT#41;
Group.InternalIndex := UINT#3;
Axis.AxisId := UDINT#401;
Axis.InternalIndex := UINT#4;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'TransformAxis');
ASSERT_TRUE(AddAxis.Done);

SetKin(AxesGroup := Group, Execute := TRUE, KinTransform := UINT#11, ExecutionMode := MC_EXECUTION_MODE#mcQueued);
ASSERT_TRUE(SetKin.Done);
ASSERT_TRUE(SetKin.CommandAccepted);
ASSERT_TRUE(SetKin.CommandID <> UINT#0);

ReadKin(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadKin.Valid);
ASSERT_TRUE(ReadKin.KinTransform = UINT#11);

SetCartesian(
    AxesGroup := Group,
    Execute := TRUE,
    TransX := REAL#1.0,
    TransY := REAL#2.0,
    TransZ := REAL#3.0,
    RotAngle1 := REAL#4.0,
    RotAngle2 := REAL#5.0,
    RotAngle3 := REAL#6.0,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(SetCartesian.Done);
ASSERT_TRUE(SetCartesian.CommandAccepted);

ReadCartesian(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcPCS);
ASSERT_TRUE(ReadCartesian.Valid);
ASSERT_TRUE(ReadCartesian.TransX = REAL#1.0);
ASSERT_TRUE(ReadCartesian.TransY = REAL#2.0);
ASSERT_TRUE(ReadCartesian.TransZ = REAL#3.0);
ASSERT_TRUE(ReadCartesian.RotAngle1 = REAL#4.0);
ASSERT_TRUE(ReadCartesian.RotAngle2 = REAL#5.0);
ASSERT_TRUE(ReadCartesian.RotAngle3 = REAL#6.0);

CoordTransformIn.X := REAL#7.0;
CoordTransformIn.Y := REAL#8.0;
CoordTransformIn.Z := REAL#9.0;
CoordTransformIn.RX := REAL#10.0;
CoordTransformIn.RY := REAL#11.0;
CoordTransformIn.RZ := REAL#12.0;
SetCoordinate(
    AxesGroup := Group,
    Execute := TRUE,
    CoordTransform := CoordTransformIn,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(SetCoordinate.Done);
ASSERT_TRUE(SetCoordinate.CommandAccepted);

ReadCoordinate(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcPCS);
ASSERT_TRUE(ReadCoordinate.Valid);
ASSERT_TRUE(ReadCoordinate.CoordTransform.X = REAL#7.0);
ASSERT_TRUE(ReadCoordinate.CoordTransform.Y = REAL#8.0);
ASSERT_TRUE(ReadCoordinate.CoordTransform.Z = REAL#9.0);
ASSERT_TRUE(ReadCoordinate.CoordTransform.RX = REAL#10.0);
ASSERT_TRUE(ReadCoordinate.CoordTransform.RY = REAL#11.0);
ASSERT_TRUE(ReadCoordinate.CoordTransform.RZ = REAL#12.0);

SetCartesianUnsupported(
    AxesGroup := Group,
    Execute := TRUE,
    TransX := REAL#0.0,
    TransY := REAL#0.0,
    TransZ := REAL#0.0,
    RotAngle1 := REAL#0.0,
    RotAngle2 := REAL#0.0,
    RotAngle3 := REAL#0.0,
    CoordSystem := MC_COORD_SYSTEM#mcWCS,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(SetCartesianUnsupported.Error);
ASSERT_TRUE(SetCartesianUnsupported.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_set_position_and_transform_position
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    CoordTransformIn : MC_COORD_REF;
    SetCoordinate : MC_SetCoordinateTransform;
    AbsolutePosition : MC_POS_REF;
    RelativePosition : MC_POS_REF;
    SetPositionAbsolute : MC_GroupSetPosition;
    SetPositionRelative : MC_GroupSetPosition;
    ReadPositionMCS : MC_GroupReadPosition;
    ReadPositionPCS : MC_GroupReadPosition;
    InputPosition : MC_POS_REF;
    TransformPosition : MC_TransformPosition;
    TransformPositionUnsupported : MC_TransformPosition;
    WorkCart : MC_CART_REF;
    WorkCartPos : MC_CART_POS_REF;
END_VAR

Group.GroupId := UDINT#42;
Group.InternalIndex := UINT#4;
Axis.AxisId := UDINT#402;
Axis.InternalIndex := UINT#5;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'PositionAxis');
ASSERT_TRUE(AddAxis.Done);

CoordTransformIn.X := REAL#7.0;
CoordTransformIn.Y := REAL#8.0;
CoordTransformIn.Z := REAL#9.0;
CoordTransformIn.RX := REAL#10.0;
CoordTransformIn.RY := REAL#11.0;
CoordTransformIn.RZ := REAL#12.0;
SetCoordinate(
    AxesGroup := Group,
    Execute := TRUE,
    CoordTransform := CoordTransformIn,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(SetCoordinate.Done);

WorkCart.X := REAL#305.0;
WorkCart.Y := REAL#315.0;
WorkCart.Z := REAL#325.0;
WorkCart.RX := REAL#230.0;
WorkCart.RY := REAL#240.0;
WorkCart.RZ := REAL#250.0;
WorkCartPos.Tcp := WorkCart;
AbsolutePosition.C := WorkCartPos;
SetPositionAbsolute(
    AxesGroup := Group,
    Execute := TRUE,
    Position := AbsolutePosition,
    Relative := FALSE,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(SetPositionAbsolute.Done);
ASSERT_TRUE(SetPositionAbsolute.CommandAccepted);
ASSERT_TRUE(SetPositionAbsolute.CommandID <> UINT#0);

ReadPositionMCS(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcMCS, Source := MC_SOURCE#mcCommandedValue);
ASSERT_TRUE(ReadPositionMCS.Valid);
ASSERT_TRUE(ReadPositionMCS.Position.C.Tcp.X = REAL#105.0);
ASSERT_TRUE(ReadPositionMCS.Position.C.Tcp.Y = REAL#115.0);
ASSERT_TRUE(ReadPositionMCS.Position.C.Tcp.Z = REAL#125.0);

WorkCart.X := REAL#5.0;
WorkCart.Y := REAL#-10.0;
WorkCart.Z := REAL#2.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
RelativePosition.C := WorkCartPos;
SetPositionRelative(
    AxesGroup := Group,
    Execute := TRUE,
    Position := RelativePosition,
    Relative := TRUE,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    ExecutionMode := MC_EXECUTION_MODE#mcQueued
);
ASSERT_TRUE(SetPositionRelative.Done);
ASSERT_TRUE(SetPositionRelative.CommandAccepted);
ASSERT_TRUE(SetPositionRelative.CommandID <> UINT#0);

ReadPositionPCS(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcPCS, Source := MC_SOURCE#mcActualValue);
ASSERT_TRUE(ReadPositionPCS.Valid);
ASSERT_TRUE(ReadPositionPCS.Position.C.Tcp.X = REAL#310.0);
ASSERT_TRUE(ReadPositionPCS.Position.C.Tcp.Y = REAL#305.0);
ASSERT_TRUE(ReadPositionPCS.Position.C.Tcp.Z = REAL#327.0);

WorkCart.X := REAL#10.0;
WorkCart.Y := REAL#20.0;
WorkCart.Z := REAL#30.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
InputPosition.C := WorkCartPos;
TransformPosition(
    AxesGroup := Group,
    Enable := TRUE,
    InputPosition := InputPosition,
    InputCoordSystem := MC_COORD_SYSTEM#mcMCS,
    OutputCoordSystem := MC_COORD_SYSTEM#mcPCS
);
ASSERT_TRUE(TransformPosition.Valid);
ASSERT_TRUE(TransformPosition.CommandID = SetPositionRelative.CommandID);
ASSERT_TRUE(TransformPosition.OutputPosition.C.Tcp.X = REAL#217.0);
ASSERT_TRUE(TransformPosition.OutputPosition.C.Tcp.Y = REAL#228.0);
ASSERT_TRUE(TransformPosition.OutputPosition.C.Tcp.Z = REAL#239.0);
ASSERT_TRUE(NOT TransformPosition.SingularPosition);

TransformPositionUnsupported(
    AxesGroup := Group,
    Enable := TRUE,
    InputPosition := InputPosition,
    InputCoordSystem := MC_COORD_SYSTEM#mcWCS,
    OutputCoordSystem := MC_COORD_SYSTEM#mcPCS
);
ASSERT_TRUE(TransformPositionUnsupported.Error);
ASSERT_TRUE(TransformPositionUnsupported.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_linear_and_direct_motion_fbs
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    LinearAbsolutePosition : MC_POS_REF;
    LinearRelativeDistance : MC_DISTANCE_REF;
    DirectAbsolutePosition : MC_POS_REF;
    DirectRelativeDistance : MC_DISTANCE_REF;
    MoveLinearAbsoluteFB : MC_MoveLinearAbsolute;
    MoveLinearRelativeFB : MC_MoveLinearRelative;
    MoveDirectAbsoluteFB : MC_MoveDirectAbsolute;
    MoveDirectRelativeFB : MC_MoveDirectRelative;
    Probe : TRUST_GROUP_TestProbe;
    WorkCart : MC_CART_REF;
    WorkCartPos : MC_CART_POS_REF;
END_VAR

Group.GroupId := UDINT#43;
Group.InternalIndex := UINT#5;
Axis.AxisId := UDINT#403;
Axis.InternalIndex := UINT#6;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'MotionAxis');
ASSERT_TRUE(AddAxis.Done);

WorkCart.X := REAL#40.0;
WorkCart.Y := REAL#50.0;
WorkCart.Z := REAL#60.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
LinearAbsolutePosition.C := WorkCartPos;
MoveLinearAbsoluteFB(
    AxesGroup := Group,
    Execute := TRUE,
    Position := LinearAbsolutePosition,
    Velocity := REAL#12.0,
    Acceleration := REAL#6.0,
    Deceleration := REAL#6.0,
    Jerk := REAL#3.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcAborting,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcLinear
);
ASSERT_TRUE(MoveLinearAbsoluteFB.CommandAccepted);
ASSERT_TRUE(MoveLinearAbsoluteFB.CommandID <> UINT#0);
ASSERT_TRUE(MoveLinearAbsoluteFB.Busy);
ASSERT_TRUE(MoveLinearAbsoluteFB.Active);

MoveLinearAbsoluteFB(
    AxesGroup := Group,
    Execute := TRUE,
    Position := LinearAbsolutePosition,
    Velocity := REAL#12.0,
    Acceleration := REAL#6.0,
    Deceleration := REAL#6.0,
    Jerk := REAL#3.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcAborting,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcLinear
);
Probe(Group := Group);
ASSERT_TRUE(MoveLinearAbsoluteFB.Done);
ASSERT_TRUE(Probe.CommandedX = REAL#40.0);
ASSERT_TRUE(Probe.PathVelocity = REAL#12.0);

WorkCart.X := REAL#10.0;
WorkCart.Y := REAL#5.0;
WorkCart.Z := REAL#-2.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
LinearRelativeDistance.C := WorkCartPos;
MoveLinearRelativeFB(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := LinearRelativeDistance,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#2.0,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVNext,
    TransitionMode := MC_TRANSITION_MODE#mcTMCornerDistance,
    TransitionParameter := REAL#1.5,
    OrientationMode := MC_ORIENTATION_MODE#mcPathBased
);
ASSERT_TRUE(MoveLinearRelativeFB.CommandAccepted);
ASSERT_TRUE(MoveLinearRelativeFB.CommandID <> UINT#0);

MoveLinearRelativeFB(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := LinearRelativeDistance,
    Velocity := REAL#8.0,
    Acceleration := REAL#4.0,
    Deceleration := REAL#4.0,
    Jerk := REAL#2.0,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVNext,
    TransitionMode := MC_TRANSITION_MODE#mcTMCornerDistance,
    TransitionParameter := REAL#1.5,
    OrientationMode := MC_ORIENTATION_MODE#mcPathBased
);
Probe(Group := Group);
ASSERT_TRUE(MoveLinearRelativeFB.Done);
ASSERT_TRUE(Probe.CommandedX = REAL#50.0);
ASSERT_TRUE(Probe.CommandedY = REAL#55.0);

WorkCart.X := REAL#5.0;
WorkCart.Y := REAL#6.0;
WorkCart.Z := REAL#7.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
DirectAbsolutePosition.C := WorkCartPos;
MoveDirectAbsoluteFB(
    AxesGroup := Group,
    Execute := TRUE,
    Position := DirectAbsolutePosition,
    Velocity := REAL#7.0,
    Acceleration := REAL#3.0,
    Deceleration := REAL#3.0,
    Jerk := REAL#1.5,
    CoordSystem := MC_COORD_SYSTEM#mcACS,
    BufferMode := MC_BUFFER_MODE#mcAborting,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcJointInterpolated
);
ASSERT_TRUE(MoveDirectAbsoluteFB.CommandAccepted);

MoveDirectAbsoluteFB(
    AxesGroup := Group,
    Execute := TRUE,
    Position := DirectAbsolutePosition,
    Velocity := REAL#7.0,
    Acceleration := REAL#3.0,
    Deceleration := REAL#3.0,
    Jerk := REAL#1.5,
    CoordSystem := MC_COORD_SYSTEM#mcACS,
    BufferMode := MC_BUFFER_MODE#mcAborting,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcJointInterpolated
);
Probe(Group := Group);
ASSERT_TRUE(MoveDirectAbsoluteFB.Done);
ASSERT_TRUE(Probe.CommandedX = REAL#105.0);

WorkCart.X := REAL#2.0;
WorkCart.Y := REAL#3.0;
WorkCart.Z := REAL#4.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
DirectRelativeDistance.C := WorkCartPos;
MoveDirectRelativeFB(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := DirectRelativeDistance,
    Velocity := REAL#6.0,
    Acceleration := REAL#3.0,
    Deceleration := REAL#3.0,
    Jerk := REAL#1.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVNext,
    TransitionMode := MC_TRANSITION_MODE#mcTMCornerDistance,
    TransitionParameter := REAL#0.5,
    OrientationMode := MC_ORIENTATION_MODE#mcFixed
);
ASSERT_TRUE(MoveDirectRelativeFB.CommandAccepted);

MoveDirectRelativeFB(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := DirectRelativeDistance,
    Velocity := REAL#6.0,
    Acceleration := REAL#3.0,
    Deceleration := REAL#3.0,
    Jerk := REAL#1.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVNext,
    TransitionMode := MC_TRANSITION_MODE#mcTMCornerDistance,
    TransitionParameter := REAL#0.5,
    OrientationMode := MC_ORIENTATION_MODE#mcFixed
);
Probe(Group := Group);
ASSERT_TRUE(MoveDirectRelativeFB.Done);
ASSERT_TRUE(Probe.CommandedX = REAL#107.0);
ASSERT_TRUE(Probe.CommandedY = REAL#109.0);
ASSERT_TRUE(Probe.CommandedZ = REAL#111.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_group_home_stop_halt_wait_and_override_fbs
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    GroupHome : MC_GroupHome;
    GroupHomeInvalid : MC_GroupHome;
    GroupStop : MC_GroupStop;
    MoveWhileStopping : MC_MoveLinearRelative;
    GroupHalt : MC_GroupHalt;
    GroupWait : MC_GroupWaitTime;
    GroupOverride : MC_GroupSetOverride;
    VelocityActualAfterOverride : MC_GroupReadVelocity;
    Probe : TRUST_GROUP_TestProbe;
    MotionFlagsApplied : BOOL;
    HomePosition : MC_POS_REF;
    RelativeDistance : MC_DISTANCE_REF;
    WorkCart : MC_CART_REF;
    WorkCartPos : MC_CART_POS_REF;
END_VAR

Group.GroupId := UDINT#44;
Group.InternalIndex := UINT#6;
Axis.AxisId := UDINT#404;
Axis.InternalIndex := UINT#7;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'HomeAxis');
ASSERT_TRUE(AddAxis.Done);

WorkCart.X := REAL#1.0;
WorkCart.Y := REAL#2.0;
WorkCart.Z := REAL#3.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
HomePosition.C := WorkCartPos;
GroupHome(
    AxesGroup := Group,
    Execute := TRUE,
    Position := HomePosition,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
Probe(Group := Group);
ASSERT_TRUE(GroupHome.CommandAccepted);
ASSERT_TRUE(GroupHome.Busy);
ASSERT_TRUE(GroupHome.Active);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupHoming);

GroupHome(
    AxesGroup := Group,
    Execute := TRUE,
    Position := HomePosition,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
Probe(Group := Group);
ASSERT_TRUE(GroupHome.Done);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStandby);

GroupHomeInvalid(
    AxesGroup := Group,
    Execute := TRUE,
    Position := HomePosition,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBlendingLow
);
ASSERT_TRUE(GroupHomeInvalid.Error);
ASSERT_TRUE(GroupHomeInvalid.ErrorID = WORD#16#0500);

g_trust_group_status[Group.InternalIndex] := MC_GROUP_STATUS#mcGroupMoving;
g_trust_group_active_coord_system[Group.InternalIndex] := MC_COORD_SYSTEM#mcMCS;
g_trust_group_transform_identity[Group.InternalIndex] := TRUE;
g_trust_group_commanded_x[Group.InternalIndex] := REAL#10.0;
g_trust_group_commanded_y[Group.InternalIndex] := REAL#20.0;
g_trust_group_commanded_z[Group.InternalIndex] := REAL#30.0;
g_trust_group_commanded_rx[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_ry[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_rz[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_x[Group.InternalIndex] := REAL#10.0;
g_trust_group_actual_y[Group.InternalIndex] := REAL#20.0;
g_trust_group_actual_z[Group.InternalIndex] := REAL#30.0;
g_trust_group_actual_rx[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_ry[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_rz[Group.InternalIndex] := REAL#0.0;
g_trust_group_stop_hold[Group.InternalIndex] := FALSE;
MotionFlagsApplied := TRUST_GROUP_SetMotionFlags(
    Group := Group,
    Tracking := TRUE,
    InSync := TRUE,
    InPosition := FALSE,
    ConstantVelocity := FALSE,
    Accelerating := FALSE,
    Decelerating := TRUE
);
ASSERT_TRUE(MotionFlagsApplied);
TRUST_GROUP_SetPathKinematics(
    Group := Group,
    CommandedPathVelocity := REAL#9.0,
    ActualPathVelocity := REAL#9.0,
    CommandedPathAcceleration := REAL#1.0,
    ActualPathAcceleration := REAL#1.0
);

GroupStop(AxesGroup := Group, Execute := TRUE, Deceleration := REAL#4.0, Jerk := REAL#2.0);
Probe(Group := Group);
ASSERT_TRUE(GroupStop.Busy);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStopping);
ASSERT_TRUE(NOT Probe.Tracking);
ASSERT_TRUE(NOT Probe.InSync);

WorkCart.X := REAL#1.0;
WorkCart.Y := REAL#0.0;
WorkCart.Z := REAL#0.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
RelativeDistance.C := WorkCartPos;
MoveWhileStopping(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := RelativeDistance,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcAborting,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcLinear
);
ASSERT_TRUE(MoveWhileStopping.Error);

GroupStop(AxesGroup := Group, Execute := TRUE, Deceleration := REAL#4.0, Jerk := REAL#2.0);
Probe(Group := Group);
ASSERT_TRUE(GroupStop.Done);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStopping);

GroupStop(AxesGroup := Group, Execute := FALSE, Deceleration := REAL#4.0, Jerk := REAL#2.0);
Probe(Group := Group);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupStandby);

g_trust_group_status[Group.InternalIndex] := MC_GROUP_STATUS#mcGroupMoving;
g_trust_group_active_coord_system[Group.InternalIndex] := MC_COORD_SYSTEM#mcPCS;
g_trust_group_transform_identity[Group.InternalIndex] := TRUE;
g_trust_group_commanded_x[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_y[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_z[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_rx[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_ry[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_rz[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_x[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_y[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_z[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_rx[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_ry[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_rz[Group.InternalIndex] := REAL#0.0;
g_trust_group_stop_hold[Group.InternalIndex] := FALSE;
MotionFlagsApplied := TRUST_GROUP_SetMotionFlags(
    Group := Group,
    Tracking := TRUE,
    InSync := TRUE,
    InPosition := FALSE,
    ConstantVelocity := FALSE,
    Accelerating := FALSE,
    Decelerating := TRUE
);
ASSERT_TRUE(MotionFlagsApplied);
GroupHalt(AxesGroup := Group, Execute := TRUE, Deceleration := REAL#3.0, Jerk := REAL#1.5, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(GroupHalt.CommandAccepted);
ASSERT_TRUE(GroupHalt.CommandID <> UINT#0);
ASSERT_TRUE(GroupHalt.Busy);
ASSERT_TRUE(GroupHalt.Active);

GroupHalt(AxesGroup := Group, Execute := TRUE, Deceleration := REAL#3.0, Jerk := REAL#1.5, BufferMode := MC_BUFFER_MODE#mcBuffered);
Probe(Group := Group);
ASSERT_TRUE(GroupHalt.Done);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupMoving);

GroupWait(AxesGroup := Group, Execute := TRUE, Duration := TIME#250ms, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(GroupWait.Busy);
ASSERT_TRUE(GroupWait.Active);
ASSERT_TRUE(GroupWait.CommandID <> UINT#0);

GroupWait(AxesGroup := Group, Execute := TRUE, Duration := TIME#250ms, BufferMode := MC_BUFFER_MODE#mcBuffered);
ASSERT_TRUE(GroupWait.Done);

GroupOverride(AxesGroup := Group, Enable := TRUE, VelFactor := REAL#1.0, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(GroupOverride.Enabled);

g_trust_group_status[Group.InternalIndex] := MC_GROUP_STATUS#mcGroupMoving;
g_trust_group_active_coord_system[Group.InternalIndex] := MC_COORD_SYSTEM#mcMCS;
g_trust_group_transform_identity[Group.InternalIndex] := TRUE;
g_trust_group_commanded_x[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_y[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_z[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_rx[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_ry[Group.InternalIndex] := REAL#0.0;
g_trust_group_commanded_rz[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_x[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_y[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_z[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_rx[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_ry[Group.InternalIndex] := REAL#0.0;
g_trust_group_actual_rz[Group.InternalIndex] := REAL#0.0;
g_trust_group_stop_hold[Group.InternalIndex] := FALSE;
TRUST_GROUP_SetPathKinematics(
    Group := Group,
    CommandedPathVelocity := REAL#5.0,
    ActualPathVelocity := REAL#5.0,
    CommandedPathAcceleration := REAL#2.0,
    ActualPathAcceleration := REAL#2.0
);
GroupOverride(AxesGroup := Group, Enable := TRUE, VelFactor := REAL#0.0, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
Probe(Group := Group);
VelocityActualAfterOverride(AxesGroup := Group, Enable := TRUE, CoordSystem := MC_COORD_SYSTEM#mcMCS, Source := MC_SOURCE#mcActualValue);
ASSERT_TRUE(GroupOverride.Enabled);
ASSERT_TRUE(Probe.Status = MC_GROUP_STATUS#mcGroupMoving);
ASSERT_TRUE(Probe.PathVelocity = REAL#5.0);
ASSERT_TRUE(VelocityActualAfterOverride.Valid);
ASSERT_TRUE(VelocityActualAfterOverride.PathVelocity = REAL#0.0);

GroupOverride(AxesGroup := Group, Enable := TRUE, VelFactor := REAL#0.0 - REAL#1.0, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(GroupOverride.Error);
ASSERT_TRUE(GroupOverride.ErrorID = WORD#16#0001);
GroupOverride(AxesGroup := Group, Enable := TRUE, VelFactor := REAL#1.0, AccFactor := REAL#0.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(GroupOverride.Error);
ASSERT_TRUE(GroupOverride.ErrorID = WORD#16#0001);
GroupOverride(AxesGroup := Group, Enable := TRUE, VelFactor := REAL#1.0, AccFactor := REAL#1.0, JerkFactor := REAL#0.0);
ASSERT_TRUE(GroupOverride.Error);
ASSERT_TRUE(GroupOverride.ErrorID = WORD#16#0001);
GroupOverride(AxesGroup := Group, Enable := TRUE, VelFactor := REAL#1.5, AccFactor := REAL#1.0, JerkFactor := REAL#1.0);
ASSERT_TRUE(GroupOverride.Error);
ASSERT_TRUE(GroupOverride.ErrorID = WORD#16#0500);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_dynamics_and_command_info_fbs
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    WriteReferenceDynamics : MC_GroupWriteReferenceDynamics;
    ReadReferenceDynamics : MC_GroupReadReferenceDynamics;
    WriteDefaultDynamics : MC_GroupWriteDefaultDynamics;
    ReadDefaultDynamics : MC_GroupReadDefaultDynamics;
    WriteTransitionReference : MC_GroupWriteParameter;
    ReadTransitionReference : MC_GroupReadParameter;
    LinearAbsolutePosition : MC_POS_REF;
    MoveLinearAbsoluteFB : MC_MoveLinearAbsolute;
    ReadCommandInfo : MC_GroupReadCommandInfo;
    WorkCart : MC_CART_REF;
    WorkCartPos : MC_CART_POS_REF;
END_VAR

Group.GroupId := UDINT#45;
Group.InternalIndex := UINT#7;
Axis.AxisId := UDINT#405;
Axis.InternalIndex := UINT#8;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'InfoAxis');
ASSERT_TRUE(AddAxis.Done);

WriteReferenceDynamics(AxesGroup := Group, Execute := TRUE, Velocity := REAL#90.0, Acceleration := REAL#45.0, Deceleration := REAL#35.0, Jerk := REAL#25.0);
ASSERT_TRUE(WriteReferenceDynamics.Done);
ReadReferenceDynamics(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadReferenceDynamics.Valid);
ASSERT_TRUE(ReadReferenceDynamics.Velocity = REAL#90.0);
ASSERT_TRUE(ReadReferenceDynamics.Acceleration = REAL#45.0);
ASSERT_TRUE(ReadReferenceDynamics.Deceleration = REAL#35.0);
ASSERT_TRUE(ReadReferenceDynamics.Jerk = REAL#25.0);

WriteDefaultDynamics(AxesGroup := Group, Execute := TRUE, Velocity := REAL#9.0, Acceleration := REAL#4.0, Deceleration := REAL#3.0, Jerk := REAL#2.0);
ASSERT_TRUE(WriteDefaultDynamics.Done);
ReadDefaultDynamics(AxesGroup := Group, Enable := TRUE);
ASSERT_TRUE(ReadDefaultDynamics.Valid);
ASSERT_TRUE(ReadDefaultDynamics.Velocity = REAL#9.0);
ASSERT_TRUE(ReadDefaultDynamics.Acceleration = REAL#4.0);
ASSERT_TRUE(ReadDefaultDynamics.Deceleration = REAL#3.0);
ASSERT_TRUE(ReadDefaultDynamics.Jerk = REAL#2.0);

WriteTransitionReference(
    AxesGroup := Group,
    Execute := TRUE,
    ParameterNumber := MC_GROUP_PARAMETER#mcTransitionReferencePoint,
    Value := REAL#1.0,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
ASSERT_TRUE(WriteTransitionReference.Done);
ASSERT_TRUE(WriteTransitionReference.CommandAccepted);
ReadTransitionReference(AxesGroup := Group, Enable := TRUE, ParameterNumber := MC_GROUP_PARAMETER#mcTransitionReferencePoint);
ASSERT_TRUE(ReadTransitionReference.Valid);
ASSERT_TRUE(ReadTransitionReference.Value = REAL#1.0);

WorkCart.X := REAL#25.0;
WorkCart.Y := REAL#35.0;
WorkCart.Z := REAL#45.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
LinearAbsolutePosition.C := WorkCartPos;
MoveLinearAbsoluteFB(
    AxesGroup := Group,
    Execute := TRUE,
    Position := LinearAbsolutePosition,
    Velocity := REAL#15.0,
    Acceleration := REAL#7.5,
    Deceleration := REAL#7.5,
    Jerk := REAL#3.5,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVNext,
    TransitionMode := MC_TRANSITION_MODE#mcTMCornerDistance,
    TransitionParameter := REAL#2.0,
    OrientationMode := MC_ORIENTATION_MODE#mcPathBased
);
ASSERT_TRUE(MoveLinearAbsoluteFB.CommandAccepted);
ASSERT_TRUE(MoveLinearAbsoluteFB.CommandID <> UINT#0);

ReadCommandInfo(AxesGroup := Group, Enable := TRUE, CommandID := MoveLinearAbsoluteFB.CommandID);
ASSERT_TRUE(ReadCommandInfo.Valid);
ASSERT_TRUE(ReadCommandInfo.CommandState = MC_COMMAND_STATE#mcAccepted);
ASSERT_TRUE(ReadCommandInfo.RemainingDistance > REAL#0.0);
ASSERT_TRUE(ReadCommandInfo.Progress = REAL#0.0);
ASSERT_TRUE(ReadCommandInfo.InfoID = WORD#16#1001);

MoveLinearAbsoluteFB(
    AxesGroup := Group,
    Execute := TRUE,
    Position := LinearAbsolutePosition,
    Velocity := REAL#15.0,
    Acceleration := REAL#7.5,
    Deceleration := REAL#7.5,
    Jerk := REAL#3.5,
    CoordSystem := MC_COORD_SYSTEM#mcPCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVNext,
    TransitionMode := MC_TRANSITION_MODE#mcTMCornerDistance,
    TransitionParameter := REAL#2.0,
    OrientationMode := MC_ORIENTATION_MODE#mcPathBased
);
ASSERT_TRUE(MoveLinearAbsoluteFB.Done);

ReadCommandInfo(AxesGroup := Group, Enable := TRUE, CommandID := MoveLinearAbsoluteFB.CommandID);
ASSERT_TRUE(ReadCommandInfo.Valid);
ASSERT_TRUE(ReadCommandInfo.CommandState = MC_COMMAND_STATE#mcActive);
ASSERT_TRUE(ReadCommandInfo.ElapsedDuration > TIME#0ms);
ASSERT_TRUE(ReadCommandInfo.RemainingDuration = TIME#0ms);
ASSERT_TRUE(ReadCommandInfo.RemainingDistance = REAL#0.0);
ASSERT_TRUE(ReadCommandInfo.Progress = REAL#1.0);
END_TEST_PROGRAM

TEST_PROGRAM plcopen_motion_coordinated_motion_unsupported_transition_and_legacy_blending_are_rejected
VAR
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    Seed : TRUST_GROUP_TestSeed;
    AddAxis : MC_AddAxisToGroup;
    RelativeDistance : MC_DISTANCE_REF;
    UnsupportedTransitionMode : MC_MoveLinearRelative;
    UnsupportedTransitionVelocity : MC_MoveLinearRelative;
    LegacyBlendBufferMode : MC_MoveLinearRelative;
    WorkCart : MC_CART_REF;
    WorkCartPos : MC_CART_POS_REF;
END_VAR

Group.GroupId := UDINT#46;
Group.InternalIndex := UINT#0;
Axis.AxisId := UDINT#406;
Axis.InternalIndex := UINT#9;
Seed(
    Group := Group,
    Status := MC_GROUP_STATUS#mcGroupStandby,
    ActiveCoordSystem := MC_COORD_SYSTEM#mcMCS,
    IdentityTransform := TRUE,
    CommandedX := REAL#0.0,
    CommandedY := REAL#0.0,
    CommandedZ := REAL#0.0,
    CommandedRX := REAL#0.0,
    CommandedRY := REAL#0.0,
    CommandedRZ := REAL#0.0,
    ActualX := REAL#0.0,
    ActualY := REAL#0.0,
    ActualZ := REAL#0.0,
    ActualRX := REAL#0.0,
    ActualRY := REAL#0.0,
    ActualRZ := REAL#0.0
);
AddAxis(AxesGroup := Group, Axis := Axis, Execute := TRUE, IdentInGroup := 'RejectAxis');
ASSERT_TRUE(AddAxis.Done);

WorkCart.X := REAL#1.0;
WorkCart.Y := REAL#0.0;
WorkCart.Z := REAL#0.0;
WorkCart.RX := REAL#0.0;
WorkCart.RY := REAL#0.0;
WorkCart.RZ := REAL#0.0;
WorkCartPos.Tcp := WorkCart;
RelativeDistance.C := WorkCartPos;
UnsupportedTransitionMode(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := RelativeDistance,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMStartVelocity,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcLinear
);
ASSERT_TRUE(UnsupportedTransitionMode.Error);
ASSERT_TRUE(UnsupportedTransitionMode.ErrorID = WORD#16#0500);

UnsupportedTransitionVelocity(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := RelativeDistance,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBuffered,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVHigh,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcLinear
);
ASSERT_TRUE(UnsupportedTransitionVelocity.Error);
ASSERT_TRUE(UnsupportedTransitionVelocity.ErrorID = WORD#16#0500);

LegacyBlendBufferMode(
    AxesGroup := Group,
    Execute := TRUE,
    Distance := RelativeDistance,
    Velocity := REAL#4.0,
    Acceleration := REAL#2.0,
    Deceleration := REAL#2.0,
    Jerk := REAL#1.0,
    CoordSystem := MC_COORD_SYSTEM#mcMCS,
    BufferMode := MC_BUFFER_MODE#mcBlendingHigh,
    TransitionVelocity := MC_TRANSITION_VELOCITY#mcTVZero,
    TransitionMode := MC_TRANSITION_MODE#mcTMNone,
    TransitionParameter := REAL#0.0,
    OrientationMode := MC_ORIENTATION_MODE#mcLinear
);
ASSERT_TRUE(LegacyBlendBufferMode.Error);
ASSERT_TRUE(LegacyBlendBufferMode.ErrorID = WORD#16#0500);
END_TEST_PROGRAM
