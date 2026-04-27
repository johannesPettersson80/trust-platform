(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Clock : CalendarClock;
    WateringAllowed : BOOL;
    LocalDay : INT;
END_VAR

Clock.Initialize();
Clock.Configure(
    LocationLatitude := REAL#59.3293,
    LocationLongitude := REAL#18.0686,
    OffsetMinutes := INT#60,
    DstEnabled := TRUE,
    LanguageIndex := INT#1
);
Clock.Update(Utc := DT#2026-07-01-02:00:00);
WateringAllowed := Clock.Night;
LocalDay := Clock.Day;
END_PROGRAM
