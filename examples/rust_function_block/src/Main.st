PROGRAM Main
VAR
    Setpoint AT %IL0 : LREAL;   (* the speed to reach, written in Live Values *)
    SpeedRamp : RAMP;
    Speed AT %QL0 : LREAL;      (* the ramped speed, a drive's speed reference *)
    AtSpeed AT %QX8.0 : BOOL;   (* TRUE once Speed has reached Setpoint *)
END_VAR
SpeedRamp(target := Setpoint, rate := 10.0);
Speed := SpeedRamp.out;
AtSpeed := SpeedRamp.done;
END_PROGRAM
