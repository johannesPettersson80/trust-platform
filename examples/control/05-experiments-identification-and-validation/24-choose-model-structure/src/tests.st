(* @trustlsp:runtime-ignore — these are model calculations, not scheduled
   PLC instances or measurements from the heater. Run with ST: Run All Tests. *)
TEST_PROGRAM Lesson24BothStatesUsePreviousValues
VAR
    WaterDeviationC : LREAL := 0.0;
    SensorDeviationC : LREAL := 0.0;
    NextWaterDeviationC : LREAL;
    NextSensorDeviationC : LREAL;
    ElapsedS : LREAL := 0.1;
    ProcessGainCPerPoint : LREAL := 0.2;
    ProcessTimeS : LREAL := 5.0;
    SensorTimeS : LREAL := 2.0;
    DelayedCommandChangePct : LREAL := 10.0;
    UpdateIndex : INT;
END_VAR
(* The 10-point command has already passed the transport delay. *)
FOR UpdateIndex := 1 TO 2 DO
    NextWaterDeviationC := WaterDeviationC + ElapsedS
        * (ProcessGainCPerPoint * DelayedCommandChangePct - WaterDeviationC)
        / ProcessTimeS;
    NextSensorDeviationC := SensorDeviationC + ElapsedS
        * (WaterDeviationC - SensorDeviationC) / SensorTimeS;
    WaterDeviationC := NextWaterDeviationC;
    SensorDeviationC := NextSensorDeviationC;
    IF UpdateIndex = 1 THEN
        ASSERT_NEAR(WaterDeviationC, LREAL#0.04, LREAL#0.000001);
        ASSERT_NEAR(SensorDeviationC, LREAL#0.0, LREAL#0.000001);
    ELSE
        ASSERT_NEAR(WaterDeviationC, LREAL#0.0792, LREAL#0.000001);
        ASSERT_NEAR(SensorDeviationC, LREAL#0.002, LREAL#0.000001);
    END_IF;
END_FOR;
END_TEST_PROGRAM

TEST_PROGRAM Lesson24ContinuousWorkedPrediction
VAR
    TimeAfterDelayS : LREAL := 5.0;
    WaterDeviationC : LREAL;
    SensorDeviationC : LREAL;
END_VAR
(* At 6 s, five seconds have elapsed since the delayed step arrived.
   This closed-form worked example uses unequal time constants 5 s and 2 s. *)
WaterDeviationC := LREAL#2.0 * (LREAL#1.0 - EXP(-TimeAfterDelayS / LREAL#5.0));
SensorDeviationC := LREAL#2.0 * (LREAL#1.0
    - (LREAL#5.0 * EXP(-TimeAfterDelayS / LREAL#5.0)
       - LREAL#2.0 * EXP(-TimeAfterDelayS / LREAL#2.0)) / LREAL#3.0);
ASSERT_NEAR(WaterDeviationC, LREAL#1.2642411, LREAL#0.000001);
ASSERT_NEAR(SensorDeviationC, LREAL#0.8831819, LREAL#0.000001);
ASSERT_NEAR(WaterDeviationC - SensorDeviationC, LREAL#0.3810592, LREAL#0.000001);
END_TEST_PROGRAM
