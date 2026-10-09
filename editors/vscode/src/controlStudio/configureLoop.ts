import { nativeSetupPrompts } from "./nativeSetupPrompts";
import { ControlSetupSession } from "./setupNavigation";
import type { SetupDialogs, SetupPromptPresenter } from "./setupPromptTypes";
import { configureControllerAdapter } from "./configureControllerAdapter";
import { configureOnOffController } from "./configureOnOffController";
import { configureLoopChannels } from "./configureLoopChannels";
import * as fs from "fs";
import * as path from "path";
import { createHash } from "crypto";
import * as vscode from "vscode";
import { validateControlLoopDefinition, type ControlLoopDefinition, type ControlSignalBinding } from "./loopDefinition";
import { assertControlLoopIdAvailable, type ProjectControlLoopResolution } from "./projectLoop";
import { MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS } from "./controlStudioLimits";

export type DiscoverControlSignals = (rootUri: string) => Promise<unknown>;
interface CompiledSignal { symbol: string; address: string; type: string }
interface CompiledController { instance: string; type: string; tasks: { name: string; intervalMs: number }[] }
interface CompiledControlSymbol { symbol: string; type: "REAL" | "LREAL" | "BOOL" }
type SetupWorkflow = "observation" | "proportional" | "on-off" | "adapter";

