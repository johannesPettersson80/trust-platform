PROGRAM Main
VAR
    Clock : CalendarClock;
    ClockObject : ICalendarClock;
    WateringAllowed : BOOL;
    LocalDay : INT;
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
ClockObject.Update(Utc := DT#2026-07-01-02:00:00);
WateringAllowed := ClockObject.Night;
LocalDay := ClockObject.Day;
END_PROGRAM
