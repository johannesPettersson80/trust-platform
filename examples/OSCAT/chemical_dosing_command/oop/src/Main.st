(* Record role: DosingMemento is a deterministic PLC-side record for tests and runtime readers. *)
TYPE DosingMemento :
STRUCT
    CommandId : DWORD;
    BeforePH : REAL;
    TankLevelBeforeMl : REAL;
    DoseAmountMl : REAL;
END_STRUCT
END_TYPE

(* Pattern role: IDosingCommand is the contract boundary for the Command and Memento example. *)
INTERFACE IDosingCommand
    PROPERTY CommandId : DWORD
    GET
    END_GET
    END_PROPERTY

    PROPERTY DoseAmountMl : REAL
    GET
    END_GET
    END_PROPERTY

    METHOD CanExecute : BOOL
    VAR_INPUT
        CalibrationValid : BOOL;
        TankLevelMl : REAL;
        SafePH : BOOL;
    END_VAR
    END_METHOD

    METHOD CaptureMemento : DosingMemento
    VAR_INPUT
        BeforePH : REAL;
        TankLevelMl : REAL;
    END_VAR
    END_METHOD

    METHOD Execute : BOOL
    END_METHOD

    METHOD Undo
    END_METHOD
END_INTERFACE

(* Pattern role: ChlorineDoseCommand is a concrete implementation in the Command and Memento example. *)
FUNCTION_BLOCK ChlorineDoseCommand IMPLEMENTS IDosingCommand
VAR
    ExecutedValue : BOOL;
    UndoneValue : BOOL;
END_VAR

METHOD PUBLIC CanExecute : BOOL
VAR_INPUT
    CalibrationValid : BOOL;
    TankLevelMl : REAL;
    SafePH : BOOL;
END_VAR
CanExecute := CalibrationValid AND SafePH AND (TankLevelMl >= REAL#120.0);
END_METHOD

METHOD PUBLIC CaptureMemento : DosingMemento
VAR_INPUT
    BeforePH : REAL;
    TankLevelMl : REAL;
END_VAR
VAR
    M : DosingMemento;
END_VAR
M.CommandId := DWORD#16#C001;
M.BeforePH := BeforePH;
M.TankLevelBeforeMl := TankLevelMl;
M.DoseAmountMl := REAL#120.0;
CaptureMemento := M;
END_METHOD

METHOD PUBLIC Execute : BOOL
ExecutedValue := TRUE;
Execute := TRUE;
END_METHOD

METHOD PUBLIC Undo
UndoneValue := TRUE;
ExecutedValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : DWORD
GET
    CommandId := DWORD#16#C001;
END_GET
END_PROPERTY

PUBLIC PROPERTY DoseAmountMl : REAL
GET
    DoseAmountMl := REAL#120.0;
END_GET
END_PROPERTY

PUBLIC PROPERTY Executed : BOOL
GET
    Executed := ExecutedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: AlumDoseCommand is a concrete implementation in the Command and Memento example. *)
FUNCTION_BLOCK AlumDoseCommand IMPLEMENTS IDosingCommand
VAR
    ExecutedValue : BOOL;
END_VAR

METHOD PUBLIC CanExecute : BOOL
VAR_INPUT
    CalibrationValid : BOOL;
    TankLevelMl : REAL;
    SafePH : BOOL;
