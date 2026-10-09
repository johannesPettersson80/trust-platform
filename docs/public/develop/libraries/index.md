# Libraries

## Shipped Library Families

- [OSCAT](oscat.md)
- [OSCAT OOP](oscat-oop.md)
- [PLCopen Motion](plcopen-motion.md)
- [PLCopen Motion OOP](plcopen-motion-oop.md)
- [Control](control.md)
- [Alarm](alarm.md)
- [HMI](hmi.md)
- [Comm](comm.md)
- [Plant](plant.md)

## Which One To Open

- Use OSCAT when you need general-purpose reusable IEC utility blocks and functions.
- Use OSCAT OOP when OSCAT-backed application code should be organized as objects with narrow interfaces.
- Use PLCopen Motion when the workload is axis control, sequencing, or motion-profile examples.
- Use PLCopen Motion OOP when an application should model axes and commands through `itfAxis` and returned command objects.
- Use Control for feedback loops: `Control.PID` for P, PI, PD and PID control, `Control.LowPass` for filtering, and the Smith predictor, gain scheduler and decoupler for harder plants.
- Use Alarm when the program raises alarms that operators acknowledge, shelve and reset in the runtime display, with the history in OpenOT.
- Use HMI in every project with screens: its `Hmi.Display` block, called in a task, serves them.
- Use Comm when the program and the operator must know whether each communication link works, since when and why it failed.
- Use Plant when a program commands a plant's motors, pumps, valves, instruments and loops and an operator works them from the HMI's panels.
- If the question is about third-party vendor libraries rather than shipped libraries, go to [Vendor Libraries](../../migrate/vendor-libraries.md).

## Related

- [Project Layout](../project-layout.md)
- [Structured Text Naming Standard](../st-naming-standard.md)
- [Vendor Libraries](../../migrate/vendor-libraries.md)
- [Libraries and Motion examples](../../examples/libraries-and-motion.md)
