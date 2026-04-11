TEST_PROGRAM plcopen_motion_single_axis_negative_transition_vel_next
VAR
    WrongBufferMode : MC_BUFFER_MODE;
END_VAR

WrongBufferMode := MC_BUFFER_MODE#mcTransitionVelNext;
END_TEST_PROGRAM
