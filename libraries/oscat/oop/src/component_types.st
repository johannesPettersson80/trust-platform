TYPE ComponentStatus :
STRUCT
    Ready : BOOL;
    Error : BOOL;
    ErrorId : WORD;
    Status : BYTE;
END_STRUCT
END_TYPE

TYPE RealRange :
STRUCT
    Low : REAL;
    High : REAL;
END_STRUCT
END_TYPE

TYPE PidGains :
STRUCT
    Kp : REAL := 1.0;
    Tn : REAL := 1.0;
    Tv : REAL := 1.0;
END_STRUCT
END_TYPE

TYPE PiGains :
STRUCT
    Kp : REAL := 1.0;
    Ki : REAL := 1.0;
END_STRUCT
END_TYPE

TYPE SunPosition :
STRUCT
    Azimuth : REAL;
    Elevation : REAL;
    RefractedElevation : REAL;
END_STRUCT
END_TYPE

TYPE SunTimes :
STRUCT
    SolarNoon : TOD;
    Sunrise : TOD;
    Sunset : TOD;
    Declination : REAL;
END_STRUCT
END_TYPE

TYPE BarGraphSnapshot :
STRUCT
    Low : BOOL;
    Q1 : BOOL;
    Q2 : BOOL;
    Q3 : BOOL;
    Q4 : BOOL;
    Q5 : BOOL;
    Q6 : BOOL;
    High : BOOL;
    Alarm : BOOL;
    Status : BYTE;
END_STRUCT
END_TYPE

VAR_GLOBAL CONSTANT
    ComponentErrorNone : WORD := WORD#16#0000;
    ComponentErrorInvalidConfiguration : WORD := WORD#16#1000;
    ComponentErrorNotReady : WORD := WORD#16#1001;
    ComponentErrorQueueFull : WORD := WORD#16#1002;
    ComponentErrorQueueEmpty : WORD := WORD#16#1003;

    DefaultPidKp : REAL := REAL#1.0;
    DefaultPidTn : REAL := REAL#1.0;
    DefaultPidTv : REAL := REAL#1.0;
    DefaultPidLowLimit : REAL := REAL#-1000.0;
    DefaultPidHighLimit : REAL := REAL#1000.0;
    DefaultPidSupervisionBand : REAL := REAL#0.0;
    DefaultPidOffset : REAL := REAL#0.0;

    DefaultPt1Gain : REAL := REAL#1.0;
    DefaultPt1TimeConstant : TIME := T#1s;
    DefaultPulseHighTime : TIME := T#1s;
    DefaultPulseLowTime : TIME := T#1s;
    DefaultCalendarOffsetMinutes : INT := INT#0;
    MaxDwordFifo16Capacity : UINT := UINT#16;
    MaxDwordStack16Capacity : UINT := UINT#16;
    MaxDwordFifo32Capacity : UINT := UINT#32;
    MaxDwordStack32Capacity : UINT := UINT#32;
END_VAR
