PROGRAM Main
VAR
    Axis : AXIS_REF;
    Stop : MC_Stop;
END_VAR

Stop(Axis := Axis, Execute := TRUE, Deceleration := REAL#1.0, Jerk := REAL#1.0);
IF Stop.Active THEN
END_IF;
END_PROGRAM
