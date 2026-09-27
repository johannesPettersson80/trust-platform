# 61. Vessel commissioning

You will choose and commission a controller for the jacketed vessel. You will write its ST integration, compare two suitable arrangements, and defend the choice using temperature requirements, actuator limits and recovery behavior.

## The problem

The vessel may need disturbance rejection, cooling, a jacket-temperature constraint, or a combination. Each adds a different requirement. Your task is to select the arrangement that meets the stated application, rather than include every available technique.

## What we need

We need to choose and verify the complete control arrangement required by
the vessel's actual temperature, actuator and constraint requirements.
Knowing several techniques does not tell us which combination is justified.

## The new idea

**Vessel commissioning** checks whether a chosen combination of control
structures meets the vessel's performance, constraint and operating requirements.
It combines capabilities already taught: a single loop measures vessel
temperature, cascade adds faster jacket feedback, split-range supplies heating
or cooling, and override gives a constraint authority over heating demand.

Select two arrangements capable of addressing your chosen application. Explain each controller’s measurement and output, where limits and selection occur, and which state must track the applied output. Calculate the steady heat and actuator demand for each event before tuning. If a requirement is infeasible, identify that from the balance and propose an explicit requirement or equipment change.

Reuse [cascade control](../../09-industrial-control-structures/52-cascade/README.md) and [controller initialization, modes and validity](../../03-production-quality-digital-pid/39-controller-mode-initialization/README.md) through measurement recovery. Include [split-range control](../../09-industrial-control-structures/54-split-range/README.md) if you choose cooling, and [override control](../../09-industrial-control-structures/55-override-control/README.md) if you choose the jacket constraint.

Use the taught vessel: jacket capacity 5 kJ/°C, vessel capacity 50 kJ/°C, jacket-to-vessel conductance 10 kW/°C, vessel loss 2 kW/°C to 20 °C, and 2 kW per heater percentage point. Start at vessel 35 °C, jacket 38 °C and heater 15%, with .1 s controller updates.

Keep these baseline requirements for both candidates:

| Separate 60 s test | Requirement |
| --- | --- |
| Vessel setpoint 35→37 °C | Peak at most 38 °C; error within ±.2 °C from 30–60 s |
| Jacket extraction 30 kW at vessel setpoint 35 °C | Minimum at least 34 °C; error within ±.2 °C from 30–60 s |

Before choosing candidates, decide whether your application also requires the taught excess-heat case, jacket limit, or both. Copy the corresponding requirements and actuator capacities from those lessons without changing them between candidates. Ordinary simulation bounds are jacket 20–55 °C and vessel 30–40 °C; the override application additionally requires jacket at most 42 °C.

## Prediction

Predict the first command after each event and the temperature that should change first. Identify a result that would reject each candidate. Select your gains and predict a specific benefit and cost of the second arrangement.

## Implementation

Write the integration in the lesson’s `src/Main.st`, reusing the stateful PI interface. Use named units for °C, kW and percent. Implement one actuator owner, initialization, manual/automatic transfer, limits, tracking, measurement validation and recovery.

Explain your initialization values. A controller producing heater percent may start with integral 15%; a controller producing net heat starts at 30 kW; an outer cascade controller starts at 38 °C. These values have different meanings and cannot be exchanged. Specify meaningful checks for your own ST, including a limited request and a mode transfer.

The prepared alternatives are the taught single-loop PI and cascade.
Set symbolic `UseCascade` FALSE or TRUE while stopped, then restart for each
independent trial. The single-loop gains are2.5/.1; cascade uses outer.5/.12
and inner5/10. These are starting candidates, not claims that both qualify.
`TrialKind` at `%MW192` selects0 for the target step,1 for30kW jacket extraction,
or2 for no event during the separate mode-transfer check. `ManualRequested`
selects manual15% heating. With unchanged setpoint and measurements, returning
to Auto must preserve that holding output. `SafetyAbort` commands zero heat
and extraction and remains latched after measurements recover. Stop, restore
the full35/38°C state and15% holding command, then explicitly clear the latch.

`ReactorTrain` reuses the stateful `Control.PID` interface. The shared library's
read-only `GetState` method copies its retained state for the P/I display.
`NominalPowerPct` is the tracking-corrected nominal PI output before its final
output clamp; it is labelled separately from applied heating. Inactive
controllers follow the selected states through the interface's manual mode.
Cooling and the42°C jacket-override case remain optional application extensions;
the prepared baseline does not claim those capabilities.

## Experiment in Control Studio

**Workflow verification remains open.** The executable project now uses the
specified physical case and Control Studio stages are accessible. The complete
lesson-specific candidate selection and validation interactions have not yet
been verified with these actual-runtime records. Keep the records; ST
calculations do not substitute for the required Control Studio analysis.

1. State the two candidate arrangements, parameter values and decision rules before opening results. Reset all physical and controller states, including the inactive controller states, for every comparison.
2. Run both baseline events for each candidate through **Test**. Use **Confirm and start** as time zero. Add the selected cooling or constraint cases with the exact event timing taught in their lessons.
3. Record vessel and jacket temperatures, requirements, P/I terms, requests, applied service commands, selected controller and validity. Use **Validate** to compare saved records against each requirement.
4. Separately test manual/automatic transfer and one invalid-temperature event using the lifecycle lessons’ timing and recovery rules. Evaluate output continuity and recovery independently of the thermal tests.
5. Stop on a stated temperature or applied-output violation, failed service exclusion, failed fault response, or invalid clock. Command zero services, remove the disturbance source, and stop the simulator if needed. Restore the complete starting state before each repeat.

## Evidence and decision

Defend a choice with the results for every requirement, including actuator use and fault behavior. Explain whether the extra controller or actuator improved a required capability. If neither qualifies, report the failed requirements and the next bounded investigation; do not choose a winner simply because one trace looks smoother.

## Conclusion

The assessment establishes that you can turn vessel requirements into a coherent ST control arrangement, calculate its feasibility, commission it through matched experiments, and justify the result.

## Reason for the next lesson

The assessment regulates one product-vessel temperature, using the jacket measurement to support that requirement. The next process has two temperatures with independently chosen setpoints, and heat transfer between the vessels means each heater can affect both measurements. Choosing two controllers without measuring those cross effects would leave the pairing decision unsupported.

[Multivariable process paths](../../10-multivariable-control/61-mimo-paths/README.md) derives and measures all four heater-to-temperature paths before selecting the pairings.
