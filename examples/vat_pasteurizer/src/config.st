(* The vat pasteurizer on one controller: the batch control every 10 ms, the simulated plant every
   20 ms and the panel's texts every 500 ms. The lamps are the bench's outputs; the register map is
   what a panel or a SCADA system reads over the network. *)
CONFIGURATION VatPasteurizer
VAR_GLOBAL
    Io : FieldSignals;
    Panel : OperatorCommands;
    Status : VatStatus;
END_VAR
RESOURCE Plc ON PLC
    TASK ControlTask (INTERVAL := T#10ms, PRIORITY := 1);
    TASK SimulationTask (INTERVAL := T#20ms, PRIORITY := 2);
    TASK PanelTask (INTERVAL := T#500ms, PRIORITY := 3);
    PROGRAM Vat WITH ControlTask : VatControl;
    PROGRAM Plant WITH SimulationTask : PlantSimulation;
    PROGRAM Texts WITH PanelTask : PanelTexts;
END_RESOURCE
VAR_CONFIG
    Vat.LampRunning AT %QX0.0 : BOOL;
    Vat.LampWarning AT %QX0.1 : BOOL;
    Vat.LampAlarm AT %QX0.2 : BOOL;
    Vat.RegState AT %MW0 : INT;
    Vat.RegAlarms AT %MW2 : INT;
    Vat.RegTemperature AT %MD4 : DINT;
    Vat.RegLevel AT %MD8 : DINT;
    Vat.RegBatches AT %MD12 : UDINT;
END_VAR
END_CONFIGURATION
