// Named HMI simulator journeys (specification 43, AT-08). The language server keeps the sessions and runs the journeys
// (`trust-lsp.hmiAgent.openSimulatorSession`, `closeSimulatorSession`, `admitSimulatorLogin`, `runSimulatorJourney`,
// `cancelSimulatorJourney`); a journey answers its start at once and reports its end as the notification
// `trust-lsp/simulatorJourneyEnded`. The extension keeps what the language server cannot hold: the prompts, the named
// login through the HMI bridge, and each session's cookie, which it hands over with each journey of that session
// (specification 39 §11 item 6).
import * as vscode from "vscode";
import { State } from "vscode-languageclient/node";
import type { LspClientProvider } from "../../lm-tools/shared";
import { getTrustConfiguration } from "../../configuration";
import { onDidChangeSelectedRuntime } from "../../selectedRuntime";
import { LiveSession, LiveEndpoint, LiveSessionDeps } from "../editor/liveSession";
import { runtimeLiveOperatorDeps } from "../editor/liveOperatorRuntime";
import type { HmiPageModel } from "../drawing/model";
import { AuthoringResult, unwrap } from "./engineCommands";

const JOURNEY_ENDED = "trust-lsp/simulatorJourneyEnded";

/** A session this window logged into: the HMI bridge that holds its cookie, the selected target it reads, and the
 * timer that drops it when the language server's expiry comes. */
interface HeldSession {
  live: LiveSession;
  target: () => Promise<LiveEndpoint | undefined>;
  expiry: ReturnType<typeof setTimeout>;
}
const held = new Map<string, HeldSession>();

/** The tool calls waiting for their journey's end, by session ID, the first first: a session runs one journey at a
 * time, so each end belongs to the earliest call still waiting. */
interface Waiter {
  resolve: (answer: Record<string, unknown>) => void;
  reject: (error: Error) => void;
}
const waiting = new Map<string, Waiter[]>();
let languageClient: LspClientProvider = () => undefined;
const listened = new WeakSet<object>();

/** Lets the journeys listen on the language client that `getClient` gives; answers what restores the previous one. */
export function setSimulatorLanguageClient(getClient: LspClientProvider): () => void {
  const previous = languageClient;
  languageClient = getClient;
  return () => {
    languageClient = previous;
  };
}

/** Whether the language client's trace logs each message's parameters, a journey's cookie among them: `verbose` in
 * `trust.languageServer.trace`, which the extension sets the trace from, or `verbose` or `compact` in the client's own
 * `trust-lsp.trace.server` (vscode-jsonrpc logs the parameters at both). */
function traceLogsParameters(): boolean {
  const logs = (value: unknown): boolean => {
    const level = typeof value === "object" && value !== null ? (value as { verbosity?: unknown }).verbosity : value;
    return level === "verbose" || level === "compact";
  };
  return (
    logs(getTrustConfiguration().get("trace.server")) ||
    logs(vscode.workspace.getConfiguration("trust-lsp").get("trace.server"))
  );
}

/** The target as the language server reads it. */
function targetOf(target: LiveEndpoint | undefined): Record<string, unknown> | null {
  return target
    ? { simulator: target.simulator, url: target.url, endpoint: target.endpoint, token: target.token ?? null }
    : null;
}

function drop(id: string): void {
  const session = held.get(id);
  if (!session) return;
  held.delete(id);
  clearTimeout(session.expiry);
  session.live.stop();
}

/** Ends every session this window holds, dropping their cookies, and every wait for a journey's end. */
export function endSimulatorSessions(): void {
  for (const id of [...held.keys()]) drop(id);
  stopWaiting("NOT_CONNECTED: the editor ended the simulator journey's session.");
}

/** Closes this window's sessions in the language server, which cancels their journeys, after the selected run target
 * changed (specification 39 §11 item 6: a target change invalidates the session). */
export async function closeSimulatorSessions(): Promise<void> {
  const ids = [...held.keys()];
  for (const id of ids) drop(id);
  await Promise.allSettled(
    ids.map((sessionId) =>
      vscode.commands.executeCommand("trust-lsp.hmiAgent.closeSimulatorSession", { sessionId }),
    ),
  );
}

function stopWaiting(reason: string): void {
  const waiters = [...waiting.values()].flat();
  waiting.clear();
  for (const waiter of waiters) waiter.reject(new Error(reason));
}

function forget(sessionId: string, waiter: Waiter): void {
  const rest = (waiting.get(sessionId) ?? []).filter((other) => other !== waiter);
  if (rest.length) waiting.set(sessionId, rest);
  else waiting.delete(sessionId);
}

/** Hands the journey's answer to the earliest tool call that waits for its session. */
function journeyEnded(answer: unknown): void {
  const sessionId = (answer as { sessionId?: unknown } | undefined)?.sessionId;
  const waiter = typeof sessionId === "string" ? waiting.get(sessionId)?.[0] : undefined;
  if (!waiter) return;
  forget(sessionId as string, waiter);
  waiter.resolve(answer as Record<string, unknown>);
}

/** Listens for journeys' ends on the language client, and gives up the waits when it stops. */
function listen(): void {
  const client = languageClient();
  if (!client) throw new Error("NOT_CONNECTED: the truST language server is not running.");
  if (listened.has(client)) return;
  listened.add(client);
  client.onNotification(JOURNEY_ENDED, journeyEnded);
  client.onDidChangeState((change) => {
    if (change.newState === State.Stopped) stopWaiting("NOT_CONNECTED: the truST language server stopped.");
  });
}

