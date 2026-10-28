(* Run devices (specification trust-plant-library §6.1): one implementation of run control,
   feedback supervision, protection trips, counters and service, shared by the motor family, the
   variable-speed motors and the heater. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK ABSTRACT FB_RunDevice EXTENDS FB_Device IMPLEMENTS IDutyMember, IServiceable, IAlarmSource
VAR_INPUT
    (* The program's run demand, followed in Auto. *)
    AutoRun : BOOL;
    (* The run contact; ignored without HasFeedback. *)
    RunningFeedback : BOOL;
    (* A protection trip such as an overload relay: stops the device and latches. *)
    Trip : BOOL;
    HasFeedback : BOOL := TRUE;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* The contactor output. *)
    RunCommand : BOOL;
    Running : BOOL;
    FailedToStart : BOOL;
    FailedToStop : BOOL;
    Tripped : BOOL;
    ServiceDue : BOOL;
END_VAR
VAR_OUTPUT RETAIN
    Starts : UDINT;
    RunHours : LREAL;
    HoursSinceService : LREAL;
END_VAR
VAR PUBLIC RETAIN
    StartTimeout : TIME := T#5s;
    StopTimeout : TIME := T#5s;
    (* Hours of running between services; 0 for none. *)
    ServiceInterval : LREAL := 4000.0;
END_VAR
VAR NON_RETAIN
    ManualRun : BOOL;
    GroupRun : BOOL;
    WasRunning : BOOL;
    StartWatch : TON;
    StopWatch : TON;
    Clock : FB_ScanClock;
    TripAlarm : Alarm.DigitalAlarm;
    FailedToStartAlarm : Alarm.DigitalAlarm;
    FailedToStopAlarm : Alarm.DigitalAlarm;
END_VAR

(* One scan of run control. A concrete block calls it from its body. *)
METHOD PROTECTED ExecuteRun
VAR
    Wanted : BOOL;
    Supervised : BOOL;
    Hours : LREAL;
END_VAR
    Clock();
    Supervised := Mode <> E_Mode#OutOfService;
    IF TakeReset() THEN
        IF NOT Trip THEN
            Tripped := FALSE;
        END_IF;
        FailedToStart := FALSE;
        FailedToStop := FALSE;
        StartWatch(IN := FALSE);
        StopWatch(IN := FALSE);
    END_IF;
    IF Supervised AND Trip THEN
        Tripped := TRUE;
    END_IF;
    Fault := Tripped OR FailedToStart OR FailedToStop;

    (* Who commands it (§3.2). *)
    CASE Mode OF
        E_Mode#Auto:
            Wanted := AutoRun OR GroupRun;
        E_Mode#Manual:
            Wanted := ManualRun;
    ELSE
        Wanted := FALSE;
    END_CASE;
    (* A start needs the permissive; a running device keeps running without it (§4.1). *)
    IF Wanted AND NOT RunCommand AND NOT MayStart() THEN
        Wanted := FALSE;
    END_IF;
    IF HeldSafe() THEN
        Wanted := FALSE;
    END_IF;
    RunCommand := Wanted;

    (* Feedback supervision, whoever commanded the device (§4.4). *)
    IF HasFeedback THEN
        Running := RunningFeedback;
    ELSE
        Running := RunCommand;
    END_IF;
    StartWatch(IN := Supervised AND HasFeedback AND RunCommand AND NOT RunningFeedback, PT := StartTimeout);
    StopWatch(IN := Supervised AND HasFeedback AND NOT RunCommand AND RunningFeedback, PT := StopTimeout);
    IF StartWatch.Q THEN
        FailedToStart := TRUE;
    END_IF;
    IF StopWatch.Q THEN
        FailedToStop := TRUE;
    END_IF;
    IF FailedToStart OR FailedToStop THEN
        Fault := TRUE;
        RunCommand := FALSE;
    END_IF;
    IF Tripped THEN
        FaultText := 'Protection trip: reset after the cause is cleared';
    ELSIF FailedToStart THEN
        FaultText := 'Failed to start: reset to try again';
    ELSIF FailedToStop THEN
        FaultText := 'Failed to stop: check the contactor, then reset';
    ELSE
        FaultText := '';
    END_IF;

    (* Bumpless (§3.4): Manual continues from the present command; any stop clears it. *)
    ManualRun := RunCommand;

    (* Alarms, suppressed by design only out of service (§4.5). *)
    TripAlarm(Condition := Tripped, Suppressed := NOT Supervised,
        Message := 'Protection trip', Priority := AlarmPriority#High);
    FailedToStartAlarm(Condition := FailedToStart, Suppressed := NOT Supervised,
        Message := 'Failed to start', Priority := AlarmPriority#Medium);
    FailedToStopAlarm(Condition := FailedToStop, Suppressed := NOT Supervised,
        Message := 'Failed to stop', Priority := AlarmPriority#High);

    (* Counters stand still out of service (§4.7). *)
    IF Supervised THEN
        IF Running AND NOT WasRunning AND Starts < UDINT#4294967295 THEN
            Starts := Starts + 1;
        END_IF;
        IF Running THEN
            Hours := Clock.Seconds / 3600.0;
            RunHours := RunHours + Hours;
            HoursSinceService := HoursSinceService + Hours;
        END_IF;
    END_IF;
    WasRunning := Running;
    ServiceDue := ServiceInterval > 0.0 AND HoursSinceService >= ServiceInterval;
    Publish();
END_METHOD

METHOD PUBLIC Start : BOOL
    Start := AcceptsStart();
    IF Start THEN
        ManualRun := TRUE;
    END_IF;
END_METHOD

METHOD PUBLIC Stop : BOOL
    Stop := AcceptsCommands();
    IF Stop THEN
        ManualRun := FALSE;
    END_IF;
END_METHOD

METHOD PUBLIC IsRunning : BOOL
    IsRunning := Running;
END_METHOD

(* A group may run it: in Auto, no fault, no interlock, and permitted or already running. *)
METHOD PUBLIC IsAvailable : BOOL
    IsAvailable := Mode = E_Mode#Auto AND NOT Fault AND NOT Interlock AND (Permissive OR RunCommand);
END_METHOD

(* A group's run demand, followed in Auto like AutoRun (§9.2). *)
METHOD PUBLIC Demand
VAR_INPUT
    Run : BOOL;
END_VAR
    GroupRun := Run;
END_METHOD

METHOD PUBLIC GetRunHours : LREAL
    GetRunHours := RunHours;
END_METHOD

METHOD PUBLIC ServiceDone : BOOL
    HoursSinceService := 0.0;
    ServiceDue := FALSE;
    ServiceDone := TRUE;
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 3;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    CASE Index OF
        1: GetAlarm := REF(TripAlarm);
        2: GetAlarm := REF(FailedToStartAlarm);
        3: GetAlarm := REF(FailedToStopAlarm);
    ELSE
        GetAlarm := NULL;
    END_CASE;
END_METHOD
END_FUNCTION_BLOCK

(* A fixed-speed motor. *)
FUNCTION_BLOCK FB_Motor EXTENDS FB_RunDevice
    ExecuteRun();
END_FUNCTION_BLOCK

(* The family below differs only in name and symbol; no process assumptions. *)
FUNCTION_BLOCK FB_Pump EXTENDS FB_Motor
    ExecuteRun();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Fan EXTENDS FB_Motor
    ExecuteRun();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Blower EXTENDS FB_Motor
    ExecuteRun();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Agitator EXTENDS FB_Motor
    ExecuteRun();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Conveyor EXTENDS FB_Motor
    ExecuteRun();
END_FUNCTION_BLOCK

END_NAMESPACE
