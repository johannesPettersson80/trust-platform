CONFIGURATION DairySeparatorAdapterStateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Separator WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Separator.StartCommand AT %IX0.0 : BOOL;
    Separator.DischargeRequest AT %IX0.1 : BOOL;
    Separator.CipMode AT %IX0.2 : BOOL;
    Separator.UseSiemensDrive AT %IX0.3 : BOOL;
    Separator.StatusWordRaw AT %IW0 : WORD;
    Separator.SpeedFeedbackRaw AT %IW2 : INT;
    Separator.VibrationRaw AT %IW4 : INT;
    Separator.FaultRaw AT %IW6 : WORD;
    Separator.FeedValveOut AT %QX0.0 : BOOL;
    Separator.DischargeValveOut AT %QX0.1 : BOOL;
    Separator.CipValveOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
