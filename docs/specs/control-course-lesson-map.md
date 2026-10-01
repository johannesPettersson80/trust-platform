# Control course lesson map

For every lesson:
- the problem it starts from, which is what the previous lesson left unsolved;
- the new control theory it teaches;
- how that theory is first said in plain words, then how it is written mathematically;
- how the lesson solves the problem, and what it leaves for the next lesson.

Every lesson is written to the [lesson template](control-course-lesson-template.md).

## Plan decisions (1 October 2026)

- **Check yourself:** every lesson ends with three questions and their answers.
- **Math you need:** a short box where a mathematical tool first appears: 6 (adding up small pieces: sums and the
  integral), 13 (derivative, exponential decay), 15 (natural logarithm), 18 (mean and root mean square), 34
  (second-order equations), 48 (integral transform), 49 (complex numbers), 54 (magnitude and angle of a complex
  number), 69 (vectors and matrices), 70 (matrix inverse), 74 (state vector and matrix product), 79 (variance and
  covariance), 80 (quadratic cost).
- **Annotated figure:** each lesson's evidence figure marks the points that answer its predictions.
- **Hardware canvas:** a lesson that introduces a plant (1 cold room, 5 heater, 31 valve, 34 shaft, 43 tank, 59
  flexible axis, 63 vessel, 69 coupled vessels, 74 two tanks, 83 arm) shows it in the hardware canvas.
- **Introductions:** 57 Nyquist stability, 79 Kalman filtering, 80 LQR control, 82 Model predictive control and 86
  Robust control are labelled as introductions; whether to split them is decided after the pilot.
- **Order:** proposal 1 below was approved with the map and carried out on 1 October: Trajectory feedforward is now
  lesson 61 and Feedforward control lesson 62.
- **1 October, later:** the user liked lessons 1–10 and asked for all remaining lessons to be written the same way now;
  the engineers' try-out follows the complete course.

## Order problems found

1. **Lessons 61–63 jump plants:** 61 Feedforward (heater) → 62 Trajectory feedforward (axis) → 63 Cascade (vessel).
   Cascade's problem, an unmeasured jacket disturbance, does not follow from the trajectory lesson. **Proposal:** put
   Trajectory feedforward directly after 60 Sampled-data stability, which uses the same axis. It then introduces
   feedforward with a planned motion, Feedforward from a measured disturbance follows on the heater, and Cascade
   follows that. Lessons 61 and 62 swap numbers. *Your decision.*
2. **Lesson 60 → 61** switches from the axis back to the heater without saying why. Lesson 60 must end with the
   heater problem, or proposal 1 removes the switch.
3. **Terms used before they are defined.** Examples: "integrator" in 42, defined in 44; "SIMC" in 22 and 44, never
   defined. The automatic term check lists all of them before any rewrite.
4. **Problem sections leaning on older lessons:** 55 lessons start from an older lesson instead of the previous one.
   Each is rewritten to start as stated below.

## 1. Feedback foundations: the cold room (1–4)

### 1. Control-loop elements
- **Problem:** a cold room should be at 2 °C but reads 4 °C, and we do not yet know which signals matter.
- **New theory:** the parts of a control loop: setpoint, measurement (process value), command, actuator, process,
  disturbance and error.
- **Plain words:** a control loop is the chain from what we want, through what we change, to what we measure, and back.
  To control something, you must know what you want, what you can measure, what you can change, and how the process
  answers that change.
- **Mathematics:** the error $e = SP - PV$. With $SP = 2$ °C and $PV = 4$ °C, $e = -2$ °C: the room is 2 °C too warm.
- **How the lesson solves it:** the student maps the cold room's signals and runs a fixed sequence: refrigeration ON
  for 20 s, then OFF, with the door open from 60 to 90 s. The temperature falls with refrigeration ON and rises with
  the door open, and the command does not react.
- **Leaves for lesson 2:** the program measures the temperature but does nothing about it.

### 2. ON/OFF control
- **Problem from lesson 1:** the refrigeration command did not react to the measured temperature.
- **New theory:** ON/OFF control; feedback; chatter.
- **Plain words:** an ON/OFF controller looks at the measurement and switches the actuator fully on or fully off. Using
  the measurement to choose the next command is called **feedback**.
- **Mathematics:** $u = 100\,\%$ when $PV \ge SP$, otherwise $u = 0\,\%$.
- **How the lesson solves it:** the student writes the comparison in ST. The room is now cooled automatically, also
  when the door opens.
- **Leaves for lesson 3:** with one switching temperature, the command reverses at every tiny crossing: 760 changes in
  120 s, which a compressor cannot take.

### 3. Hysteresis and dwell
- **Problem from lesson 2:** chatter, and a compressor needs minimum ON and OFF times.
- **New theory:** hysteresis; dwell time; requested and applied state.
- **Plain words:** hysteresis switches on at one temperature and off at a lower one, and in between keeps doing what it
  did. Dwell time makes the compressor stay on or off for at least a set time before it may switch again.
- **Mathematics:** the hysteresis width is $H = T_{on} - T_{off} = 2.5 - 1.5 = 1.0$ °C. An 8 s dwell at 0.1 s updates
  is $8.0/0.1 = 80$ updates.
- **How the lesson solves it:** switching drops from 760 to 12 changes, and every state lasts at least 8 s. The cost
  is that the temperature now moves about 1 °C instead of 0.03 °C.
- **Leaves for lesson 4:** each rule works on its own, but nobody has tested whether the complete controller meets the
  room's requirements when the heat load changes.

### 4. Cold-room commissioning (assessment)
- **Problem from lesson 3:** the rules were tested separately, not as one controller.
- **New theory:** commissioning: testing a complete controller against requirements written down before the test.
- **Plain words:** commissioning means checking that the finished controller does its job against requirements stated
  before the test, and deciding pass or fail from the recording.
- **Mathematics:** the requirements: $1 \le PV \le 3$ °C from 40 s, every ON and OFF period at least 8 s, and the
  command exactly 0 or 100 %.
- **How the lesson solves it:** the student writes the complete controller, runs the baseline and a changed-load test,
  and decides from the traces.
- **Leaves for lesson 5:** ON/OFF control can only switch fully. The next equipment, an electric heater, can be set
  anywhere between 0 and 100 %.

## 2. Continuous control of the heater (5–11)

### 5. Proportional control
- **Problem from lesson 4:** two states are not enough when the heater power can vary.
- **New theory:** proportional control; proportional gain; steady-state error.
- **Plain words:** proportional control sets the heater power in proportion to how far the water is below its target:
  twice the shortfall gives twice the power.
- **Mathematics:** $u = K_p\,e$ with $K_p$ in % per °C. Holding the water warm needs power, so at a steady temperature
  $T$ the heat in equals the heat out: $2K_p(35 - T) = 10(T - 20)$. With $K_p = 2$, $T = 24.29$ °C, which is
  10.71 °C below the target: the **steady-state error**.
- **How the lesson solves it:** the student runs P on the heater. The power falls as the water warms, and the water
  settles exactly where the heat balance says.
- **Leaves for lesson 6:** at zero error P gives zero power, yet the water keeps losing heat, so it settles below the
  target.

### 6. PI control
- **Problem from lesson 5:** P needs a remaining error to produce the holding power.
- **New theory:** integral action; PI control; integral gain.
- **Plain words:** the integral adds up the error over time. As long as the water is too cold, it keeps raising the
  power, so the power stays on even after the error has gone.
- **Mathematics:** each update adds $\Delta I = K_i\,e\,\Delta t$ and the command is $u = K_p\,e + I$. At a steady
  temperature $e = 0$, so $u = I$: the integral is the holding power.
- **How the lesson solves it:** with the same heater, $K_p = 2$ and $K_i = 0.5$, the water reaches 35 °C and holds it,
  also when colder water flows in.
- **Leaves for lesson 7:** when the heater is already at 100 %, the integral keeps growing.

### 7. Saturation and windup
- **Problem from lesson 6:** the integral keeps growing when the heater cannot give more.
- **New theory:** actuator saturation; integral windup; requested and applied output.
- **Plain words:** saturation means the heater is at full power and cannot do more. Windup is the integral piling up a
  demand the heater cannot deliver. When the target becomes reachable again, that pile must be worked off before the
  heater turns down, so the water overshoots for a long time.
- **Mathematics:** $u_r = K_p\,e + I$ is requested, but only $u_a = \min(100, \max(0, u_r))$ is applied.
- **How the lesson solves it:** the student asks for an unreachable temperature, then returns to 35 °C, and measures
  the long recovery. Here the lesson only shows the problem.
- **Leaves for lesson 8:** returning to a reachable target does not remove the piled-up integral.

