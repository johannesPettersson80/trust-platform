# Control-course teaching specification

Shared purpose and completion claims: [Control Platform Purpose and Acceptance Contract](control-platform-purpose.md).
This document retains its detailed technical or teaching authority.

Date: 6 September 2026.

Status: Required teaching design. This specification does not assert that the
existing lesson READMEs or accompanying software already meet it.

## 1. Authority and scope

The product capabilities each lesson teaches are specified individually in the
[Complete Control Studio workflow specification](control-studio-complete-workflow-specification.md)
and its WF-001–WF-082 catalogue. That specification governs the automatic and
manual product operations; this document governs the teaching sequence,
approved explanations and physical cases. Missing product behavior must be
implemented rather than replaced with an ST-generated analytical answer.

This specification records the user's agreed consecutive teaching sequence and
problem-to-solution requirements for the truST control course. It governs the
next course Markdown revision, using the existing lesson READMEs as the starting
material under Section 10. Its 1–82 order takes precedence over older lesson
numbers, earlier proposed sequences and historical acceptance claims, including
the conflicting progression in
[Control Engineering Curriculum Architecture](38-control-engineering-curriculum.md).
Older documents remain as history; their completion claims do not accept the
lessons specified here.

The numbered entries below are teaching lessons, not existing folder numbers.
The order shall be presented consecutively from 1 to 82. Existing folder names,
code, configuration, metadata and saved results shall not determine the teaching
sequence. This specification does not rename or modify those files.

The previously separate derivative-on-measurement material belongs within
**34. PD control**. Its old location is a navigation notice, not another lesson.
The controller-comparison page remains a reference; controller choices are
demonstrated in the commissioning and independent-design lessons. Neither page
adds a numbered lesson to the 82-lesson sequence.

Writing this specification authorizes no implementation, configuration,
metadata, test, build, packaging or release changes. Short ST examples and
writing exercises in subsequent lesson Markdown remain permitted teaching
content. Alignment and execution of the lesson projects are later work.

## 2. Required teaching progression

Every ordinary lesson shall follow this causal development:

1. Restate the concrete physical problem or engineering decision established by
   preceding learning.
2. Explain what the student can already do and why that is insufficient for the
   stated problem.
3. Introduce the named concept that addresses that problem.
4. Explain the physical reasoning and required mathematics before asking for a
   prediction.
5. Give an ST task and a Control Studio activity that investigate the prediction.
6. Use the observations to answer the opening question and make a decision.
7. Establish the specific remaining problem that the next row addresses.

The third column of the table is a requirement for the lesson's ending, not a
generic list of everything a technique cannot do. The next lesson shall open
with that same unresolved problem. A topic name or an unmotivated new equation
does not establish a valid handoff.

An unresolved question may concern physical behavior, mathematical
interpretation, implementation, experiment design or independent integration.
Do not describe a mathematical method as fundamentally incapable merely because
the student has not yet learned how to use it. In particular, Laplace transforms
can predict transients; poles and zeros teach the student to interpret the
structure of the resulting expressions.

Assessments consolidate previously taught concepts. At an application change,
the preceding lesson shall introduce the new requirement and explain why the
next physical example is appropriate. Do not imply that successful cold-room
control needs replacing, that a motor controller should control an unrelated
tank without a new model, or that one industrial structure is a universal
upgrade over another.

Preserve the protected sequence:

**Proportional control → observe steady-state error → add integral action in PI
→ demonstrate removal of that same error.**

In this teaching order, that is the uninterrupted transition from **5** to **6**.

## 3. Consecutive teaching order

Use the short technical lesson names below. Keep the four columns: lesson,
concept taught, unresolved problem and the next concept addressing that problem.
The last column names the following numbered lesson, except at the course end.

