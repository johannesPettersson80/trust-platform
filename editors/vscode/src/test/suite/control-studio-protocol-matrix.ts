import * as fs from "fs";
import * as path from "path";
import type { CompiledController, CompiledCatalog, CompiledSymbol, ControllerCallLink } from "../../controlStudio/loopSetup/compiledCatalog";
import { pidLoopDefinition, type PidLoopDraft } from "../../controlStudio/loopSetup/pidLoopSetup";
import { withRevision } from "../../controlStudio/loopSetup/loopBindings";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { withSimulationTimeScale } from "../../simulationSpeed";
import { pidSymbols } from "./control-studio-cascade-fixtures";

/** The D5 runtime cycle: the fast task equals it and the slow task is ten times it (Specification 35, D5). */
const RESOURCE_CYCLE_MS = 10;
/** The output rate limit of the program's Control.PID (RateMinimum and RateMaximum), declared on the loop as its output rate. */
const RATE_LIMIT_PER_S = 25;
const PROJECT_ID = "project:protocol-matrix";

interface LoopSpec {
  readonly label: string;
  readonly program: string;
  readonly programType: string;
  readonly task: "FastTask" | "SlowTask";
  readonly periodMs: number;
  readonly output: "addressed" | "unaddressed";
  readonly setpointAddress: string;
  readonly manualAddress: string;
  readonly processAddress: string;
  /** Addressed output: the manipulated variable is the output image itself. */
  readonly outputAddress?: string;
  /** Unaddressed output: the program copies the manipulated variable to this command, which the plant reads. */
  readonly commandAddress?: string;
}

/** The four loops of D5: one per task period and output kind. */
const LOOPS: readonly LoopSpec[] = [
  { label: "Fast addressed", program: "FA", programType: "AddressedFast", task: "FastTask", periodMs: 10, output: "addressed",
    setpointAddress: "%ML0", manualAddress: "%ML8", processAddress: "%IL0", outputAddress: "%QL0" },
  { label: "Fast unaddressed", program: "FU", programType: "CommandedFast", task: "FastTask", periodMs: 10, output: "unaddressed",
    setpointAddress: "%ML16", manualAddress: "%ML24", processAddress: "%IL8", commandAddress: "%QL8" },
  { label: "Slow addressed", program: "SA", programType: "AddressedSlow", task: "SlowTask", periodMs: 100, output: "addressed",
    setpointAddress: "%ML32", manualAddress: "%ML40", processAddress: "%IL16", outputAddress: "%QL16" },
  { label: "Slow unaddressed", program: "SU", programType: "CommandedSlow", task: "SlowTask", periodMs: 100, output: "unaddressed",
    setpointAddress: "%ML48", manualAddress: "%ML56", processAddress: "%IL24", commandAddress: "%QL24" },
];

