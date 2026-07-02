INTERFACE IComponent
    PROPERTY Ready : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Error : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ErrorId : WORD
    GET
    END_GET
    END_PROPERTY

    PROPERTY Status : BYTE
    GET
    END_GET
    END_PROPERTY

    METHOD Initialize
    END_METHOD

    METHOD Reset
    END_METHOD

    METHOD ClearError
    END_METHOD

    METHOD Snapshot : ComponentStatus
    END_METHOD
END_INTERFACE

INTERFACE IAutomationContext EXTENDS IComponent
    PROPERTY ConstantsLoaded : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Version : DWORD
    GET
    END_GET
    END_PROPERTY

    PROPERTY Math : CONSTANTS_MATH
    GET
    END_GET
    END_PROPERTY

    PROPERTY Pi2 : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Phys : CONSTANTS_PHYS
    GET
    END_GET
    END_PROPERTY

    PROPERTY LanguageValues : CONSTANTS_LANGUAGE
    GET
    END_GET
    END_PROPERTY

    PROPERTY DefaultLanguage : INT
    GET
    END_GET
    END_PROPERTY

    METHOD LoadConstants : BOOL
    END_METHOD

    METHOD SetDefaultLanguage
    VAR_INPUT
        LanguageIndex : INT;
    END_VAR
    END_METHOD

    METHOD DirectionName : STRING[3]
    VAR_INPUT
        Degrees : REAL;
    END_VAR
    END_METHOD

    METHOD DirectionDegrees : INT
    VAR_INPUT
        Name : STRING[3];
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IUnitConverter EXTENDS IComponent
    METHOD KelvinFromCelsius : REAL
    VAR_INPUT
        Celsius : REAL;
    END_VAR
    END_METHOD

    METHOD FahrenheitFromCelsius : REAL
    VAR_INPUT
        Celsius : REAL;
    END_VAR
    END_METHOD

    METHOD CelsiusFromKelvin : REAL
    VAR_INPUT
        Kelvin : REAL;
    END_VAR
    END_METHOD

    METHOD CelsiusFromFahrenheit : REAL
    VAR_INPUT
        Fahrenheit : REAL;
    END_VAR
    END_METHOD

    METHOD MetersPerSecondFromKilometersPerHour : REAL
    VAR_INPUT
        Kmh : REAL;
    END_VAR
    END_METHOD

    METHOD KilometersPerHourFromMetersPerSecond : REAL
    VAR_INPUT
        Mps : REAL;
    END_VAR
    END_METHOD

    METHOD BeaufortFromMetersPerSecond : INT
    VAR_INPUT
        Mps : REAL;
    END_VAR
    END_METHOD

    METHOD AngularFrequencyFromHertz : REAL
    VAR_INPUT
        Frequency : REAL;
    END_VAR
    END_METHOD

    METHOD HertzFromAngularFrequency : REAL
    VAR_INPUT
        AngularFrequency : REAL;
    END_VAR
    END_METHOD

    METHOD PeriodFromHertz : TIME
    VAR_INPUT
        Frequency : REAL;
    END_VAR
    END_METHOD

    METHOD HertzFromPeriod : REAL
    VAR_INPUT
        Period : TIME;
    END_VAR
    END_METHOD

    METHOD WattHoursFromJoules : REAL
    VAR_INPUT
        Joule : REAL;
    END_VAR
    END_METHOD

    METHOD JoulesFromWattHours : REAL
    VAR_INPUT
        WattHour : REAL;
    END_VAR
    END_METHOD

    METHOD JoulesFromCalories : REAL
    VAR_INPUT
        Calorie : REAL;
    END_VAR
    END_METHOD

    METHOD CaloriesFromJoules : REAL
    VAR_INPUT
        Joule : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IPt1Filter EXTENDS IComponent
    PROPERTY Output : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY TimeConstant : TIME
    GET
    END_GET
    END_PROPERTY

    PROPERTY Gain : REAL
    GET
    END_GET
    END_PROPERTY

    METHOD Configure
    VAR_INPUT
        TimeConstant : TIME;
        Gain : REAL;
    END_VAR
    END_METHOD

    METHOD Update : REAL
    VAR_INPUT
        Sample : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IClosedLoopController EXTENDS IComponent
    PROPERTY Actual : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY TargetValue : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Output : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ControlError : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Limited : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD Update : REAL
    VAR_INPUT
        Actual : REAL;
        Target : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IPidController EXTENDS IClosedLoopController
    PROPERTY SupervisionBand : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Manual : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ManualInput : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Offset : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY LowLimit : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HighLimit : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Kp : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Tn : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Tv : REAL
    GET
    END_GET
    END_PROPERTY

    METHOD SetKp
    VAR_INPUT
        Kp : REAL;
    END_VAR
    END_METHOD

    METHOD SetIntegralTime
    VAR_INPUT
        Tn : REAL;
    END_VAR
    END_METHOD

    METHOD SetDerivativeTime
    VAR_INPUT
        Tv : REAL;
    END_VAR
    END_METHOD

    METHOD SetSupervisionBand
    VAR_INPUT
        SupervisionBand : REAL;
    END_VAR
    END_METHOD

    METHOD SetOffset
    VAR_INPUT
        Offset : REAL;
    END_VAR
    END_METHOD

    METHOD SetLimits
    VAR_INPUT
        Limits : RealRange;
    END_VAR
    END_METHOD

    METHOD SetManual
    VAR_INPUT
        Manual : BOOL;
        ManualInput : REAL;
    END_VAR
    END_METHOD

    METHOD ApplyGains
    VAR_INPUT
        Gains : PidGains;
    END_VAR
    END_METHOD

    METHOD Configure
    VAR_INPUT
        Gains : PidGains;
        Limits : RealRange;
        SupervisionBand : REAL;
        Offset : REAL;
    END_VAR
    END_METHOD

