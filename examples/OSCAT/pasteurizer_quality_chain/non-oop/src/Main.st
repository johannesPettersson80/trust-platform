TYPE ClassicPasteurizerResult :
STRUCT
    ForwardDivert : BOOL;
    DivertToRecirc : BOOL;
    RequiredTemperature : REAL;
    RequiredHoldSeconds : INT;
    LastHandlerId : INT;
    BatchAborted : BOOL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK ClassicPasteurizer
VAR
    ResultValue : ClassicPasteurizerResult;
END_VAR

METHOD PUBLIC Update : ClassicPasteurizerResult
VAR_INPUT
    RecipeId : INT;
    ProductTemp : REAL;
    FlowRate : REAL;
    HoldSeconds : INT;
    OperatorAck : BOOL;
    SupervisorOnline : BOOL;
END_VAR
IF RecipeId = INT#2 THEN
    ResultValue.RequiredTemperature := REAL#75.0;
    ResultValue.RequiredHoldSeconds := INT#20;
ELSE
    ResultValue.RequiredTemperature := REAL#72.0;
    ResultValue.RequiredHoldSeconds := INT#15;
END_IF;

ResultValue.ForwardDivert := (ProductTemp >= ResultValue.RequiredTemperature) AND (FlowRate >= REAL#100.0) AND (HoldSeconds >= ResultValue.RequiredHoldSeconds);
ResultValue.DivertToRecirc := NOT ResultValue.ForwardDivert;
ResultValue.BatchAborted := FALSE;
ResultValue.LastHandlerId := INT#0;

IF ResultValue.DivertToRecirc THEN
    IF (ProductTemp >= (ResultValue.RequiredTemperature - REAL#1.0)) AND (FlowRate >= REAL#100.0) THEN
        ResultValue.LastHandlerId := INT#1;
    ELSIF OperatorAck THEN
        ResultValue.LastHandlerId := INT#2;
    ELSIF SupervisorOnline THEN
        ResultValue.LastHandlerId := INT#3;
    ELSE
        ResultValue.LastHandlerId := INT#4;
        ResultValue.BatchAborted := TRUE;
    END_IF;
END_IF;
Update := ResultValue;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Pasteurizer : ClassicPasteurizer;
    Result : ClassicPasteurizerResult;
END_VAR
Result := Pasteurizer.Update(RecipeId := INT#1, ProductTemp := REAL#73.0, FlowRate := REAL#120.0, HoldSeconds := INT#15, OperatorAck := FALSE, SupervisorOnline := FALSE);
END_PROGRAM
