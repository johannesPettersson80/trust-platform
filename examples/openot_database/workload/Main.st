TYPE E_FillStep : (Idle := 0, Filling := 1, Holding := 2, Complete := 3) END_TYPE
TYPE E_OperatingMode : (ModeManual := 0, ModeAutomatic := 1) END_TYPE
TYPE E_UnitProcedure : (Idle := 0, Running := 1, Complete := 2) END_TYPE
TYPE E_PackState : (Idle := 0, Starting := 1, Execute := 2, Completing := 3, Complete := 4) END_TYPE

PROGRAM Filler
VAR
    StartCmd : BOOL := TRUE;
    BatchCount : UDINT := UDINT#1;
    ReadyFlag : BOOL := TRUE;
    StatusText : STRING[16] := 'starting';
    FillStarted : BOOL {attribute 'oot' := 'message', 'template' := 'filler level batch ready status', 'severity' := '500', 'arg1' := 'Level', 'arg2' := 'BatchCount', 'arg3' := 'ReadyFlag', 'arg4' := 'StatusText'};
    Step : E_FillStep {attribute 'oot' := 'state', 'category' := 'process'} := E_FillStep#Idle;
    Mode : E_OperatingMode {attribute 'oot' := 'state', 'category' := 'mode'} := ModeManual;
    Isa88Procedure : E_UnitProcedure {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'ISA-88'} := E_UnitProcedure#Idle;
    PackMlProcedure : E_PackState {attribute 'oot' := 'state', 'category' := 'procedural', 'model' := 'PackML'} := E_PackState#Idle;
    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L', 'sampling' := 'deadband', 'deadband' := '0.5'};
END_VAR

Mode := ModeAutomatic;
Isa88Procedure := E_UnitProcedure#Running;
PackMlProcedure := E_PackState#Execute;

CASE Step OF
    E_FillStep#Idle:
        IF StartCmd THEN
            FillStarted := TRUE;
            Step := E_FillStep#Filling;
        END_IF;
    E_FillStep#Filling:
        Level := Level + REAL#5.0;
        IF Level >= REAL#10.0 THEN
            Step := E_FillStep#Holding;
        END_IF;
    E_FillStep#Holding:
        Step := E_FillStep#Complete;
    E_FillStep#Complete:
        Isa88Procedure := E_UnitProcedure#Complete;
        PackMlProcedure := E_PackState#Complete;
        StatusText := 'complete';
        ;
ELSE
END_CASE;
END_PROGRAM

TYPE E_BatchState : (Started := 0, Completed := 1, Held := 2, Resumed := 3, Aborted := 4, Paused := 5) END_TYPE

PROGRAM BatchControl
VAR
    BatchPhase : UINT;
    RecipeId : UDINT := UDINT#3001;
    RecipeVersion : STRING[16] := 'v1.2.3';
    BatchId : UDINT := UDINT#4001;
    MaterialId : UDINT := UDINT#5001;
    Quantity : LREAL := LREAL#12.25;
    AuthResult : UINT := UINT#1;
    Approver : STRING[32] := 'approver-a';
    BatchState : E_BatchState := Started {attribute 'oot' := 'batch', 'batchId' := BatchId, 'recipe' := RecipeId};
    RecipeLoadedTrigger : BOOL {attribute 'oot' := 'recipe-loaded', 'recipe' := RecipeId, 'version' := RecipeVersion, 'batch' := BatchId};
    RecipeApprovedTrigger : BOOL {attribute 'oot' := 'recipe-approved', 'recipe' := RecipeId, 'version' := RecipeVersion, 'auth' := AuthResult, 'by' := Approver};
    MaterialAdditionTrigger : BOOL {attribute 'oot' := 'material-addition', 'batch' := BatchId, 'material' := MaterialId, 'quantity' := Quantity, 'unit' := 'kg'};
END_VAR

CASE BatchPhase OF
    UINT#0:
        RecipeLoadedTrigger := TRUE;
    UINT#1:
        RecipeApprovedTrigger := TRUE;
    UINT#2:
        MaterialAdditionTrigger := TRUE;
    UINT#3:
        BatchState := Held;
