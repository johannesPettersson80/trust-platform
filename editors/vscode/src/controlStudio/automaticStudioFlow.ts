import { randomUUID } from "crypto";
import type { LoopMessageScope } from "./messages";

export type AutomaticStudioStage = "Identify" | "Tune" | "Validate";
type FlowPayload = Record<string, unknown>;
type StageHandler = (message: unknown, scope: LoopMessageScope, post: (payload: FlowPayload) => Promise<void>) => Promise<boolean>;

interface FlowStartPayload extends FlowPayload {
  from?: "find" | "recording" | "tune" | "verify";
  json?: string;
  acquisitionSettings?: FlowPayload;
  task?: FlowPayload;
  family?: string;
  applicationMode?: "auto" | "manual";
  pinnedValues?: Record<string, number>;
  tuningPreferences?: FlowPayload;
}

/** Coordinates the existing EngineerWorkflowSession stages without owning their algorithms. */
export class AutomaticStudioFlow {
  private generation = 0;
  private activeStage: AutomaticStudioStage | undefined;
  private readonly handledMessages = new Set<string>();
  private disposed = false;

  constructor(private readonly handleStage: StageHandler, private readonly savedTask: () => FlowPayload | undefined = () => undefined) {}

  get running(): boolean { return this.activeStage !== undefined; }

