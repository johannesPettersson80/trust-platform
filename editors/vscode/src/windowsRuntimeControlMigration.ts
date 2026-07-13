import { randomBytes } from "crypto";
import * as fs from "fs";
import { isIP } from "net";
import * as path from "path";

export interface RuntimeControlSourceMigrationResult {
  readonly changed: boolean;
  readonly content: string;
}

export interface RuntimeControlProjectMigrationResult {
  readonly changed: boolean;
  readonly path?: string;
  readonly failure?: RuntimeControlProjectMigrationFailure;
}

export interface RuntimeControlProjectMigrationFailure {
  readonly kind: "configuration";
  readonly code:
    | "runtime_control_toml_malformed"
    | "runtime_control_auth_requires_manual_configuration"
    | "runtime_control_toml_not_writable";
  readonly message: string;
}

const MINIMUM_CONTROL_TOKEN_LENGTH = 24;

type RuntimeControlTomlForm = "table" | "dotted";

interface RuntimeControlTomlLocation {
  readonly form: RuntimeControlTomlForm;
  readonly endpoint: string;
  readonly endpointLine: number;
  readonly authToken: string;
  readonly authLine: number;
  readonly insertionLine: number;
}

interface ParsedTomlAssignment {
  readonly key: string;
  readonly stringValue?: string;
}

export function migrateRuntimeControlTomlSource(
  source: string,
  platform: NodeJS.Platform,
  tokenFactory: () => string = generateControlToken
): RuntimeControlSourceMigrationResult {
  if (platform !== "win32") {
    return unchangedSource(source);
  }

  const eol = source.includes("\r\n") ? "\r\n" : "\n";
  const lines = source.split(/\r?\n/);
  const location = findRuntimeControlLocation(lines);
  if (!location) {
    return unchangedSource(source);
  }

  if (
    !isLoopbackTcpEndpoint(location.endpoint) ||
    (location.authToken.trim() && !isKnownPlaceholderToken(location.authToken))
  ) {
    return unchangedSource(source);
  }

  const token = tokenFactory();
  if (!isStrongGeneratedToken(token)) {
    return unchangedSource(source);
  }

  if (location.authLine >= 0) {
    lines[location.authLine] = replaceAuthToken(
      lines[location.authLine],
      token,
      location.form === "table" ? "auth_token" : "runtime.control.auth_token"
    );
  } else {
    const indentation =
      location.endpointLine >= 0
        ? (lines[location.endpointLine].match(/^\s*/)?.[0] ?? "")
        : "";
    const authKey =
      location.form === "table" ? "auth_token" : "runtime.control.auth_token";
    lines.splice(
      location.insertionLine,
      0,
      `${indentation}${authKey} = ${quoteTomlBasicString(token)}`
    );
  }

  return { changed: true, content: lines.join(eol) };
}

export function migrateWindowsRuntimeControlProject(
  projectRoot: string | undefined,
  platform: NodeJS.Platform = process.platform,
  tokenFactory: () => string = generateControlToken
): RuntimeControlProjectMigrationResult {
  if (platform !== "win32" || !projectRoot) {
    return { changed: false };
  }

  const runtimeToml = findRuntimeControlToml(projectRoot);
  if (!runtimeToml) {
    return { changed: false };
  }

  try {
    const source = fs.readFileSync(runtimeToml, "utf8");
    const preflight = inspectWindowsRuntimeControlSource(source);
    if (preflight.failure) {
      return { changed: false, path: runtimeToml, failure: preflight.failure };
    }
    if (!preflight.needsMigration) {
      return { changed: false, path: runtimeToml };
    }
    if (!canAtomicallyReplace(runtimeToml)) {
      return {
        changed: false,
        path: runtimeToml,
        failure: runtimeControlMigrationFailure(
          "runtime_control_toml_not_writable",
          "Simulator control authentication could not be added because runtime.toml is not writable. Make the file writable or open it and configure the token."
        ),
      };
    }
    const migration = migrateRuntimeControlTomlSource(
      source,
      platform,
      tokenFactory
    );
    if (!migration.changed) {
      return {
        changed: false,
        path: runtimeToml,
        failure: runtimeControlMigrationFailure(
          "runtime_control_auth_requires_manual_configuration",
          "Simulator control authentication in runtime.toml needs manual configuration. Open runtime.toml and configure a strong token."
        ),
      };
    }
    writeAtomically(runtimeToml, migration.content);
    const persisted = fs.readFileSync(runtimeToml, "utf8");
    const persistedPreflight = inspectWindowsRuntimeControlSource(persisted);
    if (persistedPreflight.failure || persistedPreflight.needsMigration) {
      return {
        changed: false,
        path: runtimeToml,
        failure:
          persistedPreflight.failure ??
          runtimeControlMigrationFailure(
            "runtime_control_auth_requires_manual_configuration",
            "Simulator control authentication in runtime.toml needs manual configuration. Open runtime.toml and configure a strong token."
          ),
      };
    }
    return { changed: true, path: runtimeToml };
  } catch {
    // Never include source text, endpoints, or generated credentials in this
    // typed failure. The Run surface owns the one-click file recovery.
    return {
      changed: false,
      path: runtimeToml,
      failure: runtimeControlMigrationFailure(
        "runtime_control_toml_not_writable",
        "Simulator control authentication could not be saved in runtime.toml. Make the file writable or open it and configure the token."
      ),
    };
  }
}

