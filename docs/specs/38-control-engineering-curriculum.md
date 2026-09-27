# Control Engineering Curriculum Architecture

## Current implementation decision — 5 September 2026

The user's approved replacement progression takes precedence over the older
84-lesson mapping below. Implement it in order: P on a tank; PI on a heater
(including its P baseline); PD on an unloaded shaft (including its P baseline);
PID on that shaft lifting a weight through a drum; manual tuning; the purpose
of a model; heater modelling from physics; heater modelling from a step
response; shaft modelling from physics; shaft identification; independent
model validation; closed-loop prediction; model-based tuning; qualification
against the same requirements. Keep the same heater and shaft throughout.
The failed manual trials must establish the reason for modelling before any
identification method is introduced.

The early Lesson 4 slot becomes the P tank lesson. Lesson 27 is not a second
introduction to P and is not repurposed. Existing paths remain stable while
each replacement project is completed. Historical acceptance does not accept
these replacements. The approved implementation includes the focused project
changes needed to deliver these experiments, with native behaviour checks;
it does not authorize publication or release work.

Lesson 4's first behaviour slice uses a tank initially at 50 cm, target 60 cm,
and a constant outlet balanced by a 20% inlet-pump command. The command is
u = 20 + 2(60-h), limited to 0–100%, with gain measured in percentage points
per centimetre. Disabled operation returns to 20%, which balances the outlet
and holds the attained level; it does not reset the tank. Invalid input or
an abort also removes automatic correction. The simulated rate is
dh/dt = 0.2(u-20) cm/s, with u expressed in percentage points. The coefficient
and outlet are configured teaching values. This known balancing command is
explained physically in the tank lesson; the older blanket restriction on
bias below does not apply to this approved example. The heater P baseline in
the next lesson establishes why a continuing holding input can require error.

## 1. Purpose

This specification is the teaching authority for the truST control-engineering course. The course shall teach real control strategies in prerequisite order. It shall not preserve a lesson, topic, equation, or project merely because it already exists.

Every lesson shall begin with a control problem the student can understand from earlier lessons, introduce the simplest recognised engineering method that solves that problem, and end with an observed limitation that creates the next lesson.

Equipment is used to make control ideas physical. The course is not about cold rooms, heaters, servos, or tanks themselves. Each plant is a worked example of a control problem that transfers to other equipment.

The current lesson folders, programs, metadata, figures, and evidence do not override this architecture. When an existing project disagrees with this specification, the project is nonconforming and shall be redesigned before its README claims the new lesson.

## 2. Accepted reference lessons

Lessons 1–4 define the required teaching style:

1. Lesson 1 identifies what must be controlled, measured, and changed, then observes the physical direction.
2. Lesson 2 uses the measurement to switch a binary command automatically and distinguishes slow plant motion from rapid command changes.
3. Lesson 3 introduces hysteresis and minimum state time as two separate solutions to two separate switching problems.
4. Lesson 4 introduces a continuously adjustable actuator, derives proportional control from the required relationship between error and correction, and exposes steady-state error.

These lessons are references for clarity and progression, not prose templates to copy mechanically. Their useful properties are:

- the first sentence states exactly what the student will do and discover;
- familiar physical language appears before technical terminology;
- only the theory needed for the lesson is included;
- every prediction tests the stated lesson purpose;
- implementation is linked rather than duplicated;
- evidence answers the prediction; and
- the final limitation creates the next lesson.

## 3. Non-negotiable curriculum rules

### Delivered loop bindings

Every delivered lesson's test-only ST sources MUST remain discoverable by
`trust-dev test` and MUST be excluded from Simulator source selection with the
existing default `@trustlsp:runtime-ignore` header marker. Ordinary PLC sources
remain included. This applies both to normal folder discovery and explicitly
configured ST include globs; a native test declaration is not a scheduled PLC
instance. Exclusion must preserve the original test declarations and assertions.

Every source and packaged lesson must pass Specification 35's loop decoder and
relationship validation. A lesson technique name is not a controller design
owner: project-defined P/PI algorithms and experiments declare `design: project`.
Open-loop response, prediction and identification comparisons declare
`topology: open-loop`; reference predictions and requested inputs are named
auxiliary evidence, not artificial controlled setpoints. The same address must
not be bound twice. The primary controller action describes the displayed loop,
not a mixture of other controllers compared by the lesson.

