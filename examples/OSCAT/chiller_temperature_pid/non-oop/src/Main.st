PROGRAM Main
VAR
    TemperatureConversion : TEMPERATURE;
    ChillerController : CTRL_PID;
    ReturnTemperatureCelsius : REAL := REAL#12.0;
    ReturnTemperatureKelvin : REAL;
    ValveCommandPercent : REAL;
END_VAR

TemperatureConversion(C := ReturnTemperatureCelsius);
ReturnTemperatureKelvin := TemperatureConversion.YK;
ChillerController(
    ACT := ReturnTemperatureCelsius,
    SET_POINT := REAL#7.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#45.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
ValveCommandPercent := ChillerController.Y;
END_PROGRAM
