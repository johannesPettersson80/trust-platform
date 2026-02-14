PROGRAM ProcessPid
VAR
    StartCmd : BOOL := FALSE;
    StopCmd : BOOL := FALSE;
    PressureSpikeCmd : BOOL := FALSE;
    BypassCmd : BOOL := FALSE;

    PumpRunning : BOOL := FALSE;
    ValveOpen : BOOL := FALSE;
    BypassOpen : BOOL := FALSE;

    ValvePosition : REAL := REAL#0.0;

    FeedLevelPct : REAL := REAL#72.0;
    ProductLevelPct : REAL := REAL#22.0;

    MainFlowM3h : REAL := REAL#0.0;
    BypassFlowM3h : REAL := REAL#0.0;
    TotalFlowM3h : REAL := REAL#0.0;
    PressureBar : REAL := REAL#1.2;

    FlowSetpointM3h : REAL := REAL#0.0;
    PressureSetpointBar : REAL := REAL#3.8;
    FeedLevelSetpointPct : REAL := REAL#60.0;
    ProductLevelSetpointPct : REAL := REAL#55.0;

    FlowDeviationM3h : REAL := REAL#0.0;
    PressureDeviationBar : REAL := REAL#0.0;

    HighPressureAlarm : BOOL := FALSE;
    LowFeedAlarm : BOOL := FALSE;
    HighProductAlarm : BOOL := FALSE;
    AnyAlarmActive : BOOL := FALSE;
    AlarmActiveCount : INT := 0;

    ModeText : STRING[80] := 'STOPPED';
    AlarmMessage : STRING[96] := 'STOPPED: WAITING FOR START';

    ScanTick : UDINT := UDINT#0;
END_VAR

ScanTick := ScanTick + UDINT#1;

IF StopCmd THEN
    PumpRunning := FALSE;
ELSIF StartCmd THEN
    PumpRunning := TRUE;
END_IF;

ValveOpen := PumpRunning;
BypassOpen := PumpRunning AND BypassCmd;

IF ValveOpen THEN
    IF BypassOpen THEN
        IF ValvePosition > REAL#68.0 THEN
            ValvePosition := ValvePosition - REAL#3.0;
        ELSIF ValvePosition < REAL#68.0 THEN
            ValvePosition := ValvePosition + REAL#1.0;
        END_IF;
    ELSE
        IF ValvePosition < REAL#100.0 THEN
            ValvePosition := ValvePosition + REAL#2.0;
        END_IF;
    END_IF;
ELSE
    IF ValvePosition > REAL#0.0 THEN
        ValvePosition := ValvePosition - REAL#4.0;
    END_IF;
END_IF;

IF ValvePosition < REAL#0.0 THEN
    ValvePosition := REAL#0.0;
ELSIF ValvePosition > REAL#100.0 THEN
    ValvePosition := REAL#100.0;
END_IF;

MainFlowM3h := ValvePosition * REAL#1.45;
IF BypassOpen THEN
    BypassFlowM3h := ValvePosition * REAL#0.55;
ELSE
    BypassFlowM3h := REAL#0.0;
END_IF;
TotalFlowM3h := MainFlowM3h + BypassFlowM3h;

IF PumpRunning THEN
    IF BypassOpen THEN
        FlowSetpointM3h := REAL#92.0;
        PressureSetpointBar := REAL#3.6;
    ELSE
        FlowSetpointM3h := REAL#128.0;
        PressureSetpointBar := REAL#4.6;
    END_IF;
ELSE
    FlowSetpointM3h := REAL#0.0;
    PressureSetpointBar := REAL#1.5;
END_IF;

IF PumpRunning THEN
    FeedLevelPct := FeedLevelPct - (TotalFlowM3h * REAL#0.0018);
    ProductLevelPct := ProductLevelPct + (TotalFlowM3h * REAL#0.0015);
    PressureBar := REAL#1.4 + (ValvePosition * REAL#0.036);
    IF BypassOpen THEN
        PressureBar := PressureBar - REAL#0.9;
    END_IF;
ELSE
    FeedLevelPct := FeedLevelPct + REAL#0.08;
    ProductLevelPct := ProductLevelPct - REAL#0.03;
    PressureBar := REAL#1.1 + (ValvePosition * REAL#0.004);
END_IF;

IF PressureSpikeCmd THEN
    PressureBar := PressureBar + REAL#3.8;
END_IF;

IF FeedLevelPct < REAL#0.0 THEN
    FeedLevelPct := REAL#0.0;
ELSIF FeedLevelPct > REAL#100.0 THEN
    FeedLevelPct := REAL#100.0;
END_IF;

IF ProductLevelPct < REAL#0.0 THEN
    ProductLevelPct := REAL#0.0;
ELSIF ProductLevelPct > REAL#100.0 THEN
    ProductLevelPct := REAL#100.0;
END_IF;

IF PressureBar < REAL#0.0 THEN
    PressureBar := REAL#0.0;
END_IF;

FlowDeviationM3h := TotalFlowM3h - FlowSetpointM3h;
PressureDeviationBar := PressureBar - PressureSetpointBar;

HighPressureAlarm := PressureBar >= (PressureSetpointBar + REAL#1.5);
LowFeedAlarm := FeedLevelPct <= (FeedLevelSetpointPct - REAL#35.0);
HighProductAlarm := ProductLevelPct >= (ProductLevelSetpointPct + REAL#35.0);

AnyAlarmActive := HighPressureAlarm OR LowFeedAlarm OR HighProductAlarm;

AlarmActiveCount := INT#0;
IF HighPressureAlarm THEN
    AlarmActiveCount := AlarmActiveCount + INT#1;
END_IF;
IF LowFeedAlarm THEN
    AlarmActiveCount := AlarmActiveCount + INT#1;
END_IF;
IF HighProductAlarm THEN
    AlarmActiveCount := AlarmActiveCount + INT#1;
END_IF;

IF PumpRunning THEN
    IF BypassOpen THEN
        ModeText := 'RUNNING: BYPASS MODE';
    ELSE
        ModeText := 'RUNNING: NORMAL MODE';
    END_IF;
ELSE
    ModeText := 'STOPPED';
END_IF;

IF HighPressureAlarm THEN
    AlarmMessage := 'ALARM: PT-001 HIGH PRESSURE';
ELSIF LowFeedAlarm THEN
    AlarmMessage := 'ALARM: LT-001 LOW FEED LEVEL';
ELSIF HighProductAlarm THEN
    AlarmMessage := 'ALARM: LT-002 HIGH PRODUCT LEVEL';
ELSIF PumpRunning THEN
    AlarmMessage := 'OK: TRANSFER RUNNING';
ELSE
    AlarmMessage := 'STOPPED: WAITING FOR START';
END_IF;
END_PROGRAM
