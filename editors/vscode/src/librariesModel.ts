export type LibrarySourceKind = "bundled" | "local" | "git";

export interface LibraryDependencyEntry {
  readonly name: string;
  readonly source: LibrarySourceKind;
  readonly path?: string;
  readonly git?: string;
  readonly version?: string;
  readonly rev?: string;
  readonly tag?: string;
  readonly branch?: string;
}

export interface DependencySpec {
  readonly path?: string;
  readonly git?: string;
  readonly version?: string;
  readonly rev?: string;
  readonly tag?: string;
  readonly branch?: string;
}

export interface SymbolSummary {
  readonly name: string;
  readonly kind: "function_block" | "function" | "type";
  readonly file: string;
  readonly declaration: string;
}

interface SectionRange {
  readonly start: number;
  readonly end: number;
}

const SECTION_RE = /^\s*\[([^\]]+)]\s*(?:#.*)?$/;
const DEPENDENCY_RE = /^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+?)\s*(?:#.*)?$/;
const STRING_RE = /^"([^"]*)"$/;

export function parseDependencyEntries(source: string): LibraryDependencyEntry[] {
  const lines = source.split(/\r?\n/);
  const range = dependenciesSection(lines);
  if (!range) {
    return [];
  }
  const entries: LibraryDependencyEntry[] = [];
  for (let i = range.start + 1; i < range.end; i += 1) {
    const match = lines[i].match(DEPENDENCY_RE);
    if (!match) {
      continue;
    }
    const name = match[1];
    const value = match[2].trim();
    const parsed = parseDependencyValue(value);
    if (!parsed) {
      continue;
    }
    entries.push({ name, ...parsed });
  }
  return entries;
}

export function upsertDependency(
  source: string,
  name: string,
  spec: DependencySpec
): string {
  const line = `${name} = ${formatDependencySpec(spec)}`;
  const lines = source.split(/\r?\n/);
  const range = dependenciesSection(lines);
  if (!range) {
    const trimmed = source.replace(/\s+$/, "");
    return `${trimmed}${trimmed ? "\n\n" : ""}[dependencies]\n${line}\n`;
  }

  for (let i = range.start + 1; i < range.end; i += 1) {
    const match = lines[i].match(DEPENDENCY_RE);
    if (match && match[1] === name) {
      lines[i] = line;
      return normalizeTrailingNewline(lines.join("\n"));
    }
  }

  lines.splice(range.end, 0, line);
  return normalizeTrailingNewline(lines.join("\n"));
}

export function removeDependency(source: string, name: string): string {
  const lines = source.split(/\r?\n/);
  const range = dependenciesSection(lines);
  if (!range) {
    return normalizeTrailingNewline(source);
  }
  for (let i = range.start + 1; i < range.end; i += 1) {
    const match = lines[i].match(DEPENDENCY_RE);
    if (match && match[1] === name) {
      lines.splice(i, 1);
      return normalizeTrailingNewline(lines.join("\n"));
    }
  }
  return normalizeTrailingNewline(source);
}

export function formatDependencySpec(spec: DependencySpec): string {
  if (spec.path && !spec.git) {
    const parts = [`path = "${escapeTomlString(spec.path)}"`];
    if (spec.version) {
      parts.push(`version = "${escapeTomlString(spec.version)}"`);
    }
    return `{ ${parts.join(", ")} }`;
  }
  if (spec.git && !spec.path) {
    const selectorCount = [spec.rev, spec.tag, spec.branch].filter(Boolean).length;
    if (selectorCount > 1) {
      throw new Error("Git dependency may set only one of rev, tag, or branch.");
    }
    const parts = [`git = "${escapeTomlString(spec.git)}"`];
    if (spec.rev) {
      parts.push(`rev = "${escapeTomlString(spec.rev)}"`);
    }
    if (spec.tag) {
      parts.push(`tag = "${escapeTomlString(spec.tag)}"`);
    }
    if (spec.branch) {
      parts.push(`branch = "${escapeTomlString(spec.branch)}"`);
    }
    if (spec.version) {
      parts.push(`version = "${escapeTomlString(spec.version)}"`);
    }
    return `{ ${parts.join(", ")} }`;
  }
  throw new Error("Library dependency must set exactly one of path or git.");
}

