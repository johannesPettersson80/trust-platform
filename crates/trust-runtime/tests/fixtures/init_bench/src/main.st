TYPE
    ChannelCfg : STRUCT
        gain : INT := 2;
        enabled : BOOL := TRUE;
    END_STRUCT;
    DefaultChannel : ChannelCfg := (gain := 7, enabled := TRUE);
END_TYPE

FUNCTION_BLOCK InitWorker
VAR_INPUT
    preset : INT;
END_VAR
VAR_OUTPUT
    observed : INT;
END_VAR
VAR PUBLIC
    publicValue : INT;
END_VAR
observed := preset + publicValue;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    cfg0, cfg1, cfg2 : DefaultChannel := (enabled := TRUE);
    worker0, worker1 : InitWorker := (preset := 3, publicValue := 4);
    total : INT;
END_VAR
VAR RETAIN
    retained : ChannelCfg := (gain := 12, enabled := TRUE);
END_VAR
worker0();
worker1();
cfg1.gain := cfg1.gain + 1;
retained.gain := retained.gain + 1;
total := worker0.observed + worker1.observed + cfg0.gain + cfg1.gain + cfg2.gain + retained.gain;
END_PROGRAM

CONFIGURATION InitBenchConfig
TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : Main;
END_CONFIGURATION
