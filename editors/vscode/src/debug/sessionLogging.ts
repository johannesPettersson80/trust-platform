import * as vscode from "vscode";

const SECRET_CONFIG_KEYS = new Set([
  "controlAuthToken",
  "authToken",
  "control_auth_token",
  "auth_token",
]);

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

function redactDebugValue(value: unknown, seen: WeakSet<object>): unknown {
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
    redacted[key] = SECRET_CONFIG_KEYS.has(key)
      ? "***"
      : redactDebugValue(entry, seen);
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