| Lesson | Concept it teaches | What it cannot yet solve | Next concept that addresses the problem |
|---|---|---|---|
| **1. Control-loop elements** | Identify setpoint, controlled variable, measurement, actuator and command. Establish the physical response direction through a manual experiment. | The program measures temperature but does not automatically correct a deviation. | **ON/OFF control:** use the measurement to select corrective action. |
| **2. ON/OFF control** | Repeatedly compare temperature with the setpoint and switch refrigeration. Write the comparison and decision in ST. | Small crossings of one threshold cause excessive switching. Equipment minimum operating times are ignored. | **Hysteresis and dwell:** separate switching thresholds and enforce minimum state times. |
| **3. Hysteresis and dwell** | Retain switching state between thresholds and postpone transitions until minimum ON/OFF times have elapsed. | Working individual rules do not establish that the complete controller meets temperature and switching requirements under changed heat load. | **Cold-room commissioning:** integrate and assess those rules independently. |
| **4. Cold-room commissioning** | Write and assess the complete binary controller, including changed heat load, temperature variation, switching frequency and reset behavior. | Binary control cannot provide the continuously adjustable heating command required by the next application. | **Proportional control:** calculate a variable heater command from temperature error. |
| **5. Proportional control** | Calculate heater output from error. Observe how output decreases as temperature approaches the setpoint. | **At zero error, P produces zero heating although heat is still being lost. The heater settles below the setpoint.** | **PI control:** retain a heating contribution when the error reaches zero. |
| **6. PI control** | Accumulate error over time and add the stored integral contribution to P. Demonstrate removal of the same steady-state error observed previously. | The integral can continue growing when the heater cannot deliver additional power. | **Saturation and windup:** examine requested output, applied output and accumulated state separately. |
| **7. Saturation and windup** | Explain actuator capacity, unreachable setpoints and continued integral accumulation during saturation. Observe delayed recovery after returning to a feasible setpoint. | Returning to a feasible setpoint does not remove the accumulated, unusable output demand. | **Anti-windup:** correct integral state using the output the actuator actually applies. |
| **8. Anti-windup** | Derive applied-output tracking, explain its correction direction and compare recovery from the same saturation event. | Recovering from saturation does not establish acceptable performance during ordinary setpoint changes and disturbances. | **Setpoint and disturbance response:** define and assess the two response requirements separately. |
| **9. Setpoint and disturbance response** | Distinguish a setpoint change from a physical load change. Set requirements for overshoot, temperature excursion, settling and output use before testing. | A failed response requirement does not reveal whether tuning, measurement, actuator behavior or capacity caused it. | **Loop diagnosis:** choose observations that distinguish those causes. |
| **10. Loop diagnosis** | Check measurement credibility, command delivery, actuator behavior, operating conditions and feasibility before changing gains. | Once other causes are excluded, the student still needs a deliberate method for improving PI settings. | **Manual PI tuning:** change one gain for a stated reason and judge the result. |
| **11. Manual PI tuning** | Choose a bounded gain change, predict its effect, repeat matched experiments and keep or reject the new settings. | Gain trials alone do not separate the heater's dynamics from the response created by feedback. | **Process dynamics:** distinguish plant response from complete control-loop response. |
| **12. Process dynamics** | Identify which equilibrium and dynamic predictions would improve the next tuning decision. Explain why a closed-loop trace is not a direct plant measurement. | The required predictions have been identified, but their physical equations and parameter meanings have not been derived. | **Energy balance:** derive the heater model from storage, input and loss. |
| **13. Energy balance** | Derive temperature dynamics from heat capacity, delivered power, heat loss and delay. Explain units, assumptions and operating-point deviations. | A physically meaningful equation does not establish the actual equipment parameters. | **Identification experiments:** obtain informative measurements under bounded conditions. |
| **14. Identification experiments** | Choose operating point, command step, duration, measurement conditions and stopping limits. Establish a repeatable baseline and retain the applied-input record. | The resulting observations do not automatically provide gain, time constant or transport delay. | **Step-response identification:** calculate model parameters from the recorded response. |
| **15. Step-response identification** | Estimate gain, time constant and delay. Distinguish detectable onset from physical delay and explain uncertainty in the estimates. | A fitted first-order model can still miss a repeatable response feature, such as additional sensor lag. | **Model structure:** add a physically justified dynamic element when the simpler model cannot explain the observations. |
| **16. Model structure** | Compare simple and extended models. Derive an additional sensor or process state from its physical operation rather than adding an unexplained fitting parameter. | Several parameter combinations may explain the same measured response, even after the model structure improves. | **Identifiability:** determine what the experiment can distinguish. |
| **17. Identifiability** | Explain practical and structural parameter ambiguity. Choose additional excitation, longer observation or an independent measurement for a specific reason. | Distinguishable parameters do not establish prediction quality outside the identification record. | **Model validation:** test the unchanged model on unused observations. |
| **18. Model validation** | Assess timing, response shape and final value on independent command and disturbance records. Decide whether the model supports the intended design decision. | A validated model predicts the plant but does not select controller gains for a desired response. | **Model-based PI tuning:** connect the model to a response requirement. |
| **19. Model-based PI tuning** | Derive starting gains from the validated model and a chosen response time. Retain relevant delay, additional dynamics and uncertainty. | These settings depend on the fitted model. An alternative experimentally derived candidate has not yet been obtained. | **Relay tuning:** obtain a candidate from a bounded oscillation experiment. |
| **20. Relay tuning** | Produce and measure a repeatable bounded cycle. Explain the approximation and empirical rule used to calculate candidate settings. | Neither model-derived nor relay-derived settings are proven suitable merely because their calculations are complete. | **Tuning comparison:** compare all available candidates under unchanged requirements. |
| **21. Tuning comparison** | Compare manual, model-based and relay-derived candidates. Assess every requirement before choosing between qualifying settings. Allow none to qualify. | A suitable PI controller can still produce an unnecessarily large proportional command change when the setpoint changes. | **Setpoint weighting:** change the setpoint contribution without reducing the measurement-feedback coefficient. |
| **22. Setpoint weighting** | Separate setpoint and measurement contributions to P while retaining full error in I. Assess reduced command steps against slower response. | Measurement noise still changes the feedback signal and can cause unnecessary heater-command movement. | **Measurement filtering:** reduce noise sensitivity while assessing added delay. |
| **23. Measurement filtering** | Derive a filter update and compare noise reduction with delayed detection of real temperature changes. Distinguish raw, filtered and physical values. | Filtering cannot recover changes missed between acquisitions, and incorrect time intervals can corrupt state updates. | **Sampling and timing:** explain acquisition limits and use actual elapsed time. |
| **24. Sampling and timing** | Explain aliasing, acquisition timestamps, controller intervals and held commands. Scale integral and filter updates correctly and reject unusable timing. | Correct update equations can still produce a bad first command when stored state is initialized incorrectly. | **Controller initialization:** establish state from the actual operating condition. |
| **25. Controller initialization** | Introduce persistent controller instances, inputs, parameters, outputs and retained state. Initialize integral, filter and timing values deliberately. | Initialization aligns the controller once; subsequent manual output changes can leave its state unsuitable for automatic operation. | **Bumpless transfer:** track the applied output while automatic control is inactive. |
| **26. Bumpless transfer** | Align the integral with manual output and current proportional action. Assess the transfer-induced command change separately from later temperature response. | Changing gains, limits or filter settings can still mix incompatible values or cause unintended output movement. | **Parameter updates:** apply related settings together and adjust retained state consistently. |
| **27. Parameter updates** | Validate and apply a complete parameter set at one update boundary. Preserve intended state continuity and distinguish deliberate limit changes from accidental jumps. | Consistent parameters do not make invalid, stale or implausible measurements usable. | **Measurement validity:** define rejection, fallback and deliberate recovery. |
| **28. Measurement validity** | Detect unusable data, select the defined output response, preserve appropriate state and require fresh measurements before restarting automatic control. | Correct individual mechanisms do not establish that the student can combine them into one complete heater controller. | **Heater commissioning:** independently integrate and assess the complete loop. |
| **29. Heater commissioning** | Write the ST integration and assess setpoint response, disturbance recovery, startup, transfer, limits, parameter changes and measurement faults. | The next application uses a moving valve whose physical position cannot change as quickly as its requested position. | **Actuator tracking:** account for the actuator's actual movement. |
| **30. Actuator tracking** | Distinguish requested position, bounded target and measured position. Explain rate-limit windup and correct integral state using actual actuator position. | Stem position alone does not establish delivered flow or heating; the installation's pressure conditions also matter. | **Valve characteristics:** establish the position-to-flow relationship. |
| **31. Valve characteristics** | Explain and measure how valve position, pressure drop and pipe losses determine flow. Convert flow into heating at stated temperatures. | A steady characteristic cannot explain ignored small requests, holding, breakaway or movement after reversal. | **Valve stiction:** distinguish command deadband from mechanical friction. |
| **32. Valve stiction** | Diagnose deadband, holding force, breakaway and sticking through command, position, force and flow observations. Separate mechanical problems from tuning problems. | The valve investigation treats position as an observed result; it has not taught how a healthy motor produces and controls that position. | **Position control:** derive motion from torque, inertia and position feedback. |
| **33. Position control** | Introduce shaft angle, velocity, torque, inertia and friction. Apply proportional position feedback and explain continued motion at zero position error. | Position error alone does not directly oppose existing velocity, so stronger P can still produce overshoot and oscillation. | **PD control:** add velocity-dependent damping. |
| **34. PD control** | Calculate braking torque from measured velocity. Explain damping, derivative on measurement and setpoint-induced derivative kick. | At rest, D contributes no torque. A constant resisting load requires a remaining P error to produce holding torque. | **PID load rejection:** use integral action to supply holding torque at zero error. |
| **35. PID load rejection** | Calculate the torque from one explicit constant load. Compare PD steady error with PID recovery and account for torque saturation. | Derivative action depends on velocity information that may be noisy when calculated from sampled angle. | **Derivative filtering:** reduce noise-driven torque while measuring the cost to damping. |
| **36. Derivative filtering** | Explain noise amplification in a finite-difference velocity estimate. Filter it and assess both torque movement and delayed braking. | Understanding the separate terms does not establish a correctly ordered complete sampled PID update. | **Discrete PID:** combine the terms using consistent timing and retained state. |
| **37. Discrete PID** | Calculate P, trial I, filtered D, requested output, output limiting and tracking for one update. Identify old and new values explicitly. | A complete-output implementation does not establish equivalence with code written in incremental form. | **Positional and incremental PID:** derive the relationship between the two implementations. |
| **38. Positional and incremental PID** | Derive output increments from consecutive complete-output equations. Preserve the state and correction terms required for equivalent behavior. | Algebraic equivalence does not establish independent commissioning of the complete servo under load and lifecycle requirements. | **Servo commissioning:** assess the integrated controller on the taught mechanism. |
| **39. Servo commissioning** | Write and assess positioning, constant-load rejection, torque limits, initialization, transfer and fault recovery on the same shaft. | The next application controls liquid level: actuator flow changes stored volume rather than directly producing shaft torque. | **Volume balance:** derive the dynamics of an accumulating process. |
| **40. Volume balance** | Derive level rate from inflow, outflow and tank area. Explain why equal flows preserve the current level rather than restore a previous level. | The constant-outflow assumption does not explain passive drainage that increases with water level. | **Gravity outflow:** derive the outlet-flow relationship from physical head. |
| **41. Gravity outflow** | Derive the square-root flow relationship and combine it with volume storage. Explain level-dependent equilibrium and response speed. | One constant gain and time constant cannot describe the nonlinear tank across all operating levels. | **Linearization:** obtain a useful local model around a stated operating point. |
| **42. Linearization** | Derive a tangent approximation, local gain and time constant. Quantify errors for small and larger departures from the operating point. | A controller tuned from one local model may respond differently at another level because the local coefficients change. | **Gain scheduling:** adjust tuning using the known operating-level dependence. |
| **43. Gain scheduling** | Calculate scheduled PI settings from the local tank model. Preserve controller state and compare scheduled and fixed gains across operating levels. | Analysing each new plant/controller combination directly through differential equations remains cumbersome, especially when more dynamics are connected. | **Laplace transforms:** express the familiar dynamic equations in algebraic form. |
| **44. Laplace transforms** | Transform physical equations, retain initial-condition contributions and connect transfer functions. Solve the simple heater response and present more complicated familiar response expressions. | The student cannot yet read those expressions to explain decay, oscillation or why setpoint and disturbance responses expose different dynamic factors. | **Poles and zeros:** connect denominator and numerator roots to those response features. |
| **45. Poles and zeros** | Define poles and zeros before using them. Explain decay, growth, oscillation, input-response shape and specific cancellation examples through heater and shaft behavior. | Interpreting a supplied transfer function does not establish the complete internal dynamics after feedback or controller state changes. | **Characteristic equations:** derive the complete connected-loop mode equation. |
| **46. Characteristic equations** | Derive the equation governing natural modes from plant dynamics and the actual feedback law. Account for feedback signs, controller state and possible cancellation. | Checking one gain choice does not show how pole locations change as a gain is varied. | **Root locus:** trace pole movement and select candidate gains. |
| **47. Root locus** | Vary one gain while holding the other controller and plant properties fixed. Relate pole movement to damping and response speed. | A plotted gain sweep leaves the complete stable gain interval and its boundary cases unestablished. | **Routh–Hurwitz stability:** calculate the stable interval from polynomial coefficients. |
| **48. Routh–Hurwitz stability** | Construct and interpret the Routh table. Find a stable gain interval and distinguish stable, marginal and unstable boundary cases. | Natural-mode stability does not quantify the response to a continuing disturbance or command that repeats at different rates. | **Periodic excitation:** produce controlled repeating inputs for that investigation. |
| **49. Periodic excitation** | Choose a bounded sinusoidal input, operating point, frequency, amplitude and observation duration. Exclude startup transients and retain actual applied signals. | Repeating traces alone do not provide a quantitative measure of attenuation and response lag. | **Frequency response:** calculate and measure amplitude ratio and phase. |
| **50. Frequency response** | Derive sinusoidal response, introduce complex magnitude and angle, and interpret Bode plots with meaningful units and references. | The plant's open-loop response does not explain how feedback treats physical disturbances and measurement noise differently. | **Sensitivity functions:** derive the distinct closed-loop signal paths. |
| **51. Sensitivity functions** | Derive sensitivity and complementary sensitivity. Trace disturbances and noise into physical output and controller command, retaining the correct physical input paths. | Identifying an unacceptable frequency tradeoff does not yet provide a controller modification that improves it. | **Loop shaping:** change controller dynamics for a stated response requirement. |
| **52. Loop shaping** | Derive and implement lead/lag compensation. Assess the complete loop, noise amplification, actuator demand and temperature performance under fixed requirements. | Added phase at one frequency does not establish stability of the complete frequency curve, especially when transport delay is present. | **Nyquist stability:** assess the full connected-loop frequency behavior. |
| **53. Nyquist stability** | Explain the critical point, directed curve, pole count and encirclements. Determine how delivery delay changes stability and interpret slow growth near the boundary. | A continuous-delay model does not establish stability with sampled measurements, held commands and computation delay. | **Sampled-data stability:** derive and assess the actual discrete timing model. |
| **54. Sampled-data stability** | Derive held-command state updates and include the pending command. Explain unit-circle stability and separate physical timing from numerical integration errors. | Correct digital timing still leaves feedback waiting for temperature error after a disturbance. A measured disturbance could support earlier action. | **Feedforward control:** calculate corrective command directly from the measured disturbance. |
| **55. Feedforward control** | Derive a disturbance-based command from physical balance. Combine it with feedback and assess measurement error, limits and delivery delay. | An unmeasured jacket disturbance still reaches the vessel before vessel-temperature feedback can respond adequately. | **Cascade control:** use an intermediate measurement that reveals the disturbance earlier. |
| **56. Cascade control** | Explain outer and inner loops, their different output quantities, relative response speeds, initialization and tracking. Assess earlier jacket-disturbance correction. | A further production requirement is different: additive flow must follow changing carrier flow. A fixed additive-flow setpoint cannot preserve the required ratio. | **Ratio control:** generate the dependent flow setpoint from measured carrier flow. |
| **57. Ratio control** | Calculate and regulate an additive-to-carrier ratio. Account for pump lag, invalid measurements, low carrier flow and residual delivery after stopping. | The next vessel requirement needs both heat addition and heat removal; a single positive heating command cannot represent both actions. | **Split-range control:** map a signed thermal demand to heating and cooling actuators. |
| **58. Split-range control** | Convert net heat demand into appropriately scaled heater and cooler commands. Explain service exclusion, neutral regions and tracking from applied net heat. | Satisfying vessel heat demand can conflict with a separate jacket-temperature constraint. | **Override control:** give the constraint authority over the normal heating request. |
| **59. Override control** | Select between normal and limiting requests, track inactive controllers and distinguish incompatible requirements from poor tuning. | Respecting constraints does not compensate significant delivery delay in a heater loop; corrective heating may still arrive too late. | **Smith prediction:** account for the known delay within the feedback arrangement. |
| **60. Smith prediction** | Combine delay-free and delayed model predictions with current measurement correction. Assess model mismatch and transfer to ordinary feedback when prediction is unsuitable. | A working individual structure does not establish which complete arrangement meets the vessel application's performance, constraint and lifecycle requirements. | **Vessel commissioning:** compare and integrate the appropriate taught structures. |
| **61. Vessel commissioning** | Select and assess suitable single-loop, cascade, split-range or override arrangements. Keep physical assumptions and requirements fixed across candidates. | A single-vessel arrangement does not establish control pairings when each of two actuators affects both controlled temperatures. | **Multivariable process paths:** measure and model all relevant cross effects. |
| **62. Multivariable process paths** | Derive coupled balances and measure each actuator's effect on each output. Introduce the gain matrix through its scalar physical relationships. | Gains measured with the other command fixed change when the other loop moves its actuator to hold its output. | **Relative gain array:** assess that interaction before selecting pairings. |
| **63. Relative gain array** | Derive isolated and compensated gains. Explain inverse, transpose and elementwise operations through the physical pairing calculation. | A steady pairing calculation does not establish transient interaction or acceptable simultaneous PI behavior. | **Decentralized PI:** run and assess the paired feedback loops together. |
| **64. Decentralized PI** | Use separate PI states for paired loops. Assess changed and unchanged outputs under both setpoint directions and disturbance locations. | The unaffected vessel may deviate before its own feedback corrects the coupling effect. | **Static decoupling:** coordinate actuator requests using the cross gains. |
| **65. Static decoupling** | Derive the virtual-to-physical command map, holding biases and correct tracking through physical limits. Compare steady cancellation and transient costs. | The nominal map may overcorrect or undercorrect when the physical coupling differs from its assumed value. | **Multivariable design selection:** compare arrangements across the stated coupling uncertainty. |
| **66. Multivariable design selection** | Compare fixed architectures under unchanged requirements and justified uncertainty cases. Separate feasibility, transient performance and evidence coverage. | Steady interaction maps do not fully describe internal storage or determine what fewer actuators and sensors can control and reveal. | **State-space models:** represent the internal dynamics explicitly. |
| **67. State-space models** | Derive the two-tank state equations from storage balances. Explain states, inputs, outputs and matrix rows through physical quantities and units. | State equations alone do not establish whether one pump can influence both state components independently over time. | **Controllability:** examine direct and propagated actuator influence. |
| **68. Controllability** | Explain reachable local transient states and the controllability calculation. Distinguish transient reachability from independently maintainable steady setpoints. | Actuator influence does not establish whether the available measurement contains enough information about both states. | **Observability:** examine the information contained in measurement history. |
| **69. Observability** | Derive how downstream level and its change reveal upstream state. Explain why noisy differentiation can be practically unreliable despite theoretical observability. | Knowing that state information exists does not select the feedback law that should use those states. | **State feedback:** establish the required controller using known states as a reference. |
| **70. State feedback** | Calculate a pump correction from both level deviations. Derive gains from chosen closed-loop modes and assess actuator limits and local-model validity. | The intended controller requires both states, while the available installation may measure only downstream level. | **State observers:** estimate the missing state from model and measurement. |
| **71. State observers** | Derive model prediction plus innovation correction. Choose observer gains, preserve update order and assess convergence and noise sensitivity. | Faster observer correction does not provide a principled balance between uncertain prediction and noisy measurement. | **Kalman filtering:** weight those information sources using stated uncertainty. |
| **72. Kalman filtering** | Define variance and covariance, derive weighting and perform a complete prediction/correction calculation. Compare estimated error with evaluation truth. | Estimation does not determine how much pump effort should be used to reduce the estimated level deviations. | **LQR control:** quantify the tradeoff between state error and control effort. |
| **73. LQR control** | Derive costs from physical scales. Explain one-step minimization, future cost, backward calculation and the resulting finite-horizon feedback gains. | A full-state design does not establish performance when its states are supplied by a noisy, imperfect estimator. | **LQG control:** connect the estimator and effort-based feedback. |
| **74. LQG control** | Explain the assumptions behind combining LQR and Kalman filtering. Integrate prediction, correction and control using the previous applied input. | An unconstrained feedback calculation does not explicitly plan around future state limits or a shared actuator-resource budget. | **Model predictive control:** select feasible future command sequences. |
| **75. Model predictive control** | Predict over a stated horizon, evaluate explicit costs and constraints, apply the first selected command and replan. Distinguish planning failure from insufficient capacity. | Feasible predictions and finite simulations do not establish closed-loop stability over a nonlinear operating region. | **Lyapunov stability:** construct a regional stability argument from the physical dynamics. |
| **76. Lyapunov stability** | Derive an energy-like function for the nonlinear arm, explain its rate of change and establish a region consistent with actuator limits. | Stability near upright does not remove angle-dependent gravity torque when the application requires tracking or holding a nonzero angle. | **Feedback linearization:** compensate the known nonlinear torque explicitly. |
| **77. Feedback linearization** | Derive nonlinear compensation from physical forces. Distinguish exact model cancellation from local approximation and assess coefficient error, noise and limits. | Compensation depends on model parameters that may change and may no longer be known accurately. | **Adaptive control:** estimate justified parameter changes and revise control consistently. |
| **78. Adaptive control** | Derive parameter estimation, state its information requirements and qualify updates before changing tuning. Compare adapted and fixed settings on the same physical condition. | Parameters cannot always be estimated reliably because excitation is insufficient or disturbances confound the calculation. | **Robust control:** design for a justified uncertainty range without requiring its exact current value. |
| **79. Robust control** | Define bounded model uncertainty, check feasibility and distinguish robust stability from robust performance. Explain finite tests and range-wide bounds. | Plant-model uncertainty bounds do not account for arbitrary communication loss, message reordering or stale remote commands. | **Networked control:** define command acceptance, holding and communication fallback. |
| **80. Networked control** | Handle sequence, connection identity, command age, clock assumptions, finite holding, local fallback and deliberate remote recovery. | Correctly delivered, fresh data can still contain a faulty measurement or disagreement between sensors. | **Fault-aware control:** select measurements only when the available evidence justifies it. |
| **81. Fault-aware control** | Distinguish range, age and disagreement faults. Implement justified sensor fallback, state alignment, fault latching and qualified recovery. | Correct mechanisms do not establish an independently justified solution for a complete application with competing requirements and uncertainty. | **Independent design:** combine, compare and defend the necessary taught concepts. |
| **82. Independent design** | Define requirements, select two suitable designs, calculate feasibility, write their ST integrations and defend the choice using matched performance, uncertainty and recovery evidence. | Untested equipment and operating conditions remain outside the demonstrated result and must be identified explicitly. | **Course complete.** Continue with implementation verification and physical commissioning, not another compulsory theory lesson. |

