// Responsibility: focused LM tools module with a single concern.
import * as crypto from "crypto";
import * as path from "path";
import * as vscode from "vscode";
import {
  type HmiBindingCatalog,
  type HmiBindingCatalogEntry,
  type HmiLayoutFileEntry,
  type HmiLayoutSnapshot,
  type HmiLockEntry,
  type HmiPlanIntentParams,
  type HmiSchemaResult,
  type HmiSchemaWidget,
  type HmiValidationCheck,
  type HmiValuesResult,
} from "../types";
import { resolveWorkspaceFolder } from "../workspace";

export function hmiSeverityRank(severity: HmiValidationCheck["severity"]): number {
  if (severity === "error") {
    return 0;
  }
  if (severity === "warning") {
    return 1;
  }
  return 2;
}

export function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

export function parseHmiSchemaPayload(value: unknown): HmiSchemaResult | undefined {
  const record = asRecord(value);
  if (!record) {
    return undefined;
  }
  const widgetValues = Array.isArray(record.widgets) ? record.widgets : [];
  const pageValues = Array.isArray(record.pages) ? record.pages : [];

  const widgets: HmiSchemaWidget[] = [];
  for (const item of widgetValues) {
    const widget = asRecord(item);
    if (!widget || typeof widget.id !== "string") {
      continue;
    }
    widgets.push({
      id: widget.id,
      path: typeof widget.path === "string" ? widget.path : "",
      label: typeof widget.label === "string" ? widget.label : widget.id,
      data_type: typeof widget.data_type === "string" ? widget.data_type : "UNKNOWN",
      writable: widget.writable === true,
      page: typeof widget.page === "string" ? widget.page : "overview",
      group: typeof widget.group === "string" ? widget.group : "General",
    });
  }

  const pages: HmiSchemaResult["pages"] = [];
  for (const item of pageValues) {
    const page = asRecord(item);
    if (!page || typeof page.id !== "string") {
      continue;
    }
    const sectionsRaw = Array.isArray(page.sections) ? page.sections : [];
    const sections = sectionsRaw
      .map((entry) => {
        const section = asRecord(entry);
        if (!section || typeof section.title !== "string") {
          return undefined;
        }
        const widgetIds = Array.isArray(section.widget_ids)
          ? section.widget_ids
              .filter((id): id is string => typeof id === "string")
              .sort((left, right) => left.localeCompare(right))
          : [];
        const normalized: { title: string; span: number; widget_ids?: string[] } = {
          title: section.title,
          span:
            typeof section.span === "number" && Number.isFinite(section.span)
              ? section.span
              : 12,
        };
        if (widgetIds.length > 0) {
          normalized.widget_ids = widgetIds;
        }
        return normalized;
      })
      .filter(
        (entry): entry is { title: string; span: number; widget_ids?: string[] } =>
          !!entry,
      );
    pages.push({
      id: page.id,
      title: typeof page.title === "string" ? page.title : page.id,
      order:
        typeof page.order === "number" && Number.isFinite(page.order)
          ? page.order
          : 0,
      kind: typeof page.kind === "string" ? page.kind : undefined,
      view: typeof page.view === "string" ? page.view : undefined,
      bind3d: parseHmiSceneBindings(page.bind3d),
      sections: sections.length > 0 ? sections : undefined,
    });
  }

  return {
    version:
      typeof record.version === "number" && Number.isFinite(record.version)
        ? record.version
        : 1,
    mode: typeof record.mode === "string" ? record.mode : "read_only",
    read_only: record.read_only !== false,
    pages: pages.sort((left, right) =>
      left.order === right.order
        ? left.id.localeCompare(right.id)
        : left.order - right.order,
    ),
    widgets: widgets.sort((left, right) => left.id.localeCompare(right.id)),
  };
}

function parseHmiSceneBindings(value: unknown): HmiSchemaResult["pages"][number]["bind3d"] {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const bindings = value
    .map((entry) => {
      const binding = asRecord(entry);
      if (
        !binding ||
        typeof binding.node !== "string" ||
        typeof binding.property !== "string" ||
        typeof binding.source !== "string"
      ) {
        return undefined;
      }
      return {
        node: binding.node,
        property: binding.property,
        source: binding.source,
      };
    })
    .filter(
      (entry): entry is { node: string; property: string; source: string } => !!entry,
    );
  return bindings.length > 0 ? bindings : undefined;
}

