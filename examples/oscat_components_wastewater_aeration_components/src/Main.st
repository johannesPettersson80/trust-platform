PROGRAM Main
VAR
    OxygenDemand : HysteresisSwitch;
    DemandObject : IHysteresisSwitch;
    AerationPulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    DemandLimits : RealRange;
    DissolvedOxygen : REAL := REAL#2.5;
    DemandActive : BOOL;
    BlowerCommand : BOOL;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
DemandObject := OxygenDemand;
DemandObject.Initialize();
DemandObject.SetLimits(Limits := DemandLimits);
DemandActive := DemandObject.Update(MeasuredValue := DissolvedOxygen);

PulseObject := AerationPulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#2s, LowTime := T#3s);
PulseObject.SetEnabled(Enabled := DemandActive);
BlowerCommand := PulseObject.Update();
END_PROGRAM
