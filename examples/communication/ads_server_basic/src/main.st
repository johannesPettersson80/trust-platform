PROGRAM Main
VAR_EXTERNAL
    TankLevel : REAL;
    PumpRunning : BOOL;
    Setpoint : REAL;
    StatusWord : WORD;
END_VAR

TankLevel := TankLevel + 1.0;
IF TankLevel > 80.0 THEN
    TankLevel := 20.0;
END_IF;

Setpoint := Setpoint + 0.25;
IF Setpoint > 20.0 THEN
    Setpoint := 10.0;
END_IF;

PumpRunning := TankLevel > 40.0;
StatusWord := WORD#16#1234;
END_PROGRAM
