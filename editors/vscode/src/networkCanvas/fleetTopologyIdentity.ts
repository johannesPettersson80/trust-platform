import type {
  FleetTopologyExternal,
  FleetTopologyLink,
  FleetTopologyResponse,
  FleetTopologyRuntime,
} from "./fleetTopology";

type IdentityAtom = string | null;

interface RuntimeOwner {
  readonly hostId: string;
  readonly containerId: string | null;
  readonly runtimeId: string;
}

interface RuntimeRecord {
  readonly owner: RuntimeOwner;
  readonly normalizedId: string;
}

interface EndpointRecord {
  readonly owner: RuntimeOwner;
  readonly normalizedId: string;
}

interface ExternalRecord {
  readonly external: FleetTopologyExternal;
  readonly rawId: string;
  readonly currentId: string;
  normalizedId: string;
  omitted: boolean;
}

type Resolution<T> =
  | { readonly kind: "missing" }
  | { readonly kind: "ambiguous" }
  | { readonly kind: "unique"; readonly value: T };

const IDENTITY_PREFIX = "trust-fleet-v1:";

/**
 * Clone and normalize the JSON fleet contract at the display boundary.
 *
 * Wire identifiers are meaningful only inside their owning host/container/runtime.
 * This projection gives render-facing objects reversible, collision-free tuple IDs
 * while retaining global shared/external IDs and every non-identity product value.
 */
export function normalizeFleetTopologySnapshot(
  response: FleetTopologyResponse
): FleetTopologyResponse {
  const topology = deepClone(response);
  const runtimesByReference = new Map<string, Map<string, RuntimeRecord>>();
  const endpointsByReference = new Map<string, Map<string, EndpointRecord>>();

  for (const host of topology.hosts ?? []) {
    const hostId = host.host_id;
    for (const runtime of host.runtimes ?? []) {
      normalizeRuntime(
        runtime,
        { hostId, containerId: null, runtimeId: rawRuntimeId(runtime.runtime_id) },
        runtimesByReference,
        endpointsByReference
      );
    }
    for (const container of host.containers ?? []) {
      const rawContainerId = rawContainerIdOf(container.container_id);
      container.container_id = encodeIdentity(["container", hostId, rawContainerId]);
      for (const runtime of container.runtimes ?? []) {
        normalizeRuntime(
          runtime,
          {
            hostId,
            containerId: rawContainerId,
            runtimeId: rawRuntimeId(runtime.runtime_id),
          },
          runtimesByReference,
          endpointsByReference
        );
      }
    }
  }

  const externalRecords = (topology.external ?? []).map((external) => ({
    external,
    rawId: rawExternalId(external.id),
    currentId: external.id,
    normalizedId: rawExternalId(external.id),
    omitted: false,
  }));

  normalizeConfiguredMeshExternals(
    externalRecords,
    topology.links ?? [],
    endpointsByReference
  );
  topology.external = externalRecords
    .filter((record) => !record.omitted)
    .map((record) => {
      record.external.id = record.normalizedId;
      return record.external;
    });

  const externalsByReference = externalReferenceMap(externalRecords);
  const sharedIds = new Set((topology.shared ?? []).map((shared) => shared.id));
  topology.links = (topology.links ?? []).flatMap((link) => {
    const normalized = normalizeLink(
      link,
      endpointsByReference,
      runtimesByReference,
      externalsByReference,
      sharedIds
    );
    return normalized ? [normalized] : [];
  });

  for (const shared of topology.shared ?? []) {
    const normalizedRuntimeIds = new Set<string>();
    for (const runtimeReference of shared.used_by ?? []) {
      const resolution = resolveReference(runtimesByReference, runtimeReference);
      if (resolution.kind === "unique") {
        normalizedRuntimeIds.add(resolution.value.normalizedId);
      }
    }
    shared.used_by = [...normalizedRuntimeIds];
  }

  return topology;
}

function normalizeRuntime(
  runtime: FleetTopologyRuntime,
  owner: RuntimeOwner,
  runtimesByReference: Map<string, Map<string, RuntimeRecord>>,
  endpointsByReference: Map<string, Map<string, EndpointRecord>>
): void {
  const currentRuntimeId = runtime.runtime_id;
  const normalizedRuntimeId = encodeRuntimeId(owner);
  const runtimeRecord = { owner, normalizedId: normalizedRuntimeId };
  addReference(runtimesByReference, owner.runtimeId, ownerKey(owner), runtimeRecord);
  addReference(runtimesByReference, currentRuntimeId, ownerKey(owner), runtimeRecord);
  addReference(runtimesByReference, normalizedRuntimeId, ownerKey(owner), runtimeRecord);
  runtime.runtime_id = normalizedRuntimeId;

  for (const endpoint of runtime.endpoints ?? []) {
    const currentEndpointId = endpoint.id;
    const rawEndpointId = rawEndpointIdOf(endpoint.id);
    const normalizedEndpointId = encodeIdentity([
      "endpoint",
      owner.hostId,
      owner.containerId,
      owner.runtimeId,
      rawEndpointId,
    ]);
    const endpointRecord = { owner, normalizedId: normalizedEndpointId };
    const endpointOwnerKey = `${ownerKey(owner)}\u0000${rawEndpointId}`;
    addReference(
      endpointsByReference,
      rawEndpointId,
      endpointOwnerKey,
      endpointRecord
    );
    addReference(
      endpointsByReference,
      currentEndpointId,
      endpointOwnerKey,
      endpointRecord
    );
    addReference(
      endpointsByReference,
      normalizedEndpointId,
      endpointOwnerKey,
      endpointRecord
    );
    endpoint.id = normalizedEndpointId;
  }
}

