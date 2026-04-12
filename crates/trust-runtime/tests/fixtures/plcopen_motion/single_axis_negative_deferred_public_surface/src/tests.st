TEST_PROGRAM plcopen_motion_single_axis_deferred_public_fbs_are_absent
VAR
    MoveSuperimposed : MC_MoveSuperimposed;
    HaltSuperimposed : MC_HaltSuperimposed;
    TorqueControl : MC_TorqueControl;
    PositionProfile : MC_PositionProfile;
    VelocityProfile : MC_VelocityProfile;
    AccelerationProfile : MC_AccelerationProfile;
    ReadDigitalInput : MC_ReadDigitalInput;
    ReadDigitalOutput : MC_ReadDigitalOutput;
    WriteDigitalOutput : MC_WriteDigitalOutput;
    DigitalCamSwitch : MC_DigitalCamSwitch;
    TouchProbe : MC_TouchProbe;
    AbortTrigger : MC_AbortTrigger;
END_VAR
END_TEST_PROGRAM