END_VAR
CanExecute := CalibrationValid AND SafePH AND (TankLevelMl >= REAL#250.0);
END_METHOD

METHOD PUBLIC CaptureMemento : DosingMemento
VAR_INPUT
    BeforePH : REAL;
    TankLevelMl : REAL;
END_VAR
VAR
    M : DosingMemento;
END_VAR
M.CommandId := DWORD#16#C002;
M.BeforePH := BeforePH;
M.TankLevelBeforeMl := TankLevelMl;
M.DoseAmountMl := REAL#250.0;
CaptureMemento := M;
END_METHOD

METHOD PUBLIC Execute : BOOL
ExecutedValue := TRUE;
Execute := TRUE;
END_METHOD

METHOD PUBLIC Undo
ExecutedValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : DWORD
GET
    CommandId := DWORD#16#C002;
END_GET
END_PROPERTY

PUBLIC PROPERTY DoseAmountMl : REAL
GET
    DoseAmountMl := REAL#250.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: AntiScalantDoseCommand is a concrete implementation in the Command and Memento example. *)
FUNCTION_BLOCK AntiScalantDoseCommand IMPLEMENTS IDosingCommand
VAR
    ExecutedValue : BOOL;
END_VAR

METHOD PUBLIC CanExecute : BOOL
VAR_INPUT
    CalibrationValid : BOOL;
    TankLevelMl : REAL;
    SafePH : BOOL;
END_VAR
CanExecute := CalibrationValid AND SafePH AND (TankLevelMl >= REAL#80.0);
END_METHOD

METHOD PUBLIC CaptureMemento : DosingMemento
VAR_INPUT
    BeforePH : REAL;
    TankLevelMl : REAL;
END_VAR
VAR
    M : DosingMemento;
END_VAR
M.CommandId := DWORD#16#C003;
M.BeforePH := BeforePH;
M.TankLevelBeforeMl := TankLevelMl;
M.DoseAmountMl := REAL#80.0;
CaptureMemento := M;
END_METHOD

METHOD PUBLIC Execute : BOOL
ExecutedValue := TRUE;
Execute := TRUE;
END_METHOD

METHOD PUBLIC Undo
ExecutedValue := FALSE;
END_METHOD

PUBLIC PROPERTY CommandId : DWORD
GET
    CommandId := DWORD#16#C003;
END_GET
END_PROPERTY

PUBLIC PROPERTY DoseAmountMl : REAL
GET
    DoseAmountMl := REAL#80.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: DosingScheduler owns the collaborating objects used by this Command and Memento example. *)
FUNCTION_BLOCK DosingScheduler
VAR
    Pending : IDosingCommand;
    HasPending : BOOL;
    AuditLog : DwordFifo32;
    LastMemento : DosingMemento;
    ExecutedCountValue : INT;
    RejectedCountValue : INT;
    LastCommandIdValue : DWORD;
    LastCanExecuteValue : BOOL;
    LastExecuteResultValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
AuditLog.Initialize();
HasPending := FALSE;
ExecutedCountValue := INT#0;
RejectedCountValue := INT#0;
LastCommandIdValue := DWORD#0;
LastCanExecuteValue := FALSE;
LastExecuteResultValue := FALSE;
END_METHOD

METHOD PUBLIC Enqueue : BOOL
VAR_INPUT
    Cmd : IDosingCommand;
END_VAR
IF HasPending THEN
    Enqueue := FALSE;
ELSE
    Pending := Cmd;
    HasPending := TRUE;
    Enqueue := TRUE;
END_IF;
END_METHOD

METHOD PUBLIC ExecuteNext : BOOL
VAR_INPUT
    CalibrationValid : BOOL;
    TankLevelMl : REAL;
    CurrentPH : REAL;
END_VAR
VAR
    SafePH : BOOL;
END_VAR
SafePH := (CurrentPH >= REAL#4.0) AND (CurrentPH <= REAL#9.0);
(* Empty-queue path returns FALSE without incrementing rejected count
   and without dereferencing Pending (uninitialized interface ref). *)
IF NOT HasPending THEN
    LastCanExecuteValue := FALSE;
    LastExecuteResultValue := FALSE;
    ExecuteNext := FALSE;
    RETURN;
END_IF;
LastCanExecuteValue := Pending.CanExecute(CalibrationValid := CalibrationValid, TankLevelMl := TankLevelMl, SafePH := SafePH);
IF LastCanExecuteValue THEN
    LastMemento := Pending.CaptureMemento(BeforePH := CurrentPH, TankLevelMl := TankLevelMl);
    LastExecuteResultValue := Pending.Execute();
    ExecuteNext := LastExecuteResultValue;
    AuditLog.Push(Value := LastMemento.CommandId);
    LastCommandIdValue := LastMemento.CommandId;
    ExecutedCountValue := ExecutedCountValue + INT#1;
    HasPending := FALSE;
ELSE
    ExecuteNext := FALSE;
    RejectedCountValue := RejectedCountValue + INT#1;
    (* Drop the rejected command so the next Enqueue can take a different
       command. Rejection is terminal — the operator must re-issue. *)
    HasPending := FALSE;
END_IF;
END_METHOD

METHOD PUBLIC ReplayLast : DWORD
ReplayLast := LastCommandIdValue;
END_METHOD

PUBLIC PROPERTY ExecutedCount : INT
GET
    ExecutedCount := ExecutedCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RejectedCount : INT
GET
    RejectedCount := RejectedCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCommandId : DWORD
GET
    LastCommandId := LastCommandIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastBeforePH : REAL
GET
    LastBeforePH := LastMemento.BeforePH;
END_GET
END_PROPERTY

PUBLIC PROPERTY HasPendingCommand : BOOL
GET
    HasPendingCommand := HasPending;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCanExecute : BOOL
GET
    LastCanExecute := LastCanExecuteValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastExecuteResult : BOOL
GET
    LastExecuteResult := LastExecuteResultValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Dosing : DosingScheduler;
    Chlorine : ChlorineDoseCommand;
    EnqueueChlorine : BOOL;
    ExecuteCommand : BOOL;
    CalibrationValid : BOOL;
    PHRaw : INT;
    TankLevelRaw : INT;
    PumpStartOut : BOOL;
    RejectOut : BOOL;
END_VAR

Dosing.Initialize();
IF EnqueueChlorine THEN
    Dosing.Enqueue(Cmd := Chlorine);
END_IF;
IF ExecuteCommand THEN
    PumpStartOut := Dosing.ExecuteNext(CalibrationValid := CalibrationValid, TankLevelMl := INT_TO_REAL(TankLevelRaw), CurrentPH := INT_TO_REAL(PHRaw) / REAL#10.0);
    RejectOut := NOT PumpStartOut;
END_IF;
END_PROGRAM
