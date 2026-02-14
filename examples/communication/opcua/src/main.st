PROGRAM Main
VAR_EXTERNAL
    TankLevel : REAL;
    PumpRunning : BOOL;
END_VAR

PumpRunning := TankLevel > 50.0;
END_PROGRAM
