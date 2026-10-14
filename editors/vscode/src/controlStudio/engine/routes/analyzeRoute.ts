import type { Failure } from "../../generated/Failure";
import type { OperationUpdate } from "../../generated/OperationUpdate";
import type { LoopMessageScope } from "../../messages";
import type { ControlStudioEngine, EngineAnswer } from "../engineClient";
import { record, scopeFields, unclassifiedFailure, type EngineLoop } from "../engineRouteContext";
import type { AnalysisEnvelope, AnalysisInputs } from "../../generated/AnalyzeAnswer";
import type { AnalysisTool, AnalyzeRequest } from "../../generated/AnalyzeRequest";
import type { AnalysisPage } from "../../generated/PageRequest";

/** The analysis route's ports on the panel's engine route. */
export interface AnalyzeRouteOptions {
  readonly client: ControlStudioEngine;
  /** The folder of the project that holds the loop. */
  readonly projectRoot: string;
  /** Posts one message to the page; the panel drops it once the page is no longer current. */
  readonly post: (message: Record<string, unknown>) => Promise<void>;
  /** Whether the engine serves the loop: then the engine takes the study's selected model. */
  readonly served: () => boolean;
  /** The selected model the TypeScript session holds: a loop the engine does not serve analyses with it (C12, V7). */
  readonly parkedModel: () => unknown;
  /** Waits until the study inputs the page changed are recorded, in the order the page made them (the inputs route). */
  readonly settled: () => Promise<void>;
}

/** The analysis route (specification 35, B9.9 and §0.1a `controlStudio/analyze`): the page's analysis, for every loop. */
export interface AnalyzeRoute {
  /**
   * Takes the loop the panel opened: answers the tools that apply to it, then restores the analysis the page last showed, which
   * the page calculates again (A8.6, B9.9).
   */
  opened(loop: EngineLoop, scope: LoopMessageScope, shown: AnalysisPage | undefined): Promise<void>;
  /** Routes one `control-studio.analysis.*` message of the page. True when the message was consumed. */
  handle(message: Record<string, unknown>, scope: LoopMessageScope): Promise<boolean>;
  /** Takes an analysis operation the engine kept running when the panel opened the loop (A8.6); its updates reach the page again. */
  adopt(operationId: string, scope: LoopMessageScope, update: OperationUpdate): void;
  /** The analysis the Tune page shows, which `controlStudio/page` records with the page (B9.9). */
  shownPage(): AnalysisPage | undefined;
  /** Forgets the loop, the shown analysis and the running operation: the panel opens another loop, or closes. */
  reset(): void;
}

/** The page's messages of the analysis and the answers the route sends back. The names are the page's (`threePageAnalysis.ts`). */
const KIND = {
  tools: "control-studio.analysis.tools",
  started: "control-studio.analysis.started",
  answer: "control-studio.analysis.answer",
  update: "control-studio.analysis.update",
  restore: "control-studio.analysis.restore",
  failure: "control-studio.analysis.failure",
} as const;