function normalizeConfiguredMeshExternals(
  records: ExternalRecord[],
  links: readonly FleetTopologyLink[],
  endpointsByReference: Map<string, Map<string, EndpointRecord>>
): void {
  for (const record of records) {
    if (!(record.external.via_protocol ?? []).includes("mesh")) {
      continue;
    }
    const owners = new Map<string, RuntimeOwner>();
    let ambiguousOwner = false;
    for (const link of links) {
      if (link.protocol !== "mesh") {
        continue;
      }
      const otherReferences: string[] = [];
      if (referencesExternal(link.from, record)) {
        otherReferences.push(link.to);
      }
      if (referencesExternal(link.to, record)) {
        otherReferences.push(link.from);
      }
      for (const otherReference of otherReferences) {
        const endpoint = resolveReference(endpointsByReference, otherReference);
        if (endpoint.kind === "ambiguous") {
          ambiguousOwner = true;
        } else if (endpoint.kind === "unique") {
          owners.set(ownerKey(endpoint.value.owner), endpoint.value.owner);
        }
      }
    }
    if (ambiguousOwner || owners.size > 1) {
      record.omitted = true;
      continue;
    }
    const owner = owners.values().next().value as RuntimeOwner | undefined;
    if (owner) {
      record.normalizedId = encodeIdentity([
        "mesh-external",
        owner.hostId,
        owner.containerId,
        owner.runtimeId,
        record.rawId,
      ]);
    }
  }
}

function normalizeLink(
  link: FleetTopologyLink,
  endpointsByReference: Map<string, Map<string, EndpointRecord>>,
  runtimesByReference: Map<string, Map<string, RuntimeRecord>>,
  externalsByReference: Map<string, Map<string, ExternalRecord>>,
  sharedIds: ReadonlySet<string>
): FleetTopologyLink | undefined {
  const wire = rawLinkIdentity(link);
  const from = resolveLinkEndpoint(
    link.from,
    endpointsByReference,
    runtimesByReference,
    externalsByReference,
    sharedIds
  );
  const to = resolveLinkEndpoint(
    link.to,
    endpointsByReference,
    runtimesByReference,
    externalsByReference,
    sharedIds
  );
  if (from.kind !== "unique" || to.kind !== "unique") {
    return undefined;
  }
  link.from = from.value;
  link.to = to.value;
  link.id = encodeIdentity([
    "link",
    wire.id,
    wire.from,
    wire.to,
    wire.protocol,
    wire.role,
    wire.direction,
    link.same_host ? "same-host" : "remote",
    link.from,
    link.to,
  ]);
  return link;
}

function resolveLinkEndpoint(
  reference: string,
  endpointsByReference: Map<string, Map<string, EndpointRecord>>,
  runtimesByReference: Map<string, Map<string, RuntimeRecord>>,
  externalsByReference: Map<string, Map<string, ExternalRecord>>,
  sharedIds: ReadonlySet<string>
): Resolution<string> {
  const endpoint = resolveReference(endpointsByReference, reference);
  if (endpoint.kind === "ambiguous") {
    return endpoint;
  }
  if (endpoint.kind === "unique") {
    return { kind: "unique", value: endpoint.value.normalizedId };
  }

  const runtime = resolveReference(runtimesByReference, reference);
  if (runtime.kind === "ambiguous") {
    return runtime;
  }
  if (runtime.kind === "unique") {
    return { kind: "unique", value: runtime.value.normalizedId };
  }

  const external = resolveReference(externalsByReference, reference);
  if (external.kind === "ambiguous") {
    return external;
  }
  if (external.kind === "unique" && !external.value.omitted) {
    return { kind: "unique", value: external.value.normalizedId };
  }

  const rawReference = rawIdentityReference(reference);
  if (sharedIds.has(reference)) {
    return { kind: "unique", value: reference };
  }
  if (sharedIds.has(rawReference)) {
    return { kind: "unique", value: rawReference };
  }
  return { kind: "missing" };
}

function externalReferenceMap(
  records: readonly ExternalRecord[]
): Map<string, Map<string, ExternalRecord>> {
  const result = new Map<string, Map<string, ExternalRecord>>();
  for (const record of records) {
    if (record.omitted) {
      continue;
    }
    addReference(result, record.rawId, record.normalizedId, record);
    addReference(result, record.currentId, record.normalizedId, record);
    addReference(result, record.normalizedId, record.normalizedId, record);
  }
  return result;
}

