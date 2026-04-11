TEST_PROGRAM plcopen_motion_single_axis_negative_stop_active
VAR
    Axis : AXIS_REF;
    Stop : MC_Stop;
END_VAR

Stop(Axis := Axis, Execute := TRUE, Deceleration := REAL#1.0, Jerk := REAL#1.0);
ASSERT_FALSE(Stop.Active);
END_TEST_PROGRAM
