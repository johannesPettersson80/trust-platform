(* Rejects products the inspection marks bad: carries each rejected product to the reject station,
   extends the reject cylinder, and waits for it to retract. A good product passes. *)
USING Plant;

FUNCTION_BLOCK RejectSequence EXTENDS Plant.FB_Sequence
VAR_INPUT
    ProductPresent : BOOL;
    InspectionFailed : BOOL;
    CylinderExtended : BOOL;
    CylinderRetracted : BOOL;
    (* The time a product needs from the sensor to the reject station. *)
    TravelTime : TIME := T#800ms;
END_VAR
VAR_OUTPUT
    RunConveyor : BOOL;
    ExtendCylinder : BOOL;
    Passed : UDINT;
    Rejected : UDINT;
END_VAR
    ExtendCylinder := FALSE;
    IF Execute() THEN
        CASE CurrentStep OF
            0:
                GoTo(Number := 10, Name := 'Wait for a product');
            10:
                Wait(Text := 'Waiting for a product');
                IF ProductPresent THEN
                    IF InspectionFailed THEN
                        GoTo(Number := 20, Name := 'Carry to the reject station');
                    ELSE
                        GoTo(Number := 40, Name := 'Product passes');
                    END_IF;
                END_IF;
            20:
                IF StepTime >= TravelTime THEN
                    GoTo(Number := 30, Name := 'Reject');
                END_IF;
            30:
                ExtendCylinder := TRUE;
                IF CylinderExtended THEN
                    Rejected := Rejected + 1;
                    GoTo(Number := 35, Name := 'Retract');
                ELSE
                    StepTimeout(Limit := T#3s, Text := 'The reject cylinder did not extend');
                END_IF;
            35:
                IF CylinderRetracted THEN
                    GoTo(Number := 10, Name := 'Wait for a product');
                ELSE
                    StepTimeout(Limit := T#3s, Text := 'The reject cylinder did not retract');
                END_IF;
            40:
                IF NOT ProductPresent THEN
                    Passed := Passed + 1;
                    GoTo(Number := 10, Name := 'Wait for a product');
                END_IF;
        END_CASE;
    ELSIF State = E_SequenceState#Stopping THEN
        (* A stop finishes the current product first. *)
        IF CurrentStep = 10 OR CurrentStep = 40 THEN
            StopDone();
        END_IF;
    END_IF;
    RunConveyor := State = E_SequenceState#Running OR State = E_SequenceState#Stopping;
END_FUNCTION_BLOCK