  async handle(message: unknown, scope: LoopMessageScope, post: (stage: string, payload: FlowPayload) => Promise<void>): Promise<boolean> {
    if (this.disposed) return false;
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId ||
        message.editorSessionId !== scope.editorSessionId || message.loopId !== scope.loopId ||
        message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string" ||
        !record(message.payload) || (message.kind !== "control-studio.flow.start" && message.kind !== "control-studio.flow.cancel")) return false;
    if (this.handledMessages.has(message.messageId)) {
      await post("flow", { flowStage: this.activeStage ?? "Identify", status: "busy", state: "running", reason: "This flow request has already been handled; no stage was repeated." });
      return true;
    }
    if (this.handledMessages.size >= 4096) {
      await post("flow", { flowStage: this.activeStage ?? "Identify", status: "busy", state: "running", reason: "Session request limit reached. Reopen the panel before starting new work." });
      return true;
    }
    this.handledMessages.add(message.messageId);
    if (message.kind === "control-studio.flow.cancel") {
      const cancelledStage = this.activeStage ?? "Identify";
      const generation = ++this.generation;
      if (this.activeStage) {
        const cancelled = await this.invokeEngineerCancel(scope, post, generation);
        if (!cancelled) return true;
        this.activeStage = undefined;
      }
      if (generation === this.generation && !this.disposed) await post("flow", { flowStage: cancelledStage, status: "paused", state: "paused", reason: "Automatic flow aborted." });
      return true;
    }
    if (this.activeStage) {
      await post("flow", { flowStage: this.activeStage, status: "busy", state: "running", reason: "An automatic flow is already running. Cancel it before starting another." });
      return true;
    }
    const generation = ++this.generation;
    const payload = message.payload as FlowStartPayload;
    const from = payload.from ?? "find";
    if (from !== "find" && from !== "recording" && from !== "tune" && from !== "verify") {
      await post("flow", { flowStage: "Identify", status: "paused", state: "paused", reason: "Choose a valid automatic flow stage." });
      return true;
    }
    const task = { ...this.savedTask(), ...(record(payload.task) ? payload.task : {}) };
    const stages: Array<{ name: AutomaticStudioStage; action: "find" | "recording" | "tune" | "verify" }> =
      from === "find" || from === "recording" ? [{ name: "Identify", action: from }] :
      from === "tune" ? [{ name: "Tune", action: "tune" }] :
      [{ name: "Validate", action: "verify" }];
    for (const stage of stages) {
      if (generation !== this.generation) return true;
      this.activeStage = stage.name;
      const stagePayload: FlowPayload = stage.action === "recording" ? { json: payload.json,
        ...(payload.recordingSelection ? { recordingSelection: payload.recordingSelection } : {}) } : stage.action === "find" ?
        { reuseRecordings: false, ...(payload.acquisitionSettings ? { acquisitionSettings: payload.acquisitionSettings } : {}) } :
        stage.action === "tune" ? { ...task, automaticRequirements: true, ...(payload.family ? { family: payload.family } : {}),
          applicationMode: payload.applicationMode ?? "auto", ...(payload.pinnedValues !== undefined ? { pinnedValues: payload.pinnedValues } : {}),
          ...(payload.tuningPreferences ? { tuningPreferences: payload.tuningPreferences } : {}) } : {};
      await post("engineer", { flowStage: stage.name, status: "running", reason: `Starting ${stage.name}.` });
      let last: FlowPayload | undefined;
      try {
        const handled = await this.handleStage({ schemaVersion: 1, kind: `control-studio.engineer.${stage.action}`,
          messageId: `automatic-flow-${randomUUID()}`, projectId: scope.projectId,
          editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
          payload: stagePayload }, scope, async update => {
          if (generation !== this.generation || this.disposed) return;
          last = update;
          await post("engineer", { ...update, flowStage: stage.name });
        });
        if (!handled) throw new Error("No configured analysis session is available.");
        if (generation !== this.generation) return true;
        if (last?.status === "recording-needed") {
          await post("flow", { ...last, flowStage: "Identify", status: "paused", missing: "recording" });
          this.activeStage = undefined;
          return true;
        }
        if (last?.status === "failed" || last?.status === "verification-failed" || last?.status === "unavailable") {
          await post("flow", { ...last, flowStage: stage.name, status: "paused", state: "paused", reason: String(last.reason ?? `${stage.name} failed.`) });
          this.activeStage = undefined;
          return true;
        }
        if ((stage.action === "find" || stage.action === "recording") && last?.status !== "model-ready") throw new Error("Identify did not produce a selected model.");
        if (stage.action === "tune" && last?.status !== "candidate") throw new Error("Tune did not produce a controller candidate.");
        if (stage.action === "verify" && last?.status !== "verified") throw new Error("Validate did not pass the selected candidate.");
      } catch (error) {
        if (generation !== this.generation || this.disposed) return true;
        await post("flow", { flowStage: stage.name, status: "paused", state: "paused", reason: error instanceof Error ? error.message : String(error) });
        this.activeStage = undefined;
        return true;
      }
    }
    if (generation !== this.generation || this.disposed) return true;
    this.activeStage = undefined;
    const completedStage = stages[stages.length - 1].name;
    await post("flow", { flowStage: completedStage, status: "completed", state: "completed",
      reason: completedStage === "Identify" ? "Model ready for review." : "Automatic flow completed; Apply remains a separate user action." });
    return true;
  }

  private async invokeEngineerCancel(scope: LoopMessageScope, post: (stage: string, payload: FlowPayload) => Promise<void>, generation: number): Promise<boolean> {
    const stage = this.activeStage ?? "Identify";
    let last: FlowPayload | undefined;
    const handled = await this.handleStage({ schemaVersion: 1, kind: "control-studio.engineer.cancel", messageId: `automatic-flow-${randomUUID()}`,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload: {} }, scope,
    async update => {
      if (generation !== this.generation || this.disposed) return;
      last = update;
      await post("engineer", { ...update, flowStage: stage });
    });
    if (!handled || last?.status === "recovery-required" || last?.cancellationUnconfirmed === true) {
      await post("flow", { flowStage: stage, status: "paused", state: "paused", recoveryRequired: true,
        reason: String(last?.reason ?? "Cancellation requires recovery confirmation before continuing.") });
      return false;
    }
    return true;
  }

  dispose(): void {
    this.disposed = true;
    this.generation++;
    this.activeStage = undefined;
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
