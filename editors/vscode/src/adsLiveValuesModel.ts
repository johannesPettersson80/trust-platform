export const ADS_LIVE_VALUES_SCHEMA_VERSION = 1;

export type AdsLiveValueAccess = "read" | "write" | "read_write";
export type AdsLiveValueQualityState = "good" | "stale" | "error";

export interface AdsLiveValueQuality {
  readonly state: AdsLiveValueQualityState;
  readonly lastUpdateMs?: number;
  readonly detail?: string;
}

export interface AdsLiveValueEntry {
  readonly connection: string;
  readonly name: string;
  readonly remoteSymbol: string;
  readonly value: string;
  readonly valueType: string;
  readonly access: AdsLiveValueAccess;
  readonly quality: AdsLiveValueQuality;
}

export type AdsLiveValuesProblemKind =
  | "incompatible_schema"
  | "invalid_snapshot"
  | "invalid_entries";

export interface AdsLiveValuesProblem {
  readonly kind: AdsLiveValuesProblemKind;
  readonly message: string;
  readonly detail: string;
}

export interface AdsLiveValuesState {
  readonly schemaVersion: typeof ADS_LIVE_VALUES_SCHEMA_VERSION;
  readonly scan: number;
  readonly entries: readonly AdsLiveValueEntry[];
  readonly problem?: AdsLiveValuesProblem;
}

export const EMPTY_ADS_LIVE_VALUES_STATE: AdsLiveValuesState = {
  schemaVersion: ADS_LIVE_VALUES_SCHEMA_VERSION,
  scan: 0,
  entries: [],
};

export function normalizeAdsLiveValuesState(
  value: unknown,
): AdsLiveValuesState {
  if (!isRecord(value)) {
    return problemState(
      "invalid_snapshot",
      "ADS values are unavailable.",
      "The runtime returned ADS data this extension could not safely read.",
    );
  }
  if (value.schemaVersion !== ADS_LIVE_VALUES_SCHEMA_VERSION) {
    const received =
      typeof value.schemaVersion === "number" ||
      typeof value.schemaVersion === "string"
        ? String(value.schemaVersion)
        : "missing";
    return problemState(
      "incompatible_schema",
      "ADS values are unavailable.",
      `The runtime sent ADS schema ${received}; this extension supports schema ${ADS_LIVE_VALUES_SCHEMA_VERSION}. Update truST so the runtime and extension match.`,
    );
  }
  const scan = nonNegativeInteger(value.scan);
  const rawEntries = Array.isArray(value.entries) ? value.entries : undefined;
  if (!rawEntries) {
    return problemState(
      "invalid_snapshot",
      "ADS values are unavailable.",
      "The runtime returned an invalid ADS entries list. Reconnect or update truST before relying on these values.",
      scan,
    );
  }
  const normalized = rawEntries.map(normalizeEntry);
  const entries = normalized.filter(
    (entry): entry is AdsLiveValueEntry => entry !== undefined,
  );
  const invalidCount = normalized.length - entries.length;
  const problem = adsEntryProblem(invalidCount, scan === undefined);
  return {
    schemaVersion: ADS_LIVE_VALUES_SCHEMA_VERSION,
    scan: scan ?? 0,
    entries,
    ...(problem ? { problem } : {}),
  };
}

function adsEntryProblem(
  invalidCount: number,
  invalidScan: boolean,
): AdsLiveValuesProblem | undefined {
  if (invalidCount === 0 && !invalidScan) {
    return undefined;
  }
  const entryDetail =
    invalidCount > 0
      ? `${invalidCount} ADS ${invalidCount === 1 ? "entry did" : "entries did"} not match the safe Live Values contract.`
      : "";
  const scanDetail = invalidScan
    ? "The runtime scan number was also invalid."
    : "";
  return {
    kind: invalidCount > 0 ? "invalid_entries" : "invalid_snapshot",
    message:
      invalidCount > 0
        ? "Some ADS variables could not be shown."
        : "ADS values may be incomplete.",
    detail: `${entryDetail} ${scanDetail} Reconnect or update truST before relying on omitted values.`.trim(),
  };
}

function problemState(
  kind: AdsLiveValuesProblemKind,
  message: string,
  detail: string,
  scan = 0,
): AdsLiveValuesState {
  return {
    schemaVersion: ADS_LIVE_VALUES_SCHEMA_VERSION,
    scan,
    entries: [],
    problem: { kind, message, detail },
  };
}

function normalizeEntry(value: unknown): AdsLiveValueEntry | undefined {
  if (!isRecord(value) || !isRecord(value.quality)) {
    return undefined;
  }
  const connection = nonEmptyString(value.connection);
  const name = nonEmptyString(value.name);
  const remoteSymbol = nonEmptyString(value.remoteSymbol);
  const valueType = nonEmptyString(value.valueType);
  const access = normalizeAccess(value.access);
  const qualityState = normalizeQualityState(value.quality.state);
  if (
    !connection ||
    !name ||
    !remoteSymbol ||
    typeof value.value !== "string" ||
    !valueType ||
    !access ||
    !qualityState
  ) {
    return undefined;
  }
  const lastUpdateMs = nonNegativeInteger(value.quality.lastUpdateMs);
  const detail = nonEmptyString(value.quality.detail);
  return {
    connection,
    name,
    remoteSymbol,
    value: value.value,
    valueType: valueType.toUpperCase(),
    access,
    quality: {
      state: qualityState,
      ...(lastUpdateMs === undefined ? {} : { lastUpdateMs }),
      ...(detail ? { detail } : {}),
    },
  };
}

function normalizeAccess(value: unknown): AdsLiveValueAccess | undefined {
  return value === "read" || value === "write" || value === "read_write"
    ? value
    : undefined;
}

function normalizeQualityState(
  value: unknown,
): AdsLiveValueQualityState | undefined {
  return value === "good" || value === "stale" || value === "error"
    ? value
    : undefined;
}

function nonEmptyString(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const text = value.trim();
  return text || undefined;
}

function nonNegativeInteger(value: unknown): number | undefined {
  return typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= 0
    ? value
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
