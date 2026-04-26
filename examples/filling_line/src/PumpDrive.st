FUNCTION_BLOCK PumpDrive IMPLEMENTS IPump
VAR
    RunningValue : BOOL;
    SpeedValue : INT;
END_VAR

METHOD PUBLIC Start : BOOL
    RunningValue := TRUE;
    Start := RunningValue;
END_METHOD

METHOD PUBLIC Stop : BOOL
    RunningValue := FALSE;
    Stop := RunningValue;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
    GET
        IsRunning := RunningValue;
    END_GET
    SET
    END_SET
END_PROPERTY

PUBLIC PROPERTY Speed : INT
    GET
        Speed := SpeedValue;
    END_GET
    SET
        SpeedValue := Speed;
    END_SET
END_PROPERTY
END_FUNCTION_BLOCK
