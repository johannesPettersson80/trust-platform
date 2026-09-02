NAMESPACE Control
TYPE PIDMode :
(
    Auto := 0,
    Manual := 1,
    Track := 2
);
END_TYPE

TYPE PIDAntiWindup :
(
    BackCalculation := 0,
    Combined := 1
);
END_TYPE

TYPE FilterOrder :
(
    FirstOrder := 0,
    SecondOrder := 1
);
END_TYPE

TYPE PIDParameters :
STRUCT
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    DerivativeGain : LREAL;
    ProportionalSetpointWeight : LREAL;
    DerivativeSetpointWeight : LREAL;
    Bias : LREAL;
    OutputMinimum : LREAL;
    OutputMaximum : LREAL;
    RateMinimum : LREAL;
    RateMaximum : LREAL;
    TrackingTime : LREAL;
    AntiWindup : PIDAntiWindup;
END_STRUCT
END_TYPE

TYPE PIDInitialState :
STRUCT
    Setpoint : LREAL;
    SetpointRate : LREAL;
    ProcessValue : LREAL;
    ProcessValueRate : LREAL;
    FeedForward : LREAL;
    NominalOutput : LREAL;
    SaturatedOutput : LREAL;
END_STRUCT
END_TYPE

TYPE PIDInput :
STRUCT
    Setpoint : LREAL;
    ProcessValue : LREAL;
    FeedForward : LREAL;
    ManualOutput : LREAL;
    TrackingOutput : LREAL;
    Mode : PIDMode;
    CycleTime : LREAL;
END_STRUCT
END_TYPE

TYPE LowPassParameters :
STRUCT
    TimeConstant : LREAL;
    Order : FilterOrder;
END_STRUCT
END_TYPE

FUNCTION_BLOCK PID
VAR
    ParametersValue : PIDParameters;
    StateValue : PIDInitialState;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    Parameters : PIDParameters;
END_VAR
ParametersValue := Parameters;
END_METHOD

METHOD PUBLIC Initialize
VAR_INPUT
    State : PIDInitialState;
END_VAR
StateValue := State;
END_METHOD

METHOD PUBLIC Update : LREAL
VAR_INPUT
    Input : PIDInput;
END_VAR
VAR
    LowerLimit : LREAL;
    UpperLimit : LREAL;
    SetpointRate : LREAL;
    ProcessValueRate : LREAL;
    NominalOutput : LREAL;
    SaturatedCandidate : LREAL;
    ProportionalIncrement : LREAL;
    IntegralIncrement : LREAL;
    DerivativeIncrement : LREAL;
    FeedForwardIncrement : LREAL;
    SaturationError : LREAL;
    ConditionalCorrection : LREAL;
    Output : LREAL;
END_VAR

(* Rate limits are anchored to the actuator-facing saturated history. *)
LowerLimit := MAX(
    ParametersValue.OutputMinimum,
    StateValue.SaturatedOutput + Input.CycleTime * ParametersValue.RateMinimum
);
UpperLimit := MIN(
    ParametersValue.OutputMaximum,
    StateValue.SaturatedOutput + Input.CycleTime * ParametersValue.RateMaximum
);

SetpointRate := (Input.Setpoint - StateValue.Setpoint) / Input.CycleTime;
ProcessValueRate := (Input.ProcessValue - StateValue.ProcessValue) / Input.CycleTime;

IF Input.Mode = Track THEN
    StateValue.NominalOutput := Input.TrackingOutput;
END_IF;

IF Input.Mode = Manual THEN
    NominalOutput := Input.ManualOutput;
ELSE
    IF ParametersValue.IntegralGain = LREAL#0.0 THEN
        NominalOutput := ParametersValue.Bias
            + ParametersValue.ProportionalGain * (Input.Setpoint - Input.ProcessValue)
            + ParametersValue.DerivativeGain
                * (ParametersValue.DerivativeSetpointWeight * SetpointRate - ProcessValueRate)
            + Input.FeedForward;
    ELSE
        ProportionalIncrement := ParametersValue.ProportionalGain
            * (ParametersValue.ProportionalSetpointWeight
                * (Input.Setpoint - StateValue.Setpoint)
                - (Input.ProcessValue - StateValue.ProcessValue));
        IntegralIncrement := ParametersValue.IntegralGain * Input.CycleTime
            * (Input.Setpoint - Input.ProcessValue);
        DerivativeIncrement := ParametersValue.DerivativeGain
            * (ParametersValue.DerivativeSetpointWeight
                * (SetpointRate - StateValue.SetpointRate)
                - (ProcessValueRate - StateValue.ProcessValueRate));
        FeedForwardIncrement := Input.FeedForward - StateValue.FeedForward;

        NominalOutput := StateValue.NominalOutput
            + ProportionalIncrement
            + IntegralIncrement
            + DerivativeIncrement
            + FeedForwardIncrement;
        SaturatedCandidate := MAX(MIN(NominalOutput, UpperLimit), LowerLimit);
        IF ParametersValue.AntiWindup = Combined THEN
            SaturationError := NominalOutput - SaturatedCandidate;
            IF IntegralIncrement * SaturationError > LREAL#0.0 THEN
                ConditionalCorrection := MIN(ABS(IntegralIncrement), ABS(SaturationError));
                IF SaturationError > LREAL#0.0 THEN
                    NominalOutput := NominalOutput - ConditionalCorrection;
                ELSE
                    NominalOutput := NominalOutput + ConditionalCorrection;
                END_IF;
            END_IF;
            (* Listing 5 deliberately reuses the candidate from before the correction. *)
            NominalOutput := NominalOutput
                - MIN(Input.CycleTime / ParametersValue.TrackingTime, LREAL#1.0)
                    * (NominalOutput - SaturatedCandidate);
        ELSE
            NominalOutput := NominalOutput
                - Input.CycleTime / ParametersValue.TrackingTime
                    * (NominalOutput - SaturatedCandidate);
        END_IF;
    END_IF;
END_IF;

StateValue.NominalOutput := NominalOutput;
Output := MAX(MIN(NominalOutput, UpperLimit), LowerLimit);
StateValue.SaturatedOutput := Output;
StateValue.Setpoint := Input.Setpoint;
StateValue.SetpointRate := SetpointRate;
StateValue.ProcessValue := Input.ProcessValue;
StateValue.ProcessValueRate := ProcessValueRate;
StateValue.FeedForward := Input.FeedForward;
Update := Output;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK LowPass
VAR
    ParametersValue : LowPassParameters;
    Stage1 : LREAL;
    Stage2 : LREAL;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    Parameters : LowPassParameters;
END_VAR
ParametersValue := Parameters;
END_METHOD

METHOD PUBLIC Initialize
VAR_INPUT
    Input : LREAL;
END_VAR
Stage1 := Input;
Stage2 := Input;
END_METHOD

METHOD PUBLIC Update : LREAL
VAR_INPUT
    Input : LREAL;
    CycleTime : LREAL;
END_VAR
VAR
    Coefficient : LREAL;
    NextStage1 : LREAL;
    NextStage2 : LREAL;
END_VAR

Coefficient := CycleTime / (ParametersValue.TimeConstant + LREAL#0.5 * CycleTime);
NextStage1 := Stage1 + Coefficient * (Input - Stage1);
Stage1 := NextStage1;

IF ParametersValue.Order = SecondOrder THEN
    NextStage2 := Stage2 + Coefficient * (NextStage1 - Stage2);
    Stage2 := NextStage2;
    Update := NextStage2;
ELSE
    Update := NextStage1;
END_IF;
END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE
