import type * as vscode from "vscode";

import { runtimeTargetForSession } from "./runtimeLifecycleModel";
import { setSelectedRuntimeId } from "./selectedRuntime";

/** Stable identity comparison shared by lifecycle-adjacent session consumers. */
export function sameRuntimeDebugSession(
  left: Pick<vscode.DebugSession, "id" | "name" | "type"> | undefined,
  right: Pick<vscode.DebugSession, "id" | "name" | "type">,
): boolean {
  if (!left) {
    return false;
  }
  if (left.id && right.id) {
    return left.id === right.id;
  }
  return left.name === right.name && left.type === right.type;
}

/** Persists the accepted debug session as the single selected runtime target. */
export async function selectRuntimeSessionTarget(
  session: vscode.DebugSession,
): Promise<void> {
  const target = runtimeTargetForSession(session);
  if (target.kind === "simulator") {
    await setSelectedRuntimeId("simulator");
  } else if (target.kind === "managed") {
    await setSelectedRuntimeId(target.id);
  } else if (target.kind === "remote" && target.endpoint) {
    await setSelectedRuntimeId(target.endpoint);
  }
}
