(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    OxygenDemand : HysteresisSwitch;
    AerationPulse : PulseGenerator;
    DemandLimits : RealRange;
    DissolvedOxygen : REAL := REAL#2.5;
    DemandActive : BOOL;
    BlowerCommand : BOOL;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
OxygenDemand.Initialize();
OxygenDemand.SetLimits(Limits := DemandLimits);
DemandActive := OxygenDemand.Update(MeasuredValue := DissolvedOxygen);

AerationPulse.Initialize();
AerationPulse.Configure(HighTime := T#2s, LowTime := T#3s);
AerationPulse.SetEnabled(Enabled := DemandActive);
BlowerCommand := AerationPulse.Update();
END_PROGRAM
