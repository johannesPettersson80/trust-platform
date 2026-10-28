// The `comm.schema` request of a runtime (specification 11). Applying, testing and saving a form
// is `trust-lsp.devicesCanvasAct`'s (networkCanvas/canvasCommands.ts; specification 25 §5.9).
import { type RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { CommSchemaResponse } from "./schemaForm";

export async function fetchCommSchema(
  runtime: RuntimeTarget,
  protocol?: string,
  timeoutMs = 2000
): Promise<CommSchemaResponse> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    throw new Error("Select a reachable online runtime before loading Communication setup.");
  }
  return await sendRuntimeControlRequest<CommSchemaResponse>(
    runtime.endpoint,
    runtime.authToken,
    "comm.schema",
    protocol ? { protocol } : undefined,
    { timeoutMs }
  );
}

/** A protocol ID as the canvas's setup stages name it: trimmed, `-` as `_`, in lower case. */
export function normalizeProtocolId(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim().replace(/-/g, "_").toLowerCase();
  return normalized.length > 0 ? normalized : undefined;
}
