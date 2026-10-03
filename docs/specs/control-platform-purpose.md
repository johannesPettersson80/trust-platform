# Control Platform Purpose and Acceptance Contract

Status: required shared product direction, agreed with the user on 11 September
2026. This document defines the purpose connecting the controller, simulation,
Control Studio and course. It does not assert that the existing implementation
or examples already satisfy every acceptance condition.

## 1. The product we are building

truST combines four parts into one practical control-engineering platform:

1. **`Control.PID`**: the implemented combined controller based on *A Practical
   Guide to PID Controller Implementation*. Preserve its specified execution
   semantics and its independent `Control.LowPass` filtering boundary.
2. **Plant simulation**: realistic, deliberately imperfect processes connected
   through ordinary PLC I/O. Include the declared dynamics, actuator limits,
   delays, disturbances, noise and faults needed to evaluate a controller.
3. **Control Studio**: observe, identify, choose a controller, tune, verify the
   actual ST, apply, observe adoption, monitor and roll back through one coherent
   engineering workflow. Use shared truST components, a clear current stage,
   one primary next action and expandable technical detail.
4. **A practical course**: teach both our controller and broader control theory
   through real physical problems, runnable ST projects and production-ready
   examples that people can adopt after the course or copy directly.

The value is the connection: learn the principle, inspect the real code, run it
against a process, measure what happened, improve the design and carry the same
controller code toward a real application.

### User direction, 29 September 2026

Control Studio is truST's general-purpose identification and tuning software:
easy to use and highly competent, able to replace difficult tools such as
MATLAB. It must work on any truST project and control structure, including a
single loop, two PIDs where one feeds the other, several PIDs, multivariable
processes and other controller types. It must not hard-code any domain or
lesson; domain names come from the project. Every example and tutorial teaches
both the control theory and how to use Control Studio for it. The competence
target that makes this concrete is proposed in Specification 35 §17.8 and
becomes binding once the user confirms it.

### User direction, 30 September 2026

The course showcases truST and how it works, and teaches practical and
theoretical control better than a university course does: from the ground up
to very advanced topics. Every lesson follows the current Control Studio user
interface and is short and straight to the point; it explains every new
control-theory term clearly where it first appears; it builds on the previous
lesson, except where a standalone lesson serves the course better; and it
assumes no mathematics beyond basic calculus and basic linear algebra,
explaining anything beyond that clearly and briefly. The course is reviewed for
missing topics and for lessons that add no value; additions and deletions are
proposed to the user before they are made. Control Studio must be competent
enough to tune every kind of control system the course teaches, in an easy and
intuitive way.

## 2. Keep the actual controller in the acceptance path

The primary PID acceptance and demonstration project MUST instantiate and
execute the real `Control.PID` library block through its `Configure`,
`Initialize` and `Update` contract. Its output must drive the simulated plant
through normal PLC outputs; feedback must return through normal PLC inputs.
Confirm the executed ST and loaded library; a fixture name, UI label or declared
controller type alone is not evidence that the real block ran.

P, PI, PD and PID are configurations of this same block. For example, P behavior
uses zero integral and derivative gains. Choosing the simplest adequate
controller does not justify replacing the block with a handwritten equation
when claiming acceptance of the PID engineering workflow.

Small equations and simplified fixtures are useful for explanation and focused
diagnosis. Label their scope explicitly. A successful P-only fixture, numerical
prediction, screenshot, compile or aggregate test count MUST NOT substitute for
the complete `Control.PID` acceptance journey.

Other controllers and techniques are legitimate where the engineering problem
or approved lesson requires them. They extend the platform through the existing
contracts; they do not displace its established PID baseline or justify parallel
implementations of existing simulation, communication, storage or UI services.

## 3. Course examples are reusable engineering deliverables

Follow the approved teaching specification and preserve its causal progression:
a physical problem, the limitation of the current approach, a motivated new
technique, implementation, experiment, evidence and the next limitation.
For example, tune P control, observe its steady-state error, introduce integral
action using `Control.PID`, and demonstrate how PI addresses that limitation.

A production-ready example MUST provide:

- complete runnable ST and project configuration using the appropriate real
  library components, with no hidden test-harness dependency;
- documented I/O bindings, units, scaling, plant assumptions, timing and
  applicability limits;
- coherent parameter/state initialization, operating modes, actuator limits
  and the fault/recovery behavior required by that application;
- reproducible verification of its stated behavior, including relevant adverse
  conditions, with honest simulation versus physical-evidence labels; and
