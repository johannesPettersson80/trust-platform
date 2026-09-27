import * as fs from "fs";
import * as path from "path";

import { controllerAdapter } from "./controllerAdapters";
import type { PidControllerGroup } from "./pidControllerGroup";
import {
  decodeControlLoopDefinition,
  validateControlLoopDefinition,
  type ControlLoopDefinition,
  type ControlSignalBinding,
} from "./loopDefinition";
import { controlLoopConfigurationPaths } from "./projectLoop";

const MAX_LOOP_FILES = 128;
const MAX_LOOP_BYTES = 512 * 1024;
const MAX_CATALOG_ENTRIES = 16_384;

/** The process-image and control-symbol payload returned by the compiled-project command. */
export interface CompiledControlCatalog {
  ok?: boolean;
  error?: string;
  processImage?: {
    commands?: readonly unknown[];
    feedbacks?: readonly unknown[];
    memory?: readonly unknown[];
  };
  controlSymbols?: {
    controllers?: readonly unknown[];
    symbols?: readonly unknown[];
    error?: string;
  };
}

export interface CompiledControlSignal {
  symbol: string;
  address: string;
  type: string;
}

export interface CompiledControllerInstance {
  instance: string;
  type: string;
  tasks: readonly { name: string; intervalMs: number }[];
  group?: PidControllerGroup;
}

export interface SelectableControlSignal {
  id: string;
  symbol: string;
  address: string;
  iecType: string;
  unit: string;
  bounds: { minimum: number; maximum: number };
  channel?: string;
  role: string;
  order: number;
}

export type ControllerAdapterCompatibility =
  | { kind: "unconfigured" }
  | { kind: "accepted"; families: readonly string[] }
  | { kind: "rejected"; reason: string };

/** One explicit loop definition that is safe to present to the three-page selectors. */
export interface ControlStudioBindingTarget {
  configurationPath: string;
  loopDefinition: ControlLoopDefinition;
  controller: CompiledControllerInstance;
  inputs: readonly SelectableControlSignal[];
  outputs: readonly SelectableControlSignal[];
  controllerCompatibility: ControllerAdapterCompatibility;
}

export interface InvalidControlStudioBinding {
  configurationPath: string;
  reason: string;
}

export interface ControlStudioBindingDiscovery {
  targets: readonly ControlStudioBindingTarget[];
  rejected: readonly InvalidControlStudioBinding[];
}

export interface ControlStudioBindingSelection {
  loopId: string;
  controllerInstance: string;
  inputIds: readonly string[];
  outputIds: readonly string[];
}

export type ControlStudioBindingSelectionResult =
  | {
      kind: "accepted";
      configurationPath: string;
      loopDefinition: ControlLoopDefinition;
      controller: CompiledControllerInstance;
      inputs: readonly SelectableControlSignal[];
      outputs: readonly SelectableControlSignal[];
    }
  | { kind: "rejected"; reason: string };

/**
 * Reads only explicit loop definitions and joins them to the compiled catalog.
 * No ST symbols are inferred, and this function never writes the project or a
 * running Control Studio session.
 */
