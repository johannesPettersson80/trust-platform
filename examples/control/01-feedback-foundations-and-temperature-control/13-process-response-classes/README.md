# 12. Process dynamics

The PI controller changes its output while the water warms. In this lesson, you will distinguish the complete control-loop response from the heater's response to an applied command, and identify what a process model must predict to help you choose gains.

## The problem

A PI record can show an acceptable temperature response while hiding the plant behavior that produced it. The controller is changing its output as error changes, so a late temperature movement may reflect both the heater and the controller. A poor response may likewise come from gains, capacity, delay, or a disturbance. Before changing gains or selecting a model, state the prediction that would distinguish the next choice.

## What we need

For a process model covering the intended operating range, identify three predictions for a known applied input:

1. the equilibrium temperature change caused by a command change;
2. the delay before the measured temperature responds; and
3. the way temperature evolves between the initial and final values.

Compare a prepared held-input record with the existing closed-loop records to identify what is missing. The temperature trace alone cannot separate the heater dynamics from the controller's changing commands.

## The new idea

**Process dynamics** describe how a physical process responds over time to
its applied input. A model represents that relationship so we can predict
what a known command will do. A closed-loop trace also includes the controller
continually changing that command, so it is not the plant response alone.

The process gain is the steady temperature change per applied command change:

$$
K=\frac{\Delta T_{steady}}{\Delta u}.
$$

Here $\Delta T_{steady}$ is the final temperature change in °C and $\Delta u$
the applied-command change in percentage points. Lesson 7 supplied
$T_{eq}=20+0.2u$, giving teaching gain $K=0.2$ °C/point. Held commands of
75% and 85% therefore predict equilibria of 35 °C and 37 °C. These are
calculations from the supplied relationship, not measurements from the
Lesson 9 or 11 feedback traces.

That calculation answers only the final-change question. It does not say when delivered heat reaches the vessel or how stored thermal energy makes PV move between equilibria. Lesson 13 derives those features from energy balance. Here we observe one supplied held-input demonstration. Lessons 14–15 later teach how to design a suitable experiment and estimate its model.

As water warms, its heat loss increases until it can balance a constant heater input. This tendency to approach a finite equilibrium is called **self-regulation**. It explains why a held command can produce a settled temperature. It does not yet explain how quickly the water approaches that temperature: for that prediction, we need the relationship between stored energy, heating and heat loss.

## Prediction

Before opening the records, write answers to these questions:

1. In a Lesson 9 setpoint or disturbance trial, which signals changed the applied command while PV was moving?
2. Can the temperature trace alone determine the heater's gain K, delay, and temperature evolution for a held command? Explain why or why not.
3. Using $T_{eq}=20+0.2u$, what final temperatures correspond to held 75% and 85% commands?
4. Which missing prediction would change your next controller decision: equilibrium capacity, response delay, or evolution between endpoints?
5. Why can the supplied gain predict a 37 °C equilibrium after a held 85% command without yet predicting the time required to approach it?

## Implementation

The prepared [ST program](src/Main.st) applies a known input rather than
calculating a temperature answer. It holds 75% before the first experiment,
maintaining the 35 °C starting condition and the one-second 75% delivery history.
When `P1.ExperimentEnabled` becomes true, it applies 85% for 30 s. PI is inactive,
so the input stays fixed while the water responds. `P1.TemperatureC` is the
simulator's measured temperature; `P1.AppliedPowerPct` is the actual command.

After completion or abort, the program removes heating. Its finished/fault
state remains latched; restart Simulator to restore the full hot state before
another trial. The task and recorded samples use 0.1 s intervals. The prepared
limits are 33–39 °C and 0–100% applied output.

Calculate your prediction on paper: $\Delta u=85-75=10$ points and
$\Delta T_{steady}=K\Delta u=0.2\times10=2$ °C. Thus the final temperature
should approach 37 °C. This calculation predicts the endpoint; it does not
manufacture a measured temperature or explain the shape of the response.

## Experiment in Control Studio

1. Start **Simulator** for this project, then open **Control Studio → Identify** and choose **Project experiment** from **Method**.
   Confirm measured temperature 35 °C, applied command 75%, and experiment
   authority false. The hot state must remain steady while you prepare.
2. In **Identify → Method → Project experiment**, click **Review project test** for the prepared 30.1 s observation. The
   program will hold 85% for 30 s; the extra 0.1 s allows its ending to be observed.
   The project supplies the step; do not select another excitation method.
3. Select **Confirm and start** on Identify. The project experiment automatically records the reviewed observation. Observe applied command jump to 85% while temperature initially remains near 35 °C.
4. Follow temperature after the delay. Check that input remains fixed while
   temperature gradually approaches 37 °C. Mark the first visible temperature
   change and the temperature near the end of the hold.
5. After restoration, confirm zero heating and inspect the complete response on Identify. Record the initial temperature, input change, final temperature and visible delay in your lesson notes before leaving the project.
6. Compare with your Lesson 11 PI observations, including applied power as well as temperature. In this held-input run, the input stays 85%; in the PI run, the controller keeps changing it. The endpoints differ, so use this comparison to separate process and feedback behavior, not to rank the two temperature responses.
7. Use the approximately 2 °C observed rise and 10-point input change to calculate
   $K\approx2/10=0.2$ °C/point. The finite 30 s record only approaches equilibrium;
   state that approximation. Identify which questions about timing remain.

Use **Abort test** if the stated bounds or data quality fail. Confirm zero
applied output and restart to the full hot state before repeating.

## Evidence and decision

The Lessons 9 and 11 traces can show whether a controller met temperature requirements, but their changing applied output means PV settling is not enough to identify plant dynamics. A useful next model must be tied to a known applied command and must predict equilibrium change, delay, and temperature evolution. The new held-input record approaches the 35→37 °C change predicted for 75→85%. Its delay and gradual approach are observations of the heater under a fixed input, while the supplied equilibrium relation predicts only its endpoint.

State which of the three model predictions would inform your next gain decision and which part of the record supports it. Continue with the physical heater model, then learn how to plan and qualify your own bounded experiment using those predictions. Do not select a controller gain merely because a closed-loop trace looks smooth.

## Conclusion

Process modelling begins with the prediction needed for a control decision. Closed-loop temperature traces reveal controller-plus-process behavior. The full applied-command history can support model fitting, but temperature settling time alone does not identify how quickly the heater responds. The supplied equilibrium relation defines process gain for a first calculation, while delay and temperature evolution require the next physical and experimental lessons.

## Reason for the next lesson

The equilibrium gain predicts the final temperature change, but it cannot supply the missing temperature-versus-time relationship by itself. We need to explain how heating changes stored energy and how heat loss changes as the vessel warms.

[Energy balance](../15-water-heater-energy-balance/README.md) derives that relationship for this same heater.
