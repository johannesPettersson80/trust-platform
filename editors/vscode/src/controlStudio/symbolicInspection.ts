/** Read-only coherent scan inspection for addressed or symbolic Control Studio bindings. */
export interface SymbolicInspectionBinding {
  readonly id: string;
  readonly iec_type: string;
  readonly address?: string;
  readonly symbol?: string;
}

export interface SymbolicInspectionIdentity {
  readonly loopRevision: string;
  readonly expectedProgramRevision?: string;
}

export type SymbolicInspectionRequest = (
  operation: string,
  params: Record<string, unknown>
) => Promise<unknown>;

export type SymbolicInspectionQuality = "good" | "bad" | "forced" | "missing";

export interface SymbolicInspectionSample {
  readonly programRevision: string;
  readonly loopRevision: string;
  readonly bindings: readonly SymbolicInspectionBinding[];
  readonly scan: number;
  readonly runtimeTimeNs: number;
  readonly capturedAtMs: number;
  readonly values: readonly (number | null)[];
  readonly forced: readonly boolean[];
  readonly quality: readonly SymbolicInspectionQuality[];
}

export type SymbolicInspectionResult =
  | { readonly kind: "pending"; readonly programRevision: string; readonly reason: string }
  | { readonly kind: "available"; readonly sample: SymbolicInspectionSample };

/**
 * Uses `control.signals.inspect` exactly as specified by the runtime. Callers must
 * separately compare the returned scan with any independently acquired sample.
 */
export async function inspectSymbolicSignals(
  request: SymbolicInspectionRequest,
  identity: SymbolicInspectionIdentity,
  bindings: readonly SymbolicInspectionBinding[]
): Promise<SymbolicInspectionResult> {
  const requestedBindings = normalizeBindings(bindings);
  const requestedIdentity = {
    loopRevision: identity.loopRevision,
    expectedProgramRevision: identity.expectedProgramRevision,
  };
  if (!nonempty(requestedIdentity.loopRevision)) throw new Error("Invalid symbolic inspection identity.");
  if (requestedIdentity.expectedProgramRevision !== undefined && !nonempty(requestedIdentity.expectedProgramRevision)) {
    throw new Error("Invalid symbolic inspection identity.");
  }
  const response = await request("control.signals.inspect", {
    loop_revision: requestedIdentity.loopRevision,
    ...(requestedIdentity.expectedProgramRevision === undefined ? {} : { expected_program_revision: requestedIdentity.expectedProgramRevision }),
    bindings: requestedBindings,
  });
  return decodeInspection(response, requestedIdentity, requestedBindings);
}

export function isUsableSymbolicInspectionValue(sample: SymbolicInspectionSample, id: string): boolean {
  const index = sample.bindings.findIndex(binding => binding.id === id);
  if (index < 0 || sample.quality[index] !== "good" || sample.forced[index] || !Number.isFinite(sample.values[index])) return false;
  const value = sample.values[index]!;
  return sample.bindings[index].iec_type.toUpperCase() !== "BOOL" || value === 0 || value === 1;
}

function decodeInspection(
  response: unknown,
  identity: SymbolicInspectionIdentity,
  expectedBindings: readonly SymbolicInspectionBinding[]
): SymbolicInspectionResult {
  if (!record(response) || response.loop_revision !== identity.loopRevision ||
      !nonempty(response.program_revision) ||
      (identity.expectedProgramRevision !== undefined && response.program_revision !== identity.expectedProgramRevision) ||
      !sameBindings(response.bindings, expectedBindings)) {
    throw invalidResponse();
  }
  if (response.available === false) {
    if (!nonempty(response.reason)) throw invalidResponse();
    return { kind: "pending", programRevision: response.program_revision, reason: response.reason };
  }
  if (response.available !== true || !safeNonnegativeInteger(response.scan) ||
      !safeNonnegativeInteger(response.runtime_time_ns) || !safeNonnegativeInteger(response.captured_at_ms) ||
      !Array.isArray(response.values) || !Array.isArray(response.forced) || !Array.isArray(response.quality) ||
      response.values.length !== expectedBindings.length || response.forced.length !== expectedBindings.length ||
      response.quality.length !== expectedBindings.length ||
      !response.values.every(value => value === null || (typeof value === "number" && Number.isFinite(value))) ||
      !response.forced.every(value => typeof value === "boolean") ||
      !response.quality.every(isQuality)) throw invalidResponse();
  return {
    kind: "available",
    sample: {
      programRevision: response.program_revision,
      loopRevision: response.loop_revision,
      bindings: expectedBindings,
      scan: response.scan,
      runtimeTimeNs: response.runtime_time_ns,
      capturedAtMs: response.captured_at_ms,
      values: response.values as (number | null)[],
      forced: response.forced as boolean[],
      quality: response.quality as SymbolicInspectionQuality[],
    },
  };
}

function normalizeBindings(bindings: readonly SymbolicInspectionBinding[]): readonly SymbolicInspectionBinding[] {
  if (bindings.length < 1 || bindings.length > 32) throw new Error("Invalid symbolic inspection bindings.");
  const ids = new Set<string>();
  return bindings.map(binding => {
    if (!record(binding) || !nonempty(binding.id) || !nonempty(binding.iec_type) || ids.has(binding.id)) {
      throw new Error("Invalid symbolic inspection bindings.");
    }
    ids.add(binding.id);
    const address = nonempty(binding.address) ? binding.address : undefined;
    const symbol = nonempty(binding.symbol) ? binding.symbol : undefined;
    if ((address === undefined) === (symbol === undefined)) throw new Error("Invalid symbolic inspection bindings.");
    return { id: binding.id, iec_type: binding.iec_type, ...(address === undefined ? { symbol } : { address }) };
  });
}

function sameBindings(value: unknown, expected: readonly SymbolicInspectionBinding[]): boolean {
  if (!Array.isArray(value) || value.length !== expected.length) return false;
  try {
    const actual = normalizeBindings(value as SymbolicInspectionBinding[]);
    return actual.every((binding, index) => binding.id === expected[index].id && binding.iec_type === expected[index].iec_type &&
      binding.address === expected[index].address && binding.symbol === expected[index].symbol);
  } catch {
    return false;
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function nonempty(value: unknown): value is string { return typeof value === "string" && value.length > 0; }
function safeNonnegativeInteger(value: unknown): value is number { return typeof value === "number" && Number.isSafeInteger(value) && value >= 0; }
function isQuality(value: unknown): value is SymbolicInspectionQuality { return value === "good" || value === "bad" || value === "forced" || value === "missing"; }
function invalidResponse(): Error { return new Error("Invalid symbolic inspection response."); }
