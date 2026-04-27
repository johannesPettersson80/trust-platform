FUNCTION_BLOCK TankFarmClassic
VAR
    Area1Tank1Stopped : BOOL;
    Area1Tank2Stopped : BOOL;
    Area2Tank1Stopped : BOOL;
    Area2Tank2Stopped : BOOL;
    Area1Tank1Alarm : BOOL := TRUE;
    Area1Tank2Alarm : BOOL := FALSE;
    Area2Tank1Alarm : BOOL := FALSE;
    Area2Tank2Alarm : BOOL := TRUE;
END_VAR

METHOD PUBLIC EmergencyStopArea
VAR_INPUT
    AreaIndex : INT;
END_VAR
IF AreaIndex = INT#1 THEN
    Area1Tank1Stopped := TRUE;
    Area1Tank2Stopped := TRUE;
ELSIF AreaIndex = INT#2 THEN
    Area2Tank1Stopped := TRUE;
    Area2Tank2Stopped := TRUE;
END_IF;
END_METHOD

METHOD PUBLIC AcknowledgeAll
Area1Tank1Alarm := FALSE;
Area1Tank2Alarm := FALSE;
Area2Tank1Alarm := FALSE;
Area2Tank2Alarm := FALSE;
END_METHOD

PUBLIC PROPERTY StoppedCount : INT
GET
    StoppedCount := INT#0;
    IF Area1Tank1Stopped THEN StoppedCount := StoppedCount + INT#1; END_IF;
    IF Area1Tank2Stopped THEN StoppedCount := StoppedCount + INT#1; END_IF;
    IF Area2Tank1Stopped THEN StoppedCount := StoppedCount + INT#1; END_IF;
    IF Area2Tank2Stopped THEN StoppedCount := StoppedCount + INT#1; END_IF;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := INT#0;
    IF Area1Tank1Alarm THEN AlarmCount := AlarmCount + INT#1; END_IF;
    IF Area1Tank2Alarm THEN AlarmCount := AlarmCount + INT#1; END_IF;
    IF Area2Tank1Alarm THEN AlarmCount := AlarmCount + INT#1; END_IF;
    IF Area2Tank2Alarm THEN AlarmCount := AlarmCount + INT#1; END_IF;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Farm : TankFarmClassic;
END_VAR
Farm.EmergencyStopArea(AreaIndex := INT#2);
END_PROGRAM