## 4. Mandatory lesson structure and language

After the short technical title and a direct opening, every teaching lesson,
including advanced lessons and assessments, shall use these nine sections in
this order:

1. The problem
2. What we need
3. The new idea
4. Prediction
5. Implementation
6. Experiment in Control Studio
7. Evidence and decision
8. Conclusion
9. Reason for the next lesson

Each section shall answer its own question directly:

| Section | Question it must answer |
|---|---|
| The problem | What remains unresolved from the preceding learning, or what new physical requirement has arisen? |
| What we need | What capability would let us solve that problem or make that decision? |
| The new idea | **What is [the new topic taught in this lesson]?** Then: what is it used for, how does it work, and what mathematics explains it? |
| Prediction | What do we expect before running, and why? |
| Implementation | How do we express the method in Structured Text? |
| Experiment in Control Studio | What exactly does the student do to use the method? |
| Evidence and decision | What happened, and does it solve the stated problem or meet the requirement? |
| Conclusion | What have we learned from the explanation and experiment? |
| Reason for the next lesson | What remains unresolved now, and why does that require the next concept? |

The first explanatory sentence of **The new idea** shall give an actual
definition of the named topic. For example: “A root locus is a plot showing
how the closed-loop poles move as one controller gain changes.” It shall not
begin with model coefficients, an instruction to calculate, or an instruction
to a future author to explain the method. Compound topics define both parts;
assessments define what the commissioning or design activity establishes.