END_CASE;
BatchPhase := BatchPhase + UINT#1;
END_PROGRAM

PROGRAM OperatorAudit
VAR
    AuditPhase : UINT;
    ActionId : UDINT := UDINT#6001;
    ContextA : UDINT := UDINT#7001;
    ContextB : UDINT := UDINT#7002;
    OperatorName : STRING[32] := 'operator-a';
    FailedOperator : STRING[32] := 'operator-x';
    Workstation : STRING[32] := 'station-1';
    FailedWorkstation : STRING[32] := 'station-2';
    Role : UINT := UINT#3;
    ReasonText : STRING[32] := 'denied';
    ActionTrigger : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName, 'context1' := ContextA, 'context2' := ContextB, 'auth' := 'Granted', 'workstation' := Workstation};
    LoginTrigger : BOOL {attribute 'oot' := 'operator-login', 'actor' := OperatorName, 'auth' := 'Granted', 'workstation' := Workstation, 'role' := Role};
    LogoutTrigger : BOOL {attribute 'oot' := 'operator-logout', 'actor' := OperatorName, 'workstation' := Workstation};
    SecurityFailureTrigger : BOOL {attribute 'oot' := 'security-failure', 'actor' := FailedOperator, 'workstation' := FailedWorkstation, 'reason' := ReasonText};
END_VAR

CASE AuditPhase OF
    UINT#0:
        ActionTrigger := TRUE;
    UINT#1:
        LoginTrigger := TRUE;
    UINT#2:
        LogoutTrigger := TRUE;
    UINT#3:
        SecurityFailureTrigger := TRUE;
END_CASE;
AuditPhase := AuditPhase + UINT#1;
END_PROGRAM

PROGRAM SignatureAudit
VAR
    SignaturePhase : UINT;
    ActionId : UDINT := UDINT#6002;
    OperatorName : STRING[32] := 'operator-a';
    TargetAction : BOOL {attribute 'oot' := 'operator-action', 'action' := ActionId, 'actor' := OperatorName};
    SignTarget : BOOL {attribute 'oot' := 'e-signature', 'action' := ActionId, 'actor' := OperatorName, 'meaning' := 'Approved', 'attests' := TargetAction, 'auth' := 'Granted'};
END_VAR

CASE SignaturePhase OF
    UINT#0:
        TargetAction := TRUE;
    UINT#1:
        TargetAction := FALSE;
        SignTarget := TRUE;
END_CASE;
SignaturePhase := SignaturePhase + UINT#1;
END_PROGRAM

PROGRAM TypedValues
VAR
    TypedPhase : UINT;
    QualityOperator : STRING[32] := 'qa-operator';
    QualityReason : STRING[32] := 'setpoint adjustment';
    Enabled : BOOL {attribute 'oot' := 'value'};
    Signed8 : SINT {attribute 'oot' := 'value'};
    Unsigned8 : USINT {attribute 'oot' := 'value'};
    Signed16 : INT {attribute 'oot' := 'value'};
    Unsigned16 : UINT {attribute 'oot' := 'value'};
    Signed32 : DINT {attribute 'oot' := 'value'};
    Unsigned32 : UDINT {attribute 'oot' := 'value'};
    Signed64 : LINT {attribute 'oot' := 'value'};
    Total : ULINT {attribute 'oot' := 'value'};
    Temperature : REAL {attribute 'oot' := 'value', 'unit' := 'degC', 'semanticRole' := 'actual'};
    PeriodicTemperature : REAL {attribute 'oot' := 'value', 'sampling' := 'periodic', 'interval' := '250'};
    HysteresisFlow : REAL {attribute 'oot' := 'value', 'sampling' := 'hysteresis', 'deadband' := '1.5'};
    Ratio : LREAL {attribute 'oot' := 'value'};
    Label : STRING[16] {attribute 'oot' := 'value'};
    SetPoint : REAL {attribute 'oot' := 'value', 'id' := '2101', 'audit' := 'true', 'actor' := QualityOperator, 'reason' := QualityReason, 'auth' := 'Granted', 'unit' := 'L', 'semanticRole' := 'setpoint'};
