# Rust PLC VS Code Acceptance Journeys

**Status:** acceptance contract, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md` v2.4.
**Applies to:** RS-118..RS-126, S13..S17, G0-in-IDE, B0-in-IDE.

This file captures the Rust PLC VS Code journeys that must be mirrored into
the broader VS Code UI/UX acceptance board when implementation starts. It
exists here so the Rust PLC package remains complete in fresh worktrees.

## Rules

- Every journey uses visible VS Code surfaces only. No terminal and no command
  palette unless a step explicitly says "advanced escape hatch".
- Every journey records reviewer != implementer.
- Every journey records screenshots or screen capture.
- Core webview surfaces require light, dark, and high-contrast evidence.
- A journey is not accepted from DOM/HTTP proof alone when the user sees a
  rendered surface.
- Evidence strings use the master claim vocabulary.

## J-R1 - Rust PLC First Contact

Goal: clean VS Code profile to running Rust PLC simulator under 10 minutes.

Steps:

1. Install/open the truST extension.
2. Click the truST Activity Bar icon.
3. Choose Create project -> Rust PLC.
4. Create the seeded motor-latch project.
5. Press Run with Target = Simulator.
6. Verify Live Values shows the Rust instance RUNNING.
7. Capture wall-clock time and screenshots.

Acceptance:

- no terminal;
- no command palette;
- project detected as `rust-plc`;
- no "Initialize truST here" false state;
- Run path uses BC-1 and BC-2;
- screenshot evidence attached.

## J-R2 - Fault And Safe State

Goal: prove F1 in the product surface.

Steps:

1. Start the Rust PLC motor-latch simulator.
2. Trigger the seeded panic/fault input from Live Values.
3. Observe instance state, fault code, source location, safe-default outputs,
   and scan continuation.
4. Reset the fault if authorized.

Acceptance:

- FAULTED instance visible in Live Values;
- `_FAULT_CODE` renders symbolic and numeric code;
- safe-default outputs are tagged;
- source link opens Rust source;
- scan continues after fault.

## J-R3 - Generated ST Review

Goal: prove generated IEC compatibility is reviewable, not hidden.

Steps:

1. Open a Rust POU.
2. Use View generated declaration.
3. Change a Rust interface field.
4. Run Check.
5. Open Review generated ST changes.

Acceptance:

- generated ST opens read-only with generated-artifact banner;
- native VS Code diff shows regenerated vs committed generated ST;
- direct generated-file edit triggers F22 flow;
- reviewer can navigate back to Rust source.

## J-R4 - Admission Refusal

Goal: prove timing admission is visible and actionable in VS Code.

Steps:

1. Open the refused coincidence-frame fixture.
2. Run Check from the sidebar.
3. Open Problems and Admission report.

Acceptance:

- Check badge reports refused state;
- F16 Problems entry anchors to `trust.toml`;
- Admission report shows worst frame first;
- coincident tasks are named;
- evidence grades render using claim vocabulary;
- no average-utilization-only summary.

## J-R5 - Brownfield Rust FB

Goal: prove an existing ST project can adopt one Rust FB without Rust-first
project migration.

Steps:

1. Open the brownfield ST corpus project.
2. Add one Rust FB through the visible Libraries flow.
3. Regenerate declaration.
4. Use the FB from ST.
5. Run and observe Live Values.
6. Induce the Rust FB fault.

Acceptance:

- no `trust.toml` greenfield dependency;
- ST completion/hover sees the Rust FB declaration;
- ST supervises Rust fault status;
- Live Values shows `_STATE/_FAULT_CODE/_OVERRUNS`;
- unchanged ST corpus behavior remains accepted.

## J-R6 - Replay Trace From Testing

Goal: prove machine tests and traces work as a VS Code workflow.

Steps:

1. Open a Rust PLC project with machine/replay tests.
2. Run tests from VS Code Testing.
3. Trigger a known replay divergence.
4. Use Replay trace action.

Acceptance:

- Machine, Replay, and Unit tests are grouped;
- failure attaches a `.trusttrace`;
- Replay action runs BC-5;
- divergence report shows variable-level diff;
- source links open the relevant Rust or generated artifact location.
