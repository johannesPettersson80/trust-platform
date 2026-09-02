# Control Engineering Learning Program

## 1. Purpose

This specification defines the behavior and teaching sequence of the truST
Control Engineering Learning Program. The program shall take a learner from
their first feedback loop through professional PID implementation, plant
modelling, identification, classical analysis, industrial control structures,
multivariable control, state-space methods, predictive control, nonlinear and
robust methods, and complete control-system commissioning.

The program shall not be a catalog of unrelated examples. Each lesson shall
begin with a limitation that the learner has already observed, explain the
theory required to understand that limitation, and introduce one new technique
that addresses it. The result of one lesson shall create the reason for the
next lesson.

Every lesson shall use named real equipment with plausible sensors, actuators,
disturbances, units, constraints, and operating conditions. Simulation may
simplify the equipment, but the lesson shall explain the simplification and
the conditions under which the model is useful. A project that only contains
labels, precomputed values, or prose describing an unexecuted method is not a
lesson in that method.

The ordinary controller-facing I/O path shall be used for both simulated and
physical plants. Simulation evidence, identified-model evidence, and physical-
plant evidence shall remain visibly distinct.

## 2. Mandatory lesson behavior

### 2.1 Problem-driven lesson sequence

After its title, course position, and one-sentence objective, every lesson shall
present its teaching material using these exact learner-facing sections in this
exact order:

1. **The problem** presents the named real-world equipment, the solution the
   learner already understands, the observable failure or limitation of that
   solution, and why that failure matters. The problem shall appear here; it
   shall not be postponed to a later workflow instruction or results table.
2. **What we need** states the required improvement in ordinary engineering
   language without relying on a technique or term that has not yet been
   explained.
3. **The new idea** introduces the technique that addresses the problem. It
   explains physical cause and effect in words first, then defines every new
   quantity, symbol, unit, equation, assumption, and validity limit.
4. **Prediction** calculates or reasons about measurable behavior before the
   learner sees practical instructions or results.
5. **Implementation** shows the concise production-style Structured Text that
   actually runs and traces the new idea through configuration, inputs,
   controller or method invocation, requested output, applied output, and plant
   input where applicable.
6. **Experiment in Control Studio** defines preparation, signal bindings,
   initial conditions, independent and controlled variables, exact visible UI
   actions, limits, abort conditions, safe state, restoration, and evidence to
   retain. It executes the baseline and changed case under equivalent
   conditions unless changing a condition is the lesson subject.
7. **Evidence and decision** defines every measure and decision rule before its
   result, presents executable evidence, qualifies invalid or ineligible runs,
   applies acceptance criteria, and selects or rejects the technique.
8. **Conclusion** reconciles prediction with evidence, explains the physical
   mechanism, and bounds the conclusion by plant class, operating region,
   assumptions, and relevant counterexamples.
9. **Reason for the next lesson** names the remaining observed limitation and
   explains why it creates the need for the next technique.

These nine section names and their order are canonical. A lesson shall not use
the former `Context`, `Reason`, `Theory`, `Worked prediction`, `Controller under
test`, `Practical preparation`, `Baseline`, `Change`, `Comparison`,
`Validation`, `Explanation`, or `Transfer` sequence as an alternative
learner-facing structure. Necessary detail from those former sections belongs
inside the applicable canonical section above.

The first lesson, or a lesson that begins a genuinely independent learning
spine, shall use an uncontrolled, manual, existing-equipment, or previously
taught baseline as the solution under examination. It shall not invent a
prerequisite lesson merely to satisfy the sequence. A capstone may inherit more
than one earlier limitation, but it shall still present one coherent problem
and one bounded engineering decision.

Practical instructions shall not use a concept, symbol, plot, tuning rule, or
decision criterion that has not already been explained in the lesson or an
explicit prerequisite. The learner may navigate freely, but the presented
lesson order shall never ask them to imitate unexplained steps.

Required mathematics shall be taught just in time before its first control
application. This includes algebra and units, rates and integrals, differential
equations, complex numbers, transforms, matrices and eigenvalues, probability,
and optimization. A mathematical operation shall first be interpreted on the
named equipment and then expressed symbolically; the course shall not assume
that a beginner already knows an unexplained mathematical convention.

Every new plant model and control law shall use the same explanatory order.
The lesson shall first describe the equipment's physical cause and effect in
ordinary words. It shall then introduce the engineering name, define each
symbol and unit, present the equation, work one numerical example, and connect
the calculated result back to the expected physical response. A symbol shall
not appear in an equipment summary, figure, equation, or practical instruction
before its meaning has been taught in the lesson or an explicit prerequisite.

Every displayed formula shall also answer three learner questions before the
workflow uses it: what the relationship means on the named equipment, why that
relationship is needed here, and how it follows from a definition, physical
balance, geometry, earlier equation, or stated approximation. Substitution of
numbers shall show the intermediate calculation and the prose immediately
afterward shall interpret the sign and magnitude physically. Merely naming a
formula, presenting a symbolic result, or asking the learner to copy it into a
tool is not pedagogical completion.

Every symbol shall be introduced in a complete plain-language sentence before
its first appearance anywhere in the lesson. The introduction shall use the
form `ordinary-language name (symbol)`, explain what the quantity physically
means on the named equipment, and state its unit. This requirement applies to
prose, equations, figures, tables, captions, plots, labels, legends, worked
examples, and practical instructions. A symbol used inside the definition or
equation of another quantity must therefore already have been introduced.

For example, a temperature-control lesson shall first explain:

> The setpoint (SP) is the desired outlet-water temperature. The process value
> (PV) is the outlet-water temperature measured by the sensor. Both quantities
> are measured in degrees Celsius. The error (e) is the setpoint minus the
> process value. A positive error means the measured water is colder than the
> desired temperature.

Only after the setpoint (SP), process value (PV), and error (e) have all been
introduced may the lesson present their relationship:

$e = SP - PV$

It shall then work a numerical example and translate the result back into
ordinary language:

$e = 35\,{}^\circ\mathrm{C} - 20\,{}^\circ\mathrm{C} = 15\,{}^\circ\mathrm{C}$

> The measured water is therefore 15 °C colder than the desired temperature.

### 2.1.1 Pedagogical README and teaching figures

Every one of the 84 lesson projects shall ship a pedagogical README that can be
followed by a learner who has completed only its declared prerequisites. The
README shall contain the complete ordered teaching flow from §2.1; metadata,
source comments, and external handbook links may support it but shall not stand
in for missing explanation.

A lesson README shall include a diagram or graph where it makes a physical
relationship, signal path, dynamic response, comparison, or limitation
materially easier to understand than prose alone. Figures are teaching
material, not decoration, and shall not be added when they merely repeat text.

Teaching figures shall obey these requirements:

- a physical or block diagram appears after the ordinary-language explanation
  and before the corresponding symbolic equation when the topology is new;
- an annotated response graph introduces the observed shape in physical words
  before labels such as gain, time constant, dead time, damping, pole, or
  stability margin are used;
- a results graph is generated from the shipped executable lesson and retained
  run evidence; invented, mocked, or hand-adjusted result curves are forbidden;
- axes, units, signals, operating event, limits, legend, and evidence class are
  explicit, and the caption states what the learner should notice and why;
- compared cases use equivalent conditions and the same visual scale;
- relative paths, useful alternative text, readable labels, and distinctions
  that remain understandable in light, dark, and high-contrast presentation
  are required; and
- lesson READMEs shall not use Mermaid or another diagram source that the
  shipped VS Code Markdown preview leaves as an empty or unrendered box; a
  diagram needed by the lesson shall be retained as a checked-in SVG with
  useful alternative text and shall be inspected in the real preview; and
- the figure source or deterministic generation command is retained with the
  lesson so a changed executable result cannot leave a stale teaching graph.

The normal teaching order is therefore: physical explanation, helpful figure,
defined names and symbols, equation, worked prediction, bounded practical run,
measured figure, and engineering conclusion.

### 2.1.2 Measurement, executable truth, and qualified conclusions

Every lesson shall use the following measurement and implementation pattern
where it is applicable:

1. **Define a decision measure before reporting it.** The lesson shall state in
   ordinary words what the measure means, then define its symbol or calculation,
   signal, unit, time window, sample treatment, tolerance, and acceptance
   direction. A label such as `movement`, `error`, `settling`, `effort`, or
   `robustness` without this definition is not an engineering measure.
2. **Use units that describe the calculation.** An instantaneous actuator
   command may use percent, while the sum of absolute command changes uses
   percentage points. A cumulative measure may exceed the actuator's
   instantaneous range. Sample-rate-sensitive measures shall retain the sample
   interval and comparison cases shall use the same interval.
3. **Qualify equilibrium before reporting steady-state performance.** A lesson
   shall declare an observable settling condition and evaluation window before
   calculating steady-state error or another equilibrium quantity. A response
   that does not satisfy that condition shall be reported as `Not settled`; a
   final sample shall not be presented as steady-state evidence. For a settled
   noisy response, the lesson shall use a declared window statistic rather than
   an arbitrary single sample.
4. **Separate calculation from physical actuation.** Whenever a limit, rate
   limit, selector, tracking path, interlock, or actuator nonideality can make
   the controller calculation differ from the plant input, the lesson shall
   introduce the requested manipulated variable and applied manipulated
   variable separately. Equations, plots, tables, run artifacts, and decisions
   shall state which one they use.
5. **Qualify conclusions by plant class and assumptions.** A lesson shall not
   turn an observation from one plant into a universal controller rule. It
   shall name the applicable plant class, operating region, equilibrium
   requirement, saturation state, model assumptions, and other conditions that
   make the conclusion true. The Transfer section shall name a counterexample
   or boundary where that conclusion does not apply when one is pedagogically
   important.
6. **Connect physical reasoning to the governing relationship.** When a simple
   equilibrium, conservation, or dynamic relationship explains the observed
   limitation, the lesson shall introduce its quantities in words, present the
   relationship, work a numerical prediction, and check the prediction against
   executable evidence. The relationship shall be qualified by its assumptions.
7. **Show the controller that actually runs.** After explaining the control
   law, the README's `Implementation` section shall contain a short excerpt
   showing the real Structured Text
   configuration and invocation that implements the lesson. It shall use the
   shipped program's names and API. Invented pseudocode, a copied alternative
   controller, or an excerpt too large to expose the governing choice is not
   acceptable.
8. **Preserve controlled-experiment discipline.** The learner shall predict
   before running, change only the declared independent variable, retain all
   conditions required to reproduce the result, and decide against criteria
   declared before inspecting the result.
9. **Describe the implemented controller form precisely.** A parameter
   definition shall say what the implementation calculates. For example, in a
   positional proportional controller, proportional gain states how much
   output is requested for each unit of error; it shall not be described as a
   change from the preceding output. The lesson shall state whether the law is
   positional or incremental, where bias enters, whether derivative acts on
   error or measurement, and which continuous-to-discrete realization runs.
10. **Explain the real actuator before simplifying it.** The lesson shall state
    whether the actuator is one-sided or bidirectional and shall define its
    physical range, rate, deadband, interlocks, and other relevant constraints
    before writing a clamp or ideal actuator equation. A negative request to a
    heater, for example, cannot be taught as active cooling when the installed
    actuator can only add heat.
11. **Keep validity separate from ranking.** A comparison shall first classify
    each run as valid, settled, saturated, unstable, aborted, or otherwise
    ineligible for a stated measure. Only eligible runs may then be ranked by
    that measure. An oscillating run shall not be credited with a smaller
    steady-state error because its last sample happened to be near the
    setpoint.
12. **Use vocabulary that prepares the next lesson.** A lesson shall name a
    concept only after defining it, but shall use the same precise name that
    later lessons build upon. For example, a zero operating-point bias shall
    not be introduced as an unexplained "fixed holding output" before bias is
    taught. The Transfer section may name the next concept and its purpose, but
    shall not rely on that concept to explain the current experiment.

These rules apply to README prose, lesson metadata, executable tests, retained
run artifacts, plots, Control Studio labels, and videos. Their names, windows,
units, and calculations shall agree across all of those surfaces.

When a time-domain comparison uses the following common measures, their
minimum definitions are:

- **Final-window absolute error** is the declared statistic of the absolute
  difference between setpoint and process value over a declared final window,
  and is reported only after the settling criterion has passed. For a sampled
  mean, explain the words first and then write
  `e_final = mean(|SP[k] - PV[k]|)` over the declared samples.
- **Process-value excursion** states whether it is peak-to-peak travel,
  departure from an operating point, or departure from setpoint. For
  peak-to-peak travel, explain the quantity first and then write
  `PV_excursion = max(PV[k]) - min(PV[k])` over the declared event window.
- **Cumulative manipulated-variable movement** is total variation, calculated
  from consecutive samples. Explain that it measures actuator activity and
  then write `MV_travel = sum(|MV[k] - MV[k-1]|)`. Report percent commands as
  percentage-point travel, and state whether `MV` is requested or applied.

These are definition templates, not mandatory metrics for every plant. A
lesson shall select only measures that answer its engineering question, define
them before the results, and retain enough sampled data to reproduce them.

### 2.1.3 Lesson source structure and teaching comments

The shipped Structured Text shall be a real executable project organized as it
would be for a production controller, with only the amount of structure the
lesson actually needs. `Main.st` owns controller-facing input and output
mapping, top-level operating and safety decisions, and composition of the
lesson's components. A stateful or substantial control algorithm shall be
implemented as a named function block in its own source file and instantiated
from `Main.st`. A function block, function, method, class, interface, or type
declaration shall not be hidden above or below the main program in `Main.st`.

A short, single-purpose calculation may remain directly in `Main.st` when
extracting it would obscure rather than clarify the lesson. File length alone
does not decide this boundary. Extraction is required when the code owns
retained algorithm state, performs a multi-stage search or estimator, exposes
a reusable controller contract, or mixes controller implementation with plant
I/O and lesson orchestration. A lesson-specific function block is acceptable;
the lesson shall not pretend that an equipment-specific teaching block is a
general production library component.

The function block interface shall use names from the lesson's explained
signal and parameter contract. `Main.st` shall make the data flow visible:
physical inputs and reviewed settings enter the block, the block reports its
requested output and status, and top-level safety or actuator logic produces
the applied output. Hidden global dependencies and duplicated implementations
of the same algorithm are forbidden.

Related parameters, forecasts, model data, commands, and results shall use
named `TYPE` records or arrays when that makes a substantial interface safer
and easier to understand. Each such datatype shall be introduced in its source
file with its physical meaning and intended use. Datatypes shall not be added
to wrap isolated scalar values or make a small lesson ceremonial. A learner
shall be able to follow the same progression used in a real project: defined
domain data, cohesive function blocks, top-level composition, physical I/O,
and explicit safety authority.

Comments shall explain control reasoning that is not evident from the
statements themselves. Relevant examples include why only the first predicted
move is applied, why an estimator update is gated, why a transfer tracks the
applied actuator, or why a constraint is checked before an objective is
ranked. Comments shall not restate assignments, narrate every line, duplicate
the README, or compensate for unclear names and oversized routines. Every
extracted teaching component shall have a short introductory comment stating
what it represents, what it is used for in the named equipment, and its
important validity or safety boundary. Additional comments appear only at
decisions whose engineering reason would otherwise be unclear.

The executable lesson tests shall compile every shipped source file together
and prove the same observable controller, constraint, fallback, and recovery
behavior before and after a structure-only refactor. A source-layout check may
enforce this ownership contract, but it does not replace execution of the
lesson.

### 2.1.4 Mathematical presentation

Mathematical notation shall make an already-explained physical relationship
easier to read; it shall never be the learner's first explanation. Before an
equation appears, the lesson shall explain the relationship in ordinary words
and introduce every quantity in the form `ordinary-language name (symbol)`,
including its physical meaning and unit. A short sentence after the equation
shall translate the result or relationship back into the named equipment.

Lesson READMEs shall use the mathematical notation rendered natively by the VS
Code and GitHub Markdown previews. Inline relationships use `$...$`. A
standalone relationship shall use its own paragraph containing one `$...$`
expression so that it remains left aligned with the explanation that introduces
it. A derivation that is too long for one readable line shall use successive
left-aligned calculation paragraphs rather than a centered display block or a
horizontal-scroll code block. Equations shall use conventional typesetting
where it improves meaning, including `\frac{}` for fractions,
braced subscripts and superscripts, `\mathbf{}` for vectors and matrices,
`\begin{bmatrix}...\end{bmatrix}` for matrices, and `\text{}` or
`\operatorname{}` for textual labels. ASCII matrix forms such as `[[...]]` and
backtick code spans used only to imitate mathematical typesetting are not
learner-facing equations.

Code fences and backticks remain mandatory for literal Structured Text source,
identifiers, commands, paths, and exact machine output. Every learner-facing
Structured Text fence shall use the shipped `st` Markdown language alias. The
excerpt shall come from the actual lesson program and the real VS Code preview
shall visibly distinguish syntax classes such as code tokens, comments, and
literals with theme-aware colours; `iecst`, `structured-text`, an unlabelled
fence, or a visually monochrome substitute is nonconforming. A numerical
results table may remain ordinary Markdown when that is clearer than
mathematical notation. The course shall not add a private equation renderer,
remote font, image-only formula, or duplicated prose-versus-math document path.

Readable notation does not replace accessibility. The prose surrounding an
equation shall contain the relationship and engineering interpretation needed
to understand it without relying on the visual rendering alone. Figures and
equations shall continue to obey the words-before-symbols rule in §2.1 and the
teaching-figure requirements in §2.1.1.

### 2.1.5 Reusable practical-lesson conformance pattern

The corrected water-heater proportional-control lesson establishes the
minimum pattern for every remaining practical lesson. It is a reference for
structure and evidential discipline, not a template whose plant-specific
conclusions may be copied.

This requirement applies to every existing and future practical lesson. An
older lesson is not accepted merely because its executable project runs; it
shall be revised when its teaching flow, terminology, measures, evidence, or
conclusion does not conform to this pattern. More advanced subject matter may
require additional explanation and validation, but it may not omit these
foundations.

The learner-facing flow shall contain these connected stages in this order:

`The problem -> What we need -> The new idea -> Prediction -> Implementation -> Experiment in Control Studio -> Evidence and decision -> Conclusion -> Reason for the next lesson`

These are required README headings, not suggestions. A short lesson may keep a
section concise, but it shall not omit, rename, merge, or reorder a stage. An
advanced lesson may add subsections inside a canonical section when needed,
but additional headings shall not obscure this nine-stage spine.

1. **One explicit learning objective.** State the practical decision the
   learner will be able to make and the physical limitation they will be able
   to explain. Do not combine several unrelated techniques in one lesson.
2. **A reason inherited from prior evidence.** Show the measured failure or
   limitation from the prerequisite lesson that creates the need for the new
   technique. A technique shall not appear merely because it belongs in a
   control-theory syllabus.
3. **Physical meaning before notation.** Introduce the equipment and each new
   quantity in complete words before its symbol, equation, plot label, table
   heading, or code name appears. The ordinary name and symbol shall be joined
   at first use in the explanatory sentence, for example setpoint (SP), process
   value (PV), and error (e); a symbol list after an unexplained equation does
   not satisfy this requirement. Define those quantities in the equipment's
   units before writing `e = SP - PV`.
4. **An unambiguous implemented law.** Explain whether a controller is
   positional, incremental, continuous, or discrete; define the sign
   convention, bias, sample-time use, limits, and one-sided or bidirectional
   actuator behavior that affect its meaning. Distinguish controller request
   from applied plant input whenever they can differ. Parameter descriptions
   shall match the implemented form: for example, a positional proportional
   gain states how much output is requested per unit of error, not how much the
   previous output "changes." Any simplified relationship, such as a
   proportional-band equation, shall state its configuration assumptions next
   to the relationship.
5. **A physical relationship and worked prediction.** Explain the applicable
   equilibrium, conservation, or dynamic relationship in words, introduce all
   quantities and units, then present the equation and calculate at least one
   expected result before the run. When the lesson explains an equilibrium
   offset, solve the relationship for that offset and show how the required
   steady plant input creates it; describing the final curve is not enough.
   The prediction shall be concrete enough to compare with retained evidence.
6. **The real controller under test.** Show the shortest useful excerpt from
   the shipped Structured Text program that implements the explained law. The
   learner shall be able to trace the preceding quantities from the equation
   into the actual configuration, invocation, requested output, applied output,
   and plant input.
7. **A controlled comparison.** Declare the independent variable and hold the
   plant, initial state, disturbance, sample schedule, duration, limits, and
   random seed equal unless one of those is intentionally under study. Record
   deviations instead of silently weakening comparability. Every setting that
   the learner is told to change shall be connected to the running program. If
   a setting is sampled only during initialization, the instructions shall say
   that a restore or restart is required; a live-mapped setting shall take
   effect at the explicitly declared scan boundary.
8. **Decision measures defined before results.** For every number in the
   comparison table, state in words what it measures and then give its exact
   calculation, source signal, unit, evaluation window, settling prerequisite,
   preferred direction, acceptance threshold, and numerical decision
   tolerance. The displayed name shall disclose the operation when ambiguity
   is possible: for example, use `cumulative applied-output movement` or
   `post-disturbance process-value peak-to-peak excursion`, not the unexplained
   labels `heater movement` or `temperature movement`. The lesson shall explain
   how that tolerance affects a boundary decision before showing any result.
   An upper-bound criterion passes only when the unrounded measured value is no
   greater than the upper threshold plus its declared tolerance. A lower-bound
   criterion passes only when the unrounded measured value is no less than the
   lower threshold minus its declared tolerance. Display rounding shall never
   decide acceptance. When no defensible physical acceptance threshold has
   been established, say so before presenting the result; lower or higher being
   preferred does not by itself make a result acceptable. Cumulative movement
   shall be identified as total variation and shall use cumulative units such
   as percentage points rather than an instantaneous percent command. A
   requested-output measure and an applied-output measure are not
   interchangeable and shall be named separately whenever saturation or
   another actuator effect can separate them.
9. **Evidence-qualified results.** Generate tables and plots from the shipped
   executable run. Report `Not settled`, `Saturated`, `Invalid`, or another
   truthful state when a measure's prerequisite is not met. Never rename a
   final sample as steady-state error or use the phase of an oscillation to
   support an equilibrium claim. A results table shall distinguish a failed
   criterion from a measure that is not valid for that run. It shall also keep
   three decisions distinct: whether a value is numerically valid, whether the
   run satisfies a declared physical acceptance criterion, and whether the
   eligible case is selected from the comparison. A lesson shall not label a
   result physically acceptable when it declared no physical acceptance
   threshold. Every reported measure, eligibility result, ranking, and
   selection shall be reproducible from the retained samples and the
   predeclared decision contract. The evidence generator shall calculate these
   conclusions from that contract; it shall not hard-code which case is
   eligible or selected. A cumulative measure shall include every retained
   transition in its declared window, including mode or shutdown transitions
   when the lesson says it measures the complete run.
10. **A bounded engineering conclusion.** Explain the observed result using the
    preceding physical relationship, state the plant class and assumptions for
    which the conclusion holds, identify an important counterexample when one
    exists, and finish with the still-unresolved limitation that motivates the
    next lesson. The next technique shall therefore answer evidence produced by
    this lesson rather than appear as an unrelated syllabus topic.

The nine sections have the following required teaching roles:

- **The problem** starts with the real equipment and a concrete observed
  problem, not with an equation or technique name. It identifies the actuator,
  sensor, controlled and manipulated quantities, disturbance, units, operating
  point, safety boundary, preceding solution, and evidence of its limitation.
  The learner shall understand what is wrong before being shown the remedy.
- **What we need** converts the observed problem into a bounded engineering
  requirement. It states what behavior must improve and what existing safety,
  actuator, quality, or performance properties must be preserved. It does not
  reveal a result or ask the learner to accept the new technique on authority.
- **The new idea** explains the remedy first through the named plant's physical
  cause and effect. The first use of every symbol pairs the complete name with
  the symbol, such as setpoint (SP), process value (PV), error (e), requested
  manipulated variable (MV_requested), and applied manipulated variable
  (MV_applied). Only then may equations, diagrams, controller forms,
  assumptions, limits, and counterexamples appear.
- **Prediction** derives at least one numerical or unambiguous directional
  expectation before the practical run. It states assumptions and units and
  records what observation would support or contradict the new idea. Where an
  equilibrium explains the problem, prediction connects required plant input
  to the controller's required error or state.
- **Implementation** shows the smallest useful excerpt from the actual shipped
  Structured Text. It explains production structure where relevant: data
  types, function blocks, initialization, configuration, modes, sample time,
  limits, requested output, applied output, safe behavior, and the plant-facing
  I/O path. Pseudocode or an alternative implementation does not conform.
- **Experiment in Control Studio** gives one executable journey using exact
  visible names and actions. It states the hypothesis, prerequisite runtime
  state, signals, independent and controlled variables, entered values and
  units, initial state, timing, duration, seed, limits, approval, abort and
  restoration behavior. It tells the learner exactly what to select or press,
  why, what should become visible, and how to recover from each applicable
  blocked or failed state. It runs the prior solution or baseline and the new
  technique under equivalent conditions. Initialization-only settings require
  an explicit restart instruction; live settings state their scan boundary.
- **Evidence and decision** defines every measure before showing results: its
  ordinary-language meaning, calculation, source signal, unit, time window,
  sample treatment, validity prerequisite, desired direction, threshold,
  tolerance, and display precision. It presents generated evidence from the
  shipped executable, marks invalid or unsettled measures truthfully, separates
  requested from applied actuation, and then applies acceptance criteria before
  ranking or selecting eligible cases. The learner records the engineering
  decision and its evidence.
- **Conclusion** explains why the evidence agreed or disagreed with prediction
  using plant physics and the implemented law. It states what the learner can
  now decide, together with the plant class, operating region, assumptions, and
  counterexample or boundary that limit the conclusion.
- **Reason for the next lesson** finishes with a limitation actually exposed by
  this lesson's evidence. It names the next required capability in ordinary
  language and explains why another technique is needed, without teaching that
  technique prematurely.

The amount of material shall be governed by the objective. A lesson shall
teach the theory required to predict, perform, and explain its experiment, but
shall move unrelated derivations, alternative methods, and later-course theory
to the lesson where evidence creates a need for them. Completeness does not
permit an encyclopedic detour that obscures the single practical decision.

The water-heater proportional-control lesson is the course's pedagogical
reference implementation. It shall pass this section's complete conformance
contract before another lesson may cite it as precedent. Subsequent lessons
shall copy its reasoning discipline, not its wording, equations, measures, or
conclusions: each lesson shall select the physical relationship, prediction,
decision measures, validity rules, and evidence appropriate to its own plant
and objective.

Course-wide review shall apply the pattern lesson by lesson. A practical
lesson is accepted only when one continuous review can trace all of the
following without filling in an unstated assumption:

1. the prior lesson's retained limitation into the present lesson's reason;
2. every physical quantity from ordinary words and units into its symbol;
3. the governing relationship into a numerical prediction;
4. the explained controller into the actual Structured Text that executes;
5. the experiment settings into retained, reproducible run evidence;
6. each reported number into a defined calculation and validity condition;
7. the eligible evidence into the engineering decision; and
8. the remaining limitation into the next lesson's reason.

Finding one broken trace rejects the lesson even when its program compiles or
its headings are present. The lesson shall be corrected at the first broken
link rather than adding prose after the conclusion to disguise the gap.

#### Reference-lesson propagation and rendered review

The water-heater proportional-control review is not local editorial feedback.
Each class of defect found there shall be treated as a course-wide defect class
and checked wherever it can occur. In particular, every applicable lesson
shall be reviewed for undefined measures, invalid equilibrium claims,
conflated requested and applied outputs, ambiguous controller form, conclusions
stated outside the applicable plant class, missing physical relationships,
unexplained dynamic mechanisms, equations without configuration assumptions,
idealized actuators that contradict the named equipment, and theory that cannot
be traced into the Structured Text that actually runs. Review shall also reject
results shown before their decision contract, rankings that include ineligible
runs, and claims of physical acceptance when only numerical validity or a
preferred direction has been established.

The Lesson 8 amplitude-saturation review establishes additional presentation
defect classes for all 84 lesson READMEs. Course-wide review shall reject a
lesson that begins with a technique instead of the observable equipment
problem, repeats theory in multiple workflow sections, substitutes generic tool
instructions for one exact Control Studio path, centers standalone equations
away from their introducing prose, uses Mermaid that renders as an empty box,
or shows Structured Text without the shipped `st` syntax highlighting. These
rules apply even when the underlying program and retained evidence are already
correct.

When review of the reference lesson reveals a reusable teaching defect, the
curriculum shall be corrected in this order:

1. express the defect as a plant-independent requirement in this specification;
2. correct the reference lesson and prove the requirement with its executable
   lesson evidence;
3. add the requirement to the implementation checklist for every applicable
   unfinished and previously completed lesson; and
4. accept each affected lesson only after its own plant-specific explanation,
   Structured Text, experiment, measures, generated figures, and conclusion
   satisfy the requirement.

A reviewer comment, a correction in only the reference README, or a generic
statement that later lessons should be "similar" does not complete this
propagation. Conversely, a plant-specific observation shall not be promoted to
a course-wide rule unless its reusable defect and applicability are stated.

A correction to the reference lesson shall trigger an audit of all unfinished
lessons and all previously implemented lessons to which the correction applies.
The other lessons shall adopt the reasoning and evidence rule, not copy the
water heater's words, equations, thresholds, or results. Their plant physics,
controller structure, prediction, measures, and acceptance criteria shall be
chosen for their own engineering objective.

Each practical lesson's acceptance evidence shall contain a lesson-specific
conformance record for the following reusable rules. A rule may be marked not
applicable only with a short physical or mathematical reason; an unchecked
rule, a copied generic statement, or "same as the reference lesson" is not a
disposition.

| Reusable rule | Required lesson-specific proof |
| --- | --- |
| Words before symbols | Every new quantity is introduced at first use as its ordinary physical name followed by its symbol in parentheses, with its unit and role stated before the symbol appears in an equation, code excerpt, plot, or table. |
| Dimensional consistency | Every equation and derived measure uses compatible units; the lesson states the resulting unit in words and checks any conversion, normalization, percentage, percentage-point, rate, integral, or cumulative quantity that could otherwise be misread. |
| Decision measures | Every reported measure is defined before results by its calculation, source signal, unit, window, validity prerequisite, preferred direction, and physical acceptance threshold or an explicit statement that none is established. Every threshold has a declared numerical tolerance and bound direction; acceptance uses unrounded retained values while rounding is presentation only. |
| Reconstructible decisions | Every measure, eligibility result, ranking, and selection can be recalculated from retained samples and the predeclared decision contract; no selected case is hard-coded, and each declared measurement window includes all of its boundary transitions. |
| Equilibrium qualification | Settling is tested before an equilibrium measure is calculated; an ineligible run reports `Not settled`, `Unstable`, `Aborted`, or the applicable state instead of a misleading number. |
| Requested and applied actuation | The controller calculation and plant input are separately named, retained, plotted, and assessed whenever limits or actuator behavior can separate them. |
| Implemented controller form | The prose states positional or incremental form, action sign, bias, sample-time use, derivative location, and discretization wherever each choice affects the implemented law. |
| Plant-specific scope | The conclusion names the plant class, operating region, actuator assumptions, and a relevant counterexample or boundary instead of presenting a local observation as a universal rule. |
| Physical relationship | Words introduce the equilibrium, conservation, or dynamic relationship and all quantities before the equation; a numerical prediction is compared with executable evidence. |
| Equilibrium explanation | When equilibrium offset is part of the lesson, the governing balance is solved for the offset in words and symbols, its assumptions are stated, and the predicted direction or value is checked against a settled eligible run. |
| Physical actuator | Direction, range, rate, deadband, interlocks, and other relevant nonidealities agree with the named real equipment and with the simulation. |
| Dynamic mechanism | The explanation identifies the physical mechanism behind the observed response, such as lag, delay, integration, coupling, friction, or noise, rather than merely describing the curve. |
| Executable trace | The explained law is traceable through a concise excerpt of the actual Structured Text into configuration, signals, requested output, applied output, and plant input. |
| Controlled comparison | The independent variable is explicit; the plant, initial state, events, timing, limits, and stochastic seed remain equivalent unless one is deliberately studied. |
| Vocabulary progression | New terms are defined before use, terminology matches later lessons, and the unresolved measured limitation creates the reason for the next lesson. |
| Rendered comprehension | At normal reading width, equations, code, tables, and generated figures form one readable causal argument and do not require the reviewer to infer a missing definition or decision. |

This conformance record is an acceptance aid, not a substitute for the lesson
itself. Each proof shall point to the lesson's own README section, actual
Structured Text, retained run artifact, generated figure, native assertion, or
rendered review evidence. Presence of headings or source phrases alone does not
prove conformance.

Automated conformance is necessary but cannot establish that a lesson is clear.
Before a practical lesson is accepted, its README shall be rendered in the
learner-facing surface at a normal reading width and reviewed as one continuous
lesson. The review shall confirm that:

1. the objective is apparent before technical detail begins;
2. no term, symbol, table, plot, or instruction relies on knowledge outside the
   declared prerequisites;
3. figures clarify the physical mechanism or engineering decision and remain
   readable with their real labels, units, legend, and caption;
4. the theory, actual Structured Text, experiment, retained evidence, measures,
   and conclusion tell the same causal story;
5. a learner can explain both why the selected result is acceptable and why an
   ineligible result cannot be ranked; and
6. the remaining measured limitation makes the purpose of the next lesson
   understandable without previewing unexplained theory;
7. standalone equations remain left aligned with their introducing prose and
   do not create avoidable horizontal scrolling;
8. every required diagram renders as visible teaching content rather than an
   empty placeholder; and
9. every Structured Text excerpt uses the `st` fence and visibly renders code
   tokens, comments, and literals with theme-aware syntax colours.

A source-text assertion, heading count, word count, successful compilation, or
unreviewed screenshot shall not satisfy this rendered pedagogical review. If
the rendered lesson is crowded, repetitive, visually ambiguous, or requires
the reviewer to infer a missing explanation, the lesson remains incomplete.

The progression between consecutive lessons is an executable teaching
contract:

1. the earlier lesson's Validation stage shall retain the evidence for an
   unresolved limitation;
2. its Transfer stage shall name that limitation without pretending that the
   next technique has already solved it;
3. the next lesson's Reason stage shall refer to the same limitation and the
   same retained evidence;
4. the next lesson's Prediction shall state how the new technique is expected
   to change the previously measured result; and
5. its Comparison and Validation stages shall test that prediction under
   equivalent conditions.

A lesson therefore does not conform merely because all eleven headings are
present. The headings shall form one causal argument from a physical problem
to an evidence-backed engineering decision.

Where several cases are compared, the results presentation shall distinguish
eligibility from performance. Each row shall first state its run status, such
as `Settled`, `Not settled`, `Saturated`, `Aborted`, or `Invalid`. A metric cell
whose prerequisite is not met shall contain the applicable status rather than
a misleading number. Only eligible rows may be ranked, selected, or described
as better by that metric.

The pattern is a conformance contract, not merely editorial guidance. For each
implemented practical lesson:

- a native executable test shall prove the controller law, signal identities,
  important constraints, and the numerical result on which the conclusion
  depends;
- a lesson-schema or rendered-interaction test shall prove that the required
  teaching stages, Control Studio workflow, and executable project are
  reachable by the learner;
- retained run data shall be the source for reported measures, tables, and
  plots, and the metric calculation shall be reproducible from that data; and
- review shall reject a lesson when its prose, Structured Text, run artifact,
  figure, table, or conclusion disagrees with another of those surfaces.

Review shall evaluate the lesson as one continuous learner journey, not as a
set of individually present headings. A lesson is nonconforming when the
learner cannot answer all of these questions after completing it:

1. What real equipment is being controlled, and what are its relevant physical
   limits?
2. What measured problem from the previous solution makes this lesson
   necessary?
3. What does every new quantity mean in words, and what unit does it use?
4. How does the explained relationship map into the Structured Text that
   actually runs?
5. What result was predicted before the experiment, and why?
6. Which single intentional change is being compared while other conditions
   remain equivalent?
7. How is every reported measure calculated, and when is that measure valid?
8. What do the retained data and generated plots actually prove?
9. Under which plant class and assumptions is the conclusion valid?
10. What remaining measured limitation creates the reason for the next lesson?

The lesson descriptor, README, Structured Text, native assertions, retained
run artifact, generated figures, Control Studio presentation, and later video
shall express the same objective, variable identities, controller form,
experiment conditions, measures, results, and conclusion. A discrepancy is a
lesson defect; prose or metadata shall not override executable evidence.

#### Per-lesson teaching and evidence record

Every practical lesson shall have one lesson-specific teaching and evidence
record. This record is the repeatable review pattern derived from the
water-heater proportional-control lesson. It shall be completed from the
lesson's own plant, program, experiment, and retained results; copying an
answer from another lesson is nonconforming.

| Record part | Required content |
| --- | --- |
| Learning spine | One objective; the prerequisite lesson and its measured unresolved limitation; the new decision this lesson enables; and the limitation intentionally left for the next lesson. |
| Physical dictionary | The real equipment, actuator, sensor, controlled quantity, manipulated quantity, disturbance, operating point, limits, units, and first-use definitions in the form complete words followed by symbols, for example setpoint (SP), process value (PV), and error (e). |
| Controller or method contract | The implemented law or analysis method in words first; its form, action sign, parameters, assumptions, sample-time treatment, bias, limits, and actuator behavior; then its equations and a concise excerpt of the actual shipped Structured Text. |
| Prediction contract | The physical relationship that explains the expected behavior, all quantities and assumptions introduced before the equation, and at least one numerical prediction made before the run. |
| Experiment contract | Hypothesis, independent variable, controlled variables, initial state, event schedule, evaluation windows, sample interval, duration, seed when applicable, limits, abort conditions, safe restoration, and exact Control Studio actions. |
| Metric contract | For every reported measure: ordinary-language meaning, exact calculation, source signal, unit, evaluation window, sample treatment, validity prerequisite, preferred direction, and physical acceptance threshold or an explicit statement that none is established. For every criterion: bound direction, numerical tolerance, the decision rule applied to the unrounded value, and display precision. |
| Result qualification | For every case: run status, metric eligibility, requested and applied actuation where they can differ, physical-criterion result, and selection result. `Not settled`, `Saturated`, `Aborted`, `Invalid`, and similar states replace numbers whose prerequisites failed. |
| Reproducibility chain | The executable run identity, plant and controller settings, sample interval, seed, limits, events, evidence class, complete sampled signals, metric summary, figure-generation source, and links from every table and plot back to the retained data. |
| Explanation and transfer | Reconcile prediction with evidence using the physical mechanism; state the applicable plant class, operating region, assumptions, and counterexample or boundary; then connect the remaining measured limitation to the next lesson. |

The README shall present this material as the continuous nine-stage learner
journey rather than as a compliance form. The record belongs in acceptance
evidence and shall let a reviewer verify the journey without guessing how a
number, conclusion, or instruction was obtained.

The following reference-lesson findings are mandatory review questions for
every lesson where they apply:

1. Is each comparison measure mathematically defined before its first result,
   with a unit that matches the calculation?
2. Is equilibrium performance withheld unless the declared settling rule
   passes, using a declared window statistic instead of an arbitrary final
   sample?
3. Are controller-requested and physically applied outputs separate throughout
   the explanation, code, evidence, plots, and decision whenever they can
   differ?
4. Does each parameter description match the actual controller form rather
   than suggesting a different positional, incremental, continuous, or
   discrete implementation?
5. Is the conclusion bounded to the applicable plant class and operating
   assumptions rather than taught as a universal controller rule?
6. Does a physical equilibrium, conservation, or dynamic relationship explain
   the observed result and support a worked prediction?
7. Does the explanation identify the mechanism behind poor behavior, such as
   lag, delay, integration, noise, friction, nonlinearity, or coupling?
8. Are simplified equations accompanied by the configuration assumptions that
   make them valid?
9. Does the actuator model match what the named real equipment can actually do?
10. Can the learner trace the theory into the actual Structured Text, then into
    the retained run, calculated measures, generated figures, and engineering
    decision?
11. Are numerical tolerances declared before results, applied to the unrounded
    retained values in the stated bound direction, and kept separate from
    display rounding?

A `yes` answer requires lesson-specific proof. A `not applicable` answer
requires a physical or mathematical reason. Any other answer leaves the lesson
incomplete.

### 2.2 Continuous Control Studio workflow

Every practical lesson shall use the applicable stages of one consistent
workflow:

- **Monitor** verifies SP, PV, requested MV, applied MV, units, operating mode,
  safety state, signal age, and quality before an experiment.
- **Test** states the engineering question, performs a bounded excitation, and
  retains the complete conditions and time-series evidence.
- **Identify** is used only after the lesson has explained why a model is
  required. It shall fit candidate models, expose uncertainty, and allow the
  result to be rejected.
- **Tune / Design** shall select the simplest eligible controller or design
  method and explain every choice rather than merely producing parameters.
- **Validate** shall test setpoint response, disturbance response, noise,
  actuator constraints, stability, robustness, and abnormal cases applicable
  to the lesson.
- **Apply** requires explicit approval, an atomic parameter update, bumpless
  transfer where possible, post-application monitoring, and a rollback path.

Every lesson README shall contain the canonical section titled `Experiment in
Control Studio` after `Implementation` and before `Evidence and decision`. It
shall list only the stages declared by that lesson's `workflowStages` contract,
numbered consecutively in the same order. Each listed stage shall tell the
learner:

1. the complete visible navigation path beginning with `truST sidebar` →
   `Control Studio` and the exact stage tab to select;
2. the exact button, menu item, disclosure, selector, and field labels as they
   appear in the shipped extension; a source-code selector or an obsolete label
   is not an acceptable substitute;
3. which lesson-specific signals, experiment settings, model structures,
   controller candidates, requirements, or constraints to inspect or enter,
   including every entered value and engineering unit;
4. why each selected option or entered value is appropriate for the lesson's
   engineering question;
5. the visible prerequisite state that enables the next action and, when an
   action is disabled, the reason shown by Control Studio and the exact action
   that resolves it;
6. what numerical prediction or safety decision the learner shall record before
   selecting `Review test`, `Confirm and start`, `Generate candidate`, `Apply`,
   or another consequential action;
7. the running state, terminal state, named plot or result, and precise visible
   observation that demonstrate successful completion;
8. the exact recovery action for every applicable rejected, aborted, stale,
   unsafe, unavailable, or failed state; and
9. the engineering conclusion the learner records before continuing.

Every lesson shall use the same visible procedure shape established by Lesson
8. After a short lesson-specific experiment introduction, the section shall
contain exactly one `### Control Studio actions` heading. Each declared stage
shall then be one numbered item in the form `N. **Stage** — <exact visible
navigation and first action>`, followed by these five labelled bullets in this
exact order:

1. `**Baseline check:**` states the visible safe starting state, signal quality,
   operating point, and prerequisites that must be true before acting.
2. `**Decision before action:**` states the prediction, engineering reason,
   safety decision, or candidate choice that the learner must make before the
   consequential control is selected.
3. `**Successful evidence:**` names the exact control to select and the visible
   running state, terminal state, plot, value, or result that proves the action
   succeeded.
4. `**Recovery:**` gives the exact action for disabled, rejected, stale,
   unavailable, unsafe, aborted, or failed states and says when the learner may
   retry.
5. `**Record:**` states the observation and engineering conclusion to retain
   before continuing.

The former `### Preparation`, `### Baseline`, `### Change`, `### Comparison`,
and `### Validation` workflow subsections are forbidden. Their useful
lesson-specific content shall be taught once inside the applicable five-part
stage item; it shall not be repeated before or after `Control Studio actions`.
Additional workflow prose may clarify the declared experiment as a whole but
shall not create a second set of instructions or a second route through the
tool.

The section shall use the lesson's declared plant, signals, units, experiment,
limits, seed, restoration behavior, acceptance criteria, and evidence class.
It shall retain every applicable baseline, changed-case, comparison, and
validation instruction inside the corresponding five-part stage item rather
than linking to an alternative workflow or repeating theory. Generic
instructions such as
`open Test`, `run the experiment`, `inspect the chart`, `identify the model`,
`tune the controller`, or `validate the result` without the exact visible
control, lesson-specific settings, reason, expected state, recovery, and
decision are nonconforming. A generated workflow paragraph is a navigation
draft until its named controls and resulting states have been exercised in the
real Extension Development Host.

The README workflow does not make a lesson Control Studio-compatible by
itself. The shipped project shall also provide a real typed loop binding, or a
single specified shared derivation path that produces the equivalent binding,
and the real Extension Development Host shall execute every declared stage
against the lesson's runtime data. A synthetic message, substituted loop, or
metadata-only assertion cannot establish this behavior.

A physical-plant test shall require independently reviewed I/O topology and
explicit approval immediately before excitation or application. An automatic
workflow shall never silently apply controller parameters.

### 2.3 Evidence and completion

A lesson is complete only when the learner can:

- explain why the preceding solution was inadequate;
- explain the new theory without relying on unexplained controls or formulas;
- predict the important result before running the experiment;
- execute the actual method on the declared plant;
- distinguish configured plant truth from an identified model;
- compare baseline and changed results from equivalent conditions;
- decide whether the result satisfies declared engineering requirements; and
- explain the technique's applicability, limitations, and next unresolved
  problem.

Displayed numerical precision shall reflect sensor resolution, engineering
tolerance, and the decision being made. Raw floating-point precision shall not
be presented as physical accuracy. Acceptance and ranking shall nevertheless
use the unrounded retained values and the predeclared tolerance rule, never the
rounded text shown in a README, table, plot label, or Control Studio field.

## 3. Curriculum sequence

The lesson clauses in this section describe required content and executable
behavior; labels such as `Context`, `Reason`, `Theory`, `Controller under test`,
`Comparison`, or `Validation` are specification-field descriptions, not
learner-facing README headings. Every lesson shall place that content under the
canonical nine headings in §2.1. In particular, the real equipment and prior
failure belong in `The problem`; the requirement belongs in `What we need`;
theory belongs in `The new idea`; controller code belongs in `Implementation`;
all practical actions belong in `Experiment in Control Studio`; comparison and
validation belong in `Evidence and decision`; explanation and applicability
belong in `Conclusion`; and the remaining limitation belongs in `Reason for the
next lesson`.

### 3.1 Feedback foundations and the temperature-control spine

#### Lesson 1 — Understand a control loop: cold-storage room

- **Reason:** A learner must understand what is being controlled before any
  controller is introduced.
- **Theory before practice:** Process, actuator, sensor, SP, PV, MV, control
  error, load disturbance, feedback, direct and reverse action, engineering
  units, operating point, normal limits, safe state, and the distinction
  between control and an independent safety system. The lesson shall explain
  why error is defined as `SP - PV`, calculate the declared `2.0 - 4.0 = -2.0
  degC` example, and translate the negative sign back to the room being warmer
  than desired. It shall derive the requested-to-applied command clamp from the
  below-range, in-range, and above-range physical cases before using its compact
  formula, then work the exact `150% requested -> 100% applied` example.
- **Practical flow:** Monitor room temperature, compressor command, door state,
  and effective surrounding temperature. In Test, select the writable operator
  request, a `150%` Manual output step, and `20 s`; review it, confirm it, watch
  the separately observed applied demand stop at `100%`, and prove restoration
  to `0%`. Then restart from the declared state and use Monitor to observe the
  uncontrolled door event from `60` through `90 s` in a separate `120 s`
  recording. Lesson 1 shall expose only Monitor and Test; it shall not direct a
  beginner to disabled shared PID Identify, Tune, or Validate controls.
- **Required conclusion:** The learner can identify every loop element and
  explain why continuous observation alone does not regulate temperature.

#### Lesson 2 — ON/OFF control: cold-storage room

- **Reason:** The room must regulate temperature automatically using a
  compressor that is either on or off.
- **Theory before practice:** Boolean control law, switching threshold, control
  direction, steady cycling, switching frequency, and why ON/OFF control does
  not converge to an exact constant temperature. Define `PV`, `SP`, and binary
  `MV` in physical terms and engineering units before writing the Boolean law.
  Explain the cooling direction in words, then derive
  `RequestedCooling := ProcessValue >= Setpoint` from the fact that rising room
  temperature requires more refrigeration. Define maximum and minimum measured
  temperature over a declared window before deriving
  `PV_range = PV_max - PV_min`. Define the transition count and window duration
  before deriving `f_switch = N_switch / T_window`, including the dimensional
  result in transitions per second. Work a neutral numerical example before
  practice so every subtraction, division, and unit is visible without
  revealing the retained result before its validity and decision criteria.
  After those criteria, reproduce the retained result explicitly as
  `(398 - 78) / (60 s - 20 s) = 320 / 40 s = 8 transitions/s`. Every formula
  shall therefore be explained before the learner is asked to use it in
  Control Studio. The prediction shall distinguish eight samples from
  the seven transitions between them: the first sample establishes an output,
  and each later opposite-side sample creates one threshold crossing and one
  output transition. Retained range evidence shall show the unrounded maximum,
  minimum, and difference before explaining display rounding and the use of
  unrounded samples for decisions.
- **Practical flow:** In Monitor, establish the declared initial state, record
  SP, PV, requested/applied MV where available, mode, and the predicted
  switching direction. Restart the Simulator, then use Test with **Observe
  without excitation**, disabled zero amplitude, and a `120 s` duration. The
  reviewed project-scripted experiment authority is the writable Boolean
  `P1.AutoMode` at `%MX16.0`: Confirm and start shall set it true so the shipped
  controller and door schedule execute, and completion or abort shall restore
  it false. The learner shall review that authority and restoration before
  starting, observe the `60`-to-`90 s` door event, record temperature range and
  switch count, and reproduce switching frequency from retained samples.
  Lesson 2 shall expose only Monitor and Test; it shall not send the learner to
  disabled shared PID Identify, Tune, or Validate controls.
- **Required conclusion:** ON/OFF control regulates a band but a noisy threshold
  causes rapid switching and equipment wear.

#### Lesson 3 — Hysteresis and dwell protection: same cold-storage room

- **Reason:** Lesson 2 exposed chatter and excessive compressor starts.
- **Theory before practice:** Separate on/off thresholds, hysteresis width,
  limit cycle, minimum on-time, minimum off-time, state memory, and the trade-off
  between temperature variation and equipment protection. Distinguish the
  hysteresis-requested cooling state from the dwell-constrained applied
  compressor state; a dwell inhibit is an observable requested-versus-applied
  mismatch, not a hidden delay. Derive `H = T_on - T_off` from the distance
  between the boundaries. Explain that centering the band places `H/2` above
  and below SP before deriving `T_on = SP + H/2` and
  `T_off = SP - H/2`. State the inclusive upper, inclusive lower, and retained
  middle regions explicitly. Derive elapsed dwell as `t_dwell = n * Ts`; for
  `8.00 s` minimum dwell and `0.10 s` task interval, work
  `8.00 / 0.10 = 80` completed scans and explain why a reversal before scan 80
  is inhibited. Explain the separately qualified initial dwell.
- **Measures before practice:** Define the temperature extrema and derive
  `PV_range = PV_max - PV_min`. Define a per-sample applied-state-change
  indicator before deriving `N_switch = sum(I_switch[k])`, including ownership
  of the initial safe-off-to-on change. Derive completed on/off durations as
  `(k_end - k_start) * Ts` and explain why an unfinished final interval is not
  eligible. Define an inhibited-sample indicator from dwell inhibit plus a
  requested/applied mismatch, then derive `N_inhibit = sum(I_inhibit[k])` and
  explain why a nonzero result proves the dwell mechanism acted.
- **Practical flow:** Monitor the declared initial state and make the threshold,
  state-memory, and dwell prediction first. Restart the Simulator, then use
  Test with **Observe without excitation**, disabled zero amplitude, and a
  `120 s` duration. The reviewed project-scripted experiment authority is
  `P1.AutoMode` at `%MX44.0`; Confirm and start shall set it true to run the
  shipped `H = 1.00 degC`, `8.00 s` minimum-on, `8.00 s` minimum-off candidate,
  and completion or abort shall restore it false. Control Studio does not run
  the two other comparison candidates: the retained three-candidate artifact
  is produced by the named native evidence test from the same Structured Text,
  plant, initial state, door event, interval, duration, and seed. The lesson
  shall state that boundary rather than present retained evidence as a newly
  executed UI run. Record requested state, applied state, each dwell-inhibited
  mismatch, temperature response, and restoration. Apply the predeclared
  criteria before ranking the retained candidates. Lesson 3 shall expose only
  Monitor and Test; it shall not direct the learner to disabled shared PID
  Identify, Tune, or Validate controls.
- **Required conclusion:** Hysteresis and dwell time protect discrete equipment,
  but deliberate cycling remains; continuous modulation is needed when tighter
  regulation is required.

#### Lesson 4 — Proportional control without bias: water heater

- **Reason:** A modulating heater can vary continuously and should avoid the
  cold room's switching cycle.
- **Theory before practice:** Self-regulating thermal behavior, proportional
  action, gain `Kp`, sign of action, one-sided heating, zero-bias proportional
  band, requested versus applied heater output, closed-loop feedback, thermal
  lag and over-correction, plus the unsaturated equilibrium relationship
  between required holding output and steady-state error. Derive
  `PB = 100% / Kp` from the 0-to-100-percent request span, including unit
  cancellation from percent divided by percent per degree Celsius. Derive
  `e_ss = MV_required / Kp` by dividing both sides of
  `MV_required = Kp * e_ss`, and state its settled, unsaturated boundary.
  Before practice, derive cumulative PV and applied-output travel as the sum of
  absolute successive changes and explain why absolute values prevent reversal
  cancellation. Define final-window PV and MV ranges as maximum minus minimum,
  use them to qualify settling, and define the final-window mean absolute error
  only for a settled response. Declare validity, travel limits, and candidate
  ranking before retained results are visible.
- **Practical flow:** The shipped project shall start at `SP = 35 degC`,
  `Kp = 8 %/degC`, Auto disabled, and zero applied heat. Monitor SP, PV,
  requested heat, and applied heat and predict the initial `120%` request and
  `100%` clamp. In Test, select **Observe without excitation**, disabled zero
  amplitude, and `120 s`. Confirm and start shall write the Boolean
  `P1.AutoMode` authority at `%MX16.0`; completion or abort shall restore it
  false and restore zero applied heater output. The project owns the scheduled
  load event. Control Studio runs the selected gain-8 candidate only; the
  retained gain-2, gain-8, and gain-40 comparison is produced by the named
  native evidence test from identical conditions. The lesson shall state that
  boundary rather than claiming that Test changes gain. Lesson 4 shall expose
  only Monitor and Test and shall not direct the learner to disabled shared PID
  Identify, Tune, or Validate controls. The native comparison shall expose and
  retain both unclamped requested heater output and clamped applied heater
  output, compare only equivalent runs, qualify settling before reporting
  final-window mean absolute error, calculate cumulative post-disturbance PV
  and applied-output travel, and reject the intentionally aggressive case as
  `Not settled` rather than treating its last sample as equilibrium evidence.
- **Required conclusion:** For this zero-bias self-regulating heater, larger
  proportional gain reduces the error needed to request nonzero holding power,
  but cannot produce that power at exactly zero error and eventually reduces
  damping because the lagged temperature has not yet shown the full effect of
  earlier commands. This conclusion shall not be generalized to every plant
  class.

#### Lesson 5 — Operating-point bias: same water heater

- **Reason:** Lesson 4 selected proportional gain 8 percent per degree Celsius
  from the tested candidates, but the selected zero-bias controller still
  retained 6.9 degrees Celsius of changed-load steady-state error because zero
  error requested zero heater power. Lesson 5 shall retain that selected gain;
  silently returning to a weaker candidate is not an equivalent progression.
- **Theory before practice:** Introduce a steady operating point, heat entering
  and leaving the water, required steady heater output, controller bias `u0`,
  deviation variables, and why a P or PD controller needs an appropriate bias
  when its actuator output is nonzero at equilibrium. Explain in words before
  notation that the controller first requests the measured holding output and
  then adds proportional correction. Separately define requested heater output
  and the 0-to-100-percent applied heater output. Bias is one measured
  operating-point value; it is not integral action and cannot adapt itself to
  an unmeasured load change.
  Derive the biased proportional law from the steady heat balance: zero stored-
  energy change requires heater input to equal heat loss, so the measured
  required output becomes the zero-error request and proportional action adds
  correction around it. Show that the units reduce to percent. For a changed
  load, derive `MV_required,new = u0 + Kp e_ss` and then
  `e_ss = (MV_required,new - u0) / Kp`; explain the sign and physical meaning of
  every term rather than presenting either equation as a rule to memorize.
- **Worked prediction:** After the learner has measured the nominal holding
  output, predict that at 35 degrees Celsius the error and proportional
  correction are zero while the requested and applied heater outputs equal the
  measured bias. Predict the direction, but not an invented exact value, of the
  new equilibrium error after the cold-water load changes.
- **Practical flow:** Monitor shall first show the real safe initial state: SP
  35 degrees Celsius, PV 20 degrees Celsius, zero requested and applied output,
  measured bias 75 percent, retained gain 8 percent per degree Celsius, and
  Manual mode. It shall distinguish that initial state from the intended
  35-degree / 75-percent operating point. Control Studio Test shall run only the
  shipped measured-bias candidate by explicitly writing `P1.AutoMode` true for
  120 seconds and restoring it false. It shall not claim to sequence candidate
  trials that its reviewed activation cannot execute. The native evidence test
  shall execute bounded 70-, 75-, and 80-percent manual-output trials, qualify
  each trial, select the eligible output with the smallest final-window
  temperature error, and compare the Lesson 4 zero-bias controller with the same
  gain plus measured bias under identical initial state, plant, cold-water
  event, sample interval, duration, limits, and seed. This executable native
  measurement, not a simulator parameter or unexplained supplied constant,
  establishes the retained bias. Start disabled and finish with a zero-output
  manual restoration. Retain setpoint, process value, requested output, applied
  output, bias, gain, load event, mode, limits, sample time, seed, and evidence
  class. Every manual trial shall execute and retain its own final Manual-mode,
  zero-requested-output and zero-applied-output restoration sample; a separate
  startup or unit test is not evidence that the recorded trial was restored.
  The retained evidence shall also contain a per-window qualification record
  with the settling result, calculated process-value and output ranges, mean
  absolute error, eligibility, and selection disposition. These fields may be
  stored in the sample trace or in a linked summary artifact, but shall not
  exist only in README prose.
- **Decision measures:** Before any retained plot, table, or result, define the
  settling rule and final windows for the manual holding-output measurement,
  nominal controller interval, and changed-load interval. Define final-window
  mean absolute temperature error in degrees Celsius, requested and applied
  output ranges in percentage points, saturation eligibility, and the applied-
  output increase after the load. Declare the simulation criteria and state
  explicitly that they are not physical heater acceptance thresholds.
- **Required conclusion:** The measured bias removes nominal operating-point
  error for this self-regulating heater without weakening the previously
  selected proportional gain. An unmeasured load change alters the required
  holding power, so fixed bias plus P control still creates permanent offset;
  that retained offset is the reason to introduce integral action in Lesson 6.

#### Lesson 6 — Integral action in isolation: same water heater

- **Reason:** Lesson 5 measured a 75 percent nominal holding output and retained
  proportional gain 8 percent per degree Celsius, but the changed cold-water
  load still produced 1.15 degrees Celsius of settled mean absolute error. The
  required holding output changed while fixed bias could not. Lesson 6 shall
  therefore test a controller state that continues changing heater output
  while error persists and can retain the new holding output when error becomes
  zero.
- **Theory before practice:** Begin with the physical behavior: when the outlet
  remains too cold, heater power must continue increasing rather than remain a
  fixed multiple of the present error. Define the setpoint (SP), process value
  (PV), error (e), integral contribution (uI), integral gain (Ki), cycle time
  (dt), requested manipulated variable (MV_requested), and applied manipulated
  variable (MV_applied), with temperature in degrees Celsius, time in seconds,
  and heater output in percent. Only then present the discrete integral update
  `uI,new = uI,old + Ki * e * dt`. Derive it by treating one scan as an
  error-time rectangle with height `e` and width `dt`, multiplying its degree-
  Celsius-second area by `Ki`, checking that the units reduce to percentage
  points, and adding that increment to the previous stored output. For constant
  error, derive `uI_N = uI_0 + N Ki e dt` and then
  `uI(t) = uI_0 + Ki e t` from `t = N dt`; use this derivation to explain why
  constant error creates a ramp and why zero error preserves the stored value.
  Explain that `Ki * e * dt` is an increment in percentage points during one
  controller scan, not an instantaneous heater percentage. Explain in ordinary
  words that the stored contribution can hold
  the nonzero power required at zero error, which is why an integrator can
  reject a constant load disturbance. Also explain that an integral-only
  controller has no immediate output term proportional to the present error:
  it must accumulate its correction over time and adds phase lag, so offset
  removal does not by itself make its transient response preferable.
- **Controller under test:** Use the shipped `Control.PID` in positional
  integral-only configuration with proportional and derivative gains and bias
  set to zero. Establish the measured 75 percent holding output in bounded
  Manual operation and explicitly preload that observed value into the
  controller state when transferring to Auto; do not hide it as an unexplained
  initial state. Expose the integral-only requested heater output
  before limits and the applied 0-to-100-percent heater output separately.
  State the 100-millisecond cycle, positive action, initialization-only gain,
  one-sided heater, output limits, and the fact that no final-window saturation
  is eligible for the equilibrium comparison. The README shall show the short
  actual Structured Text path from the error and per-scan increment through
  `Controller.Update` to the plant input.
- **Worked prediction:** With integral gain 0.5 percent per degree Celsius-
  second, cycle time 0.1 second, and a temporary positive error of 2 degrees
  Celsius, explain first that one scan adds the gain multiplied by the error
  and elapsed time. Then calculate a 0.10-percentage-point requested-output
  increase per scan and 1.0 percentage point over ten scans if the error has
  not yet changed. Record this prediction before the run and compare it with
  the initial unsaturated executable increments after the load begins.
- **Practical flow:** Monitor shall first show the real safe shipped state: SP
  35 degrees Celsius, PV 20 degrees Celsius, zero requested and applied output,
  measured holding output 75 percent, integral gain 0.5 percent per degree
  Celsius-second, and Manual mode. Control Studio Test shall run only the
  shipped integral-only candidate by explicitly writing `P1.AutoMode` true for
  120 seconds and restoring it false. Its first scan shall preload the visible
  measured 75 percent inherited from Lesson 5 and then add the calculated
  integral increment; at the declared 15-degree error that unsaturated request
  is 75.75 percent. Test shall not claim to run the biased-P comparison that its
  activation cannot execute. The native evidence test shall command the
  measured 75 percent holding output in bounded Manual operation, verify that
  the 35-degree-Celsius operating point is settled, and run two equivalent
  cases from that qualified operating point: the Lesson 5
  proportional-gain-8 plus 75-percent-bias baseline and the integral-only
  controller with integral gain 0.5 percent per degree Celsius-second. Require
  explicit Auto enable, apply the same cold-water load at 60 seconds, sample
  every 0.1 second for 120 seconds, and preserve identical plant, initial
  state, event, limits, duration, and seed. Finish and retain a Manual-mode,
  zero-requested-output and zero-applied-output restoration sample for each
  case.
- **Decision measures:** Before results, define a settled final window as
  110.1 through 120.0 seconds with process-value range no greater than 0.1
  degrees Celsius, applied-output range no greater than 1.0 percentage point,
  and no persistent saturation. Define final-window mean absolute temperature
  error as the mean absolute difference between SP and PV in degrees Celsius,
  calculated only for a settled eligible window. Define early-load mean
  absolute temperature error over 60.1 through 75.0 seconds as the mean
  absolute difference between SP and PV; lower is preferred and no physical
  acceptance threshold is established. Define cumulative applied-output
  movement after the load as the sum of absolute changes between consecutive
  applied-output samples in percentage points. Define prediction error as the
  absolute difference between the predicted and executable requested-output
  increment during an unsaturated scan with the declared error. The simulation
  criterion for offset removal is final-window mean absolute error no greater
  than 0.2 degrees Celsius; it is not a physical-heater acceptance threshold.
- **Evidence:** Retain for both cases the setpoint, process value, error,
  integral contribution where applicable, requested output, applied output,
  controller structure, gains, bias or tracked holding output, mode, load
  event, cycle time, seed, limits, phase, and evidence class. Retain a linked
  qualification summary containing settling, ranges, saturation, early-load
  error, final-window error or truthful ineligibility state, cumulative applied
  movement, prediction error where applicable, simulation-criterion result,
  eligibility, and selection disposition. Generate the comparison table and
  response figure from these executable artifacts.
- **Required conclusion:** For this self-regulating heater near the measured
  operating point, the integral state can retain the changed holding power at
  zero error and therefore remove a constant-load offset. Integral-only action
  initially changes output only by accumulated increments and provides less
  prompt correction than the biased proportional controller; zero final error
  alone is not enough to select it. An integrating plant is an important
  boundary because its process dynamics already contain integration. Lesson 7
  shall combine the immediate proportional correction retained from Lesson 5
  with the offset-removing integral state demonstrated here.

#### Lesson 7 — PI control: same water heater

- **Reason:** Lesson 5's proportional-gain-8 plus measured-75-percent-bias
  controller responded promptly but retained `1.15 degrees Celsius` final-
  window mean absolute error after the cold-water load changed. Lesson 6's
  integral-only controller reduced the same measure to `0.02 degrees Celsius`,
  but its early-load mean absolute error increased from `1.07` to `1.62 degrees
  Celsius` and its cumulative applied-output movement increased from `9.24` to
  `17.38 percentage points`. Neither incomplete controller was selected.
  Lesson 7 shall combine the already demonstrated immediate proportional
  correction and stored integral contribution, then tune their balance with a
  bounded experiment rather than present an unexplained PI setting.
- **Theory before practice:** Recall the setpoint (SP) as desired outlet-water
  temperature, process value (PV) as measured outlet-water temperature, error
  (e) as `SP - PV`, proportional gain (Kp) as percentage-point requested output
  per degree Celsius of present error, integral gain (Ki) as percentage-point
  stored-output change per degree Celsius-second of error, integral
  contribution (uI) as stored heater output, cycle time (dt) in seconds,
  requested manipulated variable (MV_requested) as the preliminary controller
  request before physical limits, and applied manipulated variable
  (MV_applied) as the bounded command received by the heater. Introduce every
  quantity in those words and units before using its symbol.

  Explain first that parallel PI adds an immediate part determined by present
  error to a stored part determined by accumulated error. Only then present the
  conceptual relationship `MV_requested = Kp * e + uI` for zero bias and no
  feedforward. Derive the shipped incremental form rather than merely stating
  it: write the positional relationship for the current and preceding scans,
  subtract the preceding relationship from the current one, and substitute
  the Lesson 6 integral-state change `uI[k] - uI[k-1] = Ki * e[k] * dt`.
  Explain that `Control.PID` therefore realizes the same PI behavior
  incrementally when integral gain is nonzero: the proportional
  increment (delta_uP) is the proportional gain multiplied by the change in
  error, the integral increment (delta_uI) is the integral gain multiplied by
  present error and cycle time, and the preliminary next request is the prior
  controller output plus both increments. After defining all terms, write
  `delta_uP[k] = Kp * (e[k] - e[k-1])`,
  `delta_uI[k] = Ki * e[k] * dt`, and
  `MV_requested[k] = MV_previous[k-1] + delta_uP[k] + delta_uI[k]` for the
  unsaturated, constant-setpoint, zero-bias case used in the normal comparison.
  State that limits and anti-windup processing can make the internal retained
  state and applied output differ from this preliminary request; Lesson 8
  examines that constrained case.

  Define the integral time (Ti) in seconds as the time required for integral
  action to accumulate the same output contribution as proportional action
  would produce from an unchanged error. Equate the proportional contribution
  `Kp * e` with the integral contribution accumulated over Ti seconds,
  `Ki * e * Ti`; for nonzero error, cancel e and divide by Ki to derive
  `Ti = Kp / Ki`. Explain from the gain units why the quotient has units of
  seconds. At fixed Kp, increasing Ki shortens Ti and normally corrects
  constant error faster, but can increase overshoot, oscillation, saturation,
  and actuator movement. Explain that selecting the largest Ki is therefore
  not a tuning method.
- **Controller under test:** Use the shipped `Control.PID` with positive action,
  proportional setpoint weight one, derivative action disabled, zero bias,
  `0.1 second` cycle time, `0..100 percent` output limits, and the existing
  combined constraint handling. Keep proportional gain fixed at the Lesson 5
  selected value of `8 percent per degree Celsius`. Expose proportional
  increment, integral increment, preliminary requested heater output, and
  applied heater output separately. Begin in Manual at zero heater power.
  Establish the Lesson 5 measured `75 percent` holding output and qualified
  `35 degrees Celsius` operating point in bounded Manual operation, then
  preload that observed output on an explicit Manual-to-Auto transition. The
  proportional and integral gains are live-mapped Control Studio settings. At
  the beginning of every `0.1-second` scan, the program shall copy their current
  values into `PIDParameters` and call `Controller.Configure` without
  reinitializing controller state. A changed gain therefore affects both the
  exposed diagnostic increments and the real `Controller.Update` calculation
  in that same scan. The lesson shall state this adoption boundary explicitly;
  it shall not present a live diagnostic calculated with a gain that the
  running controller has not adopted.

  Changing a live gain does not restore the plant or controller state. Each
  independent tuning case shall still begin from the declared Manual holding
  condition and explicit Manual-to-Auto preload so the comparison remains
  reproducible. The
  README shall show the short actual Structured Text that configures the block,
  calculates the two increments, performs the preload, invokes
  `Controller.Update`, and maps the applied output to the plant.
- **Worked prediction:** Before running, use a `2 degrees Celsius` positive
  error that has not changed since the preceding scan, Kp `8 percent per degree
  Celsius`, Ki `4 percentage points per degree Celsius-second`, dt `0.1
  second`, and previous output `75 percent`. Explain first that unchanged error
  produces no new proportional increment while integral action adds output.
  Calculate `delta_uP = 0 percentage points`, `delta_uI = 0.8 percentage
  point`, and a preliminary request of `75.8 percent`. Then calculate the four
  declared integral times: Ki `0.5`, `2`, `4`, and `8` give Ti `16`, `4`, `2`,
  and `1 second`, respectively. Record which candidate is expected to recover
  fastest and which is most likely to overshoot, saturate, or create excessive
  movement before viewing a result.
- **Practical flow:** Use only simulation evidence. The executable learner
  journey has exactly `Monitor -> Test`; it shall not advertise Tune or
  Validate controls that do not execute this lesson's experiment. The shipped
  sample starts safely at setpoint 35 degrees Celsius, process value 20 degrees
  Celsius, Manual mode, and zero requested and applied heater output. In
  Control Studio Monitor, verify those values plus the visible measured
  75-percent holding output, proportional gain 8, integral gain 4,
  proportional and integral increments, Manual/Auto mode, quality, units,
  temperature boundary `0..80 degrees Celsius`, and heater boundary `0..100
  percent`.

  In Test, the only authority-changing action writes `P1.AutoMode` from false
  to true, runs the shipped selected PI candidate for 120 seconds, and restores
  `P1.AutoMode` to false. On the first Auto scan, the error history is reset,
  so proportional increment is zero; integral increment is
  `4 * (35 - 20) * 0.1 = 6 percentage points`; and the measured 75-percent
  holding output produces an 81-percent preliminary request. The lesson shall
  tell the learner to compare those predicted values with the live signals and
  shall describe the final restoration explicitly.

  The native executable evidence test, not an unavailable Control Studio
  stage, owns the independent comparison: the Lesson 5 proportional reference,
  Lesson 6 integral-only reference, Ki candidates `0.5`, `2`, `4`, and `8` at
  fixed Kp 8, full state restoration between cases, common load event, timing,
  limits and seed, and the separate unreachable-45-degree exposure. The README
  may present that retained evidence and ask the learner to reason from it, but
  it shall not claim that pressing Test executes the sweep or unreachable
  exposure. Editing a live gain is not a case reset.
- **Decision measures:** Declare the complete decision contract before any
  result. A final window from `110.1 through 120.0 seconds` is settled only when
  PV range is no greater than `0.1 degrees Celsius`, applied-output range is no
  greater than `1.0 percentage point`, and applied output does not remain at a
  limit. Final-window mean absolute temperature error is the mean of
  `|SP[k] - PV[k]|` in degrees Celsius over that window and is calculated only
  for a settled eligible case. The simulation criterion is no greater than
  `0.2 degrees Celsius`.

  Define early-load mean absolute temperature error over `60.1 through 75.0
  seconds`; lower is preferred but no physical threshold is established.
  Define recovery time as elapsed seconds after the load until absolute error
  first becomes no greater than `0.2 degrees Celsius` and remains within that
  band through the end of the run; a case that never does so reports `Not
  recovered`. Define post-load overshoot as `max(PV[k] - SP[k], 0)` from `60.1
  through 120.0 seconds`. Define cumulative applied-output movement as total
  variation after the load in percentage points. Define normal-run saturation
  duration from samples at either applied limit. Every measure shall name its
  source signal, window, sample interval, unit, prerequisite, and direction.

  A PI candidate is eligible for the simulation tuning decision only if it is
  finite, remains within all hard limits, is settled, has final-window mean
  absolute error no greater than `0.2 degrees Celsius`, recovers within `15
  seconds`, has post-load overshoot no greater than `0.25 degrees Celsius`, and
  has zero normal-run saturation duration. These are declared simulation
  teaching criteria, not physical-heater acceptance thresholds. From eligible
  candidates select the shortest recovery time; if recovery differs by no more
  than one sample, select lower cumulative applied-output movement. The P and
  integral-only cases are explanatory references and are not eligible PI
  candidates.

  For the separate 45-degree exposure, define peak preliminary requested
  output, peak applied output, requested-minus-applied mismatch, saturation
  duration, and maximum achieved PV. Its required observation is a preliminary
  request above 100 percent while applied output remains at 100 percent and PV
  remains below the unreachable SP. It is not a tuning candidate or physical
  acceptance test.
- **Evidence:** Retain for every baseline and PI candidate the run identity,
  Kp, Ki, Ti, setpoint, process value, error, proportional increment, integral
  increment, preliminary requested output, applied output, Manual output,
  mode, plant event, cycle time, seed, limits, phase, saturation state, and
  simulation evidence class. Retain a qualification summary with settling and
  ranges, early and final errors, recovery, overshoot, applied movement,
  saturation, each criterion result, eligibility, and selection. Retain the
  separate unreachable-exposure trace and summary with its distinct purpose.
  Generate all README values, comparison tables, and plots from those native
  executable artifacts. The figure shall make the P/I/PI normal comparison and
  selected PI tuning trade-off understandable without forcing six traces onto
  one unreadable axis.
- **Required conclusion:** For this self-regulating heater near the qualified
  operating point, proportional action supplies prompt correction while the
  integral state changes and retains the holding power required for zero
  constant-load error. A bounded Ki comparison, rather than the mere presence
  of both terms, is required to select a balanced PI controller. The selected
  simulation result does not establish a physical tuning. The separate
  unreachable exposure proves that a controller's preliminary request can
  exceed what the one-sided heater applies; it does not yet explain or solve
  the resulting controller-state behavior. That measured requested/applied
  mismatch is the reason for Lesson 8.

#### Lesson 8 — Amplitude saturation: same water heater

- **Objective:** Use requested and applied heater power to determine whether a
  temperature is reachable, diagnose actuator saturation, and explain the
  resulting integral windup.
- **Context and inherited reason:** Continue with the Lesson 7 recirculating
  electric water heater, its one-sided 0-to-100-percent power stage, outlet-
  temperature sensor, 35-degree-Celsius operating point, and selected
  incremental PI tuning `Kp = 8` and `Ki = 4`. Lesson 7's separate
  45-degree-Celsius exposure recorded a 159-percent preliminary request while
  only 100 percent reached the heater and the process value remained below the
  setpoint. That measured mismatch, rather than the mere existence of an
  actuator limit, is the reason for this lesson. The lesson shall not introduce
  a simultaneous cold-water disturbance; setpoint reachability and amplitude
  saturation are the only changed phenomena.
- **Theory before practice:** In ordinary words, introduce the outlet-
  temperature setpoint (SP), measured outlet-temperature process value (PV),
  temperature error (e), preliminary requested heater power
  (MV_requested), physically applied heater power (MV_applied), lower heater
  limit (MV_min), upper heater limit (MV_max), saturation error (e_sat), and
  stored integral contribution (uI), with temperature in degrees Celsius and
  heater commands or command differences in percent or percentage points as
  appropriate. State that the heater can add heat but cannot actively cool.
  Explain that the controller calculates MV_requested, the power stage clamps
  that request into MV_applied, and only MV_applied enters the plant. Only then
  explain the nested operations: the inner minimum prevents the request from
  exceeding MV_max, and the outer maximum prevents the upper-bounded result
  from falling below MV_min. Then write
  `MV_applied = max(min(MV_requested, MV_max), MV_min)` and
  `e_sat = MV_requested - MV_applied`. A positive e_sat therefore means
  unavailable positive heating demand under this sign convention. Derive the
  windup mechanism from Lesson 6's per-scan integral update by summing it over
  N scans: `uI[N] = uI[0] + sum(Ki * e[k] * dt)`. Explain why positive Ki,
  error, and dt make every added term positive even though the external clamp
  prevents any additional plant input.
- **Reachability before execution:** Explain in words that a steady target is
  reachable only if the heater power required to balance the thermal loss lies
  inside the physical actuator range. For this simulation and before any load
  change, define ambient or zero-power outlet temperature (T0) as 20 degrees
  Celsius and steady heater gain (Kh) as 0.2 degrees Celsius per percent heater
  power. After introducing those quantities, begin from the stated steady
  heater model `PV_ss = T0 + Kh * MV_required`. Set `PV_ss = SP`, subtract T0,
  and divide by Kh to derive `MV_required = (SP - T0) / Kh`. Work both
  predictions: 35 degrees
  Celsius requires 75 percent and is reachable; 45 degrees Celsius requires
  125 percent and is not reachable. State the model and operating-region
  assumptions next to the calculation and do not present this simulation
  reachability result as physical-heater evidence.
- **Controller law and first-scan prediction:** State that the same positive-
  acting, incremental PI controller from Lesson 7 runs every 0.1 second with
  zero bias and derivative action disabled. It is deliberately given very wide
  internal output limits for this teaching experiment, while a separate
  0-to-100-percent physical clamp limits the plant input. Consequently the
  controller cannot use its internal anti-windup path to observe the physical
  clamp; Lesson 10 will compare remedies. Begin at 35 degrees Celsius in Manual
  with 75 percent applied heater power, transfer bumplessly to Auto, and let the
  baseline settle. For the step from 35 to 45 degrees Celsius, introduce the
  proportional increment (delta_uP) and integral increment (delta_uI) in words
  before their equations. Predict a first proportional increment of 80
  percentage points, a first integral increment of 4 percentage points, a
  preliminary request of 159 percent from the preceding 75-percent command, an
  applied command of 100 percent, and a positive 59-percentage-point saturation
  error.
- **Controller under test:** Show the concise actual Structured Text that
  configures `Control.PID`, initializes the retained nominal output from the
  75-percent Manual holding command on the Manual-to-Auto edge, calculates the
  preliminary PI request, applies the separate physical clamp, and publishes
  saturation error. The program shall expose the live Kp and Ki values used by
  both diagnostics and `Controller.Update`; a live edit shall be adopted at
  the declared next-scan boundary without resetting retained state. Independent
  experimental cases still require restored plant and controller state.
- **Experiment and exact learner workflow:** Use deterministic Simulation evidence with the same heater
  model, 0.1-second sample interval, and seed as Lesson 7, but without the
  Lesson 7 load event. The executable learner journey has exactly
  `Monitor -> Test`; it shall not advertise a disabled Validate action. The
  shipped sample starts at SP 35 degrees Celsius, PV 20 degrees Celsius,
  Manual mode, zero requested and applied power, and false experiment
  authority. Monitor shall display that real initial state rather than claim
  that the 35-degree / 75-percent operating point already exists.

  Test shall use `Observe without excitation`, zero disabled amplitude, and a
  95.1-second maximum duration. Its only reviewed write is a false-to-true
  transition of `P1.ExperimentEnabled`; completion restores that authority to
  false. The project owns the exact scan-timed sequence: cycle 1 safe Manual
  zero; cycles 2 through 500 Manual at 75 percent; cycles 501 through 550 Auto
  baseline at SP 35 degrees Celsius; cycles 551 through 650 Auto at SP 45
  degrees Celsius; cycles 651 through 950 Auto recovery at SP 35 degrees
  Celsius; and cycle 951 Manual zero. Abort on any nonfinite signal or applied
  heater power outside 0 through 100 percent. The native retained-evidence run
  shall execute this same project-owned sequence through the same experiment
  authority, not reproduce it through an independent test-only timeline. The
  experiment is simulation-only and shall not be offered as an approved
  physical-plant action.
- **Decision measures before results:** Define reachability margin as the upper
  physical limit minus required steady heater power in percentage points; a
  negative margin means the target is unreachable under the stated model. Write
  `R = MV_max - MV_required` after defining the quantities and sign.
  Define peak requested heater power, peak applied heater power, peak positive
  saturation error, and time at the upper limit over the declared unreachable-
  exposure window. Define post-restoration saturation hold as the elapsed time
  from restoring 35 degrees Celsius until MV_applied first leaves the upper
  limit. Define recovery to the achievable setpoint using an explicit error
  band and dwell rule; if the response does not qualify inside the retained
  window, report `Not recovered` rather than a final-sample substitute. Derive
  the two elapsed-time measures explicitly as
  `post_restore_hold = t_release - t_restore` and
  `recovery_time = t_first_sustained - t_restore`, explaining how each endpoint
  is selected from the samples. Define
  every threshold before the retained result and retain requested and applied
  signals separately.
- **Comparison and validation:** Compare three contiguous, labeled phases from
  one reproducible run: qualified achievable baseline, unreachable exposure,
  and achievable-setpoint restoration. Verify the 35-degree-Celsius
  reachability calculation gives 75 percent and positive margin, while the
  45-degree-Celsius calculation gives 125 percent and minus-25-percentage-point
  margin. Verify baseline requested/applied mismatch is within numerical
  tolerance; during exposure the request exceeds 100 percent, applied power
  never exceeds 100 percent, e_sat equals MV_requested minus MV_applied at every
  sample, PV remains below the unreachable SP, and upper-limit time is
  measurable. After restoration, verify the heater remains at 100 percent for
  a predeclared nonzero interval even though SP is again achievable. Reject any
  run that conflates the setpoint restoration with a plant disturbance, resets
  the integral state at restoration, omits safe Manual restoration, or reports
  physical acceptance from Simulation evidence.
- **Evidence and figure:** Retain run identity, phase, time, SP, PV, e, Kp, Ki,
  delta_uP, delta_uI, MV_requested, MV_applied, e_sat, mode, Manual output,
  model parameters, required steady output, reachability margin, sample
  interval, seed, limits, abort state, and Simulation evidence class. Retain a
  summary containing the declared measures, validity results, and conclusion.
  Generate a readable two-part teaching figure from the native trace: the
  upper panel shows SP and PV with the exposure and restoration events; the
  lower panel overlays MV_requested and MV_applied with the physical limits.
  The figure shall make the accumulated unavailable demand and delayed release
  from saturation visible without using invented curves.
- **Explanation and transfer:** Explain that the clamp correctly protects the
  one-sided power stage and that the plant responds only to MV_applied. Because
  the controller does not observe this external clamp, positive error continues
  adding to uI while the unreachable request is active. Returning to an
  achievable SP does not erase that stored demand, so MV_requested must unwind
  before MV_applied can leave 100 percent. Bound the conclusion to controllers
  with stored integral action that is unaware of the active actuator constraint;
  a proportional-only controller has no integral state to wind up, and a
  controller with effective anti-windup behaves differently. This retained
  slow-release evidence motivates the actuator-rate lesson and the later
  anti-windup comparison; Lesson 8 shall diagnose the failure but not silently
  solve it.

#### Lesson 9 — Rate limitation: motor-operated heating valve

- **Objective:** Configure a bounded simulated motor-operated heating valve,
  calculate how far it can physically travel during one controller update,
  distinguish the controller's requested position from the reachable applied
  position, and decide from recorded evidence whether both its position and
  directional travel-rate limits were respected.
- **Context:** Use a geared electric valve that regulates hot-water flow through
  a building heating coil. A flow transmitter measures heating-water flow in
  litres per minute. The controller requests valve opening in percent. The
  actuator can move only between fully closed at 0 percent and fully open at
  100 percent. It opens no faster than 1.5 percentage points per second and
  closes no faster than 0.5 percentage points per second. The slower closing
  direction represents a realistic asymmetric actuator; the lesson shall not
  imply that every valve has these rates. Upstream pressure remains fixed, the
  simulation begins with zero flow and a closed valve, and the safe state is a
  controlled return to the closed position followed by Manual mode. Physical
  use requires independently confirmed travel times, fail position,
  interlocks, pressure limits, and immediate authorization.
- **Reason:** Lesson 8 established that a controller request and physical plant
  input can differ when the requested amplitude is unavailable. An actuator may
  also reject an in-range position because it cannot travel there within the
  current update. Without separating that rate-created gap from plant response,
  an engineer may incorrectly increase controller gain or identify actuator
  motion as plant dynamics. This unresolved requested-versus-applied mismatch
  creates the need for Lesson 9.
- **Theory before practice:** First explain in words that an amplitude limit
  bounds position while a rate or slew limit bounds position change per unit
  time. Introduce the heating-water flow setpoint (SP) and measured flow process
  value (PV), both in litres per minute. Introduce the requested manipulated
  variable (MV_requested) as the valve position calculated by the controller
  and the applied manipulated variable (MV_applied) as the rate- and amplitude-
  constrained position delivered to the simulated plant, both in percent.
  Introduce the previous applied position used by the current update
  (MV_previous) in percent and the configured controller task interval (dt) in
  seconds. For this deterministic lesson, dt is 0.1 seconds and shall equal
  both the `TASK` interval in Structured Text and `cycle_interval_ms` in the
  runtime configuration. It is a configured nominal interval, not a
  scheduler-derived measurement. The lesson shall state that the calculation
  is valid only while those configurations remain synchronized and task timing
  is acceptably deterministic; measured elapsed time and jitter are introduced
  in Lesson 14. Introduce maximum opening rate
  (r_open) and maximum closing rate (r_close) in percentage points per second,
  then introduce the lowest admissible position (MV_low) and highest admissible
  position (MV_high) in percent. Only after all quantities are defined may the
  lesson derive the motion bounds from the definition of rate as change divided
  by time: multiply by positive dt to obtain
  `maximum_opening_change = r_open * dt` and
  `maximum_closing_change = r_close * dt`; add the opening change to the
  previous position, subtract the closing change, and intersect both with the
  0-to-100-percent amplitude range. Then state
  `MV_low = max(0%, MV_previous - r_close * dt)`,
  `MV_high = min(100%, MV_previous + r_open * dt)`, and
  `MV_applied = max(MV_low, min(MV_requested, MV_high))`.
  Introduce measured travel rate (r_measured) in percentage points per second
  as a finite-difference calculation: current applied position minus the exact
  previous position used by this update, divided by positive dt. Then state
  `r_measured = (MV_applied - MV_previous) / dt`.
  Explain that the sign distinguishes opening from closing and that dividing by
  the configured interval, rather than reporting movement per scan, gives the
  commanded trajectory a physical rate under the lesson's fixed-timing
  assumption. Explain that this software trajectory represents the declared
  simulated actuator behavior; a real actuator still requires position
  feedback or other independent proof that it followed the command.
- **Prediction before execution:** At dt = 0.1 seconds, calculate that the valve
  may open by at most 0.15 percentage point per update and close by at most 0.05
  percentage point per update. Calculate that an uninterrupted 10-percentage-
  point move therefore requires at least 6.67 seconds while opening and 20.0
  seconds while closing. Predict that MV_requested can move immediately,
  MV_applied will form direction-dependent ramps, and the early measured flow
  cannot follow the requested position. State that increasing proportional or
  integral gain cannot shorten the actuator's physical minimum travel time.
- **Controller under test:** Use the shipped `Control.PID` to calculate
  MV_requested, with the PID's internal rate bounds deliberately set wide so
  they do not hide the lesson's explicit actuator constraint. Show the concise
  actual Structured Text that uses the synchronized configured task interval
  and directional rates, snapshots MV_previous for the current calculation,
  derives MV_low and MV_high from that snapshot, constrains MV_requested into
  MV_applied, calculates r_measured from the same snapshot before updating the
  retained position for the next scan, and sends only MV_applied to the plant.
  The retained trace shall expose the snapshot actually used by the equations,
  not the already-updated state. The excerpt shall make clear that the PID does
  not receive the external rate-constrained position as tracking feedback in
  this lesson. Every setting the learner changes shall be live-mapped or shall
  explicitly require restart.
- **Experiment and exact learner workflow:** The executable learner journey has
  exactly `Monitor -> Test`; it shall not advertise a disabled Validate action.
  In Control Studio Monitor, verify SP, PV, MV_requested, MV_applied,
  MV_previous, MV_low, MV_high, r_measured, r_open, r_close, configured dt,
  mode, experiment authority, signal quality, limits, seed, and Simulation
  evidence class. Verify that the Structured Text task interval, runtime cycle
  interval, and configured dt are all 0.1 seconds before calculating the two
  per-update movements. The shipped sample starts at SP and PV zero, a closed
  valve, Manual mode, zero requested and applied position, and false experiment
  authority.

  Test shall use `Observe without excitation`, zero disabled amplitude, and a
  90.1-second maximum duration. Its only reviewed write is a false-to-true
  transition of `P1.ExperimentEnabled`, restored false at completion. The
  project owns the exact timeline: cycle 1 safe Manual zero; cycles 2 through
  151 Auto at SP 10 litres per minute; cycles 152 through 900 Auto at restored
  SP zero without resetting controller or actuator state; and cycle 901 Manual
  zero. The native retained-evidence run shall execute this same project-owned
  timeline through the same experiment authority. Abort on any nonfinite
  required signal, nonpositive dt, mismatched configured intervals, MV_applied
  outside 0 through 100 percent, or r_measured beyond its direction-specific
  bound. Keep controller, plant, initial state, upstream pressure, sample
  schedule, event timing, limits, and seed fixed. The comparison is between the
  unconstrained controller-request trajectory and the reachable actuator
  trajectory within this same run; it shall not be described as a second
  physical plant run.
- **Measures before results:** Define the maximum measured opening rate as the
  largest positive change from the retained MV_previous snapshot to MV_applied,
  divided by that sample's configured dt during the opening window. Define the
  maximum measured closing magnitude as the absolute value of the most negative
  change from the retained MV_previous snapshot to MV_applied, divided by
  configured dt during the closing window. Use percentage points per second for
  both. For every sample, the retained values shall reproduce r_measured from
  the declared equation within the stated numerical tolerance.
  Define the maximum unavailable-position gap as the largest absolute
  difference between MV_requested and MV_applied in percentage points. Define
  flow at 5 seconds as the single PV sample at the declared 5.0-second event
  time in litres per minute; it is a transient comparison value, not a settled
  or steady-state measure. Define opening and closing limit exposure durations
  from samples at their respective bounds. Define return-to-closed time by
  identifying the first zero-applied-position sample after restoration and
  subtracting the restoration time:
  `return_to_closed_time = t_closed - t_restore`. Define safe restoration as the first
  retained Manual sample with MV_requested = 0 percent, MV_applied = 0 percent,
  and finite PV. Declare tolerances and windows before showing results.
- **Validation:** A valid run shall remain finite, keep MV_applied within 0
  through 100 percent, never exceed 1.5 percentage points per second opening or
  0.5 percentage points per second closing within numerical tolerance, exercise
  both directional limits, create at least a 1-percentage-point requested-
  versus-applied gap, keep the declared 5-second PV below 6 litres per minute,
  and reach the safe restoration state. Reject rather than rank a run with
  invalid or inconsistent configured dt, a retained row that cannot reproduce
  r_measured, a missing directional movement, a violated constraint, an abort,
  or incomplete restoration. The 5-second PV threshold proves visible early
  flow restriction for this simulation only; it is not a universal valve
  acceptance limit.
- **Evidence and figure:** Retain deterministic run identity; phase and time;
  SP and PV; MV_requested, MV_applied, and MV_previous; r_measured, r_open, and
  r_close; MV_low and MV_high; dt; controller and plant settings; seed; limits;
  abort and restoration dispositions; and Simulation evidence class. The
  retained MV_previous field shall be the value used by that row's limiter and
  rate equations. Retain a
  summary containing every declared measure, validity result, and bounded
  conclusion. Generate a two-part teaching figure from that native trace: the
  upper panel shows SP and PV with request and restoration events; the lower
  panel overlays MV_requested and MV_applied and makes the asymmetric opening
  and closing ramps visible. A separate compact rate panel or annotation shall
  show the measured directional limits if it remains readable. Invented or
  manually adjusted curves are forbidden.
- **Explanation and transfer:** Explain that MV_requested is a calculation, not
  a physical trajectory. The geared motor causes MV_applied to lag that request,
  so the plant receives less opening during the early response and returns more
  slowly in the deliberately slower closing direction. Bound this conclusion
  to the declared actuator rates, operating range, fixed-pressure simulation,
  and synchronized positive configured dt. This lesson does not prove behavior
  under task jitter or missed scans. A fast actuator may make rate limiting negligible, while
  backlash, stiction, deadband, position-loop faults, or changing pressure
  require additional models and measurements. Because the PID in this lesson
  does not track the externally rate-constrained position, its stored integral
  demand can still diverge from what the actuator applies. That retained
  limitation motivates the anti-windup strategies in Lesson 10.

#### Lesson 10 — Anti-windup strategies: steam-heated batch vessel

- **Objective:** Explain how unavailable actuator demand becomes stored
  controller state, calculate the correction made by each taught anti-windup
  strategy, and select the simplest eligible strategy from equivalent recorded
  recovery, overshoot, saturation, and valve-travel evidence.
- **Context:** Use a steam-heated batch vessel whose temperature transmitter
  reports batch-temperature deviation in degrees Celsius and whose steam valve
  accepts heating trim in percent. The local operating point is zero
  temperature deviation and zero valve trim. The simulated valve is limited to
  -3.5 through 3.5 percent trim; negative trim represents less steam than the
  operating-point flow, not active cooling. Feed temperature, steam pressure,
  vessel contents, controller gains, sample schedule, and initial state remain
  identical across four fresh deterministic simulation runs. Manual zero trim
  is the safe restoration state. Physical use requires independently reviewed
  steam, pressure, temperature, relief, interlock, and batch constraints plus
  explicit authorization immediately before action.
- **Reason:** Lessons 8 and 9 proved that the controller's requested output can
  differ from applied actuator motion. Merely clamping the final actuator value
  does not correct the integral state that produced unavailable demand. That
  retained state can keep the valve at a limit after the setpoint is restored,
  causing slow recovery or overshoot. Lesson 10 therefore compares ways to
  make controller state respond to constrained actuation.
- **Theory before practice:** First restate the temperature setpoint (SP) and
  process value (PV), both in degrees Celsius, and requested manipulated
  variable (MV_requested) and applied manipulated variable (MV_applied), both
  in percent trim. Define nominal controller output (MV_nominal) as the
  incremental PI controller state before the current output limit and saturated
  candidate (MV_saturated) as that nominal state constrained to the internal
  output range. Define saturation error (e_sat) in percentage points as nominal
  output minus saturated candidate. Define integral increment (delta_MV_I) as
  the change that integral action proposes during the current update. Define
  cycle time (dt) and tracking time (Tt), both in seconds, before defining the
  tracking coefficient (alpha) as the fraction of saturation error removed in
  one update. Derive saturation error as the unavailable difference between
  nominal and saturated output. Then derive tracking correction from the rate
  needed to close that gap in Tt seconds: `correction_rate = e_sat / Tt` and,
  after multiplying by one update duration, `correction = (dt/Tt) * e_sat`.
  Only then define `e_sat = MV_nominal - MV_saturated` and `alpha = dt / Tt`.
  Explain four configurations in words before their equations:
  1. The unprotected baseline gives the PI controller deliberately wide
     internal limits and clamps only downstream applied output. The controller
     therefore cannot see the physical limit and its stored nominal output can
     continue to grow.
  2. Control-signal clamping uses the paper-compatible BackCalculation equation
     with Tt equal to dt. Substitute alpha one and the saturation-error
     difference into the stored-state equation to prove
     `MV_stored = MV_saturated` in one update.
  3. Back calculation uses
     `MV_stored = MV_nominal - alpha * e_sat`. Larger Tt means a smaller
     correction per update. In the paper-compatible equation alpha is not
     silently limited when dt/Tt exceeds one.
  4. Combined anti-windup first derives the direction test
     `delta_MV_I * e_sat > 0`: a positive product means the integral increment
     and saturation error have the same sign and therefore point farther into
     the active limit. It removes no more than the
     smaller magnitude of the integral increment and saturation error, while
     permitting integral motion away from the limit. It then applies bounded
     back calculation with `alpha_combined = min(dt / Tt, 1)` and the saturated
     candidate calculated before the conditional correction, as required by
     the `Control.PID` contract.
  Explain that anti-windup changes internal state handling, not the physical
  valve limit. A downstream clamp remains the final physical boundary in every
  run. This lesson uses the incremental PI form with derivative gain set to
  zero because derivative action is not introduced until Lesson 20. The
  amplitude limit is exercised while the rate limit is deliberately
  nonbinding, so one causal mechanism changes at a time.
- **Prediction before execution:** For e_sat = 10 percentage points and dt =
  0.1 seconds, calculate that Tt = 3.25 seconds gives alpha = 0.030769 and a
  0.30769-percentage-point correction in one update. Calculate that Tt = dt
  gives alpha = 1 and removes the complete 10-percentage-point gap from stored
  nominal state in that update. Predict that the unprotected controller will
  remain at the upper valve limit after SP restoration, while all protected
  cases will leave it earlier. Do not predict that the largest immediate state
  correction must give the best PV transient or least valve travel.
- **Controller under test:** Show the concise actual Structured Text that
  selects the four configurations, keeps proportional and integral gains and
  derivative gain zero identical, assigns wide internal limits only to the
  unprotected baseline, realizes control-signal clamping as BackCalculation
  with Tt = dt, selects ordinary BackCalculation and Combined with Tt = 3.25
  seconds, calls the same shipped `Control.PID`, and applies the same downstream
  -3.5 through 3.5 percent valve clamp in all cases. State that strategy
  selection is read during initialization and therefore requires a fresh run;
  it shall not be presented as a live mid-run switch.
- **Experiment:** The declared learner workflow is exactly Monitor then Test.
  In Monitor, verify the safe initial Manual state at SP zero, PV zero,
  requested and applied trim zero, selected strategy 2 BackCalculation, Kp, Ki,
  zero Kd, dt, Tt, physical valve limits, mode, quality, experiment authority
  false, seed, and Simulation evidence class. In Test choose Observe without
  excitation, disabled amplitude zero, and maximum duration 50.1 seconds.
  Review a plan that changes only `P1.ExperimentEnabled` from false to true and
  restores it false. The project owns the selected-strategy sequence: enter
  Auto at SP zero, apply unreachable SP 4.5 degrees Celsius beginning at 1.0
  seconds, restore SP zero at 20.0 seconds without resetting controller state,
  continue automatic recovery through 50.0 seconds, then select Manual zero at
  50.1 seconds. Control Studio shall not claim to switch the initialization-
  time strategy or execute a disabled Validate stage. The native evidence test
  creates four fresh runtimes for strategies 0 through 3 and drives that same
  project-owned authority and sequence in each. Reject the complete
  comparison if plant, tuning, events, timing, limits, initial state, seed, or
  safe restoration differs. Abort a run on nonfinite required data, invalid
  strategy, nonpositive dt or Tt where required, MV_applied outside -3.5
  through 3.5 percent, or temperature outside its declared safety envelope.
- **Measures before results:** Define recovery time as time after SP restoration
  until absolute PV is at most 0.05 degree Celsius and remains within that band
  through the 20.0-to-50.0-second automatic observation window. Report not
  recovered rather than a numeric
  value if this condition is never reached. Define post-restoration upper-limit
  duration as consecutive time after restoration while MV_applied remains at
  3.5 percent within tolerance. Define recovery undershoot as the magnitude of
  the most negative PV after restoration, in degrees Celsius; if PV never goes
  below zero, report zero. Define total applied-valve travel as
  `sum(abs(MV_applied[k] - MV_applied[k-1]))` over the complete run, in
  percentage points. The complete-run window begins with the retained initial
  Manual sample and ends with the retained final Manual-zero sample, so the
  calculation includes the final Auto-to-Manual transition. Define peak
  unavailable request as the largest positive
  `MV_requested - MV_applied` for the unprotected baseline. Define safe
  restoration as a retained final Manual sample with SP = 0, finite PV,
  MV_requested = 0, and MV_applied = 0. State windows, units, tolerances, and
  eligibility thresholds before presenting values.
- **Validation:** Every run shall remain finite, respect the physical valve and
  temperature bounds, exercise the upper valve limit, complete the same event
  sequence, and reach safe restoration. The unprotected baseline shall expose a
  positive unavailable request and at least one second of post-restoration
  upper-limit hold. Each protected strategy shall reduce recovery time by at
  least one second relative to that baseline and shall have no more than 0.1
  second of post-restoration upper-limit hold and no more than 0.05 degree
  Celsius of recovery undershoot. Classify eligibility from those rules before
  ranking. Among eligible strategies, select the shortest recovery. Recovery
  differences smaller than 0.1 second are indistinguishable; tied cases are
  then ranked by lower complete-run applied-valve travel. Travel differences
  smaller than 0.1 percentage point are indistinguishable; a remaining tie is
  resolved in favour of the strategy with the simpler state equation. Do not
  describe a strategy as fast or physically acceptable beyond these declared
  criteria. The retained summary shall calculate eligibility and selection
  from these rules rather than hard-coding a strategy identity. Combined shall
  not be selected solely because it is more elaborate.
- **Evidence and figure:** Retain deterministic run identity and strategy;
  phase and time; SP and PV; MV_requested and MV_applied; Kp, Ki, Kd, dt, Tt,
  anti-windup equation, internal and physical limits; plant settings; seed;
  recovery, undershoot, saturation, and travel measures; abort, eligibility,
  selection, and safe-restoration dispositions; and Simulation evidence class.
  Retain one trace and one summary covering all four independently initialized
  runs. Generate a teaching figure from the native trace with aligned PV and
  applied-MV panels for all strategies, the restoration event, valve limits,
  and readable strategy labels. A requested-versus-applied detail for the
  unprotected baseline may be a third panel when it materially clarifies
  windup. Invented or manually adjusted curves are forbidden.
- **Explanation and transfer:** Explain recovery differences from stored-state
  correction, conditional integration, and tracking coefficient rather than
  strategy names. Bound the result to this tuning, limit, event, deterministic
  simulation, and local vessel model. Explain that other processes may rank the
  strategies differently because noise, actuator rate, dead time, asymmetric
  limits, valve stiction, or safety constraints change the trade-off. For
  another constrained loop, reproduce the unprotected failure safely, retain
  requested and applied actuation, calculate dt/Tt, compare equivalent runs,
  and choose the simplest eligible strategy. Rate-constrained tracking shall
  use the actual applied trajectory rather than assuming amplitude saturation
  is the only actuator limitation. This experiment restores a setpoint; it
  does not prove rejection of an independently applied plant disturbance.
  That unresolved distinction creates Lesson 11's comparison of servo and
  regulatory objectives.

#### Lesson 11 — Servo and regulatory objectives: jacketed reactor

- **Objective:** Configure two fresh experiments on one jacketed-reactor loop,
  calculate objective-specific error and valve measures from retained samples,
  decide separately whether the unchanged PI controller satisfies setpoint-
  tracking and disturbance-rejection requirements, and explain why those
  decisions differ.
- **Context:** Use a jacketed stirred reactor whose product-temperature
  transmitter reports degrees Celsius and whose heating-jacket valve accepts
  0 through 100 percent opening. The local operating point is 80 degrees
  Celsius at 50 percent valve opening. The servo experiment changes the
  production-temperature setpoint. The regulatory experiment holds that
  setpoint and applies the equivalent thermal effect of colder feed. The
  equivalent disturbance is a simulation input, not a physical feed-
  temperature measurement. Manual zero-percent valve demand is the reviewed
  final simulation state. Physical use requires independent review of reaction
  chemistry, runaway and relief scenarios, jacket utilities, feed topology,
  valve action, trips, temperature limits, and the exact experiment, followed
  by explicit authorization immediately before action.
- **Reason:** Lesson 10 ended with a setpoint-restoration experiment and proved
  explicitly that it did not establish rejection of an independently applied
  plant disturbance. A production reactor must both follow intentional recipe
  changes and reject load changes. Lesson 11 therefore compares those two
  objectives without changing the plant, controller, limits, timing, seed, or
  initial operating point.
- **Theory before practice:** Define the temperature setpoint (SP) as desired
  reactor temperature and process value (PV) as measured product temperature,
  both in degrees Celsius. Define applied manipulated variable (MV_applied) as
  heating-jacket valve opening in percent. Define error (e) in degrees Celsius
  as setpoint minus process value, sample index (k) as one retained controller
  update, cycle time (dt) in seconds, and event-elapsed time (t_event[k]) in
  seconds before writing any metric equation. Explain that this lesson has no
  downstream actuator model that separates the bounded PID output from the
  plant input; `MV_applied` is therefore the controller's constrained output
  and the simulated valve input. Do not invent a separate requested-output
  trace.

  Define a servo test in words as a change to SP while the disturbance remains
  fixed, used to assess reference tracking. Define a regulatory test in words
  as a disturbance change while SP remains fixed, used to assess disturbance
  rejection. Explain that the paths enter the loop differently and therefore
  require separate run identities, windows, requirements, and decisions.

  Derive IAE by treating each retained sample as an error-time rectangle with
  height `abs(e[k])`, width `dt`, and area `abs(e[k])*dt`, then summing the
  areas. Derive ISE by squaring error before forming each rectangle and explain
  that doubling error then quadruples its contribution. Derive ITAE by
  multiplying each absolute-error rectangle by event-elapsed time so later
  error costs more. Only then write
  `IAE = sum(abs(e[k]) * dt)`,
  `ISE = sum(e[k]^2 * dt)`, and
  `ITAE = sum(t_event[k] * abs(e[k]) * dt)`.
  State their units as degree-Celsius seconds, squared-degree-Celsius seconds,
  and degree-Celsius seconds squared. Derive servo overshoot as
  `max(PV[k]-85 degC, 0)`, regulatory peak deviation as
  `max(abs(80 degC-PV[k]))`, final-window error as an arithmetic mean, and
  complete-event applied-valve travel as the sum of absolute consecutive
  movements. Define settling and recovery persistence in physical words before
  their calculations. Explain why each answers a different engineering
  question and why they shall not be collapsed into one unexplained score.
- **Prediction before execution:** State that the retained proportional gain is
  8 percentage points of valve request per degree Celsius and that proportional
  action uses the complete setpoint change. Calculate that a 5-degree-Celsius
  setpoint step initially contributes 40 percentage points above the 50-percent
  operating value, giving a 90-percent initial valve prediction before current
  integral change. Within the declared local operating region, explain that a
  sustained one-percentage-point valve increase contributes 0.25 degree
  Celsius to equilibrium temperature. Calculate that the simulated
  5-degree-Celsius cold-feed equivalent therefore requires approximately 20
  additional percentage points of steady valve opening. Predict setpoint
  overshoot risk and an initial temperature fall followed by increased valve
  opening in the regulatory run. Do not predict transient times from dynamic
  model concepts that have not yet been taught.
- **Controller under test:** Show the concise actual Structured Text that keeps
  proportional gain 8, integral gain 2, derivative gain zero, bias 50 percent,
  full proportional response to SP, Combined anti-windup, 0 through 100 percent
  output limits, and 0.1-second cycle time identical in both runs. Show the
  actual `Control.PID` configuration and update call. State that this is the
  same incremental PI controller and that derivative action remains disabled.
  The learner shall be able to trace SP, PV, Auto/Manual mode, constrained
  controller output, and simulated valve input through the shipped program.
- **Experiment:** The declared learner workflow is exactly Monitor then Test.
  Control Studio runs selected objective 0, the servo case. In Monitor verify
  SP and PV 80 degrees Celsius, MV_applied and Manual command 50 percent,
  selected objective zero, Manual mode, experiment authority false, PI
  parameters, limits, quality, 0.1-second sample interval, seed, operating
  point, and Simulation evidence class. In Test choose Observe without
  excitation, disabled amplitude zero, and maximum duration 56.5 seconds.
  Review a plan that changes only `P1.ExperimentEnabled` from false to true and
  restores it false. Record the 90-percent initial servo prediction before
  execution. Do not claim the visible run executes the fresh regulatory case
  or a disabled Validate stage.

  Run the servo case from a fresh simulation. Retain an initial Manual sample
  at time 0 with SP and PV at 80 degrees Celsius and the valve at its 50-percent
  operating value. Enter Auto, hold SP at 80 degrees Celsius through 5.0
  seconds, command 85 degrees Celsius on the next controller update at 5.1
  seconds, and observe through 55.0 seconds. Request Manual zero at 55.1
  seconds, then retain each 0.1-second restoration update until the applied
  valve reaches zero. The controller's declared valve-rate limit remains
  active during Manual operation, so the lesson shall not claim an impossible
  instantaneous closing movement. The cold-feed event shall not occur during
  this run.

  The native evidence test selects objective 1 and runs the regulatory case
  from another fresh simulation with the same initial
  Manual sample and controller settings. Enter Auto, hold SP at 80 degrees
  Celsius, apply the minus-5-degree-Celsius cold-feed equivalent at the 60.0-
  second plant boundary so its first post-event retained sample is 60.1
  seconds, and observe through 120.0 seconds. Request Manual zero at 120.1
  seconds and retain the rate-constrained restoration until the applied valve
  reaches zero. Reject the comparison if any undeclared controller, plant, initial-
  state, timing, limit, or seed difference exists. Abort either run on
  nonfinite required data, mode departure during its automatic window, PV
  outside 60 through 100 degrees Celsius, or MV_applied outside 0 through 100
  percent.
- **Measures before results:** For the servo event window, include the retained
  5.0-second pre-event sample and samples through 55.0 seconds. Define servo
  overshoot as `max(PV[k] - 85 degC, 0)` after the step. Define settling time as
  elapsed time after the 5.0-second event boundary until absolute error is at
  most the 0.5-degree-Celsius upper bound plus its 0.01-degree-Celsius numerical
  comparison tolerance and remains there through 55.0 seconds; report `Not
  settled` if the condition is not met. The reported settling time has a
  0.1-second reproduction tolerance and no physical acceptance threshold.
  Calculate IAE, ISE, and ITAE over 5.1 through 55.0 seconds. Calculate
  complete-event applied-valve travel from the 5.0-second pre-event sample
  through the 55.0-second final Auto sample, so the initial valve change is
  included but safe shutdown is not mixed into automatic-control effort.

  For the regulatory event window, include the retained 60.0-second pre-event
  sample and samples through 120.0 seconds. Define peak deviation as the largest
  absolute difference between PV and the fixed 80-degree-Celsius SP after the
  disturbance. Define recovery time as elapsed time after the 60.0-second event
  boundary until absolute error is at most the same 0.5-degree-Celsius upper
  bound plus its 0.01-degree-Celsius numerical comparison tolerance and remains
  there through 120.0 seconds; report `Not recovered` if it does not. Calculate
  IAE, ISE, and ITAE over 60.1 through 120.0 seconds. Define regulatory final-
  window absolute error as the mean absolute error from 110.1 through 120.0
  seconds, and report it only when the recovery condition has passed. Calculate
  complete-event applied-valve travel from the 60.0-second pre-event sample
  through the 120.0-second final Auto sample. Each retained table value shall
  name its signal, calculation, unit, window, validity prerequisite, direction,
  and threshold or state explicitly that no physical threshold is established.
  Before results, declare 0.01-degree-Celsius tolerances for servo overshoot and
  regulatory peak deviation, 0.1-second tolerances for servo settling and
  regulatory recovery, a 0.001-degree-Celsius tolerance for regulatory final-
  window absolute error, 0.01-unit reproduction tolerances for IAE, ISE, and
  ITAE in their respective units, and a 0.01-percentage-point reproduction
  tolerance for applied-valve travel. Decisions shall use unrounded values.
- **Validation:** Require every sample finite, both automatic windows complete,
  the declared event sequence present, PV and MV_applied within their hard
  limits, and final Manual-zero restoration retained. The servo run fails its
  declared overshoot requirement when overshoot exceeds 1.0 degree Celsius.
  Settling time, servo IAE, ISE, ITAE, and servo valve travel are diagnostic
  comparison measures with lower preferred and no physical acceptance
  threshold in this lesson. The regulatory excitation is valid only when peak
  deviation is at least 1.0 degree Celsius. The regulatory objective requires
  recovery no greater than 40.0 seconds and qualified final-window absolute
  error no greater than 0.5 degree Celsius. Regulatory IAE, ISE, ITAE, and valve
  travel are diagnostic measures with lower preferred and no physical
  acceptance threshold. Apply each requirement only to its declared objective;
  do not average the two decisions or call the controller globally good or bad.
- **Evidence and figure:** Retain run identity and objective; phase and time;
  SP, PV, error, MV_applied, and disturbance state; proportional, integral, and
  zero derivative gains; full proportional setpoint-response configuration;
  bias, anti-windup, limits, cycle time, plant settings, seed, mode, abort and
  safe-restoration states, every metric, every metric and band-comparison
  tolerance, every criterion threshold and disposition, and Simulation evidence
  class. Retain one trace and one summary covering both independently
  initialized runs. Generate a teaching figure from the native trace with a
  servo SP/PV panel and applied-valve panel and a regulatory SP/PV panel and
  applied-valve panel. Mark the setpoint and disturbance events, use readable
  units and objective labels, and keep each objective's metric window explicit.
  Invented or manually adjusted curves are forbidden.
- **Explanation and transfer:** Explain that the unchanged PI is neither
  globally good nor globally bad: its full proportional response to an SP
  change can violate the servo overshoot requirement while the same gains meet
  the declared regulatory recovery and final-window-error requirements. Bound
  the conclusion to the local reactor operating region, declared disturbance,
  tuning, limits, deterministic simulation, and omitted chemistry and utility
  dynamics. Explain that another plant or requirement set can reverse the
  result. The unresolved engineering need is to soften setpoint response
  without weakening the nominal disturbance path or retuning the regulatory-
  focused PI. That evidence creates Lesson 12's introduction of proportional
  setpoint weighting and two-degree-of-freedom setpoint handling.

#### Lesson 12 — Proportional setpoint weighting: same reactor

- **Objective:** Configure and compare proportional setpoint weights of one,
  one-half, and zero on the same jacketed-reactor PI loop; predict each
  controller's immediate valve response to a setpoint step; use retained servo
  and regulatory evidence to select the largest weight that satisfies the
  setpoint-overshoot requirement without changing the fixed-setpoint
  disturbance path; and explain why proportional setpoint weighting cannot
  remove measurement-noise transmission.
- **Context:** Keep the Lesson 11 jacketed stirred reactor, product-temperature
  transmitter, and 0 through 100 percent heating-jacket valve. The local
  operating point remains 80 degrees Celsius at 50 percent valve opening. The
  servo experiment commands an 80-to-85-degree-Celsius product-temperature
  change. The regulatory experiment holds the setpoint at 80 degrees Celsius
  while applying the same simulated minus-5-degree-Celsius cold-feed heat-load
  equivalent. The reactor-temperature validity range remains 60 through 100
  degrees Celsius. The valve is one-sided, amplitude limited, and rate limited
  to 50 percentage points per second in either direction. Manual zero-percent
  valve demand is the reviewed final simulation state. These are Simulation
  evidence runs; physical use requires the independent safety review and
  immediate authorization stated for Lesson 11.
- **Reason:** Lesson 11 retained two different facts about the unchanged PI.
  With complete proportional response to the setpoint, the servo run produced
  approximately 1.049 degrees Celsius of overshoot and failed its 1.0-degree-
  Celsius upper requirement. The regulatory run recovered from the declared
  load in approximately 11.6 seconds and met its recovery and final-window-
  error requirements. Retuning the feedback gains to repair the servo result
  could sacrifice a regulatory response that already works. Lesson 12 therefore
  changes only how the proportional term receives a setpoint change and tests
  whether that smaller intervention preserves the fixed-setpoint path.
- **Theory before practice:** Define the reactor-temperature setpoint (SP) as
  the desired product temperature and the process value (PV) as the measured
  product temperature, both in degrees Celsius. Define true error (e) as SP
  minus PV in degrees Celsius. Define sample index (k) as one retained
  controller update and cycle time (dt) as 0.1 second. Define applied
  manipulated variable (MV_applied) as the bounded, rate-limited heating-valve
  opening in percent. This lesson has no downstream actuator model that
  separates the PID's constrained output from the simulated plant input, so it
  shall not invent a second requested-output trace.

  Explain that the shipped PI is discrete and incremental: it updates the
  preceding nominal valve output from changes in SP and PV plus the present
  true error. Define setpoint change (delta_SP) as the current SP minus its
  preceding value and process-value change (delta_PV) as the current PV minus
  its preceding value, both in degrees Celsius. Define proportional setpoint
  weight (b) as a dimensionless number from zero through one that scales only
  the setpoint-change part of the proportional increment. Define proportional
  gain (Kp) as percentage points of valve increment per degree Celsius,
  integral gain (Ki) as percentage points of valve increment per degree-
  Celsius second after multiplication by dt, proportional valve increment
  (delta_MV_P) in percentage points, and integral valve increment
  (delta_MV_I) in percentage points. Only after those definitions write
  `delta_MV_P = Kp * (b * delta_SP - delta_PV)` and
  `delta_MV_I = Ki * e * dt`.

  Derive the unweighted proportional change rather than presenting it as a
  memorized rule. Start with `e_k = SP_k - PV_k`, subtract the preceding error,
  expand both parentheses, and group like terms to obtain
  `e_k - e_(k-1) = delta_SP - delta_PV`. Explain that ordinary incremental PI
  applies Kp to that complete error change. Proportional setpoint weighting is
  then the deliberate replacement of only `delta_SP` by `b * delta_SP`; it is
  not a change to the negative PV-feedback term. Derive the integral increment
  separately as the requested correction rate `Ki * e` multiplied by cycle
  time `dt`, so its percentage-point unit and unchanged true-error path are
  explicit.

  Explain that b equal to one gives the proportional term the complete SP
  change, b equal to zero removes only that reference contribution, and an
  intermediate b retains part of it. The negative `delta_PV` term remains
  unweighted, so proportional feedback to measured plant motion is unchanged.
  Integral action also continues to use the complete true error; b does not
  weaken its eventual offset correction. During a fixed-SP regulatory test,
  delta_SP is zero for every retained automatic update. With identical initial
  state, PV, disturbance, limits, timing, and numerical execution, b therefore
  contributes nothing to the regulatory state update and the three nominal
  regulatory traces are expected to coincide. Bound this prediction to this
  deterministic implementation; finite precision is assessed with a declared
  trace-agreement tolerance rather than the word "identical" alone.
- **Prediction before execution:** With Kp equal to 8 percentage points per
  degree Celsius and an SP increase of 5 degrees Celsius, calculate the
  immediate proportional increment before current integral action. Predict
  40 percentage points for b equal to one, 20 percentage points for b equal to
  one-half, and zero percentage points for b equal to zero. Starting from the
  50-percent operating valve position, predict corresponding preliminary
  nominal positions of 90, 70, and 50 percent before the current integral
  increment and physical limiting. Predict that reducing b will reduce the
  setpoint transient's overshoot but can also slow its initial reference
  response. For the three fixed-80-degree-Celsius regulatory runs, predict a
  maximum aligned PV difference and maximum aligned MV_applied difference no
  greater than their declared numerical trace-agreement tolerance.
- **Controller under test:** Show the concise actual Structured Text that
  configures `Control.PID` with Kp 8, Ki 2, derivative gain zero, b supplied by
  the lesson setting, derivative setpoint weight zero, 50-percent bias,
  Combined anti-windup, 0-through-100-percent limits, plus-or-minus-50-
  percentage-point-per-second rate limits, 1-second tracking time, and 0.1-
  second cycle time. Show SP, PV, Auto/Manual selection, a distinct Manual
  output input, the update call, and the assignment of its constrained result
  to MV_applied. State that b is sampled when the controller is initialized;
  the learner shall restore and restart the simulation after changing b. The
  lesson shall not claim that the configured weight changes a running
  controller instance.
- **Experiment:** The learner-visible Control Studio workflow contains exactly
  Monitor and Test. In Monitor, verify the safe initial state: SP and PV at 80
  degrees Celsius, MV_applied and Manual output at 50 percent, selected weight
  `b = 0.5`, selected objective zero, Manual mode, experiment authority false,
  current quality, 0.1-second sampling, limits, seed, and Simulation evidence
  class. Record the derived 20-percentage-point proportional increment and
  preliminary 70-percent nominal valve position before opening Test.

  Test shall use `Observe without excitation`, disabled zero amplitude, and a
  56.5-second maximum duration. It writes only `P1.ExperimentEnabled` from false
  to true and restores false. The project owns the selected-weight servo
  sequence: Auto at SP 80 degrees Celsius through 5.0 seconds, SP 85 degrees
  Celsius from 5.1 through 55.0 seconds, then Manual output zero from 55.1
  seconds until MV_applied reaches zero. The UI shall not claim that this one
  initialized controller executes all six comparison runs, and it shall not
  expose a disabled Validate step as executable evidence.

  The native evidence owns the complete comparison. It creates six fresh
  runtimes, selecting b in the order one, one-half, and zero for each of servo
  and regulatory objectives before initialization, and drives the same
  project-owned experiment authority. Each regulatory runtime enters Auto at
  fixed SP 80 degrees Celsius, applies the minus-5-degree-Celsius cold-feed
  equivalent at the retained 60.0-second plant boundary, observes the first
  response at 60.1 seconds through 120.0 seconds, and selects Manual zero from
  120.1 seconds until MV_applied reaches zero. Reject the comparison if any
  controller gain, integral path, plant parameter, initial state, event, limit,
  sample schedule, duration, or seed differs other than b. Abort on nonfinite
  required data, mode departure during an automatic window, PV outside 60
  through 100 degrees Celsius, MV_applied outside 0 through 100 percent, or b
  outside zero through one.
- **Measures before results:** For each servo run, include the retained
  5.0-second pre-event sample and samples through 55.0 seconds. Define servo
  overshoot as the largest positive amount by which PV exceeds the 85-degree-
  Celsius SP after the step. Its physical upper requirement is 1.0 degree
  Celsius, its numerical comparison tolerance is 0.01 degree Celsius, and a
  run passes only when its unrounded overshoot is no greater than 1.01 degrees
  Celsius. Define settling time as elapsed time after the 5.0-second event
  boundary until absolute error is no greater than the 0.5-degree-Celsius
  settling-band upper bound plus its 0.01-degree-Celsius comparison tolerance
  and remains there through 55.0 seconds. Report `Not settled` when this
  condition is not met. Settling time has a 0.1-second reproduction tolerance,
  lower is preferred, and no physical acceptance threshold is established in
  this lesson. Define complete-event applied-valve travel as the sum of the
  absolute change in MV_applied from the retained 5.0-second pre-event sample
  through the 55.0-second final Auto sample. It uses percentage points, has a
  0.01-percentage-point reproduction tolerance, lower is preferred, and has no
  physical acceptance threshold. Exclude safe shutdown from automatic-control
  effort. Select the largest b among cases that pass the common servo-
  overshoot requirement; do not hard-code the identity of that case.

  For each regulatory run, use aligned samples from the retained 60.0-second
  pre-event sample through 120.0 seconds. Define regulatory peak deviation as
  the largest absolute PV-minus-SP difference after the disturbance. Its
  validity lower requirement is 1.0 degree Celsius with a 0.01-degree-Celsius
  numerical tolerance, so the excitation is valid when the unrounded peak is
  at least 0.99 degree Celsius. Define recovery time with the same 0.5-degree-
  Celsius band and 0.01-degree-Celsius band-comparison tolerance, requiring it
  to remain satisfied through 120.0 seconds. The regulatory physical upper
  requirement is 40.0 seconds with a 0.1-second numerical tolerance. Define
  final-window absolute error as the mean absolute SP-minus-PV error from
  110.1 through 120.0 seconds, calculated only after recovery qualifies. Its
  physical upper requirement is 0.5 degree Celsius with a 0.001-degree-Celsius
  numerical tolerance. Do not report an arbitrary terminal sample as steady-
  state or final-window performance.

  Define regulatory path disagreement for each candidate against b equal to
  one as the maximum absolute difference between aligned PV samples and,
  separately, the maximum absolute difference between aligned MV_applied
  samples over 60.0 through 120.0 seconds. Each is a numerical implementation-
  equivalence check, not a physical plant-performance requirement. Its upper
  tolerance is `1e-10` in degrees Celsius for PV and `1e-10` percentage point
  for MV_applied; the unrounded difference shall be no greater than that
  tolerance. Report a missing or misaligned sample as `Invalid`, not as zero
  disagreement. Every decision uses unrounded retained values; display
  precision is presentation only.
- **Validation:** Require six complete finite runs, the exact event sequences,
  identical controlled settings, PV and MV_applied within hard limits, and a
  retained final Manual-zero sample after rate-constrained restoration for
  every run. First decide numerical validity and metric eligibility. Then apply
  the common servo requirement to all three weights and select the largest
  eligible b rather than the smallest observed overshoot. Require every
  regulatory run to meet the excitation, recovery, and final-window-error
  criteria. Require both aligned regulatory disagreement measures for b equal
  to one-half and zero to remain within their numerical tolerances. A servo
  selection is invalid if the associated regulatory evidence is missing,
  invalid, or no longer equivalent. Keep numerical validity, physical
  acceptability, regulatory equivalence, and final selection as separate
  dispositions.
- **Evidence and figure:** Retain one trace artifact covering all six runs and
  one qualification summary with one row per run. For every sample retain run
  identity, objective, b, phase, time, event, SP, PV, error, MV_applied,
  disturbance state, mode, Manual output, Kp, Ki, zero derivative gain, bias,
  anti-windup, amplitude and rate limits, cycle time, tracking time, plant model
  identity and parameters, operating point, seed, abort state, safe-restoration
  state, and Simulation evidence class. For every summary row retain status,
  eligibility, overshoot, settling, applied-valve travel, regulatory peak,
  recovery, final-window error, aligned PV and MV_applied disagreement where
  applicable, every threshold and tolerance, controlled-settings disposition,
  physical-criterion disposition, regulatory-equivalence disposition,
  selection disposition, and safe-restoration disposition.

  Generate the teaching figure from the retained native trace. Use aligned
  servo SP/PV and MV_applied panels for the three weights and aligned regulatory
  SP/PV and MV_applied panels for the same weights. Mark the setpoint and load
  events, label real units, keep the Simulation evidence boundary visible, and
  make coincident regulatory traces understandable without falsely implying
  that data are missing. The README comparison table and every plotted value
  shall be generated or reproducible from the retained artifacts; invented or
  manually adjusted curves and copied result constants are forbidden.
- **Explanation and transfer:** Explain a reduced servo overshoot from the
  smaller immediate proportional reference increment, not from weaker PV
  feedback or weaker integral correction. Explain the coincident regulatory
  traces from delta_SP equal to zero and identical controller state, plant
  input, disturbance, and numerical execution. Bound the conclusion to this
  incremental PI form, deterministic simulation, local reactor operating
  region, tuning, limits, and initialization procedure. Setpoint weighting is
  not universally beneficial: a reference-tracking loop may require b equal to
  one, and another plant or requirement may prefer a different compromise.
  Most importantly, b does not filter PV; measurement noise still enters the
  negative proportional feedback term. That unresolved mechanism creates
  Lesson 13's pressure-sensor noise and filtering experiment.

#### Lesson 13 — Measurement filtering: pump pressure loop

- **Objective:** Configure bypass, first-order, and cascaded second-order
  measurement paths for the same simulated centrifugal-pump pressure loop;
  predict their discrete first-sample response and frequency attenuation; use
  three equivalent retained runs to select the weakest filter that satisfies
  the declared pressure-noise and recycle-valve-activity requirements without
  exceeding the allowed measurement-path lag or final-window pressure bias;
  and explain why smoothing cannot reconstruct information lost before
  sampling.
- **Context:** Use a centrifugal pump, discharge-pressure transmitter, and
  recycle control valve. The controlled quantity is pressure deviation in bar
  from the pump's operating point. The manipulated quantity is recycle-valve
  trim in percentage points, limited to `-10..10 percentage points`; positive
  trim increases the modelled pressure deviation. Manual zero trim is the safe
  simulated state. The local pressure response and all of its configured
  parameters remain identical across comparison runs, but its mathematical
  plant structure is not taught here because plant modelling begins in Lesson
  15. It is a reduced-order educational response that omits pump curves,
  cavitation, hydraulic resonance, stiction, transmitter dynamics,
  pre-sampling aliasing, and physical interlocks.
- **Reason:** Lesson 12 reduced the setpoint contribution to the proportional
  term, but it also proved that proportional setpoint weighting does not filter
  the measured process value. Vibration and electrical noise in a discharge-
  pressure transmitter therefore still enter negative proportional feedback
  and create recycle-valve motion even when average pressure tracking is
  satisfactory. That measured limitation creates the need to investigate a
  measurement filter.
- **Theory before practice — measurement and sampling:** Explain in ordinary
  words that the unobserved physical pressure (`P_true`) is the pressure state
  of the simulated plant, measurement noise (`n`) is variation added by the
  transmitter path, the raw process value (`PV_raw`) is the sampled transmitter
  value, and the controller process value (`PV_controller`) is the value after
  the selected measurement path. Only after those meanings and bar units are
  established, write `PV_raw = P_true + n`. The simulator exposes `P_true` on a
  separately labelled evidence-only channel so the lesson can reproduce the
  injected noise; the controller shall never read that channel. A physical
  installation normally does not provide this inaccessible reference, so the
  lesson shall not present filtered pressure as measured plant truth or carry a
  simulator-only true-state claim into physical use.

  Explain that a spectrum describes how the variation of a sampled signal is
  distributed over frequency, while quantization rounds a measurement to
  discrete levels. They are different phenomena. The simulator supplies one
  deterministic, approximately normal broadband noise sequence with standard
  deviation `0.02 bar`. The `0.001 bar` signal resolution is display and
  metadata precision; this lesson does not apply a separate quantizer. It shall
  not claim to demonstrate quantization error.

  Introduce cycle time (`dt`) as `0.1 s`, sampling frequency (`fs`) as
  `1 / dt = 10 Hz`, and Nyquist frequency (`f_N`) as half the sampling
  frequency, `5 Hz`, only after explaining that sampled data cannot uniquely
  represent input content at or above half the sampling frequency. State that
  a digital filter applied after sampling can attenuate recorded high-frequency
  content but cannot undo aliasing that occurred before sampling. Lesson 14
  investigates sampling and timing directly.
- **Theory before practice — the implemented filters:** Explain that a low-pass
  filter retains slow pressure motion while reducing faster measurement
  variation. Introduce filter time constant (`tau_f`) as `0.1 s` and filter
  coefficient (`a`) as the fraction of the current raw-to-filter-state
  difference applied on one update. The actual shipped `Control.LowPass`
  implementation uses
  `a = dt / (tau_f + 0.5 * dt)`, not the simpler backward-Euler expression.
  Derive that coefficient from the continuous lag equation
  `tau_f * dy/dt + y = x`: approximate the derivative by
  `(y[k] - y[k-1]) / dt`, approximate interval output by the midpoint
  `(y[k] + y[k-1]) / 2`, hold the current input at `x[k]`, multiply by `dt`,
  collect the new-output terms, and solve. The resulting update shall visibly
  reduce to `y[k] = y[k-1] + a * (x[k] - y[k-1])` with the shipped `a`.
  After defining previous first-stage output (`y1[k-1]`), current first-stage
  output (`y1[k]`), and current raw measurement (`PV_raw[k]`), write
  `y1[k] = y1[k-1] + a * (PV_raw[k] - y1[k-1])`. The bypass path uses
  `PV_controller[k] = PV_raw[k]`; the first-order path uses
  `PV_controller[k] = y1[k]`; and the cascaded second-order path applies the
  same update to a second state (`y2`) driven by `y1`, then uses
  `PV_controller[k] = y2[k]`.

  Explain continuous reference cutoff frequency (`fc_reference`) as the
  `1 / (2 * pi * tau_f)` value associated with the corresponding continuous
  first-order pole: approximately `1.5915 Hz`. Derive it from
  `abs(H(f)) = 1 / sqrt(1 + (2*pi*f*tau_f)^2)` by explaining that minus three
  decibels corresponds to amplitude `1 / sqrt(2)`, setting the magnitude to
  that value, squaring, and solving for frequency. Do not present that number as
  the exact sampled filter's minus-three-decibel frequency. Explain magnitude
  attenuation in decibels and phase lag in words before plots: one continuous
  pole approaches `20 dB/decade` attenuation and two cascaded poles approach
  `40 dB/decade` away from their corner, but the executed discrete response
  deviates near Nyquist. The retained generated figure shall show the actual
  discrete response computed from `a`; for the declared configuration its
  one-stage and cascaded minus-three-decibel frequencies are approximately
  `1.9591 Hz` and `1.2118 Hz`, respectively.
- **Theory before practice — controller transmission:** Define pressure error
  (`e`) in bar as setpoint (`SP`) minus controller process value
  (`PV_controller`). Define applied manipulated variable (`MV_applied`) as the
  bounded recycle-valve trim calculated by the shipped incremental PI and sent
  directly to the simulated plant. There is no separately exposed preliminary
  request in this lesson, so no requested-output trace shall be invented. With
  proportional gain (`Kp`) `0.667 %/bar`, integral gain (`Ki`)
  `0.667 %/(bar*s)`, zero derivative, zero bias, and `0.1 s` cycle time, a raw
  measurement increment entering `PV_controller` immediately contributes
  `-Kp * delta_PV_controller` to the proportional valve increment. Filtering
  reduces that contribution but delays genuine pressure motion as well.
- **Prediction:** Calculate `a = 0.1 / (0.1 + 0.5 * 0.1) = 2/3`. For a
  `0.10 bar` raw-measurement step from initialized zero, predict a first-stage
  value of `0.0667 bar` and a second-stage value of `0.0444 bar` on the first
  update. Calculate `fs = 10 Hz`, `f_N = 5 Hz`, and
  `fc_reference = 1.5915 Hz`. Predict that controller-PV increment RMS,
  applied-valve increment RMS, and complete-window applied-valve travel decrease
  from bypass to one pole to two poles, while measurement-path lag increases in
  that order. Record these predictions before execution.
  Before practice, also derive every comparison measure. Explain increment RMS
  as square each adjacent difference, average the squares, and take the square
  root; apply it separately to controller PV and applied MV. Define pressure
  and valve attenuation as each candidate RMS divided by the corresponding
  bypass RMS. Define valve travel as the sum of absolute adjacent applied-MV
  changes. Derive lag area as the sum of each signed raw-minus-controller
  pressure gap multiplied by `dt`, and derive final-window error as the absolute
  value of the mean signed SP-minus-controller-PV error. State all windows,
  units, thresholds, tolerances, numerical-equivalence checks, invalidity
  conditions, and the fewest-passing-poles selection rule before the learner
  opens Monitor or Test.
- **Controller under test:** The README shall show the concise actual
  Structured Text that assigns `FilterParameters.TimeConstant`, maps selection
  `0`, `1`, and `2` to bypass, `FirstOrder`, and `SecondOrder`, calls
  `Filter.Update` with the configured cycle time, assigns the chosen value to
  `Input.ProcessValue`, invokes `Controller.Update`, and maps its bounded output
  to the simulated recycle valve. It shall expose the executed `Kp`, `Ki`,
  `Kd`, `tau_f`, `dt`, filter selection, `PV_raw`, `PV_controller`, mode,
  Manual output, and `MV_applied`. The selection is sampled during
  initialization; Control Studio instructions shall require a fresh restart
  after changing it. A selection outside `0..2`, nonpositive `dt` or `tau_f`,
  nonfinite signal, or invalid mode shall remove automatic authority and command
  Manual zero deterministically.
- **Experiment:** The learner-visible workflow contains exactly Monitor and
  Test. In Monitor, verify the safe selected case: SP zero, current raw and
  controller PV near zero, applied MV and Manual output zero, filter selection
  one, Manual mode, experiment authority false, separately labelled simulator-
  only `P_true`, current quality, limits, `0.1 s` timing, seed `20260824`, and
  Simulation evidence class. Record the derived first-sample value `0.0667 bar`
  and the four predeclared qualification requirements before opening Test.

  Test shall use `Observe without excitation`, disabled zero amplitude, and a
  `40.1 s` maximum duration. It writes only `P1.ExperimentEnabled` from false to
  true and restores false. The project owns the selected first-order sequence:
  Auto at SP zero through `5.0 s`, SP one bar from `5.1` through `40.0 s`, and
  Manual zero at `40.1 s`. The UI shall not claim that this initialized filter
  executes all three fresh filter cases, and it shall not expose a disabled
  Validate stage as executable evidence.

  The native evidence owns the complete comparison. It creates three fresh
  deterministic runtimes with selections bypass (`0`), first order (`1`), and
  cascaded second order (`2`) before initialization, then drives the same
  project-owned authority and event sequence. Use the same plant, seed, noise
  realization, PI settings, limits, initial filter state, cycle time, event
  timing, duration, and plot axes. Abort and retain the reason if any configured
  identity differs, any value is nonfinite, `PV_raw` or `PV_controller` leaves
  `-5..5 bar`, `MV_applied` leaves `-10..10 percentage points`, or the selector
  leaves `0..2`.
- **Comparison contract:** Declare every measure before showing a table or
  figure. The steady-noise window is the aligned samples from `20.1` through
  `40.0 s`. Controller-PV increment RMS is the square root of the mean squared
  difference between consecutive `PV_controller` samples in that window, in
  bar. Applied-valve increment RMS uses the same calculation on consecutive
  `MV_applied` samples, in percentage points. Complete-window applied-valve
  travel is `sum(abs(MV_applied[k] - MV_applied[k-1]))` over that window, in
  percentage points; it is a diagnostic with lower preferred and no physical
  acceptance threshold in this lesson.

  Define the response window as aligned samples from `5.1` through `15.0 s`.
  Measurement-path lag area is the signed sum of
  `(PV_raw[k] - PV_controller[k]) * dt` over that window, in `bar*s`. The
  bypass value shall be zero within numerical tolerance. It is a measurement-
  path comparison, not a direct plant dead-time or phase-margin measurement.
  Final-window absolute mean controller error is the absolute value of the
  mean of `SP[k] - PV_controller[k]` from `35.1` through `40.0 s`, in bar. It
  measures average bias in that declared noisy window and shall not be called
  true-plant error or a single-sample steady-state error.

  Relative pressure-noise attenuation is controller-PV increment RMS divided
  by the bypass value. Relative valve-activity attenuation is applied-valve
  increment RMS divided by the bypass value. A candidate satisfies the noise
  and activity requirements when both ratios are no greater than `0.80`, with
  `0.01` numerical comparison tolerance. It satisfies the lag requirement when
  measurement-path lag area is no greater than `0.08 bar*s`, with `0.005 bar*s`
  tolerance, and the pressure-bias requirement when final-window absolute mean
  controller error is no greater than `0.05 bar`, with `0.001 bar` tolerance.
  For each aligned sample, reproduce measurement noise as
  `n[k] = PV_raw[k] - P_true[k]`; those noise realizations from the three fresh
  runs shall agree within `1e-12 bar` to prove deterministic equivalence even
  though different valve trajectories can produce different physical-pressure
  traces. Decisions use unrounded values; RMS and
  mean error display four decimals, lag area four decimals, ratios three
  decimals, and travel two decimals. Selection shall choose the fewest filter
  poles among numerically valid candidates satisfying all four physical
  requirements. Bypass remains the comparison baseline and is ineligible for
  selection because it provides no filtering.
- **Validation and retained evidence:** A native executable test shall compile
  and run the actual Structured Text with the simulator for all three fresh
  cases, verify safe invalid-selection behavior and Manual-zero restoration,
  calculate every measure from aligned samples, prove that the controller never
  reads the simulator-only truth channel, apply the predeclared rule, and
  select the weakest qualifying filter without hard-coding its identity. Retain
  one trace with case, time, SP, `P_true`, reproduced noise, `PV_raw`,
  `PV_controller`, `MV_applied`, mode, selector, filter/controller settings,
  seed, limits, abort state, and evidence
  class; and one three-row summary with unrounded measures, ratios, thresholds,
  tolerances, individual dispositions, overall physical qualification,
  selection, and restoration result. A generated figure shall use only the
  retained trace and declared discrete filter relationship and shall contain
  readable common-axis panels for the pressure response, steady-noise pressure
  detail, applied-valve activity, and actual discrete magnitude response. The
  figure shall not imply that the filtered pressure is measured plant truth.
- **Explanation and transfer:** Explain that each filter state prevents part of
  a rapid raw-measurement change from reaching the PI on the same scan, reducing
  noise-driven valve increments and travel. Each retained state also delays
  real pressure information, so extra poles add lag; the smoothest curve is not
  automatically the soundest engineering choice. Bound the result to this
  sampled implementation, local pump model, noise record, tuning, and operating
  region. On a physical pump, use recorded spectra, transmitter resolution and
  dynamics, process bandwidth, stability margins, valve wear limits, cavitation
  protection, and an independently reviewed topology before authorizing a
  filter change. Filtering after sampling cannot remove aliasing. That remaining
  timing limitation creates Lesson 14. Derivative-specific noise amplification
  remains deferred until derivative action is introduced in Lesson 20.

#### Lesson 14 — Sampling interval and jitter: conveyor drying oven

- **Objective:** The learner shall configure and validate the execution-timing
  path of a simulated conveyor drying-oven temperature loop. From equivalent
  retained runs, the learner shall distinguish actual elapsed time from the
  interval used by the controller, calculate the sampling and aliasing limits,
  prove that measured elapsed time preserves time-scaled PI calculations inside
  a reviewed envelope, and reject a nonpositive or missed-scan interval without
  silently resuming automatic heat.
- **Physical context and boundary:** The real-world system is one temperature
  zone of a conveyor drying oven. A product-zone temperature sensor reports the
  temperature near the moving product, an electrical power stage changes heater
  power, and a belt-speed measurement identifies the operating condition. The
  controlled quantity is temperature deviation from the normal `120 degC`
  operating point. The manipulated quantity is heater-power trim about the
  normal holding power. Both may be positive or negative within the local
  operating envelope because the lesson commands a trim around an independently
  established holding point; a negative trim means less heating, not active
  cooling. The simulated temperature-deviation range is `-10..+10 degC`, the
  heater-trim range is `-10..+10 percentage points`, and belt speed remains
  `0.5 m/s`. Manual zero trim is the declared safe simulated output. The
  executable plant is deliberately a reduced local response. Its mathematical
  structure is not derived or identified in this lesson; that work begins in
  Lessons 15 and 16. The simulation omits spatial temperature gradients,
  moisture dynamics, radiation, power-stage switching, sensor dynamics,
  scheduler causality, plant trips, and physical interlocks. Every result is
  labelled **Simulation evidence** and shall not authorize changing a physical
  PLC task period or oven controller.
- **Reason from Lesson 13:** Lesson 13 showed that a measurement filter trades
  high-frequency attenuation for response lag and that filtering performed
  after sampling cannot reconstruct information that was aliased before the
  controller received it. That retained limitation creates the present
  question: what does the controller actually calculate when its update times
  are slower or irregular? The same gains and samples do not define the same
  digital controller if elapsed time is treated incorrectly.
- **Theory before practice — physical signal path:** Explain in ordinary words
  that the oven temperature changes continuously while a PLC observes it only
  at update instants. Between those instants the power stage continues applying
  the last output; this behavior is a zero-order hold. A timing diagram shall
  show continuous temperature, sample instants, sampled values, and held heater
  output before any frequency notation is introduced. Explain that a longer
  hold delays the next possible correction and that irregular holds create
  timing jitter. The diagram is explanatory; measured response curves shall
  still come from the executable lesson.
- **Theory before practice — names and units:** Define the product-zone
  temperature-deviation setpoint (SP) as the desired deviation in degrees
  Celsius and the process value (PV) as the measured deviation in degrees
  Celsius. Define the temperature error (e) as desired deviation minus measured
  deviation in degrees Celsius. Define requested manipulated-variable trim
  (MV_requested) as the output calculated by the PI controller in percentage
  points. Define applied manipulated-variable trim (MV_applied) as the held
  heater trim presented to the simulated oven in percentage points. Define the
  actual cycle time (dt_actual) as elapsed timestamp time between two executed
  controller updates in seconds. On a physical PLC those timestamps come from
  the reviewed scheduler clock; in this lesson the executable harness supplies
  the declared deterministic intervals. Define the controller cycle time
  (dt_controller) as the interval passed into the controller's time-scaled
  calculations in seconds. Define the nominal cycle time (dt_nominal) as the
  configured `0.1 s` value used by the deliberately incorrect baseline even
  when actual elapsed time differs. Define sampling frequency (fs) as the
  number of samples per second in hertz and Nyquist frequency (f_N) as half the
  sampling frequency in hertz. Define timing jitter (j) as actual cycle time
  minus nominal cycle time in seconds. Define a missed scan as an actual cycle
  time greater than the reviewed maximum `0.25 s`; a nonpositive interval is
  invalid rather than an unusually fast scan.
- **Theory before practice — governing relationships:** After the preceding
  physical definitions, state `e = SP - PV`. Explain that integral action adds
  an amount proportional to the present error and the time for which that error
  acted. First derive the correction rate `r_I = Ki * e`, cancel the
  degree-Celsius units to obtain percentage points per second, and then derive
  `Delta MV_I = r_I * dt_controller = Ki * e * dt_controller` from change equal
  to rate times elapsed time. Define the contribution in percentage points and
  explicitly use the unit cancellation to explain why using half the actual
  interval creates half the required contribution. Derive sampling frequency
  by asking how many intervals of length `dt_actual` fit in one second before
  writing `fs = 1 / dt_actual`. Explain that one sinusoidal cycle has a positive
  and negative half and therefore requires at least two samples before writing
  `f_N = fs / 2`. Derive each update's jitter as
  `j_k = dt_actual,k - dt_nominal`, including the meanings of positive and
  negative signs. Explain aliasing as a high-frequency physical variation
  producing indistinguishable sampled values at a lower apparent frequency.
  Correct `dt` arithmetic changes neither the already sampled values nor the
  zero-order-hold delay and therefore cannot undo aliasing or make slow sampling
  equivalent to fast sampling. Explain that derivative differences and actuator
  rate limits are also time-scaled, but do not introduce derivative tuning or a
  rate-limited actuator in this experiment.
- **Implemented controller form:** Use the incremental `Control.PID` in PI
  configuration with proportional gain `Kp = 0.667`,
  integral gain `Ki = 0.667 %/(degC*s)`, derivative gain `Kd = 0`, proportional
  setpoint weight `b = 1`, zero bias, `-10..+10 percentage-point` output limits,
  and the declared `Control.PID` combined anti-windup behavior. The normal path
  passes `dt_actual` as `dt_controller`. A selectable, explicitly labelled bad
  baseline passes `dt_nominal = 0.1 s` while retaining the true timestamped
  `dt_actual`; it exists only to demonstrate the defect and shall never be the
  selected result. In a valid run the ideal simulated power stage makes
  `MV_applied = MV_requested` at each controller update and holds that applied
  value until the next update. Both values shall still be retained separately
  so the hold and the safety path are observable.
- **Timing validity and recovery:** A normal update requires finite signals and
  `0 < dt_actual <= 0.25 s`. The implementation shall not clamp an invalid
  interval into the valid range or pass it to `Control.PID.Update`. A
  nonpositive interval, an interval above `0.25 s`, a nonfinite timing or loop
  signal, or a signal outside its declared hard limit shall latch a visible
  timing fault, command `MV_requested = MV_applied = 0`, and preserve the fault
  until an explicit reset is requested while the input is valid and automatic
  authority is absent. Reset shall reinitialize the controller from the current
  SP, PV, and safe output so stale incremental state cannot create a return
  bump. Automatic operation shall require a separate explicit enable after
  successful reset. This is simulator behavior, not a substitute for a real
  scheduler watchdog, safety system, or reviewed restart procedure.
- **Worked prediction:** Before execution, isolate the integral calculation by
  initializing the controller with `SP = 1 degC`, `PV = 0 degC`, and zero stored
  output, so the first incremental proportional contribution is zero and
  `e = 1 degC`. With `Ki = 0.667 %/(degC*s)`, predict integral contributions of
  `0.03335`, `0.0667`, and `0.1334 percentage points` for actual intervals of
  `0.05`, `0.1`, and `0.2 s`. Predict that the bad nominal-time baseline at an
  actual `0.2 s` interval produces only `0.0667 percentage point`, half the
  required contribution. Predict sampling/Nyquist pairs of `20/10 Hz`,
  `10/5 Hz`, and `5/2.5 Hz`. For the deterministic sequence
  `0.02, 0.18, 0.05, 0.15, 0.10 s`, predict zero mean jitter over its
  `0.50 s` period but individual jitter values of `-0.08, +0.08, -0.05,
  +0.05, 0.00 s`; zero mean does not make the individual holds uniform.
- **Controller under test:** The README shall show the concise actual
  Structured Text that validates `dt_actual`, selects measured or deliberately
  nominal `dt_controller`, passes it to `Control.PID.Update`, exposes
  `MV_requested`, applies and holds `MV_applied`, latches an invalid interval,
  performs the reviewed reset, and requires explicit automatic re-enable. The
  excerpt shall use the shipped names and API. The configured `100 ms` task in
  the example is metadata for the nominal schedule; the retained actual
  timestamps and test harness establish the deliberately varied elapsed-time
  runs and shall not be presented as proof that the simulator caused operating-
  system scheduler jitter. The shipped project shall default to Manual zero,
  `TimingCase = 1`, and false `ExperimentEnabled`. When that Boolean authority
  rises, the project shall own the selected measured-100-millisecond sequence:
  Auto baseline through scan 10, the `1 degC` setpoint step at scan 11, invalid
  `0.3 s` exposure at scan 301, valid-but-latched evidence at scan 302, Manual
  reset at scan 303, reset-removed Manual evidence at scan 304, separate Auto
  re-enable at scan 305, and SP-zero Manual restoration from scan 306. Removing
  Test authority shall also restore SP zero, valid `0.1 s` timing, reset false,
  and Manual zero.
- **Equivalent experiment:** Use fresh controller and plant state for every
  comparison. Hold the same executable plant revision, controller gains,
  `1 degC` setpoint step at `1.0 s`, zero initial deviation and trim, belt speed
  `0.5 m/s`, `30 s` physical duration, limits, and seed. First run the isolated
  first-update prediction at `0.05`, `0.1`, and `0.2 s`. Then run complete
  measured-time responses at each fixed interval. Run one complete measured-
  time response with the repeating deterministic jitter sequence
  `0.02/0.18/0.05/0.15/0.10 s`. Run the deliberately bad nominal-time response
  with actual `0.2 s` and controller `0.1 s`. Finally use separate fresh cases
  to request `0.30 s` and `0 s`, prove the latched safe state, restore a valid
  `0.1 s` input while showing that automatic operation does not resume, then
  exercise explicit reset and re-enable. No comparison shall change both timing
  and tuning.
- **Control Studio workflow:** The learner-visible workflow contains exactly
  **Monitor** and **Test**. In Monitor, confirm equipment identity, Simulation
  label, SP/PV units, requested and applied trim, belt speed, actual and
  controller interval `0.1 s`, use-actual-time true, timing valid, fault latch
  false, `TimingCase = 1`, Manual mode, false experiment authority, limits,
  quality, and controller revision. Before action, derive the `0.7337 pp` first
  PI request at scan 11 and predict every latch/reset/re-enable boundary. In
  Test, select **Observe without excitation**, disabled zero amplitude, and
  `30.6 s`; review and write only `P1.ExperimentEnabled`, require false
  restoration, and retain scans 10, 11, and 301 through 306. The generic panel
  shall not write SP, timing selection, actual interval, Auto, or reset. The
  native test shall use separately initialized runtimes for fixed, jittered,
  nominal-time, and invalid-interval comparisons and shall not imply that the
  one UI runtime executed those cases. Identify, Tune, and Validate are not
  used because no model is estimated, no tuning changes, and no executable
  shared-candidate validation action exists.
- **Decision measures declared before results:** Calculate first integral
  contribution from the first valid update in percentage points and require the
  `0.05/0.1/0.2 s` measured-time cases to equal
  `0.03335/0.0667/0.1334` within `1e-9 percentage point`. Calculate timing-use
  error as the maximum absolute difference between `dt_controller` and
  `dt_actual` over a complete run in seconds; measured-time cases require no
  more than `1e-12 s`, while the bad baseline is expected to show `0.1 s` at
  its `0.2 s` update and is ineligible. Calculate peak overshoot as positive PV
  excursion above the `1 degC` target divided by the `1 degC` step and report
  percent; a valid complete response requires no more than `20 percent` with
  `0.1 percentage-point` numerical tolerance. Define settling before using a
  final error: after the setpoint event, PV must enter and remain within
  `+/-0.02 degC` of SP through the end of the run; settling time is the first
  such timestamp minus the event timestamp and requires no more than `8.0 s`
  with `0.1 s` numerical tolerance. A run that fails this rule reports
  `Not settled` and receives no final-window error. For a settled run, final-
  window absolute mean error is the sample-interval-weighted mean of `|SP-PV|`
  over physical time `25.0..30.0 s` and requires no more than `0.02 degC` with
  `0.001 degC` tolerance. Calculate requested and applied trim total variation
  across the complete retained run in percentage points; it is an actuator-activity
  diagnostic with no physical acceptance threshold in this simulated lesson.
  Belt speed shall equal `0.5 m/s` throughout within `1e-12 m/s`. All decisions
  use unrounded retained values; display first contributions to five decimal
  places, intervals to three decimals, temperatures and trims to three
  decimals, overshoot to one decimal, and total variation to two decimals. The
  README shall teach these meanings, derivations, windows, units, thresholds,
  tolerances, eligibility order, and unrounded-decision rule in Prediction
  before Implementation and before the learner opens Monitor or Test; the
  Evidence section applies rather than introduces them.
- **Validity and selection:** A complete run is numerically valid only when its
  timestamps are strictly increasing, all values are finite, actual intervals
  agree with timestamp differences within `1e-12 s`, the declared physical
  duration and event schedule are preserved, and SP, PV, requested/applied trim,
  and belt speed remain within their hard limits. It is timing-qualified only
  when every interval is inside the reviewed envelope and measured mode passes
  actual elapsed time to the controller. Apply response criteria only after
  numerical and timing validity, then classify `Qualified`, `Not settled`,
  `Timing fault`, `Invalid`, or `Aborted`. The selected implementation is
  measured elapsed time with the latched invalid-interval path; no fixed sample
  interval is declared universally best. Fixed interval and jitter cases prove
  the qualified envelope, while the nominal-time case is retained counter-
  evidence rather than a selectable candidate.
- **Retained executable evidence:** Retain one sample record containing run and
  case identity, timestamp, interval index, actual/nominal/controller interval,
  jitter, SP, PV, error, requested/applied trim, belt speed, timing mode,
  validity, fault/reset/enable state, limits, plant/controller revision, seed,
  and Simulation evidence class. Retain one summary record per case containing
  validity state, first integral contribution where applicable, timing-use
  error, overshoot, settling state/time, final-window error where eligible,
  requested/applied total variation, fault/recovery result, thresholds,
  tolerances, and each decision. The native test shall recalculate every
  reported value and decision from the sample record rather than trust
  precomputed labels. A generated teaching figure shall show the sample/hold
  timing concept, equivalent PV responses on a common scale, requested/applied
  held output, and actual-versus-controller interval for fixed, jitter, bad
  nominal, and fault/recovery cases. The generator and source artifacts shall
  ship with the lesson.
- **Required conclusion and transfer:** The execution interval is part of the
  implemented controller. Timestamp-derived elapsed time preserves integral
  and other time-scaled calculations only inside a reviewed range; it cannot
  remove zero-order-hold delay, restore aliased information, or guarantee that
  a slow controller has adequate stability margin. Mean-zero jitter is not
  equivalent to a uniform interval. An invalid interval is a supervisory fault,
  not a number to clamp, and recovery requires explicit authority. These runs
  describe timing sensitivity but do not explain why the oven has its observed
  gain, response speed, and delay. That remaining predictive limitation creates
  Lesson 15, where the learner derives a physical thermal model before using
  model terminology to interpret plant behavior.

#### Lesson 15 — First-principles thermal model: water heater

- **Objective:** Derive a local dynamic model of a well-mixed recirculating
  water heater from stated equipment properties, use it to predict a bounded
  temperature response, and decide whether the configured simulator behaves
  consistently with that model without presenting the result as identified or
  physical-plant evidence.
- **Context:** The plant is a well-mixed recirculating vessel with a one-sided
  0-to-200-kilowatt electric heater, a 0-to-100-percent power command, an
  outlet-temperature sensor, and effective ambient and fixed-flow heat loss.
  The initial water and ambient temperatures are 20 degrees Celsius. The
  bounded simulated experiment requests 20 percent heater power, keeps water
  temperature within 0 to 80 degrees Celsius, and restores the heater request
  and applied heater power to 0 percent at the end. The README shall show a
  simple energy-boundary diagram after this physical explanation and before
  the balance equation.
- **Reason:** Lesson 14 established that the execution interval affects a
  response, but timing alone does not explain why a thermal plant has its
  observed equilibrium change, response speed, or transport delay. Earlier
  water-heater runs therefore cannot predict a changed operating condition
  from equipment properties. This unresolved limitation creates the need for
  a physical model before any identification tool is opened.
- **Theory before notation:** Explain conservation as stored thermal energy
  increasing by the heater energy entering and decreasing by heat leaving.
  Introduce water temperature (T) in degrees Celsius, elapsed time (t) in
  seconds, temperature rate (dT/dt) in degrees Celsius per second, thermal
  capacitance (C) in kilojoules per degree Celsius, heater heat input (Qin) in
  kilowatts, heat-loss coefficient (H) in kilowatts per degree Celsius,
  ambient temperature (Tambient) in degrees Celsius, and rated heater power
  (P100) in kilowatts before any of those symbols appears in an equation.
  Explain that one kilowatt is one kilojoule per second and use that fact to
  check that every term in the balance has units of kilowatts.
- **Model derivation:** Only after the physical dictionary and unit check,
  define stored energy relative to a fixed reference as `E = C(T-Tref)` and
  differentiate it to obtain accumulation `dE/dt = C*dT/dt`. Derive linear
  heat loss as `Qloss = H(T-Tambient)`, verify both terms in kilowatts, and only
  then substitute them into conservation to write
  `C * dT/dt = Qin - H * (T - Tambient)`. Explain the
  requested heater command (MV_requested) and the physically applied heater
  command (MV_applied) in percent before writing
  `Qin = (P100 / 100) * MV_applied`. The heater can add heat but cannot actively
  cool, so the applied command is the requested command limited to the physical
  0-to-100-percent range. Define equilibrium temperature (Teq) as the
  temperature at which stored energy no longer changes, then set the
  accumulation term to zero, show `0 = Qin-H(Teq-Tambient)`, move the loss term,
  divide by `H`, and derive `Teq = Tambient + Qin/H`. Derive static gain (K) by
  applying one percentage point, obtaining `P100/100` kilowatts, and dividing
  its equilibrium effect by `H`, which gives `K = (P100/100)/H`. Derive response
  speed by subtracting the equilibrium balance, defining `x=T-Teq`, writing
  `C*dx/dt=-H*x`, dividing by `C`, and recognizing that the decay rate `H/C`
  has reciprocal time constant `tau=C/H`. Write the remaining-fraction
  exponential and substitute one time constant to show
  `e^-1=0.367879` remaining and `1-e^-1=0.632121` completed. Define apparent
  dead time (L) as
  the delay before the measured response begins and state that it is a separate
  transport approximation, not a consequence of this one-state energy balance.
- **Assumptions and validity:** State that the model assumes one perfectly
  mixed temperature, constant material properties, constant heater efficiency,
  fixed recirculation flow, linear effective heat loss, and operation in the
  declared 20-to-24-degree-Celsius and 0-to-20-percent region. It omits
  stratification, changing flow, boiling, nonlinear radiation, sensor dynamics,
  and actuator faults. Agreement with a simulator configured from the same
  parameters is a consistency check of the derivation and implementation; it
  is not independent validation, identification evidence, or physical-plant
  evidence.
- **Executable safety ownership:** The typed runtime process-image boundary
  rejects a nonfinite `LREAL` before the Structured Text scan executes; the
  lesson shall test and report that rejection without claiming that an ST
  comparison handled the value. For finite values that reach the lesson, the
  ST shall accept thermal capacitance and effective heat loss only above zero
  and no greater than `1e6` in their stated units, rated heater power from zero
  through `1e6 kW`, ambient temperature and process value from 0 through 80
  degrees Celsius, and requested heater power from `-1000` through `1000`
  percent before applying the physical 0-to-100-percent clamp. A value outside
  that diagnostic envelope invalidates the scan and commands zero applied
  heater power. These deliberately generous diagnostic ceilings prevent
  nonphysical arithmetic from being presented as a valid model; they are not
  physical operating approval.
- **Prediction before practice:** From `C = 50 kJ/degC`, `H = 10 kW/degC`,
  `P100 = 200 kW`, and `Tambient = 20 degC`, calculate `K = 0.2 degC/%`,
  `tau = 5 s`, and 40 kilowatts of heat input at 20 percent applied power.
  Calculate `Teq = 24 degC`. After defining the first-order exponential as the
  fraction of the initial temperature difference that remains, predict that a
  one-second delay followed by one time constant produces about 63.2 percent
  of the four-degree change, or `22.53 degC` at six seconds.
- **Controller under test:** Show a concise excerpt of the actual shipped
  Structured Text that validates the physical parameters, derives gain, time
  constant, and equilibrium, preserves requested heater power separately from
  applied heater power, clamps the one-sided actuator, and commands safe zero
  applied power when the model inputs are invalid. A Test-authority enable
  shall be required before nonzero power can be applied. A stale nonzero
  requested-power value without Boolean authority shall not energize the
  simulated heater. While authority is true, the project shall own the reviewed
  20-percent request; when authority is removed, the project shall set the
  request to zero, expose Manual mode, and command zero applied power.
  The names in the prose, excerpt, I/O bindings, artifacts, and plots shall
  agree.
- **Experiment:** The learner-visible Control Studio workflow contains exactly
  Monitor and Test. Before running, record the numerical predictions and inspect
  the energy boundary, units, initial state, evidence label, hard limits, and
  safe state. In Control Studio, use Monitor to verify the 20-degree-Celsius
  initial process value, 0-percent requested and applied power, valid model,
  100-millisecond interval, and Simulation provenance. In Test, approve a
  project-scripted observation with disabled zero amplitude and `30.1 s`
  maximum duration. Review and write only `P1.ExperimentEnabled`; the project
  shall create the deterministic 0-to-20-percent requested-power step for 30
  seconds with seed `20260824`. Do not change a plant or model parameter during
  the run. At `30.1 seconds`, remove Test authority and finish with a retained
  Manual-mode sample in which requested and applied heater power are both 0
  percent. Validate and Identify are not used: no executable shared-candidate
  validation action exists and no unknown parameter is estimated. The native
  test owns recalculation against configured simulation truth.
- **Decision measures:** Define every measure before results. Derived gain,
  time constant, and equilibrium shall be compared with their worked values
  using the unrounded executable values and a numerical tolerance of `1e-9` in
  their stated units. The six-second prediction error is the absolute
  difference between simulated process value and `22.52848 degC` at exactly
  six seconds; its upper acceptance bound is `0.08 degC` with `0.001 degC`
  numerical tolerance. The final-window equilibrium error is the mean absolute
  difference between process value and `24 degC` from `29.0` through `30.0`
  seconds inclusive. It is eligible only when every sample is finite, requested
  and applied heater power remain at 20 percent throughout the active step,
  no hard limit is violated, and the process-value peak-to-peak range in that
  final window is no greater than `0.004 degC`; otherwise report `Not settled`
  rather than an equilibrium error. Its upper acceptance bound is `0.02 degC`
  with `0.001 degC` numerical tolerance. Display temperature to two decimals,
  gain to three decimals, time to two decimals, and power to two decimals;
  display rounding shall not decide acceptance. The README shall define and
  derive the measures, windows, eligibility order, thresholds, tolerances, and
  unrounded-decision rule in Prediction before Implementation and before the
  learner opens Monitor or Test. Evidence shall apply rather than introduce
  those rules.
- **Retained evidence and figures:** Retain time, process value, model-predicted
  temperature, requested heater power, applied heater power, model validity,
  input validity, Test authority, Manual mode, all physical parameters and
  their units, derived parameters, equilibrium, seed, limits, event state,
  restoration, separate model and configured-simulation provenance,
  assumptions, validity region, metric eligibility, unrounded measures,
  tolerances, and decisions. Generate the comparison table and a
  common-scale prediction-versus-simulation overlay from those retained
  samples. The overlay shall mark the one-second delay, six-second prediction,
  24-degree equilibrium, active step, and restoration, and shall identify the
  model curve as a derived prediction and the other curve as configured
  simulation truth.
- **Validation and explanation:** Recalculate every reported measure and
  decision from retained samples. Explain agreement through the energy balance:
  larger thermal capacitance stores more energy and slows the response, larger
  loss requires more heater power and reduces static gain, and dead time shifts
  the response without changing the derived equilibrium. A mismatch shall lead
  to checking units, assumptions, program wiring, actuator limits, and simulator
  configuration; it shall not be hidden by changing the derived parameters.
- **Required conclusion and transfer:** A first-principles model can explain
  and predict a local self-regulating thermal response when its parameters and
  assumptions are known. This configured-truth agreement does not establish
  those parameters for a real heater. The remaining problem is therefore how
  to estimate gain, time constant, and delay from a safe measured experiment;
  that evidenced need creates Lesson 16.

#### Lesson 16 — Step-response identification: same water heater

- **Objective:** Design and execute one bounded open-loop experiment on the
  recirculating water heater, qualify the immutable recording, estimate and
  compare first-order and first-order-plus-dead-time models from measured
  time/input/output data only, and explain why the selected preview still
  requires independent validation.
- **Context:** Use the same one-sided 0-to-200-kilowatt heater, outlet-
  temperature sensor, and 0-to-100-percent power stage as Lesson 15, but at the
  local 30-degree-Celsius and 20-percent operating point declared by this
  simulation configuration. The controller-facing program and Control Studio
  shall not read the configured plant gain, time constant, or dead time. The
  experiment produces measured Simulation evidence and identified-model
  evidence; configured simulation truth may be revealed only after fitting as
  a provenance-labelled teaching comparison. Neither evidence class is
  physical-plant evidence.
- **Reason:** Lesson 15 predicted the response from known thermal capacitance,
  heat loss, heater rating, and delay. An installed heater may not have
  trustworthy values for those quantities. The learner therefore needs a safe
  way to estimate combined local input/output dynamics from measurements. The
  unresolved limitation is different from validation: fitting the same record
  used for estimation cannot prove prediction on another experiment.
- **Theory before notation:** Explain an open-loop step test as a deliberate
  actuator change made while feedback cannot hide the plant response. Introduce
  the baseline input (u0) as the mean applied heater command before the step in
  percent, new input (u1) as the mean applied command after the step in percent,
  and input step amplitude (delta_u) as their difference in percentage points.
  Introduce baseline output (y0) as the mean outlet temperature before the step
  in degrees Celsius, final output (y1) as the settled mean after the step, and
  output change (delta_y) as their difference in degrees Celsius. Only then
  write `delta_u = u1 - u0`, `delta_y = y1 - y0`, and
  `K = delta_y / delta_u`, where process gain (K) is measured in degrees
  Celsius per percentage point.
- **Reaction-curve construction:** Explain apparent dead time (L) as the
  elapsed seconds from the applied input step until the measured response is
  distinguishable from its baseline. For this deterministic lesson, use the
  first retained sample at or beyond 2 percent of the qualified output change,
  with the preceding sample retained as the crossing bracket. Explain the
  63.2-percent response level in physical words before its calculation. The
  model time constant (T) is the elapsed seconds from the end of apparent delay
  to that crossing. Only after gain, delay, and time constant are understood
  from the curve may the lesson introduce the first-order-plus-dead-time model
  (FOPDT) and write
  `T * dy/dt + y = y0 + K * [u(t - L) - u0]`. Derive that equation by
  subtracting the baseline, holding the delayed input change constant after
  the step, and solving the resulting exponential decay. Show that
  `y(t) - y0 = K * delta_u * (1 - exp(-(t-L)/T))` for `t >= L`. Its settled
  change is `K * delta_u`; one time constant after the delay its completed
  fraction is `1 - exp(-1) = 0.632120...`. The learner shall therefore derive
  the 63.2-percent construction rather than memorise an unexplained rule.
- **Data qualification:** Define qualification before reporting a model. Every
  estimation sample shall contain immutable time, requested input, applied
  input, measured output, units, quality, clipping, saturation, operating mode,
  authority state, and evidence provenance. The analysis record shall contain
  exactly 500 active samples from 0.10 through 50.00 seconds at a declared
  0.10-second interval, followed by a separate restoration sample at 50.10
  seconds that is not fitted. Retained analysis timestamps shall be finite and
  strictly increasing; median interval shall be 0.10 seconds with no gap above
  0.15 seconds; quality shall be good; and clipping and saturation counts shall
  be zero. The last two seconds before the step shall have absolute baseline
  drift no greater than `0.001 degC/s`. Applied-input span shall be at least
  `9.99 percentage points`, output span at least `1.90 degC`, and signal-to-
  noise ratio at least `20 dB`. A failed prerequisite shall produce
  `Unqualified` with its reason and no model ranking.
- **Uncertainty and residuals:** The lesson shall not call a single deterministic
  record a statistical confidence interval. It shall expose measurement-
  resolution envelopes. Baseline and final-output sample bounds produce the
  gain envelope. The retained samples immediately before and at the 2-percent
  crossing bound apparent delay. The samples immediately before and at the
  63.2-percent crossing, combined with the delay bracket, bound the time
  constant. Explain that these envelopes account for sampled crossing and
  endpoint resolution only; sensor noise, parameter covariance, structural
  mismatch, and operating-range variation remain unresolved. Introduce the
  prediction residual (r[k]) as measured temperature minus model-predicted
  temperature at sample k, then define root-mean-square residual in degrees
  Celsius and normalized root-mean-square error relative to qualified output
  span before results are shown. Derive root-mean-square residual by squaring
  signed residuals so cancellation cannot hide error, averaging the `N`
  squared residuals, and taking the square root to restore degrees Celsius.
  Then derive normalized error by dividing RMSE by the qualified output span,
  which cancels the temperature unit. Explain every qualification and
  selection measure, its unit, its calculation, and why its limit is needed
  before the practical workflow begins.
- **Worked prediction:** Before opening Test, calculate that the
  reviewed 20-to-30-percent step has a 10-percentage-point input change. Use
  Lesson 15's provenance-labelled model only for experiment review to predict
  an approximately 2-degree-Celsius output change, a final value near 32
  degrees Celsius, and a 63.2-percent value near 31.264 degrees Celsius about
  six seconds after the applied step. State explicitly that these values are
  withheld from fitting and cannot be used to select or modify a model.
- **Controller under test:** Show the concise actual Structured Text that keeps
  the shared `Control.PID` in Manual mode and routes the approved experiment
  through the ordinary controller-facing output. Requested heater power and
  applied heater power shall be separate signals. The runnable project shall
  ship an explicit `control-loop.json` that binds the project-owned Manual
  output request as a read-only MV, the PID output as the applied actuator, the outlet temperature
  as PV, and a false Auto-mode signal so Control Studio reports Manual mode.
  Control Studio Test shall write one Boolean `P1.ExperimentEnabled` authority
  and no process setpoint or Manual-output amplitude. Its false-to-true
  transition starts the project-owned sequence: retain 20-percent baseline
  power for scans 1 through 100, apply 30 percent for scans 101 through 500,
  and restore 20 percent when Test restores authority false at 50.10 seconds.
  Before and after the approved experiment, the request binding shall contain
  the reviewed 20-percent operating value. A
  safety abort shall override both request and restoration and command zero
  applied power. The program shall retain the request for diagnosis, clamp
  applied power to the physical 0-to-100-percent range, and expose Manual mode,
  input validity, and abort state. Typed nonfinite process-image values shall
  be rejected by the runtime boundary; finite PV and request values outside
  declared limits shall fail safe in the program.
- **Experiment and exact UI workflow:** The learner workflow contains exactly
  `Monitor -> Test`; it shall not direct the learner to fictional or disabled
  Identify or Validate stages. In Monitor, verify 30.00 degrees Celsius PV, requested and
  applied power at 20.00 percent, Manual mode, no Test authority, good/fresh
  quality, 100-millisecond sampling, Simulation provenance, and hard limits.
  In Test, choose `Observe without excitation`, with amplitude zero and maximum
  duration 50.10 seconds, review the sole `P1.ExperimentEnabled` Boolean write,
  declare the hypothesis, record the worked prediction, and approve a
  deterministic run with seed `20260827`. The project shall retain 20-percent baseline power
  through 10.00 seconds, apply the approved 30-percent Manual request at 10.10
  seconds, and retain the response through 50.00 seconds. Restore the same
  request binding to 20 percent at 50.10 seconds and prove the applied Manual
  output returns to 20 percent. Any
  nonfinite or bad-quality signal, timestamp gap, PV outside 0-to-40 degrees
  Celsius, requested input outside 0-to-100 percent, applied clipping or
  saturation, baseline-drift failure, or unapproved event shall abort to zero
  and make the recording ineligible. After Test completes, the learner shall
  inspect the project-owned retained qualification, candidate, residual, and
  selection evidence; fitting is not represented as an extra UI stage. A physical heater requires independently
  reviewed protection/topology and immediate explicit approval before the
  step; simulation approval never transfers.
- **Model comparison and selection:** After Test, the project-owned evidence
  contract shall qualify the immutable record before fitting. It shall fit at
  least a no-delay first-order candidate and an FOPDT
  candidate from the same retained samples and units. Each candidate shall
  retain its parameters, stability, predicted samples, residual series,
  root-mean-square residual, normalized error, resolution envelopes, operating
  range, warnings, and model provenance. The normal preference is the simplest
  stable finite candidate whose normalized error is no greater than `0.05`.
  The resolved-dead-time rule overrides the simplicity preference: select the
  FOPDT candidate when its dead time exceeds two sample intervals and its
  normalized error is at least 2 percent lower than the no-delay first-order
  error. Otherwise retain the simpler eligible candidate. If neither candidate
  is eligible, report `Identification failed`; do not manufacture a model or
  reveal configured truth to alter the fit.
- **Teaching comparison with configured truth:** Only after the selected preview
  is frozen may the project-owned evidence reveal the simulator configuration. Compare estimated
  gain with `0.2 degC/%` using `0.005 degC/%` tolerance, apparent delay with
  `1.0 s` using `0.2 s` tolerance, and time constant with `5.0 s` using `0.3 s`
  tolerance. These comparisons demonstrate recovery in this deterministic
  teaching run; they are not fitter inputs and are not physical acceptance
  criteria. All decisions use unrounded retained values. Display temperature
  and power to two decimals, gain to three decimals, time to two decimals,
  residual measures to four significant engineering digits, and signal-to-
  noise ratio to one decimal; display rounding shall not decide eligibility.
- **Retained evidence and figures:** Retain run identity, project and source
  revision, seed, evidence classes, sample interval, complete active samples,
  separate restoration sample, requested/applied power, PV, quality, modes,
  authority, limits, aborts, exclusions, qualification metrics and decisions,
  baseline/final windows, input/output spans, candidate definitions, every fit,
  selected preview, crossing brackets, parameter envelopes, predicted samples,
  residuals, errors, operating range, warnings, and the `validation required`
  state. Generate an annotated reaction-curve teaching diagram and a measured-
  versus-predicted response with residual panel from retained evidence. Tables
  and figures shall distinguish measured Simulation evidence, identified-model
  prediction, and configured truth.
- **Validation and required conclusion:** Review the complete retained evidence
  chain, not future predictive performance. The learner shall be able to reconstruct
  the qualification decision, reaction-curve parameters, candidate errors,
  selection rule, resolution envelopes, restoration, and provenance from the
  retained artifacts. The selected model shall remain an unaccepted preview
  with `validation required`; Tune shall remain closed. Identification can
  estimate a useful local model when physical parameters are unknown, but fit
  to its own estimation record does not prove prediction on new data. That
  measured limitation creates Lesson 17's independent validation experiment.

#### Lesson 17 — Independent model validation: same water heater

- **Objective:** Use new data without refitting to accept the local command
  model, restrict its disturbance purpose, and retain uncertainty, residuals,
  range, and verdict.
- **Reason and equipment:** Lesson 16 could fit one recording without predicting
  a different experiment. Use the same electric recirculating water heater,
  outlet-temperature sensor, 0-to-100-percent heater power stage, and cold-flow
  load. Outlet temperature uses degrees Celsius and heater power uses percent.
  The configured simulator provides the new measured record; the frozen model
  provides only predictions. Neither is physical-plant evidence.
- **Theory before practice:** Explain that estimation data determine a model,
  while independent validation data are an immutable recording excluded from
  fitting. Introduce the sample number `(k)`, measured process value
  `(PV_measured)` in degrees Celsius, model-predicted process value
  `(PV_predicted)` in degrees Celsius, and prediction residual `(r[k])` in
  degrees Celsius before writing
  `r[k] = PV_measured[k] - PV_predicted[k]`. Derive root-mean-square residual
  as `RMSE = sqrt(sum(r[k]^2)/N)`: squaring prevents signed cancellation,
  division by `N` forms a mean, and the square root restores degrees Celsius.
  Derive residual bias as `bias = sum(r[k])/N`, which deliberately preserves
  direction. Derive lag-one residual correlation from the mean-centred
  covariance of successive residual sequences divided by the product of their
  standard deviations; explain why normalization bounds it from -1 to 1 and
  explain both signs. Correlation is a diagnostic, not proof
  that residuals or a model are statistically white or true. Explain a
  parameter uncertainty interval, the resulting prediction envelope, declared
  acceptance tolerance, interpolation, extrapolation, and the distinct
  verdicts accepted, restricted, and rejected before displaying results.
  Explain that the prediction envelope is constructed by evaluating the frozen
  model over retained parameter-bound combinations at every timestamp and
  taking the minimum and maximum prediction. It is a sample-resolution
  envelope, not a probability or universal coverage promise. Every
  qualification measure, calculation window, tolerance, and decision shall be
  taught before the practical workflow.
- **Frozen-model and evidence contract:** Load Lesson 16's selected model,
  parameter intervals, model/run identity, sample interval, units, validity
  region, and provenance. Preserve separate estimation and validation run IDs.
  The validation path shall not change model structure or parameters. Retain
  every qualified timestamp, applied input, measured value, predicted value,
  lower and upper prediction-envelope values, residual, qualification decision,
  declared limit, calculated measure, verdict, supported purpose, operating
  range, warning, abort, restoration, and provenance needed to reconstruct the
  decision. A plot or table shall distinguish measured Simulation evidence,
  identified-model prediction, and the uncertainty envelope.
- **Controller under test:** Apply the validation input through the ordinary
  `Control.PID` Manual path so the plant sees the same controller-facing output
  used elsewhere. Keep the requested validation input distinct from applied
  heater power. Control Studio Test writes only the Boolean
  `P1.TestAuthorityEnabled`. While it is true, the project owns the complete
  sequence: 20-percent power for scans 1 through 100 and 25-percent power for
  scans 101 through 600. A stale direct write to `ExperimentInput` shall not
  bypass that sequence. Safety abort, invalid input, nonfinite data,
  bad quality, a timing gap, changed frozen parameters, reused estimation data,
  or a hard-limit violation shall command zero and make the record ineligible.
  Removing Test authority after a qualified run shall restore Manual operation
  at 20 percent; safety abort shall override that restoration with zero.
- **Worked prediction:** Freeze the retained model gain `K = 0.1998606 degC/%`,
  time constant `T = 4.9 s`, and dead time `L = 1.0 s`. For a new
  20-to-25-percent input step, derive the prediction from
  `y_hat = y0 + K * delta_u * (1 - exp(-(t-t_step-L)/T))` after the delay.
  Predict a 30.9993-degree-Celsius fixed-flow equilibrium and approximately
  30.639 degrees Celsius six seconds after the step. Because the model has no
  cold-flow input, predict approximately -1.00 degree Celsius residual bias
  after a cold-flow event lowers the plant equilibrium by 1.00 degree Celsius.
- **Practical flow:** The exact learner workflow is `Monitor -> Test`; it shall
  not advertise Identify or a disabled Validate stage. In Monitor, verify model identity, run independence,
  frozen parameters, Test authority, Manual mode, SP/PV/MV bindings, units,
  quality, timing, hard limits, evidence class, aborts, and restoration. In
  Test, choose `Observe without excitation`, amplitude zero, and maximum
  duration 60.10 seconds. Review the sole Boolean authority write. The project
  shall retain a 20-percent baseline through 10.00 seconds, apply 25 percent at 10.10 seconds, evaluate fixed-flow behavior through 29.00
  seconds, retain the configured cold-flow event at 30.00 seconds, and continue
  through 60.00 seconds without refitting. At 60.10 seconds remove Test
  authority and prove Manual restoration at 20 percent. After Test, inspect the
  project-owned retained measured/predicted/envelope overlay and time-aligned
  residual plot. Keep axes, event times, windows, and evidence classes visible.
- **Decision contract:** Accept the model for local fixed-flow heater-command
  prediction only when the independent 11.00-to-29.00-second root-mean-square
  residual is no greater than `0.03 degC`, the run is qualified and independent,
  all parameters remained frozen, and the run stayed within 20-to-25-percent
  heater power and 30-to-31-degree-Celsius temperature. Retain lag-one
  correlation as a diagnostic with its exact window, not as a universal
  whiteness claim. When the 50.00-to-60.00-second residual bias magnitude is
  between `0.90 degC` and `1.10 degC`, restrict the model from cold-flow load
  prediction. Reject any purpose whose declared limits fail. Carry the accepted
  purpose and restriction into tuning eligibility.
- **Required conclusion:** A low estimation error did not validate the model.
  Independent data support local fixed-flow command prediction, while the
  structured residual exposes the missing disturbance path. Only the supported
  model purpose is eligible for Lesson 18; validation never proves a model
  universally true.

#### Lesson 18 — Model-based PI tuning: same water heater

- **Objective:** Choose a closed-loop response target, calculate PI gains from
  the validated FOPDT model, and compare them with empirical tuning under
  nominal and uncertain conditions.
- **Reason and equipment:** Lesson 7 required repeated empirical trials, while
  Lesson 17 now supplies a validated but explicitly restricted heater-command
  model. Use the same water heater, temperature sensor, heater power stage, and
  cold-flow load. Candidate calculation is identified-model evidence;
  performance runs are configured-simulation evidence; neither is physical-
  plant evidence.
- **Theory before practice:** Introduce desired closed-loop response time
  `(lambda)` in seconds as the engineering speed target; model gain `(K)` in
  degrees Celsius per percentage point; model time constant `(T)` and dead time
  `(L)` in seconds; proportional gain `(Kp)` in percent per degree Celsius;
  integral time `(Ti)` in seconds; and integral gain `(Ki)` in percent per
  degree-Celsius-second. Explain that decreasing `lambda` normally increases
  speed and gains while reducing tolerance to delay, noise, uncertainty, and
  actuator constraints. Only then present the eligible FOPDT PI rule
  `Kp = T / [K * (lambda + L)]`, `Ti = min[T, 4 * (lambda + L)]`, and
  `Ki = Kp / Ti`. Derive the proportional rule from the invertible first-order
  model, a desired `1/(lambda*s+1)` response, and the first-order delay
  approximation `exp(-L*s) ~= 1-L*s`, which produces the effective
  `(lambda+L)` response horizon. Show that the resulting ideal PI has
  `Ti = T`; then explain the SIMC disturbance-rejection modification
  `Ti = min[T, 4*(lambda+L)]` and both branches of that minimum. Derive
  `Ki = Kp/Ti` by equating the coefficient of the integral term in ideal and
  parallel PI forms, with unit cancellation. Explain model uncertainty, a representative boundary case,
  integral absolute error `(IAE)` as accumulated absolute control error in
  degree-Celsius-seconds, accumulated manipulated-variable travel in percentage
  points, saturation exposure, recommendation, validation, explicit Apply
  approval, and rollback before any calculation or run. Derive IAE as the sum
  of retained absolute-error rectangles `sum(abs(e[k])*delta_t)`, MV travel as
  the sum of absolute applied-output changes including departure from safe
  zero, saturation exposure as the count of samples on an active qualification
  limit times sample interval, and percentage improvement from baseline minus
  candidate divided by baseline. Define every calculation window, tolerance,
  eligibility rule, and tie rule before the practical workflow.
- **Eligibility and calculation:** Tune only from Lesson 17's accepted local
  fixed-flow command purpose and retain its model ID, provenance, validity
  restriction, parameter intervals, and warnings. Reject nonfinite parameters,
  nonpositive gain or time constant, negative dead time, nonpositive response
  target, values outside the declared engineering input bounds (`K` no greater
  than `1 degC/%`, `T` no greater than `3600 s`, `L` no greater than `600 s`,
  and `lambda` no greater than `3600 s`), dimensionally inconsistent inputs, or
  requirements that cannot be met inside the declared actuator and model-
  validity limits. The mapped process-image boundary shall reject nonfinite
  values before the controller executes. Rejection shall produce no usable
  gains and shall never change the running controller.
- **Worked prediction:** For `K = 0.2 degC/%`, `T = 5.0 s`, `L = 1.0 s`, and
  `lambda = 4.0 s`, calculate `Kp = 5.0 %/degC`, `Ti = 5.0 s`, and
  `Ki = 1.0 %/(degC*s)`. Compare those values with Lesson 7's empirical
  `Kp = 4.0 %/degC` and `Ki = 0.8 %/(degC*s)`. Predict a faster nominal response
  and greater actuator activity before execution.
- **Controller under test and approval:** The shipped Structured Text shall use
  the ordinary `Control.PID` PI configuration for both candidates and expose
  the selected gains, response target, tuning validity, requested output, and
  applied output. During this local model qualification, controller output shall
  remain inside the accepted 20-to-25-percent heater-command range; the physical
  actuator retains its separate 0-to-100-percent hard range. Calculating or
  selecting a candidate shall not itself apply it. A run requires explicit
  simulation-run approval; without approval, after abort, or with invalid
  inputs, the controller shall remain or return to Manual safe zero. A physical
  run additionally requires independently reviewed topology and immediate
  explicit approval; simulation approval never transfers.
- **Exact interactive workflow:** The learner workflow shall contain exactly
  `Monitor -> Test`; it shall not advertise Identify, Tune, or Validate stages
  that do not execute this project's selected run. Test shall use `Observe
  without excitation`, amplitude zero, maximum duration `120.1 s`, and one
  Boolean `P1.ExperimentEnabled` authority. On its false-to-true transition the
  project shall select Lambda tuning, generate the candidate, retain the
  prequalified validation disposition, apply it only to this Simulation
  controller, and enable the 120.0-second run. When Test restores authority
  false at `120.1 s`, the project shall remove run approval, roll back the
  empirical gains, and restore Manual safe zero. The native evidence owns the
  empirical, selected, and perturbed/noisy fresh-runtime comparison; the UI
  shall not imply one running controller produced all three cases.
- **Controlled comparison:** Freeze the plant, initial state, 100-millisecond
  sample interval and 120-second duration. Start at `30.2 degC` with `20%`
  heater power, request `30.6 degC`, and at 60 seconds lower the no-heat
  equilibrium from `30.2 degC` to `29.8 degC`. This deliberately keeps the
  complete experiment inside Lesson 17's accepted `30–31 degC` process-value
  and `20–25%` applied-command range while still exercising setpoint tracking
  and disturbance rejection. Freeze the seed, limits, anti-windup, and axes.
  First run empirical `Kp = 4.0` and `Ki = 0.8`; then run the calculated
  candidate with a separate run ID and the same plan. Retain complete SP, PV,
  requested MV, applied MV, mode, authority, quality, timing, limits, aborts,
  and restoration. Compare 0-to-60-second tracking IAE, 60-to-120-second load
  IAE, final settled error, overshoot, settling, saturation time, and
  accumulated MV travel. Display a common-axis response plot and a compact
  decision table; do not hide actuator cost.
- **Uncertainty validation:** Repeat the selected candidate with declared model
  gain `0.19 degC/%`, time constant `6.0 s`, dead time `1.3 s`, and deterministic
  `0.03 degC` measurement noise. Require finite signals, applied MV between 0
  and 100 percent, at least 2-percent improvement in both declared nominal IAE
  intervals, final 110-to-120-second mean absolute error no greater than
  `0.20 degC`, and accumulated perturbed MV travel no greater than 300 percentage
  points. Use unrounded retained values for decisions and sensible display
  precision for teaching.
- **Verdict and required conclusion:** Retain both candidates and every failed
  criterion. The calculated tuning is an unapplied recommendation until all
  nominal and uncertainty gates pass and a separate Apply action is explicitly
  approved. A validated simple model makes tuning reproducible and exposes the
  speed-versus-robustness decision; it does not remove validation, extend the
  model's purpose, or guarantee that PI is suitable for different dynamics.

### 3.2 Motion-control spine

#### Lesson 19 — P position control: servo positioning table

- **Objective, reason, and equipment:** Tune a proportional position controller
  experimentally and explain why increasing position stiffness eventually
  causes unacceptable overshoot, oscillation, and motor effort. Use a
  motor-driven industrial positioning table represented by one rigid rotational
  inertia, encoder position and velocity feedback, viscous friction, and a
  bounded motor drive. This is configured-simulation evidence, not physical-axis
  commissioning evidence. The transition from Lesson 18 is deliberate: unlike
  the self-regulating heater, the positioning table stores kinetic energy and
  can pass its target.
- **Theory before practice:** Define setpoint `r` and measured position `theta`
  in radians, error `e=r-theta`, requested torque in newton metres, applied
  torque after the drive clamp, proportional gain `Kp` in newton metres per
  radian, inertia `J` in kilogram-square-metres, viscous damping `B` in
  newton-metre-seconds per radian, angular velocity, natural frequency, damping
  ratio, overshoot, and oscillation before using their equations. Begin with
  the rotational balance `J*theta'' + B*theta' = torque`. For unsaturated P
  feedback substitute `torque=Kp*(r-theta)`, define displacement from the fixed
  target `x=theta-r`, and derive `J*x''+B*x'+Kp*x=0`. Divide by `J` and compare
  with the standard second-order form
  `x''+2*zeta*omega_n*x'+omega_n^2*x=0` to derive
  `omega_n=sqrt(Kp/J)` and `zeta=B/[2*sqrt(J*Kp)]`. Explain that `Kp` is a
  virtual spring stiffness: increasing it raises natural frequency but, when
  physical `B` and `J` stay fixed, lowers damping ratio. For the eligible
  underdamped unsaturated model, derive the ideal fractional overshoot
  `M_p=exp[-pi*zeta/sqrt(1-zeta^2)]` from the standard second-order response and
  identify it as a prediction rather than a substitute for the sampled result.
- **Worked prediction:** With `J=1 kg*m^2`, `B=1 N*m*s/rad`, and a one-radian
  step, calculate initial torque as `Kp*1 rad`: `2 N*m` for `Kp=2` and `8 N*m`
  for `Kp=8`, both below the `10 N*m` clamp. Calculate
  `omega_n=sqrt(2)=1.414 rad/s`, `zeta=1/(2*sqrt(2))=0.354` for the baseline and
  `omega_n=sqrt(8)=2.828 rad/s`, `zeta=1/(2*sqrt(8))=0.177` for the change.
  Predict the four-times-higher stiffness to double natural frequency, halve
  damping ratio, shorten rise time, increase overshoot, and require more peak
  torque.
- **Measures and decisions before practice:** Define 90-percent rise time as
  the timestamp of the first retained `PV>=0.9 rad` sample, overshoot as
  `max(PV-SP)` over the complete run, and peak applied torque as
  `max(abs(MV))`. Retain velocity and the sign changes of position error so
  oscillation is visible even though no independent oscillation-count gate is
  claimed. Freeze a 20-millisecond sample interval, common initial position and
  velocity, one-radian setpoint, plant, limits, axes, and seed. Require the
  high-gain rise-time improvement to be greater than zero with `0.001 s`
  tolerance, additional overshoot to be at least `0.1 rad` with `0.001 rad`
  tolerance, and absolute applied torque to remain at most `10 N*m` with
  `0.01 N*m` tolerance. Use unrounded retained samples for decisions.
- **Implementation and exact interactive workflow:** Use the shipped
  `Control.PID` with `Ki=Kd=0`, retain requested-versus-applied torque semantics,
  and start in Manual safe zero. The complete learner workflow shall contain
  exactly `Monitor -> Test`; it shall not advertise a disabled Validate stage.
  Test shall use `Observe without excitation`, amplitude zero, maximum duration
  `20.02 s`, and one Boolean `P1.ExperimentEnabled` authority. On its rising
  edge, the project shall configure `Kp=8 N*m/rad`, set `SP=1 rad`, initialize
  from the current measured state, and enter Auto. While authority remains true,
  the project owns that selected high-gain run. On false restoration, it shall
  restore `Kp=2`, `SP=0`, Manual mode, and applied torque zero. Invalid position
  or velocity and safety abort shall fail closed. Native evidence shall own the
  low/high comparison in fresh equivalent runtimes; the UI shall not imply the
  one selected running controller produced both traces.
- **Evidence and required conclusion:** Retain the common-axis position and
  torque traces, velocity, gain, timing, limits, rise time, overshoot, peak
  torque, restoration, and configured-simulation provenance. The expected
  retained comparison is approximately `1.30 s` versus `0.56 s` rise time,
  `0.313 rad` versus `0.596 rad` overshoot, and `2.00 N*m` versus `8.00 N*m`
  peak torque. Conclude that gain improves virtual stiffness and speed, but
  cannot create velocity-dependent damping; inadequate damping motivates the
  derivative velocity feedback introduced in Lesson 20.

#### Lesson 20 — PD position control: same positioning table

- **Objective and causal reason:** Add and tune derivative action on the same
  motor-driven industrial positioning table, then explain how velocity feedback
  reduces overshoot without changing proportional stiffness. Lesson 19 derived
  that raising `Kp` raises natural frequency but lowers damping ratio when plant
  damping is fixed. Keep `J`, `B`, `Kp=8`, plant, move, limits, timing, and seed
  fixed so only derivative damping changes. Results remain Simulation evidence.
- **Theory before practice:** Define measured angular velocity `omega` in
  radians per second, derivative gain `Kd` in `N*m*s/rad`, proportional torque,
  derivative torque, requested torque, applied torque, derivative setpoint
  weight, phase lead, damping ratio, overshoot, and late position span before
  calculation. With derivative setpoint weight zero, derive
  `tau_D=-Kd*omega`: the sign makes torque oppose either direction of motion and
  avoids differentiating a discontinuous setpoint step. Substitute
  `tau=Kp*(r-theta)-Kd*theta'` into `J*theta''+B*theta'=tau`, use
  `x=theta-r`, and derive `J*x''+(B+Kd)*x'+Kp*x=0`. Compare with the standard
  second-order equation to prove `omega_n=sqrt(Kp/J)` is unchanged and
  `zeta=(B+Kd)/[2*sqrt(J*Kp)]` increases. For sinusoidal motion derive phase
  lead from `d[A*sin(omega*t)]/dt=A*omega*cos(omega*t)`: velocity leads position
  by one quarter cycle. Derive the energy result from
  `E=0.5*J*omega^2+0.5*Kp*x^2`, giving
  `dE/dt=-(B+Kd)*omega^2`; derivative feedback removes mechanical energy when
  `Kd>=0` but does not change spring stiffness.
- **Prediction and measures:** With `J=B=1`, `Kp=8`, calculate
  `zeta=0.177` for `Kd=0` and `zeta=0.707` for `Kd=3`; natural frequency remains
  `2.828 rad/s`. At the step instant velocity is zero, so both cases request
  `8 N*m`; at error `1 rad` and velocity `+1 rad/s`, the PD request is
  `8-3=5 N*m`. Define overshoot as `max(PV-SP)` over the complete 15-second
  run, late position span as `max(PV)-min(PV)` over 5–15 seconds, overshoot
  ratio as `PD overshoot/P overshoot`, and late-span ratio likewise. Explain
  why ratios are dimensionless and require the same plan. Freeze tolerances
  before practice: PD overshoot at most `0.100 rad` with `0.001 rad` tolerance,
  overshoot ratio at most `0.25` and late-span ratio at most `0.20`, each with
  `0.01` tolerance, and absolute applied torque at most `10 N*m` with
  `0.01 N*m` tolerance.
- **Exact workflow and implementation:** Use `Control.PID`, `Kp=8`, `Ki=0`,
  derivative on measurement, and Manual safe zero initially. The learner
  workflow shall be exactly `Monitor -> Test`, with no fictional disabled
  Validate stage. Test uses `Observe without excitation`, amplitude zero,
  maximum duration `15.02 s`, and one `P1.ExperimentEnabled` Boolean. Its rising
  edge configures `Kd=3`, SP `1 rad`, initializes from measured state, and enters
  Auto. Its falling edge restores `Kd=0`, SP zero, Manual mode, and zero torque.
  Invalid position or velocity and safety abort fail closed. Fresh native
  runtimes own the P/PD comparison; one UI run shall not claim both traces.
- **Evidence and conclusion:** Retain position, velocity, requested/applied
  torque, gains, timing, limits, overshoot, late span, ratios, restoration, and
  Simulation provenance. Conclude that derivative velocity feedback adds
  damping and removes motion energy without changing proportional stiffness.
  At zero error and zero velocity PD supplies no nonzero equilibrium torque, so
  the constant-load offset introduced in Lesson 21 remains unsolved.

#### Lesson 21 — PID position control: loaded positioning table

- **Objective and causal reason:** Add integral action to the damped positioning
  table and prove that it supplies the equilibrium torque missing from PD under
  angle-dependent payload gravity. Continue directly from Lesson 20: keep `J`, `B`,
  `Kp=8`, `Kd=3`, derivative on measurement, limits, timing, and seed fixed;
  add the robot-joint model's angle-dependent gravity torque
  `T_g=G*sin(theta)` with peak coefficient `G=4 N*m`, and change only `Ki`.
  The coefficient is not a constant four-newton-metre disturbance: gravity
  torque is zero at zero angle and about `1.918 N*m` at `0.5 rad`. Results
  remain Simulation evidence, and a physical gravity-loaded axis requires
  mechanical support or a brake before motor torque is removed.
- **Theory before practice:** Define gravity torque, integral state, integral gain,
  proportional/integral/derivative torque, requested/applied torque, equilibrium,
  phase lag, windup, anti-windup, settled qualification, final-window mean
  absolute error, overshoot, saturation exposure, and recovery time before
  calculation. At rest derive the torque balance
  `0=T_controller-G*sin(theta_ss)`, hence controller torque must equal the
  gravity torque at the actual angle. For PD, velocity and derivative torque
  are zero, so `Kp*(r-theta_ss)=G*sin(theta_ss)`. This nonlinear equation has
  no elementary closed-form solution. Derive the small-angle prediction by
  using `sin(theta) approximately theta`:
  `theta_ss approximately Kp*r/(Kp+G)` and
  `e_ss approximately G*r/(Kp+G)`. Derive integral accumulation from
  `I(t)=integral(e(t) dt)` using one retained rectangle per cycle:
  `I[k]=I[k-1]+e[k]*dt`; show that `rad*s` times
  `Ki [N*m/(rad*s)]` gives torque. At PID equilibrium, `e=omega=0` and
  `theta_ss=r`, so derive `Ki*I_ss=G*sin(r)` and
  `I_ss=G*sin(r)/Ki`. Explain that the stored state, not current error, now
  supplies the angle-dependent holding torque.
- **Integral dynamics and stability:** Starting from the loaded rotational
  balance and PID torque, differentiate the constant-reference homogeneous
  equation or use `I'=e=-x`. Linearize gravity about target `r` with
  `sin(r+x) approximately sin(r)+cos(r)*x`; the constant equilibrium term
  cancels and the local characteristic polynomial is
  `J*s^3+(B+Kd)*s^2+(Kp+G*cos(r))*s+Ki=0`. Integral action adds a third
  state/pole and phase lag. For positive coefficients, show the cubic Routh
  first-column condition `(B+Kd)*(Kp+G*cos(r))>J*Ki`; it explains why
  increasing `Ki` without bound can destroy damping even though integral
  action removes offset. The implemented
  clamp limits applied torque to `[-10,+10] N*m`; windup is accumulation while
  requested and applied torque differ, and configured anti-windup drives the
  stored state toward the achievable output rather than making saturation
  harmless.
- **Prediction and measures:** With `G=4 N*m`, `r=0.5 rad`, and
  `Kp=8 N*m/rad`, derive the small-angle PD prediction
  `theta_ss approximately 0.3333 rad` and `e_ss approximately 0.1667 rad`;
  the exact numerical equilibrium is about `theta_ss=0.3354 rad` and
  `e_ss=0.1646 rad`. With `Ki=2 N*m/(rad*s)`, derive the PID holding torque
  `4*sin(0.5)=1.9177 N*m` and equilibrium integral state `0.9589 rad*s`.
  Calculate the local stability comparison
  `(B+Kd)*(Kp+G*cos(0.5)) approximately 46.04` versus `J*Ki=2`. Define
  final-window MAE over 28–30 seconds as
  the arithmetic mean of retained `abs(SP-PV)` samples, report it only when PV
  range is at most `0.010 rad` and applied-torque range at most `0.050 N*m` in
  that same window, and define the PID/PD error ratio under the common plan.
  Define conventional overshoot as `max(0,max(PV-SP))`. For the separate native
  recovery case, define saturation exposure as saturated-sample count times
  `0.02 s` and recovery as elapsed time after the 5-second restoration until
  the first 50 consecutive samples (one second) within `0.02 rad` with torque
  below `9.9 N*m`. Freeze tolerances and limits before practice: PD MAE at least
  `0.100 rad`, PID MAE at most `0.010 rad` and at most one tenth of PD, PID
  overshoot at most `0.150 rad`, and absolute applied torque at most `10 N*m`.
- **Exact workflow and implementation:** Start in Manual at zero angle, where
  simulated gravity torque and Manual motor torque are both zero, with `Ki=0`
  and SP zero. The learner workflow shall be
  exactly `Monitor -> Test`, with no fictional disabled Validate stage. Test
  uses `Observe without excitation`, amplitude zero, maximum duration `30.02 s`,
  and one `P1.ExperimentEnabled` Boolean. Its rising edge configures `Ki=2`, SP
  `0.5 rad`, initializes from measured state with the holding output, and enters
  Auto. Its falling edge restores `Ki=0`, SP zero, Manual mode, and zero torque
  in Simulation. Safety abort or invalid position/velocity leaves automatic
  control; a physical axis must be mechanically supported before a zero-torque
  state is used. Fresh native runtimes own the PD/PID comparison and the
  unreachable-2-rad saturation/recovery case; one UI run shall not claim all
  three traces.
- **Evidence and conclusion:** Retain SP, PV, velocity, requested/applied torque,
  gains, gravity model and coefficient, timing, limits, settled qualification, MAE, error ratio,
  overshoot, saturation exposure, recovery, restoration, and Simulation
  provenance. Conclude that integral memory supplies the constant equilibrium
  torque while derivative action continues to damp motion. Integral action
  adds phase lag and windup risk; the abrupt setpoint in Lesson 22 exposes the
  separate derivative-kick problem.

#### Lesson 22 — Derivative kick and setpoint weighting: same positioning table

- **Objective and causal reason:** Keep the gravity-loaded table and PID gains
  from Lesson 21, expose the command kick created when derivative action sees a
  discontinuous setpoint, and select derivative on measurement without removing
  physical-motion damping. Preserve the actual robot-joint gravity law
  `T_g=4*sin(theta) N*m`; do not relabel its peak coefficient as a constant
  four-newton-metre disturbance.
- **Theory before practice:** Define derivative setpoint weight `c`, weighted
  derivative input `x_D=c*SP-PV`, elapsed time, finite-difference derivative
  rate, derivative torque, two-degree-of-freedom control, requested torque,
  applied torque, first post-step request, and final-sample error before
  calculation. Derive
  `Delta x_D=c*Delta SP-Delta PV`. On the first sample after the command step,
  the table has not yet moved, so `Delta PV=0` and
  `v_D=Delta x_D/dt=c*Delta SP/dt`. Then derive
  `T_D=Kd*c*Delta SP/dt`. Explain that the ideal continuous derivative of a
  step is an impulse, while the implemented sampled controller represents it
  as a finite value inversely proportional to cycle time. At `c=1` this is
  derivative on full error; at `c=0`, `x_D=-PV`, a setpoint step contributes
  nothing but later measured motion still produces opposing damping torque.
- **Prediction and measures:** For `Delta SP=0.5 rad`, `dt=0.02 s`, and
  `Kd=3 N*m*s/rad`, derive derivative requests `75`, `37.5`, and `0 N*m` for
  `c=1`, `0.5`, and `0`. Derive the same first-cycle proportional request
  `Kp*e=8*0.5=4 N*m` and integral increment
  `Ki*e*dt=2*0.5*0.02=0.02 N*m`, giving complete first requests `79.02`,
  `41.52`, and `4.02 N*m`. Define first post-step request as the first
  controller result after SP changes, first applied torque after the separate
  `[-10,+10] N*m` clamp, and final-sample absolute position error at 15 seconds
  as an endpoint check rather than settled evidence. Freeze limits and
  tolerances before practice: full-error request at least `70 N*m`, intermediate
  minus measurement request at least `30 N*m`, measurement-only request at most
  `5 N*m`, absolute applied torque at most `10 N*m`, and measurement-only final
  error at most `0.020 rad`.
- **Exact workflow and implementation:** Start at zero angle in Manual zero,
  with `c=1` as the rejected diagnostic baseline. The learner workflow shall be
  exactly `Monitor -> Test`, with no fictional disabled Validate stage. Test
  uses `Observe without excitation`, amplitude zero, maximum duration `15.02 s`,
  and one `P1.ExperimentEnabled` Boolean. Its rising edge configures selected
  `c=0`, initializes at the unchanged zero setpoint and measured state, then
  applies SP `0.5 rad` in Auto so the first selected request remains directly
  comparable. Its falling edge restores `c=1`, SP zero, Manual mode, and zero
  requested/applied torque. Invalid inputs and safety abort fail closed. Fresh
  native runtimes own the `c=1`, `0.5`, and `0` comparison; one UI run shall
  not claim all three traces.
- **Evidence and conclusion:** Retain SP, PV, velocity, requested/applied torque,
  all gains and weights, timing, gravity model, limits, first post-step samples,
  final endpoint error, restoration, and Simulation provenance. Conclude that
  derivative setpoint weighting changes command response without removing the
  feedback damping caused by measured PV motion. Requested and applied torque
  answer different questions, and the encoder noise introduced in Lesson 23
  still enters derivative on measurement.

#### Lesson 23 — Filtered derivative: encoder-based servo

- **Objective and causal reason:** Continue with derivative on measurement from
  Lesson 22 and select a measurement filter from quantified noise attenuation
  and added lag. Preserve the robot-joint law
  `J*theta_ddot=T_applied-B*theta_dot-G*sin(theta)`: `G=4 N*m` is the peak
  gravity coefficient, not a constant payload torque, and gravity is zero at
  the initial zero angle. Define the noisy encoder as
  `PV_raw=theta+n`, with zero-mean seeded noise of standard deviation
  `sigma=0.002 rad`.
- **Theory and derivation before practice:** With derivative setpoint weight
  zero, derive the sampled derivative contribution
  `T_D,k=-Kd*(PV_control,k-PV_control,k-1)/dt`. For a stationary table this
  becomes `-Kd*(n_k-n_k-1)/dt`, showing why the same noise change produces
  twice the torque at half the cycle time. For independent noise, derive the
  approximate derivative-torque RMS `Kd*sqrt(2)*sigma/dt=0.424 N*m`. Then
  derive the consecutive derivative-torque-change estimate from coefficients
  `1,-2,1` as `Kd*sqrt(6)*sigma/dt=0.735 N*m`; explain why the complete
  closed-loop measurement need not equal that noise-only estimate.
- **Filter derivation:** Begin with `tau*dy/dt=x-y`. Write
  `y_k=y_k-1+Delta y`, approximate the scan rate by `Delta y/dt` and the
  within-scan output by `y_k-1+Delta y/2`, and solve to obtain
  `Delta y=[dt/(tau+0.5*dt)]*(x_k-y_k-1)`. Define the dimensionless shipped
  coefficient `a=dt/(tau+0.5*dt)` and update
  `y_k=y_k-1+a*(x_k-y_k-1)`. At `dt=0.02 s` and `tau=0.05 s`, derive
  `a=0.3333`; a `0.003 rad` gap therefore moves the output `0.001 rad` on the
  first scan. Explain that later scans continue closing the remaining gap.
  Define first order as one stage and second order as two identical cascaded
  stages. Derive the continuous first-order characteristic cutoff
  `f_c=1/(2*pi*tau)=3.18 Hz` for `tau=0.05 s`, while making the sampled
  closed-loop experiment authoritative for selection.
- **Measures and prediction:** Derive torque-increment RMS as
  `sqrt(sum((T_k-T_k-1)^2)/N)` over 15 to 20 seconds; squaring prevents sign
  cancellation and the square root restores N*m. Derive positive lag area as
  `sum(max(PV_raw-PV_control,0)*dt)` over 0 to 5 seconds; its unit is rad*s and
  its one-sided definition prevents leading and lagging differences from
  cancelling. State that its noisy raw reference makes it an
  experiment-specific comparison, not a universal delay estimate. Define
  final mean error separately over 18 to 20 seconds. Predict that increasing
  time constant or order reduces activity and increases lag. Freeze
  `RMS_DeltaT<=0.250 N*m` and `A_lag<=0.020 rad*s`; among passing candidates,
  choose the simplest order and then the smallest time constant.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  `Monitor -> Test`, with no fictional disabled Validate stage. The false
  project state is filter bypass, SP zero, Manual mode, zero requested/applied
  torque, and `P1.ExperimentEnabled=FALSE`. Test uses
  `Observe without excitation`, amplitude zero, and maximum duration
  `20.02 s`; its sole reviewed write is that Boolean authority. The rising edge
  initializes filters at the measured state, selects first order with
  `tau=0.05 s`, applies SP `0.5 rad`, and enters Auto. The falling edge restores
  the complete false state. Invalid values and safety aborts fail closed.
  Fresh native runtimes own bypass, first-order `0.02/0.05/0.10 s`, and
  second-order `0.05 s`; the selected UI run shall not claim the full matrix.
- **Evidence and conclusion:** Retain raw and controller-facing PV, SP,
  requested/applied torque, filter order, time constant, sample time, seed,
  gravity model, limits, torque-increment RMS, lag area, final mean error,
  terminal state, restoration, and Simulation provenance. The retained values
  select first-order `0.05 s`: it reduces bypass activity by 78 percent while
  remaining below the lag limit. Smoother `0.10 s` and second-order traces fail
  that limit. Conclude that derivative filtering is an engineered tradeoff;
  it attenuates noise before differentiation but delays real feedback.

#### Lesson 24 — Mechanical actuator nonidealities: same positioning table

- **Reason:** A controller that works on an ideal servo may fail on the real
  transmission.
- **Theory before practice:** Torque, velocity and acceleration limits;
  Coulomb friction; deadband; backlash; stiction; how each appears in PV and MV;
  the difference between a motion-profile limit, a drive-torque limit, and a
  mechanical transmission defect; and why retuning cannot repair every defect.
- **Implemented plant behavior:** Use the same robot-joint positioning table and
  controller-facing I/O as Lessons 19–23. Velocity and acceleration limits
  shape the position reference in the Structured Text motion profile. Torque
  amplitude and torque slew belong to the ordinary actuator. Coulomb friction
  belongs to the joint equation. Command deadband and stiction belong to the
  actuator. Mechanical backlash uses the Specification 33 `backlash` process-
  output play operator and is explicitly identified as a reduced-order
  transmission approximation rather than command deadband.
- **Theory derivations and prediction:** Begin with rotational Newton's law
  sum(T)=J*alpha and solve alpha=sum(T)/J. At zero angle and speed, explain why
  gravity and viscous friction are zero and derive the local drive-only upper
  bound 2.5 N*m / 1 kg*m2 = 2.5 rad/s2; do not present it as the whole move's
  acceleration. Derive the per-scan profile bounds
  Delta v_max=a_max*dt=1*0.02=0.02 rad/s and
  Delta SP_max=v_max*dt=0.5*0.02=0.01 rad. Starting from
  v_f^2=v_i^2+2*a*Delta theta, set final velocity to zero and derive the
  stopping-speed limit v_stop=sqrt(2*a_max*distance_remaining). Explain how
  the smaller of stopping speed and cruise speed creates acceleration, cruise,
  and braking regions. For 0.05 rad backlash, predict that amount of motor-side
  reversal travel before the load follows; a time estimate requires speed and
  is not intrinsic to the gap.
- **Practical flow:** Establish one ideal baseline, then introduce torque limit,
  trajectory velocity/acceleration limits, Coulomb friction, command deadband,
  backlash, and stiction one at a time under the same reversal experiment.
  Monitor setpoint, profiled setpoint, load position, motor-side velocity,
  requested torque, applied torque, saturation, slew limiting, deadband,
  stiction hold, and the configured backlash width. For each case, classify the
  signature before revealing the injected effect and choose among safe
  trajectory design, constraint-aware control, friction compensation, added
  dither only where justified, inspection, or mechanical maintenance.
- **Decision-measure derivations:** Define reversal delay from the 10-second
  reversal to 0.020 rad load motion in the new direction. Derive peak
  following error as max(abs(SP_profile-PV_load)), maximum drive gap as
  max(abs(T_requested-T_applied)), torque total variation as
  sum(abs(T_k-T_k-1)), and state-active time as active_sample_count*dt.
  Explain why absolute values prevent directional cancellation, why total
  variation is accumulated N*m rather than instantaneous torque, and why a
  zero flag duration is diagnostic evidence. Report final-window absolute
  error only after the position and applied-torque spans satisfy the declared
  settling qualification. Do not use controller error alone to distinguish
  deadband, backlash, and stiction.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. The false state
  is target/profile zero, Manual zero request, and
  P1.ExperimentEnabled=FALSE. Test uses Observe without excitation, amplitude
  zero, and maximum duration 20.02 s; its sole reviewed write is that Boolean.
  The project enables the checked-in combined case, commands +0.5 rad,
  reverses to -0.5 rad at 10 seconds, and restores target/profile zero and
  Manual zero when authority falls. Invalid position or velocity and safety
  abort fail closed. Fresh native runtimes own the ideal and six one-effect
  configurations; one UI run shall not claim that matrix.
- **Required conclusion:** Plant and actuator faults must be diagnosed before
  blaming controller tuning. A controller may respect limits or compensate a
  characterized friction effect, but software tuning shall not be presented as
  repair of excessive clearance, wear, or a sticking mechanism.

#### Lesson 25 — Digital realization: same positioning table

- **Reason:** Lessons 23 and 24 showed that sampling, filtering, held commands,
  and physical limits all alter the response even when the continuous control
  idea and gains are unchanged.
- **Theory before practice:** Continuous and discrete signals, zero-order hold,
  forward/backward Euler, trapezoidal/Tustin approximation, quantization,
  Nyquist frequency, aliasing, coefficient and state precision, and delay
  introduced by computation. Explain the continuous filtered parallel PID law
  in physical terms before deriving each discrete integral and derivative-state
  update; name the exact prior/current samples used by every method.
- **Required derivations:** Derive sampling frequency fs=1/dt as updates per
  second and Nyquist frequency fN=fs/2 from the two-samples-per-cycle boundary.
  Derive the forward and backward integral rectangles and Tustin trapezoid from
  TI,k=TI,k-1+Ki*integral(e dt). Begin derivative filtering with
  tauD*dD/dt+D=Kd*dPV/dt. Derive forward Euler
  Dk=Dk-1+(dt/tauD)*(Kd*r_k-1-Dk-1), backward Euler
  Dk=[Dk-1+(Kd/tauD)*(PVk-PVk-1)]/[1+dt/tauD], and Tustin
  Dk=[(2*tauD-dt)*Dk-1+2*Kd*(PVk-PVk-1)]/(2*tauD+dt), explicitly stating that
  D is subtracted from torque for measurement damping. Derive
  f_alias=abs(f_signal-m*fs), choosing integer m to place the apparent
  frequency between zero and Nyquist.
- **Implemented controller behavior:** Keep the positioning plant on a 10 ms
  simulator step. Execute the lesson controller every configurable integer
  number of base scans and hold applied torque between updates, making the
  zero-order hold observable. Provide forward Euler, backward Euler, and
  trapezoidal/Tustin candidates for the same integral and filtered-derivative
  dynamics, with identical continuous gains and filter time constant. Preserve
  the previous calculated torque so an optional one-controller-update
  computation delay is real state, not a shifted plot. Quantize the
  controller-facing position explicitly and inject a deterministic sinusoidal
  encoder component whose frequency is above Nyquist for at least one tested
  sample interval.
- **Worked prediction:** For every tested interval, calculate update frequency,
  Nyquist frequency, samples per plant time constant, the discrete integral
  weight, the derivative-filter coefficient or coefficients, the apparent
  alias frequency, and total delay in seconds before execution.
- **Practical flow:** Establish the fastest Tustin baseline; compare forward
  Euler, backward Euler, and Tustin at the same interval; then hold method and
  continuous parameters fixed while testing 10, 20, and 100 ms controller
  intervals. Repeat with encoder quantization, the declared sinusoidal
  interference, and one-update delay. Retain the actual update timestamps and
  held-output intervals rather than assuming nominal timing from configuration.
- **Decision measures:** Define and retain maximum absolute position error,
  overshoot, settling qualification, applied-torque total variation, held-
  output duration, sample-interval maximum and jitter, aliased measurement
  frequency, and finite/bounded-state checks. An unstable or nonsettling case
  shall be labelled as such rather than ranked by its final sample.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 20.01 s, and
  one P1.ExperimentEnabled Boolean. Its rising edge selects Tustin at 100 ms,
  no added delay, enables control, and applies SP 0.5 rad. Its falling edge
  restores SP zero, Tustin/10 ms, control disabled, and zero held torque.
  Invalid position or velocity and safety abort fail closed. Fresh native
  runtimes own the three-method 20 ms comparison, 10/20/100 ms comparison, and
  delayed 10 ms case; the UI run shall not claim the entire matrix.
- **Required conclusion:** A continuous design is not complete until its
  discrete realization is qualified at the actual sampling, precision, noise,
  hold, and delay boundary. A discretization method that is acceptable at one
  interval is not thereby qualified at another.

### 3.3 Production-quality digital PID

#### Lesson 26 — Positional, incremental, and combined forms: heater and positioning table

- **Objective:** Explain and verify why a production controller must calculate
  P and PD control positionally, calculate PI and PID control incrementally,
  and select the form from the presence of integral action rather than expose a
  misleading user-selectable form switch.
- **Reason:** Lesson 25 turned the continuous controller into state updates.
  Reusing one recurrence for every controller type now creates a concrete
  failure: with integral action disabled and error unchanged, an incremental P
  recurrence calculates zero output increment and can remain at an arbitrary
  previous output. Conversely, a positional PI implementation requires
  additional state correction to make gain and mode changes bumpless.
- **Theory before practice:** First describe the controller output as the
  actuator request calculated at the current sample and the previous
  controller output as retained state. Then introduce the sample index, current
  and previous error, error change, sample interval, bias, and proportional,
  integral, and derivative gains with their equipment-specific units. Only
  then present the positional law, which calculates the complete output from
  current terms, and the incremental or velocity law, which adds P, I, and D
  increments to the previous output. Explain that constant nonzero error makes
  the proportional increment zero even though positional proportional output
  is nonzero. Explain why changing proportional gain at unchanged error changes
  positional P output immediately but cannot correct an arbitrary incremental
  P state. Explain that the nonzero integral increment continually anchors an
  incremental PI/PID controller to the error and makes output tracking,
  clamping anti-windup, and bumpless parameter changes practical. Do not teach
  positional, incremental, parallel, ideal, and series as synonyms: the first
  pair describes the state-update realization, while the latter forms describe
  parameterization and are taught separately.
- **Required derivation:** Write positional PI at samples k and k-1, subtract
  the equations, cancel the unchanged bias, replace the proportional
  difference with Kp*(e_k-e_k-1), and use the backward-Euler integral
  increment Ki*Ts*e_k to derive
  u_k=u_k-1+Kp*Delta e_k+Ki*Ts*e_k. Then show why a gain change requires the
  exact proportional difference Kp_k*e_k-Kp_k-1*e_k-1; the shortcut
  Kp_k*Delta e is zero at unchanged error and misses the new P demand. Explain
  that the shipped incremental PI/PID gain-change behavior intentionally
  retains output for bumpless operation, rather than claiming arbitrary
  retained state is always correct.
- **Implemented controller behavior:** Use the shipped `Control.PID` without a
  public form-selection parameter. When integral gain is exactly zero it shall
  calculate the positional P/PD law from bias, present error, present rates,
  and feedforward. When integral gain is nonzero it shall calculate the
  incremental PI/PID law from the retained nominal output and signal
  increments. The executable lesson shall connect one controller to the same
  encoder-controlled positioning table used in Lessons 19–25 with P action and
  a second controller to the recirculating water heater used in Lessons 4–18
  with PI action. A diagnostic calculation may reproduce the invalid
  incremental-P recurrence, but it shall be visibly labelled as a rejected
  calculation, shall never drive a plant output, and shall not create another
  reusable controller implementation.
- **Worked prediction:** At a constant position error of 0.5 rad, increasing
  proportional gain from 4 to 8 N m/rad shall change positional P torque from
  2 to 4 N m before limiting. For the rejected incremental-P recurrence,
  unchanged error means zero error change and therefore zero torque increment;
  an arbitrary retained torque of 1 N m remains 1 N m after the same gain
  change. For PI at unchanged error, calculate the next output change from the
  integral increment and show that changing proportional gain alone does not
  introduce a full proportional-output jump.
- **Practical flow:** First run the constant-error diagnostic and compare the
  predicted positional P result with the rejected incremental-P state. Change
  proportional gain while holding setpoint, process value, previous error,
  sample interval, bias, limits, and retained starting output equal. Then run
  the positioning table with zero integral gain and verify that `Control.PID`
  follows the positional result. Run the heater with nonzero integral gain,
  change proportional gain at a declared sample, and verify from the immediate
  output increment and subsequent temperature response that the controller
  retained its prior output rather than recomputing a positional PI sum. Repeat
  both plant runs after restore; do not compare different initial states.
- **Decision measures:** Retain error, error change, current and previous
  requested output, applied output, proportional gain, integral gain, form
  selected by the controller contract, and gain-change timestamp. Define the
  immediate gain-change output difference over exactly one controller update,
  the difference from the predicted positional P output, the rejected
  incremental-P state error, and the PI output discontinuity after subtracting
  the expected integral increment. Report limiting or rate limiting separately
  because either invalidates an unrestricted-law comparison.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 30.02 s, and
  one P1.ExperimentEnabled Boolean. Its rising edge applies the two plant
  setpoints in Auto with Kp=4; after 5 seconds the project changes both gains
  to 8. Its falling edge restores table SP zero, heater SP 20 degC, both gains
  to 4, Manual mode, and both outputs zero. Invalid values and safety abort
  fail closed. Because live plant error changes, a fresh-state native
  fixed-input comparison owns the exact positional, rejected-incremental, and
  PI calculations; the UI sequence shall not claim that fixed-input proof.
- **Required conclusion:** The realization forms are mathematically related but
  not interchangeable in a practical stateful controller. `Control.PID` shall
  use positional form for P/PD and incremental form for PI/PID so one public
  controller supports all four types without a second PID implementation or a
  form option that lets the user select an invalid combination. Initialization,
  manual, tracking, disabled, and transition behavior remain unresolved and
  motivate Lesson 27.

#### Lesson 27 — Initialization and operating modes: booster pump

- **Objective:** Start a running booster-pump pressure loop from its actual
  pressure and valve state, select the intended authority mode, and prevent
  automatic control until controller and filter histories represent the plant.
- **Reason:** Lesson 26 proved that the controller output is retained state.
  Starting that state, previous setpoint, previous process value, or a pressure
  filter at arbitrary zero values can create a valve command unrelated to the
  already-running pump even when the PID equation and gains are correct.
- **Theory before practice:** Introduce raw discharge pressure, filtered
  discharge pressure, recirculation-valve command, actual valve-position
  feedback, nominal controller output, applied output, and the previous SP/PV
  histories in their units before naming controller state. Explain that
  initialization copies known present conditions and performs no control
  calculation. Define `Manual`, `Auto`, and `Track` as the three modes of
  `Control.PID`: Manual uses the supplied manual output, Auto performs feedback
  calculation, and Track preloads the retained nominal output before the normal
  controller branch. Define `Disabled` separately as station-level supervision
  that does not invoke `Control.PID` and commands the declared safe output; it
  is not a fourth `PIDMode` and shall not be added to the paper-compatible enum.
  Explain why a low-pass filter initialized from the present measurement has no
  artificial startup transient, while an unknown filter state must settle under
  nonautomatic authority before Auto is permitted.
- **Implemented controller behavior:** Use one `Control.PID` and one
  `Control.LowPass` connected through ordinary process-image I/O to a simulated
  booster pump. A deliberate baseline option may initialize all histories to
  zero. The conforming path shall initialize both filter stages from raw
  pressure; initialize previous SP and PV from current values; initialize
  nominal and saturated controller output from independently supplied actual
  valve-position feedback; and mark the controller ready atomically. A mode
  supervisor shall map Disabled, Manual, Auto, and Track to the three supported
  PID calls or the external disabled action without creating a second control
  calculation. Auto shall remain blocked in Manual at the actual valve position
  until initialization is complete and the filter is settled within a declared
  tolerance for a declared consecutive-update count.
- **Worked prediction:** At 6.0 bar setpoint and process value with the physical
  valve already at 40 percent, measured-state initialization predicts a first
  automatic request of 40 percent because signal differences and error are
  zero. Zero initialization predicts an unrelated starting state and a
  nonzero pressure-filter transient. Repeat the equilibrium calculation for
  valve positions 20, 40, and 70 percent using the pump relationship declared
  by the simulator.
- **Required numerical derivation:** From PV_eq=2 bar+(0.1 bar/%)*MV derive
  equilibrium pressures 4, 6, and 9 bar at 20, 40, and 70 percent. For the
  deliberate zero-state case derive the two-stage first update using
  a=dt/(tau+0.5*dt)=0.1/(0.5+0.05)=0.181818: stage one becomes 1.09091 bar and
  stage two 0.19835 bar from a 6 bar input. Derive the controller's first
  maximum output movement as rate_limit*dt=100%/s*0.1s=10%, explaining the
  resulting 30-percentage-point mismatch from the physical 40% valve.
- **Practical flow:** Start in Disabled and verify the external safe output.
  For 20, 40, and 70 percent actual valve positions, set the corresponding
  equilibrium pressure and setpoint; perform zero initialization with the
  settle interlock deliberately bypassed only in simulation and record the
  first-output mismatch. Restore, perform measured-state initialization, remain
  in Manual until the filter-settled indication is true, then select Auto and
  record the first request. Exercise Manual and Track once only to confirm their
  defined calculation sources; transition quality is reserved for Lesson 28.
- **Decision measures:** Define first-update valve mismatch as the absolute
  first requested output minus actual valve feedback, in percentage points.
  Define filter initialization error as the absolute filtered minus raw
  pressure immediately after initialization, in bar. Retain controller-ready,
  filter-settled, requested mode, active authority, startup-blocked state,
  previous/current SP and PV, nominal/requested/applied output, and limit state.
  Compare equivalent initial conditions and report any rate or amplitude limit
  that masks the unrestricted result.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 10.1 s, and one
  P1.ExperimentEnabled Boolean. Its rising edge issues exactly one
  measured-state initialization at SP/raw pressure 6 bar and actual valve 40%,
  then requests Auto while Manual retains 40% until three settled 100 ms
  updates pass. Its falling edge selects station Disabled and the 30% safe
  recirculation command. Invalid values and safety abort select Disabled.
  Fresh native runtimes own zero-state and 20/40/70% comparisons; the UI run
  shall not claim those independent initial states.
- **Required conclusion:** Initialization and authority semantics are observable
  control behavior. Automatic feedback is safe to start only after the product
  controller and filter histories represent the actual plant and supervisory
  interlocks grant authority. This lesson does not claim that transitions are
  bumpless; aligning state across authority changes motivates Lesson 28.

#### Lesson 28 — Bumpless transfer and external tracking: same pump

- **Objective:** Transfer booster-pump valve authority between an external
  station and automatic pressure control without an avoidable command step, and
  identify transitions for which exact bumpless behavior is infeasible.
- **Reason:** Lesson 27 initialized the controller correctly at startup, but an
  inactive controller can become stale after another station changes the valve.
  Correct startup state therefore does not prove a later authority handover is
  aligned.
- **Theory before practice:** Introduce authority owner, external applied valve
  output, tracking input, retained nominal output, last applied output, and
  transition instant before defining bumpless transfer. Bumpless means that the
  new authority's first requested output equals the last physically applied
  output within a declared tolerance; it does not mean that PV is already at
  setpoint or that subsequent feedback increments are zero. Explain that
  `Track` with integral action writes the external applied output into retained
  nominal state before executing the incremental branch. Explain that simply
  leaving a controller inactive does not align its state. For Auto-to-Manual,
  the external manual station—not `Control.PID`—must first adopt the current
  applied output. State the boundaries: tracking has no aligning effect in the
  positional P/PD branch of the paper-compatible controller; an unavailable or
  stale applied-output signal, an output outside enforceable limits, a rate
  constraint, or an authority change during an unaccounted signal change can
  make exact bumpless transfer impossible.
- **Implemented controller behavior:** Use the same pump and one
  `Control.PID`. Provide station authority states for Disabled, Manual, Auto,
  Track, and a simulation-only inactive-without-tracking diagnostic. Manual,
  Auto, and Track shall call the corresponding product mode. The inactive
  diagnostic shall apply the external output while deliberately not updating
  the controller, making stale state observable without implementing another
  control law. Track shall receive independently observed applied output, not a
  pre-limit request. No hidden automatic transition or silent mode change is
  permitted.
- **Worked prediction:** Initialize both controller and plant at 40 percent and
  6 bar. Let external authority move the applied valve to 60 percent while SP
  and PV are held at 6 bar for the exact transition update. With Track active,
  retained nominal output becomes 60 percent and the first Auto request is
  60 percent. Without Track, retained output remains 40 percent and the first
  Auto request steps by 20 percentage points. From a 40 percent Auto output, a
  manual station still set to 10 percent creates a 30 percentage-point step;
  adopting 40 percent before transfer predicts zero.
- **Required derivation:** Substitute the isolated transition conditions
  e_k=0 and Delta e_k=0 into the incremental law
  u_k=u_k-1+Kp*Delta e_k+Ki*Ts*e_k. After Track assigns
  u_k-1=MV_track, derive u_k=MV_track. Calculate tracked, untracked, unaligned
  Manual, and aligned Manual steps as the absolute first-new-request minus
  last-applied differences 0, 20, 30, and 0 percentage points. Explain why
  zero pressure error does not establish retained-output alignment.
- **Practical flow:** Restore and qualify the Lesson 27 initialization. Run an
  external 40-to-60 percent change while calling Track with actual applied
  output, then change only authority to Auto and retain the transition sample.
  Restore and repeat with the controller inactive and untracked. Restore again
  for Auto-to-Manual and compare an unrelated manual value with a manual value
  first aligned to current applied output. Finally demonstrate and label one
  infeasible boundary without operating physical hardware.
- **Decision measures:** Define authority-transition output step as the
  absolute difference between the first requested output from the new authority
  and the last applied output before the transition, in percentage points.
  Define retained-state alignment error as retained nominal output minus
  external applied output immediately before transition. Retain requested and
  active authority, mode, SP, raw/filtered PV, requested/applied MV, tracking
  value and quality, limits, transition timestamp, and subsequent recovery; do
  not infer alignment from PV alone.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 10.1 s, and one
  P1.ExperimentEnabled Boolean. Its rising edge initializes at 40% and 6 bar,
  calls Track with independently observed applied output during the external
  move to 60%, and then changes only authority to Auto. Its falling edge
  selects station Disabled and the 30% safe command. Invalid or stale tracking,
  invalid values, and safety abort select Disabled. Fresh native states own
  untracked, stale-tracking, aligned-Manual, and unaligned-Manual cases; the UI
  sequence shall not claim all counterfactuals.
- **Required conclusion:** Bumpless transfer is a shared contract between the
  controller and the authority supervisor. Integral Track keeps an inactive
  controller aligned for later Auto, while the external station must adopt the
  current output before taking Manual authority. These rules enable later
  selectors, cascade handover, parallel gain schedules, and redundancy, but do
  not repair infeasible constraints or positional P/PD tracking. Lesson 29 next
  tests whether parameters and signal contributions are updated coherently
  inside each authoritative scan.

#### Lesson 29 — Atomic updates and signal ordering: booster-pump station

- **Objective:** Apply a complete online PID configuration as one coherent
  revision and prove that feedforward and actuator constraints execute in the
  specified order within the same booster-pump scan.
- **Reason:** Lesson 28 aligned state between authority owners. That alignment
  can still be destroyed if one scan combines gains from two configurations or
  if a feedforward contribution is added outside the controller after
  anti-windup, amplitude limiting, and rate limiting have already executed.
- **Theory before practice:** Introduce candidate configuration, active
  configuration, configuration revision, coherent snapshot, and scan boundary
  before describing atomicity. An atomic update makes either the complete old
  record or complete new record visible to one controller update; a mixture is
  forbidden. Introduce feedback contribution and feedforward contribution in
  output units, nominal requested output before constraints, saturation
  candidate, applied output, previous applied output, and effective rate-limit
  interval before writing their ordering. Explain that feedforward is an
  externally calculated contribution from a measured disturbance, not a
  second controller; its design is deferred to Lesson 53. The required order is
  coherent parameter snapshot, signal differences, feedback and feedforward
  combination, anti-windup, amplitude/rate limiting, applied-output retention,
  then publication. Rate limits shall be anchored to previous applied output,
  not an unconstrained nominal request.
- **Implemented controller behavior:** Use one `Control.PID` and the same pump.
  The station shall hold candidate and active `PIDParameters` records and apply
  all candidate fields to the active record on one explicit apply pulse before
  invoking `Configure` once. A simulation-only unsafe sequencer may expose a
  mixed revision by applying candidate fields across separate scans, but it
  shall be visibly rejected and shall never become the normal configuration
  path. The real feedforward value shall enter `PIDInput.FeedForward`; a
  calculation-only rejected external sum may show the value that would result
  from adding feedforward after the returned limited output, but that value
  shall never drive the plant. Publish active revision, active parameters,
  configured-this-scan, mixed-revision diagnostic, requested output, applied
  output, feedforward, and limit states.
- **Worked prediction:** Start at 6 bar and 40 percent with active gains 5 and
  1, then request gains 15 and 3 plus tracking time 0.5 seconds on the same scan
  as a 1 bar setpoint step. The coherent first increment is 15.3 percentage
  points; the deliberately mixed first increment is 15.1 points because it
  combines new proportional gain with old integral gain. At zero error and
  40 percent retained output, a 20 percent feedforward contribution with a
  20 percent-per-second rate limit and 0.1 second sample interval permits only
  42 percent applied output. Adding feedforward after the returned output would
  incorrectly request 60 percent and bypass that limit.
- **Required derivation:** For the 1 bar error step derive
  Delta MV_P=Kp*Delta e=15*1=15% and
  Delta MV_I=Ki*dt*e=3*0.1*1=0.3%, giving 55.3% from the retained 40%.
  Replace only Ki by its old value 1 to derive the invalid mixed 55.1%.
  For feedforward, derive the unconstrained target 40+20=60%, permitted
  movement rate*dt=20*0.1=2%, and current rate interval [38,42]%, so the
  constraint-aware result is 42% and the external-sum violation is 18 points.
- **Practical flow:** Restore the Lesson 28 aligned state. Apply the new record
  atomically with the setpoint step and retain the exact update. Restore and
  run the unsafe sequential diagnostic to expose the mixed record, then reject
  it. Restore at zero error, apply the 20 percent feedforward through
  `PIDInput`, and compare the constrained plant command with the rejected
  external sum. Continue several scans to prove each rate bound starts from the
  preceding applied output. Change one factor at a time and do not tune
  feedforward in this lesson.
- **Decision measures:** Define configuration coherence as all active fields
  matching either the complete old revision or complete new revision at every
  controller update. Define first-update increment from the output immediately
  before and after the update. Define external-sum limit violation as the
  rejected post-controller sum minus the maximum output allowed by amplitude
  and current rate constraints. Define applied-output rate as successive
  applied-output difference divided by measured cycle time. Retain exact scan
  index, revision, parameter values, SP/PV histories, feedforward, nominal or
  diagnostic request, applied output, and constraint state.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 10.1 s, and one
  P1.ExperimentEnabled Boolean. The project initializes at 6 bar/40%, applies
  the complete candidate and 1 bar step atomically, then reinitializes the
  40% aligned state and sends 20% feedforward through PIDInput under the 20%/s
  limit. Its falling edge disables the controller and applies safe 30%.
  Invalid values and safety abort fail closed. Fresh native states own mixed
  revision and post-limit-sum diagnostics, neither of which may drive the UI
  plant.
- **Required conclusion:** Runtime data flow and ordering are part of the
  controller law. A correct equation with a mixed configuration or an external
  post-limit sum is an incorrect implementation. Atomic snapshots and one
  constraint-aware output path are prerequisites for safe online tuning,
  scheduling, feedforward, and later multiloop structures. Lesson 30 next
  defines what happens when a value, signal, timing condition, or configuration
  is invalid rather than coherent.

#### Lesson 30 — Invalid data and safe behavior: booster-pump station

- **Objective:** Diagnose each invalid input class deterministically, prevent
  the invalid scan from changing PID configuration or dynamic state, select a
  declared safe valve output, and restore automatic authority only through an
  acknowledged and qualified recovery sequence.
- **Reason:** Lesson 29 proved coherent data flow when every value is valid.
  Real stations also receive nonfinite or implausible values, stale sensor
  samples, invalid elapsed time, missed deadlines, and invalid candidate
  records. Passing any of them into the PID can corrupt retained state and make
  later apparently valid scans unsafe.
- **Theory before practice:** Introduce valid numeric domain, physical
  plausibility range, data quality, measurement age, freshness limit, measured
  cycle time, deadline, watchdog, fault cause, first-fault latch,
  acknowledgement, recovery qualification, safe output, hold-last output, and
  renewed authority before symbols or code names. Explain that `NaN` compares
  unequal to every number, infinity exceeds physical bounds, and a combined
  finite-and-range predicate rejects both without sending them into arithmetic.
  A hold-last policy preserves the last command but can preserve the action
  that created a hazard; the pump station therefore chooses a 30 percent safe
  recirculation output for loss of trustworthy control data. Acknowledgement
  records that an operator saw the fault; it does not prove recovery and shall
  not clear the latch.
- **Validity contract:** The station shall accept SP and PV only within 0 to
  12 bar, actual valve feedback within 0 to 100 percent, feedforward within
  -100 to 100 percent, sensor age from 0 through 0.5 seconds with asserted
  quality, and measured cycle time greater than 0 and no greater than 0.2
  seconds. A candidate record is valid only when proportional and integral
  gains are bounded, tracking time is positive, output minimum is below output
  maximum inside 0 to 100 percent, and both rate limits are positive. A missed
  deadline is an invalid scan even when its supplied cycle time remains in
  range.
- **Deterministic diagnostic behavior:** Publish one stable fault code with
  this priority when several conditions occur together: invalid numeric or
  physical range `1`, stale or bad-quality measurement `2`, invalid cycle time
  `3`, missed deadline `4`, invalid configuration `5`. Code `0` means no active
  fault. The first detected cause shall remain latched until qualified reset;
  later faults shall be retained as event evidence but shall not silently
  replace the operator-facing first cause. On a faulting scan, do not call
  `Control.PID.Update`, do not call `Configure`, do not advance the active
  revision, and command the 30 percent safe output.
- **Recovery contract:** While latched, require three consecutive scans that
  satisfy every current signal, timing, and active-configuration predicate.
  An acknowledgement may occur at any time but only records operator awareness.
  Reset requires acknowledgement, the three-scan recovery qualification, and
  an explicit reset request. The reset scan shall configure the last accepted
  active record, initialize SP/PV and nominal/applied histories from present
  measured state, and hold current actual valve feedback. Automatic control
  remains unauthorized until a separate renewed Auto request on a later scan.
- **Implemented controller behavior:** Use the same single `Control.PID` and
  first-order booster-pump plant. The station-level guard owns validity,
  diagnostics, latching, acknowledgement, recovery, and safe output; no second
  controller law is permitted. Candidate configuration shall enter the active
  record only after complete validation and shall be rejected without partial
  assignment otherwise. Publish fault code, latched and acknowledged states,
  recovery count/readiness, active revision, controller-called-this-scan,
  configured-this-scan, authority state, SP, PV, age, measured cycle time,
  requested/applied MV, and evidence class.
- **Worked prediction:** Begin at 6 bar and 40 percent with revision 0. A
  candidate whose output minimum is 80 percent and maximum is 20 percent shall
  latch code 5, leave revision 0 unchanged, skip both controller calls, and
  command 30 percent. Correcting the values without acknowledgement and reset
  shall still command 30 percent. After acknowledgement and three consecutive
  valid scans, reset at actual valve feedback 30 percent shall clear the latch,
  retain 30 percent on that scan, and leave Auto unauthorized. A later renewed
  Auto request at SP equal to PV shall begin from 30 percent without an output
  step. Separate restored runs shall map a nonfinite PV to code 1, sensor age
  0.6 seconds to code 2, zero cycle time to code 3, and a missed-deadline input
  to code 4.
- **Required logic derivation:** Express each physical range as a conjunction,
  for example PV_valid=(PV>=0) AND (PV<=12), and explain how either failed
  bound rejects infinity and how unordered NaN comparisons cannot make the
  conjunction true. Define the recovery recurrence as count=min(count+1,3)
  only when acknowledged, not on the acknowledgement scan, and all present
  predicates are valid; otherwise count=0. Derive reset acceptance as
  acknowledged AND count=3 AND reset_request. Explain why reset initializes
  and holds actual output but does not call Update or grant Auto.
- **Practical flow:** Record the valid baseline. Restore before each injection
  and change only one of PV numeric value, age/quality, cycle time, deadline, or
  candidate record. At the exact fault scan, retain diagnostic identity,
  revision, whether PID or Configure was called, and applied output. For the
  invalid-configuration case, correct the values without acknowledgement or
  reset, acknowledge, collect three valid recovery scans, reset, and separately
  renew Auto. Never inject these cases on physical equipment merely to create
  teaching evidence.
- **Decision measures:** Define diagnostic correctness as exact equality
  between injected class and published code on the first fault scan. Define
  invalid-scan state mutation as any Configure or PID update call or active
  revision change on that scan; required value is zero. Define safe-output
  error as absolute applied output minus 30 percent while latched, in percentage
  points; required maximum is 0.001. Define recovery qualification as exactly
  three consecutive valid scans, reset acceptance as acknowledgement plus
  qualification plus request, and recovery output step as the absolute first
  renewed-Auto output minus the reset-scan applied output.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 10.1 s, and one
  P1.ExperimentEnabled Boolean. The project first initializes revision 0,
  injects the reversed 80/20 candidate on a later scan, corrects without
  clearing, acknowledges on its own scan, supplies three valid scans, resets
  at actual output 30%, and renews Auto only on the next scan. Its falling edge
  removes Auto and commands safe 30%. Fresh native runs own isolated fault
  codes 1 through 4; the UI sequence shall claim only the code-5 recovery path.
- **Required conclusion:** A production controller must fail predictably before
  invalid data reaches its arithmetic. Detection, safe action, evidence,
  acknowledgement, state reinitialization, and renewed authority are distinct
  operations. An invalid value, mode, or configuration must never silently
  become automatic control. Lesson 31 next uses only valid, qualified behavior
  to select the minimum sufficient controller for different plants.

### 3.4 Controller selection and plant modelling

#### Lesson 31 — Select a controller: cold room, heater, tank, and positioning table

- **Objective:** Select and defend the minimum controller structure that meets
  a declared requirement for four real loops, then prove each selected
  structure on its executable plant without treating PID as the default.
- **Reason:** Lessons 1 through 30 developed ON/OFF, P, PI, PD, and PID and made
  their implementations safe. The learner now needs a repeatable engineering
  decision process: the most feature-rich controller can add state, noise
  sensitivity, tuning work, and failure modes without improving the required
  result.
- **Theory before practice:** Introduce controlled-variable accuracy,
  acceptable band, permanent load error, self-regulating plant, integrating
  plant, damping requirement, actuator resolution, one-sided/bidirectional and
  binary/continuous actuator, setpoint or servo objective, disturbance or
  regulatory objective, noise sensitivity, constraint exposure, and minimum
  sufficient controller before naming candidate structures. The minimum
  sufficient controller is the least complex structure that satisfies every
  declared requirement with margin; simplicity shall never excuse a failed
  requirement. Explain that plant integration can supply the integral action
  needed for setpoint tracking, while an unknown constant load may still create
  offset, and that derivative is justified by a demonstrated damping need, not
  by a desire to use all PID terms.
- **Selection table and implemented behavior:** Use four simultaneously
  executable loops and publish their selected structure codes: ON/OFF `1`, P
  `2`, PI `3`, PD `4`, and PID `5`.
  - The cold-storage room has a binary compressor, permits a 1.5 to 2.5 degree
    Celsius band, and prioritizes protected cycling over exact temperature.
    Select ON/OFF with hysteresis and five-second minimum dwell; a continuously
    calculated PID output cannot be realized by this actuator.
  - The recirculating water heater is self-regulating, has a continuous
    one-sided 0 to 100 percent heater, and must remove offset under a constant
    heat-loss change. Select PI. P fails the accuracy requirement and D is not
    justified by the slow nonoscillatory thermal response.
  - The buffer tank integrates net flow, uses a continuous pump trim around a
    known 20 percent equilibrium bias, and in this lesson must track level
    setpoint without an unmodelled persistent outflow change. Select P. Plant
    integration already makes the setpoint error converge to zero; added
    controller integral action is unnecessary for the declared case. State the
    boundary explicitly: a changed unknown outflow or exact regulatory
    requirement can justify PI later.
  - The loaded positioning table has bidirectional torque, inertia, damping
    need, and angle-dependent gravity 4*sin(theta) N*m while requiring zero
    final position error. Select PID: P lacks damping, PD leaves
    gravity-dependent offset, and integral action is required in addition to
    derivative damping.
  The project shall use one controller instance per continuous loop and one
  explicit ON/OFF law for the binary loop; it shall not implement an automatic
  structure-selection algorithm or pretend that these decisions generalize
  without their requirements.
- **Worked prediction:** The cold-room thresholds are 1.5 and 2.5 degrees
  Celsius. The heater plant has 0.2 degrees Celsius per percent gain from a
  20-degree baseline, so 50 percent is required for 30 degrees; PI can retain
  that nonzero output at zero error. The tank has gain 0.2 centimetres per
  percent-second and P gain 2 percent per centimetre, giving an ideal
  closed-loop time constant of 1 / (0.2 * 2) = 2.5 s; at zero level error the
  20 percent bias balances known outflow. For the table's implemented Kp=8 and
  SP=1 rad, derive PD equilibrium from 8*(1-theta)=4*sin(theta), giving
  theta=0.6840 rad and 0.3160 rad offset. PID instead retains
  4*sin(1)=3.366 N*m at zero error.
- **Practical flow:** Before running, write the requirements and predict one
  rejected structure for each plant. In Control Studio monitor all four SP/PV/
  requested/applied MV sets, structure code, terms enabled, limits, state,
  quality, and simulation label. Start from common declared initial states and
  run the chosen ON/OFF, PI, biased P, and PID structures. For the heater apply
  the declared heat-loss event; for the table retain angle-dependent gravity.
  Compare evidence with each requirement, then change one requirement at
  a time in a decision worksheet to show when the selection would change. Do
  not retune a failing structure until deciding whether its missing mechanism
  makes it ineligible.
- **Decision measures:** For the cold room define band violation time after
  initial pull-down and compressor transitions per hour with dwell compliance.
  For the heater define final-window absolute temperature error after settling
  and heater total variation. For the tank define settling qualification,
  final-window absolute level error, and pump movement. For the table define
  overshoot, settling, final-window absolute position error, and torque total
  variation. Define requirement coverage as the count of declared requirements
  passed; a candidate is eligible only when it passes all. Define complexity as
  enabled dynamic mechanisms and retained states, used only to choose among
  eligible candidates rather than to excuse a failure.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 120.1 s, and
  one P1.ExperimentEnabled Boolean. Its rising edge applies the four selected
  setpoints and enables the selected ON/OFF, PI, biased-P, and PID paths. Its
  falling edge restores initial setpoints, compressor off, heater/table Manual
  zero, and tank Manual 20%. Invalid values and safety abort fail closed.
  Native evidence owns the candidate rejection matrix and the heater's
  precisely timed load-event comparison; the UI sequence demonstrates the
  selected structures and shall not claim automatic selection.
- **Required conclusion:** Controller structure follows plant physics, actuator
  capability, disturbance class, and quantified requirements, not feature
  count. ON/OFF, P, and PI can be the state-of-practice answer when their
  requirements make additional terms unnecessary. Lesson 32 next explains how
  to recognize plant response classes before making those selections on an
  unfamiliar process.

#### Lesson 32 — Static and dynamic plant behavior: modular process skid

- **Objective:** Use one normalized bounded open-loop experiment to classify
  self-regulating, integrating, delayed, and oscillatory responses from physical
  evidence before choosing a model structure or controller.
- **Reason:** Lesson 31 selected controllers only because each plant class was
  already known. On an unfamiliar skid, the first samples of a slow, delayed,
  integrating, or lightly damped response can look similar. Choosing PI or a
  first-order model from that short view can be wrong and unsafe.
- **Theory before practice:** Introduce operating point, manipulated-input
  change, process-output change, control direction, steady value, static gain,
  stored mass, stored energy, stored momentum, time constant, continuing slope,
  integration, response delay, natural frequency, damping, resonance, and
  observation window before symbols or equations. A self-regulating plant
  approaches a new finite equilibrium; an integrating plant retains a nonzero
  slope under constant net input; a delayed plant initially shows no output
  response; and an oscillatory plant crosses its eventual value because stored
  energy exchanges between states. Explain that these properties can coexist
  in a real plant and this lesson isolates them to establish signatures.
- **Implemented plant behavior:** Use a modular training skid with four
  independent, physical-unit modules and no feedback controller: a
  self-regulating heated vessel, an integrating buffer tank, a delayed conveyor
  oven, and a flexible mechanical drive. A normalized experiment command from
  0 to 1 maps to a 20-percentage-point heater step, 10-percentage-point net
  tank-inflow step, 20-percentage-point oven-heater step, and one-newton-metre
  drive-torque step. The normalized input exists only to make experiment size
  comparable; plots and decisions shall retain the real commands and PV units.
  Structured Text shall fan the one approved experiment value into those four
  physical commands directly. It shall not instantiate a PID or fit a model.
- **Worked prediction:** The vessel begins at 50 degrees Celsius with gain 0.2
  degrees Celsius per percent and a three-second time constant; its 20-point
  input step predicts a 54-degree equilibrium and about 53.93 degrees after
  four time constants. The tank begins at 50 centimetres with integrating gain
  0.1 centimetres per percent-second; its 10-point step predicts a constant
  one-centimetre-per-second slope and about 62 centimetres after 12 seconds.
  The oven begins at 80 degrees Celsius, waits four seconds, then has gain 0.25
  degrees Celsius per percent and a two-second time constant; it predicts no
  change before four seconds and about 84.91 degrees at 12 seconds. The flexible
  drive has unit static gain, natural frequency one radian per second, and
  damping ratio 0.2; its one-newton-metre step predicts about 52.7 percent
  overshoot and repeated crossings of the eventual one-radian position.
- **Required derivations:** For the vessel use
  T(t)=T0+K*DeltaMV*(1-exp(-t/tau)) to derive 54 degC equilibrium and
  53.927 degC at 12 s. For the tank derive slope
  dL/dt=0.1*10=1 cm/s and L(12)=50+1*12=62 cm. For the delayed oven replace
  elapsed time by t-L and derive
  T(12)=80+5*(1-exp(-(12-4)/2))=84.908 degC. For the drive derive fractional
  overshoot exp(-zeta*pi/sqrt(1-zeta^2))=0.5266 at zeta=0.2, predicting a
  1.527 rad first peak. Explain the physical meaning of every factor before
  substitution.
- **Practical flow:** In Control Studio verify Manual/open-loop authority,
  physical signal scaling, initial operating points, limits, quality, 100 ms
  sampling, 12-second duration, abort conditions, and simulation label. Sketch
  all four physical responses before applying the common normalized step. Run
  once, retain the exact input and all PVs, and hide the configured model labels
  until the learner classifies each trace from its signature. Repeat with a
  shortened observation window to expose ambiguity, but do not fit or tune a
  controller in this lesson.
- **Decision measures:** Define response delay as time from the applied input
  step to the first PV change above sensor resolution. Define late slope from a
  declared final window in PV units per second. Define steady-value eligibility
  only when late slope is sufficiently near zero. Define overshoot as peak PV
  beyond the eligible steady value divided by total steady change, in percent.
  Define crossing count around the eligible steady value after first arrival.
  The vessel shall exhibit bounded final value and near-zero late slope; the
  tank shall retain approximately one centimetre-per-second late slope; the
  oven shall show at least four seconds of delay; and the drive shall exceed 40
  percent overshoot and cross its final value more than once. Do not classify
  from final value alone.
- **Exact workflow and ownership:** The learner workflow shall be exactly
  Monitor -> Test, with no fictional disabled Validate stage. Test uses
  Observe without excitation, amplitude zero, maximum duration 12.1 s, and one
  P1.ExperimentEnabled Boolean. Authority true makes the project set its
  normalized input to one and map it to all four physical commands; authority
  false restores normalized input zero and baseline commands. Invalid PVs and
  safety abort restore zero. No feedback controller, Identify, Tune, or Apply
  stage is present.
- **Required conclusion:** Similar early trends can arise from different stored
  quantities and transport mechanisms. Observation duration and excitation are
  part of the evidence. Classify before fitting: a stable first-order model is
  not eligible for an integrating or oscillatory response merely because a
  short segment looks smooth. Lesson 33 next derives the tank and heater
  behavior from conservation laws instead of treating these signatures as
  unexplained curve shapes.

#### Lesson 33 — Conservation-law models: tank and heater

- **Objective:** Derive executable dynamic models for a constant-area buffer
  tank and an electrically heated vessel from conservation laws, predict their
  transient and equilibrium behavior with units, and decide whether simulator
  evidence is consistent with those predictions. The learner is not fitting a
  curve or tuning a controller in this lesson.
- **Context:** The workshop contains two independent physical modules. The
  buffer tank receives a measured inlet flow and loses a measured outlet flow;
  its process value is liquid level in centimetres. The heated vessel receives
  a one-sided electrical-heater command from zero to 100 percent; its process
  value is temperature in degrees Celsius, and it loses heat to an ambient
  environment. Name every declared parameter, state, flow, power, and
  temperature in words with its unit before assigning a symbol.
- **Reason:** Lesson 32 classified response shapes from equal open-loop tests,
  but labels such as `integrating` and `self-regulating` do not explain why the
  shapes occur or how changing equipment dimensions changes them. Conservation
  laws provide that causal explanation and a prediction that can be checked
  before any model identification or controller tuning is attempted.
- **Theory before practice:** Explain a stored quantity and a state variable in
  ordinary words. State that the rate at which a stored quantity accumulates
  equals the total rate entering minus the total rate leaving. Introduce the
  tank volume as `V = A h`, then use constant area to derive
  `dV/dt = A dh/dt`. Introduce the tank cross-sectional area (A), liquid level (h), inlet volumetric flow
  (q_in), and outlet volumetric flow (q_out), including conversion between
  litres and cubic metres, before writing
  `A * dh/dt = q_in - q_out`. Explain that equal flows produce equilibrium and
  unequal constant flows produce a continuing level slope when area is
  constant.

  Introduce stored thermal energy relative to ambient as
  `E = C (T - T_ambient)` and derive `dE/dt = C dT/dt` for constant
  capacitance and ambient. Introduce the vessel thermal capacitance (C), vessel temperature (T),
  ambient temperature (T_ambient), electrical heating power (P_heater), and
  heat-loss coefficient (H), including kilowatts and kilojoules per degree
  Celsius, before writing
  `C * dT/dt = P_heater - H * (T - T_ambient)`. Explain why heat loss increases
  with temperature difference, why this creates a finite equilibrium, and why
  thermal capacitance changes response speed rather than final equilibrium.
  Only after those physical statements introduce derivative notation,
  differential equations, equilibrium as zero accumulation, static gain, time
  constant, and transport delay.
- **Derived heater response:** Set accumulation to zero to derive
  `T_eq = T_ambient + P_heater/H`. Divide the dynamic balance by `C` and
  match its decay coefficient `H/C` to `1/tau` to derive `tau = C/H`.
  Then derive the delayed first-order response
  `T(t) = T_eq + (T_0 - T_eq) exp(-(t-L)/tau)` for `t >= L`; do not present
  the 63.2-percent result as an unexplained rule.
- **Declared assumptions and boundary:** The tank has constant area, constant
  liquid density, perfectly mixed contents, no overflow, and independently
  specified flows. The heated vessel is perfectly mixed, has constant thermal
  capacitance and linear heat loss, and is evaluated around the declared
  operating region. Sensor dynamics, evaporation, temperature-dependent
  properties, nonlinear gravity outflow, and unmeasured disturbances are
  absent. The simulator is executable evidence for these declared models, not
  physical-plant evidence.
- **Worked tank prediction:** Use a tank area of 0.5 square metres, an initial
  level of 50 centimetres, inlet flow of 12 litres per second, and outlet flow
  of 10 litres per second. Convert the two-litre-per-second net inflow to 0.002
  cubic metres per second before division by area. Predict a level rate of
  0.004 metres per second, or 0.4 centimetres per second, and therefore a level
  of 58 centimetres after 20 seconds. Predict zero level slope when inlet and
  outlet are both 10 litres per second.
- **Worked heater prediction:** Use thermal capacitance of 50 kilojoules per
  degree Celsius, heat-loss coefficient of 10 kilowatts per degree Celsius,
  full-scale heater power of 200 kilowatts, ambient and initial temperature of
  20 degrees Celsius, a 20-percent applied command, and one second of transport
  delay. Calculate 40 kilowatts of heating power, equilibrium temperature of
  24 degrees Celsius, static gain of 0.2 degrees Celsius per percentage point,
  and time constant of five seconds. After the one-second delay plus one time
  constant, predict approximately 22.53 degrees Celsius using the explained
  63.2-percent first-order response relationship.
- **Executable model under test:** Show the small actual Structured Text excerpt
  that performs the visible unit conversions and predictions. It shall publish
  the bounded heater command and tank net-flow command to two simulator plants
  but shall contain no hidden PID or other feedback controller. The simulator
  configuration shall express the same tank integration factor, heater static
  gain, time constant, delay, biases, initial states, addresses, and units as
  the prose and native assertions.
- **Prediction:** Before running, the learner records the expected tank slope,
  tank level at 20 seconds, equal-flow equilibrium behavior, heater power,
  heater equilibrium, heater time constant, and heater temperature at six
  seconds. The learner also predicts which parameter changes slope, final
  equilibrium, or response speed and explains the choice from the balance.
- **Experiment:** In Control Studio, first Monitor both modules and verify
  signal identity, units, quality, initial state, applied input, and simulation
  evidence class. In Test, select `Observe without excitation`, amplitude zero,
  and 30.1 seconds. The test may write only `P1.ExperimentEnabled`; the project
  owns the unequal-flow and 20-percent-heater inputs and restores equal
  10/10-litre-per-second flows and zero heater command when authority becomes
  false. Retain the complete input and output traces. A fresh native run owns
  the independent equal-flow drift case because the UI run does not reset the
  simulated tank state. Do not Identify, Tune, Apply, or invent a separate
  Validate stage: no controller is under evaluation.
- **Comparison measures:** Define tank level-rate error as the absolute
  difference between the measured late-window slope and 0.4 centimetres per
  second. Define tank 20-second prediction error as the absolute difference
  between measured and predicted level in centimetres. Define equal-flow drift
  as maximum level minus minimum level over the run in centimetres. Define
  heater six-second prediction error and heater final-window equilibrium error
  in degrees Celsius. Declare each window and tolerance before displaying
  results; use run data rather than values copied from the source or metadata.
- **Validation:** The unequal-flow tank shall continue with a measured slope
  consistent with 0.4 centimetres per second and reach approximately 58
  centimetres at 20 seconds. The equal-flow tank shall retain its initial level
  within the declared numerical tolerance. The heater shall remain unchanged
  during its declared delay, pass approximately 22.53 degrees Celsius at six
  seconds, and approach 24 degrees Celsius with a near-zero late slope. A run
  with inconsistent units, limits, initial state, sample timing, or simulator
  parameters is invalid rather than contrary evidence for the balance.
- **Explanation:** Reconcile every prediction with the retained traces. The
  tank integrates because nonzero net volumetric flow continuously changes
  stored liquid volume. The heater self-regulates because increasing
  temperature increases heat loss until loss equals heater power. Explain why
  tank area scales slope, thermal capacitance scales time constant, heater power
  and heat-loss coefficient set equilibrium, and delay shifts the visible
  response without changing that equilibrium.
- **Transfer and required conclusion:** First-principles models reveal causal
  structure, units, equilibrium, and the effect of physical parameters. They
  are limited by their assumptions and by uncertain or unavailable equipment
  data; model detail is not automatically useful control knowledge. Lesson 34
  therefore introduces nonlinear gravity outflow, an operating point, and a
  local linear model when a single constant-area integrator no longer describes
  the plant over its full operating range.

#### Lesson 34 — Linearization and transfer functions: gravity tank

- **Objective:** Starting from the conservation balance in Lesson 33, derive a
  first-order transfer function for a gravity-drained tank around three
  operating points, predict each local response, and determine from executable
  error evidence where each linear approximation is valid. The learner shall
  distinguish plant linearization from controller gain scheduling.
- **Context:** A variable-speed inlet pump supplies a vertical tank with
  constant cross-sectional area. Liquid leaves through a fixed outlet under
  gravity, so outlet flow increases with the square root of liquid level. The
  controlled quantity is level in centimetres, while the physical state used
  in the balance is level in metres. The manipulated quantity is inlet flow in
  cubic metres per second. The exercise is open loop and simulation-only.
- **Reason:** Lesson 33 used independently prescribed inlet and outlet flows,
  which made the tank a constant-gain integrator. A real gravity outlet depends
  on level. The same inlet-flow change therefore produces different equilibrium
  and response speed at low and high levels. A single linear model cannot be
  assumed valid over the entire tank.
- **Theory before practice:** First recall in words that tank-volume
  accumulation equals inlet flow minus gravity outlet flow. Introduce tank area
  (A), level (h), inlet flow (q), and outlet coefficient (c), with their units,
  before writing `A * dh/dt = q - c * sqrt(h)`. Explain why the square root
  makes this equation nonlinear.

  Define an operating point as a constant inlet flow (q0) and constant level
  (h0) at which accumulation is zero, so `q0 = c * sqrt(h0)`. Define level
  deviation (delta_h) as actual level minus operating-point level and inlet-flow
  deviation (delta_q) as actual flow minus operating-point flow. Explain a
  first-order Taylor approximation in words as replacing a curved relationship
  by its tangent near the operating point. Derive the tangent from
  `d(c sqrt(h))/dh = c/(2 sqrt(h))`, substitute
  `h = h0 + delta_h` and `q = q0 + delta_q`, and cancel the equilibrium terms
  before writing
  `d(delta_h)/dt = -(c / (2 * A * sqrt(h0))) * delta_h + (1 / A) * delta_q`.

  Explain that neglected second- and higher-order terms are the reason the
  approximation is local. Introduce the Laplace transform only as a
  representation that converts this linear differential relation, under zero
  initial deviation, into an input-output algebraic relation. Show
  `(A s + c/(2 sqrt(h0))) DeltaH(s) = DeltaQ(s)`, then divide by the constant
  denominator term to derive
  `DeltaH(s) / DeltaQ(s) = K / (tau * s + 1)`, where local static gain
  `K = 2 * sqrt(h0) / c` and local time constant
  `tau = 2 * A * sqrt(h0) / c`. Define input, output, transfer function, gain,
  time constant, order, pole, zero, and dead time in words before using their
  symbols or interpreting the model. State that this first-order model has one
  pole at `-1/tau`, no finite zero, and no dead time; do not add an FOPDT delay
  merely because that form is familiar.
- **Declared plant and worked predictions:** Use tank area `A = 0.5 m2` and
  outlet coefficient `c = 0.02 m2*sqrt(m)/s`. At low, middle, and high levels
  `h0 = 0.25, 0.50, and 0.81 m`, calculate equilibrium inlet flows
  `q0 = 0.010000, 0.014142, and 0.018000 m3/s`, local gains
  `K = 50.000, 70.711, and 90.000 m/(m3/s)`, and time constants
  `tau = 25.000, 35.355, and 45.000 s`. For a small `+0.001 m3/s` inlet-flow
  deviation, predict local steady level changes of `5.000, 7.071, and 9.000
  cm`. The exact nonlinear equilibria predict `5.250, approximately 7.321, and
  9.250 cm`, so each small-step local error is about `0.250 cm`.
- **Local-model implementation under test:** Show the concise actual Structured
  Text that calculates each equilibrium, gain, time constant, and pole from the
  declared physical parameters and advances the three local first-order
  deviation models at the 100-millisecond sample interval. The simulator shall
  execute three independent nonlinear gravity-tank plants from the same three
  operating points and physical parameters. No PID, model fitting, scheduled
  controller, or artificial dead time is allowed in this lesson.
- **Prediction:** Before running, the learner completes a table containing each
  operating point, equilibrium flow, local gain, time constant, pole, predicted
  small-step final level, and declared validity interval. The learner then
  predicts why applying the low-level model at the high-level operating point,
  or using a much larger step, will increase error.
- **Experiment:** In Control Studio, Monitor all physical and local-model
  signals, metres-versus-centimetres conversion, initial equilibria, applied
  inlet flows, sample interval, quality, limits, and Simulation label. In Test,
  select `Observe without excitation`, amplitude zero, and 300.1 seconds. The
  test may write only `P1.ExperimentEnabled`; the project applies the same
  `+0.001 m3/s` deviation simultaneously to all three nonlinear tanks and local
  models, then restores the deviation to zero when authority becomes false.
  Retain the input, nonlinear level, local-model level, operating point, model
  parameters, and evidence class. A fresh native run owns the independent
  `+0.005 m3/s` low-level extrapolation case because it requires a fresh
  equilibrium and a different excitation. Do not Identify, Tune, Apply, or
  invent a disabled Validate stage.
- **Comparison measures:** Define local-model prediction error as absolute
  nonlinear level minus local-model level in centimetres at a declared time.
  Define equilibrium error only after both nonlinear and local responses meet a
  declared late-slope condition. Define model validity before inspecting the
  results: the `+0.001 m3/s` step shall remain within `0.30 cm` equilibrium
  error at each operating point. The low-level `+0.005 m3/s` step shall be
  labelled outside that small-signal validity case and is expected to exceed
  `5 cm` equilibrium error; it is not a failed numerical solver.
- **Validation:** Check the calculated equilibrium flows, gains, time constants,
  and poles against independent arithmetic. Verify each nonlinear tank remains
  at equilibrium before excitation. For the small steps, compare complete
  transient traces as well as qualified final errors. For the large step,
  retain both the tangent-model prediction and nonlinear equilibrium and show
  the divergence. A local model shall never be presented without its operating
  point, input range, output range, and validation evidence.
- **Explanation:** At higher level, a given flow change produces a larger local
  steady level change and a slower response because the tangent of the
  square-root outlet curve changes with level. Linearization preserves the
  nearby slope, not the entire nonlinear curve. A pole describes the local
  exponential mode; it does not make the physical tank globally linear.
- **Transfer and required conclusion:** A transfer function is a compact local
  input-output model derived under stated assumptions and zero initial
  deviation. It can support analysis and design only inside validated bounds.
  Multiple local plant models do not by themselves constitute gain scheduling.
  Lesson 35 next uses poles and zeros to compare qualitative behavior across
  several real plant classes before any controller design is attempted.

#### Lesson 35 — Poles, zeros, and real plant classes

- **Objective:** Use pole and zero locations to predict the qualitative
  open-loop responses of six real equipment classes, then match hidden
  pole-zero maps to executable time responses and justify every match from
  evidence. The lesson teaches interpretation, not controller design.
- **Context:** The workshop contains a heated vessel, storage tank, flexible
  motor drive, overhead crane, magnetic bearing, and boiler drum. A common
  normalized experiment is converted into each plant's physical input, while
  outputs retain temperature, level, position, displacement, and drum-level
  units. All six models are declared simulation models around stated operating
  points; their traces are not physical-plant evidence.
- **Reason:** Lesson 34 derived one first-order transfer function, but reading
  coefficients plant by plant is slow and does not expose general response
  limitations. Pole and zero locations provide a compact way to anticipate
  decay, integration, oscillation, instability, inverse response, and dominant
  dynamics before selecting a model or controller.
- **Theory before practice:** Define a transfer function as the zero-initial-
  deviation relationship between an input and output. Explain that a pole is a
  value of the complex frequency variable (s) that makes the denominator zero
  and represents a natural mode; a zero makes the numerator zero and shapes how
  the input appears at the output. Introduce the complex plane, real part,
  imaginary part, left half-plane, right half-plane, and origin in words before
  plotting or naming locations.

  Explain that a negative-real pole produces decay, a pole at the origin
  produces integration, and a positive-real pole produces exponential growth.
  A complex-conjugate pole pair produces oscillation; its real part sets decay
  or growth and its imaginary part sets oscillation frequency. Define natural
  frequency and damping ratio before using them to locate a pair. Define
  dominant poles as the modes that decay most slowly and therefore govern the
  visible long response; dominance is conditional on the input-output path and
  cannot be inferred from distance to the origin alone when a mode is weakly
  excited or observed.

  Explain that a left-half-plane zero can accelerate or reshape a response,
  while a right-half-plane zero makes the initial response oppose the eventual
  change and imposes a nonminimum-phase limitation. Distinguish this inverse
  response from dead time: dead time initially produces no response, whereas a
  right-half-plane zero can produce immediate movement in the wrong direction.
  Explain that exact pole-zero cancellation is physically fragile: uncertain
  plant modes remain internally present even when an input-output algebraic
  model appears to cancel them. An unstable or lightly damped plant pole shall
  never be declared removed by an approximate controller zero.
- **Declared executable plants and predictions:** Use one bounded unit step and
  the following declared local models:
  - the heated vessel has `G(s) = 1 / (5*s + 1)`, one stable real pole at
    `-0.2 1/s`, no finite zero, and a monotonic response reaching approximately
    63.2 percent at five seconds;
  - the storage tank has `G(s) = 0.2 / s`, one pole at the origin, no finite
    zero, and a continuing slope of `0.2 cm/s` under its unit net-flow input;
  - the flexible drive has unit static gain, natural frequency `1 rad/s`, and
    damping ratio `0.2`, giving poles near `-0.2 +/- j0.980 1/s`, approximately
    52.7 percent overshoot, and repeated crossings;
  - the overhead crane has one slow real pole at `-0.1 1/s` and a faster pair at
    `-1 +/- j4 1/s`; predict that the slow pole dominates the late payload-
    position response even though the early response contains decaying swing;
  - the open-loop magnetic bearing has one real pole at `+0.4 1/s`; a positive
    force perturbation shall grow rather than settle, and the run shall abort at
    its declared displacement boundary;
  - the boiler drum has stable poles at `-0.05` and `-0.5 1/s` and a
    right-half-plane zero at `+0.0625 1/s`. Its unit feedwater step shall move
    drum level initially negative and eventually approach a positive unit
    change.
- **Derivation requirement:** Do not present the six signatures as a lookup
  table. Derive the heater result from `1 - exp(-t/5)` and the tank ramp from
  the inverse transform of `0.2/s2`. Derive the flexible-drive roots from
  `s2 + 2*zeta*omega_n*s + omega_n2` and its overshoot from
  `exp(-pi*zeta/sqrt(1-zeta2))`. Compare the crane envelopes `exp(-0.1t)` and
  `exp(-t)`. Solve the bearing equation `dx/dt = 0.4x + u` for its 10 mm abort
  time. For the boiler, show `G(0) = 1` and the negative high-frequency
  coefficient that gives an immediate negative step-response slope.
- **Executable experiment under test:** The actual Structured Text shall clamp
  the normalized experiment and convert it into the six declared physical
  commands without feedback control. The simulator shall instantiate the
  declared first-order, integrating, second-order, and state-space plants. Pole
  and zero tables shall be generated from the same declared model parameters,
  not manually chosen to resemble measured plots. No PID, compensator,
  cancellation, or hidden stabilizing loop is allowed.
- **Prediction:** Before running, the learner marks each pole and zero on an
  unlabeled complex-plane worksheet and writes the expected response signature:
  settles monotonically, ramps, rings and settles, contains a slow dominant
  tail, diverges, or initially moves opposite its final direction. The learner
  shall state an observable test for each prediction before seeing the trace.
- **Experiment:** In Control Studio, Monitor physical commands, outputs, units,
  quality, initial operating points, limits, sample interval, and Simulation
  evidence. In Test, select `Observe without excitation`, amplitude zero, and
  30.1 seconds. The test may write only `P1.ExperimentEnabled`; the project
  applies the common unit request to the five bounded plants and restores all
  commands to zero when authority becomes false. Bearing force remains zero in
  the UI run. A fresh native run owns the magnetic-bearing excitation and its
  declared automatic abort boundary, preserving pre-abort samples. Hide
  equipment and map labels during the matching exercise, commit each match with
  a written reason, then reveal the configured identities. Do not Identify,
  Tune, Apply, or invent a disabled Validate stage.
- **Comparison measures:** Define monotonicity from signed successive output
  increments after the step; late slope in output units per second over a
  declared final window; overshoot relative to an eligible steady value;
  crossing count around that value; dominant-tail decay rate from a declared
  window; unstable growth ratio over two declared times; inverse-response
  minimum, time of minimum, and final sign; and first-response time above sensor
  resolution to distinguish inverse response from delay. Metrics requiring a
  steady value shall be `Not settled` for the tank and magnetic bearing.
- **Validation:** The learner's match is accepted only when both the map and
  measured signature support it. Verify the heater's monotonic 63.2-percent
  point, tank slope, flexible-drive overshoot and crossings, crane slow tail,
  magnetic-bearing growth and abort, and boiler initial-negative/eventual-
  positive response. A correct label without the causal reason is incomplete.
  A pole-zero plot generated from different parameters than the executable
  plant is invalid evidence.
- **Explanation:** Reconcile each trace with its pole and zero structure. State
  which conclusions concern internal stability, bounded-input response,
  transient shape, or input-output visibility. Explain why a right-half-plane
  zero limits how aggressively the boiler level can be controlled and why
  unstable or flexible modes should not be cancelled on uncertain equipment.
- **Transfer and required conclusion:** Pole-zero structure predicts important
  qualitative limitations before tuning, but it is a model-based prediction
  that still requires operating-range and data validation. Lesson 36 next moves
  to state space because a transfer function can hide internal stored
  quantities even when its external input-output response is known.

#### Lesson 36 — State-space introduction: interconnected tanks

- **Objective:** Build an executable state-space model of two interconnected
  water tanks from their flow balances, predict both internal levels after an
  inlet-flow step, and demonstrate why one downstream input-output transfer
  relation cannot reveal the complete stored-water condition.
- **Context:** An inlet pump feeds the upstream tank. Liquid flows from the
  upstream tank into a downstream tank and then to a drain. Both tanks have
  level transmitters for this lesson, but the hidden-state comparison initially
  reveals only downstream level. Levels use metres in the model and
  centimetres in learner-facing plots; pump flow uses cubic metres per second.
- **Reason:** Lesson 35 showed that poles and zeros compactly predict an
  input-output response. A transfer function assumes specified initial
  deviation and can hide internal stored quantities. Two installations can
  therefore have the same pump command and downstream-level reading now but
  different upstream stored water and different downstream behavior next.
- **Theory before practice:** Define a state variable as the smallest physical
  quantity whose present value, together with future input, is required to
  predict future behavior. Define upstream level (h1) and downstream level
  (h2) in metres before collecting them into the state vector (x). Define pump
  inlet flow (u) in cubic metres per second and measured levels as the output
  vector (y). Explain that a vector is an ordered collection and a matrix is a
  table of coefficients whose row and column meanings and units must be
  declared.

  Use equal tank areas `A1 = A2 = 1 m2`, inter-tank flow coefficient
  `k12 = 0.5 m2/s`, and downstream-outlet coefficient `k2 = 0.2 m2/s`.
  Explain the two balances in words before writing
  `A1 * dh1/dt = u - k12*h1` and
  `A2 * dh2/dt = k12*h1 - k2*h2`.

  Define the state equation as the rule that advances internal state and the
  output equation as the rule that maps state and direct input to measurements.
  Only then write `dx/dt = A*x + B*u` and `y = C*x + D*u`, with
  `A = [[-0.5, 0], [0.5, -0.2]] 1/s`,
  `B = [[1], [0]] 1/m2`, `C = [[1, 0], [0, 1]]`, and `D = [[0], [0]]`.
  Explain every row: the first advances upstream level, the second advances
  downstream level, and the output rows publish those two physical states.

  Derive the matrix entries by dividing each physical balance by its own tank
  area; do not introduce the matrix as an unexplained table. Derive the
  eigenvalues from `det(lambda I - A) = 0` and explain why the triangular
  matrix makes them its diagonal entries.

  Define an equilibrium as a constant state under constant input, so state
  accumulation is zero. For `u0 = 0.1 m3/s`, solve `0 = A*x0 + B*u0` and
  predict `h1 = 0.2 m` and `h2 = 0.5 m`. Define an eigenvalue as a natural
  state-space mode and show that the triangular A matrix has eigenvalues
  `-0.5` and `-0.2 1/s`. Relate these to the poles of the zero-initial-
  deviation transfer functions, without claiming that an external transfer
  function contains the current internal state.
- **Hidden-state prediction:** Compare two restored zero-input tanks whose
  downstream levels both read zero. Case Empty starts with
  `x = [0.0, 0.0] m`. Case Upstream charged starts with
  `x = [0.2, 0.0] m`. Predict that Empty remains at zero, whereas Upstream
  charged transfers stored water into the downstream tank. Its downstream
  level should peak after approximately `3.05 s` at approximately `0.109 m`
  even though both cases had the same initial downstream reading and input.
  Derive this prediction: show `h1(t) = 0.2 exp(-0.5t)` and
  `h2(t) = (1/3)(exp(-0.2t) - exp(-0.5t))`, set the derivative of `h2` to zero,
  and obtain `t_peak = ln(2.5)/0.3` before substituting the peak level.
- **Executable model under test:** The simulator shall execute the two-state
  matrices directly and publish both physical states. It shall also execute the
  two restored hidden-state cases from identical downstream readings and pump
  inputs but different upstream states. The actual Structured Text shall clamp
  and route only the declared pump experiments; it shall contain no PID,
  observer, estimator, state feedback, optimal controller, or MPC.
- **Prediction:** Before running, the learner labels every vector component,
  matrix row, matrix column, coefficient unit, equilibrium value, and
  eigenvalue. The learner computes the 0.1-cubic-metre-per-second equilibrium
  and predicts the direction of both levels. Before the hidden-state run, the
  learner commits whether identical current downstream readings imply identical
  future downstream readings and explains why.
- **Experiment:** In Control Studio, Monitor both internal levels, pump input,
  units, initial state, matrix revision, 100-millisecond sample interval,
  quality, limits, and Simulation evidence. In Test, select `Observe without
  excitation`, amplitude zero, and 300.1 seconds. The test may write only
  `P1.ExperimentEnabled`; the project requests `0.1 m3/s` for the main plant
  while authority is true and restores zero flow when it becomes false. A
  fresh native run owns the Empty-versus-Upstream-charged comparison because
  its evidence depends on exact time-zero states. Initially reveal only the
  identical downstream readings, commit the future prediction, and then reveal
  upstream states and traces. Do not Identify, Tune, Apply, or invent a
  disabled Validate stage.
- **Comparison measures:** Define upstream and downstream equilibrium error in
  centimetres only after a declared late-slope condition. Define hidden-state
  downstream separation as the absolute difference between the two downstream
  levels at the same time. Define charged-case peak level and time from retained
  samples. Define matrix-to-balance consistency by comparing each executable
  coefficient, sign, and unit to its physical flow balance; file presence or a
  plausible curve is insufficient.
- **Validation:** Verify the state-space plant reaches approximately 20 cm
  upstream and 50 cm downstream for the declared step, and that its measured
  decay modes agree with eigenvalues `-0.5` and `-0.2 1/s`. Verify both hidden
  cases begin with identical downstream readings and zero input, Empty remains
  at zero, and Upstream charged produces approximately 10.9 cm downstream peak
  near 3.05 seconds. A model with transposed states, inconsistent units, wrong
  signs, or mismatched simulator matrices is invalid even if one output looks
  reasonable.
- **Explanation:** The state vector preserves both stored-water quantities.
  The downstream-only transfer relation describes how pump deviations affect
  downstream level under its initial-state assumption; it does not say how much
  water is already stored upstream. State space therefore separates internal
  evolution from measurement selection and scales naturally to several inputs
  and outputs.
- **Transfer and required conclusion:** State space exposes internal dynamics
  needed later for controllability, observability, estimation, optimal control,
  and MPC. It does not make an unmeasured state known automatically and does not
  replace model validation. Lesson 37 next asks how to design a safe,
  informative physical experiment before any unknown model can be identified.

### 3.5 Experiment design and model identification

#### Lesson 37 — Safe experiment design: plate heat exchanger

- **Objective:** Design, review, execute, abort, and restore one bounded
  plate-heat-exchanger step experiment; distinguish an informative simulation
  plan from an unsafe plan and from a plan that is not authorised for physical
  execution.
- **Context:** A steam-side control valve changes product outlet temperature
  on a plate heat exchanger. Product outlet temperature is measured in degrees
  Celsius and valve position is commanded in percent. The normal operating
  point is 50 degrees Celsius at 60 percent valve. The local simulation model
  is first-order plus dead time with a process gain of 0.5 degrees Celsius per
  percentage point, a 20-second time constant, and 3 seconds dead time. The
  lesson's executable project is simulation-only; it cannot provide physical-
  plant evidence or authorise a real steam-valve movement.
- **Reason:** Lesson 36 showed why a useful model must preserve the dynamics
  that matter. An unknown heat exchanger cannot be identified from a constant
  operating record, but an arbitrary valve movement can overheat product,
  exceed an actuator limit, mix a load disturbance into the response, or end
  before the dynamics are visible. The experiment must therefore be engineered
  before data are collected.
- **Theory before practice:** Define the engineering question as the one
  unknown relationship the experiment must reveal: how much and how quickly
  outlet temperature changes after a small steam-valve change near the declared
  operating point. Define a baseline as a time interval before excitation used
  to prove that input, output, operating mode, and disturbances are sufficiently
  steady. Define excitation amplitude as the intentional change from the
  baseline valve position, and excitation duration as the time for which that
  change is held. Define a confounding disturbance as another input change that
  prevents the measured output change from being attributed to the planned
  excitation alone.

  Define the baseline valve position (MV0) as 60 percent, planned valve change
  (delta_MV) in percentage points, baseline outlet temperature (PV0) as 50
  degrees Celsius, local process gain (K) as 0.5 degrees Celsius per percentage
  point, time constant (T) as 20 seconds, and dead time (L) as 3 seconds. Only
  then state the local prediction `delta_PV_final = K * delta_MV`. Explain that
  a positive 10-percentage-point step predicts a 5-degree-Celsius final rise,
  so the predicted temperature is 55 degrees Celsius. After the dead time, a
  first-order response follows
  `PV(t) = PV0 + K*delta_MV*(1-exp(-(t-L)/T))`. Derive that it has completed
  `1-exp(-3) = 0.9502`, approximately 95 percent, in three time constants and
  calculate `PV(63) = 54.751 degC`. The planned hold is therefore 63 seconds: 3 seconds of
  dead time followed by 60 seconds, or three time constants, of visible
  response. This observes nearly all of the local change without claiming exact
  equilibrium from one final sample.

  Define a soft experiment bound as a stricter planned operating envelope and
  a hard limit as a condition that requires immediate abort. For this lesson,
  an accepted plan shall keep the applied valve between 40 and 80 percent and
  predicted outlet temperature between 45 and 58 degrees Celsius. The hard
  outlet-temperature limit is 58 degrees Celsius. Bad or stale signal quality,
  a nonfinite value, an undeclared disturbance, a hard-limit violation, or loss
  of the authorised operating mode shall abort the run. Abort and normal
  completion shall both restore the valve request to 60 percent; restoration is
  not complete until the output is returned and the measured temperature has
  re-entered its declared baseline band.

  Define experiment approval as permission for one reviewed plan, not general
  permission to move equipment. Simulation approval permits only the simulated
  plant. A physical experiment additionally requires independently reviewed I/O
  topology, confirmed interlocks and safe state, an authorised operator, and
  explicit approval immediately before the physical action. A simulation run
  and a physical eligibility review shall remain separate evidence classes.
- **Prediction:** Before running, evaluate two plans. Plan A changes the valve
  from 60 to 70 percent for 63 seconds and predicts approximately 55 degrees
  Celsius, inside all bounds. Plan B changes it from 60 to 85 percent and
  predicts approximately 62.5 degrees Celsius, outside both the valve envelope
  and temperature limit; it shall be rejected without excitation. Predict that
  invalidating signal quality during Plan A immediately aborts the experiment
  and restores the valve request to 60 percent.
- **Controller under test:** There is no feedback controller and no model fit
  in this lesson. The actual Structured Text shall validate the proposed
  amplitude and duration, qualify the baseline, latch the reviewed simulation
  plan, issue the bounded requested valve position, clamp it into the applied
  valve position, supervise abort predicates, and restore the baseline output.
  Requested and applied valve positions shall be separate observable signals.
  The implementation shall expose concise reason codes for plan rejection and
  run abort; it shall not hide these decisions behind precomputed prose.
- **Experiment:** In Control Studio Monitor, verify Simulation evidence, outlet
  temperature, requested and applied valve position, units, quality, operating
  mode, sample interval, soft bounds, hard limit, and absence of a confounding
  disturbance. In Test, select `Observe without excitation`, amplitude zero,
  and a 160.1-second timeout. The test may write only
  `P1.ExperimentEnabled`; the project owns Plan A review, at least 10 seconds
  of qualified baseline, its 63-second step, abort supervision, and restoration
  to 60 percent plus the baseline temperature band. Fresh native runs own Plan
  B rejection and the injected bad-quality case because those require
  different reviewed conditions and fresh initial state. Do not use Identify,
  Tune, Apply, or invent a disabled Validate stage in this lesson.
- **Comparison measures:** Define baseline temperature range as the largest
  minus smallest outlet-temperature sample during the 10-second baseline, in
  degrees Celsius. Define baseline slope from the first and last samples over
  that same window, in degrees Celsius per second. Define excitation size as
  applied valve position minus baseline valve position, in percentage points.
  Define observed temperature change as the final qualified-window mean minus
  the baseline-window mean, in degrees Celsius. Define prediction error as the
  absolute difference between predicted and observed temperature change, in
  degrees Celsius. Define restoration error as the absolute difference between
  restored requested valve position and 60 percent, in percentage points.
  Report a response as `Not qualified` rather than calculate identification
  measures when baseline, quality, disturbance, duration, or limit conditions
  are not satisfied.
- **Validation:** Prove Plan B is rejected before the applied valve leaves 60
  percent. Prove Plan A cannot start until its baseline range is at most 0.05
  degrees Celsius and absolute baseline slope is at most 0.01 degrees Celsius
  per second. For the accepted simulation run, retain the exact plan, seed,
  100-millisecond sample interval, requested and applied valve traces, outlet-
  temperature trace, quality, disturbance state, state transitions, reason
  codes, and Simulation evidence class. Verify the applied step is 10
  percentage points, no limit is crossed, the late-window temperature change is
  within 0.35 degrees Celsius of the 5-degree prediction, and restoration error
  is zero. Prove the bad-quality case is marked Aborted and restores the valve;
  its interrupted response shall not be accepted as identification data.
- **Explanation:** Reconcile the accepted trace with the predicted gain, delay,
  and lag. Explain why Plan B was rejected from its predicted physical result
  rather than from a magic amplitude rule, and why the aborted run remains
  useful safety evidence but invalid identification evidence. Explain that a
  steady baseline and one changed input support causal interpretation; they do
  not prove the assumed model structure.
- **Transfer and required conclusion:** Experiment design is a control-
  engineering decision that precedes data collection. A reviewed plan states
  the question, operating region, excitation, expected response, informative
  duration, controlled disturbances, limits, aborts, restoration, evidence
  class, and approval boundary. Lesson 38 next compares excitation shapes
  because the safe step designed here does not necessarily reveal every plant
  dynamic or parameter.

#### Lesson 38 — Excitation methods: water heater and flexible-drive servo

- **Objective:** Select and execute an excitation that is informative for a
  slow water heater or a resonant flexible-drive servo while respecting each
  plant's actuator, operating-region, duration, and signal-quality limits.
- **Context:** The water heater uses a one-sided 0-to-100-percent heating
  command around a 20-percent operating point and measures temperature in
  degrees Celsius. Its local response has 0.2 degrees Celsius per percentage
  point gain, 5 seconds time constant, and 1 second dead time. The flexible
  drive uses bidirectional torque limited to plus or minus 0.2 newton metres
  and measures shaft position in radians. Its local response has a 6-radian-
  per-second natural frequency and 0.08 damping ratio. Both are simulated with
  deterministic measurement noise and a 100-millisecond task interval.
- **Reason:** Lesson 37 designed one safe step for a slow, self-regulating heat
  exchanger. A step reveals steady gain, delay, and a dominant lag well, but it
  concentrates most input variation at low frequency. It can leave a flexible
  shaft ringing without deliberately separating its resonance, and a long
  sustained move may consume unnecessary thermal operating range. The
  excitation must match both the dynamics sought and the equipment that must
  survive the test.
- **Theory before practice:** Define excitation as a deliberate, bounded input
  variation used to make plant dynamics observable above normal measurement
  variation. Define amplitude as the largest intended departure from the
  baseline input and frequency content as the range of change rates present in
  that input.

  Define a step as one sustained input change; it contains strong low-frequency
  information and is useful for steady gain, delay, and dominant lag. Define a
  pulse as a step followed by restoration; it reduces the time spent away from
  the operating point but may end before a slow response becomes clear. Define
  a doublet as equal positive and negative pulses; its approximately zero net
  input area reduces lasting displacement and exposes directional asymmetry.
  Define relay excitation as feedback-dependent switching between two input
  levels; it deliberately seeks an oscillation and therefore requires the
  separate limit-cycle safeguards taught in Lesson 43. It shall be explained
  here but rejected by this open-loop lesson rather than imitated as a square
  wave.

  Define a pseudo-random binary sequence (PRBS) as a deterministic sequence of
  two input levels whose switching pattern spreads information over a chosen
  frequency band. Define dwell time as the duration of each binary level and
  explain that a slow plant needs longer dwell than a fast plant. Define a
  chirp as a sinusoidal input whose frequency changes continuously from a
  declared start frequency to a declared end frequency. It maps frequency-
  dependent response efficiently but its amplitude, sweep rate, endpoint, and
  resonance dwell determine safety and resolution.

  Define persistent excitation as input variation containing enough
  independent temporal content to estimate every parameter in the chosen model
  structure. It does not mean simply using many samples or switching as fast as
  possible. Define signal amplitude (A_signal) as the expected output change
  attributable to excitation and noise amplitude (A_noise) as the declared
  root-mean-square measurement variation. Define signal-to-noise ratio (SNR)
  as their dimensionless ratio, then write `SNR = A_signal / A_noise`. Explain
  that a large input that clips or leaves the model's operating region is not
  rescued by a high SNR.

  For the heater, a 5-percentage-point input amplitude predicts 1 degree
  Celsius output amplitude. With 0.02-degree-Celsius root-mean-square noise,
  derive the predicted SNR as `(0.2*5)/0.02 = 50`. Use a 2.5-second PRBS dwell so switching is not much
  faster than the 5-second thermal lag. For the servo, 0.1-newton-metre torque
  remains inside its bound. Use a 0.3-second PRBS dwell, exactly three
  100-millisecond task scans, and a 20-second chirp
  from 0.2 to 2 hertz; derive the flexible-mode frequency from
  `f_n = omega_n/(2*pi) = 6/(2*pi) = 0.955 Hz`. Define the chirp law
  `f(t)=f0+(f1-f0)t/T` and derive phase as the integral of frequency, so the
  scan update adds `2*pi*f(t)*dt`. Derive the five-second positive and negative
  doublet area as `(+0.1)(5)+(-0.1)(5)=0 N m s`.
- **Prediction:** Before running, classify each method by sustained offset,
  return to baseline, sign changes, approximate net input area, and useful
  frequency band. Predict that the heater step gives the clearest final gain,
  while a too-fast chirp is strongly attenuated and gives poor thermal
  information. Predict that the servo doublet returns torque to zero with
  approximately zero net torque area, while the chirp produces its largest
  response near 0.95 hertz. Predict deterministic rejection of relay selection
  in this open-loop lesson.
- **Controller under test:** There is no feedback controller or fitted model.
  The actual Structured Text shall generate bounded step, pulse, doublet, PRBS,
  and chirp commands from the selected plant-specific amplitude and timing. It
  shall use one deterministic PRBS state sequence, retain the actual switching
  times, restore each baseline, and expose requested and applied excitation.
  Method selection shall reject an unsafe amplitude, invalid frequency band,
  insufficient duration, or relay request with a concise reason code before
  either plant input moves.
- **Experiment:** In Control Studio Monitor, verify Simulation evidence, plant
  selection, baseline input and output, requested and applied excitation,
  measurement noise, sample interval, actuator and output limits, method,
  amplitude, duration, dwell or sweep band, quality, and restoration. In Test,
  select `Observe without excitation`, amplitude zero, and 30.1 seconds. The
  test may write only `P1.ExperimentEnabled`; the project owns the selected
  servo chirp, its plant/method configuration, timing, and zero-torque
  restoration. Fresh native runs own the heater step, pulse, slow PRBS,
  too-fast chirp, servo doublet, fast PRBS, and rejected relay so comparisons
  use fresh initial state with unchanged noise and seed. Do not Identify, Tune,
  Apply, or invent a disabled Validate stage.
- **Comparison measures:** Define input total variation as the sum of absolute
  successive applied-input changes, in percentage points for the heater and
  newton metres for the servo. Define signed input area as the sampled sum of
  input deviation multiplied by sample interval, in percentage-point-seconds
  or newton-metre-seconds. Define output span as maximum minus minimum measured
  output, in degrees Celsius or radians. Define return error as the absolute
  final applied-input deviation from baseline. Define heater information ratio
  as measured temperature span divided by the declared temperature-noise root-
  mean-square amplitude. Define servo resonance peak frequency as the chirp
  frequency at the largest qualified shaft-position amplitude. Every measure
  shall retain its signal, unit, sample interval, window, and qualification
  state; a rejected or clipped run is `Not qualified`.
- **Validation:** Prove every waveform's declared timing and sign sequence from
  applied-input samples, not its method label. Verify the heater step produces
  the predicted approximately 1-degree-Celsius change and the declared PRBS
  has lower switching rate than the servo PRBS. Verify the doublet's signed
  torque area is within 0.01 newton-metre-seconds of zero and final torque
  returns to zero. Verify the servo chirp's largest response occurs within 0.15
  hertz of 0.95 hertz without exceeding torque or position limits. Confirm the
  too-fast heater chirp has materially lower qualified output span than its
  step, demonstrating that more bandwidth is not automatically more
  information. Relay selection shall return an ineligible reason and zero
  input departure. All results are simulation evidence.
- **Explanation:** Connect the heater results to its lag acting as a low-pass
  process and the servo results to energy stored in its flexible mode. Explain
  why plant response, not waveform complexity, determines useful information;
  why PRBS dwell and chirp sweep band are engineering choices; and why SNR,
  limits, and operating-region validity must be considered together.
- **Transfer and required conclusion:** Excitation shall match the parameters,
  time scales, frequency band, directionality, constraints, and evidence
  quality of the plant and candidate model. No single waveform is universally
  best. Lesson 39 next uses qualified data to compare model structures, because
  informative excitation still does not decide whether the plant is first-
  order, delayed, second-order, or integrating.

#### Lesson 39 — Fit and select model structure: conveyor dryer

- **Objective:** Fit first-order, first-order-plus-dead-time, second-order, and
  integrating candidates to one conveyor-dryer experiment, reject structures
  that contradict repeatable response features, and select the simplest model
  that predicts independent data within declared tolerances.
- **Context:** A continuous conveyor carries wet product through a heated-air
  drying tunnel. Heater demand is the manipulated input in percent and product-
  zone temperature is the measured output in degrees Celsius. Air transport
  and sensor location delay the measured response. Near 30 percent heater and
  80 degrees Celsius, the local simulated dryer has gain 0.4 degrees Celsius
  per percentage point, time constant 12 seconds, and dead time 4 seconds.
  Measurement noise has a deterministic 0.05-degree-Celsius root-mean-square
  amplitude. The local model is valid only from 20 to 50 percent heater and 75
  to 90 degrees Celsius.
- **Reason:** Lesson 38 showed how qualified excitation can contain useful
  information across the dynamics of interest. Informative data still do not
  name the model structure. Several candidates can lower their error on the
  same noisy record, and a more flexible candidate can follow noise or mimic
  delay without predicting another experiment.
- **Theory before practice:** Define a model structure as the chosen form and
  number of dynamic relationships before numerical parameters are estimated.
  Define a parameter estimate as a numerical value chosen from data inside that
  structure. Define an estimation dataset as samples used to choose parameters
  and a validation dataset as an independent experiment withheld from fitting.

  Define measured output (y_measured) and model-predicted output (y_predicted)
  in degrees Celsius. Define residual (r) as measured minus predicted output,
  then write `r = y_measured - y_predicted`. Define root-mean-square prediction
  error (RMSE) as the square root of the mean squared residual over a declared
  sample window. Explain in words before writing
  `RMSE = sqrt((1/N) sum(r_k^2))`. Explain that squaring prevents signed
  cancellation, division by N normalizes record length, and the square root
  restores degrees Celsius. Define residual bias as `(1/N) sum(r_k)` and
  residual autocorrelation as the similarity of residuals separated by a
  declared sample lag. A small RMSE with biased or strongly correlated
  residuals means repeatable dynamics remain unexplained.

  Define model complexity as the number and freedom of estimated dynamic
  parameters, not the length of the equation. Define underfit as a structure
  too simple to reproduce repeatable plant behavior and overfit as a structure
  that explains accidental estimation noise or disturbance instead of
  repeatable dynamics. Explain the bias-variance trade-off: too little
  flexibility creates systematic error, while unnecessary flexibility makes
  estimates vary more between equally valid datasets.

  Define a first-order candidate as one gain and one lag with no delay; it
  begins responding immediately. Define a first-order-plus-dead-time (FOPDT)
  candidate as one gain, one lag, and one delay. Define a second-order candidate
  as one gain plus two dynamic modes represented by natural frequency and
  damping, with no delay in this comparison. Define an integrating candidate as
  an output rate proportional to input change, so it does not approach a finite
  new equilibrium after a sustained nonzero input deviation.

  Define parameter uncertainty as a range of parameter values consistent with
  the declared data and noise assumptions. This lesson shall estimate it by
  repeating the fit on early and late qualified estimation subsets and report
  the spread; it shall not present one optimizer result as exact plant truth.
  Define material validation improvement before viewing results as
  `100*(RMSE_simple-RMSE_complex)/RMSE_simple`: an added
  structure is justified only when this reduces independent-validation RMSE by at
  least 10 percent, removes a repeatable residual feature, and keeps every
  parameter physically plausible and sufficiently certain.
- **Prediction:** A positive 10-percentage-point heater step predicts a
  4-degree-Celsius final rise from 80 to 84 degrees Celsius. No response should
  be attributable to the heater during the first 4 seconds. Derive the FOPDT
  prediction `T(t)=80+4*(1-exp(-(t-4)/12))` for `t >= 4 s`, including
  `T(16)=82.528 degC`. A first-order
  no-delay model should therefore produce early residual bias. An integrating
  model should fail the observed finite equilibrium. An FOPDT candidate should
  explain delay, monotonic lag, and equilibrium with three physically
  interpretable parameters. A second-order no-delay candidate may reduce
  estimation error by bending its two modes but shall not be preferred unless
  independent evidence meets the predeclared material-improvement rule.
- **Controller under test:** There is no feedback controller. The actual
  Structured Text shall issue only the reviewed manual heater experiment and
  restore 30 percent. The simulator shall execute the unknown-to-the-learner
  dryer response and seeded measurement noise. Candidate prediction and
  residual traces shall be produced by the Control Studio identification
  implementation from retained samples; the lesson source shall not contain
  hidden candidate scores or the accepted answer.
- **Experiment:** In Monitor, verify Simulation evidence, heater request and
  applied demand, temperature, units, 100-millisecond sampling, operating
  region, quality, noise declaration, and absence of an undeclared load. In
  Test, select `Observe without excitation`, amplitude zero, and 90.1 seconds.
  The test may write only `P1.ExperimentEnabled`; the project owns the
  10-percentage-point estimation step and restoration to 30 percent. A fresh
  native run owns the independent 5-percentage-point validation pulse and fits
  all four frozen structures to the same estimation window with the same
  weighting and sample inclusion. Do not expose a shared Identify, Tune,
  Apply, or disabled Validate stage as authority for these project-owned fits.
- **Comparison measures:** For every candidate, retain parameter count,
  estimates, uncertainty ranges, estimation RMSE, validation RMSE, residual
  bias, lag-one residual autocorrelation, stability, equilibrium behavior,
  physical plausibility, and deterministic rejection reason. Define delay-
  window absolute bias as the absolute mean residual during the first 4 seconds
  after the input step. Define parameter subset spread as maximum minus minimum
  estimate across the early and late qualified fits. Comparison tables shall
  identify `Not applicable`, `Not identifiable`, or `Rejected` rather than
  inventing a number whose prerequisite is absent.
- **Validation:** Verify the retained plant data have approximately 4 seconds
  delay and 4 degrees Celsius equilibrium change within noise tolerance. Require
  the first-order candidate to fail the delay-window bias criterion and the
  integrating candidate to fail equilibrium behavior. Require the FOPDT
  candidate to recover gain within 0.03 degrees Celsius per percentage point,
  time constant within 1.5 seconds, and dead time within 0.5 seconds, with
  validation RMSE at most 0.10 degrees Celsius and absolute residual bias at
  most 0.03 degrees Celsius. Reject a second-order candidate when it does not
  improve independent RMSE by 10 percent and remove a repeatable residual
  feature. Select FOPDT only if these gates all pass; otherwise the correct
  lesson result is no accepted model.
- **Explanation:** Connect the first-order residuals to missing transport delay,
  the integrating failure to the observed finite equilibrium, and any
  second-order estimation improvement to extra flexibility rather than assumed
  physics. Explain why the FOPDT model is selected from independent prediction
  and residual behavior, not because its name matches the simulator
  configuration or because it has the lowest training error.
- **Transfer and required conclusion:** More parameters do not automatically
  produce a more useful control model. Select the simplest structure that
  preserves the repeatable behavior needed for the engineering decision and
  predicts independent data inside a declared operating region. Lesson 40 next
  shows that even correct candidate structures must be rejected when excitation
  and closed-loop correlation make their parameters unidentifiable.

#### Lesson 40 — Poor excitation and closed-loop identification: heat exchanger

- **Objective:** Distinguish an undersized open-loop experiment, a qualified
  open-loop experiment, and an ordinary closed-loop record on the same plate
  heat exchanger; accept only data that can support the declared identification
  method and retain a deterministic reason for every rejection.
- **Context:** A hot-water valve heats a product stream through a plate heat
  exchanger. Valve command is measured in percent and outlet temperature in
  degrees Celsius. The local simulated plant operates at 20 percent valve and
  50 degrees Celsius with gain 0.5 degrees Celsius per percentage point, time
  constant 20 seconds, dead time 3 seconds, and 0.15-degree-Celsius root-mean-
  square measurement noise. The lesson runs three independent copies of this
  same plant with the same 100-millisecond task interval and declared evidence
  class.
- **Reason:** Lesson 39 selected a model only after independent prediction, but
  that comparison assumed the estimation data could distinguish the candidate
  parameters. An identification algorithm can still return finite, plausible-
  looking numbers from a nearly constant input or from feedback data whose input
  was created in response to the measured noise. Numerical output is not proof
  that the parameters were observable from the experiment.
- **Theory before practice:** Define identifiability as the ability of the
  available input-output data, under stated assumptions, to distinguish the
  parameters being estimated. Define an identifiable parameter as one whose
  different plausible values produce measurably different predictions in the
  collected data. Define an unidentifiable parameter as one for which several
  values or structures remain indistinguishable inside the measurement noise
  and experiment bandwidth.

  Reuse persistent excitation from Lesson 38: the input must contain enough
  independent temporal variation for the chosen model order. Define input span
  as maximum minus minimum applied input in percentage points. Define expected
  response amplitude (A_response) in degrees Celsius and measurement-noise
  amplitude (A_noise) in degrees Celsius. Define response-to-noise ratio (SNR)
  only after both quantities, then write `SNR = A_response / A_noise`. The
  qualification threshold for this lesson is SNR at least 5, applied-input span
  at least 5 percentage points, and a response window covering dead time plus
  three time constants.

  Define collinearity as two explanatory signals changing in nearly the same
  pattern, so their separate effects cannot be determined reliably. Explain
  that fast sampling creates more rows of nearly repeated data but does not add
  independent information. Define an exogenous excitation as a reviewed input
  change chosen independently of current measurement noise. Define an
  endogenous input as one partly produced in response to the measured output,
  as occurs in feedback.

  Define measurement noise (n) as the sensor variation added to the underlying
  process temperature. Define controller-used temperature (PV_used) as the
  measured value used to calculate the next valve request. Define valve
  deviation (delta_MV) as applied valve position minus its 20-percent baseline.
  The ordinary proportional feedback record uses 2 percentage points per
  degree Celsius and therefore contains the relationship
  `delta_MV = 2 * (50 - PV_used)`. Measurement noise inside `PV_used` therefore
  also appears in `delta_MV`: substituting `PV_used = PV_true + n` places the
  term `-2n` directly in the valve deviation and explains the negative
  association.

  Define correlation coefficient (rho) explicitly as
  `sum((x-x_mean)(y-y_mean)) / sqrt(sum((x-x_mean)^2) sum((y-y_mean)^2))`.
  Its value lies
  from minus one to plus one. A magnitude near one means strong linear
  association, not causation. In the closed-loop record, strong negative
  correlation between `PV_used` and `delta_MV` is partly created by the
  controller itself. An ordinary open-loop estimator that assumes input is
  independent of noise is biased on this record; closed-loop identification
  requires a method and excitation designed for that dependency.

  Define deterministic data qualification as a fixed ordered set of checks
  evaluated before parameter fitting. This lesson uses reason `1` for
  insufficient response-to-noise ratio or input span, reason `2` for missing
  independent excitation or disallowed closed-loop correlation, reason `3` for
  clipping, saturation, gaps, drift, invalid quality, nonfinite samples, or an
  inadequate observation window, and reason `0` only when all checks pass.
- **Prediction:** The undersized open-loop step is 0.2 percentage points, so it
  predicts only 0.1 degree Celsius response. Dividing by 0.15 degree Celsius
  noise predicts SNR `0.67`; it shall be rejected with reason 1 even if a fit
  returns finite gain, time, and delay. The qualified 10-percentage-point step
  predicts 5 degrees Celsius and SNR `33.33`; it should pass data qualification
  when duration and quality also pass. Derive its 63-second movement as
  `5*(1-exp(-(63-3)/20)) = 5*(1-exp(-3)) = 4.751 degC`. The ordinary feedback record should show
  valve movement and an input-output relationship but strong controller-created
  correlation; the open-loop identification route shall reject it with reason
  2 rather than report a model.
- **Controller under test:** The actual Structured Text shall route separate
  requested and applied commands to the undersized and qualified open-loop
  plants. A third path shall execute the declared proportional feedback law and
  expose the exact temperature sample used in that calculation. The code shall
  contain no model parameters, candidate fit, precomputed score, or hidden
  accepted answer. All three plants shall use the same physical parameters but
  distinct retained signals and noise streams.
- **Experiment:** In Control Studio Monitor, verify Simulation evidence, all
  three requested/applied inputs and outputs, units, quality, 100-millisecond
  sampling, noise declaration, limits, modes, and controller-used measurement.
  In Test, select `Observe without excitation`, amplitude zero, and 70.1
  seconds. The test may write only `P1.ExperimentEnabled`; the project applies
  the 0.2-point and 10-point steps, executes the feedback law with no independent
  excitation, and restores all three valves to 20 percent when authority
  becomes false. Native evidence owns the ordinary open-loop qualification
  arithmetic and reason codes. Do not expose Identify, Tune, Apply, or a
  disabled Validate stage as authority for rejected records.
- **Comparison measures:** Define applied-input span, output span, predicted
  response amplitude, measured noise amplitude, SNR, observation duration,
  missing-sample count, clipped-sample count, late-window slope, and quality for
  every record. For the feedback record, additionally retain whether excitation
  is exogenous, correlation between controller-used temperature and valve
  deviation, and the identification method's independence assumption. Define
  parameter sensitivity spread as maximum minus minimum fitted parameter across
  noise-consistent resamples; report it only after the data pass basic
  qualification. A rejected record retains samples and reason but has no
  accepted model or parameter estimate.
- **Validation:** Verify the poor experiment has applied-input span 0.2
  percentage points, predicted equilibrium response 0.1 degree Celsius, SNR
  approximately 0.67, and deterministic reason 1. Verify the qualified
  experiment has 10-point span, predicted equilibrium response 5 degrees
  Celsius, approximately 4.75 degrees Celsius response at the minimum
  dead-time-plus-three-time-constants window, SNR approximately 33.33, adequate
  duration, and reason 0. The 63-second value is a dynamic-window check, not a
  claim that the noisy response has fully settled. Verify the ordinary feedback record has no
  exogenous excitation and correlation magnitude at least 0.95 between the
  controller-used measurement and its computed valve deviation; reject it with
  reason 2 for the open-loop estimator. No failure path may create a model,
  enable tuning, or discard the raw evidence.
- **Explanation:** Explain that poor-case parameter variation comes from the
  response being smaller than ordinary noise, while the closed-loop problem is
  causal dependence: the controller makes the input respond to the measured
  output and its noise. More samples, a higher fit percentage, or a more complex
  model cannot manufacture the missing independent information. The qualified
  step passes because its amplitude, duration, operating region, quality, and
  independence match the stated estimator assumptions.
- **Transfer and required conclusion:** The correct identification result may
  be `Data inadequate`, accompanied by retained samples and a precise reason,
  rather than a model. Improve the experiment or choose a qualified closed-loop
  method; do not tune from unidentifiable evidence. Lesson 41 next assumes data
  pass these gates and asks whether the resulting fitted model predicts truly
  independent amplitudes, disturbances, and operating points.

#### Lesson 41 — Independent validation and uncertainty: heat exchanger

- **Objective:** Validate one frozen first-order-plus-dead-time heat-exchanger
  model against three independent records and issue an accepted, restricted,
  or rejected verdict with an explicit purpose and operating region. The
  learner shall explain why low estimation error and a narrow parameter
  interval do not establish prediction outside the evidence used to obtain
  them.
- **Context:** Lesson 40's qualified record supports a frozen local candidate
  with gain 0.5 degrees Celsius per valve percentage point, time constant 20
  seconds, and dead time 3 seconds. Its fit reports gain interval 0.47 to 0.53,
  time-constant interval 18 to 22 seconds, and dead-time interval 2.5 to 3.5
  seconds at the declared 95-percent confidence level. The model covers a
  plate heat exchanger near 20-percent valve position and 50 degrees Celsius.
  Three independent simulator records use a new 5-percentage-point valve step:
  the same local fixed-flow condition, the same condition followed by an
  unmodelled cold-flow load, and a high-flow operating point whose true gain is
  0.25 degrees Celsius per percentage point. Each plant copy has independent
  0.15-degree-Celsius measurement noise and 100-millisecond sampling.
- **Reason:** Lesson 40 established that a record can contain enough independent
  information to fit a model. That qualification does not prove the fitted
  model predicts unseen input amplitudes, disturbances, or operating points.
  Reusing estimation samples, changing parameters after seeing validation
  residuals, or reporting a fit percentage again would answer the wrong
  question.
- **Theory before practice:** Define estimation data as the immutable record
  used to select structure and estimate parameters. Define validation data as
  a different immutable record excluded from all fitting decisions. Define a
  frozen model as a model whose structure, parameters, intervals, initial-state
  rule, and preprocessing cannot change after validation begins. A holdout
  experiment produces validation data under a predeclared condition and
  acceptance rule.

  Define measured outlet temperature (PV_measured) as the simulator or sensor
  value in degrees Celsius. Define model-predicted outlet temperature
  (PV_predicted) as the frozen model's output for the recorded applied valve
  position. Define the residual at sample number (k), residual (r[k]), as
  measured minus predicted temperature, then write
  `r[k] = PV_measured[k] - PV_predicted[k]`.

  Define root-mean-square prediction error (RMSE) as the square root of the mean
  squared residual over a declared window. Define residual bias as the mean
  residual over that window. Define lag-one residual autocorrelation (rho_1) as
  the centered correlation between each residual and the preceding residual.
  RMSE measures typical prediction error, bias exposes a persistent offset, and
  autocorrelation exposes repeatable time structure left unexplained. A small
  RMSE alone cannot distinguish random measurement noise from systematic model
  mismatch.

  Define a parameter uncertainty interval as the range of parameter values
  supported by the identification method and its assumptions. Define confidence
  level as the long-run coverage attached to that interval-producing procedure;
  it is not a guarantee that this particular realized interval contains the
  physical truth. Define prediction uncertainty envelope as the minimum and
  maximum predicted output produced by the frozen parameter intervals plus the
  declared measurement-noise allowance at every sample. Define envelope
  coverage as `N_inside/N_qualified`. For the 5-point step, show that the gain
  interval alone gives 2.35 to 2.65 degrees Celsius equilibrium movement and
  that adding the plus-or-minus-0.30-degree noise allowance expands it to 2.05
  through 2.95 degrees Celsius; transient bounds must also evaluate the time-
  constant and delay interval corners.
  A narrow envelope is useful only when independent coverage agrees.

  Define interpolation as prediction between conditions represented by the
  qualified evidence. Define extrapolation as prediction beyond the tested
  input, disturbance, or operating region. Define model mismatch as repeatable
  plant behavior absent from the candidate model. Define validity region as the
  inputs, outputs, disturbances, modes, and operating points for which the
  validation evidence applies.

  An **accepted** verdict means all predeclared measures pass for one stated
  purpose and region. A **restricted** verdict means a useful subset passes but
  another required purpose or condition fails; both the supported and excluded
  uses shall be named. A **rejected** verdict means the model fails the declared
  purpose even after the data and validation calculation themselves qualify.
  None of these verdicts may refit the candidate or discard a failing record.
- **Prediction:** For the local 5-percentage-point step, the frozen gain predicts
  2.5 degrees Celsius equilibrium movement. One time constant after the
  3-second delay derive `2.5*(1-exp(-1)) = 1.580`, about 63.2 percent of that movement,
  degrees Celsius. The local fixed-flow record should stay inside the frozen
  uncertainty envelope apart from the declared noise allowance. When cold flow
  later lowers equilibrium temperature by 2 degrees Celsius, a model without a
  cold-flow input continues to predict the former temperature and should develop
  approximately minus-2-degree-Celsius residual bias. At the high-flow point,
  the real gain 0.25 predicts only 1.25 degrees Celsius movement; applying the
  frozen 0.5 gain predicts 2.5, so the late residual should approach minus 1.25
  degrees Celsius and leave the local envelope.
- **Controller under test:** The actual Structured Text shall generate the same
  reviewed 20-to-25-percent open-loop valve step for three independent plant
  copies, retain requested and applied commands separately, clamp each valve to
  0 through 100 percent, and restore 20 percent when disabled. It shall contain
  no PID controller, fitted model, uncertainty interval, prediction, residual,
  score, or verdict. Model evaluation belongs to the validation analysis so
  plant measurement cannot be overwritten and expected answers cannot be
  embedded in the lesson program.
- **Experiment:** In Monitor, verify Simulation evidence, the frozen model and
  interval provenance from Lesson 40, distinct estimation and validation run
  identities, three plant conditions, requested/applied valve positions,
  measured temperatures, units, 100-millisecond sampling, noise, quality,
  limits, operating point, and safe restoration. Before running, record the
  2.5-degree equilibrium, 1.58-degree one-time-constant, minus-2-degree load,
  and minus-1.25-degree high-flow predictions and freeze the metrics below.
  In Test select `Observe without excitation`, amplitude zero, and 160.1
  seconds. The test may write only `P1.ExperimentEnabled`; the project applies
  all three 5-point steps and restores all commands to 20 percent. Native
  evidence owns the exact time-zero records, 90-second cold-flow event, and
  replay through the frozen model and interval corners without refitting. Do
  not expose shared Identify, Tune, Apply, or a disabled Validate stage as
  authority for this project-owned validation.
- **Comparison measures:** Calculate RMSE, residual bias, lag-one residual
  autocorrelation, uncertainty-envelope width, envelope coverage, missing and
  invalid samples, clipping, saturation, late-window slope, and operating-region
  membership from each retained record. State exact source signals, units,
  sample inclusion, windows, and validity prerequisites. Compare the local
  fixed-flow window from 20 to 80 seconds, the cold-flow record before the load
  over the same window and after the load from 130 to 160 seconds, and the
  high-flow record from 130 to 160 seconds. Do not average passing and failing
  regions into one reassuring score.
- **Validation:** The local record is accepted for 20-to-25-percent valve steps
  near 50 degrees Celsius only when RMSE is at most 0.20 degrees Celsius,
  absolute residual bias is at most 0.05 degrees Celsius, absolute lag-one
  residual autocorrelation is at most 0.20, and at least 90 percent of qualified
  samples lie inside the frozen prediction envelope. The cold-flow record shall
  pass those local pre-load gates but develop post-load residual bias magnitude
  from 1.8 to 2.1 degrees Celsius; classify the model as restricted to fixed-
  flow command prediction and rejected for cold-flow disturbance prediction.
  The high-flow record shall have late residual bias magnitude at least 0.9
  degrees Celsius and envelope coverage at most 20 percent; reject the local
  model for that operating point. Each verdict shall retain record identity,
  frozen model identity, intervals, predictions, envelope, residuals, metrics,
  purpose, validity region, failed criteria, seed, and Simulation label.
- **Explanation:** The local holdout passes because the plant retains the gain,
  lag, delay, and disturbance assumptions represented by the estimation data.
  The load case is restricted because the candidate has no cold-flow input; its
  systematic residual is information about missing model structure. The high-
  flow case is rejected because heat-transfer gain changed with operating point;
  a confident local estimate cannot justify extrapolation. Refit only in a new
  identification cycle with new estimation data, never during validation.
- **Transfer and required conclusion:** Only a validated model may support
  tuning, prediction, or robustness claims, and only for its recorded purpose
  and validity region. Carry restrictions and rejected regions into controller
  selection. Lesson 42 next uses the accepted local command model while keeping
  the cold-flow and high-flow limitations visible as validation obligations for
  every tuning candidate.

### 3.6 PID tuning methods

#### Lesson 42 — Manual and historical tuning: heat exchanger

- **Objective:** Build a conservative proportional-integral tuning by staged
  manual experiments, reproduce a bounded Ziegler-Nichols ultimate-cycle
  calculation in simulation, and decide which candidate better satisfies the
  heat exchanger's declared accuracy, response, and valve-use requirements.
  The learner shall understand what each term changes before using an automatic
  tuning method.
- **Context:** Use the Lesson 41 heat exchanger inside its accepted local
  fixed-flow region: gain 0.5 degrees Celsius per valve percentage point, time
  constant 20 seconds, dead time 3 seconds, 20-percent equilibrium valve
  position, 50-degree-Celsius initial temperature, 100-millisecond sampling,
  and 0.05-degree-Celsius measurement noise. Four independent copies receive
  the same 50-to-52.5-degree-Celsius setpoint change: manual proportional-only,
  manual proportional-integral, critical proportional-only, and historical
  proportional-integral. A minus-1-degree-Celsius equilibrium load occurs at
  100 seconds on the manual and historical PI copies. Valve position is limited
  from 0 through 100 percent.
- **Reason:** Lesson 41 established a model that may support local tuning, but a
  model does not choose the desired compromise between response speed,
  overshoot, disturbance rejection, robustness, and actuator movement. Before
  relay or model-based automation, the learner must see how deliberate manual
  changes and a historical formula encode different performance assumptions.
- **Theory before practice:** Define control error (e) as setpoint (SP) minus
  measured outlet temperature (PV), in degrees Celsius. Define operating-point
  bias as the valve position needed to hold the initial temperature at zero
  error. Define proportional gain (Kp) as requested valve percentage points per
  degree Celsius of present error. Proportional action responds immediately;
  increasing it reduces offset and speeds response but consumes stability
  margin in a lagged and delayed plant.

  Define integral contribution (I) as accumulated past error in valve percent.
  Define integral gain (Ki) as valve percentage points per degree Celsius per
  second. Integral action removes a persistent error but excessive integral
  gain creates overshoot, oscillation, saturation, and slow recovery. Define
  integral time (Ti) as the proportional gain divided by integral gain, then
  write `Ti = Kp / Ki` when `Ki` is nonzero.

  Define derivative gain (Kd) as valve percentage-point seconds per degree
  Celsius and derivative time (Td) as derivative gain divided by proportional
  gain. Derivative action reacts to measured rate and can add damping, but it
  amplifies measurement noise and is unnecessary when PI already satisfies this
  slow thermal loop. This lesson therefore explains derivative action but does
  not add it merely to produce a PID controller.

  The parallel positional form is explained in words before notation: bias,
  present-error contribution, accumulated-error contribution, and measured-rate
  contribution are combined and limited to the actuator range. With derivative
  on measurement, write
  `MV_requested = Bias + Kp*e + I - Kd*d(PV)/dt` and
  `dI/dt = Ki*e`. The shipped controller returns the actuator-facing limited
  output. This lesson names that signal applied manipulated variable
  (MV_applied), reports saturation separately, and does not present it as an
  unconstrained request.

  Manual tuning changes one term at a time. Begin with bias and proportional
  action, observe offset and oscillation tendency, then add only enough integral
  action to remove the remaining error inside the required time. Derivative is
  added only when a measured damping problem justifies its noise cost.

  Define ultimate gain (Ku) as the proportional-only gain at which the declared
  closed-loop experiment produces approximately constant-amplitude oscillation.
  Define ultimate period (Pu) as the time in seconds between equivalent peaks
  of that oscillation. Both depend on plant, operating point, limits, sampling,
  and experiment method; they are not universal plant constants. For the
  accepted FOPDT model, the critical frequency solves the minus-180-degree phase
  condition. The predicted values are `Ku = 22.24 %/degC` and `Pu = 11.35 s`.

  The historical Ziegler-Nichols closed-loop PI rule uses
  `Kp = 0.45*Ku`, `Ti = Pu/1.2`, and `Ki = Kp/Ti`. Its quarter-amplitude-decay
  intent means successive deviation peaks were expected to shrink to roughly
  one quarter of the preceding peak in the idealized test. That aggressive
  historical objective is not a modern default and is not guaranteed on a
  constrained, noisy, nonlinear, or uncertain plant.
- **Prediction:** The manual proportional candidate uses bias 20 percent and
  `Kp = 2.0 %/degC`. For a 2.5-degree-Celsius setpoint change, the plant and
  controller equilibrium obey `delta_y=0.5*(2e)=e` and
  `e=2.5-delta_y`; solving them gives 1.25 degrees Celsius rise and 1.25
  degrees Celsius remaining error;
  predict that offset before running. The manual PI candidate keeps
  `Kp = 2.0` and first adds `Ki = 0.05 %/(degC*s)`, so `Ti = 40 s`; predict that
  it will miss the 0.10-degree-Celsius post-load requirement at 180 seconds.
  Retain that rejected trial, then raise only integral gain to
  `Ki = 0.10 %/(degC*s)`, so `Ti = 20 s`, as the second manual candidate. From `Ku = 22.24` and
  `Pu = 11.35 s`, derive historical PI
  `Kp=0.45*22.235=10.006 %/degC`, `Ti=11.35/1.2=9.458 s`, and
  `Ki=10.006/9.458=1.058 %/(degC*s)`. Predict faster correction and
  materially greater valve travel than the manual PI candidate.
- **Controller under test:** The actual Structured Text shall implement the
  manual and critical proportional laws explicitly and instantiate the shipped
  `Control.PID` once for manual PI and once for historical PI. Both instances
  shall use the same parallel form, 20-percent bias, measurement derivative
  weighting, 0-through-100-percent limits, cycle time, anti-windup, and initial
  state. The only tuning differences shall be the declared Kp and Ki values;
  Kd remains zero. The program shall expose each applied command and measured
  temperature, accept the common outlet-temperature setpoint directly from its
  reviewed Control Studio binding, and contain no second experiment-enable
  write, precomputed response metric, ranking, or accepted candidate.
- **Experiment:** In Monitor, verify Simulation evidence, the Lesson 41 model
  and validity restriction, four independent plant copies, SP, PV, applied MV,
  controller form, bias, gains, limits, modes, quality, 100-millisecond timing,
  noise, seed, and restoration. Record every numerical prediction and decision
  threshold first. In Test select `Observe without excitation`, amplitude zero,
  and 180.1 seconds. The test may write only `P1.ExperimentEnabled`; the project
  commands 52.5 degrees Celsius for all four cases and restores 50 degrees
  Celsius. Native evidence owns the exact 100-second load, the separately
  compiled slow-integral trial, critical peak-period calculation, equivalent
  windows, and unapplied candidate decision. Do not expose shared Tune, Apply,
  or a disabled Validate stage as authority for controllers already declared
  by the project.
- **Comparison measures:** Define proportional-only late error as mean absolute
  SP-minus-PV over 80 to 100 seconds, before the load. Define setpoint rise time
  as first 10-to-90-percent crossing duration, overshoot as peak PV above final
  SP divided by setpoint change, settling time using a declared plus-or-minus
  0.10-degree-Celsius band, load integral absolute error (IAE) as the time sum of
  absolute error from 100 to 180 seconds, and cumulative valve movement as the
  sum of absolute applied-MV changes in percentage points. Also retain peak MV,
  saturation duration, alternating critical-loop peaks, measured Pu, invalid
  samples, and abort reason. A response that does not satisfy its settling rule
  reports `Not settled`.
- **Validation:** The manual P case shall retain late error from 1.1 to 1.4
  degrees Celsius. The first manual PI trial with `Ki = 0.05` shall retain
  absolute post-load error at least 0.25 degrees Celsius at 180 seconds and be
  rejected for insufficient recovery. The critical P case shall show at least six alternating
  error peaks with measured Pu from 9 to 14 seconds and no accepted tuning
  verdict. Both PI candidates shall reach absolute final error at most 0.10
  degrees Celsius after the load without leaving the valve limits. Historical
  PI gains shall match the independent formula within 0.02, its setpoint rise
  time shall be lower than manual PI, and its cumulative valve movement shall
  be at least twice manual PI. The manual candidate is preferred for this lesson
  only if it satisfies final accuracy with no more than 5 percent overshoot and
  materially lower valve movement. Retain both candidates and every failed
  criterion; do not silently apply the preferred result.
- **Explanation:** Manual PI is slower because its integral state is deliberately
  accumulated gradually, leaving more margin against delay and noise. The
  historical rule is faster and more active because it was derived from a
  critical oscillation and targets aggressive decay. The critical experiment
  and historical formula are useful for understanding loop dynamics, but their
  risk and performance assumptions must be validated rather than inherited.
- **Transfer and required conclusion:** A tuning method is a set of performance
  assumptions, not a guarantee or universal default. Choose the simplest
  controller and least aggressive tuning that satisfy stated requirements on
  retained evidence. Lesson 43 next shows how a bounded relay experiment can
  estimate the critical point without asking an operator to hold a physical
  plant continuously near instability.

#### Lesson 43 — Relay autotuning: industrial temperature loop

- **Objective:** Execute and qualify a bounded feedback-relay experiment on the
  same heat exchanger, estimate its critical gain and period from the measured
  limit cycle, reject a record that cannot support those estimates, and derive
  one explicitly unapplied PI candidate. The learner shall be able to
  distinguish actual feedback relay from a time-driven square wave.
- **Context:** Continue inside the Lesson 41 heat exchanger's accepted local
  operating region. The plate heat exchanger begins at 50 degrees Celsius with
  20-percent steam-valve position, gain 0.5 degrees Celsius per valve
  percentage point, time constant 20 seconds, dead time 3 seconds,
  100-millisecond sampling, and deterministic 0.05-degree-Celsius measurement
  noise. One qualified simulated copy uses a symmetric 18-percentage-point relay
  around the 20-percent valve bias and 0.08-degree-Celsius process-value
  hysteresis. A second independent copy uses only 0.20 percentage points of
  relay amplitude against the same noise so its oscillation record is
  deliberately unqualified. Both valve commands remain inside 0 through 100
  percent and both tests have a 180-second maximum duration.
- **Reason:** Lesson 42 estimated critical gain by deliberately operating a
  proportional controller near sustained oscillation. That experiment was
  useful in simulation, but asking an operator to approach instability by
  repeated gain changes is imprecise and potentially hazardous. A feedback
  relay can generate a bounded repeating experiment automatically, but only if
  the switching law, noise protection, data qualification, restoration, and
  application boundary are explicit.
- **Theory before practice:** Define relay bias as the manipulated-variable
  value that holds the initial operating point, in valve percent. Define relay
  amplitude (d) as the positive distance in valve percentage points from bias
  to either relay level. The high output is bias plus amplitude and the low
  output is bias minus amplitude. For the qualified case, predict 38 percent
  and 2 percent before running.

  Define relay center as the outlet temperature around which the experiment
  switches, in degrees Celsius. Define hysteresis half-width (epsilon) as the
  positive temperature distance between the center and either switching
  threshold. The relay retains its current output inside the band, changes to
  the low output only when measured temperature reaches the upper threshold,
  and changes to the high output only when measured temperature reaches the
  lower threshold. For center 50 degrees Celsius and hysteresis 0.08 degrees
  Celsius, predict upper and lower thresholds of 50.08 and 49.92 degrees
  Celsius. Explain that hysteresis prevents measurement noise from causing
  rapid false switching; it is not a periodic dwell timer.

  Define a limit cycle as a repeatable closed-loop oscillation sustained by the
  feedback relay. Define measured process amplitude (a) as half the difference
  between qualified upper and lower outlet-temperature extrema, in degrees
  Celsius. Define measured relay period (Pu) as the mean elapsed time between
  equivalent-direction threshold crossings after transient cycles have been
  discarded, in seconds. Define period variation and amplitude variation as
  `max(abs(x_i-x_mean))/x_mean` for periods and amplitudes respectively. The record is periodic only when both variations are at most 10
  percent over at least six complete qualified cycles.

  The ideal symmetric-relay describing-function approximation relates relay
  amplitude, measured process amplitude, and critical-gain estimate (Ku). After
  defining all three quantities, write `Ku = 4*d/(pi*a)`. Explain that the
  approximation assumes a stable repeatable limit cycle, negligible actuator
  distortion, small hysteresis relative to measured amplitude, adequate
  sampling, and a plant whose relevant dynamics are represented by the
  oscillation. It is an estimate at the induced frequency, not a universal
  plant constant. The original feedback-relay method is distinct from the
  implementation-focused PID reference of Specification 32.

  Define oscillation signal-to-noise ratio as measured process amplitude
  divided by the declared measurement-noise standard deviation. A qualified
  record requires signal-to-noise ratio at least 5, measured amplitude greater
  than four hysteresis half-widths, at least six complete cycles, amplitude and
  period variation at most 10 percent, no limit or quality violation, and
  confirmed restoration. A finite number calculated from an unqualified record
  shall not be presented as a tuning result.

  Only after qualification, reuse the already explained historical PI rule:
  proportional gain is 0.45 times the estimated critical gain, integral time is
  measured relay period divided by 1.2, and integral gain is proportional gain
  divided by integral time. The result is a candidate with its method,
  assumptions, source run, and evidence class; relay completion never applies
  it.
- **Prediction:** For the qualified relay, predict valve levels 2 and 38
  percent, thresholds 49.92 and 50.08 degrees Celsius, a repeatable period near
  the 11.35-second Lesson 42 critical period, and a critical-gain estimate in
  the same engineering region as 22.24 percentage points per degree Celsius.
  With measured `a=1.293 degC`, show `Ku=4*18/(pi*1.293)=17.72 %/degC`;
  with `Pu=11.938 s`, derive `Kp=7.98 %/degC`, `Ti=9.95 s`, and
  `Ki=0.80 %/(degC*s)`. Predict that weak measured amplitude `0.085 degC`
  gives SNR `0.085/0.05=1.7` and is below `4*epsilon=0.32 degC`.
  Thus 0.20-percentage-point relay amplitude produces a temperature
  oscillation comparable to noise and hysteresis, so its record shall be
  retained but rejected rather than converted into gains.
- **Controller under test:** The actual Structured Text shall implement two
  stateful feedback relays directly from each copy's measured outlet
  temperature. It shall retain the current high or low state while the
  measurement is inside the hysteresis band, switch only at the appropriate
  feedback threshold, enforce 0-through-100-percent output bounds, restore the
  20-percent bias when its reviewed relay-amplitude request is zero or the test
  times out, and expose that request, applied valve position, and measured
  outlet temperature. The Test page shall write only
  `P1.ExperimentEnabled`; the lesson program moves the amplitude request from
  zero to 18 percentage points while authority is true; the lesson program
  alone owns feedback-dependent relay switching. It shall contain no second
  enable write, timer-driven alternating
  waveform, precomputed extrema, period, gain estimate, tuning formula, quality
  verdict, or accepted candidate. Control Studio derives those results from
  the retained record.
- **Experiment:** In Monitor, verify Simulation evidence, accepted model scope,
  stable 50-degree-Celsius baseline, both independent copies, measured process
  values, applied manipulated variables, good quality, 100-millisecond timing,
  noise, bias, output limits, relay levels, feedback thresholds, maximum
  duration, aborts, and restoration. In Test, select `Observe without
  excitation`, amplitude zero, and 180.1 seconds; the test may write only the
  Boolean authority and must restore it false. Record at least two transient cycles followed
  by at least six evaluation cycles, abort on any declared invalid condition,
  and confirm restoration to 20 percent. The low-amplitude copy runs under the
  same project authority. Native evidence calculates a candidate only from a
  record that passes every qualification criterion. Do not expose Identify,
  Tune, Apply, or a disabled Validate stage as authority for this project-owned
  experiment.
- **Comparison measures:** Retain applied high and low valve levels in percent,
  total valve transitions, upper and lower threshold-crossing times in seconds,
  upper and lower temperature extrema in degrees Celsius, measured process
  amplitude, mean period, amplitude variation, period variation, oscillation
  signal-to-noise ratio, saturation duration, invalid samples, stop reason, and
  restoration result. For a qualified record retain the critical-gain estimate
  and historical PI candidate with units and formula inputs. For an unqualified
  record report the failed criteria and `No tuning candidate`; do not expose
  meaningless gain values as if they were valid.
- **Validation:** The qualified relay shall apply exactly 2 and 38 percent,
  switch from measured threshold crossings rather than elapsed dwell, remain
  inside plant and actuator limits, produce at least six complete evaluation
  cycles, and restore exactly 20 percent. Its mean period shall be from 9 to 14
  seconds, amplitude and period variation shall each be at most 10 percent,
  signal-to-noise ratio shall be at least 5, measured amplitude shall exceed
  four times hysteresis, and estimated Ku shall be within 20 percent of the
  independent Lesson 42 critical value. The derived PI candidate shall retain
  its source run and remain unapplied. The low-amplitude record shall fail
  amplitude-to-hysteresis or signal-to-noise qualification, retain its samples
  and reason, produce no candidate, and restore the same bias.
- **Explanation:** Connect feedback-dependent switching, process lag, and dead
  time to the sustained oscillation. Explain why the qualified relay estimates
  one frequency-domain point without fitting a full process model, why
  hysteresis trades false switching against phase and estimation error, and why
  repeatability and amplitude above noise matter more than merely counting
  output transitions. Reconcile the estimate with Lesson 42 without claiming
  exact equality.
- **Transfer and required conclusion:** Relay autotuning automates a bounded
  experiment and candidate calculation; it does not automate plant eligibility,
  hazard review, data qualification, controller choice, validation, or gain
  application. A physical relay test requires independently reviewed topology,
  process hazards, operating limits, interlocks, restoration, authorised roles,
  and explicit approval immediately before the action. Lesson 44 next uses
  validated plant models to choose an explicit robustness-speed target without
  deliberately creating a critical oscillation.

#### Lesson 44 — Lambda/IMC and SIMC tuning: heater, tank, and dryer

- **Objective:** Given one independently validated local model, determine
  whether SIMC proportional-integral tuning is eligible, choose and explain a
  closed-loop response target, calculate an unapplied candidate in the truST
  parallel-controller convention, and compare its behavior without declaring
  it safe or best before later stability and robustness analysis.
- **Context:** Use three ordinary process loops whose different dynamics make
  the model domain visible. The electric recirculating water heater is a
  self-regulating first-order-plus-dead-time process. A buffer tank is an
  integrating process because unequal inlet and outlet flow keeps changing its
  level rather than approaching a new equilibrium. A continuous dryer is a
  slow self-regulating first-order-plus-dead-time process with materially more
  transport delay than the heater. Each simulated plant uses its normal
  controller-facing I/O, physical units, actuator limits, safe manual output,
  deterministic initial condition, and an explicit local model-validity range.
- **Reason:** Lesson 42 produced plausible manual and historical tunings but no
  direct way to ask for more speed or more robustness. Lesson 43 estimated one
  critical oscillation point without a complete plant model. Lessons 17 and 41
  established that an independently validated local model can predict new
  data. This lesson now uses that model to make the desired speed-versus-
  robustness choice explicit. It does not repeat model fitting and does not
  treat the simulator's configured truth as identified physical-plant evidence.
- **Theory before practice:** First explain in words that the desired
  closed-loop time (lambda) is the approximate time scale the designer asks the
  controlled response to follow: a smaller value requests faster correction
  and usually consumes more robustness and actuator effort, while a larger
  value requests a slower, less aggressive response. Introduce the
  self-regulating plant gain (K) as the eventual output change per unit input
  change, the dominant time constant (T) as the time describing most of the
  gradual response, and the effective dead time (L) as the delay before useful
  response is observed. Introduce the integrating rate gain (Kprime) separately
  as output change per unit input and per second; it is not a steady-state gain.
  State every plant-specific unit before using these names in an equation.

  After those quantities are defined, teach the SIMC PI relations used by
  Control Studio. For an accepted positive-gain first-order or FOPDT model:

  `Kp = T / (K * (lambda + L))`

  `Ti = min(T, 4 * (lambda + L))`

  For an accepted positive-rate-gain integrating model with resolved positive
  effective delay:

  `Kp = 1 / (Kprime * (lambda + L))`

  `Ti = 4 * (lambda + L)`

  In both cases, introduce the proportional gain (Kp) in actuator units per
  controlled-variable unit, the integral time (Ti) in seconds, and the parallel
  integral gain (Ki) in actuator units per controlled-variable unit per second
  before presenting the conversion:

  `Ki = Kp / Ti`

  Explain that SIMC is derived from internal-model-control or Lambda reasoning,
  but these equations produce a feedback PI controller rather than a runtime
  model-predictive controller. A first-order model produces PI settings;
  derivative action is not invented. A stable monotone positive-gain local
  model, acceptable independent validation error, compatible controller form,
  known units, and resolved timing are eligibility conditions. Oscillatory,
  inverse-response, open-loop unstable, negative-gain without an explicit
  action transformation, unvalidated, or badly uncertain models are rejected
  rather than forced through the formulas.
- **Response choices:** Control Studio shall expose `Fast`, `Balanced`, and
  `Robust` as named presets, while retaining the numerical desired closed-loop
  time. For first-order models, the fastest preset time is the larger of the
  effective delay and one third of the dominant time constant; for integrating
  models it is the positive effective delay. `Fast` uses that fastest value,
  `Balanced` uses twice it, and `Robust` uses four times it. These are truST
  product presets around the published SIMC tuning parameter, not three new
  published tuning rules. The learner shall see the actual time in seconds and
  may not select a label whose numerical value or derivation is hidden.
- **Worked predictions:** Use the validated heater model with gain `0.20
  degC/%`, time constant `5.0 s`, and delay `1.0 s`; the tank model with rate
  gain `0.05 level-%/(valve-%*s)` and effective delay `0.5 s`, obtained by the
  already taught model-reduction treatment of its `1.0 s` valve lag; and the dryer
  model with gain `0.40 dryness-%/heater-%`, time constant `20.0 s`, and delay
  `8.0 s`. Before opening Tune, calculate the three response times and at least
  the Balanced `Kp`, `Ti`, and `Ki` for every plant. Show the arithmetic:
  heater Fast is `max(1,5/3)=1.667 s` and Balanced gives
  `Kp=5/(0.2*(3.333+1))=5.769`, `Ti=5`, `Ki=1.154`; tank Balanced gives
  `Kp=1/(0.05*(1+0.5))=13.333`, `Ti=4*(1.5)=6`, `Ki=2.222`; dryer Fast is
  `max(8,20/3)=8 s` and Balanced gives
  `Kp=20/(0.4*(16+8))=2.083`, `Ti=20`, `Ki=0.104`. Predict that Fast produces
  the largest proportional gain, Robust the smallest, the integrating tank
  requires the integrating-process equation rather than a fictitious steady-
  state gain, and the dryer's delay prevents a fast response comparable to the
  heater's.
- **Controller under test:** The shipped Structured Text shall contain one real
  `Control.PID` instance for each plant, accept the candidate proportional and
  integral gains through visible lesson variables, set derivative gain to
  zero, and expose requested and applied manipulated variables separately.
  One reviewed Boolean `P1.ExperimentEnabled` shall select the
  20-to-25-degree-Celsius heater setpoint step and derive the
  equivalent 50-to-52-percent tank-level and 40-to-45-percent dryer-dryness
  steps on the same scan. The project shall restore all three baseline
  setpoints when authority is false.
  Controller code shall not contain the tuning formulas, precomputed response
  measures, acceptance flags, or a second private controller. Control Studio
  owns the candidate calculation; the executable program proves what those
  settings do when the ordinary controller runs against each plant.
- **Experiment:** In Monitor, verify all three model identities, operating
  ranges, SP, PV, requested MV, applied MV, units, good quality, 100-millisecond
  controller timing, automatic/manual state, limits, and Simulation evidence
  class. In Test select `Observe without excitation`, amplitude zero, and
  220.1 seconds; the test may write only `P1.ExperimentEnabled` and must restore
  it false. The project exposes the Balanced candidate and its two linked
  setpoints. Fresh native runs calculate and compare Fast, Balanced, and Robust
  for all three plants from equivalent initial states and the same setpoint and
  load-disturbance schedule. Abort
  on nonfinite data, invalid quality, an output-limit violation, or departure
  from the declared local model range; restore the safe manual output. Do not
  expose shared Tune, Apply, or a disabled Validate stage as authority for
  project-declared controllers; every candidate remains unapplied.
- **Controlled comparison:** For each plant, change only the response target
  and resulting candidate gains. Hold plant, initial state, setpoint and load
  events, sample schedule, duration, actuator limits, seed, and metric windows
  equal. Across plants, do not compare raw seconds, gains, or errors as though
  their units and physical purposes were identical; compare whether each
  candidate exhibits the predicted direction of the robustness-speed trade-off.
- **Decision measures:** Before viewing results, define setpoint rise time as
  the elapsed time from 10 to 90 percent of the commanded PV change, settling
  time as the first time after which PV remains inside the declared band,
  overshoot as peak PV beyond the requested change in percent of that change,
  late-window absolute error only after settling qualification, and cumulative
  applied-MV movement as the sum of absolute successive applied-output changes
  in percentage points. Retain peak applied MV, time at an output limit, and
  the exact model/candidate identity. A response that does not satisfy the
  settling rule reports `Not settled`; a saturated run does not receive an
  unqualified linear-model conclusion.
- **Validation:** Confirm formula arithmetic and units independently of the
  tuning adapter. Confirm every candidate is finite, positive, parallel-form,
  model-bound, revision-bound, and `applied: false`. On each plant, verify that
  Fast is no slower than Balanced and Balanced is no slower than Robust under
  equivalent conditions, while also exposing any increase in overshoot,
  applied-output movement, saturation, or sensitivity under the declared gain
  and delay perturbations. Include nominal, `+20 percent` plant gain, and `+50
  percent` effective-delay cases plus the same bounded measurement noise. This
  lesson may reject a candidate, but it shall not name a winning method or
  approve physical application because Lessons 45–50 have not yet established
  the required stability and frequency-domain evidence.
- **Explanation:** Reconcile each response with the equations. Increasing
  lambda enlarges the denominator of `Kp`, reducing proportional action and
  normally slowing response while preserving more room for model error and
  delay. Explain why the tank gain has a per-second unit, why its integral time
  comes from `4 * (lambda + L)`, and why inserting that rate gain into the
  self-regulating equation would be dimensionally and physically wrong.
  Explain why the delayed dryer remains slower even under its Fast preset and
  why a formula-qualified candidate can still fail actuator or uncertainty
  validation.
- **Transfer and required conclusion:** Model-based tuning is reproducible only
  when model structure, parameter units, validation status, controller form,
  response target, operating region, and uncertainty are explicit. It turns a
  speed-versus-robustness intention into reviewable gains, but the formula does
  not prove closed-loop stability, margins, noise rejection, or physical-plant
  safety. Lesson 45 therefore begins the classical stability sequence before
  Lesson 51 performs a final tuning-method comparison.

### 3.7 Classical stability and frequency-domain design

#### Lesson 45 — Closed-loop poles and Routh-Hurwitz: overhead crane

- **Objective:** Derive the complete proportional-gain stability interval for
  one overhead-crane trolley-position loop with a Routh array, classify one
  stable, one marginal, and one unstable gain before simulation, and confirm
  each classification with the executable crane without treating a finite
  trace as the proof.
- **Context and plant:** The real equipment is an overhead travelling crane.
  Motor force moves the trolley while a suspended load contributes oscillatory
  mechanical dynamics. Trolley-position setpoint is in metres, measured
  trolley position is in metres, and requested and applied motor force are in
  percent of the reviewed drive range. The local linear model applies only
  around the declared load, rope length, trolley position, and force range.
  Three independent simulated copies shall have identical dynamics, initial
  displacement, timing, limits, and seed; only proportional gain differs.
- **Reason:** Lesson 44 produced finite responses for selected model-based
  candidates and two declared perturbations, but a finite record cannot prove
  stability for every gain between or beyond those trials. Before ranking
  speed or robustness, the learner needs a method that establishes an entire
  gain interval without deliberately operating an unstable crane.
- **Theory before practice:** Explain in words that the characteristic
  equation is the equation whose roots are the closed-loop poles, and that a
  pole is a natural response mode of the closed loop. Define the complex
  frequency variable (s), left half-plane, imaginary axis, right half-plane,
  asymptotic stability, marginal stability, and instability before drawing or
  calculating pole locations. Explain that a polynomial coefficient is the
  multiplier of one power of `s`, and that the Routh array rearranges those
  coefficients so sign changes in its first column count right-half-plane
  poles without solving for every root. Teach both special cases before use: a
  zero first-column element requires the declared epsilon/limit treatment, and
  a complete zero row requires the auxiliary polynomial and indicates
  symmetric roots, including a possible imaginary-axis boundary.
- **Physical model and prediction:** Explain the crane force balance and
  linearised trolley/load dynamics in words before presenting the resulting
  loop polynomial. From the declared state model derive
  `x''' = -2*x'' - 3*x' + u`, then
  `G(s)=1/(s^3+2*s^2+3*s)`. Closing negative proportional feedback adds `K`
  to the characteristic denominator. With positive proportional gain (K), the declared local
  closed-loop characteristic polynomial is
  `p(s) = s^3 + 2*s^2 + 3*s + K`. Build the array visibly:
  the `s^3` row is `[1, 3]`, the `s^2` row is `[2, K]`, and derive the `s^1` first element
  as `(2*3-1*K)/2=(6-K)/2`; the `s^0` element is `K`. Explain each row calculation.
  Requiring every first-column element to be positive gives `0 < K < 6`.
  Before running, calculate zero sign changes for `K = 4`, the marginal
  boundary and zero-row treatment for `K = 6`, including auxiliary polynomial
  `2*s^2+6` and roots `+/-j*sqrt(3)`, and two sign changes for
  `K = 8`.
- **Controller under test:** The Structured Text shall contain three explicit
  proportional feedback paths using the same setpoint-minus-position sign
  convention and gains `4`, `6`, and `8`. Each path shall retain requested and
  applied force separately and apply the same reviewed force limit. The plant
  configuration shall contain three independent copies of the same open-loop
  crane state-space model so proportional feedback produces the declared
  characteristic polynomial. Routh calculations, pole results, stability
  labels, envelope metrics, and acceptance decisions shall not be embedded in
  the PLC program.
- **Experiment:** In Monitor, verify Simulation evidence, the three crane
  identities, initial state, setpoint, PV/MV units, quality, 10-millisecond
  timing, force and position limits, and manual/restoration state. Construct
  the Routh array and record the three predictions before enabling any case.
  In Test select `Observe without excitation`, amplitude zero, and 40.1
  seconds; the test may write only `P1.ExperimentEnabled` and must restore it
  false. This releases the same small initial trolley displacement on all three
  copies with zero setpoint and no disturbance; do not use a setpoint step that
  hides natural-response stability. Native evidence compares the array and
  numerical-root classifications with measured decay, persistence, and growth.
  Abort an individual simulated case on nonfinite data, bad quality,
  force-limit contact, or departure from the local position range; retain the
  pre-abort record and restoration state. Do not expose shared Tune or a
  disabled Validate stage as authority for project-owned gain paths.
- **Controlled comparison and measures:** Change only proportional gain. Hold
  model, initial displacement, setpoint, sample schedule, duration, limits,
  and seed equal. Define first-column sign-change count as an integer before
  results. Define position-envelope ratio as the peak absolute position in a
  declared late window divided by the peak absolute position in a declared
  early window. Define force travel as the sum of absolute successive applied-
  force changes in percentage points. Also retain peak position, peak applied
  force, limit contact, stop reason, and the exact polynomial and gain. The
  stable case shall have a decaying envelope, the marginal case shall neither
  be reported as settled nor as asymptotically stable, and the unstable case
  shall have a growing envelope or reach a declared abort limit.
- **Validation:** An independent numerical root oracle shall confirm the Routh
  right-half-plane counts and imaginary-axis boundary within declared
  tolerances. The native executable test shall then confirm that `K = 4`
  decays, `K = 6` remains marginal within finite-run tolerance, and `K = 8`
  grows or safely aborts under equivalent initial excitation. A short quiet
  prefix, final sample, or weakly excited mode shall not override the algebra.
  Simulation evidence shall never be presented as authorisation to test an
  unstable gain on a physical crane.
- **Explanation:** Relate the sign of `(6 - K)/2` to the observed cases. At
  `K = 4`, every first-column term is positive. At `K = 6`, the boundary loses
  asymptotic decay and requires the special-case treatment. At `K = 8`, the
  sequence changes sign twice, so two poles lie in the right half-plane even
  if their growth is initially difficult to see. Explain why test duration and
  initial conditions affect visibility but not the actual pole classification.
- **Transfer and required conclusion:** Routh-Hurwitz can establish a
  polynomial stability interval before hazardous trials, but it does not show
  how poles move, which modes dominate, or whether a stable response has
  acceptable damping. Lesson 46 therefore introduces root locus to explain
  pole motion and determine when gain alone cannot meet the motion requirement.

#### Lesson 46 — Root locus: servo positioning axis

- **Objective:** Construct the root locus of one industrial positioning servo,
  prove why proportional gain alone cannot meet the simultaneous damping and
  settling requirements, design one lead compensator from explicit pole and
  zero choices, and confirm the predicted improvement on an equivalent
  executable servo without violating motor-current limits.
- **Context and plant:** The real equipment is a belt-driven industrial
  positioning carriage. Carriage-position setpoint and process value are in
  millimetres; requested and applied motor-current commands are percentages of
  a bidirectional drive rating. Two independent simulated copies use the same
  local model, initial state, position move, 10-millisecond motion-control
  interval, current limits, and seed. One copy uses proportional control and
  the other the designed lead controller.
- **Reason:** Lesson 45 established which crane gains are stable, but its Routh
  array did not show how individual poles move, which pair dominates the
  response, or whether a stable choice meets damping and speed requirements.
  The servo now requires no more than 5-percent overshoot, 2-percent settling
  within 2.0 seconds, zero final position error, and no current saturation.
  These requirements create the reason to inspect and reshape the pole paths.
- **Theory before practice:** Explain in words that an open-loop pole is a
  natural mode before feedback, an open-loop zero is a frequency at which the
  transfer numerator becomes zero, and a root-locus branch is the path followed
  by a closed-loop pole as one positive scalar gain changes. Define the real
  axis, complex-conjugate pole pair, branch start and end, real-axis segment,
  breakaway point, asymptote, centroid, angle condition, magnitude condition,
  natural frequency, damping ratio, constant-damping line, dominant poles, and
  non-dominant pole before using them in a plot or calculation. Explain that a
  lead compensator places a zero and a farther-left pole to add positive phase
  and reshape the locus; a lag compensator instead emphasizes low-frequency
  accuracy and is not the design used here.
- **Uncompensated model and prediction:** Start from the scaled physical balance
  `x'' + 2*x' = u`; define position, speed, acceleration, current, inertia, and
  viscous damping, then take its zero-initial-condition Laplace transform to
  derive `G(s) = 1 / (s*(s+2))`. Its open-loop poles are `0` and `-2`, with no finite
  zero. The real-axis locus lies between them, breaks away at `-1`, and its two
  asymptotes leave the centroid `-1` at plus/minus 90 degrees. Proportional
  gain (K) gives `s^2 + 2*s + K = 0`. Derive the real-axis gain
  `K = -s*(s+2)`, differentiate it to obtain the breakaway at `-1`, and derive
  the centroid and asymptote angles from the two poles and zero finite zeros.
  For `K > 1`, the poles are
  `-1 +/- j*sqrt(K-1)`: increasing gain changes oscillation frequency but
  leaves real part `-1`, so the approximate 2-percent settling time remains
  about 4 seconds while damping decreases. At `K = 4`, predict poles
  `-1 +/- j*1.732`, natural frequency `2 rad/s`, damping ratio `0.5`, about
  16.3-percent overshoot from the exponential overshoot equation, about four
  seconds settling from `4/(zeta*omega_n)`, and failure of both declared overshoot and settling
  requirements.
- **Lead design and prediction:** Explain why the desired dominant region needs
  a more negative real part while retaining damping near `0.7`. Place the lead
  zero at `-2.5`, the lead pole at `-15`, and use lead gain `50`, giving
  `C(s) = 50*(s+2.5)/(s+15)`. Apply the angle and magnitude conditions in words
  and arithmetic before presenting the compensated characteristic polynomial
  `s*(s+2)*(s+15) + 50*(s+2.5) = 0`, expand both products, and only then present
  `s^3 + 17*s^2 + 80*s + 125 = 0`. An independent numerical calculation
  shall predict the dominant pair near `-3.235 +/- j*1.186`, damping ratio near
  `0.939`, and the non-dominant pole near `-10.530`. Evaluate the complete
  closed-loop transfer function, including its zero, rather than inferring
  overshoot from the dominant-pole damping ratio alone. Predict settling near
  `1.36 s`, overshoot near 3.2 percent, zero final error by evaluating the
  complete transfer function at zero frequency, and an initial requested
  current of 50 percent from the one-millimetre initial error and zero filter state for the declared
  one-millimetre move.
- **Controller under test:** Structured Text shall expose two explicit control
  laws rather than hide them behind generic PID instances. The proportional
  path calculates gain times position error. The lead path shall implement the
  declared first-order compensator state at 10 milliseconds and calculate its
  requested current from that state. Both paths shall retain requested and
  applied current separately and use identical `-100` to `100` percent limits.
  Pole locations, damping, response metrics, acceptance flags, and a winner
  shall not be embedded in the PLC program.
- **Experiment:** Follow the exact two-stage lesson workflow. In **Monitor →
  Response**, verify Simulation evidence, both servo identities, zero initial
  position, units, good quality, 10-millisecond timing, current limits, and
  abort/restoration state, then record the pole and response predictions. In
  **Test**, use **Observe without excitation**, disabled amplitude `0`, and an
  `8.1 s` maximum duration. The only Control Studio write is the existing
  Boolean project authority `P1.ExperimentEnabled` at `%MX96.0`. The project
  owns the simultaneous one-millimetre moves, the eight-second observation,
  safe zero-current behavior, and restoration to zero; Control Studio shall
  not inject another step. The native test owns the independent roots, complete
  responses, and requirement verdicts. Do not present unavailable shared Tune
  or Validate actions as lesson steps. Abort on nonfinite or bad-quality data,
  explicit safety abort, current-limit contact, or departure from the local
  position range.
- **Controlled comparison and measures:** Change only controller structure and
  its derived parameters; hold plant, initial state, setpoint event, sample
  schedule, duration, limits, and seed equal. Define percent overshoot as peak
  position above setpoint divided by the commanded position change, times 100.
  Define 2-percent settling time as the first time after which position remains
  within plus/minus 2 percent of the move. Define final-window absolute error
  only for a settled response. Define applied-current travel as the sum of
  absolute successive applied-current changes in percentage points. Retain
  peak requested and applied current, time at limit, pole identities, damping,
  stop reason, and exact windows. Report `Not settled` rather than a settling
  time when the condition is not met.
- **Validation:** The root-locus construction, direct polynomial roots, and
  executable response shall agree within declared tolerances. The proportional
  case shall exhibit approximately 16-percent overshoot and fail the 2-second
  settling requirement. The lead case shall have less than 5-percent
  overshoot, settle within 2 seconds, reach the final-error tolerance, and
  remain unsaturated. A faster response that violates current limits is
  rejected. Simulation evidence shall not authorise physical servo tuning.
- **Explanation:** Explain that gain alone cannot move the uncompensated pair
  left of real part `-1`; it trades damping for oscillation frequency along the
  vertical branches. The added zero and farther-left pole change the angle
  condition, pulling the compensated dominant branches into the required
  damping region while the third pole remains sufficiently faster to be
  non-dominant. Reconcile any difference between second-order predictions and
  the third-order executable response instead of hiding it.
- **Transfer and required conclusion:** Root locus makes pole motion and
  controller-structure effects visible, and proves when gain alone is
  insufficient. It remains a model-based pole-location method and does not
  expose frequency-dependent resonance, noise amplification, or gain/phase
  margins across the full spectrum. Lesson 47 therefore introduces Bode
  analysis on a flexible drive whose resonance is poorly exposed by one step.

#### Lesson 47 — Bode analysis: flexible motor drive

- **Objective:** Measure and explain the frequency response of one flexible-
  shaft drive, identify the resonant mode that a coarse step test can hide,
  calculate every gain crossover and its phase margin, and compare an
  unshaped loop with one explicitly unapplied resonance-shaped candidate.
- **Context and plant:** The real equipment is a motor driving a load through a
  flexible shaft. Applied motor torque is expressed as percent of rated torque;
  measured load-side speed is revolutions per minute. Three independent
  simulated copies use the same local flexible mode, 1-percent sinusoidal
  torque amplitude, operating point, 10-millisecond sample interval, limits,
  and seed. They run low-, resonant-, and high-frequency probes simultaneously
  so frequency is the only intentional difference.
- **Reason:** Lesson 46 located closed-loop poles and improved one commanded
  move, but pole locations and one transient do not directly show how strongly
  the plant amplifies disturbances at each frequency, how much phase it loses,
  or whether more than one gain crossover exists. A flexible shaft can look
  acceptable over a short or coarsely sampled move while storing damaging
  torsional energy near resonance.
- **Theory before practice:** Explain that sinusoidal steady state is the
  sustained response after the transient has decayed, angular frequency
  (omega) is radians per second, magnitude is output amplitude divided by input
  amplitude, and phase is output timing relative to input before using symbols
  or plots. Define logarithmic frequency axis, decade, magnitude ratio,
  decibel, phase lag, Bode magnitude plot, Bode phase plot, resonance,
  resonant frequency, bandwidth, open-loop transfer, gain crossover, phase
  crossover, gain margin, phase margin, delay margin, multiple crossover, and
  coherence. Derive `20*log10(magnitude)` from the amplitude-squared power
  relationship and explain why ratios `1`, `10`, and `0.1` are `0`, `20`, and
  `-20 dB`. Phase shall be unwrapped consistently. A margin is invalid or
  unavailable when its required crossing or data quality is absent.
- **Physical model and prediction:** Explain shaft stiffness, inertia, and
  viscous damping in words, start from `y'' + 0.2*y' + 4*y = u`, and take its
  zero-initial-condition Laplace transform to derive
  `G(s) = 1/(s^2 + 0.2*s + 4)`. Derive static gain by setting `s=0`, then
  compare the denominator with `s^2 + 2*zeta*omega_n*s + omega_n^2` to obtain
  `0.25 rpm/%`, `omega_n=2 rad/s`, and `zeta=0.05`. For a sinusoidal torque frequency (omega), explain real and imaginary
  denominator parts before writing
  `|G(j*omega)| = 1/sqrt((4-omega^2)^2 + (0.2*omega)^2)` and
  `phase = -atan2(0.2*omega, 4-omega^2)`. Predict magnitude/phase pairs of
  approximately `0.267 rpm/%` and `-1.5 degrees` at `0.5 rad/s`,
  `2.500 rpm/%` and `-90.0 degrees` at `2 rad/s`, and `0.0167 rpm/%` and
  `-178.5 degrees` at `8 rad/s` before running, showing each numerical
  substitution rather than only listing the answers.
- **Controller-candidate prediction:** Use the same low- and high-frequency
  controller gain of `5` for both candidates. The unshaped open loop is
  `L_base(s)=5*G(s)`. It has the same loop authority at zero frequency but a
  resonant magnitude of `12.5`, one high-frequency zero-decibel gain crossover
  near `2.99 rad/s`, and a phase margin near `6.9 degrees`. Derive these by
  setting magnitude to one, substituting `x=omega^2`, solving the resulting
  quadratic, evaluating phase with `atan2`, and subtracting its lag from 180
  degrees. The resonance-
  shaped controller is `C_shape(s)=5*(s^2+0.2*s+4)/(s^2+3.2*s+4)`, so its
  nominal open loop reduces to `L_shape(s)=5/(s^2+3.2*s+4)`, with one gain
  crossover near `1.44 rad/s` and phase margin near `112.6 degrees`; derive
  both values through the same explicit magnitude and phase steps. Explain
  that nominal pole-zero cancellation is model-dependent and shall be
  perturbed in validation; the candidate remains unapplied.
- **Controller under test:** The executable Structured Text shall generate the
  three bounded sinusoidal torque requests from one elapsed-time state and
  retain requested and applied torque separately. It shall not contain Bode
  results, fitted amplitudes, phases, coherence, margins, controller rankings,
  or precomputed acceptance. Control Studio owns frequency-response estimation
  and candidate calculation. The simulator shall use exactly the same
  coefficients and numerator gain as the equations.
- **Experiment:** Follow the exact two-stage lesson workflow. In **Monitor →
  Response**, verify Simulation evidence, plant identities, the 1000-rpm
  operating point, units, requested/applied torque, load-speed PVs, good
  quality, timing, limits, seed, and restoration, and record all three hand
  predictions. In **Test**, use **Observe without excitation**, disabled
  amplitude `0`, and maximum duration `120.1 s`. The only Control Studio write
  is the existing Boolean project authority `P1.ExperimentEnabled` at
  `%MX96.0`; the project owns all three probes, the 120-second window, and zero-
  torque restoration. Control Studio shall not inject another excitation.
  The native test owns the steady-state fits, coherence, every crossover,
  analytical comparisons, model perturbations, and verdicts. Do not present
  unavailable shared Identify, Tune, or Validate actions as lesson steps.
  Abort on nonfinite or bad-quality data, explicit safety abort, torque-limit
  contact, or departure from the local speed range.
- **Controlled comparison and measures:** Change only probe frequency across
  the three plant copies. Define input and output amplitude from the fitted
  sinusoidal fundamental, phase lag from the fitted input/output phase
  difference, magnitude in physical units and decibels, and coherence on the
  declared steady-state window. Declare the input/output timestamp reference.
  Because this simulator records a command at the end of the interval over
  which it was held, reference that command to the interval midpoint before
  comparing measured phase with the continuous-time model; retain both the raw
  sampled phase and corrected comparison phase. For controller candidates,
  retain every gain crossover, the worst valid phase margin, phase crossover
  and gain margin or explicit `Unavailable`, delay margin, resonance peak and
  frequency, bandwidth, minimum distance to minus one where available, and
  model revision. An analysis method shall retain every crossing it finds; it
  shall not select a favourable crossing while hiding another one.
- **Validation:** Native executable evidence shall reproduce the three
  predicted magnitude and phase values within declared amplitude and angular
  tolerances. The analytical oracle shall independently find the unshaped
  candidate's crossover near `2.99 rad/s`, the shaped candidate's crossover
  near `1.44 rad/s`, and both candidates' phase margins.
  Repeat candidate calculations with `-20 percent` shaft damping and
  `+10 percent` resonant frequency. Reject exact-cancellation claims that are
  not robust to those perturbations. Compare time responses only after the
  frequency criteria are declared; similar rise scale or final value shall not
  override materially different ringing and margins.
- **Explanation:** At `2 rad/s`, stiffness and inertia terms cancel in the real
  denominator, leaving only the small damping term and therefore large
  magnitude with 90-degree lag. Above resonance, the inertial term dominates
  and phase approaches minus 180 degrees. The shaped denominator adds damping
  near the flexible mode, but its apparent cancellation is only as accurate as
  the model. Explain every disagreement between analytical and measured
  response using resolution, windowing, transients, sampling, noise, or model
  error rather than adjusting results silently.
- **Transfer and required conclusion:** A Bode plot exposes resonance,
  bandwidth, phase loss, multiple crossings, and robustness trade-offs that a
  single transient can hide. Bode margins still rely on crossing conventions
  and do not by themselves count unstable closed-loop poles when open-loop
  instability, delay, or ambiguous encirclement is present. Lesson 48 therefore
  introduces the complete Nyquist contour on a delay-dominant thermal process.

#### Lesson 48 — Nyquist stability: delayed thermal process

- **Objective:** Use the complete Nyquist contour to predict and then verify
  stable and unstable proportional temperature loops on one conveyor curing
  oven, including a long-delay case whose wrapped Bode phase gives a dangerously
  misleading apparent margin.
- **Context and plant:** The real equipment is a continuous conveyor oven whose
  heater changes product temperature measured by a downstream infrared sensor.
  Heater command is percent of installed power and product temperature is
  degrees Celsius. Three independent simulated copies use the same local
  first-order thermal dynamics, 100-degree-Celsius operating point,
  50-percent holding command, proportional gain, one-degree-Celsius setpoint
  step, 100-millisecond sample interval, limits, and seed. Only transport delay
  changes: `5 s`, `25 s`, and `60 s`, representing successively greater sensor
  distance or lower conveyor speed. The comparison is Simulation evidence and
  cannot authorise a physical oven experiment.
- **Reason:** Lesson 47 showed how to find gain crossovers and phase margin, but
  a phase value displayed only between minus 180 and plus 180 degrees loses the
  number of complete rotations caused by long delay. At 60 seconds of delay,
  the wrapped phase at crossover appears near plus 2.3 degrees and a naive
  calculation appears to give a large positive margin even though the loop is
  unstable. A complete contour must retain the rotations and relate them to
  closed-loop pole count.
- **Theory before practice:** Explain a complex number as a real component and
  an imaginary component before plotting one. Define complex loop response as
  the open-loop response at one frequency, the complex plane, real axis,
  imaginary axis, positive-frequency path, negative-frequency conjugate path,
  closed contour, traversal direction, critical point at minus one,
  encirclement, clockwise and counter-clockwise sign conventions, winding
  number, open-loop right-half-plane pole count, and closed-loop right-half-
  plane pole count. Define a stable closed loop as having zero closed-loop
  right-half-plane poles. Explain in words that pure transport delay changes
  phase without changing magnitude and therefore continues rotating the
  contour as frequency increases.
- **Nyquist relationship:** Introduce the counter-clockwise encirclement count
  `(N_ccw)`, open-loop right-half-plane pole count `(P)`, and closed-loop
  right-half-plane pole count `(Z)` in that order. With the contour traversal
  and negative-feedback convention used by Control Studio, explain the
  relationship in words before writing `Z = P - N_ccw`. A negative
  counter-clockwise count represents clockwise encirclements. The inference is
  invalid if the contour crosses the minus-one point, the sweep omits a
  material part of the curve, a pole lies on the contour without an explicit
  indentation, or the response data are unqualified.
- **Physical model and prediction:** Define product-temperature deviation
  `(y)` as measured product temperature minus 100 degrees Celsius, applied
  heater-command deviation `(u)` as applied command minus 50 percent, the
  continuous frequency variable `(s)`, thermal time constant `(T)` in seconds,
  and transport delay `(L)` in seconds before presenting
  `G(s) = exp(-L*s)/(T*s + 1)`, with `T = 20 s`. Explain why the delay factor
  has unit magnitude and contributes phase `-L*omega`. Define proportional gain `(K)`
  as requested heater percentage points per degree Celsius of error, use
  `K = 2 %/degC`, and define the open-loop response
  `Loop(s) = K*G(s)`. Derive its magnitude, set that magnitude to one, square
  and rearrange it to obtain the common gain crossover
  `omega_gc = sqrt(K^2 - 1)/T = 0.08660 rad/s`. Explain unwrapped phase before
  writing `phase_gc = -atan(T*omega_gc) - L*omega_gc`.
- **Closed-loop equilibrium:** Recall that stable proportional control on this
  self-regulating plant does not imply zero setpoint error. Define the
  product-temperature setpoint `(SP)`, process value `(PV)`, and predicted
  equilibrium temperature `(PV_eq)` before writing
  `PV_eq = 100 degC + K*(SP - PV_eq)` for unit plant gain. With
  `SP = 101 degC` and `K = 2 %/degC`, collect the process-value terms to show
  `3*PV_eq=302`, then predict `PV_eq = 100.667 degC` and remaining error
  `0.333 degC`. Stability is therefore evaluated as decay
  toward `PV_eq`, not as convergence to the setpoint.
- **Worked predictions:** Before running, first show that the thermal phase is
  `-atan(sqrt(3))=-60 degrees`, calculate each delay phase in radians, and
  convert it to degrees. Then predict these independently checked results. The
  `5 s` case has unwrapped crossover phase `-84.81 degrees`, phase
  margin `95.19 degrees`, counter-clockwise encirclement count `0`, open-loop
  pole count `0`, inferred closed-loop pole count `0`, and is stable. The
  `25 s` case has unwrapped phase `-184.05 degrees`, phase margin
  `-4.05 degrees`, counter-clockwise count `-2`, inferred closed-loop pole count
  `2`, and is unstable. The `60 s` case has unwrapped phase
  `-357.72 degrees`; wrapping it gives `+2.28 degrees`, which would create the
  false apparent margin `182.28 degrees`. Its complete contour instead has
  counter-clockwise count `-2`, inferred closed-loop pole count `2`, and is
  unstable.
- **Controller under test:** The executable Structured Text shall implement
  three explicit positional proportional paths. Each calculates requested
  total heater command as the 50-percent holding command plus gain times
  setpoint minus measured temperature, then clamps that request into the
  applied 0-to-100-percent command. It shall retain requested and applied
  commands separately and independently abort a case to 50 percent if its
  temperature leaves the declared local range. It shall not contain Nyquist
  points, encirclement counts, margins, pole counts, stability labels, or
  precomputed results. Control Studio owns those calculations.
- **Experiment:** Follow the exact two-stage lesson workflow. In **Monitor →
  Response**, verify all three plant identities, the only intentional delay
  difference, SP, PV, requested/applied MV, units, quality, timing, operating
  point, limits, seed, aborts, restoration, and Simulation label, then record
  every hand prediction. In **Test**, use **Observe without excitation**,
  disabled amplitude `0`, and maximum duration `600.1 s`. The only Control
  Studio write is the existing Boolean project authority
  `P1.ExperimentEnabled` at `%MX96.0`. The project owns all three simultaneous
  setpoint changes, the 600-second observation, local aborts to the holding
  command, and restoration to 100 degrees; Control Studio shall not inject a
  second setpoint step. The native test owns complete contours, winding counts,
  pole-count inference, response envelopes, and verdicts. Do not present
  unavailable shared Identify, Tune, or Validate actions as lesson steps.
- **Controlled comparison and measures:** Change only transport delay. Retain
  the full positive- and negative-frequency contour, traversal direction,
  unwrapped phase, wrapped display phase, gain crossover, minimum complex-plane
  distance to minus one, counter-clockwise encirclement count, open-loop pole
  count, inferred closed-loop pole count, model and controller revision, sweep
  bounds and density, endpoint residual, and evidence class. In time-domain
  evidence, define equilibrium-deviation envelope as the maximum absolute
  difference between process value and the independently predicted
  `100.667 degC` proportional-control equilibrium in declared early and late
  windows. Use `100 s` through `200 s` for the early envelope and `500 s`
  through `600 s` for the late envelope. A decreasing envelope supports the
  stable prediction. A late-to-early envelope ratio greater than `1.20`, or a
  local-range abort, supports the time-domain manifestation of instability.
  The separate
  setpoint-error result shall retain the expected `0.333 degC` proportional
  offset. A bounded trace that is clipped by heater limits is not evidence of
  linear closed-loop stability.
- **Validation:** The numerical oracle shall support declared nonzero pure
  delay, retain the complete contour, report the sign convention, and infer
  `Z = P - N_ccw`. It shall reject nonfinite coefficients, invalid sweeps,
  a contour too close to minus one for the declared numerical tolerance, and
  insufficient endpoint closure. The `5 s`, `25 s`, and `60 s` contour results
  shall agree with the worked predictions. Executable response envelopes shall
  decay only for the `5 s` case and grow or reach a declared abort for both
  unstable cases. Reversing contour traversal in an independent oracle shall
  reverse `N_ccw` without changing inferred physical stability after the
  convention is transformed consistently.
- **Explanation:** Delay leaves open-loop magnitude and crossover frequency
  unchanged but rotates the response farther as frequency rises. The 60-second
  case loses almost one complete additional phase revolution, which a wrapped
  phase display hides. The Nyquist contour preserves that rotation; its two
  clockwise encirclements with zero open-loop right-half-plane poles imply two
  unstable closed-loop poles. Explain any difference between contour and time
  evidence using sweep resolution, endpoint closure, sample timing, saturation,
  abort, or model mismatch rather than changing the verdict silently.
- **Transfer and required conclusion:** Nyquist provides a complete stability
  count when delay, phase wrapping, open-loop unstable poles, or ambiguous
  crossings make a simple margin reading inadequate. Stability count alone
  still does not state how disturbances, measurement noise, control effort, or
  uncertainty propagate. Lesson 49 therefore introduces sensitivity and
  complementary sensitivity on a precision motor-speed loop.

#### Lesson 49 — Sensitivity and loop shaping: precision motor speed

- **Objective:** Compare an aggressive proportional speed loop with a shaped
  speed loop on the same precision motor and decide which one satisfies
  declared load-rejection, encoder-noise, robustness, speed-of-response, and
  drive-effort requirements. Explain why no single frequency region may be
  optimized without checking the others.
- **Context and plant:** The real equipment is a precision motor driving a load
  through a compliant coupling. Rated-torque command is the manipulated
  quantity and is measured in percent. Load-side speed is the controlled
  quantity and is measured in revolutions per minute. The lesson uses local
  deviations about a `1000 rpm` operating speed: a displayed speed deviation
  of `1 rpm` therefore means an actual speed of `1001 rpm`. A separately
  retained true-speed channel permits seeded encoder noise to be distinguished
  from physical shaft motion. Load torque is a second physical plant input,
  not an unexplained change to the measured speed.
  Two independent simulated copies shall use the same state-space plant,
  setpoint, `10 ms` sample interval, initial state, `0.1 percent` rated load-
  torque step at `60 s`, `120 s` duration, torque limits, identical `10 rad/s`
  deterministic encoder interference, seed, and abort range. Only the
  controller changes. This is Simulation evidence;
  a physical drive requires independently reviewed current, speed, vibration,
  coupling, guard, emergency-stop, and machine-motion limits.
- **Reason:** Lesson 48 proved whether a closed loop is stable, while Lesson 47
  calculated margins around crossover. Neither result says how a load-torque
  disturbance, encoder noise, model error, or controller action propagates at
  each frequency. The aggressive proportional candidate can be stable and
  still amplify motion and torque near the flexible mode. This measured gap is
  the reason to introduce sensitivity functions and loop shaping now.
- **Theory before practice:** First explain the open-loop response as the
  controller and plant response encountered around the feedback loop. Denote it
  only afterward as loop response `L`. The sensitivity function describes the
  fraction of an output disturbance or local model error that remains after
  feedback; denote it as `S` only after that physical meaning is clear. The
  complementary-sensitivity function describes setpoint transmission and the
  transmission of measurement noise into physical output; denote it as `T`.
  Explain that the two paths share the same feedback denominator before writing
  `S(s)=1/(1+L(s))`, `T(s)=L(s)/(1+L(s))`, and `S(s)+T(s)=1`; derive the last
  identity by adding the two fractions over their common denominator.
  The plant-input load-disturbance path is `G(s)S(s)`. The measurement-noise to
  requested-torque path is the control-sensitivity function `C(s)S(s)`, with a
  negative sign that changes direction but not plotted magnitude. Define every
  transfer magnitude as output amplitude divided by input amplitude at one
  angular frequency, with the corresponding physical units.
  Define closed-loop bandwidth as the first frequency at which `|T|` falls
  below `1/sqrt(2)` of its low-frequency reference, maximum sensitivity `M_s`
  as the largest qualified value of `|S|`, and maximum complementary
  sensitivity `M_t` as the largest qualified value of `|T|`. Define roll-off as
  the rate at which high-frequency magnitude falls. Explain in words that low
  sensitivity over one band must be recovered elsewhere for the stable,
  proper, minimum-phase loop used here; only then name this constrained
  redistribution the waterbed effect. Do not present that qualified result as
  universal for plants with unstable poles, nonminimum-phase zeros, delay, or
  nonproper controllers.
- **Plant and controller relationships:** Start from the scaled balance
  `y''+0.6*y'+y=10*(u-d)`, define speed deviation `y`, controller torque `u`,
  and opposing load `d`, and take its zero-initial-condition Laplace transform.
  With `d=0`, derive
  `G(s)=10/(s^2+0.6s+1) rpm/%`. The baseline positional proportional controller
  requests `0.4 percent` rated torque for each `1 rpm` of speed error:
  `C_base(s)=0.4 %/rpm`. The shaped candidate retains the same zero-frequency
  gain, reduces loop gain around the flexible mode, adds phase around crossover,
  and rolls controller response down above the useful band:
  `C_shape(s)=0.4*((1+s/0.3)/(1+s/3))*((1+s/0.8)/(1+s/0.08))*(1/(1+s/8))`.
  Explain the lead, lag, and low-pass factors separately, including their DC
  and high-frequency gains, before presenting the complete product. Derive the
  state realization of each `(1+s/z)/(1+s/p)` factor from
  `x'=-p*x+e` and `v=(p/z)*(e+(z-p)*x)`. The candidate is proper and shall be
  implemented with explicit filter state rather than an ideal derivative.
- **Worked prediction:** First derive `L(0)=4`, `S(0)=0.2`, `T(0)=0.8`, and
  `|G(0)S(0)|=2 rpm/%`. Explain the repeatable complex-grid calculation: at
  each frequency form `L`, `S`, `T`, `GS`, and `CS`; find maxima, unity-gain
  crossings, phase margins, and the first `|T(0)|/sqrt(2)` bandwidth crossing.
  On a common logarithmic grid from `0.001` through `100 rad/s`, predict for baseline versus shaped control respectively:
  low-frequency `|S|` at `0.01 rad/s` of approximately `0.200` versus `0.201`;
  maximum sensitivity `M_s` of `3.197` versus `1.724`; maximum complementary
  sensitivity `M_t` of `3.009` versus `1.334`; gain crossover of `2.187` versus
  `1.706 rad/s`; phase margin of `19.13` versus `44.11 degrees`; closed-loop
  bandwidth of `3.43` versus `2.83 rad/s`; high-frequency `|T|` at `10 rad/s`
  of `0.0420` versus `0.0247`; and high-frequency `|CS|` of `0.4168` versus
  `0.2449 %/rpm`. At `0.01 rad/s`, predict similar load-disturbance magnitude
  `|GS|` of `2.000` versus `2.013 rpm/%`. These predictions establish before
  the run that shaping should reduce resonant and noise amplification while
  retaining nearly the same slow-load authority, at the cost of bandwidth and
  some transient disturbance recovery. Derive the time-domain reference
  equilibria too: `y=4*(1-y)` gives `0.8 rpm` before the load, while
  `y=4*(1-y)-1` gives `0.6 rpm` after the `0.1 percent` opposing load.
- **Controller under test:** The Structured Text shall expose two explicit
  paths. The baseline path calculates positional proportional torque. The
  shaped path shall implement the declared lead, lag, and low-pass states at
  the actual cycle interval. Both paths shall retain speed setpoint, noisy
  measured speed, independently observed true speed, speed error, requested
  torque, applied torque, load torque, abort state, and actual cycle interval.
  Torque requests shall be clamped symmetrically to the declared drive range;
  requested and applied values shall remain distinct. Structured Text shall
  contain no sensitivity curve, frequency metric, candidate ranking, or
  accepted answer.
- **Prediction and experiment:** Follow the exact two-stage lesson workflow.
  Before starting, calculate the low-frequency result, equilibria, crossover
  trade-off, and expected encoder-noise torque direction. In **Monitor →
  Response**, verify identities, units, true and measured speed, requested and
  applied torque, load torque, quality, timing, limits, seed, abort, restoration,
  and Simulation status. In **Test**, use **Observe without excitation**,
  disabled amplitude `0`, and maximum duration `120.1 s`. The only Control
  Studio write is `P1.ExperimentEnabled` at `%MX96.0`; the project owns the
  one-rpm request, both load events at 60 seconds, encoder interference, aborts,
  and restoration. The native test owns frequency-grid calculations, time
  metrics, and verdicts. Do not present unavailable shared Identify, Tune, or
  Validate actions as lesson steps.
- **Comparison measures:** Change only the controller. Retain the common
  frequency grid and complex `L`, `S`, `T`, `GS`, and `CS` values, complementarity
  residual, every crossover, bandwidth rule, `M_s`, `M_t`, phase margin, model
  and controller revision, and evidence class. In time data, define setpoint
  overshoot relative to each candidate's predicted pre-load equilibrium,
  two-percent settling time, maximum true-speed departure after the load event,
  final-window equilibrium error, requested- and applied-torque total variation
  in percentage points, saturation duration, and abort state. Encoder-noise
  movement shall use a declared quiet final window and shall not be confused
  with physical true-speed movement.
- **Validation:** Reject nonfinite values, unequal or unordered frequency grids,
  an `S+T` residual above `1e-9`, missing low- or high-frequency coverage,
  ambiguous crossover selection, actuator saturation, bad or stale quality,
  timing outside tolerance, departure from the local speed region, or a run
  whose settling prerequisite is not met. Report the ineligible metric as a
  state rather than a number. The shaped candidate shall retain low-frequency
  `|S|` and `|GS|` within `2 percent` of baseline, reduce `M_s` below `1.8`,
  reduce `M_t` below `1.4`, provide at least `40 degrees` phase margin, reduce
  both `|T|` and `|CS|` at `10 rad/s` by at least `35 percent`, and keep
  bandwidth between `2.6` and `3.0 rad/s`. Native execution shall confirm less
  overshoot and less requested-torque movement while also exposing, rather
  than hiding, any slower load recovery.
- **Explanation:** Reconcile the curves and traces. High loop gain makes `S`
  small at low frequency and rejects slow load changes, but `S+T=1` prevents
  both paths from being small at the same frequency. The baseline crosses near
  the flexible mode with little phase reserve, producing large `M_s`, `M_t`,
  ringing, and controller activity. The shaped controller redistributes gain,
  adds crossover phase, and rolls off high-frequency control action. Its lower
  bandwidth is a deliberate cost, not a hidden defect. Differences between
  predicted and measured results shall be explained using discrete filter
  realization, sample timing, finite windows, seeded noise, limits, or model
  mismatch rather than silently changing tolerances.
- **Transfer and required conclusion:** Disturbance rejection, tracking,
  measurement-noise attenuation, robustness, response speed, and actuator
  effort are coupled design choices. A stable loop with acceptable classical
  margins may still fail sensitivity or effort requirements, and the lowest
  sensitivity peak is not automatically the best controller. Lesson 50 next
  asks whether the selected continuous design remains stable when its sampled
  implementation is examined in the z-plane.

#### Lesson 50 — Discrete-time stability and the z-plane: positioning system

- **Objective:** Predict and verify why the same continuous position-controller
  gains are stable at a `10 ms` sample interval but unstable at a `100 ms`
  interval with one-cycle computation delay, then select a corrected `100 ms`
  discrete controller whose poles, motion, and force satisfy declared
  requirements.
- **Context and plant:** The real equipment is an industrial positioning table.
  Motor-force command is the manipulated quantity and is measured in percent
  of rated force. Carriage position is the controlled quantity and is measured
  in millimetres; carriage velocity is measured in millimetres per second.
  Three independent simulated tables use the same continuous mechanics,
  one-millimetre position request, initial state, `10 ms` plant integration,
  force limit, local position range, and deterministic seed. The fine case
  updates its controller every `10 ms`. The coarse baseline and corrected case
  update every `100 ms`; each holds its output between updates and applies the
  request calculated at the preceding update, representing one complete sample
  of acquisition, computation, communication, and actuation delay. This is
  Simulation evidence, not physical-axis evidence.
- **Reason:** Lesson 49 selected a continuous loop shape by balancing
  sensitivity, noise, robustness, bandwidth, and effort. The controller will
  execute as sampled code, however. A continuous pole or margin does not prove
  the sampled loop when zero-order hold, finite sample interval, computation
  delay, and discrete controller realization are present. This implementation
  gap is why the z-plane is required now.
- **Theory before practice:** Explain a sample as one measured value taken at a
  declared instant and a held output as a command kept constant until the next
  controller update. A discrete sequence records values by sample index `k`.
  The z-transform represents how such sequences shift and evolve; introduce
  the discrete complex variable `z` only after that meaning. One-sample delay
  means that a requested value is used at the next index and is written
  `z^-1`.
  A discrete pole is a natural response multiplier per sample. Its magnitude
  is distance from the origin in the z-plane. The unit circle contains points
  of magnitude one: poles strictly inside decay, poles on it do not
  asymptotically decay, and poles outside grow. Pole angle describes sampled
  oscillation. Introduce the exact pole mapping `z=exp(s*T_s)` only after the
  continuous pole `s` and sample interval `T_s` are defined. Explain that this
  mapping places a stable isolated continuous pole inside the unit circle, but
  does not justify mapping a completed continuous closed loop while ignoring
  hold and computation delay.
  Explain the zero-order hold as the actuator holding one command between
  samples. Exact hold discretization can create a sampling zero even when the
  continuous plant has no finite zero. Define the Nyquist angular frequency as
  the highest sinusoidal frequency uniquely representable at a sample interval
  before writing `omega_N=pi/T_s`; higher content aliases to a false lower
  frequency. Explain a discrete root locus as the path of closed-loop z-plane
  poles as a declared digital gain changes.
- **Continuous plant and exact sampled model:** The local table mechanics use
  position `x`, velocity `v`, and force command `u`:
  `dx/dt=v` and `dv/dt=-2v+10u`. Take the zero-initial-condition Laplace
  transform of each equation and combine `X=V/s` with `V/U=10/(s+2)` to derive
  `G(s)=10/(s(s+2)) mm/%`. For a sample interval `T_s`, solve the constant-input
  velocity equation exactly to obtain
  `v[k+1]=a*v[k]+5*(1-a)*u[k]`, where `a=exp(-2T_s)`, then integrate velocity
  over the interval to obtain the position update. Only then present the exact zero-order-hold matrices
  `A_d=[[1,(1-a)/2],[0,a]]` and
  `B_d=[[5T_s-2.5(1-a)],[5(1-a)]]`.
  Derive the exact sampled plant denominator from the two eigenvalues `1` and
  `a` as `(z-1)*(z-a)=z^2-(1+a)z+a`. Its numerator is
  `b_p*z + ((1-a)/2)*b_v - a*b_p`, where `b_p` and `b_v` are the position and
  velocity entries of `B_d`. Predict sampling zeros near `-0.9934` at `10 ms`
  and `-0.9355` at `100 ms`; qualify both as discretization results rather than
  physical flexible modes.
- **Controller and delay model:** The positional proportional-derivative
  controller requests force from position error and measured velocity. The
  continuous-design gains are `K_p=4 %/mm` and `K_d=0.4 %/(mm/s)`. Requested
  force is `u_request=K_p(SP-x)-K_d*v`. Each digital case applies the preceding
  request, so augment the sampled plant with held-force state `u_hold`. The
  closed-loop state is `[x,v,u_hold]`. Explain how the plant rows consume held
  force and how the last row computes the next held force before presenting its
  exact transition matrix `[[A_d,B_d],[-K_p,-K_d,0]]`.
  At `10 ms`, predict poles `0.03984` and
  `0.97018 +/- j0.05688`, whose complex magnitude is `0.97184`. At `100 ms`,
  the same gains predict poles `0.17276` and
  `0.82299 +/- j0.63790`, whose complex magnitude is `1.04126`; the coarse
  implementation is unstable even though the continuous gains did not change.
  The corrected `100 ms` design uses `K_p=1 %/mm` and
  `K_d=0.4 %/(mm/s)`, predicting poles `0.74361` and
  `0.53756 +/- j0.37370`, with complex magnitude `0.65469`. For each complex
  pole show that magnitude is `sqrt(real^2+imaginary^2)` and compare it with
  the unit-circle boundary rather than merely attaching a verdict.
- **Sampling and alias predictions:** Predict Nyquist angular frequencies of
  `pi/0.01=314.16 rad/s` at `10 ms` and `pi/0.1=31.416 rad/s` at `100 ms`.
  Derive the coarse sampling frequency `2*pi/0.1=62.832 rad/s`; a deliberately
  analysis-only `40 rad/s` encoder component is representable by the fine
  sample but aliases to `abs(40-62.832)=22.832 rad/s` at the coarse sample. The executable
  motion run shall not inject that component into the physical control path;
  it is a bounded pre-run sampling demonstration.
- **Controller under test:** The Structured Text shall execute all three
  explicit cases in one `10 ms` task. Counters shall trigger the fine update
  every cycle and both coarse updates every ten cycles. At each trigger, the
  prior pending request becomes applied force, a new pending request is
  calculated, and the plant receives the held applied force until the next
  trigger. Retain position setpoint, measured position and velocity, requested,
  pending, and applied force, sample counter, effective interval, saturation,
  and abort separately for every case. Structured Text shall not contain pole,
  zero, unit-circle, stability, settling, or accepted-candidate answers.
- **Prediction and experiment:** Follow the exact two-stage lesson workflow.
  In **Monitor → Response**, verify all three table identities, units, initial
  state, position and velocity, requested/pending/applied force, actual plant
  and controller intervals, limits, controlled-stop state, seed, quality,
  restoration, and Simulation label, then record the pole, zero, Nyquist, and
  alias predictions. In **Test**, use **Observe without excitation**, disabled
  amplitude `0`, and maximum duration `10.1 s`. The only Control Studio write
  is `P1.ExperimentEnabled` at `%MX96.0`; the project owns all three moves,
  sample/hold timing, delays, controlled stops, and restoration. The native
  test owns exact matrices, roots, aliasing, time metrics, and verdicts. Do not
  present unavailable shared Identify, Tune, or Validate actions as lesson
  steps.
- **Comparison measures:** First change only controller update interval between
  the fine and coarse baseline. Then hold the `100 ms` interval and one-sample
  delay constant while changing only `K_p` from `4` to `1`; `K_d` remains
  `0.4`. Retain continuous model, exact discrete matrices, sampling zero,
  augmented poles and magnitudes, unit-circle clearance, Nyquist frequency,
  alias calculation, sample/hold/delay convention, model and controller
  revisions, and evidence class. Time measures shall define overshoot, two-
  percent settling, final-window position error, requested- and applied-force
  total variation in percentage points, saturation duration, abort trigger,
  controlled-stop completion, and timing tolerance before reporting results.
- **Validation:** Independent analytical oracles and Control Studio shall agree
  on every declared pole within `1e-6`, pole magnitude within `1e-6`, sampling
  zero within `1e-4`, and alias frequency within `0.001 rad/s`. Reject a result
  for nonfinite or inconsistent matrices, an interval or delay different from
  the executed case, incomplete root-locus gains, bad or stale data, timing
  outside tolerance, force saturation where linear metrics are claimed, or a
  missing controlled stop. The fine and corrected cases shall remain inside
  the unit circle, avoid saturation and local-range abort, settle within their
  declared ten-second record, and end within `0.02 mm` of setpoint. The coarse
  baseline shall be classified unstable from its pole pair before its bounded
  motion trace reaches the abort; an attractive early sample shall not override
  the pole evidence.
- **Explanation:** Reconcile the three cases using the actual implementation.
  Increasing the interval lets more mechanical motion occur before correction,
  zero-order hold delays changes between updates, and the explicit one-sample
  pipeline contributes another discrete state. Those effects move the coarse
  baseline pole pair outside the unit circle. Reducing proportional gain at the
  same coarse interval brings the corrected poles inside but slows response.
  The sampling zero and alias calculation are consequences of observing and
  holding a continuous system, not newly discovered physical modes.
- **Transfer and required conclusion:** Digital stability must be proved for
  the sample, hold, delay, numerical realization, limits, and timing that
  actually execute. A continuous design, left-half-plane pole plot, or short
  calm trace is insufficient. Lesson 51 next compares tuning methods only after
  time-, frequency-, and discrete-domain qualification are available.

### 3.8 Tuning qualification and automatic tuning

#### Lesson 51 — Tuning-method comparison: heater, tank, and dryer

- **Objective:** Given the already accepted models of a recirculating water
  heater, an integrating buffer tank, and a continuous belt dryer, determine
  which shipped PI tuning methods are eligible, validate every resulting
  candidate under equivalent conditions, and decide whether one candidate,
  several trade-off candidates, or no candidate satisfies the declared
  engineering requirements. The learner shall be able to explain why a
  formula returning finite gains is not itself evidence that the tuning is
  acceptable.
- **Context:** Reuse the three Simulation-only processes and their operating
  regions from Lesson 44. Heater power controls water temperature; inlet-valve
  position controls buffer-tank level against outlet flow; dryer heater command
  controls product dryness against moisture load. Every actuator is one-sided
  and limited from `0` through `100 percent`. The controller interval is
  `0.1 s`. Each compared controller shall drive an independent copy of the same
  local plant so that one candidate cannot alter another candidate's initial
  condition. The lesson shall retain model identity and revision, controller
  form and convention, signals and units, initial state, sample times, limits,
  noise seed, events, requested output, applied output, and evidence class.
- **Reason:** Lesson 44 showed how a learner can choose Fast, Balanced, or
  Robust response targets within one SIMC method. It did not answer what to do
  when another qualified method returns different gains for the same accepted
  model. Lessons 45–50 supplied time-, frequency-, sensitivity-, and
  discrete-domain evidence needed to judge those candidates. This lesson is
  therefore about comparing methods, not repeating model identification or
  selecting a SIMC response label.
- **Theory before practice — eligibility:** Define method eligibility as the
  set of plant-model assumptions under which a published rule has been
  qualified. A candidate is **Unavailable** when its method is outside that
  domain; it is not a failed controller and shall not be fabricated. SIMC PI is
  eligible for an accepted stable, monotone, positive-gain first-order or
  first-order-plus-dead-time model and, with a resolved positive effective
  delay, for an integrating rate model. AMIGO PI is eligible only for an
  accepted stable, monotone, positive-gain first-order-plus-dead-time model
  with positive dead time and delay ratio `0 < L/T <= 1`; it is unavailable
  for an integrating model. Model error greater than `0.25`, nonfinite or
  nonpositive required parameters, unsupported structure, stale or
  insufficient data, or a method-domain failure shall prevent candidate
  creation with a deterministic reason.
- **Theory before practice — candidate form:** Both methods create an ordinary
  parallel `Control.PID` PI candidate, not a different runtime controller.
  Define proportional gain (Kp), integral time (Ti), and parallel integral gain
  (Ki), with equipment-specific units, before writing `Ki = Kp / Ti`.
  Derivative gain is zero. State that SIMC's response target changes the desired
  closed-loop time (lambda), while published AMIGO PI has no adjustable
  response-speed parameter; a response preference may affect a later
  comparison but shall not silently change the AMIGO coefficients. Every
  candidate remains unapplied.
- **Tuning equations and derivation:** For stable FOPDT SIMC, state and explain
  `Kp=T/[K*(lambda+L)]`, `Ti=min(T,4*(lambda+L))`, and `Ki=Kp/Ti`. For an
  integrating rate model, use `Kp=1/[Kprime*(lambda+L)]` and
  `Ti=4*(lambda+L)`. For eligible FOPDT AMIGO PI, state and explain
  `Kp=(1/K)*(0.15+(0.35-L*T/(L+T)^2)*(T/L))` and
  `Ti=0.35*L+13*L*T^2/(T^2+12*L*T+7*L^2)`, followed by `Ki=Kp/Ti`.
  Explain why the divisions by finite positive gain and delay create method-
  domain requirements. Substitute every heater, tank, and dryer parameter and
  show the arithmetic that produces the declared gains; do not list formula
  outputs without their derivation.
- **Theory before practice — error measures:** Explain control error as the
  difference between setpoint and process value before introducing error (e).
  Integral absolute error (IAE) accumulates the magnitude of error and retains
  process-unit seconds. Integral squared error (ISE) penalizes large errors
  more strongly and retains squared-process-unit seconds. Integral
  time-weighted absolute error (ITAE) penalizes errors that persist late in the
  run and retains process-unit seconds squared. For samples separated by
  interval `Delta t`, introduce the equations only after those meanings:
  `IAE = sum(|e[k]| Delta t)`, `ISE = sum(e[k]^2 Delta t)`, and
  `ITAE = sum(t[k] |e[k]| Delta t)`. These measures shall use the same declared
  event window and sample schedule for every eligible candidate.
- **Theory before practice — transient and actuator measures:** Define
  overshoot relative to the commanded process-value change and define settling
  as entering and remaining in a declared band for a declared dwell. A run that
  fails the settling condition is `Not settled`; it receives no steady-state-
  error value. Define cumulative applied-output movement as total variation,
  `MV_travel = sum(|MV_applied[k] - MV_applied[k-1]|)`, in percentage points.
  Also retain peak applied output, maximum applied-output rate, saturation
  duration, and noise-driven applied-output root-mean-square activity. Requested
  and applied output shall remain separate throughout.
- **Theory before practice — robustness measures:** Define gain margin as the
  additional loop-gain factor at the minus-180-degree phase crossing, phase
  margin as the remaining phase before minus 180 degrees at the unity-gain
  crossing, and delay margin as the additional pure delay predicted to consume
  the phase margin at crossover. Define the sensitivity function as the
  closed-loop response from output disturbance to error before introducing
  sensitivity (S), and define complementary sensitivity as the path from
  setpoint or measurement noise to process output before introducing
  complementary sensitivity (T). Only then write
  `S = 1 / (1 + L_loop)` and `T = L_loop / (1 + L_loop)`, where loop transfer
  (L_loop) is controller times plant. Maximum sensitivity (Ms) and maximum
  complementary sensitivity (Mt) are the largest magnitudes over the declared
  frequency grid. State the grid, interpolation convention, and numerical
  tolerance before comparing them.
- **Theory before practice — decisions:** Eligibility is checked first,
  mandatory feasibility requirements second, and preference only among
  feasible candidates. Define Pareto dominance in words: one candidate
  dominates another only when it is no worse on every declared preference
  measure and better on at least one. A Pareto frontier may contain several
  candidates because response speed, robustness, and actuator activity conflict.
  A single weighted score may be shown only with its inputs, normalization,
  weights, objective, and tie rule visible; it shall not replace the raw
  measures or mandatory requirements. If no eligible candidate passes, the
  answer is **No acceptable candidate**. If several feasible candidates remain
  without a declared preference that separates them, the answer is
  **Inconclusive**, not an invented winner.
- **Prediction:** The accepted heater model has plant gain `K = 0.20 degC/%`,
  time constant `T = 5.0 s`, and dead time `L = 1.0 s`. Balanced SIMC predicts
  `Kp = 5.769 %/degC`, `Ti = 5.000 s`, and
  `Ki = 1.154 %/(degC*s)`. AMIGO predicts `Kp = 6.028 %/degC`,
  `Ti = 3.883 s`, and `Ki = 1.552 %/(degC*s)`. Predict that AMIGO will reject
  heater error sooner but use more heater movement and have smaller margins.
  The tank model has integrating rate gain
  `Kprime = 0.05 level-%/(valve-%*s)` and effective delay `L = 0.5 s`.
  Balanced SIMC predicts `Kp = 13.333 valve-%/level-%`, `Ti = 6.000 s`, and
  `Ki = 2.222 valve-%/(level-%*s)`; predict the AMIGO attempt will be
  `Unavailable` because no finite steady plant gain or time constant exists.
  The dryer model has `K = 0.40 dryness-%/heater-%`, `T = 20.0 s`, and
  `L = 8.0 s`. Balanced SIMC predicts `Kp = 2.083 heater-%/dryness-%`,
  `Ti = 20.000 s`, and `Ki = 0.104 heater-%/(dryness-%*s)`. AMIGO predicts
  `Kp = 1.287 heater-%/dryness-%`, `Ti = 17.829 s`, and
  `Ki = 0.072 heater-%/(dryness-%*s)`. Predict that AMIGO will trade slower
  dryer recovery for less actuator movement and larger robustness margins.
- **Controller under test:** Show the short actual Structured Text that selects
  one reviewed method's `Kp` and `Ki`, configures the same `Control.PID` PI
  form, initializes it at the physical operating point, passes the real
  `0.1 s` cycle interval to `Update`, and retains requested and applied outputs.
  The Structured Text shall contain no performance result, method score,
  accepted label, Pareto result, or hidden winner. An ineligible method shall
  not be approximated by another gain set.
- **Experiment:** Follow the exact two-stage lesson workflow. In Control Studio **Monitor → Response**, verify Simulation evidence,
  plant and model revisions, model class and units, signal quality, operating
  point, sample interval, limits, seed, restoration, and candidate state. In
  **Test**, use **Observe without excitation**, disabled amplitude `0`, and
  maximum duration `220.1 s`; review the project-owned comparison plan, confirm
  that its Boolean experiment authority `P1.ExperimentEnabled` at `%MX96.0` is
  disabled, and start it once. Control Studio shall enable that
  authority, observe the synchronized heater, tank, and dryer cases, and restore
  it to disabled on completion, abort, timeout, or failure; it shall not issue a
  second setpoint step. The native lesson test owns each method's eligibility,
  formula oracle, common setpoint/load/noise/model-variation plan, margins,
  sensitivity measures, Pareto results, and verdicts. Do not present unavailable
  shared Tune or Validate actions as lesson steps. The project starts heater,
  tank, and dryer at `20 degC`, `50 percent`, and `40 percent`; at `10 s`, set
  them to `25 degC`, `52 percent`, and `45 percent`; at `100 s`, apply the same
  declared heat-loss, outlet-flow, and moisture-load events; continue through
  `220 s`. Restore and reinitialize before each candidate. Do not use **Apply**.
- **Controlled comparison:** Change only the method and its resulting `Kp` and
  `Ki`. Keep plant model and revision, controller form and convention, bias,
  setpoint weights, anti-windup, limits, rate limits, tracking time, initial
  state, event schedule, sample interval, duration, noise generator and seed,
  metric windows, frequency grid, uncertainty cases, and abort criteria equal.
  For the tank, retain the AMIGO availability reason but execute no fictional
  AMIGO run. Results shall show raw metrics with sensible engineering precision,
  status and failed requirement before any Pareto or objective presentation.
- **Common qualification windows:** Setpoint tracking uses `10..100 s` and is
  settled only after remaining within `5 percent` of the commanded process-value
  change for `5 continuous seconds`. Disturbance rejection uses `100..220 s`,
  with final mean absolute error over `210..220 s`. Measurement-noise activity
  is the root-mean-square deviation of applied output from its mean over
  `210..220 s`; it shall not exceed `0.5 percent RMS` for the declared
  `0.02`-process-unit, `0.5 Hz` interference. Peak applied output shall not
  exceed `100 percent`, maximum applied-output rate shall not exceed
  `1000 percent/s`, and saturation duration at either output limit shall be
  zero. Retain a separate `Pass`, `Fail`, or `Inconclusive` disposition for
  setpoint tracking, disturbance rejection, measurement noise, and bounded
  model variation.
- **Validation:** Reject or mark inconclusive any candidate with invalid data,
  mismatched conditions, nonfinite response, output outside `0..100 percent`,
  unhandled saturation, a missing event, model revision mismatch, unstable
  bounded variation, gain margin below `6 dB`, phase margin below `30 degrees`,
  or an uncomputed declared sensitivity measure. Use independent formula
  oracles for every gain within relative tolerance `1e-7`; compare frequency
  results with the qualified numerical analyzer and declared plot tolerance.
  Validate setpoint tracking, disturbance rejection, noise response, actuator
  use, and bounded gain/delay variation separately. A candidate that is faster
  but fails a mandatory limit remains rejected. Requirements shall be stated
  in the plant's physical units: the heater and dryer use no-more-than-10-
  percent normalized overshoot, while the integrating tank uses no more than
  `0.8` level percentage points of peak excursion above its `52 percent`
  setpoint rather than inheriting an unrelated normalized thermal requirement.
  Retain the Pareto frontier and
  an explicit `Inconclusive` or `No acceptable candidate` outcome when
  applicable. All candidates remain unapplied Simulation evidence.
- **Explanation:** Reconcile the heater and dryer differences with their gain,
  integral time, delay ratio, error measures, actuator movement, margins, and
  sensitivity peaks. Explain why AMIGO is unavailable rather than inferior for
  the integrating tank. Explain any apparent conflict between a time-domain
  benefit and a frequency- or actuator-domain cost instead of collapsing it
  into a universal method ranking.
- **Transfer and required conclusion:** A tuning method is chosen in three
  distinct steps: prove its model-domain eligibility, prove the resulting
  candidate satisfies mandatory requirements under one common validation plan,
  and only then apply an explicit engineering preference among remaining
  trade-offs. No method is universally best, a finite formula result is not an
  acceptance certificate, and the correct conclusion may be several Pareto
  candidates, `Inconclusive`, or `No acceptable candidate`. Lesson 52 may
  automate this sequence only after every intermediate decision remains
  inspectable and no gain is silently applied.

#### Lesson 52 — Complete automatic tuning: recirculating water heater

- **Objective:** Run and audit a complete Automatic Tune sequence for the
  recirculating water heater, explain why each stage passed or stopped, recover
  safely from every taught failure, and export one validated but unapplied
  controller recommendation. The learner shall distinguish exact Simulation
  configuration evidence from an identified model and shall know which
  additional approvals and experiments a recorded or physical plant requires.
- **Context:** Reuse the accepted local heater model from Lessons 44 and 51:
  heater power is the one-sided manipulated variable from `0` through
  `100 percent`; water temperature is the process value in degrees Celsius;
  plant gain is `0.20 degC/%`, time constant is `5.0 s`, dead time is `1.0 s`,
  the operating point is `20 degC` at `20 percent` power, and the controller
  interval is `0.1 s`. The running demonstration controller is the reviewed
  Robust SIMC PI with `Kp = 3.261 %/degC` and
  `Ki = 0.652 %/(degC*s)`. A second independent simulated copy may execute an
  unapplied recommendation for validation; it shall not replace the running
  copy. Retain project, loop, plant, model and controller identities and
  revisions throughout.
- **Reason:** Lesson 51 separated method eligibility, candidate feasibility,
  and engineering preference. The learner can now judge every decision that
  automation will sequence. Repeating those decisions manually is slow and
  error-prone, but automation becomes unsafe when it hides reused evidence,
  silently excites a plant, weakens requirements, loses failure evidence, or
  applies its own recommendation.
- **Theory before practice — sequence and ownership:** Automatic Tune is an
  orchestrator over the existing **Test**, **Identify**, **Tune**, and
  **Validate** functions. It shall not contain private copies of their plant
  excitation, identification, formula, numerical, validation, apply, or
  rollback algorithms. Define a stage as one bounded engineering decision with
  declared inputs, outputs, pass condition, failure reason, and recovery. A
  stage may reuse accepted evidence only when its identity, revision, source,
  units, operating region, and freshness satisfy the next stage's contract.
- **Theory before practice — plan and approval:** Before execution, present one
  concise immutable plan containing evidence source and reuse decision; any
  proposed excitation and its affected signal, magnitude, duration, limits,
  stop conditions and restoration; model structures; controller structures;
  tuning methods and response objective; validation scenarios and mandatory
  requirements; and the fact that the outcome is export-only until a separate
  Apply review. Simulation requires confirmation of the complete bounded plan.
  A physical plant requires independent topology and hazard review plus
  explicit approval immediately before each physical action; one approval may
  not authorize a changed, enlarged, repeated, or later experiment.
- **Theory before practice — data qualification and Test:** Explain data
  qualification as deciding whether signals are trustworthy and informative
  enough for the intended model. Define signal quality, freshness, units,
  monotonic sample time, missing samples, forced values, saturation, operating
  range, excitation amplitude, signal-to-noise ratio, and restoration before
  the learner sees a qualification verdict. An exact saved simulated model may
  be reused as **Exact Simulation model** configuration evidence when its
  revision and parameters are valid; no excitation is then performed, and the
  stage shall say so. This shortcut shall never be relabelled as an identified
  physical model. Recorded or physical data require the bounded Test plan and
  retain its actual command, samples, stop reason, abort state, and restoration.
- **Theory before practice — Identify:** Explain that model structure states
  which dynamic behavior is represented, parameters give its numerical scale,
  fitting uses one selected data interval, and independent validation tests
  prediction on data not used for fitting. For the exact simulation route,
  Identify verifies the saved FOPDT structure, finite positive gain and time
  constant, nonnegative delay, revision, units, and operating region; it does
  not claim to estimate known parameters. For an observed route, compare
  eligible structures, select the simplest acceptable model, retain residuals
  and uncertainty, and reject rather than tune an inadequate model.
- **Theory before practice — Tune:** Reuse Lesson 51's eligibility rules and
  common candidate convention. Evaluate SIMC, AMIGO, and the already qualified
  bounded P/PI/PD/PID comparison neighborhood only where applicable. Retain
  every `Feasible`, `Rejected`, or `Unavailable` attempt, raw metrics, visible
  objective and scoring inputs, tie rule, and unavailability or failed-
  requirement reason. A score improvement below the Control Studio
  `10-percent` material-benefit threshold uses the visible deterministic method
  order rather than claiming false numerical superiority. A tuning-stage
  failure does not cause automatic formula substitution or requirement changes.
  Define material improvement explicitly as
  `100*(current_score-new_score)/current_score`, explain its normalization, and
  show why a `7.4 percent` improvement remains inside the declared `10 percent`
  tie band.
- **Theory before practice — Validate:** Compare the running controller and
  every candidate against one immutable validation-conditions record. Define
  setpoint, load, measurement-noise and bounded-model-variation scenarios;
  initial state; exact sample times; duration; disturbance trajectory; seed;
  output and rate limits; time measures; IAE, ISE and ITAE; applied-output
  movement in percentage points; margins; and evidence class before running.
  Each mandatory requirement reports `Pass`, `Fail`, or `Inconclusive` with
  value, limit, unit, source and reason. Missing or mismatched evidence is
  `Inconclusive`; it never passes. For this lesson, the default model-predicted
  noise-response limit is the larger of `120 percent` of the running Robust
  controller's noise-driven heater movement and `0.1 percent` of the
  `0..100 percent` heater-output span. Retain the chosen numeric limit before
  validation; do not change it after seeing a candidate result.
- **Theory before practice — failure, abort and restoration:** A stage failure
  stops the sequence at that stage, preserves earlier valid evidence, names the
  nearest recovery, and does not run later stages. An abort is a requested or
  automatic early stop during an actual experiment. Restoration is the
  separately observed return to the reviewed setpoint, mode and safe applied
  output; issuing a restoration request is not proof that restoration
  completed. Restoration failure is terminal and shall remain visible. Invalid
  configuration, unsafe requested excitation, poor data, identification
  rejection, no applicable tuning, no acceptable controller, abort, and
  restoration failure shall have deterministic distinct outcomes.
- **Theory before practice — recommendation, Apply and rollback:** A completed
  sequence creates an unapplied candidate. Recommendation means that the
  candidate passed the declared comparison; it is not permission to change a
  runtime. Export retains all evidence and units. Apply is a separate explicit
  review and authorization with current and candidate parameters, expected
  controller revision, warnings and rollback. Confirmed read-back after an
  atomic scan-boundary write is required before calling a controller applied.
  Rollback restores the preceding confirmed revision; neither Apply nor
  rollback is performed in this Simulation lesson.
- **Prediction:** The exact saved heater model is valid FOPDT configuration
  evidence, so the plan shall state that no excitation is required. With the
  Balanced objective, predict that Test will qualify the saved plant, Identify
  will accept `K = 0.20 degC/%`, `T = 5.0 s`, and `L = 1.0 s` without fitting,
  Tune will retain SIMC, AMIGO and bounded structure attempts, and Validate will
  recommend an unapplied PI candidate. The qualified SIMC result is expected to
  be `Kp = 5.769 %/degC`, `Ti = 5.000 s`, and
  `Ki = 1.154 %/(degC*s)`. Derive these values from
  `Kp=T/[K*(lambda+L)]`, `Ti=min(T,4*(lambda+L))`, and `Ki=Kp/Ti`, substituting
  `K=0.20`, `T=5`, `L=1`, and balanced `lambda=3.333 s`. The model-predicted Balanced response is expected to
  settle in about `13.5 s`, overshoot by about `0.094 percent`, and retain a
  final normalized error below `0.001`; these values are predictions to verify,
  not PLC constants.
- **Controller under test:** Show the actual Structured Text for the current
  Robust PI and the independent validation copy using the recommended Balanced
  PI. Both use the same `Control.PID`, bias, limits, setpoint weights,
  anti-windup, initialization and `0.1 s` update. Retain each requested and
  applied heater command separately. The program shall contain no stage verdict,
  model fit, method ranking, acceptance status, auto-apply write, or embedded
  response result.
- **Experiment — successful exact-simulation route:** Follow the exact visible
  two-stage lesson workflow. In Control Studio **Monitor → Response**, verify
  Simulation source, project/loop/plant/model/controller
  revisions, signal identities and units, good quality, operating point,
  limits, mode, restoration, and current Robust gains. In **Test**, use
  **Observe without excitation**, disabled amplitude `0`, and maximum duration
  `220.1 s`. The only Control Studio write is `P1.ExperimentEnabled` at
  `%MX96.0`; the project owns both controller runs, the load event, and
  restoration. The native lesson test owns the internal
  `Test -> Identify -> Tune -> Validate` orchestration, method attempts,
  failure cases, unchanged-revision check, and unapplied export verdict. Those
  service stages shall remain inspectable in native evidence but shall not be
  presented as additional student screens or as Control Studio writes.
- **Native failure and recovery laboratory:** Starting from a fresh copy
  before each case, exercise: missing selected plant or unsaved configuration
  at Test; nonfinite, nonpositive or unsupported model information at Identify;
  an excitation request outside reviewed signal, range, duration or topology
  at plan review; deliberately poor or insufficient observed data at Identify;
  a model outside every tuning method's domain at Tune; mandatory validation
  failure and `No acceptable controller found` at Validate; user abort during a
  bounded Simulation test; automatic abort on a declared limit; successful
  restoration; and injected restoration failure. Verify that completed stages,
  failed stage, exact reason, retained evidence and nearest recovery agree and
  that no later stage or controller write occurs. Physical actions are not
  performed; their missing independent review and immediate approval shall be
  shown as the boundary.
- **Comparison measures:** Compare the running Robust PI with the unapplied
  recommendation using the one saved conditions record and the measures already
  defined in Lesson 51. Also measure orchestration behavior: planned versus
  executed stage order, evidence revision equality, number of physical or
  Simulation approvals, excitation count, stop reason, restoration state,
  completed-stage prefix, failed-stage identity, candidate `applied = false`,
  and active-controller revision before and after every path that reaches the
  caller's execution boundary. Counts and states are exact; performance values
  use the units and precision declared by their requirement rows. A pure
  pre-execution service failure cannot observe caller approval, runtime
  excitation, or controller revision. Such a record shall mark execution
  evidence `Not applicable` with that reason, retain only the stage outcome it
  actually returned, and link to the caller- or runtime-owned proof. It shall
  never synthesize those measurements merely to fill the record.
- **Validation:** The success path shall execute exactly
  `Test -> Identify -> Tune -> Validate`, reuse the exact Simulation model with
  zero excitation, retain all candidate attempts and requirements, produce the
  predicted Balanced PI within formula tolerance `1e-7`, keep the active Robust
  controller revision unchanged, and export an unapplied candidate. Every
  failure case shall stop at the first failed stage, execute no later stage,
  preserve completed evidence, and offer the specified recovery. Every path
  that executes through the caller shall retain its approval count, excitation
  count and controller revision before/after; a path rejected wholly inside the
  pure service shall retain the explicit `Not applicable` execution-evidence
  disposition instead. Abort shall
  stop output changes and restoration shall be proven from observed state.
  Any silent excitation, repeated approval, weakened requirement, missing
  failure reason, fabricated model, evidence-source promotion, controller
  revision change, or `applied = true` fails the lesson.
- **Explanation:** Reconcile the fast successful path with its evidence source:
  it is short because an exact saved Simulation model already supplies accepted
  configuration evidence, not because identification is optional on an unknown
  plant. Explain how the same orchestrator must request a bounded Test and
  independent model validation when evidence cannot be reused. Explain why
  stopping early and returning a specific recovery is correct behavior rather
  than incomplete automation.
- **Transfer and required conclusion:** Automatic Tune is valuable when it
  makes a long engineering sequence reproducible while leaving every decision
  inspectable. It shall never turn Simulation evidence into physical evidence,
  perform an unapproved physical action, hide a rejected model or candidate,
  weaken a requirement to obtain a winner, or silently apply gains. Completion
  means **validated and unapplied**. Application and rollback remain separate
  explicitly authorized operations.

### 3.9 Industrial control structures

Lessons 53 through 60 form the industrial-control module within the shipped
**Advanced** course. Their lesson metadata MUST therefore use `level =
"Advanced"`, and the Advanced course card MUST identify its range as Lessons
41–60 so every lesson in this module is reachable from the normal curriculum
navigation.

#### Lesson 53 — Feedforward control: conveyor oven

- **Objective:** Use measured conveyor speed to design static and dynamic
  feedforward for a continuous conveyor oven, compare both designs with the
  same feedback-only proportional-integral controller, and decide from
  executable evidence whether early disturbance compensation is worth its
  actuator movement and model dependence. The learner shall be able to retain
  feedback trim, place feedforward inside the authoritative output constraints,
  and reject invalid disturbance data rather than letting it act silently.
- **Context:** A continuous industrial oven heats product moving through a
  controlled zone. Heater power is the one-sided manipulated variable from `0`
  through `100 percent`. Product-zone temperature is the process value in
  degrees Celsius. Conveyor speed is a measured disturbance expressed as a
  relative speed ratio: `1.0` is nominal production and `2.0` is twice the
  nominal speed. The desired temperature is `120 degC`; nominal heater power is
  `70 percent`; and the controller executes every `0.1 s`. Three independent
  simulated ovens use the same initial state, heater path, speed-disturbance
  path, PI settings, limits, timing, and seed. They differ only in having no
  feedforward, static feedforward, or dynamic feedforward. All results are
  Simulation evidence.
- **Reason:** Lesson 52 showed that a controller can be correctly identified,
  tuned, and validated while still accumulating load-disturbance error: even a
  qualified feedback controller must wait until its process value changes. In
  this oven, conveyor speed is measured before faster-moving product changes
  the controlled-zone temperature. That available early information creates a
  reason to act before feedback error appears. It does not create permission to
  remove feedback or bypass actuator protection.
- **Theory before practice — signals and paths:** First define the temperature
  setpoint (SP) as the desired product-zone temperature in degrees Celsius and
  the process value (PV) as the measured product-zone temperature in degrees
  Celsius. Define the temperature error (e) as desired temperature minus
  measured temperature. Define the requested manipulated variable
  (MV_requested) as the heater command calculated before physical actuator
  effects, and the applied manipulated variable (MV_applied) as the bounded
  heater command received by the simulated oven, both in percent heater power.
  Define the measured disturbance (d) as conveyor speed relative to nominal
  speed and the disturbance deviation (delta_d) as measured speed minus its
  nominal value.

  Explain feedback as action caused by an observed controlled-variable error.
  Explain feedforward as action calculated from a measured cause of future
  error. Define the process path as the dynamic relationship from heater power
  to temperature and the disturbance path as the separate dynamic relationship
  from conveyor speed to temperature. Feedforward requires models of both
  paths because its purpose is to make the heater-path effect oppose the
  predicted disturbance-path effect.

  Only after all quantities are defined may the lesson write:

  `e = SP - PV`

  `MV_requested = MV_feedback + MV_feedforward`

  The actual `Control.PID` receives `MV_feedforward` through
  `PIDInput.FeedForward`; its own rate limit, amplitude limit, anti-windup and
  tracking calculation produce `MV_applied`. Adding a feedforward value after
  `PID.Update` is forbidden because the controller would not know the actuator
  command it caused and could not constrain or track it correctly.
- **Theory before practice — static compensation:** Define the steady heater-
  path gain (K_u) as the eventual temperature change in degrees Celsius per
  percentage point of heater command. Define the steady disturbance-path gain
  (K_d) as the eventual temperature change in degrees Celsius per unit change
  in relative conveyor speed. For the local oven model, `K_u = 2.0 degC/%` and
  `K_d = -20.0 degC/relative-speed`. Faster product therefore lowers the final
  temperature unless heater power increases. Static feedforward cancels only
  the predicted final effect:

  `MV_feedforward_static = -(K_d / K_u) * delta_d`

  A speed change from `1.0` to `2.0` has `delta_d = 1.0`, so static
  feedforward predicts an additional `10.0 percentage points` of heater power.
  Explain that this calculation can give the correct final correction while
  still giving a poor transient when the two paths respond at different rates.
- **Theory before practice — dynamic compensation:** Define the heater-path
  time constant (T_u) as `10.0 s` and the disturbance-path time constant (T_d)
  as `4.0 s`. The disturbance therefore reaches temperature faster than the
  ordinary heater response. Define the filtered disturbance deviation (z) as a
  first-order representation of the `4.0 s` disturbance path. After defining
  these physical meanings, present the local path models and dynamic
  feedforward relation:

  `G_u(s) = 2 / (10*s + 1)`

  `G_d(s) = -20 / (4*s + 1)`

  `MV_feedforward_dynamic = 25 * delta_d - 15 * z`

  `4 * dz/dt + z = delta_d`

  Derive it by requiring `G_u*MV_feedforward + G_d*delta_d = 0`, hence
  `MV_feedforward/delta_d=-G_d/G_u=10*(10*s+1)/(4*s+1)`, and divide that ratio
  into `25-15/(4*s+1)` before introducing `z`. This proper lead-lag realization is equivalent to `-G_d(s)/G_u(s)` for the
  declared local model. At the instant the speed doubles, `z` has not moved,
  so dynamic feedforward predicts a `25 percentage-point` increase. As `z`
  approaches `1.0`, the correction decays to the same `10 percentage points`
  as static feedforward. Explain why the initial lead is compensation for the
  slower heater path rather than an arbitrary kick.
- **Theory before practice — feedback trim, validity and mismatch:** The PI
  controller remains active in all three cases. It removes residual error from
  feedforward-model mismatch and unmeasured heat loss. Feedforward shall be
  disabled when the conveyor-speed signal is stale, forced, nonfinite, badly
  scaled, or outside its reviewed range; falling back to feedback is visible
  and does not turn bad data into heater action. Define model mismatch as a
  difference between the assumed path gain or timing and the actual oven. A
  nominal cancellation result is not evidence that a physical oven will match
  the model. A feedforward design is acceptable only over its declared local
  speed, temperature, heater and timing range.
- **Prediction:** Before simulation, calculate that feedback-only requests no
  immediate correction when the speed measurement changes because temperature
  error is still zero. Static feedforward requests `80 percent` total heater
  power immediately: the `70 percent` operating value plus `10 percentage
  points`. Dynamic feedforward initially requests `95 percent`, then approaches
  `80 percent`. Predict that dynamic feedforward gives the smallest nominal
  temperature excursion because it matches both path gains and time constants;
  static feedforward improves final compensation but permits a larger transient;
  and feedback-only gives the largest event-window error. Predict that a
  deliberately `20 percent` low dynamic-feedforward scale leaves residual
  error that PI must correct, proving why feedback remains necessary.
- **Controller under test:** The shipped Structured Text shall contain three
  real `Control.PID` instances with identical PI parameters, nominal output,
  setpoint weights, anti-windup, limits, rate limits, initialization and cycle
  time. The first receives zero feedforward. The second receives the static
  value from the measured speed deviation. The third receives the dynamic
  lead-lag value implemented with the shipped `Control.LowPass`; one visible
  scale parameter permits the declared mismatch run. The program shall expose
  measured speed, speed validity, static and dynamic feedforward, and the
  heater requests written to the plant interface. The simulator or physical
  actuator boundary shall expose the separately applied heater values. The
  program shall contain no precomputed response metrics, preferred-case flag
  or embedded experiment verdict.
- **Experiment:** Follow the exact two-stage lesson workflow. In Control Studio **Monitor → Response**, verify all three oven
  identities, Simulation evidence, SP and PV in degrees Celsius, measured
  speed, disturbance-data quality, feedforward values, requested and applied
  MV in percent, `0.1 s` timing, mode, limits, restoration, and absence of saturation. In
  **Test**, use **Observe without excitation**, disabled amplitude `0`, and
  maximum duration `90.1 s`. The only Control Studio write is the Boolean
  project authority `P1.ExperimentEnabled` at `%MX136.2`; the project starts
  every oven at `120 degC`, `70 percent` heater and speed `1.0`.
  At `30.0 s`, change only speed to `2.0`; retain all samples through `90.0 s`.
  Record the predictions before revealing the traces. Repeat from the same
  initial state with the dynamic-feedforward scale changed from `1.0` to `0.8`.
  The native test owns the separate scale-mismatch and invalid-speed cases,
  all metrics, and verdicts; unavailable shared Identify, Tune, or Validate
  actions are not lesson steps. Confirm invalid speed falls back to feedback
  without a hidden feedforward command. Abort on nonfinite
  data, invalid actuator feedback, output outside `0..100 percent`, temperature
  outside the local `90..145 degC` range, or loss of runtime freshness.
- **Controlled comparison:** For the nominal comparison, change only the
  feedforward structure. Hold the plant equations, initial states, PI
  parameters, speed event, sample schedule, duration, output and rate limits,
  seed and metric windows equal. For the mismatch comparison, change only the
  visible dynamic-feedforward scale. A comparison is invalid if one case uses a
  different disturbance, starting condition, timing or limit.
- **Decision measures:** Define event-window integral absolute temperature
  error (IAE_event) as the `0.1 s`-weighted sum of absolute `SP - PV` from
  `30.0` through `60.0 s`, in `degC*s`. Define peak temperature deviation as
  the largest absolute `PV - 120 degC` over the same window, in degrees Celsius.
  Define cumulative applied-heater movement as the sum of absolute changes in
  `MV_applied` from `30.0` through `60.0 s`, in percentage points. Define
  final-window mean absolute error over `80.0` through `90.0 s`, in degrees
  Celsius, but report it only if the PV range over that window is at most
  `0.05 degC` and applied-MV range is at most `0.10 percentage points`.
  Retain maximum requested and applied heater power, saturation duration,
  feedforward peak, signal-valid state and fallback state.
- **Validation:** Every sample shall be finite; applied heater power shall stay
  within `0..100 percent`; and no accepted result may be based on saturation.
  Under the nominal model, static feedforward shall reduce `IAE_event` below
  feedback-only, and dynamic feedforward shall reduce it below static
  feedforward while keeping the predicted `25 percentage-point` peak and
  `10 percentage-point` final correction within declared tolerance. The
  `0.8`-scale mismatch run may be worse than nominal dynamic compensation, but
  it shall recover through PI, remain bounded, and outperform feedback-only on
  the declared event measure. Invalid speed data shall produce visible
  feedback-only fallback. Retain all four run identities and do not label any
  Simulation result as physical-plant qualification.
- **Explanation:** Relate the feedback-only delay to causality: the controller
  has no error at the speed event and therefore has no reason to move until PV
  falls. Relate static feedforward's residual transient to the faster
  disturbance path. Relate dynamic feedforward's larger initial command to the
  lead needed to make the slower heater path oppose that disturbance. Explain
  the mismatch residual as model error removed by feedback trim. Compare
  reduced temperature error with increased actuator movement rather than
  calling the smallest-error case universally best.
- **Transfer and required conclusion:** Measured-disturbance feedforward is
  justified when a trustworthy disturbance is available early, both paths are
  identified over the operating region, actuator authority remains adequate,
  and equivalent validation shows useful improvement. Static compensation
  addresses steady gain; dynamic compensation also addresses path timing.
  Neither replaces feedback, safety limits, data qualification or physical
  commissioning. Lesson 54 next asks what to do when the useful early signal is
  not an external disturbance but a faster controlled variable with its own
  actuator-facing loop; that measured limitation creates cascade control.

#### Lesson 54 — Cascade control: jacketed reactor

- **Objective:** Commission a jacket-temperature inner loop before closing a
  reactor-temperature outer loop, prove that the secondary measurement sees a
  utility disturbance earlier and responds materially faster, and decide from
  equivalent experiments whether cascade improves product-temperature
  disturbance rejection without hiding valve limits or inactive-controller
  state.
- **Context:** A jacketed production reactor uses a steam or hot-water valve to
  control product temperature. Reactor product temperature is the primary
  controlled variable in degrees Celsius. Jacket temperature is the secondary
  controlled variable in degrees Celsius. Heating-utility condition is an
  independently changing disturbance expressed as an equivalent jacket-
  temperature contribution in degrees Celsius. Valve command is the one-sided
  manipulated variable from `0` through `100 percent`. The operating point is
  `80 degC` reactor temperature, `70 degC` jacket temperature and `50 percent`
  valve command. Every controller executes at `0.1 s`.

  Three independent simulated reactors have identical physical dynamics,
  initial state, disturbance, timing, limits and seed. The first uses one
  direct reactor-temperature PI. The second uses a properly commissioned fast
  jacket inner PI and reactor outer PI. The third uses the same outer PI but an
  intentionally slow inner PI to test the time-scale requirement. All evidence
  is Simulation evidence.
- **Reason:** Lesson 53 used a measured external disturbance and two path models
  to calculate feedforward. A jacketed reactor often has a more useful early
  signal: jacket temperature directly measures the intermediate thermal effect
  of utility pressure or temperature, but it is itself a variable that the
  valve can control. Feedforward from utility condition alone would require a
  trustworthy utility measurement and disturbance-path model. Cascade instead
  makes the fast jacket variable a closed inner loop. The technique is needed
  only if that inner loop sees the disturbance before reactor temperature and
  is fast enough to obey the outer loop.
- **Theory before practice — loop roles and authority:** Define the primary
  setpoint (SP_primary) as desired reactor temperature and the primary process
  value (PV_primary) as measured reactor temperature, both in degrees Celsius.
  Define primary error (e_primary) as desired minus measured reactor
  temperature. Define the secondary setpoint (SP_secondary) as the jacket
  temperature requested by the outer controller and the secondary process
  value (PV_secondary) as measured jacket temperature, both in degrees
  Celsius. Define secondary error (e_secondary) as requested minus measured
  jacket temperature. Define the valve request (MV_requested) and applied valve
  position (MV_applied) separately in percent.

  Only after those names are introduced may the lesson write:

  `e_primary = SP_primary - PV_primary`

  `SP_secondary = OuterPI(e_primary)`

  `e_secondary = SP_secondary - PV_secondary`

  `MV_requested = InnerPI(e_secondary)`

  The outer loop owns only the secondary setpoint. The inner loop is the only
  automatic controller with direct valve authority. A design in which both
  controllers independently write the valve is not cascade control.
- **Theory before practice — physical model and time scale:** Define jacket-
  temperature deviation (x_j) as jacket temperature minus `70 degC`, reactor-
  temperature deviation (x_r) as reactor temperature minus `80 degC`, valve
  deviation (u_v) as valve request minus `50 percent`, and utility disturbance
  (w) as the equivalent change applied to the jacket path in degrees Celsius.
  Explain first that jacket energy changes from valve heat, utility condition
  and jacket heat loss, while reactor energy changes more slowly from the
  jacket-to-product temperature difference. Then present the local model:

  `2 * dx_j/dt = -x_j + 1.4 * u_v + w`

  `12 * dx_r/dt = -x_r + (8/7) * x_j`

  The open jacket path has a `2 s` time constant and the product path has a
  `12 s` time constant. Define time-scale separation as the ratio between the
  outer controlled dynamics and the closed inner-loop dynamics. Teach the
  practical requirement that the accepted inner loop must settle at least
  three times faster than the outer loop under the declared operating region;
  a ratio is evidence to measure, not a slogan that replaces validation.
  Derive the steady valve predictions from the first balance: with
  `dx_j/dt=0`, `x_j=1.4*u_v+w`; a `5 degC` jacket step needs
  `u_v=5/1.4=3.57 percentage points`, while rejecting `w=-10 degC` at
  `x_j=0` needs `u_v=10/1.4=7.14 percentage points`. Explain that the reactor
  balance contains no direct utility term, so the jacket must move first.
- **Theory before practice — commissioning order:** Tune and validate the inner
  loop first while the outer controller tracks the test jacket setpoint. Verify
  direction, measurement, output range, response time, disturbance rejection,
  saturation, restoration and stability. Only then close the outer loop and
  treat the closed inner loop as the actuator-facing process seen by the outer
  controller. Tuning the outer loop around an uncommissioned inner loop is
  forbidden because a later inner-loop change alters the plant seen by the
  outer loop.
- **Theory before practice — constraints and tracking:** The inner controller
  owns the valve limit and anti-windup. Its saturation reduces the authority
  available to the outer loop. Define saturation propagation as making that
  loss of authority visible to the outer controller so the outer integral does
  not continue demanding an impossible jacket setpoint. During inner-loop
  commissioning, Manual mode, loss of the secondary measurement, or a detected
  valve limit, the inactive or blocked outer controller shall track the actual
  secondary setpoint. Any inactive direct or inner controller shall track the
  applied valve value before it can regain authority. A mode or authority
  transition is accepted only when the first new valve request matches the
  preceding applied value within the declared bump tolerance.
- **Prediction:** Before simulation, predict that a `5 degC` jacket-setpoint
  step will settle substantially faster with the commissioned inner tuning than
  with the intentionally slow tuning. For the regulatory test, predict that a
  `w = -10 degC` utility event at `40.0 s` moves jacket temperature before
  reactor temperature. The direct controller initially keeps the valve near
  `50 percent` because primary error is still near zero. The commissioned inner
  controller immediately increases the valve to restore jacket temperature,
  so the cascade reactor accumulates less product-temperature error. The slow
  inner loop is predicted to lose most of that advantage even though its block
  diagram is labelled cascade.
- **Controller under test:** The shipped Structured Text shall contain one
  direct PI and two explicit outer/inner PI pairs. All paths use the same
  `Control.PID` implementation, declared initialization, limits and cycle time.
  A visible commissioning mode gives the inner controllers a bounded jacket
  test setpoint while both outer controllers track it. In normal cascade mode,
  each outer output is the corresponding inner setpoint and each inner output
  is the only valve request. Visible state shall include both temperatures,
  both secondary setpoints, every valve request, utility disturbance,
  commissioning mode, inner availability and valve-limit state. The simulator
  exposes applied valve values. The PLC shall not contain response metrics,
  precomputed conclusions or a preferred-architecture flag.
- **Experiment:** Follow the exact two-stage lesson workflow. In **Monitor →
  Response**, verify Simulation evidence, plant/controller identities,
  SP/PV/MV units, good quality, timing, operating point, modes, secondary-
  setpoint range, valve limits, tracking and restoration. In **Test**, use
  **Observe without excitation**, disabled amplitude `0`, and maximum duration
  `120.1 s`. The only Control Studio write is the Boolean project authority
  `P1.ExperimentEnabled` at `%MX136.3`. The project owns commissioning mode,
  the `70` to `75 degC` jacket test from `10` through `30 s`, its restoration,
  closing cascade, the `-10 degC` utility event at `40 s`, and final
  restoration. Control Studio shall not inject another event. The native test
  owns fresh commissioning, authority-loss, metric, and verdict cases;
  unavailable shared Identify, Tune, or Validate actions are not lesson steps.
  Abort on invalid data, bad quality, valve or temperature limits, or loss of
  an authority/tracking invariant.
- **Controlled comparison:** Change only controller structure for direct versus
  commissioned cascade, and only inner tuning for commissioned versus slow
  cascade. Hold physical model, initial state, outer PI, disturbance, timing,
  limits, seed, duration and metric windows equal. The commissioning comparison
  holds the jacket step equal and does not use the reactor response to disguise
  an inadequate inner loop.
- **Decision measures:** Define inner-loop 10-to-90-percent rise time and
  two-percent settling time for the `5 degC` jacket step, in seconds. Define
  disturbance-detection lead as the time when absolute jacket deviation first
  exceeds `0.10 degC` subtracted from the time when absolute reactor deviation
  first exceeds `0.10 degC`, in seconds. Define reactor event-window integral
  absolute error as the `0.1 s`-weighted absolute primary error from `40`
  through `90 s`, in `degC*s`. Define peak jacket and reactor deviations in
  degrees Celsius and cumulative applied-valve movement in percentage points
  over the same window. Define final-window mean absolute reactor error over
  `110..120 s`, but report it only when reactor-temperature span is at most
  `0.05 degC` and applied-valve span is at most `0.10 percentage points`.
  Retain secondary-setpoint range, valve saturation duration, authority,
  tracking error, abort and restoration state. Define authority-loss tracking
  error as the largest absolute departure of the held secondary setpoint while
  inner availability is false. Define restoration bump as the first applied-
  valve change after availability returns, in percentage points.
- **Validation:** The commissioned inner loop shall be stable, unsaturated and
  at least three times faster than the declared outer response; the slow inner
  case shall visibly fail that design requirement. Jacket disturbance
  detection shall precede reactor detection. Commissioned cascade shall reduce
  reactor event IAE and peak deviation below direct feedback under the same
  utility event. The slow-inner case shall not be credited merely because it
  has two controllers; its measured benefit and time-scale ratio determine its
  status. Every valve value shall remain finite and within range. Any reported
  final error requires the settling qualification. Simulation evidence does
  not authorize a physical reactor experiment. The separate authority-loss
  record passes only when secondary-setpoint tracking error is within numerical
  tolerance, restoration bump is at most `1 percentage point`, no abort occurs,
  and normal authority is visibly restored.
- **Explanation:** Relate the direct controller's delay to its missing primary
  error at the utility event. Relate the commissioned cascade improvement to
  the jacket sensor seeing and correcting the disturbance before the slow
  product state moves. Explain why a slow inner loop makes the outer loop issue
  commands faster than the secondary process can obey, reducing separation and
  potentially adding oscillation or saturation. Explain tracking as preservation
  of internal controller state when authority or achievable output changes.
- **Transfer and required conclusion:** Cascade is justified only when the
  secondary variable is physically meaningful, controllable by the same final
  element, measures the important disturbance earlier, and can be commissioned
  materially faster than the primary loop. Inner-first tuning, constraint
  propagation and tracking are part of the architecture, not optional polish.
  Lesson 55 next addresses a different structure: when product quality depends
  on maintaining a proportion between two measured flows rather than nesting a
  faster variable, ratio control is required.

#### Lesson 55 — Ratio control: blending skid

- **Objective:** Configure a two-stream blending ratio station, predict and
  verify its response to production-rate changes, retain composition feedback
  as trim for measurement/model error, and decide when low or invalid wild-flow
  data must remove automatic ratio authority.
- **Context:** A liquid blending skid meters a base liquid through an upstream
  production valve and injects additive through a separate `0..60 L/min`
  dosing valve. Base-liquid flow is the independently established or **wild
  stream**, in litres per minute. Additive flow is the **controlled stream**,
  also in litres per minute. A downstream analyser reports the additive-to-base
  flow ratio after mixing lag. The normal point is `100 L/min` base flow,
  `30 L/min` additive flow and ratio `0.30`. The lesson uses two equivalent
  local mixing plants: composition-feedback-only and ratio-plus-feedback-trim.
  Both run at `0.1 s`; all evidence is Simulation evidence.
- **Reason:** Lesson 54 nested a fast controllable jacket variable beneath a
  slower product-temperature loop. A blender has no useful nested variable:
  product quality depends directly on keeping two material flows proportional
  while production rate changes independently. A fixed additive-flow setpoint
  or composition feedback alone waits for mixed product to reach the analyser.
  Ratio control is needed to move additive demand at the measured wild-flow
  change, before downstream ratio error appears.
- **Theory before practice — relationship and scaling:** Define measured wild-
  stream flow (q_w) and additive-flow request (q_a) in litres per minute.
  Define ratio setpoint (R_SP) and measured blend ratio (R_PV) in litres of
  additive per litre of base liquid. Define ratio error (e_R) as desired minus
  measured ratio. Define feedback trim (q_trim) in litres per minute. Only then
  write:

  `q_a,requested = R_SP * q_w + q_trim`

  `e_R = R_SP - R_PV`

  At `R_SP = 0.30` and `q_w = 100 L/min`, the untrimmed additive demand is
  `30 L/min`. When valid measured wild flow rises to `120 L/min`, it becomes
  `36 L/min` in the same controller scan. The lesson shall distinguish this
  flow ratio from additive fraction of the final combined stream; the latter
  would be `R_SP / (1 + R_SP)` and is not the controlled quantity here. Derive
  that denominator from total flow and calculate `0.30/1.30=0.2308`, so the
  difference is concrete. It
  shall verify meter engineering units, range, zero and scaling before using a
  multiplication that could turn a unit error into an actuator demand.
- **Theory before practice — feedback trim:** Ratio calculation is measured-
  disturbance feedforward based on a material relationship. It does not prove
  actual mixture quality. A downstream analyser PI supplies only q_trim so it
  can correct meter bias, concentration variation and local-model error. The
  complete request enters the same valve limits and anti-windup path. The
  lesson shall expose untrimmed ratio demand, PI trim, total requested additive
  flow and applied additive flow separately; the trim controller shall not
  hide a large persistent scaling defect that should instead invalidate the
  ratio station.
- **Theory before practice — low-flow singularity and authority:** Measured
  flow ratio uses division by q_w. As q_w approaches zero, noise and offset can
  produce an arbitrarily large reported ratio, while even the multiplicative
  demand becomes smaller than the dosing valve's controllable range. Define a
  reviewed minimum wild flow of `10 L/min`. If wild-flow quality is bad, the
  value is nonfinite or stale, or q_w is outside `10..160 L/min`, ratio
  authority is unavailable. This lesson's safe simulated response tracks the
  additive request to `0 L/min`, freezes no misleading composition claim, and
  requires restoration qualification before re-entering automatic ratio
  control. A physical skid may require a different reviewed safe state.
- **Theory before practice — local plant model:** Define blend-ratio deviation
  (x_R) as measured ratio minus `0.30`, additive-flow deviation (u_a) as applied
  additive flow minus `30 L/min`, and wild-flow deviation (u_w) as actual base
  flow minus `100 L/min`. Explain that the downstream analyser behaves as a
  two-second local mixing lag. Linearising the physical ratio q_a/q_w at the
  operating point gives sensitivities `1/100 = 0.01` per litre per minute for
  additive flow and `-30/100^2 = -0.003` per litre per minute for wild flow.
  Only then write:

  `2 * dx_R/dt = -x_R + 0.01 * u_a - 0.003 * u_w`

  This is a local model for the declared `80..130 L/min` practical range, not a
  claim that a linear ratio model remains physical near zero flow.
- **Prediction:** Before simulation, calculate that a valid base-flow step from
  `100` to `120 L/min` requires additive demand to move from `30` to `36 L/min`.
  Feedback-only initially remains near `30 L/min` because downstream ratio has
  not moved. Show that the local model gives
  `x_R=0.01*0-0.003*20=-0.06`, hence `0.24`; then show that ratio feedforward
  gives `0.01*6-0.003*20=0`, hence `0.30`.
  With a separate `+5 L/min` wild-flow meter bias, untrimmed demand is
  `37.5 L/min`; analyser trim must remove approximately `1.5 L/min` at
  equilibrium. With measured wild flow below `10 L/min` or invalid quality,
  predict removal of ratio authority and a tracked zero additive request.
- **Controller under test:** The shipped Structured Text shall contain two real
  `Control.PID` instances with identical composition-trim tuning. One receives
  zero feedforward; the other receives the valid ratio demand. It shall expose
  actual and measured wild flow, quality and low-flow qualification, baseline
  and ratio analyser values, untrimmed ratio demand, trim contribution,
  requested outputs and plant-facing inputs. It shall contain no precomputed
  metric, preferred controller flag or timed production event.
- **Experiment:** Follow the exact two-stage lesson workflow. In **Monitor →
  Response**, verify Simulation evidence,
  both plant/controller identities, actual and measured wild flow, quality,
  `L/min` scaling, ratio SP/PV, requested/applied additive flows, ratio demand,
  trim, limits, `0.1 s` timing, restoration, and operating point. In **Test**,
  use **Observe without excitation**, disabled amplitude `0`, and maximum
  duration `90.1 s`. The only Control Studio write is
  `P1.ExperimentEnabled` at `%MX160.1`; the project changes actual and measured
  wild flow from `100` to `120 L/min` at `30.0 s`, observes through `90.0 s`,
  and restores nominal flow. The native test owns separate restored runs that
  change only measured wild flow to `125 L/min` after the physical step, then
  set quality invalid, and finally request `5 L/min`. Never interpret the local
  plant model at the low-flow fallback point as valid composition evidence.
  Unavailable shared Identify, Tune, or Validate actions are not lesson steps.
  Abort on invalid required data, an additive request outside `0..60 L/min`,
  ratio outside `0.10..0.50` during a qualified run, lost identity/authority,
  or failed restoration.
- **Controlled comparison:** For feedback-only versus ratio-plus-trim, hold the
  two plant models, initial state, PI tuning, production event, sample schedule,
  duration, limits and metric windows equal. For bias and low-flow cases,
  restore the nominal state and change only the declared measurement property.
- **Decision measures:** Define event-window blend-ratio integral absolute
  error as the `0.1 s`-weighted absolute ratio error from `30` through `60 s`,
  in seconds because ratio is dimensionless. Define peak absolute ratio
  deviation over the same window. Define cumulative applied-additive-flow
  movement as total variation in `L/min`. Define final-window mean absolute
  ratio error over `80..90 s`, eligible only if ratio span is at most `0.001`
  and applied-flow span at most `0.02 L/min`. Retain first-scan ratio demand,
  final trim, saturation duration, quality/low-flow classification, authority,
  abort and restoration state.
- **Validation:** The nominal ratio path shall request `36 L/min` immediately,
  keep event IAE below `35 percent` of feedback-only, and have lower peak
  deviation. Under `+5 L/min` meter bias, PI trim shall recover final mean
  absolute ratio error below `0.002` without saturation and its final trim
  shall oppose the predicted `+1.5 L/min` demand error. Invalid or below-
  minimum wild flow shall visibly remove ratio authority and track additive
  request to zero; no blend-quality metric is eligible in that state. Every
  numeric result requires finite, in-range, correctly scaled signals.
- **Explanation:** Relate the prompt ratio action to the material-flow
  relationship, and the feedback-only delay to downstream mixing/analyser lag.
  Explain why analyser trim repairs modest mismatch but cannot make invalid
  wild-flow data trustworthy. Explain why a multiplication may be numerically
  finite at low flow while the underlying ratio measurement and dosing
  authority are physically unusable.
- **Transfer and required conclusion:** Ratio control is justified when a
  measured wild stream establishes production rate and a controlled stream
  must follow a reviewed proportion. Scaling, quality, minimum-flow authority,
  valve limits and feedback trim are part of the design. Lesson 56 next asks
  how one temperature controller should coordinate two different final elements
  when heating alone cannot cover both sides of the operating range.

#### Lesson 56 — Split-range control: reactor heating and cooling

- **Objective:** Convert one signed reactor-temperature controller demand into
  coordinated heating and cooling valve requests, compare dead-zone, overlap
  and installed-gain-characterized mappings under equivalent temperature
  transitions, and select the simplest mapping that crosses neutral without a
  control gap or simultaneous utility waste.
- **Context:** An exothermic batch reactor uses a hot-water valve to raise
  temperature and a cooling-water valve to lower it. Reactor-temperature
  setpoint and process value are in degrees Celsius. The temperature controller
  produces one signed heat-balance demand from `-100` through `+100 percent`:
  positive asks for heating and negative asks for cooling. Each physical valve
  is one-sided from `0` through `100 percent`. Around the `80 degC` point, one
  percent heating raises temperature at `0.04 degC/s` before heat loss, while
  one percent cooling lowers it at `0.06 degC/s`. Three equivalent simulated
  reactors compare a four-point dead zone, four-point overlap, and a mutually
  exclusive installed-gain-characterized mapping. All execute at `0.1 s`; all
  evidence is Simulation evidence.
- **Reason:** Lesson 55 coordinated two simultaneously useful material streams
  to preserve a ratio. Heating and cooling are opposing utilities: the plant
  needs heating above the operating balance and cooling below it, but energising
  both wastes energy and may create nonlinear cancellation. A single signed
  PID demand cannot be wired directly to two unipolar valves. The mapping
  between controller and final elements must define which valve owns each
  region and how authority crosses neutral.
- **Theory before practice — signed demand and split point:** Define reactor-
  temperature setpoint (SP), reactor-temperature process value (PV), and
  temperature error (e) in degrees Celsius. Define signed controller demand
  (D) in percent, heating-valve request (H_requested) and cooling-valve request
  (C_requested) in percent, and their separately observed applied positions
  (H_applied and C_applied). Define the **split point** as `D = 0 percent`, where
  authority changes final element. Only then write the ideal mutually exclusive
  mapping:

  `e = SP - PV`

  `D = TemperaturePI(e)`

  `H_requested = max(D, 0)`

  `C_requested = max(-D, 0)`

  At `D = +30 percent`, heating is `30 percent` and cooling is zero. At
  `D = -20 percent`, heating is zero and cooling is `20 percent`. At zero, both
  close. The PID owns temperature feedback and one signed output; the split-
  range mapper owns actuator selection. Separate PIDs fighting through the two
  valves are not this structure.
- **Theory before practice — dead zone and overlap:** A **dead zone** is an
  interval around the split point where neither valve responds. For a four-
  point half-width g, the lesson's dead-zone candidate uses
  `H = max(D - g, 0)` and `C = max(-D - g, 0)`. It avoids simultaneous utilities
  but removes control authority near neutral, so integral demand must traverse
  the gap before the plant responds. An **overlap** intentionally opens both
  valves around neutral: `H = max(D + o, 0)` and `C = max(o - D, 0)` for
  overlap o. It can hide mechanical deadband but consumes opposing utilities,
  and unequal installed gains create a net heat disturbance even at D=0.
  Gap or overlap shall be a reviewed physical design choice, never an
  unexplained percentage added for a smoother plot.
- **Theory before practice — installed gain:** Define installed heating gain
  (K_H) as local temperature-rate change per percent heating valve and installed
  cooling gain (K_C) as the magnitude per percent cooling valve. Equal signed
  controller demand should create comparable thermal authority on either side.
  Here `K_H = 0.04 degC/(s*percent)` and
  `K_C = 0.06 degC/(s*percent)`. Heating remains `H = D` for positive demand;
  cooling for negative demand is characterized as:

  `C_requested = (-D) * K_H / K_C = (-D) * 2/3`

  Derive that scale by equating thermal-rate magnitudes,
  `K_C*C = K_H*(-D)`, then dividing by `K_C`. With the declared gains,
  `K_H/K_C = 0.04/0.06 = 2/3`. For the overlap candidate at zero demand,
  explicitly calculate the net rate as `0.04*4 - 0.06*4 = -0.08 degC/s`;
  the drift follows from unequal installed gains, not from the PID.

  This local characterization makes equal-magnitude signed demands produce
  equal-magnitude initial temperature rates. It does not compensate valve
  stiction, pressure variation or a changing nonlinear installed characteristic.
- **Theory before practice — sequencing, limits and crossover:** Selection,
  characterization, individual valve limits and rate limits occur after the
  signed PID demand. Requested and applied valve positions remain distinct.
  The inactive valve shall be explicitly commanded closed. Simultaneous
  heating and cooling is an energy-conflict state for the accepted mapping.
  On loss of automatic authority, the PID tracks zero signed demand with its
  setpoint aligned to the current process value and both valve requests close.
  A restored controller may regain authority only after quality, modes and
  applied valve positions are confirmed; its first demand and valve pair must
  satisfy the declared bump tolerance.
- **Theory before practice — local plant:** Define reactor-temperature
  deviation (x_T) as temperature minus `80 degC`, heating input (u_H) and
  cooling input (u_C) as applied valve percentages. Explain that ambient heat
  loss returns the unforced local plant toward `80 degC`, while the two valves
  act in opposite directions. Only then write:

  `dx_T/dt = -0.05 * x_T + 0.04 * u_H - 0.06 * u_C`

  The linear plant and constant installed gains apply only over the declared
  `72..88 degC` range.
- **Prediction:** Before simulation, map `D = +30`, `0` and `-20 percent` for
  all three candidates. Predict that the dead-zone case has both valves closed
  while nonzero demand lies inside `-4..+4 percent`; the overlap case has both
  valves open at `4 percent` when D is zero and therefore an initial net
  cooling rate of `0.08 degC/s`; the characterized candidate has no simultaneous
  utility use and maps `D = -20 percent` to `13.33 percent` cooling. Predict
  which candidate will accumulate gap time, utility overlap and asymmetric
  response before viewing traces.
- **Controller under test:** The shipped Structured Text shall contain three
  real `Control.PID` instances with identical signed PI tuning and limits. It
  shall implement the three explicit mapping candidates after those
  controllers, expose each signed demand, requested heating/cooling pair,
  applied values and authority, and contain no precomputed score, preferred-
  mapping flag, forced result or timed setpoint event.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  **Monitor**, verify Simulation evidence,
  all three equivalent plant/controller identities, SP/PV/D units and signs,
  heating/cooling identities, requested/applied positions, installed gains,
  mapping parameters, `0.1 s` timing, quality, authority and limits. In
  **Test**, select **Observe without excitation**, amplitude `0`, and duration
  `200.1 s`. The only writable signal is project experiment authority
  `P1.ExperimentEnabled` at `%MX88.5`. The runnable project starts at `80 degC`,
  steps SP to `84 degC` at `20.0 s`, to `76 degC` at `80.0 s`, and back to
  `80 degC` at `140.0 s`; it owns safe restoration when authority clears.
  Retain full-resolution samples around each demand-zero crossing. In a separate
  restored run, remove automatic authority and prove zero tracked demand and
  closed valves. Abort on invalid required data, temperature outside
  `72..88 degC`, any valve outside `0..100 percent`, simultaneous valves in the
  characterized candidate, lost authority, or failed restoration.
- **Controlled comparison:** Hold the three plant models, initial state, PI
  tuning, setpoint sequence, sample schedule, duration, limits and seed equal;
  change only the mapper. The dead-zone and overlap widths are both exactly
  four percentage points. Do not retune a candidate to conceal the mapping
  behavior being compared.
- **Decision measures:** Define setpoint-event integral absolute temperature
  error over `20..80 s`, `80..140 s`, and `140..200 s`, in `degC*s`; peak
  deviation and two-percent settling time per event; cumulative applied heating
  and cooling movement in percentage points; simultaneous-utility exposure as
  the time for which both applied valves exceed `0.1 percent`; utility
  throughput as the time integral of H_applied+C_applied in `percent*s`; and
  dead-zone exposure as time with `|D| >= 0.25 percent` while both applied valves
  are at most `0.1 percent`. Define crossover bump as the largest change in net
  thermal input `K_H*H_applied-K_C*C_applied` between adjacent samples spanning
  a demand-sign change, in `degC/s`. Final-window error is eligible only after a
  declared temperature and valve-span settling rule passes.
- **Validation:** Every signal shall be finite and bounded. The characterized
  candidate shall have zero simultaneous-utility exposure, zero unjustified
  dead-zone exposure, both valves closed at zero demand, and comparable heating
  and cooling authority after installed-gain scaling. The dead-zone candidate
  shall expose its lost-authority time; the overlap candidate shall expose
  simultaneous duration and greater utility throughput rather than being
  selected merely for a smooth trace. The accepted crossover shall meet the
  declared net-thermal-input bump tolerance and authority-loss test. Report
  `Not settled`, `Saturated`, `Invalid` or `Aborted` before ranking ineligible
  results.
- **Explanation:** Relate dead-zone delay to missing actuator authority,
  overlap consumption and zero-demand drift to opposing unequal utilities, and
  characterized symmetry to installed thermal gains. Explain that the mapper
  is part of loop dynamics: changing it changes the effective plant seen by the
  PI and can require renewed tuning and validation.
- **Transfer and required conclusion:** Split-range control is justified when
  one controlled variable requires sequenced final elements over different
  portions of one demand range. Neutral behavior, installed gain, physical
  valve characteristics, energy conflict, limits, tracking and crossover are
  design inputs. Lesson 57 next adds a higher-priority equipment constraint
  that must override ordinary production control and return authority without
  windup or a bump.

#### Lesson 57 — Selector and override control: compressor protection

- **Objective:** Configure a recycle-valve high selector that gives a minimum-
  flow-margin controller priority over discharge-pressure production control,
  compare untracked and externally tracked inactive controllers, and prove
  bounded constraint takeover and hysteretic bumpless return under one
  reproducible pressure-demand cycle.
- **Context:** A centrifugal compressor uses one recycle valve from `0` through
  `100 percent`. Opening recycle increases protected flow margin in percentage
  points but reduces discharge pressure in bar. Normal discharge-pressure
  control therefore has negative action: higher pressure demand tends to close
  recycle. A minimum-flow-margin controller has positive action: falling margin
  requests more recycle. The normal point is `8 bar`, `20 percent` margin and
  `30 percent` recycle. Three equivalent local compressors compare normal-only
  control, an override selector whose inactive controllers keep integrating,
  and the same selector with external tracking and a `14/16 percent` takeover/
  release band. Every controller executes at `0.1 s`; all evidence is
  Simulation evidence and is not anti-surge or machinery-protection proof.
- **Reason:** Lesson 56 selected one of two opposing final elements from a
  single temperature demand. Here two objectives compete for the same final
  element. Production wants pressure, but minimum safe flow has higher
  priority. A valve mapper cannot decide this because both candidates are
  complete controller demands. A selector must reallocate authority before the
  protected constraint is crossed and return it without releasing stored
  integral action.
- **Theory before practice — candidate demands and selector direction:** Define
  discharge-pressure setpoint (SP_p), process value (PV_p) and error (e_p) in
  bar. Define minimum-flow-margin setpoint (SP_m), process value (PV_m) and
  error (e_m) in percentage points. Define normal recycle request (MV_normal),
  protective recycle request (MV_protect), selected request (MV_selected) and
  applied recycle position (MV_applied), all in percent. Opening recycle raises
  margin, so the safer candidate is the larger valve request. Only after the
  directions and common units are established may the lesson write:

  `e_p = SP_p - PV_p`

  `e_m = SP_m - PV_m`

  `MV_selected = max(MV_normal, MV_protect)`

  A MIN selector would be correct only for an actuator/sign arrangement in
  which the smaller common-unit request is safer. Selector choice follows
  process physics, not a universal preference for MIN or MAX.
- **Theory before practice — authority and priority:** Define **authority** as
  permission for one candidate to determine the applied valve. In normal
  operation pressure control owns authority. When measured margin reaches the
  `14 percent` takeover threshold, the protection latch becomes active and the
  high selector gives any larger protective request priority. Pressure error is
  intentionally accepted while equipment margin has priority. This regulatory
  override is not a safety-instrumented function, emergency trip, compressor-
  map surge controller or substitute for independently engineered machinery
  protection.
- **Theory before practice — inactive windup and external tracking:** An
  unselected integral controller still sees error. Without knowledge of the
  applied output, its integral may move far from the selector value. Define
  external tracking error for candidate i as selected/applied output minus that
  candidate's internal request. The inactive controller shall use Track mode
  against MV_applied while retaining its real SP and PV histories for a
  bumpless transition. In the truST PID form, tracking establishes the
  actuator-facing state before the current incremental terms; therefore the
  lesson shall verify the first post-selection request, not assume that a mode
  label alone proves exact tracking. The untracked comparison intentionally
  leaves both candidates in Auto to expose delayed takeover or a release bump.
- **Theory before practice — hysteresis and release:** A single threshold can
  chatter when noisy margin repeatedly crosses it. Define takeover threshold
  (M_on) as `14 percent` and release threshold (M_off) as `16 percent`. Because
  higher margin is safer, M_off is above M_on. Protection latches when
  `PV_m <= M_on`. It may release only when `PV_m >= M_off` and the normal demand
  has risen enough that transferring authority cannot close the applied valve.
  The two-percentage-point separation is **release hysteresis**; it changes
  switching logic, not the protected physical minimum of `12 percent`.
- **Theory before practice — local plant:** Define pressure deviation (x_p) as
  discharge pressure minus `8 bar`, margin deviation (x_m) as flow margin minus
  `20 percent`, and recycle deviation (u_r) as applied valve position minus
  `30 percent`. Explain first that pressure has a five-second local time
  constant and decreases with recycle, while margin has a two-second time
  constant and increases with recycle. Only then write:

  `dx_p/dt = -0.20 * x_p - 0.01 * u_r`

  `dx_m/dt = -0.50 * x_m + 0.50 * u_r`

  The steady local gains are `-0.05 bar/percent` and
  `+1 margin-percentage-point/percent`. Derive them by setting the derivatives
  to zero: `x_p=-0.01*u_r/0.20=-0.05*u_r` and
  `x_m=0.50*u_r/0.50=u_r`. Closing recycle from `30` to `10 percent` gives
  `u_r=-20`, hence pressure `8+1=9 bar` but margin `20-20=0 percent`.
  Holding margin at `12 percent` means `x_m=-8`, so the valve must remain at
  least `30-8=22 percent`; this limits pressure to `8-0.05*(-8)=8.4 bar`.
- **Prediction:** Before simulation, calculate the preceding steady values.
  Predict that a pressure-setpoint change from `8` to `9 bar` at `20.0 s`
  drives normal-only recycle toward `10 percent` and violates the 12-percent
  margin. Predict that the untracked protective integrator has accumulated the
  wrong state while margin was high and therefore takes over late. Predict that
  external tracking keeps the inactive candidate adjacent to MV_applied, so the
  tracked override acts near the 14-percent threshold. When pressure setpoint
  returns to `8 bar` at `80.0 s`, predict that the normal request rises,
  protected margin recovers through `16 percent`, and tracked authority returns
  with a smaller valve step than the untracked selector.
- **Controller under test:** The shipped Structured Text shall contain five
  real `Control.PID` instances: one normal-only controller, an untracked normal/
  protection pair, and a tracked normal/protection pair. Candidate tuning,
  limits and signs shall be explicit. The tracked pair shall expose latch,
  active authority, both requests, selected request and applied valve. The PLC
  shall contain no timed setpoint event, precomputed metric, hidden minimum-
  margin clamp, preferred-case flag or claim of machinery protection.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  **Monitor**, verify Simulation evidence,
  all three compressor/controller identities, pressure and margin SP/PV units,
  request and applied-valve units, signs, selector direction, active authority,
  latch, thresholds, limits, quality, `0.1 s` timing and operating point. In
  **Test**, select **Observe without excitation**, amplitude `0`, and duration
  `140.1 s`. The only writable signal is project experiment authority
  `P1.ExperimentEnabled` at `%MX89.1`. The runnable project starts at `8 bar`,
  `20 percent` margin and `30 percent` recycle, steps pressure SP to `9 bar` at
  `20.0 s`, returns it to `8 bar` at `80.0 s`, and owns restoration when
  authority clears. Retain full-resolution samples around takeover and release.
  In a separate restored run, invalidate margin quality and prove that automatic
  selector operation becomes unavailable rather than presenting an unprotected
  valve as protected. Abort on invalid required data, pressure outside
  `7..10 bar`, margin below the zero model boundary, valve outside `0..100
  percent`, inconsistent selector authority, chatter beyond the declared
  transition limit, or failed restoration.
- **Controlled comparison:** Hold three plant models, initial state, normal and
  protective tuning, setpoint sequence, threshold values, sample schedule,
  duration, limits and seed equal. Change only absence of override, absence of
  tracking, or presence of tracking/hysteresis. Do not retune the untracked case
  to erase the state-management behavior being taught.
- **Decision measures:** Define minimum margin during `20..80 s` in percentage
  points; time below the protected `12 percent` minimum; pressure event IAE in
  `bar*s`; takeover delay from first `PV_m <= 14 percent` sample to first
  protection-authority sample; release delay from first `PV_m >= 16 percent`
  eligible sample to normal authority; authority transition count; maximum
  adjacent applied-valve change at takeover and release in percentage points;
  inactive tracking error in percentage points; and cumulative applied-valve
  movement. Define final-window pressure and margin errors over `130..140 s`
  only after declared PV and MV span conditions pass.
- **Validation:** Normal-only control shall demonstrate why the override is
  required by crossing the 12-percent margin. The untracked selector shall
  retain its delayed/bumped behavior rather than being described as adequate
  because it eventually protects. The tracked selector shall keep minimum
  margin at or above `11.5 percent` within numerical tolerance, make exactly one
  takeover and one release, avoid threshold chatter, and keep each authority-
  transition valve step within `1.0 percentage point`. Its inactive candidate
  tracking error shall remain within the declared incremental-update tolerance.
  Pressure sacrifice shall be visible and intentional. Invalid margin quality
  invalidates protection evidence. Classify `Unprotected`, `Late takeover`,
  `Chattering`, `Not settled`, `Invalid` or `Aborted` before ranking metrics.
- **Explanation:** Relate normal-only margin loss to the pressure controller's
  physically correct recycle-closing action. Relate untracked delay or bump to
  stored inactive integral state. Relate the tracked response to common-unit
  candidate requests aligned with the actual selected valve and hysteresis that
  separates takeover from release. Explain why maintaining a simulated margin
  is control evidence, not certification of compressor protection.
- **Transfer and required conclusion:** Override control is justified when
  several controllers compete for one actuator and one constraint has explicit
  priority. Direction, common units, selector, external tracking, latch,
  hysteresis, quality and bumpless release are one architecture. Lesson 58 next
  addresses a plant whose dynamics themselves change with operating point, so
  no selector of fixed candidates can make one PI tuning behave consistently.

#### Lesson 58 — Gain scheduling: gravity-drained tank

- **Objective:** Derive and validate three local gravity-tank models, prove why
  one fixed PI gives inconsistent low/middle/high-level response, then compare
  continuous atomic parameter interpolation with hysteretically selected
  parallel tracked PIs over both scheduling directions and noisy boundaries.
- **Context:** A vented water tank with `1.0 m2` cross-sectional area receives
  pump flow from `0` through `0.060 m3/s`. Gravity outflow passes a fixed outlet
  whose coefficient is `0.050 m2*sqrt(m)/s`. Liquid level is controlled from
  `0.30` through `0.80 m`. Three equivalent nonlinear tanks compare a fixed PI,
  one PI whose qualified parameters are interpolated atomically, and three
  parallel local PIs with only the selected controller in Auto and the others
  tracking applied pump flow. The lesson uses deterministic `0.0005 m` sensor
  noise, a `0.1 s` controller interval and Simulation evidence only.
- **Reason:** Lesson 57 selected between competing controller outputs because
  equipment priority changed. Here authority remains with level control, but
  the plant dynamics change with level. A fixed controller safe at the slow
  high-level operating point is unnecessarily slow low in the tank; tuning for
  low level can become too aggressive as the process time constant and gain
  increase. A selector cannot repair this because the correct controller
  dynamics, not the objective priority, depend on operating point.
- **Theory before practice — nonlinear equilibrium:** Define liquid level (h)
  in metres, inlet pump flow (q_in) and gravity outlet flow (q_out) in cubic
  metres per second, tank area (A) in square metres and outlet coefficient (c)
  in square metres times square-root metre per second. Explain conservation in
  words first: stored volume rises by inflow minus outflow, and gravity outflow
  grows with the square root of level. Only then write:

  `q_out = c * sqrt(h)`

  `A * dh/dt = q_in - q_out`

  At equilibrium dh/dt is zero, so `q_in,eq = c*sqrt(h_eq)`. Calculate pump
  flows `0.02739`, `0.03708` and `0.04472 m3/s` at `0.30`, `0.55` and `0.80 m`.
- **Theory before practice — local linear models:** Define level deviation
  (delta_h) from an equilibrium and pump-flow deviation (delta_q) from its
  equilibrium. Linearising the square-root outflow gives a first-order local
  model whose steady gain (K_h) and time constant (T_h) are:

  `K_h = 2 * sqrt(h_eq) / c`

  `T_h = 2 * A * sqrt(h_eq) / c`

  Derive them from the tangent
  `sqrt(h_eq+delta_h) ~= sqrt(h_eq)+delta_h/(2*sqrt(h_eq))`, subtract the
  equilibrium balance, and normalise the result to
  `T_h*d(delta_h)/dt + delta_h = K_h*delta_q`.

  Therefore the three gains are `21.91`, `29.66` and `35.78 m/(m3/s)` and the
  corresponding time constants are `21.91`, `29.66` and `35.78 s`. The lesson
  shall derive these numbers and validate each local model with a bounded pump
  pulse around its own equilibrium; a local model shall not be presented as a
  global replacement for the nonlinear plant.
- **Theory before practice — local tuning and fixed baseline:** Define
  proportional gain (Kp) in `(m3/s)/m` and integral gain (Ki) in
  `(m3/s)/(m*s)`. For the declared `10 s` closed-loop design time, the qualified
  local PI points use common `Kp = 0.100` and Ki values `0.00456`, `0.00337` and
  `0.00280` from low to high level. The fixed baseline uses the conservative
  high-level values everywhere. The lesson shall explain that the varying Ki,
  rather than a visually dramatic Kp change, is still gain scheduling: the
  integral time must follow the changing plant time constant.
  Derive the tuning for design time `lambda=10 s` with
  `Kp=T_h/(K_h*lambda)=0.100`, `Ti=T_h`, and `Ki=Kp/Ti`; for example the low
  point gives `0.100/21.91=0.00456 (m3/s)/(m*s)`.
- **Theory before practice — scheduling variable and interpolation:** Define
  scheduling variable (rho) as the bounded requested operating level because it
  is known before the level transition and was used to qualify the local
  models. It is not the controller error. For adjacent schedule points rho_0
  and rho_1 and parameter theta, first explain linear interpolation in words,
  then write:

  `alpha = (rho - rho_0) / (rho_1 - rho_0)`

  `theta(rho) = theta_0 + alpha * (theta_1 - theta_0)`

  Rho is filtered with a declared `2 s` time constant and clamped to the
  validated `0.30..0.80 m` range. Clamping is visible and invalidates any claim
  beyond the boundary. Kp, Ki, limits, rate limits, anti-windup and tracking
  time form one immutable parameter record; the record shall be calculated and
  applied atomically between controller updates so no scan observes mixed old
  and new tuning.
- **Theory before practice — parallel controllers and hysteresis:** The
  alternative architecture keeps low, middle and high PIs as separate stateful
  controllers. Define active region from measured level, with nominal
  boundaries halfway between schedule points: `0.425` and `0.675 m`. A
  `0.010 m` hysteresis half-band requires an upward crossing above `0.435` or
  `0.685 m` and a downward crossing below `0.415` or `0.665 m`. Only the selected
  controller is Auto. Every inactive controller receives a compensated external
  tracking reference that makes its evaluated request equal the applied pump
  flow while preserving the real setpoint and measurement histories. The
  compensation shall account for the PID implementation's current incremental
  terms; merely replacing the setpoint with current level is forbidden because
  it creates a proportional step when that controller receives authority. Define a
  bumpless region transition by the first newly selected request matching the
  preceding applied flow within `0.0005 m3/s`.
- **Theory before practice — architecture tradeoff:** One interpolated PID has
  one state and smooth parameters but depends on atomic reconfiguration and
  qualified interpolation between points. Parallel PIs preserve distinct local
  states and can use discrete region logic, but require external tracking and
  hysteresis. Neither architecture adapts or identifies online. Scheduling
  variable validity, model envelope and transition logic remain explicit.
- **Prediction:** Before simulation, calculate all three equilibrium flows,
  local gains/time constants and Ki values. Predict that fixed high-level Ki
  produces the slowest low-level recovery. Predict smooth Ki movement with no
  output step for the interpolated controller. Predict exactly two upward and
  two downward region transitions during a `0.30 -> 0.55 -> 0.80 -> 0.55 ->
  0.30 m` sequence, with no chatter when noisy level remains inside a boundary
  band. Predict both scheduled architectures improve the worst regional
  settling/IAE without changing pump constraints.
- **Controller under test:** The shipped Structured Text shall contain five
  real `Control.PID` instances: fixed, interpolated, and three parallel local
  PIs, plus the real `Control.PIDGainScheduler`. It shall expose requested
  level, each measured level and pump request, filtered scheduling value,
  interpolation fraction, active Kp/Ki, parallel region and transition count.
  It shall contain no timed level sequence, precomputed metrics, preferred-
  architecture flag or hidden plant classification.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  **Monitor**, verify Simulation evidence,
  three nonlinear plant and five controller identities, level and flow units,
  requested/applied pump flow, actual `0.1 s` timing, schedule validity,
  filtered rho, segment/fraction, Kp/Ki record, parallel region, hysteresis,
  tracking, noise, limits and operating point. In **Test**, select **Observe
  without excitation**, amplitude `0`, and duration `420.1 s`. The only
  writable signal is project experiment authority `P1.ExperimentEnabled` at
  `%MX89.2`. The runnable project starts at `0.30 m`; requests `0.55 m` at
  `30 s`, `0.80 m` at `120 s`, `0.55 m` at `240 s`, and `0.30 m` at `330 s`;
  run through `420 s`, and owns restoration when authority clears. The native
  lesson test owns the local pulse qualifications and separate restored holds
  around both boundaries that exercise deterministic sensor noise without a
  commanded crossing. Abort on
  invalid/stale/nonfinite level, level outside `0.20..0.90 m`, pump flow outside
  `0..0.060 m3/s`, invalid schedule, excessive transition count, tracking
  violation or failed restoration.
- **Controlled comparison:** Hold nonlinear plant parameters, initial state,
  level sequence, local tuning records, common limits, sample schedule, noise
  seed, duration and measure windows equal. Change only fixed versus
  interpolated versus parallel-tracked architecture. Boundary-noise holds use
  identical actual level/noise traces for transition comparisons.
- **Decision measures:** Define regional integral absolute level error in `m*s`,
  peak deviation in metres, 2-percent settling time in seconds, pump-flow total
  variation in `m3/s`, saturation/rate-limit duration, interpolated-parameter
  adjacent-step size, region transition count, first-request bump in `m3/s`,
  inactive tracking error in `m3/s`, and boundary chatter count. A final-window
  level error is eligible only when level span and pump-flow span meet declared
  settling conditions. Retain schedule validity/clamping and evidence class.
- **Validation:** The fixed baseline shall demonstrate materially worse low-
  level regional IAE or settling than at least one scheduled architecture. Both
  scheduled forms shall remain finite and inside common plant/actuator limits,
  improve the declared worst-region measure, and retain acceptable performance
  at all three points. Interpolated tuning shall change atomically without a
  pump step above `0.0005 m3/s` attributable to parameters. Parallel selection
  shall make exactly four commanded region transitions, no boundary-noise
  chatter, and no first-request bump above `0.0005 m3/s`. Out-of-envelope rho
  shall be visibly clamped/unqualified. Classify `Unqualified region`,
  `Chattering`, `Not settled`, `Saturated`, `Invalid` or `Aborted` before
  ranking.
- **Explanation:** Relate increasing local gain and time constant to the square-
  root outlet derivative. Relate fixed low-level sluggishness to the high-level
  conservative integral gain. Explain interpolation as scheduled parameters
  in one retained PID state, and parallel tracking/hysteresis as state and
  authority management across several PIs. Neither result proves behavior
  outside the validated level range.
- **Transfer and required conclusion:** Gain scheduling is justified only by
  measured or derived operating-point dependence and validated local models.
  Scheduling variable, envelope, atomicity, tracking and transition logic are
  part of the controller. Lesson 59 next addresses a different failure: a
  validated process may remain nearly linear while long transport delay consumes
  the feedback stability margin.

#### Lesson 59 — Smith predictor: long conveyor oven

- **Objective:** Identify and validate a delayed conveyor-oven temperature
  model, qualify the strongest ordinary PI that satisfies declared robustness
  limits, then design a Smith-predictor loop and decide from nominal and
  mismatched-model evidence whether delay compensation is justified.
- **Context:** A continuous food-drying conveyor passes through a recirculated
  heated-air duct. Heater command is `0..100 percent`; a downstream product-
  temperature sensor measures `15..60 degC`. Air and belt transport cause an
  `16 s` delay before a heater change reaches that sensor. Around the declared
  `20 degC`, zero-command operating point, the nominal local process has gain
  `0.8 degC/%` and time constant `12 s`. Heater slew is limited to `10 %/s`,
  deterministic sensor-noise standard deviation is `0.08 degC`, and a
  `-2 degC` inlet-temperature disturbance occurs at `70 s`. The lesson uses
  Simulation evidence only.
- **Reason:** Lesson 58 changed controller parameters because the tank dynamics
  varied with operating level. The oven remains inside one qualified local
  region, so gain scheduling has no measured reason to act. Its problem is
  transport delay: ordinary feedback cannot observe recent heater action until
  the product reaches the downstream sensor. Increasing PI gain therefore
  consumes phase margin and eventually produces unacceptable overshoot or
  cycling; the strongest acceptable feedback PI remains slower than desired.
- **Theory before practice — delayed plant:** Define heater command (u) in
  percent, downstream product temperature (y) in degrees Celsius, operating
  heater command (u_0), operating temperature (y_0), process gain (K) in
  degrees Celsius per percent, time constant (T) in seconds and transport delay
  (L) in seconds. Explain in words that the local temperature tends toward the
  equilibrium created by an earlier heater command, not the command being sent
  now. Only then write the first-order-plus-dead-time relationship:

  `T * dy(t)/dt + (y(t) - y_0) = K * (u(t - L) - u_0)`

  For `K = 0.8 degC/%`, a permanent `10 percent` heater change predicts an
  eventual `8 degC` rise. Nothing attributable to that change reaches the
  downstream sensor during the first `16 s`; after the delay, about `63 percent`
  of the rise appears in one `12 s` time constant. Define the delay-to-time-
  constant ratio as `L/T = 16/12 = 1.333` and explain why transport delay
  dominates the feedback dynamics.
- **Theory before practice — strongest ordinary PI:** Define proportional gain
  (Kp), integral gain (Ki), closed-loop design time (lambda), overshoot,
  settling, phase margin and delay margin in words and units before using them.
  Generate a monotonic candidate sweep from conservative through aggressive,
  using the complete delayed model and common actuator constraints. A candidate
  is eligible only when nominal and declared gain/time/delay perturbations stay
  bounded, heater limits are respected, overshoot is no more than `2 degC`, the
  final settled error is no more than `0.5 degC`, phase margin is at least
  `35 deg`, and delay margin is at least `2 s`. Select the highest-bandwidth
  eligible PI; do not call the weakest tried tuning "best" and do not select an
  aggressive candidate from nominal setpoint IAE alone.
  Derive the retained gains from `Kp=T/[K*(lambda+L)]`, `Ti=T`, and
  `Ki=Kp/Ti`. The accepted `lambda=16 s` gives
  `Kp=12/[0.8*(16+16)]=0.46875` and `Ki=0.0390625`; the next candidate uses
  `lambda=8 s`, giving `Kp=0.625` and `Ki=0.052083`. The Smith design applies
  the same 8-second target to the delay-free model, so `L=0` in the controller
  design and the gains are `1.875` and `0.15625`; the physical delay remains.
- **Theory before practice — Smith prediction:** Define the delay-free model
  output (y_m0), delayed model output (y_mL), measured product temperature
  (y_meas), model correction (c_m) and predicted current temperature (y_pred),
  all in degrees Celsius. The delay-free model estimates what recent heater
  action is doing before the downstream sensor can show it. The delayed model
  predicts what the sensor should show. Their difference anchors the internal
  prediction to measurement:

  `c_m = y_meas - y_mL`

  `y_pred = y_m0 + c_m`

  The PI receives y_pred only after the delay history is complete and all model,
  signal and operating-envelope checks pass. Otherwise ordinary measured
  feedback owns the loop. The correction is limited to `+/-5 degC`; reaching
  the limit is invalid model evidence, not a successful prediction.
- **Theory before practice — disturbance and mismatch:** The Smith structure
  removes nominal delay from the controller's internal setpoint path; it does
  not make physical transport disappear. An unmeasured inlet-temperature
  disturbance still reaches the downstream sensor after transport. Incorrect
  K, T or L makes y_mL disagree with y_meas and moves c_m. Define model mismatch,
  correction saturation, predictor availability and fallback before testing.
  A model fitted and validated around one product, belt speed and airflow shall
  not be generalized to another operating condition without evidence.
- **Prediction:** Before simulation, calculate the `8 degC` final response to a
  `10 percent` heater step, the `16 s` no-response interval, the `12 s` 63-percent
  interval and `L/T = 1.333`. Predict that the next-more-aggressive ordinary PI
  fails at least one predeclared robustness condition. For the accepted PI and
  predictor design, predict lower nominal integral absolute temperature error
  than ordinary feedback because y_pred responds before y_meas. Predict worse
  error, larger correction or fallback when actual gain, time constant or delay
  differs from the retained model. Do not predict that Smith control eliminates
  delayed disturbance response.
- **Controller under test:** The shipped Structured Text shall contain one real
  `Control.PID` and one real `Control.SmithPredictorFOPDT`. Separate restored
  runs select the accepted ordinary-feedback tuning or the Smith design; a live
  unqualified gain jump is forbidden. Expose setpoint, measured temperature,
  predicted temperature, delay-free and delayed model outputs, correction,
  requested heater command, predictor enabled/active/fallback and model-valid
  state. The code shall contain no timed setpoint, precomputed metric, hidden
  model perturbation or preferred-result flag.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  **Monitor**, verify Simulation evidence,
  oven/controller/model identities, setpoint, measured and predicted
  temperatures, requested and applied heater command, units, quality, `0.1 s`
  actual timing, rate and amplitude limits, history completeness, model
  validity, correction and fallback. In **Test**, select **Observe without
  excitation**, amplitude `0`, and duration `180.1 s`. The only writable signal
  is project experiment authority `P1.ExperimentEnabled` at `%MX90.7`. The
  runnable project performs the restored ordinary or Smith run: hold `20 degC` through
  `10 s`, request `35 degC`, apply the declared inlet disturbance at `70 s`, and
  run through `180 s`, and restore `20 degC` when authority clears. The native
  lesson test owns the bounded model qualification, ordinary-PI sweep, and
  moderate and severe mismatch cases.
- **Controlled comparison:** Hold initial state, setpoint and disturbance
  sequence, plant limits, sample schedule, duration, noise seed and metric
  windows equal. The ordinary candidate sweep changes only tuning. The nominal
  architecture comparison records both the accepted ordinary tuning and Smith
  tuning and attributes the complete design change honestly; an additional
  same-tuning comparison may isolate predictor contribution. Mismatch runs
  change only the declared actual plant parameter while retaining the internal
  model.
- **Decision measures:** Define integral absolute temperature error in `degC*s`,
  peak overshoot in degrees Celsius, 2-percent settling time in seconds,
  final-window absolute error in degrees Celsius, requested/applied heater total
  variation in percentage points, saturation/rate-limit duration, predictor-
  active fraction, peak absolute correction in degrees Celsius, correction-
  limit duration and fallback count. Steady-state measures require declared
  temperature and heater spans over the final `10 s`; invalid or fallback
  samples are not ranked as predictor performance.
- **Validation:** The accepted ordinary PI shall satisfy every declared nominal
  and perturbation condition, while the next-more-aggressive candidate shall
  fail at least one, proving the selection boundary. Nominal Smith control shall
  remain finite, respect common heater constraints, settle within `0.5 degC`,
  reach `90 percent` of the requested temperature rise at least `15 percent`
  sooner and reduce setpoint-window integral absolute temperature error,
  evaluated from the `10 s` request until immediately before the `70 s`
  disturbance, relative to accepted ordinary feedback. Post-disturbance
  regulatory error shall be reported separately and shall not be credited as
  delay-free disturbance compensation. Moderate mismatch shall remain
  bounded but show worse IAE, peak correction or settling than nominal Smith.
  Severe mismatch or correction saturation shall remove predictor authority and
  visibly fall back to measured feedback; it shall not be ranked as successful
  Smith performance. Classify `Not settled`, `Unstable`, `Saturated`, `Rate
  limited`, `Model mismatch`, `Correction limited`, `Fallback`, `Invalid` or
  `Aborted` before ranking.
- **Explanation:** Relate ordinary PI's delayed over-correction to heater action
  that is physically in transit but absent from y_meas. Relate nominal Smith
  improvement to y_m0 and correction anchoring, not to removal of real delay.
  Relate mismatch degradation and fallback to disagreement between y_mL and
  the sensor. A nominal improvement does not justify a predictor without an
  uncertainty envelope and monitored validity.
- **Transfer and required conclusion:** Smith prediction is justified only for
  delay-dominant operation with a validated model, explicit mismatch tests,
  bounded correction and deterministic measured-feedback fallback. It can
  improve nominal response but becomes risky when gain, time or delay
  uncertainty is understated. Lesson 60 next shows a different failure: even a
  sound controller and model cannot compensate reliably for an uncharacterized
  or mechanically sticking final control element.

#### Lesson 60 — Valve characterization and stiction diagnosis

- **Objective:** Characterize the installed flow relationship of a pneumatic
  control valve, distinguish healthy motion, command deadband and mechanical
  stiction from equivalent experiments, and decide when the evidence requires
  valve maintenance rather than controller retuning.
- **Context:** The real equipment is a cooling-water flow bench supplied from a
  regulated header. A pneumatically actuated globe valve meters water through
  the test line. The flow setpoint and measured process flow are in cubic
  metres per hour. The proportional-integral controller requests valve stem
  position from `0` through `100 percent`; a digital positioner independently
  reports physical stem position over the same range. The valve is one-sided,
  closes on loss of authority, is limited to `25 percentage points per second`,
  and has a `0.4 s` local actuator lag. Flow responds to effective valve
  capacity with a `3 s` hydraulic time constant. The declared operating point
  is `80 percent` stem position and `64 m3/h`. Three equivalent
  simulated benches contain the same installed characteristic, initial state,
  controller, timing and limits. Only their actuator condition differs:
  healthy, `2 percentage points` command deadband, or `5 percentage points`
  stiction with a `2-percentage-point` slip jump. All results are Simulation
  evidence, not inspection or proof of a physical valve fault.
- **Reason:** Lesson 59 showed how a validated dynamic model can compensate a
  delay-dominant process while mismatch monitoring prevents unjustified trust
  in prediction. A sound controller and process model can still appear to fail
  when the final control element does not follow its request. Increasing
  integral action may make a sticking valve move more often, but it can also
  enlarge the breakaway cycle and accelerate wear. The actuator must therefore
  be characterized and diagnosed before another tuning change is considered.
- **Theory before practice — inherent and installed behavior:** Explain that
  an inherent valve characteristic relates stem travel to flow capacity at a
  fixed pressure drop, while the installed characteristic relates stem travel
  to actual process flow after the valve is installed in its piping system.
  Define valve stem position (x) in percent, rangeability (R) as a dimensionless
  ratio, normalized flow capacity (c) from zero through one, and measured flow
  (q) in cubic metres per hour before writing the lesson's equal-percentage
  reference relationship:

  `c = (R^(x/100) - 1) / (R - 1)`

  Explain the normalization: `x/100` is fractional travel, exponentiation
  creates the equal-percentage shape, subtracting one makes the closed value
  zero, and dividing by `R-1` makes full travel equal one.

  Equal increments of stem travel therefore multiply inherent capacity by
  approximately the same factor over the useful range; they do not add equal
  flow. The executable simulator deliberately does not call that inherent
  equation the installed curve. Under its fixed-head piping approximation,
  normalized installed flow target is the square of normalized position. After
  defining installed target flow (q_target) in cubic metres per hour and
  hydraulic response time (T_q) in seconds, write

  `q_target = 100 * (x/100)^2`

  `T_q * dq/dt = q_target - q`

  with `T_q = 3 s`. This distinction teaches that the installed relationship
  must be characterized rather than copied from an inherent-valve datasheet.
  Pump curves, piping losses, cavitation, fluid properties and pressure
  variation can make a physical installed curve different again.
- **Theory before practice — four different lost-motion mechanisms:** Define
  requested manipulated variable (MV_requested) as the position demanded by
  the controller, in percent. Define applied manipulated variable (MV_applied)
  as the independently reported physical stem position, also in percent. The
  process value (PV) is measured flow in cubic metres per hour. **Command
  deadband** means a small request change around the last accepted request is
  ignored. **Backlash** means direction-dependent mechanical play: after a
  reversal, motion is consumed taking up clearance before the driven element
  follows, so the same command can correspond to different positions depending
  on approach direction. **Stiction** means static friction holds the stem even
  while request-to-position difference grows; after the breakaway threshold is
  exceeded, the stem slips by a finite amount. Backlash shall be recalled from
  Lesson 24 and contrasted using its reversal memory, but it shall not be
  falsely implemented by relabelling command deadband or stiction.
- **Theory before practice — diagnostic signatures:** Healthy motion makes
  stem position begin following each resolvable request after the ordinary lag.
  Deadband produces ignored small request increments relative to the last
  accepted request, followed by smooth motion once the accumulated request
  exceeds the band. Stiction produces a growing difference between request and
  measured position, a stationary stem, then a discrete breakaway jump.
  Backlash appears specifically after direction reversal and follows different
  opening and closing paths. In a closed loop with integral action, stiction
  can produce a repeated hold, wind-up, breakaway, overshoot and reversal cycle.
  Process flow and controller request alone may suggest several of these causes;
  independently measured position and repeatable reversal evidence are required
  for a confirmed valve diagnosis in this lesson.
- **Prediction — installed curve:** Before execution, calculate the expected
  fixed-head flows for positions `0`, `20`, `40`, `60`, `80` and `100 percent`:
  `0.00`, `4.00`, `16.00`, `36.00`, `64.00` and `100.00 m3/h`. Explain that the
  local flow gain rises from `0.20 m3/h per percentage point`
  over `0..20 percent` to `1.80 m3/h per percentage point` over `80..100
  percent`. A single position-to-flow gain is therefore not valid across the
  full travel range.
  Derive those secant gains explicitly as `(4-0)/(20-0)=0.20` and
  `(100-64)/(100-80)=1.80 m3/h per percentage point`.
- **Prediction — reversal probe:** Restore all three benches to `70 percent`
  position. Increase the common request in `0.5-percentage-point` increments.
  Predict immediate lagged motion on the healthy valve. Predict no deadband-
  valve motion while the change from its last accepted request is at most `2
  percentage points`, followed by smooth motion on the first larger request.
  Predict that the stiction valve holds while request-to-position difference is
  at most `5 percentage points`, then makes an initial `2-percentage-point`
  breakaway movement. These are mechanism predictions, not tolerances inferred
  after viewing the result.
- **Controller under test:** The shipped Structured Text shall contain three
  real `Control.PID` instances with identical proportional and integral gains,
  `80 percent` bias, output limits and initial state. A shared manual demand
  shall drive all three controllers in Manual for characterization and reversal
  probes. In Auto, each controller shall use the shared flow setpoint and its
  own measured flow. The program shall expose each flow measurement and each
  requested position. Physical position, applied input, deadband state and
  stiction state shall come from the existing simulated-plant status contract;
  the Structured Text shall not calculate fake position feedback, diagnosis
  labels, comparison metrics or a preferred maintenance verdict.
- **Experiment — characterize before closing the loop:** Use only **Monitor**
  and **Test**. In Control Studio **Monitor**, verify Simulation evidence, the three bench identities, flow and
  position units, `0.1 s` timing, data quality, mode, common manual demand,
  requested positions, independently reported positions, applied inputs,
  actuator-state flags, limits and safe-close restoration. In **Test**, select
  **Observe without excitation**, amplitude `0`, and duration `180.1 s`. The
  only writable signal is project experiment authority `P1.ExperimentEnabled`
  at `%MX91.1`. The project keeps the controllers in Manual and first runs the healthy bench through `0`, `20`,
  `40`, `60`, `80` and `100 percent`, holding each point until position and flow
  satisfy their declared settling rules. Compare measured flow with the
  predicted installed table. Restore `70 percent`, then apply the same
  `0.5-percentage-point` opening increments and mirrored closing increments to
  all three benches. Repeat the reversal probe at least three times. Finally
  restore the declared operating point, enter Auto bumplessly at `110 s` and change the
  common flow setpoint from `64` to `70 m3/h` on all cases.
  Abort on nonfinite or stale required data, bad quality, valve position outside
  `0..100 percent`, flow outside `0..105 m3/h`, inconsistent mode/authority, or
  failed safe-close/restoration behavior. The project closes the valves when
  authority clears; the native test owns the three independently restored
  probes and numeric diagnosis.
- **Controlled comparison:** The healthy, deadband and stiction runs shall use
  the same installed curve, initial position, request sequence, PI settings,
  sample schedule, duration, limits, seed and metric windows. Only deadband or
  stiction parameters may differ. Characterization evidence and closed-loop
  diagnosis evidence shall remain separate: an installed curve explains
  position-dependent gain but does not by itself prove a mechanical fault.
- **Decision measures:** Define installed-curve error as measured settled flow
  minus predicted flow at each commanded position, in cubic metres per hour.
  Define request-to-position gap as the absolute difference between requested
  and measured position, in percentage points. Define stationary-position time
  as accumulated samples during which request changes but measured position
  changes by no more than `0.02 percentage points`, in seconds. Define
  breakaway size as the first measured position change after a stationary
  interval, in percentage points. Define reversal lost motion as the request
  travel after a direction change before measured position moves more than
  `0.02 percentage points`, in percentage points. For the closed-loop window,
  define process-flow peak-to-peak excursion in cubic metres per hour and
  cumulative applied-position movement as total variation in percentage
  points. State the exact sample window and qualification condition before
  reporting each value; a motion signature that does not repeat is
  `Inconclusive`, not a confirmed fault.
- **Validation:** Require the healthy manual characterization to agree with
  every declared table point within `0.05 m3/h` after settling. Under the
  reversal probe, the healthy position shall respond to every resolvable
  request, the deadband case shall hold through `2 percentage points` and then
  move without the configured `2-percentage-point` slip jump, and the stiction
  case shall hold through `5 percentage points` and then exhibit the declared
  breakaway jump. Require the hold/breakaway signature to repeat in at least
  three restored reversal probes before classifying the simulated fault. The
  closed-loop run shall then relate that verified mechanism to its flow effect;
  it shall not be required to manufacture a sustained limit cycle when the
  declared plant and operating point instead produce offset or a single
  transition. The healthy and
  deadband cases shall not be labelled stiction merely because flow is
  nonlinear or delayed. Controller gains shall remain identical throughout;
  no apparent improvement from a retune may replace the position-based fault
  evidence. If position feedback is absent, stale or invalid, retain the
  request/PV evidence but classify the mechanical diagnosis as unconfirmed.
- **Explanation:** Relate the installed flow table to the changing local slope,
  the deadband trace to the last accepted request, and the stiction trace to
  stored pneumatic force overcoming static friction. Explain why integral
  action keeps increasing requested position while a stuck stem leaves flow
  error unchanged, then reverses only after the slip-induced flow jump. A gain
  change can alter this cycle's period or amplitude without removing its
  mechanical cause.
- **Transfer and required conclusion:** Characterize a valve when its installed
  flow gain is unknown, and diagnose mechanical stiction only from repeatable
  request, independent stem-position and process-response evidence. A simulated
  injected fault demonstrates a signature; it does not certify a physical
  valve diagnosis. Physical work requires reviewed topology, positioner
  calibration, pressure conditions, safe operating envelope, abort and
  restoration procedure, and explicit approval. Where the evidence confirms a
  sticking final element, inspection or maintenance is the correct next action;
  retuning alone is not a repair. Lesson 61 next introduces a different cause
  of misleading single-loop behavior: two manipulated variables physically
  interact with two controlled compositions.

### 3.10 Multivariable control

Lessons 61 through 63 begin the multivariable and state-space progression in
the shipped **Modern** course. Their lesson metadata MUST use `level =
"Modern"` so each lesson remains reachable through normal curriculum
navigation.

#### Lesson 61 — Interaction testing and Relative Gain Array: distillation column

- **Objective:** Measure all four local input-output gains of a binary
  distillation column, calculate and interpret its Relative Gain Array, and
  select the defensible decentralized loop pairing without yet designing the
  controllers.
- **Context:** The real-world example is a continuous binary distillation
  column near one qualified production operating point. The overhead
  light-product purity deviation (`y1`) and bottoms heavy-product purity
  deviation (`y2`) are measured in composition percentage points relative to
  their normal product targets. The reflux-valve change (`u1`) and reboiler-
  steam-valve change (`u2`) are manipulated changes in percentage points from
  their normal positions. Each simulated command is limited to `-10` through
  `+10` percentage points; each experiment uses only a `+5`-percentage-point
  move. The local model is valid only inside that declared region and does not
  model tray hydraulics, analyzer transport delay, flooding, weeping, product
  inventory or a physical safety system.
- **Reason:** Lesson 60 showed that apparent loop trouble can originate in one
  actuator and must be localized with independent position evidence. Here both
  final elements are healthy, but reflux and reboiler steam each move both
  product compositions. Treating tag proximity or the largest open-loop gain
  as a pairing rule can therefore give the wrong feedback direction after the
  other loop is closed. Interaction must be measured before decentralized
  controllers are chosen.
- **Theory before practice:** Explain a single-input, single-output path in
  words before introducing matrix notation. The steady-state gain from input
  `uj` to output `yi`, written `gij`, is the settled change in that output
  divided by the independently applied change in that input, with every other
  manipulated input held constant. Its unit is composition percentage points
  per valve percentage point. Explain that the first index names the output
  row and the second names the input column, then arrange the four gains as
  `G = [[g11, g12], [g21, g22]]`.

  Loop interaction means that closing one loop changes the effective gain seen
  by another because both controllers act through the same plant. A gain sign
  observed with the other input fixed may therefore differ from the effective
  sign after the other output is held by feedback. The Relative Gain Array
  (`Lambda`) compares those open-loop and closed-loop effective gains without
  depending on engineering-unit scaling. For a nonsingular steady-state gain
  matrix, explain element-by-element multiplication before writing
  `Lambda = G .* transpose(inverse(G))`. Explain the inverse, transpose and
  element-wise product in words; ordinary matrix multiplication is not used in
  the final operation.

  For the two-by-two case derive `det(G)=g11*g22-g12*g21` and
  `inverse(G)=(1/det(G))*[[g22,-g12],[-g21,g11]]`. For the declared matrix,
  `det(G)=0.8*0.7-(-0.4)*(-0.3)=0.44`; after transposition the elementwise
  products give `lambda11=0.8*0.7/0.44=1.2727` and
  `lambda12=(-0.4)*0.3/0.44=-0.2727`, with the symmetric second row.

  Each row and column of an exact Relative Gain Array sums to one. A relative
  gain near `+1` usually indicates a pairing weakly altered by closing the
  other loop. A large magnitude indicates strong interaction and sensitivity.
  A negative relative gain warns that the paired path's effective feedback
  direction reverses when the other loop regulates its output. These are
  pairing diagnostics, not automatic stability or performance guarantees.
  The calculation is undefined for a singular gain matrix and unreliable when
  gain estimates are poorly excited, unsettled, uncertain or valid at a
  different operating point.
- **Physical model and prediction:** The executable lesson uses four stable
  first-order path states whose contributions sum into the two measured
  products. In words, the overhead response to reflux has a `20 s` response
  time, the overhead response to reboiler steam has a `35 s` response time,
  the bottoms response to reflux has a `40 s` response time, and the bottoms
  response to reboiler steam has a `30 s` response time. A reflux increase
  improves overhead light-product purity and reduces bottoms heavy-product
  purity; a reboiler-steam increase reduces overhead light-product purity and
  improves bottoms heavy-product purity. After settling, the declared local
  gain matrix is `G = [[0.8, -0.4], [-0.3, 0.7]]`. Lesson 61 uses the qualified
  final values for pairing but retains the unequal transient paths so Lesson
  62 can test whether steady or dynamic decoupling is justified.

  Before running, calculate that a `+5`-percentage-point reflux move should
  produce `+4.00` and `-1.50` composition-percentage-point changes. A separate
  `+5`-percentage-point reboiler move should produce `-2.00` and `+3.50`
  composition-percentage-point changes. The determinant is `0.44`, so this
  local matrix is invertible. The predicted Relative Gain Array is
  `[[1.2727, -0.2727], [-0.2727, 1.2727]]`; display it to two decimals only
  after retaining the higher-precision calculation. Predict that the diagonal
  pairing, overhead purity to reflux and bottoms purity to reboiler steam, is
  eligible but still interacting. Reject the cross pairing because both
  relative gains are negative.
- **Controller under test:** This is an open-loop interaction-identification
  lesson, so Structured Text shall not disguise a centralized controller as
  the experiment. The shipped program shall expose Manual mode, independent
  reflux and reboiler changes, both measured composition deviations and a safe
  zero-deviation restoration. A short actual Structured Text excerpt shall
  show the commands being limited and routed to the two-input plant. The
  Relative Gain Array and pairing verdict shall be calculated from retained
  experiment evidence, not hard-coded into Structured Text.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  Control Studio Monitor, verify the `Simulation` evidence
  label, column identity, Manual authority, good and fresh signal quality,
  `0.1 s` sample interval, both input limits, both requested and applied input
  changes, both composition deviations and safe restoration. In Test, record a
  zero-input baseline for `20 s` by selecting **Observe without excitation**,
  amplitude `0`, and duration `400.1 s`. The only writable signal is project
  experiment authority `P1.ExperimentEnabled` at `%MX16.1`. The project changes
  only reflux from `20` through `200 s`, restores both inputs from `200` through
  `220 s`, then changes only reboiler steam through `400 s`. It restores both
  to zero when authority clears. Treat the two excited windows as independent
  records; never combine both inputs in one gain estimate.

  For each output, qualify settling only when its change over the final `10 s`
  is no more than `0.02` composition percentage points. Calculate the baseline
  and final values as means over their respective final `10 s` windows. Abort
  and invalidate the run for nonfinite, stale, forced or invalid signals;
  wrong authority; an input outside `-10` through `+10`; an output outside
  `-6` through `+6` composition percentage points; a second input that moves
  by more than `0.02` percentage points; missing sample intervals; or failed
  restoration. Keep the plant, initial state, sample interval, duration,
  limits and seed equal between experiments.
- **Comparison measures:** Define the input change as final-window applied
  input mean minus baseline-window applied input mean, in percentage points.
  Define each output change analogously in composition percentage points.
  Define each steady-state gain as output change divided by its independently
  excited input change, and reject division when the absolute input change is
  below `4.95` percentage points. Report the four gains with signs, units,
  settling status and windows before forming the matrix. Define determinant,
  matrix-inverse validity, every Relative Gain Array element, row/column sums
  and the selected/rejected pairings before showing results.
- **Validation:** The executable native test shall compile and run the shipped
  Structured Text with the shipped two-input plant; execute the two restored
  step records; prove both non-excited inputs remain fixed; qualify settling;
  estimate every gain within `0.01` composition percentage points per valve
  percentage point of the declaration; calculate the determinant and Relative
  Gain Array independently from those estimates; and prove row and column sums
  within `0.001` of one. The executable evidence shall also recover the four
  `63.2 percent` response times within `1.0 s` of `20/35/40/30 s`; this
  tolerance accounts for estimating the settled change from the finite
  `180 s` record rather than an infinite-time asymptote. These
  dynamic measurements are retained for Lesson 62 but do not alter the steady-
  state RGA calculation. The diagonal pairing is accepted only when both
  diagonal relative gains are positive, both cross alternatives are negative,
  the determinant is sufficiently separated from zero, every run is valid and
  the repeated calculation agrees with the retained data. A missing or invalid
  path makes the pairing verdict `Unconfirmed`; no matrix may be completed from
  configured coefficients merely to keep the workflow moving.
- **Explanation:** Relate each measured sign to material and energy movement in
  the column, then explain why holding one product with its controller changes
  the gain available to the other loop. The large positive diagonal values
  show that diagonal feedback must work harder than the corresponding
  open-loop paths suggest; the negative cross values warn of an effective sign
  reversal. The Relative Gain Array chooses a pairing but does not remove
  interaction, specify controller gains, include dynamics, or prove robust
  stability.
- **Transfer and required conclusion:** Select input-output pairings from
  qualified local multivariable evidence, not tag names or isolated direct
  gains. Repeat the complete four-path experiment when operating point,
  product, pressure, inventory, analyzer or actuator conditions materially
  change. Physical-column tests require an independently reviewed topology,
  production and safety limits, perturbation size, dwell, abort/restoration
  procedure and explicit approval. This lesson's evidence is simulation only.
  The remaining positive diagonal interaction motivates Lesson 62: compare
  decentralized proportional-integral control with justified static or dynamic
  decoupling under common events and model mismatch.

#### Lesson 62 — Decentralized PID and decoupling: same column

- **Objective:** Commission two decentralized proportional-integral loops on
  the diagonal pairing selected in Lesson 61, then decide from equivalent
  nominal and mismatch evidence whether direct control, steady decoupling or
  realizable dynamic decoupling is the simplest acceptable architecture.
- **Context:** Use the same local binary-column gain matrix and four path
  response times measured in Lesson 61. The overhead purity setpoint (`SP1`)
  and bottoms purity setpoint (`SP2`) are composition deviations in percentage
  points. The measured deviations are `PV1` and `PV2`. The virtual loop
  corrections (`v1`, `v2`) are the two independent controllers' requested
  actions before cross compensation. The requested reflux and reboiler-steam
  changes (`u1_requested`, `u2_requested`) are limited to `-10` through `+10`
  valve percentage points; the separately retained applied changes are the
  plant inputs. A bounded feed-composition test disturbance affects both
  products after the servo comparison.
- **Reason:** Lesson 61 rejected the cross pairing and selected the diagonal
  pairing, but its diagonal relative gains were `1.2727`, not one, and its four
  path response times were unequal. Pairing prevents the effective sign
  reversal; it does not stop one loop from disturbing the other. The remaining
  measured interaction creates the reason to compare decoupling rather than
  presenting it as an automatic improvement.
- **Theory before practice:** Explain decentralized control as two ordinary
  feedback controllers, each using one setpoint and one process value, whose
  physical commands act on the same coupled plant. Introduce both proportional-
  integral laws in the ordinary words and notation already established by the
  course. State the exact parallel, positional `Control.PID` form, sample time,
  initialization, output/rate limits and tracking path. Tune the overhead loop
  from the diagonal `0.8` gain and `20 s` response time, and the bottoms loop
  from the diagonal `0.7` gain and `30 s` response time. The same two controller
  settings shall be retained in every architecture comparison.
  Derive the overhead settings with `Kp=T/(K*lambda)`, `Ti=T`, `Ki=Kp/Ti`
  using `K=0.8`, `T=20 s`, `lambda=15 s`, which gives `1.6667` and
  `0.08333`. Apply the same rule to the bottoms path with `K=0.7`, `T=30 s`,
  `lambda=20 s`, giving `2.142857` and `0.0714286`.

  A static decoupler is a steady linear map between virtual controller
  corrections and physical actuator changes. Normalize the measured matrix by
  the diagonal gains in words before writing
  `M = [[1, -0.5], [-0.428571, 1]]`. Its inverse maps the virtual corrections
  into physical changes: `u = inverse(M) * v`. The resulting steady mapping is
  `G * inverse(M) = [[0.8, 0], [0, 0.7]]`. Explain that each off-diagonal
  command is feedforward from the other controller's demand; ordinary feedback
  remains responsible for residual error. Reject the inverse if the measured
  matrix is singular, poorly conditioned or invalid.
  Show that row normalization gives `-0.4/0.8=-0.5` and
  `-0.3/0.7=-0.428571`, then calculate `det(M)=0.785714` and the two-by-two
  inverse before multiplying by `G`.

  Static cancellation uses only final gains. With unequal path dynamics, its
  cross contribution can arrive too early or too late and create a transient
  product-quality excursion even when its final cross effect becomes zero. A
  dynamic decoupler shapes each cross command with a causal transfer function.
  For this plant, explain the two required ratios in words before writing
  `D12(s) = 0.5 * (20 s + 1) / (35 s + 1)` and
  `D21(s) = 0.428571 * (30 s + 1) / (40 s + 1)`. Here `s` is the Laplace
  frequency variable, not seconds. Each numerator order is no greater than its
  denominator order, so both candidates are proper and causal. Express the
  implemented forms as a direct part plus a first-order filtered part:
  `D12 = 0.285714 + 0.214286 / (35 s + 1)` and
  `D21 = 0.321429 + 0.107143 / (40 s + 1)`.
  Derive these from `D12=-G12/G11` and `D21=-G21/G22`. Decompose each ratio
  with `A*(a*s+1)/(b*s+1)=A*a/b+A*(1-a/b)/(b*s+1)` so the direct and filtered
  coefficients are traceable rather than asserted.

  Explain realizability before use: an inverse that requires prediction, an
  improper transfer function, cancellation of an uncertain unstable zero, an
  excessive high-frequency gain or unavailable fresh signals is rejected.
  Dynamic decoupling can amplify noise and timing error and places more model
  dependence in the controller. It is not a replacement for feedback,
  constraints or robustness validation.
- **Prediction:** For an overhead-only setpoint increase, direct decentralized
  control should move bottoms purity because reflux has the measured `g21`
  path. Static decoupling should remove its settled cross offset but leave a
  transient because the `20 s` direct and `40 s` cross paths differ. The
  nominal dynamic decoupler should reduce that transient by matching the path
  ratios. Make the equivalent prediction for a bottoms-only setpoint move.
  Under a perturbed cross gain or response time, predict that exact dynamic
  cancellation will degrade and may no longer justify its added complexity.
- **Controller under test:** The shipped Structured Text shall contain exactly
  two real `Control.PID` instances, one qualified `Control.StaticDecoupler2x2`
  path, and two real `Control.LowPass` states implementing the proper dynamic
  cross terms. One explicit architecture selector shall choose direct,
  steady-decoupled or dynamic-decoupled mapping while leaving controller
  parameters, setpoints, plant, timing and limits unchanged. Requested and
  applied physical commands, virtual commands, filter states, selected
  architecture and fallback status shall remain observable. Analytical
  performance metrics and the winning architecture shall not be hard-coded in
  Structured Text.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  Control Studio Monitor, verify `Simulation` evidence,
  column/model identity, Auto authority, both SP/PV pairs, both virtual
  corrections, requested/applied physical inputs, architecture, decoupler
  qualification, filters, constraints, good/fresh data, `0.1 s` timing and
  zero-deviation restoration. In **Test**, select **Observe without
  excitation**, amplitude `0`, and duration `220.1 s`. The only writable signal
  is project experiment authority `P1.ExperimentEnabled` at `%MX24.3`. The
  project selects qualified dynamic decoupling, holds zero through `20 s`, then
  applies an overhead-only `0 -> +2` point request and restores every event
  signal when authority clears. The native lesson test owns the separately
  restored direct, static, bottoms-only, disturbance, and perturbed-model
  comparisons without retuning.

  Abort for invalid, stale, forced or nonfinite required signals; wrong model,
  timing or authority; an unqualified decoupler; a requested/applied input
  outside `-10` through `+10`; either composition deviation outside `-6`
  through `+6`; sustained saturation that makes the comparison ineligible;
  missing samples; or failed restoration. When dynamic qualification fails,
  fall back deterministically to the already qualified static or direct path
  and record the reason; do not silently retain stale filter state.
- **Comparison measures:** Define primary-loop integral absolute error over the
  declared event window, unintended cross-output peak excursion in composition
  percentage points, final-window absolute error after settling, requested and
  applied input total variation in percentage points, saturation duration,
  settling status and restoration status before results. Use identical windows
  for every architecture. Report a metric as ineligible rather than ranking it
  when its prerequisite fails. Compare nominal servo, nominal disturbance and
  perturbed-model results separately; a nominal improvement cannot erase a
  mismatch failure.
- **Declared nominal requirements:** For each `+2`-composition-percentage-point
  servo record, unintended cross-output peak excursion shall not exceed `0.05`
  composition percentage points, combined two-output integral absolute error
  shall not exceed `100` composition-percentage-point seconds, and combined
  applied-input total variation shall not exceed `6` valve percentage points.
  For the feed-disturbance record, combined integral absolute error shall not
  exceed `40` composition-percentage-point seconds. These are separate
  requirements: a very small cross peak does not excuse excessive primary-loop
  error, load error or actuator travel.
  On the perturbed model, dynamic compensation is materially better than the
  qualified static case only when cross peak is at least `20 percent` lower,
  combined integral absolute error is no more than `5 percent` higher, applied
  input travel is no more than `25 percent` higher, and all absolute safety and
  final-error requirements remain eligible.
- **Validation:** First prove both direct diagonal loops are stable, bounded and
  correctly signed. Prove the static mapping agrees with the independently
  calculated normalized inverse and reduces settled cross error. Prove the
  dynamic coefficients and filter states implement the two declared proper
  transfer functions and reduce nominal transient cross excursion relative to
  static decoupling while meeting the separately declared servo-error, load-
  error, saturation and applied-input-travel limits. Dynamic decoupling need not
  beat static or direct control on every metric; its extra complexity is
  justified only when it is required to pass a material requirement that the
  simpler eligible architecture misses. Then perturb at least one cross gain
  and one cross-path response time.
  Select dynamic decoupling only if it remains eligible and materially improves
  the declared requirements; otherwise retain static or direct control and
  state the failed criterion. No architecture is applied from a configured
  matrix without executed controller/plant evidence.
- **Explanation:** Trace one controller correction through its direct physical
  input, compensating cross input, unequal plant paths and both product
  measurements. Relate the static residual to arrival-time mismatch and the
  dynamic improvement to the causal path-ratio filters. Relate degradation on
  the perturbed plant to model error entering the feedforward compensation.
  Explain why feedback eventually trims residual error while saturation or
  noise can make an aggressive inverse harmful.
- **Transfer and required conclusion:** Decoupling is justified only when a
  qualified interaction model, realizable inverse and common validation
  evidence show that it satisfies requirements better than the simpler direct
  architecture. Static decoupling cancels steady interaction; eligible dynamic
  decoupling can also shape transient interaction, but transfers additional
  model uncertainty and noise sensitivity into the controller. Physical-column
  use requires independently reviewed models, topology, limits, product
  disposition, abort/restoration and explicit approval. This lesson remains
  simulation evidence. Lesson 63 next asks whether separate single-loop checks
  can establish robustness for the coupled architecture; they cannot.

#### Lesson 63 — Multivariable robustness and architecture: distillation column

- **Objective:** Determine whether the direct and statically decoupled
  proportional-integral architectures from Lesson 62 remain acceptable for
  the binary distillation column over one declared interaction-uncertainty
  set, compare them with a centralized direction-preserving constraint
  coordinator, and select or reject an architecture from common robustness,
  performance and constraint evidence. The learner shall be able to explain
  why two satisfactory single-loop responses do not prove multivariable
  robustness.
- **Context:** Continue with the same two product measurements and two physical
  inputs used in Lessons 61 and 62. The overhead purity deviation is the first
  process value, measured in composition percentage points. The bottoms purity
  deviation is the second process value in the same unit. Reflux-valve and
  reboiler-steam-valve changes are applied input deviations in valve percentage
  points. Both inputs remain limited to `-10` through `+10` percentage points,
  both outputs remain limited to `-6` through `+6` composition percentage
  points, and the controller executes every `0.1 s`.

  The nominal local gain matrix remains
  `G0 = [[0.8, -0.4], [-0.3, 0.7]]`. Its four response times remain
  `20/35/40/30 s`. One separately identified interaction case retains both
  diagonal gains but changes the cross gains to `-0.64` and `-0.49` and the
  cross-path response times to `45 s` and `50 s`. Call this second matrix
  `G1 = [[0.8, -0.64], [-0.49, 0.7]]`. These two models form the complete
  declared uncertainty set for the lesson; passing them does not prove every
  possible column condition. The stronger-interaction case represents a
  changed but still locally stable separation condition, not an arbitrary
  hidden coefficient sweep.
- **Reason:** Lesson 62 found that nominal dynamic cancellation was excellent
  on its fitted model but did not justify its complexity after cross-path
  mismatch. Static decoupling remained the qualified fallback. That decision
  was based on individual servo and load records. A coupled plant can have a
  weak input direction in which two actuator effects nearly cancel, even when
  each diagonal loop looks stable by itself. Separate single-input,
  single-output margins therefore cannot establish robustness of the complete
  two-input, two-output architecture.
- **Theory before practice:** First describe an input direction in physical
  words: it is a chosen proportion of reflux and reboiler movement, independent
  of the overall size of that movement. Describe an output direction as a
  chosen proportion of overhead and bottoms composition movement. Introduce
  the input vector (`u`) as the two valve changes and the output vector (`y`) as
  the two product deviations. Introduce the Euclidean length of a vector as
  its combined magnitude, written `||u||2` or `||y||2`, only after explaining
  that it is the square root of the sum of the two squared components.

  A matrix does not have one gain for every direction. The largest singular
  value (`sigma_max`) is the largest output-vector magnitude produced by any
  unit-length input direction. The smallest singular value (`sigma_min`) is
  the smallest output-vector magnitude produced by any unit-length input
  direction. The corresponding right singular vectors identify the physical
  input directions; the left singular vectors identify the resulting output
  directions. Only after these meanings are established may the lesson write
  `sigma_max = max(||M*u||2)` and `sigma_min = min(||M*u||2)` for all input
  vectors with `||u||2 = 1`.

  Normalize each gain matrix by its diagonal gains before comparing directions.
  The nominal normalized matrix is
  `M0 = [[1, -0.5], [-0.428571, 1]]`. Its largest and smallest singular values
  are `1.4649` and `0.5364`. The interaction case is
  `M1 = [[1, -0.8], [-0.7, 1]]`, with singular values `1.7512` and `0.2512`.
  The condition number (`kappa`) is the largest singular value divided by the
  smallest singular value. It therefore changes from `2.73` to `6.97`.
  Explain in physical words that the smaller value reveals a direction in
  which reflux and reboiler effects nearly cancel, while the larger condition
  number means inverse compensation is more sensitive to model and measurement
  error. The largest possible steady inverse-command amplification is
  `1 / sigma_min`; it changes from `1.86` to `3.98`.
  For a two-by-two matrix form `M^T*M=[[a,b],[b,d]]`, calculate its
  eigenvalues as `((a+d) +/- sqrt((a-d)^2+4*b^2))/2`, then take their square
  roots. Show `a=1.183673,b=-0.928571,d=1.25` for `M0` and
  `a=1.49,b=-1.50,d=1.64` for `M1` so the reported singular values are
  reproducible.

  Interaction uncertainty (`DeltaM`) is the difference between the actual
  normalized gain matrix and the nominal normalized gain matrix. Its largest
  singular value is the largest directional size of that mismatch. For this
  lesson, `DeltaM = M1 - M0` and `sigma_max(DeltaM) = 0.30`. State the useful
  sufficient test before using it: if the uncertainty size is less than the
  nominal smallest singular value, every matrix inside that norm-bounded set
  remains nonsingular. Here `0.30 < 0.5364`, so loss of invertibility is not
  predicted, but good conditioning or closed-loop performance is not thereby
  guaranteed.
  Show `DeltaM=[[0,-0.30],[-0.271429,0]]`; its squared singular values are the
  diagonal entries of `DeltaM^T*DeltaM`, so the largest singular value is
  `0.30`.

  The sensitivity matrix (`S`) maps an output-side disturbance into the
  remaining controlled-output error when feedback is closed. The loop-transfer
  matrix (`L`) combines the dynamic plant and controller in a specified
  architecture. After defining the identity matrix (`I`) as the mapping that
  leaves both vector components unchanged, write `S(s) = inverse(I + L(s))`.
  The frequency variable (`s`) represents sinusoidal frequency in this
  analysis. The maximum singular value of `S` at one frequency is the largest
  error amplification over all disturbance directions at that frequency. A
  multivariable sensitivity peak is therefore searched over both frequency and
  direction; inspecting the two diagonal elements alone is insufficient.

  Define the three implemented candidates precisely. Direct decentralized PI
  uses the two fixed Lesson 62 controllers with no cross mapping. Static
  decoupling applies the fixed nominal inverse from Lesson 62 and limits each
  physical command independently. The centralized constraint coordinator uses
  the same nominal inverse and the same two PI settings, but observes both
  requested physical commands together. If either magnitude would exceed the
  common input limit, it multiplies both commands by one nonnegative scale
  factor (`alpha`) so their requested direction is preserved. Define the
  largest requested magnitude (`u_peak`) before writing
  `alpha = min(1, 10 / u_peak)` and `u_applied = alpha*u_requested`.
  This is a centralized constraint-allocation candidate, not model predictive
  control: it has no prediction horizon, optimizer or future constraint model.

  Architecture selection is lexicographic. First reject invalid, unstable,
  unbounded, unrestored or constraint-violating records. Next require every
  declared normal-envelope record on both plant models to pass its accuracy,
  interaction, sensitivity and actuator-effort limits. Among architectures
  that pass, select the one with the least coupling and state unless a more
  centralized candidate provides a predeclared material benefit. A result on
  an infeasible request cannot be used to claim robustness; the requested
  operating target must instead be rejected or coordinated at a supervisory
  level.
- **Prediction:** Before running, calculate the two normalized singular-value
  pairs, condition numbers, inverse amplification values and uncertainty size
  above using Control Studio. Predict that the stronger-interaction plant will
  have a much weaker common-product direction even though both diagonal gains
  are unchanged. For a simultaneous `+1`-composition-percentage-point target
  on both products, the nominal steady input is `+2.50/+2.50` valve percentage
  points while the stronger-interaction model needs approximately
  `+5.44/+5.24`; both are feasible. For a simultaneous `+2/+2` target, the
  stronger-interaction model needs approximately `+10.88/+10.47`, which lies
  outside the declared input limits. Predict that no feedback architecture can
  remove this physical infeasibility. Predict that independent clipping will
  distort the requested input direction, while the centralized coordinator
  will preserve that direction but still retain product error.
  Derive the stronger-model commands from
  `inverse(G1)=(1/0.2464)*[[0.7,0.64],[0.49,0.8]]`: multiplying by `[1,1]`
  gives `[5.44,5.24]`, and doubling the target gives `[10.88,10.47]`.
- **Controller under test:** The shipped Structured Text shall instantiate
  exactly the same two real `Control.PID` controllers and one
  `Control.StaticDecoupler2x2` used for the static Lesson 62 comparison. An
  architecture selector shall choose direct decentralized PI, independently
  limited static decoupling or the centralized direction-preserving constraint
  coordinator. The program shall expose both errors, virtual PI corrections,
  requested physical commands, common scale factor, applied physical commands,
  saturation state and selected architecture. The selection verdict, singular
  values, sensitivity peak and performance metrics shall be computed from
  retained evidence outside the controller program rather than hard-coded in
  Structured Text.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  Control Studio Monitor, verify `Simulation` evidence,
  column and model identity, both SP/PV pairs, both virtual corrections,
  requested and applied inputs, architecture, common scale, saturation,
  authority, quality, `0.1 s` timing and zero-deviation restoration. In
  **Test**, select **Observe without excitation**, amplitude `0`, and duration
  `720.1 s`. The only writable signal is project experiment authority
  `P1.ExperimentEnabled` at `%MX24.4`. The project selects the centralized
  coordinator, holds zero to `20 s`, applies feasible `+1/+1` to `240 s`,
  restores to zero through `480 s`, then applies infeasible `+2/+2`; it enters
  Manual and restores every event signal to zero when authority clears.

  The native lesson test runs each architecture from a separately restored state on each
  model. Apply an overhead-only `0 -> +1` setpoint, a bottoms-only `0 -> +1`
  setpoint, a simultaneous `0/0 -> +1/+1` setpoint, and the same bounded feed-
  composition disturbance used in Lesson 62. These are the normal-envelope
  records. Then run the simultaneous `+2/+2` stronger-interaction constraint
  challenge as a separate deliberately infeasible record. Keep model, initial
  state, sample interval, event time, duration, limits and disturbance equal
  across architecture comparisons. After every event, return both setpoints
  and the disturbance to zero, transfer both controllers to reviewed Manual
  authority with zero manual outputs, and allow up to `300 s` for the existing
  rate limits and plant dynamics to restore the column. Restoration qualifies
  only when the final `10 s` combined absolute applied input is no greater than
  `0.02` valve percentage points and combined absolute product deviation is no
  greater than `0.02` composition percentage points. A new record shall start
  from freshly initialized controller state. Do not retune from a validation
  result.

  Abort and invalidate a record for nonfinite, stale, forced or bad-quality
  required signals; wrong model, authority, timing or architecture; either
  applied input outside `-10` through `+10`; either output outside `-6` through
  `+6`; missing samples; or failed restoration. A limited but finite
  constraint-challenge record is retained as `Infeasible`, not accepted as a
  normal performance record.
- **Comparison measures:** Define final-window combined absolute error as the
  mean of the sum of the two absolute SP-minus-PV errors over the final `10 s`,
  after both outputs satisfy their settling rule. Define unintended cross peak
  as the largest absolute movement of the output whose setpoint was not changed
  in a single-axis servo test. Define combined integral absolute error as the
  time integral of both absolute errors over the event window. Define applied-
  input total variation as the sum of absolute changes in both applied inputs,
  in valve percentage points. Define saturation duration, minimum common scale,
  restoration error, the singular values and condition number for each matrix,
  and the maximum singular value of sensitivity over `0.002` through `2`
  radians per second. State the calculation, source signals, units, windows,
  validity prerequisites and preferred direction before showing any result.
- **Declared requirements:** Every normal-envelope record shall remain finite,
  inside the physical input/output limits and restore to within `0.02`
  composition percentage points. Both outputs shall settle to a combined
  final-window absolute error no greater than `0.10` composition percentage
  points. A single-axis unintended cross peak shall not exceed `0.20`
  composition percentage points. No normal-envelope record may spend more than
  `0.5 s` at an input limit. The calculated multivariable sensitivity peak
  shall not exceed `1.20` on either model. A more centralized candidate is a
  material improvement only if it turns a failed normal-envelope requirement
  into a pass, or reduces the worst eligible combined error or applied-input
  travel by at least `20 percent` without worsening another declared limit.
  The `+2/+2` constraint challenge is expected to fail final accuracy and shall
  not be included when ranking eligible normal-envelope performance.
- **Validation:** A native executable test shall compile and run the shipped
  Structured Text against both shipped column models, prove that all three
  architecture paths use identical PI settings, execute every normal-envelope
  record and the constraint challenge, and independently calculate every
  selection measure. An independent numerical oracle shall reproduce both
  singular-value pairs within `0.001`, both condition numbers within `0.01`,
  the uncertainty norm within `0.001`, and the sensitivity sweep within the
  declared numerical tolerance. Prove that the centralized common scale stays
  at one when neither command requires limiting, decreases when the constraint
  challenge exceeds a limit, preserves the requested command ratio, and never
  presents an infeasible response as settled accuracy. Select the simplest
  architecture only after every normal-envelope record is eligible; otherwise
  reject the complete set and retain the failed criterion.
- **Explanation:** Trace the weak direction from simultaneous product demands
  through opposing reflux/reboiler effects. Explain why unchanged diagonal
  gains can coexist with a much smaller minimum singular value, why inverse
  compensation magnifies error in that direction, and why a sensitivity matrix
  must be checked as a whole. Relate independent clipping to a changed input
  direction and common scaling to preserved direction but reduced magnitude.
  Neither allocation rule creates actuator capacity that the plant does not
  have.
- **Transfer and required conclusion:** Use the simplest architecture that
  satisfies the complete declared uncertainty set and common requirements.
  Do not infer multivariable robustness from separate loop margins, nominal
  cancellation or one favorable direction. If no candidate can achieve a
  constrained target, reject or reshape the target instead of hiding the
  failure with a more elaborate controller name. Physical-column use requires
  independently reviewed models, topology, product limits, uncertainty set,
  abort/restoration and explicit approval; all Lesson 63 results remain
  simulation evidence. The need to reason about internal modes, sensor and
  actuator placement, and coordinated constraints motivates Lessons 64–68 on
  state space, estimation, optimal control and predictive control.

### 3.11 State-space, estimation, optimal and predictive control

#### Lesson 64 — Controllability and observability: interconnected tanks

- **Objective:** Determine from both matrix calculations and executed two-tank
  evidence whether a proposed pump can move every relevant liquid-level state
  and whether a proposed level sensor can reveal every state. The learner shall
  accept the upstream-pump/downstream-sensor layout, reject deliberately
  deficient actuator and sensor layouts for full-state control or estimation,
  and distinguish controllability/observability from the weaker properties of
  stabilisability/detectability before attempting controller or observer design.
- **Context:** The real-world example is a pilot water-treatment buffer made of
  two interconnected vented tanks. Tank 1 receives the normal inlet-pump flow
  and drains through a connecting restriction into Tank 2. Tank 2 drains to
  the next treatment stage. Level deviations from the qualified operating
  point are measured in centimetres. Pump-flow changes are measured in litres
  per minute. The normal actuator adds flow to Tank 1. A deliberately deficient
  maintenance bypass adds flow directly to Tank 2 and cannot refill Tank 1.
  The normal instrument is the Tank-2 level transmitter. A deliberately
  deficient alternative measures only Tank 1.

  The simulator exposes both true state deviations as Simulation-only truth
  channels so the learner can validate reachability and hidden-state claims.
  The selected measurement presented to an analysis or future observer remains
  separate from those truth channels. A physical controller cannot use an
  uninstalled truth sensor merely because the simulator can display it.

  In the local model, Tank 1 loses `0.05` of its deviation per second, the
  Tank-1 deviation drives Tank 2 at `0.03` per second, and Tank 2 loses `0.04`
  of its deviation per second. One litre-per-minute change through either pump
  path contributes `0.20 centimetres per second` to the tank it directly
  enters. Pump commands are limited to `-2` through `+2 L/min`; level deviations
  are limited to `-10` through `+10 cm`; sampling is `0.1 s`.
- **Reason:** Lesson 63 showed that weak combined input directions and joint
  constraints can invalidate conclusions drawn from separate loops. A state-
  space description makes internal directions and modes explicit, but writing
  state equations does not prove that installed actuators can move every mode
  or that installed sensors contain enough information to reconstruct every
  state. Designing feedback or an observer first would hide a hardware
  architecture defect behind tuning calculations.
- **Theory before practice:** Define a state in physical words as the minimum
  internal quantity needed to predict future plant behavior when future inputs
  are known. Define the state vector (`x`) as the Tank-1 and Tank-2 level
  deviations. Define the input vector (`u`) as the selected pump-flow change.
  Define the measured-output vector (`y`) as the selected level-transmitter
  reading. Define the state matrix (`A`) as the natural level-to-level dynamics,
  the input matrix (`B`) as the way an installed actuator changes state rates,
  and the output matrix (`C`) as the way installed sensors form measurements.
  Only then write the continuous state and measurement relationships
  `dx/dt = A*x + B*u` and `y = C*x`.
  State `A=[[-0.05,0],[0.03,-0.04]]` and explain each row as the Tank-1 loss
  and the one-way transfer plus Tank-2 loss.

  Explain matrix multiplication on this equipment before using powers. The
  first column `B` is the immediate state-rate direction created by the pump.
  The next column `A*B` is the direction that natural tank dynamics create from
  that initial effect. For a two-state, one-input system, the controllability
  matrix (`Qc`) places those columns side by side: `Qc = [B, A*B]`.
  Controllability means that suitable bounded input history can move the state
  from any local initial state to any other local state in finite time within
  the linear model. Matrix rank is the number of independent directions in a
  matrix. Full controllability requires `rank(Qc) = 2`; a nonzero determinant
  is an equivalent two-by-two test, but rank is the general concept.

  Explain observability as the sensor-side dual. The first row `C` states what
  is measured immediately. The next row `C*A` states how natural dynamics make
  another state appear in the measurement rate. For a two-state, one-output
  system, the observability matrix (`Qo`) stacks those rows:
  `Qo = [C; C*A]`. Observability means that known input and a sufficiently rich
  output history uniquely determine the local initial state. Full observability
  requires `rank(Qo) = 2`.

  Define a mode as one independent natural pattern of state motion. Define an
  eigenvalue (`lambda`) as the mode's exponential rate. A negative real
  eigenvalue means that the mode decays in this continuous model; a positive
  value means it grows. The two tank modes are `-0.05` and `-0.04 per second`.
  Stabilisability permits an uncontrollable mode only when that mode is already
  stable. Detectability permits an unobservable mode only when that mode is
  already stable. These weaker properties can make some control or estimation
  tasks safe, but they do not turn a missing actuator or sensor into full-state
  authority or information.

  For the normal upstream pump, define
  `B_upstream = [[0.20], [0]]`. Its controllability matrix is
  `Qc_upstream = [[0.20, -0.010], [0, 0.006]]`; its determinant is `0.0012`
  and rank is two. For the downstream bypass, define
  `B_bypass = [[0], [0.20]]`. Its matrix is
  `Qc_bypass = [[0, 0], [0.20, -0.008]]`, with rank one. The bypass can move
  Tank 2 but cannot move Tank 1.
  Show `A*B_upstream=[-0.010,0.006]^T` and calculate
  `0.20*0.006=0.0012`. Show `A*B_bypass=[0,-0.008]^T`; both bypass columns
  lie on the Tank-2 axis, which is why rank is one.

  For the normal downstream transmitter, define `C_downstream = [[0, 1]]`.
  Its observability matrix is `Qo_downstream = [[0, 1], [0.03, -0.04]]`;
  its determinant is `-0.03` and rank is two. Tank-1 liquid eventually reaches
  Tank 2, so its initial deviation appears in downstream history. For the
  upstream-only transmitter, define `C_upstream = [[1, 0]]`. Its matrix is
  `Qo_upstream = [[1, 0], [-0.05, 0]]`, with rank one. A Tank-2-only initial
  deviation never changes the upstream measurement in this one-way hydraulic
  model.
  Show `C_downstream*A=[0.03,-0.04]`, whose stacked determinant is `-0.03`,
  and `C_upstream*A=[-0.05,0]`, whose two rows contain only Tank-1 information.
  Because `A` is triangular, derive its eigenvalues directly from its diagonal.
- **Prediction:** Before simulation, calculate all four matrices, determinants,
  ranks, and the two stable eigenvalues. For a `+1 L/min` upstream-pump step,
  predict an immediate Tank-1 state-rate increase of `0.20 cm/s` and no
  immediate Tank-2 rate change; Tank 2 shall begin moving only as Tank-1 level
  transfers through the restriction. With the same command routed to the
  downstream bypass, predict Tank 1 remains at zero while Tank 2 responds
  immediately. With an initial `+5 cm` Tank-1 deviation and downstream sensing,
  predict the downstream measurement first rises and then decays, revealing
  the upstream state through history. With an initial `+5 cm` Tank-2 deviation
  and upstream-only sensing, predict a zero selected measurement while the
  Simulation truth channel shows the hidden Tank-2 state decaying.
- **Controller under test:** This lesson deliberately has no feedback
  controller or observer. The shipped Structured Text shall expose a bounded
  manual flow request, route it to either the upstream inlet pump or downstream
  maintenance bypass, select either the downstream or upstream level
  transmitter, and publish the selected measurement separately from both
  Simulation truth levels. The source shall contain no PID instance, observer,
  hard-coded matrix rank, eigenvalue, or accepted-layout verdict. Hardware
  feasibility is calculated from declared `A`, `B`, and `C` matrices and
  verified against the executed plant before design begins.
- **Experiment and exact workflow:** Use only **Monitor** and **Test**. In
  Control Studio Monitor, verify `Simulation` evidence,
  two-tank identity, Manual authority, selected actuator and sensor layouts,
  requested and applied flow, selected measurement, both truth levels, limits,
  good/fresh quality, `0.1 s` timing and zero-flow restoration. Truth levels
  shall be visually marked as simulation validation channels, not installed
  controller inputs.

  In **Test**, select **Observe without excitation**, amplitude `0`, and
  duration `300.1 s`. The only writable signal is project experiment authority
  `P1.ExperimentEnabled` at `%MX32.0`. The project selects the upstream pump and
  downstream transmitter, records a `20 s` zero-flow baseline, applies
  `+1 L/min` through `200 s`, then restores zero flow. The native lesson test
  owns the fresh bypass and two zero-input initial-state records.

  Abort for a nonfinite, stale, forced or bad-quality required signal; wrong
  model, layout, authority or sample time; requested/applied flow outside
  `-2..+2 L/min`; either truth level outside `-10..+10 cm`; a selected
  measurement inconsistent with the chosen sensor; missing samples; or failed
  zero-flow restoration. Keep initial state, command, timing, limits and seed
  equal where actuator layouts are compared.
- **Comparison measures:** Define immediate state-rate change as the first
  one-second least-squares slope after the input event, in centimetres per
  second. Define peak level deviation and time of first detectable response for
  each tank. Define restoration error as the final `10 s` mean absolute truth-
  level sum. Define measurement-to-truth residual for the selected sensor.
  Retain every matrix element, singular value, rank, determinant, eigenvalue,
  controllable/observable status and stabilisable/detectable status with the
  exact actuator and sensor layout that produced it.
- **Declared requirements:** The normal upstream-pump/downstream-sensor layout
  shall have controllability and observability rank two and matrix condition
  number no greater than `50`. The `+1 L/min` upstream step shall produce a
  Tank-1 first-second slope within `0.02 cm/s` of `0.20 cm/s`, a Tank-2 response
  after Tank 1 begins moving, and both finite levels inside bounds. The bypass
  layout shall retain Tank-1 peak magnitude no greater than `0.001 cm` while
  Tank 2 moves by at least `1 cm`, agreeing with controllability rank one. The
  downstream sensor shall show at least `0.10 cm` response to the Tank-1
  initial-state record. The upstream sensor shall remain within `0.001 cm` of
  zero during the Tank-2-only initial-state record while the hidden truth level
  initially exceeds `4.9 cm`, agreeing with observability rank one. Every
  selected measurement shall agree with its corresponding truth channel within
  `0.001 cm`.
- **Validation:** A native executable test shall compile the shipped manual
  routing program, load the shipped two-input/two-truth-output plant, calculate
  all four controllability/observability matrices independently, reproduce the
  declared ranks, determinants, eigenvalues and condition limits, and execute
  all four records. The test shall prove that the normal layout is fully
  controllable and observable, the bypass layout is rank-one but stabilisable,
  and the upstream-only sensor layout is rank-one but detectable because the
  hidden mode is stable. It shall also prove the corresponding physical
  movement or absence of measurement from time-series evidence. An analytical
  rank alone does not complete the lesson when the executable routing or sensor
  selection disagrees.
- **Explanation:** Trace the upstream pump's immediate Tank-1 effect into the
  delayed Tank-2 response and connect those two independent directions to the
  columns of `Qc`. Trace the Tank-1 initial deviation into downstream history
  and connect immediate and rate information to the rows of `Qo`. Explain why
  direct downstream injection cannot move upstream liquid in a one-way model,
  and why an upstream sensor cannot see a downstream-only deviation. Stable
  hidden or unreachable modes justify stabilisability/detectability labels, not
  claims of complete hardware capability.
- **Transfer and required conclusion:** Check controllability and observability
  against the actual actuator and sensor placement before controller or
  observer tuning. Rank is a structural local-model test; poor conditioning,
  uncertainty, noise, delay, saturation and a wrong hydraulic direction can
  still make a full-rank layout impractical. Physical-tank work requires an
  independently reviewed topology, calibrated level instruments and pumps,
  overflow/dry-run limits, isolation and restoration procedure, and explicit
  approval immediately before action. Lesson 64 remains Simulation evidence.
  Lesson 65 next uses a qualified actuator/state layout to place feedback poles
  and a qualified sensor/model layout to estimate an unmeasured motor state.

#### Lesson 65 — State feedback and observer: magnetic bearing and motor load

- **Objective:** Use the actuator and sensor feasibility reasoning from Lesson
  64 to place two closed-loop modes on a magnetic-bearing training rig, then
  reconstruct an unmeasured load-torque state on a motor drive. The learner
  shall calculate the feedback, reference, and observer gains before running;
  verify the resulting modes against executable state and estimation records;
  and reject the claim that an observer output is a measured physical state
  when model mismatch makes the estimate absorb an unmodelled effect.
- **Context:** The first real-world bench is a small active magnetic-bearing
  rotor used for commissioning training. Rotor displacement is measured in
  millimetres and rotor velocity in millimetres per second. A bidirectional
  coil-current deviation is the actuator and is measured in amperes. The local
  linear model is valid only inside `-0.5..+0.5 mm`, velocity must remain inside
  `-2..+2 mm/s`, and applied current is limited to `-2..+2 A`. The zero-current
  state is the physical safe output, but it does not levitate a real rotor;
  independent mechanical touchdown bearings remain required.

  The second bench is a conveyor motor whose encoder measures shaft speed in
  radians per second while a changing load torque is not directly instrumented.
  Drive torque is bidirectional, measured in newton metres, and limited to
  `-3..+3 N m`. Speed is limited to `-2..+2 rad/s`. The simulator exposes true
  load torque only as a clearly labelled Simulation validation channel. The
  observer and feedback law shall receive encoder speed and applied drive
  torque, never the truth channel.

  Both local models use a `0.1 s` control interval and deterministic seed
  `20260829`. They intentionally omit coil electrical dynamics, magnetic-force
  nonlinearity, shaft elasticity, friction nonlinearities, encoder quantisation,
  and drive delay. Their conclusions apply only inside the declared local
  ranges.
- **Reason:** Lesson 64 proved that a model can be structurally controllable and
  observable with the proposed hardware. That result does not choose the
  closed-loop dynamics. Position-only feedback on the unstable magnetic-force
  model can change stiffness but cannot independently set both position and
  velocity modes. Adding measured velocity permits state feedback and pole
  placement. The motor then exposes the next limitation: a useful load state
  is observable from speed history but is not measured, so a feedback law that
  assumes zero load retains speed error. This creates the need for an observer.
- **Theory before practice:** Define the magnetic-bearing state vector (`x_b`)
  as displacement and velocity. Define the bearing input (`u_b`) as applied
  coil-current deviation. Define the bearing state matrix (`A_b`) as
  `[[0, 1], [4, 0]]` and the input matrix (`B_b`) as `[[0], [1]]`. In physical
  words, velocity changes displacement, the local magnetic force drives the
  rotor away from centre at four millimetres per second squared for each
  millimetre of displacement, and one ampere changes acceleration by one
  millimetre per second squared.

  Define the feedback-gain row (`K_b`) as the displacement and velocity weights
  used to oppose the measured state. Define the reference gain (`N_b`) as the
  scale that makes a constant requested displacement produce that displacement
  at model equilibrium. Only then write the requested current law
  `u_b_requested = N_b*r_b - K_b*x_b`, where the bearing reference (`r_b`) is
  the desired displacement. The applied current (`u_b_applied`) is the requested
  current limited to `-2..+2 A`.

  Explain pole placement as choosing the eigenvalues of `A_b - B_b*K_b`, not
  directly drawing a desired trace. For `K_b=[k_x,k_v]`, derive
  `A_b-B_b*K_b=[[0,1],[4-k_x,-k_v]]` and its characteristic polynomial
  `s^2+k_v*s+(k_x-4)`. Position-only gain `k_x=5, k_v=0` therefore produces
  `s^2 + 1 = 0` and the undamped poles `+j` and `-j`; it does not settle.
  Desired poles at `-3` and `-4 per second` expand as
  `(s+3)(s+4)=s^2+7*s+12`. Matching coefficients gives `k_v=7` and
  `k_x-4=12`, hence `K_b = [16, 7]`. Derive the reference gain separately:
  at constant equilibrium, set velocity and acceleration to zero, require
  `x=r_b`, and substitute `u=N_b*r_b-16*x` into `0=4*x+u`; this gives
  `0=(N_b-12)*r_b`, so `N_b=12`. Work the initial `+0.1 mm`
  displacement prediction: the first requested current is `-1.6 A`, which is
  inside the current limit. For a `+0.1 mm` reference from centred conditions,
  the first requested current is `+1.2 A`.

  Define the motor state vector (`x_m`) as shaft speed and opposing load torque.
  Define the measured motor output (`y_m`) as encoder speed. Define the motor
  state matrix (`A_m`) as `[[-1, -1], [0, 0]]`, input matrix (`B_m`) as
  `[[1], [0]]`, and output matrix (`C_m`) as `[[1, 0]]`. In physical words,
  speed naturally decays, positive load torque reduces acceleration, drive
  torque increases acceleration, and load torque is constant during the
  nominal record.

  Define the estimated state (`x_hat`) as the observer's speed and load
  prediction. Define innovation (`nu`) as measured encoder speed minus estimated
  speed. Define observer gain (`L`) as the weights that feed innovation into
  the two state predictions. Only then write
  `dx_hat/dt = A_m*x_hat + B_m*u_m_applied + L*nu` and
  `nu = y_m - C_m*x_hat`. Observer error follows `de/dt = (A_m - L*C_m)*e`.
  For `L=[l_1,l_2]^T`, derive the error polynomial
  `s^2+(1+l_1)*s-l_2`. The desired poles expand as
  `(s+4)(s+5)=s^2+9*s+20`; coefficient matching gives `l_1=8` and
  `l_2=-20`, hence `L=[[8],[-20]]`. The negative load-gain sign follows from
  the model's convention that positive load reduces speed: a measured speed
  below prediction must increase the positive opposing-load estimate. Forward
  Euler maps each continuous pole `lambda` to `z=1+0.1*lambda`, so the poles
  become `0.6` and `0.5`; magnitudes below one mean sampled error decay.

  Define the motor speed-feedback gain (`K_m`) as `2 N m/(rad/s)` and the motor
  reference gain (`N_m`) as `3 N m/(rad/s)`. The requested drive torque is
  `u_m_requested = N_m*r_m - K_m*speed_hat + load_hat`; applied torque is that
  request limited to `-3..+3 N m`. Derive the two equilibria from
  `0=-speed-load+u`. With `r_m=1`, load `0.5`, and `load_hat=0`, substituting
  `u=3-2*speed` gives `0=2.5-3*speed`, hence `0.8333 rad/s`. With a converged
  observer, `load_hat=load=0.5`; the load terms cancel and
  `0=3-3*speed`, hence `1.0000 rad/s`. If real damping is `1.2` while the
  observer retains `1.0`, the missing damping near `1 rad/s` has the same
  encoder effect as another `0.2 N m` of opposing load. This derives the
  predicted `0.7 N m` estimate and `0.2 N m` physical-load error.

  Explain the separation principle: for an exact linear model without active
  limits, feedback poles and observer-error poles can be chosen separately and
  appear together in the output-feedback system. It does not guarantee
  robustness, truth-state accuracy, acceptable noise sensitivity, or behavior
  while saturation changes the assumed dynamics.
- **Prediction:** Before execution, calculate the position-only bearing poles,
  state-feedback gains, reference gain, observer gain, continuous and discrete
  observer-error poles, and the motor's `0.8333/1.0000 rad/s` equilibria. Predict
  that the position-only bearing oscillates without meeting a settling rule,
  while measured-state feedback restores the `+0.1 mm` initial displacement.
  Predict that the nominal observer's load error decays with the declared error
  modes and removes the motor's zero-load-assumption bias. For a separate motor
  whose true speed damping is `1.2` while the observer retains `1.0`, predict
  that the observer can preserve speed regulation by estimating effective
  opposing torque, but its load estimate will exceed the true `0.5 N m` load by
  about `0.2 N m` near `1 rad/s`.
- **Controller under test:** One shipped Lesson-65 project shall execute both
  benches without a generic state-space-controller function block. Its concise
  Structured Text shall expose a bearing architecture selector, calculate
  either position-only or measured-state requested current, apply the common
  current limit, update the motor observer from encoder innovation and the
  previously applied torque, calculate requested torque from the estimated
  states, and apply the drive limit. Design matrices, pole calculations,
  acceptance labels, and expected numerical answers shall remain in Control
  Studio and the independent test oracle, not hard-coded as pass/fail results
  in Structured Text. Comments shall explain the previous-input observer update
  and the sign of the load innovation only where those choices are not obvious.
- **Experiment and exact Control Studio workflow:** The visible workflow shall
  contain exactly `Monitor` followed by `Test`. Monitor shall show Simulation
  evidence, both bench identities, units, sample time, requested and applied
  currents and torques, measured bearing states, encoder speed, estimated
  states, innovation, separately labelled Simulation-only true load, quality,
  limits, and disabled zero-output behavior. The learner shall calculate
  `K_b`, `N_b`, `L`, `K_m`, and `N_m` from the theory before granting authority;
  no disabled Identify, Tune, or Validate screen may be presented as part of
  the lesson workflow or as evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `200.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  runnable project owns the sequence. While authority is true, the project
  shall select measured-state feedback and the nominal observer, regulate the
  initial `+0.1 mm` displacement at zero bearing reference for `80 s`, track a
  `+0.1 mm` bearing reference from `80..160 s`, return the bearing reference to
  zero, and regulate the motor at `1 rad/s` throughout. Removing authority,
  invalid inputs, or a safety abort shall disable both actuators, request and
  apply zero current and torque, restore both references to zero, and reset the
  observer state.

  The fresh position-only, zero-load-assumption, nominal comparison, and
  damping-mismatch records remain native executable-test cases. The visible
  run may be correlated with those retained results, but it shall not be
  described as having secretly executed them.

  Abort for nonfinite, stale, forced or bad-quality required signals; wrong
  model, evidence class, authority, interval, initial state, architecture or
  gains; bearing displacement outside `-0.5..+0.5 mm`; velocity outside
  `-2..+2 mm/s`; speed outside `-2..+2 rad/s`; requested or applied current
  outside `-2..+2 A`; torque outside `-3..+3 N m`; selected truth used as a
  controller input; or failure to reach the disabled zero-output state.
- **Comparison measures:** Define bearing settling as the first time after
  which absolute displacement error stays at or below `0.002 mm` and absolute
  velocity stays at or below `0.005 mm/s` for `2 s`. A record that never meets
  both conditions is `Not settled`. Define final bearing error as the final
  `2 s` mean absolute displacement error only for a settled record. Define peak
  displacement, peak absolute applied current, time at a current limit, and
  cumulative applied-current movement in amperes.

  Define motor final speed error as the final `5 s` mean absolute difference
  between reference and encoder speed. Define nominal load-estimation error as
  the final `5 s` mean absolute difference between estimated and Simulation-
  truth load. Define estimation settling as the first time after which absolute
  speed-estimation error is at most `0.01 rad/s` and absolute load-estimation
  error is at most `0.02 N m` for `2 s`. Define peak innovation, peak applied
  torque, time at a torque limit, and cumulative applied-torque movement. The
  mismatch load-estimation error is a model-validity diagnostic, not a reason
  to expose truth load to the controller.
- **Declared requirements:** All records shall remain finite and inside their
  physical ranges. Position-only bearing feedback shall be classified `Not
  settled`; it is a causal baseline, not an eligible final design. Measured-
  state feedback shall settle within `4 s`, have final displacement error at
  most `0.002 mm`, and avoid sustained current limiting. The reference record
  shall settle within `4 s` with final error at most `0.002 mm`.

  The zero-load motor baseline shall retain final speed error of at least
  `0.10 rad/s`. The nominal observer record shall settle its estimates within
  `3 s`, have final speed error at most `0.02 rad/s`, final load-estimation error
  at most `0.02 N m`, and no sustained torque limiting. The mismatched record
  shall remain bounded but show final load-estimation error of at least
  `0.10 N m`, proving that successful output regulation does not make the
  estimated physical load true.
- **Validation:** A native executable test shall compile and execute the shipped
  Structured Text with the nominal bearing, nominal motor, and mismatched motor
  simulations. An independent numerical oracle shall reproduce the open-loop,
  position-only, state-feedback, and observer poles and every declared gain.
  The test shall calculate all time measures from synchronized requested,
  applied, measured, estimated, and Simulation-truth records; prove the
  requested/applied limits and disabled outputs; prove that ST never reads the
  load-truth binding; and reject any result whose settling prerequisite is not
  met. A rendered Control Studio journey shall show the pole design, both
  bearing traces, all three motor traces, estimate-versus-truth and innovation
  plots, constraint state, and the explicit Simulation evidence class.
- **Explanation:** Trace velocity feedback into the damping coefficient of the
  bearing characteristic polynomial and explain why position gain alone cannot
  independently select both modes. Trace the motor's speed innovation through
  both observer corrections, including why the load correction has a negative
  sign. Explain the nominal convergence with the observer-error poles. Then
  show how extra physical damping produces the same encoder effect as extra
  opposing load in the retained model, so the observer estimates an effective
  model state rather than measuring physical torque.
- **Transfer and required conclusion:** State feedback is justified only after
  actuator/state feasibility and model validity have been established.
  Observers reconstruct states of a declared model from input and output
  history; they do not create sensors or truth. The separation principle is a
  nominal linear result and must be followed by limit, noise, delay, and model-
  mismatch validation. Physical magnetic-bearing or motor experiments require
  independently reviewed topology, calibrated scaling, mechanical containment,
  current/overspeed protection, immediate approval, abort and restoration; all
  Lesson-65 results remain Simulation evidence. The deterministic observer's
  inability to express relative process and measurement uncertainty creates the
  reason for Lesson 66's weigh-belt Kalman filter.

#### Lesson 66 — Kalman filtering: weigh-belt estimator

- **Objective:** Estimate mass flow on a weigh-belt feeder from a noisy load-cell
  signal, choose process and measurement covariance assumptions from declared
  evidence, and diagnose a biased or statistically inconsistent estimator from
  its innovation record. The learner shall compare a fixed low-pass baseline,
  a qualified scalar Kalman filter, sensor-overtrust and sensor-distrust cases,
  and a load-cell-bias fault without treating a smoother trace as proof of a
  better estimate.
- **Context:** The real-world example is a weigh-belt feeder used to meter dry
  material into a blending process. Belt-speed command is measured in percent.
  Noise-free mass-flow truth and noisy load-cell measurement are measured in
  kilograms per second. The local feeder response has gain
  `1 (kg/s)/percent` and time constant `1 s`; command is limited to `0..20
  percent`, mass flow to `0..20 kg/s`, and sampling is `0.1 s`.

  Two equivalent simulator copies receive the same applied command. One
  publishes noise-free Simulation truth only for validation. The other adds
  deterministic approximate-white measurement noise with zero mean and
  standard deviation `sqrt(3) = 1.73205 kg/s`, using seed `20260829`. A separate
  fault configuration adds a `1.0 kg/s` measurement bias. Neither truth nor
  configured noise parameters are available as physical controller inputs.
  The model omits belt transport delay, load-cell dynamics, material impacts,
  bias drift, colored noise, non-Gaussian outliers, slip and changing material
  density.
- **Reason:** Lesson 65 used a deterministic observer and showed that model
  mismatch can be absorbed into an apparently plausible state estimate. That
  observer gain did not state how much uncertainty belongs to plant prediction
  and how much belongs to the sensor. A weigh-belt load cell makes this tradeoff
  unavoidable: trusting every noisy sample creates estimate movement, while
  trusting the model too strongly delays detection of real material-flow
  change. Covariance gives the assumptions physical and numerical meaning, and
  innovation provides evidence with which to challenge them.
- **Theory before practice:** Define a random variable as a quantity whose next
  value is not known exactly before observation. Define mean as the expected
  centre of repeated values. Define variance as the expected squared departure
  from that mean, and covariance as the corresponding joint uncertainty between
  states. The scalar lesson has one state, so covariance is one nonnegative
  variance in squared kilograms per squared second. Standard deviation is the
  square root of variance; `1.73205^2` is approximately `3`.

  Define the previous corrected mass-flow estimate (`x_post_previous`) and its
  corrected covariance (`P_post_previous`). Define the applied belt command
  from the preceding interval (`u_previous`). Start from the continuous feeder
  equation `dx/dt=-(x-u)/tau`, with `tau=1 s`. For a command held constant over
  one `0.1 s` sample, solve that equation to obtain
  `x_k=exp(-dt/tau)*x_previous+(1-exp(-dt/tau))*u_previous`. Thus the exact
  discrete feeder coefficient (`a`) is `exp(-0.1/1) = 0.9048374180`; the command
  coefficient (`b`) is `1-a = 0.0951625820`. Their sum is one because the state
  moves partway from its old value toward the constant-command equilibrium.
  Define process-noise variance (`Q`) as the uncertainty added per sample for
  unmodelled process behavior. The prediction is:

  `x_prior = a*x_post_previous + b*u_previous`

  `P_prior = a^2*P_post_previous + Q`

  Explain that covariance uses `a^2`: propagation multiplies state error by
  `a`, so it multiplies squared error by `a^2`.

  Define load-cell measurement (`z`) as the noisy observed mass flow. Define
  measurement-noise variance (`R`) as the assumed variance of measurement error.
  Define innovation (`nu`) as measurement minus predicted estimate. Define
  innovation variance (`S`) as the predicted uncertainty of that difference.
  Define Kalman gain (`K`) as the fraction of innovation accepted by the
  correction:

  `nu = z - x_prior`

  `S = P_prior + R`

  `K = P_prior / S`

  `x_post = x_prior + K*nu`

  `P_post = (1-K)*P_prior`

  Define normalized innovation squared (`NIS`) as `nu^2/S`. Its units cancel
  because both terms are squared mass-flow units. Under the exact
  scalar model and declared zero-mean independent Gaussian assumptions, its
  long-record mean is expected near one. A persistent nonzero innovation mean,
  excessive normalized innovation, or correlated successive innovations is a
  model/noise-assumption diagnostic. A short record, approximate-white
  generator, saturation, and closed-loop correlation limit statistical claims;
  the lesson uses declared engineering bands rather than pretending to perform
  a universal hypothesis test.

  The fixed-filter coefficient (`alpha`) is `0.2`. Its baseline update is
  `x_simple = x_simple_previous + alpha*(z-x_simple_previous)`. It has no model
  input or covariance and therefore cannot explain why its fixed weighting is
  appropriate. For `P_post_previous=1.9`, `Q=0.1`, and `R=3`, calculate
  `P_prior=0.904837418^2*1.9+0.1=1.65559`, then
  `S=1.65559+3=4.65559`, `K=1.65559/4.65559=0.35561`, and
  `P_post=(1-0.35561)*1.65559=1.06684`. An innovation of `5 kg/s` corrects the
  Kalman estimate by `0.35561*5=1.77807 kg/s`; the fixed filter corrects by
  `0.2*5=1.00000 kg/s`. Reducing `R` increases `K=P_prior/(P_prior+R)` because
  the denominator falls; increasing `Q` first raises `P_prior` and therefore
  also raises the fraction accepted from the measurement.
- **Prediction:** Before running, calculate the complete first correction and
  predict the direction of every covariance variation. Reducing `R` from `3`
  to `0.25 (kg/s)^2` shall increase gain and measurement-noise sensitivity.
  Increasing `R` to `12 (kg/s)^2` shall reduce gain and delay response to a real
  command step. Increasing `Q` shall increase gain because the model prediction
  is declared less certain. With a `1 kg/s` sensor bias absent from the model,
  predict persistent innovation inconsistency and an estimate biased away from
  noise-free truth; the Kalman filter shall not invent a bias state.
- **Controller under test:** The shipped Structured Text shall calculate the
  fixed and Kalman estimates from the same measurement in one scan, use the
  previously applied belt command in prediction, expose prior estimate,
  innovation, innovation variance, gain, corrected covariance and normalized
  innovation squared, and select either estimate for downstream display. It
  shall bound requested belt command to `0..20 percent`. Invalid `Q<0`, `R<=0`,
  nonpositive innovation variance, or nonfinite covariance shall set an
  explicit invalid state, retain finite estimator state, and select the fixed
  baseline rather than divide by an invalid value. The ST shall contain no
  configured truth read, random-number generator, precomputed result table or
  acceptance verdict.
- **Experiment and exact Control Studio workflow:** The visible workflow shall
  contain exactly `Monitor` followed by `Test`. Monitor shall show Simulation
  evidence, both equivalent feeder identities, `0.1 s` timing, deterministic
  seed, requested/applied command, noisy measurement, separately labelled
  Simulation-only truth, both estimates, selected estimate, `Q`, `R`,
  covariance, gain, innovation, innovation variance, normalized innovation
  squared, validity, units, ranges, and the zero-command disabled state. The
  learner shall derive the model and first covariance correction before
  granting authority. No disabled Identify, Tune, or Validate screen may be
  presented as a workflow stage or as evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `30.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  runnable project owns the event. While authority is true, the project shall
  use nominal `Q=0.1`, `R=3`, select the Kalman estimate, enable the drive,
  request `5 percent` for `5 s`, then `10 percent` until `30 s`, then zero.
  Removing authority, invalid inputs, or a safety abort shall disable the drive
  and restore requested and applied commands to zero.

  The fixed-filter, `R=0.25`, `R=12`, `1 kg/s` bias-fault, and invalid-`Q`
  comparisons remain native executable-test records from identical initial
  states and seeds. The visible nominal run may be correlated with their
  retained evidence but shall not be described as having executed them.

  Abort for wrong model, seed, evidence class, interval, initial state, command
  event, estimator initialization or covariance; invalid, stale, forced,
  missing or nonfinite required signals; command outside `0..20 percent`; truth
  outside `0..20 kg/s`; negative corrected covariance; nonpositive innovation
  variance while valid; truth used by ST; or failed disabled output.
- **Comparison measures:** Define root-mean-square estimation error as the square
  root of the mean squared estimate-minus-truth difference over `5..30 s`, in
  kilograms per second. Persistent measurement noise means that requiring every
  sample to remain inside a narrow settling band would confuse noise with trend
  response. Instead, define two-second mean 90-percent response time as the
  first `20`-sample window after the `5 -> 10 percent` command step whose
  arithmetic-mean estimate is at least `9.5 kg/s`. It is valid only when the
  requested and applied command steps agree and all samples in the window are
  finite; report `Not reached` otherwise. Define estimate movement as total
  variation, the sum of absolute changes between consecutive estimate samples,
  in kilograms per second. Define innovation mean, innovation root-mean-square
  value, mean normalized innovation squared, and lag-one innovation correlation
  over `5..30 s`. Report those diagnostics only for a valid finite estimator
  and retain the sample count, window, `Q/R`, seed, and evidence class.
- **Declared requirements:** The first nominal correction shall reproduce
  `P_prior=1.65559`, `K=0.35561`, `P_post=1.06684`, and a `1.77807 kg/s`
  correction for a `5 kg/s` innovation within declared numerical tolerances.
  Every valid covariance and innovation variance shall remain finite and
  nonnegative/positive respectively. The nominal Kalman record shall have lower
  root-mean-square truth error than the fixed filter and shall reach the defined
  two-second mean 90-percent response no later than the fixed filter, without
  greater estimate movement than the sensor-overtrust case.

  The `R=0.25` case shall have higher average gain and at least `20 percent`
  greater estimate movement than nominal. The `R=12` case shall have lower
  average gain and no earlier two-second mean 90-percent response than nominal.
  Nominal innovation mean shall remain within `0.25 kg/s`, mean normalized
  innovation squared within `0.4..1.6`, and absolute lag-one innovation
  correlation no greater than `0.30`. The bias case shall show absolute
  innovation mean at least
  `0.20 kg/s` or mean normalized innovation squared above `1.6`, and final
  estimate bias at least `0.50 kg/s`. Invalid covariance input shall make the
  estimator invalid, keep all outputs finite, select the fixed estimate, and
  never alter the Simulation-truth boundary.
- **Validation:** A native executable test shall compile the shipped Structured
  Text, load the nominal and biased two-copy feeder simulations, reproduce the
  exact discrete model and first covariance correction with an independent
  oracle, and execute every covariance and fault record from identical seeds
  and events. It shall calculate all comparison measures from synchronized
  noisy, truth, estimate, covariance, innovation and command samples; prove
  requested/applied limits and disabled output; and prove ST does not read the
  truth binding. A rendered Control Studio journey shall show the same-scale
  truth/measurement/estimate plot, gain and covariance, innovation and
  normalized-innovation plots, comparison table, invalid fallback and explicit
  Simulation classification.
- **Explanation:** Trace the previous command through model prediction, then
  trace measurement surprise through innovation variance and Kalman gain into
  correction. Explain why smaller `R` moves the estimate more, why larger `R`
  produces lag, and why `Q` and `R` are engineering assumptions that require
  independent data rather than tuning by visual smoothness. Use the biased
  record to show that a filter lacking a bias state cannot distinguish sensor
  bias from plant/model error merely because its covariance remains finite.
- **Transfer and required conclusion:** A Kalman filter is a model-based
  uncertainty-weighted observer, not a generic smoothing control and not a
  truth generator. Physical deployment requires separately qualified process-
  model residuals and stationary sensor records, checked innovation statistics,
  calibrated units, reviewed timing and fallback, and explicit approval before
  affecting a plant. All Lesson-66 results remain Simulation evidence. The
  need to trade several state errors against actuator effort creates Lesson
  67's active-suspension LQR/LQG design problem.

#### Lesson 67 — LQR and LQG: active vehicle suspension

- **Objective:** Design and qualify a sampled active-suspension regulator that
  makes its trade among suspension travel, body motion and hydraulic effort
  explicit. The learner shall derive several linear-quadratic regulator gains
  from declared costs, select an eligible compromise from executable evidence,
  then replace the full-state measurements with a position-only Kalman estimate
  and determine what the resulting linear-quadratic-Gaussian controller does
  and does not guarantee.
- **Context:** The real-world example is the locally linear body-bounce mode of
  one corner of an active road vehicle. Suspension deflection (`z`) is measured
  in metres, body vertical velocity (`v`) in metres per second, hydraulic force
  (`u`) in kilonewtons, and the road-equivalent vertical acceleration
  disturbance (`d`) in metres per squared second. Positive force increases
  positive body acceleration. The local continuous model is explained in words
  as velocity changing deflection and spring, damper, actuator and road forces
  changing velocity. Only after those meanings are established shall it be
  written as:

  `z_dot = v`

  `v_dot = -4*z - 1.2*v + 0.5*u + d`

  Therefore state vector (`x`) is `[z, v]^T`, state matrix (`A`) is
  `[[0,1],[-4,-1.2]]`, actuator matrix (`B`) is `[[0],[0.5]]`, and disturbance
  matrix (`E`) is `[[0],[1]]`. The force request and applied force are separate;
  the actuator is bidirectional and limited to `-5..5 kN`. The declared local
  range is `abs(z)<=0.08 m`, `abs(v)<=0.50 m/s`, and `abs(d)<=2 m/s^2` at a
  `0.1 s` controller interval.

  This reduced model is deliberately not a full tyre, unsprung-mass, hydraulic,
  chassis-pitch or chassis-roll model. It is suitable for learning the design
  workflow and for local Simulation evidence only. It cannot support a physical
  vehicle claim without identified corner parameters, actuator dynamics, road
  data, delays, structural modes and an independently reviewed safety case.
- **Reason:** Lesson 65 showed that pole placement can assign a desired pole set,
  but its magnetic-bearing gain did not express an engineering preference among
  several state errors and actuator effort. Lesson 66 then showed how missing
  state information can be estimated with explicit uncertainty assumptions.
  Active suspension requires both ideas at once: many stabilizing pole sets are
  possible, yet the engineer must justify which compromise is wanted and must
  still control when only a noisy displacement measurement is available.
- **Theory before practice:** Define a state penalty as the cost assigned to
  state departure and an input penalty as the cost assigned to actuator effort.
  Define state-weight matrix (`Q`) and input-weight matrix (`R`) only after the
  learner understands those physical penalties and their units. Define the
  quadratic performance cost (`J`) as accumulated state and actuator penalty:

  `J = integral(x^T*Q*x + u^T*R*u) dt`

  Explain that raw numerical weights depend on engineering units. Before any
  candidate is designed, normalize deflection by `0.05 m`, velocity by
  `0.25 m/s`, and force by `5 kN`; then show how a declared normalized
  preference maps back to dimensional `Q` and `R`. Specifically, expanding
  `q_z*(z/0.05)^2+q_v*(v/0.25)^2+r_u*(u/5)^2` gives dimensional weights
  `Q_zz=q_z/0.05^2`, `Q_vv=q_v/0.25^2`, and `R=r_u/5^2`. Multiplying a signal's
  unit scale without transforming its weight changes the design and is
  therefore a configuration error, not harmless formatting.

  Define the positive-semidefinite Riccati matrix (`P`) as the matrix that
  relates present state to future optimal cost. After defining every matrix and
  confirming that the plant is stabilizable, introduce the continuous
  algebraic Riccati equation and the linear-quadratic regulator gain (`K`):

  `A^T*P + P*A - P*B*R^-1*B^T*P + Q = 0`

  `K = R^-1*B^T*P`

  `u_requested = -K*x`

  Explain how the gain is obtained rather than presenting it as a table lookup:
  solve the Riccati equation for symmetric `P`, then use the gain equation.
  Since `B=[0,0.5]^T`, it reduces to
  `K=[0.5*P_12/R,0.5*P_22/R]`. For the balanced design the numerical solution
  has `P_12=4.39607806` and `P_22=3.81444278`; with `R=1` this produces
  `K=[2.19803903,1.90722139]`. Substitution into the Riccati equation and stable
  eigenvalues of `A-B*K` independently verify the solution.

  State that this optimum applies to the declared linear model, quadratic cost,
  unconstrained calculation and complete state. Saturation breaks the
  unconstrained optimality result. A lower value of one candidate's own `J`
  cannot rank candidates that were designed and evaluated with different cost
  definitions; common physical measures are required for comparison.

  The independently qualified continuous designs shall be:

  | Candidate | Q diagonal | R | K | Intended emphasis |
  |---|---:|---:|---:|---|
  | Body-motion | `[20,20]` | `1` | `[1.16515139,3.11548779]` | penalize vertical velocity |
  | Suspension-travel | `[120,2]` | `1` | `[5.56465997,3.07892689]` | penalize deflection |
  | Balanced | `[40,4]` | `1` | `[2.19803903,1.90722139]` | retain both state penalties |
  | Effort-saving | `[40,4]` | `5` | `[0.48528137,0.51566896]` | penalize hydraulic force |

  A pole-placement baseline uses `K=[13,6]`. It is a causal comparison, not an
  LQR candidate. The native oracle shall solve the Riccati equation rather than
  merely compare the shipped gain literals.

  Define the sampled state-transition matrix (`A_d`), actuator matrix (`B_d`)
  and disturbance matrix (`E_d`) as the exact zero-order-hold discretisation for
  `0.1 s`. Derive them from `A_d=exp(A*dt)`,
  `B_d=integral_0^dt exp(A*tau)*B d tau`, and the corresponding integral with
  `E`. The exponential advances unforced state; each integral accumulates one
  input held constant through the sample. Their declared values are:

  `A_d = [[0.98084004,0.09360616],[-0.37442462,0.86851265]]`

  `B_d = [[0.00239499],[0.04680308]]`

  `E_d = [[0.00478999],[0.09360616]]`

  For the position-only case, define measured position (`y`), predicted state
  estimate (`x_prior`), corrected state estimate (`x_post`), innovation (`nu`),
  steady-state estimator gain (`L`), process-noise covariance (`W`) and
  measurement-noise covariance (`V`) in words before matrix equations. With
  `W=diag(1e-7,1e-4)` and `V=4e-6 m^2`, an independent discrete Riccati oracle
  shall produce `L=[0.58488259,2.24682847]^T`. The shipped steady-state Kalman
  estimator then uses:

  `x_prior = A_d*x_post_previous + B_d*u_previous + E_d*d_previous`

  `nu = y - [1,0]*x_prior`

  `x_post = x_prior + L*nu`

  The linear-quadratic-Gaussian controller uses the balanced regulator gain on
  the corrected estimate: `u_requested = -K_balanced*x_post`. Explain the
  separation principle as nominal closed-loop pole separation for the exact
  linear model. It does not create robustness to wrong dynamics, covariance,
  delay, saturation or non-Gaussian faults.
- **Prediction:** Before simulation, calculate requested force for the common
  worked state `z=0.03 m`, `v=0`: pole placement requests `-0.39000 kN`, while
  the body-motion, suspension-travel, balanced and effort-saving LQR candidates
  request `-0.03495`, `-0.16694`, `-0.06594`, and `-0.01456 kN` respectively.
  Predict that travel emphasis reduces deflection at an effort cost, effort
  emphasis reduces force energy at a motion cost, and no candidate can be
  called universally optimal. Predict that the LQG run approaches the balanced
  full-state result only while the estimator assumptions remain credible.
- **Controller under test:** One shipped Structured Text project shall expose a
  bounded design selector for the pole-placement baseline, all four LQR
  candidates, and balanced LQG. It shall calculate the same requested/applied
  force distinction in every mode, publish the road disturbance separately,
  use measured full state only in full-state modes, and use only noisy position
  plus previous applied inputs in LQG mode. Simulation-truth velocity may be
  bound for validation but shall not appear in the LQG estimator calculation.
  Invalid selector, nonfinite/out-of-range input, estimator-invalid state or
  covariance mismatch shall force zero output and an explicit invalid status.
  Disabled mode shall apply zero hydraulic force.
- **Experiment and exact Control Studio workflow:** Use two equivalent model copies receiving identical applied
  hydraulic force and road disturbance. The validation copy publishes clean
  deflection and velocity as Simulation truth. The position-only copy publishes
  deflection with deterministic zero-mean `0.002 m` standard-deviation
  measurement noise using seed `20260829`; its velocity remains unavailable to
  the LQG calculation. Start at zero state. Apply zero road acceleration for
  `1 s`, `+0.5 m/s^2` from `1.0..1.5 s`, then zero through `12 s`.

  The visible workflow shall contain exactly `Monitor` followed by `Test`. In
  Monitor, verify Simulation classification, plant identities, `0.1 s`
  timing, selector, requested/applied force, road event, clean validation state,
  noisy position, estimated state, innovation, saturation, local-range state,
  units, quality and disabled zero output. Before granting authority, inspect
  normalized preferences, dimensional `Q/R`, independently solved gains and
  closed-loop poles; no disabled Identify, Tune, or Validate screen may be
  presented as a workflow stage or as evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `12.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  project owns the event. While authority is true, the project shall select
  balanced position-only LQG, apply zero road acceleration for `1 s`, apply
  `+0.5 m/s^2` from `1.0..1.5 s`, then restore it to zero. Removing authority,
  invalid inputs, or a safety abort shall disable hydraulic control and restore
  requested and applied road acceleration and applied force to zero.

  Pole placement, all four full-state LQR candidates, balanced full-state LQR,
  doubled position noise, and model mismatch remain native executable-test
  records from identical conditions. The visible nominal LQG run may be
  correlated with those results but shall not be described as executing them.

  Abort for wrong model, design constants, sample interval, initial state,
  event, seed, evidence class or estimator initialization; invalid, stale,
  forced, missing or nonfinite signals; force outside `-5..5 kN`; departure from
  the local state/disturbance range; undeclared saturation; truth velocity used
  by LQG; or failed disabled output and restoration.
- **Comparison measures:** Define suspension-deflection root-mean-square value
  and body-velocity root-mean-square value over `1..12 s` in metres and metres
  per second. Define body-acceleration root-mean-square value from the declared
  state equation over the same window, in metres per squared second. Define
  hydraulic-force energy as `sum(u_applied^2*0.1 s)` in squared kilonewton
  seconds, force movement as total variation in kilonewtons, peak absolute
  force in kilonewtons, saturation duration in seconds, and return time as the
  first point after the road pulse for which the next continuous second remains
  inside `abs(z)<=0.002 m` and `abs(v)<=0.01 m/s`. Report `Not returned` when
  that validity rule is not met. For LQG, additionally define position- and
  velocity-estimation root-mean-square error against Simulation truth and
  innovation mean over `1..12 s`. Every value shall retain its source signals,
  window, units, sample count, selector, model, seed and evidence class.
- **Declared requirements:** The independent Riccati oracles shall reproduce all
  four regulator gains and the estimator gain within declared numerical
  tolerances, and shall show every selected regulator pole and estimator pole
  strictly inside its continuous/discrete stability boundary. Every executable
  record shall remain finite, inside the local model range and force limit, and
  shall return or truthfully report `Not returned`.

  The suspension-travel candidate shall have lower deflection RMS than the
  effort-saving candidate. The body-motion candidate shall have lower velocity
  RMS than the effort-saving candidate. The effort-saving candidate shall have
  lower force energy than the travel and body-motion candidates. A balanced
  selection is eligible only if it avoids saturation and lies on the measured
  common-metric tradeoff rather than winning only its private quadratic cost.
  Nominal LQG shall keep finite estimates, position-estimation root-mean-square
  error at most `0.01 m`, velocity-estimation root-mean-square error at most
  `0.10 m/s`, and absolute innovation mean at most `0.002 m`. Its deflection
  root-mean-square value shall be no more than `1.5` times the balanced
  full-state value and its force energy no more than `2.0` times the balanced
  value. Force movement remains a separately reported actuator-wear measure;
  good motion and energy do not make increased movement disappear. The
  doubled-noise record shall have no lower position-estimation error than
  nominal, and the model-mismatch record shall have no lower velocity-
  estimation error than nominal. Both abnormal records shall be classified
  separately rather than silently admitted to nominal evidence.
- **Validation:** A native executable test shall compile the shipped Structured
  Text, independently solve the continuous and discrete Riccati equations,
  confirm gains and stability, execute all full-state and LQG records through
  the shipped simulator, calculate every reported measure from synchronized
  samples, and prove force limiting, local-range enforcement, estimator signal
  ownership, invalid-selector behavior and disabled output. A rendered Control
  Studio journey shall show normalized weights beside dimensional `Q/R`, pole
  and estimator evidence, common-scale response/force plots, an honest Pareto
  comparison, LQG estimation/innovation plots, abnormal-case disposition and
  explicit Simulation classification.
- **Explanation:** Relate each measured tradeoff to the corresponding weight,
  not merely to a gain magnitude. Explain why unit scaling changes raw weights,
  why a Riccati solution is meaningful only for the declared model and cost,
  why force saturation invalidates the unconstrained optimum, and how estimator
  error enters the LQG force request. A smooth position estimate is not evidence
  that hidden velocity is correct; synchronized truth is available only because
  this is a simulator qualification exercise.
- **Transfer and required conclusion:** Linear-quadratic regulation makes a
  multi-state/effort preference explicit and reproducible; it does not discover
  the engineering preference. Linear-quadratic-Gaussian control combines that
  regulator with a model-qualified estimator and is nominally optimal only for
  the declared linear stochastic model, weights and covariance. Physical use
  requires identified dynamics and noise, hydraulic and structural limits,
  independent topology/safety review, explicit approval and rollback. All
  Lesson-67 results remain Simulation evidence. Hard multivariable constraints
  and future disturbance/price decisions create Lesson 68's building-HVAC
  model-predictive-control problem.

#### Lesson 68 — Constrained Model Predictive Control: building HVAC

- **Objective:** Operate two thermally coupled building zones through forecast
  weather, occupancy and tariff changes while respecting zone, heater, heater-
  rate and shared-power constraints. The learner shall formulate the prediction
  problem, prove that a real optimizer executes every control interval, compare
  it with a qualified two-zone PI baseline, and distinguish feasible optimum,
  infeasibility, invalid solver configuration and deterministic fallback.
- **Context:** The real-world example is a small office floor with two adjacent
  hydronic heating zones. Zone-one temperature (`T1`) and zone-two temperature
  (`T2`) are measured in degrees Celsius. Zone-one and zone-two heater commands
  (`u1`, `u2`) are measured in percent. Outdoor-air forecast (`To`) and the two
  occupancy heat-gain forecasts (`q1`, `q2`) are expressed as equivalent degrees
  Celsius so their physical effect remains visible. Electricity tariff (`p`) is
  a relative cost per unit heating energy.

  Heat is lost from each zone to outdoor air, exchanged between zones, added by
  its heater, and added by occupancy. The continuous local model uses a
  `600 s` outdoor-loss time constant, `1800 s` inter-zone coupling time constant
  and heater coefficient `1/3000 degC/(percent*s)`. Only after these physical
  meanings are established shall the two-state matrix model be introduced.
  Derive, for example,
  `dT1/dt=(To-T1)/600+(T2-T1)/1800+u1/3000+q1/600`, with the zone indices
  exchanged for Zone 2. At `T1=T2=21`, `To=5`, `q1=0`, and `u1=80`, outdoor
  loss is `(5-21)/600=-0.026667 degC/s`, neighbour exchange is zero, and heater
  gain is `80/3000=+0.026667 degC/s`; this derives the initial equilibrium.

  Exact zero-order-hold sampling at `60 s` uses `A_d=exp(A*60)` and
  `B_d=integral_0^60 exp(A*tau)*B d tau`; the first advances stored thermal
  state and the second accumulates inputs held constant for the minute. It
  produces:

  `A_d = [[0.875659571463,0.029177846573],`
  `       [0.029177846573,0.875659571463]]`

  For input vector `[u1,u2,To,q1,q2]^T`, sampled input matrix (`B_d`) is:

  `B_d = [[0.018727354703,0.000305161690,0.095162581964,0.093636773515,0.001525808449],`
  `       [0.000305161690,0.018727354703,0.095162581964,0.001525808449,0.093636773515]]`

  Each heater is limited to `0..100 percent`, each one-minute change to
  `-20..20 percentage points`, and their shared electrical limit to `170
  percent`. The nominal comfort envelope is `20.0..22.5 degC`. The declared
  local range is `15..27 degC`, outside temperature `-10..15 degC`, and each
  occupancy-equivalent gain `0..4 degC`.
- **Reason:** Lesson 67 could reject a control request only after an
  unconstrained LQR calculation encountered the force limit. A building must
  coordinate two heaters under a shared capacity limit, predict slow
  temperature motion, and respond before a known weather, occupancy or tariff
  change arrives. A reactive PI loop cannot choose several future moves or
  enforce future coupled constraints, while a precomputed affine command is not
  evidence that an optimization ran.
- **Theory before practice:** Define an optimization problem as a declared set
  of choices, an objective to minimize and constraints every eligible choice
  must satisfy. Define decision variables as the future heater moves. Define
  prediction horizon (`N=3`) as the next three one-minute intervals and control
  move (`Delta u`) as change from the preceding heater command.

  The executable lesson uses the finite move set `{-20,0,+20}` percentage
  points for each heater at each horizon position. Two heaters over three
  positions create `2*3=6` decisions, each with three alternatives, so the
  multiplication rule gives `3^6=729` sequences. The solver shall enumerate all 729 sequences every
  interval, propagate the sampled two-zone model, reject candidates violating
  individual power, shared power or predicted comfort constraints, and retain
  the feasible candidate with least objective. This is a genuine online finite-
  grid constrained optimizer. It is not a continuous quadratic-program solver;
  its answer is optimal only on the declared move grid.

  Define zone tracking error (`e_i,j`) as predicted zone temperature minus its
  setpoint for zone `i` at horizon position `j`; squaring treats positive and
  negative errors alike while penalizing larger misses more strongly. Define
  tariff-weighted energy as forecast price times total heat, and define move
  effort as squared heater change so grid-point chatter costs more. After defining temperature
  weight (`w_T`), energy weight (`w_E`) and move weight (`w_Delta`), write:

  `J = sum(j=1..3, w_T*(e_1,j^2 + e_2,j^2)`
  `    + w_E*p_j*(u_1,j + u_2,j)`
  `    + w_Delta*(Delta u_1,j^2 + Delta u_2,j^2))`

  The nominal weights are `w_T=100`, `w_E=0.01` and `w_Delta=0.0025`. All terms
  and scale consequences shall be explained before tuning. A private objective
  cannot replace common temperature, energy, movement and constraint measures.

  Define receding horizon as applying only the first optimized move, measuring
  the next state, shifting the forecasts and solving all 729 candidates again.
  Define feasibility as existence of at least one sequence satisfying every
  hard constraint. Define optimality only relative to the feasible finite grid.
  Define solver completion separately from feasibility: a completed exhaustive
  search can truthfully return infeasible. Invalid weights, nonfinite inputs or
  an incomplete enumeration are solver failures, not infeasibility.

  The fallback is a separately visible incremental PI pair. Proportional gain
  is `20 percent/degC`, integral gain is `0.02 percent/(degC*s)`, and the same
  individual/rate/shared limits are applied. The fallback is not called MPC and
  does not inherit predictive claims.
- **Prediction:** At `T1=T2=21 degC`, `To=5 degC`, zero occupancy gain, setpoints
  `21 degC`, previous commands `80/80 percent`, flat tariff `0.1`, and constant
  three-step forecasts, predict that the zero-move sequence maintains both
  zones at `21 degC`. It is feasible because individual power, rate, shared
  power and comfort limits are satisfied. Calculate its first predicted state
  and objective before running. Predict that a comfort minimum of `22 degC`
  from the same state is infeasible in the first horizon step under the shared
  limit; a completed solver shall report infeasible and select PI fallback, not
  clip a fictitious optimum.
- **Controller under test:** One shipped Structured Text project shall contain
  the two-zone incremental PI fallback and the three-step finite-grid optimizer.
  Each scan shall reset candidate and feasible counts, evaluate exactly `729`
  complete sequences when configuration and signals are valid, publish best
  objective and first requested moves, distinguish solver-complete, feasible,
  infeasible, fallback-active and configuration-valid states, and apply only the
  selected first move. Requested and applied heater commands shall remain
  separate. The code shall not contain a precomputed affine policy, prerecorded
  move table, embedded acceptance metrics or Simulation-truth future state.
- **Experiment and exact Control Studio workflow:** The simulator shall execute the continuous two-zone model at
  the same `60 s` interval. Start both zones and setpoints at `21 degC`, outside
  temperature at `5 degC`, occupancy gains at zero and prior heaters at
  `80/80 percent`. The normal `20 min` schedule keeps outdoor temperature at
  `5 degC` through minute 5 then `4 degC`; introduces zone-one occupancy gain
  `2 degC` at minute 8 and zone-two gain `1 degC` at minute 10; and raises
  tariff from `0.10` to `0.60` from minute 8 through 12 before returning it to
  `0.15`. At every minute the three forecast positions shall contain the next
  three declared schedule values.

  The visible workflow shall contain exactly `Monitor` followed by `Test`. In
  Monitor, verify Simulation classification, both zone identities, `60 s`
  interval, present measurements/disturbances, all three forecast positions,
  setpoints, previous/requested/applied heater commands, individual/rate/shared
  limits, objective weights, candidate/feasible counts, best cost, solver state,
  fallback state, comfort state, quality and disabled output. Before granting
  authority, reproduce the sampled-model equilibrium, `729` sequence count,
  and worked first prediction. No disabled Identify, Tune, or Validate screen
  may be presented as a workflow stage or as evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `1200.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  project owns the twenty-minute weather, occupancy, tariff, and forecast
  schedule. While authority is true, the project shall select nominal MPC and
  publish the next three declared schedule values. Removing authority, invalid
  inputs, or a safety abort shall disable both heaters and restore outdoor and
  occupancy inputs to `5,0,0 degC`.

  Qualified PI, completed-search infeasibility at a `22 degC` minimum, invalid
  negative temperature weight, fallback, and recovery remain native executable-
  test records. The visible normal-MPC run may be correlated with them but
  shall not be described as executing them.

  Abort for wrong model, interval, initial state, schedule, forecast, weight,
  move grid, horizon, limit, PI state or evidence class; invalid, stale, forced,
  missing or nonfinite required signals; heater outside `0..100 percent`, move
  outside `+-20 percentage points`, combined power above `170 percent`, plant
  outside the local range, unexplained candidate count, an infeasible or failed
  solve presented as optimal, or failed disable/restoration.
- **Comparison measures:** Define zone absolute-error integral as the sum of
  absolute setpoint-minus-temperature error times `60 s`, in degree-Celsius
  seconds. Define comfort violation as accumulated magnitude and duration below
  `20.0 degC` or above `22.5 degC`. Define tariff-weighted heating use as
  `sum(p_k*(u1_k+u2_k)*60 s)`, in relative cost-percent seconds. Define heater
  movement as total variation in percentage points, peak individual and shared
  power in percent, active-constraint duration in seconds, fallback duration in
  seconds and completed/infeasible/failed solve counts. Every value shall retain
  its source, units, window, schedule revision, model, weights, horizon, move
  grid, sample count and evidence class.
- **Declared requirements:** The independent oracle and shipped ST shall agree
  on candidate count, feasible count, first move and objective within declared
  tolerance for the worked case and each normal MPC interval. Every applied
  command shall satisfy individual, rate and shared limits. Normal MPC shall
  complete every solve without fallback, retain both zones inside the comfort
  envelope, and use no greater tariff-weighted heating cost than qualified PI
  without greater total comfort violation. If the finite grid cannot improve on
  PI, the lesson shall report that result rather than manufacture a win.

  The `22 degC` minimum case shall complete all 729 candidates, report zero
  feasible candidates and infeasible status, and select bounded PI fallback.
  Negative weight shall report invalid configuration without claiming
  infeasibility, skip optimization, and select the same bounded fallback.
  Restoring nominal bounds and weights shall recover on the next complete solve
  without an unbounded command step. Disabled mode shall apply zero heating.
- **Validation:** A native executable test shall compile the shipped Structured
  Text, implement an independent finite-grid enumeration oracle, prove the
  sampled prediction against the continuous simulator, execute PI, MPC,
  infeasible, invalid and recovery records, and calculate all reported measures
  from synchronized samples. It shall prove no prerecorded policy controls the
  result by changing forecasts and obtaining an independently predicted first
  move. A rendered Control Studio journey shall expose the formulation,
  forecasts, horizon, weights, constraints, solver status, first-move/receding-
  horizon behavior, common PI/MPC plots and metrics, infeasibility, fallback,
  recovery and explicit Simulation classification without presenting hundreds
  of candidate parameters in the main view.
- **Explanation:** Trace one candidate sequence through all three predictions,
  each constraint, every objective term and the first applied move. Explain why
  preview can change a move before a disturbance arrives, why only the first
  move is applied, why a completed solver may be infeasible, why clipping an
  unconstrained command is not equivalent to constrained optimization, and why
  grid resolution creates a quantifiable optimality limitation.
- **Transfer and required conclusion:** Model-predictive control is justified
  here by coupled future prediction, preview and hard constraints. A
  precomputed affine command or clipped PI output is not MPC. Physical building
  use requires identified thermal/actuator dynamics, qualified forecasts,
  occupancy/privacy boundaries, equipment interlocks, independent facilities
  and safety review, explicit approval before application and rollback. All
  Lesson-68 results remain Simulation evidence. The local linear guarantees and
  finite explored envelope create Lesson 69's need for nonlinear region-of-
  attraction and Lyapunov reasoning.

### 3.12 Nonlinear, adaptive and robust control

#### Lesson 69 — Nonlinear stability and Lyapunov reasoning: inverted pendulum

- **Objective:** Use a nonlinear motor-driven inverted-pendulum laboratory rig
  to distinguish a successful trajectory from a mathematical stability
  certificate. The learner shall locate the upright equilibrium, interpret a
  phase portrait, evaluate a Lyapunov function and its derivative, map a
  declared certified sublevel set, and report recoveries outside that set only
  as simulated observations.
- **Context:** The real-world example is a rigid pendulum driven by a torque
  motor at its pivot. Absolute pendulum angle (`theta`) is measured in radians,
  angular rate (`omega`) in radians per second, and requested/applied motor
  torque (`tau_requested`, `tau_applied`) in newton metres. The upright
  equilibrium is `theta=pi rad`, `omega=0 rad/s`, and zero torque. Define angle
  error (`alpha`) as absolute angle minus the upright setpoint only after the
  physical orientation has been explained.

  The simulator shall use the existing nonlinear `robot_joint` plant with
  inertia `1 kg*m^2`, gravity-torque coefficient `1 N*m`, zero viscous and
  Coulomb friction, and `10 ms` plant substeps. Its physical equation is
  `theta_dot=omega` and
  `omega_dot=tau_applied-sin(theta)`. Because
  `sin(pi+alpha)=-sin(alpha)`, the error dynamics near upright are
  `alpha_dot=omega` and `omega_dot=tau_applied+sin(alpha)`. The controller task
  interval is `100 ms`; requested torque is limited to `-2..2 N*m` before it is
  applied. All runs remain Simulation evidence.
- **Reason:** Lesson 68 used a linear sampled model and finite horizon around a
  declared operating range. A single inverted-pendulum recovery under a local
  feedback law does not prove that every nearby initial state converges, that
  trajectories remain inside the assumed region, or that actuator saturation
  preserves the argument. A state-space design therefore needs an explicit
  nonlinear region and certificate before it can support a stability claim.
- **Theory before practice:** Define a state as the smallest set of quantities
  needed to predict motion. Define an equilibrium as a state that remains
  constant under its equilibrium input. Define a phase plane as a plot whose
  horizontal coordinate is angle error and vertical coordinate is angular
  rate; each initial condition produces one trajectory through this plane.
  Define local stability as a claim restricted to a neighbourhood, global
  stability as a claim over the complete admissible state space, and region of
  attraction as the set of initial states known to converge to the equilibrium.
  A finite collection of simulated trajectories samples that set but cannot
  prove its boundary.

  Explain a Lyapunov function as a positive energy-like scalar that is zero at
  equilibrium and positive elsewhere. Only then define the state vector
  `x=[alpha,omega]^T` and the local feedback
  `tau_requested=-4*alpha-2*omega`. Linearisation gives closed-loop matrix
  `A_cl=[[0,1],[-3,-2]]`. Obtain the symmetric matrix `P` by solving
  `A_cl^T*P+P*A_cl=-I`; choosing `-I` makes the linearized derivative equal
  minus the squared state length. The three scalar equations give
  `P=[[4/3,1/6],[1/6,1/3]]`. Its leading diagonal element and determinant `5/12`
  are positive, so the quadratic function is positive definite:

  `V(x)=0.5*x^T*P*x`

  or, with named states (where the mixed coefficient is `2*P_12=1/3`),

  `V=0.5*((4/3)*alpha^2+(1/3)*alpha*omega+(1/3)*omega^2)`.

  For the actual nonlinear unsaturated plant, explain the nonlinear remainder
  `r(alpha)=sin(alpha)-alpha` before writing the derivative:

  `V_dot=-0.5*(alpha^2+omega^2)`
  `      +(alpha/6+omega/3)*(sin(alpha)-alpha)`.

  Define the certified sublevel set as `V<=rho` with `rho=0.025`. Minimize `V`
  over rate for fixed angle to obtain `V>=(5/8)*alpha^2`, hence
  `|alpha|<=0.2`; minimize over angle for fixed rate to obtain
  `V>=(5/32)*omega^2`, hence `|omega|<=0.4`. For feedback row `c=[4,2]`, derive
  the maximum request on the ellipsoid as
  `sqrt(2*rho*c^T*P^-1*c)=sqrt(0.96)=0.980 N*m`, below the `2 N*m` limit. Use
  `|sin(alpha)-alpha|<=|alpha|^3/6` inside the angle bound to obtain the strict
  negative derivative bound stated by the lesson. The
  certificate therefore establishes an invariant local subset of the region
  of attraction for the declared continuous nonlinear model. It does not prove
  global stability, behavior under saturation, a different sample interval, an
  unmodelled plant, or every point outside the sublevel set.
- **Prediction:** At `alpha=0.1 rad` and `omega=0`, calculate requested torque
  `-0.4 N*m`, Lyapunov value `0.0066667`, and continuous derivative about
  `-0.0050028` Lyapunov units per second. Predict that this point is inside the
  `0.025` sublevel set and unsaturated. At `alpha=0.25 rad`, `omega=0`, calculate
  `V=0.0416667`; predict that the point is outside the certificate even if its
  simulated trajectory happens to recover. State these predictions before
  opening the traces.
- **Controller under test:** The shipped Structured Text shall calculate angle
  error from the measured absolute angle, request `-4*alpha-2*omega`, apply the
  explicit torque limit, and publish requested torque, applied torque,
  saturation, `V`, `V_dot`, sampled `Delta V`, evidence-initialized state,
  sublevel membership, and certificate-valid state. The code shall use the
  measured nonlinear plant state; it shall not integrate a private linear
  pendulum, replay a trajectory, embed final acceptance metrics, or label a
  simulated recovery as certified.
- **Experiment and exact Control Studio workflow:** The visible workflow shall
  contain exactly `Monitor` followed by `Test`. The visible run shall use the
  simulator's declared `alpha=0.1 rad`, `omega=0` initial state for `8.1 s` at
  the fixed `100 ms` controller interval. In Monitor, verify absolute angle, upright setpoint, angle error, rate,
  requested/applied torque, saturation, `V`, `V_dot`, `Delta V`, sublevel and
  certificate state, units, sample interval, quality and Simulation class.
  Before granting authority, reproduce positive definiteness, derivative, and
  `rho` calculations. No disabled Identify, Tune, or Validate screen may be
  presented as a workflow stage or evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `8.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  simulator owns initial state and the project owns feedback and certificate
  evaluation. Removing authority, invalid inputs, or a safety abort shall
  disable feedback and apply exactly zero torque.

  The remaining certified initial-state map, `+/-0.25 rad` observational cases,
  and `0.8 rad` saturation rejection remain native executable-test records
  under the same model, gains, limits, sampling, and duration. The visible run
  may be correlated with them but shall not be described as executing them.

  Abort for wrong equilibrium, initial state, model, coefficient, task or plant
  interval, torque limit, gain, matrix, `rho`, duration or evidence class;
  invalid, stale, forced, missing or nonfinite angle/rate; requested/applied
  mismatch without saturation; a certified point with nonnegative `V_dot` or
  sampled `Delta V` away from equilibrium; an escaped certified trajectory; or
  any outside trajectory presented as certificate proof.
- **Comparison measures:** Define initial and maximum Lyapunov value,
  maximum positive sampled `Delta V`, maximum continuous `V_dot`, final angle
  and rate magnitude, time to enter `|alpha|<=0.01 rad` and
  `|omega|<=0.02 rad/s`, peak requested/applied torque, saturation duration,
  sublevel-exit count, and certificate-violation count. Every measure shall
  state its signal, unit, window, equilibrium tolerance and evidence class.
  Settling time is reported only when both state tolerances remain satisfied
  for the declared final window.
- **Declared requirements:** An independent oracle shall match the nonlinear
  RK4 plant response and all ST certificate calculations within declared
  tolerances. Every non-equilibrium sample of every certified trajectory shall
  retain `V<=0.025`, have negative `V_dot`, nonpositive sampled `Delta V` after
  initialization, remain unsaturated, and converge to the equilibrium
  tolerance. At least one `alpha=+/-0.25 rad` release shall be shown outside the
  certified set from its first sample; whether it recovers or not shall not
  alter its uncertified status. Disabled mode shall apply zero torque.
- **Validation:** A native executable test shall compile the shipped Structured
  Text, run it against the nonlinear simulator, compare plant steps with an
  independent nonlinear RK4 oracle, execute the complete initial-state map,
  calculate the declared measures and prove fail-closed certificate state for
  outside-region and saturation cases. A rendered Control Studio journey shall
  show the physical orientation, phase portrait, state traces, torque limit,
  `V` and derivative without presenting the quadratic expression before its
  physical purpose is explained.
- **Explanation:** Trace one certified release from physical state through
  feedback torque, nonlinear motion, Lyapunov value, derivative and sampled
  decrease. Explain why negative derivative makes the sublevel set invariant,
  why a finite trajectory grid is validation rather than a mathematical proof,
  why recovery outside the set does not expand the certified region, and why
  actuator saturation breaks the unsaturated derivative calculation.
- **Transfer and required conclusion:** The learner may claim local asymptotic
  stability only for the declared nonlinear model, feedback, torque limit,
  sampling assumptions and certified sublevel set. A successful simulation is
  not a global certificate. The strong nonlinear dependence of required torque
  across a robot joint's working range creates Lesson 70's comparison of gain
  scheduling with model-based feedback linearisation.

#### Lesson 70 — Gain scheduling versus feedback linearization: robot joint

- **Objective:** Move one gravity-loaded industrial robot joint through a wide
  angular range and select among fixed PID, gain-scheduled PID and feedback
  linearisation using common tracking, torque, constraint and payload-mismatch
  evidence. The learner shall explain what is scheduled, what is cancelled,
  why feedback remains necessary, and when the model-based architecture loses
  eligibility.
- **Context:** The real-world example is one rigid robot arm axis driven by a
  bidirectional torque motor. Joint-angle setpoint (`q_sp`) and measured joint
  angle (`q`) use radians, measured angular rate (`q_dot`) uses radians per
  second, and requested/applied motor torque (`tau_requested`, `tau_applied`)
  use newton metres. The simulated joint has inertia `J=1 kg*m^2`, nominal
  gravity-torque coefficient `G=9.81 N*m`, viscous coefficient
  `b=0.2 N*m*s/rad`, zero Coulomb friction, `5 ms` nonlinear plant substeps,
  and a `20 ms` controller interval. Applied torque is limited to
  `-30..30 N*m`. The validated motion envelope is `-1.2..1.2 rad` and
  `-4..4 rad/s`. A heavy-payload record increases actual gravity coefficient by
  `20 percent` to `11.772 N*m` without changing the controller's nominal model.
- **Reason:** Lesson 69 deliberately certified only a small invariant region.
  A production robot cannot remain near one angle: gravity torque and local
  stiffness change continuously as the arm moves. One fixed tuning therefore
  changes its effective dynamics across the range. Extending the local
  certificate does not decide whether to interpolate validated local
  controllers or cancel the known nonlinearity with a model.
- **Theory before practice:** State the physical joint balance in words: motor
  torque accelerates inertia after gravity and viscous friction are overcome.
  Only then write
  `J*q_ddot+b*q_dot+G*sin(q)=tau_applied`.

  Define a local model as a linear approximation around one operating angle
  (`q0`). Derive it by writing `q=q0+delta_q` and using
  `sin(q0+delta_q) approximately sin(q0)+cos(q0)*delta_q`; the constant term is
  holding torque and the coefficient of `delta_q` is local gravity stiffness
  `G*cos(q0)`. With feedback, the small-error equation is
  `J*delta_q_ddot+(b+Kd)*delta_q_dot+(G*cos(q0)+Kp)*delta_q=0`. Dividing by `J`
  and matching the desired polynomial `s^2+2*zeta*omega_n*s+omega_n^2`
  produces the gains below. For desired natural
  frequency `omega_n=5 rad/s` and damping ratio `zeta=0.8`, the proportional
  and derivative gains that preserve the local second-order coefficients are
  `Kp(q0)=J*omega_n^2-G*cos(q0)` and
  `Kd=2*zeta*omega_n*J-b`. Calculate proportional gains
  `15.190`, `16.904` and `21.445 N*m/rad` at absolute operating angles
  `0`, `0.6` and `1.2 rad`, with derivative gain `7.800 N*m*s/rad`; the learner
  shall show the substitutions `25-9.81*cos(q0)` and
  `2*0.8*5*1-0.2`, not merely copy the results.

  Define gain scheduling as selecting or interpolating controller parameters
  from independently validated local designs using a measurable scheduling
  variable. The scheduled PID shall linearly interpolate proportional gain over
  absolute setpoint angle, retain integral gain `Ki=12 N*m/(rad*s)` and
  derivative gain `7.8 N*m*s/rad`, clamp the scheduling variable to the
  validated range, and expose the active gain. Explain that interpolation does
  not prove stability between scheduling points or during a transition; those
  cases require validation.

  The fixed and scheduled PID records shall use the same integral gain. The
  value `12 N*m/(rad*s)` gives integral times `Ti=Kp/Ki` from about `1.27 s`
  at zero angle to `1.79 s` at `1.2 rad`. This supplies the gravity-holding
  torque within the declared plateau while leaving proportional-gain
  scheduling as the only architectural difference between those two records.

  Define feedback linearisation as using the nonlinear model to cancel known
  terms before applying desired linear error dynamics. With nominal estimates
  `J_hat=1`, `G_hat=9.81` and `b_hat=0.2`, define
  `tau_model=G_hat*sin(q)+b_hat*q_dot` and
  `tau_requested=tau_model+J_hat*(24*(q_sp-q)-8*q_dot)`.
  The proportional coefficient is deliberately below the `25 N*m/rad`
  coefficient implied by the nominal `5 rad/s` local design. This leaves
  `1.2 N*m` of command margin for a `1.2 rad` step instead of placing the first
  nominal request exactly on the `30 N*m` actuator limit. The lesson shall
  explain this actuator-authority margin rather than presenting it as an
  arbitrary retuning.
  The gravity and friction terms cancel exactly only when the model matches.
  Substitution into the nominal plant gives
  `q_ddot=24*(q_sp-q)-8*q_dot`; for constant setpoint and
  `e=q_sp-q`, this is `e_ddot+8*e_dot+24*e=0`, with roots
  `-4 plus/minus j*sqrt(8)`. This is the linear error behavior that cancellation
  is intended to create.
  Feedback remains because initial-state error, disturbances, measurement error
  and model error remain after nominal cancellation.

  Define input effectiveness as the coefficient multiplying the actuator in
  the state equation. Feedback linearisation is eligible here because
  `1/J_hat` is finite, positive and bounded throughout the declared envelope.
  Explain that dividing by an input effectiveness that approaches zero creates
  a singular command and makes the architecture ineligible. Define residual
  dynamics as the tracking dynamics left by coefficient error after imperfect
  cancellation. The `20 percent` heavy payload shall expose that residual
  without silently changing `G_hat`.
- **Prediction:** At `q=0.5 rad`, `q_dot=0`, nominal model compensation is
  `9.81*sin(0.5)=4.703 N*m`. At `q_sp=1.2 rad`, the scheduled proportional gain
  is `21.445 N*m/rad`; at zero setpoint it is `15.190 N*m/rad`. At
  `q=q_sp=1.2 rad`, exact feedback linearisation requests the nominal holding
  torque `9.81*sin(1.2)=9.143 N*m`. With actual `G=11.772 N*m` but unchanged
  nominal estimate, predict an uncompensated gravity residual of about
  `1.829 N*m`; the no-integral feedback-linearised design therefore needs about
  `0.076 rad` error to balance that residual with `24 N*m/rad` feedback.
- **Controller under test:** The shipped Structured Text shall use the shared
  `Control.PID` function block for fixed and scheduled PID behavior and for the
  feedback portion of the linearised design. It shall expose requested and
  active architecture, fixed/scheduled/active gains, model compensation,
  requested/applied torque, saturation, model eligibility and fallback. The
  scheduling law and nonlinear model term shall remain visible outside the PID
  function block. It shall not use a precomputed trajectory, simulated future
  state, actual payload truth inside the controller, embedded performance
  metrics, or a second control library.
- **Experiment and exact Control Studio workflow:** Start at `q=0 rad`, `q_dot=0`. Hold zero setpoint for `1 s`,
  command `+1.2 rad` through `11 s`, return to zero through `16 s`, command
  `-1.2 rad` through `26 s`, then return to zero through `31 s`. The ten-second
  nonzero plateaus are part of the validity design: they allow the declared
  integral dynamics to reach the settling window instead of treating a short
  transient as steady state. The visible workflow shall contain exactly
  `Monitor` followed by `Test`. In Monitor, verify setpoint, angle, rate, requested/active architecture,
  scheduling variable, gains, nominal model coefficients, model compensation,
  requested/applied torque, saturation, quality, envelope and Simulation
  class. Before granting authority, reproduce all three local gains,
  interpolation, cancellation, nominal poles, and mismatch predictions. No
  disabled Identify, Tune, or Validate screen may be presented as a workflow
  stage or evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `31.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  project owns scheduled-PID selection and the full setpoint trajectory.
  Removing authority, invalid inputs, or a safety abort shall restore the
  setpoint to zero, put the PID in zero-output Manual mode, and apply zero
  torque.

  Fixed PID, feedback linearisation, all heavy-payload comparisons, invalid-
  inertia fallback, and envelope cases remain native executable-test records.
  The visible nominal scheduled-PID run may be correlated with them but shall
  not be described as executing them.

  Abort for wrong plant, payload, interval, schedule, initial state, envelope,
  local-design point, interpolation, PID form, gain, model coefficient, torque
  limit or evidence class; invalid, stale, forced, missing or nonfinite state;
  requested/applied torque outside its declared treatment; scheduling outside
  the validated envelope without a clamp/rejection; division by invalid input
  effectiveness; an invalid model used for cancellation; or failed disable.
- **Comparison measures:** Define segment integrated absolute angle error in
  radian-seconds, overshoot relative to each commanded angle change in percent,
  settling time only after `|q_sp-q|<=0.02 rad` and
  `|q_dot|<=0.05 rad/s` remain true for `0.5 s`, peak requested/applied torque
  in newton metres, applied-torque total variation in newton metres, saturation
  duration, final-window mean absolute error over the final `0.5 s` of each
  plateau, and architecture/model/fallback state. Compare identical windows
  and do not rank an unsettled segment by final error.
- **Declared requirements:** The ST active scheduled gain and nominal model
  compensation shall match independent equations within `1e-9`. Every applied
  torque shall remain within `-30..30 N*m`. On the nominal plant, scheduled PID
  shall reduce the variation of segment settling behavior across operating
  angles relative to fixed PID, while eligible feedback linearisation shall
  meet every segment tolerance without saturation. Under the heavy payload,
  scheduled PID integral action shall retain final-window mean absolute error
  no greater than `0.02 rad`; the unchanged no-integral linearised controller
  shall expose a materially larger load-dependent residual rather than hiding
  it with actual plant truth. Invalid estimated inertia shall activate scheduled
  fallback and disabled mode shall apply zero torque.
- **Validation:** A native executable test shall compile the shipped project
  with the shared Control library, qualify schedule interpolation, model
  compensation and first commands against independent equations, execute all
  six common trajectory records plus invalid and disabled cases, and calculate
  the declared measures from synchronized nonlinear simulation. A rendered
  Control Studio journey shall overlay common state/torque traces, show the
  active scheduling gain and cancellation term, and keep nominal, mismatch and
  Simulation classifications visible without presenting every parameter in the
  main view.
- **Explanation:** Explain fixed-controller variation through changing local
  gravity stiffness, scheduled behavior through interpolated validated gains,
  and nominal linearised behavior through cancellation. Then trace the heavy-
  payload coefficient error into residual gravity torque and tracking offset.
  Explain why integral feedback can reject a constant residual while slower
  motion does not make the model-based architecture generally unsafe.
- **Transfer and required conclusion:** Gain scheduling is appropriate when a
  finite operating envelope and local designs can be validated without trusting
  complete nonlinear cancellation. Feedback linearisation can give simpler,
  faster nominal dynamics when the nonlinear model and input effectiveness are
  trustworthy, but it requires stronger model qualification and a bounded
  fallback. A process whose parameters drift continuously instead of occupying
  validated regions creates Lesson 71's need for constrained adaptation.

#### Lesson 71 — Adaptive control: extruder pressure

- **Objective:** Regulate die pressure on one polymer extruder while resin grade
  changes at a known production transition and effective material gain also
  drifts continuously. Select fixed, recipe-scheduled, or supervised adaptive
  PI control from common pressure, screw-speed, estimation, excitation and
  recovery evidence. The learner shall be able to explain why parameter motion
  is not evidence of correct identification and why adaptation must freeze when
  information or signal validity is lost.
- **Context:** The real equipment is a single-screw polymer extruder whose
  motor-speed request changes material throughput and die pressure. Die-pressure
  setpoint (`p_sp`) and measured die pressure (`p`) use bar. Requested screw
  speed (`u_requested`) and applied screw speed (`u_applied`) use percent and
  are limited to `0..100 percent`. A small identification probe (`u_probe`) is
  an explicitly approved addition to the common controller request, not hidden
  noise. The controller executes every `100 ms` and the simulated plant advances
  every `20 ms`.

  The nominal first-order material balance has pressure time constant
  `tau_p=5 s`, zero output bias and nominal resin-grade gains `K_A=1.0 bar/%`
  and `K_B=0.75 bar/%`. At `60 s`, the known recipe changes from grade A to
  grade B. Within both grades, unmeasured viscosity variation multiplies the
  current gain by `1+0.15*sin(2*pi*t/120)`. The effective gain therefore changes
  continuously and remains inside `0.6375..1.15 bar/%`. The controller receives
  grade identity, pressure and its own applied command; it never receives the
  simulator's effective gain. Every record is Simulation evidence.
- **Reason:** Lesson 70 showed that a finite set of validated local controllers
  can cover known robot configurations, but model cancellation retained an
  offset when an unmeasured payload coefficient changed. The extruder does have
  two known resin recipes suitable for scheduling, yet moisture, temperature,
  wear and blend variation move the pressure gain within each recipe. Adding
  ever more schedule points for an unmeasured drifting property does not solve
  the evidence problem. This creates a reason to estimate a bounded parameter
  online while preserving a nonadaptive fallback.
- **Theory before practice:** First explain the plant in words: screw speed
  creates an equilibrium pressure through a material-dependent gain, while the
  pressure approaches that equilibrium with thermal and transport storage.
  After defining pressure time constant (`tau_p`), effective process gain
  (`K`), output bias (`B`) and applied screw speed (`u_applied`), write
  `tau_p*dp/dt=K*u_applied+B-p`.

  Define online parameter estimation as updating a model from synchronized
  measured input/output pairs while the plant runs. Over one controller interval
  (`Delta t`), define the reconstructed equilibrium-pressure observation (`z`)
  in words as the pressure target implied by the measured pressure change. Only
  then write
  `z=p_previous+tau_p*(p-p_previous)/Delta t`
  and the linear regression
  `z/100=theta_K*(u_previous/100)+theta_B`.
  Derive `z` by rearranging the plant equation as
  `K*u+B=p+tau_p*dp/dt`, approximating the derivative from consecutive pressure
  samples, and using the pressure and applied command belonging to that
  interval. It is a reconstructed equilibrium term, not another sensor.
  Estimated gain (`K_hat`) is `theta_K`; estimated pressure bias (`B_hat`) is
  `100*theta_B`. Scaling both regression signals by 100 keeps the numerical
  magnitudes suitable for the PLC implementation.

  Define recursive least squares (RLS) as a recursive estimator that corrects
  the parameter vector from prediction error and an uncertainty matrix. Define
  regressor vector (`phi`) as `[u_previous/100,1]`, scaled observation (`y_id`)
  as `z/100`, prediction error or innovation (`epsilon`) as measured observation
  minus predicted observation, covariance matrix (`P`) as estimator uncertainty,
  and forgetting factor (`lambda`) as the weight retained from older evidence.
  For `lambda=0.98`, the geometric history contains roughly
  `1/(1-lambda)=50` effective samples, which gives an evidence horizon of about
  `5 s` at the `100 ms` update interval. Explain the update in words before writing
  `L=P*phi/(lambda+phi^T*P*phi)`,
  `theta_next=theta+L*epsilon`, and
  `P_next=(P-L*phi^T*P)/lambda`.
  A forgetting factor below one permits slow tracking but also makes unsupported
  drift possible; it is not a substitute for informative data.

  Define persistent excitation as enough independent variation in the regressor
  to distinguish gain from bias. A constant screw speed cannot separate the two
  parameters even if many identical samples are collected. The implementation
  shall maintain a forgetting information matrix
  `R_next=0.98*R+phi*phi^T` and define information determinant (`D_R`) as
  `R_11*R_22-R_12^2`. Adaptation is information-eligible only when
  `D_R>=0.10`. The common probe alternates `+10` and `-10 percentage points` every
  `2 s`; all three architecture records receive the same probe so its pressure
  and actuator costs are not hidden from the comparison.

  Persistent excitation is a data condition, not experiment authorization.
  Adaptive updates shall additionally require the bounded probe window to be
  explicitly enabled. Without this gate, gain changes can move controller
  output, the estimator can interpret that self-induced motion as new
  information, and adaptation can sustain its own apparent excitation. Removing
  probe authorization shall therefore freeze RLS immediately even while the
  historical information determinant decays.

  Define parameter projection as constraining an estimator update to a reviewed
  physical set. Project `K_hat` into `0.5..1.5 bar/%` and `B_hat` into
  `-20..20 bar`; expose projection rather than silently clipping it. A value on
  either projection boundary is not eligible for adaptive controller authority.
  Define
  estimator confidence as the covariance trace `P_11+P_22`. An estimate becomes
  qualified only after at least `20` valid informative updates reduce that
  trace to `10.0` or less, and qualification shall be revoked by nonfinite
  state, out-of-bound state or bad signal quality.

  Define supervised adaptation as a state machine around the estimator. It may
  update only while adaptation is requested, pressure data are healthy, the
  controller is enabled, applied output is not saturated, persistent
  excitation is present, and the bounded probe window is authorized. Loss of
  excitation authorization freezes the last qualified
  estimate; it does not reset it or continue learning. Before an estimate is
  qualified, an adaptive request uses recipe-scheduled PI. Bad pressure quality
  freezes estimation and applies the declared zero-speed safe output rather than
  allowing either adaptive or fallback feedback to act on corrupt data.

  Define indirect adaptive control as estimating plant parameters and deriving
  ordinary controller gains from them. The base model-based PI design uses
  closed-loop design time `lambda_c=3 s`:
  `Kp=tau_p/(K_design*lambda_c)` and
  `Ki=1/(K_design*lambda_c)`.
  Derive these by matching the first-order process to the desired closed-loop
  response time; proportional gain supplies `tau_p/(K_design*lambda_c)`, and
  choosing integral time `Ti=tau_p` gives
  `Ki=Kp/Ti=1/(K_design*lambda_c)`.
  Fixed PI always uses `K_design=1.0`; scheduled PI uses `1.0` for grade A and
  `0.75` for grade B; adaptive PI uses the qualified projected `K_hat`.
  All modes use the same shared `Control.PID`, zero derivative gain, combined
  anti-windup, identical probe and `0..100 percent` applied limits.
- **Prediction:** For grade A, fixed and scheduled PI both calculate
  `Kp=1.667 %/bar` and `Ki=0.333 %/(bar*s)`. For grade B, scheduled PI calculates
  `Kp=2.222 %/bar` and `Ki=0.444 %/(bar*s)`. If the qualified estimate is
  `K_hat=0.65 bar/%`, adaptive PI calculates `Kp=2.564 %/bar` and
  `Ki=0.513 %/(bar*s)`. If a candidate update requests `K_hat=2.3 bar/%`,
  projection retains `1.5 bar/%` and raises the projection state. Predict that
  the estimates stop changing immediately when probe authorization is removed;
  the information determinant then decays below eligibility on the constant-
  grade validation record.
- **Controller under test:** The shipped Structured Text shall contain one real
  `Control.PID`, the two-parameter RLS equations, information-matrix update,
  projection and supervisor. It shall expose requested and active architecture,
  resin grade, adaptation request/active/frozen state, data health, persistent
  excitation, estimate qualification, scheduled fallback, projection,
  estimated gain and bias, covariance trace, information determinant,
  innovation, active PI gains, probe, requested/applied screw speed and
  saturation. It shall not contain actual effective gain, simulator time,
  embedded comparison metrics, a preferred architecture, precomputed parameter
  history, or a second control library.
- **Experiment and exact Control Studio workflow:** The visible workflow shall
  contain exactly `Monitor` followed by `Test`. Start the visible record at `p=40 bar`, `p_sp=40 bar`, grade A,
  nominal estimates `K_hat=1.0 bar/%`, `B_hat=0 bar`, and the same covariance,
  information state and deterministic probe phase. Hold `40 bar` through
  `30 s`, command `50 bar` through `90 s`, then command `60 bar` through
  `120 s`. At `60 s`, change only the declared resin grade from A to B; the
  simulator applies the same unmeasured continuous gain multiplier throughout.
  visible project shall select supervised-adaptive mode with the probe enabled.

  Continue a separately restored adaptive supervision record on constant grade
  A with its continuous viscosity multiplier disabled. Hold `40 bar` with the
  probe through `60 s`, disable only the probe through `90 s`, mark pressure
  data unhealthy from `90..95 s`, then restore healthy data and the probe and
  continue through `120 s`; finally disable and verify zero applied speed. This
  separate record is required because the drifting comparison plant can create
  naturally informative command variation even after the deliberate probe is
  removed; it shall not be mislabeled as loss of excitation. In a
  separate bounded-state case, write `K_hat=2.3 bar/%` while adaptation is
  frozen and prove immediate projection to `1.5 bar/%` without an estimator
  update.

  In Monitor, verify all physical signals, units, requested/active architecture,
  recipe, estimator, supervisor, constraints, quality and Simulation class.
  Before granting authority, reconstruct one observation, innovation,
  information determinant, RLS update, and the fixed, grade-B, and
  `K_hat=0.65` PI gains. No disabled Identify, Tune, or Validate screen may be
  presented as a workflow stage or evidence.

  In Test, select `Observe without excitation`, amplitude `0`, and duration
  `120.1 s`. The only writable lesson signal shall be Boolean experiment
  authority `P1.ExperimentEnabled`; amplitude zero is intentional because the
  project owns adaptive authorization, the probe, grade change, and setpoint
  schedule. Removing authority, invalid inputs, or a safety abort shall freeze
  adaptation, disable the controller, restore setpoint to zero, and apply zero
  screw speed.

  Fixed and recipe-scheduled common records, projection, loss-of-excitation,
  bad-quality, restoration, and disable cases remain native executable-test
  records. The visible adaptive run may be correlated with them but shall not
  be described as executing them.

  Abort for wrong project, plant, grade timing, setpoint schedule, interval,
  initial state, seed, probe, gain drift, estimator scaling, forgetting factor,
  information threshold, covariance, projection bounds, PI form, output bounds
  or evidence class; invalid, stale, forced, missing or nonfinite pressure;
  adaptation outside the supervisor; an unqualified estimate used by control;
  continuing parameter motion after excitation loss; corrupt data driving the
  actuator; failed restoration; or nonzero disabled output.
- **Comparison measures:** Define pressure integrated absolute error as the
  `0.1 s`-weighted absolute setpoint-minus-pressure error over declared windows,
  in `bar*s`. Define settling time only after absolute pressure error remains at
  most `0.5 bar` for `5 s`. Define applied-speed total variation as the sum of
  absolute consecutive applied-speed changes, in percentage points. Define
  estimate root-mean-square error only in the native Simulation oracle, using
  the independently reconstructed effective gain and a declared post-
  qualification window; it shall never be presented as a field measurement.
  Also retain peak requested/applied speed, saturation duration, projection
  count, informative-update count, frozen duration, confidence, architecture
  transitions and exact estimate change during each frozen window.
- **Declared requirements:** Every RLS state and PI gain shall match an
  independent oracle within declared floating-point tolerance. Estimates and
  covariance shall remain finite; projected gain and bias shall remain inside
  their bounds. Scheduled PI shall improve the grade-B transition over fixed PI
  under the common record. After qualification, adaptive PI shall track the
  continuous effective gain with declared RMSE no greater than `0.08 bar/%` and
  shall not have greater post-grade pressure IAE than scheduled PI. Any extra
  probe-driven applied-speed movement shall remain visible. Loss of persistent
  excitation on the constant-grade validation plant shall freeze both parameter
  values exactly within `1e-12`; bad
  quality shall retain those values and apply zero speed; restored healthy
  excitation shall resume bounded updates. A `2.3 bar/%` forced candidate shall
  project to `1.5 bar/%`; disabled mode shall apply zero speed.
- **Validation:** A native executable test shall compile the shipped project
  with the shared Control library, compare every estimator scan with an
  independent two-parameter RLS oracle, execute the three common drift records,
  and calculate the declared performance, estimation, constraint and
  supervision measures. A rendered Control Studio journey shall show pressure
  and applied speed as the dominant plot, with estimated versus Simulation-only
  effective gain in Identify/Validate, and shall demonstrate information loss,
  freeze, bad-quality safe output, recovery and projection without placing all
  estimator matrices in the primary view.
- **Explanation:** Trace the known grade change into scheduled PI gains and the
  unmeasured within-grade drift into estimator innovation. Trace probe variation
  into information determinant, the RLS correction into projected estimates,
  and the qualified gain into PI settings. Explain any adaptive performance
  improvement together with its probe movement and model assumptions. Explain
  why a frozen qualified estimate is safer than unsupported updates, and why
  bad pressure quality requires safe output rather than confident-looking
  adaptation.
- **Transfer and required conclusion:** Adaptive control is justified only when
  the parameterization, excitation, bounds, confidence, supervision, fallback
  and recovery behavior are validated for the real equipment. Parameter motion
  alone is not learning, and adaptation without information or constraints is a
  hazard rather than an improvement. When uncertainty spans dynamics that
  cannot be safely tracked as slowly varying parameters, Lesson 72 introduces
  worst-case robust design instead of faster adaptation.

#### Lesson 72 — Robust control and H-infinity loop shaping: flexible airframe

- **Objective:** On a ground vibration-test flexible-wing rig, use declared
  model uncertainty and sampled-loop evidence to reject an aggressive nominal
  controller, select one filtered-PD candidate from a finite mixed-sensitivity
  design set, and state exactly which robust-stability and robust-performance
  claims the retained evidence does and does not support.
- **Context:** The real-world example is a laboratory flexible-wing section
  used to study gust-load alleviation before any flight implementation. The
  wing-root bending response (`y`) is the accelerometer-derived bending signal
  in `g`. The control-surface command (`u`) is the requested surface position
  in percent, limited to `-15..15 percent`. The applied control-surface command
  (`u_applied`) is the command after that limit. The gust-equivalent load (`d`)
  is an independently generated aerodynamic-load input in `g`. The desired
  bending response or setpoint (`r`) is zero `g`. Positive surface command
  increases positive measured bending, so feedback must command the opposite
  sign. The controller interval is `20 ms`; the simulated plant advances in
  `5 ms` substeps. All evidence is ground-rig Simulation evidence, not flight,
  aeroelastic-clearance, certification, or physical-airframe evidence.
- **Reason:** Lesson 71 may track a few slowly changing parameters only while
  an authorized informative experiment supports them. A flexible wing instead
  has uncertain resonant frequency, damping, control effectiveness, and
  unmodelled residual dynamics across a declared set. Treating that uncertainty
  as one drifting scalar or adapting faster would not protect the resonant
  mode. The aggressive nominal proportional controller also ignores the
  sampled one-cycle plant delay and becomes marginal or unstable in the
  declared low-damping cases. This creates the need for worst-case loop shaping
  and an explicit uncertainty boundary.
- **Theory before practice:** First explain that the nominal plant (`P_0`) is
  the best single linear model at the reviewed operating condition. Explain
  structured uncertainty as uncertainty in named physical parameters and
  unstructured uncertainty as a bounded residual whose internal dynamics are
  not individually parameterized. Explain multiplicative uncertainty in words:
  the true frequency response may differ from the local model by a bounded
  fraction at every frequency. Only then write
  `P=P_i*(1+W_delta*Delta)`, where local grid plant (`P_i`) is one structured
  case, residual-uncertainty weight (`W_delta`) is the allowed fractional model
  error, and normalized unknown residual (`Delta`) satisfies
  `||Delta||_infinity<=1`.

  Describe the wing model before its equation. Surface motion and gust load
  accelerate one flexible bending mode; stiffness returns it toward zero and
  damping removes energy. Natural angular frequency (`omega_n`) is the
  undamped modal frequency in radians per second. Damping ratio (`zeta`) is the
  dimensionless fraction describing modal energy loss. Control effectiveness
  (`g_u`) scales the installed surface-to-bending path. Derive the transfers by
  dividing `y_ddot+2*zeta*omega_n*y_dot+omega_n^2*y`
  `=0.1*g_u*omega_n^2*u+omega_n^2*d` by its characteristic operator. The declared local
  transfer functions are then
  `P_i(s)=0.1*g_u*omega_n^2/(s^2+2*zeta*omega_n*s+omega_n^2)` from percent
  surface command to `g`, and
  `P_d,i(s)=omega_n^2/(s^2+2*zeta*omega_n*s+omega_n^2)` from gust-equivalent
  `g` to measured bending `g`. The nominal values are
  `omega_n=4.0 rad/s`, `zeta=0.08`, and `g_u=1.0`. The structured design grid
  is the Cartesian product
  `omega_n in {3.2,4.0,4.8} rad/s`,
  `zeta in {0.03,0.08,0.15}`, and `g_u in {0.8,1.0,1.2}`, producing 27 cases.
  The grid is executable qualification evidence at those cases; it is not by
  itself proof for every value between them.

  Define loop transfer (`L`) as plant response multiplied by controller
  response. Define sensitivity (`S`) as the closed-loop path that determines
  low-frequency error and disturbance rejection. Define complementary
  sensitivity (`T`) as the path that exposes resonance and model uncertainty.
  Define control sensitivity (`KS`) as the path from error-producing inputs to
  controller effort. After those meanings are established, write
  `L=P_i*K`, `S=1/(1+L)`, `T=L/(1+L)`, and `KS=K/(1+L)`.
  Derive the common denominator by substituting negative feedback into the
  plant equation and collecting the output terms; this explains why the same
  `1+L` governs error, model uncertainty, and control effort.

  Explain a weighting function as a frequency-dependent engineering limit, not
  a decorative curve. Performance weight (`W_p`) requires useful low-frequency
  disturbance rejection. Residual-uncertainty weight (`W_delta`) represents a
  reviewed `15 percent` low-frequency residual that rises toward `75 percent`
  at high frequency. Control-effort weight (`W_u`) scales percent surface
  command into the common dimensionless decision. With angular frequency
  variable (`s`) in radians per second, use
  `W_p(s)=1.5/(1+s/1)`,
  `W_delta(s)=0.15*(1+s/2)/(1+s/10)`, and
  `W_u=0.005 g/%`.

  Define the H-infinity norm (`||H||_infinity`) as the greatest singular value
  of a stable transfer over all frequencies; for these scalar paths it is the
  largest magnitude. Define mixed-sensitivity score (`gamma`) as the worst of
  the three weighted paths over every declared plant and frequency:
  `gamma=max(||W_p*S||_infinity,||W_delta*T||_infinity,||W_u*KS||_infinity)`.
  Each individual peak must be below `1.0`; averaging may not hide one failed
  requirement. Explain the small-gain robust-stability condition: if every
  local closed loop is stable and `||W_delta*T||_infinity<1`, then every stable
  residual `Delta` inside the declared multiplicative envelope preserves
  stability. Robust performance additionally requires the performance and
  effort bounds. A sampled frequency grid estimates these norms; it shall be
  called a qualified numerical bound, not an exact continuous-frequency proof.

  The executable frequency oracle shall use the actual `20 ms` sampled loop:
  exact zero-order-hold discretization of every local plant, the one-cycle
  command-to-next-measurement relationship, and the same first-order filter and
  backward measurement difference executed by the Structured Text. It shall
  evaluate `0.01..100 rad/s` on at least 20,000 logarithmically spaced points,
  retain the plant and frequency of every peak, and reject nonfinite values or
  a closed-loop pole on or outside the unit circle. Frequencies above the
  `pi/0.02=157.08 rad/s` Nyquist frequency are outside this lesson's claim.

  The nominal controller is positional proportional control with
  `K_nominal=40 %/g`. The robust candidates are filtered proportional-
  derivative controllers whose proportional gain (`Kp`) is in `%/g`,
  derivative gain (`Kd`) is in `%*s/g`, and measurement-filter time constant
  (`tau_f`) is in seconds. The shipped finite candidate set is
  `(8,3,0.05)`, `(10,2,0.05)`, `(10,3,0.05)`, and `(12,3,0.05)` for
  `(Kp,Kd,tau_f)`. Candidate selection shall be computed from the common
  27-plant sampled frequency grid; a preferred result may not be embedded in
  the plant, controller, or test oracle.

  The selected candidate uses the actual `Control.LowPass` first-order filter
  followed by the derivative-on-measurement form of `Control.PID`, with zero
  integral gain, zero derivative setpoint weight, `-15..15 percent` output
  limits, and the real `20 ms` cycle time. Its sampled frequency response shall
  be derived from those exact update equations, not from an unrelated
  continuous controller. Requested and applied commands shall remain separate
  so saturation cannot be hidden.
- **Prediction:** At zero frequency on the weakest `g_u=0.8` plant, the
  selected `Kp=10 %/g` gives loop gain `0.1*0.8*10=0.8`; sensitivity is
  `1/(1+0.8)=0.556`, so weighted low-frequency sensitivity is approximately
  `1.5*0.556=0.833`. Predict that this performance corner sets or nearly sets
  the selected design score. On the full sampled frequency grid, predict that
  nominal proportional control violates at least one bound and has at least
  one unstable discrete pole, while candidate `(10,3,0.05)` keeps all three
  weighted peaks below one and all poles inside the unit circle.

  For the corresponding continuous filtered-PD approximation, normalize the
  surface path so proportional and derivative coefficients are `1.0` and
  `0.3`, with filter time constant `0.05 s`. The cubic characteristic has
  positive coefficients, and its Routh-Hurwitz determinant reduces to
  `2*zeta*omega_n + 0.25*g_u*omega_n^2 + 0.2*zeta^2*omega_n^2 + 0.005*zeta*omega_n^3 + 0.03*g_u*zeta*omega_n^3`.
  This expression is the difference in the cubic Routh-Hurwitz condition
  `a2*a1>a3*a0` after forming the plant-plus-filter characteristic polynomial.
  Every term is positive for the declared positive physical parameters. This
  predicts continuous local stability, but it does not replace the sampled-
  pole or residual-uncertainty checks.
- **Controller under test:** The shipped Structured Text shall instantiate one
  nominal `Control.PID`, one robust `Control.LowPass`, and one robust
  `Control.PID`. It shall execute both controllers against independently
  restored equivalent plant copies, generate the same bounded gust record for
  both, and expose setpoint, both measured bending responses, gust load,
  requested and applied surface commands, saturation, controller enable, data
  quality, filter output, and controller identity. Concise comments shall
  explain the separate uncertainty/safety boundary and the filtered derivative
  realization; the program shall not contain frequency-grid results, accepted
  gamma, a hidden winner, simulated true parameters, or precomputed traces.
- **Experiment and exact Control Studio workflow:** Use exactly **Monitor** then
  **Test**. In Monitor, verify the flexible-wing project,
  Simulation class, zero setpoint, units, `20 ms` controller interval, `5 ms`
  plant substep, `-15..15 percent` applied limits, requested/applied commands,
  data quality, and zero disabled output. Before authority, derive the weighted
  paths and review the native candidate qualification. No disabled Identify,
  Tune, or Validate screen may be presented as a stage or evidence. In Test,
  select `Observe without excitation`, amplitude `0`, and duration `25.1 s`.
  The only writable lesson signal is Boolean `P1.ExperimentEnabled`; the project
  owns the identical `+0.10 g` gust from `5.0..6.0 s` and restores gust and both
  surfaces to zero when authority, input validity, or safety permission is lost.
  The candidate search, all 27 cases, and quality fault remain native tests and
  shall not be described as executed by the visible run.

  Abort for a wrong project, model, uncertainty set, residual weight, candidate
  set, frequency range or count, sample interval, substep, initial state, gust
  timing, limits, controller form, filter realization, sign, evidence class,
  or unequal comparison. Also abort for nonfinite state, an unstable selected
  pole, any weighted peak at or above one, requested/applied conflation,
  continued actuation from bad bending data, failed recovery, nonzero disabled
  output, or a claim beyond the tested ground-rig envelope.
- **Comparison:** First classify frequency cases as stable/unstable and
  eligible/ineligible. Report maximum weighted sensitivity, maximum weighted
  complementary sensitivity, maximum weighted control sensitivity, mixed-
  sensitivity score, worst plant, peak frequency, and maximum discrete-pole
  radius. For each time record, define peak absolute bending as the maximum
  `|y|` after gust onset in `g`; bending integrated absolute error as
  `sum(|y[k]|*0.02 s)` over `5..25 s` in `g*s`; settling time as the first time
  after gust removal that `|y|<=0.005 g` continuously for `2 s`; cumulative
  applied-surface movement as `sum(|u_applied[k]-u_applied[k-1]|)` in percentage
  points; peak requested and applied surface positions in percent; and
  saturation duration in seconds. An unstable or nonsettling run shall say so
  instead of receiving an equilibrium metric.
- **Validation:** The native test shall compile and execute the shipped
  Structured Text and shared Control library, match filter/controller updates
  to an independent scan oracle, and qualify the four candidates and nominal
  controller independently. Candidate `(10,3,0.05)` shall have all sampled
  weighted peaks below `1.0`, all sampled-loop poles strictly inside the unit
  circle with a declared numerical tolerance, and the lowest eligible gamma in
  the declared candidate set. Nominal proportional control shall be rejected
  from its actual failed bound or pole evidence, not from its label. The robust
  time response shall remain finite and unsaturated at every declared case and
  shall improve worst-case bending IAE and settling over the nominal baseline;
  any nominal unstable/nonsettling case remains visible. Bad quality shall
  apply exactly zero surface command, healthy restoration shall resume bounded
  control without a discontinuity beyond the declared limit, and disabled
  output shall be zero. Measured results shall be inserted only after this test
  passes.
- **Explanation:** Trace low-frequency rejection into `S`, resonance and model
  error into `T`, surface demand into `KS`, and each weight into its engineering
  requirement. Explain why the aggressive nominal controller's sampled delay
  consumes damping, why filtered derivative action adds damping, why the robust
  design accepts less nominal aggressiveness, and why one scalar gamma cannot
  replace the three component peaks or time-domain actuator evidence. Explain
  the difference between an analytic local-stability result, a sampled-grid
  numerical qualification, and a flight-worthy guarantee.
- **Transfer and required conclusion:** Robust design trades nominal ambition
  and controller effort for explicitly bounded worst-case behavior. Its claim
  covers only the stated model set, residual envelope, frequency range,
  sampling, actuator, sensor, and operating condition. It says nothing about
  unmodelled modes outside the envelope, nonlinear aeroelastic effects,
  structural damage, flutter clearance, certification, or flight safety.
  Lesson 73 then adds communication latency, jitter, loss, reordering, and stale
  data because a controller qualified on a local deterministic link is not yet
  qualified for a remote pumping network.

### 3.13 Networked and fault-aware control

#### Lesson 73 — Networked control: remote pumping station

- **Objective:** Qualify the communication contract for a remotely coordinated
  high-zone water-pumping station. The learner shall calculate the remote
  loop's delay margin, construct a conservative latency budget, compare an
  arrival-order-only receiver with a timestamped bounded receiver under the
  same packet record, and decide when the station must transfer to local
  pressure control.
- **Context:** A variable-speed pump maintains a distribution-header pressure
  of `6 bar`. The local pressure transmitter is the process-value source, and
  the drive accepts an applied pump-speed command from `0` through `100 %`.
  Under healthy communication, a reviewed remote coordinator sends a requested
  pump-speed command. A local pressure controller remains available at the
  station and does not depend on the network. The exercise uses two equivalent
  simulated headers: the naive receiver and protected receiver see the same
  delivered packets, while their independently applied pump commands drive
  separate plant copies. This is deterministic Simulation evidence. The
  lesson does not claim a measured transport, synchronized physical clocks,
  authenticated packets, redundant networks, certified pump protection, or
  permission to control a physical pumping station.
- **Reason:** Lesson 72 qualified a controller only on a local deterministic
  sampled path. A remote loop adds time between measurement, calculation and
  actuation, and that time consumes phase margin. Variable delivery time adds
  jitter; missing packets create loss; later packets can arrive before earlier
  ones; retained data become stale; and clocks can disagree. Controller gains
  that are acceptable locally do not establish that a remotely delivered
  command is current or that the resulting loop remains stable.
- **Theory before practice:** Explain every communication term in physical
  words before introducing its name or symbol:
  - A source timestamp is the time at which the remote command was produced,
    expressed in the agreed synchronized time domain. The source timestamp
    (`t_source`) is carried in the packet. The station's current local time
    (`t_local`) is generated locally and shall not be supplied by the sender.
  - Nominal packet age is elapsed time since production. Only after defining
    both clocks write `age_nominal = t_local - t_source`.
  - Clock uncertainty (`epsilon_clock`) bounds the remaining possible
    synchronization error. Conservative packet age is the greatest credible
    age, `age_worst = age_nominal + epsilon_clock`. A timestamp farther in the
    future than the same uncertainty permits is invalid. The lesson shall not
    present a sender-supplied `DataFresh` flag or age field as freshness proof.
  - Latency is elapsed time through one path. Round-trip loop delay (`L_total`)
    includes measurement transport, remote calculation, command transport,
    bounded jitter, clock uncertainty and at most one local task interval.
    Jitter is variation in latency, not merely a slow packet. Packet loss means
    an expected update never arrives. Reordering means a smaller or repeated
    sequence number arrives after a newer accepted packet.
  - A sequence number (`n`) establishes production order only within its
    declared restart and wrap policy. In this bounded exercise, a packet is
    newer only when `n > n_last`; restart and integer wrap require a separately
    specified session protocol and are outside the lesson.
  - A jitter or resequencing buffer deliberately waits for data to reduce
    timing variation or restore order. Waiting consumes delay margin. This
    station therefore uses a zero-wait, latest-eligible policy: it rejects a
    late or reordered command instead of delaying actuation in hope that an
    older gap will be filled. The accepted command is retained only until its
    timestamp expires; that retained value is not called a reorder buffer.
  - Delay margin (`L_margin`) is the additional pure delay that brings the
    declared linear remote loop to its stability boundary. For the lesson's
    first-order pressure plant and remote proportional controller, first define
    plant gain (`K_plant`), plant time constant (`tau`), remote proportional
    gain (`K_remote`), gain-crossover angular frequency (`omega_c`), and plant
    phase lag. Then write
    `omega_c = sqrt((K_remote K_plant)^2 - 1) / tau` and
    `L_margin = (pi - atan(omega_c tau)) / omega_c` when
    `K_remote K_plant > 1`. Explain that this is a linear-model calculation,
    not proof for packet logic, nonlinear hydraulics, or every network state.
    Derive both expressions rather than presenting them as rules to memorize.
    Starting from
    `L(s) = K_remote K_plant exp(-s L) / (tau s + 1)`, the pure delay has
    magnitude one, so gain crossover satisfies
    `1 = K_remote K_plant / sqrt(1 + (omega_c tau)^2)`; squaring and solving
    gives the stated `omega_c`. At that frequency the first-order plant
    contributes phase `-atan(omega_c tau)` and the delay contributes
    `-omega_c L`. The stability boundary is the point where their sum reaches
    `-pi`, so solving
    `atan(omega_c tau) + omega_c L_margin = pi` gives the delay-margin
    expression. This derivation also explains why seconds result: radians are
    dimensionless and dividing phase by radians per second leaves seconds.
  - A latency budget is the sum of reviewed worst-case contributions:
    `L_budget = L_measure + L_compute + L_command + L_jitter + epsilon_clock + T_task`.
    Delay reserve is `L_reserve = L_margin - L_budget`. The remote design is
    ineligible when the reserve is non-positive even if a short nominal run
    appears smooth.
  - The naive receiver clamps and applies every delivered demand in arrival
    order, then retains the last arrival indefinitely. The protected receiver
    accepts a packet only when remote operation is enabled, signal quality is
    good, clocks are synchronized, clock uncertainty is within `0.05 s`, the
    source timestamp is not impossibly future-dated, conservative age is no
    greater than `0.50 s`, its sequence is newer, and the demand is finite and
    within the declared command range. A received ineligible packet latches a
    communication fault and selects local control. A later eligible packet may
    clear that communication fault only through the declared recovery path.
    Loss without a receive pulse selects local control when the retained
    packet's conservative age exceeds `0.50 s`.
- **Physical and controller relationships:** The independently simulated
  header plants are self-regulating first-order models. Pressure moves toward
  the zero-speed pressure plus pump gain times applied speed. Introduce header
  pressure (`p`) in bar, applied pump speed (`u_applied`) in percent,
  zero-speed pressure (`p_0`) in bar, plant gain (`K_plant`) in bar per percent,
  and time constant (`tau`) in seconds before writing
  `dp/dt = (p_0 + K_plant u_applied - p) / tau`.
  Use `p_0 = 4 bar`, `K_plant = 0.04 bar/%`, and `tau = 4 s`; therefore
  `50 %` pump speed holds `6 bar` in the declared model. The local fallback is
  the shipped positional `Control.PID` configured as PI control from the live
  local pressure measurement, with requested output separate from the
  `0..100 %` applied drive command. Its integral state shall track while remote
  mode owns the drive so transfer to local control is bumpless within the
  declared tolerance.
- **Prediction:** Use remote proportional gain `200 %/bar`. The loop's static
  gain is `8`, so the learner shall calculate
  `omega_c = sqrt(8^2 - 1) / 4 = 1.984 rad/s`. The plant phase lag there is
  `atan(1.984 * 4) = 1.446 rad`, giving a model-based delay margin of
  approximately `0.855 s`. With measurement transport `0.10 s`, calculation
  `0.02 s`, command transport `0.15 s`, bounded jitter `0.05 s`, clock
  uncertainty `0.05 s`, and one `0.10 s` station scan, calculate a conservative
  delay budget of `0.47 s` and reserve of approximately `0.385 s`. Predict that
  a fresh `50 %` packet is accepted, a `0.60 s` conservative-age packet is
  rejected, loss expires the retained command no later than one station scan
  after the `0.50 s` threshold, a repeated or lower sequence selects local
  control, and a later eligible packet restores remote ownership without an
  applied-command step larger than the declared bumpless tolerance.
- **Controller under test:** Show the shortest actual Structured Text excerpts
  that calculate nominal and conservative age from local time and the packet
  source timestamp, qualify a newly received sequence, retain only an eligible
  command, expire the retained command, latch an ineligible receive, select
  remote or local ownership, track the inactive local PI controller, and clamp
  the applied drive command. The excerpt shall expose both naive and protected
  requested/applied commands. It shall not replace the shipped code with
  pseudocode.
- **Experiment:** Use a `100 ms` controller task, `20 ms` plant substep,
  deterministic seed `20260829`, identical initial pressure `6 bar`, identical
  plant parameters and one predeclared delivered-packet record. Retain local
  arrival time, source timestamp, conservative age, sequence, command, quality,
  synchronization, clock uncertainty, receive pulse, acceptance/rejection
  reason, communication-fault latch, ownership, requested/applied outputs and
  both pressures. Run these events without changing controller settings:
  1. Establish a healthy `50 %` remote command.
  2. Deliver eligible sequence `1` at conservative age `0.10 s`.
  3. Omit updates until the accepted packet expires.
  4. Deliver a new but stale packet at conservative age `0.60 s`.
  5. Deliver newer sequence `3`, then delayed sequence `2`, to expose arrival
     order versus production order.
  6. Deliver a packet with bad quality, one with invalid synchronization, one
     with excessive clock uncertainty, and one future-dated beyond its bound.
  7. Restore a healthy newer packet through the explicit recovery path.
  Abort if a pressure or command is non-finite, pressure leaves `0..12 bar`, or
  applied speed leaves `0..100 %`.
- **Comparison and decision measures:** Define each measure before presenting
  a result. Invalid-packet acceptance count is the number of stale, reordered,
  bad-quality, unsynchronized, excessive-uncertainty or impossible-future
  packets that gain command ownership; lower is better and the protected value
  shall be zero. Safe-transfer latency is elapsed time from the first scan on
  which a retained command becomes ineligible to local ownership; it shall be
  no greater than one `100 ms` scan. Recovery discontinuity is the absolute
  difference between consecutive applied pump-speed samples at approved
  recovery, in percentage points; it shall be no greater than `2.0 percentage
  points`. Pressure absolute-error area is the sampled integral of
  `|6 bar - p|` over the declared impairment window, in `bar*s`; lower is
  better. Peak pressure departure is the largest absolute departure from
  `6 bar` in the same window, in bar. Applied pump-speed travel is total
  variation of consecutive applied commands in percentage points. Remote
  availability is the fraction of the run in qualified remote ownership; it is
  descriptive and shall not be maximized by accepting ineligible packets.
- **Validation:** A native executable test shall compile and execute the
  shipped Structured Text with the shared Control library against both
  simulated headers and an independent packet-policy/plant oracle. It shall
  reproduce every impairment, confirm the analytic delay-margin and budget
  calculations within declared tolerances, prove zero protected invalid-packet
  acceptances, one-scan expiry/fault transfer, bounded recovery, finite/ranged
  outputs, and source/plant agreement. It shall also prove that the naive path
  accepts or retains at least the stale and reordered commands so the lesson
  contains a real comparison rather than two equivalent implementations.
  A registered learner-contract test shall prove the complete nine-stage
  explanation, words before symbols, actual Structured Text, metric
  definitions, Control Studio procedure, and Simulation/physical boundary.
  Measured results and figures shall be inserted only after native green.
- **Explanation:** Trace an eligible packet from source timestamp and sequence
  through age calculation, acceptance, retained authority, ownership,
  requested output, applied output and pressure. Trace each impairment to its
  rejection reason and local fallback. Explain why a naive last-arrival rule
  can accept an older production command, why retaining a command indefinitely
  converts packet loss into stale actuation, why a larger reorder buffer may
  reduce jitter but consume delay reserve, why local fallback remains stable
  without the network, and why a positive model-based delay reserve does not
  prove transport integrity or physical hydraulic safety.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor first confirms finite pressures and commands,
  healthy quality/synchronization inputs, and local ownership before any run.
  In Test select `Observe without excitation`, keep amplitude at `0`, and run
  for `25.1 s`. Control Studio may write exactly one Boolean,
  `P1.ExperimentEnabled`; it must not write packet times, sequence numbers,
  demands, quality flags, synchronization flags, controller ownership, or
  pump commands. While that Boolean is true, the runnable project owns the
  visible healthy stream: it advances local time, produces a `50 %` packet
  every `0.4 s`, and gives each packet a `0.05 s` nominal age. Removing the
  Boolean disables remote ownership and restores the declared local-safe
  baseline. The native executable test, not a hidden or disabled workflow
  stage, owns stale, lost, reordered, bad-quality, unsynchronized,
  excessive-uncertainty, future-dated and recovery cases. Identify, Tune and
  Validate are therefore not learner steps for this lesson.
- **Transfer and required conclusion:** Network timing and message validity are
  part of the feedback and safety contracts. A remotely coordinated plant
  needs locally generated time, bounded synchronization uncertainty, explicit
  freshness/order/session rules, an independently stable fallback, bumpless
  ownership transfer, and evidence using the deployed transport. This lesson
  qualifies only deterministic injected impairments around a first-order
  simulated header. Lesson 74 is required because timely, ordered packets do
  not prove that sensors and actuators update at compatible rates or remain
  healthy.

#### Lesson 74 — Multirate, redundant and fault-aware control: pumping station

- **Objective:** Commission fault-aware local control for the same high-zone
  pumping station. The learner shall combine fast redundant pressure
  transmitters, a slower suction-reservoir level transmitter, and independent
  drive-speed feedback; detect and isolate the declared sensor and actuator
  faults; select a qualified degraded mode or safe output; and restore
  automatic operation only through an explicit supervised sequence.
- **Context:** The protected station from Lesson 73 remains local and does not
  depend on the remote network for its safety function. Two physical discharge
  pressure transmitters measure the same header in bar every `100 ms`. A
  suction-reservoir level transmitter reports percent full every `500 ms`.
  Variable-speed-drive feedback reports actual pump speed in percent every
  `200 ms`, independently of the requested speed. The drive request is limited
  to `0..100 %` and is rate limited to `10 %/s` during restoration. Two
  equivalent simulated header paths compare an unprotected controller that
  always trusts pressure transmitter A with a fault-aware controller. Sensor
  bias, quality loss, missing updates, low suction level and a stuck actuator
  are deterministic Simulation injections. They are not evidence from a
  physical transmitter, drive, reservoir, pump or network.
- **Reason:** Lesson 73 established that a packet can be timely, ordered and
  synchronized. Those properties say nothing about whether its sensor value
  is plausible, whether two devices agree, whether one has drifted, whether a
  slow measurement is still safe to hold, or whether the actuator followed the
  requested command. Treating every timely value as correct leaves the loop
  vulnerable to component faults and mismatched device update rates.
- **Theory before practice:** Explain each concept in ordinary words before
  notation or state-machine names:
  - A sample interval is the time between new measurements from one device.
    Multirate sampling means connected devices use different intervals. The
    station task runs every `100 ms`; pressure is new every task, level every
    fifth task, and drive feedback every second task.
  - Resampling converts observations onto the controller's task timeline. A
    zero-order hold retains the latest qualified sample between device updates.
    Holding does not make a sample new. Each retained value keeps its source
    timestamp and age. Linear interpolation shall not invent a future safety
    measurement between two level samples.
  - Freshness is a declared maximum age, not merely a quality bit. Pressure age
    shall be no greater than `0.25 s`, level age no greater than `1.0 s`, and
    drive-feedback age no greater than `0.40 s`. A watchdog is the stateful
    supervision that detects when an expected update or execution event has
    not occurred before its deadline.
  - Plausibility asks whether a value is finite, inside a physical range, and
    consistent with limits on change. Use pressure range `0..12 bar`, level
    range `0..100 %`, and speed-feedback range `0..100 %`. A value inside range
    can still be biased and therefore requires consistency checks.
  - Physical redundancy uses independent devices measuring the same physical
    quantity. Analytical redundancy compares a device with a prediction from a
    qualified model. Two disagreeing transmitters alone reveal disagreement
    but cannot identify which one is wrong. The pressure-model prediction
    supplies the third independent relation needed for the bounded isolation
    rule in this lesson.
  - Pressure discrepancy (`r_AB`) is the absolute difference between pressure
    transmitter A (`p_A`) and pressure transmitter B (`p_B`). Model residuals
    (`r_A` and `r_B`) are their absolute differences from predicted pressure
    (`p_hat`). Only after defining the quantities write
    `r_AB = |p_A - p_B|`, `r_A = |p_A - p_hat|`, and
    `r_B = |p_B - p_hat|`.
  - The model prediction uses the preceding applied speed and the same declared
    first-order local model as Lesson 73. Introduce predicted pressure
    (`p_hat[k]`), preceding prediction (`p_hat[k-1]`), preceding applied speed
    (`u_applied[k-1]`), task interval (`T`), zero-speed pressure (`p_0`), plant
    gain (`K_plant`) and time constant (`tau`) before writing the executable
    Euler prediction
    `p_hat[k] = p_hat[k-1] + (T/tau)(p_0 + K_plant u_applied[k-1] - p_hat[k-1])`.
    Derive this from the continuous balance
    `d p_hat/dt = (p_0 + K_plant u_applied - p_hat)/tau`: approximate the
    derivative over one task by `(p_hat[k] - p_hat[k-1])/T`, substitute the
    preceding command so the calculation is causal, and solve for `p_hat[k]`.
    With `T = 0.1 s` and `tau = 4 s`, `T/tau = 0.025`, which is the coefficient
    visible in the Structured Text.
    This predictor is a diagnostic relation, not a controller measurement and
    not Simulation truth.
  - A pressure discrepancy greater than `0.30 bar` for three consecutive new
    pressure samples creates a confirmed disagreement. If exactly one model
    residual is greater than `0.50 bar` while the other is no greater than
    `0.50 bar`, isolate the larger-residual transmitter and continue in
    degraded single-sensor control. If both or neither can be identified, the
    fault is ambiguous and automatic pressure control loses authority.
  - Conservative resampling protects a slowly sampled constraint. The maximum
    credible reservoir drawdown rate (`v_level`) is `0.5 percentage points/s`.
    Retained level (`level_sample`) and its age (`age_level`) give conservative
    available level (`level_safe`):
    `level_safe = level_sample - v_level age_level`.
    The subtraction is a worst-case bound: during the unobserved age, level
    could have fallen by as much as rate times time. It is not an estimate of
    the most likely level.
    The low-suction trip is `20 %`; supervised restoration requires at least
    `30 %`. This projection does not replace the `1.0 s` freshness deadline.
  - Actuator tracking residual (`r_u`) is the absolute difference between
    requested speed (`u_requested`) and measured drive speed (`u_feedback`):
    `r_u = |u_requested - u_feedback|`. Residual greater than
    `5 percentage points` for three consecutive new drive-feedback samples
    confirms a tracking fault. Because feedback updates every `200 ms`, the
    third affected sample arrives two sample gaps, or `0.4 s`, after the first;
    the `0.6 s` requirement also covers a fault beginning just after an update
    and waiting nearly one interval to become observable.
  - Fault detection decides that evidence violates a rule. Isolation assigns
    the fault to a declared component only when evidence distinguishes it.
    Graceful degradation continues a reduced service with fewer qualified
    measurements. A safe state removes automatic pump authority when the
    remaining evidence cannot support control. Acknowledgement records operator
    awareness; it does not clear a fault. Reset approval permits restoration
    only after the cause is absent and all required evidence has remained
    healthy for ten consecutive `100 ms` station scans.
- **Prediction:** At the normal equilibrium, both pressure transmitters and the
  predictor read approximately `6 bar`, so all three pressure residuals are
  approximately zero. Injecting `+1.0 bar` into transmitter A predicts
  `r_AB approximately 1.0 bar`, `r_A approximately 1.0 bar`, and
  `r_B approximately 0 bar`; transmitter A shall therefore be isolated on the
  third new pressure sample, no later than `0.30 s` after the first biased
  sample. During the same interval, the unprotected controller will interpret
  the false high pressure as real and reduce pump speed. Predict that loss of
  both pressure qualities removes automatic authority in one station scan,
  that a `19 %` reservoir sample trips immediately, and that a stuck drive is
  confirmed on the third `200 ms` feedback update. Predict no automatic reset,
  a required `1.0 s` healthy qualification, and at most `1.0 percentage point`
  of applied-speed change per `100 ms` restoration scan from the declared
  `10 %/s` rate limit.
- **Controller under test:** Show concise excerpts from the actual Structured
  Text for timestamped sample retention, freshness and plausibility, the model
  predictor, three-sample pressure disagreement confirmation, residual-based
  sensor isolation, conservative level projection, three-update actuator
  watchdog, deterministic fault priority, degraded/safe mode selection,
  acknowledgement, ten-scan recovery qualification, approved reset, PID
  tracking, requested output, rate-limited applied output and actuator
  feedback. Simulation fault controls or truth channels shall not appear in
  the controller's decision path.
- **Experiment:** Use one `100 ms` station task, `20 ms` plant substeps, seed
  `20260829`, identical first-order header models and the same `6 bar` initial
  state. Pressure A and B source timestamps update every `100 ms`, reservoir
  level every `500 ms`, and drive feedback every `200 ms`. Retain raw and held
  values, source timestamps, ages, quality, plausibility, residuals, counters,
  isolated component, active measurement, operating mode, fault code,
  acknowledgement, recovery count, requested/applied/feedback speeds, and both
  pressures. Run restored cases under otherwise equal conditions:
  1. healthy multirate operation with no false trip;
  2. a `+1.0 bar` bias on pressure transmitter A while quality remains good;
  3. loss of A alone, then loss of both pressure qualities;
  4. missing slow level updates until the freshness watchdog expires;
  5. a fresh `19 %` low-level sample followed by restored level above `30 %`;
  6. a stuck drive whose feedback differs from request for three new feedback
     samples, followed by physical release; and
  7. acknowledgement, ten healthy scans, explicit reset approval and
     rate-limited restoration after each latched safe-state fault.
  Abort if required data are non-finite, either pressure leaves `0..12 bar`, or
  requested/applied speed leaves `0..100 %`.
- **Comparison and decision measures:** Sensor-isolation latency is time from
  the first biased new sample through confirmed isolation, in seconds, and
  shall be no greater than `0.30 s`. Actuator-fault confirmation latency is
  time from the first mismatched new feedback through the latched fault, in
  seconds, and shall be no greater than `0.60 s`. False-trip count is the
  number of fault or degraded transitions during the healthy run and shall be
  zero. Wrong-sensor selection count is the number of scans in which an
  isolated or ineligible pressure value drives the protected controller and
  shall be zero. Pressure absolute-error area and peak pressure departure from
  `6 bar` use the declared fault window and units `bar*s` and `bar`.
  Safe-output latency is time from an ambiguous, low-level, stale-critical, or
  actuator-fault decision through zero requested output and shall be no greater
  than one station scan after confirmation. Restoration rate is the greatest
  consecutive applied-speed change divided by `0.10 s`, in percent per second,
  and shall not exceed `10 %/s`. Recovery qualification time, selected mode,
  and service availability are reported separately; availability shall not be
  inflated by using ineligible data.
- **Validation:** A native executable test shall compile and execute the
  shipped Structured Text and Control library against the declared simulated
  sensor/plant/actuator records. An independent oracle shall reproduce sample
  arrivals and ages, predictor updates, residuals, confirmation counters,
  isolation, conservative level, watchdog, fault priority, mode ownership,
  recovery qualification and plant response. It shall prove the healthy case,
  single-sensor degraded case, ambiguous-pressure safe case, stale-level case,
  low-suction trip, actuator mismatch, non-clearing acknowledgement, premature
  reset rejection, approved recovery and bounded restoration. A registered
  learner-contract test shall prove the nine-stage causal explanation,
  words-before-symbols theory, actual ST, measures, Control Studio actions and
  Simulation/physical boundary. Results shall be inserted only after native
  green.
- **Explanation:** Trace each device from its own update interval and timestamp
  through retention, age, plausibility and residuals. Explain why quality-good
  biased data require redundancy; why two disagreeing devices need an
  independent relation for isolation; why degraded single-sensor control is
  permitted only for an identified fault; why held slow level must remain aged
  and conservatively projected; why a drive command is not evidence of drive
  motion; and why acknowledgement, cause removal, health qualification and
  reset are separate recovery steps.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor first confirms current finite pressure, level,
  feedback, requested/applied command, and mode signals. In Test select
  `Observe without excitation`, use amplitude `0`, and run for `28.1 s`.
  Control Studio may write exactly one Boolean, `P1.ExperimentEnabled`. The
  runnable project owns the complete device-rate and fault schedule,
  acknowledgements, reset approvals, setpoint changes, and restoration. On
  removal of authority it restores healthy qualities, sample pulses, the
  `6 bar` setpoint, `35 %` level, and false acknowledgement/reset requests.
  The native executable test owns the independent oracle and the pass/fail
  decisions. Identify, Tune and Validate are not learner steps and disabled
  generic controls must not be presented as evidence.
- **Transfer and required conclusion:** Fault handling is an explicitly
  designed operating mode with declared evidence, priorities, authority and
  recovery. It is not an exception hidden in the controller equation. This
  lesson covers named deterministic faults and model-consistent residuals in
  Simulation. It does not prove diagnostic coverage for every failure or
  authorize a physical pump. Lesson 75 is required because a commissioned loop
  can degrade gradually or exhibit overlapping symptoms whose cause cannot be
  selected from one threshold; those symptoms must be diagnosed before gains
  are changed.

### 3.14 Performance monitoring and diagnosis

#### Lesson 75 — Diagnose before retuning: operating process area

- **Objective:** Diagnose eight blinded retained records from an industrial
  steam-distribution area and recommend the next evidence-producing action.
  The learner shall distinguish a healthy baseline, poor controller tuning,
  valve stiction, flow-transmitter drift, actuator saturation, a measured
  upstream disturbance, interaction from another steam user, and
  operating-region model or schedule mismatch without changing controller
  gains before the evidence supports that remedy.
- **Context:** A modulating valve controls process steam flow to one production
  user. Steam-flow setpoint and measured flow use `t/h`; requested valve demand
  and measured valve-stem position use percent; upstream header pressure uses
  `bar`; an independently maintained reference flow meter supplies diagnostic
  evidence in `t/h`; the adjacent user's valve demand uses percent; operating
  region is a discrete state; quality, mode, limits, schedule transitions and
  task timing are retained with every sample. The normal local process is a
  first-order-plus-dead-time flow path with gain `1.0 t/h/%`, time constant
  `5 s`, delay `1 s`, and a `100 ms` controller task. The baseline PI controller
  has proportional gain `2.0 %/(t/h)` and integral gain `0.2 %/(t/h*s)`.
  Every case is an executable deterministic Simulation run. No record is
  physical-plant evidence and no injected case label is visible to the
  controller or learner during diagnosis.
- **Reason:** Lesson 74 detected named faults from dedicated redundant signals
  and thresholds. A commissioned process area often degrades more gradually,
  and several causes can create a similar oscillating or offset process-value
  trace. Retuning from the process value alone can conceal a failing valve,
  calibrate the controller around a biased instrument, demand unavailable
  actuator authority, or amplify interaction. The next action must follow a
  causal signature and a test capable of rejecting plausible alternatives.
- **Theory before practice:** Explain all terms and signals in words before
  symbols, calculations or plots:
  - A performance baseline is a qualified healthy record at the same operating
    region, setpoint range, controller revision, actuator limits, sample
    interval and evidence class. It supplies expected variability and response
    timing; it is not an eternal definition of normal operation.
  - An event is a bounded change with a declared onset, duration and source.
    Event detection identifies when a signal departs from its baseline band.
    Diagnosis requires synchronized samples so the order of cause, controller
    reaction, actuator motion and process response can be compared.
  - Process error (`e`) is steam-flow setpoint (`SP`) minus measured steam flow
    (`PV`). Requested manipulated variable (`MV_requested`) is the controller's
    valve demand. Applied manipulated variable (`MV_applied`) is measured valve
    position. These shall not be conflated.
  - Integrated absolute error (`IAE`) measures accumulated flow error over the
    declared window with sample interval (`T`):
    `IAE = sum(|SP[k] - PV[k]| T)`. It reports degradation but does not identify
    its cause. Derive the sum as a rectangle approximation to the continuous
    area under `|e(t)|`: each absolute-error sample is a rectangle height and
    `T` is its width. This also derives the resulting unit `t/h*s`.
  - Limit occupancy is the fraction of eligible samples at the lower or upper
    applied-valve limit. A persistent error while applied position remains at
    `100 %` proves unavailable authority. Increasing controller gain cannot
    create additional valve or steam capacity.
    Calculate it as
    `100 N_limit / N_eligible`, where `N_limit` counts eligible samples at the
    relevant limit. Invalid or missing samples belong in neither count.
  - An oscillation is a repeatable alternating component, not any noisy trace.
    Oscillation period (`P_osc`) is time between equivalent consecutive peaks.
    The normalized cross-correlation lag from requested valve movement to
    process flow estimates which signal consistently leads the other. Poor
    tuning is eligible only when the requested valve moves first, measured stem
    follows without hidden hold/jump behavior, no limit is active, disturbance
    and interaction signals do not lead, measurement quality is eligible, and
    the oscillation repeats under the same operating condition.
  - Valve stiction is static friction that holds measured stem position while
    requested demand continues to move, followed by a breakaway jump. Define
    held-demand travel (`TV_hold`) as total requested-demand variation while
    measured stem movement remains inside `0.2 percentage points`, and stem
    jump (`Delta_stem`) as the subsequent applied-position change. A confirmed
    stiction signature requires repeated hold/ramp/jump cycles; an oscillating
    process value alone is insufficient.
  - Sensor bias is a persistent measurement offset. Sensor drift is a slowly
    changing offset. Flow disagreement (`d_flow`) is the primary flow
    transmitter minus the independent reference meter:
    `d_flow[k] = PV[k] - PV_reference[k]`.
    Drift slope is change in that disagreement divided by elapsed time. A
    process change affects both qualified meters; divergence isolated to one
    meter supports an instrumentation action, not retuning.
    For the bounded endpoint estimate, write
    `m = (d_flow[b] - d_flow[a]) / (t[b] - t[a])`. Subtracting the endpoints
    removes a constant offset; dividing by elapsed seconds makes the remaining
    change a rate in `t/h/s`.
  - A measured load disturbance is an exogenous signal that changes before the
    controlled flow and controller response. Upstream-pressure departure
    (`d_header`) is actual header pressure minus its baseline. The retained
    event times shall establish `header -> flow -> valve response`; correlation
    without precedence is not a causal diagnosis.
  - Interaction occurs when another loop's manipulated action changes the
    shared process before the diagnosed loop reacts. Adjacent-user valve demand
    is therefore retained. A repeating `other MV -> header/flow -> local MV`
    order supports interaction testing or coordination; it does not by itself
    prove a particular decoupler.
  - Model residual (`epsilon_model`) is measured flow minus the independently
    predicted flow from the commissioned local model. Mean residual, residual
    trend and lag structure are evaluated only where inputs, measurements and
    model validity are eligible. Residual change aligned with an operating-
    region transition can indicate an incorrect local model or schedule
    boundary. A gain change is not justified until the model and schedule are
    revalidated.
  - A diagnostic hypothesis names a possible cause. A discriminating test is a
    bounded next action whose outcomes differ between the two leading
    hypotheses. Confidence is a statement about evidence sufficiency, not a
    probability invented without a calibrated statistical model. When required
    signatures overlap, contradict each other, or lack the needed signal, the
    result shall be `Inconclusive` with a request for the missing evidence.
- **Prediction:** Before revealing any case, predict the discriminating order
  and remedy:
  1. Healthy baseline has eligible quality, no sustained limit occupancy, no
     repeated stem hold/jump, bounded model residual and no unexplained event.
  2. Poor tuning has repeatable oscillation with requested valve demand leading
     measured stem and flow, while stem tracks request and no external or limit
     signal leads. Only this case may proceed to a controlled tuning comparison.
  3. Stiction has at least three repeated demand-ramp/stem-hold/breakaway cycles
     with held-demand travel greater than `2 percentage points` and stem jumps
     greater than `1 percentage point`; the next action is valve inspection or
     a reviewed valve travel test.
  4. Flow-transmitter drift produces disagreement slope greater than
     `0.01 t/h/s` against the qualified reference while valve position and
     reference flow remain consistent; the next action is instrument
     verification or calibration.
  5. Saturation has applied valve position at `100 %` for more than `90 %` of
     the declared event window while positive error persists; the next action
     concerns capacity, operating demand or equipment, not higher gain.
  6. Upstream disturbance shows header-pressure event before both flow and
     corrective local valve movement; the next action is disturbance-source
     investigation or a separately justified feedforward design.
  7. Interaction shows adjacent-user valve demand before the shared-header and
     local-loop response on at least three events; the next action is
     interaction testing and coordination, pairing or decoupling review.
  8. Model/schedule mismatch begins at a retained operating-region transition,
     leaves sensors and valve eligible, and creates a persistent model residual;
     the next action is model and schedule validation before any retuning.
- **Controller and evidence source under test:** Show the concise actual
  Structured Text that configures and calls the baseline PI, keeps controller
  request separate from measured stem position, exposes independent reference
  flow, header pressure, adjacent-user demand, quality, operating region,
  schedule transition and cycle time, and applies no diagnostic label or
  automatic gain change. Scenario generation belongs to Simulation
  configuration and retained artifacts, not hidden branches in the controller.
- **Experiment:** Generate and retain eight artifacts: one qualified healthy
  baseline plus seven degraded blinded cases. Each artifact shall contain the same
  project identity, controller revision, `100 ms` interval, duration, units,
  time origin, operating-point fields and evidence label. The deliberate
  independent variable and scenario-specific plant/sensor/actuator event shall
  be declared in a sealed answer key, not in the learner-facing trace. Run
  every case through Control Studio without changing gains:
  1. Monitor data quality, synchronization, mode, limits and operating region.
  2. Test or open the retained bounded event and mark its onset without viewing
     the answer key.
  3. Identify the two leading hypotheses, required signatures and missing
     signals; calculate only metrics eligible for that record.
  4. In Tune/Design, leave gains unchanged unless the poor-tuning signature is
     uniquely supported. For other cases choose the instrument, valve,
     capacity, disturbance, interaction, model/schedule or evidence-gathering
     action.
  5. Validate the hypothesis with one discriminating follow-up test, then
     reveal the answer key and record correct, incorrect or inconclusive.
  Abort classification when timestamps are unsynchronized, required quality is
  invalid, an essential signal is missing, controller revision differs from
  the baseline without declaration, or a metric's prerequisite fails.
- **Comparison and decision measures:** Diagnostic coverage is the number of
  declared cases for which the required signals and rules can reach a result;
  it is not a claim about undeclared faults. Correct diagnosis count, incorrect
  diagnosis count and justified inconclusive count are reported separately.
  Remedy-selection accuracy counts cases whose chosen next action matches the
  evidence-qualified action; it shall not treat “retune” as a default. Event
  precedence is reported in seconds with uncertainty of one sample. Oscillation
  period uses seconds; limit occupancy uses percent of eligible samples;
  held-demand travel and stem jump use percentage points; flow-disagreement
  slope uses `t/h/s`; model-residual mean uses `t/h`; IAE uses `t/h*s`.
  Every metric retains its signal sources, window, sample interval, validity
  condition and preferred direction. A case with missing discriminating
  evidence shall remain `Inconclusive`, not be forced into the nearest label.
- **Validation:** A native executable test shall compile and execute the actual
  Structured Text and Control library to create the baseline and all eight
  deterministic scenario records from shipped configurations. An independent
  diagnostic oracle shall calculate the declared signatures without reading
  the sealed scenario label, return exactly one supported diagnosis or
  `Inconclusive`, select the evidence-qualified remedy, and compare the result
  with the answer key only after classification. It shall prove that all eight
  complete records are classified correctly, that a deliberately missing stem
  signal leaves the tuning-versus-stiction case inconclusive, and that no case
  changes controller gains automatically. A registered learner-contract test
  shall prove the full nine-stage causal explanation, words-before-symbols
  theory, actual ST, blind workflow, defined metrics, decision boundaries and
  Simulation/physical distinction. Measured results and figures shall be
  inserted only after native green.
- **Explanation:** For each case, narrate the time order of external event,
  measured process response, controller request, actual valve motion and
  recovery. State which alternative hypothesis was rejected and by which
  signal. Explain why oscillation is a symptom rather than a cause, why pinned
  output is an authority problem, why sensor-reference disagreement is not a
  tuning error, why stiction requires stem feedback, why external precedence
  matters for disturbance and interaction, and why schedule-aligned residuals
  require model validation.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor establishes the qualified reference signals and
  a prediction. Test uses `Observe without excitation`, amplitude `0`, and
  duration `60.1 s`; Control Studio may write only `P1.ExperimentEnabled`.
  The visible runnable project owns a healthy reference sequence: `50 t/h`
  for `10 s`, `60 t/h` through `40 s`, then recovery to `50 t/h`. Removing
  authority disables the controller and restores `50 t/h`. The native test
  owns the seven degraded blinded configurations, metric calculations, answer
  reveal and pass/fail decision. Identify, Tune and Validate are not learner
  steps, and their disabled generic controls are not diagnostic evidence.
- **Transfer and required conclusion:** Retuning is one maintenance action and
  is allowed only after diagnosis supports a controller-dynamics cause and a
  controlled comparison. Instrument, valve, equipment-capacity, process,
  interaction, schedule/model and evidence-acquisition remedies have their own
  owners and approval boundaries. This lesson proves a declared Simulation
  casebook, not universal diagnostic coverage or physical-plant health.
  Lesson 76 begins the capstones: the learner must now select and combine
  previously taught methods without being told the controller or remedy in
  advance.

### 3.15 Integrated capstones

Capstones shall not introduce unexplained theory. They require the learner to
select and combine previously taught methods without being told which method to
use.

#### Lesson 76 — Beginner capstone: cold-storage room

- **Objective:** Commission the temperature control of a walk-in cold-storage
  room by selecting the simplest discrete controller that keeps qualified room
  temperature inside the declared product band without short-cycling the
  compressor. The learner shall derive and justify the switch thresholds and
  minimum on/off times, reject inadequate simpler candidates with measured
  evidence, and validate normal cycling, a door-opening load, sensor noise,
  controller restart, invalid measurement, manual mode and recovery. The
  exercise shall not instruct the learner which candidate to select before the
  comparison.
- **Context:** The room stores refrigerated food with a `2.0 degC` desired
  temperature and an accepted qualified air-temperature band from `1.0 degC`
  through `3.0 degC`. The compressor can remove heat but cannot actively add
  heat. Its requested and applied demands are therefore discrete `0 %` or
  `100 %`; measured compressor-running feedback, starter-ready interlock and
  temperature-sensor quality are separate signals. The deterministic room
  model is the already validated introductory first-order thermal model: gain
  `-0.05 degC/%`, time constant `20 s`, no-cooling equilibrium `4.0 degC`,
  initial temperature `2.0 degC`, `100 ms` controller interval, and fixed seed
  `20260829`. A door-opening event raises the no-cooling equilibrium to
  `6.0 degC` from `60 s` through `90 s`. Qualified measurement noise has
  `0.03 degC` standard deviation. All results are Simulation evidence; the
  simplified air-temperature model, ideal refrigeration capacity and
  deterministic starter feedback are not food-safety, refrigerant, electrical,
  compressor-endurance or physical commissioning evidence.
  Derive the physical interpretation from
  `dT/dt = (T_eq + K_u u - T)/tau`: the bracket is the temperature toward
  which the room moves minus its current temperature, and division by `tau`
  converts that difference into a rate. Full cooling gives a closed-door
  equilibrium `4 + (-0.05)(100) = -1 degC` and an open-door equilibrium
  `6 + (-0.05)(100) = 1 degC`, establishing simulated cooling authority before
  controller candidates are compared.
- **Reason:** Lessons 1 through 3 separately showed that observation does not
  regulate temperature, a single threshold chatters under measurement noise,
  and hysteresis plus dwell protection bounds cycling. Lesson 75 established
  that a control method must follow evidence rather than habit. The capstone
  now removes the guided answer: the learner must translate product-temperature
  and compressor-life requirements into controller settings, compare the
  simplest candidates fairly, and defend why continuous P, PI or PID control
  would add complexity without useful authority for this two-state compressor.
- **Theory recalled before practice:** Recall each concept in words and units
  before using its symbol or equation:
  - The setpoint (`SP`) is the desired room temperature in degrees Celsius.
    The process value (`PV`) is the qualified measured room temperature. For a
    cooling plant, rising `PV` creates a request to turn cooling on.
  - Requested manipulated variable (`MV_requested`) is the thermostat's
    `0 %` or `100 %` demand. Applied manipulated variable (`MV_applied`) is the
    measured compressor-running state expressed as `0 %` or `100 %`. Starter
    lockout, an interlock or a fault can make them differ.
  - A single-threshold controller changes state at one temperature. Noise near
    that boundary can produce repeated transitions even when the physical room
    has barely changed.
  - Hysteresis uses a warmer switch-on threshold and a colder switch-off
    threshold. Hysteresis width (`Delta_T_hys`) is their difference. With
    desired temperature `SP`, symmetric thresholds are introduced in words
    before writing `T_on = SP + Delta_T_hys/2` and
    `T_off = SP - Delta_T_hys/2`. Explain the halves: equal offsets place the
    band symmetrically around `SP`, and subtracting the two equations recovers
    the required total separation `Delta_T_hys`.
  - Minimum off time protects the compressor from restarting too soon after a
    stop. Minimum on time prevents a newly started compressor from stopping
    before its declared protected interval. A dwell timer measures elapsed
    time in the current applied state; it is not inferred from task-scan count
    when the actual interval is available.
  - The independent starter-ready signal owns electrical and compressor
    restart permission. After controller restart the PLC shall default to no
    compressor command until temperature quality is valid and the starter
    reports ready. The PLC shall not invent retained starter history.
  - A safety band and a control band have different meanings. The switch
    thresholds shall sit inside the `1.0..3.0 degC` qualified product band with
    explicit margin for noise and dynamic excursion. Crossing a control
    threshold commands ordinary action; crossing a safety limit aborts the
    experiment and requires the declared safe/manual response.
  - Transition count is the number of changes in measured applied state.
    Completed on duration and completed off duration are elapsed seconds
    between measured state changes. Maximum temperature is eligible only while
    sensor quality and synchronization are valid. Time outside the product
    band is accumulated in seconds from eligible samples.
- **Prediction:** Before running, derive candidate settings from the
  requirements. A symmetric `1.0 degC` hysteresis about `2.0 degC` gives a
  `2.5 degC` switch-on threshold and `1.5 degC` switch-off threshold, leaving
  `0.5 degC` static margin to each product limit. A `0.03 degC` noise standard
  deviation is far smaller than the `1.0 degC` threshold separation, so noise
  alone shall not reverse the requested state. The chosen minimum on and off
  times are each `5.0 s`; therefore every completed applied-state interval,
  except a declared safety or manual interruption, shall be at least `5.0 s`.
  Predict that a single threshold fails the cycling requirement, hysteresis
  reduces noise transitions, and hysteresis plus dwell is the simplest
  candidate eligible for final validation. Also predict that restart with
  starter-ready false produces `0 %` applied demand even when temperature asks
  for cooling, then permits a normal transition only after readiness becomes
  true.
- **Controller under test:** One shipped Structured Text program shall expose
  `AutoMode`, `TemperatureQualityGood`, `StarterReady`, `Setpoint`,
  `HysteresisWidth`, `MinimumOnTime`, `MinimumOffTime`, `MeasuredCycleTime`,
  `ProcessValue`, `CoolingRequested`, `CompressorCommand`,
  `CompressorRunning`, `DwellElapsed`, `DwellInhibit`, `SwitchCount` and a
  latched invalid-quality state. It shall implement the configured two-state
  thermostat directly; it shall not contain PI/PID objects, hidden case labels,
  precomputed transitions or a hard-coded candidate verdict. Requested cooling
  is calculated from qualified temperature and thresholds. Applied compressor
  command is `100 %` only when automatic mode, measurement quality, starter
  readiness, requested cooling and the dwell rule all permit it. Manual,
  invalid-quality and not-ready states command `0 %`; restoration requires
  valid quality and explicit re-entry to automatic mode. Concise comments may
  explain why starter readiness is external and why the dwell timer follows
  measured compressor state.
- **Experiment:** Use one project and three declared parameter sets rather than
  three different controllers. In Control Studio:
  1. **Monitor** the setpoint, measured temperature, requested cooling, applied
     compressor state, quality, starter readiness, dwell time, inhibit state,
     transitions, limits, units, task interval and Simulation label.
  2. **Test** the single-threshold candidate with zero hysteresis and zero
     dwell against the fixed seeded noise record. Retain its transition count
     and reject it if noise creates repeated switching.
  3. **Identify** the observed failure as threshold sensitivity, not a room
     model or continuous-control problem. Calculate the symmetric thresholds
     and choose minimum times from the stated compressor requirement.
  4. **Tune/Design** the hysteresis-only and hysteresis-plus-dwell candidates.
     Change only the declared width and dwell settings; keep plant, initial
     state, seed, sample interval, event schedule, limits and duration equal.
  5. **Validate** the selected candidate over at least `140 s`: normal cycling,
     the `60..90 s` door load, qualified noise, controller restart with starter
     not ready for `5 s`, starter release, one invalid-quality interval, manual
     safe output, and explicit automatic recovery. Repeat with the same seed to
     prove deterministic evidence.
  Abort on nonfinite temperature or timing, temperature outside
  `-30..20 degC`, an applied command other than `0 %` or `100 %`, a command
  while quality or starter readiness is false, an undeclared interval shorter
  than `5 s` in the selected design, or any unlabeled physical evidence.
- **Comparison and decision measures:** Define calculations before results.
  Maximum eligible temperature uses degrees Celsius over the door-event and
  recovery windows; minimum eligible temperature is reported separately.
  Time outside the `1.0..3.0 degC` product band is the number of eligible
  samples outside the inclusive band multiplied by sample interval and uses
  seconds. Transition count uses measured compressor-running changes, not
  requested changes. Minimum completed on and off durations use seconds and
  exclude the initial partial interval and declared manual/safety interruption.
  Noise-only transition count uses the fixed quiet-room noise window. Restart
  command violations count samples with nonzero applied demand while starter
  readiness or temperature quality is false. The comparison shall classify
  each candidate as eligible or rejected before ranking transition count or
  temperature performance.
- **Validation:** A native executable test shall compile and execute the actual
  Structured Text against the shipped simulated room. It shall independently
  reproduce thermostat thresholds, requested state, dwell qualification,
  starter/quality gating and room dynamics; demonstrate the single-threshold
  noise failure; prove the selected design's `2.5/1.5 degC` thresholds,
  `5.0 s` completed minimum durations, discrete bounded commands, door-load
  response, restart lockout, invalid-quality safe output, manual output and
  deterministic recovery; and confirm that no continuous controller is
  instantiated. A registered learner-contract test shall prove the recalled
  theory, independent candidate choice, exact Control Studio journey, measure
  definitions, actual Structured Text, results provenance and Simulation/
  physical boundary. Plots shall be generated only from the retained run and
  shall show temperature with product and switching limits above requested and
  applied compressor state on a shared time axis.
- **Explanation:** Explain why the single threshold reacts to measurement
  crossings rather than meaningful thermal change, why hysteresis supplies
  state memory, why dwell protection constrains equipment cycling, why a door
  load changes cycling and peak temperature without changing the controller
  principle, and why starter readiness must come from the equipment boundary.
  Reconcile every prediction with executable evidence. State explicitly that
  ON/OFF control is selected because the installed actuator is discrete and
  the achieved band is acceptable; a continuous controller is not rejected for
  every refrigeration plant.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor confirms the selected hysteresis-plus-dwell
  settings, current finite temperature, request, measured running feedback,
  quality, starter readiness and mode. Test uses
  `Observe without excitation`, amplitude `0`, and duration `140.1 s`.
  Control Studio may write only `P1.ExperimentEnabled`; the project owns the
  door load, starter lockout, invalid-quality interval, manual interval,
  recovery and safe restoration. Removing authority commands manual safe
  output and restores valid quality and starter readiness. The native test
  owns comparison and rejection of the simpler candidates. Identify, Tune and
  Validate are not learner steps and disabled generic panels are not evidence.
- **Transfer and required conclusion:** Produce a concise commissioning record
  containing requirements, selected structure, rejected candidates, threshold
  derivation, dwell rationale, complete settings, plant and seed identity,
  eligible measures, abnormal tests, safe/manual behavior, retained plots,
  evidence class, limitations and physical approval boundary. The accepted
  result is a complete explainable discrete design, not merely a running
  example. Lesson 77 next requires the same independent workflow for a
  continuously modulated water heater where proportional, integral, modelling
  and tuning choices are justified by different physical evidence.

#### Lesson 77 — Process capstone: recirculating water heater

- **Objective:** Commission a recirculating electric water-heater outlet-
  temperature loop from requirements through model, controller structure,
  tuning and abnormal validation. The learner shall derive a physical model,
  identify a model from a separate safe experiment, validate both against data
  not used for fitting, reject inadequate P, integral-only and unnecessarily
  derivative-based structures, compare at least two eligible PI tunings on
  equivalent plants, and produce one unapplied commissioning recommendation
  with explicit limits, anti-windup, filtering, manual tracking and rollback.
- **Context:** A continuously modulated electric heater raises recirculating
  water from a nominal zero-power equilibrium of `20 degC` to a `35 degC`
  outlet setpoint. Heater request and applied power use percent and are limited
  to `0..100 %`; the heater can add heat but cannot actively cool. Outlet
  temperature uses degrees Celsius. Nominal local dynamics are
  first-order-plus-dead-time with gain `0.2 degC/%`, time constant `5 s` and
  delay `1 s`, operated at `75 %` holding power. The controller and simulator
  use `100 ms` intervals. A colder-water/load event lowers the zero-power
  equilibrium by `3 degC`. Qualified sensor interference is a deterministic
  `0.02 degC`, `0.5 Hz` sine. Model-validation copies cover at least `-20 %`
  and `+20 %` gain variation and delay through `1.5 s`. The simplified
  well-mixed thermal model excludes element skin temperature, boiling,
  circulation loss, pump failure, electrical protection and independent
  overtemperature trips; all executable results are Simulation evidence.
- **Reason:** Lessons 4 through 10 separately exposed load-dependent P offset,
  operating-point bias, slow integral-only response, the value of PI,
  saturation, windup and anti-windup. Lessons 36 through 44 established that a
  fitted model is useful only after structure selection, data qualification and
  independent validation. Lessons 45 through 52 compared model-based tuning
  methods and an unapplied Automatic Tune workflow. The capstone must now turn
  those fragments into one auditable engineering decision without assuming
  that a known simulated coefficient or a tuning formula is automatically the
  correct physical model or controller.
- **Theory recalled before practice:** Recall in ordinary words before symbols:
  - The setpoint (`SP`) is desired outlet temperature; process value (`PV`) is
    qualified measured outlet temperature; error (`e`) is setpoint minus
    process value. Requested manipulated variable (`MV_requested`) is the
    controller's heater calculation, while applied manipulated variable
    (`MV_applied`) is bounded power delivered to the simulated heater.
  - At thermal equilibrium, electrical heat input balances heat loss and the
    incoming-water load. For the local linear model, steady temperature change
    equals process gain (`K`) multiplied by the applied-power change. The
    nominal holding-power prediction is introduced in words before
    `MV_hold = (SP - T_zero)/K = (35 - 20)/0.2 = 75 %`.
  - Thermal capacitance and heat-loss conductance produce a first-order time
    constant; transport and sensor placement produce dead time. State all
    physical assumptions and parameter uncertainty before reducing the energy
    balance to a first-order-plus-dead-time model.
  - A safe power-step identification experiment estimates gain from settled
    input and output changes, delay from the first response beyond declared
    noise, and time constant from `63.2 %` of the delay-adjusted final change.
    Competing first-order and first-order-plus-dead-time structures shall be
    fitted. Fit data and independent validation data shall not be the same
    record.
  - P with correct bias can remove nominal offset but not changed-load offset.
    Integral-only control can remove constant error but responds without prompt
    proportional correction. PI is the simplest structure eligible when both
    prompt response and zero constant-load error are required. Derivative action
    is rejected unless a declared response requirement and noise-qualified
    comparison show benefit.
  - The implemented parallel PI law, bias, units and discrete interval shall be
    explained using the shared `Control.PID` contract. Integral gain units are
    percent per degree-Celsius-second. Requested and applied heater power remain
    separate so saturation is observable.
  - The shared controller's combined anti-windup compares its private nominal
    candidate with its bounded actuator-facing output so unavailable command
    does not accumulate as hidden integral state. That private candidate is not
    presented as a public measured signal. Requested controller output and
    physically applied heater power remain separate for actuator lag, interlock
    or hardware mismatch. Tracking follows manual or externally applied power
    so automatic re-entry is bumpless.
  - A model-based tuning method is eligible only for the validated plant class.
    Compare at least Robust SIMC (`Kp = 3.26087 %/degC`,
    `Ki = 0.65217 %/(degC*s)`) and Balanced SIMC
    (`Kp = 5.76923 %/degC`, `Ki = 1.15385 %/(degC*s)`) against the same
    requirements; include the previously empirical PI (`Kp = 4`, `Ki = 0.8`)
    as historical evidence, not as an automatically inferior straw candidate.
    Derive the SIMC values from
    `Kp = T/[K(lambda + L)]`, `Ti = min[T, 4(lambda + L)]`, and `Ki = Kp/Ti`.
    With `K = 0.2`, `T = 5 s`, and `L = 1 s`, Robust `lambda = 6.667 s`
    gives `Kp = 3.26087`, `Ti = 5 s`, and `Ki = 0.65217`; Balanced
    `lambda = 3.333 s` gives `Kp = 5.76923`, `Ti = 5 s`, and
    `Ki = 1.15385`. Explain that smaller `lambda` requests faster closed-loop
    response, which is why it increases gain and actuator movement.
  - Settling, final-window error, overshoot, integrated absolute error,
    applied-power total variation, saturation time, recovery time and
    robustness eligibility shall each have a declared calculation, signal,
    unit, window and validity condition before results are shown.
- **Prediction:** Derive `75 %` nominal holding power. Predict a P-plus-bias
  controller will return near `35 degC` before the load change but retain
  changed-load error. Predict PI will remove that error. From the validated
  `K = 0.2 degC/%`, `T = 5 s`, `L = 1 s` model, independently reproduce the
  declared Robust and Balanced SIMC gains before configuring either. Predict
  Balanced PI will normally reduce error area and settling time but use more
  actuator movement and robustness reserve than Robust PI. Predict the
  unreachable `45 degC` demand will hold applied power at `100 %`; combined
  anti-windup shall limit post-restoration saturation and tracking shall avoid
  an unnecessary output step after manual mode.
- **Controller under test:** One shipped project shall contain explicit P-plus-
  bias, historical PI, Robust PI and Balanced PI validation instances on
  equivalent plant copies, or an equivalently transparent sequential candidate
  harness. The final recommended controller shall be a real `Control.PID`
  instance. Structured Text shall expose setpoint, qualified PV, requested and
  applied heater power, output-limit state, mode, manual/tracking value,
  controller revision, gains, cycle time and selected candidate. It shall use
  the shared controller's combined anti-windup and tracking path; no result
  table, model fit, tuning formula, scenario label or precomputed winner may be
  embedded in the PLC program. The running/confirmed controller shall not be
  replaced merely because a candidate exists.
- **Experiment:** Perform one coherent Control Studio journey:
  1. **Monitor** signal identity, units, quality, limits, mode, applied versus
     requested power, initial equilibrium, controller revision, sample time,
     plant identity and Simulation label.
  2. **Test** a bounded open-loop power step on a dedicated identification copy
     after approval. Retain the pre-step equilibrium, step amplitude, response,
     noise band, aborts and restoration. Do not interrupt the running loop.
  3. **Identify** competing first-order and FOPDT models; calculate parameter
     uncertainty and reject an inadequate structure. Independently validate
     the physical and identified models on a second load/step record and record
     residuals and whiteness/structure limitations.
  4. **Tune/Design** P-plus-bias, historical PI, Robust SIMC and Balanced SIMC
     candidates only from eligible evidence. State requirements before running
     them and leave the active controller unchanged.
  5. **Validate** equivalent candidate copies for a `35 degC` servo change,
     the colder-water event, sensor interference, `0..100 %` saturation, an
     unreachable `45 degC` exposure followed by restoration, manual operation,
     tracking and automatic re-entry, `+/-20 %` gain and `1.5 s` delay cases.
     Select the simplest candidate satisfying every mandatory requirement,
     retain rejected runs, then export an unapplied recommendation and rollback
     plan. Application requires a separate explicit approval.
  Abort on invalid quality, nonfinite data or coefficients, model-region exit,
  temperature outside `0..80 degC`, applied power outside `0..100 %`, unsafe
  experiment state, loss of circulation permission, or any attempt to present
  Simulation evidence as physical commissioning.
- **Comparison and decision measures:** Model fit and independent validation
  fit are reported separately; a good fitting score cannot replace independent
  prediction. Final-window absolute error uses the mean absolute `SP-PV` only
  after the declared settling condition passes. Overshoot is peak eligible PV
  above setpoint after a setpoint increase. Integrated absolute error uses
  `degC*s`. Applied heater movement is total variation in percentage points.
  Saturation occupancy and post-restoration saturation time use the applied
  output. Requested-versus-applied mismatch is reported separately where the
  actuator model creates one; it shall not be renamed as the controller's
  private nominal saturation error. Robustness validation reports each plant variant as
  eligible, settled, saturated, not settled or invalid before any candidate is
  ranked. The decision shall use mandatory limits first and a transparent
  trade-off among error, response time, actuator activity and robustness second.
- **Validation:** Native executable tests shall compile and execute the actual
  Structured Text with the shared Control library and shipped equivalent plant
  configurations. Independent oracles shall reproduce the physical equilibrium,
  identified FOPDT coefficients, validation residuals, SIMC gains, controller
  equations, plant copies and declared measures. They shall prove P-plus-bias
  changed-load offset, PI offset removal, requested/applied separation,
  anti-windup recovery, manual tracking and no silent application; compare all
  candidates under nominal, load, noise, saturation, gain and delay cases; and
  reject invalid or unqualified evidence deterministically. A registered
  learner test shall prove the complete nine-stage causal account and exact
  Control Studio actions. Retained plots shall use executable run artifacts and
  show SP/PV above requested/applied power, limits, events, candidate identity,
  units and evidence class on aligned time axes.
- **Explanation:** Reconcile the energy-balance holding-power prediction,
  identified model and independent validation. Explain why P-plus-bias fails
  after the load changes, why integral action removes constant-load error, why
  PI is sufficient, why the more aggressive candidate trades actuator movement
  and robustness for speed, and how anti-windup and tracking use the physically
  applied output. Distinguish a selected Simulation recommendation from a
  commissioned physical controller.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor confirms all four equivalent copies, controller
  revision, modes, limits and the prediction. Test uses
  `Observe without excitation`, amplitude `0`, and duration `220.1 s`;
  Control Studio may write only `P1.ExperimentEnabled`. The runnable project
  owns the common load, unreachable-demand, manual, quality, recovery and
  safe-restoration sequence without changing revision `77`. Removing authority
  restores `35 degC`, valid quality and tracked `60 %` manual power. Native
  tests own identification, independent validation, uncertainty variants,
  candidate ranking and the unapplied recommendation. Identify, Tune and
  Validate are not learner steps; disabled generic panels are not evidence.
- **Transfer and required conclusion:** Produce an auditable, unapplied
  commissioning package containing requirements, equipment and safety scope,
  physical derivation, identification record, independent model validation,
  rejected structures, tuning calculations, equivalent candidate evidence,
  nominal and abnormal validation, requested/applied plots, selected gains,
  controller revision, manual/tracking behavior, limitations, physical approval
  steps and rollback. Lesson 78 next applies the same complete decision process
  to a motion system where derivative damping, resonance, frequency response,
  discrete implementation, encoder noise and torque limits become material.

#### Lesson 78 — Motion capstone: servo positioning system

- **Objective:** Commission the load-position loop of a flexible rotary indexing
  table from mechanical requirements through controller-structure selection,
  classical analysis, digital qualification and abnormal validation. The
  learner shall reject P and PD for measured reasons, justify filtered PID as
  the minimum eligible structure, prove the selected continuous and sampled
  design against time-, frequency- and actuator-domain requirements, reject an
  unchanged coarse implementation, and export an unapplied controller revision
  with approval and rollback requirements.
- **Context:** A servomotor drives a payload table through an elastic shaft. The
  motor inertia is `0.2 kg*m^2`, payload inertia is `0.8 kg*m^2`, shaft
  stiffness is `20 N*m/rad`, shaft damping is `0.5 N*m*s/rad`, motor viscous
  damping is `0.2 N*m*s/rad`, and payload viscous damping is
  `0.3 N*m*s/rad`. The bidirectional drive accepts `-10..10 N*m` and no more
  than `200 N*m/s`. A qualified encoder measures payload angle; synchronized
  motor/load state retained by Simulation is validation truth, not an
  additional physical sensor. The normal controller interval is `10 ms`. The
  table moves from `0` to `0.5 rad` at `1 s`; an opposing `1.5 N*m` payload
  torque begins at `10 s`; and the run lasts `30 s`. Encoder interference is a
  deterministic `0.002 rad`, `20 Hz` sine. A heavy-payload validation case
  increases payload inertia by `25 %`; a reduced-shaft-damping case shall also
  be retained. The local linear two-inertia model excludes backlash, Coulomb
  friction, structural modes above the represented shaft resonance, drive
  current-loop dynamics, thermal limits, hard stops and personnel safety. All
  executable results are Simulation evidence.
- **Reason:** Lessons 19 through 25 showed the causal motion progression: P
  improved position but left a lightly damped response; PD supplied damping but
  could not supply the constant payload torque at zero error; PID removed that
  offset; derivative-on-measurement avoided setpoint kick; filtering traded
  encoder-noise attenuation against lag; mechanical nonidealities had to be
  diagnosed rather than tuned away; and the implemented discrete realization
  mattered. Lessons 45 through 50 then supplied Routh, root-locus, Bode,
  Nyquist, sensitivity and z-plane evidence. The capstone must combine those
  results into one coherent design instead of treating a smooth time trace or
  a continuous tuning as sufficient commissioning evidence.
- **Theory recalled before practice — signals and mechanics:** The setpoint
  (`SP`) is desired payload angle in radians. The process value (`PV`) is the
  qualified encoder angle in radians. The position error (`e`) is setpoint
  minus process value. Requested manipulated variable (`MV_requested`) is the
  controller's requested motor torque in newton metres; applied manipulated
  variable (`MV_applied`) is the torque remaining after drive amplitude and
  slew limits. They shall remain separate even when an ideal nominal run makes
  them numerically equal. Motor angle (`theta_m`) and payload angle
  (`theta_l`) are joined by the shaft. Shaft twist is motor angle minus payload
  angle; the shaft torque is stiffness times twist plus damping times relative
  angular velocity. Only after those quantities are explained may the learner
  write the two rotational balances and construct the four-state model. The
  model shall predict a flexible-mode natural frequency near
  `sqrt(20*(1/0.2 + 1/0.8)) = 11.18 rad/s`, before damping shifts the measured
  resonance.
- **Theory recalled before practice — controller structure:** Proportional gain
  (`Kp`) requests torque per radian of present position error. Derivative gain
  (`Kd`) requests damping torque from measured angular velocity and has units
  `N*m*s/rad`; derivative setpoint weight is zero so a reference step does not
  create derivative kick. Integral gain (`Ki`) changes retained torque from
  accumulated position error and has units `N*m/(rad*s)`. P cannot adequately
  damp the represented flexible response. PD can damp motion, but under a
  constant `1.5 N*m` load and `Kp = 7 N*m/rad` its equilibrium prediction is
  `e_ss = 1.5/7 = 0.2143 rad`. PID is therefore the minimum candidate capable
  of both damping and zero constant-load error. Combined anti-windup and manual
  tracking remain required because demanded torque can exceed the drive.
- **Theory recalled before practice — filtering and nonidealities:** Raw encoder
  position and controller-facing filtered position are distinct signals. A
  first-order low-pass filter with `0.05 s` time constant attenuates the
  declared `20 Hz` interference before numerical differentiation, but adds
  phase lag. The learner shall not hide raw noise by plotting only the filtered
  signal. Derive the attenuation from `H(s) = 1/(1 + tau_f s)`, whose
  magnitude is `1/sqrt(1 + (omega tau_f)^2)`. At `20 Hz`,
  `omega = 2 pi 20 = 125.66 rad/s`; with `tau_f = 0.05 s`, the magnitude is
  approximately `0.157`. The phase is `-atan(omega tau_f)`, explaining why
  stronger smoothing also consumes stability margin. Backlash, stiction, saturation, slew limiting, resonance and sensor
  noise have different measured signatures; unexplained lost motion or
  requested/applied mismatch shall trigger diagnosis rather than automatic
  gain increase.
- **Theory recalled before practice — root locus and frequency response:** The
  root locus shows how closed-loop poles move as a declared controller gain
  changes. Pole location predicts damping and speed only for the stated model
  and controller form. A Bode plot shows loop magnitude and phase against
  angular frequency; the flexible resonance and filter lag shall remain
  visible. Gain margin is the additional loop-gain factor at the minus-180-
  degree crossing. Phase margin is the remaining phase before minus 180 degrees
  at unity gain. Sensitivity (`S`) is the disturbance-to-error path and
  complementary sensitivity (`T`) is the setpoint/noise-to-output path. After
  defining loop transfer (`L_loop`) in words, recall
  `S = 1/(1 + L_loop)` and `T = L_loop/(1 + L_loop)`. Maximum sensitivity
  (`Ms`) and maximum complementary sensitivity (`Mt`) are maxima over the
  declared frequency grid; they are not inferred from one time response.
- **Theory recalled before practice — digital execution:** Sampling observes
  the encoder only at update instants. Zero-order hold keeps applied torque
  between updates. A one-update computation delay adds a real state. The
  Nyquist angular frequency is pi divided by sample interval. It is
  `314.159 rad/s` at `10 ms` and `31.416 rad/s` at `100 ms`. The learner shall
  derive the exact held discrete model and include the implemented delay before
  inspecting z-plane poles. A stable continuous design does not authorize a
  different interval, delay convention, filter realization or timing jitter.
- **Prediction:** Use the physical model and the installed `0.05 s` filter to
  independently reproduce the selected parallel PID values
  `Kp = 7 N*m/rad`, `Ki = 1.2 N*m/(rad*s)` and
  `Kd = 2 N*m*s/rad`. On the declared continuous frequency grid, predict a
  crossover near `3.10 rad/s`, phase margin near `38 degrees`, gain margin near
  `1.94`, and maximum sensitivity near `2.07`; exact qualification tolerances
  shall be declared before comparison. Predict a `3.5 N*m` initial
  proportional request for the `0.5 rad` step, no derivative kick, P
  oscillation, PD settling with approximately `0.214 rad` load offset, and PID
  removal of that offset. Predict the unchanged `100 ms` plus one-update-delay
  implementation will violate the overshoot or settling requirement even if it
  eventually approaches setpoint.
- **Controller under test:** One shipped project shall execute four equivalent
  table copies: P at `Kp = 7`; filtered PD at `Kp = 7`, `Kd = 2`; selected
  filtered PID at `Kp = 7`, `Ki = 1.2`, `Kd = 2` and `10 ms`; and the same
  filtered PID updated at `100 ms` with an explicit one-update command delay.
  Reuse `Control.PID` and `Control.LowPass`; a small lesson-owned candidate
  function block may compose them to avoid duplicated configuration. The
  program shall expose raw and filtered encoder position, velocity, setpoint,
  requested, pending and applied torque, load torque, update pulse, effective
  interval, delay state, mode, quality, saturation, abort and unchanged
  controller revision. It shall contain no root-locus poles, Bode margins,
  performance table, accepted label or hidden winner. Concise comments shall
  explain derivative-on-measurement, the held/delayed command and why raw
  encoder evidence remains visible.
- **Experiment:** Perform one complete Control Studio journey:
  1. **Monitor** identities, units, Simulation label, raw and filtered encoder,
     synchronized validation state, requested/pending/applied torque, load,
     limits, quality, mode, sample interval, update pulse, delay convention,
     plant/controller revision and initial zero-energy state.
  2. **Test** use equivalent initial conditions to execute P, PD and PID through
     the `0.5 rad` command and `1.5 N*m` load event. Retain the same noise,
     duration, limits and seed. Record predictions before enabling motion.
  3. **Identify** compare the physical two-inertia model with a bounded swept-
     sine or multisine record from a dedicated copy. Confirm the flexible mode
     and residual limitations; do not identify from the closed-loop candidate
     comparison record or present the simulator coefficients as physical
     identification evidence.
  4. **Tune/Design** use root locus to reject gain-only P, add measured-motion
     damping, add integral action only for the measured load offset, and add the
     minimum derivative filter justified by encoder evidence. Inspect Bode,
     Nyquist, sensitivity and torque-sensitivity plots before selecting the
     unapplied candidate.
  5. **Validate** run the selected `10 ms` design, unchanged delayed `100 ms`
     design, heavy-payload case, reduced-damping case, doubled encoder-noise
     case, torque saturation/restoration exposure, manual/tracking transition,
     invalid quality and controlled stop. Keep the confirmed revision unchanged
     and export explicit application and rollback steps.
  Abort on invalid or stale position, nonfinite state or model data, wrong
  sample/delay identity, position outside `-1.5..1.5 rad`, speed outside
  `-5..5 rad/s`, applied torque outside `-10..10 N*m`, missing controlled stop,
  unqualified saturation, or an unlabeled physical-evidence claim.
- **Comparison and decision measures:** Classify every run and every measure as
  eligible, not settled, saturated, aborted or invalid before ranking it.
  Setpoint overshoot is peak eligible payload position above `0.5 rad`, reported
  both in radians and as percent of the `0.5 rad` move. Two-percent settling
  time is the first time position enters and remains within `0.01 rad` for at
  least `1 s`; a nonsettled response receives no steady-state-error value.
  Load-recovery error is final-window mean absolute `SP-PV` over `25..30 s` only
  after settling qualification. Applied-torque total variation is cumulative
  absolute sample-to-sample change in newton metres. Noise-driven torque root-
  mean-square activity uses the declared quiet window. Also retain peak speed,
  peak requested and applied torque, saturation duration, requested/applied
  mismatch, controlled-stop time, dominant poles, damping ratios, crossover,
  gain/phase margins, delay margin, `Ms`, `Mt`, frequency grid and numerical
  tolerances. Time and frequency results shall name continuous or implemented
  discrete evidence explicitly.
- **Validation:** The selected nominal `10 ms` PID shall overshoot no more than
  `35 %`, satisfy the `0.01 rad` settling band within `5 s`, have final-window
  load error no greater than `0.02 rad`, remain inside torque and speed limits,
  and keep quiet-window torque RMS below `0.10 N*m`. Its continuous-model phase
  margin shall be at least `35 degrees`, gain margin at least `1.7`, and
  `Ms <= 2.5`; the exact implemented discrete poles shall remain inside the
  unit circle with declared clearance. The unchanged delayed `100 ms` case
  shall be rejected by its predeclared time- or discrete-domain requirement,
  not by appearance. Heavy payload, reduced damping and doubled noise shall be
  reported separately and shall not appear better because a metric became
  invalid. Native tests shall compile and execute the actual Structured Text
  with all shipped plant configurations and use independent mechanical,
  frequency, discrete and metric oracles. A registered learner test shall prove
  the complete nine-stage causal account and exact Control Studio journey.
  Retained plots shall come from run artifacts and align SP/PV, raw/filtered
  position, velocity, requested/pending/applied torque, load, events and limits;
  root locus, Bode/sensitivity and z-plane plots shall retain model, grid,
  controller and sample/delay identity.
- **Explanation:** Reconcile the measured flexible mode with the physical shaft
  model. Explain why P gain alone cannot meet damping and robustness together,
  why derivative-on-measurement adds damping without a reference kick, why PD
  must retain error to balance constant load, and why integral action supplies
  the missing equilibrium torque. Explain the filter's noise-versus-phase cost
  and the coarse implementation's hold and delay penalty. A candidate is
  acceptable only when the same controller satisfies time response, frequency
  margins, discrete poles, torque, noise and abnormal requirements.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor confirms zero-energy initial state, raw and
  filtered encoder signals, requested/pending/applied torque, update pulses,
  mode, quality and revision. Test uses `Observe without excitation`, amplitude
  `0`, and duration `30.01 s`; Control Studio may write only
  `P1.ExperimentEnabled`. The project owns the `0.5 rad` reference at `1 s`,
  the `1.5 N*m` opposing load at `10 s`, and zero-reference, zero-load,
  zero-torque restoration. Native tests own frequency, exact discrete,
  uncertainty, abnormal and candidate qualification. Identify, Tune and
  Validate are not learner steps and disabled generic panels are not evidence.
- **Transfer and required conclusion:** Export requirements, physical model,
  identification and residual evidence, rejected P/PD structures, controller
  and filter equations, root-locus and frequency evidence, exact discrete
  realization, equivalent time traces, uncertainty and abnormal results,
  requested/applied torque, selected gains, unchanged revision, evidence class,
  explicit approval, rollback and maintenance limits. The conclusion is an
  unapplied digitally qualified motion controller, not a generic claim that PID
  controls every servo. Lesson 79 next requires the learner to coordinate
  several interacting loop structures and mode transitions across an entire
  jacketed-reactor operating sequence.

#### Lesson 79 — Industrial capstone: jacketed chemical reactor

- **Objective:** Commission one coherent temperature-control architecture for a
  jacketed stirred reactor from startup through production, measured feed-load
  changes, high-temperature constraint takeover, instrument degradation,
  manual intervention and controlled shutdown. The learner shall commission
  the utility-facing jacket loop before granting the reactor loop authority,
  justify measured-load feedforward, map one signed thermal demand onto
  mutually exclusive heating and cooling valves, prove tracked override and
  bumpless mode transitions, and export an unapplied architecture revision with
  approval and rollback requirements.
- **Context:** A continuously stirred vessel produces a temperature-sensitive
  liquid product. Reactor temperature is measured in degrees Celsius and has a
  normal setpoint of `82 degC`, an operating limit of `84 degC`, and a
  Simulation abort limit of `86 degC`. A secondary transmitter measures jacket
  outlet temperature. A qualified feed-flow/temperature calculation publishes
  signed thermal load: positive values mean colder or greater feed removes more
  heat; negative values represent additional reaction or feed heat. Steam and
  cooling-water valves are independently measured from `0..100 %`; they shall
  not be open simultaneously in the selected architecture. Loss of both
  utilities removes automatic temperature authority. The `100 ms` controller
  task drives a local two-state jacket/reactor model: the jacket time constant
  is approximately `2 s`, reactor time constant approximately `12 s`, jacket-
  to-reactor steady gain approximately `1.14`, and normal `80 degC` equilibrium
  requires `50 %` steam with cooling closed. Two equivalent plant copies compare
  feedback-only cascade with the integrated measured-load feedforward
  architecture. The model excludes reaction kinetics, mixing gradients,
  pressure, level, chemistry, runaway calorimetry, relief systems, utility
  hydraulics, valve dynamics, independent trips and functional safety. All
  executable results are Simulation evidence.
- **Reason:** Lessons 7 through 12 supplied PI/PID, anti-windup, servo/regulatory
  objectives and setpoint weighting. Lessons 27 through 30 established
  initialization, manual/automatic authority, tracking, atomic configuration
  and deterministic invalid-data handling. Lessons 53 through 57 then showed
  feedforward, cascade, ratio, split-range and override separately. A real
  reactor cannot run those as unrelated demonstrations: the inner loop must be
  available before cascade closes; the same physical valves must obey normal,
  protection, manual and shutdown authority; inactive controllers must track;
  and loss of a measurement shall remove only the claim that depends on it.
- **Theory recalled before practice — signals and energy path:** The reactor-
  temperature setpoint (`SP_r`) is desired product temperature in degrees
  Celsius. Reactor process value (`PV_r`) is qualified measured vessel
  temperature. Jacket-temperature setpoint (`SP_j`) is the outer controller's
  requested jacket temperature, and jacket process value (`PV_j`) is the
  measured jacket outlet temperature. Signed utility demand (`U`) is the inner
  controller's common heating/cooling demand before split range. Requested and
  applied steam positions (`H_requested`, `H_applied`) and cooling positions
  (`C_requested`, `C_applied`) use percentage points and remain separate. Feed
  thermal load (`D_feed`) is a measured disturbance in declared normalized
  load units. Explain the vessel and jacket energy balances in words before
  writing their local two-state equations; heat crosses the jacket quickly,
  while vessel inventory makes product temperature respond more slowly.
- **Theory recalled before practice — cascade commissioning:** Cascade means the
  slower reactor-temperature controller requests a jacket-temperature setpoint
  and the faster jacket controller moves utilities to achieve it. The inner
  closed-loop response shall be at least three times faster than the outer
  response before cascade is eligible. During startup, commission and validate
  the inner loop with the outer controller in Track. Grant production authority
  only after jacket measurement, utility availability, mode identity and the
  time-scale criterion are valid. A cascade label does not compensate for a
  slow, saturated or unavailable inner loop.
- **Theory recalled before practice — measured-disturbance feedforward:**
  Feedback waits for reactor temperature to change. Feedforward uses a qualified
  measured thermal load to change jacket demand before that consequence appears
  in `PV_r`. From the declared local balances, one positive feed-load unit
  predicts `0.60 degC` steady reactor cooling if feedback does nothing. Jacket-
  to-reactor gain `1.1429` therefore predicts feedforward gain
  `K_ff = 0.60/1.1429 = 0.525 degC/load-unit`. Feedforward supplements PI trim;
  it shall fall back to zero when its measurement is invalid and shall never
  bypass jacket-setpoint, valve or safety limits.
- **Theory recalled before practice — split range:** The inner loop produces one
  signed utility demand. Positive demand requests steam; negative demand
  requests cooling. The installed neutral operating point uses `50 %` steam.
  The characterized mapping shall close steam continuously before opening
  cooling, preserve the declared installed gains, and guarantee mutual
  exclusion. Dead zone would leave no utility authority around crossover;
  overlap would waste opposing utilities and obscure the actual heat balance.
  Requested and measured applied valve positions shall be compared before
  claiming that split range worked.
- **Theory recalled before practice — constraint override and tracking:** The
  normal cascade path requests the utility demand needed for product quality.
  The high-temperature protection path requests a lower demand when
  `PV_r >= 83.5 degC`; lower demand closes steam and then opens cooling. A low
  selector gives the more cooling-oriented request authority. Hysteresis keeps
  protection active until `PV_r <= 82.5 degC` and the normal request is no less
  protective than the override request. The inactive normal path, outer loop
  and any displaced manual path shall track the measured applied demand or the
  corresponding active setpoint. Tracking aligns internal state; it does not
  prove an independent safety instrumented function. Invalid reactor
  temperature removes automatic authority rather than disabling protection by
  assumption.
- **Theory recalled before practice — operating modes:** Define explicit modes
  before using them: `InnerCommissioning` grants only the jacket loop authority;
  `Production` permits qualified outer cascade and feedforward;
  `Manual` applies one reviewed signed utility demand while all automatic paths
  track; `ControlledShutdown` moves the jacket toward `70 degC`, then closes
  utilities when the reactor reaches the declared shutdown condition; and
  `Safe` closes both valves. Every transition shall declare its owner,
  prerequisites, first requested/applied outputs, completion condition,
  timeout, abort and rollback. Startup and shutdown are sequences, not PID gain
  sets.
- **Prediction:** From the local model, predict a jacket time constant near
  `2 s`, reactor time constant near `12 s`, and a time-scale ratio near `6`, so
  cascade is eligible after independent inner-loop commissioning. Predict the
  normal `80 degC` equilibrium uses `50 %` steam and no cooling. Predict a
  positive `4 load-unit` feed event would create about `2.4 degC` steady
  reactor error without feedback correction; feedforward adds approximately
  `2.10 degC` to jacket setpoint immediately. Predict integrated feedforward
  reduces reactor-temperature error area relative to feedback-only cascade but
  cannot remove model-mismatch error without PI. Predict the high-temperature
  event activates the low-selector override, closes steam before opening
  cooling, and keeps reactor temperature below `86 degC`. Predict tracked
  release and manual/production return keep the first applied-valve change
  within the declared `5 percentage-point` transition limit.
- **Controller under test:** One shipped project shall instantiate two
  equivalent reactor trains through a small lesson-owned function block that
  composes the shared `Control.PID` objects and owns one authority decision.
  The local jacket model has pole magnitude `0.5 1/s`; select inner
  proportional gain `10 %/degC` and integral gain `5 %/(degC*s)` so the PI zero
  cancels that qualified local pole and predicts a `1.25 s` inner closed-loop
  time constant. Derive it explicitly: the local plant is `0.08/(s + 0.5)`
  and the PI is `10(s + 0.5)/s`; cancelling the qualified pole leaves open
  loop `0.8/s`, so unity feedback has pole `-0.8` and time constant
  `1/0.8 = 1.25 s`. Use the same inner tuning in both trains.
  Both trains use the same commissioned outer and inner PI, characterized split
  range, selector hysteresis, limits and modes; only feedforward gain differs
  (`0` versus `0.525`). The Structured Text shall expose mode, prerequisites,
  reactor/jacket setpoints and measurements, raw/qualified feed load, outer
  feedback output, feedforward contribution, normal utility demand, protection
  demand, selected authority, requested/applied steam and cooling, selector
  state, tracking state, transition state, abort, restoration and unchanged
  architecture revision. It shall contain no performance table, scenario label,
  accepted architecture or precomputed winner. Concise comments shall explain
  inner-first commissioning, selector direction, mutually exclusive valve
  mapping and why inactive paths track the physically applied authority. The
  project shall also publish a Boolean controller-automatic indication derived
  from `OperatingMode = Production`; Control Studio uses that explicit signal
  to permit a reviewed setpoint experiment without guessing the numeric mode
  encoding.
- **Experiment:** Perform one coherent Control Studio journey:
  1. **Monitor** signal identities, units, quality, utility availability,
     requested/applied valves, mode, active authority, tracking state, limits,
     task interval, plant/controller revisions, initial energy balance and
     Simulation label.
  2. **Test** retain the inner jacket response first with the outer path tracked.
     Apply an `80` to `82 degC` jacket step from `1.0` through `9.0 s`,
     require the final second to enter the declared `0.3 degC` band, and
     confirm direction, saturation, split-range crossover and restoration.
     Then approve production authority; do not close an unqualified cascade.
  3. **Identify** compare the physical two-state balance with a bounded inner-
     utility experiment and a separate reactor/load record. Validate jacket and
     reactor time constants, cross gain, feed-load gain and residual limitations
     on independent data before using feedforward.
  4. **Tune/Design** tune the inner loop first, then the outer loop against the
     commissioned inner response. Calculate the feedforward gain separately.
     Review the characterized valve map and override thresholds. Compare the
     feedback-only and integrated copies under equivalent conditions without
     changing the confirmed architecture revision.
  5. **Validate** run startup, an `80` to `82 degC` production setpoint, positive
     `4 load-unit` cold-feed event, feedforward-quality loss, an unmeasured
     negative `12 load-unit` thermal-load/high-temperature event with the
     feedforward-quality signal invalid, tracked override takeover and release, steam-
     unavailable case while steam is selected, and a separate `-30 %` manual
     cooling request while cooling is unavailable, followed by invalid reactor
     quality, manual demand and production return, controlled shutdown and Safe
     state. Retain an unapplied architecture
     package with explicit physical approval and rollback.
  Abort on invalid or stale reactor/jacket temperature, nonfinite timing or
  demand, reactor temperature outside `0..86 degC`, jacket temperature outside
  `0..100 degC`, simultaneous applied heating and cooling, a valve outside
  `0..100 %`, unavailable selected utility, wrong authority/mode identity,
  failed transition timeout, or an unlabeled physical-evidence claim.
- **Comparison and decision measures:** Classify each interval and measure as
  eligible, tracking, overridden, saturated, aborted, invalid or not settled
  before ranking it. Inner-loop settling is the first time `PV_j` enters and
  remains within the declared band around `SP_j`; outer-loop settling uses its
  own longer band and dwell. Time-scale ratio uses those two qualified settling
  times. Reactor integrated absolute error uses `degC*s` over the declared feed
  event. Maximum reactor departure uses degrees Celsius from the pre-event
  operating point. Feedforward onset delay uses seconds from the first qualified
  disturbance sample to the jacket-setpoint change. Steam and cooling total
  variation use percentage points. Simultaneous-utility occupancy uses seconds
  for samples where both measured applied valves exceed the closed tolerance.
  Override takeover/release times, maximum protected temperature, authority
  transition jumps, saturation occupancy, tracking error, shutdown completion
  and utility-close verification shall define signals, windows, tolerances and
  validity conditions before results.
- **Validation:** The inner loop shall be at least three times faster than the
  outer loop. Integrated feedforward shall reduce eligible cold-feed reactor
  error area by at least `25 %` relative to feedback-only cascade without
  increasing maximum reactor departure, violating jacket limits or creating
  simultaneous utilities. Feedforward-quality loss shall fall back to feedback
  without changing architecture revision or causing a valve jump greater than
  `5 percentage points`. The high-temperature campaign shall activate override
  before `84 degC`, keep the simulated reactor below `86 degC`, close steam
  before opening cooling, release only after hysteresis and normal-demand
  qualification, and keep takeover/release valve jumps within `5 percentage
  points`. Manual return and shutdown transitions shall meet the same jump
  bound; controlled shutdown shall reach its declared temperature and close
  both utilities. Invalid primary temperature or loss of required utilities
  shall remove automatic authority deterministically. Native tests shall compile
  and execute the actual Structured Text and shipped plant configurations, with
  independent energy-balance, controller, selector, mapping, timing and metric
  oracles. A registered learner test shall prove the complete nine-stage
  causal account and exact Control Studio journey. Retained plots shall align
  reactor/jacket SP/PV, feed load and quality, normal/feedforward/protection/
  selected demand, requested/applied valves, modes, authority, limits and events.
- **Explanation:** Reconcile the inner/outer time-scale measurements with the
  two-state energy path. Explain why feedforward acts before reactor feedback,
  why PI trim remains necessary, why one signed demand simplifies authority,
  why characterized split range avoids dead zone and overlap, and why a tracked
  low selector protects the temperature constraint without an avoidable release
  bump. Explain what each degraded mode can still claim and why this ordinary
  control override is not an independent safety layer.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor confirms the initial energy balance, mode,
  authority, quality, requested/applied utilities and prediction. Test uses
  `Observe without excitation`, amplitude `0`, and duration `150.1 s`;
  Control Studio may write only `P1.ExperimentEnabled`. The project owns inner
  commissioning, Production, qualified and invalid feed-load intervals,
  override exposure, Manual, controlled shutdown and Safe restoration for
  both trains. Native tests own independent model and performance decisions.
  Identify, Tune and Validate are not learner steps and disabled generic
  panels are not evidence.
- **Transfer and required conclusion:** Export requirements, equipment and
  safety scope, physical and identified models, independent validation, inner-
  first commissioning evidence, outer tuning, feedforward calculation,
  characterized valve map, selector and tracking logic, complete mode/authority
  table, equivalent comparison, normal and abnormal run artifacts, selected
  architecture, unchanged revision, evidence class, explicit approval, rollback
  and maintenance tests. The result is one maintainable unapplied industrial
  control strategy, not a collection of individually successful loops. Lesson
  80 next asks whether one fixed-gain design remains valid when a nonlinear
  gravity-drained process crosses several operating regions.

#### Lesson 80 — Nonlinear capstone: gravity-drained tank

- **Objective:** Commission level control for one vented gravity-drained water
  tank across its qualified `0.30..0.80 m` operating envelope. The learner
  shall prove why the square-root outlet makes one fixed PI design inconsistent
  across that envelope, derive and independently validate three local models,
  compare a conservative fixed PI with one atomically interpolated gain-
  scheduled PI on equivalent nonlinear plants, and validate boundaries,
  tracking, measurement degradation, pump loss, manual return and an unapplied
  controller revision.
- **Context:** The tank has constant cross-sectional area `A = 1.0 m2`. A
  variable-speed inlet pump can apply `0..0.060 m3/s`. Water leaves through a
  fixed gravity outlet with coefficient
  `c = 0.050 m2*sqrt(m)/s`. The normal qualified level range is
  `0.30..0.80 m`; `0.20 m` and `0.90 m` are Simulation abort boundaries, not
  certified low-low or high-high trips. A `100 ms` controller task receives
  measured level, measurement quality, pump availability, operating mode,
  manual flow and cycle time. It publishes requested pump flow, applied pump
  flow, scheduling level, interpolation interval and fraction, active gains,
  schedule eligibility, tracking state, fault state and unchanged revision
  `80`. Two equivalent nonlinear plants compare the fixed and scheduled
  controllers with the same initial state, setpoint sequence, noise seed,
  pump limits and disturbance assumptions. The simulation omits tank geometry
  changes, inlet pressure, pump curve, valve and pipe dynamics, overflow,
  cavitation, foam, density changes, sensor lag, independent trips and
  functional safety. Results are Simulation evidence only.
- **Reason:** Lesson 79 showed that several controllers may need coordinated
  authority even when each local model is approximately fixed. This tank
  exposes the next limitation: its gravity outflow changes with level, so the
  same pump-flow change produces different level dynamics at different
  operating points. Lesson 34 derived local tangent models and Lesson 58
  introduced scheduling. The capstone now requires one complete engineering
  record that proves a fixed controller's measured limitation, qualifies the
  local models before scheduling, and keeps parameter changes, mode changes
  and faults inside one controller-facing authority path.
- **Theory recalled before practice — physical balance:** Introduce liquid
  level (`h`) in metres, inlet pump flow (`q_in`) and gravity outlet flow
  (`q_out`) in cubic metres per second, tank area (`A`) in square metres, and
  outlet coefficient (`c`) in square metres times square-root metre per second
  before notation. Explain that stored volume rises when inflow exceeds
  outflow, and that gravity outflow grows with the square root of level. Only
  then write `q_out = c*sqrt(h)` and
  `A*dh/dt = q_in - q_out`. At equilibrium `q_in = q_out`; predict pump flows
  `0.02739`, `0.03708` and `0.04472 m3/s` at `0.30`, `0.55` and `0.80 m`.
  State explicitly that the pump is one-sided and cannot apply negative inflow.
- **Theory recalled before practice — local models and their limits:** Define
  equilibrium level (`h_eq`), level deviation (`delta_h = h - h_eq`) and pump-
  flow deviation (`delta_q = q_in - q_in_eq`) in words and units. Explain that
  linearization replaces the curved local relationship by a tangent valid only
  near one operating point. Define local steady gain (`K_h`) as equilibrium
  level change per pump-flow change and local time constant (`T_h`) as the time
  for about `63.2 %` of a small first-order response. Then derive
  `K_h = 2*sqrt(h_eq)/c` and `T_h = 2*A*sqrt(h_eq)/c`, predicting
  `K_h = T_h = 21.91, 29.66, 35.78` in their declared units at the three
  operating levels. Show the derivation: the tangent slope is
  `dq_out/dh = c/[2 sqrt(h_eq)]`; small deviations therefore obey
  `A d(delta_h)/dt = delta_q - [c/(2 sqrt(h_eq))] delta_h`. Matching this to
  `T_h d(delta_h)/dt + delta_h = K_h delta_q` produces both formulas.
  The learner shall identify each model from a separately
  restored bounded pulse, validate it on unused local data, retain uncertainty
  and residuals, and reject extrapolation beyond the validated neighbourhood.
- **Theory recalled before practice — fixed and scheduled PI:** Define
  proportional gain (`Kp`) as requested pump-flow change per metre of current
  level error and integral gain (`Ki`) as retained pump-flow change per metre-
  second of accumulated error. For a declared `10 s` local design time, use
  `Kp = 0.100 (m3/s)/m` and qualified `Ki` values `0.00456`, `0.00337` and
  `0.00280 (m3/s)/(m*s)` from low to high level. The fixed baseline shall use
  the conservative high-level `Ki = 0.00280` throughout. The scheduled
  candidate shall change only the qualified parameter record; it shall not
  hide a second control law or an online-adaptation claim. Derive the integral
  values from `Ki = Kp/T_h`: `0.100` divided by the three local time constants
  gives `0.00456`, `0.00337`, and `0.00280`.
- **Theory recalled before practice — scheduling law:** Define the scheduling
  variable (`rho`) as the qualified requested operating level, not controller
  error. Define interpolation fraction (`alpha`) in words as the position
  between adjacent validated scheduling points, then write
  `alpha = (rho-rho_0)/(rho_1-rho_0)` and
  `theta(rho) = theta_0 + alpha*(theta_1-theta_0)`. Filter `rho` with a
  `2 s` time constant, require valid quality, and reject rather than
  extrapolate below `0.30 m` or above `0.80 m`. A
  `Control.PIDGainScheduler` shall return one complete parameter record, and
  the program shall apply that record atomically between PID updates so one
  scan cannot mix old and new gains or limits.
- **Theory recalled before practice — authority and tracking:** Define
  requested pump flow (`q_requested`) as the controller calculation and
  applied pump flow (`q_applied`) as the bounded, available command delivered
  to the simulated plant. `Automatic` grants either fixed or scheduled PI
  authority. `Manual` applies the reviewed manual flow while the automatic
  controller tracks the actual applied flow. `ControlledStop` tracks zero and
  closes the pump. `Safe` closes the pump for invalid level, invalid schedule,
  unavailable selected pump authority, nonfinite timing or an out-of-envelope
  request. Return to Automatic is eligible only after quality, schedule and
  pump availability are restored and the controller has tracked the actual
  applied flow. The first automatic request shall differ from the preceding
  applied flow by no more than `0.0005 m3/s`.
- **Prediction:** Predict the three equilibrium flows and local gains/time
  constants before running. Predict that the fixed high-level integral gain is
  slowest at low level and that interpolated `Ki` changes continuously between
  `0.00456` and `0.00280`. For the common sequence `0.30 -> 0.55 -> 0.80 ->
  0.55 -> 0.30 m`, predict that the scheduled candidate reduces the worst
  eligible regional integrated absolute level error without increasing the
  pump-flow limit, leaving the schedule envelope, or creating a parameter or
  authority bump. Predict zero applied flow during invalid-quality and pump-
  unavailable intervals, followed by a tracked bounded recovery.
- **Controller under test:** One shipped project shall compose the shared
  `Control.PID` and `Control.PIDGainScheduler` objects in a small lesson-owned
  tank-controller function block. Instantiate it twice against equivalent
  nonlinear plants: fixed scheduling disabled with the high-level PI record,
  and scheduling enabled with the three validated records. The block shall own
  modes, quality, schedule eligibility, atomic configuration, external
  tracking, pump limitation and applied output. Expose level setpoint and
  measurement, mode, quality, pump availability, requested/applied flows,
  filtered `rho`, interpolation interval and fraction, active `Kp`/`Ki`,
  schedule validity/clamping, tracking, fault, model region and revision. The
  Structured Text shall contain no experiment timing, regional IAE, accepted
  architecture or precomputed winner. Concise comments shall explain atomic
  schedule application, no extrapolation, and tracking to physical authority.
- **Experiment:** Perform one Control Studio journey:
  1. **Monitor** verify plant/controller identity, Simulation label, metres and
     cubic-metres-per-second units, raw/qualified level, requested/applied pump
     flow, pump availability, mode, tracking, schedule point identities,
     filtered `rho`, active gains, interval, limits and revision `80`.
  2. **Test** first run the fixed PI and retain its regional limitation on the
     common sequence: start at `0.30 m`; request `0.55 m` at `30 s`, `0.80 m`
     at `120 s`, `0.55 m` at `240 s`, and `0.30 m` at `330 s`; end at `420 s`.
  3. **Identify** restore separately to `0.30`, `0.55` and `0.80 m`; apply the
     approved local pump pulse; fit gain and time constant; quantify residuals
     and uncertainty; validate on unused local records; and reject any model
     that fails its declared neighbourhood.
  4. **Tune/Design** derive the three PI records from accepted local models,
     review the `2 s` scheduling filter and envelope, and keep revision `80`
     unapplied. Run the scheduled candidate on the equivalent common sequence.
  5. **Validate** repeat the nominal sequence and declared outlet-coefficient
     uncertainty cases. Hold just inside and outside each interpolation point
     with deterministic sensor noise. Exercise invalid level quality, invalid
     schedule, pump unavailable, Manual tracking and return, out-of-envelope
     request, ControlledStop, restoration and rollback. Do not run any physical
     pump without the independently reviewed topology and immediate approval.
- **Comparison and decision measures:** Classify each interval as eligible,
  tracking, saturated, unqualified region, invalid, aborted or not settled
  before ranking it. Regional integrated absolute level error is
  `sum(|SP[k]-PV[k]|*0.1 s)` in metre-seconds over each declared transition
  window. Worst regional IAE is the maximum of the four eligible values.
  Maximum level departure is the largest absolute SP/PV difference in metres.
  Applied pump-flow total variation is
  `sum(|q_applied[k]-q_applied[k-1]|)` in cubic metres per second. Schedule
  parameter jump is the largest consecutive change in active `Ki`; authority
  return bump is the first automatic requested flow minus the last manual or
  safe applied flow, in cubic metres per second. Settling time and final-window
  error require a declared level band, pump-span band and dwell; otherwise
  report `Not settled`. Retain source signals, exact windows, sample interval,
  seed and validity reason with every result.
- **Validation:** Both copies shall remain finite, within `0.20..0.90 m`, and
  within `0..0.060 m3/s` whenever their evidence window is eligible. The
  scheduled candidate shall improve worst regional IAE by at least `5 %`
  relative to fixed PI, shall not increase maximum level departure, and shall
  remain inside the qualified schedule. Active `Ki` shall remain continuously
  within `0.00280..0.00456`; one scan shall never expose a mixed parameter
  record. Noise holds shall not create discontinuous parameter changes.
  Invalid quality, invalid or clamped scheduling, unavailable pump authority
  and nonfinite inputs shall close the applied pump deterministically. Manual
  and restored Automatic transitions shall remain within `0.0005 m3/s` and
  use actual applied-flow tracking. ControlledStop shall close the pump,
  revision `80` shall remain unchanged, and no Simulation result shall be
  presented as overflow, dry-running, pump, or physical commissioning proof.
  Native tests shall compile and execute the actual Structured Text against the
  shipped nonlinear plants and independent balance, local-model, scheduling,
  transition and metric oracles. A registered learner test shall prove the
  nine-stage causal account and exact Control Studio journey.
- **Explanation:** Reconcile the square-root balance with increasing local gain
  and time constant. Explain why the conservative fixed high-level integral
  gain is slow low in the tank, why interpolation changes a qualified local
  design rather than discovering one online, why filtering and atomic records
  prevent discontinuous parameters, and why tracking must use applied pump
  flow. Explain the cost as well as the benefit: scheduling adds models,
  validity management and maintenance obligations, so a fixed controller
  remains preferable when it already meets the full operating-envelope
  requirements.
- **Control Studio workflow:** The learner-facing workflow is exactly
  **Monitor -> Test**. Monitor confirms both tanks, schedule validity, applied
  authority, active gains and prediction. Test uses
  `Observe without excitation`, amplitude `0`, and duration `420.1 s`;
  Control Studio may write only `P1.ExperimentEnabled`. The project owns the
  `0.30 -> 0.55 -> 0.80 -> 0.55 -> 0.30 m` schedule and safe restoration.
  Native tests own local identification, fixed-versus-scheduled comparison,
  abnormal cases and pass/fail. Identify, Tune and Validate are not learner
  steps and disabled generic panels are not evidence.
- **Transfer and required conclusion:** Export the physical balance, three
  identification and independent-validation records, local uncertainty and
  validity regions, fixed limitation, PI calculations, schedule record,
  equivalent nominal and uncertainty results, requested/applied pump evidence,
  boundary and abnormal results, unchanged revision, Simulation label,
  explicit physical approval requirements and rollback. The conclusion shall
  be one maintainable unapplied full-range level strategy, not a universal
  claim that gain scheduling improves every nonlinear plant. Lesson 81 next
  asks why even a locally valid model and tuning may be insufficient when
  transport delay consumes stability margin.

#### Lesson 81 — Delay capstone: conveyor oven

- **Objective:** Commission downstream product-temperature control for one
  continuous food-drying conveyor oven with a long heated-air transport path.
  The learner shall identify and independently validate the delayed heater
  model, qualify the strongest acceptable ordinary PI from time and frequency
  evidence, add measured upstream-temperature feedforward only for the
  disturbance it can predict, add Smith prediction only when the retained
  setpoint-response requirement still fails, and validate the integrated
  architecture plus measured-feedback fallback under model mismatch before
  leaving revision `81` unapplied.
- **Context:** Product travels through a recirculated heated-air duct. The
  downstream product-temperature sensor measures `15..60 degC`; normal
  operation begins at `20 degC` and requests `35 degC`. Heater power is one-
  sided, bounded to `0..100 %`, and slewed by at most `10 percentage points/s`.
  Around the declared product, airflow and belt-speed region, the heater-to-
  downstream-temperature path has process gain `K = 0.8 degC/%`, time constant
  `T = 12 s`, and transport delay `L = 16 s`. A qualified upstream inlet-
  temperature measurement announces a `-2 degC` disturbance at `54 s`; the
  corresponding physical disturbance reaches the downstream product at
  `70 s`. The `100 ms` controller exposes requested and applied heater power,
  measured and predicted temperatures, delay-free and delayed model outputs,
  model correction, upstream measurement and quality, feedforward contribution,
  predictor history, model validity, active/fallback authority, mode and
  revision. Equivalent delayed plants compare ordinary PI, PI plus feedforward,
  and Smith prediction plus feedforward. The model excludes product moisture,
  reaction or drying kinetics, spatial gradients, recirculation changes,
  heater and fan protection, belt mechanics, fire protection and functional
  safety. Every result is Simulation evidence.
- **Reason:** Lesson 80 changed PI parameters because local gain and time
  constant varied with operating level. This oven stays inside one qualified
  local region, so scheduling has no measured reason to act. Its problem is
  delay: the downstream sensor cannot report heater action still travelling
  with the air and product. The strongest robust ordinary PI therefore misses
  the required setpoint rise time. A qualified upstream temperature is also
  available before its load reaches the product, but feedforward cannot predict
  an unmeasured setpoint error or model all delayed behavior. The capstone must
  assign each technique only to the evidenced limitation it can address.
- **Theory recalled before practice — delayed model and identification:**
  Introduce heater command (`u`) in percent, downstream product temperature
  (`y`) in degrees Celsius, operating command (`u_0`), operating temperature
  (`y_0`), process gain (`K`), time constant (`T`) and transport delay (`L`)
  in words and units. Explain that downstream temperature responds to an
  earlier applied heater command. Only then write
  `T*dy(t)/dt + (y(t)-y_0) = K*(u(t-L)-u_0)`. A `10 %` heater step predicts an
  eventual `8 degC` rise, no attributable downstream response for `16 s`, and
  about `63.2 %` of the rise during the following `12 s`. The delay ratio is
  `L/T = 1.333`, so delay dominates the visible feedback dynamics. Design a
  bounded heater step, qualify excitation and timing, fit competing delay-free
  and FOPDT structures, estimate uncertainty, validate on unused data, inspect
  residual autocorrelation and reject the model if delay or residual evidence
  is inadequate.
  In the executable plant boundary, physical upstream temperature is converted
  to its independently known equivalent thermal-input deviation and added to
  applied heater power before the common FOPDT transport path. This net thermal
  input is simulator wiring, not a controller signal: loss of upstream
  measurement quality removes feedforward but does not remove the physical
  disturbance.
- **Theory recalled before practice — strongest ordinary PI:** Define
  proportional gain (`Kp`) as requested heater percentage points per degree-
  Celsius error and integral gain (`Ki`) as retained percentage points per
  degree-Celsius-second. The accepted ordinary PI uses
  `Kp = 0.46875 %/degC` and `Ki = 0.0390625 %/(degC*s)`. A declared next-
  aggressive candidate uses `Kp = 0.9375` and `Ki = 0.078125` in the same units
  and shall be rejected when it violates the `35 deg` phase-margin requirement
  requirement. The baseline is the highest-bandwidth ordinary candidate that
  passes the complete nominal and uncertainty envelope, not an intentionally
  weak comparison.
- **Theory recalled before practice — Bode, Nyquist and delay margin:** Define
  angular frequency, loop magnitude, loop phase, crossover frequency, gain
  margin, phase margin and delay margin in words before plotting or calculating
  them. The ordinary loop is the accepted PI multiplied by the validated FOPDT
  plant. Bode evidence shall retain the delay's phase loss and show the
  crossover used for margins. Explain that extra pure delay adds phase lag
  proportional to frequency, then calculate delay margin as phase margin in
  radians divided by crossover angular frequency. Require at least `35 deg`
  phase margin and `2 s` additional-delay margin across declared gain/time/
  delay uncertainty. The Nyquist plot shall use the complete indented contour,
  state open-loop right-half-plane pole count (`P`), count clockwise
  encirclements (`N`) of `-1`, and infer closed-loop right-half-plane poles
  (`Z`) using the declared sign convention. A wrapped phase trace or nominal
  step shall not replace this stability argument.
- **Theory recalled before practice — measured disturbance feedforward:**
  Introduce upstream inlet-temperature deviation (`D_inlet`) as measured inlet
  temperature minus its normal value, in degrees Celsius. The physical
  disturbance and compensating heater action traverse the same `16 s` path.
  The static feedforward gain (`Kff`) is the heater change needed per degree of
  qualified inlet-temperature change. With heater gain `0.8 degC/%`, a
  `-2 degC` inlet event predicts `+2.5 %` heater contribution:
  `Kff = -1/K = -1.25 %/degC` and
  `MV_ff = Kff*D_inlet = (-1.25)*(-2) = 2.5 %`. Feedforward is zero when the
  upstream measurement is invalid and never bypasses heater limits, slew,
  feedback trim or safety authority.
- **Theory recalled before practice — Smith prediction:** Define the delay-free
  model output (`y_model_now`) as the model's estimate before transport delay,
  delayed model output (`y_model_delayed`) as what that model predicts the
  downstream sensor should now report, and model correction (`c_model`) as
  measured temperature minus delayed-model temperature. Predicted current
  product temperature (`y_predicted`) is delay-free output plus correction:
  `c_model = y_measured - y_model_delayed` and
  `y_predicted = y_model_now + c_model`. The Smith PI uses a `6 s` desired
  delay-free closed-loop time constant `lambda = 6 s`. For
  `G0(s)=K/(T*s+1)`, choose `Ti=T` so the PI zero cancels the nominal model
  pole. The remaining loop is `K*Kp/(T*s)`; matching its closed loop to
  `1/(lambda*s+1)` requires `Kp=T/(K*lambda)`. Therefore
  `Kp=12/(0.8*6)=2.5 %/degC` and
  `Ki=Kp/Ti=2.5/12=0.2083333 %/(degC*s)` against predicted feedback.
  Prediction changes the controller's internal feedback path; it does not move
  heat or product faster and does not eliminate the physical `16 s` delay.
- **Theory recalled before practice — mismatch, fallback and authority:** Model
  mismatch means actual gain, time constant or delay differs from the retained
  values. Prediction is eligible only with complete history, qualified
  temperature and inlet measurements, finite timing, a valid operating region,
  and absolute model correction below `5 degC`. Otherwise measured downstream
  feedback deterministically owns the loop. `Automatic` uses the selected
  ordinary or predictor feedback plus qualified feedforward; `Manual` tracks
  the actual applied heater; `ControlledStop` ramps heater power to zero;
  `Safe` closes it for invalid primary temperature or unavailable heater
  authority. Requested and applied heater power remain separate, and return to
  Automatic shall be bumpless against applied feedback.
- **Prediction:** Before running, predict the `8 degC` heater-step response,
  `16 s` silent interval, `12 s` following time constant, `L/T = 1.333`, Bode/
  Nyquist margins for the ordinary PI, and `+2.5 %` feedforward at `54 s`.
  Predict that ordinary PI passes robustness but misses a `45 s` 90-percent
  rise requirement; PI plus feedforward improves the measured inlet-load
  response but cannot improve the preceding setpoint window; nominal Smith plus
  feedforward reaches 90 percent at least `15 %` sooner than ordinary PI and
  reduces the qualified setpoint-window IAE. Predict bounded degradation under
  actual `K = 0.76`, `T = 13 s`, `L = 17 s`; predict measured-feedback fallback
  under severe `K = 0.60`, `T = 18 s`, `L = 22 s` mismatch or invalid model
  evidence.
- **Controller under test:** One shipped project shall compose the shared
  `Control.PID` and `Control.SmithPredictorFOPDT` objects in a small lesson-owned
  oven-controller function block. Instantiate three equivalent nominal copies:
  accepted ordinary PI, ordinary PI plus qualified upstream feedforward, and
  Smith PI plus the same feedforward. The predictor instance shall retain
  measured/predicted feedback, delay-free/delayed model, correction, history,
  validity and fallback separately. The wrapper shall own mode, quality,
  feedforward qualification, requested/applied heater constraints, tracking and
  revision. It shall contain no experiment time, performance metric, accepted
  label or precomputed winner. Concise comments shall explain why feedforward
  begins upstream, why prediction never removes physical transport, and why
  invalid prediction falls back to measured feedback.
  The top-level simulation wiring, outside the controller block, shall expose
  each net plant thermal input as applied heater power plus the physical
  upstream-load equivalent. The controller shall never use that net truth as
  feedback or as a substitute for the quality-gated measured disturbance.
- **Experiment:** Perform the exact two-stage Control Studio journey. Control
  Studio may write only Boolean `P1.ExperimentEnabled`; the runnable project
  owns every setpoint, disturbance, mismatch, invalid-data, fallback, stop and
  restoration event.
  1. **Monitor** verify oven/controller/model identities, Simulation label,
     units, downstream and upstream quality, setpoint, measured and predicted
     temperatures, delay-free/delayed model, correction, feedforward,
     requested/applied heater, predictor history, active/fallback authority,
     mode, limits, `100 ms` interval and revision `81`.
  2. **Test** select **Observe without excitation**, amplitude `0`, and duration
     `180.1 s`. Review that the sole write is `P1.ExperimentEnabled` at
     `%MX240.4`, with baseline and restoration `FALSE`. The project starts from
     `20 degC`, requests `35 degC` at `10 s`, announces the qualified `-2 degC`
     upstream event at `54 s`, lets it reach the downstream product at `70 s`,
     and owns ordinary-candidate screening, nominal comparisons, mismatch,
     invalid qualities, incomplete history, unavailable heater, Manual return,
     ControlledStop, safe restoration and rollback evidence. The learner
     interprets those live results using the derived identification, frequency,
     feedforward and Smith theory; disabled Identify, Tune or Validate screens
     are not lesson authority.
- **Comparison and decision measures:** First classify each interval as
  eligible ordinary feedback, eligible feedforward, eligible prediction,
  model mismatch, correction limited, fallback, saturated, rate limited,
  invalid, aborted or not settled. Setpoint-window integrated absolute
  temperature error is `sum(|SP[k]-PV[k]|*0.1 s)` from `10..70 s` in
  degree-Celsius-seconds. Regulatory IAE uses `70..180 s`. Ninety-percent rise
  time begins at the `10 s` setpoint event and ends when measured temperature
  first reaches `33.5 degC`. Overshoot is peak measured temperature minus
  `35 degC`. Heater total variation is cumulative applied-heater movement in
  percentage points. Also define upstream-to-feedforward onset delay,
  predictor-active fraction, peak absolute correction, fallback count,
  saturation/rate occupancy and final-window mean absolute error from
  `170..180 s`, with settling eligibility declared before use. Frequency
  artifacts retain model, controller, grid, contour, crossings, margins,
  encirclements and uncertainty case.
- **Validation:** The accepted ordinary PI shall meet `<=2 degC` overshoot,
  `<=0.5 degC` eligible final-window error, `>=35 deg` phase margin and `>=2 s`
  delay margin throughout its declared uncertainty envelope; the aggressive
  candidate shall fail at least one predeclared requirement. PI plus qualified
  feedforward shall reduce regulatory IAE by at least `20 %` relative to
  ordinary PI without changing setpoint-window eligibility or exceeding heater
  limits. Nominal Smith plus the same feedforward shall reach 90 percent at
  least `15 %` sooner and reduce setpoint-window IAE relative to ordinary PI.
  Moderate mismatch shall remain bounded but degrade error, correction or
  settling relative to nominal. Severe mismatch, correction limit, incomplete
  history or invalid model evidence shall remove predictor authority and use
  measured feedback; fallback intervals shall not be ranked as Smith success.
  Invalid upstream quality shall remove only feedforward, while valid ordinary
  feedback remains. Invalid primary temperature or unavailable heater shall
  close applied output. Manual return and ControlledStop shall be bounded,
  requested/applied heater shall respect amplitude and slew, and revision `81`
  shall remain unchanged. Native tests shall compile and execute actual ST
  against the shipped delayed plants and independent identification, frequency,
  Nyquist, predictor, timing and metric oracles. A registered learner test
  shall prove the nine-stage causal account and exact Control Studio journey.
- **Explanation:** Trace the setpoint event through the predictor and the
  upstream event through feedforward. Explain why ordinary feedback waits for
  downstream evidence, why feedforward addresses only a qualified measurable
  disturbance, and why Smith prediction improves nominal setpoint response by
  accounting for heater action already in transit. Reconcile Bode/Nyquist
  margins with the accepted fallback controller. Explain why correction and
  mismatch grow when the retained model is wrong and why measured feedback,
  rather than confident extrapolation, must regain authority.
- **Transfer and required conclusion:** Export the safe experiment, FOPDT and
  rejected delay-free fits, independent validation, uncertainty, accepted and
  rejected ordinary candidates, Bode/Nyquist/delay evidence, disturbance-path
  feedforward calculation, Smith model and tuning, equivalent time records,
  mismatch and fallback evidence, requested/applied heater, unchanged revision,
  Simulation label, explicit physical approval requirements and rollback. The
  required conclusion selects the simplest architecture satisfying both the
  setpoint and measurable-disturbance requirements and states the evidence
  conditions under which it falls back. It is not a claim that Smith prediction
  removes transport delay. Lesson 82 next asks how controller selection changes
  when two manipulated variables and two outputs interact strongly.

#### Lesson 82 — MIMO capstone: distillation process

- **Objective:** Commission an unapplied local control architecture for a
  continuous binary distillation column by identifying all four input-output
  paths, selecting loop pairings from Relative Gain Array evidence, screening
  direct, decoupled and centrally constrained candidates, and choosing the
  simplest architecture that satisfies the complete nominal and interaction-
  uncertainty envelope. The learner shall be able to explain why an
  infeasible production request is rejected rather than hidden by a more
  complicated controller.
- **Context:** Reflux-valve change and reboiler-steam-valve change are the two
  manipulated inputs, in valve percentage points. Overhead light-product
  purity deviation and bottoms heavy-product purity deviation are the two
  measured controlled outputs, in composition percentage points. The declared
  local range is `-10..10 pp` for each applied valve change and `-6..6 pp` for
  each purity deviation at a `0.1 s` task interval. The nominal four-path
  steady gain matrix is `[[0.8,-0.4],[-0.3,0.7]]`; corresponding first-order
  response times are `[[20,35],[40,30]] s`. The qualified stronger-interaction
  model retains diagonal paths but uses gains
  `[[0.8,-0.64],[-0.49,0.7]]` and response times
  `[[20,45],[50,30]] s`. These are two local Simulation models around one
  declared feed, pressure, tray-efficiency and product operating region, not a
  nonlinear physical-column model.
- **Reason:** Lesson 81 selected separate feedforward and prediction paths for
  one manipulated variable and one controlled output. A distillation column
  cannot be reduced honestly to two independent single-loop copies: both
  valves move both product purities, sometimes in opposing directions. Lessons
  61 through 63 introduced interaction, RGA, decoupling and multivariable
  robustness separately. This capstone requires one continuous engineering
  decision from raw interaction tests through architecture selection, without
  assuming that centralized control is automatically better.
- **Theory recalled before practice — four-path identification:** Define the
  physical input vector (`u`) only after naming reflux and reboiler changes,
  and define the output vector (`y`) only after naming both purity deviations.
  The local dynamic matrix (`G(s)`) contains one response model for each input-
  output path. Excite one input at a time from a restored operating point while
  holding the other input fixed; use positive and negative checks where valve
  direction or nonlinearity is uncertain. Estimate each path's gain and
  response time, retain covariance and residuals, and validate all four paths
  on unused simultaneous-input data. The input records shall be independently
  exciting; a fitted four-path matrix from one common input direction is
  ineligible.
- **Theory recalled before practice — pairing and RGA:** Explain interaction as
  one valve changing both products. Explain a relative gain as the effective
  steady gain of one chosen pairing when the other loop also holds its output,
  divided by that path's open-loop gain. Only then introduce the steady gain
  matrix (`G0`) and Relative Gain Array (`Lambda`):
  `Lambda = G0 elementwise_multiply transpose(inverse(G0))`. The nominal
  normalized matrix is `[[1,-0.5],[-0.428571,1]]`, with
  `Lambda = [[1.272727,-0.272727],[-0.272727,1.272727]]`. Explain why positive
  diagonal elements near one support overhead-to-reflux and bottoms-to-
  reboiler pairing, why negative off-diagonal elements reject the crossed
  pairing, and why RGA is local steady evidence rather than a dynamic
  stability proof.
- **Theory recalled before practice — candidate architectures:** Direct
  decentralized PI sends each paired PI output directly to its valve. Static
  decoupling maps the two virtual PI corrections through the qualified nominal
  inverse before applying physical limits. Dynamic decoupling additionally
  attempts to match cross-path response times; recall the Lesson 62 mismatch
  evidence and exclude it unless it remains causal, noise-bounded and
  materially better over the capstone uncertainty set. The centrally
  constrained candidate retains the same two PIs and static inverse but scales
  both requested physical commands by one common factor when a joint valve
  limit is reached, preserving their requested direction. Call this central
  constraint coordination, not centralized MIMO control or MPC: it has no
  optimizer, prediction horizon or future constraint model.
- **Theory recalled before practice — directional robustness:** Explain a
  singular value as output-vector movement produced by an input direction of
  unit length before using `sigma_max` or `sigma_min`. The nominal normalized
  matrix has singular values `1.4649` and `0.5364`, condition number `2.73`,
  and maximum steady inverse amplification `1.86`. The stronger-interaction
  matrix has singular values `1.7512` and `0.2512`, condition number `6.97`,
  and inverse amplification `3.98`. Define the interaction uncertainty
  (`DeltaM`) as actual minus nominal normalized matrix; its largest singular
  value is `0.30`. Explain that `0.30 < 0.5364` preserves invertibility for the
  declared set but does not guarantee acceptable conditioning or closed-loop
  performance.
- **Theory recalled before practice — multivariable sensitivity and
  feasibility:** Define the loop matrix (`L`) from the plant, fixed paired PI
  controllers and selected mapping. Define the sensitivity matrix (`S`) in
  words as the remaining output error caused by an output-side disturbance,
  then write `S(s) = inverse(I + L(s))`. Maximum singular value of `S` is the
  worst disturbance amplification over direction at one frequency. It shall be
  swept over `0.002..2 rad/s` for both models; separate diagonal margins do not
  replace it. Define feasibility from the steady inverse and valve limits
  before judging tracking. The stronger-interaction model predicts about
  `5.44/5.24 pp` valve changes for a feasible `+1/+1 pp` purity target and
  `10.88/10.47 pp` for an infeasible `+2/+2 pp` target.
- **Theory recalled before practice — authority and constraints:** Requested
  reflux/reboiler changes remain separate from applied changes. Independent
  clipping can rotate a requested two-input direction. Central coordination
  calculates requested peak magnitude (`u_peak`) and common scale (`alpha`) in
  words before `alpha = min(1, 10/u_peak)` and
  `u_applied = alpha*u_requested`. `Automatic` uses the selected qualified
  architecture; `Manual` tracks independently observed applied valves;
  `ControlledStop` returns both applied changes toward zero; `Safe` closes the
  relevant command for invalid primary composition, unqualified mapping or
  unavailable valve authority. A return to Automatic shall be bounded and
  bumpless. Revision `82` remains unapplied.
- **Prediction:** Before running, calculate all four nominal gains and response
  times from the restored step records, the determinant and RGA, both singular-
  value pairs, condition numbers, inverse amplifications, uncertainty norm,
  steady commands for `+1/+1` and `+2/+2`, and expected common scale for the
  infeasible request. Predict diagonal pairing, direct-PI failure under the
  stronger simultaneous target, static-decoupler acceptance over the normal
  envelope, no material central-coordination benefit while constraints remain
  inactive, and direction preservation but nonzero error when the stronger
  `+2/+2` request exceeds physical capacity.
- **Controller under test:** The shipped capstone shall compose exactly two
  shared `Control.PID` objects and one shared
  `Control.StaticDecoupler2x2`. A small lesson-owned column controller shall
  expose requested and active architecture plus qualification, both SP/PV pairs, virtual PI
  corrections, requested and applied physical inputs, common scale,
  saturation, quality, availability, operating mode, tracking, fault and
  revision. The same fixed PI settings shall serve every architecture:
  overhead `Kp = 1.6666667`, `Ki = 0.0833333`; bottoms
  `Kp = 2.1428571`, `Ki = 0.0714286`. The ST shall contain no RGA, singular
  value, performance metric, precomputed winner, experiment timing or silently
  applied revision. Concise comments shall explain why controller parameters
  remain fixed and why common scaling preserves direction.
  Derive those fixed gains from each diagonal first-order path before using
  them: with `Ti=T`, direct synthesis gives `Kp=T/(K*lambda)` and
  `Ki=Kp/Ti`. Thus overhead uses `20/(0.8*15)=1.6666667` and
  `1.6666667/20=0.0833333`; bottoms uses
  `30/(0.7*20)=2.1428571` and `2.1428571/30=0.0714286`.
- **Experiment:** Perform the exact two-stage Control Studio journey. Control
  Studio may write only Boolean `P1.ExperimentEnabled`; the project owns the
  complete restored architecture and abnormal sequence.
  1. **Monitor** verify project/model identity, Simulation label, operating
     region, both SP/PV pairs and units, virtual corrections, requested/applied
     valves, architecture and qualification, common scale, saturation, mode,
     quality, availability, `100 ms` interval, seed and revision `82`.
  2. **Test** select **Observe without excitation**, amplitude `0`, and duration
     `720.1 s`. Review that the sole write is `P1.ExperimentEnabled` at
     `%MX116.1`, with baseline and restoration `FALSE`. The project owns the
     overhead-only and bottoms-only direct records, simultaneous static and
     centrally coordinated records, feed-disturbance record, `+2/+2`
     infeasible challenge, invalid composition, unavailable valve, Manual,
     ControlledStop, restoration and rollback. The learner derives and
     interprets identification, RGA, singular-value, sensitivity and feasibility
     evidence from the live project results; disabled Identify, Tune or
     Validate screens are not authority.
- **Comparison and decision measures:** Classify each record as `Eligible`,
  `Unqualified model`, `Unqualified pairing`, `Saturated`, `Infeasible`,
  `Fallback`, `Invalid`, `Aborted` or `Not settled` before ranking. Final-window
  combined absolute error is the final `10 s` mean of both absolute purity
  errors, in composition percentage points, only after settling. Unintended
  cross peak is maximum absolute movement of the product with unchanged SP
  during a single-axis test. Combined IAE is the `0.1 s` time sum of both
  absolute errors in composition-percentage-point-seconds. Applied-input total
  variation sums consecutive absolute changes of both applied valves in valve
  percentage points. Also retain four-path fit/validation error, determinant,
  RGA with uncertainty, singular values and directions, condition number,
  inverse amplification, maximum sensitivity singular value, limit duration,
  minimum common scale, command-direction error, restoration error and
  requested/applied mismatch.
- **Validation:** A normal record requires combined final error `<=0.10 pp`,
  single-axis cross peak `<=0.20 pp`, input-limit duration `<=0.5 s`, final
  restoration error `<=0.02 pp`, and maximum multivariable sensitivity
  `<=1.20`. Direct PI shall fail at least one stronger-interaction normal
  requirement. Static decoupling shall pass the complete normal envelope.
  Central coordination shall be selected only if it improves an eligible error
  or effort measure by at least `20 %` or turns a failed normal constraint case
  into a pass. The stronger `+2/+2` record shall remain `Infeasible`; common
  scaling shall preserve requested input direction but shall not be credited
  with achieving an impossible purity target. Invalid model or mapping evidence
  removes decoupling, invalid primary composition or unavailable valve fails
  closed, Manual and ControlledStop track and restore, and revision `82`
  remains unchanged. Native tests shall compile and execute the actual ST
  against shipped nominal/stronger plants and use independent identification,
  matrix, RGA, singular-value, sensitivity, metric and feasibility oracles. A
  registered learner test shall prove the nine-stage causal flow and exact
  Control Studio journey.
- **Explanation:** Trace each single-axis test into one column of the gain
  matrix, the matrix into RGA pairing, the paired controllers into virtual
  commands, and the selected mapping into physical requested/applied valves.
  Explain why opposing cross effects create a weak direction, why nominal
  cancellation becomes fragile as `sigma_min` falls, why direct PI fails the
  stronger simultaneous case, why static decoupling is sufficient in the
  normal envelope, why inactive central coordination cannot earn added
  complexity, and why no allocator can manufacture missing valve capacity.
- **Transfer and required conclusion:** Export raw and restored interaction
  records, four fitted paths, unused validation, uncertainty, determinant/RGA
  and pairing, excluded dynamic-decoupler evidence, fixed PI settings,
  singular directions, sensitivity sweeps, equivalent architecture records,
  infeasibility and direction evidence, abnormal/fallback/restoration records,
  unchanged revision, Simulation label, physical approval requirements and
  rollback. Select static decoupling as the simplest architecture for the
  declared normal envelope while retaining direct measured-feedback fallback
  when mapping evidence is invalid. State explicitly that this local linear
  result does not authorize another operating point or a physical column.
  Lesson 83 next asks whether prediction and optimization earn their cost when
  several future comfort, air-quality, energy and actuator constraints must be
  coordinated simultaneously.

#### Lesson 83 — Predictive-control capstone: constrained building HVAC

- **Objective:** Commission an unapplied controller recommendation for two
  thermally coupled office zones by qualifying the sampled thermal model and
  additive-load estimator, screening the strongest eligible conventional
  baseline, executing a real constrained receding-horizon optimizer, and
  deciding from common comfort, energy, actuator, solver and abnormal evidence
  whether predictive control earns its added complexity.
- **Context:** Zone-one and zone-two air temperatures are the controlled
  outputs, in degrees Celsius. Their hydronic heating commands are manipulated
  inputs, in percent. Outdoor temperature, two occupancy heat-gain forecasts
  and electricity tariff form the preview schedule. The nominal continuous
  two-zone balance, exact `60 s` sampled matrices, `0..100 %` individual heater
  limits, `+-20 pp/min` move limits, `170 %` shared limit, `20.0..22.5 degC`
  comfort envelope and `15..27 degC` local range are those qualified in Lesson
  68. A declared mismatch plant increases outdoor heat-loss conductance by
  about `11 %` while retaining the same measured signals and actuator limits.
  Every result is Simulation evidence.
- **Reason:** Lesson 82 found that static decoupling was sufficient while only
  present two-input constraints were active; central coordination earned no
  normal-envelope benefit. Building weather, occupancy and price changes are
  known in advance, and comfort plus shared heating constraints must be
  satisfied over future intervals. That creates a legitimate reason for
  prediction and optimization. It does not prove that MPC will outperform a
  strong conventional controller enough to justify estimator, forecast,
  computation and maintenance cost.
- **Theory recalled before practice — model and estimator:** Reintroduce the
  continuous heat balances in physical words, then the exact sampled state
  transition (`A_d`) and input (`B_d`) matrices from Lesson 68. Both air
  temperatures are measured. Derive the sampled matrices from
  `x_dot=A_c*x+B_c*u` under inputs held for `Delta t=60 s`:
  `A_d=exp(A_c*Delta t)` and
  `B_d=integral_0^Delta_t exp(A_c*tau)*B_c d tau`. Explain that the matrix
  exponential retains loss and inter-zone exchange throughout the minute while
  the integral accumulates heater, outdoor and occupancy effects.
  temperatures are measured, so an observer for those states would add no
  information. The required estimator instead represents unmeasured net
  thermal-load bias for each zone. Define previous one-step predicted
  temperature (`T_pred_prev`), current measured temperature (`T_measured`),
  innovation (`nu = T_measured - T_pred_prev`), and estimated additive
  temperature bias per interval (`d_hat`). With estimator gain `Ke = 0.4`,
  explain before writing `d_hat_next = clamp(d_hat + Ke*nu,-1,1) degC/step`.
  Explain `Ke=0.4` as accepting `40 %` of the newest innovation and retaining
  `60 %` of prior error: for a constant bias, the uncorrected part decays as
  `0.6^n` and has half-life `ln(0.5)/ln(0.6)=1.36` intervals.
  Each horizon prediction adds the retained `d_hat`. Estimation is eligible
  only after initialization with qualified measurements, finite timing and
  absolute innovation `<=2 degC`; otherwise predictive authority is removed
  and the conventional fallback owns the heaters.
- **Theory recalled before practice — conventional baseline screening:** A
  cascade controller requires a separately measured, meaningfully faster
  inner variable and an actuator that the inner loop can regulate. This plant
  exposes no supply-water temperature or inner valve/coil state, so inventing a
  cascade loop would be false complexity. Record cascade as `Ineligible: no
  qualified inner variable`. The strongest eligible conventional baseline is
  the independently qualified incremental PI pair from Lesson 68 with
  `Kp = 20 %/degC`, `Ki = 0.02 %/(degC*s)`, fresh measured feedback, outdoor-
  load operating-point compensation, anti-windup through actual applied heat,
  and the same individual, move and shared limits as MPC. It is called PI, not
  predictive control.
- **Theory recalled before practice — optimization:** Define future heater
  moves, the three-step prediction horizon (`N=3`), the move grid
  `{-20,0,+20} pp`, the six decisions and `3^6=729` complete sequences before
  showing loops. Define the objective's temperature, tariff-energy and move-
  effort terms before
  `J = sum(w_T*e^2 + w_E*p*u + w_Delta*Delta_u^2)` with
  `w_T=100`, `w_E=0.01`, `w_Delta=0.0025`. Define receding horizon as applying
  only the first move, measuring again and resolving. The finite-grid solution
  is optimal only on the declared grid; it is not a continuous quadratic-
  program solution.
- **Theory recalled before practice — feasibility and solver truth:** Define
  configuration validity, enumeration completion, feasible-sequence count,
  optimum, infeasibility and solver failure as separate states. A completed
  729-candidate search with zero feasible sequences is `Infeasible`. Invalid
  weights, nonfinite signals, invalid estimator evidence or incomplete search
  are failures, not infeasibility. Neither case may silently reuse a stale MPC
  move. Requested and applied heaters remain separate through individual, move,
  shared, mode and availability authority.
- **Prediction:** Reproduce the nominal one-step `21 degC`, `80/80 %`,
  `5 degC` outdoor, zero-occupancy prediction and the 729-sequence count.
  Predict a feasible zero first move for that equilibrium. Predict completed-
  search infeasibility when comfort minimum is raised to `22 degC`, invalid
  configuration for negative temperature weight, and conventional fallback in
  both cases. Under the outdoor-loss mismatch, predict innovations of the
  physically correct sign, bounded bias-estimate convergence and smaller
  one-step residual after estimator convergence than with bias fixed at zero.
- **Controller under test:** The shipped project shall execute the nominal
  sampled predictor, two additive-load estimators, the qualified incremental PI
  fallback and all 729 finite-grid candidate sequences online every eligible
  interval. Expose measurement, previous prediction, innovation, estimate,
  estimator validity/initialization, all forecasts, weights, candidate and
  feasible counts, objective, solver states, fallback reason, requested/applied
  heaters, constraints, mode, quality, availability and revision `83`. The ST
  shall contain no prerecorded plan, precomputed affine policy, acceptance
  metric or winner. Comments shall explain why estimator bias enters every
  horizon prediction, why only the first move is applied and why stale or
  invalid predictive evidence cannot retain authority.
- **Experiment:** Perform the exact two-stage Control Studio journey. Control
  Studio may write only Boolean `P1.ExperimentEnabled`; the project owns every
  schedule, infeasibility, invalid-data, fallback, mode and restoration event.
  1. **Monitor** verify Simulation/project/model identity, both temperatures
     and setpoints, measurement quality, estimator signals, weather/occupancy/
     tariff present values and forecasts, previous/requested/applied heaters,
     individual/move/shared limits, mode, availability, solver states, `60 s`
     interval, schedule revision, controller revision `83` and rollback.
  2. **Test** select **Observe without excitation**, amplitude `0`, and duration
     `1200.1 s`. Review that the sole write is `P1.ExperimentEnabled` at
     `%MX242.0`, with baseline and restoration `FALSE`. At its declared `60 s`
     task interval the project owns the twenty-minute weather, occupancy and
     tariff schedule, the nominal estimator/MPC comparison, `22 degC`
     infeasibility, negative-weight invalid configuration, invalid quality,
     unavailable heater, Manual return, ControlledStop, restoration and
     recovery. The learner uses the derived model, estimator and objective to
     interpret the live counts and moves; disabled Identify, Tune or Validate
     screens are not lesson authority.
- **Comparison and decision measures:** Classify each interval and run before
  ranking. Define zone absolute-error integral, comfort-violation magnitude and
  duration, tariff-weighted heating, applied-heater total variation, peak
  individual/shared heat, active-constraint duration, fallback duration,
  completed/infeasible/failed solve counts, estimator innovation RMS and bias-
  corrected one-step prediction error. Retain source signals, units, windows,
  model/schedule/forecast revisions, estimator parameters, weights, horizon,
  grid, sample count and evidence class. An MPC candidate is materially better
  only if it improves a declared comfort or energy requirement without
  exceeding the predeclared acceptable actuator-movement ratio; a private
  objective value cannot win the comparison.
- **Validation:** The independent oracle and ST shall agree on every normal
  candidate count, feasible count, first move and objective within tolerance.
  Applied heaters shall satisfy every individual, move and shared limit. Normal
  MPC shall complete without fallback and preserve comfort. The conventional
  baseline and every MPC weight candidate shall be reported even when MPC
  loses. The existing Balanced grid result—`146.72` versus `158.67 degC*s`
  error and `46,200` versus `46,624` relative cost-percent-seconds, but `160.0`
  versus `18.7 pp` heater travel—shall be treated as a tradeoff, not an
  automatic win. If no tested weight meets the declared movement criterion,
  retain PI as the application recommendation and MPC as an unapplied research
  candidate. The mismatch estimator shall reduce qualified one-step residual
  without hiding invalid evidence. Infeasible, invalid, stale, unavailable and
  failed cases shall use bounded conventional authority; recovery requires a
  new complete solve. Disable/Safe closes both heaters; Manual and
  ControlledStop track applied heat; revision `83` remains unchanged.
- **Explanation:** Trace measured temperature into innovation and bias update,
  then trace one candidate through all three corrected predictions, constraints,
  objective terms and its first applied move. Explain preview, receding horizon,
  estimator/model mismatch, finite-grid limitations, completed infeasibility,
  deterministic fallback, and why a modest error/energy improvement can still
  lose an engineering decision because of actuator movement and lifecycle cost.
- **Transfer and required conclusion:** Export model and estimator qualification,
  cascade screening, conventional baseline, weight candidates, independent
  oracle comparisons, nominal/mismatch schedules, solver and constraint states,
  infeasibility/failure/recovery, requested/applied heat, unchanged revision,
  Simulation label, physical approval requirements and rollback. Select only
  the simplest candidate satisfying every declared requirement; do not force an
  MPC selection merely because this is the MPC capstone. Physical use requires
  identified thermal and actuator dynamics, qualified forecasts, privacy-safe
  occupancy data, facilities/cybersecurity/safety review and explicit approval.
  Lesson 84 next requires the learner to apply this complete evidence discipline
  to a self-selected real plant without a prescribed controller family.

#### Lesson 84 — Independent engineering capstone

- **Objective:** Independently commission an unapplied control-system
  recommendation for one learner-selected, platform-supported real plant. The
  learner shall derive requirements from equipment purpose and constraints,
  request only the theory and experiments that the decision needs, compare the
  strongest eligible simple and advanced candidates, and defend the selected
  architecture from reproducible normal, robustness and abnormal evidence.
  The shipped reference submission shall perform the complete journey on an
  industrial compressed-air receiver; it demonstrates the required standard
  but is not a prescribed answer for another selected plant.
- **Selection boundary:** A learner may select an executable Simulation plant
  shipped by the platform or a separately reviewed physical topology. Before
  work begins, the plant identity, owner, product purpose, controlled and
  manipulated quantities, measured disturbances, sensors, actuator, units,
  local operating region, equipment limits, safety functions outside the
  controller, evidence class and approval authority shall be declared. A
  generic transfer function without named equipment, a prose-only method, a
  mocked result or an unsupported plant is not an eligible capstone. Physical
  work remains open unless the independently reviewed topology and immediate
  action approval exist.
- **Reference equipment and requirement:** The executable reference is a
  factory compressed-air receiver supplied by one variable-speed compressor.
  Header pressure is the controlled quantity, in bar. Compressor-delivery
  request is the manipulated quantity, in percent. Actual compressor delivery
  is measured in percent. Plant air demand is a measured disturbance, in
  percent of design flow. Normal pressure setpoint is `7.0 bar`; normal demand
  is `50 %`; applied compressor request is limited to `0..100 %` and
  `+-10 pp/s`; normal validation shall remain inside `6.5..7.5 bar`, while the
  independently owned hard pressure trip boundary is `6.0..8.0 bar`. The
  controller is not the pressure-relief valve, certified high-pressure trip or
  compressor protection system.
- **Reason:** Lesson 83 proved that the most sophisticated candidate need not
  win: MPC improved two measures but failed the declared actuator-movement
  requirement. Course completion therefore requires the learner to start from
  a new plant's requirements, decide which earlier techniques are actually
  applicable, and reject both underpowered and unjustifiably complicated
  solutions without following a supplied controller-family instruction.
- **Theory before practice — receiver physics:** Explain mass accumulation in
  words before notation: header pressure rises when compressor delivery exceeds
  plant demand, falls when demand exceeds delivery and remains constant when
  the flows balance. Define header pressure (`p`) in bar, actual compressor
  delivery (`q_c`) and plant demand (`q_d`) in percent of design flow, and
  receiver rate gain (`K_r`) in bar per percent-flow second before writing
  `dp/dt = K_r*(q_c-q_d)`. The reference value is
  `K_r = 0.005 bar/(%*s)`. Explain the internally controlled compressor-delivery
  lag in words, define delivery request (`u`) in percent and delivery time
  constant (`T_c = 5 s`), then write
  `dq_c/dt = (u-q_c)/T_c`. State that this local model omits compressor staging,
  temperature, moisture, piping modes and real protection dynamics.
- **Theory before practice — evidence plan:** Define requirement, hypothesis,
  independent variable, controlled condition, identification record,
  validation record, uncertainty and evidence class before using them. Derive
  which model quantities the controller decision needs. The reference shall
  identify compressor-delivery lag from a bounded request pulse and receiver
  rate gain from independently restored delivery-demand imbalance, then
  validate both on an unused demand sequence. Configured Simulation truth and
  identified estimates shall remain separately visible.
- **Theory before practice — architecture screening:** Screen ON/OFF, P, PI,
  PID, measured-demand feedforward, cascade, scheduling, Smith prediction,
  decoupling/MIMO and MPC against the actual topology and requirements. The
  reference shall explain why ON/OFF wastes a variable-speed actuator; P alone
  cannot supply unknown nonzero equilibrium flow at zero pressure error without
  an exact operating bias; integral action is needed for unmeasured imbalance;
  derivative is not justified by the pressure/noise and lag evidence; measured
  demand makes one-for-one static feedforward eligible; a second PLC cascade is
  ineligible because the installed drive already owns the delivery inner loop;
  and scheduling, Smith, MIMO and MPC have no evidenced requirement in the
  declared local envelope. Rejection shall be specific evidence, not a claim
  that a technique is generally inferior.
- **Theory before practice — selected reference law:** Define pressure setpoint
  (`SP`), measured header pressure (`PV`), pressure error (`e = SP-PV`),
  feedback request (`MV_feedback`), measured-demand feedforward
  (`MV_feedforward`), total requested compressor delivery (`MV_requested`) and
  bounded applied request (`MV_applied`) in words and units. Compare qualified
  feedback PI with the same PI plus feedforward. The fixed reference tuning is
  `Kp = 40 %/bar`, `Ki = 1.0 %/(bar*s)`, bias `50 %`, derivative zero, combined
  anti-windup, actual-applied-output tracking and `1 s` update. The integrated
  path uses `MV_feedforward = q_d-50 %`, so
  `MV_requested = MV_feedback + MV_feedforward`; it shall not add feedforward
  after actuator limiting or bypass tracking.
  Derive the one-for-one feedforward gain from steady receiver balance:
  `dp/dt=0` requires `q_c=q_d`, so one additional demand percentage point
  requires one additional delivery percentage point.
- **Theory before practice — stability and safety:** Use the identified
  integrating-plus-lag model to calculate closed-loop poles, Bode gain/phase
  margins, delay margin and sensitivity before applying either candidate in
  Simulation. State the sign convention and complete loop transfer. A local
  margin result shall not be called compressor protection proof. Explain
  Automatic, Manual, ControlledStop and Safe authority, valid pressure/demand/
  delivery quality, compressor availability, amplitude/slew constraints,
  measured update interval, its qualified `0.8..1.2 s` range, bumpless
  transfer, restoration and rollback before the learner operates them. PID
  integration and the `10 pp/s` slew boundary shall use the measured interval;
  invalid timing removes automatic authority.
  Show the PI derivation from the complete loop. With
  `G(s)=K_r/[s(T_c*s+1)]` and `C(s)=(Kp*s+Ki)/s`, the closed-loop
  characteristic is
  `s^3+(1/T_c)s^2+(K_r*Kp/T_c)s+K_r*Ki/T_c=0`. Matching the qualified
  coefficients `0.2`, `0.04`, `0.001` for `T_c=5` and `K_r=0.005` gives
  `Kp=0.04*T_c/K_r=40` and `Ki=0.001*T_c/K_r=1`; solving this polynomial
  gives the reported poles and evaluating the same open loop gives the margins.
- **Prediction:** At `7 bar`, `50 %` delivery and `50 %` demand, predict zero
  pressure rate. For a demand step to `65 %`, predict an initial
  `-0.075 bar/s` pressure rate until delivery changes. Predict that static
  one-for-one feedforward immediately adds `15 pp` to the request but that the
  five-second compressor lag prevents an instantaneous delivery change; PI
  trim remains necessary for model mismatch. Predict the sign of feedback and
  integral action, the amplitude/slew-limited first request, and the abnormal
  outputs before running.
- **Controller under test:** The shipped `examples/control/14-integrated-capstones/84-independent`
  reference shall contain two equivalent compressed-air plants: qualified PI
  and PI plus measured-demand feedforward. Actual Structured Text shall use the
  shared `Control.PID`, expose every term and requested/applied command, pass
  feedforward through the controller's configured authority/anti-windup path,
  track actual applied delivery, enforce mode/quality/availability/amplitude/
  slew authority, and publish unchanged revision `84`. It shall contain no
  response metric, precomputed trace, winner, application approval or hidden
  acceptance result. Concise comments shall explain only non-obvious ownership
  and tracking decisions.
- **Experiment:** Perform the exact two-stage Control Studio journey. Control
  Studio may write only Boolean `P1.ExperimentEnabled`; the reference project
  owns all demand, abnormal, mode and restoration events.
  1. **Monitor** verifies project/plant/evidence identity, SP/PV/MV identities
     and units, pressure, requested/applied/actual delivery, demand, quality,
     mode, availability, sample interval, limits, external protection boundary,
     revision `84` and rollback.
  2. **Test** select **Observe without excitation**, amplitude `0`, and duration
     `300.1 s`. Review that the sole write is `P1.ExperimentEnabled` at
     `%MX9.2`, with baseline and restoration `FALSE`. The project owns the
     `50->65 %` demand change at `60 s`, `65->55 %` change at `180 s`, invalid
     demand, pressure and delivery quality, unavailable compressor, Manual,
     ControlledStop, Safe restoration and recovery. Separate project/native
     records own identification, uncertainty and frequency evidence. The
     learner interprets and defends those results using the derived equations;
     disabled Identify, Tune or Validate screens are not authority.
- **Comparison and decision measures:** Classify records before ranking. Define
  pressure integrated absolute error in `bar*s`, maximum pressure departure in
  bar, two-percent settling only for settled records, time outside
  `6.5..7.5 bar`, requested/applied total variation in percentage points,
  peak and time at delivery limit, maximum tracking residual, feedforward onset
  delay, stability margins, sensitivity peak, model residual and recovery bump.
  Retain calculations, exact windows, samples, revisions, uncertainty and
  Simulation label. Candidate acceptance requires no normal-envelope violation,
  no hard-limit violation, qualified margins, deterministic fallback and at
  least `30 %` lower regulatory IAE without more than `25 %` additional applied
  movement versus feedback PI.
- **Validation:** Native execution shall compile and run the actual ST against
  nominal and uncertainty plants and compare it with independent continuous,
  sampled, identification, time-domain and frequency-domain oracles. The
  integrated candidate shall be selected only if every declared requirement
  passes. Invalid demand shall remove feedforward while retaining qualified PI;
  invalid primary pressure or unavailable compressor shall fail closed;
  invalid delivery feedback shall remove automatic authority because applied-
  output tracking is no longer trustworthy. Manual and ControlledStop shall be
  bounded and bumpless; recovery requires restored quality and a new reviewed
  run. Revision `84` remains unapplied.
- **Explanation:** Trace receiver physics into the two identified parameters,
  those parameters into the PI and margin calculation, measured demand into
  feedforward, both terms through one actuator/anti-windup boundary, and common
  run evidence into the final decision. Explain why feedforward acts before
  pressure error but cannot correct an unknown leak, why PI supplies trim, why
  compressor lag limits improvement, and why rejected advanced architectures
  would add cost without an evidenced failure to solve.
- **Transfer and required result:** Produce a reproducible engineering report
  for the learner-selected plant containing requirements, topology, safety
  ownership, theory, calculations, experiment approvals, raw/restored records,
  identified models and uncertainty, rejected and retained architectures,
  tuning/design, stability/robustness, normal/abnormal evidence, selected
  unapplied revision, application approval plan, rollback, monitoring and
  maintenance triggers. Distinguish calculations, configured Simulation truth,
  identified-model evidence and physical evidence. The reference compressed-
  air solution proves that the assessment workflow is executable; course
  completion still requires the learner to defend every material choice for
  their selected plant rather than copy the reference conclusion.

## 4. Required progression behavior

The normal course order is Lessons 1 through 84. A learner may enter later only
after demonstrating the stated prerequisite knowledge. The product shall make
the dependency chain visible and shall not present advanced lessons as equally
appropriate alternatives to foundational ones.

The top-level `examples/control/README.md` shall provide the complete course
map as one table in this same canonical order. Each row shall link to the real
runnable project and state the lesson number, lesson name, real equipment, the
principal theory taught, the applicable Control Studio workflow stages, and
the practical skill or engineering decision the learner will practise. The
table shall be derived from and agree with the shipped catalog and lesson
descriptors; it shall not maintain a second lesson sequence or advertise an
unimplemented example.

Each canonical lesson project's directory basename shall begin with its
two-digit, zero-padded course number followed by a hyphen, from `01-` through
`84-`. Stable lesson IDs shall remain independent of the filesystem name. The
catalog, course map, package generator, tests, and live documentation links
shall all resolve the numbered canonical path rather than retaining aliases or
duplicate unnumbered lesson directories.

The parent directories under `examples/control/` shall represent only the 14
consecutive learner modules defined by this specification and its implementation
checklist. A module shall contain one uninterrupted lesson-number range; a
lesson shall not be filed by perceived difficulty, audience, plant industry, or
an overlapping technique taxonomy. The root README shall instead give engineers
separate tables for finding lessons by control technique, plant behavior or
engineering problem, and applicable Control Studio workflow. Those tables are
navigation aids over the same canonical lessons, not alternate curricula.

The progression has these mandatory causal transitions:

- ON/OFF chatter motivates hysteresis and dwell protection.
- Persistent cycling motivates continuous P control.
- P operating-point behavior motivates bias.
- Load-dependent P offset motivates integral action and PI.
- Saturation and slew limits motivate anti-windup.
- Conflicting servo and regulatory objectives motivate setpoint weighting.
- Noise and timing sensitivity motivate filtering and digital implementation.
- Repeated empirical tuning motivates modelling.
- Unknown physical parameters motivate identification.
- Possible overfit motivates independent validation.
- A validated model motivates model-based tuning.
- Oscillatory mechanical behavior motivates PD and classical stability tools.
- Unexplained gain effects motivate root locus.
- Hidden resonance and uncertainty motivate Bode analysis.
- Delay and difficult pole configurations motivate Nyquist analysis.
- Disturbance/noise/robustness trade-offs motivate sensitivity functions.
- Measured early disturbances motivate feedforward.
- Fast secondary disturbances motivate cascade.
- Competing safety constraints motivate selectors and tracking.
- Operating-point dependence motivates gain scheduling.
- Delay dominance motivates Smith prediction.
- Strong interaction motivates RGA, decoupling and MIMO control.
- Hidden internal states motivate observers and Kalman filtering.
- Multistate performance trade-offs motivate LQR/LQG.
- Coupled predictions and constraints motivate MPC.
- Large nonlinear excursions motivate nonlinear stability methods.
- Continuous plant drift motivates adaptive control.
- Bounded model uncertainty motivates robust control.
- Communication and component faults motivate networked and fault-aware modes.
- Long-term degradation motivates performance monitoring and diagnosis.

No lesson may reverse these dependencies by using a technique before explaining
the observed problem and prerequisite theory that justify it.

## 5. Relationship to product specifications

This learning program uses, but does not redefine:

- [`32-control-pid.md`](32-control-pid.md) for the `Control` controller and
  filter behavior;
- [`33-simulation-plant-models.md`](33-simulation-plant-models.md) for plant
  simulation and nonideality behavior;
- [`35-vscode-control-studio.md`](35-vscode-control-studio.md) for Monitor,
  Test, Identify, Tune and Validate behavior;
- [`36-control-pid-performance-evaluation.md`](36-control-pid-performance-evaluation.md)
  for controller performance evidence; and
- [`37-control-advanced-structures.md`](37-control-advanced-structures.md) for
  currently specified Smith prediction, gain scheduling and decoupling.

If a lesson requires product behavior not yet defined by those specifications,
that behavior shall be specified and implemented before the lesson may claim
to execute it. Curriculum prose or a prepared screenshot shall not stand in for
missing controller, simulator, numerical, plotting or Control Studio behavior.

## 6. Explicit exclusions

Fractional-order PID, second-derivative controllers, repetitive control and
other research variants are not part of the core sequence. They may be added
only after Lesson 84 when they solve a stated real-world problem, have the
required preceding theory, execute a genuine algorithm, and can be compared
honestly with the simpler qualified alternatives.
