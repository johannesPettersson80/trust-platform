import { randomUUID } from "crypto";
import type { ParameterApplicationClient, ParameterReview } from "./parameterApplication";
import type { ParameterBinding } from "./parameterBinding";
import type { LoopMessageScope } from "./messages";

export type ParameterConnection = { client: ParameterApplicationClient; source: "simulation" | "physical" };
type Review = ParameterConnection & { id: string; review: ParameterReview; rollback: boolean };

/** Keeps a one-use review on its original endpoint and discards stale async replies. */
export class ParameterSession {
  private review: Review | undefined;
  private generation = 0;
  private applying = false;
  constructor(private readonly binding: ParameterBinding,
    private readonly connect: () => Promise<ParameterConnection>,
    private readonly selectedAnalysisValues?: () => Record<string, number> | undefined) {}

  async handle(message: unknown, scope: LoopMessageScope, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(message) || message.schemaVersion !== 1 || message.projectId !== scope.projectId || message.editorSessionId !== scope.editorSessionId ||
        message.loopId !== scope.loopId || message.expectedLoopRevision !== scope.loopRevision || typeof message.messageId !== "string" ||
        !record(message.payload) || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.parameters.")) return false;
    const action = message.kind.slice("control-studio.parameters.".length);
    if (!["inspect", "selected-analysis", "review", "review-rollback", "apply", "invalidate"].includes(action)) return false;
    const generation = ++this.generation;
    const respond = async (payload: Record<string, unknown>) => { if (generation === this.generation) await post(payload); };
    try {
      if (action === "invalidate") { this.review = undefined; await respond({ status: "edited", reason: "Values changed. Review again before applying." }); return true; }
      if (this.applying) throw new Error("A parameter transaction is awaiting confirmation. Inspect its status before another write.");
      if (action === "apply") {
        const retained = this.review;
        this.review = undefined;
        if (!retained || retained.id !== message.payload.reviewId) throw new Error("This review expired. Review the exact values again.");
        this.applying = true;
        await respond({ status: "applying", reason: "Waiting for the owning task and observed values…" });
        if (generation !== this.generation) { this.applying = false; return true; }
        try {
          const result = retained.rollback
            ? await retained.client.rollback(retained.review, message.payload.physicalApproved === true)
            : await retained.client.apply(retained.review, message.payload.physicalApproved === true);
          await respond({ status: "applied", observed: result, source: retained.source,
            reason: retained.rollback ? "Previous values restored and observed after ST execution." : "Reviewed values observed after ST execution." });
        } finally { this.applying = false; }
        return true;
      }
      this.review = undefined;
      if (action === "selected-analysis") {
        const values = this.selectedAnalysisValues?.();
        if (!values) throw new Error("Calculate a selected model candidate with an explicit ST parameter mapping first.");
        await respond({ status: "draft", values, reason: "Selected analysis values copied for review. The runtime has not changed." });
        return true;
      }
      const connection = await this.connect();
      if (generation !== this.generation) return true;
      if (action === "inspect") {
        await respond({ status: "inspected", observed: await connection.client.status(), binding: this.binding, source: connection.source });
        return true;
      }
      if (action === "review" && (typeof message.payload.json !== "string" || message.payload.json.length > 65536)) throw new Error("Enter a parameter JSON object of at most 64 kB.");
      const review = action === "review-rollback" ? await connection.client.reviewRollback()
        : await connection.client.review(JSON.parse(message.payload.json as string));
      if (generation !== this.generation) return true;
      const retained = { ...connection, id: randomUUID(), review, rollback: action === "review-rollback" };
      this.review = retained;
      await respond({ status: "reviewed", reviewId: retained.id, values: review.values, observed: review.observed,
        binding: this.binding, source: retained.source, rollback: retained.rollback });
    } catch (error) {
      await respond({ status: "failed", reason: error instanceof Error ? error.message : String(error) });
    }
    return true;
  }
  dispose(): void { this.generation++; this.review = undefined; }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
