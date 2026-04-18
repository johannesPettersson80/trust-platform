(* OSCAT BASIC 3.33 signal-generator helpers. *)

FUNCTION_BLOCK _RMP_B
VAR_INPUT
    DIR : BOOL;
    E : BOOL := TRUE;
    TR : TIME;
END_VAR
VAR_IN_OUT
    RMP : BYTE;
END_VAR
VAR
    TxMs : UDINT;
    LastMs : UDINT;
    LastTr : TIME;
    Init : BOOL;
    LastDir : BOOL;
    Start : BYTE;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());

IF E AND Init AND (DIR = LastDir) AND (RMP <> SEL(G := DIR, IN0 := BYTE#0, IN1 := BYTE#255)) AND TR = LastTr THEN
    RMP := FRMP_B(START := Start, DIR := DIR, TD := DWORD_TO_TIME(UDINT_TO_DWORD(TxMs - LastMs)), TR := TR);
ELSE
    Init := TRUE;
    LastMs := TxMs;
    LastTr := TR;
    Start := RMP;
END_IF;
LastDir := DIR;
END_FUNCTION_BLOCK

FUNCTION_BLOCK _RMP_NEXT
VAR_INPUT
    E : BOOL := TRUE;
    IN : BYTE;
    TR : TIME;
    TF : TIME;
    TL : TIME;
END_VAR
VAR_OUTPUT
    DIR : BOOL;
    UP : BOOL;
    DN : BOOL;
END_VAR
VAR_IN_OUT
    OUT : BYTE;
END_VAR
VAR
    Ramp : _RMP_B;
    DirectionTrend : TREND_DW;
    Lockout : TP;
    EnableRamp : BOOL;
    RampDir : BOOL;
END_VAR

DirectionTrend(X := BYTE_TO_DWORD(IN := IN));
Lockout(IN := FALSE, PT := TL);

IF DirectionTrend.TU AND (OUT < IN) THEN
    IF NOT RampDir AND EnableRamp THEN
        Lockout(IN := TRUE);
    END_IF;
    EnableRamp := TRUE;
    RampDir := TRUE;
ELSIF DirectionTrend.TD AND (OUT > IN) THEN
    IF RampDir AND EnableRamp THEN
        Lockout(IN := TRUE);
    END_IF;
    EnableRamp := TRUE;
    RampDir := FALSE;
ELSIF EnableRamp THEN
    IF (RampDir AND (OUT >= IN)) OR (NOT RampDir AND (OUT <= IN)) THEN
        EnableRamp := FALSE;
        IF TL > T#0s THEN
            Lockout(IN := TRUE);
        END_IF;
    END_IF;
END_IF;

IF NOT Lockout.Q AND EnableRamp THEN
    UP := RampDir;
    DIR := RampDir;
    DN := NOT RampDir;
ELSE
    UP := FALSE;
    DN := FALSE;
END_IF;

Ramp(RMP := OUT, E := E AND (UP OR DN), DIR := DIR, TR := SEL(G := DIR, IN0 := TF, IN1 := TR));
END_FUNCTION_BLOCK

FUNCTION_BLOCK _RMP_W
VAR_INPUT
    DIR : BOOL;
    E : BOOL := TRUE;
    TR : TIME;
END_VAR
VAR_IN_OUT
    RMP : WORD;
END_VAR
VAR
    Tx : DWORD;
    LastTime : DWORD;
    StepValue : DINT;
    Init : BOOL;
    LastDir : BOOL;
END_VAR

Tx := T_PLC_MS();
IF E AND Init THEN
    IF DIR XOR LastDir THEN
        LastTime := Tx;
        LastDir := DIR;
    END_IF;
    IF TR > T#0s THEN
        StepValue := REAL_TO_DINT(UDINT_TO_REAL(DWORD_TO_UDINT(Tx) - DWORD_TO_UDINT(LastTime)) * 65536.0 / UDINT_TO_REAL(DWORD_TO_UDINT(TIME_TO_DWORD(TR))));
    ELSE
        StepValue := DINT#65535;
    END_IF;
    IF StepValue > 0 THEN
        LastTime := Tx;
        IF NOT DIR THEN
            StepValue := -StepValue;
        END_IF;
        RMP := DINT_TO_WORD(LIMIT(MN := DINT#0, IN := WORD_TO_DINT(RMP) + StepValue, MX := DINT#65535));
    END_IF;
ELSE
    LastTime := Tx;
    Init := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_PULSE
VAR_INPUT
    ENQ : BOOL := TRUE;
    PTH : TIME;
    PTL : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    TxMs : UDINT;
    NextMs : UDINT;
    Init : BOOL;
END_VAR

IF ENQ THEN
    TxMs := DWORD_TO_UDINT(T_PLC_MS());
    IF NOT Init THEN
        Init := TRUE;
        NextMs := TxMs;
    END_IF;
    IF TxMs - NextMs >= DWORD_TO_UDINT(TIME_TO_DWORD(SEL(G := Q, IN0 := PTH, IN1 := PTL))) THEN
        NextMs := NextMs + DWORD_TO_UDINT(TIME_TO_DWORD(SEL(G := Q, IN0 := PTH, IN1 := PTL)));
        Q := NOT Q;
    END_IF;
ELSE
    Q := FALSE;
    Init := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_PW2
VAR_INPUT
    ENQ : BOOL;
    TH1 : TIME;
    TL1 : TIME;
    TH2 : TIME;
    TL2 : TIME;
    TS : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    TH : TIME;
    TL : TIME;
END_VAR
VAR
    HighTime : TIME;
    LowTime : TIME;
    TxMs : UDINT;
    StartMs : UDINT;
    Init : BOOL;
    ElapsedMs : UDINT;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());

IF NOT Init THEN
    StartMs := TxMs;
    Init := TRUE;
    TH := T#0s;
    TL := T#0s;
END_IF;

IF TS THEN
    HighTime := TH2;
    LowTime := TL2;
ELSE
    HighTime := TH1;
    LowTime := TL1;
END_IF;

IF ENQ THEN
    ElapsedMs := TxMs - StartMs;
    IF NOT Q THEN
        IF ElapsedMs >= DWORD_TO_UDINT(TIME_TO_DWORD(LowTime)) THEN
            Q := TRUE;
            StartMs := TxMs;
            TL := T#0s;
        ELSE
            TL := DWORD_TO_TIME(UDINT_TO_DWORD(ElapsedMs));
        END_IF;
    ELSE
        IF ElapsedMs >= DWORD_TO_UDINT(TIME_TO_DWORD(HighTime)) THEN
            Q := FALSE;
            StartMs := TxMs;
            TH := T#0s;
        ELSE
            TH := DWORD_TO_TIME(UDINT_TO_DWORD(ElapsedMs));
        END_IF;
    END_IF;
ELSE
    Q := FALSE;
    TH := T#0s;
    TL := T#0s;
    StartMs := TxMs;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_RDM
VAR_INPUT
    PT : TIME;
    AM : REAL := 1.0;
    OS : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    OUT : REAL;
END_VAR
VAR
    TxMs : UDINT;
    LastMs : UDINT;
    ElapsedMs : UDINT;
    Init : BOOL;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());

IF NOT Init THEN
    Init := TRUE;
    LastMs := TxMs;
    ElapsedMs := UDINT#0;
ELSE
    ElapsedMs := TxMs - LastMs;
END_IF;

IF ElapsedMs >= DWORD_TO_UDINT(TIME_TO_DWORD(PT)) THEN
    LastMs := LastMs + DWORD_TO_UDINT(TIME_TO_DWORD(PT));
    ElapsedMs := ElapsedMs - DWORD_TO_UDINT(TIME_TO_DWORD(PT));
    OUT := AM * (RDM(LAST := REAL#0.0) - 0.5) + OS;
    Q := TRUE;
ELSE
    Q := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_RDT
VAR_INPUT
    Enable : BOOL := TRUE;
    Min_Time_ms : TIME := T#1s;
    Max_Time_ms : TIME := T#1200ms;
    TP_Q : TIME := T#100ms;
END_VAR
VAR_OUTPUT
    xQ : BOOL;
END_VAR
VAR
    RandomTimer : TON;
    PulseHold : TOF;
    RandomTime : TIME;
    RandomSeed : REAL;
    MinMs : UDINT;
    MaxMs : UDINT;
END_VAR

RandomTimer(IN := Enable, PT := RandomTime);
PulseHold(IN := RandomTimer.Q, PT := TP_Q);
xQ := PulseHold.Q;
IF RandomTimer.Q THEN
    xQ := TRUE;
    RandomSeed := RDM(LAST := RandomSeed);
    MinMs := DWORD_TO_UDINT(TIME_TO_DWORD(Min_Time_ms));
    MaxMs := DWORD_TO_UDINT(TIME_TO_DWORD(Max_Time_ms));
    RandomTime := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(
        RandomSeed * UDINT_TO_REAL(MaxMs - MinMs) + UDINT_TO_REAL(MinMs)
    )));
    RandomTimer(IN := FALSE);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_RMP
VAR_INPUT
    PT : TIME := T#1s;
    AM : REAL := 1.0;
    OS : REAL;
    DL : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    OUT : REAL;
END_VAR
VAR
    TxMs : UDINT;
    LastMs : UDINT;
    ElapsedMs : UDINT;
    Init : BOOL;
    Temp : REAL;
    LastTemp : REAL;
    DelayLocal : REAL;
    PeriodMs : UDINT;
    PeriodMsReal : REAL;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());
PeriodMs := DWORD_TO_UDINT(TIME_TO_DWORD(PT));
PeriodMsReal := UDINT_TO_REAL(PeriodMs);
DelayLocal := MODR(IN := DL, DIVI := REAL#1.0);
IF DelayLocal < 0.0 THEN
    DelayLocal := 1.0 - DelayLocal;
END_IF;

IF NOT Init THEN
    Init := TRUE;
    LastMs := TxMs;
    ElapsedMs := UDINT#0;
ELSE
    ElapsedMs := TxMs - LastMs;
END_IF;

IF ElapsedMs >= PeriodMs THEN
    LastMs := LastMs + PeriodMs;
    ElapsedMs := ElapsedMs - PeriodMs;
END_IF;

LastTemp := Temp;
IF PeriodMs > UDINT#0 THEN
    Temp := FRACT(X := (UDINT_TO_REAL(ElapsedMs) + PeriodMsReal * DelayLocal) / PeriodMsReal);
END_IF;
OUT := AM * Temp + OS;
Q := Temp < LastTemp;
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_SIN
VAR_INPUT
    PT : TIME;
    AM : REAL := 1.0;
    OS : REAL;
    DL : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    OUT : REAL;
END_VAR
VAR
    TxMs : UDINT;
    LastMs : UDINT;
    ElapsedMs : UDINT;
    Init : BOOL;
    Temp : REAL;
    DelayLocal : REAL;
    PeriodMs : UDINT;
    PeriodMsReal : REAL;
END_VAR

TxMs := DWORD_TO_UDINT(T_PLC_MS());
PeriodMs := DWORD_TO_UDINT(TIME_TO_DWORD(PT));
PeriodMsReal := UDINT_TO_REAL(PeriodMs);
DelayLocal := MODR(IN := DL, DIVI := REAL#1.0);
IF DelayLocal < 0.0 THEN
    DelayLocal := 1.0 - DelayLocal;
END_IF;

IF NOT Init THEN
    Init := TRUE;
    LastMs := TxMs;
    ElapsedMs := UDINT#0;
ELSE
    ElapsedMs := TxMs - LastMs;
END_IF;

IF ElapsedMs >= PeriodMs THEN
    LastMs := LastMs + PeriodMs;
    ElapsedMs := ElapsedMs - PeriodMs;
END_IF;

IF PeriodMs > UDINT#0 THEN
    Temp := SIN(MATH.PI2 * (UDINT_TO_REAL(ElapsedMs) + PeriodMsReal * DelayLocal) / PeriodMsReal);
END_IF;
OUT := AM * 0.5 * Temp + OS;
Q := NOT SIGN_R(IN := Temp);
END_FUNCTION_BLOCK

FUNCTION_BLOCK GEN_SQR
VAR_INPUT
    PT : TIME;
    AM : REAL := 1.0;
    OS : REAL;
    DC : REAL := 0.5;
    DL : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
    OUT : REAL;
END_VAR
VAR
    TxMs : UDINT;
    LastMs : UDINT;
    ElapsedMs : UDINT;
    Init : BOOL;
    DelayLocal : REAL;
    DutyLocal : REAL;
    PeriodMs : UDINT;
    PeriodMsReal : REAL;
    PhaseStart : REAL;
    PhaseEnd : REAL;
    ElapsedReal : REAL;
END_VAR

IF DC = 0.0 THEN
    OUT := -AM * 0.5 + OS;
    Q := FALSE;
ELSIF DC = 1.0 THEN
    OUT := AM * 0.5 + OS;
    Q := TRUE;
ELSE
    TxMs := DWORD_TO_UDINT(T_PLC_MS());
    PeriodMs := DWORD_TO_UDINT(TIME_TO_DWORD(PT));
    PeriodMsReal := UDINT_TO_REAL(PeriodMs);
    DelayLocal := MODR(IN := DL, DIVI := REAL#1.0);
    IF DelayLocal < 0.0 THEN
        DelayLocal := 1.0 - DelayLocal;
    END_IF;
    DutyLocal := MODR(IN := DC, DIVI := REAL#1.0);
    IF DutyLocal < 0.0 THEN
        DutyLocal := 1.0 - DutyLocal;
    END_IF;

    IF NOT Init THEN
        Init := TRUE;
        LastMs := TxMs;
        ElapsedMs := UDINT#0;
    ELSE
        ElapsedMs := TxMs - LastMs;
    END_IF;

    IF ElapsedMs >= PeriodMs THEN
        LastMs := LastMs + PeriodMs;
        ElapsedMs := ElapsedMs - PeriodMs;
    END_IF;

    PhaseStart := PeriodMsReal * DelayLocal;
    PhaseEnd := PeriodMsReal * (DelayLocal + DutyLocal);
    ElapsedReal := UDINT_TO_REAL(ElapsedMs);
    IF PhaseEnd >= PeriodMsReal THEN
        IF ElapsedReal >= PhaseEnd - PeriodMsReal THEN
            OUT := -AM * 0.5 + OS;
            Q := FALSE;
        END_IF;
        IF ElapsedReal >= PhaseStart THEN
            OUT := AM * 0.5 + OS;
            Q := TRUE;
        END_IF;
    ELSE
        IF ElapsedReal >= PhaseStart THEN
            OUT := AM * 0.5 + OS;
            Q := TRUE;
        END_IF;
        IF ElapsedReal >= PhaseEnd THEN
            OUT := -AM * 0.5 + OS;
            Q := FALSE;
        END_IF;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK PWM_DC
VAR_INPUT
    F : REAL;
    DC : REAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Clock : CLK_PRG;
    Pulse : TP_X;
    PeriodMs : REAL;
END_VAR

IF F > 0.0 THEN
    PeriodMs := 1000.0 / F;
    Clock(PT := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(PeriodMs))));
    Pulse(IN := Clock.Q, PT := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(PeriodMs * DC))));
    Q := Pulse.Q;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK PWM_PW
VAR_INPUT
    F : REAL;
    PW : TIME;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
VAR
    Clock : CLK_PRG;
    Pulse : TP_X;
END_VAR

IF F > 0.0 THEN
    Clock(PT := DWORD_TO_TIME(UDINT_TO_DWORD(REAL_TO_UDINT(1000.0 / F))));
    Pulse(IN := Clock.Q, PT := PW);
    Q := Pulse.Q;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK RMP_B
VAR_INPUT
    SET_HIGH : BOOL;
    PT : TIME;
    E : BOOL := TRUE;
    UP : BOOL := TRUE;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    OUT : BYTE;
    BUSY : BOOL;
    HIGH : BOOL;
    LOW : BOOL;
END_VAR
VAR
    Ramp : _RMP_B;
END_VAR

Ramp(DIR := UP, E := E, TR := PT, RMP := OUT);

IF RST THEN
    OUT := BYTE#0;
ELSIF SET_HIGH THEN
    OUT := BYTE#255;
END_IF;

LOW := OUT = BYTE#0;
HIGH := OUT = BYTE#255;
BUSY := NOT (LOW OR HIGH) AND E;
END_FUNCTION_BLOCK

FUNCTION_BLOCK RMP_SOFT
VAR_INPUT
    IN : BOOL;
    VAL : BYTE;
END_VAR
VAR_INPUT CONSTANT
    PT_ON : TIME;
    PT_OFF : TIME;
END_VAR
VAR_OUTPUT
    OUT : BYTE;
END_VAR
VAR
    Ramp : _RMP_B;
    Target : BYTE;
END_VAR

Target := SEL(G := IN, IN0 := BYTE#0, IN1 := VAL);
IF Target > OUT THEN
    Ramp(DIR := TRUE, E := TRUE, TR := PT_ON, RMP := OUT);
    IF OUT > Target THEN
        OUT := Target;
    END_IF;
ELSIF Target < OUT THEN
    Ramp(DIR := FALSE, E := TRUE, TR := PT_OFF, RMP := OUT);
    IF OUT < Target THEN
        OUT := Target;
    END_IF;
ELSE
    Ramp(E := FALSE, RMP := OUT);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK RMP_W
VAR_INPUT
    SET_HIGH : BOOL;
    PT : TIME;
    E : BOOL := TRUE;
    UP : BOOL := TRUE;
    RST : BOOL;
END_VAR
VAR_OUTPUT
    OUT : WORD;
    BUSY : BOOL;
    HIGH : BOOL;
    LOW : BOOL;
END_VAR
VAR
    Ramp : _RMP_W;
END_VAR

Ramp(DIR := UP, E := E, TR := PT, RMP := OUT);

IF RST THEN
    OUT := WORD#0;
ELSIF SET_HIGH THEN
    OUT := WORD#65535;
END_IF;

LOW := OUT = WORD#0;
HIGH := OUT = WORD#65535;
BUSY := NOT (LOW OR HIGH) AND E;
END_FUNCTION_BLOCK