Binding acceptance establishes only configuration compatibility. It does not
establish that a lesson's controller, timing claim or learning outcome is correct.

### 3.1 Real strategies and prerequisite order

Lesson 3's delivered program publishes the fixed 2 °C target, 1 °C hysteresis
width and 8 s minimum ON/OFF times without hidden memory writes. The initial
OFF interval is qualified; normal subsequent transitions obey dwell, while
disable/abort forces both requested and applied cooling OFF. The time, count
and dwell-inhibit observations must describe actual transitions. Its test
programs must be excluded from runtime compilation and remain runnable through
`trust-dev test`. The door event remains simulator-relative at 60–90 s.

Lesson 2 must run directly from its delivered project: publish the fixed 2 °C
target, keep refrigeration OFF before experiment enablement, then command ON
at or above the target and OFF below it. No hidden memory initialization is
required. Count every command transition. Its door event remains at simulator
time 60–90 s, as stated in the README. Disable or abort stops refrigeration.
Its native test file remains discoverable by `trust-dev test` and is excluded
from runtime compilation using the default runtime-ignore header marker.

Lesson 1's fixed experiment starts when `P1.ExperimentEnabled` becomes true,
regardless of time spent in the running simulator beforehand. With the
experiment disabled, refrigeration stays OFF, the door stays closed and the
configured 4 °C initial equilibrium is maintained. Refrigeration is ON for
the first 200 updates of 0.1 s and OFF afterward. The door disturbance applies
from experiment time 60 s up to 90 s, then returns to the closed-door condition.
Stopping or aborting clears both commands. A repeat starts by restarting the
simulator to restore the same thermal state; merely enabling another experiment
does not reset a warm room.

Lesson 1's shipped native ST tests remain discoverable by `trust-dev test`,
but simulator source discovery must exclude `src/tests.st` from the running
PLC program using the existing runtime-ignore mechanism.
The compiled binding catalog used by Devices & Connections must honor the same
default `@trustlsp:runtime-ignore` header marker as the simulator. Ignored test
programs must not prevent listing the configured PLC input/output connections;
errors in ordinary runtime sources must still be reported.
The delivered simulator run needs no hidden writes to initialize the fixed
2 °C reference or make `InputsValid` true. The lesson program publishes its
fixed reference and derives input validity from the measured temperature on
every update. Nonfinite values and values outside −10 to 10 °C inhibit both
refrigeration and the door disturbance; the learner then aborts the test.

This lesson may represent its existing single thermal store with the supported
`state_space` model so that compressor and surroundings have independent
inputs. Its physical relationship remains
`dT/dt = (Ts - T - 0.05*u) / 20`, with T and Ts in °C, time in seconds,
and compressor encoding u equal to 0 (OFF) or 100 (ON). Ts is 4 °C with the
door closed and 6 °C with it open. These are configured teaching parameters;
the lesson does not introduce state-space mathematics or claim measured
equipment parameters. This is a truST experiment contract, not IEC semantics.

- Every controller taught as a solution shall be a defensible control strategy for the stated plant and objective.
- A mathematical component shall not become a standalone controller lesson merely to isolate its equation.
- Integral action shall first be taught as the integral part of a PI controller that removes the steady-state error observed with P control. The foundation sequence shall not operate or recommend an integral-only temperature controller.
- Bias may be introduced later for initialization, tracking, feedforward, or a known operating point. It shall not interrupt the beginner progression from P to PI.
- Saturation shall follow PI because saturation exposes integral windup. Anti-windup shall immediately follow that observed failure.
- Plant response classes shall be taught before controller selection.
- Safe experiment design and excitation shall be taught before identification.
- A model shall be validated on unused evidence before it is used for model-based tuning.
- State feedback shall precede LQR. Observers shall be taught before Kalman filtering, and LQR plus a qualified estimator shall precede LQG.
- Diagnosis shall precede retuning or selecting a fault response.
- Capstones are parallel assessments with declared prerequisites. They shall not pretend that one plant capstone causes the next.

### 3.2 One problem and one new idea

Each lesson shall answer one engineering question. Closely coupled safeguards may share a lesson only when the lesson explicitly separates their purposes, as hysteresis and minimum state time do in Lesson 3.

