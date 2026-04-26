TEST_PROGRAM WastewaterAerationComponentsGatePulseWithDemand
VAR
    OxygenDemand : HysteresisSwitch;
    DemandObject : IHysteresisSwitch;
    AerationPulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    DemandLimits : RealRange;
    ClassicDemand : HYST;
    ClassicPulse : GEN_PULSE;
    DemandActive : BOOL;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
DemandObject := OxygenDemand;
DemandObject.Initialize();
DemandObject.SetLimits(Limits := DemandLimits);
PulseObject := AerationPulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#2s, LowTime := T#3s);

ClassicDemand(In := REAL#2.5, HIGH_LIMIT := REAL#2.0, LOW_LIMIT := REAL#1.0);
DemandActive := DemandObject.Update(MeasuredValue := REAL#2.5);
ClassicPulse(ENQ := ClassicDemand.Q, PTH := T#2s, PTL := T#3s);
PulseObject.SetEnabled(Enabled := DemandActive);

ASSERT_TRUE(IN := DemandActive = ClassicDemand.Q);
ASSERT_TRUE(IN := PulseObject.Update() = ClassicPulse.Q);
END_TEST_PROGRAM
