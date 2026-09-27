import type { ControlLoopDefinition } from "./loopDefinition";
import type { ControlStudioWorkflowResult } from "./workflowResult";
import type { RuntimeVerificationResult } from "./verificationClient";
import type { IdentificationChannels } from "./identifiedStateModel";
import { loopSignal } from "./loopChannels";

export interface RecordingSignalSelection { inputs: readonly string[]; outputs: readonly string[] }
export interface RecordingSignalCatalog {
  channels: readonly { id: string; unit: string }[];
  selection: RecordingSignalSelection;
  kind: "archive" | "legacy";
}

/** Recording identities are data coordinates, never inferred runtime addresses. */
export function openedRecordingCatalog(file: Pick<ControlStudioWorkflowResult, "units">, loop: ControlLoopDefinition,
  runs: readonly RuntimeVerificationResult[], requested?: unknown): RecordingSignalCatalog {
  const channels = runs.length ? runs[0].run.channels.filter(channel => runs.every(run =>
    run.run.channels.some(other => other.id === channel.id && other.unit === channel.unit)))
    : [{ id: "appliedMv", unit: file.units.mv }, { id: "pv", unit: file.units.pv }];
  const configured = configuredRecordingSelection(loop);
  const defaults = runs.length ? {
    inputs: configured.inputs.filter(id => channels.some(channel => channel.id === id)),
    outputs: configured.outputs.filter(id => channels.some(channel => channel.id === id)),
  } : { inputs: ["appliedMv"], outputs: ["pv"] };
  const selection = requested === undefined ? defaults : decodeSelection(requested);
  if ([...selection.inputs, ...selection.outputs].some(id => !channels.some(channel => channel.id === id))) {
    throw new Error("Choose input and output signals present in this recording.");
  }
  return { kind: runs.length ? "archive" : "legacy", channels, selection };
}

export function decodeRecordingCatalog(value: unknown): RecordingSignalCatalog | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const catalog = value as Record<string, unknown>;
  if (Object.keys(catalog).some(key => !["kind", "channels", "selection"].includes(key))
    || !["archive", "legacy"].includes(String(catalog.kind)) || !Array.isArray(catalog.channels) || catalog.channels.length > 256) return undefined;
  const channels = catalog.channels as { id: string; unit: string }[];
  if (channels.some(channel => !channel || typeof channel.id !== "string" || !channel.id || channel.id.length > 1024
    || typeof channel.unit !== "string" || channel.unit.length > 64) || new Set(channels.map(channel => channel.id)).size !== channels.length) return undefined;
  try {
    const selection = decodeSelection(catalog.selection);
    if ([...selection.inputs, ...selection.outputs].some(id => !channels.some(channel => channel.id === id))) return undefined;
    return { kind: catalog.kind as RecordingSignalCatalog["kind"], channels: channels.map(({ id, unit }) => ({ id, unit })), selection };
  } catch { return undefined; }
}

export function configuredRecordingSelection(loop: ControlLoopDefinition): RecordingSignalSelection {
  return {
    inputs: loop.channels?.inputs.map(input => loopSignal(loop, input.signalId).symbol) ?? [loop.signals.manipulatedVariable.symbol],
    outputs: loop.channels?.outputs.map(output => loopSignal(loop, output.signalId).symbol) ?? [loop.signals.processVariable.symbol],
  };
}

/** Observed ranges describe imported data; they do not grant live excitation limits. */
export function selectedRecordingChannels(runs: readonly RuntimeVerificationResult[], selection: RecordingSignalSelection): IdentificationChannels {
  const channel = (id: string) => {
    const declared = runs[0].run.channels.find(channel => channel.id === id);
    if (!declared || runs.some(run => !run.run.channels.some(channel => channel.id === id && channel.unit === declared.unit))) {
      throw new Error(`Recording signal ${id} is missing or has inconsistent units.`);
    }
    let minimum = Infinity, maximum = -Infinity;
    for (const run of runs) for (const sample of run.run.samples) {
      const value = sample.values[id];
      if (value?.quality !== "good" || typeof value.value !== "number" || !Number.isFinite(value.value)) throw new Error(`Recording signal ${id} has incomplete or invalid samples.`);
      minimum = Math.min(minimum, value.value); maximum = Math.max(maximum, value.value);
    }
    if (!Number.isFinite(minimum)) throw new Error(`Recording signal ${id} has no samples.`);
    const padding = minimum === maximum ? Math.max(1e-9, Math.abs(minimum) * 1e-9) : 0;
    return { id, unit: declared.unit, operatingRange: { minimum: minimum - padding, maximum: maximum + padding } };
  };
  return { inputs: selection.inputs.map(channel), outputs: selection.outputs.map(channel) };
}

function decodeSelection(value: unknown): RecordingSignalSelection {
  const object = value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
  const ids = (items: unknown): items is string[] => Array.isArray(items) && items.length <= 16
    && items.every(id => typeof id === "string" && id.length > 0 && id.length <= 1024) && new Set(items).size === items.length;
  if (!object || Object.keys(object).some(key => !["inputs", "outputs"].includes(key)) || !ids(object.inputs) || !ids(object.outputs)
    || object.inputs.some(id => (object.outputs as string[]).includes(id))) throw new Error("Choose distinct plant input and measured output signals from the recording.");
  return { inputs: [...object.inputs], outputs: [...object.outputs] };
}