/** What the compiled program declares for the four loops; it must match the Structured Text in `MATRIX_PROGRAM`. */
const MATRIX_CATALOG: CompiledCatalog = {
  symbols: LOOPS.flatMap((spec): CompiledSymbol[] => [
    { symbol: `${spec.program}.Setpoint`, type: "LREAL", address: spec.setpointAddress },
    { symbol: `${spec.program}.ManualOutput`, type: "LREAL", address: spec.manualAddress },
    { symbol: `${spec.program}.AutoMode`, type: "BOOL" },
    { symbol: `${spec.program}.ProcessValue`, type: "LREAL", address: spec.processAddress },
    { symbol: `${spec.program}.ManipulatedValue`, type: "LREAL", ...(spec.outputAddress ? { address: spec.outputAddress } : {}) },
    ...(spec.commandAddress ? [{ symbol: `${spec.program}.Command`, type: "LREAL", address: spec.commandAddress }] : []),
    ...pidSymbols(`${spec.program}.Regulator.Controller`, `${spec.program}.Regulator.Parameters`),
  ]),
  controllers: LOOPS.flatMap((spec): CompiledController[] => [
    { instance: spec.program, type: spec.programType, tasks: [{ name: spec.task, intervalMs: spec.periodMs }] },
    { instance: `${spec.program}.Regulator.Controller`, type: "Control.PID", tasks: [{ name: spec.task, intervalMs: spec.periodMs }] },
  ]),
  links: LOOPS.flatMap((spec): ControllerCallLink[] => {
    const controller = `${spec.program}.Regulator.Controller`;
    return [
      { instance: controller, method: "Configure", argument: "Parameters", record: `${spec.program}.Regulator.Parameters`, output: null, fields: {} },
      { instance: controller, method: "Update", argument: "Input", record: `${spec.program}.Regulator.Input`,
        output: `${spec.program}.ManipulatedValue`,
        fields: { Setpoint: [`${spec.program}.Setpoint`], ProcessValue: [`${spec.program}.ProcessValue`], ManualOutput: [`${spec.program}.ManualOutput`] },
        mode: { symbol: `${spec.program}.AutoMode`, whenTrue: "Auto" as const },
        // In Auto the program copies the output into the manual output, so Manual starts where Auto left off (Specification 35, A6.4).
        copiesOfOutput: [`${spec.program}.ManualOutput`] },
    ];
  }),
  writes: LOOPS.flatMap(spec => [
    `${spec.program}.ManipulatedValue`, `${spec.program}.ManualOutput`,
    ...(spec.commandAddress ? [`${spec.program}.Command`] : []),
  ]),
};

/** One loop of the matrix, bound to the runtime that runs it. */
export interface MatrixLoop {
  readonly label: string;
  readonly task: "FastTask" | "SlowTask";
  readonly output: "addressed" | "unaddressed";
  readonly periodMs: number;
  /** The proportional gain: the parameter check writes it in Manual and rolls it back. */
  readonly proportionalGain: string;
  readonly loop: ControlLoopDefinition;
}

/** The four D5 loops, each a library Control.PID built by `pidLoopDefinition` from `MATRIX_CATALOG`. */
export function matrixLoops(runtimeId: string): MatrixLoop[] {
  return LOOPS.map(spec => {
    const draft: PidLoopDraft = {
      projectId: PROJECT_ID, runtimeId, name: spec.label, instance: `${spec.program}.Regulator.Controller`, action: "direct",
      parameterRecord: `${spec.program}.Regulator.Parameters`,
      processVariable: { symbol: `${spec.program}.ProcessValue`, unit: "degC", minimum: 0, maximum: 100 },
      manipulatedVariable: { symbol: `${spec.program}.ManipulatedValue`, unit: "%", minimum: 0, maximum: 100 },
      setpoint: { symbol: `${spec.program}.Setpoint` },
      mode: { symbol: `${spec.program}.AutoMode`, whenTrue: "Auto" },
      manualOutput: `${spec.program}.ManualOutput`,
    };
    const base = pidLoopDefinition(MATRIX_CATALOG, draft);
    return {
      label: spec.label, task: spec.task, output: spec.output, periodMs: spec.periodMs,
      proportionalGain: `${spec.program}.Regulator.Parameters.ProportionalGain`,
      loop: withRevision({ ...base, limits: { ...base.limits, rateUpPerS: RATE_LIMIT_PER_S, rateDownPerS: RATE_LIMIT_PER_S } }),
    };
  });
}

/**
 * The record periods of D5: with these planned durations and expected settling times, recordPeriodExecutions gives k = 2 for the
 * 10 ms task and for the 100 ms task (Specification 35, A4.2). The journey checks that before it starts each recording. The planned
 * durations are the recordings' maximum process time, so they leave room for the requests around a 3 s experiment at time scale 10.
 */
export const MATRIX_RECORDING: Readonly<Record<"FastTask" | "SlowTask", { plannedMs: number; settlingSeconds: number }>> = {
  FastTask: { plannedMs: 60_000, settlingSeconds: 5 },
  SlowTask: { plannedMs: 60_000, settlingSeconds: 50 },
};

