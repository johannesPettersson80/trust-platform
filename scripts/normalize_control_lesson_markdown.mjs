#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const controlRoot = path.join(repositoryRoot, "examples", "control");
const manuallyCuratedExperimentLessons = new Set([
  "control-cold-storage-loop-anatomy",
  "control-cold-storage-on-off",
  "control-cold-storage-hysteresis-dwell",
  "control-water-heater-p",
  "control-water-heater-bias",
]);

const diagramMetadata = new Map(Object.entries({
  "60-valve-stiction": {
    file: "valve-stiction-signal-path",
    title: "Valve-stiction control-loop signal path",
    description: "The flow controller requests a stem position, the pneumatic valve and positioner determine the physical stem position, and the installed flow curve produces the measured flow returned to the controller.",
  },
  "62-distillation-decoupling": {
    file: "distillation-decoupling-signal-path",
    title: "Distillation decoupling signal path",
    description: "Two purity errors enter unchanged PI controllers, an architecture selector maps virtual corrections into limited physical inputs, and the coupled column returns both measured purities.",
  },
  "63-distillation-robustness": {
    file: "distillation-robustness-signal-path",
    title: "Distillation robustness architecture path",
    description: "Two product errors pass through fixed PI controllers and the selected direct, static-inverse, or centrally constrained architecture before limited inputs act on the coupled column.",
  },
  "64-two-tank-observability": {
    file: "two-tank-signal-path",
    title: "Two-tank process and measurement path",
    description: "An upstream pump fills tank one, a restriction connects it to tank two, a downstream bypass also enters tank two, and separate transmitters measure both levels.",
  },
  "65-magnetic-bearing-state-feedback": {
    file: "state-feedback-observer-signal-path",
    title: "State-feedback and observer signal paths",
    description: "The magnetic-bearing loop uses measured displacement and velocity for limited current feedback, while the conveyor-motor loop estimates speed and load before applying limited drive torque and comparing the estimate with simulation truth.",
  },
  "66-weigh-feeder-kalman": {
    file: "weigh-feeder-estimation-signal-path",
    title: "Weigh-feeder filtering and Kalman-estimation path",
    description: "The same belt command drives truth and measured feeder copies; noisy load-cell data enter a fixed filter and a predict-correct Kalman estimator whose errors are compared with simulation truth.",
  },
  "67-suspension-lqr": {
    file: "suspension-lqr-lqg-signal-path",
    title: "Active-suspension LQR and LQG signal path",
    description: "Road acceleration and applied hydraulic force drive the suspension body mode; full simulation state feeds LQR while a noisy position sensor feeds a Kalman estimate used by the position-only LQG controller.",
  },
  "68-building-hvac-mpc": {
    file: "building-hvac-mpc-signal-path",
    title: "Building-HVAC predictive-control signal path",
    description: "Weather, occupancy, tariff, and measured zone temperatures enter a three-step optimizer that commands both zone heaters, with bounded PI fallback taking authority when optimization is infeasible or invalid.",
  },
  "69-pendulum-lyapunov": {
    file: "pendulum-lyapunov-signal-path",
    title: "Inverted-pendulum feedback and Lyapunov-certificate path",
    description: "Measured angle and angular rate enter local state feedback, requested torque passes through a two-sided limit into the nonlinear pendulum, and the same states feed the Lyapunov certificate.",
  },
  "75-loop-diagnosis": {
    file: "loop-diagnosis-signal-path",
    title: "Steam-flow loop diagnosis signal path",
    description: "The PI controller requests valve demand, valve mechanics determine applied stem position, header pressure and adjacent demand affect the steam path, and primary plus independent meters support diagnosis.",
  },
  "76-cold-storage": {
    file: "cold-storage-capstone-signal-path",
    title: "Cold-storage capstone signal path",
    description: "The thermostat combines setpoint, measured temperature, quality, and dwell state; an equipment gate uses starter readiness before commanding the compressor and returning running feedback.",
  },
  "77-water-heater": {
    file: "water-heater-capstone-signal-path",
    title: "Water-heater capstone signal path",
    description: "The controller candidate uses setpoint, qualified temperature, and manual or tracking power; requested heater power passes through the physical power stage before the recirculating heater and cold-water load determine measured temperature.",
  },
  "78-servo": {
    file: "servo-capstone-signal-path",
    title: "Servo capstone signal path",
    description: "Payload-angle error and the qualified encoder enter the controller, requested motor torque passes through drive limits into the motor, elastic shaft, and payload, and the payload angle returns to the encoder.",
  },
  "79-reactor": {
    file: "reactor-capstone-signal-path",
    title: "Reactor cascade and protection signal path",
    description: "An outer reactor PI with measured-load feedforward requests jacket temperature, an inner jacket PI and tracked protection selector command mutually exclusive heating or cooling, and both temperatures close the cascade.",
  },
  "80-gravity-tank": {
    file: "gravity-tank-capstone-signal-path",
    title: "Gravity-tank gain-scheduling comparison path",
    description: "A quality and envelope check feeds fixed and scheduled controller paths, their requested flows are independently limited, and equivalent nonlinear tanks return measured levels for comparison.",
  },
  "81-conveyor-oven": {
    file: "conveyor-oven-capstone-signal-path",
    title: "Conveyor-oven predictive-control signal path",
    description: "Setpoint, measured-load feedforward, model correction, and predicted current temperature enter the PI controller; requested heater power passes through limits before the delayed physical transport path returns downstream temperature.",
  },
  "82-distillation-column": {
    file: "distillation-column-capstone-signal-path",
    title: "Distillation-column capstone architecture path",
    description: "Two product errors enter fixed PI controllers and the selected direct, static-inverse, or centrally coordinated architecture before limited inputs act on the coupled column and return two product measurements.",
  },
}));

