// The authoring engine's commands in trust-lsp (specification 43, §3). Each function forwards one request to
// its `trust-lsp.hmiAgent.*` command and returns the answer unchanged. The command decodes the argument and
// checks the envelope; no rule of authoring lives here.

import * as path from "path";
import * as vscode from "vscode";
import { remoteVariableContext } from "../editor/remoteVariables";
import { cancellableNativeRead } from "./lifetime";
import { selectionsOf } from "./queryMetadata";
import { simulatorSessions } from "./simulator";

/** The error codes of specification 43, §4. */
export type AuthoringErrorCode =
  | "INVALID_ARGUMENT"
  | "UNSUPPORTED_VERSION"
  | "PROJECT_UNAVAILABLE"
  | "COMPILATION_FAILED"
  | "CONFLICT"
  | "CANCELLED"
  | "VALIDATION_FAILED"
  | "RENDER_FAILED"
  | "NOT_CONNECTED"
  | "UNSUPPORTED_CONTEXT"
  | "LIMIT_EXCEEDED"
  | "EXPIRED_DRAFT";

/** The answer envelope of every authoring command (specification 43, §4). */
export type AuthoringResult =
  | { ok: true; data: Record<string, unknown>; revisions: Record<string, string>; warnings: string[] }
  | { ok: false; code: AuthoringErrorCode; message: string; retryable: boolean; details?: unknown };

/** A request envelope, `{ contractVersion, workspaceId, input }`, as the command's boundary reads it. */
export type Envelope = unknown;

/** The project one authoring command works on. */
export interface EngineProject {
  /** The project folder: a `file` URI of an extension-host workspace folder. */
  readonly root: vscode.Uri;
  /** The identity of the connected workspace. The commands refuse a request that names another one. */
  readonly workspaceId: string;
  /** The absolute folder that holds this workspace's draft records. */
  readonly storage: string;
  /** Ends the wait for a read. A mutation is awaited once started, so its committed outcome is reported. */
  readonly token: vscode.CancellationToken;
}

/** One open editor document of the project: its text, its editor version and whether it is unsaved. */
export interface OpenText {
  text: string;
  version: number;
  dirty: boolean;
}

/** The outcome of a command the extension renders or applies a receipt for (`apply`, phase `receipt`). */
export interface ApplyReceipt {
  status: "committed" | "no-source-change" | "uncertain";
  code?: string;
  message?: string;
  outcome?: Record<string, unknown>;
}

/** The envelope of a request the extension makes itself, around one tool input. */
export function envelopeFor(workspaceId: string, input: Record<string, unknown>): Envelope {
  return { contractVersion: 1, workspaceId, input };
}

/** The open editor documents under the project, by project-relative path. */
export function openDocuments(root: vscode.Uri): Record<string, OpenText> {
  const open: Record<string, OpenText> = {};
  for (const document of vscode.workspace.textDocuments) {
    if (document.uri.scheme !== root.scheme || document.uri.authority !== root.authority) continue;
    const relative = path.relative(root.fsPath, document.uri.fsPath);
    if (!relative || relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) continue;
    open[relative.split(path.sep).join("/")] = {
      text: document.getText(),
      version: document.version,
      dirty: document.isDirty,
    };
  }
  return open;
}

/** A native HMI command: its name and input, answered as the command answers. */
export type NativeCall = (command: string, input: Record<string, unknown>) => Promise<Record<string, unknown>>;

/** A native HMI command (`trust-lsp.hmi…`), ended by the token; its answer is returned as it is. */
export function nativeRead(
  token: vscode.CancellationToken,
  command: string,
  input: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  return cancellableNativeRead(token, () =>
    Promise.resolve(vscode.commands.executeCommand<Record<string, unknown>>(command, input)),
  );
}

/** A native HMI command whose refusal (`ok !== true`) is an error of the authoring operation. */
export async function nativeCall(
  token: vscode.CancellationToken,
  command: string,
  input: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  const result = await nativeRead(token, command, input);
  if (result.ok !== true) {
    throw new Error(`VALIDATION_FAILED: ${String(result.error ?? JSON.stringify(result.errors) ?? "Native HMI service refused the document")}`);
  }
  return result;
}

/** Calls the command and turns a rejected call (the language server is not running) into a refusal. */
async function answerOf(call: () => Promise<AuthoringResult>): Promise<AuthoringResult> {
  try {
    return await call();
  } catch (error) {
    if (error instanceof vscode.CancellationError) throw error;
    const message = error instanceof Error ? error.message : String(error);
    return {
      ok: false,
      code: "NOT_CONNECTED",
      message: `the truST language server did not answer: ${message}`,
      retryable: false,
    };
  }
}

