(* @trustlsp:runtime-ignore — run this arithmetic project with trust-dev test;
   it does not drive the simulated heater. *)
FUNCTION WaterHeaterPIProportionalContribution : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    Error : LREAL;
END_VAR

(* Proportional action responds to the error measured in this scan. *)
WaterHeaterPIProportionalContribution := ProportionalGain * Error;
END_FUNCTION

FUNCTION WaterHeaterPIIntegralIncrement : LREAL
VAR_INPUT
    IntegralGain : LREAL;
    Error : LREAL;
    CycleTime : LREAL;
END_VAR

(* Integral action adds one small correction for the error that remained during
   this scan. Repeating this addition builds the holding command. *)
WaterHeaterPIIntegralIncrement := IntegralGain * Error * CycleTime;
END_FUNCTION

FUNCTION WaterHeaterPIApplyPhysicalRange : LREAL
VAR_INPUT
    RequestedHeaterPower : LREAL;
END_VAR

IF RequestedHeaterPower <= LREAL#0.0 THEN
    WaterHeaterPIApplyPhysicalRange := LREAL#0.0;
ELSIF RequestedHeaterPower >= LREAL#100.0 THEN
    WaterHeaterPIApplyPhysicalRange := LREAL#100.0;
ELSE
    WaterHeaterPIApplyPhysicalRange := RequestedHeaterPower;
END_IF;
END_FUNCTION


(* Arithmetic teaching checks only. The native Rust integral_action filter in
   control_curriculum_foundation_heater executes the delivered Main and verifies
   its real Control.PID instance, state, output ownership and plant experiment. *)
TEST_PROGRAM Lesson5PIKeepsAccumulatedCorrectionAtZeroError
VAR
    ProportionalContribution : LREAL;
    IntegralIncrement : LREAL;
    RequestedPower : LREAL;
END_VAR

ProportionalContribution := WaterHeaterPIProportionalContribution(ProportionalGain := LREAL#2.0, Error := LREAL#0.0);
IntegralIncrement := WaterHeaterPIIntegralIncrement(IntegralGain := LREAL#0.5, Error := LREAL#0.0, CycleTime := LREAL#0.1);
RequestedPower := LREAL#72.0 + ProportionalContribution + IntegralIncrement;

ASSERT_TRUE(IN := RequestedPower = LREAL#72.0);
END_TEST_PROGRAM

TEST_PROGRAM Lesson5PIAddsPresentAndAccumulatedCorrection
VAR
    ProportionalContribution : LREAL;
    IntegralIncrement : LREAL;
    RequestedPower : LREAL;
END_VAR

ProportionalContribution := WaterHeaterPIProportionalContribution(ProportionalGain := LREAL#2.0, Error := LREAL#5.0);
IntegralIncrement := WaterHeaterPIIntegralIncrement(IntegralGain := LREAL#0.5, Error := LREAL#5.0, CycleTime := LREAL#0.1);
RequestedPower := LREAL#75.0 + ProportionalContribution + IntegralIncrement;

ASSERT_TRUE(IN := ProportionalContribution = LREAL#10.0);
ASSERT_TRUE(IN := IntegralIncrement = LREAL#0.25);
ASSERT_TRUE(IN := RequestedPower = LREAL#85.25);
END_TEST_PROGRAM
