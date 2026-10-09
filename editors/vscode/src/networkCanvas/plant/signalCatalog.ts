import type { IoState } from "../../io-panel/types";

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

export type ManualPlantSignalValidation =
  | { readonly ok: true; readonly address: string }
  | {
      readonly ok: false;
      readonly code:
        "catalog_unavailable" | "not_compiled" | "wrong_direction" | "duplicate_feedback" | "physical_io_enabled";
      readonly message: string;
    };

export interface PlantSignalValidationContext {
  readonly usedFeedbackTargets?: readonly string[];
}

export function enabledPhysicalIoDrivers(schema: unknown, topology?: unknown): string[] {
  const drivers = new Set<string>();
  collectSchemaPhysicalDrivers(schema, drivers);
  collectTopologyPhysicalDrivers(topology, drivers);
  return [...drivers];
}

function collectSchemaPhysicalDrivers(schema: unknown, drivers: Set<string>): void {
  if (!schema || typeof schema !== "object") return;
  const protocols = (schema as { protocols?: unknown }).protocols;
  if (!Array.isArray(protocols)) return;
  const nonphysical = new Set(["simulated", "loopback"]);
  for (const value of protocols) {
    if (!value || typeof value !== "object") continue;
    const protocol = value as Record<string, unknown>;
    const id = stringField(protocol.id).toLowerCase();
    if (protocol.category !== "field_device" || nonphysical.has(id)) continue;
    const instances = Array.isArray(protocol.instances) ? protocol.instances : [];
    const enabled = instances.some(
      (instance) =>
        !!instance && typeof instance === "object" && (instance as Record<string, unknown>).enabled !== false,
    );
    if (!enabled) continue;
    const title = stringField(protocol.title);
    drivers.add(title || id || "physical I/O");
  }
}

// The canvas already owns a merged offline/live topology. Use it as the source of
// configured device truth when a running runtime returns a schema without instances.
function collectTopologyPhysicalDrivers(topology: unknown, drivers: Set<string>): void {
  if (!topology || typeof topology !== "object") return;
  const hosts = (topology as { hosts?: unknown }).hosts;
  if (!Array.isArray(hosts)) return;
  for (const hostValue of hosts) {
    if (!hostValue || typeof hostValue !== "object") continue;
    const host = hostValue as Record<string, unknown>;
    const runtimes = [
      ...(Array.isArray(host.runtimes) ? host.runtimes : []),
      ...(Array.isArray(host.containers)
        ? host.containers.flatMap((container) =>
            container && typeof container === "object" && Array.isArray((container as Record<string, unknown>).runtimes)
              ? ((container as Record<string, unknown>).runtimes as unknown[])
              : [],
          )
        : []),
    ];
    for (const runtimeValue of runtimes) {
      if (!runtimeValue || typeof runtimeValue !== "object") continue;
      const endpoints = (runtimeValue as Record<string, unknown>).endpoints;
      if (!Array.isArray(endpoints)) continue;
      for (const endpointValue of endpoints) {
        if (!endpointValue || typeof endpointValue !== "object") continue;
        const endpoint = endpointValue as Record<string, unknown>;
        const protocol = stringField(endpoint.protocol).toLowerCase();
        const physical = endpoint.category === "field_device" || endpoint.kind === "field";
        if (
          !physical ||
          protocol === "simulated" ||
          protocol === "loopback" ||
          stringField(endpoint.health).toLowerCase() === "disabled"
        ) {
          continue;
        }
        drivers.add(stringField(endpoint.display_name) || stringField(endpoint.name) || protocol || "physical I/O");
      }
    }
  }
}