**What we need** states the missing capability, not a parameter list. For root
locus, we need a way to choose proportional gain for acceptable speed and
overshoot by seeing how pole locations change with gain. Shaft parameters and
model equations belong with the explanation that uses them; reset settings
belong with implementation or the experiment. Preserve those necessary details
when relocating them, rather than deleting conditions needed to reproduce a run.

These questions govern the actual lesson text, not only its headings. The
individual concept explanations in Section 11 supply the content to use.

Use the clear explanations and physical reasoning of the existing foundational
READMEs on control-loop elements, ON/OFF control, hysteresis/dwell and
proportional heater control as the style reference. Their historical folder
numbers are not the numbering of this specification.

The title shall name the concept concisely, as in “PI control”, “Laplace
transforms” and “Poles and zeros”. The opening shall state the capability and
physical purpose directly. If a lesson teaches disturbance rejection, it shall
say so at the top. Use standard technical terms such as setpoint, measurement,
disturbance, torque and integral state; explain them at first use rather than
replacing them with imprecise expressions.

Write professional, concise prose for a student new to control theory. Use short
connected paragraphs, small worked examples and explained units. Avoid slogans,
administrative wording, unexplained formulas, expert shorthand and repeated
disclaimers. Nine correct headings alone do not establish pedagogical quality.