END_VAR

Enabled := TRUE;
Signed8 := SINT#-8;
Unsigned8 := USINT#8;
Signed16 := INT#-16;
Unsigned16 := UINT#16;
Signed32 := DINT#-32;
Unsigned32 := UDINT#32;
Signed64 := LINT#-64;
Total := ULINT#42;
Temperature := REAL#21.5;
PeriodicTemperature := PeriodicTemperature + REAL#0.25;
CASE TypedPhase OF
    UINT#0:
        HysteresisFlow := REAL#1.0;
    UINT#1:
        HysteresisFlow := REAL#1.5;
    UINT#2:
        HysteresisFlow := REAL#3.0;
    UINT#3:
        HysteresisFlow := REAL#2.0;
    UINT#4:
        HysteresisFlow := REAL#0.5;
END_CASE;
Ratio := LREAL#1.25;
Label := 'ready';
CASE TypedPhase OF
    UINT#0:
        SetPoint := REAL#10.0;
    UINT#1:
        SetPoint := REAL#12.5;
END_CASE;
TypedPhase := TypedPhase + UINT#1;
END_PROGRAM

PROGRAM ConditionLifecycle
VAR
    ConditionPhase : UINT;
    OperatorName : STRING[32] := 'operator-a';
    ReasonText : STRING[32] := 'maintenance';
    CommentText : STRING[32] := 'operator comment';
    ShelveSecs : UDINT := UDINT#300;
    PreviousPriority : UINT := UINT#600;
    NewPriority : UINT := UINT#900;
    AckHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'acknowledge', 'by' := OperatorName};
    HighPressureAlarm : BOOL {attribute 'oot' := 'alarm', 'conditionid' := '9101', 'class' := 'alarm', 'severity' := '900'};
    GuardOpen : BOOL;
    GuardInterlock : BOOL {attribute 'oot' := 'alarm', 'conditionid' := '9102', 'class' := 'interlock', 'severity' := '950', 'cause' := 'GuardOpen'};
    ConfirmHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'confirm', 'by' := OperatorName};
    ShelveHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'shelve', 'by' := OperatorName, 'seconds' := ShelveSecs};
    UnshelveHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'unshelve'};
    SuppressHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'suppress', 'reason' := ReasonText};
    UnsuppressHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'unsuppress'};
    OosHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'out-of-service', 'by' := OperatorName};
    InServiceHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'in-service'};
    ResetHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'reset', 'by' := OperatorName};
    CommentHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'comment', 'comment' := CommentText, 'by' := OperatorName};
    PriorityHighPressure : BOOL {attribute 'oot' := 'condition', 'of' := HighPressureAlarm, 'event' := 'priority-changed', 'previous-priority' := PreviousPriority, 'new-priority' := NewPriority, 'by' := OperatorName};
END_VAR

CASE ConditionPhase OF
    UINT#0:
        HighPressureAlarm := TRUE;
        GuardOpen := TRUE;
        GuardInterlock := TRUE;
        AckHighPressure := TRUE;
    UINT#1:
        ConfirmHighPressure := TRUE;
    UINT#2:
        ShelveHighPressure := TRUE;
    UINT#3:
        UnshelveHighPressure := TRUE;
    UINT#4:
        SuppressHighPressure := TRUE;
    UINT#5:
        UnsuppressHighPressure := TRUE;
    UINT#6:
        OosHighPressure := TRUE;
    UINT#7:
        InServiceHighPressure := TRUE;
    UINT#8:
        ResetHighPressure := TRUE;
    UINT#9:
        CommentHighPressure := TRUE;
    UINT#10:
        PriorityHighPressure := TRUE;
    UINT#11:
        HighPressureAlarm := FALSE;
        GuardOpen := FALSE;
        GuardInterlock := FALSE;
END_CASE;
ConditionPhase := ConditionPhase + UINT#1;
END_PROGRAM