### 8. Anti-windup
- **Problem from lesson 7:** the integral piles up while the heater is at its limit.
- **New theory:** anti-windup by back-calculation; tracking time.
- **Plain words:** anti-windup stops the integral from piling up a demand the heater cannot deliver. Whenever the
  requested power is above what is applied, it pulls the integral back towards what the heater actually does.
- **Mathematics:** $I_{next} = I^* + \frac{\Delta t}{T_t}(u_a - u_r)$, where $T_t$ is the tracking time.
- **How the lesson solves it:** the same saturation event as lesson 7 now recovers much faster.
- **Leaves for lesson 9:** recovering from saturation says nothing about how well the controller handles normal target
  changes and disturbances.

### 9. Setpoint and disturbance response
- **Problem from lesson 8:** there are no requirements yet for normal operation.
- **New theory:** setpoint response and disturbance response; overshoot, settling time and excursion; requirements set
  before the test.
- **Plain words:** a setpoint change is when we ask for a new temperature. A disturbance is when something outside
  changes the temperature, such as colder inlet water. We judge both by numbers decided before the test: how far the
  temperature overshoots, how long it takes to settle, and how far a disturbance pushes it.
- **Mathematics:** overshoot = (peak − target) / step size; settling time = the last time the error is outside its band.
- **How the lesson solves it:** the student states the requirements, runs both tests and checks each requirement.
- **Leaves for lesson 10:** when a requirement fails, the result does not say whether the controller, the sensor, the
  heater or its capacity is to blame.

### 10. Loop diagnosis
- **Problem from lesson 9:** a failed requirement does not say why it failed.
- **New theory:** diagnosis before tuning: measurement, command delivery, actuator, operating conditions, capacity.
- **Plain words:** loop diagnosis finds out why a loop performs badly before anyone touches the controller. Is the
  measurement right? Does the command reach the heater? Does the heater respond? Is the target reachable at all?
- **Mathematics:** the capacity check: the holding power $u_{hold} = \frac{H(T - T_a) + D}{a}$ must stay below 100 %.
- **How the lesson solves it:** the student checks each cause in turn on recorded data and finds the real one.
- **Leaves for lesson 11:** once the other causes are ruled out, we still need a method for improving the PI settings.

### 11. Manual PI tuning
- **Problem from lesson 10:** there is no method for changing the gains.
- **New theory:** manual tuning; integral time.
- **Plain words:** manual tuning changes one setting at a time, predicts what will happen, runs the same test again,
  and keeps the change only if every requirement still passes.
- **Mathematics:** the integral time $T_i = K_p / K_i$, in seconds: how long the integral needs to add as much as P
  gives for a constant error.
- **How the lesson solves it:** the student changes one gain by a stated amount, compares matched runs, and keeps or
  rejects the change.
- **Leaves for lesson 12:** trial runs mix the heater's own behaviour with the controller's, so they do not show how
  the heater itself responds.

## 3. Models and tuning (12–22)

### 12. Process dynamics
- **Problem from lesson 11:** trials do not separate the heater from the controller.
- **New theory:** process dynamics; process gain, time constant and delay; open loop and closed loop.
- **Plain words:** process dynamics describe how the heater itself answers a change of power, with no controller
  involved: how much the temperature finally changes, how quickly, and after what delay.
- **Mathematics:** the process gain $K = \Delta T_{steady} / \Delta u$, in °C per %.
- **How the lesson solves it:** the student compares a controlled trace with a step of the heater alone.
- **Leaves for lesson 13:** we know what we want to predict, but not the physics that produces it.

### 13. Energy balance
- **Problem from lesson 12:** the heater has no physical equation yet.
- **New theory:** energy balance; heat capacity; a first-order model with delay; deviations from an operating point.
- **Plain words:** an energy balance says the water's stored heat changes by the heat going in minus the heat going out.
- **Mathematics:** $C\,\frac{dT}{dt} = a\,u(t - L) - H(T - T_a) - D$. For this heater
  $50\,\frac{dT}{dt} = 2u(t-1) - 10(T - 20) - D$, so the time constant is $\tau = C/H = 5$ s and the gain is
  $K = a/H = 0.2$ °C per %. A 10 % step from 35 °C gives $T(t) = 35 + 2(1 - e^{-(t-1)/5})$.
- **How the lesson solves it:** the student derives the model and predicts the step response.
- **Leaves for lesson 14:** the equation has the right form, but the real heater's numbers are unknown.

### 14. Identification experiments
- **Problem from lesson 13:** the model's numbers are unknown.
- **New theory:** designing an identification experiment: operating point, step size, duration, limits and baseline.
- **Plain words:** an identification experiment changes the heater's power on purpose, safely and as planned, so that
  the recorded response reveals the heater's numbers.
- **Mathematics:** to reach 95 % of the final change, hold the step for at least $L + 3\tau$, because
  $1 - e^{-3} = 0.95$.
- **How the lesson solves it:** the student plans and runs a step test in Control Studio (**Identify**).
- **Leaves for lesson 15:** the recording does not yet give the gain, time constant and delay.

### 15. Step-response identification
- **Problem from lesson 14:** the gain, time constant and delay must be calculated from the recording.
- **New theory:** step-response identification of a first-order model with delay.
- **Plain words:** step-response identification reads three numbers from the response to a step. The gain is how far
  the temperature finally moves per % of power. The delay is how long before it starts to move. The time constant is
  how quickly it gets there.
- **Mathematics:** $K = \Delta T / \Delta u$; $T_f - T(t) = (T_f - T_0)\,e^{-(t-L)/\tau}$; on a log scale
  $\ln\frac{T_f - T}{T_f - T_0} = -\frac{t - L}{\tau}$ is a straight line whose slope gives $\tau$.
- **How the lesson solves it:** the student calculates the three numbers and compares them with Control Studio's fit.
- **Leaves for lesson 16:** a first-order model can miss a repeatable feature, such as the sensor warming up slowly.

### 16. Model structure
- **Problem from lesson 15:** the model misses the sensor's own lag.
- **New theory:** model structure: adding a physically justified store instead of a fitting factor.
- **Plain words:** model structure is which physical parts the model contains. If the sensor itself warms up slowly,
  the model needs a second store for the sensor, not an extra number to make the curve fit.
- **Mathematics:** the water $5\dot x + x = 0.2\,v(t-1)$ and the sensor $\tau_s\,\dot m + m = x$.
- **How the lesson solves it:** the student compares the one-store and two-store models with the same recording.
- **Leaves for lesson 17:** different numbers can fit the same recording equally well.

### 17. Identifiability
- **Problem from lesson 16:** the parameters are ambiguous.
- **New theory:** identifiability, structural and practical.
- **Plain words:** a parameter is identifiable when the experiment can tell its value apart from other values. If two
  different heaters would give the same recording, the experiment cannot tell which one we have.
- **Mathematics:** early in a step only the ratio is visible, $x \approx \frac{K}{\tau}\Delta u\,t$. The two-store
  model $\tau_p\tau_s\ddot m + (\tau_p + \tau_s)\dot m + m = K\,v(t-L)$ shows only the sum and the product of the
  time constants, not which belongs to the sensor.
- **How the lesson solves it:** the student chooses a longer test, a different input, or a second measurement that
  separates them.
- **Leaves for lesson 18:** being able to tell the parameters apart does not prove the model predicts new situations.

### 18. Model validation
- **Problem from lesson 17:** the model has only been checked on the data it was fitted to.
- **New theory:** validation on independent data; residuals; RMSE.
- **Plain words:** model validation tests the unchanged model on a new recording it has never seen. If it predicts that
  recording well enough for our purpose, we can design with it.
- **Mathematics:** the residual $r_i$ = measured − predicted, its mean $\bar r$, and
  $\mathrm{RMSE} = \sqrt{\tfrac{1}{N}\sum r_i^2}$.
- **How the lesson solves it:** the student lets Control Studio check the model against a new step, and checks it
  against a heat-loss record by hand, because Control Studio compares a model only with a recorded command step.
- **Leaves for lesson 19:** a validated model predicts the heater, but it does not choose the controller gains.

### 19. Model-based PI tuning
- **Problem from lesson 18:** the gains are not yet derived from the model.
- **New theory:** model-based tuning by a chosen response time $\lambda$ (IMC/SIMC).
- **Plain words:** model-based tuning calculates the PI settings from the model and the speed we want. We choose how
  quickly the temperature should follow a change, and the formula gives the gains.
