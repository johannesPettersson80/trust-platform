PROGRAM Main
VAR
    Setpoint : LREAL := 50.0;
    SpeedRamp : RAMP;
    Speed : LREAL;
    AtSpeed : BOOL;
END_VAR
SpeedRamp(target := Setpoint, rate := 10.0);
Speed := SpeedRamp.out;
AtSpeed := SpeedRamp.done;
END_PROGRAM
