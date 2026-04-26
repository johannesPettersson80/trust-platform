PROGRAM Main
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
    IntakeObject : IPt1Filter;
    ExhaustObject : IPt1Filter;
    RawAirflow : REAL := REAL#10.0;
    IntakeAirflow : REAL;
    ExhaustAirflow : REAL;
END_VAR

IntakeObject := IntakeFilter;
ExhaustObject := ExhaustFilter;
IntakeObject.Initialize();
ExhaustObject.Initialize();
IntakeObject.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustObject.Configure(TimeConstant := T#1s, Gain := REAL#2.0);
IntakeAirflow := IntakeObject.Update(Sample := RawAirflow);
ExhaustAirflow := ExhaustObject.Update(Sample := RawAirflow);
END_PROGRAM
