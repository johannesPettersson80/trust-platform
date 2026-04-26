PROGRAM Main
VAR
    Context : AutomationContext;
    ContextObject : IAutomationContext;
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    WindMetersPerSecond : REAL := REAL#10.0;
    BeaufortIndex : INT;
    WindDirection : STRING[3];
END_VAR

ContextObject := Context;
ContextObject.Initialize();
ContextObject.LoadConstants();
ConverterObject := Converter;
ConverterObject.Initialize();
BeaufortIndex := ConverterObject.BeaufortFromMetersPerSecond(Mps := WindMetersPerSecond);
WindDirection := ContextObject.DirectionName(Degrees := REAL#90.0);
END_PROGRAM
