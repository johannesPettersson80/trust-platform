const DERIVED_TOPOLOGY_PARAM_KEYS = new Set(["clients_count"]);

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function globalName(path: string): string {
  return path.replace(/^global\./, "");
}

export function buildExposeApplyParams(
  current: Record<string, unknown>,
  paths: string[],
  allowWrites: boolean
): { names: string[]; params: Record<string, unknown> } {
  const names = paths.map(globalName);
  const params: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(current)) {
    if (key.endsWith("_set") || DERIVED_TOPOLOGY_PARAM_KEYS.has(key)) {
      continue;
    }
    params[key] = value;
  }

  params.expose = Array.from(new Set([...stringArray(current.expose), ...names]));
  if (allowWrites && "writable" in current) {
    params.writable = Array.from(new Set([...stringArray(current.writable), ...names]));
  }

  return { names, params };
}
