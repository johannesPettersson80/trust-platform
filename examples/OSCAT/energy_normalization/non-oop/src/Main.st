PROGRAM Main
VAR
    EnergyConversion : ENERGY;
    SpeedConversion : SPEED;
    EnergyJoules : REAL := REAL#3600.0;
    SpeedMetersPerSecond : REAL := REAL#10.0;
    EnergyWattHours : REAL;
    SpeedKilometersPerHour : REAL;
END_VAR

EnergyConversion(J := EnergyJoules);
SpeedConversion(ms := SpeedMetersPerSecond);
EnergyWattHours := EnergyConversion.YWh;
SpeedKilometersPerHour := SpeedConversion.Ykmh;
END_PROGRAM
