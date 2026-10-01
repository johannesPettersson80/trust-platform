(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM TheHoldingCommandsShowTheCapacity
(* u = [H·(T − 20) + D]/2: 60, 75 and 90 % hold 35 °C; the 36 °C step needs up to 96 % and the 10 kW extraction up to 95 %.
   38 °C at 12 kW/°C would need 108 %: no tuning reaches it with this heater. *)
ASSERT_NEAR(EXPECTED := LREAL#60.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#8.0, TemperatureC := LREAL#35.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#90.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#12.0, TemperatureC := LREAL#35.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#96.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#12.0, TemperatureC := LREAL#36.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#95.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#12.0, TemperatureC := LREAL#35.0, ExtractionKw := LREAL#10.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#108.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#12.0, TemperatureC := LREAL#38.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheEnvelopeBoundsEveryHeater
(* W(0) = 2/8 = 0.25 and W(0.2) = 2/√164 + 0.08 = 0.23617. The reference heater has no error; every heater of the grid stays
   within W at every sampled frequency. *)
VAR
    H : INT;
    L : INT;
    K : INT;
    Omega : LREAL;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := ModelErrorBound(OmegaRadS := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2361738, ACTUAL := ModelErrorBound(OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := RelativeModelError(OmegaRadS := LREAL#0.2, LossKwPerC := LREAL#10.0, DelayS := LREAL#0.8), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2256153, ACTUAL := RelativeModelError(OmegaRadS := LREAL#0.2, LossKwPerC := LREAL#8.0, DelayS := LREAL#1.2), DELTA := LREAL#1.0e-7);
FOR H := 8 TO 12 BY 2 DO
    FOR L := 8 TO 12 BY 2 DO
        FOR K := 0 TO 60 DO
            Omega := LREAL#0.001 * EXPT(LREAL#10.0, INT_TO_LREAL(K) / LREAL#10.0);
            ASSERT_TRUE(IN := RelativeModelError(OmegaRadS := Omega, LossKwPerC := INT_TO_LREAL(H), DelayS := INT_TO_LREAL(L) / LREAL#10.0)
                <= ModelErrorBound(OmegaRadS := Omega));
        END_FOR;
    END_FOR;
END_FOR;
END_TEST_PROGRAM

TEST_PROGRAM TheSmallGainConditionOfBothCandidates
(* Kp = 2.5, Ki = 0.5: |T0| = 0.1/√(0.01 + ω² − 0.2·ω·sin 0.8ω), and W·|T0| stays below the bound 0.25378 from 0.001 to
   1000 rad/s. Kp = 30, Ki = 6: at 1.701 rad/s W·|T0| = 1.447, above 1, so the condition fails. *)
VAR
    K : INT;
    Omega : LREAL;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#0.4787634, ACTUAL := ComplementaryMagnitude(OmegaRadS := LREAL#0.2, Kp := LREAL#2.5, Ki := LREAL#0.5), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.25378, ACTUAL := SQRT(LREAL#0.25 ** LREAL#2.0 + LREAL#0.04 ** LREAL#2.0 / LREAL#0.84), DELTA := LREAL#1.0e-5);
FOR K := 0 TO 120 DO
    Omega := LREAL#0.001 * EXPT(LREAL#10.0, INT_TO_LREAL(K) / LREAL#20.0);
    ASSERT_TRUE(IN := ModelErrorBound(OmegaRadS := Omega) * ComplementaryMagnitude(OmegaRadS := Omega, Kp := LREAL#2.5, Ki := LREAL#0.5)
        <= LREAL#0.25378);
END_FOR;
ASSERT_NEAR(EXPECTED := LREAL#1.4469779, ACTUAL := ModelErrorBound(OmegaRadS := LREAL#1.701) * ComplementaryMagnitude(OmegaRadS := LREAL#1.701, Kp := LREAL#30.0, Ki := LREAL#6.0), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
