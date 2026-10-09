# Packaging line

A conveyor, a debounced product sensor, a single-acting reject cylinder and `RejectSequence`,
an application block extending `FB_Sequence`. Start runs the line; a product marked bad by the
inspection is carried to the reject station and pushed off; Stop finishes the product on the way
and then stops the conveyor.

| Signal | Meaning |
|---|---|
| `StartButton`, `StopButton` | Start and stop the line |
| `ProductSensor`, `InspectionFailed` | Product present, and the inspection's verdict |
| `ConveyorRunning`, `ConveyorTrip` | Conveyor contactor feedback and overload |
| `CylinderExtendedSensor`, `CylinderRetractedSensor` | Reject cylinder end sensors |
| `ConveyorRun`, `RejectValve` | Conveyor contactor and reject solenoid |

A cylinder that does not reach its end within 1 s latches a fault and the sequence fails with
the reason; Reset on the cylinder's panel clears it, and Reset on the sequence returns it to Idle.
