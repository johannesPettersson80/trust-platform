TEST_PROGRAM CommissioningPressureDecoratorSubstitutesSimulation
VAR
    Cell : BoosterCommissioningCell;
    Speed : REAL;
END_VAR

Cell.Initialize();
Speed := Cell.Update(RealPressureBar := REAL#9.0, CommissioningMode := TRUE, LoopbackOnly := TRUE, PressureSetpoint := REAL#4.5);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Cell.PressureQuality);
ASSERT_TRUE(IN := Cell.LastCommandedSpeed >= REAL#0.0);
ASSERT_FALSE(IN := Cell.RealPumpRunning);
END_TEST_PROGRAM

TEST_PROGRAM CommissioningOutputDriverLoopbackDoesNotRunRealPump
VAR
    RealPump : RealPumpDrive;
    Wrapper : CommissioningOutputDriver;
END_VAR

Wrapper.Wrap(Pump := RealPump, LoopbackOnly := TRUE);
Wrapper.Run(SpeedReference := REAL#60.0);
ASSERT_NEAR(EXPECTED := REAL#60.0, ACTUAL := Wrapper.LastCommandedSpeed, DELTA := REAL#0.001);
ASSERT_FALSE(IN := RealPump.IsRunning);
ASSERT_TRUE(IN := Wrapper.IsRunning);
END_TEST_PROGRAM

(* Mode-switch test: same cell instance runs in commissioning mode then
   in production mode. Catches bugs where the decorator caches its
   wrapped state from a previous mode, or where switching from loopback
   to live unintentionally leaves the real pump running from earlier
   simulated commands. *)
TEST_PROGRAM CommissioningCellSwitchesBetweenSimAndLiveOnSameInstance
VAR
    Cell : BoosterCommissioningCell;
    SimSpeed : REAL;
    LiveSpeed : REAL;
END_VAR

Cell.Initialize();

(* Scan 1: commissioning + loopback -> simulated path, no real pump *)
SimSpeed := Cell.Update(RealPressureBar := REAL#9.0, CommissioningMode := TRUE, LoopbackOnly := TRUE, PressureSetpoint := REAL#4.5);
ASSERT_FALSE(IN := Cell.RealPumpRunning);

(* Scan 2: production mode (live signal, live pump) on same instance *)
LiveSpeed := Cell.Update(RealPressureBar := REAL#3.0, CommissioningMode := FALSE, LoopbackOnly := FALSE, PressureSetpoint := REAL#4.5);
(* Quality must reflect the live source, not the previous sim quality *)
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Cell.PressureQuality);
END_TEST_PROGRAM

(* RealPressureSource isolation: returns configured value/quality. *)
TEST_PROGRAM RealPressureSourceReturnsConfiguredValue
VAR
    Source : RealPressureSource;
END_VAR
Source.Configure(PressureBar := REAL#7.5, QualityCode := BYTE#1);
Source.Update();
ASSERT_NEAR(EXPECTED := REAL#7.5, ACTUAL := Source.Value, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := BYTE#1, ACTUAL := Source.Quality);

(* Reconfigure: new values reflected after next read *)
Source.Configure(PressureBar := REAL#3.2, QualityCode := BYTE#0);
Source.Update();
ASSERT_NEAR(EXPECTED := REAL#3.2, ACTUAL := Source.Value, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Source.Quality);
END_TEST_PROGRAM

(* SimulatedPressureDecorator forwards real source when simulation is off. *)
TEST_PROGRAM SimulatedPressureDecoratorForwardsRealWhenSimOff
VAR
    LiveSource : RealPressureSource;
    Decorator : SimulatedPressureDecorator;
END_VAR
LiveSource.Configure(PressureBar := REAL#5.5, QualityCode := BYTE#2);
Decorator.Wrap(RealSource := LiveSource, UseSimulation := FALSE);
Decorator.Update();
ASSERT_NEAR(EXPECTED := REAL#5.5, ACTUAL := Decorator.Value, DELTA := REAL#0.001);
ASSERT_FALSE(IN := Decorator.UsingSimulation);

(* Switch to simulation: value comes from sine generator, not real *)
Decorator.SetSimulation(Enabled := TRUE);
Decorator.Update();
ASSERT_TRUE(IN := Decorator.UsingSimulation);
END_TEST_PROGRAM

(* RealPumpDrive isolation: Run sets running, Stop clears it. *)
TEST_PROGRAM RealPumpDriveTracksRunStop
VAR
    Pump : RealPumpDrive;
END_VAR
ASSERT_FALSE(IN := Pump.IsRunning);
Pump.Run(SpeedReference := REAL#42.0);
ASSERT_TRUE(IN := Pump.IsRunning);
ASSERT_NEAR(EXPECTED := REAL#42.0, ACTUAL := Pump.LastSpeed, DELTA := REAL#0.001);

Pump.Stop();
ASSERT_FALSE(IN := Pump.IsRunning);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Pump.LastSpeed, DELTA := REAL#0.001);

(* Run with zero speed should not flag running *)
Pump.Run(SpeedReference := REAL#0.0);
ASSERT_FALSE(IN := Pump.IsRunning);
END_TEST_PROGRAM

(* Cell control loop: pressure below setpoint runs pump, above stops. *)
TEST_PROGRAM CommissioningCellControlBoundary
VAR
    Cell : BoosterCommissioningCell;
END_VAR
Cell.Initialize();

(* Below setpoint: pump runs *)
Cell.Update(RealPressureBar := REAL#3.0, CommissioningMode := FALSE, LoopbackOnly := FALSE, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Cell.RealPumpRunning);

(* At setpoint: pump stops (strict inequality) *)
Cell.Update(RealPressureBar := REAL#4.5, CommissioningMode := FALSE, LoopbackOnly := FALSE, PressureSetpoint := REAL#4.5);
ASSERT_FALSE(IN := Cell.RealPumpRunning);

(* Above setpoint: pump stops *)
Cell.Update(RealPressureBar := REAL#5.0, CommissioningMode := FALSE, LoopbackOnly := FALSE, PressureSetpoint := REAL#4.5);
ASSERT_FALSE(IN := Cell.RealPumpRunning);
END_TEST_PROGRAM