- instructions for adaptation and the configuration and validation required
  before use on another plant.

Normal control code must use normal PLC I/O rather than simulation-only control
logic. Simulated plants support development and rehearsal; a copyable example
is not a claim of universal tuning, hardware safety or site qualification.

Teaching snippets may simplify an idea, but must be distinguished from the
complete reusable project. Passing software tests alone does not establish that
the explanation or learning sequence is understandable.

## 4. Acceptance must name what actually ran

For the primary PID journey, evidence MUST cover the real block and the actual
Control Studio path: model acquisition or supply, tuning, actual-ST verification,
explicit application, observed parameter/state adoption, monitoring and rollback.
Exercise the applicable controller features, including modes, limits, retained
state, anti-windup and filtering, in their specified scenarios.

Every completion report MUST identify the project, controller implementation,
runtime/build identity, plant configuration, timing, operations exercised and
evidence locations. Separate focused tests, executed integration, rendered UI,
teaching review and physical qualification. Name missing evidence and blockers.

Do not report this connected PID goal complete on the strength of another
controller's successful journey. Do not replace a missing integration with a
simpler demonstration or relabel it to obtain a green result. Retain useful
fixtures, recordings and earlier valid results with their actual scope.

### Whole-platform visual review and the user acceptance sequence

For every required UI/UX screenshot, the agent must explicitly inspect visual
polish (the user's requirement: stunning), ease of use, self-explanatory and
explorable interaction, and consistency across the platform. This includes all
plant-simulation, Devices & Connections, Control Studio, settings, dialogs,
charts and other included surfaces. Looking only at selected successful images
or checking that a canvas exists does not establish acceptance.

Development, agent execution of the complete tutorial/platform, inspection of
all required UI/UX pictures, and independent review must be complete before
asking the user to perform the final full tutorial walkthrough. The user then
personally exercises the entire course. Agents repair findings and reverify
before user retest; only the user can provide that final acceptance. An early
preview explicitly requested by the user does not count as this acceptance.

Track these gates and release preparation in
`docs/internal/testing/checklists/control-platform-release-readiness.md` in the
recovered control implementation checkout. Checklist creation does not close
any implementation, verification, user or release gate.

## 5. Existing authorities and checkout recovery

This contract owns shared purpose and completion claims. Existing detailed
specifications continue to own their algorithms, interfaces and behavior:

| Area | Authority path within the control implementation checkout |
|---|---|
| PID and filter | `docs/specs/32-control-pid.md` |
| Plant simulation | `docs/specs/33-simulation-plant-models.md` |
| Plant configuration UI | `docs/specs/34-vscode-plant-simulation-ui.md` |
| Control Studio | `docs/specs/35-vscode-control-studio.md` and `docs/specs/control-studio-complete-workflow-specification.md` |
| Executed PID benchmarks | `docs/specs/36-control-pid-performance-evaluation.md` |
| Composed advanced structures | `docs/specs/37-control-advanced-structures.md` |
| Teaching and example progression | `docs/specs/control-course-teaching-specification.md` |
| Engineering rationale and history | `docs/guides/CONTROL_PID_ENGINEERING_REPORT.md` |

Keep these responsibilities separate; do not duplicate their detailed contracts
here. Resolve an apparent conflict against the user's agreed purpose and the
owning specification before making an acceptance claim. Record explicit user
changes to the purpose or acceptance target here; ordinary implementation
choices within the authorized scope need no additional approval.

The canonical agent-file checkout and a feature worktree may contain different
product code. If a listed authority or implementation is absent, recover the
existing worktree and session checkpoint using Git worktree/history evidence.
Absence in the current checkout is not evidence that the feature needs to be
implemented again. Report the chosen checkout, branch, HEAD and preserved WIP.
Do not overwrite or silently merge differing implementations during recovery.

## 6. Mandatory task checkpoint

Use this checkpoint for every new or resumed task concerning any of the four
parts. Record it in the active checklist or task record; retain earlier entries
with their evidence rather than reusing an old checkmark for a new task.

- [ ] **Start:** read this contract and the owning specifications; name the
  part being advanced, the recovered implementation/worktree, the actual
  controller and plant where applicable, and what the task must demonstrate.
- [ ] **Finish:** compare the result with that stated purpose; name what
  actually executed and what was reviewed, link the evidence, state remaining
  gaps, and ensure no simplified fixture has replaced the acceptance target.

For documentation-only work, record the intended execution target and verify
the document's references and consistency. Do not invent runtime, UI, teaching
or hardware execution evidence for a documentation change.