A lesson shall not combine several mathematical methods, plant classes, controller architectures, faults, or workflows and call the collection one idea. If the student must make more than one independent conceptual leap, revise the sequence into useful engineering steps, not isolated mathematical fragments.

One new idea means one useful advance in understanding, designing, operating or improving control. Each lesson shall identify what the student can now explain, decide or control that the preceding lesson could not, and demonstrate that value in its experiment. Modeling and analysis qualify when they enable a concrete prediction or control decision; introducing notation or manipulating an equation alone does not. Better control is not necessarily a faster response: accuracy, stability, robustness and safe operation can require explicit tradeoffs.

Do not create a standalone lesson for the I-term before PI. Explain accumulation and its mathematics inside the PI lesson, where adding integral action to the existing P controller solves the observed steady-state error. Shortness is not a reason to separate a method into lessons that have no useful control outcome of their own.

Keep lessons short by limiting what they teach, not by leaving gaps in the explanation. Teach one idea, observe what it solves and what remains, then build on that result. For example: P control supplies a correction proportional to error; the heater experiment reveals a remaining steady-state error; the next lesson adds integral action within PI to remove that error. Do not teach PI prematurely in the P lesson or repeat the whole P explanation in the PI lesson.

Use short paragraphs, a compact worked calculation, or a purposeful figure where these explain more clearly than continuous prose. Remove repetition, digressions and unnecessary terminology; never remove a necessary reasoning step merely to meet a word count. Briefly recall and link an earlier explanation instead of deriving it again. Essential new reasoning must remain in the lesson, not be hidden behind an optional link. If a complete explanation becomes a wall of text, narrow the concept or revise the prerequisite sequence before authoring; do not silently expand or renumber the course.

### 3.3 README section order

Every lesson README shall use exactly these sections:

1. `The problem`
2. `What we need`
3. `The new idea`
4. `Prediction`
5. `Implementation`
6. `Experiment in Control Studio`
7. `Evidence and decision`
8. `Conclusion`
9. `Reason for the next lesson`

The title is followed immediately by one plain-language sentence stating what the student will do and what they will discover. Course-position metadata, evidence classifications, safety disclaimers, and author-facing objectives shall not precede the lesson.

## 4. Mandatory manual authoring method

Structure alone does not establish teaching quality. Every lesson shall be written manually using this method.

### 4.1 Define the lesson before writing

Before editing a README, answer these questions in four plain sentences:

1. What does the student already understand?
2. What specific problem remains?
3. What single new idea solves it?
4. What remaining problem creates the next lesson?

If those four sentences do not form one causal chain, stop and correct the curriculum architecture before writing prose.

### 4.2 Write and approve the first sentence first

The first sentence shall state exactly what the student will do and discover using only words already taught. If the sentence is vague, abstract, or depends on undefined terminology, stop. Do not write the remainder until it is clear.

### 4.3 Rewrite from an empty page

Do not shorten, normalize, or rearrange the existing README. Existing prose may preserve the wrong lesson, duplicated theory, unsupported claims, or language written for reviewers rather than students. Read it for factual evidence, then write the lesson from an empty page.

### 4.4 Write one section at a time

Complete and reread one section before writing the next. Do not generate the complete lesson and attempt to repair it afterward.

After every paragraph, ask:

> Which part of this lesson does this paragraph help the student understand?

Delete the paragraph if there is no precise answer.

### 4.5 Introduce ideas in causal order

Use this order whenever the elements are needed:

1. physical situation;
2. cause and effect;
3. plain-language solution;
4. technical name;
5. mathematics;
6. implementation;
7. experiment; and
8. observed result.

Do not introduce a term or formula and explain its physical meaning afterward.

### 4.6 Apply the strict content filter

Every lesson shall have:

- no undefined terms;
- no unnecessary abbreviations;
- no repeated explanations;
- no copied Structured Text;
- no irrelevant safety or evidence language;
- no unexplained numbers;
- no equation unless the student needs it to solve the lesson problem;
- a physical or definitional derivation for every necessary equation;
- only OFF and ON for binary commands, never percentages; and
- only content that helps the student understand, apply, observe, or decide the lesson concept.

The `Implementation` section shall link to the commented Structured Text. The README shall not duplicate source code that can drift from the program.

