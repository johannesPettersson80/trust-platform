import type { AnalysisEnvelope, AnalysisInputs, AnalysisResult, ToolOffer } from "../generated/AnalyzeAnswer";
import type { AnalysisTool } from "../generated/AnalyzeRequest";
import type { Failure } from "../generated/Failure";

/**
 * The page's side of the analysis answers (specification 35, B9.9 and §0.1a `controlStudio/analyze`). The engine's messages
 * (`control-studio.analysis.*`, sent by `engine/routes/analyzeRoute.ts`) reach the analysis presenter. This module names no tool,
 * label, default or limit: it checks the scope of each message and passes the engine's answer on.
 */

/** The messages the page sends the analysis route (the host's `analyzeRoute.ts` takes them). */
export const ANALYSIS_KIND = {
  tools: "control-studio.analysis.tools",
  run: "control-studio.analysis.run",
  cancel: "control-studio.analysis.cancel",
  show: "control-studio.analysis.show",
} as const;

/** One tool's answer as the engine's result holds it: the generated `AnalysisResult` keys each answer by its tool's name. */
type ToolAnswer<K extends AnalysisTool> = Extract<AnalysisResult, Record<K, unknown>> extends Record<K, infer Answer> ? Answer : never;

/** Each tool's answer, under the tool's name (B9.9). */
export type AnalysisAnswers = { [K in AnalysisTool]: ToolAnswer<K> };

export type RootLocusAnswer = AnalysisAnswers["rootLocus"];
export type RouthAnswer = AnalysisAnswers["routh"];
export type PoleZeroAnswer = AnalysisAnswers["poleZero"];
export type FrequencyAnswer = AnalysisAnswers["frequency"];
export type MeasuredFrequencyAnswer = AnalysisAnswers["measuredFrequency"];
export type FeedbackAnswer = AnalysisAnswers["feedback"];
export type LeadLagAnswer = AnalysisAnswers["leadLag"];
export type InitialStateAnswer = AnalysisAnswers["initialState"];
export type StateStructureAnswer = AnalysisAnswers["stateStructure"];
export type RelativeGainAnswer = AnalysisAnswers["relativeGain"];

/** The answer a result holds under `tool`, or undefined when the result holds none (a tool whose calculation failed). */
export function answerOf<K extends AnalysisTool>(result: AnalysisResult | undefined, tool: K): AnalysisAnswers[K] | undefined {
  const byTool: Partial<Record<AnalysisTool, unknown>> = { ...result };
  // The one cast: TypeScript cannot follow a lookup by a runtime key, so the value is typed by the generated union it came from.
  return byTool[tool] as AnalysisAnswers[K] | undefined;
}

/**
 * The window event that tells the page the study's model changed (B9.9): the page asks the engine which tools apply and which of the
 * answers it holds are no longer current for the model, and drops those. The engine's Identify answers send it when they show, clear or
 * accept a model (engineIdentify.ts).
 */
export const ENGINE_MODEL_CHANGED_EVENT = "trust-control-studio-model-changed";

/** The state of an analysis operation the engine runs (`controlStudio/update` of `controlStudio/analyze`). */
export interface AnalysisUpdate {
  operationId: string;
  state: "running" | "finished" | "failed" | "cancelled";
  progress?: string;
  /** The calculation's envelope once the operation finished. */
  envelope?: AnalysisEnvelope;
  failure?: Failure;
  /** The engine's sentence of a cancelled operation (`OperationUpdate.message`, B9.4); the page writes none. */
  message?: string;
}

/** What the presenter receives from the engine's messages. */
export interface EngineAnalysisEvents {
  /**
   * The tools that apply to the loop now; `failure` says why none can be given. `stale` lists the tools whose held answers are no longer
   * current for the model (B9.9): the engine decides which, and the page drops those answers.
   */
  tools(offers: readonly ToolOffer[] | undefined, failure: Failure | undefined, stale?: readonly AnalysisTool[]): void;
  /** The engine started an operation for the tool. */
  started(tool: AnalysisTool, operationId: string): void;
  /** A calculation answered: a quick tool's, or an operation's when it finished. */
  answer(envelope: AnalysisEnvelope): void;
  /** A refusal of the engine: a calculation's (`tool` named), or a method's (`tool` absent). */
  refused(tool: AnalysisTool | undefined, failure: Failure): void;
  /** The state of an operation the engine runs for the loop. */
  update(update: AnalysisUpdate): void;
  /** The analysis the page showed when the panel opened the loop: its tool and inputs, which the page calculates again (A8.6). */
  restore(tool: AnalysisTool, inputs: AnalysisInputs): void;
}

/** Routes the engine's analysis messages to the presenter, for the messages of this loop's scope only. */
export function bindEngineAnalysis(root: HTMLElement, events: EngineAnalysisEvents): void {
  window.addEventListener("message", event => {
    const message = event.data;
    if (!isRecord(message) || typeof message.kind !== "string" || !message.kind.startsWith("control-studio.analysis.")) return;
    if (message.schemaVersion !== 1 || message.projectId !== root.dataset.projectId || message.editorSessionId !== root.dataset.editorSessionId
      || message.loopId !== root.dataset.loopId || (message.expectedLoopRevision ?? message.loopRevision) !== root.dataset.loopRevision) return;
    switch (message.kind) {
      case "control-studio.analysis.tools":
        events.tools(Array.isArray(message.tools) ? message.tools as ToolOffer[] : undefined, failureOf(message.failure),
          Array.isArray(message.stale) ? message.stale as AnalysisTool[] : undefined);
        return;
      case "control-studio.analysis.started":
        events.started(message.tool as AnalysisTool, String(message.operationId));
        return;
      case "control-studio.analysis.answer": {
        const failure = failureOf(message.failure);
        if (isRecord(message.envelope)) events.answer(message.envelope as unknown as AnalysisEnvelope);
        if (failure) events.refused(typeof message.tool === "string" ? message.tool as AnalysisTool : undefined, failure);
        return;
      }
      case "control-studio.analysis.update":
        events.update({
          operationId: String(message.operationId),
          state: message.state as AnalysisUpdate["state"],
          ...(typeof message.progress === "string" ? { progress: message.progress } : {}),
          ...(typeof message.message === "string" ? { message: message.message } : {}),
          ...(isRecord(message.envelope) ? { envelope: message.envelope as unknown as AnalysisEnvelope } : {}),
          ...(failureOf(message.failure) ? { failure: failureOf(message.failure) } : {}),
        });
        return;
      case "control-studio.analysis.restore":
        events.restore(message.tool as AnalysisTool, (isRecord(message.inputs) ? message.inputs : {}) as AnalysisInputs);
        return;
      case "control-studio.analysis.failure": {
        const failure = failureOf(message.failure);
        if (failure) events.refused(typeof message.tool === "string" ? message.tool as AnalysisTool : undefined, failure);
        return;
      }
      default:
        return;
    }
  });
}

function failureOf(value: unknown): Failure | undefined {
  return isRecord(value) && typeof value.message === "string" ? value as unknown as Failure : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
