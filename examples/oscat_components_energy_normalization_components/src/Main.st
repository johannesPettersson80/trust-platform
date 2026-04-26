PROGRAM Main
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    EnergyJoules : REAL := REAL#3600.0;
    SpeedMetersPerSecond : REAL := REAL#10.0;
    EnergyWattHours : REAL;
    SpeedKilometersPerHour : REAL;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
EnergyWattHours := ConverterObject.WattHoursFromJoules(Joule := EnergyJoules);
SpeedKilometersPerHour := ConverterObject.KilometersPerHourFromMetersPerSecond(Mps := SpeedMetersPerSecond);
END_PROGRAM
