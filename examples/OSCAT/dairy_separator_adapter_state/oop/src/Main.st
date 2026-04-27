(* Pattern role: ISeparatorDrive is the contract boundary for the Adapter and State example. *)
INTERFACE ISeparatorDrive
    PROPERTY IsRunning : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY IsAtSpeed : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HasFault : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY BowlSpeedRpm : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY VibrationMmS : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY FaultCode : DWORD
    GET
    END_GET
    END_PROPERTY

    METHOD Decode
    VAR_INPUT
        StatusWord : WORD;
        SpeedFeedbackRpm : REAL;
        VibrationMmSRaw : REAL;
        FaultRaw : WORD;
    END_VAR
    END_METHOD

    METHOD Run
    VAR_INPUT
        SpeedSetpointRpm : REAL;
    END_VAR
    END_METHOD

    METHOD Stop
    END_METHOD
END_INTERFACE

(* Pattern role: AbbSeparatorDriveAdapter is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK AbbSeparatorDriveAdapter IMPLEMENTS ISeparatorDrive
VAR
    RunningValue : BOOL;
    FaultValue : BOOL;
    BowlSpeedRpmValue : REAL;
    VibrationMmSValue : REAL;
    FaultRawValue : WORD;
END_VAR

METHOD PUBLIC Decode
VAR_INPUT
    StatusWord : WORD;
    SpeedFeedbackRpm : REAL;
    VibrationMmSRaw : REAL;
    FaultRaw : WORD;
END_VAR
(* ABB separator adapter: bit 2 is the running indication in this raw VFD status word. *)
RunningValue := (WORD_TO_DWORD(StatusWord) AND DWORD#16#0004) <> DWORD#0;
(* Example vibration trip: 8.0 mm/s is the skid commissioning limit used by the tests; replace with the site/vendor limit. *)
FaultValue := ((WORD_TO_DWORD(StatusWord) AND DWORD#16#0008) <> DWORD#0) OR (VibrationMmSRaw > REAL#8.0);
BowlSpeedRpmValue := SpeedFeedbackRpm;
VibrationMmSValue := VibrationMmSRaw;
FaultRawValue := FaultRaw;
END_METHOD

METHOD PUBLIC Run
VAR_INPUT
    SpeedSetpointRpm : REAL;
END_VAR
RunningValue := SpeedSetpointRpm > REAL#0.0;
IF BowlSpeedRpmValue < SpeedSetpointRpm THEN
    BowlSpeedRpmValue := SpeedSetpointRpm;
END_IF;
END_METHOD

METHOD PUBLIC Stop
RunningValue := FALSE;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsAtSpeed : BOOL
GET
    (* Separator at-speed threshold for this example skid: production may start above 6000 RPM. *)
IsAtSpeed := (BowlSpeedRpmValue >= REAL#6000.0) AND NOT FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HasFault : BOOL
GET
    HasFault := FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BowlSpeedRpm : REAL
GET
    BowlSpeedRpm := BowlSpeedRpmValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY VibrationMmS : REAL
GET
    VibrationMmS := VibrationMmSValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCode : DWORD
GET
    IF VibrationMmSValue > REAL#8.0 THEN
        FaultCode := DWORD#16#D00000F1;
    ELSE
        FaultCode := DWORD#16#D0000000 OR WORD_TO_DWORD(FaultRawValue);
    END_IF;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: SiemensSeparatorDriveAdapter is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK SiemensSeparatorDriveAdapter IMPLEMENTS ISeparatorDrive
VAR
    RunningValue : BOOL;
    FaultValue : BOOL;
    BowlSpeedRpmValue : REAL;
    VibrationMmSValue : REAL;
    FaultRawValue : WORD;
END_VAR

METHOD PUBLIC Decode
VAR_INPUT
    StatusWord : WORD;
    SpeedFeedbackRpm : REAL;
    VibrationMmSRaw : REAL;
    FaultRaw : WORD;
END_VAR
(* Siemens separator adapter: bit 10 is the running indication, different from the ABB profile above. *)
RunningValue := (WORD_TO_DWORD(StatusWord) AND DWORD#16#0400) <> DWORD#0;
FaultValue := ((WORD_TO_DWORD(StatusWord) AND DWORD#16#0008) <> DWORD#0) OR (VibrationMmSRaw > REAL#8.0);
BowlSpeedRpmValue := SpeedFeedbackRpm;
VibrationMmSValue := VibrationMmSRaw;
FaultRawValue := FaultRaw;
END_METHOD

METHOD PUBLIC Run
VAR_INPUT
    SpeedSetpointRpm : REAL;
END_VAR
RunningValue := SpeedSetpointRpm > REAL#0.0;
IF BowlSpeedRpmValue < SpeedSetpointRpm THEN
    BowlSpeedRpmValue := SpeedSetpointRpm;
END_IF;
END_METHOD

METHOD PUBLIC Stop
RunningValue := FALSE;
END_METHOD

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsAtSpeed : BOOL
GET
    IsAtSpeed := (BowlSpeedRpmValue >= REAL#6000.0) AND NOT FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HasFault : BOOL
GET
    HasFault := FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BowlSpeedRpm : REAL
GET
    BowlSpeedRpm := BowlSpeedRpmValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY VibrationMmS : REAL
GET
    VibrationMmS := VibrationMmSValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCode : DWORD
GET
    IF VibrationMmSValue > REAL#8.0 THEN
        FaultCode := DWORD#16#500000F1;
    ELSE
        FaultCode := DWORD#16#50000000 OR WORD_TO_DWORD(FaultRawValue);
    END_IF;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: ISeparatorState is the contract boundary for the Adapter and State example. *)
INTERFACE ISeparatorState
    PROPERTY StateId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD OnExecute : INT
    VAR_INPUT
        StartCommand : BOOL;
        DischargeRequest : BOOL;
        CipMode : BOOL;
        AtSpeed : BOOL;
        DriveFault : BOOL;
        VibrationHigh : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: IdleState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK IdleState IMPLEMENTS ISeparatorState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    AtSpeed : BOOL;
    DriveFault : BOOL;
    VibrationHigh : BOOL;
END_VAR
IF DriveFault OR VibrationHigh THEN
    OnExecute := INT#90;
ELSIF CipMode THEN
    OnExecute := INT#40;
ELSIF StartCommand THEN
    OnExecute := INT#10;
ELSE
    OnExecute := INT#0;
END_IF;
END_METHOD
PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: SpinUpState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK SpinUpState IMPLEMENTS ISeparatorState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    AtSpeed : BOOL;
    DriveFault : BOOL;
    VibrationHigh : BOOL;
END_VAR
IF DriveFault OR VibrationHigh THEN
    OnExecute := INT#90;
ELSIF CipMode THEN
    OnExecute := INT#40;
ELSIF AtSpeed THEN
    OnExecute := INT#20;
ELSE
    OnExecute := INT#10;
END_IF;
END_METHOD
PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#10;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: ProductionState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK ProductionState IMPLEMENTS ISeparatorState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    AtSpeed : BOOL;
    DriveFault : BOOL;
    VibrationHigh : BOOL;
END_VAR
IF DriveFault OR VibrationHigh OR NOT AtSpeed THEN
    OnExecute := INT#90;
ELSIF CipMode THEN
    OnExecute := INT#40;
ELSIF DischargeRequest THEN
    OnExecute := INT#30;
ELSE
    OnExecute := INT#20;
END_IF;
END_METHOD
PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#20;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: DischargeState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK DischargeState IMPLEMENTS ISeparatorState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    AtSpeed : BOOL;
    DriveFault : BOOL;
    VibrationHigh : BOOL;
END_VAR
IF DriveFault OR VibrationHigh THEN
    OnExecute := INT#90;
ELSIF CipMode THEN
    OnExecute := INT#40;
ELSIF NOT DischargeRequest THEN
    OnExecute := INT#20;
ELSE
    OnExecute := INT#30;
END_IF;
END_METHOD
PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#30;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: CipState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK CipState IMPLEMENTS ISeparatorState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    AtSpeed : BOOL;
    DriveFault : BOOL;
    VibrationHigh : BOOL;
END_VAR
IF DriveFault OR VibrationHigh THEN
    OnExecute := INT#90;
ELSIF NOT CipMode THEN
    OnExecute := INT#0;
ELSE
    OnExecute := INT#40;
END_IF;
END_METHOD
PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#40;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: FaultState is a concrete implementation in the Adapter and State example. *)
FUNCTION_BLOCK FaultState IMPLEMENTS ISeparatorState
METHOD PUBLIC OnExecute : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    AtSpeed : BOOL;
    DriveFault : BOOL;
    VibrationHigh : BOOL;
END_VAR
OnExecute := INT#90;
END_METHOD
PUBLIC PROPERTY StateId : INT
GET
    StateId := INT#90;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: SeparatorController owns the collaborating objects used by this Adapter and State example. *)
FUNCTION_BLOCK SeparatorController
VAR
    AbbDrive : AbbSeparatorDriveAdapter;
    SiemensDrive : SiemensSeparatorDriveAdapter;
    Drive : ISeparatorDrive;
    Idle : IdleState;
    SpinUp : SpinUpState;
    Production : ProductionState;
    Discharge : DischargeState;
    Cip : CipState;
    Fault : FaultState;
    Current : ISeparatorState;
    StateIdValue : INT;
    FeedValveOpenValue : BOOL;
    DischargeValveOpenValue : BOOL;
    CipValveOpenValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
Drive := AbbDrive;
Current := Idle;
StateIdValue := INT#0;
END_METHOD

METHOD PUBLIC SelectDrive
VAR_INPUT
    UseSiemens : BOOL;
END_VAR
IF UseSiemens THEN
    Drive := SiemensDrive;
ELSE
    Drive := AbbDrive;
END_IF;
END_METHOD

METHOD PUBLIC InjectDriveStatus
VAR_INPUT
    UseSiemens : BOOL;
    StatusWord : WORD;
    SpeedFeedbackRpm : REAL;
    VibrationMmSRaw : REAL;
    FaultRaw : WORD;
END_VAR
SelectDrive(UseSiemens := UseSiemens);
Drive.Decode(StatusWord := StatusWord, SpeedFeedbackRpm := SpeedFeedbackRpm, VibrationMmSRaw := VibrationMmSRaw, FaultRaw := FaultRaw);
END_METHOD

METHOD PUBLIC ResolveState : ISeparatorState
VAR_INPUT
    StateId : INT;
END_VAR
IF StateId = INT#10 THEN
    ResolveState := SpinUp;
ELSIF StateId = INT#20 THEN
    ResolveState := Production;
ELSIF StateId = INT#30 THEN
    ResolveState := Discharge;
ELSIF StateId = INT#40 THEN
    ResolveState := Cip;
ELSIF StateId = INT#90 THEN
    ResolveState := Fault;
ELSE
    ResolveState := Idle;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    UseSiemens : BOOL;
END_VAR
VAR
    NextStateId : INT;
END_VAR
SelectDrive(UseSiemens := UseSiemens);
NextStateId := Current.OnExecute(StartCommand := StartCommand, DischargeRequest := DischargeRequest, CipMode := CipMode, AtSpeed := Drive.IsAtSpeed, DriveFault := Drive.HasFault, VibrationHigh := Drive.VibrationMmS > REAL#8.0);
IF NextStateId <> Current.StateId THEN
    Current := ResolveState(StateId := NextStateId);
END_IF;
StateIdValue := Current.StateId;
FeedValveOpenValue := FALSE;
DischargeValveOpenValue := FALSE;
CipValveOpenValue := FALSE;
IF StateIdValue = INT#10 THEN
    Drive.Run(SpeedSetpointRpm := REAL#6500.0);
ELSIF StateIdValue = INT#20 THEN
    Drive.Run(SpeedSetpointRpm := REAL#6500.0);
    FeedValveOpenValue := TRUE;
ELSIF StateIdValue = INT#30 THEN
    Drive.Run(SpeedSetpointRpm := REAL#6200.0);
    DischargeValveOpenValue := TRUE;
ELSIF StateIdValue = INT#40 THEN
    Drive.Stop();
    CipValveOpenValue := TRUE;
ELSE
    Drive.Stop();
END_IF;
Update := StateIdValue;
END_METHOD

PUBLIC PROPERTY StateId : INT
GET
    StateId := StateIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FeedValveOpen : BOOL
GET
    FeedValveOpen := FeedValveOpenValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY DischargeValveOpen : BOOL
GET
    DischargeValveOpen := DischargeValveOpenValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY CipValveOpen : BOOL
GET
    CipValveOpen := CipValveOpenValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY BowlRunning : BOOL
GET
    BowlRunning := Drive.IsRunning;
END_GET
END_PROPERTY

PUBLIC PROPERTY NormalizedFaultCode : DWORD
GET
    NormalizedFaultCode := Drive.FaultCode;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Separator : SeparatorController;
    StartCommand : BOOL;
    DischargeRequest : BOOL;
    CipMode : BOOL;
    UseSiemensDrive : BOOL;
    StatusWordRaw : WORD;
    SpeedFeedbackRaw : INT;
    VibrationRaw : INT;
    FaultRaw : WORD;
    FeedValveOut : BOOL;
    DischargeValveOut : BOOL;
    CipValveOut : BOOL;
END_VAR
Separator.Initialize();
Separator.InjectDriveStatus(UseSiemens := UseSiemensDrive, StatusWord := StatusWordRaw, SpeedFeedbackRpm := INT_TO_REAL(SpeedFeedbackRaw), VibrationMmSRaw := INT_TO_REAL(VibrationRaw) / REAL#10.0, FaultRaw := FaultRaw);
Separator.Update(StartCommand := StartCommand, DischargeRequest := DischargeRequest, CipMode := CipMode, UseSiemens := UseSiemensDrive);
FeedValveOut := Separator.FeedValveOpen;
DischargeValveOut := Separator.DischargeValveOpen;
CipValveOut := Separator.CipValveOpen;
END_PROGRAM
