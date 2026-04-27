(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Converter : UnitConverter;
    ChillerController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    ReturnTemperatureCelsius : REAL := REAL#12.0;
    ReturnTemperatureKelvin : REAL;
    ValveCommandPercent : REAL;
END_VAR

Converter.Initialize();
ReturnTemperatureKelvin := Converter.KelvinFromCelsius(Celsius := ReturnTemperatureCelsius);

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
ChillerController.Initialize();
ChillerController.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
ChillerController.SetManual(Manual := TRUE, ManualInput := REAL#45.0);
ValveCommandPercent := ChillerController.Update(Actual := ReturnTemperatureCelsius, Target := REAL#7.0);
END_PROGRAM