export interface MatrixProject {
  readonly root: string;
  /** The runtime.toml this project was written with, which every start restores. */
  readonly runtimeText: string;
}

/** Writes the four-loop program, the 10 ms resource cycle and the four plants into the workspace copy of the fixture. */
export function writeMatrixProject(root: string): MatrixProject {
  fs.writeFileSync(path.join(root, "src", "Main.st"), MATRIX_PROGRAM);
  const runtimePath = path.join(root, "runtime.toml");
  const runtimeText = fs.readFileSync(runtimePath, "utf8").replace(/cycle_interval_ms = 100\b/, `cycle_interval_ms = ${RESOURCE_CYCLE_MS}`);
  if (!runtimeText.includes(`cycle_interval_ms = ${RESOURCE_CYCLE_MS}`)) throw new Error("The fixture's runtime.toml has no cycle_interval_ms = 100 to change.");
  fs.writeFileSync(runtimePath, runtimeText);
  fs.writeFileSync(path.join(root, "simulation.toml"), MATRIX_SIMULATION);
  // The fixture's control-loop.json describes the old P1 loop, whose symbols this program no longer has.
  fs.rmSync(path.join(root, "control-loop.json"), { force: true });
  return { root, runtimeText };
}

/** Puts the saved time scale into simulation.toml, the value the extension's own simulator starts with. */
export function setMatrixTimeScale(project: MatrixProject, timeScale: number): void {
  const file = path.join(project.root, "simulation.toml");
  fs.writeFileSync(file, withSimulationTimeScale(fs.readFileSync(file, "utf8"), timeScale));
}

/** Restores runtime.toml as written, because an attached start rewrites its endpoint and removes its token. */
export function restoreMatrixRuntime(project: MatrixProject): void {
  fs.writeFileSync(path.join(project.root, "runtime.toml"), project.runtimeText);
}

