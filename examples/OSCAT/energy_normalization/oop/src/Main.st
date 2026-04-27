(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Converter : UnitConverter;
    EnergyJoules : REAL := REAL#3600.0;
    SpeedMetersPerSecond : REAL := REAL#10.0;
    EnergyWattHours : REAL;
    SpeedKilometersPerHour : REAL;
END_VAR

Converter.Initialize();
EnergyWattHours := Converter.WattHoursFromJoules(Joule := EnergyJoules);
SpeedKilometersPerHour := Converter.KilometersPerHourFromMetersPerSecond(Mps := SpeedMetersPerSecond);
END_PROGRAM
