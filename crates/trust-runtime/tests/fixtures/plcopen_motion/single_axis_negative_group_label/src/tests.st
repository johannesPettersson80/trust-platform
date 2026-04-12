TEST_PROGRAM plcopen_motion_single_axis_negative_group_label
VAR
    WrongAxisStatus : MC_AXIS_STATUS;
END_VAR

WrongAxisStatus := MC_AXIS_STATUS#GroupStandby;
END_TEST_PROGRAM
