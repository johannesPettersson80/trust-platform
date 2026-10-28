CONFIGURATION Wastewater
RESOURCE Cpu ON PLC
    TASK Control (INTERVAL := T#200ms, PRIORITY := 1);
    PROGRAM Station WITH Control : Main;
END_RESOURCE
VAR_CONFIG
    Station.LevelSignal AT %IL0 : LREAL;
    Station.FlowSignal AT %IL1 : LREAL;
    Station.Pump1Running AT %IX2.0 : BOOL;
    Station.Pump2Running AT %IX2.1 : BOOL;
    Station.Pump3Running AT %IX2.2 : BOOL;
    Station.Pump1Trip AT %IX2.3 : BOOL;
    Station.Pump2Trip AT %IX2.4 : BOOL;
    Station.Pump3Trip AT %IX2.5 : BOOL;
    Station.Pump1Run AT %QX0.0 : BOOL;
    Station.Pump2Run AT %QX0.1 : BOOL;
    Station.Pump3Run AT %QX0.2 : BOOL;
    Station.Pump1Speed AT %QL1 : LREAL;
    Station.Pump2Speed AT %QL2 : LREAL;
    Station.Pump3Speed AT %QL3 : LREAL;
END_VAR
END_CONFIGURATION
