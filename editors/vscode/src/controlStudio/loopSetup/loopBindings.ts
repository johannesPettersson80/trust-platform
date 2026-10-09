import { createHash } from "crypto";
import type { ControlLoopDefinition, ControlSignalBinding } from "../loopDefinition";
import type { CompiledCatalog, CompiledSymbol } from "./compiledCatalog";
import { MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS } from "../controlStudioLimits";

export interface MeasuredSignal { symbol: string; unit: string; minimum: number; maximum: number }

export function compiled(catalog: CompiledCatalog, symbol: string): CompiledSymbol {
  const entry = catalog.symbols.find(candidate => candidate.symbol === symbol);
  if (!entry) throw new Error(`${symbol} is not a compiled symbol of this project.`);
  return entry;
}

export function realType(entry: CompiledSymbol): "REAL" | "LREAL" {
  if (entry.type !== "REAL" && entry.type !== "LREAL") throw new Error(`${entry.symbol} must be REAL or LREAL.`);
  return entry.type;
}

/** A loop signal: addressed, or a program variable that has no address (Specification 35 §17.1). */
export function binding(catalog: CompiledCatalog, signal: MeasuredSignal, direction: "read" | "write"): ControlSignalBinding {
  const entry = compiled(catalog, signal.symbol);
  return { symbol: entry.symbol, ...(entry.address ? { address: entry.address } : {}), iecType: realType(entry), direction, unit: signal.unit,
    range: { minimum: signal.minimum, maximum: signal.maximum } };
}

/**
 * A measured output, which validation's simulated plant writes (Specification 33 §13.2): an input, or a program variable
 * the program only reads. A variable the program computes itself would overwrite the simulated measurement every scan.
 */
export function measurementBinding(catalog: CompiledCatalog, signal: MeasuredSignal): ControlSignalBinding {
  const entry = compiled(catalog, signal.symbol);
  if (!entry.address && catalog.writes?.some(written => written.toUpperCase() === entry.symbol.toUpperCase())) {
    throw new Error(`${entry.symbol} is computed by the program, so a simulated measurement would be overwritten every scan. `
      + "Choose the measurement it is computed from.");
  }
  return binding(catalog, signal, "read");
}

export function loopId(name: string): string {
  return `loop:${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "loop"}`;
}

export function withRevision(loop: ControlLoopDefinition): ControlLoopDefinition {
  return { ...loop, revision: `sha256:${createHash("sha256").update(JSON.stringify({ ...loop, revision: undefined })).digest("hex")}` };
}

/** The engineer's identification duration, within what Studio's experiment limits accept (A7: 1 s to 24 h). */
/**
 * The experiment a setup writes (B1.3). Setup writes one only when the engineer set both its output step and its
 * identification duration; otherwise the block is left out. Identify then chooses the step and its hold within the
 * limits (B3.3, B3.4), and Limits asks for the maximum duration when it is missing (A7).
 */
export function explicitExperiment(name: string, kind: { execution: "control-studio" | "project-scripted"; kind: "mv_step" | "observation" },
  step: number | undefined, seconds: number | undefined): { experiment?: ControlLoopDefinition["experiment"] } {
  if (step === undefined && seconds === undefined) return {};
  if (step === undefined || seconds === undefined) {
    throw new Error(`${name} needs both the output step and the identification duration, or neither: Identify chooses the one that is left.`);
  }
  return { experiment: { ...kind, amplitude: step, durationSeconds: identificationDuration(name, seconds), seed: 1 } };
}

export function identificationDuration(name: string, seconds: number | undefined): number {
  if (seconds === undefined || !Number.isFinite(seconds) || seconds < MINIMUM_EXPERIMENT_DURATION_MS / 1_000 || seconds > MAXIMUM_EXPERIMENT_DURATION_MS / 1_000) {
    throw new Error(`${name} needs an identification duration between ${MINIMUM_EXPERIMENT_DURATION_MS / 1_000} s and ${MAXIMUM_EXPERIMENT_DURATION_MS / 3_600_000} h.`);
  }
  return seconds;
}
