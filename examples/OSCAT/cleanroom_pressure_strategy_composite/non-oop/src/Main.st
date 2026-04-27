FUNCTION_BLOCK ClassicCleanroomCascade
VAR
    ActiveModeIdValue : INT;
    AlarmCountValue : INT;
END_VAR
METHOD PUBLIC Update : REAL
VAR_INPUT
    ModeRequest : INT;
    Room1Pressure : REAL;
    Room2Pressure : REAL;
    DoorOpen : BOOL;
END_VAR
VAR
    Target : REAL;
    ErrorSum : REAL;
END_VAR
IF ModeRequest = INT#3 THEN
    ActiveModeIdValue := INT#3;
    Target := REAL#0.0;
ELSIF ModeRequest = INT#2 THEN
    ActiveModeIdValue := INT#2;
    Target := REAL#20.0;
ELSE
    ActiveModeIdValue := INT#1;
    Target := REAL#15.0;
END_IF;
AlarmCountValue := INT#0;
IF DoorOpen OR (ABS(Target - Room1Pressure) > REAL#10.0) THEN AlarmCountValue := AlarmCountValue + INT#1; END_IF;
IF ABS((Target - REAL#5.0) - Room2Pressure) > REAL#10.0 THEN AlarmCountValue := AlarmCountValue + INT#1; END_IF;
ErrorSum := (Target - Room1Pressure) + ((Target - REAL#5.0) - Room2Pressure);
Update := LIMIT(REAL#0.0, ErrorSum * REAL#2.0, REAL#100.0);
END_METHOD
PUBLIC PROPERTY ActiveModeId : INT
GET
    ActiveModeId := ActiveModeIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := AlarmCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Cascade : ClassicCleanroomCascade;
    Command : REAL;
END_VAR
Command := Cascade.Update(ModeRequest := INT#1, Room1Pressure := REAL#10.0, Room2Pressure := REAL#5.0, DoorOpen := FALSE);
END_PROGRAM
