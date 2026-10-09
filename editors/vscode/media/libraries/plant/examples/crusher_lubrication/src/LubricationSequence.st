(* Establishes lubrication before the crusher may run: the duty pump runs, then oil flow and
   pressure must be proven within ten seconds; losing either afterwards fails the sequence. The
   sequence commands the pumps through RunPumps, wired to the pump group in Main. *)
USING Plant;

FUNCTION_BLOCK LubricationSequence EXTENDS Plant.FB_Sequence
VAR_INPUT
    PumpRunning : BOOL;
    OilFlow : BOOL;
    Pressure : LREAL;
    MinimumPressure : LREAL := 2.0;
END_VAR
VAR_OUTPUT
    RunPumps : BOOL;
    CrusherPermit : BOOL;
END_VAR
VAR_TEMP
    Proven : BOOL;
END_VAR
    Proven := PumpRunning AND OilFlow AND Pressure >= MinimumPressure;
    IF Execute() THEN
        CASE CurrentStep OF
            0:
                GoTo(Number := 10, Name := 'Establish lubrication');
            10:
                Wait(Text := 'Waiting for a running pump, oil flow and pressure');
                IF Proven THEN
                    GoTo(Number := 20, Name := 'Lubrication ready');
                ELSE
                    StepTimeout(Limit := T#10s, Text := 'Lubrication did not establish within 10 s');
                END_IF;
            20:
                IF NOT Proven THEN
                    Fail(Text := 'Oil flow or pressure lost');
                END_IF;
        END_CASE;
    ELSIF State = E_SequenceState#Stopping THEN
        Wait(Text := 'Stopping the pumps');
        IF NOT PumpRunning THEN
            StopDone();
        END_IF;
    END_IF;
    (* The pumps keep running while the sequence is held; every other state stops them. *)
    RunPumps := State = E_SequenceState#Running OR State = E_SequenceState#Held;
    CrusherPermit := State = E_SequenceState#Running AND CurrentStep = 20;
END_FUNCTION_BLOCK
