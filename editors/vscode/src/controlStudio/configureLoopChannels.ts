import type { SetupDialogs } from "./setupPromptTypes";
import * as vscode from "vscode";
import type { ControlLoopDefinition, ControlAuxiliarySignalBinding } from "./loopDefinition";
import { validateLoopChannelBindings, type ActuatorCapabilities, type LoopChannels } from "./loopChannels";

interface CompiledSignal { symbol: string; address: string; type: string }
/** Extends the same setup object; compiled symbols remain the source of signal identity. */
export async function configureLoopChannels(loop: ControlLoopDefinition, catalog: readonly CompiledSignal[], controlSymbols: readonly { symbol: string; type: string }[] = [], ui: SetupDialogs = vscode.window): Promise<boolean> {
  const mode = await ui.showQuickPick([
    { label: "Single channel", description: "Use the selected PV, MV and setpoint", multi: false },
    { label: "Multiple channels", description: "Bind ordered model inputs, outputs and per-channel requirements", multi: true },
  ], { title: "Control channels" });
  if (!mode) return false;
  const actuator = await actuatorSetup(ui, loop.limits.outputMinimum, loop.limits.outputMaximum, !!loop.controller.onOff || loop.signals.manipulatedVariable.iecType === "BOOL");
  if (!actuator) return false;
  const inputs: LoopChannels["inputs"][number][] = [{ modelChannelId: loop.signals.manipulatedVariable.symbol,
    signalId: "manipulatedVariable", actuator }];
  const outputs: LoopChannels["outputs"][number][] = [{ modelChannelId: loop.signals.processVariable.symbol,
    signalId: "processVariable", ...(loop.signals.setpoint ? { setpointSignalId: "setpoint" } : {}) }];
  const additional: ControlAuxiliarySignalBinding[] = [...(loop.signals.additional ?? [])];
  if (mode.multi) {
    const claimed = new Set([loop.signals.processVariable.symbol, loop.signals.manipulatedVariable.symbol, loop.signals.setpoint?.symbol]);
    const selected = await ui.showQuickPick(catalog.filter(signal => !claimed.has(signal.symbol)).map(signal => ({
      label: signal.symbol, description: `${signal.address} · ${signal.type}`, signal,
    })), { title: "Additional model signals", canPickMany: true });
    if (!selected) return false;
    for (const { signal } of selected) {
      const role = await ui.showQuickPick(["MV", "PV", "SP"] as const, { title: `${signal.symbol}: signal role` });
      if (role !== "MV" && role !== "PV" && role !== "SP") return false;
      const channel = await ui.showInputBox({ title: `${signal.symbol}: channel name`, validateInput: value => value.trim() ? undefined : "Enter a channel name." });
      if (!channel) return false;
      const unit = await ui.showInputBox({ title: `${signal.symbol}: physical unit`, validateInput: value => value.trim() ? undefined : "Enter a unit." });
      if (!unit) return false;
      const minimum = await numeric(ui, `${signal.symbol}: minimum`); if (minimum === undefined) return false;
      const maximum = await numeric(ui, `${signal.symbol}: maximum`, value => value > minimum); if (maximum === undefined) return false;
      const id = `signal:${signal.symbol}`;
      additional.push({ id, channel, role, symbol: signal.symbol, address: signal.address, iecType: signal.type,
        direction: role === "MV" && loop.experiment?.execution === "control-studio" ? "write" : "read", unit, range: { minimum, maximum } });
      if (role === "MV") {
        const capability = await actuatorSetup(ui, minimum, maximum); if (!capability) return false;
        inputs.push({ modelChannelId: signal.symbol, signalId: id, actuator: capability });
      } else if (role === "PV") outputs.push({ modelChannelId: signal.symbol, signalId: id });
    }
    for (const output of outputs.slice(1)) {
      const signal = additional.find(signal => signal.id === output.signalId)!;
      const references = additional.filter(reference => reference.role === "SP" && reference.channel === signal.channel && reference.unit === signal.unit);
      const selectedReference = await ui.showQuickPick([
        { label: "Auxiliary measurement", id: undefined as string | undefined },
        ...references.map(reference => ({ label: reference.symbol, id: reference.id })),
      ], { title: `${signal.symbol}: controlled objective` });
      if (!selectedReference) return false;
      if (selectedReference.id) {
        output.setpointSignalId = selectedReference.id;
        const referenceChange = await numeric(ui, `${signal.symbol}: target change`); if (referenceChange === undefined) return false;
        const maximumError = await numeric(ui, `${signal.symbol}: maximum error`, value => value > 0); if (maximumError === undefined) return false;
        const settlingBand = await numeric(ui, `${signal.symbol}: settling allowance`, value => value > 0 && value < maximumError); if (settlingBand === undefined) return false;
        const settlingTimeSeconds = await numeric(ui, `${signal.symbol}: response deadline (s)`, value => value > 0); if (settlingTimeSeconds === undefined) return false;
        output.requirements = { referenceChange, maximumError, settlingBand, settlingTimeSeconds };
      }
    }
  }
  if (inputs.length > 1 && loop.parameterBinding) {
    const used = new Set([loop.controller.manualOutputSignal, loop.parameterBinding.mode.symbol,
      ...catalog.map(signal => signal.symbol), ...loop.parameterBinding.parameters.map(parameter => parameter.symbol)]);
    for (const input of inputs.slice(1)) {
      const selected = await ui.showQuickPick([
        { label: "No Manual command", description: "Use closed-loop acquisition; Auto transfer needs a Manual command", signal: undefined as { symbol: string; type: string } | undefined },
        ...controlSymbols.filter(signal => !used.has(signal.symbol) && ["REAL", "LREAL", "BOOL"].includes(signal.type))
          .map(signal => ({ label: signal.symbol, description: signal.type, signal })),
      ], { title: `${input.modelChannelId}: Manual command` });
      if (!selected) return false;
      if (selected.signal) {
        input.manualOutputSignal = selected.signal.symbol;
        input.manualOutputType = selected.signal.type as "REAL" | "LREAL" | "BOOL";
        used.add(selected.signal.symbol);
      }
    }
  }
  const feedbackUsed = new Set([loop.signals.processVariable.symbol, loop.signals.manipulatedVariable.symbol, loop.signals.setpoint?.symbol,
    ...additional.map(signal => signal.symbol)]);
  for (const [index, input] of inputs.entries()) {
    const commandSignal = index === 0 ? loop.signals.manipulatedVariable : additional.find(signal => signal.id === input.signalId)!;
    const selected = await ui.showQuickPick([
      { label: "Use the controller output", description: "No separate applied-action feedback is declared", signal: undefined as CompiledSignal | undefined },
      ...catalog.filter(signal => signal.address.startsWith("%I") && !feedbackUsed.has(signal.symbol) && ["REAL", "LREAL", "BOOL"].includes(signal.type))
        .map(signal => ({ label: signal.symbol, description: `${signal.type} · applied action in ${commandSignal.unit}`, signal })),
    ], { title: `${input.modelChannelId}: applied actuator feedback` });
    if (!selected) return false;
    if (!selected.signal) continue;
    const binding = { symbol: selected.signal.symbol, address: selected.signal.address, iecType: selected.signal.type,
      direction: "read" as const, unit: commandSignal.unit, range: { minimum: input.actuator.minimum, maximum: input.actuator.maximum } };
    if (index === 0) loop.signals.actuator = binding;
    else {
      const id = `applied:${selected.signal.symbol}`;
      additional.push({ ...binding, id, channel: input.modelChannelId, role: "actuator" });
      input.appliedSignalId = id;
    }
    feedbackUsed.add(selected.signal.symbol);
  }
  loop.signals.additional = additional;
  loop.channels = { inputs, outputs };
  loop.limits.rateUpPerS = actuator.rateUpPerS;
  loop.limits.rateDownPerS = actuator.rateDownPerS;
  const physicalChoices: Array<{ label: string; physicalKind?: "thermal-balance" | "gravity-tank" | "gravity-tank-cascade" }> = [{ label: "Automatic dynamic model" }];
  if (loop.signals.manipulatedVariable.unit === "W" && ["K", "degC", "°C"].includes(loop.signals.processVariable.unit)) physicalChoices.push({ label: "Thermal energy balance", physicalKind: "thermal-balance" });
  if (["m3/s", "m^3/s", "m³/s"].includes(loop.signals.manipulatedVariable.unit) && loop.signals.processVariable.unit === "m") {
    physicalChoices.push({ label: "Gravity-tank volume balance", physicalKind: "gravity-tank" });
    if (outputs.length === 2) physicalChoices.push({ label: "Equal-area gravity-tank cascade", physicalKind: "gravity-tank-cascade" });
  }
  if (physicalChoices.length > 1) {
    const physical = await ui.showQuickPick(physicalChoices, { title: "Identification model structure", placeHolder: "Choose a physical balance only when its equation describes the plant" });
    if (!physical) return false;
    loop.physicalIdentification = physical.physicalKind ? { kind: physical.physicalKind } : undefined;
  }
  validateLoopChannelBindings(loop);
  return true;
}
async function actuatorSetup(ui: SetupDialogs, minimum: number, maximum: number, switching = false): Promise<ActuatorCapabilities | undefined> {
  const kind = switching ? { capability: "on-off" as const } : await ui.showQuickPick([
    { label: "Continuous output", capability: "continuous" as const },
    { label: "ON/OFF output", capability: "on-off" as const },
  ], { title: "Actuator capability" });
  if (!kind) return undefined;
  const result: ActuatorCapabilities = { kind: kind.capability, minimum, maximum };
  const keys = kind.capability === "continuous" ? ["rateUpPerS", "rateDownPerS"] as const : ["minimumOnSeconds", "minimumOffSeconds"] as const;
  for (const key of keys) {
    const titles = { rateUpPerS: "Maximum rising output rate (output units/s)", rateDownPerS: "Maximum falling output rate (output units/s)",
      minimumOnSeconds: "Minimum ON dwell (s)", minimumOffSeconds: "Minimum OFF dwell (s)" };
    const raw = await ui.showInputBox({ title: titles[key], prompt: "Leave blank when no limit is specified.",
      validateInput: value => !value.trim() || Number.isFinite(Number(value)) && Number(value) >= 0 ? undefined : "Enter a nonnegative finite value." });
    if (raw === undefined) return undefined;
    if (raw.trim()) result[key] = Number(raw);
  }
  return result;
}
async function numeric(ui: SetupDialogs, title: string, valid: (value: number) => boolean = () => true): Promise<number | undefined> {
  const raw = await ui.showInputBox({ title, validateInput: text => text.trim() && Number.isFinite(Number(text)) && valid(Number(text))
    ? undefined : "Enter a finite value within the declared range." });
  return raw === undefined ? undefined : Number(raw);
}
