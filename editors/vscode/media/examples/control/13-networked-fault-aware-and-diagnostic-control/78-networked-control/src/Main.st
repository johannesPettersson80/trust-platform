USING Control;

(* One remote command packet: the connection it belongs to (its epoch), its sequence number within that connection, the time
   it was sent in milliseconds on the clock that sender and receiver share, and the heater command. *)
TYPE CommandPacket :
STRUCT
    Epoch : UDINT;
    Sequence : UDINT;
    SentMs : UDINT;
    CommandPct : LREAL;
END_STRUCT
END_TYPE

(* The heater's rules for remote commands. A packet is accepted only if it belongs to the established connection, its
   sequence number is newer than the last accepted one (any number for the connection's first packet), its age, receive
   time minus send time, is 0–200 ms, and its command is 0–100 %. HoldMs is the time since the last accepted packet; more
   than 300 ms ends remote authority. Remote authority returns only after an acknowledgement, three packets accepted from
   then on, and a remote command within 0.1 point of the command applied. Times are whole milliseconds, so the 300 ms
   boundary is exact: in LREAL seconds 5.3 − 5.0 is 0.30000000000000071, which is more than 0.3. *)
FUNCTION_BLOCK CommandReceiver
VAR_INPUT
    NowMs : UDINT;
    Arrived : BOOL;
    Packet : CommandPacket;
    AppliedPct : LREAL;
    Acknowledge : BOOL;
END_VAR
VAR_OUTPUT
    RemoteAuthority : BOOL := FALSE;
    RemoteCommandPct : LREAL := 0.0;
    HoldMs : UDINT := 0;
    AgeMs : UDINT := 0;
    LastSequence : UDINT := 0;
    Fresh : UDINT := 0;
    RejectedOldSession : UDINT := 0;
    RejectedNotNewer : UDINT := 0;
    RejectedByAge : UDINT := 0;
    RejectedCommand : UDINT := 0;
END_VAR
VAR
    Epoch : UDINT := 0;
    HaveAccepted : BOOL := FALSE;
    LastReceiptMs : UDINT := 0;
    Recovering : BOOL := FALSE;
    Accepted : BOOL := FALSE;
    AcknowledgeBefore : BOOL := FALSE;
END_VAR

(* A new connection: its epoch, the time it is established and the command it starts from, with remote authority. *)
METHOD PUBLIC Establish
VAR_INPUT
    NewEpoch : UDINT;
    AtMs : UDINT;
    CommandPct : LREAL;
END_VAR
Epoch := NewEpoch;
HaveAccepted := FALSE;
LastSequence := UDINT#0;
LastReceiptMs := AtMs;
RemoteCommandPct := CommandPct;
RemoteAuthority := TRUE;
Recovering := FALSE;
Fresh := UDINT#0;
HoldMs := UDINT#0;
AgeMs := UDINT#0;
RejectedOldSession := UDINT#0;
RejectedNotNewer := UDINT#0;
RejectedByAge := UDINT#0;
RejectedCommand := UDINT#0;
END_METHOD

Accepted := FALSE;
AgeMs := UDINT#0;
IF Arrived THEN
    IF Packet.SentMs <= NowMs THEN
        AgeMs := NowMs - Packet.SentMs;
    END_IF;
    IF Packet.Epoch <> Epoch THEN
        RejectedOldSession := RejectedOldSession + UDINT#1;
    ELSIF HaveAccepted AND Packet.Sequence <= LastSequence THEN
        RejectedNotNewer := RejectedNotNewer + UDINT#1;
    ELSIF Packet.SentMs > NowMs OR AgeMs > UDINT#200 THEN
        RejectedByAge := RejectedByAge + UDINT#1;
    ELSIF NOT (Packet.CommandPct >= LREAL#0.0 AND Packet.CommandPct <= LREAL#100.0) THEN
        RejectedCommand := RejectedCommand + UDINT#1;
    ELSE
        HaveAccepted := TRUE;
        LastSequence := Packet.Sequence;
        LastReceiptMs := NowMs;
        RemoteCommandPct := Packet.CommandPct;
        Accepted := TRUE;
    END_IF;
END_IF;
HoldMs := NowMs - LastReceiptMs;
IF RemoteAuthority AND HoldMs > UDINT#300 THEN
    RemoteAuthority := FALSE;
    Recovering := FALSE;
END_IF;
IF NOT RemoteAuthority THEN
    IF Acknowledge AND NOT AcknowledgeBefore THEN
        Recovering := TRUE;
        Fresh := UDINT#0;
    END_IF;
    IF Recovering THEN
        IF HoldMs > UDINT#300 THEN
            Fresh := UDINT#0;
        ELSIF Accepted THEN
            Fresh := Fresh + UDINT#1;
        END_IF;
        IF Fresh >= UDINT#3 AND ABS(RemoteCommandPct - AppliedPct) <= LREAL#0.1 THEN
            RemoteAuthority := TRUE;
            Recovering := FALSE;
        END_IF;
    END_IF;
END_IF;
AcknowledgeBefore := Acknowledge;
END_FUNCTION_BLOCK

(* What every heater needs from the experiment: its first update, whether it runs, a latched fault, the updates since the
   start (100 ms each) and the acknowledgement time, read only at the start. *)
TYPE ExperimentState :
STRUCT
    Start : BOOL;
    Running : BOOL;
    Faulted : BOOL;
    Updates : DINT;
    AcknowledgeMs : UDINT;
END_STRUCT
END_TYPE

(* The results over the six heaters: passed cases, the highest temperature, the largest distance from 36 °C from 30 s, the
   largest command change at a transfer between remote and local control, and whether every heater started at rest. *)
TYPE NetworkResults :
STRUCT
    StepPassed : LREAL;
    HighestPeakC : LREAL;
    LargestStepErrorC : LREAL;
    LargestJumpPct : LREAL;
    StartedAtRest : BOOL;
END_STRUCT
END_TYPE

(* A temperature outside 0–80 °C. *)
FUNCTION OutsideBand : BOOL
VAR_INPUT
    TemperatureC : LREAL;
END_VAR
OutsideBand := TemperatureC < LREAL#0.0 OR TemperatureC > LREAL#80.0;
END_FUNCTION

(* One heater under remote control over a network, with a local Control.PID as fallback. The receiver is the part to reuse;
   the remote controller and the network are simulated here.
   Remote side: RemoteLoop, a Control.PID, sends its command every update as packet number Updates, sent at Updates·100 ms, in
   connection 7. While the receiver last reported remote authority it controls; otherwise it tracks the command applied,
   which it receives in the heater's status.
   Network: normally each packet arrives one update after it was sent, 100 ms old. After 5 s each case's network misbehaves:
   0 dropped: packet 50 is lost;
   1 reordered: packet 51 arrives at 5.1 s and packet 50 only at 5.2 s;
   2 late: packet 50 arrives at 5.3 s; packets 51 and 52 are lost;
   3 old session: at 5.1 s a packet of connection 6 (sequence 4000, sent at 5.0 s, 0 %) arrives instead of packet 50;
   4 outage and 5 outage with recovery: packets 50–58 are lost. Case 5's operator acknowledges at the experiment setting.
   Heater side: LocalLoop, a Control.PID, tracks the applied command while the remote controller has authority and controls
   when it does not. The record: the highest temperature from 0 s to 60 s, the largest distance from 36 °C from 30 s to 60 s,
   and the largest command change at a transfer. At 60 s the case passes if it started at rest, peaked at most at 37 °C and
   stayed within 0.2 °C from 30 s. *)
FUNCTION_BLOCK NetworkedHeater
VAR_INPUT
    Run : ExperimentState;
    TemperatureC : LREAL;
    Scenario : INT;
END_VAR
VAR_IN_OUT
    Results : NetworkResults;
END_VAR
VAR_OUTPUT
    CommandPct : LREAL := 75.0;
    RemoteAuthority : BOOL := TRUE;
    HoldS : LREAL := 0.0;
    AgeS : LREAL := 0.0;
    Fresh : LREAL := 0.0;
    RejectedOldSession : LREAL := 0.0;
    RejectedNotNewer : LREAL := 0.0;
    RejectedByAge : LREAL := 0.0;
END_VAR
VAR
    RemoteLoop : Control.PID;
    LocalLoop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
        IntegralGain := LREAL#0.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Rest : Control.PIDInitialState;
    RemoteInput : Control.PIDInput;
    LocalInput : Control.PIDInput;
    Receiver : CommandReceiver;
    Sent : CommandPacket;
    Delivered : CommandPacket;
    History : ARRAY[0..3] OF CommandPacket;
    Arrives : BOOL := FALSE;
    Update : UDINT := 0;
    NowMs : UDINT := 0;
    LocalPct : LREAL := 75.0;
    PreviousPct : LREAL := 75.0;
    WasRemote : BOOL := TRUE;
    PeakC : LREAL := 35.0;
    LateErrorC : LREAL := 0.0;
    JumpPct : LREAL := 0.0;
    AtRest : BOOL := FALSE;
    Passed : BOOL := FALSE;
END_VAR

(* Both loops configured and initialized at the rest state (setpoint 35 °C, the current temperature, 75 %), and connection 7
   established from 75 % with remote authority. *)
METHOD PRIVATE ResetToRest
RemoteLoop.Configure(Parameters := Parameters);
LocalLoop.Configure(Parameters := Parameters);
Rest.Setpoint := LREAL#35.0;
Rest.ProcessValue := TemperatureC;
Rest.NominalOutput := LREAL#75.0;
Rest.SaturatedOutput := LREAL#75.0;
RemoteLoop.Initialize(State := Rest);
LocalLoop.Initialize(State := Rest);
Receiver.Establish(NewEpoch := UDINT#7, AtMs := UDINT#0, CommandPct := LREAL#75.0);
END_METHOD

IF Run.Faulted THEN
    CommandPct := LREAL#0.0;
    AgeS := LREAL#0.0;
ELSIF Run.Running THEN
    IF Run.Start THEN
        ResetToRest();
        PreviousPct := LREAL#75.0;
        WasRemote := TRUE;
        AtRest := ABS(TemperatureC - LREAL#35.0) <= LREAL#0.01;
        PeakC := TemperatureC;
        LateErrorC := LREAL#0.0;
        JumpPct := LREAL#0.0;
        Passed := FALSE;
    END_IF;
    Update := DINT_TO_UDINT(Run.Updates);
    NowMs := Update * UDINT#100;

    (* Remote side: the packet sent in this update. *)
    RemoteInput.Setpoint := LREAL#36.0;
    RemoteInput.ProcessValue := TemperatureC;
    RemoteInput.TrackingOutput := PreviousPct;
    RemoteInput.CycleTime := LREAL#0.1;
    IF Receiver.RemoteAuthority THEN
        RemoteInput.Mode := PIDMode#Auto;
    ELSE
        RemoteInput.Mode := PIDMode#Track;
    END_IF;
    Sent.Epoch := UDINT#7;
    Sent.Sequence := Update;
    Sent.SentMs := NowMs;
    Sent.CommandPct := RemoteLoop.Update(Input := RemoteInput);
    History[Update MOD UDINT#4] := Sent;

    (* Network: what arrives in this update. *)
    Arrives := Update >= UDINT#1;
    IF Arrives THEN
        Delivered := History[(Update - UDINT#1) MOD UDINT#4];
    END_IF;
    CASE Scenario OF
        0:
            IF Update = UDINT#51 THEN
                Arrives := FALSE;
            END_IF;
        1:
            IF Update = UDINT#51 THEN
                Delivered := History[51 MOD 4];
            ELSIF Update = UDINT#52 THEN
                Delivered := History[50 MOD 4];
            END_IF;
        2:
            IF Update = UDINT#51 OR Update = UDINT#52 THEN
                Arrives := FALSE;
            ELSIF Update = UDINT#53 THEN
                Delivered := History[50 MOD 4];
            END_IF;
        3:
            IF Update = UDINT#51 THEN
                Delivered.Epoch := UDINT#6;
                Delivered.Sequence := UDINT#4000;
                Delivered.SentMs := UDINT#5000;
                Delivered.CommandPct := LREAL#0.0;
            END_IF;
        4, 5:
            IF Update >= UDINT#51 AND Update <= UDINT#59 THEN
                Arrives := FALSE;
            END_IF;
    END_CASE;

    (* Heater side. *)
    Receiver(NowMs := NowMs, Arrived := Arrives, Packet := Delivered, AppliedPct := PreviousPct,
        Acknowledge := Scenario = 5 AND NowMs = Run.AcknowledgeMs);
    LocalInput.Setpoint := LREAL#36.0;
    LocalInput.ProcessValue := TemperatureC;
    LocalInput.TrackingOutput := PreviousPct;
    LocalInput.CycleTime := LREAL#0.1;
    IF Receiver.RemoteAuthority THEN
        LocalInput.Mode := PIDMode#Track;
    ELSE
        LocalInput.Mode := PIDMode#Auto;
    END_IF;
    LocalPct := LocalLoop.Update(Input := LocalInput);
    IF Receiver.RemoteAuthority THEN
        CommandPct := Receiver.RemoteCommandPct;
    ELSE
        CommandPct := LocalPct;
    END_IF;
    IF Receiver.RemoteAuthority <> WasRemote THEN
        JumpPct := MAX(JumpPct, ABS(CommandPct - PreviousPct));
    END_IF;
    WasRemote := Receiver.RemoteAuthority;
    PreviousPct := CommandPct;
    IF Arrives THEN
        AgeS := UDINT_TO_LREAL(Receiver.AgeMs) / LREAL#1000.0;
    ELSE
        AgeS := LREAL#0.0;
    END_IF;

    IF Run.Updates <= 600 THEN
        PeakC := MAX(PeakC, TemperatureC);
        IF Run.Updates >= 300 THEN
            LateErrorC := MAX(LateErrorC, ABS(LREAL#36.0 - TemperatureC));
        END_IF;
        IF Run.Updates = 600 THEN
            Passed := AtRest AND PeakC <= LREAL#37.0 AND LateErrorC <= LREAL#0.2;
        END_IF;
    END_IF;
ELSE
    (* At rest 75 % holds 35 °C, and both loops and the connection wait at the rest state. *)
    ResetToRest();
    CommandPct := LREAL#75.0;
    AgeS := LREAL#0.0;
END_IF;
RemoteAuthority := Receiver.RemoteAuthority;
HoldS := UDINT_TO_LREAL(Receiver.HoldMs) / LREAL#1000.0;
Fresh := UDINT_TO_LREAL(Receiver.Fresh);
RejectedOldSession := UDINT_TO_LREAL(Receiver.RejectedOldSession);
RejectedNotNewer := UDINT_TO_LREAL(Receiver.RejectedNotNewer);
RejectedByAge := UDINT_TO_LREAL(Receiver.RejectedByAge);
Results.HighestPeakC := MAX(Results.HighestPeakC, PeakC);
Results.LargestStepErrorC := MAX(Results.LargestStepErrorC, LateErrorC);
Results.LargestJumpPct := MAX(Results.LargestJumpPct, JumpPct);
IF Passed THEN
    Results.StepPassed := Results.StepPassed + LREAL#1.0;
END_IF;
Results.StartedAtRest := Results.StartedAtRest AND AtRest;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Six of Lesson 13's heaters, 10 kW/°C with 1 s delivery delay, each under a remote Control.PID over its own network
       and a local Control.PID as fallback. All take the setpoint step from 35 to 36 °C; each network misbehaves after 5 s.
       A temperature outside 0–80 °C or an update interval outside 100 ms ± 10 ms latches zero heat. *)
    DroppedC, ReorderedC, LateC, OldSessionC, OutageC, RecoveryC : LREAL := 35.0;
    DroppedPct, ReorderedPct, LatePct, OldSessionPct, OutagePct, RecoveryPct : LREAL := 75.0;
    (* When case 5's operator acknowledges the outage, a Control Studio experiment setting frozen at the start. *)
    AcknowledgementTime : LREAL := 8.0;
    Cases : ARRAY[0..5] OF NetworkedHeater;
    Temperatures : ARRAY[0..5] OF LREAL;
    Commands : ARRAY[0..5] OF LREAL;
    K : DINT;
    Run : ExperimentState;
    Results : NetworkResults;
    NoResults : NetworkResults := (StepPassed := LREAL#0.0, HighestPeakC := LREAL#0.0, LargestStepErrorC := LREAL#0.0,
        LargestJumpPct := LREAL#0.0, StartedAtRest := TRUE);
    Updates : DINT := 0;
    (* The published results: the setpoint, the hold times and rejections of the cases that show them, the acknowledged
       case's fresh packets, the late case's packet age, and the results over all six. *)
    SetpointC : LREAL := 35.0;
    DroppedHoldS : LREAL := 0.0;
    LateHoldS : LREAL := 0.0;
    OutageHoldS : LREAL := 0.0;
    ReorderedRejected : LREAL := 0.0;
    LateRejected : LREAL := 0.0;
    OldSessionRejected : LREAL := 0.0;
    StepPassed : LREAL := 0.0;
    LargestJumpPct : LREAL := 0.0;
    HighestPeakC : LREAL := 35.0;
    LargestStepErrorC : LREAL := 0.0;
    RecoveryFresh : LREAL := 0.0;
    LateAgeS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Start : BOOL := FALSE;
    StartedAtRest : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    OutageRemote : BOOL := FALSE;
    RecoveryRemote : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#90ms AND (NowTime - PreviousTime) <= T#110ms);
Temperatures[0] := DroppedC;
Temperatures[1] := ReorderedC;
Temperatures[2] := LateC;
Temperatures[3] := OldSessionC;
Temperatures[4] := OutageC;
Temperatures[5] := RecoveryC;
IF NOT ClockValid THEN
    FaultLatched := TRUE;
END_IF;
FOR K := 0 TO 5 DO
    IF OutsideBand(TemperatureC := Temperatures[K]) THEN
        FaultLatched := TRUE;
    END_IF;
END_FOR;
Start := ExperimentEnabled AND NOT WasEnabled;
IF Start THEN
    Updates := 0;
    IF AcknowledgementTime >= LREAL#0.0 AND AcknowledgementTime <= LREAL#60.0 THEN
        Run.AcknowledgeMs := LREAL_TO_UDINT(AcknowledgementTime * LREAL#1000.0);
    ELSE
        Run.AcknowledgeMs := UDINT#16#FFFFFFFF;
    END_IF;
ELSIF ExperimentEnabled THEN
    Updates := Updates + 1;
END_IF;
Run.Start := Start;
Run.Running := ExperimentEnabled AND NOT FaultLatched;
Run.Faulted := FaultLatched;
Run.Updates := Updates;
IF Run.Running THEN
    SetpointC := LREAL#36.0;
ELSE
    SetpointC := LREAL#35.0;
END_IF;
Results := NoResults;
FOR K := 0 TO 5 DO
    Cases[K](Run := Run, TemperatureC := Temperatures[K], Scenario := DINT_TO_INT(K), Results := Results,
        CommandPct => Commands[K]);
END_FOR;
DroppedPct := Commands[0];
ReorderedPct := Commands[1];
LatePct := Commands[2];
OldSessionPct := Commands[3];
OutagePct := Commands[4];
RecoveryPct := Commands[5];
DroppedHoldS := Cases[0].HoldS;
LateHoldS := Cases[2].HoldS;
OutageHoldS := Cases[4].HoldS;
ReorderedRejected := Cases[1].RejectedNotNewer;
LateRejected := Cases[2].RejectedByAge;
OldSessionRejected := Cases[3].RejectedOldSession;
LateAgeS := Cases[2].AgeS;
OutageRemote := Cases[4].RemoteAuthority;
RecoveryRemote := Cases[5].RemoteAuthority;
RecoveryFresh := Cases[5].Fresh;
StepPassed := Results.StepPassed;
LargestJumpPct := Results.LargestJumpPct;
HighestPeakC := Results.HighestPeakC;
LargestStepErrorC := Results.LargestStepErrorC;
StartedAtRest := Results.StartedAtRest;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.DroppedC AT %IL0 : LREAL;
    P1.ReorderedC AT %IL8 : LREAL;
    P1.LateC AT %IL16 : LREAL;
    P1.OldSessionC AT %IL24 : LREAL;
    P1.OutageC AT %IL32 : LREAL;
    P1.RecoveryC AT %IL40 : LREAL;
    P1.DroppedPct AT %QL0 : LREAL;
    P1.ReorderedPct AT %QL8 : LREAL;
    P1.LatePct AT %QL16 : LREAL;
    P1.OldSessionPct AT %QL24 : LREAL;
    P1.OutagePct AT %QL32 : LREAL;
    P1.RecoveryPct AT %QL40 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.DroppedHoldS AT %ML8 : LREAL;
    P1.LateHoldS AT %ML16 : LREAL;
    P1.OutageHoldS AT %ML24 : LREAL;
    P1.ReorderedRejected AT %ML32 : LREAL;
    P1.LateRejected AT %ML40 : LREAL;
    P1.OldSessionRejected AT %ML48 : LREAL;
    P1.StepPassed AT %ML56 : LREAL;
    P1.LargestJumpPct AT %ML64 : LREAL;
    P1.HighestPeakC AT %ML72 : LREAL;
    P1.LargestStepErrorC AT %ML80 : LREAL;
    P1.RecoveryFresh AT %ML88 : LREAL;
    P1.LateAgeS AT %ML96 : LREAL;
    P1.ExperimentEnabled AT %MX112.0 : BOOL;
    P1.StartedAtRest AT %MX112.1 : BOOL;
    P1.FaultLatched AT %MX112.2 : BOOL;
    P1.ClockValid AT %MX112.3 : BOOL;
    P1.OutageRemote AT %MX112.4 : BOOL;
    P1.RecoveryRemote AT %MX112.5 : BOOL;
END_VAR
END_CONFIGURATION