- **Mathematics:** $K_p = \frac{\tau}{K(\lambda + L)}$ and $T_i = \tau$, so $K_i = K_p / \tau$.
- **How the lesson solves it:** the student calculates the gains, runs them, and compares with Control Studio's design.
- **Leaves for lesson 20:** these gains depend on the model. A candidate found without a model would be a useful
  check.

### 20. Relay tuning
- **Problem from lesson 19:** there is only a model-based candidate.
- **New theory:** the relay feedback test; ultimate gain and period; an empirical tuning rule.
- **Plain words:** relay tuning switches the heater between two levels whenever the temperature crosses the target. The
  loop then oscillates steadily, and the size and period of that oscillation tell us how to set the PI.
- **Mathematics:** the ultimate gain $K_u \approx \frac{4d}{\pi a}$ from relay amplitude $d$ and oscillation amplitude
  $a$; then $K_p = 0.45K_u$ and $T_i = P_u / 1.2$.
- **How the lesson solves it:** the student runs Control Studio's relay test and calculates the candidate.
- **Leaves for lesson 21:** neither candidate is proven suitable just because its calculation is complete.

### 21. Tuning comparison
- **Problem from lesson 20:** the candidates have not been compared fairly.
- **New theory:** comparison under requirements fixed beforehand; "none qualifies" is a valid result; command activity.
- **Plain words:** a tuning comparison tests every candidate in the same experiment against requirements written down
  before looking at any result, and picks the simplest that passes, or none.
- **Mathematics:** the command activity $A = \sum_k |u_k - u_{k-1}|$ measures how much the heater is worked.
- **How the lesson solves it:** the student runs the manual, model-based and relay candidates in Control Studio and
  decides.
- **Leaves for lesson 22:** every changed requirement means designing and comparing again by hand.

### 22. Tuning to requirements
- **Problem from lesson 21:** each new requirement needs a new manual design.
- **New theory:** designing to stated requirements (allowed error, settling band, settling time); refusing infeasible
  requirements.
- **Plain words:** tuning to requirements means stating what the loop must achieve (how big the change is, how close
  the temperature must get, and by when) and letting Control Studio design a controller that meets it, or tell you
  that none can.
- **Mathematics:** with $K_p = \frac{\tau}{K(\lambda + L)}$, the temperature reaches the band $b$ at
  $t = L + \lambda\ln\frac{\Delta r}{b}$. Settling by $T_s$ therefore needs $\lambda \le \frac{T_s - L}{\ln(\Delta r / b)}$.
- **How the lesson solves it:** three requirement sets: two met (More robust, Balanced) and verified on the actual ST,
  one refused with its reason.
- **Leaves for lesson 23:** a PI that meets the requirements can still kick the heater hard when the target changes.

## 4. Implementation in the controller (23–30)

### 23. Setpoint weighting
- **Problem from lesson 22:** the PI meets its requirements, but a target change makes the heater command jump.
- **New theory:** setpoint weighting.
- **Plain words:** setpoint weighting lets the P part react only partly to a change of target, while it still reacts
  fully to the measured temperature. The heater gets a gentler push when the target changes, and disturbances are
  fought as hard as before.
- **Mathematics:** $P = K_p\,[\beta\,(SP - T_0) - (PV - T_0)]$ with $0 \le \beta \le 1$; the integral still uses the full
  error $SP - PV$.
- **How the lesson solves it:** the student compares $\beta = 1$ and $\beta = 0.8$ on the tuned PI: a smaller command
  jump, at the cost of a slightly slower response ($\beta = 0.3$ fails the 15 s settling requirement).
- **Leaves for lesson 24:** sensor noise still makes the command jitter.

### 24. Measurement filtering
- **Problem from lesson 23:** measurement noise moves the heater command.
- **New theory:** the first-order measurement filter; filter time constant; its delay cost.
- **Plain words:** a measurement filter smooths a noisy reading by following it only part of the way at each update.
  It calms the command, but it also notices real changes later.
- **Mathematics:** $T_f\,\frac{dy_f}{dt} + y_f = y_r$; per update $y_{f,new} = y_{f,old} + \alpha\,(y_r - y_{f,old})$
  with $\alpha = \frac{\Delta t}{T_f + \Delta t}$.
- **How the lesson solves it:** the student compares command noise and response delay with and without the filter.
- **Leaves for lesson 25:** a filter cannot recover changes the controller never sampled, and a wrong time step corrupts
  the update.

### 25. Sampling and timing
- **Problem from lesson 24:** sampling limits and wrong time steps.
- **New theory:** sampling; aliasing; scan time; held commands.
- **Plain words:** sampling means the controller looks at the temperature only at separate moments. Anything that
  changes faster than it looks can be missed or misread, and every update must use the time that really passed.
- **Mathematics:** $\Delta I = K_i\,e\,\Delta t$ with the actual $\Delta t$. Aliasing: an 8 Hz signal sampled at 10 Hz
  gives the same samples as a 2 Hz signal.
- **How the lesson solves it:** the student sees aliasing and a wrong-$\Delta t$ integral, then uses the real elapsed
  time.
- **Leaves for lesson 26:** correct updates can still give a bad first command if the stored values start wrong.

### 26. Controller initialization
- **Problem from lesson 25:** the first command can jump because the controller's memory starts wrong.
- **New theory:** retained state; initialization from the operating point.
- **Plain words:** a controller remembers values between updates, such as its integral. Initialization sets those
  values to match the plant's present state, so the first command is the one the plant already has.
- **Mathematics:** $I_0 = u_a - P$: the integral starts at the applied command minus the P part.
- **How the lesson solves it:** the controller starts at a hot operating point without a jump.
- **Leaves for lesson 27:** after a period of manual operation, the remembered values no longer match.

### 27. Bumpless transfer
- **Problem from lesson 26:** switching from manual to automatic makes the command jump.
- **New theory:** bumpless transfer; tracking while in manual.
- **Plain words:** bumpless transfer means switching between manual and automatic without a sudden change in the heater
  command. While in manual, the integral is kept in step with the manual output.
- **Mathematics:** in manual, $I = u_a - P$ at every update.
- **How the lesson solves it:** the student switches modes and measures no command jump.
- **Leaves for lesson 28:** changing gains or limits while running can still make the command jump.

### 28. Parameter updates
- **Problem from lesson 27:** changing settings while running is unsafe.
- **New theory:** applying a complete parameter set at one update, with state compensation.
- **Plain words:** a parameter update changes the controller's settings while it runs. All related settings change
  together at one update, and the integral is adjusted so the command does not jump.
- **Mathematics:** $I_{new} = I_{old} + P_{old} - P_{new}$ keeps $u = P + I$ unchanged at the switch.
- **How the lesson solves it:** the student changes the gains mid-run with no jump in the command.
- **Leaves for lesson 29:** correct settings do not help if the measurement itself is wrong or old.

### 29. Measurement validity
- **Problem from lesson 28:** the controller would use a wrong or old reading.
- **New theory:** validity checks (range and age); a defined fallback; deliberate recovery.
- **Plain words:** measurement validity means checking that a reading can be trusted before the controller uses it: is
  it within range, and is it recent? If not, the controller holds a safe output until fresh readings return.
- **Mathematics:** the age $a = t_{now} - t_{acquisition}$ must stay below its limit.
- **How the lesson solves it:** the student injects stale and out-of-range readings and checks the fallback and the
  recovery.
- **Leaves for lesson 30:** each mechanism works on its own, but the complete heater controller has not been built and
  tested.

### 30. Heater commissioning (assessment)
- **Problem from lesson 29:** the mechanisms have not been combined into one controller.
- **New theory:** commissioning the complete heater loop.
- **Plain words:** heater commissioning checks the finished heater controller against all its requirements: target and
  disturbance response, startup, mode changes, limits, parameter changes and measurement faults.
- **Mathematics:** the model $50\,\frac{dT}{dt} = 2u(t-1) - 10(T - 20) - D$ and the holding power
  $u_{hold} = \frac{H(T - T_a) + D}{a}$.
- **How the lesson solves it:** the student writes the complete controller and assesses it against requirements stated
  beforehand.
- **Leaves for lesson 31:** the next heater is fed through a valve that cannot move as fast as it is commanded.

## 5. Real actuators (31–33)

### 31. Actuator tracking
- **Problem from lesson 30:** the valve moves more slowly than its command.
- **New theory:** rate limit; requested, target and actual position; tracking with the actual position.
- **Plain words:** a valve needs time to move. Actuator tracking feeds the valve's actual position back into the
  controller, so the integral does not pile up while the valve is still travelling.
- **Mathematics:** $I_{next} = I^* + \frac{\Delta t}{T_t}(x - u_r)$ with the measured position $x$; the delivered heat is
  $Q = c_p\,q\,(T_{in} - T)$.
