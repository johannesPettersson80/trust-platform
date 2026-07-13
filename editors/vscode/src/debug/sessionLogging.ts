import type * as vscode from "vscode";

const SECRET_CONFIG_KEYS = new Set([
  "controlAuthToken",
  "authToken",
  "control_auth_token",
  "auth_token",
]);
const SECRET_TEXT_ASSIGNMENT = new RegExp(
  String.raw`((?:["'])?(?:[A-Za-z_][\w-]*\.)*(?:controlAuthToken|authToken|control_auth_token|auth_token)(?:["'])?\s*[:=]\s*)("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s,}\]&;]+)`,
  "gi"
);
const SECRET_QUERY_PARAMETER =
  /([?&](?:controlAuthToken|authToken|control_auth_token|auth_token|token)=)[^&#\s]*/gi;

// The debug output channel is user-visible; never log injected or configured
// control credentials, including nested reverse-DAP configurations.
export function redactDebugConfig(
  config: vscode.DebugConfiguration
): Record<string, unknown> {
  return redactDebugValue(config, new WeakSet()) as Record<string, unknown>;
}

/** Redacts credentials carried inside a DAP launch or attach request. */
export function redactDapMessage(value: unknown): unknown {
  return redactDebugValue(value, new WeakSet());
}

/** Redacts credentials embedded in error text or serialized launch arguments. */
export function redactDebugText(value: unknown): string {
  return String(value)
    .replace(SECRET_QUERY_PARAMETER, "$1***")
    .replace(SECRET_TEXT_ASSIGNMENT, (_match, prefix: string, raw: string) => {
      const quote = raw[0];
      const replacement = quote === '"' || quote === "'" ? `${quote}***${quote}` : "***";
      return `${prefix}${replacement}`;
    });
}

function redactDebugValue(value: unknown, seen: WeakSet<object>): unknown {
  if (typeof value === "string") {
    return redactDebugText(value);
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  if (seen.has(value)) {
    return "[Circular]";
  }
  seen.add(value);
  if (Array.isArray(value)) {
    return value.map((item) => redactDebugValue(item, seen));
  }
  const redacted: Record<string, unknown> = {};
  for (const [key, entry] of Object.entries(value)) {
    if (SECRET_CONFIG_KEYS.has(key)) {
      redacted[key] = "***";
    } else {
      redacted[key] = redactDebugValue(entry, seen);
    }
  }
  return redacted;
}

export function stringifyDebugSession(
  session: Pick<vscode.DebugSession, "configuration">
): string {
  try {
    return JSON.stringify(redactDebugConfig(session.configuration));
  } catch (error) {
    return String(error);
  }
}