### 4.7 Align prediction, experiment, and evidence

Prediction questions shall match the opening sentence exactly. Every question shall test something the lesson promised to teach.

Every experiment step shall make clear:

1. what the student does;
2. what the student observes; and
3. why that observation matters.

Evidence shall answer the prediction questions directly. Excessive precision, unrelated measurements, implementation-test descriptions, and evidence that does not affect the lesson decision shall be removed.

### 4.8 Perform the beginner reread

Read the completed lesson sentence by sentence as a student who knows only the declared prerequisites. For every sentence, identify its teaching purpose. Rewrite or delete any sentence that is vague, premature, repetitive, unsupported, or unnecessary.

Then compare the complete lesson with the preceding and following lessons. Confirm that it uses only ideas already taught and ends with the exact problem solved next. Only then may work move to the next lesson.

### 4.9 Perform and record the reference-lesson review

After the beginner reread, review the completed lesson against all four accepted reference lessons:

1. Compare it with Lesson 1: is the purpose immediate, concrete, and free of text that does not teach the stated concept?
2. Compare it with Lesson 2: does it distinguish what the controller changes from how the physical process responds?
3. Compare it with Lesson 3: if two mechanisms appear, are their separate jobs stated and observed separately?
4. Compare it with Lesson 4: does physical cause lead to the required relationship, and does that relationship lead to mathematics only when mathematics is needed?

The review shall also answer:

- Can a beginner explain why this lesson exists after reading the first sentence and `The problem`?
- Does every prediction receive a direct answer in `Evidence and decision`?
- Can any sentence, equation, number, figure, or experiment step be removed without weakening the promised learning? If yes, remove it.
- Does the actual Structured Text and simulation perform the experiment described by the README?

Record `revise` until every answer passes. Record `accepted` only after rereading the corrected lesson. Do not open the next lesson while the current lesson remains `revise`.

## 5. Physics and mathematics

### 5.0 Assumed knowledge and mathematical progression

The mathematical entry prerequisite is basic calculus, including ordinary algebra, derivatives and integrals. No prior control theory, Laplace transforms, complex-number methods, matrix methods, specialist differential-equation solution methods or frequency-domain analysis is assumed. Introduce each additional tool before using it, motivated by a problem the student already understands. Even familiar calculus notation must be connected to the physical quantities and units in the current example.

Explain where a plant model comes from, not merely what the symbols in a supplied formula mean. Begin with the physical mechanism and assumptions, build the balance or relationship, then derive its mathematical form. For a tank, for example, connect inflow minus outflow to stored volume, use the tank geometry to relate volume to level, and only then express the rate of level change. Distinguish a physical law, a measured relationship and a chosen simulation approximation.

A transform is a mathematical tool, not the physical origin of the model. Before using Laplace transforms, explain the difficulty they address, derive the properties needed for the example from their definition, and explain the role of initial conditions. Do not use phrases such as "frequency domain" as explanations or treat Laplace transformation and frequency response as interchangeable. Teach missing prerequisites in the curriculum sequence rather than inserting several new mathematical subjects into one lesson.

### 5.1 Physical explanation comes first

Before any plant or controller equation, explain what physically stores, transfers, adds, removes, or resists energy, mass, momentum, force, pressure, flow, temperature, or information. Explain the direction of every cause and effect.

A simulator relationship shall not be presented as a universal physical law. State the assumptions that make the relationship useful.

### 5.2 Derive only necessary equations

Every equation shall come from one of these sources:

- a definition;
- conservation of mass, energy, or momentum;
- Newton's laws;
- geometry;
- an earlier derived equation;
- measured evidence; or
- a clearly stated approximation.

Before an equation appears, define every symbol and unit in ordinary language. Derive the relationship step by step, explain every sign, work one useful numerical example, and translate the result back into physical behavior.

Show every intermediate step that depends on a newly introduced idea, with a short reason for the operation. In the numerical example, show substitution, arithmetic and units through to the result, then state what the student should observe in the process or truST simulation. Do not hide an untaught method behind "obviously", "it follows" or "after some algebra". Concision means the shortest complete explanation for a reader with basic calculus and the preceding lessons, not the fewest words regardless of missing reasoning.