const correctedNeeds = new Map(Object.entries({
  "control-cold-storage-loop-anatomy": "We need a clear map of the cold room, compressor, temperature sensor, desired temperature, measured temperature, heat loads, and safe command. We also need one bounded manual run that shows which way temperature moves and whether the response is immediate or delayed.",
  "control-cold-storage-on-off": "We need the measured room temperature to switch the binary compressor automatically. The rule must say exactly when cooling is on, when it is off, and what happens at the threshold so that its limitations can be measured rather than hidden.",
  "control-cold-storage-hysteresis-dwell": "We need to stop measurement noise from toggling the compressor at one threshold and prevent rapid reversals that damage real equipment. The room must still remain inside its temperature requirement while the compressor receives enough off-time and run-time protection.",
  "control-water-heater-p": "We need a continuously variable heater command that becomes stronger as the measured water moves farther from its desired temperature. We must then find how much response speed can be gained before overshoot, oscillation, or actuator effort becomes unacceptable.",
  "control-water-heater-bias": "We need the controller to supply the heater power required to hold the normal operating temperature even when temperature error is zero. That holding power must come from a bounded measurement, not from an unexplained guessed constant.",
  "control-water-heater-integral": "We need the controller to remember persistent temperature error and gradually change heater power until a constant load no longer leaves an offset. We must isolate that memory effect so its slower initial response is visible as well as its benefit.",
  "control-water-heater-pi": "We need one controller that reacts promptly to a new temperature error and also removes the remaining constant-load error. The immediate and accumulated contributions must stay visible so integral gain can be varied without changing the proven proportional action.",
  "control-rate-limit": "We need the command applied to the heating valve to respect how far the actuator can physically travel during one controller interval. The requested and applied positions must remain separate so a slow response is not misdiagnosed as poor controller tuning.",
  "control-saturation-antiwindup": "We need the controller's stored demand to respond when the steam valve cannot deliver the requested position. Several correction rules may solve that problem differently, so they must be compared under the same limit and recovery event.",
  "control-reactor-servo-regulatory": "We need separate evidence for changing the desired reactor temperature and for rejecting an independent heat-load change. One unchanged controller may be acceptable for one objective and unacceptable for the other, so neither result may stand in for both.",
  "control-reactor-setpoint-weighting": "We need to soften only the controller's immediate response to a new reactor-temperature request while preserving its response to measured temperature and its accumulated correction. The setting must be compared from fresh identical runs because it is adopted during initialization.",
  "control-measurement-noise": "We need to reduce valve movement caused by transmitter noise without adding so much measurement delay that real pressure changes are hidden. The weakest useful smoothing must therefore be selected from both noise reduction and lag evidence.",
  "control-sampling-interval": "We need every time-dependent controller calculation to use the elapsed time that actually occurred, not an assumed perfect interval. We also need a bounded timing envelope and a safe response when a scan arrives too late to trust.",
  "control-water-heater-energy-balance": "We need a model whose temperature change follows from stored thermal energy, heater input, and heat loss in the named equipment. Its units, assumptions, and operating range must make a numerical prediction possible before the simulator is run.",
  "control-heater-step-identification": "We need a bounded input change that reveals the installed heater's local response from measured input and output samples. The data must be qualified before parameters are estimated, and the resulting model must remain only a candidate until new data test it.",
  "control-independent-heater-model-validation": "We need to challenge the frozen heater model with data that were not used to estimate it. Command-response accuracy and load-response accuracy must be judged separately so passing one purpose cannot silently approve the other.",
  "control-nominal-fopdt": "We need to turn the accepted local model and a chosen response target into reproducible controller settings. The resulting candidate must stay unapplied until common nominal and uncertain cases show the speed-versus-robustness trade-off.",
  "control-servo-position-p": "We need motor torque to increase with position error so the table moves toward its target with controllable stiffness. Several gains must be compared because the fastest response is not useful if it creates excessive overshoot, oscillation, or torque.",
  "control-servo-position-pd": "We need a torque contribution that opposes measured motion so damping can increase without reducing the positional restoring stiffness. The comparison must hold the proportional setting fixed to isolate that effect.",
  "control-loaded-position-pid": "We need the positioning table to create the steady motor torque required to oppose a constant payload load without retaining position error. That stored correction must be added without losing the damping already established by motion feedback.",
  "control-servo-derivative-kick": "We need the motion-damping term to react to physical movement without treating an instantaneous setpoint change as infinite motion. Requested and applied torque must remain separate so drive saturation cannot hide the command kick.",
  "control-servo-filtered-derivative": "We need to attenuate encoder noise before it becomes alternating motor torque while retaining enough real motion information for damping. Filter strength and number of stages must be selected from both torque-noise and lag evidence.",
  "control-servo-mechanical-nonidealities": "We need synchronized evidence that separates intentional trajectory shaping, drive limits, friction, deadband, backlash, and stiction. Only then can the engineer decide whether the remedy belongs in the controller, command path, or mechanism.",
  "control-servo-digital-realization": "We need to specify the actual sampled recurrence, held torque, elapsed interval, numerical precision, quantization, and computation delay that the controller executes. A continuous equation alone cannot establish which digital implementation remains credible.",
  "control-pid-forms": "We need the controller to choose a recurrence that behaves correctly both with and without stored integral action. The learner must see why an unchanged-error proportional controller cannot rely on an output-increment equation that produces no new increment.",
  "control-booster-pump-modes": "We need every retained controller and filter history to match the already-running pump before automatic authority is allowed. Disabled, Manual, Auto, and Track must each have one explicit owner of the applied valve command.",
  "control-booster-pump-bumpless-transfer": "We need an inactive controller to follow the valve position applied by another authority so its internal output is current when control returns. The transfer must be judged from the actual command step, including cases where exact matching is physically impossible.",
  "control-booster-pump-atomic-update": "We need each scan to use one complete, coherent parameter revision and one constraint-aware path for every output contribution. A half-applied configuration or a feedforward sum added after limits must be impossible.",
  "control-booster-pump-invalid-data": "We need invalid measurements and invalid configuration values to be rejected before they can update retained controller state. Recovery must require a known safe output, acknowledgement, qualified data, reinitialization, and renewed authority.",
  "control-controller-selection": "We need to choose the simplest controller whose physical actuation and dynamic mechanisms can satisfy each loop's requirements. Eligibility must come from executable evidence rather than a habit of applying the most complex controller everywhere.",
  "control-plant-response-classes": "We need a bounded observation that distinguishes equilibrium, continuing drift, delay, and oscillation before any model is fitted. The record must last long enough that a slow or delayed process is not mistaken for a simple low-gain response.",
  "control-tank-heater-conservation": "We need the tank-level and heater-temperature responses to follow directly from material and energy balances. Each term must have a physical meaning and compatible unit so the model predicts more than the shape of one curve.",
  "control-gravity-tank-linearization": "We need a simple local relationship between pump-flow changes and level changes around a declared operating point. Its valid range must be measured because gravity outflow changes with level and one local approximation cannot represent the complete tank.",
  "control-pole-zero-plant-classes": "We need a way to connect stored modes and input-output cancellations to response features an engineer can actually observe. Each qualitative prediction must be made before the executable response is revealed.",
  "control-two-tank-state-space": "We need both stored water levels to remain explicit because the same current downstream measurement can hide different upstream inventories and future responses. The model must advance those internal states and map them to the measured outputs.",
  "control-safe-heat-exchanger-experiment": "We need an experiment plan that is informative, bounded, reviewable, abortable, and restorable before the steam valve is allowed to move. A rejected plan must leave the valve unchanged, and an invalid run must not become identification evidence.",
  "control-excitation-methods": "We need to choose an input shape whose duration and frequency content expose the dynamics relevant to the intended model without leaving the safe operating envelope. A slow thermal process and a resonant mechanical process require different evidence.",
  "control-dryer-model-selection": "We need to compare several physically plausible model structures using both estimation evidence and a separate validation record. Complexity is justified only when it improves independent prediction and leaves acceptable residual behavior.",
  "control-poor-excitation": "We need a qualification rule that can distinguish a merely long recording from one whose input actually reveals the heat exchanger's response. Weak open-loop, bounded open-loop, and ordinary feedback records must be judged by the same information and safety criteria.",
  "control-independent-model-validation": "We need independent tests that show where one frozen heat-exchanger model predicts well and where its purpose or operating range must be restricted. Amplitude, disturbance, and operating-point changes must receive separate dispositions instead of one blanket pass.",
  "control-manual-historical-tuning": "We need a disciplined way to change proportional and integral action one term at a time, then compare the result with a historical critical-cycle rule under identical conditions. The least aggressive eligible setting must remain unapplied to physical equipment.",
  "control-relay-autotuning": "We need a bounded feedback experiment that can create a usable oscillation without confusing any oscillation with valid tuning evidence. The record must be rejected when amplitude, period, limits, or signal quality are not informative enough.",
  "control-model-based-tuning": "We need to convert each eligible process model into several explicit speed-versus-robustness choices and validate them under common conditions. A calculated candidate remains unapplied until its model range, actuator use, and uncertain response are acceptable.",
  "control-crane-routh-stability": "We need to determine the complete trolley-gain interval that keeps every closed-loop mode stable before risking an executable crane response. Boundary and outside-range cases must then confirm what the calculation predicts.",
  "control-servo-root-locus": "We need more freedom than proportional gain alone provides to place the positioning carriage's dominant response while keeping the remaining modes acceptable. Desired pole location, added dynamics, and executable response must tell the same story.",
  "control-flexible-drive-bode": "We need to measure how the flexible shaft responds across frequency, locate its resonance, and judge stability margins before shaping the loop. Any added shaping must reduce the resonant risk without hiding poor excitation or violating time-domain requirements.",
  "control-delayed-thermal-nyquist": "We need a stability test that accounts for the conveyor oven's transport delay and the complete frequency contour, including cases that a partial plot can misclassify. Predicted encirclements must agree with stable and unstable executable runs.",
  "control-motor-sensitivity-loop-shaping": "We need the motor-speed loop to reject low-frequency load changes, avoid amplifying high-frequency noise, and retain adequate stability margins at the same time. Competing loop shapes must be compared against those common frequency- and time-domain requirements.",
  "control-position-z-plane-sampling": "We need the positioning controller's poles and zeros to reflect the interval at which the computer actually samples and holds torque. Unchanged continuous settings at a slower interval must be rejected when the discrete response no longer meets stability and tracking requirements.",
  "control-tuning-method-comparison": "We need to compare tuning rules only on plant classes for which their assumptions are eligible and then run every real candidate under the same requirements. The result may be one choice, several explicit trade-offs, or no acceptable candidate.",
  "control-automatic-tuning-sequence": "We need a visible, recoverable sequence that qualifies data, accepts a model, generates a candidate, validates it, and stops honestly when any prerequisite fails. Completion must produce an unapplied recommendation rather than silently changing the running heater.",
  "control-conveyor-oven-feedforward": "We need the measured conveyor-speed change to influence heater demand before downstream temperature error appears, while feedback still corrects model error and unmeasured loads. Early action is useful only if it remains inside controller and actuator constraints.",
  "control-reactor-cascade": "We need the jacket-temperature loop to reject utility disturbances before they propagate into reactor product temperature. The inner loop must be commissioned and proven sufficiently faster before the slower outer loop is allowed to command it.",
  "control-blending-ratio": "We need additive flow to follow valid production flow immediately while downstream composition feedback supplies only the slower trim. Ratio authority must be removed safely when the wild-flow measurement is invalid or below the controllable range.",
  "control-reactor-split-range": "We need one signed reactor-temperature demand to command heating on one side of neutral and cooling on the other without wasting opposing utilities. Crossover dead zone, overlap, and installed valve gain must be compared on equivalent evidence.",
  "control-compressor-override": "We need minimum-flow protection to take valve authority whenever the production controller would drive the compressor toward surge, then return authority without a bump or chatter. Inactive controllers must track the applied valve position throughout the takeover.",
  "control-gravity-tank-scheduling": "We need controller settings that remain appropriate as gravity outflow changes across the tank's operating range. Any change between local settings must preserve output continuity and avoid boundary chatter, and its added complexity must outperform one fixed setting.",
  "control-heated-air-duct-smith": "We need the controller to act on a useful estimate of current oven temperature instead of waiting only for the delayed downstream measurement. That benefit must survive declared model mismatch and fall back safely when the prediction is not trustworthy.",
  "control-valve-stiction": "We need evidence that separates controller demand, positioner behavior, physical stem motion, and installed flow so deadband and mechanical sticking are not mistaken for poor tuning. The resulting decision must identify when maintenance, rather than gain adjustment, is justified.",
  "control-distillation-mimo": "We need to measure how each distillation input affects both product purities and use those four gains to choose which input should control which output. The calculated pairing must agree with the observed interaction before any loop is designed.",
  "control-distillation-decoupling": "We need to reduce cross-coupling between the two commissioned purity loops without creating a fragile or unrealizable correction. Direct, steady inverse, and dynamic alternatives must face the same nominal and model-mismatch events.",
  "control-distillation-robustness": "We need both purity controllers to respect the shared physical direction and input limits when their separate requests cannot be achieved together. The architecture must remain defensible under interaction, uncertainty, and an explicitly infeasible production target.",
  "control-two-tank-observability": "We need to know whether the installed pumps can move every important stored-water direction and whether the transmitters can reveal every state needed for control. Normal and deliberately deficient layouts must be accepted or rejected before controller design.",
  "control-magnetic-bearing-state-feedback": "We need feedback from the bearing's dynamic state and a separate estimate of conveyor speed and load where direct measurement is unavailable. Measured state, estimated state, limits, and model-mismatch bias must remain visibly distinct.",
  "control-weigh-feeder-kalman": "We need an estimate that combines the feeder model with noisy load-cell data according to declared uncertainty instead of choosing whichever trace looks smoothest. Innovation and truth-error evidence must reveal overtrust, distrust, sensor bias, and invalid covariance.",
  "control-suspension-lqr": "We need to turn explicit priorities for body motion, suspension travel, and hydraulic effort into comparable feedback designs. A position-only estimate must then preserve the balanced design under declared sensor noise and faults without being called universally optimal.",
  "control-building-hvac-mpc": "We need the two office zones to share limited heating power while considering predicted weather, occupancy, tariff, and future temperature constraints. The online search must distinguish a feasible optimum from infeasible or invalid cases and transfer safely to bounded fallback.",
  "control-pendulum-lyapunov": "We need a mathematical certificate that proves local upright stability for declared pendulum states and a separate simulation record outside that certified region. Successful trajectories must not be promoted into a global proof.",
  "control-robot-joint-nonlinear": "We need joint control that remains credible as gravity torque and payload change across a wide angular range. Fixed, operating-point-adjusted, and model-cancelling alternatives must be compared with the same tracking, torque, constraint, and model-trust evidence.",
  "control-extruder-adaptive": "We need die-pressure control that can use known recipe changes and cautiously learn an unmeasured material drift without adapting on bad or uninformative data. Loss of excitation or pressure quality must freeze learning and preserve safe authority.",
  "control-flexible-airframe-robust": "We need one load-alleviation controller whose worst-case evidence remains acceptable across the declared flexible-wing model set, not merely at the nominal plant. The claim must stop at the tested uncertainty and cannot stand in for flight or certification evidence.",
  "control-networked-pumping": "We need remote pump commands to own the station only while their age, order, and communication delay remain inside a calculated reserve. Invalid packets must be rejected and authority must transfer to local pressure control without an unsafe command jump.",
  "control-pumping-fault-aware": "We need the pumping station to distinguish supported sensor and actuator faults, continue only in a justified degraded mode, and enter a safe state when independent evidence is insufficient. Automatic authority must return through an explicit qualified recovery sequence.",
  "control-loop-diagnosis": "We need to match each abnormal steam-flow record to the next evidence-producing action instead of treating retuning as the default cure. Valve, instrument, capacity, disturbance, interaction, model, and schedule causes must remain separate until the record supports one.",
  "control-capstone-cold-storage": "We need the simplest discrete compressor strategy that keeps qualified room temperature inside the product band without short-cycling. Candidate rejection, abnormal operation, restoration, and the physical-evidence boundary must all be explicit before commissioning is recommended.",
  "control-capstone-water-heater": "We need one traceable path from heater requirements and physical balance through identification, independent validation, controller choice, tuning, and abnormal tests. The final recommendation must remain unapplied until approval and rollback are ready.",
  "control-capstone-servo": "We need a payload-position controller justified from mechanical requirements, dynamic analysis, exact digital execution, actuator limits, and encoder noise. Simpler structures and unqualified sample intervals must be rejected by the same declared evidence.",
  "control-capstone-reactor": "We need one maintainable authority chain for reactor production, feed-load compensation, protection takeover, degradation, Manual operation, and controlled shutdown. Every applied valve movement must be traceable to the measurement, mode, and owner that requested it.",
  "control-capstone-gravity-tank": "We need to prove where one fixed controller becomes inadequate and whether operating-point-dependent settings improve the complete tank range enough to justify their validity and maintenance burden. Transitions, faults, approval, and rollback must remain explicit.",
  "control-capstone-conveyor-oven": "We need to add complexity to the delayed oven only when ordinary feedback cannot meet a declared requirement. Identification, prediction, measured-load compensation, delay compensation, mismatch, fallback, and heater authority must form one qualified chain.",
  "control-capstone-distillation": "We need to progress from measured four-path interaction to pairing and then to the simplest architecture that handles coupling and shared constraints. An infeasible purity target must be rejected rather than hidden by a more complex controller.",
  "control-capstone-building-hvac": "We need the simplest controller that keeps both coupled office zones comfortable within the shared heating limit and lifecycle constraints. Predictive control is justified only if qualified evidence improves on strong feedback enough to repay its added movement and maintenance cost.",
  "control-capstone-independent": "We need a repeatable engineering process that begins with equipment purpose and measurable requirements, gathers only the model evidence the decision needs, rejects inapplicable structures, and defends the simplest eligible recommendation. The learner, not the lesson title, must decide which supported controller family wins.",
}));

