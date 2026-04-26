TYPE
    PumpState : (Idle := 0, Starting := 1, Running := 2, Fault := 3);

    PumpCommand :
    STRUCT
        Enable : BOOL;
        TargetSpeed : REAL;
    END_STRUCT;

    PumpStatus :
    STRUCT
        Running : BOOL;
        State : PumpState;
        ActualSpeed : REAL;
        Alarm : UDINT;
    END_STRUCT;
END_TYPE
