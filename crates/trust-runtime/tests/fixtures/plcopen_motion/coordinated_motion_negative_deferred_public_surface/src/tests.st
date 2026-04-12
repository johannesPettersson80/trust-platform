TEST_PROGRAM plcopen_motion_coordinated_motion_deferred_public_surface_is_absent
VAR
    MoveCircularAbsolute : MC_MoveCircularAbsolute;
    MoveCircularRelative : MC_MoveCircularRelative;
    CircMode : MC_CIRC_MODE;
    CircPathChoice : MC_CIRC_PATHCHOICE;
    PathSelect : MC_PathSelect;
    MovePath : MC_MovePath;
    GroupInterrupt : MC_GroupInterrupt;
    GroupContinue : MC_GroupContinue;
    ReadDHParameters : MC_ReadDHParameters;
    ReadJointInfo : MC_ReadJointInfo;
    GroupJog : MC_GroupJog;
    GroupJogVector : MC_GroupJogVector;
    GroupWriteJoggingDynamics : MC_GroupWriteJoggingDynamics;
    GroupReadJoggingDynamics : MC_GroupReadJoggingDynamics;
    WriteToolData : MC_WriteToolData;
    ReadToolData : MC_ReadToolData;
    SelectTool : MC_SelectTool;
    ReadTool : MC_ReadTool;
    ToolSource : MC_TOOL_SOURCE;
    WritePayloadData : MC_WritePayloadData;
    ReadPayloadData : MC_ReadPayloadData;
    SelectPayload : MC_SelectPayload;
    ReadPayload : MC_ReadPayload;
    ReadRigidBodyDynamic : MC_ReadRigidBodyDynamic;
    WriteRigidBodyDynamic : MC_WriteRigidBodyDynamic;
    SetDynCoordTransform : MC_SetDynCoordTransform;
    TrackConveyorBelt : MC_TrackConveyorBelt;
    SyncAxisToGroup : MC_SyncAxisToGroup;
    SyncGroupToAxis : MC_SyncGroupToAxis;
    TrackRotaryTable : MC_TrackRotaryTable;
END_VAR
END_TEST_PROGRAM
