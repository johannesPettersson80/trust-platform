FUNCTION_BLOCK ClassicBackwashController
METHOD PUBLIC Update : INT
VAR_INPUT
    FilterType : INT;
    ElapsedSeconds : INT;
    TurbidityNtu : REAL;
    FlowOk : BOOL;
END_VAR
VAR
    RequiredSeconds : INT;
    TurbidityLimit : REAL;
END_VAR
IF FilterType = INT#2 THEN
    RequiredSeconds := INT#420;
    TurbidityLimit := REAL#1.0;
ELSE
    RequiredSeconds := INT#300;
    TurbidityLimit := REAL#2.0;
END_IF;
IF NOT FlowOk THEN
    Update := INT#90;
ELSIF ElapsedSeconds < INT#60 THEN
    Update := INT#10;
ELSIF ElapsedSeconds < RequiredSeconds THEN
    Update := INT#20;
ELSIF TurbidityNtu > TurbidityLimit THEN
    Update := INT#30;
ELSE
    Update := INT#100;
END_IF;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Backwash : ClassicBackwashController;
    StepId : INT;
END_VAR
StepId := Backwash.Update(FilterType := INT#1, ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE);
END_PROGRAM