An equation written on its own row shall use a left-aligned `$...$` Markdown paragraph. Centered `$$...$$` blocks are not permitted in lesson READMEs.

### 5.3 Explain every number

Every number shall be identified as one of:

- a process requirement;
- a measured initial condition;
- equipment or manufacturer data;
- a value identified from a named experiment;
- a result inherited from a validated earlier lesson;
- a simulation configuration; or
- a teaching choice.

Explain how the equivalent value would be obtained on physical equipment. Do not present a configured number as measured, identified, tuned, safe, or universal.

## 6. Figures, Control Studio, and evidence

Use a diagram or graph only when it explains a physical relationship, signal path, time sequence, response, comparison, or limitation more clearly than prose. A figure shall replace redundant prose, not repeat it. A wrong or stale figure shall be removed until corrected.

Result figures shall come from the lesson's actual retained run. Binary traces shall be labelled OFF and ON. Compared cases shall use equivalent conditions and scales.

Control Studio instructions shall describe only controls the student actually uses. The workflow shall state the initial condition, the action, the observed signals, the purpose of any event, the abort condition, and the restored final state. Interface labels may be named exactly, but unexplained implementation language such as “sole write,” “authority,” or “restoration” shall not replace a student-facing explanation.

Simulation evidence establishes only simulated behavior. This qualification belongs beside the conclusion it limits and shall be stated once. It shall not interrupt the teaching repeatedly.

No README lint test, Rust curriculum test, VS Code curriculum test, prose generator, or mechanically generated teaching text shall be created. Native Structured Text tests may verify program behavior, but test descriptions shall not replace teaching evidence in a README.

## 7. Curriculum architecture

The course contains 84 numbered lessons. The order below is authoritative. Existing projects that implement a different topic shall be renamed, moved, merged, split, or rewritten during implementation.

### 7.1 Feedback and practical temperature control

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 1 | Identify what must be controlled, measured, and changed; observe the process direction. | Manual observation does not regulate the result. |
| 2 | Use the measured temperature to switch refrigeration OFF or ON automatically. | One switching boundary causes chatter. |
| 3 | Use separate ON/OFF boundaries and minimum state times. | A binary actuator still forces the temperature to cycle. |
| 4 | Make a continuously adjustable heater command proportional to present error. | P control needs steady error to supply holding power. |
| 5 | Add accumulated correction to P control to form PI control. | The PI calculation can request power the heater cannot apply. |
| 6 | Separate requested and applied output; observe saturation and integral windup. | Stored integral output continues growing while the actuator is limited. |
| 7 | Make integral state follow achievable output with anti-windup. | An actuator can also fail to reach an in-range request quickly enough. |
| 8 | Track a rate-limited actuator and make controller state follow the applied output. | A controller must be judged separately for target changes and load changes. |
| 9 | Compare setpoint response with disturbance rejection. | One PI tuning may respond too aggressively to setpoint changes. |
| 10 | Weight the setpoint in the proportional path. | Measurement noise can still create unnecessary actuator motion. |
| 11 | Filter only the measurement variation that the control objective does not need. | Filter behavior depends on elapsed update time. |
| 12 | Use real elapsed time and understand missed samples and aliasing. | Observed curves alone do not explain why a plant responds as it does. |

Lesson 12's current project is a scripted arithmetic comparison: its
`ScriptedElapsedTime` value represents the hypothetical 0.2 s missed-update
case at cycle 10. It is not a scheduler measurement. The configured runtime
task remains 100 ms; plant-internal substeps and held outputs are independent.
A real elapsed-time acquisition contract remains future work.

### 7.2 Plant physics and models

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 13 | Observe self-regulating, integrating, delayed, oscillating, inverse, and interacting responses without naming equations first. | Response shapes describe behavior but do not explain its physical cause. |
| 14 | Derive accumulation equals inflow minus outflow from conservation. | Energy storage and loss require a specific dynamic thermal model. |
| 15 | Derive a first-order water-heater model from stored energy, heater input, and heat loss. | Some plants contain nonlinear flow relationships. |
| 16 | Derive the gravity-tank square-root outflow law from pressure head and conservation. | A nonlinear model is difficult to use with basic linear design tools. |
| 17 | Linearize the gravity-tank model around one equilibrium and state the valid region. | The local differential equation still needs an input-output representation. |
| 18 | Derive a transfer function from the local differential equation using the Laplace transform. | Coefficients alone do not reveal the response contribution of poles and zeros. |
| 19 | Connect poles and zeros to stable, integrating, oscillating, and inverse responses. | One input-output equation can hide internal stored quantities. |
| 20 | Derive state equations by assigning one state to each independent physical store. | Useful numerical models often contain unknown parameters. |

