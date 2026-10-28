import * as vscode from "vscode";
import { randomUUID } from "crypto";
import { LiveSession, LiveEndpoint } from "../editor/liveSession";
import { runtimeLiveOperatorDeps } from "../editor/liveOperatorRuntime";
import { sameModel } from "../editor/pageDocument";
import type { HmiPageModel } from "../drawing/model";

export interface SimulatorJourney {
  id: string;
  name: string;
  session: Pick<LiveSession, "request" | "openSocket" | "isCurrentTarget">;
  expires: number;
  busy: boolean;
}
interface NamedSimulator extends SimulatorJourney {
  workspace: string;
  endpoint: string;
  session: LiveSession;
}
const sessions = new Map<string, NamedSimulator>();
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
export function simulatorSessions(workspace: string): Array<{ sessionId: string; name: string; expiresAt: string }> {
  for (const [id, session] of sessions)
    if (session.expires < Date.now()) {
      session.session.stop();
      sessions.delete(id);
    }
  return [...sessions.values()]
    .filter((session) => session.workspace === workspace)
    .map((session) => ({
      sessionId: session.id,
      name: session.name,
      expiresAt: new Date(session.expires).toISOString(),
    }));
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
      const target = await deps.target();
      if (!(await isLocalSimulator(target)))
        throw new Error(
          "Start a local simulator with an HMI web endpoint before connecting a simulation journey. Physical or remote targets are refused.",
        );
      simulatorSessions(folder.uri.toString());
      if (sessions.size >= 5)
        throw new Error("Five simulator sessions already exist; wait for one to expire or restart the editor.");
      const name = await vscode.window.showInputBox({
        title: "Name this HMI simulation journey",
        prompt: "This session permits bounded HMI commands only on the selected local simulator.",
        value: target!.label,
      });
      if (!name) return;
      const user = await vscode.window.showInputBox({ title: "Simulator HMI user" });
      if (!user) return;
      const password = await vscode.window.showInputBox({ title: "Simulator HMI password", password: true });
      if (password === undefined) return;
      const guarded = {
        ...deps,
        target: async () => {
          const current = await deps.target();
          return (await isLocalSimulator(current)) &&
            current?.endpoint === target!.endpoint &&
            current?.url === target!.url
            ? current
            : undefined;
        },
      };
      const session = new LiveSession(guarded);
      const answer = await session.request("POST", "/api/hmi/login", { user, password });
      if (answer.status !== 200) {
        session.stop();
        throw new Error("The simulator refused the named HMI login.");
      }
      const id = randomUUID();
      sessions.set(id, {
        id,
        workspace: folder.uri.toString(),
        name,
        endpoint: target!.endpoint,
        session,
        expires: Date.now() + 10 * 60_000,
        busy: false,
      });
      await vscode.window.showInformationMessage(
        `Simulator journey “${name}” is available for 10 minutes. Agent context lists its session ID.`,
      );
      return { sessionId: id, name };
    }),
  );
  context.subscriptions.push({
    dispose() {
      for (const session of sessions.values()) session.session.stop();
      sessions.clear();
    },
  });
}
export interface SimulatorStep {
  page: string;
  object?: string;
  action?: string;
  value?: boolean | number | string;
  expect?: boolean | number | string;
  path?: string;
}
/** Execute against deployed matching pages; a draft is never silently deployed for a journey. */
export async function runSimulator(
  workspace: string,
  id: string,
  steps: SimulatorStep[],
  models: Record<string, HmiPageModel>,
  signal: AbortSignal,
): Promise<Record<string, unknown>> {
  simulatorSessions(workspace);
  const owner = sessions.get(id);
  if (!owner || owner.workspace !== workspace)
    throw new Error("EXPIRED_DRAFT: select an unexpired named simulator session in this workspace.");
  return executeSimulatorJourney(owner, steps, models, signal);
}
/** Bounded journey engine shared by named sessions and transport-level regression fixtures. */
export async function executeSimulatorJourney(
  owner: SimulatorJourney,
  steps: SimulatorStep[],
  models: Record<string, HmiPageModel>,
  signal: AbortSignal,
): Promise<Record<string, unknown>> {
  const id = owner.id;
  if (owner.busy) throw new Error("CONFLICT: this simulator journey is already running.");
  if (!steps.length || steps.length > 12) throw new Error("LIMIT_EXCEEDED: use 1 to 12 bounded simulator steps.");
  const outcomes: Array<Record<string, unknown>> = [];
  owner.busy = true;
  try {
    for (const step of steps) {
      if (signal.aborted)
        throw new Error(
          "CANCELLED: simulator journey cancelled; completed commands are retained in outcomes and are not undone.",
        );
      if (Date.now() > owner.expires || !(await owner.session.isCurrentTarget()))
        throw new Error("CONFLICT: the simulator target or named session changed.");
      const page = await owner.session.request("GET", `/api/hmi/page?name=${encodeURIComponent(step.page)}`);
      const data = page.body as { page?: { model: HmiPageModel }; bundle?: string };
      if (page.status !== 200 || !data.page || !sameModel(models[step.page], data.page.model))
        throw new Error(
          "CONFLICT: the deployed simulator page does not match this draft; apply and deploy explicitly before exercising it.",
        );
      const stream = await owner.session.openSocket(`/ws/hmi?page=${encodeURIComponent(step.page)}`);
      if (!stream) throw new Error("NOT_CONNECTED: simulator readback stream unavailable.");
      let disconnected = false;
      const values: Record<string, { value: unknown }> = {};
      const commands = new Map<string, Record<string, unknown>>();
      stream.onclose = () => {
        disconnected = true;
      };
      stream.onmessage = (event) => {
        try {
          const frame = JSON.parse(String(event.data));
          Object.assign(values, frame.values ?? {});
          if (frame.command?.id) commands.set(frame.command.id, frame.command);
        } catch {
          disconnected = true;
        }
      };
      try {
        let commandId: string | undefined;
        if (step.object) {
          const object = data.page.model.object.find((object) => object.id === step.object);
          if (!object) throw new Error("INVALID_ARGUMENT: simulator step names an unknown control.");
          commandId = randomUUID();
          const reply = await owner.session.request("POST", "/api/hmi/command", {
            id: commandId,
            bundle: data.bundle,
            page: step.page,
            object: step.object,
            action: step.action,
            value: step.value,
          });
          const answer = reply.body as Record<string, unknown>;
          outcomes.push({
            page: step.page,
            object: step.object,
            commandId,
            status: reply.status,
            outcome: answer.outcome,
            readback: answer.readback,
            reason: answer.reason,
          });
          if (reply.status !== 200 || !["applied", "pending", "confirmed"].includes(String(answer.outcome))) break;
          commands.set(commandId, answer);
        }
        const deadline = Date.now() + 5000;
        while (!signal.aborted && !disconnected && Date.now() < deadline) {
          const confirmed = !commandId || commands.get(commandId)?.outcome === "confirmed";
          const readback = step.path ? values[step.path]?.value : undefined;
          if (
            confirmed &&
            (step.path === undefined ||
              (Object.prototype.hasOwnProperty.call(values, step.path) &&
                (step.expect === undefined || readback === step.expect)))
          ) {
            outcomes.push({ page: step.page, outcome: "observed", commandId, path: step.path, value: readback });
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 50));
        }
        if (signal.aborted || disconnected || Date.now() >= deadline) {
          outcomes.push({
            page: step.page,
            outcome: signal.aborted ? "cancelled" : disconnected ? "disconnected" : "timed-out",
            commandId,
          });
          break;
        }
      } finally {
        stream.close();
      }
    }
    return {
      origin: "simulator",
      sessionId: id,
      name: owner.name,
      outcomes,
      passed: outcomes.length > 0 && outcomes[outcomes.length - 1].outcome === "observed",
    };
  } catch (error) {
    return { origin: "simulator", sessionId: id, name: owner.name, outcomes, passed: false, error: String(error) };
  } finally {
    owner.busy = false;
  }
}
