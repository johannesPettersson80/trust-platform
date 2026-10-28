(* A temperature process held by the program's own PI, in parallel form:
   HeaterPct = Kp * (SetpointC - TemperatureC) + Integral, where Integral adds Ki * error every second.
   In Manual the output is ManualPct and Integral follows it, so switching to Auto does not bump the output. *)
PROGRAM Main
VAR
    AutoMode : BOOL := FALSE;
    ManualPct : LREAL := 0.0;
    SetpointC : LREAL := 20.0;
    Kp : LREAL := 1.0;
    Ki : LREAL := 0.2;
    Integral : LREAL := 0.0;
    TemperatureC : LREAL := 20.0;
    HeaterPct : LREAL := 0.0;
    ErrorC : LREAL := 0.0;
END_VAR
VAR CONSTANT
    CycleS : LREAL := 0.1;
END_VAR

ErrorC := SetpointC - TemperatureC;
IF AutoMode THEN
    Integral := MIN(MAX(Integral + Ki * ErrorC * CycleS, LREAL#0.0), LREAL#100.0);
    HeaterPct := Kp * ErrorC + Integral;
ELSE
    Integral := ManualPct;
    HeaterPct := ManualPct;
END_IF;
HeaterPct := MIN(MAX(HeaterPct, LREAL#0.0), LREAL#100.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SetpointC AT %ML0 : LREAL;
    P1.ManualPct AT %ML8 : LREAL;
    P1.HeaterPct AT %QL0 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