### 7.3 Experiments, identification, validation, and tuning

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 21 | Define safe experiment bounds, initial conditions, aborts, and restoration before excitation. | A safe experiment may still contain too little information. |
| 22 | Choose step, pulse, ramp, relay, or multisine excitation from the dynamics that must be measured. | Informative data still require a model-estimation method. |
| 23 | Estimate gain, time constant, and delay from a qualified step response. | One assumed model structure may not explain the data. |
| 24 | Compare candidate model structures and choose the simplest adequate one. | A good fit can hide parameters the experiment cannot distinguish. |
| 25 | Test parameter identifiability and redesign weak experiments. | An identifiable fitted model may still fail on new conditions. |
| 26 | Freeze the model and validate it on unused amplitudes, disturbances, and operating points. | A validated model does not choose a controller automatically. |
| 27 | Select ON/OFF, P, PI, PD, or PID from plant behavior and control requirements. | The selected controller still needs numerical settings. |
| 28 | Tune PI manually by changing one term at a time and observing the trade-offs. | Manual iteration can be slow and subjective. |
| 29 | Use a bounded relay experiment to estimate critical behavior and propose gains. | Critical-cycle rules ignore a validated process model and desired response speed. |
| 30 | Derive model-based PI gains from a validated model and a chosen response target. | Different eligible methods produce different defensible trade-offs. |
| 31 | Compare tuning methods under identical requirements and select the simplest passing candidate. | Thermal PI does not provide damping for an oscillating mechanical plant. |

### 7.4 Motion control and derivative action

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 32 | Use proportional position feedback and interpret it as commanded stiffness. | More stiffness can increase oscillation because the mechanism lacks damping. |
| 33 | Add measured velocity feedback as damping to form PD control. | PD cannot supply steady holding torque against a constant load. |
| 34 | Add integral action to PD to form PID and supply holding torque. | Differentiating a changed setpoint can create a torque kick. |
| 35 | Apply derivative action to measurement rather than the setpoint change. | Derivative action still amplifies measurement noise. |
| 36 | Filter the derivative signal while preserving useful damping. | Continuous equations must be executed at discrete update times. |

### 7.5 Production digital PID

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 37 | Derive discrete integral and derivative updates from sampled areas and slopes. | Equivalent continuous ideas can be stored in different digital forms. |
| 38 | Compare positional and incremental PID forms through their state and output behavior. | Stored state must be initialized before a controller receives authority. |
| 39 | Initialize Disabled, Manual, Auto, and Track modes from measured process and actuator state. | Correct initialization alone does not prevent a transfer bump. |
| 40 | Track the actual applied output to make Manual-to-Auto transfer bumpless. | Several related settings can still change incoherently within one scan. |
| 41 | Apply controller parameters as one atomic configuration revision. | Coherent parameters are still unsafe when input data are invalid. |
| 42 | Reject invalid or stale data without corrupting state, then recover deliberately. | Mathematical closed-loop stability has not yet been established. |

### 7.6 Classical stability and frequency design

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 43 | Derive the closed-loop characteristic equation and connect its roots to physical response. | Calculating roots repeatedly does not directly give the complete stable gain interval. |
| 44 | Use the Routh-Hurwitz table to find the stable gain interval. | The interval does not show how individual poles move with gain. |
| 45 | Use root locus to follow pole motion and place a compensator. | Pole locations do not directly show response across frequency. |
| 46 | Build a Bode plot from sinusoidal response and read resonance and margins. | Large delay can make margin readings incomplete or ambiguous. |
| 47 | Use the Nyquist contour to determine closed-loop stability with delay and unstable open-loop poles. | Stability alone does not measure disturbance rejection or noise amplification. |
| 48 | Derive sensitivity and complementary sensitivity from the feedback equations. | Improving one frequency range necessarily affects another. |
| 49 | Shape the loop to balance tracking, disturbance rejection, noise, robustness, and actuator effort. | The continuous design must survive sampling and computation delay. |
| 50 | Map the sampled system into the z-plane and verify discrete-time stability. | Single-loop tools do not solve coordination between several control paths. |