export async function discoverControlStudioBindings(
  projectRoot: string,
  runtimeId: string,
  catalog: unknown,
): Promise<ControlStudioBindingDiscovery> {
  const rejected: InvalidControlStudioBinding[] = [];
  const targets: ControlStudioBindingTarget[] = [];
  const compiled = normalizeCatalog(catalog);
  if (isRejected(compiled)) {
    return { targets, rejected: [{ configurationPath: projectRoot, reason: compiled.reason }] };
  }

  let configurationPaths: string[];
  try {
    configurationPaths = await controlLoopConfigurationPaths(projectRoot);
  } catch {
    return {
      targets,
      rejected: [{ configurationPath: path.join(projectRoot, "control-loops"), reason: "The control-loops directory could not be read." }],
    };
  }
  if (configurationPaths.length > MAX_LOOP_FILES) {
    return { targets, rejected: [{ configurationPath: projectRoot, reason: `The project declares more than ${MAX_LOOP_FILES} loop definitions.` }] };
  }
  const loopPathsById = new Map<string, string[]>();
  for (const configurationPath of configurationPaths) {
    let value: unknown;
    try {
      const stat = await fs.promises.stat(configurationPath);
      if (stat.size > MAX_LOOP_BYTES) {
        rejected.push({ configurationPath, reason: `${path.basename(configurationPath)} exceeds the ${MAX_LOOP_BYTES} byte loop-definition bound.` });
        continue;
      }
      value = JSON.parse(await fs.promises.readFile(configurationPath, "utf8"));
    } catch (error) {
      if (isMissingFile(error)) continue;
      rejected.push({ configurationPath, reason: `${path.basename(configurationPath)} could not be read as valid JSON.` });
      continue;
    }
    const decoded = decodeControlLoopDefinition(value);
    if (decoded.kind !== "accepted") {
      rejected.push({ configurationPath, reason: `${path.basename(configurationPath)} does not contain a complete Control Studio loop definition.` });
      continue;
    }
    const loop = decoded.loop;
    const pathsForId = loopPathsById.get(loop.id) ?? [];
    pathsForId.push(configurationPath);
    loopPathsById.set(loop.id, pathsForId);
    if (loop.runtimeId !== runtimeId) {
      rejected.push({ configurationPath, reason: `The configured loop uses ${loop.runtimeId}, but ${runtimeId} is the selected target.` });
      continue;
    }
    const validation = validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId });
    if (validation.kind !== "accepted") {
      rejected.push({ configurationPath, reason: formatValidationFailure(validation.reason) });
      continue;
    }
    const target = targetForLoop(configurationPath, loop, compiled);
    if (isRejected(target)) rejected.push({ configurationPath, reason: target.reason });
    else targets.push(target);
  }
  const duplicateIds = new Set([...loopPathsById.entries()].filter(([, paths]) => paths.length > 1).map(([id]) => id));
  for (const id of duplicateIds) {
    targets.splice(0, targets.length, ...targets.filter(target => target.loopDefinition.id !== id));
    for (const configurationPath of loopPathsById.get(id) ?? []) {
      rejected.push({ configurationPath, reason: `Duplicate control loop ID ${id}; the target is ambiguous.` });
    }
  }
  return { targets, rejected };
}

/** Validates exact IDs supplied by a selector without changing the selected target. */
export function validateControlStudioBindingSelection(
  discovery: ControlStudioBindingDiscovery,
  selection: ControlStudioBindingSelection,
): ControlStudioBindingSelectionResult {
  const target = discovery.targets.find(candidate => candidate.loopDefinition.id === selection.loopId);
  if (!target) return { kind: "rejected", reason: `No discovered loop has ID ${selection.loopId}.` };
  if (selection.controllerInstance !== target.controller.instance) {
    return { kind: "rejected", reason: `Controller ${selection.controllerInstance} is not the declared compiled target ${target.controller.instance}.` };
  }
  const inputs = selectedInDeclaredOrder(target.inputs, selection.inputIds, "input");
  if (inputs.kind === "rejected") return inputs;
  const outputs = selectedInDeclaredOrder(target.outputs, selection.outputIds, "output");
  if (outputs.kind === "rejected") return outputs;
  return {
    kind: "accepted",
    configurationPath: target.configurationPath,
    loopDefinition: cloneLoop(target.loopDefinition),
    controller: clone(target.controller),
    inputs: inputs.values,
    outputs: outputs.values,
  };
}

/** Resolves an exact selector choice and returns an isolated snapshot for reopening. */
export function resolveControlStudioBinding(
  discovery: ControlStudioBindingDiscovery,
  selection: ControlStudioBindingSelection,
): ControlStudioBindingSelectionResult {
  return validateControlStudioBindingSelection(discovery, selection);
}

