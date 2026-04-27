FUNCTION_BLOCK ClassicCraneController
VAR
    StateValue : INT;
    FaultCodeValue : DWORD;
    HoistRunningValue : BOOL;
END_VAR

METHOD PUBLIC Update : INT
VAR_INPUT
    HoistStatus : WORD;
    TravelStatus : WORD;
    LiftCommand : BOOL;
    TravelCommand : BOOL;
    LowerCommand : BOOL;
    UpperLimit : BOOL;
    Overload : BOOL;
END_VAR
IF ((WORD_TO_DWORD(HoistStatus) AND DWORD#16#0008) <> DWORD#0) OR Overload THEN
    StateValue := INT#90;
    FaultCodeValue := DWORD#16#A0000044;
ELSIF StateValue = INT#0 THEN
    IF LiftCommand AND NOT UpperLimit THEN
        StateValue := INT#10;
        HoistRunningValue := TRUE;
    ELSIF TravelCommand THEN
        StateValue := INT#20;
    ELSIF LowerCommand THEN
        StateValue := INT#30;
    END_IF;
ELSIF StateValue = INT#10 AND NOT LiftCommand THEN
    StateValue := INT#0;
    HoistRunningValue := FALSE;
END_IF;
Update := StateValue;
END_METHOD

PUBLIC PROPERTY NormalizedFaultCode : DWORD
GET
    NormalizedFaultCode := FaultCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Crane : ClassicCraneController;
    StateId : INT;
END_VAR
StateId := Crane.Update(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400, LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE);
END_PROGRAM