### 7.7 Industrial control structures

These lessons introduce alternative structures in increasing order of architectural coordination. Each lesson begins from the limitation of the simplest structure already known; the next lesson is not justified merely by changing equipment.

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 51 | Add measured-disturbance feedforward while feedback corrects model mismatch. | Some important disturbances are not measured before they enter the plant. |
| 52 | Add a fast inner feedback loop beneath a slower primary loop to form cascade control. | Some processes require one material stream to follow another rather than an inner state. |
| 53 | Make one measured flow follow another with ratio control and feedback trim. | One control demand may need two opposing actuators. |
| 54 | Map one signed demand across heating and cooling actuators with split-range control. | A protective objective may need priority over the production objective. |
| 55 | Use selectors, override control, and tracking to protect a constraint. | One fixed tuning may not work across a nonlinear operating range. |
| 56 | Schedule validated gains across operating points without bumps or chatter. | A long transport delay cannot be removed by local gain changes. |
| 57 | Use a Smith predictor when a validated model can separate transport delay from the local response. | Poor response may originate in the final control element rather than the controller. |
| 58 | Measure installed valve position and flow to characterize gain, deadband, and travel rate. | Static characterization may not reveal direction-dependent sticking. |
| 59 | Diagnose stiction from repeated requested-position, measured-position, and process-response patterns. | Similar loop symptoms can also come from sensors, capacity, disturbances, interaction, or communication. |
| 60 | Diagnose the complete loop before retuning and choose the next evidence-producing action. | Several manipulated and controlled variables can interact simultaneously. |

### 7.8 Multivariable control

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 61 | Measure every input-output path under equivalent conditions. | Path gains alone do not show the effect of closing the other loops. |
| 62 | Derive the Relative Gain Array and use it to select pairings. | Pairing does not remove remaining interaction. |
| 63 | Commission paired decentralized PI loops and measure their interaction. | Strong cross-effects may prevent the paired loops meeting requirements. |
| 64 | Add static or dynamic decoupling only where the interaction model supports it. | Model mismatch and constraints can make cancellation fragile. |
| 65 | Select a robust decentralized, decoupled, or centralized architecture under uncertainty. | Centralized state-based methods require actuator and sensor feasibility. |

### 7.9 State-space, estimation, optimal control, and prediction

Lessons 66 and 67 shall retain the learner-selected actuator and sensor layouts
through disabled, enabled and aborted scans. Layout 0 selects the upstream
pump/downstream sensor; layout 1 selects the bypass pump/upstream sensor.
The selected pump alone receives the bounded reviewed flow; the other pump
remains zero. Disabling authority, invalid inputs or safety abort shall restore
both pump outputs to zero without resetting the comparison selections. The
READMEs shall identify the existing Live Values selection controls and require
separate runs from matching initial conditions for a fair comparison. These are
truST lesson contracts, not changes to IEC language semantics.

Lessons 68 and 70 shall likewise retain their mapped `BearingArchitecture`
and `MotorObserverEnabled` comparison selections instead of overwriting them
on every scan. Bearing architecture 0 is position-only feedback and 1 uses
measured displacement and velocity. With the motor observer disabled, feedback
uses measured motor speed and zero estimated load; when enabled, it uses the
existing innovation-driven state/load estimate. Authority removal or abort
shall clear applied current/torque and observer state without changing those
learner selections. Compare cases in separate runs from matching initial states.

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 66 | Test controllability from the physical actuator placement and state model. | Reachable states may not be reconstructable from installed sensors. |
| 67 | Test observability from the physical sensor placement and state model. | Feasible state access does not yet place the closed-loop dynamics. |
| 68 | Place closed-loop poles with state feedback. | Pole placement does not express a systematic state-error versus effort trade-off. |
| 69 | Derive LQR from a declared quadratic state-and-effort cost. | State feedback requires states that may not be measured. |
| 70 | Reconstruct unmeasured states with a model-based observer and measured-output correction. | Fixed observer poles do not express process and measurement uncertainty. |
| 71 | Derive Kalman filtering from declared process and measurement uncertainty. | A qualified estimator and optimal regulator still need to be combined. |
| 72 | Combine LQR with the Kalman estimate to form LQG and test robustness limits. | Quadratic costs do not directly enforce hard future constraints. |
| 73 | Use receding-horizon prediction to choose the next move under explicit constraints. | Linear predictive control remains limited by nonlinear behavior and model uncertainty. |