export function parseHmiValuesPayload(value: unknown): HmiValuesResult | undefined {
  const record = asRecord(value);
  if (!record) {
    return undefined;
  }
  const rawValues = asRecord(record.values) ?? {};
  const values: HmiValuesResult["values"] = {};
  for (const [widgetId, rawEntry] of Object.entries(rawValues)) {
    const entry = asRecord(rawEntry);
    if (!entry) {
      continue;
    }
    const quality = typeof entry.q === "string" ? entry.q : "unknown";
    const ts =
      typeof entry.ts_ms === "number" && Number.isFinite(entry.ts_ms)
        ? entry.ts_ms
        : Date.now();
    values[widgetId] = {
      v: entry.v,
      q: quality,
      ts_ms: ts,
    };
  }
  return {
    connected: record.connected !== false,
    timestamp_ms:
      typeof record.timestamp_ms === "number" && Number.isFinite(record.timestamp_ms)
        ? record.timestamp_ms
        : Date.now(),
    values,
  };
}

export function coerceInt(
  value: unknown,
  fallback: number,
  minimum: number,
  maximum: number,
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(minimum, Math.min(maximum, Math.trunc(value)));
}

export async function sleepWithCancellation(
  durationMs: number,
  token: vscode.CancellationToken,
): Promise<boolean> {
  if (token.isCancellationRequested) {
    return false;
  }
  return await new Promise<boolean>((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) {
        return;
      }
      settled = true;
      disposable.dispose();
      resolve(true);
    }, Math.max(0, durationMs));
    const disposable = token.onCancellationRequested(() => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve(false);
    });
  });
}

export function normalizeEvidenceRunId(value: string | undefined): string | undefined {
  if (!value) {
    return undefined;
  }
  const trimmed = value.trim();
  return /^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z$/.test(trimmed)
    ? trimmed
    : undefined;
}

export function layoutDescriptorPages(files: HmiLayoutFileEntry[]): HmiLayoutFileEntry[] {
  return files.filter(
    (file) =>
      file.name !== "_config.toml" &&
      file.name !== "_intent.toml" &&
      !file.name.startsWith("_"),
  );
}

export function parseQuotedArrayFromToml(content: string, key: string): string[] {
  const match = content.match(
    new RegExp(`^\\s*${key}\\s*=\\s*\\[(.*)\\]\\s*$`, "m"),
  );
  if (!match || typeof match[1] !== "string") {
    return [];
  }
  const values: string[] = [];
  for (const quoted of match[1].matchAll(/"([^"]+)"/g)) {
    const item = (quoted[1] ?? "").trim();
    if (item) {
      values.push(item);
    }
  }
  return normalizeStringList(values);
}

export function normalizeErrorCode(value: string | undefined): string | undefined {
  if (!value) {
    return undefined;
  }
  const normalized = value.trim().toUpperCase().replace(/[^A-Z0-9_]/g, "_");
  return normalized.length > 0 ? normalized : undefined;
}

export function extractErrorCode(message: string): string | undefined {
  const match = message.match(/^\s*([A-Z0-9_]{3,})\s*:/);
  if (!match || typeof match[1] !== "string") {
    return undefined;
  }
  return normalizeErrorCode(match[1]);
}

export function errorCodeMatches(
  expected: string | undefined,
  code: string | undefined,
  detail: string,
): boolean {
  const normalizedExpected = normalizeErrorCode(expected);
  if (!normalizedExpected) {
    return false;
  }
  const normalizedCode = normalizeErrorCode(code);
  if (normalizedCode && normalizedCode === normalizedExpected) {
    return true;
  }
  return detail.toUpperCase().includes(normalizedExpected);
}

