(* Two tanks in series held by state feedback: the pump fills tank 1, which drains into tank 2. The law feeds both
   measured levels back to the pump and holds tank 2 at its setpoint. The gains are ordinary variables, so Control
   Studio can design, validate and apply them. *)
PROGRAM Main
VAR
    (* TRUE selects Auto; in Manual the pump delivers ManualM3PerS. *)
    FeedbackEnabled : BOOL := FALSE;
    ManualM3PerS : LREAL := 0.010;
    Tank2SetpointM : LREAL := 0.25;
    (* The law around the operating point: PumpM3PerS = 0.010 + N·(Tank2SetpointM − 0.25) − K1·(Tank1M − 0.25)
       − K2·(Tank2M − 0.25). K1, K2 and N are in m³/s per m. *)
    K1 : LREAL := 0.0;
    K2 : LREAL := 0.0;
    N : LREAL := 0.0;
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    PumpM3PerS : LREAL := 0.010;
END_VAR
VAR CONSTANT
    (* The operating point at which the tanks are identified: both at 0.25 m with the pump at 0.010 m³/s. *)
    OperatingFlowM3PerS : LREAL := 0.010;
    OperatingLevelM : LREAL := 0.25;
END_VAR

IF FeedbackEnabled THEN
    PumpM3PerS := OperatingFlowM3PerS + N * (Tank2SetpointM - OperatingLevelM)
        - K1 * (Tank1M - OperatingLevelM) - K2 * (Tank2M - OperatingLevelM);
ELSE
    PumpM3PerS := ManualM3PerS;
END_IF;
(* The pump delivers 0–0.020 m³/s. *)
PumpM3PerS := MIN(MAX(PumpM3PerS, LREAL#0.0), LREAL#0.020);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PumpM3PerS AT %QL0 : LREAL;
    P1.Tank1M AT %IL0 : LREAL;
    P1.Tank2M AT %IL8 : LREAL;
    P1.Tank2SetpointM AT %ML0 : LREAL;
    P1.ManualM3PerS AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
