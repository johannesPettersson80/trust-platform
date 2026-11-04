// Named HMI simulator journeys (specification 43, AT-08). The language server keeps the sessions and runs the journeys
// (`trust-lsp.hmiAgent.openSimulatorSession`, `closeSimulatorSession`, `runSimulatorJourney`, `cancelSimulatorJourney`).
// The extension keeps what the language server cannot hold: the prompts, the named login through the HMI bridge, and
// each session's cookie, which it hands over with each journey of that session (specification 39 §11 item 6).
import * as vscode from "vscode";
import { LiveSession, LiveEndpoint, LiveSessionDeps } from "../editor/liveSession";
import { runtimeLiveOperatorDeps } from "../editor/liveOperatorRuntime";
import type { HmiPageModel } from "../drawing/model";
import { AuthoringResult, unwrap } from "./engineCommands";

/** A session this window logged into: the HMI bridge that holds its cookie, the selected target it reads, and the
 * timer that drops it when the language server's expiry comes. */
interface HeldSession {
  live: LiveSession;
  target: () => Promise<LiveEndpoint | undefined>;
  expiry: ReturnType<typeof setTimeout>;
}
const held = new Map<string, HeldSession>();

/** Whether trust-lsp accepts the target for a simulator journey: a simulator whose web address and control endpoint are
 * on this computer (`trust-lsp.hmiAgent.checkTarget`, specification 43 AT-08). Without an answer it is refused. */
export async function isLocalSimulator(target: LiveEndpoint | undefined): Promise<boolean> {
  try {
    const answer = await vscode.commands.executeCommand<{ local?: unknown } | undefined>(
      "trust-lsp.hmiAgent.checkTarget",
      target ? { simulator: target.simulator, url: target.url, endpoint: target.endpoint } : null,
    );
    return answer?.local === true;
  } catch {
    return false;
  }
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

/** Ends every session this window holds, dropping their cookies. */
export function endSimulatorSessions(): void {
  for (const id of [...held.keys()]) drop(id);
}

/** Connects a named simulator journey: the language server opens the session for the selected target, the HMI bridge
 * logs in to that target with the user's `credentials` and keeps the cookie. A login that is cancelled or that the
 * simulator refuses closes the session. */
export async function connectSimulator(
  workspace: string,
  name: string,
  deps: LiveSessionDeps,
  credentials: () => Promise<{ user: string; password: string } | undefined>,
): Promise<{ sessionId: string; name: string; expiresAt: string } | undefined> {
  const target = await deps.target();
  const opened = unwrap(
    await vscode.commands.executeCommand<AuthoringResult>("trust-lsp.hmiAgent.openSimulatorSession", {
      workspace,
      name,
      target: targetOf(target),
    }),
  ).data as { sessionId: string; name: string; expiresAt: string };
  const close = async (): Promise<void> => {
    await vscode.commands.executeCommand("trust-lsp.hmiAgent.closeSimulatorSession", { sessionId: opened.sessionId });
  };
  let live: LiveSession | undefined;
  try {
    const login = await credentials();
    if (login) {
      // The login goes only to the target the session was opened for, while trust-lsp still accepts it.
      live = new LiveSession({
        ...deps,
        target: async () => {
          const current = await deps.target();
          return (await isLocalSimulator(current)) &&
            current?.endpoint === target?.endpoint &&
            current?.url === target?.url
            ? current
            : undefined;
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
export function registerSimulatorSessions(context: vscode.ExtensionContext): void {
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
  );
  context.subscriptions.push({ dispose: endSimulatorSessions });
}

/** Runs a journey in the language server with the cookie of its named session and the selected target. Cancelling
 * `signal` cancels the journey there; its answer still comes back, with the outcomes its steps completed. */
export async function runSimulator(
  workspace: string,
  sessionId: string,
  steps: unknown,
  models: Record<string, HmiPageModel>,
  signal: AbortSignal,
): Promise<Record<string, unknown>> {
  if (signal.aborted) throw new vscode.CancellationError();
  const session = held.get(sessionId);
  const target = await session?.target();
  const cancel = (): void => {
    void vscode.commands.executeCommand("trust-lsp.hmiAgent.cancelSimulatorJourney", { sessionId });
  };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    const answer = await vscode.commands.executeCommand<AuthoringResult>("trust-lsp.hmiAgent.runSimulatorJourney", {
      workspace,
      sessionId,
      cookie: session?.live.namedCookie() ?? null,
      target: targetOf(target),
      steps,
      models,
    });
    return unwrap(answer).data;
  } finally {
    signal.removeEventListener("abort", cancel);
  }
}