// These details were present in the lesson-specific procedures before the
// course-wide Lesson 8 workflow was adopted. Keep them inside the canonical
// action bullets so normalization cannot trade teaching content for structure.
const workflowPedagogy = new Map(Object.entries({
  "control-cold-storage-on-off": { stage: "Test", label: "Recovery", text: "At the end of the run, disable Auto, confirm 0 percent compressor demand, and retain that safe restoration state." },
  "control-cold-storage-hysteresis-dwell": { stage: "Test", label: "Baseline check", text: "Compare candidates on the same plant, initial state, door event, sample interval, duration, and seed." },
  "control-water-heater-bias": { stage: "Test", label: "Recovery", text: "Return to Manual mode and confirm zero percent requested and applied output as the restoration state." },
  "control-water-heater-integral": { stage: "Test", label: "Recovery", text: "Begin each run in the declared Manual holding condition and finish each run in Manual with zero requested and applied output as the restoration state." },
  "control-water-heater-pi": { stage: "Test", label: "Baseline check", previousText: "Begin in Manual at 75 percent and wait until the outlet temperature is settled before transferring to Auto. A gain edit is not a case reset: restore the Manual holding condition before beginning the next candidate, and finish every run at 120.1 seconds in Manual with zero requested and applied output.", text: "Begin in Manual at 75 percent and wait until the outlet temperature is settled before transferring to Auto. A gain edit is not a case reset: restore the Manual holding condition before beginning the next candidate, and finish every run at 120.1 seconds in Manual with zero requested and applied output. Treat the separate 45 degrees Celsius exposure as a diagnostic run, never as another tuning candidate." },
  "control-water-heater-saturation": { stage: "Test", label: "Baseline check", text: "Start in Manual at zero output, command 75 percent, and wait for 35 degrees Celsius; transfer to Auto, step the setpoint to 45 degrees Celsius for 10 seconds, restore it to 35 degrees Celsius, then return to Manual at zero output." },
  "control-rate-limit": { stage: "Test", label: "Baseline check", text: "Start with SP = 0 and a closed valve, step the flow request to 10 litres per minute for 15 seconds, restore SP = 0 without resetting controller state, then return to Manual at zero output." },
  "control-saturation-antiwindup": { stage: "Test", label: "Baseline check", previousText: "Retain the initial Manual sample with SP = 0 and zero trim at 4.5 degrees Celsius, enter Auto, request a 1.0 degree Celsius change through 20.0 seconds, then restore SP = 0 without resetting controller state.", text: "Retain the initial Manual sample with SP = 0 and zero trim at 4.5 degrees Celsius, enter Auto, request a 1.0 degree Celsius change through 20.0 seconds, then restore SP = 0 without resetting controller state. Continue the automatic recovery observation through 50.0 seconds, select Manual zero at 50.1 seconds, and include the initial Manual sample, final Manual sample, and Auto-to-Manual transition in complete-run travel." },
  "control-reactor-servo-regulatory": { stage: "Test", label: "Baseline check", previousText: "For the servo run, retain the initial Manual sample through 5.0 seconds, enter Auto at 5.1 seconds, return to Manual zero at 55.0 seconds, and from 55.1 seconds rate the output down until it reaches zero. Use another fresh simulation for the regulatory run.", text: "For the servo run, retain the initial Manual sample through 5.0 seconds, enter Auto at 5.1 seconds, return to Manual zero at 55.0 seconds, and from 55.1 seconds rate the output down until it reaches zero. Use another fresh simulation for the regulatory run: retain the 60.0-second boundary, apply the load at 60.1 seconds, observe through 120.0 seconds, select Manual zero at 120.1 seconds, and confirm restoration to zero." },
  "control-reactor-setpoint-weighting": { stage: "Test", label: "Baseline check", previousText: "Execute six independently initialized runs: three weights for the servo event and the same three weights for the regulatory event. In every servo run retain the 5.0 seconds, 5.1 seconds, 55.0 seconds, and 55.1 seconds boundary samples.", text: "Execute six independently initialized runs: three weights for the servo event and the same three weights for the regulatory event. In every servo run retain the 5.0 seconds, 5.1 seconds, 55.0 seconds, and 55.1 seconds boundary samples. In every regulatory run retain the 60.0-second boundary, 60.1 seconds, 120.0 seconds, and 120.1 seconds boundary samples." },
  "control-measurement-noise": { stage: "Test", label: "Baseline check", previousText: "Execute three fresh runs with the same seed and noise sequence, PI settings, timing, and limits; change only the filter selection.", text: "Execute three fresh runs with the same seed and noise sequence, PI settings, timing, and limits; change only the filter selection. Retain the 5.0 seconds, 5.1 seconds, 40.0 seconds, and 40.1 seconds boundary samples, then confirm Manual zero." },
  "control-sampling-interval": { stage: "Test", label: "Baseline check", previousText: "Start every timing case fresh with the same plant, gains, setpoint, belt speed, duration, limits, and seed.", text: "Start every timing case fresh with the same plant, gains, setpoint, belt speed, duration, limits, and seed. These are Simulation evidence, not physical task period measurements." },
  "control-water-heater-energy-balance": { stage: "Monitor", label: "Decision before action", previousText: "Until the hand prediction and unit check are complete, Monitor, Test, Validate, and Identify remain closed. The declared observation has a 30.00-second duration and a 100-millisecond sample interval.", text: "Until the hand prediction and unit check are complete, Monitor, Test, Validate, and Identify remain closed. The declared observation has a 30.00-second duration and a 100-millisecond sample interval. At 30.10 seconds retain the safe-restoration sample with 0.00 percent requested and 0.00 percent applied heater output." },
  "control-heater-step-identification": { stage: "Identify", label: "Baseline check", previousText: "Qualify exactly 500 active samples at a 0.10-second interval over 50.00 seconds before fitting. At 50.10 seconds, return to Manual restoration at 20.00 percent heater output. Preserve the declared 0.15 seconds gap between the estimation and validation records. Reject the record if baseline drift exceeds 0.001 degrees Celsius per second, input span is below 9.99 percentage points, output span is below 1.90 degrees Celsius, or signal-to-noise ratio is below 20 decibels. Resolve dead time only when it persists for two sample intervals and crosses the 2 percent lower threshold; that dead-time rule overrides the least-complex preference. Treat the resulting parameter bounds as a sample-resolution envelope, not statistical confidence. The model remains validation required and Tune stays closed.", text: "Qualify exactly 500 active samples at a 0.10-second interval over 50.00 seconds before fitting. At 50.10 seconds, return to Manual restoration at 20.00 percent heater output. Preserve the declared 0.15 seconds gap between the estimation and validation records. Reject the record if baseline drift exceeds 0.001 degrees Celsius per second, input span is below 9.99 percentage points, output span is below 1.90 degrees Celsius, or signal-to-noise ratio is below 20 decibels. Resolve dead time only when it persists for two sample intervals and crosses the 2 percent lower threshold; that dead-time rule overrides the least-complex preference. Treat the resulting parameter bounds as a sample-resolution envelope, not statistical confidence. The model remains validation required and Tune stays closed. Repeating this experiment on a physical heater requires a separately reviewed plan and explicit approval." },
  "control-independent-heater-model-validation": { stage: "Test", label: "Recovery", previousText: "At 60.10 seconds return to Manual and confirm 20.00 percent heater output before retaining the restoration record. Start the comparison at 30.2 degrees Celsius and 20 percent heater power, request 30.6 degrees Celsius, then apply the declared 29.8 degrees Celsius cold-flow condition.", text: "Start the comparison at 30.2 degrees Celsius and 20 percent heater power, request 30.6 degrees Celsius, then apply the declared 29.8 degrees Celsius cold-flow condition. At 60.10 seconds return to Manual and confirm 20.00 percent heater output before retaining the restoration record." },
  "control-nominal-fopdt": { stage: "Test", label: "Decision before action", text: "Start at 30.2 degrees Celsius and 20 percent heater power, request 30.6 degrees Celsius, and then apply the 29.8 degrees Celsius cold-flow condition." },
  "control-servo-mechanical-nonidealities": { stage: "Test", label: "Baseline check", text: "Run six one-effect cases from a fresh ideal baseline before the checked-in combined case." },
  "control-booster-pump-modes": { stage: "Test", label: "Baseline check", text: "Retain the zero-state startup comparison: it produces a 10.000 percent command step and a 0.198 bar pressure deviation before measured-state initialization is enabled." },
  "control-plant-response-classes": { stage: "Test", label: "Decision before action", text: "Keep the plant identities hidden until every response has been classified from evidence; reveal them only in the checked answer below." },
  "control-tank-heater-conservation": { stage: "Test", label: "Decision before action", text: "Use this run only to check the conservation prediction; do not Identify, Tune, or Apply." },
  "control-gravity-tank-linearization": { stage: "Test", label: "Decision before action", text: "Use this run only to test the declared local approximation; do not Identify, Tune, or Apply." },
  "control-pole-zero-plant-classes": { stage: "Test", label: "Recovery", text: "Run each response in a separate restored plant copy, keep the unstable bearing case abort-bounded, and never reuse its terminal state. These are response-class experiments; do not Identify, Tune, or Apply." },
  "control-two-tank-state-space": { stage: "Test", label: "Decision before action", text: "Use the run to expose hidden state only; do not Identify, Tune, or Apply." },
  "control-safe-heat-exchanger-experiment": { stage: "Test", label: "Decision before action", text: "Execute only the approved bounded plan; do not use Identify, Tune, or Apply." },
  "control-excitation-methods": { stage: "Test", label: "Decision before action", text: "Compare the declared excitation records only; do not Identify, Tune, or Apply." },
  "control-dryer-model-selection": { stage: "Identify", label: "Baseline check", text: "Compare every candidate on the same estimation window, the same weighting, and the same sample inclusion rule. Use independent validation with the declared 5-percentage-point pulse before selecting a structure. Do not Tune or Apply from the estimation fit." },
  "control-poor-excitation": { stage: "Identify", label: "Baseline check", text: "Retain requested and applied input separately and use the controller-used measurement when deciding whether feedback has removed independent excitation." },
  "control-independent-model-validation": { stage: "Test", label: "Baseline check", text: "Use a common 10-second baseline, apply the cold-flow load at 90 seconds, observe through 160 seconds, and compare only against the frozen model. Do not Tune or Apply from validation evidence." },
  "control-manual-historical-tuning": { stage: "Test", label: "Baseline check", text: "Use the common 10-second baseline, introduce the 100-second load event, and retain the complete 180 seconds for every candidate. Do not run this comparison on a physical plant or select Apply from simulation evidence." },
  "control-relay-autotuning": { stage: "Test", label: "Recovery", text: "Do not run this relay experiment on a physical plant or select Apply from simulation evidence." },
  "control-model-based-tuning": { stage: "Test", label: "Baseline check", text: "Change only the response target; keep the same eligible plant, initial state, events, sample interval, and limits for Fast, Balanced, and Robust candidates." },
  "control-automatic-tuning-sequence": { stage: "Identify", label: "Recovery", text: "Remove the selected saved plant, provide a negative gain candidate, return to Identify, and confirm Request excitation is rejected with the displayed reason. Then inject poor data, confirm tuning cannot proceed to Validate, and retain the terminal message No acceptable controller found. Exercise user abort, automatic abort, and restoration failure as distinct terminal states." },
  "control-conveyor-oven-feedforward": { stage: "Monitor", label: "Baseline check", text: "In Control Studio Monitor retain the pre-event state through 30.0 s and the restored terminal state at 90.0 s." },
  "control-reactor-cascade": { stage: "Monitor", label: "Baseline check", text: "In Control Studio Monitor select commissioning mode, retain the inner-loop boundary at 10.0 s, and retain the outer-loop boundary at 40.0 s." },
  "control-blending-ratio": { stage: "Monitor", label: "Baseline check", text: "In Control Studio Monitor retain the normal-flow boundary at 30.0 s and the restored boundary at 90.0 s." },
  "control-reactor-split-range": { stage: "Monitor", label: "Baseline check", text: "In Control Studio Monitor retain the actuator-map boundaries at 20.0 s, 80.0 s, 140.0 s, and 200.0 s." },
  "control-compressor-override": { stage: "Monitor", label: "Baseline check", text: "In Control Studio Monitor retain the production, override, and restored authority boundaries at 20.0 s, 80.0 s, and 140.0 s." },
  "control-gravity-tank-scheduling": { stage: "Monitor", label: "Baseline check", text: "Use Control Studio Monitor before Identify and Tune, and retain the schedule boundaries at 30 s, 120 s, 240 s, 330 s, and 420 s." },
  "control-heated-air-duct-smith": { stage: "Monitor", label: "Baseline check", text: "Use Control Studio Monitor before Identify and Tune; retain 10 s at the 35 degC baseline, then the 70 s event and the 180 s restoration boundary." },
  "control-valve-stiction": { stage: "Test", label: "Record", text: "This is Simulation evidence; a diagnosis on a physical valve requires installed position and flow evidence." },
  "control-distillation-mimo": { stage: "Monitor", label: "Baseline check", previousText: "Control Studio Monitor must show Simulation mode, Manual authority, and the declared 0.1-second sample interval before either input step. Retain a zero-input baseline for 20 seconds. Change only reflux in one fresh run, restore it, then in a new run change only reboiler steam.", text: "Control Studio Monitor must show Simulation mode, Manual authority, and the declared 0.1-second sample interval before either input step. Retain a zero-input baseline for 20 seconds. Change only reflux and hold it for 180 seconds; restore it, then in a new run change only reboiler steam for 180 seconds. Use the final 10 seconds only when each composition varies by no more than 0.02 composition percentage points. If an input-output path is missing or invalid, report the gain and pairing as Unconfirmed. This is simulation only; physical-column tests require explicit approval." },
  "control-distillation-decoupling": { stage: "Monitor", label: "Baseline check", text: "Control Studio Monitor must show Simulation mode and Auto authority, with requested and applied inputs visible separately. Execute the overhead-only event from 0 to +2 and restore it before starting the bottoms-only event. Repeat both events under the feed-composition disturbance and the perturbed interaction model." },
  "control-distillation-robustness": { stage: "Monitor", label: "Baseline check", text: "Control Studio Monitor must show Simulation mode with requested and applied input vectors visible together. Retain overhead-only, bottoms-only, simultaneous, and feed-composition cases as separate comparisons." },
  "control-two-tank-observability": { stage: "Monitor", label: "Baseline check", text: "Control Studio Monitor must show Simulation mode, Manual authority, both actuator paths, and both sensor paths before feasibility is judged." },
  "control-weigh-feeder-kalman": { stage: "Monitor", label: "Baseline check", text: "Use Control Studio Monitor to establish the innovation record, then use Identify before Test so uncertainty choices precede the comparison." },
  "control-suspension-lqr": { stage: "Monitor", label: "Baseline check", text: "Use Control Studio Monitor before Tune/Design, then execute Test and finish in Validate on common physical measures." },
  "control-building-hvac-mpc": { stage: "Monitor", label: "Baseline check", text: "Use Control Studio Monitor before Identify and Tune/Design, then execute Test and finish in Validate." },
  "control-loop-diagnosis": { stage: "Monitor", label: "Baseline check", text: "The industrial steam-distribution area record must show steam-flow setpoint in t/h and valve-stem position in percent before diagnosis." },
  "control-capstone-cold-storage": { stage: "Test", label: "Decision before action", text: "For the walk-in cold-storage room, select the simplest discrete controller that keeps temperature inside the product band without compressor short-cycling." },
  "control-capstone-servo": { stage: "Test", label: "Baseline check", text: "Execute the heavy payload, reduced damping, doubled encoder noise, Manual transfer, invalid quality, and controlled stop cases as separate restored abnormalities. Keep the selected revision unapplied until explicit approval, with rollback ready." },
  "control-capstone-gravity-tank": { stage: "Test", label: "Baseline check", text: "Retain the complete operating-envelope boundaries at 30 seconds, 120 seconds, 240 seconds, 330 seconds, and 420 seconds." },
  "control-capstone-conveyor-oven": { stage: "Test", label: "Baseline check", text: "Retain the delay and mismatch boundaries at 10 seconds, 54 seconds, 70 seconds, and 180 seconds. Separately exercise invalid upstream quality, invalid downstream quality, incomplete history, and heater unavailable; verify Manual fallback and ControlledStop." },
  "control-capstone-distillation": { stage: "Test", label: "Recovery", text: "Execute invalid model, invalid composition, and unavailable valve cases, then prove restoration and rollback without applying the candidate." },
  "control-capstone-building-hvac": { stage: "Test", label: "Recovery", text: "Exercise invalid data, stale data, unbounded innovation, and an unavailable heater; verify Manual fallback and ControlledStop." },
  "control-capstone-independent": { stage: "Test", label: "Decision before action", text: "Apply the same evidence process to a learner-selected, platform-supported plant only after defending the shipped reference compressed-air receiver." },
}));

