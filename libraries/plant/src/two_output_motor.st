(* Motors with two exclusive run outputs (specification trust-plant-library §6.4, §6.5): the
   reversing motor (forward, reverse) and the two-speed motor (slow, fast). One implementation of
   the exclusive outputs, the transition delay, supervision, trips and counters; the concrete
   blocks name their own I/O. Direction or speed 0 is stopped, 1 is the first output, 2 the
   second. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK ABSTRACT FB_TwoOutputMotor EXTENDS FB_Device IMPLEMENTS IAlarmSource, IServiceable
VAR_INPUT
    Trip : BOOL;
    HasFeedback : BOOL := TRUE;
END_VAR
VAR_OUTPUT NON_RETAIN
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
    (* The output commanded now, the operator's choice, and the last running output. *)
    Commanded : INT;
    ManualWanted : INT;
    LastRun : INT;
    ActiveFirst : BOOL;
    ActiveSecond : BOOL;
    WasRunning : BOOL;
    Gap : TON;
    StartWatch : TON;
    StopWatch : TON;
    Clock : FB_ScanClock;
    TripAlarm : Alarm.DigitalAlarm;
    FailedToStartAlarm : Alarm.DigitalAlarm;
    FailedToStopAlarm : Alarm.DigitalAlarm;
END_VAR

(* One scan. AutoWanted is the program's choice; Delay is the transition time; with FullStop the
   motor must have stopped for Delay before the other output (reversing), else both outputs are
   simply dropped for Delay (changing speed). *)
METHOD PROTECTED ExecuteOutputs
VAR_INPUT
    AutoWanted : INT;
    FirstFeedback : BOOL;
    SecondFeedback : BOOL;
    Delay : TIME;
    FullStop : BOOL;
END_VAR
VAR
    Wanted : INT;
    Supervised : BOOL;
    Matching : BOOL;
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

    CASE Mode OF
        E_Mode#Auto:
            Wanted := AutoWanted;
        E_Mode#Manual:
            Wanted := ManualWanted;
    ELSE
        Wanted := 0;
    END_CASE;
    IF Wanted < 0 OR Wanted > 2 THEN
        Wanted := 0;
    END_IF;
    (* A start, including one in the other direction or speed, needs the permissive. *)
    IF Wanted <> 0 AND Wanted <> Commanded AND NOT MayStart() THEN
        Wanted := 0;
    END_IF;
    IF HeldSafe() THEN
        Wanted := 0;
    END_IF;

    (* The transition: stop first, then the other output after the delay. *)
    IF Wanted = 0 THEN
        Commanded := 0;
    ELSIF Commanded <> 0 AND Commanded <> Wanted THEN
        Commanded := 0;
    END_IF;
    IF FullStop THEN
        Gap(IN := Commanded = 0 AND NOT Running, PT := Delay);
    ELSE
        Gap(IN := Commanded = 0, PT := Delay);
    END_IF;
    IF Commanded = 0 AND Wanted <> 0 THEN
        IF LastRun = 0 OR LastRun = Wanted OR Gap.Q THEN
            Commanded := Wanted;
        END_IF;
    END_IF;

    (* Feedback supervision (§4.4). *)
    IF HasFeedback THEN
        ActiveFirst := FirstFeedback;
        ActiveSecond := SecondFeedback;
    ELSE
        ActiveFirst := Commanded = 1;
        ActiveSecond := Commanded = 2;
    END_IF;
    Running := ActiveFirst OR ActiveSecond;
    Matching := (Commanded = 1 AND FirstFeedback AND NOT SecondFeedback)
        OR (Commanded = 2 AND SecondFeedback AND NOT FirstFeedback);
    StartWatch(IN := Supervised AND HasFeedback AND Commanded <> 0 AND NOT Matching, PT := StartTimeout);
    StopWatch(IN := Supervised AND HasFeedback AND Commanded = 0 AND Running, PT := StopTimeout);
    IF StartWatch.Q THEN
        FailedToStart := TRUE;
    END_IF;
    IF StopWatch.Q THEN
        FailedToStop := TRUE;
    END_IF;
    IF FailedToStart OR FailedToStop THEN
        Fault := TRUE;
        Commanded := 0;
        Wanted := 0;
    END_IF;
    IF Commanded <> 0 THEN
        LastRun := Commanded;
    END_IF;
    IF Tripped THEN
        FaultText := 'Protection trip: reset after the cause is cleared';
    ELSIF FailedToStart THEN
        FaultText := 'Failed to start: reset to try again';
    ELSIF FailedToStop THEN
        FaultText := 'Failed to stop: check the contactors, then reset';
    ELSE
        FaultText := '';
    END_IF;

    (* Bumpless (§3.4); a refused or stopped start leaves nothing pending in Manual. *)
    ManualWanted := Wanted;

    TripAlarm(Condition := Tripped, Suppressed := NOT Supervised,
        Message := 'Protection trip', Priority := AlarmPriority#High);
    FailedToStartAlarm(Condition := FailedToStart, Suppressed := NOT Supervised,
        Message := 'Failed to start', Priority := AlarmPriority#Medium);
    FailedToStopAlarm(Condition := FailedToStop, Suppressed := NOT Supervised,
        Message := 'Failed to stop', Priority := AlarmPriority#High);

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

(* The service is done: the service count starts again; RunHours is unchanged (§4.7). *)
METHOD PUBLIC ServiceDone : BOOL
    HoursSinceService := 0.0;
    ServiceDue := FALSE;
    ServiceDone := TRUE;
END_METHOD

(* The operator's choice of output: accepted in Manual while nothing blocks a start. *)
METHOD PROTECTED RequestOutput : BOOL
VAR_INPUT
    Which : INT;
END_VAR
    RequestOutput := (Which = 1 OR Which = 2) AND AcceptsStart();
    IF RequestOutput THEN
        ManualWanted := Which;
    END_IF;
END_METHOD

METHOD PUBLIC Stop : BOOL
    Stop := AcceptsCommands();
    IF Stop THEN
        ManualWanted := 0;
    END_IF;
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

(* Forward and reverse. A change of direction stops the motor and waits ReversalDelay after it
   has stopped. Both Auto inputs TRUE means stop. *)
FUNCTION_BLOCK FB_ReversingMotor EXTENDS FB_TwoOutputMotor IMPLEMENTS IReversingDevice
VAR_INPUT
    AutoForward : BOOL;
    AutoReverse : BOOL;
    ForwardFeedback : BOOL;
    ReverseFeedback : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    ForwardCommand : BOOL;
    ReverseCommand : BOOL;
    Forward : BOOL;
    Reverse : BOOL;
END_VAR
VAR PUBLIC RETAIN
    ReversalDelay : TIME := T#2s;
END_VAR
VAR_TEMP
    Wanted : INT;
END_VAR
    IF AutoForward AND NOT AutoReverse THEN
        Wanted := 1;
    ELSIF AutoReverse AND NOT AutoForward THEN
        Wanted := 2;
    ELSE
        Wanted := 0;
    END_IF;
    ExecuteOutputs(AutoWanted := Wanted, FirstFeedback := ForwardFeedback,
        SecondFeedback := ReverseFeedback, Delay := ReversalDelay, FullStop := TRUE);
    ForwardCommand := Commanded = 1;
    ReverseCommand := Commanded = 2;
    Forward := ActiveFirst;
    Reverse := ActiveSecond;

METHOD PUBLIC StartForward : BOOL
    StartForward := RequestOutput(Which := 1);
END_METHOD

METHOD PUBLIC StartReverse : BOOL
    StartReverse := RequestOutput(Which := 2);
END_METHOD
END_FUNCTION_BLOCK

(* Slow and fast. Changing speed while running drops both outputs for ChangeoverDelay. *)
FUNCTION_BLOCK FB_TwoSpeedMotor EXTENDS FB_TwoOutputMotor IMPLEMENTS ITwoSpeedDevice
VAR_INPUT
    AutoRun : BOOL;
    AutoFast : BOOL;
    SlowFeedback : BOOL;
    FastFeedback : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    SlowCommand : BOOL;
    FastCommand : BOOL;
    Fast : BOOL;
END_VAR
VAR PUBLIC RETAIN
    ChangeoverDelay : TIME := T#1s;
END_VAR
VAR_TEMP
    Wanted : INT;
END_VAR
    IF NOT AutoRun THEN
        Wanted := 0;
    ELSIF AutoFast THEN
        Wanted := 2;
    ELSE
        Wanted := 1;
    END_IF;
    ExecuteOutputs(AutoWanted := Wanted, FirstFeedback := SlowFeedback,
        SecondFeedback := FastFeedback, Delay := ChangeoverDelay, FullStop := FALSE);
    SlowCommand := Commanded = 1;
    FastCommand := Commanded = 2;
    Fast := ActiveSecond;

METHOD PUBLIC StartSlow : BOOL
    StartSlow := RequestOutput(Which := 1);
END_METHOD

METHOD PUBLIC StartFast : BOOL
    StartFast := RequestOutput(Which := 2);
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