export function parsePackageVersion(source: string): string | undefined {
  let inPackage = false;
  for (const raw of source.split(/\r?\n/)) {
    const section = raw.match(SECTION_RE);
    if (section) {
      inPackage = section[1].trim() === "package";
      continue;
    }
    if (!inPackage) {
      continue;
    }
    const match = raw.match(/^\s*version\s*=\s*"([^"]+)"/);
    if (match) {
      return match[1];
    }
  }
  return undefined;
}

export function collectSymbolSummaries(
  files: ReadonlyArray<{ readonly file: string; readonly text: string }>
): SymbolSummary[] {
  const symbols: SymbolSummary[] = [];
  const seen = new Set<string>();
  for (const file of files) {
    for (const match of file.text.matchAll(
      /^\s*(FUNCTION_BLOCK|FUNCTION|TYPE)\s+([A-Za-z_][A-Za-z0-9_]*)\b/gim
    )) {
      const kind =
        match[1].toUpperCase() === "FUNCTION_BLOCK"
          ? "function_block"
          : match[1].toUpperCase() === "FUNCTION"
            ? "function"
            : "type";
      const name = match[2];
      const key = `${kind}:${name}`;
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      const start = match.index ?? 0;
      const lineStart = file.text.lastIndexOf("\n", start) + 1;
      const lineEnd = file.text.indexOf("\n", start);
      const declaration = file.text
        .slice(lineStart, lineEnd >= 0 ? lineEnd : undefined)
        .trim()
        .replace(/\s+/g, " ");
      symbols.push({ name, kind, file: file.file, declaration });
    }
  }
  symbols.sort((a, b) => a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name));
  return symbols;
}

export function classifyGitPin(value: string): { rev?: string; tag?: string; branch?: string } {
  const trimmed = value.trim();
  if (/^[0-9a-f]{7,40}$/i.test(trimmed)) {
    return { rev: trimmed };
  }
  if (/^v?\d+\.\d+/.test(trimmed)) {
    return { tag: trimmed };
  }
  return { branch: trimmed };
}

export function posixPath(value: string): string {
  return value.replace(/\\/g, "/");
}

function dependenciesSection(lines: readonly string[]): SectionRange | undefined {
  let start = -1;
  for (let i = 0; i < lines.length; i += 1) {
    const match = lines[i].match(SECTION_RE);
    if (!match) {
      continue;
    }
    if (match[1].trim() === "dependencies") {
      start = i;
      continue;
    }
    if (start >= 0) {
      return { start, end: i };
    }
  }
  return start >= 0 ? { start, end: lines.length } : undefined;
}

function parseDependencyValue(
  value: string
): Omit<LibraryDependencyEntry, "name"> | undefined {
  const stringMatch = value.match(STRING_RE);
  if (stringMatch) {
    return { source: "local", path: stringMatch[1] };
  }
  if (!value.startsWith("{") || !value.endsWith("}")) {
    return undefined;
  }
  const fields = new Map<string, string>();
  for (const match of value.matchAll(/([A-Za-z_][A-Za-z0-9_]*)\s*=\s*"([^"]*)"/g)) {
    fields.set(match[1], match[2]);
  }
  const path = fields.get("path");
  const git = fields.get("git");
  if (path && !git) {
    return {
      source: "local",
      path,
      version: fields.get("version"),
    };
  }
  if (git && !path) {
    const version = fields.get("version");
    const rev = fields.get("rev");
    const tag = fields.get("tag");
    const branch = fields.get("branch");
    return {
      source: "git",
      git,
      ...(version ? { version } : {}),
      ...(rev ? { rev } : {}),
      ...(tag ? { tag } : {}),
      ...(branch ? { branch } : {}),
    };
  }
  return undefined;
}

function escapeTomlString(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function normalizeTrailingNewline(value: string): string {
  return `${value.replace(/\s+$/, "")}\n`;
}
