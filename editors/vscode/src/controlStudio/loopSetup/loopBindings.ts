import { createHash } from "crypto";
import type { ControlLoopDefinition, ControlSignalBinding } from "../loopDefinition";
import type { CompiledCatalog, CompiledSymbol } from "./compiledCatalog";

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

/** The engineer's identification duration, within what Studio's experiment limits accept (2–600 s). */
export function identificationDuration(name: string, seconds: number | undefined): number {
  if (seconds === undefined || !Number.isFinite(seconds) || seconds < 2 || seconds > 600) {
    throw new Error(`${name} needs an identification duration between 2 and 600 s.`);
  }
  return seconds;
}
