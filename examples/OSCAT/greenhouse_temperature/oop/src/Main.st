(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Converter : UnitConverter;
    VentilationSwitch : HysteresisSwitch;
    Limits : RealRange;
    InvalidLimits : RealRange;
    SwitchStatus : ComponentStatus;
    TemperatureCelsius : REAL := REAL#29.0;
    TemperatureKelvin : REAL;
    VentilationRequest : BOOL;
    ConfigurationRejected : BOOL;
END_VAR

Converter.Initialize();
TemperatureKelvin := Converter.KelvinFromCelsius(Celsius := TemperatureCelsius);

Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
InvalidLimits.Low := REAL#28.0;
InvalidLimits.High := REAL#18.0;
VentilationSwitch.Initialize();
VentilationSwitch.SetLimits(Limits := InvalidLimits);
SwitchStatus := VentilationSwitch.Snapshot();
ConfigurationRejected := SwitchStatus.Error AND SwitchStatus.ErrorId = ComponentErrorInvalidConfiguration;
VentilationSwitch.ClearError();
VentilationSwitch.SetLimits(Limits := Limits);
VentilationRequest := VentilationSwitch.Update(MeasuredValue := TemperatureCelsius);
END_PROGRAM
