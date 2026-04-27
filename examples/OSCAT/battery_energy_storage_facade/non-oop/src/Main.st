FUNCTION_BLOCK ClassicBessCabinet
VAR
    EnabledValue : BOOL;
    AlarmCountValue : INT;
    MqttPublishReadyValue : BOOL;
    PowerCommandValue : REAL;
    FireIsolatedValue : BOOL;
END_VAR
METHOD PUBLIC Enable
EnabledValue := TRUE;
END_METHOD
METHOD PUBLIC Update : REAL
VAR_INPUT
    StateOfChargePct : REAL;
    GridPowerCommandKw : REAL;
    FireInputHealthy : BOOL;
END_VAR
IF NOT FireInputHealthy THEN
    EnabledValue := FALSE;
    FireIsolatedValue := TRUE;
    PowerCommandValue := REAL#0.0;
    AlarmCountValue := AlarmCountValue + INT#1;
    MqttPublishReadyValue := TRUE;
ELSIF EnabledValue THEN
    FireIsolatedValue := FALSE;
    PowerCommandValue := LIMIT(REAL#-250.0, GridPowerCommandKw, REAL#250.0);
END_IF;
Update := PowerCommandValue;
END_METHOD
PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := AlarmCountValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY FireIsolated : BOOL
GET
    FireIsolated := FireIsolatedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Cabinet : ClassicBessCabinet;
    Power : REAL;
END_VAR
Cabinet.Enable();
Power := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#120.0, FireInputHealthy := TRUE);
END_PROGRAM
