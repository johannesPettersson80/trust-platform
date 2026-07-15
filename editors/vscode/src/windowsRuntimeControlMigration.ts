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
}

const MINIMUM_CONTROL_TOKEN_LENGTH = 24;

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
  const sectionStart = lines.findIndex(
    (line) => tomlSectionName(line) === "runtime.control"
  );
  if (sectionStart < 0) {
    return unchangedSource(source);
  }

  let sectionEnd = lines.length;
  for (let index = sectionStart + 1; index < lines.length; index += 1) {
    if (tomlSectionName(lines[index]) !== undefined) {
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
    if (assignment?.key === "endpoint") {
      endpoint = assignment.value;
      endpointLine = index;
    } else if (assignment?.key === "auth_token") {
      authToken = assignment.value;
      authLine = index;
    }
  }

  if (
    !isLoopbackTcpEndpoint(endpoint) ||
    (authToken.trim() && !isKnownPlaceholderToken(authToken))
  ) {
    return unchangedSource(source);
  }

  const token = tokenFactory();
  if (!isStrongGeneratedToken(token)) {
    return unchangedSource(source);
  }

  if (authLine >= 0) {
    lines[authLine] = replaceAuthToken(lines[authLine], token);
  } else {
    let insertionLine = sectionEnd;
    while (
      insertionLine > sectionStart + 1 &&
      lines[insertionLine - 1].trim() === ""
    ) {
      insertionLine -= 1;
    }
    const indentation =
      endpointLine >= 0 ? (lines[endpointLine].match(/^\s*/)?.[0] ?? "") : "";
    lines.splice(
      insertionLine,
      0,
      `${indentation}auth_token = ${quoteTomlBasicString(token)}`
    );
  }

  return { changed: true, content: lines.join(eol) };
}

export function migrateWindowsRuntimeControlProject(
  projectRoot: string | undefined,
  platform: NodeJS.Platform = process.platform
): RuntimeControlProjectMigrationResult {
  if (platform !== "win32" || !projectRoot) {
    return { changed: false };
  }

  const runtimeToml = findRuntimeToml(projectRoot);
  if (!runtimeToml) {
    return { changed: false };
  }

  try {
    const source = fs.readFileSync(runtimeToml, "utf8");
    const migration = migrateRuntimeControlTomlSource(source, platform);
    if (!migration.changed) {
      return { changed: false, path: runtimeToml };
    }
    writeAtomically(runtimeToml, migration.content);
    return { changed: true, path: runtimeToml };
  } catch {
    // The subsequent runtime load remains the source of the actionable error. Do not include
    // generated credentials in migration errors or logs.
    return { changed: false, path: runtimeToml };
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

function findRuntimeToml(projectRoot: string): string | undefined {
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

function parseTomlAssignment(
  line: string
): { readonly key: string; readonly value: string } | undefined {
  const match = /^\s*([A-Za-z0-9_-]+)\s*=\s*(.*?)\s*$/.exec(
    stripInlineComment(line)
  );
  if (!match) {
    return undefined;
  }
  const rawValue = match[2].trim();
  const value =
    (rawValue.startsWith('"') && rawValue.endsWith('"')) ||
    (rawValue.startsWith("'") && rawValue.endsWith("'"))
      ? rawValue.slice(1, -1)
      : rawValue;
  return { key: match[1], value };
}

function replaceAuthToken(line: string, token: string): string {
  const commentStart = inlineCommentStart(line);
  const assignment = line.slice(0, commentStart);
  const comment = line.slice(commentStart);
  const match = /^(\s*auth_token\s*=\s*)(.*?)(\s*)$/.exec(assignment);
  if (!match) {
    return line;
  }
  return `${match[1]}${quoteTomlBasicString(token)}${match[3]}${comment}`;
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