function targetForLoop(
  configurationPath: string,
  loop: ControlLoopDefinition,
  compiled: NormalizedCatalog,
): ControlStudioBindingTarget | { kind: "rejected"; reason: string } {
  const controller = compiled.controllers.find(candidate => candidate.instance === loop.controller.instance);
  if (!controller) return { kind: "rejected", reason: `Compiled controller ${loop.controller.instance} is unavailable.` };
  if (controller.type !== loop.controller.type) {
    return { kind: "rejected", reason: `Compiled controller ${loop.controller.instance} has type ${controller.type}, expected ${loop.controller.type}.` };
  }
  if (controller.tasks.length > 0 && !controller.tasks.some(task => Math.abs(task.intervalMs - loop.controller.taskPeriodMs) < 1e-6)) {
    return { kind: "rejected", reason: `Compiled controller ${loop.controller.instance} is not bound to the declared ${loop.controller.taskPeriodMs} ms task.` };
  }
  for (const member of loop.controller.group?.members ?? []) {
    const actual = compiled.controllers.find(candidate => candidate.instance === member.instance);
    if (!actual || actual.type !== member.type || !actual.tasks.some(task => task.name === loop.parameterBinding?.task_name
      && Math.abs(task.intervalMs - loop.controller.taskPeriodMs) < 1e-6)) {
      return { kind: "rejected", reason: `Compiled PID group member ${member.instance} is unavailable in the declared task.` };
    }
  }
  for (const signal of allDeclaredSignals(loop)) {
    const compiledSignal = compiled.signals.get(signal.symbol);
    if (!compiledSignal || compiledSignal.address !== signal.address || compiledSignal.type.toUpperCase() !== signal.iecType.toUpperCase()) {
      return { kind: "rejected", reason: `Declared signal ${signal.symbol} does not match the compiled process-image identity.` };
    }
  }
  for (const symbol of declaredControllerSymbols(loop)) {
    const compiledSymbol = compiled.symbols.get(symbol);
    if (!compiledSymbol) return { kind: "rejected", reason: `Declared controller symbol ${symbol} is unavailable in the compiled catalog.` };
  }
  for (const [symbol, expectedType] of declaredControllerSymbolTypes(loop)) {
    const compiledSymbol = compiled.symbols.get(symbol);
    if (compiledSymbol && compiledSymbol.type.toUpperCase() !== expectedType.toUpperCase()) {
      return { kind: "rejected", reason: `Compiled controller symbol ${symbol} has type ${compiledSymbol.type}, expected ${expectedType}.` };
    }
  }
  const inputs = selectableSignals(loop, "input");
  const outputs = selectableSignals(loop, "output");
  if (!inputs.length || !outputs.length) return { kind: "rejected", reason: "The loop must declare at least one input and one output channel." };
  const controllerCompatibility = adapterCompatibility(loop);
  return {
    configurationPath,
    loopDefinition: cloneLoop(loop),
    controller: { ...clone(controller), ...(loop.controller.group ? { group: clone(loop.controller.group) } : {}) },
    inputs,
    outputs,
    controllerCompatibility,
  };
}

function selectableSignals(loop: ControlLoopDefinition, direction: "input" | "output"): SelectableControlSignal[] {
  const result: SelectableControlSignal[] = [];
  const additional = loop.signals.additional ?? [];
  if (loop.channels) {
    if (direction === "input") loop.channels.inputs.forEach((channel, order) => {
      const signal = loopSignal(loop, channel.signalId);
      result.push({ id: channel.signalId, symbol: signal.symbol, address: signal.address, iecType: signal.iecType,
        unit: signal.unit, bounds: { minimum: channel.actuator.minimum, maximum: channel.actuator.maximum }, channel: channel.modelChannelId,
        role: "MV", order });
    });
    else loop.channels.outputs.forEach((channel, order) => {
      const signal = loopSignal(loop, channel.signalId);
      result.push({ id: channel.signalId, symbol: signal.symbol, address: signal.address, iecType: signal.iecType,
        unit: signal.unit, bounds: { ...signal.range }, channel: channel.modelChannelId,
        role: signalRole(loop, channel.signalId), order });
    });
    return result;
  }
  const primary = direction === "input" ? { id: "manipulatedVariable", signal: loop.signals.manipulatedVariable, role: "MV" }
    : { id: "processVariable", signal: loop.signals.processVariable, role: "PV" };
  result.push({ id: primary.id, symbol: primary.signal.symbol, address: primary.signal.address, iecType: primary.signal.iecType,
    unit: primary.signal.unit, bounds: { ...primary.signal.range }, channel: undefined, role: primary.role, order: 0 });
  additional.filter(signal => direction === "input" ? signal.role === "MV" : signal.role === "PV").forEach((signal, index) => {
    result.push({ id: signal.id, symbol: signal.symbol, address: signal.address, iecType: signal.iecType,
      unit: signal.unit, bounds: { ...signal.range }, channel: signal.channel, role: signal.role, order: index + 1 });
  });
  return result;
}