## 5. Physical and mathematical continuity

- Assume only basic algebra and calculus. Introduce additional operations and
  notation where they are needed, with enough reasoning to follow the worked
  calculation.
- Explain equipment, controlled quantity, measurement, action, disturbances and
  constraints before using the model. Derive physical relationships or explain
  how their coefficients could be measured. Mark supplied illustrative values
  as such.
- Keep one plant and operating-point record within each connected investigation.
  State every deliberate equipment, load, parameter or operating-point change,
  its reason and its expected effect.
- Preserve the introductory heater's 35 °C setpoint, 20 °C warming example,
  proportional gain 2 and introductory integral gain 0.5, on the 0–100% command
  scale. State gain units and retain one PI parameterization. Distinguish that
  initial warming experiment from later experiments starting at a settled hot
  operating point.
- Carry the student's heater model, parameter meanings, units and uncertainty
  from identification into validation and tuning. Do not silently replace it
  with supplied values or discard an essential model state to use a convenient
  tuning formula.
- Retain one shaft, sign convention and unit system through position control,
  damping, holding, digital implementation and servo commissioning. Use the
  same explicitly derived constant load in the holding calculation and
  experiment. Introduce the nonlinear arm's angle-dependent gravity separately
  and explain why that mechanism is needed.
- Retain one tank through volume balance, gravity outflow, linearization and gain
  scheduling; one coupled process through multivariable design; and one two-tank
  local model and operating point through state control and estimation.
- Distinguish command, applied actuator quantity, physical state, measurement
  and calculated estimate. Distinguish absolute quantities from deviations and
  percentages from percentage-point changes.
- Define symbols at first use and explain whether each relationship is a
  physical law, approximation, empirical tuning rule or design choice. Check
  units, signs, limiting cases and worked numbers.
- Explain advanced design inputs and essential calculations in the lesson.
  Repeated arithmetic may be delegated to a tool after its meaning is clear;
  unexplained solver output or a long recursion is not a substitute for teaching.

The transition from **44. Laplace transforms** to **45. Poles and zeros** is a
required explicit review point. Lesson 44 shall establish an actual unresolved
interpretation problem using the familiar models and responses. Lesson 45 shall
restate it, define poles and zeros, and use them to answer it. Lesson 46 shall
then add complete connected-loop reasoning rather than repeat the same
denominator-root exercise.

## 6. Structured Text and Control Studio

Every actual lesson shall include a concrete ST reading, writing or modification
task linked to its principal concept. Explain any new construct before requiring
it. Reinforcing an existing skill is sufficient; a new language feature is not
required in every lesson.

The writing progression shall develop declarations and assignment, comparisons
and decisions, retained switching state, P arithmetic, integral updates,
requested/applied output and tracking, model calculations, stateful interfaces,
initialization, modes, parameter records, measurement validity, and appropriate
advanced state or coefficient calculations. Assessments shall require coherent
integrations and meaningful checks with reduced scaffolding.

Control Studio activities shall connect observation, experiment design,
identification, validation, tuning and comparison. Each activity shall specify:

- initial physical, controller and model states;
- the student's prediction and the setting or design they choose;
- signals, units and the purpose of observing each signal;
- which actions the student performs and which changes the experiment performs;
- one clear time origin, event times and a justified duration;
- requirements and comparison conditions fixed before viewing results;
- stopping conditions, the physical consequence of stopping and a full reset;
- how observations answer the prediction and support the stated decision.

An exercise about selecting settings shall give the student a real choice.
Filtering, damping, compensation and other changes shall be assessed for costs
as well as benefits. Smaller error, a smoother trace or a more elaborate method
shall not automatically be called better control. Infeasible requirements shall
be distinguished from poor tuning, and “none of these candidates meets the
requirements” shall remain a valid result.

Current software limitations shall not dictate the intended teaching. The text
shall distinguish proposed activities, mathematical predictions and actual
recorded observations. No execution result, software qualification or physical
equipment result may be invented.

## 7. Course goals and assessments

The complete course shall develop all five capabilities below through student
tasks, not merely mention their terminology.

| Course goal | Required demonstration |
|---|---|
| Practical control theory | Explain the physical response, derive the necessary relationships, make a prediction and interpret the observed consequence. All commissioning lessons and independent design require this reasoning. |
| Production-quality ST understanding | Explain retained state, units, initialization, modes, limits, validity and recovery. Heater, servo and vessel commissioning require integrated handling. |
| Writing ST | Progress from small decisions and calculations to independently written controller integrations and meaningful checks. Cold-room, heater, servo, vessel and independent-design assessments require written work. |
| Using truST and Control Studio | Carry observations through experiment planning, modelling, validation, tuning and matched comparison. Assessments require the student to organize and interpret that workflow. |
| Designing real loops | Set requirements, check capacity, establish parameter origins, account for actuator behavior and uncertainty, compare designs and defend a fallback or redesign decision. Independent design brings these decisions together. |

Assessments shall introduce no untaught plant physics or control theory. A
controller-comparison reference may support the decision but shall not supply
the conclusion the student is supposed to defend.

## 8. Review and acceptance

For each lesson, start with its mapped existing README and correct only the
failed passages under Section 10. Then read it from the student's available
knowledge. Answer its prediction questions using only that knowledge, walk
through the activity on paper, recalculate worked examples and read the
predecessor's ending beside its opening. Also read its ending beside the next
lesson's opening. Correct every missing explanation or inconsistent transition.

Accept an individual lesson only when its opening promise, explanation,
prediction, ST task, experiment, decision and remaining problem agree. A later
dependency change requires re-review of affected lessons. A draft-complete
label, a heading count, a valid link or a passing software test is insufficient
for pedagogical acceptance.

For the actual content to write, use each lesson's individual definition,
equations, worked calculation and student activity in Section 11. Those
requirements must be present and connected in the lesson, not replaced by
instructions such as “explain the filter” or “derive the gain”. A topic mention,
a formula without defined quantities, or an unrelated experiment does not
fulfil the content requirement.

