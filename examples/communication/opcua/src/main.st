PROGRAM Main
VAR_EXTERNAL
    TankLevel : REAL;
    PumpRunning : BOOL;
END_VAR

TankLevel := TankLevel + 1.0;
IF TankLevel > 80.0 THEN
    TankLevel := 20.0;
END_IF;

PumpRunning := TankLevel > 50.0;
END_PROGRAM
