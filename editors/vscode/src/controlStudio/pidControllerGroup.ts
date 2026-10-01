import type { ControlLoopDefinition } from "./loopDefinition";
import type { ProjectControllerBinding } from "./projectController";
import { loopSignal } from "./loopChannels";
import { assertLibraryPidStates } from "./libraryPidInitialization";

export interface PidControllerGroupMember {
  instance: string;
  type: "Control.PID";
  inputChannelId: string;
  outputChannelId: string;
  modeSymbol: string;
  parameterSymbols: readonly string[];
  applicationStateSymbols?: readonly string[];
}
export interface PidControllerGroup { members: readonly PidControllerGroupMember[] }

/** Group membership is declared, never inferred from the number of selected signals. */
export function validPidControllerGroup(value: unknown, channels: ControlLoopDefinition["channels"]): value is PidControllerGroup {
  if (!record(value) || Object.keys(value).some(key => key !== "members") || !Array.isArray(value.members)
    || value.members.length < 2 || value.members.length > 16 || !channels) return false;
  const members = value.members;
  if (!members.every(member => record(member)
    && Object.keys(member).every(key => ["instance", "type", "inputChannelId", "outputChannelId", "modeSymbol", "parameterSymbols", "applicationStateSymbols"].includes(key))
    && text(member.instance) && member.type === "Control.PID" && text(member.inputChannelId) && text(member.outputChannelId)
    && text(member.modeSymbol) && symbols(member.parameterSymbols) && member.parameterSymbols.length > 0
    && (member.applicationStateSymbols === undefined || symbols(member.applicationStateSymbols)))) return false;
  const declared = members as PidControllerGroupMember[];
  return new Set(declared.map(member => member.instance.toUpperCase())).size === members.length
    && members.length === channels.inputs.length && members.length === channels.outputs.length
    && channels.inputs.every(channel => declared.filter(member => member.inputChannelId === channel.modelChannelId).length === 1)
    && channels.outputs.every(channel => declared.filter(member => member.outputChannelId === channel.modelChannelId).length === 1);
}

/** Reuse the existing single scan-boundary transaction only for a complete group map. */
export function assertPidGroupAdapter(loop: ControlLoopDefinition, project: ProjectControllerBinding | undefined): void {
  const group = loop.controller.group;
  if (!group) return;
  if (!validPidControllerGroup(group, loop.channels)) throw new Error("Declare each PID group member and its input/output channels exactly once.");
  if (!project || !loop.parameterBinding) throw new Error("This PID group needs a project-owned coherent parameter and state adapter before tuning or Apply.");
  const roots = new Set([...group.members.map(member => member.instance), loop.parameterBinding.mode.symbol,
    ...loop.parameterBinding.parameters.map(parameter => parameter.symbol), ...project.initialization.states.map(state => state.symbol),
    ...loop.channels!.inputs.flatMap(input => [loopSignal(loop, input.signalId).symbol, input.manualOutputSignal ?? loop.controller.manualOutputSignal ?? ""]),
    ...loop.channels!.outputs.flatMap(output => [loopSignal(loop, output.signalId).symbol, output.setpointSignalId ? loopSignal(loop, output.setpointSignalId).symbol : ""])]
    .map(symbol => symbol.split(".")[0].toUpperCase()));
  if (roots.size !== 1) throw new Error("Coherent PID group application requires all members and transfer signals to belong to the same compiled program.");
  const parameters = new Set<string>();
  const states = new Set<string>();
  const claim = (set: Set<string>, symbol: string, kind: string) => {
    const key = symbol.toUpperCase();
    if (set.has(key)) throw new Error(`PID group ${kind} ${symbol} has more than one owner.`);
    set.add(key);
  };
  for (const member of group.members) {
    if (member.modeSymbol !== loop.parameterBinding.mode.symbol) throw new Error(`PID group member ${member.instance} must use the declared shared mode for coherent application.`);
    for (const symbol of member.parameterSymbols) claim(parameters, symbol, "parameter");
    const inputIndex = loop.channels!.inputs.findIndex(input => input.modelChannelId === member.inputChannelId);
    const outputIndex = loop.channels!.outputs.findIndex(output => output.modelChannelId === member.outputChannelId);
    const output = loop.channels!.outputs[outputIndex];
    if (!output.setpointSignalId) throw new Error(`PID group member ${member.instance} needs its declared setpoint.`);
    const ownStates = project.initialization.states.filter(state => state.symbol.startsWith(`${member.instance}.StateValue.`));
    assertLibraryPidStates(member.instance, ownStates, loopSignal(loop, output.signalId).unit,
      loopSignal(loop, loop.channels!.inputs[inputIndex].signalId).unit, inputIndex, outputIndex, project.seed?.kind === "decoupled-pid");
    const owned = new Set([...ownStates.map(state => state.symbol), ...(member.applicationStateSymbols ?? [])]);
    for (const symbol of owned) {
      if (!symbol.toUpperCase().startsWith(`${member.instance.split(".")[0]}.`.toUpperCase())) throw new Error(`PID group state ${symbol} must belong to ${member.instance}'s owning program.`);
      claim(states, symbol, "state");
    }
  }
  const complete = (actual: readonly { symbol: string }[], expected: Set<string>) => actual.length === expected.size
    && new Set(actual.map(entry => entry.symbol.toUpperCase())).size === actual.length
    && actual.every(entry => expected.has(entry.symbol.toUpperCase()));
  // A static decoupler's variables belong to the group as a whole (Specification 35 §17.15).
  if (project.seed?.kind === "decoupled-pid") for (const symbol of project.seed.matrixSymbols.flat()) claim(parameters, symbol, "parameter");
  if (!complete(loop.parameterBinding.parameters, parameters)) throw new Error("Declare the complete PID group parameter union with one owner per parameter.");
  if (!complete(project.initialization.states, states)) throw new Error("Declare the complete PID group retained and application state union with one owner per state.");
}

/** The selected group describes compiled wiring; a model edit cannot rewire a PID. */
export function assertPidGroupSignalMapping(loop: ControlLoopDefinition, mapping: NonNullable<ControlLoopDefinition["channels"]>): void {
  if (!loop.controller.group || !loop.channels) return;
  for (const kind of ["inputs", "outputs"] as const) {
    for (const original of loop.channels[kind]) {
      const next = mapping[kind].find(channel => channel.modelChannelId === original.modelChannelId);
      if (next?.signalId !== original.signalId) throw new Error(`Keep the PID group's declared signal ${loopSignal(loop, original.signalId).symbol}; select a compatible controller group for different wiring.`);
    }
  }
}

function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function text(value: unknown): value is string { return typeof value === "string" && value.trim().length > 0; }
function symbols(value: unknown): value is string[] {
  return Array.isArray(value) && value.length <= 128 && value.every(text) && new Set(value.map(symbol => symbol.toUpperCase())).size === value.length;
}
