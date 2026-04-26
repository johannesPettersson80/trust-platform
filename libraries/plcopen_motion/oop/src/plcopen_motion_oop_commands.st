FUNCTION_BLOCK MC_OopCommand IMPLEMENTS itfCommand
VAR
    DoneValue : BOOL;
    BusyValue : BOOL;
    ActiveValue : BOOL;
    CommandAbortedValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : MC_ERROR;
END_VAR

METHOD PUBLIC SetStatus : BOOL
VAR_INPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorId : MC_ERROR;
END_VAR
DoneValue := Done;
BusyValue := Busy;
ActiveValue := Active;
CommandAbortedValue := CommandAborted;
ErrorValue := Error;
ErrorIdValue := ErrorId;
SetStatus := TRUE;
END_METHOD

METHOD PUBLIC SetError : BOOL
VAR_INPUT
    ErrorId : MC_ERROR;
END_VAR
DoneValue := FALSE;
BusyValue := FALSE;
ActiveValue := FALSE;
CommandAbortedValue := FALSE;
ErrorValue := TRUE;
ErrorIdValue := ErrorId;
SetError := TRUE;
END_METHOD

METHOD PUBLIC Abort : MC_ERROR
DoneValue := FALSE;
BusyValue := FALSE;
ActiveValue := FALSE;
CommandAbortedValue := TRUE;
ErrorValue := FALSE;
ErrorIdValue := WORD#16#0000;
Abort := WORD#16#0000;
END_METHOD

METHOD PUBLIC Wait : MC_ERROR
VAR_INPUT
    Timeout : TIME;
    AbortOnTimeout : BOOL;
END_VAR
Wait := WORD#16#0500;
END_METHOD

PUBLIC PROPERTY Done : BOOL
GET
    Done := DoneValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Busy : BOOL
GET
    Busy := BusyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Active : BOOL
GET
    Active := ActiveValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY CommandAborted : BOOL
GET
    CommandAborted := CommandAbortedValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : MC_ERROR
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_OopAxisCommand EXTENDS MC_OopCommand IMPLEMENTS itfAxisCommand
METHOD PUBLIC Update : MC_ERROR
VAR_INPUT
    Position : REAL;
    Velocity : REAL;
    EndVelocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
Update := WORD#16#0500;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_OopContinuousAxisCommand EXTENDS MC_OopAxisCommand IMPLEMENTS itfContinuousAxisCommand
VAR
    InVelocityValue : BOOL;
END_VAR

METHOD PUBLIC SetContinuousStatus : BOOL
VAR_INPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorId : MC_ERROR;
    InVelocity : BOOL;
END_VAR
VAR
    Updated : BOOL;
END_VAR
Updated := SetStatus(
    Done := Done,
    Busy := Busy,
    Active := Active,
    CommandAborted := CommandAborted,
    Error := Error,
    ErrorId := ErrorId
);
InVelocityValue := InVelocity;
SetContinuousStatus := Updated;
END_METHOD

METHOD PUBLIC SetContinuousError : BOOL
VAR_INPUT
    ErrorId : MC_ERROR;
END_VAR
VAR
    Updated : BOOL;
END_VAR
Updated := SetError(ErrorId := ErrorId);
InVelocityValue := FALSE;
SetContinuousError := Updated;
END_METHOD

PUBLIC PROPERTY InVelocity : BOOL
GET
    InVelocity := InVelocityValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_OopSynchronizedAxisCommand EXTENDS MC_OopAxisCommand IMPLEMENTS itfSynchronizedAxisCommand
VAR
    InSyncValue : BOOL;
END_VAR

METHOD PUBLIC SetSynchronizedStatus : BOOL
VAR_INPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorId : MC_ERROR;
    InSync : BOOL;
END_VAR
VAR
    Updated : BOOL;
END_VAR
Updated := SetStatus(
    Done := Done,
    Busy := Busy,
    Active := Active,
    CommandAborted := CommandAborted,
    Error := Error,
    ErrorId := ErrorId
);
InSyncValue := InSync;
SetSynchronizedStatus := Updated;
END_METHOD

METHOD PUBLIC SetSynchronizedError : BOOL
VAR_INPUT
    ErrorId : MC_ERROR;
END_VAR
VAR
    Updated : BOOL;
END_VAR
Updated := SetError(ErrorId := ErrorId);
InSyncValue := FALSE;
SetSynchronizedError := Updated;
END_METHOD

PUBLIC PROPERTY InSync : BOOL
GET
    InSync := InSyncValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