/** Owns explicit setup and an exclusive configuration save; never edits ST or writes runtime values. */
export async function configureControlLoop(
  runtimeId: string,
  discover: DiscoverControlSignals | undefined,
  presenter: SetupPromptPresenter = nativeSetupPrompts
): Promise<ProjectControlLoopResolution> {
  const ui = new ControlSetupSession(presenter);
  const discoveries = new Map<string, Promise<unknown>>();
  try {
    return await ui.run<ProjectControlLoopResolution>(async () => {
      const folders = vscode.workspace.workspaceFolders ?? [];
      const folder = folders.length === 1 ? folders[0] : (await ui.showQuickPick(
        folders.map(folder => ({ label: folder.name, description: folder.uri.fsPath, folder })),
        { title: "Configure control loop", placeHolder: "Choose the project containing your controller" }
      ))?.folder;
      if (!folder) return { kind: "cancelled" };
      if (!discover) return { kind: "missing", reason: "Compiled signal discovery is unavailable. Wait for the language server and retry." };
      const source = folder.uri.toString();
      const discovery = discoveries.get(source) ?? discover(source);
      discoveries.set(source, discovery);
      const payload = await discovery as {
        ok?: boolean; error?: string;
        processImage?: { commands?: unknown[]; feedbacks?: unknown[]; memory?: unknown[] };
        controlSymbols?: { controllers?: unknown[]; symbols?: unknown[]; error?: string };
      };
      if (payload?.ok !== true) throw new Error(payload?.error ?? "Compile the project successfully before configuring its loop.");
      const signals = [...(payload.processImage?.commands ?? []), ...(payload.processImage?.feedbacks ?? []),
        ...(payload.processImage?.memory ?? [])].filter(isCompiledSignal);
      if (!signals.length) throw new Error("No compiled process-image signals are available. Symbolic-only setup is not available yet.");
      const name = await input(ui, "Loop name"); if (name === undefined) return { kind: "cancelled" };
      const controllers = (payload.controlSymbols?.controllers ?? []).filter(isCompiledController);
      const chosen = controllers.length ? await ui.showQuickPick([
        ...controllers.map(controller => ({
          label: controller.instance,
          description: controller.type === controller.instance ? undefined : controller.type,
          controller: controller as CompiledController | undefined,
        })),
        { label: "Enter a custom controller", description: "Declare its identity explicitly", controller: undefined },
      ], { title: "Controller", placeHolder: "Choose the compiled controller instance" }) : { controller: undefined };
      if (!chosen) return { kind: "cancelled" };
      const instance = chosen.controller?.instance ?? await input(ui, "Controller instance"); if (instance === undefined) return { kind: "cancelled" };
      const type = chosen.controller?.type ?? await input(ui, "Controller type"); if (type === undefined) return { kind: "cancelled" };
      const task = chosen.controller?.tasks.length === 1 && chosen.controller.tasks[0].intervalMs > 0 ? chosen.controller.tasks[0] : undefined;
      const period = task?.intervalMs ?? await number(ui, "Task period (ms)", value => value > 0);
      if (period === undefined) return { kind: "cancelled" };
      const pv = await selectSignal(ui, "Process variable", signals); if (!pv) return { kind: "cancelled" };
      const mv = await selectSignal(ui, "Controller output", signals.filter(signal => signal.address !== pv.address));
      if (!mv) return { kind: "cancelled" };
      const sp = await ui.showQuickPick([
        { label: "No setpoint (open loop)", signal: undefined as CompiledSignal | undefined },
        ...signals.filter(signal => signal.address !== pv.address && signal.address !== mv.address)
          .map(signal => ({ label: signal.symbol, description: `${signal.address} · ${signal.type}`, signal })),
      ], { title: "Setpoint", placeHolder: "Choose the reference used by the controller, if any" });
      if (!sp) return { kind: "cancelled" };
      const workflow = await selectWorkflow(ui, Boolean(sp.signal));
      if (!workflow) return { kind: "cancelled" };
      ui.phase = 2;
      const processUnit = await input(ui, "Process unit"); if (processUnit === undefined) return { kind: "cancelled" };
      const processMinimum = await number(ui, "Process minimum"); if (processMinimum === undefined) return { kind: "cancelled" };
      const processMaximum = await number(ui, "Process maximum", value => value > processMinimum);
      if (processMaximum === undefined) return { kind: "cancelled" };
      const outputUnit = await input(ui, "Output unit"); if (outputUnit === undefined) return { kind: "cancelled" };
      const outputMinimum = await number(ui, "Output minimum"); if (outputMinimum === undefined) return { kind: "cancelled" };
      const outputMaximum = await number(ui, "Output maximum", value => value > outputMinimum);
      if (outputMaximum === undefined) return { kind: "cancelled" };
      const projectId = `project:${digest(folder.uri.toString())}`;
      const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "loop";
      const loopId = `loop:${slug}`;
      const binding = (signal: CompiledSignal, unit: string, minimum: number, maximum: number,
        direction: "read" | "write" = "read"): ControlSignalBinding => ({
        symbol: signal.symbol, address: signal.address, iecType: signal.type,
        direction, unit, range: { minimum, maximum },
      });
      ui.phase = 3;
      const proportional = workflow === "proportional" ? await proportionalSetup(ui, 
        payload.controlSymbols?.symbols ?? [], task?.name
      ) : undefined;
      if (workflow === "proportional" && !proportional) return { kind: "cancelled" };
      const manualOutput = proportional ? signals.find(signal =>
        signal.symbol === proportional.manualCommand.symbol && signal.type === proportional.manualCommand.type
      ) : undefined;
      // B1.3: a proportional setup never asks for an identification duration; Identify chooses it and Limits supplies the maximum.
      // The observation workflow keeps its duration, because a project-scripted experiment block needs one (loop schema).
      // A recording keeps any experiment of up to 24 h: a long one is recorded with a record period (A4.2, A7).
      const durationSeconds = workflow === "proportional" ? undefined : await number(ui, "Observation duration (seconds)", value =>
        value >= MINIMUM_EXPERIMENT_DURATION_MS / 1_000 && value <= MAXIMUM_EXPERIMENT_DURATION_MS / 1_000);
      if (workflow !== "proportional" && durationSeconds === undefined) return { kind: "cancelled" };
      const loop: ControlLoopDefinition = {
        schemaVersion: 2, id: loopId, name, projectId, runtimeId,
        revision: "sha256:pending", topology: sp.signal ? "closed-loop" : "open-loop",
        // This setup establishes observation. Runtime revision and adoption contracts
        // must be resolved independently before any controller parameter application.
        controller: { instance, type, design: type === "Control.PID" && sp.signal ? "pid" : "project",
          revision: "sha256:unverified", action: "direct", taskPeriodMs: period,
          ...(proportional ? { law: { kind: "proportional" as const, gainSymbol: proportional.gain.symbol },
            manualOutputSignal: proportional.manualCommand.symbol, manualOutputType: proportional.manualCommand.type,
            ...(manualOutput ? { manualOutputAddress: manualOutput.address } : {}),
            modeSignal: proportional.mode.symbol,
            modeWhenTrue: proportional.mode.manualWhenTrue ? "Manual" as const : "Auto" as const } : {}), },
        signals: {
          processVariable: binding(pv, processUnit, processMinimum, processMaximum),
          manipulatedVariable: binding(mv, outputUnit, outputMinimum, outputMaximum,
            proportional ? "write" : "read"),
          ...(sp.signal ? { setpoint: binding(sp.signal, processUnit, processMinimum, processMaximum) } : {}),
        },
        limits: { outputMinimum, outputMaximum }, observedSampleTiming: { nominalPeriodMs: period },
        ...(durationSeconds === undefined ? {} : { experiment: { execution: proportional ? "control-studio" as const : "project-scripted" as const,
          kind: "observation" as const, amplitude: 0, durationSeconds, seed: 1 } }),
        ...(proportional ? { parameterBinding: {
          loop_id: loopId, task_name: proportional.taskName,
          parameters: [{ symbol: proportional.gain.symbol, iec_type: proportional.gain.type, unit: `${outputUnit}/${processUnit}`,
            convention: "proportional gain", minimum: proportional.minimum, maximum: proportional.maximum }],
          mode: { symbol: proportional.mode.symbol, manual_value: proportional.mode.manualWhenTrue },
        } } : {}),
      };
      if (workflow === "adapter" && !await configureControllerAdapter(loop,
        (payload.controlSymbols?.symbols ?? []).filter(isCompiledControlSymbol), task?.name, ui)) return { kind: "cancelled" };
      if (workflow === "on-off" && !await configureOnOffController(loop,
        (payload.controlSymbols?.symbols ?? []).filter(isCompiledControlSymbol), task?.name,
        { text: title => input(ui, title), number: (title, valid) => number(ui, title, valid),
          symbol: (title, symbols, types) => selectControlSymbol(ui, title, symbols, types) }, ui)) return { kind: "cancelled" };
      ui.phase = 4;
      if (!await configureLoopChannels(loop, signals, (payload.controlSymbols?.symbols ?? []).filter(isCompiledControlSymbol), ui)) return { kind: "cancelled" };
      loop.revision = `sha256:${digest(JSON.stringify(loop))}`;
      const validated = validateControlLoopDefinition(loop, { projectId, runtimeId });
      if (validated.kind !== "accepted") throw new Error(`The selected bindings are incompatible: ${validated.reason.replace(/_/g, " ")}.`);
      const action = await vscode.window.showInformationMessage(
        `Save ${name} for ${runtimeId}? PV: ${pv.symbol} (${processUnit}); output: ${mv.symbol} (${outputUnit}); ${period} ms. ${proportional ? "This declares the existing proportional controller; it does not write the runtime or edit ST." : workflow === "on-off" || workflow === "adapter" ? "This declares the existing controller and its typed adapter; it does not edit ST or write runtime values." : "This loop observes the existing program."}`,
        { modal: true }, "Save loop"
      );
      if (action !== "Save loop") return { kind: "cancelled" };
      await assertControlLoopIdAvailable(folder.uri.fsPath, loop.id);
      const directory = path.join(folder.uri.fsPath, "control-loops");
      await fs.promises.mkdir(directory, { recursive: true });
      await fs.promises.writeFile(path.join(directory, `${slug}.json`), `${JSON.stringify(loop, null, 2)}\n`, { flag: "wx" });
      return { kind: "accepted", request: { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId, runtimeId }, loopDefinition: loop } };
    });
  } catch (error) {
    return { kind: "missing", reason: error instanceof Error ? error.message : "Loop setup failed. No runtime values were changed." };
  }
}

