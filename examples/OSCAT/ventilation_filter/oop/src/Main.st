(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
    RawAirflow : REAL := REAL#10.0;
    IntakeAirflow : REAL;
    ExhaustAirflow : REAL;
END_VAR

IntakeFilter.Initialize();
ExhaustFilter.Initialize();
IntakeFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustFilter.Configure(TimeConstant := T#1s, Gain := REAL#2.0);
IntakeAirflow := IntakeFilter.Update(Sample := RawAirflow);
ExhaustAirflow := ExhaustFilter.Update(Sample := RawAirflow);
END_PROGRAM
