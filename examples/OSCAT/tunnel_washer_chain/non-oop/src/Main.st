FUNCTION_BLOCK ClassicWasherQuality
VAR
    LastHandlerIdValue : INT;
    RejectCounter : DwordCounter;
END_VAR

METHOD PUBLIC Initialize
RejectCounter.Initialize();
RejectCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC Evaluate : INT
VAR_INPUT
    SoilLevel : REAL;
    Conductivity : REAL;
    RinseConductivity : REAL;
    OperatorAvailable : BOOL;
END_VAR
IF (SoilLevel > REAL#70.0) AND (Conductivity < REAL#3.0) THEN
    LastHandlerIdValue := INT#1;
ELSIF RinseConductivity > REAL#1.5 THEN
    LastHandlerIdValue := INT#2;
ELSIF OperatorAvailable THEN
    LastHandlerIdValue := INT#3;
ELSE
    LastHandlerIdValue := INT#4;
    RejectCounter.Update(Up := TRUE, Down := FALSE);
END_IF;
Evaluate := LastHandlerIdValue;
END_METHOD

PUBLIC PROPERTY RejectCount : DWORD
GET
    RejectCount := RejectCounter.Count;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Washer : ClassicWasherQuality;
    Handler : INT;
END_VAR
Washer.Initialize();
Handler := Washer.Evaluate(SoilLevel := REAL#80.0, Conductivity := REAL#2.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE);
END_PROGRAM
