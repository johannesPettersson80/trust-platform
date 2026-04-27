CONFIGURATION BoosterCommissioningDecoratorOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Cell WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Cell.CommissioningMode AT %IX0.0 : BOOL;
    Cell.LoopbackOnly AT %IX0.1 : BOOL;
    Cell.RealPressureRaw AT %IW0 : INT;
    Cell.PumpSpeedRaw AT %QW0 : INT;
    Cell.TestLampOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