Read each section against its question in Section 4. In particular, read
“What we need” as a standalone answer about the missing capability, then read
the opening of “The new idea” as the answer to “What is [topic]?” Check the
meaning of those answers directly; a topic-name match or heading check cannot
establish that they explain the concept.

After individual reviews, read the complete 1–82 sequence in order. Check that
new terms and operations have prior or local explanations, connected examples
retain their physical meanings, requirements remain fixed within comparisons,
and every ordinary transition carries the stated problem forward. Check the
five course goals against actual teaching and assessment tasks. Check titles,
numbering, links and the two reference/navigation dispositions separately.

Report Markdown review and software execution as separate outcomes. Completion
of a future Markdown revision means that the course text has been brought into
conformance and reviewed against this specification; it does not mean the lesson projects have
been implemented, executed or physically qualified.

## 9. Later implementation boundary

Subsequent implementation work shall align the ST projects, shared plant
models, simulation states and events, controller interfaces, Control Studio
activities, configuration and metadata with the accepted teaching. It shall
then verify the specified behavior through appropriate executable and interface
checks. Builds, packaging, releases and physical commissioning are outside the
present specification-writing task.

When a learner copies one lesson project, links to other catalogued lessons
must remain actionable without assuming sibling project folders were copied.
Resolve both numbered and descriptive link labels through the course catalogue,
retain the authored label, and provide the current lesson number and gallery
navigation. Keep external links, anchors, unresolved relative links and the
original bundled README unchanged.

Project ownership of the running controller does not replace the required
Control Studio analysis. If a lesson teaches finding a root locus, the student
must obtain and use that plot in Control Studio; implementing a root calculator
in ST and observing a time trace is not the same lesson. ST implements the
controller or the specifically taught programming task.

Historical inspection on 7 September 2026 found that
`controlStudioStageAvailability` disabled Identify, Tune and
Validate for `project-scripted` execution. At that checkpoint these lesson
instructions requested excluded stages:

| Lesson | Required stage excluded at the 7 September checkpoint |
|---|---|
| 15. Step-response identification | Identify |
| 16. Model structure | Identify |
| 17. Identifiability | Identify |
| 18. Model validation | Validate |
| 19. Model-based PI tuning | Tune |
| 21. Tuning comparison | Tune, Validate |
| 47. Root locus | Tune / Root locus |
| 52. Loop shaping | Tune, Validate |
| 61. Vessel commissioning | Validate |

These findings required workflow implementation, not replacement prose or
moving the analysis into ST. Subsequent native Lesson 15 evidence now exercises
Identify, fitting and independent validation with project-scripted execution;
its analysis persistence is covered by the closed shared-state refactor. Use the
[release-readiness tracker](../internal/testing/checklists/control-platform-release-readiness.md)
for current per-lesson status; this historical table is not a reason to repeat
completed implementation. The same inspection found that Lesson 47 contained
the older carriage/lead-comparison project rather than the specified shaft
model. Before runnable acceptance, align the project, model, controller,
stage availability and exact plotting/selection behavior, then verify the
actual Control Studio interaction. A Markdown update or a copied bundled
README alone does not supply that implementation evidence.


## 10. Reuse and drift prevention

### 10.1 Reuse existing teaching content

The default action is to **reuse the existing lesson and edit only what fails
this specification**. A new position or shorter title does not make a lesson a
new writing task. Every numbered lesson has an existing source in Section 10.4;
none requires a blank-file draft merely because the course has been renumbered.

Reuse means retaining useful text, physical explanations, equations, worked
examples, ST exercises and experiment instructions. Keeping a folder or filename
while replacing all its prose does not satisfy this requirement.

Before changing a lesson, read its mapped source, the relevant physical-case
explanations and the adjacent teaching requirements. Identify what already
works and the precise remaining defect. Preserve every passage that is clear,
correct, appropriate to the student's knowledge and consistent with the agreed
lesson purpose. In particular, preserve the useful foundational explanations
and the existing PI accumulation example; do not normalize them into fresh prose
for stylistic uniformity.

Keep current working-tree content and pre-existing user changes. Do not restore
historical drafts, reset files to Git HEAD, overwrite the course from a template
or discard earlier corrections to start again. Historical acceptance labels
remain insufficient, but a withdrawn acceptance claim is not a reason to throw
away valid teaching material.

### 10.2 Make a targeted correction

Use the smallest correction that resolves the actual gap:

1. If the concept and explanation already work, change only the required title,
   numbering, prerequisite wording or navigation.
2. If a transition is missing, establish the unresolved problem in the
   predecessor's ending and restate it in the next opening. Amend the body or
   activity only where needed to make that handoff true.
3. If a term, equation or worked step is unexplained, add or repair that
   explanation next to its first necessary use. Retain correct surrounding
   calculations and physical assumptions.
4. If an experiment cannot answer the prediction, correct the relevant input,
   observation, state, timing or decision rule while preserving the comparison's
   declared physical conditions and requirements.
5. If sections need a clearer order, move and connect reusable passages before
   considering replacement prose.

Replace a whole lesson only when its mapped source cannot support the agreed
learning outcome through targeted changes. First identify the concrete
mismatch and why local repairs are insufficient; even then, recover useful
passages and calculations. An isolated bad sentence, an unrelated lesson's
failure, a new author, or the desire for a uniform voice does not justify a
whole-lesson or whole-course rewrite.

A local failure shall lead to a local correction and a focused check for the
same failure elsewhere. It shall not invalidate or restart unrelated lessons.
A valid equation or example shall not be removed simply to make a lesson
shorter. Conversely, preserving material is not a reason to retain a confirmed
error or an explanation the student cannot follow.

### 10.3 Freeze the agreed design while revising

The teaching order, short names and outcomes in Section 3 are the agreed design.
An editing pass shall not silently reorder the course, introduce another
curriculum, switch the shared plant, alter gain conventions or change the
comparison requirements. A change to those design decisions needs an explicit
new user decision; ordinary corrections already required by this specification
shall proceed without another permission request.

Use titles and links to the actual source locations while correcting
navigation. Do not rename directories or modify ST, metadata or configuration
as an incidental consequence of renumbering Markdown lessons. Keep legacy
folder numbers out of the learner-facing teaching order.

Do not invent an experimental failure to make a transition fit. Establish the
remaining problem with an explained calculation, a stated new requirement or a
clearly proposed observation. A change of application must be explicit and must
not be presented as evidence that the previous method failed on its own task.

If work is delegated, give the author the mapped file, the specific teaching
gap, the passages and physical values to preserve, and the permitted scope.
Delegation does not authorize a new draft from a topic name alone. The reviewing
agent shall compare the actual change with the existing text and read the
relevant predecessor and successor passages.

Review the changed lesson in full, but reopen neighbouring lessons only where
the change affects their explanation, prerequisites, case data or transition.
At final review, read the complete sequence without automatically rewriting it.
Checks of numbering, links and headings support this review; they do not prove
that an explanation is pedagogically adequate.

### 10.4 Existing-source map

This is an authoring map, not a second teaching order or an acceptance table.
The left column uses the new consecutive lesson numbers. Links on the right
identify the existing files to reuse; their directory names may retain legacy
numbers. Each source must still be checked against its new lesson's requirement.

