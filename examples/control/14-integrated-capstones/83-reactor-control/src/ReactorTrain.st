USING Control;

(* One selected arrangement owns one heater. Both alternatives reuse the
   same stateful Control.PID interface; inactive loops track selected states. *)
FUNCTION_BLOCK ReactorTrain
VAR
    Single : Control.PID;
    Outer : Control.PID;
    Inner : Control.PID;
    SingleInput : Control.PIDInput;
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    PreviousApplied : LREAL := 15.0;
    Snapshot : Control.PIDInitialState;
END_VAR
VAR_OUTPUT
    AppliedPowerPct : LREAL := 15.0;
    NominalPowerPct : LREAL := 15.0;
    SelectedJacketC : LREAL := 38.0;
    OuterIntegralC : LREAL := 38.0;
    HeaterIntegralPct : LREAL := 15.0;
    HeaterProportionalPct : LREAL := 0.0;
END_VAR
METHOD PUBLIC Configure
VAR
    Parameters : Control.PIDParameters;
    Initial : Control.PIDInitialState;
END_VAR
Parameters.Bias := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.ProportionalGain := LREAL#2.5;
Parameters.IntegralGain := LREAL#0.1;
Single.Configure(Parameters := Parameters);
Initial.Setpoint := LREAL#35.0;
Initial.ProcessValue := LREAL#35.0;
Initial.NominalOutput := LREAL#15.0;
Initial.SaturatedOutput := LREAL#15.0;
Single.Initialize(State := Initial);
Parameters.ProportionalGain := LREAL#0.5;
Parameters.IntegralGain := LREAL#0.12;
Parameters.OutputMinimum := LREAL#20.0;
Parameters.OutputMaximum := LREAL#50.0;
Outer.Configure(Parameters := Parameters);
Initial.NominalOutput := LREAL#38.0;
Initial.SaturatedOutput := LREAL#38.0;
Outer.Initialize(State := Initial);
Parameters.ProportionalGain := LREAL#5.0;
Parameters.IntegralGain := LREAL#10.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Inner.Configure(Parameters := Parameters);
Initial.Setpoint := LREAL#38.0;
Initial.ProcessValue := LREAL#38.0;
Initial.NominalOutput := LREAL#15.0;
Initial.SaturatedOutput := LREAL#15.0;
Inner.Initialize(State := Initial);
END_METHOD
METHOD PUBLIC Update
VAR_INPUT
    SetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
    UseCascade : BOOL;
    Automatic : BOOL;
    Faulted : BOOL;
END_VAR
VAR
    ManualPower : LREAL;
    Unused : LREAL;
END_VAR
ManualPower := LREAL#15.0;
IF Faulted THEN ManualPower := LREAL#0.0; END_IF;
SingleInput.Setpoint := SetpointC;
SingleInput.ProcessValue := VesselC;
SingleInput.CycleTime := LREAL#0.1;
SingleInput.Mode := PIDMode#Manual;
SingleInput.ManualOutput := PreviousApplied;
OuterInput.Setpoint := SetpointC;
OuterInput.ProcessValue := VesselC;
OuterInput.CycleTime := LREAL#0.1;
OuterInput.Mode := PIDMode#Manual;
OuterInput.ManualOutput := JacketC;
InnerInput.ProcessValue := JacketC;
InnerInput.CycleTime := LREAL#0.1;
InnerInput.Mode := PIDMode#Manual;
InnerInput.ManualOutput := PreviousApplied;
IF Automatic AND NOT Faulted THEN
    IF UseCascade THEN
        OuterInput.Mode := PIDMode#Auto;
        InnerInput.Mode := PIDMode#Auto;
    ELSE SingleInput.Mode := PIDMode#Auto;
    END_IF;
ELSE
    SingleInput.ManualOutput := ManualPower;
    InnerInput.ManualOutput := ManualPower;
END_IF;
SelectedJacketC := Outer.Update(Input := OuterInput);
InnerInput.Setpoint := SelectedJacketC;
IF UseCascade THEN
    AppliedPowerPct := Inner.Update(Input := InnerInput);
    SingleInput.ManualOutput := AppliedPowerPct;
    Unused := Single.Update(Input := SingleInput);
    Snapshot := Inner.GetState();
    HeaterProportionalPct := LREAL#5.0 * (SelectedJacketC - JacketC);
ELSE
    AppliedPowerPct := Single.Update(Input := SingleInput);
    InnerInput.ManualOutput := AppliedPowerPct;
    Unused := Inner.Update(Input := InnerInput);
    Snapshot := Single.GetState();
    HeaterProportionalPct := LREAL#2.5 * (SetpointC - VesselC);
END_IF;
(* The nominal output is the retained, tracking-corrected PI output, not an
   uncorrected pre-limit request. Its P+I decomposition is shown explicitly. *)
NominalPowerPct := Snapshot.NominalOutput;
HeaterIntegralPct := NominalPowerPct - HeaterProportionalPct;
Snapshot := Outer.GetState();
OuterIntegralC := Snapshot.NominalOutput - LREAL#0.5 * (SetpointC - VesselC);
PreviousApplied := AppliedPowerPct;
END_METHOD
END_FUNCTION_BLOCK
