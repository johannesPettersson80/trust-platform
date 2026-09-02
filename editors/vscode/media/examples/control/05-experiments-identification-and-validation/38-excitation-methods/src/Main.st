PROGRAM Main
VAR
    PlantSelection : INT := 0;
    MethodSelection : INT := 1;
    StartExperiment : BOOL := FALSE;
    ResetExperiment : BOOL := FALSE;
    ExperimentRunning : BOOL := FALSE;
    MethodEligible : BOOL := FALSE;
    ExperimentComplete : BOOL := FALSE;
    RejectReason : INT := 0;
    HeaterRequestedCommand : LREAL := LREAL#20.0;
    HeaterAppliedCommand : LREAL := LREAL#20.0;
    ServoRequestedTorque : LREAL := LREAL#0.0;
    ServoAppliedTorque : LREAL := LREAL#0.0;
    InstantFrequencyHz : LREAL := LREAL#0.0;
    HeaterTemperature : LREAL := LREAL#24.0;
    ServoPosition : LREAL := LREAL#0.0;
    AmplitudeScale : LREAL := LREAL#1.0;
    RequestedDuration : LREAL := LREAL#30.0;
    ChirpStartHz : LREAL := LREAL#0.2;
    ChirpEndHz : LREAL := LREAL#2.0;
    State : INT := 0;
    ScanInRun : DINT := 0;
    ElapsedSeconds : LREAL := LREAL#0.0;
    SweepElapsed : LREAL := LREAL#0.0;
    ChirpPhase : LREAL := LREAL#0.0;
    NormalizedExcitation : LREAL := LREAL#0.0;
    PatternIndex : INT := 0;
END_VAR

HeaterRequestedCommand := LREAL#20.0;
ServoRequestedTorque := LREAL#0.0;
NormalizedExcitation := LREAL#0.0;
InstantFrequencyHz := LREAL#0.0;

IF ResetExperiment THEN
    State := 0;
    ScanInRun := 0;
    ElapsedSeconds := LREAL#0.0;
    ChirpPhase := LREAL#0.0;
    ExperimentRunning := FALSE;
    ExperimentComplete := FALSE;
    MethodEligible := FALSE;
    RejectReason := 0;
END_IF;

IF (State = 0) AND StartExperiment AND NOT ResetExperiment THEN
    MethodEligible := FALSE;
    RejectReason := 0;

    IF (AmplitudeScale <= LREAL#0.0) OR (AmplitudeScale > LREAL#1.0) THEN
        RejectReason := 1;
    ELSIF RequestedDuration < LREAL#30.0 THEN
        RejectReason := 2;
    ELSIF (MethodSelection = 6) AND
          ((ChirpStartHz <= LREAL#0.0) OR (ChirpEndHz <= ChirpStartHz) OR
           (ChirpEndHz > LREAL#2.0)) THEN
        RejectReason := 3;
    ELSIF MethodSelection = 4 THEN
        (* Relay feedback needs the separate limit-cycle safeguards in Lesson 43. *)
        RejectReason := 4;
    ELSIF ((PlantSelection = 0) AND
           ((MethodSelection = 1) OR (MethodSelection = 2) OR
            (MethodSelection = 5) OR (MethodSelection = 6))) OR
          ((PlantSelection = 1) AND
           ((MethodSelection = 2) OR (MethodSelection = 3) OR
            (MethodSelection = 5) OR (MethodSelection = 6))) THEN
        MethodEligible := TRUE;
        State := 1;
        ExperimentRunning := TRUE;
        ExperimentComplete := FALSE;
        ScanInRun := 0;
        ChirpPhase := LREAL#0.0;
    ELSE
        RejectReason := 5;
    END_IF;
END_IF;

IF State = 1 THEN
    ElapsedSeconds := DINT_TO_LREAL(ScanInRun) * LREAL#0.1;

    CASE MethodSelection OF
        1:
            IF (ElapsedSeconds >= LREAL#5.0) AND (ElapsedSeconds < LREAL#25.0) THEN
                NormalizedExcitation := LREAL#1.0;
            END_IF;
        2:
            IF (ElapsedSeconds >= LREAL#5.0) AND (ElapsedSeconds < LREAL#10.0) THEN
                NormalizedExcitation := LREAL#1.0;
            END_IF;
        3:
            IF (ElapsedSeconds >= LREAL#5.0) AND (ElapsedSeconds < LREAL#10.0) THEN
                NormalizedExcitation := LREAL#1.0;
            ELSIF (ElapsedSeconds >= LREAL#10.0) AND (ElapsedSeconds < LREAL#15.0) THEN
                NormalizedExcitation := LREAL#-1.0;
            END_IF;
        5:
            IF (ElapsedSeconds >= LREAL#5.0) AND (ElapsedSeconds < LREAL#25.0) THEN
                IF PlantSelection = 0 THEN
                    (* Twenty-five scans give the slow heater a 2.5 s dwell. *)
                    PatternIndex := DINT_TO_INT((ScanInRun - 50) / 25) MOD 15;
                ELSE
                    PatternIndex := DINT_TO_INT((ScanInRun - 50) / 3) MOD 15;
                END_IF;
                CASE PatternIndex OF
                    0, 1, 4, 6, 7, 10, 13: NormalizedExcitation := LREAL#1.0;
                ELSE NormalizedExcitation := LREAL#-1.0;
                END_CASE;
            END_IF;
        6:
            IF (ElapsedSeconds >= LREAL#5.0) AND (ElapsedSeconds < LREAL#25.0) THEN
                SweepElapsed := ElapsedSeconds - LREAL#5.0;
                InstantFrequencyHz := ChirpStartHz +
                    (ChirpEndHz - ChirpStartHz) * SweepElapsed / LREAL#20.0;
                ChirpPhase := ChirpPhase +
                    LREAL#6.283185307179586 * InstantFrequencyHz * LREAL#0.1;
                NormalizedExcitation := SIN(ChirpPhase);
            END_IF;
    END_CASE;

    IF PlantSelection = 0 THEN
        HeaterRequestedCommand := LREAL#20.0 +
            LREAL#5.0 * AmplitudeScale * NormalizedExcitation;
    ELSE
        ServoRequestedTorque := LREAL#0.1 * AmplitudeScale * NormalizedExcitation;
    END_IF;

    ScanInRun := ScanInRun + 1;
    IF ElapsedSeconds >= RequestedDuration THEN
        State := 2;
        ExperimentRunning := FALSE;
        ExperimentComplete := TRUE;
        HeaterRequestedCommand := LREAL#20.0;
        ServoRequestedTorque := LREAL#0.0;
    END_IF;
END_IF;

IF HeaterRequestedCommand < LREAL#0.0 THEN
    HeaterAppliedCommand := LREAL#0.0;
ELSIF HeaterRequestedCommand > LREAL#100.0 THEN
    HeaterAppliedCommand := LREAL#100.0;
ELSE
    HeaterAppliedCommand := HeaterRequestedCommand;
END_IF;

IF ServoRequestedTorque < LREAL#-0.2 THEN
    ServoAppliedTorque := LREAL#-0.2;
ELSIF ServoRequestedTorque > LREAL#0.2 THEN
    ServoAppliedTorque := LREAL#0.2;
ELSE
    ServoAppliedTorque := ServoRequestedTorque;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PlantSelection AT %MW0 : INT;
    P1.MethodSelection AT %MW2 : INT;
    P1.StartExperiment AT %MX4.0 : BOOL;
    P1.ResetExperiment AT %MX4.1 : BOOL;
    P1.ExperimentRunning AT %MX4.2 : BOOL;
    P1.MethodEligible AT %MX4.3 : BOOL;
    P1.ExperimentComplete AT %MX4.4 : BOOL;
    P1.RejectReason AT %MW6 : INT;
    P1.HeaterRequestedCommand AT %ML8 : LREAL;
    P1.HeaterAppliedCommand AT %QL0 : LREAL;
    P1.ServoRequestedTorque AT %ML16 : LREAL;
    P1.ServoAppliedTorque AT %QL8 : LREAL;
    P1.InstantFrequencyHz AT %ML24 : LREAL;
    P1.HeaterTemperature AT %IL0 : LREAL;
    P1.ServoPosition AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