- **How the lesson solves it:** the student compares the recovery with and without tracking.
- **Leaves for lesson 32:** the valve position does not tell the flow; that also depends on pressure.

### 32. Valve characteristics
- **Problem from lesson 31:** the relation between opening and flow is unknown.
- **New theory:** inherent and installed valve characteristic.
- **Plain words:** a valve characteristic tells how much flow each opening gives. In a real pipe the flow also depends
  on how much pressure is left for the valve, so equal steps in opening do not give equal steps in flow.
- **Mathematics:** $q = 2z\sqrt{\Delta p / 100\ \text{kPa}}$ kg/s with $z = x/100$; installed in this pipe
  $q = 2z\sqrt{1.28125 / (1 + 0.5z^2)}$ kg/s.
- **How the lesson solves it:** the student measures the flow at several openings and compares with the formula.
- **Leaves for lesson 33:** a smooth characteristic cannot explain a valve that ignores small commands and then jumps.

### 33. Valve stiction
- **Problem from lesson 32:** the valve sticks and jumps.
- **New theory:** deadband and stiction (static friction, breakaway).
- **Plain words:** stiction is a valve that sticks. It ignores small commands, then jumps when the push becomes big
  enough. A deadband is different: the controller deliberately ignores small changes. Both look like poor tuning, but
  neither is.
- **Mathematics:** the spring force $F = k_s(u_t - x)$ must exceed the static friction before the stem moves.
- **How the lesson solves it:** the student tells the two apart from the command, position, force and flow traces.
- **Leaves for lesson 34:** we have treated the position as a result; we have not learned how a motor produces and
  controls a position.

## 6. Motion control (34–42)

### 34. Position control
- **Problem from lesson 33:** how a motor moves a shaft to a position is still unknown.
- **New theory:** motion (inertia, friction, torque); proportional position control.
- **Plain words:** position control turns a shaft to a target angle by applying torque in proportion to the angle
  error. Because the shaft has inertia, it is still moving when it reaches the target, so stronger P makes it
  overshoot and swing.
- **Mathematics:** $J\dot\omega = u - B\omega - \tau_L$ and $\dot\theta = \omega$. With $u = K_p(r - \theta)$:
  $J\ddot\theta + B\dot\theta + K_p\theta = K_p r$.
- **How the lesson solves it:** the student runs P at two gains and sees the higher one overshoot.
- **Leaves for lesson 35:** P does not oppose the shaft's speed, so it cannot stop the overshoot.

### 35. PD control
- **Problem from lesson 34:** nothing brakes the moving shaft.
- **New theory:** derivative action; damping; derivative on measurement; derivative kick.
- **Plain words:** the derivative part brakes the shaft in proportion to how fast it moves, like a shock absorber.
  Taking the speed from the measured angle avoids a kick when the target jumps.
- **Mathematics:** $u = K_p(r - \theta) - K_d\,\omega$ gives $J\ddot\theta + (B + K_d)\dot\theta + K_p\theta = K_p r$.
  Braking just enough not to overshoot needs $K_d = 2\sqrt{J K_p} - B = 7$ N·m·s/rad.
- **How the lesson solves it:** the student compares P and PD: same target, no overshoot.
- **Leaves for lesson 36:** a constant load needs torque at rest, and PD gives it only from a remaining error.

### 36. PID load rejection
- **Problem from lesson 35:** a constant load leaves PD with an error.
- **New theory:** PID; load torque; holding at zero error.
- **Plain words:** PID adds the integral to PD. The controller then keeps up the holding torque that a constant load
  needs, even with the shaft exactly at its target.
- **Mathematics:** the load $\tau_L = m\,g\,r = 1.962$ N·m; PD's error is $\tau_L / K_p = 0.1226$ rad; the integral
  adds $K_i\,e\,\Delta t$ until it supplies $\tau_L$.
- **How the lesson solves it:** with a weight on the drum, PD holds 0.12 rad off and PID holds the target.
- **Leaves for lesson 37:** the derivative uses a speed calculated from noisy angle readings.

### 37. Derivative filtering
- **Problem from lesson 36:** the speed estimate is noisy.
- **New theory:** noise amplification of a finite difference; the derivative filter.
- **Plain words:** a speed calculated from two noisy angle readings is very noisy, because the noise is divided by a
  tiny time step. A derivative filter smooths that speed, at the cost of braking a little later.
- **Mathematics:** $\omega_{raw} = \frac{\theta_k - \theta_{k-1}}{\Delta t}$ and
  $\omega_f \leftarrow \omega_f + \frac{\Delta t}{T_f + \Delta t}(\omega_{raw} - \omega_f)$.
- **How the lesson solves it:** the student compares torque noise and damping with and without the filter.
- **Leaves for lesson 38:** knowing the separate terms does not give a correctly ordered complete update.

### 38. Discrete PID
- **Problem from lesson 37:** the terms are not yet combined into one sampled update.
- **New theory:** the discrete PID update and its order: trial integral, limit, tracking.
- **Plain words:** a discrete PID calculates its output once per update, in a fixed order: error, P, a trial integral,
  filtered D, the request, the limit, and finally the integral correction for the limit.
- **Mathematics:** $e_k = r_k - \theta_k$, $I^*_k = I_{k-1} + K_i\,e_k\,\Delta t$, $u_r = K_p e_k + I^*_k - K_d\,\omega_f$,
  $u_a = \mathrm{clamp}(u_r)$.
- **How the lesson solves it:** the student calculates one update by hand and checks it against `Control.PID`.
- **Leaves for lesson 39:** code written in incremental form must be shown to behave the same.

### 39. Positional and incremental PID
- **Problem from lesson 38:** a second way of writing PID exists, and it is not yet shown to be equivalent.
- **New theory:** positional and incremental (velocity) form.
- **Plain words:** a positional PID calculates the whole output at every update; an incremental PID calculates only how
  much to change it. Both give the same output if they keep the same state.
- **Mathematics:** $\Delta u_k = K_p\,\Delta e_k + K_i\,e_k\,\Delta t - K_d\,\Delta\omega_{f,k}$ (plus the limit correction).
- **How the lesson solves it:** the student computes both forms on the same data and compares.
- **Leaves for lesson 40:** Control Studio tunes `Control.PID`, not a PID law the program computes itself.

### 40. Your own PID law
- **Problem from lesson 39:** the program's own PID cannot be tuned by Control Studio yet.
- **New theory:** declaring a program's own control law to the tuning tool: gains, remembered values, initialization.
- **Plain words:** Control Studio can tune a PID your program computes itself, once you tell it which variables are the
  gains and which values the law remembers between updates.
- **Mathematics:** $u_k = K_p e_k + I_k + D_k$, with $I_k$ and the previous measurement carried from update to update.
- **How the lesson solves it:** the student sets the law up with **Configure control loop**; Automatic designs the PID,
  validates it on the ST and applies it.
- **Leaves for lesson 41:** a tuned law is not yet a servo commissioned under load and in all operating states.

### 41. Servo commissioning (assessment)
- **Problem from lesson 40:** the servo controller has not been assessed as a whole.
- **New theory:** commissioning the complete motion controller.
- **Plain words:** servo commissioning checks the finished position controller on the same shaft: moving to targets,
  holding a load, staying within torque limits, starting, mode changes and faults.
- **Mathematics:** without control, the load makes the shaft drift:
  $\theta(t) = -\frac{\tau_L}{B}\left(t - \frac{J}{B}(1 - e^{-Bt/J})\right)$.
- **How the lesson solves it:** the student writes and assesses the integration against stated requirements.
- **Leaves for lesson 42:** the heater got PI and the shaft PID by habit. A new process needs its controller chosen from
  its model.

### 42. Controller families
- **Problem from lesson 41:** the controller type was chosen by habit, not from the process.
- **New theory:** choosing P, PI, PD or PID from the model and the requirements.
- **Plain words:** the controller family is which parts the controller uses: P, PI, PD or PID. The process decides:
  - a process that settles by itself needs I to reach a new level without error;
  - a shaft that keeps moving needs D to brake;
  - a constant load needs I to hold it.
- **Mathematics:** the heater's P error is $e = \frac{\Delta r}{1 + K K_p}$; the shaft's error dies away at the rate
  $\sigma = \frac{B + K_d}{2J}$, which only D can raise.
- **How the lesson solves it:** Control Studio designs all four families for the heater and the shaft, recommends PI and
  PID, and gives the reason for every refusal.
- **Leaves for lesson 43:** the next process is a tank: a pump changes how much water is stored, a new kind of process.

## 7. Tanks (43–47)

