/**
 * The PLC signals a plant's ports can bind to, as Rust's `plant.signals` answers them (specification
 * 47 §8): compiled numeric commands and feedbacks with their current values, the enabled physical
 * I/O drivers, or why none is offered. Rust also judges every binding (`plant.edit`).
 */

export interface PlantSignalCandidate {
  readonly symbol: string;
  readonly address: string;
  readonly dataType: string;
  readonly currentValue?: string;
}

export interface PlantSignalCatalog {
  readonly status: "ready" | "compile_error";
  readonly commands: readonly PlantSignalCandidate[];
  readonly feedbacks: readonly PlantSignalCandidate[];
  readonly physicalDrivers?: readonly string[];
  readonly error?: string;
}

export type PlantSignalDirection = "command" | "feedback";

/** The catalog to show while Rust's answer cannot be had, with the reason. */
export function unavailablePlantSignals(error: string): PlantSignalCatalog {
  return { status: "compile_error", commands: [], feedbacks: [], error };
}
