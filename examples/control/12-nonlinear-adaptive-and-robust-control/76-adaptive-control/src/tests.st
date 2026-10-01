(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheLeastSquaresEstimate
(* (x, y) = (15, 120) and (16, 128): Σxy = 3848 and Σx² = 481, so Ĥ = 8 kW/°C. Without data, no estimate. *)
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := LeastSquaresHeatLoss(SumXY := LREAL#3848.0, SumXX := LREAL#481.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LeastSquaresHeatLoss(SumXY := LREAL#0.0, SumXX := LREAL#0.0), DELTA := LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM TheOneSecondBalance
(* At rest at 35 °C with 60 points delivered: x = 15 °C and y = 120 kW. Warming from 35 to 35.4 °C over the second with 70
   points delivered: x = 15.2 °C and y = 140 − 50·0.4 = 120 kW. *)
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := BalanceX(StartC := LREAL#35.0, EndC := LREAL#35.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#120.0, ACTUAL := BalanceY(StartC := LREAL#35.0, EndC := LREAL#35.0, DeliveredPct := LREAL#60.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.2, ACTUAL := BalanceX(StartC := LREAL#35.0, EndC := LREAL#35.4), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#120.0, ACTUAL := BalanceY(StartC := LREAL#35.0, EndC := LREAL#35.4, DeliveredPct := LREAL#70.0), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheSteadyWindowIsRejected
(* Thirty seconds at 35 °C with 60 points give Ĥ = 8 kW/°C, but x never changes: a constant unknown extraction would fit
   just as well, so the window is rejected. *)
VAR
    Window : HeatLossWindow;
    Temperatures : ARRAY[0..30] OF LREAL;
    Delivered : ARRAY[0..29] OF LREAL;
    K : INT;
END_VAR
FOR K := 0 TO 30 DO
    Temperatures[K] := LREAL#35.0;
END_FOR;
FOR K := 0 TO 29 DO
    Delivered[K] := LREAL#60.0;
END_FOR;
Window(Temperatures := Temperatures, Delivered := Delivered);
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := Window.EstimateKwPerC, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Window.SpanC, DELTA := LREAL#1.0e-12);
ASSERT_FALSE(Window.Accepted);
END_TEST_PROGRAM