### 43. Volume balance
- **Problem from lesson 42:** there is no model of how pumping changes a level.
- **New theory:** volume balance (conservation of mass); accumulation.
- **Plain words:** a volume balance says the water in the tank changes by what flows in minus what flows out. Equal
  flows hold the level where it is; they do not bring it back to where it was.
- **Mathematics:** $A\,\frac{dh}{dt} = q_{in} - q_{out}$. With $A = 0.5$ m², 0.012 m³/s in and 0.010 m³/s out, the level
  rises $0.002/0.5 = 0.004$ m/s.
- **How the lesson solves it:** the student runs a flow step; the level rises, then stays at its new height.
- **Leaves for lesson 44:** with pumps on both sides, the level never settles by itself, so something must hold it.

### 44. Level control
- **Problem from lesson 43:** the level drifts whenever the flows differ, and nothing brings it back.
- **New theory:** integrating process; P's error set by the outflow; PI with a slow integral; a tuning rule for
  integrating processes.
- **Plain words:** an integrating process keeps adding up what goes into it, so it never settles by itself. In a tank,
  more water in than out makes the level rise and keep rising; only equal flows hold it, at whatever level it happens
  to be. A heater is the opposite: it settles at a new temperature by itself, because it loses more heat as it gets
  warmer.
- **Mathematics:** the rate gain $K' = 0.0008$ m/s per % of pump. P settles with $e = u_{out} / K_p$. The rule
  $K_p = \frac{1}{K'(\lambda + L)}$, $T_i = 4(\lambda + L)$ keeps the level from swinging.
- **How the lesson solves it:** Control Studio identifies the tank, refuses P (0.098 m low), shows a fast integral
  ringing (43 % overshoot), and designs, validates and applies a PI.
- **Leaves for lesson 45:** many tanks drain by gravity, and their outflow grows with the level.

### 45. Gravity outflow
- **Problem from lesson 44:** the outflow depends on the level.
- **New theory:** gravity outflow; the square-root law; equilibrium level; nonlinearity.
- **Plain words:** when a tank drains through a hole at the bottom, the water's height pushes it out: the fuller the
  tank, the faster it drains. The tank finds the level at which the outflow equals the inflow, so it settles by
  itself, but doubling the inflow does not double the level.
- **Mathematics:** $v = \sqrt{2gh}$, so $q_{out} = c\sqrt h$ and $A\,\frac{dh}{dt} = q_{in} - c\sqrt h$; at rest
  $h_{eq} = (q_{in}/c)^2$.
- **How the lesson solves it:** the student steps the inflow at two levels and sees different responses.
- **Leaves for lesson 46:** one gain and one time constant cannot describe the tank at all levels.

### 46. Linearization
- **Problem from lesson 45:** the tank is nonlinear.
- **New theory:** linearization by the tangent; local gain and time constant.
- **Plain words:** linearization replaces a curve with the straight line that touches it at the operating point. Near
  that level, the tank then behaves like a simple first-order process with its own gain and time constant.
- **Mathematics:** $g = \frac{dq}{dh}\Big|_{h_0} = \frac{c}{2\sqrt{h_0}}$; then $A\,\dot x = v - g\,x$, so $K = 1/g$ and
  $\tau = A/g$.
- **How the lesson solves it:** the student compares linear and nonlinear predictions for a small and a large step.
- **Leaves for lesson 47:** gains tuned at one level behave differently at another.

### 47. Gain scheduling
- **Problem from lesson 46:** fixed gains do not fit every level.
- **New theory:** gain scheduling.
- **Plain words:** gain scheduling changes the controller's settings with the operating level, using what we know about
  how the tank's behaviour changes with level.
- **Mathematics:** $K_p = A/\lambda$ and $K_i(h) = \frac{g(h)}{\lambda} = \frac{c}{2\lambda\sqrt h}$.
- **How the lesson solves it:** the student compares scheduled and fixed gains at several levels.
- **Leaves for lesson 48:** analysing every new plant and controller with differential equations is becoming
  cumbersome.

## 8. Classical analysis and frequency design (48–60)

### 48. Laplace transforms
- **Problem from lesson 47:** working each new combination through differential equations is cumbersome.
- **New theory:** the Laplace transform; the transfer function.
- **Plain words:** the Laplace transform turns a differential equation into ordinary algebra: every derivative becomes a
  multiplication by $s$. A process can then be written as one fraction, its transfer function, which says how its
  output answers its input.
- **Mathematics:** $X(s) = \int_0^\infty x(t)\,e^{-st}\,dt$; $\mathcal L\{\dot x\} = sX(s) - x(0)$;
  $\mathcal L\{e^{-at}\} = \frac{1}{s + a}$. The heater becomes $G(s) = \frac{0.2\,e^{-s}}{5s + 1}$.
- **How the lesson solves it:** the student transforms the heater and shaft models and solves the heater's step
  response algebraically.
- **Leaves for lesson 49:** we cannot yet read such a fraction to see whether the response decays, grows or oscillates.

### 49. Poles and zeros
- **Problem from lesson 48:** the transfer functions cannot be read yet.
- **New theory:** poles, zeros, and their meaning.
- **Plain words:** poles are the values of $s$ that make a transfer function's denominator zero. Each pole is one natural
  way the system moves: a negative real pole dies away, a positive one grows, and a complex pair oscillates. Zeros
  shape how the input appears in the response.
- **Mathematics:** the heater's pole $s = -0.2$ gives $e^{-0.2t}$. The shaft under P control, $K_p = 4$, has
  $s = -0.5 \pm j1.936$, which gives $e^{-0.5t}\cos(1.936t)$.
- **How the lesson solves it:** the student matches each pole to the response Control Studio recorded.
- **Leaves for lesson 50:** a supplied transfer function does not give the complete loop's behaviour once feedback is
  connected.

### 50. Characteristic equations
- **Problem from lesson 49:** the poles of the complete loop are unknown.
- **New theory:** the closed loop's characteristic equation.
- **Plain words:** the characteristic equation is the equation whose roots are the closed loop's poles. It comes from the
  plant and the controller together, and it tells whether every natural motion of the complete loop dies away.
- **Mathematics:** the heater with PI gives $s^2 + 0.3s + 0.02 = (s + 0.1)(s + 0.2) = 0$, so both modes decay.
- **How the lesson solves it:** the student derives the equation and checks it against the recorded response.
- **Leaves for lesson 51:** we checked one gain; we do not know how the poles move when the gain changes.

### 51. Root locus
- **Problem from lesson 50:** pole movement with the gain is unknown.
- **New theory:** the root locus.
- **Plain words:** a root locus is a plot of where the closed-loop poles go as one gain increases. From it you choose the
  gain that gives the speed and the overshoot you want.
- **Mathematics:** for $s^2 + 8s + K_p = 0$: $s = \frac{-8 \pm \sqrt{64 - 4K_p}}{2}$. The poles meet at $K_p = 16$ and
  then become a complex, oscillating pair.
- **How the lesson solves it:** the student uses Control Studio's root locus and chooses a gain.
- **Leaves for lesson 52:** the plot does not state the exact range of gains that keeps the loop stable.

### 52. Routh–Hurwitz stability
- **Problem from lesson 51:** the exact stable range is unknown.
- **New theory:** the Routh–Hurwitz criterion.
- **Plain words:** the Routh–Hurwitz test tells, from the coefficients of the characteristic equation alone, whether
  every pole is on the stable side, and for which gains.
- **Mathematics:** the Routh table built with $C_j = \frac{B_0A_{j+1} - A_0B_{j+1}}{B_0}$. For $s^2 + s + K_p$, stable
  exactly when $K_p > 0$.
- **How the lesson solves it:** the student builds the table by hand and with Control Studio's Routh–Hurwitz tool.
- **Leaves for lesson 53:** stability does not tell how the loop answers inputs that repeat at different speeds.

### 53. Periodic excitation
- **Problem from lesson 52:** the response to repeating inputs is unknown.
- **New theory:** sinusoidal excitation; frequency; the steady periodic response.
- **Plain words:** periodic excitation moves the input up and down in a sine wave at a chosen speed and watches how the
  output follows. The faster the wave, the smaller and the later the answer.
- **Mathematics:** $u(t) = u_0 + d\sin(\omega t)$ with $\omega = 2\pi / P$.
- **How the lesson solves it:** the student runs sines at three speeds as a Control Studio project experiment.
- **Leaves for lesson 54:** the traces show "smaller and later", but not by how much.

### 54. Frequency response
- **Problem from lesson 53:** there is no number for how much smaller and how much later.
- **New theory:** frequency response; amplitude ratio; phase; Bode plot.
- **Plain words:** the frequency response tells, for each speed of a sine wave, how much smaller the output wave is (the
  amplitude ratio) and how far it lags behind (the phase).
- **Mathematics:** $G(j\omega) = \frac{0.2\,e^{-j\omega}}{1 + 5j\omega}$, so $M = \frac{0.2}{\sqrt{1 + 25\omega^2}}$ and
  $\phi = -\omega - \arctan(5\omega)$.
- **How the lesson solves it:** the student measures the response with Control Studio and compares it with the model's
  Bode plot.
- **Leaves for lesson 55:** the plant's response does not show how feedback treats disturbances and noise.

### 55. Sensitivity functions
- **Problem from lesson 54:** the closed loop's paths for disturbances and noise are unknown.
- **New theory:** the sensitivity function $S$ and the complementary sensitivity $T$.
- **Plain words:** the sensitivity tells how much of a disturbance still gets through the closed loop at each speed.
  The complementary sensitivity tells how much measurement noise reaches the process. Where one is small, the other is
  large.
- **Mathematics:** with the loop $L = CG$: $S = \frac{1}{1 + L}$, $T = \frac{L}{1 + L}$, and $y = Tr + S\,G_D D - Tn$,
  where $S + T = 1$.
- **How the lesson solves it:** the student injects a load sine and a noise sine and compares them with $|S|$ and $|T|$
  in Control Studio.
- **Leaves for lesson 56:** seeing a bad trade-off does not tell how to change the controller.

### 56. Loop shaping
- **Problem from lesson 55:** there is no method to change the loop's frequency behaviour.
- **New theory:** loop shaping; lead and lag compensation.
- **Plain words:** loop shaping changes the controller's frequency behaviour on purpose. A lead adds phase near the
  crossover so the loop can be made faster safely; a lag adds gain at slow speeds to fight loads.
- **Mathematics:** $F(s) = \frac{1 + T_z s}{1 + T_p s}$, with
  $\angle F = \arctan(\omega T_z) - \arctan(\omega T_p)$.
- **How the lesson solves it:** the student designs a lead with Control Studio's lead/lag tool and validates it.
- **Leaves for lesson 57:** extra phase at one frequency does not prove that the whole loop is stable, especially with
  a delay.

### 57. Nyquist stability
- **Problem from lesson 56:** the whole loop's stability with a delay is unchecked.
- **New theory:** the Nyquist criterion.
- **Plain words:** the Nyquist test draws the loop's frequency response as a curve and counts how it goes around the
  point −1. That count tells whether the closed loop is stable, delay included.
- **Mathematics:** $Z = P + N$: unstable closed-loop poles = unstable open-loop poles + clockwise encirclements of −1.
- **How the lesson solves it:** the student uses Control Studio's Nyquist view for a 1 s and an 8 s delay.
- **Leaves for lesson 58:** the loop is stable for the model, but we do not know how far it is from becoming unstable
  if the real plant differs.

### 58. Robustness margins
- **Problem from lesson 57:** there is no measure of the distance to instability.
- **New theory:** gain margin, phase margin, delay margin, peak sensitivity.
- **Plain words:** robustness margins say how much the real plant may differ from its model before the loop becomes
  unstable: how much more gain, how much more lag, how much more delay.
- **Mathematics:** $GM = 1/|L(j\omega_{180})|$; $PM = 180° + \angle L(j\omega_c)$; $DM = PM/\omega_c$;
  $M_s = \max |1/(1 + L)|$. For $L(s) = \frac{a}{s}e^{-s\,t_d}$, with delay $t_d$: $GM = \frac{\pi}{2a\,t_d}$ and $DM = \frac{\pi}{2a} - t_d$.
- **How the lesson solves it:** the student compares Control Studio's margins with these formulas, then validates on
  plants inside and outside the margins.
- **Leaves for lesson 59:** a flexible part can raise the loop gain at one frequency and use up the margin a rigid model
  promised.

### 59. Mechanical resonance
- **Problem from lesson 58:** a resonance can consume the gain margin.
- **New theory:** resonance (natural frequency, damping ratio); the notch filter.
- **Plain words:** a resonance is a frequency at which a flexible part swings strongly, like a weight on a spring. A
  notch filter removes just that frequency from the measurement, so the controller stops pushing the swing.
- **Mathematics:** $\ddot\theta_l + 2\zeta\omega_r\dot\theta_l + \omega_r^2\theta_l = \omega_r^2\theta_m$; at $\omega_r$
  the gain is $1/(2\zeta)$ times the rigid value. The notch is
  $N(s) = \frac{s^2 + \omega_n^2}{s^2 + (\omega_n/Q)s + \omega_n^2}$.
- **How the lesson solves it:** Control Studio measures the 3 Hz peak with a multisine, then chooses and validates the
  notch on the actual ST.
- **Leaves for lesson 60:** this analysis ignores that the controller samples and holds its command.

### 60. Sampled-data stability
- **Problem from lesson 59:** sampling and computation delay are not analysed.
- **New theory:** the sampled-data model; the held command; computation delay; the unit circle.
- **Plain words:** a digital controller looks and acts only at its update instants and holds its command in between.
  Sampled-data analysis models exactly that. The loop is stable when every pole lies inside the unit circle.
- **Mathematics:** $x_{k+1} = x_k + b\,\omega_k + c\,u_k$, $\omega_{k+1} = a\,\omega_k + d\,u_k$ with $a = e^{-h}$,
  $u_{k+1} = -K_p x_k - K_d\omega_k$; stable when $|z| < 1$ for every pole $z$.
- **How the lesson solves it:** the student compares 0.02 s and 0.6 s updates on the shaft.
- **Leaves for lesson 61:** with correct timing, feedback still waits for a following error before it supplies the
  torque a planned move needs.

## 9. Industrial control structures (61–68)

### 61. Trajectory feedforward
- **Problem from lesson 60:** a planned move lags behind its plan, because feedback supplies its torque only after a
  following error has appeared.
- **New theory:** the motion profile; trajectory feedforward (the model's inverse).
- **Plain words:** trajectory feedforward calculates, from the planned motion, the torque the axis will need to
  accelerate and to overcome friction, and gives it right away. The PID corrects only the small remainder.
- **Mathematics:** $u_{ff} = J\,\ddot r + B\,\dot r$; a trapezoidal move of distance $D$ takes $T = \frac{D}{V} + \frac{V}{A}$.
- **How the lesson solves it:** Control Studio designs $K_v = B$ and $K_a = J$; the following error falls from 0.092 to
  0.001 rad.
- **Leaves for lesson 62:** a disturbance that is measured but not planned, such as the heat colder inlet water draws
  from the water heater, still reaches the process before feedback reacts.

### 62. Feedforward control
- **Problem from lesson 61:** a measured but unplanned disturbance, colder inlet water on the heater, reaches the
  water before feedback can react.
- **New theory:** feedforward from a measured disturbance.
- **Plain words:** feedforward measures a disturbance and acts on it right away, before the temperature has changed,
  with the command that cancels it. Feedback then corrects whatever the feedforward gets wrong.
- **Mathematics:** $u_{ff} = D_m / a$ and $u = P + I + u_{ff}$.
- **How the lesson solves it:** the student compares PI alone with PI plus feedforward for the same heat extraction.
- **Leaves for lesson 63:** a disturbance nobody measures still reaches the process before feedback reacts.

### 63. Cascade control
- **Problem from lesson 62:** an unmeasured jacket disturbance reaches the vessel before vessel-temperature feedback
  can respond; feedforward needs a measurement of it.
- **New theory:** cascade control (inner and outer loop).
- **Plain words:** cascade control uses two loops, one inside the other. The inner loop quickly holds an intermediate
  temperature, the jacket, that shows a disturbance early. The outer loop sets the inner loop's target to hold the
  product temperature.
- **Mathematics:** $5\dot T_j = 2u - 10(T_j - T_v) - D_j$ and $50\dot T_v = 10(T_j - T_v) - 2(T_v - 20) - D_v$; the
  inner loop must be clearly faster than the outer one.
- **How the lesson solves it:** Control Studio tunes the inner loop first and the outer loop through it; the jacket
  disturbance is corrected earlier.
- **Leaves for lesson 64:** a new requirement: an additive must follow a changing carrier flow, which a fixed setpoint
  cannot do.

### 64. Ratio control
- **Problem from lesson 63:** one flow must stay in proportion to another, changing flow.
- **New theory:** ratio control.
- **Plain words:** ratio control keeps one flow at a fixed proportion of another by giving the dependent flow's
  controller a target calculated from the measured main flow.
- **Mathematics:** $q_{a,sp} = R\,q_c$.
- **How the lesson solves it:** the student implements the ratio and tests it with a changing carrier, low flow and a
  stop.
- **Leaves for lesson 65:** the vessel now needs both heating and cooling, and one positive command cannot ask for both.

### 65. Split-range control
- **Problem from lesson 64:** one controller output must drive both a heater and a cooler.
- **New theory:** split-range control.
- **Plain words:** split-range control divides one controller output between two actuators: one part of the range
  heats, the other part cools.
- **Mathematics:** the net heat is $Q = 2u_h - u_c$, with heating and cooling never used at the same time.
- **How the lesson solves it:** the student maps the signed demand to the two actuators and tests both directions.
- **Leaves for lesson 66:** meeting the vessel's heat demand can break a separate limit on the jacket temperature.

### 66. Override control
- **Problem from lesson 65:** a limit must win over normal control.
- **New theory:** override (selector) control.
- **Plain words:** override control lets a limiting controller take over from the normal one whenever a limit would
  otherwise be broken. The smaller request wins.
- **Mathematics:** $u_r = \min(u_{normal}, u_{limit})$, with the inactive controller tracking the applied output.
- **How the lesson solves it:** the student tests the hand-over in both directions without bumps.
- **Leaves for lesson 67:** a long delivery delay still makes the heating correction arrive too late.

### 67. Smith prediction
- **Problem from lesson 66:** a known delay slows the feedback.
- **New theory:** the Smith predictor.
- **Plain words:** a Smith predictor lets the controller act on a model's prediction of the temperature without the
  delay, and corrects that prediction with what is really measured.
- **Mathematics:** $y_{pred} = y_{m,0} + (y - y_{m,L})$: the delay-free model plus the measured model error.
- **How the lesson solves it:** Control Studio designs the predictor from the model's delay; the student also tests it
  with a wrong delay.
- **Leaves for lesson 68:** each structure works on its own; which combination meets the vessel's requirements?

### 68. Vessel commissioning (assessment)
- **Problem from lesson 67:** the structures have not been combined and compared for the vessel.
- **New theory:** choosing and commissioning a combination of control structures.
- **Plain words:** vessel commissioning picks the combination of loops the vessel needs and checks it against its
  performance, limits and operating requirements.
- **Mathematics:** the vessel's heat balance and requirements, unchanged across all candidates.
- **How the lesson solves it:** the student compares candidate arrangements and defends the choice from the recordings.
- **Leaves for lesson 69:** with two actuators that each affect both temperatures, it is unclear which actuator should
  control which temperature.

## 10. Multivariable control (69–73)

### 69. Multivariable process paths
- **Problem from lesson 68:** two actuators each affect both temperatures.
- **New theory:** multivariable processes; the gain matrix.
- **Plain words:** a multivariable process has several inputs and outputs, and each input moves several outputs. The
  gain matrix lists, for every input, how much it moves every output.
- **Mathematics:** $18x_1 - 8x_2 = 2v_1$ and $-8x_1 + 18x_2 = 2v_2$ give $x = Kv$ with
  $K = \begin{bmatrix}9/65 & 4/65\\ 4/65 & 9/65\end{bmatrix}$ °C per point.
- **How the lesson solves it:** Control Studio's measured multivariable paths give $K$ from one experiment per heater.
- **Leaves for lesson 70:** a gain measured with the other heater fixed changes when the other loop moves its heater.

### 70. Relative gain array
- **Problem from lesson 69:** the gains change when the other loop is working.
- **New theory:** the relative gain array (RGA).
- **Plain words:** the relative gain compares how strongly an input moves its output with the other loop off, and with
  the other loop holding its output. Close to 1 means the pair barely disturbs each other: a good pairing.
- **Mathematics:** $\Lambda = K \circ (K^{-1})^T$, element by element; here
  $K^{-1} = \begin{bmatrix}9 & -4\\ -4 & 9\end{bmatrix}$ and $\lambda_{11} = \frac{81}{65} = 1.25$.
- **How the lesson solves it:** the student calculates the RGA by hand and with Control Studio's RGA tool, and picks
  the pairing.
- **Leaves for lesson 71:** a steady pairing does not show how the two loops behave together over time.

### 71. Decentralized PI
- **Problem from lesson 70:** the paired loops have not been run together.
- **New theory:** decentralized (independent-loop) control.
- **Plain words:** decentralized PI uses one PI controller per chosen pair, each with its own memory, all running at the
  same time.
- **Mathematics:** each member is tuned on its own path with the gain it has when the other loop is closed,
  $K_{11}/\lambda_{11}$.
- **How the lesson solves it:** Control Studio designs both PIs from the group model; the student tests setpoint
  changes and disturbances on both sides.
- **Leaves for lesson 72:** the vessel that was not asked to change still deviates until its own loop catches up.

### 72. Static decoupling
- **Problem from lesson 71:** the loops still disturb each other.
- **New theory:** static decoupling.
- **Plain words:** static decoupling coordinates the two heaters so that a request for one temperature does not move
  the other: when one heater goes up, the other is turned down by just enough to cancel the cross effect.
- **Mathematics:** $u = Mv$ with $M = K^{-1}\,\tfrac{1}{9}I = \begin{bmatrix}1 & -4/9\\ -4/9 & 1\end{bmatrix}$, so that $KM = \tfrac{1}{9}I$:
  each heater request moves only its own temperature, by 1/9 °C per point.
- **How the lesson solves it:** Control Studio calculates and applies $M$ together with both PIs; the cross effect
  disappears at steady state.
- **Leaves for lesson 73:** if the real coupling differs from the model, the decoupler over- or under-corrects.

### 73. Multivariable design selection
- **Problem from lesson 72:** the arrangements have not been compared under coupling uncertainty.
- **New theory:** choosing a multivariable arrangement under stated uncertainty.
- **Plain words:** design selection compares the complete arrangements on the same requirements, including the cases
  where the coupling is weaker or stronger than modelled, and picks the simplest that passes them all.
- **Mathematics:** $\begin{bmatrix}10 + H_c & -H_c\\ -H_c & 10 + H_c\end{bmatrix}x = 2v$ for the range of coupling
  $H_c$.
- **How the lesson solves it:** the student validates each arrangement across the coupling range in Control Studio. With
  Control Studio's designs no arrangement passes every coupling case, so no single design is chosen for the whole
  range; the decoupled group qualifies from 8 kW/°C.
- **Leaves for lesson 74:** steady gain matrices say nothing about stored water inside the process, or about what
  fewer sensors can reveal.

## 11. State space, estimation and optimal control (74–82)

### 74. State-space models
- **Problem from lesson 73:** gain matrices do not describe a process's internal stores.
- **New theory:** the state-space model; states, inputs, outputs.
- **Plain words:** a state-space model keeps track of every store inside the process, here the levels of two tanks, and
  says how each store changes with the inputs and the other stores, and what the sensors see.
- **Mathematics:** $\dot x_1 = -0.04x_1 + 2v$ and $\dot x_2 = 0.04x_1 - 0.04x_2$, written as $\dot x = Ax + Bv$, $y = Cx$.
- **How the lesson solves it:** the student derives the model from the two tanks' volume balances and checks it against
  a recording.
- **Leaves for lesson 75:** the equations do not tell whether one pump can steer both levels.

### 75. Controllability
- **Problem from lesson 74:** can one pump steer both levels?
- **New theory:** controllability.
- **Plain words:** a system is controllable when its inputs can move every store to any value over time. One pump can
  steer both tanks, because the first tank feeds the second.
- **Mathematics:** $\mathcal C = [B,\ AB] = \begin{bmatrix}2 & -0.08\\ 0 & 0.08\end{bmatrix}$; full rank means
  controllable.
- **How the lesson solves it:** the student calculates the matrix and checks it with Control Studio's state structure.
- **Leaves for lesson 76:** being able to steer the states does not mean the sensor tells us what they are.

### 76. Observability
- **Problem from lesson 75:** can the downstream sensor tell us both levels?
- **New theory:** observability.
- **Plain words:** a system is observable when its states can be worked out from what the sensors show over time. The
  downstream level, and how fast it changes, reveal the upstream level.
- **Mathematics:** $x_1 = y + 25\dot y$; $\mathcal O = \begin{bmatrix}C\\ CA\end{bmatrix}$ of full rank.
- **How the lesson solves it:** the student reconstructs the upstream level and sees why noisy differentiation makes
  that unreliable in practice.
- **Leaves for lesson 77:** knowing the states can be found does not choose how the controller should use them.

### 77. State feedback
- **Problem from lesson 76:** the controller does not yet use the states.
- **New theory:** state feedback; pole placement.
- **Plain words:** state feedback sets the pump from every state, not just from one error, and so can choose how quickly
  each natural motion dies away.
- **Mathematics:** $v = -Kx$; the closed loop $\dot x = (A - BK)x$ has the poles chosen by
  $\det(sI - A + BK) = $ the desired polynomial.
- **How the lesson solves it:** Control Studio places the poles and validates the law on the actual ST.
- **Leaves for lesson 78:** the law needs both levels, but only the downstream level is measured.

### 78. State observers
- **Problem from lesson 77:** one state is not measured.
- **New theory:** the state observer.
- **Plain words:** an observer runs the model alongside the process and corrects the model's states with the difference
  between what the sensor shows and what the model expected.
- **Mathematics:** $\dot{\hat x} = A\hat x + Bv + L(y - C\hat x)$; the estimate error obeys $\dot e = (A - LC)e$.
- **How the lesson solves it:** the student chooses $L$ and sees the estimate converge, and how noise limits speed.
- **Leaves for lesson 79:** a faster correction does not say how to weigh an uncertain model against a noisy sensor.

### 79. Kalman filtering
- **Problem from lesson 78:** there is no principled balance between model and sensor.
- **New theory:** variance and covariance; the Kalman filter.
- **Plain words:** a Kalman filter blends the model's prediction with the measurement, trusting each in proportion to
  how certain it is: the noisier the sensor, the less the estimate follows it.
- **Mathematics:** for one state $\hat x^+ = \hat x^- + K(z - \hat x^-)$ with $K = \frac{P^-}{P^- + R}$; in matrix form
  $K = P^-C^T(CP^-C^T + R)^{-1}$.
- **How the lesson solves it:** the student performs one prediction and correction by hand and compares the filter with
  the observer on the same recording.
- **Leaves for lesson 80:** estimation does not decide how much pump effort is worth spending.

### 80. LQR control
- **Problem from lesson 79:** the trade-off between error and effort is not stated.
- **New theory:** the linear-quadratic regulator (cost function, optimal gains).
- **Plain words:** LQR chooses the feedback gains that best balance two costs we state: how much the levels may deviate
  and how much the pump may work.
- **Mathematics:** minimise $J = \sum (x_k^TQx_k + v_k^TRv_k)$; the gain follows from
  $K = (R + B_d^TPB_d)^{-1}B_d^TPA_d$.
- **How the lesson solves it:** Control Studio calculates the gains from $Q$ and $R$; the student compares two choices
  of $R$.
- **Leaves for lesson 81:** the design assumes measured states, but ours come from a noisy estimator.

### 81. LQG control
- **Problem from lesson 80:** LQR needs states that are only estimated.
- **New theory:** LQG (LQR with a Kalman filter).
- **Plain words:** LQG feeds the Kalman filter's estimates into the LQR law, so the optimal feedback works with only
  the measured level.
- **Mathematics:** $v_k = -K\hat x_k^+$, with $\hat x$ from the Kalman filter using the previously applied input.
- **How the lesson solves it:** the student sets the combination up with the setup form and validates it on the actual
  ST.
- **Leaves for lesson 82:** the law does not plan around future limits or a shared actuator budget.

### 82. Model predictive control
- **Problem from lesson 81:** future limits and a shared budget are not planned for.
- **New theory:** model predictive control.
- **Plain words:** MPC predicts the future for several possible plans, picks the best plan that respects every limit,
  applies only its first step, and plans again at the next update.
- **Mathematics:** minimise $J = \sum_{k=1}^{3}[(T_{1,k} - r_1)^2 + (T_{2,k} - r_2)^2] + 0.002\sum \Delta u^2$ subject to
  $0 \le u_i \le 100$ and $u_1 + u_2 \le 100$.
- **How the lesson solves it:** the student runs MPC on two heaters sharing 100 % of power and compares it with two PIs.
- **Leaves for lesson 83:** feasible plans and simulations do not prove stability over a nonlinear operating range.

## 12. Nonlinear, robust and fault-aware control (83–89)

### 83. Lyapunov stability
- **Problem from lesson 82:** there is no stability proof for a nonlinear process.
- **New theory:** Lyapunov stability (an energy-like function).
- **Plain words:** Lyapunov's method finds an energy-like quantity that can only decrease while the arm moves. If the
  arm's energy keeps falling, it must come to rest at the target.
- **Mathematics:** $V = \tfrac12 J\omega^2 + \tfrac12 K_p\phi^2 + G(\cos\phi - 1)$ and
  $\dot V = -(B + K_d)\,\omega^2 \le 0$.
- **How the lesson solves it:** the student derives $V$, finds the region where it works within the torque limit, and
  checks it with runs.
- **Leaves for lesson 84:** near upright the arm is stable, but holding another angle still fights the angle-dependent
  gravity torque.

### 84. Feedback linearization
- **Problem from lesson 83:** gravity's torque changes with the angle.
- **New theory:** feedback linearization.
- **Plain words:** feedback linearization adds the torque that cancels the known nonlinear gravity torque, so the
  remaining loop behaves like the simple linear one we already know how to tune.
- **Mathematics:** $\tau = K_p(r - \phi) - K_d\dot\phi - \hat G\sin\phi$ gives
  $J\ddot\phi + (B + K_d)\dot\phi + K_p\phi = K_p r + (G - \hat G)\sin\phi$.
- **How the lesson solves it:** the student compares exact and wrong values of $\hat G$.
- **Leaves for lesson 85:** the compensation relies on a parameter that may change.

### 85. Adaptive control
- **Problem from lesson 84:** model parameters change during operation.
- **New theory:** adaptive control; parameter estimation.
- **Plain words:** adaptive control estimates a changing process parameter from operating data and updates the
  controller with it, but only when the data really contain that information.
- **Mathematics:** the least-squares estimate of the heat loss $\hat H = \frac{\sum x_i y_i}{\sum x_i^2}$.
- **How the lesson solves it:** the student compares adapted and fixed settings after the heat loss changes.
- **Leaves for lesson 86:** a parameter cannot always be estimated reliably, because the data lack excitation or
  disturbances confuse it.

### 86. Robust control
- **Problem from lesson 85:** the parameter is uncertain and cannot always be estimated.
- **New theory:** robust control; uncertainty bounds; robust stability and performance.
- **Plain words:** robust control designs one controller that works for the whole range the process can be in, without
  knowing its exact value at any moment.
- **Mathematics:** the relative model error $\delta(j\omega)$ must stay inside a bound $W(\omega)$, and stability needs
  $|T(j\omega)|\,W(\omega) < 1$ at every frequency.
- **How the lesson solves it:** Control Studio validates the controller on the corner cases of the range.
- **Leaves for lesson 87:** model bounds do not cover lost, reordered or old commands over a network.

### 87. Networked control
- **Problem from lesson 86:** commands arrive over an unreliable network.
- **New theory:** networked control: message order, age, holding and fallback.
- **Plain words:** networked control accepts a remote command only if it is new, in order and recent enough, holds the
  last good command for a limited time, and then falls back to a safe local action.
- **Mathematics:** the age $= t_{now} - t_{sent}$; a command is accepted only while its age is at most 0.2 s, and the last
  accepted command is held for at most 0.3 s before the safe local action.
- **How the lesson solves it:** the student tests loss, reordering and stale commands.
- **Leaves for lesson 88:** fresh, correctly delivered data can still come from a faulty sensor.

### 88. Fault-aware control
- **Problem from lesson 87:** a sensor can be wrong even when its data arrive correctly.
- **New theory:** fault detection (range, age, disagreement); sensor fallback; latching and recovery.
- **Plain words:** fault-aware control checks whether its measurements still make sense, by range, by age and by
  comparing two sensors, and switches to a backup or a safe action when they do not.
- **Mathematics:** two sensors disagree when $d = |T_A - T_B|$ exceeds a stated limit.
- **How the lesson solves it:** the student injects sensor faults and checks the fallback and the recovery.
- **Leaves for lesson 89:** each mechanism works; a complete application with competing requirements has not been
  designed independently.

### 89. Independent design (final assessment)
- **Problem from lesson 88:** the course's methods have not yet been combined by the student alone.
- **New theory:** independent control design: requirements, two candidate designs, feasibility, a defended choice.
- **Plain words:** independent design means stating the requirements yourself, choosing two suitable designs,
  calculating whether they can work, building both in ST, and defending your choice with measured evidence.
- **Mathematics:** for example, the feasibility check $u_{hold} = \frac{H(T - T_a) + D}{a} \le 100\,\%$.
- **How the lesson solves it:** the student designs, validates and defends a complete application in Control Studio.
- **Leaves:** the course ends here. What remains is physical commissioning on real equipment.
