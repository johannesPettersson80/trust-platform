# Controller comparison reference

Use this page as a compact reference when choosing among actions already taught in the course.

| Controller | Use it when | Main limitation |
| --- | --- | --- |
| ON/OFF with hysteresis and dwell | The actuator has binary states and a temperature band is acceptable | The process cycles between boundaries |
| P | Proportional correction is sufficient for the required response, including any accepted steady-state error | Load or bias changes can leave steady-state error |
| PI | Integral action is needed to remove steady-state error caused by a persistent load | Integral state can wind up or respond slowly |
| PD | Position error must be corrected while measured motion needs damping | It cannot provide constant holding torque at zero error and reacts to measurement noise |
| PID | PI needs damping and measured derivative action is available and filtered appropriately | More state, noise sensitivity, and tuning/limit interactions |

Choose from the actuator, required steady error, persistent load, motion damping, measurement quality, and output limits. Use the [course guide](../../README.md) for each equipment path and its prerequisites. The [heater path](../../01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md) develops P and PI; the [motion path](../../02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md) develops P, PD and PID. Their assessments require you to justify the choice. This reference adds no new plant, controller, experiment, or numerical result.
