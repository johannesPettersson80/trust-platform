FUNCTION_BLOCK BatchReactorClassic
VAR
    StateValue : INT;
    StepValue : INT;
    RecipeIdValue : INT;
    AlarmActiveValue : BOOL;
    AlarmCodeValue : DWORD;
    BatchCompleteValue : BOOL;
    AcidPhCalibrator : Calibrator;
    PolymerController : PidController;
    BaseWashPwm : PwmController;
    Limits : RealRange;
    Gains : PidGains;
END_VAR

METHOD PUBLIC Initialize
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.2;
AcidPhCalibrator.Initialize();
AcidPhCalibrator.Configure(OffsetTarget := REAL#0.0, ScaleTarget := REAL#1.0);
PolymerController.Initialize();
PolymerController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
BaseWashPwm.Initialize();
BaseWashPwm.Configure(Frequency := REAL#2.0);
Reset();
END_METHOD

METHOD PUBLIC Reset
StateValue := INT#0;
StepValue := INT#0;
RecipeIdValue := INT#0;
AlarmActiveValue := FALSE;
AlarmCodeValue := DWORD#0;
BatchCompleteValue := FALSE;
END_METHOD

METHOD PRIVATE RaiseAlarm
VAR_INPUT
    Code : DWORD;
END_VAR
AlarmActiveValue := TRUE;
AlarmCodeValue := Code;
StateValue := INT#900;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    AbortCommand : BOOL;
    LidClosedA : BOOL;
    LidClosedB : BOOL;
    EStopHealthy : BOOL;
    PressureReliefIntact : BOOL;
    RecipeId : INT;
    ProductTemp : REAL;
    PH : REAL;
    HoldComplete : BOOL;
    SampleOk : BOOL;
END_VAR
VAR
    PwmPulse : BOOL;
    PolymerCommand : REAL;
    CalibratedPH : REAL;
END_VAR
IF AbortCommand OR NOT EStopHealthy OR NOT PressureReliefIntact THEN
    RaiseAlarm(Code := DWORD#16#A001);
ELSIF StateValue <> INT#0 AND NOT (LidClosedA AND LidClosedB) THEN
    RaiseAlarm(Code := DWORD#16#A002);
ELSIF ProductTemp > REAL#95.0 THEN
    RaiseAlarm(Code := DWORD#16#A003);
ELSE
    CASE StateValue OF
        INT#0:
            IF StartCommand AND LidClosedA AND LidClosedB THEN
                RecipeIdValue := RecipeId;
                CASE RecipeId OF
                    INT#1:
                        StepValue := INT#110;
                        StateValue := INT#100;
                    INT#2:
                        StepValue := INT#210;
                        StateValue := INT#100;
                    INT#3:
                        StepValue := INT#310;
                        StateValue := INT#100;
                ELSE
                    RaiseAlarm(Code := DWORD#16#B001);
                END_CASE;
            END_IF;
        INT#100:
            CASE RecipeIdValue OF
                INT#1:
                    CalibratedPH := AcidPhCalibrator.Update(Sample := PH, CalibrateOffset := FALSE, CalibrateScale := FALSE);
                    IF StepValue = INT#110 AND CalibratedPH < REAL#6.8 THEN
                        StepValue := INT#120;
                    ELSIF StepValue = INT#120 AND HoldComplete AND ProductTemp >= REAL#55.0 THEN
                        StepValue := INT#130;
                    ELSIF StepValue = INT#130 AND SampleOk THEN
                        StepValue := INT#140;
                        BatchCompleteValue := TRUE;
                        StateValue := INT#800;
                    END_IF;
                INT#2:
                    PolymerCommand := PolymerController.Update(Actual := ProductTemp, Target := REAL#72.0);
                    IF StepValue = INT#210 AND PolymerCommand >= REAL#0.0 THEN
                        StepValue := INT#220;
                    ELSIF StepValue = INT#220 AND HoldComplete THEN
                        StepValue := INT#230;
                    ELSIF StepValue = INT#230 AND SampleOk AND PH >= REAL#6.5 THEN
                        StepValue := INT#240;
                        BatchCompleteValue := TRUE;
                        StateValue := INT#800;
                    END_IF;
                INT#3:
                    PwmPulse := BaseWashPwm.Update(ControlInput := REAL#0.5);
                    IF StepValue = INT#310 AND PwmPulse THEN
                        StepValue := INT#320;
                    ELSIF StepValue = INT#320 AND HoldComplete AND ProductTemp >= REAL#35.0 THEN
                        StepValue := INT#330;
                    ELSIF StepValue = INT#330 AND SampleOk THEN
                        StepValue := INT#340;
                        BatchCompleteValue := TRUE;
                        StateValue := INT#800;
                    END_IF;
            END_CASE;
        INT#800:
            IF NOT StartCommand THEN
                StateValue := INT#0;
            END_IF;
    END_CASE;
END_IF;
Update := StateValue;
END_METHOD

PUBLIC PROPERTY SequenceStep : INT
GET
    SequenceStep := StepValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmActive : BOOL
GET
    AlarmActive := AlarmActiveValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCode : DWORD
GET
    AlarmCode := AlarmCodeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BatchComplete : BOOL
GET
    BatchComplete := BatchCompleteValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Reactor : BatchReactorClassic;
    StateValue : INT;
END_VAR
Reactor.Initialize();
StateValue := Reactor.Update(StartCommand := TRUE, AbortCommand := FALSE, LidClosedA := TRUE, LidClosedB := TRUE, EStopHealthy := TRUE, PressureReliefIntact := TRUE, RecipeId := INT#1, ProductTemp := REAL#56.0, PH := REAL#6.5, HoldComplete := TRUE, SampleOk := TRUE);
END_PROGRAM
