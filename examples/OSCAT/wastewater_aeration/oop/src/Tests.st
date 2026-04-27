TEST_PROGRAM WastewaterAerationOopGatePulseWithDemand
VAR
    OxygenDemand : HysteresisSwitch;
    AerationPulse : PulseGenerator;
    DemandLimits : RealRange;
    ClassicDemand : HYST;
    ClassicPulse : GEN_PULSE;
    DemandActive : BOOL;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
OxygenDemand.Initialize();
OxygenDemand.SetLimits(Limits := DemandLimits);
AerationPulse.Initialize();
AerationPulse.Configure(HighTime := T#2s, LowTime := T#3s);

ClassicDemand(In := REAL#2.5, HIGH_LIMIT := REAL#2.0, LOW_LIMIT := REAL#1.0);
DemandActive := OxygenDemand.Update(MeasuredValue := REAL#2.5);
ClassicPulse(ENQ := ClassicDemand.Q, PTH := T#2s, PTL := T#3s);
AerationPulse.SetEnabled(Enabled := DemandActive);

ASSERT_TRUE(IN := DemandActive = ClassicDemand.Q);
ASSERT_TRUE(IN := AerationPulse.Update() = ClassicPulse.Q);
END_TEST_PROGRAM

(* Multi-cycle DO-demand gating test: drives the same components
   through high oxygen (demand off) -> low oxygen (demand on) -> back
   to high. Verifies the alarm switch transitions and the pulse
   generator follows the demand state across cycles. *)
TEST_PROGRAM WastewaterAerationGatesPulseAcrossDemandTransitions
VAR
    OxygenDemand : HysteresisSwitch;
    AerationPulse : PulseGenerator;
    DemandLimits : RealRange;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
OxygenDemand.Initialize();
OxygenDemand.SetLimits(Limits := DemandLimits);
AerationPulse.Initialize();
AerationPulse.Configure(HighTime := T#2s, LowTime := T#3s);

(* High DO (above HIGH_LIMIT) -> demand TRUE, pulse enabled *)
AerationPulse.SetEnabled(Enabled := OxygenDemand.Update(MeasuredValue := REAL#2.5));
ASSERT_TRUE(IN := AerationPulse.Enabled);

(* In window -> demand retains, pulse stays enabled *)
AerationPulse.SetEnabled(Enabled := OxygenDemand.Update(MeasuredValue := REAL#1.5));
ASSERT_TRUE(IN := AerationPulse.Enabled);

(* Low DO (below LOW_LIMIT) -> demand FALSE, pulse disabled *)
AerationPulse.SetEnabled(Enabled := OxygenDemand.Update(MeasuredValue := REAL#0.5));
ASSERT_FALSE(IN := AerationPulse.Enabled);
END_TEST_PROGRAM

(* Multi-call regression: drive the demand high, then sustain across
   many scans. The HysteresisSwitch Q should stay latched and the
   pulse stays enabled. Catches bugs where Update would re-evaluate
   the latch without crossing a limit. *)
TEST_PROGRAM WastewaterAerationDemandStaysLatchedAcrossScans
VAR
    OxygenDemand : HysteresisSwitch;
    AerationPulse : PulseGenerator;
    DemandLimits : RealRange;
    I : INT;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
OxygenDemand.Initialize();
OxygenDemand.SetLimits(Limits := DemandLimits);
AerationPulse.Initialize();
AerationPulse.Configure(HighTime := T#2s, LowTime := T#3s);

(* Cross HIGH limit once *)
OxygenDemand.Update(MeasuredValue := REAL#2.5);
AerationPulse.SetEnabled(Enabled := OxygenDemand.Q);

(* Hold inside window for many scans - Q should stay latched TRUE *)
FOR I := 1 TO 20 DO
    OxygenDemand.Update(MeasuredValue := REAL#1.5);
    AerationPulse.SetEnabled(Enabled := OxygenDemand.Q);
END_FOR;
ASSERT_TRUE(IN := OxygenDemand.Q);
ASSERT_TRUE(IN := AerationPulse.Enabled);
ASSERT_TRUE(IN := OxygenDemand.Window);
END_TEST_PROGRAM

(* Boundary test: exact limit values. The implementation uses strict
   less-than for low and strict greater-than for high, so values at
   the limits land in the window. *)
TEST_PROGRAM WastewaterAerationExactLimitsLandInWindow
VAR
    OxygenDemand : HysteresisSwitch;
    DemandLimits : RealRange;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
OxygenDemand.Initialize();
OxygenDemand.SetLimits(Limits := DemandLimits);

(* Exactly at HIGH limit -> not above -> stays in default Q (FALSE) and Window TRUE *)
OxygenDemand.Update(MeasuredValue := REAL#2.0);
ASSERT_TRUE(IN := OxygenDemand.Window);
ASSERT_FALSE(IN := OxygenDemand.Q);

(* Above HIGH limit -> Q latches TRUE *)
OxygenDemand.Update(MeasuredValue := REAL#2.5);
ASSERT_TRUE(IN := OxygenDemand.Q);
ASSERT_FALSE(IN := OxygenDemand.Window);

(* Exactly at LOW limit -> Window again, Q holds latched value *)
OxygenDemand.Update(MeasuredValue := REAL#1.0);
ASSERT_TRUE(IN := OxygenDemand.Window);
ASSERT_TRUE(IN := OxygenDemand.Q);
END_TEST_PROGRAM

(* Per-FB isolation: drive a PulseGenerator without any hysteresis. The
   Enabled property should follow SetEnabled directly. *)
TEST_PROGRAM WastewaterAerationPulseGeneratorRespondsToEnableTransitions
VAR
    Pulse : PulseGenerator;
END_VAR

Pulse.Initialize();
Pulse.Configure(HighTime := T#2s, LowTime := T#3s);
ASSERT_FALSE(IN := Pulse.Enabled);

Pulse.SetEnabled(Enabled := TRUE);
ASSERT_TRUE(IN := Pulse.Enabled);

Pulse.SetEnabled(Enabled := FALSE);
ASSERT_FALSE(IN := Pulse.Enabled);
END_TEST_PROGRAM

(* Edge input: a measurement well below LOW limit on initial scan
   leaves Q at default FALSE (not latched on). *)
TEST_PROGRAM WastewaterAerationDeepLowInitialMeasurementHoldsQFalse
VAR
    OxygenDemand : HysteresisSwitch;
    DemandLimits : RealRange;
END_VAR

DemandLimits.Low := REAL#1.0;
DemandLimits.High := REAL#2.0;
OxygenDemand.Initialize();
OxygenDemand.SetLimits(Limits := DemandLimits);

OxygenDemand.Update(MeasuredValue := REAL#-5.0);
ASSERT_FALSE(IN := OxygenDemand.Q);
ASSERT_FALSE(IN := OxygenDemand.Window);
END_TEST_PROGRAM