function selectedInDeclaredOrder(
  available: readonly SelectableControlSignal[],
  requested: readonly string[],
  label: string,
): { kind: "accepted"; values: readonly SelectableControlSignal[] } | { kind: "rejected"; reason: string } {
  if (!Array.isArray(requested) || requested.length === 0) return { kind: "rejected", reason: `Choose at least one ${label} channel.` };
  if (requested.length !== available.length) {
    return { kind: "rejected", reason: `Select every declared ${label} channel before opening this loop.` };
  }
  const ids = new Set<string>();
  for (const id of requested) {
    if (typeof id !== "string" || ids.has(id) || !available.some(signal => signal.id === id)) {
      return { kind: "rejected", reason: `The selected ${label} channel identity is not declared by the loop.` };
    }
    ids.add(id);
  }
  return { kind: "accepted", values: available.filter(signal => ids.has(signal.id)).map(clone) };
}

function allDeclaredSignals(loop: ControlLoopDefinition): ControlSignalBinding[] {
  return [loop.signals.setpoint, loop.signals.processVariable, loop.signals.manipulatedVariable, loop.signals.actuator,
    ...(loop.signals.additional ?? [])].filter((signal): signal is ControlSignalBinding => signal !== undefined);
}

function loopSignal(loop: ControlLoopDefinition, id: string): ControlSignalBinding {
  if (id === "setpoint") return loop.signals.setpoint ?? missingSignal(id);
  if (id === "processVariable") return loop.signals.processVariable;
  if (id === "manipulatedVariable") return loop.signals.manipulatedVariable;
  if (id === "actuator") return loop.signals.actuator ?? missingSignal(id);
  const signal = loop.signals.additional?.find(candidate => candidate.id === id);
  return signal ?? missingSignal(id);
}

function signalRole(loop: ControlLoopDefinition, id: string): string {
  if (id === "setpoint") return "SP";
  if (id === "processVariable") return "PV";
  if (id === "manipulatedVariable") return "MV";
  if (id === "actuator") return "actuator";
  return loop.signals.additional?.find(signal => signal.id === id)?.role ?? "unknown";
}

function missingSignal(id: string): never { throw new Error(`Channel ${id} does not identify a declared signal.`); }

function declaredControllerSymbols(loop: ControlLoopDefinition): readonly string[] {
  const symbols = new Set<string>();
  const add = (value: unknown) => { if (typeof value === "string" && value.includes(".") && !value.startsWith("%")) symbols.add(value); };
  const walk = (value: unknown, key = "") => {
    if (Array.isArray(value)) { value.forEach(item => walk(item, key)); return; }
    if (!value || typeof value !== "object") {
      if (!["parameterRecordSymbol", "parameterRevisionSymbol"].includes(key) && (/symbol(s)?$/i.test(key) || /Signal$/.test(key))) add(value);
      return;
    }
    for (const [childKey, child] of Object.entries(value)) walk(child, childKey);
  };
  walk(loop.parameterBinding);
  walk(loop.controller);
  return [...symbols];
}

function declaredControllerSymbolTypes(loop: ControlLoopDefinition): ReadonlyMap<string, string> {
  const types = new Map<string, string>();
  for (const parameter of loop.parameterBinding?.parameters ?? []) types.set(parameter.symbol, parameter.iec_type);
  if (loop.parameterBinding) types.set(loop.parameterBinding.mode.symbol, "BOOL");
  if (loop.controller.manualOutputSignal && loop.controller.manualOutputType) types.set(loop.controller.manualOutputSignal, loop.controller.manualOutputType);
  const walk = (value: unknown) => {
    if (Array.isArray(value)) { value.forEach(walk); return; }
    if (!value || typeof value !== "object") return;
    const record = value as Record<string, unknown>;
    if (typeof record.symbol === "string" && typeof record.iecType === "string") types.set(record.symbol, record.iecType);
    if (typeof record.symbol === "string" && typeof record.iec_type === "string") types.set(record.symbol, record.iec_type);
    Object.values(record).forEach(walk);
  };
  walk(loop.controller.pidInitialization);
  walk(loop.controller.stateFeedback);
  walk(loop.controller.onOff);
  return types;
}

function adapterCompatibility(loop: ControlLoopDefinition): ControllerAdapterCompatibility {
  if (!loop.parameterBinding && !loop.controller.law && !loop.controller.project && !loop.controller.onOff && !loop.controller.stateFeedback) return { kind: "unconfigured" };
  try { return { kind: "accepted", families: [...controllerAdapter(loop).families] }; }
  catch (error) { return { kind: "rejected", reason: error instanceof Error ? error.message : String(error) }; }
}

