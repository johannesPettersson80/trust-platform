CONFIGURATION Packaging
RESOURCE Cpu ON PLC
    TASK Line (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Machine WITH Line : Main;
END_RESOURCE
VAR_CONFIG
    Machine.StartButton AT %IX0.0 : BOOL;
    Machine.StopButton AT %IX0.1 : BOOL;
    Machine.ProductSensor AT %IX0.2 : BOOL;
    Machine.InspectionFailed AT %IX0.3 : BOOL;
    Machine.ConveyorRunning AT %IX0.4 : BOOL;
    Machine.ConveyorTrip AT %IX0.5 : BOOL;
    Machine.CylinderExtendedSensor AT %IX0.6 : BOOL;
    Machine.CylinderRetractedSensor AT %IX0.7 : BOOL;
    Machine.ConveyorRun AT %QX0.0 : BOOL;
    Machine.RejectValve AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