export async function readHmiLayoutSnapshot(
  rootPath: string | undefined,
  token: vscode.CancellationToken,
): Promise<{ snapshot?: HmiLayoutSnapshot; error?: string }> {
  const resolved = resolveWorkspaceFolder(rootPath);
  if (resolved.error || !resolved.folder) {
    return { error: resolved.error ?? "Unable to resolve workspace folder." };
  }
  const hmiRoot = vscode.Uri.joinPath(resolved.folder.uri, "hmi");
  let entries: [string, vscode.FileType][];
  try {
    entries = await vscode.workspace.fs.readDirectory(hmiRoot);
  } catch {
    return {
      snapshot: {
        exists: false,
        rootPath: resolved.folder.uri.fsPath,
        hmiPath: hmiRoot.fsPath,
        config: null,
        pages: [],
        files: [],
        assets: [],
      },
    };
  }

  const tomlFiles = entries
    .filter(([name, kind]) => kind === vscode.FileType.File && isHmiTomlFileName(name))
    .map(([name]) => name)
    .sort((left, right) => left.localeCompare(right));
  const svgFiles = entries
    .filter(
      ([name, kind]) =>
        kind === vscode.FileType.File &&
        name.toLowerCase().endsWith(".svg"),
    )
    .map(([name]) => name)
    .sort((left, right) => left.localeCompare(right));

  const files: HmiLayoutFileEntry[] = [];
  for (const fileName of tomlFiles) {
    if (token.isCancellationRequested) {
      return { error: "Cancelled." };
    }
    const fileUri = vscode.Uri.joinPath(hmiRoot, fileName);
    const bytes = await vscode.workspace.fs.readFile(fileUri);
    files.push({
      name: fileName,
      path: path.posix.join("hmi", fileName),
      content: Buffer.from(bytes).toString("utf8"),
    });
  }

  const config = files.find((entry) => entry.name === "_config.toml") ?? null;
  const pages = files
    .filter((entry) => isHmiPageTomlFileName(entry.name))
    .sort((left, right) => left.name.localeCompare(right.name));

  return {
    snapshot: {
      exists: true,
      rootPath: resolved.folder.uri.fsPath,
      hmiPath: hmiRoot.fsPath,
      config,
      pages,
      files,
      assets: svgFiles,
    },
  };
}

export function isHmiTomlFileName(name: string): boolean {
  return name.toLowerCase().endsWith(".toml");
}

export function isHmiPageTomlFileName(name: string): boolean {
  const lower = name.toLowerCase();
  return isHmiTomlFileName(name) && lower !== "_config.toml" && !lower.endsWith(".view.toml");
}

export async function writeUtf8File(uri: vscode.Uri, text: string): Promise<void> {
  await vscode.workspace.fs.writeFile(uri, Buffer.from(text, "utf8"));
}

export function normalizeStringList(values: string[] | undefined): string[] {
  if (!Array.isArray(values)) {
    return [];
  }
  return Array.from(
    new Set(
      values
        .map((value) => value.trim())
        .filter((value) => value.length > 0),
    ),
  ).sort((left, right) => left.localeCompare(right));
}

export function tomlQuote(value: string): string {
  return `"${value
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n")
    .replace(/\t/g, "\\t")}"`;
}

export function renderIntentToml(params: HmiPlanIntentParams): string {
  const summary = (params.summary ?? "").trim();
  const goals = normalizeStringList(params.goals);
  const personas = normalizeStringList(params.personas);
  const kpis = normalizeStringList(params.kpis);
  const priorities = normalizeStringList(params.priorities);
  const constraints = normalizeStringList(params.constraints);
  const lines: string[] = [];
  lines.push("version = 1");
  lines.push("");
  lines.push("[intent]");
  lines.push(
    `summary = ${tomlQuote(summary || "Operator-focused HMI intent plan")}`,
  );
  lines.push(
    `personas = [${personas.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `goals = [${goals.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `kpis = [${kpis.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `priorities = [${priorities.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push(
    `constraints = [${constraints.map((value) => tomlQuote(value)).join(", ")}]`,
  );
  lines.push("");
  lines.push("[workflow]");
  lines.push("requires_validation = true");
  lines.push("requires_evidence = true");
  lines.push("requires_journey = true");
  return `${lines.join("\n")}\n`;
}

export type HmiLayoutBindingRef = { file: string; path: string };

export function extractLayoutBindingRefs(
  files: HmiLayoutFileEntry[],
): HmiLayoutBindingRef[] {
  const refs: HmiLayoutBindingRef[] = [];
  const pattern = /^\s*(bind|source)\s*=\s*"([^"]+)"/;
  for (const file of files) {
    for (const line of file.content.split(/\r?\n/)) {
      const match = line.match(pattern);
      if (!match) {
        continue;
      }
      const bindPath = (match[2] ?? "").trim();
      if (!bindPath) {
        continue;
      }
      refs.push({ file: file.name, path: bindPath });
    }
  }
  refs.sort((left, right) =>
    left.path === right.path
      ? left.file.localeCompare(right.file)
      : left.path.localeCompare(right.path),
  );
  return refs;
}

export function parseWritePolicyFromConfigToml(configContent: string | undefined): {
  enabled: boolean;
  allow: string[];
} {
  if (!configContent) {
    return { enabled: false, allow: [] };
  }
  let inWriteSection = false;
  let collectingAllow = false;
  let enabled = false;
  const allow = new Set<string>();
  for (const rawLine of configContent.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith("#")) {
      continue;
    }
    const section = line.match(/^\[([^\]]+)\]$/);
    if (section) {
      inWriteSection = section[1].trim().toLowerCase() === "write";
      collectingAllow = false;
      continue;
    }
    if (!inWriteSection) {
      continue;
    }
    const enabledMatch = line.match(/^enabled\s*=\s*(true|false)\s*$/i);
    if (enabledMatch) {
      enabled = enabledMatch[1].toLowerCase() === "true";
      continue;
    }
    if (collectingAllow || /^allow\s*=/.test(line)) {
      collectingAllow = true;
      for (const quoted of line.matchAll(/"([^"]+)"/g)) {
        const value = (quoted[1] ?? "").trim();
        if (value) {
          allow.add(value);
        }
      }
      if (line.includes("]")) {
        collectingAllow = false;
      }
    }
  }
  return {
    enabled,
    allow: Array.from(allow.values()).sort((left, right) =>
      left.localeCompare(right),
    ),
  };
}

