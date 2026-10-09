import type { EngineerWorkflowSession } from "./engineerWorkflowSession";
import type { LoopMessageScope } from "./messages";

export type ReviewedStudioApplicationAction = "apply" | "restore";

export interface ReviewedStudioApplicationCommand {
  action: ReviewedStudioApplicationAction;
  messageId: string;
}

export function decodeReviewedStudioApplicationCommand(value: unknown,
  expected: LoopMessageScope): ReviewedStudioApplicationCommand | undefined {
  if (!record(value) || !record(value.payload) || JSON.stringify(value).length > 16_000 ||
      Object.keys(value).sort().join(",") !== "editorSessionId,expectedLoopRevision,kind,loopId,messageId,payload,projectId,schemaVersion" ||
      value.schemaVersion !== 1 || typeof value.messageId !== "string" || value.messageId.length === 0 ||
      value.projectId !== expected.projectId || value.editorSessionId !== expected.editorSessionId ||
      value.loopId !== expected.loopId || value.expectedLoopRevision !== expected.loopRevision ||
      Object.keys(value.payload).length !== 0) return undefined;
  if (value.kind === "control-studio.application.apply") return { action: "apply", messageId: value.messageId };
  if (value.kind === "control-studio.application.restore") return { action: "restore", messageId: value.messageId };
  return undefined;
}

export interface ReviewedStudioApplicationOwner {
  engineer: EngineerWorkflowSession;
  scope: LoopMessageScope;
  post: (payload: Record<string, unknown>) => Promise<void>;
  isCurrent?: () => boolean;
}

/** Owns the one-click Validate Apply/Restore review-confirm sequence. */
export class ReviewedStudioApplicationCoordinator {
  private readonly handled = new Set<string>();
  private activeRequest: string | undefined;
  private generation = 0;
  private disposed = false;

  dispose(): void {
    this.disposed = true;
    this.generation += 1;
  }

  async handle(value: unknown, owner: ReviewedStudioApplicationOwner): Promise<boolean> {
    const command = decodeReviewedStudioApplicationCommand(value, owner.scope);
    if (!command) return false;
    if (this.disposed || owner.isCurrent?.() === false) return true;
    if (this.handled.has(command.messageId)) {
      await this.forward(owner, { status: "busy", reason: "This application request has already been handled; no runtime operation was repeated.", applicationAction: command.action });
      return true;
    }
    if (this.activeRequest !== undefined) {
      await this.forward(owner, { status: "busy", reason: "Another application request is awaiting confirmation.", applicationAction: command.action });
      return true;
    }
    if (this.handled.size >= 4096) {
      await this.forward(owner, { status: "busy", reason: "Application request limit reached. Reopen Control Studio before starting another operation.", applicationAction: command.action });
      return true;
    }
    this.handled.add(command.messageId);
    this.activeRequest = command.messageId;
    const generation = this.generation;
    const actionKind = command.action === "apply" ? "apply" : "rollback";
    const reviewMessage = this.message(owner.scope, `${command.messageId}:review`, actionKind);
    let reviewed = false;
    let terminalReviewPayload: Record<string, unknown> | undefined;
    try {
      const handled = await owner.engineer.handle(reviewMessage, owner.scope, async payload => {
        if (generation !== this.generation || this.disposed || owner.isCurrent?.() === false) return;
        const marked = { ...payload, applicationAction: command.action };
        if (payload.status === "review" && payload.rollback === (command.action === "restore")) reviewed = true;
        else if (payload.status === "failed" || payload.status === "recovery-required" || payload.status === "busy") terminalReviewPayload = marked;
        await this.forward(owner, marked);
      });
      if (!handled || !reviewed || generation !== this.generation || this.disposed || owner.isCurrent?.() === false) {
        if (!reviewed && !terminalReviewPayload && generation === this.generation && !this.disposed && owner.isCurrent?.() !== false) {
          await this.forward(owner, { status: "failed", reason: `The ${command.action} review was not available.`, applicationAction: command.action });
        }
        return true;
      }
      const confirmMessage = this.message(owner.scope, `${command.messageId}:confirm`, "confirm");
      await owner.engineer.handle(confirmMessage, owner.scope, async payload => {
        if (generation !== this.generation || this.disposed || owner.isCurrent?.() === false) return;
        await this.forward(owner, { ...payload, applicationAction: command.action });
      });
      return true;
    } finally {
      if (this.activeRequest === command.messageId) this.activeRequest = undefined;
    }
  }

  private async forward(owner: ReviewedStudioApplicationOwner, payload: Record<string, unknown>): Promise<void> {
    if (!this.disposed && owner.isCurrent?.() !== false) await owner.post(payload);
  }

  private message(scope: LoopMessageScope, messageId: string, action: "apply" | "rollback" | "confirm") {
    return { schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
      expectedLoopRevision: scope.loopRevision, payload: {} };
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