function isCompiledSignal(value: unknown): value is CompiledSignal {
  if (!value || typeof value !== "object") return false;
  const signal = value as Partial<CompiledSignal>;
  return typeof signal.symbol === "string" && !!signal.symbol && typeof signal.address === "string"
    && typeof signal.type === "string";
}
function selectSignal(ui: SetupDialogs, title: string, signals: readonly CompiledSignal[]): Promise<CompiledSignal | undefined> {
  return Promise.resolve(ui.showQuickPick(signals.map(signal => ({
    label: signal.symbol, description: `${signal.address} · ${signal.type}`, signal,
  })), { title })).then(item => item?.signal);
}
function input(ui: SetupDialogs, title: string): Thenable<string | undefined> {
  return ui.showInputBox({ title, ignoreFocusOut: true,
    validateInput: value => value.trim() ? undefined : "Enter a value." }).then(value => value?.trim());
}
async function number(ui: SetupDialogs, title: string, valid: (value: number) => boolean | string = () => true): Promise<number | undefined> {
  const value = await ui.showInputBox({ title, ignoreFocusOut: true,
    validateInput: text => {
      if (!text.trim() || !Number.isFinite(Number(text))) return "Enter a valid finite number.";
      const validation = valid(Number(text));
      return validation === true ? undefined : typeof validation === "string" ? validation : "Enter a valid finite number.";
    } });
  return value === undefined ? undefined : Number(value);
}
function digest(value: string): string { return createHash("sha256").update(value).digest("hex"); }

