PROGRAM Main
VAR_EXTERNAL
    TankLevel : REAL;
    PumpRunning : BOOL;
    Setpoint : REAL;
    StatusWord : WORD;
END_VAR

PumpRunning := TankLevel > 40.0;
StatusWord := WORD#16#1234;
END_PROGRAM