| New lesson | Existing source README to reuse |
|---|---|
| 1. Control-loop elements | [cold-storage-loop-anatomy](../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/README.md) |
| 2. ON/OFF control | [cold-storage-on-off](../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/README.md) |
| 3. Hysteresis and dwell | [cold-storage-hysteresis-dwell](../../examples/control/01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/README.md) |
| 4. Cold-room commissioning | [cold-storage](../../examples/control/14-integrated-capstones/80-cold-storage/README.md) |
| 5. Proportional control | [water-heater-p](../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md) |
| 6. PI control | [water-heater-pi](../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi/README.md) |
| 7. Saturation and windup | [water-heater-saturation-windup](../../examples/control/01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup/README.md) |
| 8. Anti-windup | [water-heater-anti-windup](../../examples/control/01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup/README.md) |
| 9. Setpoint and disturbance response | [setpoint-and-disturbance-response](../../examples/control/01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response/README.md) |
| 10. Loop diagnosis | [loop-diagnosis](../../examples/control/09-industrial-control-structures/60-loop-diagnosis/README.md) |
| 11. Manual PI tuning | [manual-pi-tuning](../../examples/control/06-pid-tuning-methods/28-manual-pi-tuning/README.md) |
| 12. Process dynamics | [process-response-classes](../../examples/control/01-feedback-foundations-and-temperature-control/13-process-response-classes/README.md) |
| 13. Energy balance | [water-heater-energy-balance](../../examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md) |
| 14. Identification experiments | [safe-informative-experiment](../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/README.md) |
| 15. Step-response identification | [step-response-identification](../../examples/control/05-experiments-identification-and-validation/23-step-response-identification/README.md) |
| 16. Model structure | [choose-model-structure](../../examples/control/05-experiments-identification-and-validation/24-choose-model-structure/README.md) |
| 17. Identifiability | [parameter-identifiability](../../examples/control/05-experiments-identification-and-validation/25-parameter-identifiability/README.md) |
| 18. Model validation | [independent-model-validation](../../examples/control/05-experiments-identification-and-validation/26-independent-model-validation/README.md) |
| 19. Model-based PI tuning | [model-based-pi-tuning](../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning/README.md) |
| 20. Relay tuning | [relay-tuning](../../examples/control/06-pid-tuning-methods/29-relay-tuning/README.md) |
| 21. Tuning comparison | [tuning-method-comparison](../../examples/control/06-pid-tuning-methods/31-tuning-method-comparison/README.md) |
| 22. Setpoint weighting | [proportional-setpoint-weighting](../../examples/control/01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting/README.md) |
| 23. Measurement filtering | [measurement-filtering](../../examples/control/01-feedback-foundations-and-temperature-control/11-measurement-filtering/README.md) |
| 24. Sampling and timing | [elapsed-time-and-sampling](../../examples/control/01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling/README.md) |
| 25. Controller initialization | [controller-mode-initialization](../../examples/control/03-production-quality-digital-pid/39-controller-mode-initialization/README.md) |
| 26. Bumpless transfer | [bumpless-transfer](../../examples/control/03-production-quality-digital-pid/40-bumpless-transfer/README.md) |
| 27. Parameter updates | [atomic-parameter-update](../../examples/control/03-production-quality-digital-pid/41-atomic-parameter-update/README.md) |
| 28. Measurement validity | [invalid-stale-data](../../examples/control/03-production-quality-digital-pid/42-invalid-stale-data/README.md) |
| 29. Heater commissioning | [water-heater](../../examples/control/14-integrated-capstones/81-water-heater/README.md) |
| 30. Actuator tracking | [heating-valve-rate-tracking](../../examples/control/01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking/README.md) |
| 31. Valve characteristics | [valve-characterization](../../examples/control/09-industrial-control-structures/58-valve-characterization/README.md) |
| 32. Valve stiction | [valve-stiction](../../examples/control/09-industrial-control-structures/59-valve-stiction/README.md) |
| 33. Position control | [proportional-position-stiffness](../../examples/control/02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md) |
| 34. PD control | [derivative-motion-damping](../../examples/control/02-derivative-action-and-dynamic-response/33-derivative-motion-damping/README.md) |
| 35. PID load rejection | [servo-pid-holding-torque](../../examples/control/02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque/README.md) |
| 36. Derivative filtering | [filtered-derivative](../../examples/control/02-derivative-action-and-dynamic-response/36-filtered-derivative/README.md) |
| 37. Discrete PID | [discrete-integral-derivative](../../examples/control/03-production-quality-digital-pid/37-discrete-integral-derivative/README.md) |
| 38. Positional and incremental PID | [positional-incremental-pid](../../examples/control/03-production-quality-digital-pid/38-positional-incremental-pid/README.md) |
| 39. Servo commissioning | [servo](../../examples/control/14-integrated-capstones/82-servo/README.md) |
| 40. Volume balance | [conservation-and-accumulation](../../examples/control/01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation/README.md) |
| 41. Gravity outflow | [gravity-tank-nonlinearity](../../examples/control/01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity/README.md) |
| 42. Linearization | [gravity-tank-linearization](../../examples/control/01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization/README.md) |
| 43. Gain scheduling | [gain-scheduling](../../examples/control/09-industrial-control-structures/56-gain-scheduling/README.md) |
| 44. Laplace transforms | [transfer-function-from-physics](../../examples/control/01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics/README.md) |
| 45. Poles and zeros | [poles-zeros-and-response](../../examples/control/02-derivative-action-and-dynamic-response/19-poles-zeros-and-response/README.md) |
| 46. Characteristic equations | [characteristic-equation](../../examples/control/07-classical-stability-and-frequency-design/43-characteristic-equation/README.md) |
| 47. Root locus | [root-locus](../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/README.md) |
| 48. Routh–Hurwitz stability | [routh-hurwitz](../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz/README.md) |
| 49. Periodic excitation | [excitation-for-dynamics](../../examples/control/05-experiments-identification-and-validation/22-excitation-for-dynamics/README.md) |
| 50. Frequency response | [bode-frequency-response](../../examples/control/07-classical-stability-and-frequency-design/46-bode-frequency-response/README.md) |
| 51. Sensitivity functions | [sensitivity-functions](../../examples/control/07-classical-stability-and-frequency-design/48-sensitivity-functions/README.md) |
| 52. Loop shaping | [loop-shaping](../../examples/control/07-classical-stability-and-frequency-design/49-loop-shaping/README.md) |
| 53. Nyquist stability | [nyquist-stability](../../examples/control/07-classical-stability-and-frequency-design/47-nyquist-stability/README.md) |
| 54. Sampled-data stability | [position-z-plane](../../examples/control/07-classical-stability-and-frequency-design/50-position-z-plane/README.md) |
| 55. Feedforward control | [feedforward](../../examples/control/09-industrial-control-structures/51-feedforward/README.md) |
| 56. Cascade control | [cascade](../../examples/control/09-industrial-control-structures/52-cascade/README.md) |
| 57. Ratio control | [ratio-control](../../examples/control/09-industrial-control-structures/53-ratio-control/README.md) |
| 58. Split-range control | [split-range](../../examples/control/09-industrial-control-structures/54-split-range/README.md) |
| 59. Override control | [override-control](../../examples/control/09-industrial-control-structures/55-override-control/README.md) |
| 60. Smith prediction | [smith-predictor](../../examples/control/09-industrial-control-structures/57-smith-predictor/README.md) |
| 61. Vessel commissioning | [reactor-control](../../examples/control/14-integrated-capstones/83-reactor-control/README.md) |
| 62. Multivariable process paths | [mimo-paths](../../examples/control/10-multivariable-control/61-mimo-paths/README.md) |
| 63. Relative gain array | [relative-gain-array](../../examples/control/10-multivariable-control/62-relative-gain-array/README.md) |
| 64. Decentralized PI | [decentralized-pi](../../examples/control/10-multivariable-control/63-decentralized-pi/README.md) |
| 65. Static decoupling | [decoupling](../../examples/control/10-multivariable-control/64-decoupling/README.md) |
| 66. Multivariable design selection | [robust-mimo-selection](../../examples/control/10-multivariable-control/65-robust-mimo-selection/README.md) |
| 67. State-space models | [state-equations-from-stores](../../examples/control/02-derivative-action-and-dynamic-response/20-state-equations-from-stores/README.md) |
| 68. Controllability | [controllability](../../examples/control/11-state-space-estimation-and-predictive-control/66-controllability/README.md) |
| 69. Observability | [observability](../../examples/control/11-state-space-estimation-and-predictive-control/67-observability/README.md) |
| 70. State feedback | [state-feedback](../../examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/README.md) |
| 71. State observers | [state-observer](../../examples/control/11-state-space-estimation-and-predictive-control/70-state-observer/README.md) |
| 72. Kalman filtering | [kalman-filter](../../examples/control/11-state-space-estimation-and-predictive-control/71-kalman-filter/README.md) |
| 73. LQR control | [lqr](../../examples/control/11-state-space-estimation-and-predictive-control/69-lqr/README.md) |
| 74. LQG control | [lqg](../../examples/control/11-state-space-estimation-and-predictive-control/72-lqg/README.md) |
| 75. Model predictive control | [mpc](../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/README.md) |
| 76. Lyapunov stability | [lyapunov-stability](../../examples/control/12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability/README.md) |
| 77. Feedback linearization | [feedback-linearization](../../examples/control/12-nonlinear-adaptive-and-robust-control/75-feedback-linearization/README.md) |
| 78. Adaptive control | [adaptive-control](../../examples/control/12-nonlinear-adaptive-and-robust-control/76-adaptive-control/README.md) |
| 79. Robust control | [robust-control](../../examples/control/12-nonlinear-adaptive-and-robust-control/77-robust-control/README.md) |
| 80. Networked control | [networked-control](../../examples/control/13-networked-fault-aware-and-diagnostic-control/78-networked-control/README.md) |
| 81. Fault-aware control | [fault-aware-control](../../examples/control/13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control/README.md) |
| 82. Independent design | [independent](../../examples/control/14-integrated-capstones/84-independent/README.md) |