/** How a request may be ended by cancellation: a read ends when the token is cancelled; a mutation is refused before it
 * starts when the token is already cancelled, and is awaited once started; a receipt always records its outcome (`receipt`). */
type Kind = "read" | "start" | "receipt";

/** Sends one request to a command, by its kind (`Kind`). */
async function invoke(
  project: EngineProject,
  command: string,
  request: Envelope,
  fields: Record<string, unknown>,
  kind: Kind,
): Promise<AuthoringResult> {
  const argument = {
    workspaceId: project.workspaceId,
    root_uri: project.root.toString(),
    storage: project.storage,
    open: openDocuments(project.root),
    remote: (await remoteVariableContext(project.root)).catalogue,
    selections: selectionsOf(project.root),
    simulatorSessions: simulatorSessions(project.root.toString()),
    request,
    ...fields,
  };
  const call = (): Promise<AuthoringResult> =>
    answerOf(() => Promise.resolve(vscode.commands.executeCommand<AuthoringResult>(command, argument)));
  if (kind === "read") return cancellableNativeRead(project.token, call);
  if (kind === "start" && project.token.isCancellationRequested) throw new vscode.CancellationError();
  return call();
}

/** `hmi_get_context`: the project context (AT-01). */
export function context(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.context", request, {}, "read");
}

/** `hmi_browse_library`: components and composition patterns (AT-02). */
export function library(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.library", request, {}, "read");
}

/** `hmi_inspect_page`: a page or a draft's page (AT-03). */
export function inspect(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.inspect", request, {}, "read");
}

/** `hmi_compose_page`: a new draft from constraints or a copy of a page (AT-04). */
export function compose(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.compose", request, {}, "start");
}

/** `hmi_edit_draft`: the typed operations on one page of a draft (AT-05). */
export function edit(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.edit", request, {}, "start");
}

/** `hmi_validate_draft`: the engineering checks and geometry and routing findings of a draft (AT-06). */
export function validate(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.validate", request, {}, "read");
}

/** The drafts of this workspace: `list`, one draft's pages with `read`, or `cleanup` of finished records. */
export function drafts(
  project: EngineProject,
  action: "list" | "read" | "cleanup",
  draftId?: string,
): Promise<AuthoringResult> {
  return invoke(
    project,
    "trust-lsp.hmiAgent.drafts",
    envelopeFor(project.workspaceId, {}),
    { action, draftId },
    action === "cleanup" ? "start" : "read",
  );
}

/** `hmi_apply_draft` with `review`: phase `prepare` answers the review, phase `record` records it after rendering. */
export function review(
  project: EngineProject,
  phase: "prepare" | "record",
  request: Envelope,
): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.review", request, { phase }, phase === "record" ? "start" : "read");
}

/** `hmi_apply_draft` with `apply`: phase `prepare` runs the preflight and answers the files to write, phase `receipt` records the outcome. */
export function apply(
  project: EngineProject,
  phase: "prepare" | "receipt",
  request: Envelope,
  receipt?: ApplyReceipt,
): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.apply", request, { phase, ...receipt }, phase === "receipt" ? "receipt" : "start");
}

/** `hmi_apply_draft` with `discard`: removes the draft's pages and keeps its identity. */
export function discard(project: EngineProject, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.discard", request, {}, "start");
}

/** The schema check of a tool's input, for the tools the extension renders (AT-07 and AT-08). */
export function checkInput(project: EngineProject, tool: string, request: Envelope): Promise<AuthoringResult> {
  return invoke(project, "trust-lsp.hmiAgent.checkInput", request, { tool }, "read");
}

/** The tool input of a request envelope; an envelope without an input object is left to the command to refuse. */
export function inputOf(request: unknown): Record<string, unknown> {
  const input = typeof request === "object" && request !== null ? (request as { input?: unknown }).input : undefined;
  return typeof input === "object" && input !== null && !Array.isArray(input) ? (input as Record<string, unknown>) : {};
}

/** The success of a command's answer. A refusal is thrown with its code, which the adapter reports unchanged. */
export function unwrap(answer: AuthoringResult): Extract<AuthoringResult, { ok: true }> {
  if (!answer.ok) throw new Error(`${answer.code}: ${answer.message}`);
  return answer;
}

/** A flow's answer: the data and revisions, the warnings, and the PNG images the answer carries in order. */
export interface FlowResult {
  data: Record<string, unknown>;
  revisions: Record<string, string>;
  warnings: string[];
  images: string[];
}