export function createAnalyzeRoute(options: AnalyzeRouteOptions): AnalyzeRoute {
  const { client } = options;
  /** The loop the panel opened; none while the panel shows no loop. */
  let opened: EngineLoop | undefined;
  /** The analysis the Tune page shows: its tool and the inputs the page holds for it. */
  let shown: AnalysisPage | undefined;
  /** The analysis operation the engine runs for the page; Cancel stops it. */
  let running: string | undefined;
  /** The subscription to the running operation's updates. */
  let watch: { dispose(): void } | undefined;

  const stopWatching = (): void => {
    running = undefined;
    watch?.dispose();
    watch = undefined;
  };

  /**
   * One engine call, after the study inputs the page changed are recorded. A call that cannot be made, or that answers nothing, is the
   * call's failure, so every reply below is a method's answer.
   */
  async function attempt<T>(run: () => Promise<EngineAnswer<T> | undefined>): Promise<EngineAnswer<T>> {
    try {
      await options.settled();
      return (await run()) ?? { failure: unclassifiedFailure(NO_ANSWER) };
    } catch (error) {
      return { failure: unclassifiedFailure(error) };
    }
  }

  /** The request for the loop. A loop the engine does not serve carries the TypeScript session's model; a served loop's is the study's. */
  function requestFor(current: EngineLoop, extra: Partial<AnalyzeRequest>): AnalyzeRequest {
    const model = options.served() ? undefined : record(options.parkedModel());
    return {
      projectRoot: options.projectRoot,
      runtimeId: current.runtimeId,
      loopId: current.id,
      ...extra,
      ...(model ? { model } : {}),
    };
  }

  const send = (scope: LoopMessageScope, kind: string, fields: Record<string, unknown>): Promise<void> =>
    options.post({ ...scopeFields(scope), kind, ...fields });

  /** Relays one analysis operation's updates to the page until it ends or the route is reset. */
  function follow(operationId: string, scope: LoopMessageScope): void {
    stopWatching();
    running = operationId;
    watch = client.onUpdate(operationId, (update: OperationUpdate) => {
      if (running !== operationId) return;
      if (update.state !== "running") stopWatching();
      void send(scope, KIND.update, updateFields(operationId, update));
      // The calculation ended with the inputs the engine read: the page shows them again when the panel reopens (A8.6, B9.9).
      if (update.state === "finished" && update.result && opened) {
        shown = answeredAnalysis(update.result as unknown as AnalysisEnvelope);
        void recordPage(opened, scope);
      }
    });
  }

  async function refresh(current: EngineLoop, scope: LoopMessageScope): Promise<void> {
    const reply = await attempt(() => client.analyze(requestFor(current, {})));
    if ("failure" in reply) {
      // The engine does not run the program's own law of a parked loop (until V7): the loop offers no tool here, and the refusal is
      // the TypeScript route's, not a failure the page shows (B9.9).
      if (reply.failure.cause === "controller-capability") await send(scope, KIND.tools, { tools: [] });
      else await send(scope, KIND.tools, { failure: reply.failure });
      return;
    }
    const tools = record(reply.result)?.tools;
    if (Array.isArray(tools)) await send(scope, KIND.tools, { tools });
    else await send(scope, KIND.failure, { method: "tools", failure: unclassifiedFailure(UNEXPECTED_ANSWER) });
  }

  /** Records the page the Tune page shows, with its analysis when one is shown (`controlStudio/page`, B9.9). */
  async function recordPage(current: EngineLoop, scope: LoopMessageScope): Promise<void> {
    if (!options.served()) return;
    const reply = await attempt(() => client.page({
      loopId: current.id,
      page: "Tune",
      projectRoot: options.projectRoot,
      runtimeId: current.runtimeId,
      ...(shown ? { analysis: shown } : {}),
    }));
    if ("failure" in reply) await send(scope, KIND.failure, { method: "page", failure: reply.failure });
  }

  async function show(current: EngineLoop, scope: LoopMessageScope, payload: Record<string, unknown>): Promise<void> {
    const tool = typeof payload.tool === "string" ? (payload.tool as AnalysisTool) : undefined;
    shown = tool ? { tool, inputs: (record(payload.inputs) ?? {}) as AnalysisInputs } : undefined;
    await recordPage(current, scope);
  }

  async function run(current: EngineLoop, scope: LoopMessageScope, payload: Record<string, unknown>): Promise<void> {
    if (typeof payload.tool !== "string") return;
    const tool = payload.tool as AnalysisTool;
    const inputs = (record(payload.inputs) ?? {}) as AnalysisInputs;
    shown = { tool, inputs };
    const reply = await attempt(() => client.analyze(requestFor(current, { tool, inputs })));
    if ("failure" in reply) {
      await send(scope, KIND.answer, { tool, failure: reply.failure });
      return;
    }
    const answer = reply.result;
    const operationId = record(answer)?.operationId;
    if (typeof operationId === "string") {
      await send(scope, KIND.started, { tool, operationId });
      follow(operationId, scope);
    } else if (typeof record(answer)?.tool === "string") {
      const envelope = answer as AnalysisEnvelope;
      // The inputs the engine read are the ones the page records and shows again (A8.6), not the ones it sent.
      shown = answeredAnalysis(envelope);
      await send(scope, KIND.answer, { envelope });
    } else {
      await send(scope, KIND.failure, { method: "run", failure: unclassifiedFailure(UNEXPECTED_ANSWER) });
    }
    await recordPage(current, scope);
  }

  async function cancel(scope: LoopMessageScope): Promise<void> {
    const operationId = running;
    if (!operationId) return;
    const reply = await attempt(() => client.cancel({ operationId }));
    if ("failure" in reply) await send(scope, KIND.failure, { method: "cancel", failure: reply.failure });
  }

  return {
    async opened(loop, scope, restored) {
      opened = loop;
      shown = restored;
      await refresh(loop, scope);
      if (restored) await send(scope, KIND.restore, { tool: restored.tool, inputs: restored.inputs });
    },

    async handle(message, scope) {
      const current = opened;
      if (!current) return false;
      switch (message.kind) {
        case "control-studio.analysis.tools":
          await refresh(current, scope);
          return true;
        case "control-studio.analysis.run":
          await run(current, scope, record(message.payload) ?? {});
          return true;
        case "control-studio.analysis.cancel":
          await cancel(scope);
          return true;
        case "control-studio.analysis.show":
          await show(current, scope, record(message.payload) ?? {});
          return true;
        default:
          return false;
      }
    },

    adopt(operationId, scope, update) {
      follow(operationId, scope);
      void send(scope, KIND.update, updateFields(operationId, update));
    },

    shownPage: () => shown,

    reset() {
      opened = undefined;
      shown = undefined;
      stopWatching();
    },
  };
}

const UNEXPECTED_ANSWER = "The engine answered the analysis with a result the page does not show.";
const NO_ANSWER = "The engine did not answer the analysis.";

/** The analysis the page shows for a calculation: its tool, with the inputs the engine read for it (`AnalysisPage`, B9.9). */
function answeredAnalysis(envelope: AnalysisEnvelope): AnalysisPage {
  return { tool: envelope.tool, inputs: envelope.inputs };
}

/** The page's fields of one operation update: its state, its progress, and its calculation or failure once it ends. */
function updateFields(operationId: string, update: OperationUpdate): Record<string, unknown> {
  const failure: Failure | undefined = update.failure ?? undefined;
  return {
    operationId,
    state: update.state,
    ...(update.progress ? { progress: update.progress } : {}),
    ...(update.state === "finished" && update.result ? { envelope: update.result as unknown as AnalysisEnvelope } : {}),
    ...(failure ? { failure } : {}),
  };
}