function referencesExternal(reference: string, record: ExternalRecord): boolean {
  if (reference === record.currentId) {
    return true;
  }
  if (decodeIdentity(record.currentId)?.[0] === "mesh-external") {
    return false;
  }
  return rawIdentityReference(reference) === record.rawId;
}

function resolveReference<T>(
  references: Map<string, Map<string, T>>,
  reference: string
): Resolution<T> {
  const direct = references.get(reference);
  const candidates = direct ?? references.get(rawIdentityReference(reference));
  if (!candidates || candidates.size === 0) {
    return { kind: "missing" };
  }
  if (candidates.size !== 1) {
    return { kind: "ambiguous" };
  }
  return { kind: "unique", value: candidates.values().next().value as T };
}

function addReference<T>(
  references: Map<string, Map<string, T>>,
  reference: string,
  identity: string,
  value: T
): void {
  const candidates = references.get(reference) ?? new Map<string, T>();
  candidates.set(identity, value);
  references.set(reference, candidates);
}

function rawLinkIdentity(link: FleetTopologyLink): {
  readonly id: string | null;
  readonly from: string;
  readonly to: string;
  readonly protocol: string;
  readonly role: string | null;
  readonly direction: string;
} {
  const decoded = decodeIdentity(link.id);
  if (
    decoded?.[0] === "link" &&
    (typeof decoded[1] === "string" || decoded[1] === null) &&
    typeof decoded[2] === "string" &&
    typeof decoded[3] === "string" &&
    typeof decoded[4] === "string" &&
    (typeof decoded[5] === "string" || decoded[5] === null) &&
    typeof decoded[6] === "string"
  ) {
    return {
      id: decoded[1],
      from: decoded[2],
      to: decoded[3],
      protocol: decoded[4],
      role: decoded[5],
      direction: decoded[6],
    };
  }
  return {
    id: link.id ?? null,
    from: rawIdentityReference(link.from),
    to: rawIdentityReference(link.to),
    protocol: link.protocol,
    role: link.role ?? null,
    direction: link.direction,
  };
}

function rawContainerIdOf(value: string): string {
  return rawIdentityPart(value, "container", 2);
}

export function rawRuntimeId(value: string): string {
  return rawIdentityPart(value, "runtime", 3);
}

function rawEndpointIdOf(value: string): string {
  return rawIdentityPart(value, "endpoint", 4);
}

function rawExternalId(value: string): string {
  return rawIdentityPart(value, "mesh-external", 4);
}

function rawIdentityReference(value: string): string {
  const decoded = decodeIdentity(value);
  if (!decoded) {
    return value;
  }
  switch (decoded[0]) {
    case "runtime":
      return typeof decoded[3] === "string" ? decoded[3] : value;
    case "endpoint":
    case "mesh-external":
      return typeof decoded[4] === "string" ? decoded[4] : value;
    default:
      return value;
  }
}

function rawIdentityPart(value: string, kind: string, index: number): string {
  const decoded = decodeIdentity(value);
  return decoded?.[0] === kind && typeof decoded[index] === "string"
    ? decoded[index]
    : value;
}

function encodeRuntimeId(owner: RuntimeOwner): string {
  return encodeIdentity([
    "runtime",
    owner.hostId,
    owner.containerId,
    owner.runtimeId,
  ]);
}

function ownerKey(owner: RuntimeOwner): string {
  return JSON.stringify([owner.hostId, owner.containerId, owner.runtimeId]);
}

function encodeIdentity(parts: readonly IdentityAtom[]): string {
  const encoded = Buffer.from(JSON.stringify(parts), "utf8")
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
  return `${IDENTITY_PREFIX}${encoded}`;
}

function decodeIdentity(value: string | undefined): IdentityAtom[] | undefined {
  if (!value?.startsWith(IDENTITY_PREFIX)) {
    return undefined;
  }
  try {
    const encoded = value.slice(IDENTITY_PREFIX.length);
    if (!/^[A-Za-z0-9_-]+$/.test(encoded)) {
      return undefined;
    }
    const base64 = encoded.replace(/-/g, "+").replace(/_/g, "/");
    const decoded = JSON.parse(
      Buffer.from(base64, "base64").toString("utf8")
    ) as unknown;
    if (
      !Array.isArray(decoded) ||
      decoded.some((part) => typeof part !== "string" && part !== null)
    ) {
      return undefined;
    }
    return decoded as IdentityAtom[];
  } catch {
    return undefined;
  }
}

function deepClone<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => deepClone(item)) as unknown as T;
  }
  if (value !== null && typeof value === "object") {
    const clone: Record<string, unknown> = {};
    for (const [key, nested] of Object.entries(value as Record<string, unknown>)) {
      clone[key] = deepClone(nested);
    }
    return clone as unknown as T;
  }
  return value;
}
