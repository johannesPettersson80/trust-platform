PROGRAM Main
VAR_EXTERNAL
    TankLevel : REAL;
    PumpRunning : BOOL;
    StatusWord : WORD;
END_VAR

// The program keeps TankLevel and PumpRunning moving. It leaves Setpoint, the writable
// symbol, to the ADS client, which reads back what it wrote.
TankLevel := TankLevel + 1.0;
IF TankLevel > 80.0 THEN
    TankLevel := 20.0;
END_IF;

PumpRunning := TankLevel > 40.0;
StatusWord := WORD#16#1234;
END_PROGRAM
