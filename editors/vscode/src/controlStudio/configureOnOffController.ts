import type { SetupDialogs } from "./setupPromptTypes";
import * as vscode from "vscode";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { ParameterBinding } from "./parameterBinding";
import type { ControllerInitialization } from "./controllerInitialState";

interface Symbol { symbol: string; type: "REAL" | "LREAL" | "BOOL" }
interface SetupInputs {
  text(title: string): PromiseLike<string | undefined>;
  number(title: string, valid?: (value: number) => boolean): PromiseLike<number | undefined>;
  symbol(title: string, symbols: readonly Symbol[], types: readonly string[]): Promise<Symbol | undefined>;
}

/** Maps the existing switching program; no PLC source or runtime value is modified. */
export async function configureOnOffController(loop: ControlLoopDefinition, catalog: readonly Symbol[],
  selectedTask: string | undefined, inputs: SetupInputs, ui: SetupDialogs = vscode.window): Promise<boolean> {
  const task = selectedTask ?? await inputs.text("Controller task");
  if (!task) return false;
  const mode = await inputs.symbol("Manual mode", catalog, ["BOOL"]);
  if (!mode) return false;
  const polarity = await ui.showQuickPick([
    { label: "TRUE means Manual", manual: true }, { label: "TRUE means Auto", manual: false },
  ], { title: "Manual mode convention" });
  if (!polarity) return false;
  const manual = await inputs.symbol("Manual output command", catalog, loop.signals.manipulatedVariable.iecType === "BOOL" ? ["BOOL"] : ["REAL", "LREAL"]);
  if (!manual) return false;
  const used = new Set([mode.symbol, manual.symbol, loop.signals.processVariable.symbol,
    loop.signals.manipulatedVariable.symbol, loop.signals.setpoint?.symbol]);
  const parameters: ParameterBinding["parameters"][number][] = [];
  for (const [title, unit] of [["Hysteresis width", loop.signals.processVariable.unit], ["Minimum ON time", "s"], ["Minimum OFF time", "s"]]) {
    const symbol = await inputs.symbol(title, catalog.filter(symbol => !used.has(symbol.symbol)), ["REAL", "LREAL"]);
    if (!symbol || symbol.type === "BOOL") return false;
    const minimum = await inputs.number(`${title}: minimum`, value => value >= 0);
    if (minimum === undefined) return false;
    const maximum = await inputs.number(`${title}: maximum`, value => value >= minimum);
    if (maximum === undefined) return false;
    parameters.push({ symbol: symbol.symbol, iec_type: symbol.type, unit, convention: title, minimum, maximum });
    used.add(symbol.symbol);
  }
  const convention = await ui.showQuickPick([
    { label: "Half-band", description: "Thresholds lie one declared width either side of the target", value: "half-band" as const },
    { label: "Full band", description: "Declared width is the separation between thresholds", value: "full-band" as const },
  ], { title: "Existing hysteresis convention" });
  if (!convention) return false;
  const noiseBand = await inputs.number("Measured process noise half-band", value => value >= 0);
  if (noiseBand === undefined) return false;
  const selected = await ui.showQuickPick(catalog.filter(symbol => !used.has(symbol.symbol)).map(symbol => ({
    label: symbol.symbol, description: symbol.type, symbol,
  })), { title: "Retained switching states", placeHolder: "Select the output latch, dwell timers and other states that require initialization", canPickMany: true });
  if (!selected) return false;
  if (selected.length > 32) throw new Error("The current transfer adapter supports at most 32 declared controller states.");
  const states: ControllerInitialization["states"][number][] = [];
  for (const { symbol } of selected) {
    const unit = symbol.type === "BOOL" ? "1" : await inputs.text(`${symbol.symbol}: state unit`);
    if (!unit) return false;
    const minimum = symbol.type === "BOOL" ? 0 : await inputs.number(`${symbol.symbol}: minimum`);
    if (minimum === undefined) return false;
    const maximum = symbol.type === "BOOL" ? 1 : await inputs.number(`${symbol.symbol}: maximum`, value => value >= minimum);
    if (maximum === undefined) return false;
    let initial: ControllerInitialization["states"][number]["initial"];
    if (symbol.type === "BOOL") {
      const role = await ui.showQuickPick([
        { label: "Output ON latch", value: "output" }, { label: "Initially FALSE", value: "false" }, { label: "Initially TRUE", value: "true" },
      ], { title: `${symbol.symbol}: initialization` });
      if (!role) return false;
      initial = role.value === "output" ? { source: "model-input-is", index: 0, value: loop.limits.outputMaximum } : Number(role.value === "true");
    } else {
      const value = await inputs.number(`${symbol.symbol}: initial value`, value => value >= minimum && value <= maximum);
      if (value === undefined) return false;
      initial = symbol.type === "REAL" ? Math.fround(value) : value;
      if (initial < minimum || initial > maximum) throw new Error(`Rounded initial value for ${symbol.symbol} is outside its declared bounds.`);
    }
    states.push({ symbol: symbol.symbol, iec_type: symbol.type, unit, minimum, maximum, initial, postScan: "bounded" });
  }
  loop.parameterBinding = { loop_id: loop.id, task_name: task, parameters, mode: { symbol: mode.symbol, manual_value: polarity.manual } };
  loop.controller = { ...loop.controller, design: "project", manualOutputSignal: manual.symbol, manualOutputType: manual.type,
    modeSignal: mode.symbol, modeWhenTrue: polarity.manual ? "Manual" : "Auto",
    onOff: { hysteresisSymbol: parameters[0].symbol, minimumOnTimeSymbol: parameters[1].symbol, minimumOffTimeSymbol: parameters[2].symbol,
      bandConvention: convention.value, lowOutput: loop.limits.outputMinimum, highOutput: loop.limits.outputMaximum, noiseBand,
      initialization: { kind: "declared-state-v1", states } } };
  loop.signals.manipulatedVariable.direction = "write";
  loop.experiment = { execution: "control-studio", kind: "observation", amplitude: 0,
    durationSeconds: loop.experiment?.durationSeconds ?? 10, seed: loop.experiment?.seed ?? 1 };
  return true;
}
