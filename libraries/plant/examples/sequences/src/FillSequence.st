(* A customer sequence: fill a tank to a target level, hold it there for a mixing time, then drain
   it. Hold pauses with the valves closed and the timer standing still; Stop drains the tank before
   it ends. The manager overview needs no change for a new sequence. *)
USING Plant;

FUNCTION_BLOCK FillSequence EXTENDS Plant.FB_Sequence
VAR_INPUT
    Level : LREAL;
    TargetLevel : LREAL := 2.0;
    EmptyLevel : LREAL := 0.1;
    MixingTime : TIME := T#5m;
END_VAR
VAR_OUTPUT
    OpenInlet : BOOL;
    OpenOutlet : BOOL;
    RunMixer : BOOL;
END_VAR
    OpenInlet := FALSE;
    OpenOutlet := FALSE;
    RunMixer := FALSE;
    IF Execute() THEN
        CASE CurrentStep OF
            0:
                GoTo(Number := 10, Name := 'Fill');
            10:
                OpenInlet := TRUE;
                Wait(Text := 'Filling to the target level');
                IF Level >= TargetLevel THEN
                    GoTo(Number := 20, Name := 'Mix');
                ELSE
                    StepTimeout(Limit := T#30m, Text := 'The tank did not fill within 30 min');
                END_IF;
            20:
                RunMixer := TRUE;
                IF StepTime >= MixingTime THEN
                    GoTo(Number := 30, Name := 'Drain');
                END_IF;
            30:
                OpenOutlet := TRUE;
                IF Level <= EmptyLevel THEN
                    Complete();
                ELSE
                    StepTimeout(Limit := T#30m, Text := 'The tank did not drain within 30 min');
                END_IF;
        END_CASE;
    ELSIF State = E_SequenceState#Stopping THEN
        OpenOutlet := TRUE;
        Wait(Text := 'Draining before stopping');
        IF Level <= EmptyLevel THEN
            StopDone();
        END_IF;
    END_IF;
END_FUNCTION_BLOCK
