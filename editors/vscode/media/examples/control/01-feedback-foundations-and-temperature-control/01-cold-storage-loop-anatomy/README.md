# 1. Control-loop elements

Before writing a controller, you will learn to identify:

1. what you want to control;
2. what you can measure;
3. what you can change; and
4. how the measurement responds to the change you make.

## The problem

A cold room should be at 2 °C, but its sensor reads 4 °C. The room is too warm.

We will map the relevant signals and run one manual experiment. Automatic control begins in Lesson 2.

## What we need

We need to connect the desired temperature, sensor, refrigeration command, and cold room. Then we must change the command and observe the temperature.

## The new idea

**Control-loop elements** are the parts that connect a desired value,
a measurement, a control decision, an actuator and the physical process.
We first identify those parts and observe the action's effect; automatic
feedback comes in the next lesson.

![The command, equipment, and measurement in the cold room](figures/cold-room-loop.svg)

The parts have standard control names:

| Question | Cold-room answer | Control name |
| --- | --- | --- |
| What do we want to control? | Room temperature | Controlled variable |
| What value do we want? | 2 °C | Setpoint (**SP**) |
| What can we measure? | Temperature-sensor reading | Process value (**PV**) |
| What can we change? | Refrigeration OFF or ON | Manipulated variable (**MV**) |
| What equipment applies the command? | Refrigeration system | Actuator |
| What equipment responds? | Cold room and contents | Process or plant |

When refrigeration switches ON, it removes heat from the room. Because the room and its contents store thermal energy, the measured temperature falls gradually rather than immediately.

The temperature error is the desired value minus the measured value:

$$e = SP - PV$$

The signed error is e = SP - PV, with SP and PV measured in °C. With
SP = 2.0 °C and PV = 4.0 °C, e = 2.0 - 4.0 = -2.0 °C. The negative
sign says that the room is warmer than
the desired value. This calculation describes the difference; this lesson's
fixed sequence does not use it to choose the command.

The manual command is a percentage on the simulator's 0–100% scale. OFF is
0%, ON is 100%, and the applied command equals the selected state in this
exercise. These are command units, not a claim that a real compressor can
deliver a continuously adjustable percentage of cooling.

Heat entering through the open door is a **disturbance**: an outside influence
that changes the temperature we want to control.

For this experiment, the program follows a fixed sequence: refrigeration ON for 20 s, then OFF. The door opens from 60 to 90 s, allowing more heat into the room. These times count from starting the experiment. You start the sequence; the program performs the changes. It does not switch refrigeration back ON just because the room becomes too warm.

## Prediction

Before running, predict:

1. When refrigeration switches ON, what will happen to the measured temperature, and why will the change be gradual?
2. Opening the door allows more heat to enter the room. What will happen to the measured temperature while refrigeration is OFF?
3. Will the refrigeration command react to the rising temperature? Why or why not?

Do not predict exact temperatures; that requires a mathematical model introduced later.

## Implementation

Open [`src/Main.st`](src/Main.st) and identify the setpoint, temperature measurement and refrigeration command.

**ST practice.** Declare the setpoint and measured temperature as `REAL` values, which permit fractional quantities, and the refrigeration state as a `BOOL`, whose values are `TRUE` and `FALSE`:

```st
VAR
    SetpointC : REAL := 2.0;
    MeasuredTemperatureC : REAL;
    RefrigerationOn : BOOL := FALSE;
END_VAR
```

Use the project’s temperature input as the measured value. `VAR` and `END_VAR` delimit the declarations. Each declaration gives a name and type; `:=` supplies an initial value. Write `RefrigerationOn := TRUE;` as an assignment in the program body, then write the corresponding OFF assignment. Explain why neither assignment changes the temperature measurement directly. During the experiment, compare the Boolean state with the displayed command encoding: FALSE corresponds to 0 and TRUE to 100.

## Experiment in Control Studio

1. Open this lesson's project and start **Simulator**. Open **Control Studio → Monitor → Response**. Display temperature setpoint, measured temperature, requested and applied refrigeration commands, and door state. The compressor is the part of the refrigeration system switched by these commands: 0 means OFF and 100 means ON; this experiment uses no intermediate power settings.
2. Confirm temperature setpoint 2 °C, measured temperature 4 °C, both commands OFF, and door closed. `P1.ExperimentEnabled` should be false, meaning the sequence has not started. State your predictions before Test; **Confirm and start** begins the Test-owned recording.
3. Open **Control Studio → Test**. Select **Observe without excitation**, leave the disabled amplitude at **0**, and set maximum duration to **120.1 s**. This covers cooling, warming with refrigeration OFF, and the door opening and closing.
4. Select **Review test**, then **Confirm and start**. This enables the fixed sequence; you do not switch refrigeration or change the setpoint during the run.
5. During the first 20 s, record how measured temperature changes with refrigeration ON. After it switches OFF, record what happens to temperature. From 60 to 90 s, watch the open door indication and check whether rising temperature makes the refrigeration command change.
6. At completion, confirm `P1.ExperimentEnabled` is false again, both commands are OFF, and the door is closed. Temperature can continue changing after the sequence ends.

To stop early, select **Abort test**. Also stop if temperature leaves −10 to 10 °C or the signals stop updating. Confirm refrigeration OFF; if the simulator does not respond, stop it with the debug toolbar's **Stop** button. Before repeating, stop and restart **Simulator** and confirm the original 4 °C temperature, closed door and OFF commands.

These are exercise limits for the simulator, not operating limits or protection for a real cold room.

## Evidence and decision

![The compressor running lowers temperature; the open door raises it again](figures/manual-cooling-response.svg)

- With the compressor ON, measured temperature falls from 4.00 °C to 0.84 °C by 20.0 s.
- After the compressor switches OFF, measured temperature rises to 2.84 °C by 40.0 s.
- With the door open and compressor OFF, measured temperature rises from 3.57 °C at 59.9 s to 5.46 °C at 89.9 s.

Under these conditions, switching refrigeration ON makes the measured temperature fall. We can accept that response direction for this simulator.

The [saved readings](figures/manual-cooling-response.csv) answer the first two predictions: temperature falls gradually while refrigeration removes heat and rises with refrigeration OFF, including while the open door admits extra heat. The room and its contents must lose or gain stored heat for their temperature to change.

The third prediction concerns the command: it stays OFF while temperature rises because the fixed sequence has no rule to switch ON when the room is too warm. Measuring temperature alone does not make the program control it. Check this against your recorded command and temperature traces.

## Conclusion

Before writing a controller, identify what you want to control, what you can measure, what you can change, and how the measurement responds.

Switching refrigeration ON lowers the measured room temperature.

## Reason for the next lesson

We now know that switching refrigeration ON lowers the room temperature. However, keeping the room near its temperature setpoint requires the refrigeration command to respond to the measured temperature.

[ON/OFF feedback](../02-cold-storage-on-off/README.md) introduces this feedback by using the measured temperature to switch refrigeration OFF or ON automatically.