### 7.10 Nonlinear, adaptive, robust, networked, and fault-aware control

| Lesson | Lesson problem and new idea | Limitation that creates the next lesson |
| ---: | --- | --- |
| 74 | Use an energy-like Lyapunov function to certify a local nonlinear stability region. | A local certificate does not control a wide nonlinear operating range. |
| 75 | Cancel a qualified known nonlinearity with feedback linearization and retain a bounded fallback. | Known structure does not cover unknown parameter drift. |
| 76 | Adapt a parameter only with sufficient excitation, projection bounds, supervision, and fallback. | Some uncertainty is safer to bound than to estimate online. |
| 77 | Qualify one controller over a declared uncertainty set with robust-control analysis. | A local deterministic connection does not include network timing and loss. |
| 78 | Preserve local control authority through latency, jitter, loss, reordering, and stale remote commands. | Healthy communication does not prove that sensors and actuators are healthy. |
| 79 | Detect and isolate supported faults, enter a defined degraded mode, and recover only with evidence. | The remaining task is to synthesize complete control systems without new theory. |

### 7.11 Capstone assessments

Capstones are parallel assessments. Each uses only techniques already taught, introduces no new control theory, and ends in a defensible engineering decision rather than a transition to another technique.

| Lesson | Assessment |
| ---: | --- |
| 80 | Commission a cold-storage ON/OFF controller with justified hysteresis and minimum state times. |
| 81 | Derive, identify, validate, tune, and qualify a water-heater PI controller. |
| 82 | Select and qualify a servo PID controller under load, noise, limits, and sampling. |
| 83 | Select an industrial or multivariable architecture under interaction, constraints, faults, and fallback requirements. |
| 84 | Independently defend the simplest safe control-system recommendation from requirements, physics, evidence, uncertainty, approval, and rollback. |

## 8. Lesson implementation order

Implementation shall proceed in strict numerical order.

For each lesson:

1. write the four-sentence lesson definition;
2. compare it with the authoritative row in Section 7;
3. inspect the preceding and following lessons;
4. identify whether the existing project implements the specified strategy;
5. stop and report a project mismatch rather than writing false prose;
6. rewrite the README manually from an empty page;
7. reread every sentence as a beginner;
8. perform and record the reference-lesson review in Section 4.9; and
9. accept the lesson before opening the next one.

Lessons 5–84 are not accepted merely because existing files, programs, tests, figures, or evidence exist. In particular, the former standalone bias and integral-only sequence is superseded. Lesson 5 shall teach PI directly from the steady-state error observed in Lesson 4.

## 9. Acceptance criteria

A lesson is accepted only when all of the following are true:

- its first sentence states the concrete student action and discovery;
- its problem was observed or established in a prerequisite lesson;
- it teaches one recognised control strategy or one necessary supporting concept;
- all new terminology follows a plain-language explanation;
- all necessary mathematics is physically or definitionally derived;
- every number has an honest source;
- prediction, experiment, evidence, conclusion, and next limitation form one throughline;
- binary commands are described only as OFF and ON;
- the README contains no copied Structured Text;
- every paragraph has a specific teaching purpose;
- the actual project behavior agrees with the lesson; and
- the next lesson solves the limitation demonstrated here.

The complete course is accepted only after all 84 lessons have passed this manual review in order and the entire sequence has been reread as one continuous beginner-to-advanced curriculum.

## 10. Explicit exclusions

The curriculum shall not use:

- an integral-only temperature controller as a foundation lesson;
- arbitrary lesson count preservation as a reason to retain a topic;
- equipment changes as substitutes for conceptual progression;
- copied code as teaching prose;
- formulas presented before physical meaning;
- unexplained plant models or controller gains;
- invented or stale evidence;
- repeated safety disclaimers that do not affect the current decision;
- generated prose or mechanical README normalization; or
- README, Rust, or VS Code tests whose purpose is to grade lesson prose.
