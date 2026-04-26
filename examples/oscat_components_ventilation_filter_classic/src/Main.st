PROGRAM Main
VAR
    IntakeFilter : FT_PT1;
    ExhaustFilter : FT_PT1;
    RawAirflow : REAL := REAL#10.0;
    IntakeAirflow : REAL;
    ExhaustAirflow : REAL;
END_VAR

IntakeFilter(in := RawAirflow, T := T#1s, K := REAL#1.0);
ExhaustFilter(in := RawAirflow, T := T#1s, K := REAL#2.0);
IntakeAirflow := IntakeFilter.out;
ExhaustAirflow := ExhaustFilter.out;
END_PROGRAM