/** Connects a named simulator journey: the language server opens the session for the selected target, the HMI bridge
 * logs in to that target with the user's `credentials` and keeps the cookie. The login goes only to a target the
 * language server admits as the session's own. A login that is cancelled or that the simulator refuses closes the
 * session. */
export async function connectSimulator(
  workspace: string,
  name: string,
  deps: LiveSessionDeps,
  credentials: () => Promise<{ user: string; password: string } | undefined>,
): Promise<{ sessionId: string; name: string; expiresAt: string } | undefined> {
  const opened = unwrap(
    await vscode.commands.executeCommand<AuthoringResult>("trust-lsp.hmiAgent.openSimulatorSession", {
      workspace,
      name,
      target: targetOf(await deps.target()),
    }),
  ).data as { sessionId: string; name: string; expiresAt: string };
  const close = async (): Promise<void> => {
    await vscode.commands.executeCommand("trust-lsp.hmiAgent.closeSimulatorSession", { sessionId: opened.sessionId });
  };
  let live: LiveSession | undefined;
  try {
    const login = await credentials();
    if (login) {
      live = new LiveSession({
        ...deps,
        target: async () => {
          const current = await deps.target();
          const answer = await vscode.commands.executeCommand<AuthoringResult>(
            "trust-lsp.hmiAgent.admitSimulatorLogin",
            { sessionId: opened.sessionId, target: targetOf(current) },
          );
          return answer?.ok && answer.data.admitted === true ? current : undefined;
        },
      });
      const answer = await live.request("POST", "/api/hmi/login", login);
      if (answer.status !== 200) throw new Error("The simulator refused the named HMI login.");
    }
  } catch (error) {
    live?.stop();
    await close();
    throw error;
  }
  if (!live) {
    await close();
    return undefined;
  }
  const expiry = setTimeout(() => drop(opened.sessionId), Math.max(0, Date.parse(opened.expiresAt) - Date.now()));
  held.set(opened.sessionId, { live, target: () => deps.target(), expiry });
  return opened;
}

/** The operator explicitly selects and logs into an already-running local simulator. */
export function registerSimulatorSessions(context: vscode.ExtensionContext, getClient?: LspClientProvider): void {
  setSimulatorLanguageClient(getClient ?? (() => undefined));
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.hmiBuilder.connectSimulator", async () => {
      const folders = vscode.workspace.workspaceFolders ?? [];
      const folder =
        folders.length === 1
          ? folders[0]
          : (
              await vscode.window.showQuickPick(
                folders.map((folder) => ({ label: folder.name, folder })),
                { title: "Choose the HMI simulation project" },
              )
            )?.folder;
      if (!folder) return;
      const deps = runtimeLiveOperatorDeps(folder.uri.fsPath, true).session;
      const name = await vscode.window.showInputBox({
        title: "Name this HMI simulation journey",
        prompt: "This session permits bounded HMI commands only on the selected local simulator.",
        value: (await deps.target())?.label,
      });
      if (!name) return;
      const opened = await connectSimulator(folder.uri.toString(), name, deps, async () => {
        const user = await vscode.window.showInputBox({ title: "Simulator HMI user" });
        if (!user) return undefined;
        const password = await vscode.window.showInputBox({ title: "Simulator HMI password", password: true });
        return password === undefined ? undefined : { user, password };
      });
      if (!opened) return;
      await vscode.window.showInformationMessage(
        `Simulator journey “${opened.name}” is available for 10 minutes. Agent context lists its session ID.`,
      );
      return { sessionId: opened.sessionId, name: opened.name };
    }),
    onDidChangeSelectedRuntime(() => void closeSimulatorSessions()),
    { dispose: endSimulatorSessions },
  );
}

/** Runs a journey in the language server with the cookie of its named session and the selected target, and waits for
 * its answer, which the language server reports when the journey ends. Cancelling `signal` before the start goes out
 * starts nothing; cancelling it later cancels the journey there, also when the cancel overtakes the start, and the
 * journey's answer still comes back, with the outcomes its steps completed. */
export async function runSimulator(
  workspace: string,
  sessionId: string,
  steps: unknown,
  models: Record<string, HmiPageModel>,
  signal: AbortSignal,
): Promise<Record<string, unknown>> {
  // Listening before any await, so that no cancel is lost; only a start that went out is cancelled.
  let started = false;
  const cancel = (): void => {
    if (started) void vscode.commands.executeCommand("trust-lsp.hmiAgent.cancelSimulatorJourney", { sessionId });
  };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    if (signal.aborted) throw new vscode.CancellationError();
    if (traceLogsParameters())
      throw new Error(
        "UNSUPPORTED_CONTEXT: the language server trace logs each message's parameters, which would log the named session's cookie; set trust.languageServer.trace to off or messages to run a simulator journey.",
      );
    listen();
    const session = held.get(sessionId);
    const target = await session?.target();
    if (signal.aborted) throw new vscode.CancellationError();
    let waiter: Waiter | undefined;
    const ended = new Promise<Record<string, unknown>>((resolve, reject) => {
      waiter = { resolve, reject };
      waiting.set(sessionId, [...(waiting.get(sessionId) ?? []), waiter]);
    });
    // A wait given up before the start answers is reported by the start itself.
    ended.catch(() => undefined);
    started = true;
    try {
      unwrap(
        await vscode.commands.executeCommand<AuthoringResult>("trust-lsp.hmiAgent.runSimulatorJourney", {
          workspace,
          sessionId,
          cookie: session?.live.namedCookie() ?? null,
          target: targetOf(target),
          steps,
          models,
        }),
      );
    } catch (error) {
      if (waiter) forget(sessionId, waiter);
      throw error;
    }
    return await ended;
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}