function isCompiledController(value: unknown): value is CompiledController {
  if (!value || typeof value !== "object") return false;
  const controller = value as Partial<CompiledController>;
  return typeof controller.instance === "string" && !!controller.instance
    && typeof controller.type === "string" && !!controller.type && Array.isArray(controller.tasks)
    && controller.tasks.every(task => typeof task?.name === "string" && Number.isFinite(task.intervalMs));
}

function isCompiledControlSymbol(value: unknown): value is CompiledControlSymbol {
  if (!value || typeof value !== "object") return false;
  const symbol = value as Partial<CompiledControlSymbol>;
  return typeof symbol.symbol === "string" && !!symbol.symbol && (symbol.type === "REAL" || symbol.type === "LREAL" || symbol.type === "BOOL");
}

async function selectWorkflow(ui: SetupDialogs, hasSetpoint: boolean): Promise<SetupWorkflow | undefined> {
  const selected = await ui.showQuickPick([
    { label: "Observation", description: "Record the existing program without declaring its control law", workflow: "observation" as const },
    ...(hasSetpoint ? [{ label: "Use an existing controller adapter", description: "Import a typed state, industrial or predictive controller mapping", workflow: "adapter" as const }] : []),
    ...(hasSetpoint ? [{ label: "Tune existing ON/OFF controller", description: "Bind hysteresis, dwell and retained switching states", workflow: "on-off" as const }] : []),
    ...(hasSetpoint ? [{ label: "Tune existing proportional controller", description: "Confirm u = Kp * (SP - PV) and bind its existing parameters", workflow: "proportional" as const }] : []),
  ], { title: "Control workflow", placeHolder: "Observation is the default setup route" });
  return selected?.workflow;
}

async function proportionalSetup(ui: SetupDialogs, symbols: readonly unknown[], selectedTask: string | undefined): Promise<{
    gain: CompiledControlSymbol & { type: "REAL" | "LREAL" };
    manualCommand: CompiledControlSymbol & { type: "REAL" | "LREAL" };
    mode: CompiledControlSymbol & { type: "BOOL"; manualWhenTrue: boolean };
    taskName: string; minimum: number; maximum: number;
  } | undefined> {
  const catalog = symbols.filter(isCompiledControlSymbol);
  const gain = await selectControlSymbol(ui, "Proportional gain", catalog, ["REAL", "LREAL"]);
  if (!gain || (gain.type !== "REAL" && gain.type !== "LREAL")) return undefined;
  const numericGain = gain as CompiledControlSymbol & { type: "REAL" | "LREAL" };
  const minimum = await number(ui, "Gain minimum"); if (minimum === undefined) return undefined;
  const maximum = await number(ui, "Gain maximum", value => value >= minimum); if (maximum === undefined) return undefined;
  const manualCommand = await selectControlSymbol(ui, "Manual command", catalog, ["REAL", "LREAL"]);
  if (!manualCommand || (manualCommand.type !== "REAL" && manualCommand.type !== "LREAL")) return undefined;
  const numericManualCommand = manualCommand as CompiledControlSymbol & { type: "REAL" | "LREAL" };
  const rawMode = await selectControlSymbol(ui, "Manual mode", catalog, ["BOOL"]);
  if (!rawMode || rawMode.type !== "BOOL") return undefined;
  const mode = rawMode as CompiledControlSymbol & { type: "BOOL" };
  const polarity = await ui.showQuickPick([
    { label: "TRUE means Manual", manualWhenTrue: true },
    { label: "TRUE means Auto", manualWhenTrue: false },
  ], { title: "Manual mode polarity", placeHolder: "Confirm the existing BOOL convention" });
  if (!polarity) return undefined;
  const taskName = selectedTask ?? await input(ui, "Controller task");
  if (taskName === undefined) return undefined;
  return { gain: numericGain, manualCommand: numericManualCommand, mode: { ...mode, manualWhenTrue: polarity.manualWhenTrue }, taskName, minimum, maximum };
}

function selectControlSymbol(ui: SetupDialogs, title: string, symbols: readonly CompiledControlSymbol[], types: readonly string[]): Promise<CompiledControlSymbol | undefined> {
  return Promise.resolve(ui.showQuickPick(symbols.filter(symbol => types.includes(symbol.type)).map(symbol => ({
    label: symbol.symbol, description: symbol.type, symbol,
  })), { title, placeHolder: "Choose a compiled symbolic leaf" })).then(item => item?.symbol);
}
