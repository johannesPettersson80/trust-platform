import * as vscode from "vscode";
import { projectRequest } from "./projectRequest";

// The runtime that a debug session attached to, named to the language server for the inline values of the session's
// workspace folder (specification 25 §6.8, `trust-lsp.attachedRuntime`). The extension names the session, its folder
// and the endpoint, which are facts; the language server finds the token of the folder's own runtime itself, so no
// token crosses (specification 14 §7.1.2).

const DEBUG_TYPE = "structured-text";

/** What the language server is told of an attach session: its identity, its workspace folder and its endpoint. */
export interface AttachedRuntimeNote {
  readonly session: string;
  readonly folder: string | null;
  readonly endpoint: string | null;
}

type Session = Pick<vscode.DebugSession, "id" | "type" | "configuration" | "workspaceFolder">;

/** The note of `session` when it is an attach session of truST, else none. */
export function attachedRuntimeOf(session: Session): AttachedRuntimeNote | undefined {
  const configuration = session.configuration;
  if (session.type !== DEBUG_TYPE || configuration?.request !== "attach" || typeof configuration.endpoint !== "string") {
    return undefined;
  }
  return { session: session.id, folder: session.workspaceFolder?.uri.fsPath ?? null, endpoint: configuration.endpoint };
}

/** Sends the note of a starting attach session. */
export function attachedSessionStarted(session: Session, send: (note: AttachedRuntimeNote) => void): void {
  const note = attachedRuntimeOf(session);
  if (note) send(note);
}

/** Sends the end of an attach session: its note with `endpoint: null`. */
export function attachedSessionEnded(session: Session, send: (note: AttachedRuntimeNote) => void): void {
  const note = attachedRuntimeOf(session);
  if (note) send({ ...note, endpoint: null });
}

/** Names each attach session to the language server when it starts, and again with `endpoint: null` when it ends. */
export function registerAttachedRuntimeForwarding(context: vscode.ExtensionContext): void {
  const send = (note: AttachedRuntimeNote) => void projectRequest("trust-lsp.attachedRuntime", note);
  context.subscriptions.push(
    vscode.debug.onDidStartDebugSession((session) => attachedSessionStarted(session, send)),
    vscode.debug.onDidTerminateDebugSession((session) => attachedSessionEnded(session, send))
  );
}
