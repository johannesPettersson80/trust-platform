FUNCTION_BLOCK ClassicCommissioningBooster
VAR
    SineGen : SineSignalGenerator;
    LastCommandedSpeedValue : REAL;
    RealPumpRunningValue : BOOL;
    PressureValue : REAL;
END_VAR

METHOD PUBLIC Initialize
SineGen.Initialize();
SineGen.Configure(Period := T#10s, Amplitude := REAL#0.5, Offset := REAL#4.5, Delay := REAL#0.0);
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    RealPressureBar : REAL;
    CommissioningMode : BOOL;
    LoopbackOnly : BOOL;
    PressureSetpoint : REAL;
END_VAR
IF CommissioningMode THEN
    PressureValue := SineGen.Update();
ELSE
    PressureValue := RealPressureBar;
END_IF;

IF PressureValue < PressureSetpoint THEN
    LastCommandedSpeedValue := REAL#50.0;
    IF NOT LoopbackOnly THEN
        RealPumpRunningValue := TRUE;
    END_IF;
ELSE
    LastCommandedSpeedValue := REAL#0.0;
    IF NOT LoopbackOnly THEN
        RealPumpRunningValue := FALSE;
    END_IF;
END_IF;
Update := LastCommandedSpeedValue;
END_METHOD

PUBLIC PROPERTY LastCommandedSpeed : REAL
GET
    LastCommandedSpeed := LastCommandedSpeedValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RealPumpRunning : BOOL
GET
    RealPumpRunning := RealPumpRunningValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Cell : ClassicCommissioningBooster;
    Speed : REAL;
END_VAR
Cell.Initialize();
Speed := Cell.Update(RealPressureBar := REAL#9.0, CommissioningMode := TRUE, LoopbackOnly := TRUE, PressureSetpoint := REAL#4.5);
END_PROGRAM