type RuntimeControlSourcePreflight = {
  readonly needsMigration: boolean;
  readonly failure?: RuntimeControlProjectMigrationFailure;
};

function inspectWindowsRuntimeControlSource(
  source: string
): RuntimeControlSourcePreflight {
  const lines = source.split(/\r?\n/);
  const location = findRuntimeControlLocation(lines);
  if (!location) {
    return containsTcpRuntimeControlEvidence(lines)
      ? {
          needsMigration: false,
          failure: runtimeControlMigrationFailure(
            "runtime_control_toml_malformed",
            "The runtime.control TCP settings in runtime.toml could not be read safely. Open runtime.toml and fix that section before starting the Simulator."
          ),
        }
      : { needsMigration: false };
  }
  const endpoint = location.endpoint.trim();
  if (!endpoint.toLowerCase().startsWith("tcp://")) {
    return { needsMigration: false };
  }
  if (!isValidTcpEndpoint(endpoint)) {
    return {
      needsMigration: false,
      failure: runtimeControlMigrationFailure(
        "runtime_control_toml_malformed",
        "The runtime.control TCP endpoint in runtime.toml is invalid. Open runtime.toml and fix that section before starting the Simulator."
      ),
    };
  }
  if (
    location.authToken.trim() &&
    !isKnownPlaceholderToken(location.authToken)
  ) {
    return { needsMigration: false };
  }
  if (!isLoopbackTcpEndpoint(endpoint)) {
    return {
      needsMigration: false,
      failure: runtimeControlMigrationFailure(
        "runtime_control_auth_requires_manual_configuration",
        "The runtime.control TCP endpoint in runtime.toml requires a strong authentication token. truST adds one automatically only for this computer; open runtime.toml to configure this endpoint."
      ),
    };
  }
  return { needsMigration: true };
}

function runtimeControlMigrationFailure(
  code: RuntimeControlProjectMigrationFailure["code"],
  message: string
): RuntimeControlProjectMigrationFailure {
  return { kind: "configuration", code, message };
}

