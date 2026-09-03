
PROGRAM Main
VAR
    Controller : LessonDiscretePID;
    MethodSelection : INT := 2;
    IntervalSelection : INT := 0;
    DelayEnabled : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ControlEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    Setpoint : LREAL := 0.0;
    MeasuredPosition : LREAL := 0.0;
    MotorVelocity : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    UpdatePulse : BOOL := FALSE;
    BaseScanCounter : INT := 0;
    ControllerIntervalScans : INT := 1;
    ControllerCycleTime : LREAL := 0.01;
    StartupInitialized : BOOL := FALSE;
END_VAR

IF NOT StartupInitialized THEN
    StartupInitialized := TRUE;
END_IF;

(* One reviewed Test authority selects the diagnostic 100 ms Tustin case.
   Native tests use fresh runtimes for the full method/interval/delay matrix. *)
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    MethodSelection := 2;
    IntervalSelection := 2;
    DelayEnabled := FALSE;
    Setpoint := LREAL#0.5;
    BaseScanCounter := 0;
    ControlEnabled := TRUE;
ELSIF ExperimentEnabled THEN
    ControlEnabled := TRUE;
ELSIF ExperimentWasEnabled THEN
    Setpoint := LREAL#0.0;
    MethodSelection := 2;
    IntervalSelection := 0;
    DelayEnabled := FALSE;
    ControlEnabled := FALSE;
    RequestedTorque := LREAL#0.0;
    BaseScanCounter := 0;
END_IF;

CASE IntervalSelection OF
    0:
        ControllerIntervalScans := 1;
        ControllerCycleTime := LREAL#0.01;
    1:
        ControllerIntervalScans := 2;
        ControllerCycleTime := LREAL#0.02;
ELSE
    ControllerIntervalScans := 10;
    ControllerCycleTime := LREAL#0.10;
END_CASE;

UpdatePulse := FALSE;
InputsValid := MeasuredPosition >= LREAL#-4.0 AND MeasuredPosition <= LREAL#4.0
    AND MotorVelocity >= LREAL#-20.0 AND MotorVelocity <= LREAL#20.0;
IF ControlEnabled AND InputsValid AND NOT SafetyAbort THEN
    BaseScanCounter := BaseScanCounter + 1;
    IF BaseScanCounter >= ControllerIntervalScans THEN
        BaseScanCounter := 0;
        RequestedTorque := Controller.Update(
            Setpoint := Setpoint,
            ProcessValue := MeasuredPosition,
            CycleTime := ControllerCycleTime,
            MethodSelection := MethodSelection,
            DelayEnabled := DelayEnabled
        );
        UpdatePulse := TRUE;
    END_IF;
ELSE
    RequestedTorque := LREAL#0.0;
    BaseScanCounter := 0;
END_IF;
(* Between controller updates RequestedTorque is deliberately unchanged. The
   drive therefore receives the zero-order-held command. *)
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ControlEnabled AT %MX16.0 : BOOL;
    P1.ExperimentEnabled AT %MX16.1 : BOOL;
    P1.SafetyAbort AT %MX16.2 : BOOL;
    P1.InputsValid AT %MX16.3 : BOOL;
    P1.MethodSelection AT %MW32 : INT;
    P1.IntervalSelection AT %MW34 : INT;
    P1.DelayEnabled AT %MX36.0 : BOOL;
    P1.Setpoint AT %ML40 : LREAL;
    P1.MeasuredPosition AT %IL0 : LREAL;
    P1.MotorVelocity AT %IL8 : LREAL;
    P1.RequestedTorque AT %QL0 : LREAL;
    P1.UpdatePulse AT %MX48.0 : BOOL;
END_VAR
END_CONFIGURATION