The remaining two source locations retain their non-lesson roles:

- [controller-selection](../../examples/control/04-controller-selection-and-plant-models/27-controller-selection/README.md) — Controller-comparison reference: reuse as a reference and as support for selection tasks in the assessments; do not add a numbered lesson.
- [derivative-on-measurement](../../examples/control/02-derivative-action-and-dynamic-response/35-derivative-on-measurement/README.md) — Derivative-on-measurement notice: retain navigation to new lesson 34, PD control; reuse any still-useful explanation there rather than repeat the introduction.

### 10.5 Known targeted review points

These are concrete starting points for correction, not a claim that every other
passage has already passed review:

- **New lessons 1–3 and 5:** preserve the existing foundational examples,
  explanations and useful wording. Apply the new names and navigation without
  rebuilding their teaching from scratch.
- **New lesson 6, PI control:** retain the clear successive integral updates and
  explanation of nonzero holding output at zero error. Check its connection to
  the unchanged P experiment; do not replace it with another introduction.
- **New lesson 15, Step-response identification:** retain the gain and response
  calculations. Make selection of windows, fitting procedure and uncertainty
  understandable and actionable where the current wording is insufficient.
- **New lessons 44–46:** reuse the heater transform derivation, shaft equations
  and valid root calculations. Repair the actual causal handoff, define poles
  and zeros before using them, and ensure characteristic equations add complete
  connected-loop reasoning rather than repeat an earlier calculation.
- **New lesson 73, LQR control:** retain the valid model, weights, first-step
  calculation, horizon and gain results. Add the missing explanatory and worked
  intermediate steps needed to understand future cost and backward recursion.
- **Assessments 4, 29, 39, 61 and 82:** reuse the taught plants, requirements and
  experiment material. Correct only missing independent decisions, integration
  tasks, declared prerequisites or fair comparisons; add no untaught physics.
- **Changed neighbours throughout the sequence:** repair titles, links and the
  incoming/outgoing problem. Do not assume that a new neighbour requires new
  explanations in all nine sections.

### 10.6 Acceptance of a reused lesson

A reused lesson is accepted only when it meets the teaching checks in Section 8
and its actual edits are justified by a specific gap. Inspect the before/after
text to confirm that correct explanations and calculations were preserved,
requirements were not changed to obtain a pass, and new prose stayed within the
lesson's agreed purpose.

A lesson may require only navigation changes, a small local repair, or a more
substantial repair to particular sections. Do not prescribe a percentage of text
to change or retain: the criterion is the required student capability and a
truthful problem-to-solution progression. Reuse reduces unnecessary writing;
it does not reduce the required quality of the final reading.

## 11. Required content to write in each lesson

The following files are normative parts of this specification. They expand
every Section 3 lesson into its actual explanation of what was learned before,
what problem or decision remains, what the new concept does and why we use it.
That explanation must appear before the procedure and mathematics. It must be
written as comprehensible teaching, like Lesson 20's step-test versus
relay-test explanation, not as an instruction to a future author to supply one.
Each entry also specifies its definition, physical meaning,
equations and symbol meanings, worked calculation, ST task, Control Studio
experiment and concluding decision. They preserve the agreed order and source
map. Use these specific contents to repair the mapped READMEs; do not replace
them with a generic instruction to explain a subject.

| Lessons | Detailed specification |
|---|---|
| 1–11 | [Foundations and PI control](control-course-content/01-11-foundations-and-pi.md) |
| 12–21 | [Models, identification and tuning](control-course-content/12-21-models-and-tuning.md) |
| 22–32 | [Implementation and actuators](control-course-content/22-32-implementation-and-actuators.md) |
| 33–43 | [Motion and tank dynamics](control-course-content/33-43-motion-and-tanks.md) |
| 44–54 | [Classical control analysis](control-course-content/44-54-classical-analysis.md) |
| 55–66 | [Industrial and multivariable control](control-course-content/55-66-industrial-and-multivariable.md) |
| 67–75 | [State estimation and optimal control](control-course-content/67-75-state-estimation-and-optimal-control.md) |
| 76–82 | [Nonlinear control, uncertainty and independent design](control-course-content/76-82-nonlinear-and-recovery.md) |

The detail belongs in this specification so that lesson prose can remain
short. Retain correct existing derivations and examples, show the essential
worked steps, and use supported tools for repeated arithmetic. The exact
project/UI instructions must agree with the resulting lesson implementation;
the specification does not assert that a requested plot or control already
exists. Record a concrete software gap when necessary rather than invent an
action or substitute a demonstration of a different concept.
