TEST_PROGRAM plcopen_motion_single_axis_negative_power_enable_split
VAR
    Axis : AXIS_REF;
    Power : MC_Power;
END_VAR

Power(Axis := Axis, Enable := TRUE, EnableNegative := TRUE);
END_TEST_PROGRAM
