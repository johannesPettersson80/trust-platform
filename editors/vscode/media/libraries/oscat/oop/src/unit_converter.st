FUNCTION_BLOCK UnitConverter IMPLEMENTS IUnitConverter
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    EnergyFb : ENERGY;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := ComponentErrorNone;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC KelvinFromCelsius : REAL
VAR_INPUT
    Celsius : REAL;
END_VAR
OSCAT_BASIC_Constants();
KelvinFromCelsius := C_TO_K(Celsius := Celsius);
END_METHOD

METHOD PUBLIC FahrenheitFromCelsius : REAL
VAR_INPUT
    Celsius : REAL;
END_VAR
FahrenheitFromCelsius := C_TO_F(celsius := Celsius);
END_METHOD

METHOD PUBLIC CelsiusFromKelvin : REAL
VAR_INPUT
    Kelvin : REAL;
END_VAR
OSCAT_BASIC_Constants();
CelsiusFromKelvin := K_TO_C(Kelvin := Kelvin);
END_METHOD

METHOD PUBLIC CelsiusFromFahrenheit : REAL
VAR_INPUT
    Fahrenheit : REAL;
END_VAR
OSCAT_BASIC_Constants();
CelsiusFromFahrenheit := F_TO_C(fahrenheit := Fahrenheit);
END_METHOD

METHOD PUBLIC MetersPerSecondFromKilometersPerHour : REAL
VAR_INPUT
    Kmh : REAL;
END_VAR
MetersPerSecondFromKilometersPerHour := KMH_TO_MS(kmh := Kmh);
END_METHOD

METHOD PUBLIC KilometersPerHourFromMetersPerSecond : REAL
VAR_INPUT
    Mps : REAL;
END_VAR
KilometersPerHourFromMetersPerSecond := MS_TO_KMH(ms := Mps);
END_METHOD

METHOD PUBLIC BeaufortFromMetersPerSecond : INT
VAR_INPUT
    Mps : REAL;
END_VAR
BeaufortFromMetersPerSecond := MS_TO_BFT(MS := Mps);
END_METHOD

METHOD PUBLIC AngularFrequencyFromHertz : REAL
VAR_INPUT
    Frequency : REAL;
END_VAR
OSCAT_BASIC_Constants();
AngularFrequencyFromHertz := F_TO_OM(F := Frequency);
END_METHOD

METHOD PUBLIC HertzFromAngularFrequency : REAL
VAR_INPUT
    AngularFrequency : REAL;
END_VAR
OSCAT_BASIC_Constants();
HertzFromAngularFrequency := OM_TO_F(OM := AngularFrequency);
END_METHOD

METHOD PUBLIC PeriodFromHertz : TIME
VAR_INPUT
    Frequency : REAL;
END_VAR
PeriodFromHertz := F_TO_PT(F := Frequency);
END_METHOD

METHOD PUBLIC HertzFromPeriod : REAL
VAR_INPUT
    Period : TIME;
END_VAR
HertzFromPeriod := PT_TO_F(PT := Period);
END_METHOD

METHOD PUBLIC WattHoursFromJoules : REAL
VAR_INPUT
    Joule : REAL;
END_VAR
EnergyFb(J := Joule, C := REAL#0.0, Wh := REAL#0.0);
WattHoursFromJoules := EnergyFb.YWh;
END_METHOD

METHOD PUBLIC JoulesFromWattHours : REAL
VAR_INPUT
    WattHour : REAL;
END_VAR
EnergyFb(J := REAL#0.0, C := REAL#0.0, Wh := WattHour);
JoulesFromWattHours := EnergyFb.YJ;
END_METHOD

METHOD PUBLIC JoulesFromCalories : REAL
VAR_INPUT
    Calorie : REAL;
END_VAR
EnergyFb(J := REAL#0.0, C := Calorie, Wh := REAL#0.0);
JoulesFromCalories := EnergyFb.YJ;
END_METHOD

METHOD PUBLIC CaloriesFromJoules : REAL
VAR_INPUT
    Joule : REAL;
END_VAR
EnergyFb(J := Joule, C := REAL#0.0, Wh := REAL#0.0);
CaloriesFromJoules := EnergyFb.YC;
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