export function validateManualPlantSignalAddress(
  catalog: PlantSignalCatalog,
  direction: PlantSignalDirection,
  address: string,
  context: PlantSignalValidationContext = {},
): ManualPlantSignalValidation {
  if (catalog.status !== "ready") {
    return {
      ok: false,
      code: "catalog_unavailable",
      message: "Compile the project successfully before entering a PLC signal.",
    };
  }
  const candidates = direction === "command" ? catalog.commands : catalog.feedbacks;
  const opposite = direction === "command" ? catalog.feedbacks : catalog.commands;
  const requested = canonicalAddress(address);
  const candidate = candidates.find((entry) => canonicalAddress(entry.address) === requested);
  if (!candidate) {
    if (opposite.some((entry) => canonicalAddress(entry.address) === requested)) {
      return {
        ok: false,
        code: "wrong_direction",
        message:
          direction === "command"
            ? "Use a compiled numeric %Q output or %M memory value for a command."
            : "Use a compiled numeric %I input for feedback.",
      };
    }
    return {
      ok: false,
      code: "not_compiled",
      message:
        direction === "command"
          ? "Enter or choose a compiled numeric %Q output or %M memory value."
          : "Enter or choose a compiled numeric %I input.",
    };
  }
  if (
    direction === "feedback" &&
    context.usedFeedbackTargets?.some((used) => canonicalAddress(used) === canonicalAddress(candidate.address))
  ) {
    return {
      ok: false,
      code: "duplicate_feedback",
      message: `Choose a different %I input; ${candidate.address} is already used by another plant.`,
    };
  }
  if (catalog.physicalDrivers?.length) {
    return {
      ok: false,
      code: "physical_io_enabled",
      message: `${candidate.address} is owned by enabled physical I/O (${catalog.physicalDrivers.join(", ")}), so simulated ${direction === "command" ? "command" : "feedback"} cannot ${direction === "command" ? "read" : "write"} it. Disable ${catalog.physicalDrivers.join(", ")} in Devices & Connections or choose another compiled %${direction === "command" ? "Q output" : "I input"}.`,
    };
  }
  return { ok: true, address: candidate.address };
}

/** Builds the plant signal catalog from the `trust-lsp.hmiBindings` response. */
export function buildPlantSignalCatalog(
  compiled: unknown,
  live: IoState,
  communicationSchema?: unknown,
  communicationTopology?: unknown,
): PlantSignalCatalog {
  if (!compiled || typeof compiled !== "object") {
    return compileError("Compiled signal metadata is unavailable.");
  }
  const response = compiled as Record<string, unknown>;
  if (response.ok !== true) {
    return compileError(
      typeof response.error === "string" && response.error.trim()
        ? response.error
        : "The current project did not compile.",
    );
  }
  const processImage =
    response.processImage && typeof response.processImage === "object"
      ? (response.processImage as Record<string, unknown>)
      : undefined;

  // A command reads a PLC output or a memory value, such as a disturbance the program or the HMI
  // sets (specification 33, §13.1).
  const catalog: PlantSignalCatalog = {
    status: "ready",
    commands: [
      ...normalizeCandidates(processImage?.commands, live.outputs),
      ...normalizeCandidates(processImage?.memory, live.memory ?? []),
    ],
    feedbacks: normalizeCandidates(processImage?.feedbacks, live.inputs),
  };
  const physicalDrivers = enabledPhysicalIoDrivers(communicationSchema, communicationTopology);
  return physicalDrivers.length ? { ...catalog, physicalDrivers } : catalog;
}

function compileError(error: string): PlantSignalCatalog {
  return { status: "compile_error", commands: [], feedbacks: [], error };
}

function normalizeCandidates(value: unknown, liveEntries: IoState["inputs"]): PlantSignalCandidate[] {
  if (!Array.isArray(value)) return [];
  const liveByAddress = new Map(liveEntries.map((entry) => [canonicalAddress(entry.address), entry] as const));
  return value.flatMap((candidate): PlantSignalCandidate[] => {
    if (!candidate || typeof candidate !== "object") return [];
    const record = candidate as Record<string, unknown>;
    const symbol = stringField(record.symbol);
    const address = stringField(record.address);
    const dataType = stringField(record.dataType || record.type).toUpperCase();
    if (!symbol || !address || !dataType) return [];
    const result: PlantSignalCandidate = { symbol, address, dataType };
    const liveEntry = liveByAddress.get(canonicalAddress(address));
    if (liveEntry && isCurrentValue(liveEntry.value)) {
      return [{ ...result, currentValue: liveEntry.value }];
    }
    return [result];
  });
}

function stringField(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function canonicalAddress(address: string): string {
  return address.trim().toUpperCase();
}

function isCurrentValue(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return normalized !== "" && normalized !== "unresolved" && !normalized.startsWith("error:");
}