const MATRIX_PROGRAM = `USING Control;

FUNCTION SamePidParameters : BOOL
VAR_INPUT
    Left : Control.PIDParameters;
    Right : Control.PIDParameters;
END_VAR
SamePidParameters :=
    Left.ProportionalGain = Right.ProportionalGain
    AND     Left.IntegralGain = Right.IntegralGain
    AND     Left.DerivativeGain = Right.DerivativeGain
    AND     Left.ProportionalSetpointWeight = Right.ProportionalSetpointWeight
    AND     Left.DerivativeSetpointWeight = Right.DerivativeSetpointWeight
    AND     Left.Bias = Right.Bias
    AND     Left.OutputMinimum = Right.OutputMinimum
    AND     Left.OutputMaximum = Right.OutputMaximum
    AND     Left.RateMinimum = Right.RateMinimum
    AND     Left.RateMaximum = Right.RateMaximum
    AND     Left.TrackingTime = Right.TrackingTime
    AND     Left.AntiWindup = Right.AntiWindup;
END_FUNCTION

(* One loop type serves all four loops. Each program passes its own cycle time in seconds. *)
FUNCTION_BLOCK MatrixLoop
VAR_INPUT
    CycleSeconds : LREAL;
    Setpoint : LREAL;
    ProcessValue : LREAL;
    ManualOutput : LREAL;
    AutoMode : BOOL;
END_VAR
VAR_OUTPUT
    ManipulatedValue : LREAL;
END_VAR
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#1.0,
        IntegralGain := LREAL#0.2,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-25.0,
        RateMaximum := LREAL#25.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
END_VAR

(* The runtime adopts this complete record at the task boundary. Copy it into
   the actual controller before each Update; declaration defaults run only at startup. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ManipulatedValue;
    InitialState.SaturatedOutput := ManipulatedValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := ManualOutput;
Input.CycleTime := CycleSeconds;
ManipulatedValue := Controller.Update(Input := Input);
END_FUNCTION_BLOCK

(* FA and FU run on the 10 ms task, which equals the resource cycle. FA writes its output to %QL0 directly. *)
PROGRAM AddressedFast
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.01, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
(* In Auto the manual output follows the output, so Auto -> Manual starts where Auto left off (Specification 35, A6.4). *)
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

PROGRAM CommandedFast
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    Command : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.01, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
(* The output has no address: the program copies it to Command, which the plant reads at %QL8. *)
Command := ManipulatedValue;
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

(* SA and SU run on the 100 ms task, ten times the resource cycle. *)
PROGRAM AddressedSlow
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.1, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

PROGRAM CommandedSlow
VAR
    Regulator : MatrixLoop;
    Setpoint : LREAL := 20.0;
    ManualOutput : LREAL := 0.0;
    AutoMode : BOOL := FALSE;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    Command : LREAL := 0.0;
END_VAR
Regulator(CycleSeconds := LREAL#0.1, Setpoint := Setpoint, ProcessValue := ProcessValue,
          ManualOutput := ManualOutput, AutoMode := AutoMode);
ManipulatedValue := Regulator.ManipulatedValue;
Command := ManipulatedValue;
IF AutoMode THEN
    ManualOutput := ManipulatedValue;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK FastTask(INTERVAL := T#10ms, PRIORITY := 1);
    TASK SlowTask(INTERVAL := T#100ms, PRIORITY := 2);
    PROGRAM FA WITH FastTask : AddressedFast;
    PROGRAM FU WITH FastTask : CommandedFast;
    PROGRAM SA WITH SlowTask : AddressedSlow;
    PROGRAM SU WITH SlowTask : CommandedSlow;
END_RESOURCE
VAR_CONFIG
    FA.Setpoint AT %ML0 : LREAL;
    FA.ManualOutput AT %ML8 : LREAL;
    FA.ManipulatedValue AT %QL0 : LREAL;
    FA.ProcessValue AT %IL0 : LREAL;
    FU.Setpoint AT %ML16 : LREAL;
    FU.ManualOutput AT %ML24 : LREAL;
    FU.Command AT %QL8 : LREAL;
    FU.ProcessValue AT %IL8 : LREAL;
    SA.Setpoint AT %ML32 : LREAL;
    SA.ManualOutput AT %ML40 : LREAL;
    SA.ManipulatedValue AT %QL16 : LREAL;
    SA.ProcessValue AT %IL16 : LREAL;
    SU.Setpoint AT %ML48 : LREAL;
    SU.ManualOutput AT %ML56 : LREAL;
    SU.Command AT %QL24 : LREAL;
    SU.ProcessValue AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
`;

/** One first-order plant per loop: a gain of 1.5 degC per percent of output, a 2 s time constant, no noise, the example's actuator limits. */
function plantBlock(id: string, command: string, feedback: string): string {
  return `[[plants]]
id = "${id}"
model = "first_order"
command_sources = ["${command}"]
feedback_targets = ["${feedback}"]
initial_state = [0.0]
initial_input = [0.0]
input_offset = [0.0]
output_offset = [20.0]

[plants.parameters]
gain = 1.5
time_constant_s = 2.0

[plants.actuator]
minimum = 0.0
maximum = 100.0
rate_up_per_s = 25.0
rate_down_per_s = 25.0
`;
}

const MATRIX_SIMULATION = `[simulation]
enabled = true
seed = 20260824
time_scale = 1
plant_step_ms = ${RESOURCE_CYCLE_MS}
max_plant_substeps_per_cycle = 100
plant_trace_capacity = 2000

${plantBlock("fast-addressed", "%QL0", "%IL0")}
${plantBlock("fast-unaddressed", "%QL8", "%IL8")}
${plantBlock("slow-addressed", "%QL16", "%IL16")}
${plantBlock("slow-unaddressed", "%QL24", "%IL24")}`;