export function stableComponent(value: string): string {
  const source = value.trim().toLowerCase();
  if (!source) {
    return "x";
  }
  let out = "";
  let previousDash = false;
  for (const char of source) {
    const code = char.charCodeAt(0);
    const isAlphaNum =
      (code >= 48 && code <= 57) ||
      (code >= 97 && code <= 122);
    if (isAlphaNum) {
      out += char;
      previousDash = false;
      continue;
    }
    if (!previousDash) {
      out += "-";
      previousDash = true;
    }
  }
  const normalized = out.replace(/^-+/, "").replace(/-+$/, "");
  return normalized || "x";
}

export function canonicalWidgetIdFromPath(pathValue: string): string {
  const trimmed = pathValue.trim();
  if (trimmed.toLowerCase().startsWith("global.")) {
    const name = trimmed.slice("global.".length);
    return `resource/resource/global/${stableComponent(name)}`;
  }
  const parts = trimmed.split(".");
  if (parts.length >= 2) {
    const program = parts[0];
    const field = parts.slice(1).join(".");
    return `resource/resource/program/${stableComponent(program)}/field/${stableComponent(field)}`;
  }
  return `resource/resource/path/${stableComponent(trimmed)}`;
}

export function normalizeHmiBindingsCatalog(response: unknown): HmiBindingCatalog {
  const byPath = new Map<string, HmiBindingCatalogEntry>();
  const entries: HmiBindingCatalogEntry[] = [];
  const payload =
    response && typeof response === "object"
      ? (response as Record<string, unknown>)
      : {};
  const programs = Array.isArray(payload.programs) ? payload.programs : [];
  const globals = Array.isArray(payload.globals) ? payload.globals : [];
  for (const program of programs) {
    if (!program || typeof program !== "object") {
      continue;
    }
    const variables = Array.isArray((program as { variables?: unknown }).variables)
      ? ((program as { variables?: unknown[] }).variables ?? [])
      : [];
    for (const variable of variables) {
      if (!variable || typeof variable !== "object") {
        continue;
      }
      const record = variable as Record<string, unknown>;
      const pathValue = typeof record.path === "string" ? record.path.trim() : "";
      if (!pathValue) {
        continue;
      }
      const entry: HmiBindingCatalogEntry = {
        id: canonicalWidgetIdFromPath(pathValue),
        path: pathValue,
        dataType:
          typeof record.type === "string"
            ? record.type
            : typeof record.data_type === "string"
              ? record.data_type
              : "UNKNOWN",
        qualifier:
          typeof record.qualifier === "string" ? record.qualifier : "UNKNOWN",
        writable: record.writable === true,
        unit: typeof record.unit === "string" ? record.unit : null,
        min: Number.isFinite(record.min) ? Number(record.min) : null,
        max: Number.isFinite(record.max) ? Number(record.max) : null,
        enumValues: normalizeStringList(
          Array.isArray(record.enum_values)
            ? (record.enum_values.filter((value) => typeof value === "string") as string[])
            : [],
        ),
      };
      byPath.set(pathValue, entry);
      entries.push(entry);
    }
  }

  for (const variable of globals) {
    if (!variable || typeof variable !== "object") {
      continue;
    }
    const record = variable as Record<string, unknown>;
    const pathValue = typeof record.path === "string" ? record.path.trim() : "";
    if (!pathValue) {
      continue;
    }
    const entry: HmiBindingCatalogEntry = {
      id: canonicalWidgetIdFromPath(pathValue),
      path: pathValue,
      dataType:
        typeof record.type === "string"
          ? record.type
          : typeof record.data_type === "string"
            ? record.data_type
            : "UNKNOWN",
      qualifier:
        typeof record.qualifier === "string" ? record.qualifier : "UNKNOWN",
      writable: record.writable === true,
      unit: typeof record.unit === "string" ? record.unit : null,
      min: Number.isFinite(record.min) ? Number(record.min) : null,
      max: Number.isFinite(record.max) ? Number(record.max) : null,
      enumValues: normalizeStringList(
        Array.isArray(record.enum_values)
          ? (record.enum_values.filter((value) => typeof value === "string") as string[])
          : [],
      ),
    };
    byPath.set(pathValue, entry);
    entries.push(entry);
  }

  entries.sort((left, right) =>
    left.path === right.path
      ? left.id.localeCompare(right.id)
      : left.path.localeCompare(right.path),
  );

  return { entries, byPath };
}

