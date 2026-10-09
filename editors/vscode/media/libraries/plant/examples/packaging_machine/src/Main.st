(* Packaging line: an infeed conveyor carries products past a product sensor and the inspection
   result; RejectSequence pushes bad products off with a single-acting reject cylinder. *)
USING Plant;

PROGRAM Main
VAR
    (* Hardware signals, mapped to I/O in Configuration.st. *)
    StartButton : BOOL;
    StopButton : BOOL;
    ProductSensor : BOOL;
    InspectionFailed : BOOL;
    ConveyorRunning : BOOL;
    ConveyorTrip : BOOL;
    CylinderExtendedSensor : BOOL;
    CylinderRetractedSensor : BOOL;
    ConveyorRun : BOOL;
    RejectValve : BOOL;

    Product : Plant.FB_DigitalInput := (Debounce := T#20ms);
    Conveyor : Plant.FB_Conveyor;
    Reject : Plant.FB_Cylinder := (ExtendTimeout := T#1s, RetractTimeout := T#1s);
    Line : RejectSequence;
    StartEdge : R_TRIG;
    StopEdge : R_TRIG;
END_VAR
    Product(Input := ProductSensor);
    StartEdge(CLK := StartButton);
    StopEdge(CLK := StopButton);
    IF StartEdge.Q THEN
        Line.Start();
    END_IF;
    IF StopEdge.Q THEN
        Line.Stop();
    END_IF;
    Line(ProductPresent := Product.Value, InspectionFailed := InspectionFailed,
        CylinderExtended := Reject.Extended, CylinderRetracted := Reject.Retracted);
    Conveyor(AutoRun := Line.RunConveyor, RunningFeedback := ConveyorRunning, Trip := ConveyorTrip);
    Reject(AutoExtend := Line.ExtendCylinder, ExtendedSensor := CylinderExtendedSensor,
        RetractedSensor := CylinderRetractedSensor);
    ConveyorRun := Conveyor.RunCommand;
    RejectValve := Reject.ExtendCommand;
END_PROGRAM
