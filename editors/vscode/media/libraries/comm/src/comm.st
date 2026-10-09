(* The truST communication library (specification 42): the state of each link the runtime runs,
   for the program and the HMI. The runtime fills a Link's outputs at the start of every scan
   from the link its Name names. *)
NAMESPACE Comm

USING Alarm;

TYPE LinkState :
(
    Unknown := 0,
    Connecting := 1,
    Connected := 2,
    Degraded := 3,
    Failed := 4,
    Disabled := 5
);
END_TYPE

FUNCTION_BLOCK Link
VAR_INPUT
    Name : STRING[80];
    AlarmDelay : TIME := T#10s;
    (* FALSE switches an io.toml driver's link off; a rising edge of Test tests it once; one of
       Reset starts the counters again (specification 42, §4 items 6-8). *)
    Enable : BOOL := TRUE;
    Test : BOOL;
    Reset : BOOL;
END_VAR
VAR_OUTPUT
    State : LinkState := LinkState#Unknown;
    Connected : BOOL;
    Disabled : BOOL;
    Known : BOOL;
    StateTime : TIME;
    LastGoodAge : TIME;
    LastError : STRING[120];
    Exchanges : UDINT;
    Failures : UDINT;
    ResponseTime : TIME;
    TestResult : STRING[120];
    Detail : STRING[120];
END_VAR
VAR
    LinkFailed : DigitalAlarm;
END_VAR
    (* The runtime wrote the outputs at the start of the scan (specification 42, §4). A link that
       delivers no data, failed, unknown or still connecting, raises the alarm after its delay;
       one switched off on purpose does not. *)
    LinkFailed(Condition := NOT Connected AND State <> LinkState#Disabled, OnDelay := AlarmDelay,
        Message := CONCAT(Name, ' failed'), Priority := AlarmPriority#Medium);
END_FUNCTION_BLOCK

END_NAMESPACE
