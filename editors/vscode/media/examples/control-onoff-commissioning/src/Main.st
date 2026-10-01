(* A cold room held near its setpoint by switching a compressor on and off. The switching band and the
   minimum ON and OFF times are ordinary retained variables, so Control Studio can tune and apply them. *)
PROGRAM Main
VAR
    (* TRUE selects Auto; the compressor starts off in Manual. *)
    AutoMode : BOOL := FALSE;
    ManualPct : LREAL := 0.0;
    SetpointC : LREAL := 5.0;
    (* Half the switching band: the compressor switches on above SetpointC + HysteresisC and off below
       SetpointC - HysteresisC. *)
    HysteresisC : LREAL := 1.0;
    MinimumOnS : LREAL := 20.0;
    MinimumOffS : LREAL := 20.0;
    RoomC : LREAL := 12.0;
    CompressorPct : LREAL := 0.0;
    (* Retained switching state: the latch and the time spent in the current state. *)
    CoolingOn : BOOL := FALSE;
    DwellS : LREAL := 0.0;
END_VAR

DwellS := DwellS + LREAL#0.1;
IF AutoMode THEN
    IF CoolingOn AND RoomC <= SetpointC - HysteresisC AND DwellS >= MinimumOnS THEN
        CoolingOn := FALSE;
        DwellS := LREAL#0.0;
    ELSIF NOT CoolingOn AND RoomC >= SetpointC + HysteresisC AND DwellS >= MinimumOffS THEN
        CoolingOn := TRUE;
        DwellS := LREAL#0.0;
    END_IF;
ELSIF CoolingOn <> (ManualPct >= LREAL#50.0) THEN
    CoolingOn := ManualPct >= LREAL#50.0;
    DwellS := LREAL#0.0;
END_IF;
IF CoolingOn THEN
    CompressorPct := LREAL#100.0;
ELSE
    CompressorPct := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.CompressorPct AT %QL0 : LREAL;
    P1.RoomC AT %IL0 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.ManualPct AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