export function stableSortDeep(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((entry) => stableSortDeep(entry));
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  const source = value as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  for (const key of Object.keys(source).sort((left, right) =>
    left.localeCompare(right),
  )) {
    out[key] = stableSortDeep(source[key]);
  }
  return out;
}

export function stableJsonString(value: unknown): string {
  return JSON.stringify(stableSortDeep(value), null, 2);
}

export function bindingFingerprint(entry: Omit<HmiLockEntry, "binding_fingerprint">): string {
  return crypto
    .createHash("sha256")
    .update(stableJsonString(entry))
    .digest("hex")
    .slice(0, 16);
}

export function buildHmiLockEntries(
  layoutRefs: HmiLayoutBindingRef[],
  catalog: HmiBindingCatalog,
): { entries: HmiLockEntry[]; unknownPaths: string[] } {
  const filesByPath = new Map<string, Set<string>>();
  for (const ref of layoutRefs) {
    const files = filesByPath.get(ref.path) ?? new Set<string>();
    files.add(ref.file);
    filesByPath.set(ref.path, files);
  }
  const layoutPaths = Array.from(filesByPath.keys()).sort((left, right) =>
    left.localeCompare(right),
  );
  const targetPaths =
    layoutPaths.length > 0
      ? layoutPaths
      : Array.from(catalog.byPath.keys()).sort((left, right) =>
          left.localeCompare(right),
        );
  const unknownPaths: string[] = [];
  const entries: HmiLockEntry[] = [];
  for (const pathValue of targetPaths) {
    const match = catalog.byPath.get(pathValue);
    if (!match) {
      unknownPaths.push(pathValue);
    }
    const base: Omit<HmiLockEntry, "binding_fingerprint"> = {
      id: match?.id ?? canonicalWidgetIdFromPath(pathValue),
      path: pathValue,
      data_type: match?.dataType ?? "UNKNOWN",
      qualifier: match?.qualifier ?? "UNKNOWN",
      writable: match?.writable ?? false,
      constraints: {
        unit: match?.unit ?? null,
        min: match?.min ?? null,
        max: match?.max ?? null,
        enum_values: match?.enumValues ?? [],
      },
      files: Array.from(filesByPath.get(pathValue) ?? []).sort((left, right) =>
        left.localeCompare(right),
      ),
    };
    entries.push({
      ...base,
      binding_fingerprint: bindingFingerprint(base),
    });
  }
  entries.sort((left, right) =>
    left.id === right.id
      ? left.path.localeCompare(right.path)
      : left.id.localeCompare(right.id),
  );
  unknownPaths.sort((left, right) => left.localeCompare(right));
  return { entries, unknownPaths };
}
