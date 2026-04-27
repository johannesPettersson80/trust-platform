(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Context : AutomationContext;
    Converter : UnitConverter;
    WindMetersPerSecond : REAL := REAL#10.0;
    BeaufortIndex : INT;
    WindDirection : STRING[3];
END_VAR

Context.Initialize();
Context.LoadConstants();
Converter.Initialize();
BeaufortIndex := Converter.BeaufortFromMetersPerSecond(Mps := WindMetersPerSecond);
WindDirection := Context.DirectionName(Degrees := REAL#90.0);
END_PROGRAM
