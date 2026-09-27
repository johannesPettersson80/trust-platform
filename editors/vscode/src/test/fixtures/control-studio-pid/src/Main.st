PROGRAM Main
VAR
    Kp : LREAL := LREAL#1.0;
    Ki : LREAL := LREAL#0.0;
    Kd : LREAL := LREAL#0.0;
    Integral : LREAL := LREAL#0.0;
    PreviousError : LREAL := LREAL#0.0;
    Manual : BOOL := TRUE;
    ManualCommand AT %ML8 : LREAL := LREAL#0.0;
    SP AT %ML0 : LREAL := LREAL#0.0;
    PV AT %IL0 : LREAL;
    MV AT %QL0 : LREAL;
END_VAR

IF Manual THEN
    MV := ManualCommand;
ELSE
    Integral := Integral + Ki * (SP - PV) * LREAL#0.02;
    MV := LIMIT(LREAL#-20.0, Kp * (SP - PV) + Integral + Kd * ((SP - PV) - PreviousError) / LREAL#0.02, LREAL#20.0);
    PreviousError := SP - PV;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
