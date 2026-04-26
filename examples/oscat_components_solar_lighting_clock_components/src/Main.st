PROGRAM Main
VAR
    Clock : CalendarClock;
    ClockObject : ICalendarClock;
    ExteriorLightingRequest : BOOL;
    LocalYear : INT;
END_VAR

ClockObject := Clock;
ClockObject.Initialize();
ClockObject.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    Language := INT#1
);
ClockObject.Update(Utc := DT#2026-04-26-10:00:00);
ExteriorLightingRequest := ClockObject.Night;
LocalYear := ClockObject.Year;
END_PROGRAM
