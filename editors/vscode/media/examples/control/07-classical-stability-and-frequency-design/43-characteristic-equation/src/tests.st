(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheCharacteristicEquationOfTheShaft
VAR
    A1 : LREAL;
    A0 : LREAL;
END_VAR
(* J = B = 1, Kp = 16, Kd = 7: s² + 8s + 16 = (s + 4)², a repeated root because a1² = 4·a0. *)
A1 := DampingCoefficientOf(Inertia := LREAL#1.0, Friction := LREAL#1.0, DerivativeGain := LREAL#7.0);
A0 := StiffnessCoefficientOf(Inertia := LREAL#1.0, ProportionalGain := LREAL#16.0);
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := A1, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#16.0, ACTUAL := A0, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := A1 * A1 - LREAL#4.0 * A0, DELTA := LREAL#1.0e-12);
(* Kp = 4: s² + 8s + 4, the distinct roots −4 ± √12 = −0.536 and −7.464 s⁻¹. *)
A0 := StiffnessCoefficientOf(Inertia := LREAL#1.0, ProportionalGain := LREAL#4.0);
ASSERT_NEAR(EXPECTED := LREAL#-0.53590, ACTUAL := -A1 / LREAL#2.0 + SQRT(A1 * A1 / LREAL#4.0 - A0), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#-7.46410, ACTUAL := -A1 / LREAL#2.0 - SQRT(A1 * A1 / LREAL#4.0 - A0), DELTA := LREAL#1.0e-5);
(* A derivative term added with the wrong sign: s² − 6s + 16, real part +3 s⁻¹, a growing swing. *)
ASSERT_NEAR(EXPECTED := LREAL#3.0,
    ACTUAL := -DampingCoefficientOf(Inertia := LREAL#1.0, Friction := LREAL#1.0, DerivativeGain := LREAL#-7.0) / LREAL#2.0,
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheRepeatedRootAfterAKnock
(* From z = 0 with 1 rad/s: z = t·e^(−4t), largest at t = 1/4 s with 1/(4e) = 0.09197 rad, where the velocity is zero. *)
ASSERT_NEAR(EXPECTED := LREAL#0.091970,
    ACTUAL := RepeatedRootDisplacement(Displacement := LREAL#0.0, Velocity := LREAL#1.0, Root := LREAL#-4.0, Seconds := LREAL#0.25),
    DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := RepeatedRootVelocity(Displacement := LREAL#0.0, Velocity := LREAL#1.0, Root := LREAL#-4.0, Seconds := LREAL#0.25),
    DELTA := LREAL#1.0e-12);
(* From 0.1 rad at rest: z = 0.1·(1 + 4t)·e^(−4t), 0.3·e^(−2) = 0.0406 rad at 0.5 s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.3 * EXP(LREAL#-2.0),
    ACTUAL := RepeatedRootDisplacement(Displacement := LREAL#0.1, Velocity := LREAL#0.0, Root := LREAL#-4.0, Seconds := LREAL#0.5),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheSampledLawSplitsTheRoot
VAR
    Slow : LREAL;
    Fast : LREAL;
END_VAR
(* P 10 ms late and D 20 ms late: 0.86·s² + 7.84·s + 16 = 0, the roots −3.084 and −6.032 s⁻¹. *)
Slow := QuadraticRoot(SquareCoefficient := LREAL#0.86, LinearCoefficient := LREAL#7.84, ConstantCoefficient := LREAL#16.0, Sign := LREAL#1.0);
Fast := QuadraticRoot(SquareCoefficient := LREAL#0.86, LinearCoefficient := LREAL#7.84, ConstantCoefficient := LREAL#16.0, Sign := LREAL#-1.0);
ASSERT_NEAR(EXPECTED := LREAL#-3.0843, ACTUAL := Slow, DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#-6.0319, ACTUAL := Fast, DELTA := LREAL#1.0e-4);
(* From z = 0 with 1 rad/s: z = (e^(p1·t) − e^(p2·t))/(p1 − p2), which starts at 0 with the slope 1. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := TwoRootDisplacement(Displacement := LREAL#0.0, Velocity := LREAL#1.0, FirstRoot := Slow, SecondRoot := Fast, Seconds := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := TwoRootVelocity(Displacement := LREAL#0.0, Velocity := LREAL#1.0, FirstRoot := Slow, SecondRoot := Fast, Seconds := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := (EXP(Slow * LREAL#0.3) - EXP(Fast * LREAL#0.3)) / (Slow - Fast),
    ACTUAL := TwoRootDisplacement(Displacement := LREAL#0.0, Velocity := LREAL#1.0, FirstRoot := Slow, SecondRoot := Fast, Seconds := LREAL#0.3),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheHeaterKeepsBothModes
VAR
    X : LREAL;
    I : LREAL;
    NextX : LREAL;
    Count : INT;
END_VAR
(* x(0) = 0.1 °C and i(0) = 0: the rates are −0.03 °C/s and −0.05 %/s. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.03, ACTUAL := HeaterTemperatureRate(TemperatureDeviationC := LREAL#0.1, IntegralDeviationPct := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.05, ACTUAL := HeaterIntegralRate(TemperatureDeviationC := LREAL#0.1), DELTA := LREAL#1.0e-12);
(* Both states advanced together in 1 ms steps follow x = −0.1·e^(−0.1t) + 0.2·e^(−0.2t): 0.01292 °C at 5 s and
   −0.00972 °C at 10 s. *)
X := LREAL#0.1;
I := LREAL#0.0;
FOR Count := 1 TO 10000 DO
    NextX := X + LREAL#0.001 * HeaterTemperatureRate(TemperatureDeviationC := X, IntegralDeviationPct := I);
    I := I + LREAL#0.001 * HeaterIntegralRate(TemperatureDeviationC := X);
    X := NextX;
    IF Count = 5000 THEN
        ASSERT_NEAR(EXPECTED := LREAL#0.01292, ACTUAL := X, DELTA := LREAL#1.0e-4);
    END_IF;
END_FOR;
ASSERT_NEAR(EXPECTED := LREAL#-0.00972, ACTUAL := X, DELTA := LREAL#1.0e-4);
(* With the integral left out, x = 0.1·e^(−0.3t) stays above the setpoint: 0.00498 °C at 10 s. *)
X := LREAL#0.1;
FOR Count := 1 TO 10000 DO
    X := X + LREAL#0.001 * HeaterTemperatureRate(TemperatureDeviationC := X, IntegralDeviationPct := LREAL#0.0);
END_FOR;
ASSERT_NEAR(EXPECTED := LREAL#0.00498, ACTUAL := X, DELTA := LREAL#1.0e-4);
END_TEST_PROGRAM
