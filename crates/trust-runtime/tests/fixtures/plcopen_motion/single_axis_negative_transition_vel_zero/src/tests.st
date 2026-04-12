TEST_PROGRAM plcopen_motion_single_axis_negative_transition_vel_zero
VAR
    WrongBufferMode : MC_BUFFER_MODE;
END_VAR

WrongBufferMode := MC_BUFFER_MODE#mcTransitionVelZero;
END_TEST_PROGRAM
