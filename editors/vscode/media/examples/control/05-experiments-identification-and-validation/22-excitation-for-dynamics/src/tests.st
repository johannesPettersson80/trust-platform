(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheSineAboutTheOperatingPoint
(* u = 75 + 5·sin(0.2·t): a period of 2π/0.2 = 31.416 s, 80 % a quarter period in, 70 % at three quarters. *)
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := SineCommand(OperatingPct := LREAL#75.0, AmplitudePct := LREAL#5.0,
    OmegaRadS := LREAL#0.2, ElapsedS := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := SineCommand(OperatingPct := LREAL#75.0, AmplitudePct := LREAL#5.0,
    OmegaRadS := LREAL#0.2, ElapsedS := LREAL#7.853981633974483), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#70.0, ACTUAL := SineCommand(OperatingPct := LREAL#75.0, AmplitudePct := LREAL#5.0,
    OmegaRadS := LREAL#0.2, ElapsedS := LREAL#23.56194490192345), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheExcitationRunsFivePeriods
VAR
    Excitation : PeriodicExcitation;
    Updates : INT := 0;
    Count : INT;
END_VAR
(* ω = 0.4 rad/s: five periods are 78.54 s, 786 updates of 0.1 s; afterwards the command is 75 % again. *)
Excitation(Start := TRUE, Enable := TRUE, AmplitudePct := LREAL#5.0, OmegaRadS := LREAL#0.4);
IF Excitation.Running THEN
    Updates := 1;
END_IF;
FOR Count := 1 TO 900 DO
    Excitation(Start := FALSE, Enable := TRUE, AmplitudePct := LREAL#5.0, OmegaRadS := LREAL#0.4);
    IF Excitation.Running THEN
        Updates := Updates + 1;
    END_IF;
END_FOR;
ASSERT_EQUAL(EXPECTED := 786, ACTUAL := Updates);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := Excitation.CommandPct, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM ARejectedSettingExcitesNothing
VAR
    Excitation : PeriodicExcitation;
END_VAR
(* 20 points would drive the heater between 55 % and 95 %: outside the 5–10 points allowed here. *)
Excitation(Start := TRUE, Enable := TRUE, AmplitudePct := LREAL#20.0, OmegaRadS := LREAL#0.2);
ASSERT_TRUE(IN := Excitation.Rejected);
ASSERT_FALSE(IN := Excitation.Running);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := Excitation.CommandPct, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