function appendWorkflowPedagogy(actions, lessonId) {
  const addition = workflowPedagogy.get(lessonId);
  if (!addition || actions.includes(addition.text)) return actions;
  if (addition.previousText && actions.includes(addition.previousText)) {
    return actions.replace(addition.previousText, addition.text);
  }
  const escapedStage = addition.stage.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const escapedLabel = addition.label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const stagePattern = new RegExp(`(^\\d+\\. \\*\\*${escapedStage}\\*\\*[\\s\\S]*?)(?=^\\d+\\. \\*\\*|(?![\\s\\S]))`, "m");
  const stageMatch = actions.match(stagePattern);
  if (!stageMatch) throw new Error(`${lessonId} is missing the ${addition.stage} workflow stage`);
  const bulletPattern = new RegExp(`(^   - \\*\\*${escapedLabel}:\\*\\*[^\\n]*)`, "m");
  if (!bulletPattern.test(stageMatch[1])) {
    throw new Error(`${lessonId} is missing ${addition.label} in ${addition.stage}`);
  }
  const updatedStage = stageMatch[1].replace(bulletPattern, `$1 ${addition.text}`);
  return actions.replace(stageMatch[1], updatedStage);
}

function lessonDirectories(directory, result = []) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const candidate = path.join(directory, entry.name);
    if (fs.existsSync(path.join(candidate, "lesson.json"))) result.push(candidate);
    else lessonDirectories(candidate, result);
  }
  return result;
}

