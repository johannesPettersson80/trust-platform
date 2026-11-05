// Credentials on the language server's channel (specification 25 §6.4). The language server names, in its `initialize`
// answer, the request fields that carry a credential and the command answers that carry one; the extension refuses
// such a command while its language client's trace logs each message's parameters, before anything is sent, so that
// no trace logs a runtime's token, a control token or a session's cookie.
import * as vscode from "vscode";
import { getTrustConfiguration } from "./configuration";

/** The credentials the language server names (`capabilities.experimental.credentials`). */
export interface CredentialNames {
  /** The request fields that carry a credential. */
  fields: string[];
  /** The commands, by their action, whose answer carries a credential. */
  answers: { command: string; action?: string }[];
}

/** The refusal of a command that carries a credential while the trace logs message parameters. */
export const CREDENTIAL_TRACE_REFUSAL =
  "UNSUPPORTED_CONTEXT: the language server trace logs each message's parameters, which would log a credential; set trust.languageServer.trace to off or messages, and trust-lsp.trace.server to off or messages in text format, to use this.";

/** Whether the language client's trace logs each message's parameters, credentials among them: `verbose` in
 * `trust.languageServer.trace`, which the extension sets the trace from, or `verbose` or `compact` in the client's own
 * `trust-lsp.trace.server` (vscode-jsonrpc logs the parameters at both), or that setting's `json` format, at which
 * vscode-jsonrpc logs whole messages at any level (vscode-languageclient traces `off` as `messages` while its output
 * channel logs at Trace). */
export function traceLogsParameters(): boolean {
  const own = vscode.workspace.getConfiguration("trust-lsp").get<unknown>("trace.server");
  const object = typeof own === "object" && own !== null ? (own as { verbosity?: unknown; format?: unknown }) : undefined;
  const logs = (value: unknown): boolean => value === "verbose" || value === "compact";
  return (
    object?.format === "json" ||
    logs(object ? object.verbosity : own) ||
    logs(getTrustConfiguration().get("trace.server"))
  );
}

/** The credentials named in an `initialize` answer's experimental capabilities, or undefined when it names none. */
export function credentialNamesOf(experimental: unknown): CredentialNames | undefined {
  const named = (experimental as { credentials?: unknown } | undefined)?.credentials as
    | { fields?: unknown; answers?: unknown }
    | undefined;
  if (!named || !Array.isArray(named.fields) || !Array.isArray(named.answers)) return undefined;
  return {
    fields: named.fields.filter((field): field is string => typeof field === "string"),
    answers: named.answers.filter(
      (answer): answer is { command: string; action?: string } =>
        typeof (answer as { command?: unknown })?.command === "string",
    ),
  };
}

/** Whether `value` holds a named credential field, at any depth: a field that carries a credential holds non-blank
 * text (specification 25 §6.4), so `token: true` or an empty token does not. */
function holdsCredential(value: unknown, fields: ReadonlySet<string>): boolean {
  if (Array.isArray(value)) return value.some((item) => holdsCredential(item, fields));
  if (typeof value !== "object" || value === null) return false;
  return Object.entries(value).some(
    ([key, field]) =>
      (fields.has(key) && typeof field === "string" && field.trim() !== "") || holdsCredential(field, fields),
  );
}

/** Whether `command` with `args` sends a named credential or is answered with one. */
export function carriesCredential(command: string, args: unknown[], names: CredentialNames): boolean {
  const action = (args[0] as { action?: unknown } | undefined)?.action;
  const answered = names.answers.some(
    (answer) => answer.command === command && (answer.action === undefined || answer.action === action),
  );
  return answered || holdsCredential(args, new Set(names.fields));
}

/** The refusal of a language-model tool's command that carries or answers a credential: a model never receives one. */
export const MODEL_CREDENTIAL_REFUSAL =
  "UNSUPPORTED_CONTEXT: this command sends or answers a credential, which a language-model tool never does.";

/** When the extension last told the user of a refusal: at most once a minute, since Live Values and the canvas ask
 * again and again. */
let toldAt = 0;
const TELL_EVERY_MS = 60_000;

/** The refusal of a command that carries a credential while the trace logs parameters: the error for the caller, and a
 * warning with the setting to change for the user, since some callers fall back to what they do without an answer. */
export function credentialRefusal(): Error {
  const now = Date.now();
  if (now - toldAt >= TELL_EVERY_MS) {
    toldAt = now;
    void vscode.window
      .showWarningMessage(CREDENTIAL_TRACE_REFUSAL, "Open Settings")
      .then((choice) =>
        choice ? vscode.commands.executeCommand("workbench.action.openSettings", "trust.languageServer.trace") : undefined,
      );
  }
  return new Error(CREDENTIAL_TRACE_REFUSAL);
}
