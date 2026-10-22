/// <reference lib="dom" />
import { pollValue, httpJson, evaluateInWebview, type CdpConnection } from "./plant-rendered-harness";
export async function attachControlStudio(connection: CdpConnection, port: number, previousEditorSession = "", expectedLoopId = ""): Promise<string> {
  return pollValue("Control Studio webview", async () => {
    const targets = await httpJson(port, "/json");
    for (const target of targets.filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
      let sessionId: string | undefined;
      try {
        const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
        if (!sessionId)
          continue;
        await connection.send("Runtime.enable", {}, sessionId);
        const marker = await evaluateInWebview(connection, sessionId, `var root=d.querySelector('[data-control-studio]');return String(Boolean(root&&root.dataset.editorSessionId&&root.dataset.editorSessionId!==${JSON.stringify(previousEditorSession)}&&(!${JSON.stringify(expectedLoopId)}||root.dataset.loopId===${JSON.stringify(expectedLoopId)})));`);
        if (marker === "true")
          return sessionId;
        await connection.send("Target.detachFromTarget", { sessionId });
      }
      catch {
        if (sessionId)
          await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
    }
    return "";
  }, (value) => value.length > 0, 20000);
}

/** Attaches to the webview whose document contains the selector. */
export async function attachWebview(connection: CdpConnection, port: number, selector: string): Promise<string> {
  return pollValue(`webview with ${selector}`, async () => {
    for (const target of (await httpJson(port, "/json")).filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
      const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true }).catch(() => undefined);
      const sessionId = typeof attached?.sessionId === "string" ? attached.sessionId : undefined;
      if (!sessionId) continue;
      await connection.send("Runtime.enable", {}, sessionId);
      const found = await evaluateInWebview(connection, sessionId, `return String(Boolean(d.querySelector(${JSON.stringify(selector)})));`).catch(() => "false");
      if (found === "true") return sessionId;
      await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
    }
    return "";
  }, value => value.length > 0, 20000);
}

/** One engine answer as the page holds it: the status its receiver gives it, and the answer's fields a journey checks. */
export interface EngineAnswerRecord {
  kind: string;
  method?: string;
  status: string;
  command?: string;
  state?: string;
  operationId?: string;
  flowStage?: string;
  reason?: string;
  failure?: Record<string, unknown>;
  family?: string;
  values?: Record<string, number>;
  provisional?: boolean;
  applicationAction?: string;
  result?: Record<string, any>;
}

/**
 * Records, in the webview, every `control-studio.engine.*` envelope the host posts to the page, as `window.__engineAnswers`.
 * Each record carries the status that the page's receivers give the answer (`webview/engineOpened.ts`, `engineStages.ts` and
 * the stage receivers), because the engine route dispatches no `trust-control-studio-engineer` status events: a wait reads this
 * record, and the DOM checks confirm what the engineer sees. Self-contained (no names from outside), because
 * `engineAnswerRecorderScript` sends its source into the webview.
 */
export function installEngineAnswerRecorder(d: Document): void {
  const win = d.defaultView as (Window & { __engineAnswers?: EngineAnswerRecord[] }) | null;
  if (!win || win.__engineAnswers) return;
  const answers: EngineAnswerRecord[] = [];
  win.__engineAnswers = answers;
  const flowStageOf: Record<string, string> = { identify: "Identify", tune: "Tune", validate: "Validate" };
  /** The status of an Apply, Restore or Keep answer, by the kind the engine names it with (§0.1a, `webview/engineApplication.ts`). */
  const applicationStatus = (result: Record<string, any>): string => {
    switch (result.kind) {
      case "applicationReview":
      case "keepReview":
        return "review";
      case "applyOutcome": {
        const commissioning = result.result ?? {};
        return commissioning.status === "passed" ? "applied" : commissioning.status === "cancelled" ? "cancelled" : "failed";
      }
      case "restoreOutcome":
        return result.restored === true ? "commissioned" : "failed";
      case "keepResult":
        return "kept";
      default:
        return "failed";
    }
  };
  /** The status of an Identify answer, by the kind the engine names it with (§0.1a, `webview/engineStages.ts`). */
  const identifyStatus: Record<string, string> = { identification: "model-ready", reset: "reset", projectTest: "completed", manualIdentification: "manual" };
  const project = (message: Record<string, any>): EngineAnswerRecord => {
    const kind = String(message.kind);
    const update = message.update as Record<string, any> | undefined;
    if (kind === "control-studio.engine.opened") return { kind, status: "opened", result: message.result };
    if (kind === "control-studio.engine.update") {
      const command = String(update?.command ?? "");
      const base = { kind, method: command, command, state: String(update?.state ?? ""), operationId: update?.operationId,
        flowStage: typeof update?.stage === "string" ? update.stage : flowStageOf[command] };
      if (base.state === "running") return { ...base, status: "running", reason: typeof update?.progress === "string" ? update.progress : undefined };
      if (base.state === "cancelled") return { ...base, status: "cancelled" };
      if (base.state === "finished") return { ...base, status: "completed" };
      return { ...base, status: "failed", failure: update?.failure, reason: update?.failure?.message };
    }
    const method = typeof message.method === "string" ? message.method : undefined;
    if (kind === "control-studio.engine.failure") {
      return { kind, method, status: "failed", failure: message.failure, reason: message.failure?.message, flowStage: method ? flowStageOf[method] : undefined };
    }
    const result = (message.result ?? {}) as Record<string, any>;
    const base = { kind, method, result: message.result };
    switch (method) {
      case "identify":
        return result.kind === "identification"
          ? { ...base, status: "model-ready", provisional: result.provisional === true }
          : { ...base, status: identifyStatus[String(result.kind)] ?? "unknown" };
      case "tune":
        return { ...base, status: "candidate", family: result.candidate?.structure, values: result.settings };
      case "preview":
        return { ...base, status: "preview", reason: typeof result.reason === "string" ? result.reason : result.predictionUnavailable };
      case "validate":
        return { ...base, status: result.applyAvailable === true ? "verified" : "verification-failed", failure: result.failure };
      case "inputs":
        return { ...base, status: result.invalidated?.validation === true ? "edited" : "inputs" };
      case "run":
        return { ...base, status: result.needs ? "paused" : "completed" };
      case "apply":
      case "restore":
      case "keep":
        return { ...base, status: applicationStatus(result), applicationAction: method };
      default:
        return { ...base, status: "result" };
    }
  };
  win.addEventListener("message", (event: MessageEvent) => {
    const message = event.data as Record<string, any> | null;
    if (message && typeof message.kind === "string" && message.kind.startsWith("control-studio.engine.")) answers.push(project(message));
  });
}

/** The webview script that installs the recorder. Send it once the page has loaded, and again after each reopen (a new document). */
export function engineAnswerRecorderScript(): string {
  return `(${installEngineAnswerRecorder.toString()})(d);return 'engine answers recorded';`;
}
