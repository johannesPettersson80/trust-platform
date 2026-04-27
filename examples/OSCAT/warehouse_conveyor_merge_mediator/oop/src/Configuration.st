CONFIGURATION WarehouseConveyorMergeMediatorOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Merge WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Merge.Lane1Request AT %IX0.0 : BOOL;
    Merge.Lane2Request AT %IX0.1 : BOOL;
    Merge.Lane3Request AT %IX0.2 : BOOL;
    Merge.DownstreamReady AT %IX0.3 : BOOL;
    Merge.Lane1GrantOut AT %QX0.0 : BOOL;
    Merge.Lane2GrantOut AT %QX0.1 : BOOL;
    Merge.Lane3GrantOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
