# Control engineering with Structured Text

Learn to design feedback control from the physical process, implement it in
Structured Text, and assess its response to setpoint changes and disturbances.
Follow the lesson order below: each new method addresses a problem or decision
established by the preceding learning.

The course develops five capabilities:

- **Practical control theory:** explain the physical response, derive the relationships needed for a control decision, and check their consequences.
- **Understanding production ST:** read stateful controller code, including initialization, operating modes, limits, measurement validity and recovery.
- **Writing ST:** progress from comparisons and assignments to complete controller integrations with meaningful tests.
- **Using truST and Control Studio:** observe the loop, plan experiments, identify and validate models, tune controllers and compare results.
- **Designing control loops:** establish requirements, check feasibility, account for uncertainty and justify the selected controller.

Basic algebra and calculus are the only assumed mathematics. Additional methods
are explained when they become necessary. Follow the consecutive teaching order
below. The lesson titles use this order; directory names retain their earlier
numbers so existing project links remain usable.

Begin with the cold room and its commissioning assessment. The heater sequence
then keeps one physical investigation through P, PI, limits, tuning, modelling
and commissioning. Later lessons introduce each new equipment requirement
explicitly and build on the relevant earlier reasoning.

The introductory heater setpoint is 35 °C, with water initially at 20 °C. Later
experiments establish a settled operating point before a setpoint or disturbance
change. Keep your model and experiment records: identification, validation and
tuning use the model you developed, including its uncertainty.

## Course order

