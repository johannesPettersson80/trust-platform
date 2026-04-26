FUNCTION_BLOCK ValveActuator IMPLEMENTS IValve
VAR
    OpenState : BOOL;
END_VAR

METHOD PUBLIC Open : BOOL
    OpenState := TRUE;
    Open := OpenState;
END_METHOD

METHOD PUBLIC Close : BOOL
    OpenState := FALSE;
    Close := OpenState;
END_METHOD

PUBLIC PROPERTY IsOpen : BOOL
    GET
        IsOpen := OpenState;
    END_GET
END_PROPERTY
END_FUNCTION_BLOCK