type NormalizedCatalog = { signals: Map<string, CompiledControlSignal>; symbols: Map<string, { symbol: string; type: string }>; controllers: CompiledControllerInstance[] };

function normalizeCatalog(value: unknown): NormalizedCatalog | { kind: "rejected"; reason: string } {
  if (!value || typeof value !== "object" || Array.isArray(value)) return { kind: "rejected", reason: "Compiled control catalog is unavailable." };
  const catalog = value as CompiledControlCatalog;
  if (catalog.ok !== undefined && catalog.ok !== true) return { kind: "rejected", reason: catalog.error ?? "Compiled control catalog is unavailable." };
  const signalLists = [catalog.processImage?.commands ?? [], catalog.processImage?.feedbacks ?? [], catalog.processImage?.memory ?? []];
  if (signalLists.some(list => list.length > MAX_CATALOG_ENTRIES)) return { kind: "rejected", reason: "Compiled process-image catalog exceeds its bounded entry limit." };
  if ((catalog.controlSymbols?.symbols?.length ?? 0) > MAX_CATALOG_ENTRIES || (catalog.controlSymbols?.controllers?.length ?? 0) > MAX_CATALOG_ENTRIES) {
    return { kind: "rejected", reason: "Compiled control-symbol catalog exceeds its bounded entry limit." };
  }
  const signalEntries = signalLists.flat().filter(isCompiledSignal);
  const signals = new Map<string, CompiledControlSignal>();
  for (const signal of signalEntries) {
    const previous = signals.get(signal.symbol);
    if (previous && (previous.address !== signal.address || previous.type.toUpperCase() !== signal.type.toUpperCase())) {
      return { kind: "rejected", reason: `Compiled signal ${signal.symbol} has conflicting identities.` };
    }
    signals.set(signal.symbol, signal);
  }
  const symbols = new Map<string, { symbol: string; type: string }>();
  for (const value of catalog.controlSymbols?.symbols ?? []) {
    if (!isCompiledSymbol(value)) continue;
    const previous = symbols.get(value.symbol);
    if (previous && previous.type.toUpperCase() !== value.type.toUpperCase()) return { kind: "rejected", reason: `Compiled symbol ${value.symbol} has conflicting types.` };
    symbols.set(value.symbol, value);
  }
  const controllers: CompiledControllerInstance[] = [];
  for (const value of catalog.controlSymbols?.controllers ?? []) {
    if (!isCompiledController(value)) continue;
    const previous = controllers.find(candidate => candidate.instance === value.instance);
    if (previous && previous.type !== value.type) return { kind: "rejected", reason: `Compiled controller ${value.instance} has conflicting types.` };
    if (!previous) controllers.push(value);
  }
  return { signals, symbols, controllers };
}

function isCompiledSignal(value: unknown): value is CompiledControlSignal {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as Partial<CompiledControlSignal>;
  return typeof candidate.symbol === "string" && candidate.symbol.length > 0 && typeof candidate.address === "string"
    && candidate.address.length > 0 && typeof candidate.type === "string" && candidate.type.length > 0;
}
function isCompiledSymbol(value: unknown): value is { symbol: string; type: string } {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as { symbol?: unknown; type?: unknown };
  return typeof candidate.symbol === "string" && candidate.symbol.length > 0 && typeof candidate.type === "string" && candidate.type.length > 0;
}
function isCompiledController(value: unknown): value is CompiledControllerInstance {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as Partial<CompiledControllerInstance>;
  return typeof candidate.instance === "string" && candidate.instance.length > 0 && typeof candidate.type === "string" && candidate.type.length > 0
    && Array.isArray(candidate.tasks) && candidate.tasks.every(task => !!task && typeof task.name === "string" && Number.isFinite(task.intervalMs));
}

function formatValidationFailure(reason: string): string { return `Loop definition is invalid: ${reason.replace(/_/g, " ")}.`; }
function isMissingFile(error: unknown): boolean { return error instanceof Error && "code" in error && error.code === "ENOENT"; }
function isRejected(value: unknown): value is { kind: "rejected"; reason: string } {
  return !!value && typeof value === "object" && (value as { kind?: unknown }).kind === "rejected"
    && typeof (value as { reason?: unknown }).reason === "string";
}
function clone<T>(value: T): T { return structuredClone(value); }
function cloneLoop(loop: ControlLoopDefinition): ControlLoopDefinition { return clone(loop); }