function dotEscape(value) {
  return value.replaceAll("<br/>", "\\n").replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

function parseNode(token, nodes) {
  const match = token.trim().match(/^([A-Za-z0-9_]+)(?:\[([\s\S]*?)\]|\{([\s\S]*?)\})?$/);
  if (!match) throw new Error(`Unsupported Mermaid node: ${token}`);
  const [, id, boxLabel, decisionLabel] = match;
  if (boxLabel !== undefined || decisionLabel !== undefined) {
    nodes.set(id, {
      label: boxLabel ?? decisionLabel,
      shape: decisionLabel === undefined ? "box" : "diamond",
    });
  } else if (!nodes.has(id)) {
    nodes.set(id, { label: id, shape: "box" });
  }
  return id;
}

function mermaidToDot(source, title, description) {
  const nodes = new Map();
  const edges = [];
  for (const rawLine of source.split("\n")) {
    const line = rawLine.trim();
    if (!line || /^flowchart\s+LR$/i.test(line)) continue;
    let match = line.match(/^(.+?)\s+-\.\s*(.*?)\s*\.->\s+(.+)$/);
    if (match) {
      edges.push({ from: parseNode(match[1], nodes), to: parseNode(match[3], nodes), label: match[2], style: "dashed" });
      continue;
    }
    match = line.match(/^(.+?)\s+-->\|([^|]+)\|\s+(.+)$/);
    if (match) {
      edges.push({ from: parseNode(match[1], nodes), to: parseNode(match[3], nodes), label: match[2], style: "solid" });
      continue;
    }
    match = line.match(/^(.+?)\s+-->\s+(.+)$/);
    if (match) {
      edges.push({ from: parseNode(match[1], nodes), to: parseNode(match[2], nodes), label: "", style: "solid" });
      continue;
    }
    throw new Error(`Unsupported Mermaid statement: ${line}`);
  }
  const dot = [
    `// Title: ${title}`,
    `// Description: ${description}`,
    "digraph lesson_signal_path {",
    "  graph [rankdir=LR, bgcolor=\"transparent\", pad=\"0.25\", nodesep=\"0.45\", ranksep=\"0.65\", splines=ortho];",
    "  node [shape=box, style=\"rounded,filled\", fillcolor=\"#f8fafc\", color=\"#2563eb\", fontcolor=\"#0f172a\", fontname=\"Arial\", fontsize=12, margin=\"0.16,0.10\"];",
    "  edge [color=\"#475569\", fontcolor=\"#334155\", fontname=\"Arial\", fontsize=10, penwidth=1.6, arrowsize=0.75];",
  ];
  for (const [id, node] of nodes) {
    dot.push(`  ${id} [label=\"${dotEscape(node.label)}\", shape=${node.shape}];`);
  }
  for (const edge of edges) {
    const attributes = [];
    if (edge.label) attributes.push(`label=\"${dotEscape(edge.label)}\"`);
    if (edge.style === "dashed") attributes.push("style=dashed");
    dot.push(`  ${edge.from} -> ${edge.to}${attributes.length ? ` [${attributes.join(", ")}]` : ""};`);
  }
  dot.push("}", "");
  return dot.join("\n");
}

function renderAccessibleSvg(dotPath, svgPath, title, description) {
  execFileSync("dot", ["-Tsvg", dotPath, "-o", svgPath]);
  let svg = fs.readFileSync(svgPath, "utf8");
  svg = svg.replace(
    /<svg ([^>]+)>/,
    `<svg $1 role="img" aria-labelledby="diagram-title diagram-description">\n<title id="diagram-title">${title}</title>\n<desc id="diagram-description">${description}</desc>`
  );
  fs.writeFileSync(svgPath, svg);
}

function sentence(value) {
  const normalized = String(value ?? "").trim().replace(/\s+/g, " ");
  if (!normalized) return "";
  return /[.!?]$/.test(normalized) ? normalized : `${normalized}.`;
}

function experimentUnit(lesson) {
  return lesson.signals?.find((signal) => signal.name === lesson.experiment?.signal)?.unit ?? "";
}

function exactExperimentSection(markdown, lesson) {
  if (manuallyCuratedExperimentLessons.has(lesson.id)) return markdown;
  const match = markdown.match(
    /^## Experiment in Control Studio\s+([\s\S]*?)^## Evidence and decision$/m
  );
  if (!match) throw new Error(`${lesson.id} is missing the canonical experiment section`);
  const current = match[1].trim();
  const actionsMarker = "### Control Studio actions";
  const actionsIndex = current.indexOf(actionsMarker);
  if (actionsIndex < 0) throw new Error(`${lesson.id} is missing Control Studio actions`);
  let actions = current.slice(actionsIndex);
  actions = actions.replace(
    /^(\s+- )\*\*(?:Setup|Prerequisite|Initialization rule):\*\*/gm,
    "$1**Baseline check:**"
  );
  actions = appendWorkflowPedagogy(actions, lesson.id);
  const alreadyExact = current.startsWith(actionsMarker)
    && [...actions.matchAll(/^### (.+)$/gm)].every((heading) => heading[1] === "Control Studio actions")
    && !/\*\*(?:Setup|Prerequisite|Initialization rule):\*\*/.test(actions);
  if (alreadyExact) return markdown;

  const experiment = lesson.experiment ?? {};
  const change = sentence(lesson.learning?.change);
  const duration = Number.isFinite(experiment.durationSeconds)
    ? ` The declared event lasts ${experiment.durationSeconds} seconds.`
    : "";
  const restoration = experiment.restorationValue !== undefined
    ? ` It restores ${experiment.signal} to ${experiment.restorationValue} ${experimentUnit(lesson)} afterward.`
    : "";
  const seed = experiment.seed !== undefined
    ? ` The deterministic Simulation seed is ${experiment.seed}.`
    : "";
  const introduction = `This procedure tests one declared lesson change under controlled conditions. ${change}${duration}${restoration}${seed}`;
  const replacement = `## Experiment in Control Studio\n\n${introduction}\n\n${actions}\n\n## Evidence and decision`;
  return markdown.slice(0, match.index) + replacement + markdown.slice(match.index + match[0].length);
}

for (const directory of lessonDirectories(controlRoot)) {
  const readmePath = path.join(directory, "README.md");
  const lesson = JSON.parse(fs.readFileSync(path.join(directory, "lesson.json"), "utf8"));
  let markdown = fs.readFileSync(readmePath, "utf8");

  markdown = markdown.replace(/^Lesson objective:\s*/m, "**Lesson objective:** ");

  const normalizedMarkdown = markdown.replace(/[`*_]/g, "").replace(/\s+/g, " ").toLowerCase();
  const normalizedObjective = String(lesson.objective ?? "").replace(/[`*_]/g, "").replace(/\s+/g, " ").toLowerCase();
  if (normalizedObjective && !normalizedMarkdown.includes(normalizedObjective)) {
    markdown = markdown.replace(
      /^## The problem$/m,
      `**Lesson objective:** ${lesson.objective}\n\n## The problem`
    );
  }
  if (!/simulation(?: evidence|[- ]only|[\s\S]{0,120}(?:not|rather than)[\s\S]{0,80}physical)/i.test(markdown)) {
    markdown = markdown.replace(
      /^## The problem$/m,
      "**Evidence class:** Simulation evidence, not physical-plant qualification evidence.\n\n## The problem"
    );
  }

  markdown = markdown.replace(/^```(?:iecst|structured-text)$/gm, "```st");
  markdown = markdown.replace(/^\$\$\s*\n([\s\S]*?)\n\$\$\s*$/gm, (_whole, equation) => {
    const leftAligned = equation.split("\n").map((line) => line.trim()).filter(Boolean).join(" ");
    return `$${leftAligned}$\n`;
  });
  markdown = markdown.replace(/^(\$[^\n]+\$)\n(?=\S)/gm, "$1\n\n");

  markdown = markdown.replace(/^```mermaid\s*\n([\s\S]*?)\n```\s*$/gm, (_whole, source) => {
    const metadata = diagramMetadata.get(path.basename(directory));
    if (!metadata) throw new Error(`Missing diagram metadata for ${directory}`);
    const figuresDirectory = path.join(directory, "figures");
    fs.mkdirSync(figuresDirectory, { recursive: true });
    const dotPath = path.join(figuresDirectory, `${metadata.file}.dot`);
    const svgPath = path.join(figuresDirectory, `${metadata.file}.svg`);
    fs.writeFileSync(dotPath, mermaidToDot(source, metadata.title, metadata.description));
    renderAccessibleSvg(dotPath, svgPath, metadata.title, metadata.description);
    return `![${metadata.title}](figures/${metadata.file}.svg)`;
  });

  const problemMatch = markdown.match(/^## The problem\s+([\s\S]*?)^## What we need$/m);
  if (!problemMatch) throw new Error(`${lesson.id} is missing The problem`);
  if (!problemMatch[1].toLocaleLowerCase().includes(String(lesson.equipment).toLocaleLowerCase())) {
    const namedProblem = `The equipment in this lesson is ${lesson.equipment}.\n\n${problemMatch[1].trim()}`;
    markdown = markdown.slice(0, problemMatch.index)
      + `## The problem\n\n${namedProblem}\n\n## What we need`
      + markdown.slice(problemMatch.index + problemMatch[0].length);
  }

  const correctedNeed = correctedNeeds.get(lesson.id);
  if (correctedNeed) {
    const needMatch = markdown.match(/^## What we need\s+([\s\S]*?)^## The new idea$/m);
    if (!needMatch) throw new Error(`${lesson.id} is missing What we need`);
    markdown = markdown.slice(0, needMatch.index)
      + `## What we need\n\n${correctedNeed}\n\n## The new idea`
      + markdown.slice(needMatch.index + needMatch[0].length);
  }

  markdown = exactExperimentSection(markdown, lesson);

  fs.writeFileSync(readmePath, markdown);
}

const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
const courseEntries = catalog.filter((entry) => entry.id?.startsWith("control-"));
const escapeCell = (value) => String(value ?? "").replaceAll("|", "\\|").replace(/\s+/g, " ").trim();
const lessonRows = courseEntries.map((entry, index) => {
  const descriptor = JSON.parse(fs.readFileSync(
    path.join(repositoryRoot, "examples", entry.path, "lesson.json"), "utf8"
  ));
  const relativePath = `./${entry.path.replace(/^control\//, "")}/`;
  return `| ${index + 1} | [${escapeCell(descriptor.title)}](${relativePath}) | ${escapeCell(descriptor.equipment)} | ${escapeCell(descriptor.principalTechnique)} | ${escapeCell((descriptor.workflowStages ?? []).join(" → "))} — ${escapeCell(descriptor.objective)} |`;
});
const courseReadmePath = path.join(controlRoot, "README.md");
let courseReadme = fs.readFileSync(courseReadmePath, "utf8");
courseReadme = courseReadme.replace(
  /(## Complete lesson map\n\n\| Lesson \| Runnable project \| Real equipment \| Theory \| Practice in Control Studio \|\n\| ---: \| --- \| --- \| --- \| --- \|\n)[\s\S]*?(?=\n## For control engineers)/,
  `$1${lessonRows.join("\n")}\n`
);
fs.writeFileSync(courseReadmePath, courseReadme);
