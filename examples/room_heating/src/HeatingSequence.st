// The room heating's start and stop. Start: P-101 starts and FS-101 confirms circulation, EH-101
// warms T-101, and once the tank is ready the room loop and the fan heat the room. Stop: room
// heating and the heater stop at once, and the pump runs on so the hot water leaves the heater.
// Low water or lost circulation fails the sequence; Reset returns it to Idle.
USING Plant;

FUNCTION_BLOCK HeatingSequence EXTENDS Plant.FB_Sequence
VAR_INPUT
    (* T-101 has water: its low-water switch is clear. *)
    WaterOk : BOOL;
    (* FS-101 confirms circulation. *)
    Flow : BOOL;
    TankTemperature : LREAL;
END_VAR
VAR_OUTPUT
    PumpRun : BOOL;
    HeaterRun : BOOL;
    RoomHeating : BOOL;
END_VAR
VAR PUBLIC RETAIN
    (* The tank temperature at which room heating starts, in °C. *)
    TankReady : LREAL := 40.0;
    PumpOverrun : TIME := T#30s;
END_VAR
VAR
    LostFlow : TON;
END_VAR
    IF Execute() AND NOT WaterOk THEN
        Fail(Text := 'T-101 water low');
    END_IF;
    LostFlow(IN := State = E_SequenceState#Running AND CurrentStep >= 20 AND NOT Flow, PT := T#10s);
    IF LostFlow.Q THEN
        Fail(Text := 'Circulation lost: FS-101 sees no flow');
    END_IF;
    CASE State OF
        E_SequenceState#Running:
            CASE CurrentStep OF
                0:
                    GoTo(Number := 10, Name := 'Start circulation');
                10:
                    Wait(Text := 'Waiting for FS-101 to confirm flow');
                    IF Flow THEN
                        GoTo(Number := 20, Name := 'Heat the tank');
                    ELSE
                        StepTimeout(Limit := T#20s, Text := 'No flow on FS-101 after P-101 started');
                    END_IF;
                20:
                    Wait(Text := 'Waiting for T-101 to reach the ready temperature');
                    IF TankTemperature >= TankReady THEN
                        GoTo(Number := 30, Name := 'Heat the room');
                    END_IF;
            END_CASE;
            PumpRun := TRUE;
            HeaterRun := CurrentStep >= 20;
            RoomHeating := CurrentStep >= 30;
        E_SequenceState#Stopping:
            IF CurrentStep < 100 THEN
                GoTo(Number := 100, Name := 'Pump overrun');
            END_IF;
            Wait(Text := 'P-101 runs on to carry the heat away');
            HeaterRun := FALSE;
            RoomHeating := FALSE;
            PumpRun := StepTime < PumpOverrun;
            IF NOT PumpRun THEN
                StopDone();
            END_IF;
        E_SequenceState#Held:
            (* Held: the tank stays warm and the room is not heated. *)
            RoomHeating := FALSE;
    ELSE
        PumpRun := FALSE;
        HeaterRun := FALSE;
        RoomHeating := FALSE;
    END_CASE;
END_FUNCTION_BLOCK
