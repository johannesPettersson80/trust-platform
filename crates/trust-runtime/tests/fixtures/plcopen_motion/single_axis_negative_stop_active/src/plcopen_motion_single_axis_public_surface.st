TYPE AXIS_REF :
STRUCT
    AxisId : UDINT;
    InternalIndex : UINT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK MC_Stop
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
END_FUNCTION_BLOCK
