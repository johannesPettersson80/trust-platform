import { execFile } from "child_process";

// Spec 11 §6.8.0a: the runtime owns a project's control endpoint and its token. The extension asks
// `trust-runtime ctl --project <dir> endpoint --json` for both and never reads runtime.toml, the
// project's endpoint file or a per-user token file itself. This module only runs that command and
// reads its JSON answer; every failure arrives as `{"error":{"code","message"}}`, whose message the
// extension shows.

export type ProjectControlSource = "endpoint_file" | "configured" | "default";

export interface ProjectControlEndpoint {
  readonly endpoint: string;
  readonly authToken?: string;
  readonly source: ProjectControlSource;
}

export type ProjectControlEndpointResult =
  | { readonly ok: true; readonly value: ProjectControlEndpoint }
  | { readonly ok: false; readonly code: string; readonly message: string };

// Returns the command's stdout whatever its exit status: the endpoint command reports every failure
// as JSON on stdout with a non-zero exit. Undefined means that the command could not run.
export type ProjectCommandRunner = (
  bin: string,
  args: readonly string[],
  cwd: string
) => Promise<string | undefined>;

const SOURCES: readonly ProjectControlSource[] = [
  "endpoint_file",
  "configured",
  "default",
];

export function parseProjectControlEndpoint(
  stdout: string | undefined
): ProjectControlEndpointResult {
  const parsed = parseJsonObject(stdout);
  if (!parsed) {
    return {
      ok: false,
      code: "invalid_response",
      message: "trust-runtime did not report the runtime control endpoint.",
    };
  }
  if (isRecord(parsed.error)) {
    return {
      ok: false,
      code: stringOrUndefined(parsed.error.code) ?? "unknown",
      message:
        stringOrUndefined(parsed.error.message) ??
        "The runtime control endpoint is unavailable.",
    };
  }
  const endpoint = stringOrUndefined(parsed.endpoint);
  const source = SOURCES.find((candidate) => candidate === parsed.source);
  if (!endpoint || !source) {
    return {
      ok: false,
      code: "invalid_response",
      message: "trust-runtime reported an incomplete runtime control endpoint.",
    };
  }
  return {
    ok: true,
    value: {
      endpoint,
      authToken: stringOrUndefined(parsed.auth_token),
      source,
    },
  };
}

export async function resolveProjectControlEndpoint(
  bin: string,
  projectDir: string,
  run: ProjectCommandRunner = runCommand
): Promise<ProjectControlEndpointResult> {
  const stdout = await run(
    bin,
    ["ctl", "--project", projectDir, "endpoint", "--json"],
    projectDir
  );
  return parseProjectControlEndpoint(stdout);
}

function runCommand(
  bin: string,
  args: readonly string[],
  cwd: string
): Promise<string | undefined> {
  return new Promise((resolve) => {
    execFile(
      bin,
      [...args],
      { cwd, timeout: 15_000, maxBuffer: 1024 * 1024 },
      (_error, stdout) => {
        resolve(typeof stdout === "string" ? stdout : undefined);
      }
    );
  });
}

function parseJsonObject(
  text: string | undefined
): Record<string, unknown> | undefined {
  if (!text || !text.trim()) {
    return undefined;
  }
  try {
    const value: unknown = JSON.parse(text);
    return isRecord(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringOrUndefined(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