function containsTcpRuntimeControlEvidence(lines: readonly string[]): boolean {
  let section = "";
  for (const line of lines) {
    const sectionName = tomlSectionName(line);
    if (sectionName !== undefined) {
      section = sectionName;
      continue;
    }
    const content = stripInlineComment(line);
    if (
      (section === "runtime.control" && /^\s*endpoint\s*=\s*["']tcp:\/\//i.test(content)) ||
      /^\s*runtime\.control\.endpoint\s*=\s*["']tcp:\/\//i.test(content)
    ) {
      return true;
    }
  }
  return false;
}

function canAtomicallyReplace(target: string): boolean {
  try {
    const fileMode = fs.statSync(target).mode;
    const directory = path.dirname(target);
    const directoryMode = fs.statSync(directory).mode;
    if ((fileMode & 0o222) === 0 || (directoryMode & 0o222) === 0) {
      return false;
    }
    fs.accessSync(target, fs.constants.W_OK);
    fs.accessSync(directory, fs.constants.W_OK);
    return true;
  } catch {
    return false;
  }
}

function unchangedSource(source: string): RuntimeControlSourceMigrationResult {
  return { changed: false, content: source };
}

function generateControlToken(): string {
  return randomBytes(24).toString("hex");
}

function isStrongGeneratedToken(token: string): boolean {
  const trimmed = token.trim();
  if (trimmed.length < MINIMUM_CONTROL_TOKEN_LENGTH) {
    return false;
  }
  return !isKnownPlaceholderToken(trimmed);
}

function isKnownPlaceholderToken(token: string): boolean {
  return /^(?:change-?me|replace-?me|placeholder|some-secret-value|your-secret-here)$/i.test(
    token.trim()
  );
}

export function findRuntimeControlToml(projectRoot: string): string | undefined {
  for (const candidate of [
    path.join(projectRoot, "runtime.toml"),
    path.join(projectRoot, "bundle", "runtime.toml"),
  ]) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

function writeAtomically(target: string, content: string): void {
  const mode = fs.statSync(target).mode & 0o777;
  const temporary = `${target}.trust-migrate-${process.pid}-${randomBytes(4).toString("hex")}`;
  let descriptor: number | undefined;
  try {
    descriptor = fs.openSync(temporary, "wx", mode);
    fs.writeFileSync(descriptor, content, "utf8");
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = undefined;
    fs.chmodSync(temporary, mode);

    // Node's fs.rename contract overwrites an existing destination on every supported platform,
    // including Windows. Keeping the temporary file beside the target makes this one-filesystem
    // replacement instead of exposing a tokenless intermediate runtime.toml.
    fs.renameSync(temporary, target);
  } finally {
    if (descriptor !== undefined) {
      fs.closeSync(descriptor);
    }
    if (fs.existsSync(temporary)) {
      fs.unlinkSync(temporary);
    }
  }
}

function tomlSectionName(line: string): string | undefined {
  const match = /^\s*\[([^\[\]]+)\]\s*$/.exec(stripInlineComment(line));
  return match?.[1].trim();
}

function findRuntimeControlLocation(
  lines: readonly string[]
): RuntimeControlTomlLocation | undefined {
  const tableStarts: number[] = [];
  let root = true;
  let dottedEndpoint = "";
  let dottedEndpointLine = -1;
  let dottedAuthToken = "";
  let dottedAuthLine = -1;

  for (let index = 0; index < lines.length; index += 1) {
    const sectionName = tomlSectionName(lines[index]);
    if (sectionName === "runtime.control") {
      tableStarts.push(index);
    }
    if (isTomlHeaderBoundary(lines[index])) {
      root = false;
    }

    const assignment = parseTomlAssignment(lines[index]);
    if (
      assignment?.key !== "runtime.control.endpoint" &&
      assignment?.key !== "runtime.control.auth_token"
    ) {
      continue;
    }
    if (!root || assignment.stringValue === undefined) {
      return undefined;
    }
    if (assignment.key === "runtime.control.endpoint") {
      if (dottedEndpointLine >= 0) {
        return undefined;
      }
      dottedEndpoint = assignment.stringValue;
      dottedEndpointLine = index;
    } else {
      if (dottedAuthLine >= 0) {
        return undefined;
      }
      dottedAuthToken = assignment.stringValue;
      dottedAuthLine = index;
    }
  }

  const hasDottedControl = dottedEndpointLine >= 0 || dottedAuthLine >= 0;
  if (tableStarts.length > 1 || (tableStarts.length === 1 && hasDottedControl)) {
    return undefined;
  }
  if (hasDottedControl) {
    if (dottedEndpointLine < 0) {
      return undefined;
    }
    return {
      form: "dotted",
      endpoint: dottedEndpoint,
      endpointLine: dottedEndpointLine,
      authToken: dottedAuthToken,
      authLine: dottedAuthLine,
      insertionLine: dottedEndpointLine + 1,
    };
  }
  if (tableStarts.length === 0) {
    return undefined;
  }

  const sectionStart = tableStarts[0];
  let sectionEnd = lines.length;
  for (let index = sectionStart + 1; index < lines.length; index += 1) {
    if (isTomlHeaderBoundary(lines[index])) {
      sectionEnd = index;
      break;
    }
  }
  let endpoint = "";
  let endpointLine = -1;
  let authToken = "";
  let authLine = -1;
  for (let index = sectionStart + 1; index < sectionEnd; index += 1) {
    const assignment = parseTomlAssignment(lines[index]);
    if (assignment?.key !== "endpoint" && assignment?.key !== "auth_token") {
      continue;
    }
    if (assignment.stringValue === undefined) {
      return undefined;
    }
    if (assignment.key === "endpoint") {
      if (endpointLine >= 0) {
        return undefined;
      }
      endpoint = assignment.stringValue;
      endpointLine = index;
    } else {
      if (authLine >= 0) {
        return undefined;
      }
      authToken = assignment.stringValue;
      authLine = index;
    }
  }
  if (endpointLine < 0) {
    return undefined;
  }

  let insertionLine = sectionEnd;
  while (
    insertionLine > sectionStart + 1 &&
    lines[insertionLine - 1].trim() === ""
  ) {
    insertionLine -= 1;
  }
  return {
    form: "table",
    endpoint,
    endpointLine,
    authToken,
    authLine,
    insertionLine,
  };
}

function isTomlHeaderBoundary(line: string): boolean {
  return stripInlineComment(line).trimStart().startsWith("[");
}

function parseTomlAssignment(
  line: string
): ParsedTomlAssignment | undefined {
  const match = /^\s*([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)\s*=\s*(.*?)\s*$/.exec(
    stripInlineComment(line)
  );
  if (!match) {
    return undefined;
  }
  return { key: match[1], stringValue: parseSimpleTomlString(match[2]) };
}

function parseSimpleTomlString(value: string): string | undefined {
  const trimmed = value.trim();
  const basic = /^"([^"\\]*)"$/.exec(trimmed);
  if (basic) {
    return basic[1];
  }
  return /^'([^']*)'$/.exec(trimmed)?.[1];
}

function replaceAuthToken(line: string, token: string, key: string): string {
  const commentStart = inlineCommentStart(line);
  const assignment = line.slice(0, commentStart);
  const comment = line.slice(commentStart);
  const match = new RegExp(
    `^(\\s*${escapeRegularExpression(key)}\\s*=\\s*)(.*?)(\\s*)$`
  ).exec(assignment);
  if (!match) {
    return line;
  }
  return `${match[1]}${quoteTomlBasicString(token)}${match[3]}${comment}`;
}

function escapeRegularExpression(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function stripInlineComment(line: string): string {
  return line.slice(0, inlineCommentStart(line));
}

function inlineCommentStart(line: string): number {
  let singleQuoted = false;
  let doubleQuoted = false;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (doubleQuoted && character === "\\" && !escaped) {
      escaped = true;
      continue;
    }
    if (character === "'" && !doubleQuoted) {
      singleQuoted = !singleQuoted;
    } else if (character === '"' && !singleQuoted && !escaped) {
      doubleQuoted = !doubleQuoted;
    } else if (character === "#" && !singleQuoted && !doubleQuoted) {
      return index;
    }
    escaped = false;
  }
  return line.length;
}

function quoteTomlBasicString(value: string): string {
  return `"${value
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n")}"`;
}

function isLoopbackTcpEndpoint(endpoint: string): boolean {
  const match = /^tcp:\/\/(?:\[([^\]]+)\]|([^:/]+)):(\d+)$/.exec(endpoint.trim());
  if (!match) {
    return false;
  }
  const port = Number(match[3]);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    return false;
  }

  const host = (match[1] ?? match[2]).toLowerCase();
  if (host === "localhost" || host === "::1") {
    return true;
  }
  return isIP(host) === 4 && host.split(".")[0] === "127";
}

function isValidTcpEndpoint(endpoint: string): boolean {
  const match = /^tcp:\/\/(?:\[([^\]]+)\]|([^:/]+)):(\d+)$/.exec(
    endpoint.trim()
  );
  if (!match || !(match[1] ?? match[2]).trim()) {
    return false;
  }
  const port = Number(match[3]);
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}
