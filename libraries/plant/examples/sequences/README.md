# Customer sequences

`FillSequence` extends `FB_Sequence`: its body calls `Execute()` and writes its steps as a CASE on
`CurrentStep`, using `GoTo`, `Wait`, `StepTimeout`, `Complete` and `StopDone`. Its outputs are ordinary
outputs the program wires to the valves' and mixers' `AutoOpen` and `AutoRun`. An
`FB_SequenceManager` shows both tanks' sequences and can stop or abort them together; adding a
sequence changes only the manager's call.

Hold closes the valves and stops the mixer while the step time stands still; Resume continues.
Stop drains the tank before the sequence reports Stopped.