END_INTERFACE

INTERFACE IHysteresisSwitch EXTENDS IComponent
    PROPERTY MeasuredValue : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY LowLimit : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HighLimit : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Q : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Window : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD SetLimits
    VAR_INPUT
        Limits : RealRange;
    END_VAR
    END_METHOD

    METHOD Update : BOOL
    VAR_INPUT
        MeasuredValue : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE IPulseGenerator EXTENDS IComponent
    PROPERTY Enabled : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Output : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HighTime : TIME
    GET
    END_GET
    END_PROPERTY

    PROPERTY LowTime : TIME
    GET
    END_GET
    END_PROPERTY

    METHOD Configure
    VAR_INPUT
        HighTime : TIME;
        LowTime : TIME;
    END_VAR
    END_METHOD

    METHOD SetEnabled
    VAR_INPUT
        Enabled : BOOL;
    END_VAR
    END_METHOD

    METHOD Update : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IDwordQueue EXTENDS IComponent
    PROPERTY Empty : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Full : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Value : DWORD
    GET
    END_GET
    END_PROPERTY

    PROPERTY Capacity : UINT
    GET
    END_GET
    END_PROPERTY

    METHOD Push : BOOL
    VAR_INPUT
        Value : DWORD;
    END_VAR
    END_METHOD

    METHOD TryPop : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IDwordStack EXTENDS IComponent
    PROPERTY Empty : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Full : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Value : DWORD
    GET
    END_GET
    END_PROPERTY

    PROPERTY Capacity : UINT
    GET
    END_GET
    END_PROPERTY

    METHOD Push : BOOL
    VAR_INPUT
        Value : DWORD;
    END_VAR
    END_METHOD

    METHOD TryPop : BOOL
    END_METHOD
END_INTERFACE

INTERFACE ICalendarClock EXTENDS IComponent
    PROPERTY Utc : DT
    GET
    END_GET
    END_PROPERTY

    PROPERTY LocalDateTime : DT
    GET
    END_GET
    END_PROPERTY

    PROPERTY LocalDate : DATE
    GET
    END_GET
    END_PROPERTY

    PROPERTY LocalTimeOfDay : TOD
    GET
    END_GET
    END_PROPERTY

    PROPERTY Year : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Month : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Day : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Weekday : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY OffsetMinutes : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY DstEnabled : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY DstOn : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY LanguageIndex : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY Longitude : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Latitude : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Sunrise : TOD
    GET
    END_GET
    END_PROPERTY

    PROPERTY Sunset : TOD
    GET
    END_GET
    END_PROPERTY

    PROPERTY SolarNoon : TOD
    GET
    END_GET
    END_PROPERTY

    PROPERTY SolarElevation : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY SolarHorizontalProjection : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY SolarVerticalProjection : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Night : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Holiday : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HolidayName : STRING[30]
    GET
    END_GET
    END_PROPERTY

    PROPERTY WorkWeek : INT
    GET
    END_GET
    END_PROPERTY

    METHOD Configure
    VAR_INPUT
        LocationLatitude : REAL;
        LocationLongitude : REAL;
        OffsetMinutes : INT;
        DstEnabled : BOOL;
        LanguageIndex : INT;
    END_VAR
    END_METHOD

    METHOD Update
    VAR_INPUT
        Utc : DT;
    END_VAR
    END_METHOD

    METHOD CalculateSunPosition : SunPosition
    VAR_INPUT
        Utc : DT;
        Latitude : REAL;
        Longitude : REAL;
    END_VAR
    END_METHOD

    METHOD CalculateSunTime : SunTimes
    VAR_INPUT
        UtcDate : DATE;
        Latitude : REAL;
        Longitude : REAL;
        Horizon : REAL;
    END_VAR
    END_METHOD
END_INTERFACE