| Lesson | What you will learn |
| --- | --- |
| [1. Control-loop elements](01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/README.md) | Identify setpoint, controlled variable, measurement, actuator and command. Establish the physical response direction through a manual experiment. |
| [2. ON/OFF control](01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/README.md) | Repeatedly compare temperature with the setpoint and switch refrigeration. Write the comparison and decision in ST. |
| [3. Hysteresis and dwell](01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell/README.md) | Retain switching state between thresholds and postpone transitions until minimum ON/OFF times have elapsed. |
| [4. Cold-room commissioning](14-integrated-capstones/80-cold-storage/README.md) | Write and assess the complete binary controller, including changed heat load, temperature variation, switching frequency and reset behavior. |
| [5. Proportional control](01-feedback-foundations-and-temperature-control/04-water-heater-p/README.md) | Calculate heater output from error. Observe how output decreases as temperature approaches the setpoint. |
| [6. PI control](01-feedback-foundations-and-temperature-control/05-water-heater-pi/README.md) | Accumulate error over time and add the stored integral contribution to P. Demonstrate removal of the same steady-state error observed previously. |
| [7. Saturation and windup](01-feedback-foundations-and-temperature-control/06-water-heater-saturation-windup/README.md) | Explain actuator capacity, unreachable setpoints and continued integral accumulation during saturation. Observe delayed recovery after returning to a feasible setpoint. |
| [8. Anti-windup](01-feedback-foundations-and-temperature-control/07-water-heater-anti-windup/README.md) | Derive applied-output tracking, explain its correction direction and compare recovery from the same saturation event. |
| [9. Setpoint and disturbance response](01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response/README.md) | Distinguish a setpoint change from a physical load change. Set requirements for overshoot, temperature excursion, settling and output use before testing. |
| [10. Loop diagnosis](09-industrial-control-structures/60-loop-diagnosis/README.md) | Check measurement credibility, command delivery, actuator behavior, operating conditions and feasibility before changing gains. |
| [11. Manual PI tuning](06-pid-tuning-methods/28-manual-pi-tuning/README.md) | Choose a bounded gain change, predict its effect, repeat matched experiments and keep or reject the new settings. |
| [12. Process dynamics](01-feedback-foundations-and-temperature-control/13-process-response-classes/README.md) | Identify which equilibrium and dynamic predictions would improve the next tuning decision. Explain why a closed-loop trace is not a direct plant measurement. |
| [13. Energy balance](01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance/README.md) | Derive temperature dynamics from heat capacity, delivered power, heat loss and delay. Explain units, assumptions and operating-point deviations. |
| [14. Identification experiments](05-experiments-identification-and-validation/21-safe-informative-experiment/README.md) | Choose operating point, command step, duration, measurement conditions and stopping limits. Establish a repeatable baseline and retain the applied-input record. |
| [15. Step-response identification](05-experiments-identification-and-validation/23-step-response-identification/README.md) | Estimate gain, time constant and delay. Distinguish detectable onset from physical delay and explain uncertainty in the estimates. |
| [16. Model structure](05-experiments-identification-and-validation/24-choose-model-structure/README.md) | Compare simple and extended models. Derive an additional sensor or process state from its physical operation rather than adding an unexplained fitting parameter. |
| [17. Identifiability](05-experiments-identification-and-validation/25-parameter-identifiability/README.md) | Explain practical and structural parameter ambiguity. Choose additional excitation, longer observation or an independent measurement for a specific reason. |
| [18. Model validation](05-experiments-identification-and-validation/26-independent-model-validation/README.md) | Assess timing, response shape and final value on independent command and disturbance records. Decide whether the model supports the intended design decision. |
| [19. Model-based PI tuning](06-pid-tuning-methods/30-model-based-pi-tuning/README.md) | Derive starting gains from the validated model and a chosen response time. Retain relevant delay, additional dynamics and uncertainty. |
| [20. Relay tuning](06-pid-tuning-methods/29-relay-tuning/README.md) | Produce and measure a repeatable bounded cycle. Explain the approximation and empirical rule used to calculate candidate settings. |
| [21. Tuning comparison](06-pid-tuning-methods/31-tuning-method-comparison/README.md) | Compare manual, model-based and relay-derived candidates. Assess every requirement before choosing between qualifying settings. Allow none to qualify. |
| [22. Setpoint weighting](01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting/README.md) | Separate setpoint and measurement contributions to P while retaining full error in I. Assess reduced command steps against slower response. |
| [23. Measurement filtering](01-feedback-foundations-and-temperature-control/11-measurement-filtering/README.md) | Derive a filter update and compare noise reduction with delayed detection of real temperature changes. Distinguish raw, filtered and physical values. |
| [24. Sampling and timing](01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling/README.md) | Explain aliasing, acquisition timestamps, controller intervals and held commands. Scale integral and filter updates correctly and reject unusable timing. |
| [25. Controller initialization](03-production-quality-digital-pid/39-controller-mode-initialization/README.md) | Introduce persistent controller instances, inputs, parameters, outputs and retained state. Initialize integral, filter and timing values deliberately. |
| [26. Bumpless transfer](03-production-quality-digital-pid/40-bumpless-transfer/README.md) | Align the integral with manual output and current proportional action. Assess the transfer-induced command change separately from later temperature response. |
| [27. Parameter updates](03-production-quality-digital-pid/41-atomic-parameter-update/README.md) | Validate and apply a complete parameter set at one update boundary. Preserve intended state continuity and distinguish deliberate limit changes from accidental jumps. |
| [28. Measurement validity](03-production-quality-digital-pid/42-invalid-stale-data/README.md) | Detect unusable data, select the defined output response, preserve appropriate state and require fresh measurements before restarting automatic control. |
| [29. Heater commissioning](14-integrated-capstones/81-water-heater/README.md) | Write the ST integration and assess setpoint response, disturbance recovery, startup, transfer, limits, parameter changes and measurement faults. |
| [30. Actuator tracking](01-feedback-foundations-and-temperature-control/08-heating-valve-rate-tracking/README.md) | Distinguish requested position, bounded target and measured position. Explain rate-limit windup and correct integral state using actual actuator position. |
| [31. Valve characteristics](09-industrial-control-structures/58-valve-characterization/README.md) | Explain and measure how valve position, pressure drop and pipe losses determine flow. Convert flow into heating at stated temperatures. |
| [32. Valve stiction](09-industrial-control-structures/59-valve-stiction/README.md) | Diagnose deadband, holding force, breakaway and sticking through command, position, force and flow observations. Separate mechanical problems from tuning problems. |
| [33. Position control](02-derivative-action-and-dynamic-response/32-proportional-position-stiffness/README.md) | Introduce shaft angle, velocity, torque, inertia and friction. Apply proportional position feedback and explain continued motion at zero position error. |
| [34. PD control](02-derivative-action-and-dynamic-response/33-derivative-motion-damping/README.md) | Calculate braking torque from measured velocity. Explain damping, derivative on measurement and setpoint-induced derivative kick. |
| [35. PID load rejection](02-derivative-action-and-dynamic-response/34-servo-pid-holding-torque/README.md) | Calculate the torque from one explicit constant load. Compare PD steady error with PID recovery and account for torque saturation. |
| [36. Derivative filtering](02-derivative-action-and-dynamic-response/36-filtered-derivative/README.md) | Explain noise amplification in a finite-difference velocity estimate. Filter it and assess both torque movement and delayed braking. |
| [37. Discrete PID](03-production-quality-digital-pid/37-discrete-integral-derivative/README.md) | Calculate P, trial I, filtered D, requested output, output limiting and tracking for one update. Identify old and new values explicitly. |
| [38. Positional and incremental PID](03-production-quality-digital-pid/38-positional-incremental-pid/README.md) | Derive output increments from consecutive complete-output equations. Preserve the state and correction terms required for equivalent behavior. |
| [39. Servo commissioning](14-integrated-capstones/82-servo/README.md) | Write and assess positioning, constant-load rejection, torque limits, initialization, transfer and fault recovery on the same shaft. |
| [40. Volume balance](01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation/README.md) | Derive level rate from inflow, outflow and tank area. Explain why equal flows preserve the current level rather than restore a previous level. |
| [41. Gravity outflow](01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity/README.md) | Derive the square-root flow relationship and combine it with volume storage. Explain level-dependent equilibrium and response speed. |
| [42. Linearization](01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization/README.md) | Derive a tangent approximation, local gain and time constant. Quantify errors for small and larger departures from the operating point. |
| [43. Gain scheduling](09-industrial-control-structures/56-gain-scheduling/README.md) | Calculate scheduled PI settings from the local tank model. Preserve controller state and compare scheduled and fixed gains across operating levels. |
| [44. Laplace transforms](01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics/README.md) | Transform physical equations, retain initial-condition contributions and connect transfer functions. Solve the simple heater response and present more complicated familiar response expressions. |
| [45. Poles and zeros](02-derivative-action-and-dynamic-response/19-poles-zeros-and-response/README.md) | Define poles and zeros before using them. Explain decay, growth, oscillation, input-response shape and specific cancellation examples through heater and shaft behavior. |
| [46. Characteristic equations](07-classical-stability-and-frequency-design/43-characteristic-equation/README.md) | Derive the equation governing natural modes from plant dynamics and the actual feedback law. Account for feedback signs, controller state and possible cancellation. |
| [47. Root locus](07-classical-stability-and-frequency-design/45-root-locus/README.md) | Vary one gain while holding the other controller and plant properties fixed. Relate pole movement to damping and response speed. |
| [48. Routh–Hurwitz stability](07-classical-stability-and-frequency-design/44-routh-hurwitz/README.md) | Construct and interpret the Routh table. Find a stable gain interval and distinguish stable, marginal and unstable boundary cases. |
| [49. Periodic excitation](05-experiments-identification-and-validation/22-excitation-for-dynamics/README.md) | Choose a bounded sinusoidal input, operating point, frequency, amplitude and observation duration. Exclude startup transients and retain actual applied signals. |
| [50. Frequency response](07-classical-stability-and-frequency-design/46-bode-frequency-response/README.md) | Derive sinusoidal response, introduce complex magnitude and angle, and interpret Bode plots with meaningful units and references. |
| [51. Sensitivity functions](07-classical-stability-and-frequency-design/48-sensitivity-functions/README.md) | Derive sensitivity and complementary sensitivity. Trace disturbances and noise into physical output and controller command, retaining the correct physical input paths. |
| [52. Loop shaping](07-classical-stability-and-frequency-design/49-loop-shaping/README.md) | Derive and implement lead/lag compensation. Assess the complete loop, noise amplification, actuator demand and temperature performance under fixed requirements. |
| [53. Nyquist stability](07-classical-stability-and-frequency-design/47-nyquist-stability/README.md) | Explain the critical point, directed curve, pole count and encirclements. Determine how delivery delay changes stability and interpret slow growth near the boundary. |
| [54. Sampled-data stability](07-classical-stability-and-frequency-design/50-position-z-plane/README.md) | Derive held-command state updates and include the pending command. Explain unit-circle stability and separate physical timing from numerical integration errors. |
| [55. Feedforward control](09-industrial-control-structures/51-feedforward/README.md) | Derive a disturbance-based command from physical balance. Combine it with feedback and assess measurement error, limits and delivery delay. |
| [56. Cascade control](09-industrial-control-structures/52-cascade/README.md) | Explain outer and inner loops, their different output quantities, relative response speeds, initialization and tracking. Assess earlier jacket-disturbance correction. |
| [57. Ratio control](09-industrial-control-structures/53-ratio-control/README.md) | Calculate and regulate an additive-to-carrier ratio. Account for pump lag, invalid measurements, low carrier flow and residual delivery after stopping. |
| [58. Split-range control](09-industrial-control-structures/54-split-range/README.md) | Convert net heat demand into appropriately scaled heater and cooler commands. Explain service exclusion, neutral regions and tracking from applied net heat. |
| [59. Override control](09-industrial-control-structures/55-override-control/README.md) | Select between normal and limiting requests, track inactive controllers and distinguish incompatible requirements from poor tuning. |
| [60. Smith prediction](09-industrial-control-structures/57-smith-predictor/README.md) | Combine delay-free and delayed model predictions with current measurement correction. Assess model mismatch and transfer to ordinary feedback when prediction is unsuitable. |
| [61. Vessel commissioning](14-integrated-capstones/83-reactor-control/README.md) | Select and assess suitable single-loop, cascade, split-range or override arrangements. Keep physical assumptions and requirements fixed across candidates. |
| [62. Multivariable process paths](10-multivariable-control/61-mimo-paths/README.md) | Derive coupled balances and measure each actuator's effect on each output. Introduce the gain matrix through its scalar physical relationships. |
| [63. Relative gain array](10-multivariable-control/62-relative-gain-array/README.md) | Derive isolated and compensated gains. Explain inverse, transpose and elementwise operations through the physical pairing calculation. |
| [64. Decentralized PI](10-multivariable-control/63-decentralized-pi/README.md) | Use separate PI states for paired loops. Assess changed and unchanged outputs under both setpoint directions and disturbance locations. |
| [65. Static decoupling](10-multivariable-control/64-decoupling/README.md) | Derive the virtual-to-physical command map, holding biases and correct tracking through physical limits. Compare steady cancellation and transient costs. |
| [66. Multivariable design selection](10-multivariable-control/65-robust-mimo-selection/README.md) | Compare fixed architectures under unchanged requirements and justified uncertainty cases. Separate feasibility, transient performance and evidence coverage. |
| [67. State-space models](02-derivative-action-and-dynamic-response/20-state-equations-from-stores/README.md) | Derive the two-tank state equations from storage balances. Explain states, inputs, outputs and matrix rows through physical quantities and units. |
| [68. Controllability](11-state-space-estimation-and-predictive-control/66-controllability/README.md) | Explain reachable local transient states and the controllability calculation. Distinguish transient reachability from independently maintainable steady setpoints. |
| [69. Observability](11-state-space-estimation-and-predictive-control/67-observability/README.md) | Derive how downstream level and its change reveal upstream state. Explain why noisy differentiation can be practically unreliable despite theoretical observability. |
| [70. State feedback](11-state-space-estimation-and-predictive-control/68-state-feedback/README.md) | Calculate a pump correction from both level deviations. Derive gains from chosen closed-loop modes and assess actuator limits and local-model validity. |
| [71. State observers](11-state-space-estimation-and-predictive-control/70-state-observer/README.md) | Derive model prediction plus innovation correction. Choose observer gains, preserve update order and assess convergence and noise sensitivity. |
| [72. Kalman filtering](11-state-space-estimation-and-predictive-control/71-kalman-filter/README.md) | Define variance and covariance, derive weighting and perform a complete prediction/correction calculation. Compare estimated error with evaluation truth. |
| [73. LQR control](11-state-space-estimation-and-predictive-control/69-lqr/README.md) | Derive costs from physical scales. Explain one-step minimization, future cost, backward calculation and the resulting finite-horizon feedback gains. |
| [74. LQG control](11-state-space-estimation-and-predictive-control/72-lqg/README.md) | Explain the assumptions behind combining LQR and Kalman filtering. Integrate prediction, correction and control using the previous applied input. |
| [75. Model predictive control](11-state-space-estimation-and-predictive-control/73-mpc/README.md) | Predict over a stated horizon, evaluate explicit costs and constraints, apply the first selected command and replan. Distinguish planning failure from insufficient capacity. |
| [76. Lyapunov stability](12-nonlinear-adaptive-and-robust-control/74-lyapunov-stability/README.md) | Derive an energy-like function for the nonlinear arm, explain its rate of change and establish a region consistent with actuator limits. |
| [77. Feedback linearization](12-nonlinear-adaptive-and-robust-control/75-feedback-linearization/README.md) | Derive nonlinear compensation from physical forces. Distinguish exact model cancellation from local approximation and assess coefficient error, noise and limits. |
| [78. Adaptive control](12-nonlinear-adaptive-and-robust-control/76-adaptive-control/README.md) | Derive parameter estimation, state its information requirements and qualify updates before changing tuning. Compare adapted and fixed settings on the same physical condition. |
| [79. Robust control](12-nonlinear-adaptive-and-robust-control/77-robust-control/README.md) | Define bounded model uncertainty, check feasibility and distinguish robust stability from robust performance. Explain finite tests and range-wide bounds. |
| [80. Networked control](13-networked-fault-aware-and-diagnostic-control/78-networked-control/README.md) | Handle sequence, connection identity, command age, clock assumptions, finite holding, local fallback and deliberate remote recovery. |
| [81. Fault-aware control](13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control/README.md) | Distinguish range, age and disagreement faults. Implement justified sensor fallback, state alignment, fault latching and qualified recovery. |
| [82. Independent design](14-integrated-capstones/84-independent/README.md) | Define requirements, select two suitable designs, calculate feasibility, write their ST integrations and defend the choice using matched performance, uncertainty and recovery evidence. |

## Using the engineering workflow

Use **Monitor** to establish the signals, units and current operating condition. In **Test**, specify the excitation, initial state, duration and stopping conditions before recording the response. Use **Identify** to estimate a model, keeping the estimation record separate from data reserved for validation.

In **Tune**, propose controller settings and predict their consequences. In **Validate**, assess each requirement separately and compare candidates under the same conditions. Keep requested and applied output distinct, and distinguish measured responses from model predictions.

The commissioning assessments consolidate cold-room control (4), heater control
(29), servo control (39) and vessel control (61). Independent design (82) asks
you to compare two suitable solutions for familiar equipment and defend your
recommendation, including uncertainty and recovery.

The [controller-comparison reference](04-controller-selection-and-plant-models/27-controller-selection/README.md) summarizes already-taught actions. Derivative on measurement is taught with [34. PD control](02-derivative-action-and-dynamic-response/33-derivative-motion-damping/README.md); the old separate page is a navigation notice.

The course text defines the intended learning workflow. Alignment and execution verification of the accompanying lesson projects are separate from this Markdown revision.
