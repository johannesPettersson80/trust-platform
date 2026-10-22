import type { Failure } from "../../generated/Failure";
import type { OperationUpdate } from "../../generated/OperationUpdate";
import type { ControlStudioEngine, EngineAnswer } from "../engineClient";
import { record, scopeFields, unclassifiedFailure, type EngineLoop, type EngineScope } from "../engineRouteContext";
import type { AnalysisEnvelope, AnalysisInputs } from "../../generated/AnalyzeAnswer";
import type { AnalysisTool, AnalyzeRequest, HeldAnswer } from "../../generated/AnalyzeRequest";
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
  /** Waits until the study inputs the page changed are recorded, in the order the page made them (the inputs route). */
  readonly settled: () => Promise<void>;
}

/** The analysis route (specification 35, B9.9 and §0.1a `controlStudio/analyze`): the page's analysis, for every loop. */
export interface AnalyzeRoute {
  /**
   * Takes the loop the panel opened: answers the tools that apply to it, then restores the analysis the page last showed, which
   * the page calculates again (A8.6, B9.9).
   */
  opened(loop: EngineLoop, scope: EngineScope, shown: AnalysisPage | undefined): Promise<void>;
  /** Routes one `control-studio.analysis.*` message of the page. True when the message was consumed. */
  handle(message: Record<string, unknown>, scope: EngineScope): Promise<boolean>;
  /** Takes an analysis operation the engine kept running when the panel opened the loop (A8.6); its updates reach the page again. */
  adopt(operationId: string, scope: EngineScope, update: OperationUpdate): void;
  /** Forgets the loop and the running operation: the panel opens another loop, or closes. */
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

  /** The request for the loop: the engine analyses the study's model, which is the only model it holds for the loop. */
  function requestFor(current: EngineLoop, extra: Partial<AnalyzeRequest>): AnalyzeRequest {
    return {
      projectRoot: options.projectRoot,
      runtimeId: current.runtimeId,
      loopId: current.id,
      ...extra,
    };
  }

  const send = (scope: EngineScope, kind: string, fields: Record<string, unknown>): Promise<void> =>
    options.post({ ...scopeFields(scope), kind, ...fields });

  /**
   * Relays one analysis operation's updates to the page until it ends or the route is reset. An operation that ended before this
   * subscription stood reaches the page as its final update all the same (`ControlStudioEngine.onUpdate`).
   */
  function follow(operationId: string, scope: EngineScope): void {
    stopWatching();
    running = operationId;
    watch = client.onUpdate(operationId, (update: OperationUpdate) => relayUpdate(operationId, scope, update));
  }

  /** Sends the page one update of the operation the route follows; its final update ends the follow. */
  function relayUpdate(operationId: string, scope: EngineScope, update: OperationUpdate): void {
    if (running !== operationId) return;
    if (update.state !== "running") stopWatching();
    void send(scope, KIND.update, updateFields(operationId, update));
  }

  async function refresh(current: EngineLoop, scope: EngineScope, held?: HeldAnswer[]): Promise<void> {
    const reply = await attempt(() => client.analyze(requestFor(current, held ? { held } : {})));
    // A loop the engine refuses offers no tool: the engine answers the list empty, and the refusal is the open answer's (B9.9).
    if ("failure" in reply) {
      await send(scope, KIND.tools, { failure: reply.failure });
      return;
    }
    const answered = record(reply.result);
    const tools = answered?.tools;
    // The engine lists the held answers that are no longer current for the model (B9.9); the route forwards the list and decides none.
    const stale = answered?.stale;
    if (Array.isArray(tools)) await send(scope, KIND.tools, { tools, ...(Array.isArray(stale) ? { stale } : {}) });
    else await send(scope, KIND.failure, { method: "tools", failure: unclassifiedFailure(UNEXPECTED_ANSWER) });
  }

  /**
   * Records the analysis the Tune page shows with the page (`controlStudio/page`, B9.9), as the page names it; none when it shows none.
   * The engine records the analysis a calculation answers by itself, with the inputs it read: this route stores nothing of its own.
   */
  async function show(current: EngineLoop, scope: EngineScope, payload: Record<string, unknown>): Promise<void> {
    if (!options.served()) return;
    const analysis = analysisPageOf(payload);
    const reply = await attempt(() => client.page({
      loopId: current.id,
      page: "Tune",
      projectRoot: options.projectRoot,
      runtimeId: current.runtimeId,
      ...(analysis ? { analysis } : {}),
    }));
    if ("failure" in reply) await send(scope, KIND.failure, { method: "page", failure: reply.failure });
  }

  async function run(current: EngineLoop, scope: EngineScope, payload: Record<string, unknown>): Promise<void> {
    if (typeof payload.tool !== "string") return;
    const tool = payload.tool as AnalysisTool;
    const inputs = (record(payload.inputs) ?? {}) as AnalysisInputs;
    const reply = await attempt(() => client.analyze(requestFor(current, { tool, inputs })));
    if ("failure" in reply) {
      await send(scope, KIND.answer, { tool, failure: reply.failure });
      return;
    }
    const answer = reply.result;
    const operationId = record(answer)?.operationId;
    if (typeof operationId === "string") {
      // `started` is posted at once, and the route subscribes before it awaits the post: an update sent while the page acknowledges
      // `started` reaches the route. An update sent before the subscription is recovered by `follow`.
      const posted = send(scope, KIND.started, { tool, operationId });
      follow(operationId, scope);
      await posted;
    } else if (typeof record(answer)?.tool === "string") {
      await send(scope, KIND.answer, { envelope: answer as AnalysisEnvelope });
    } else {
      // The tool is named, so that the page ends the calculation it waits for (the refusal clears its pending run).
      await send(scope, KIND.failure, { method: "run", tool, failure: unclassifiedFailure(UNEXPECTED_ANSWER) });
    }
  }

  async function cancel(scope: EngineScope): Promise<void> {
    const operationId = running;
    if (!operationId) return;
    const reply = await attempt(() => client.cancel({ operationId }));
    if ("failure" in reply) await send(scope, KIND.failure, { method: "cancel", failure: reply.failure });
  }

  return {
    async opened(loop, scope, restored) {
      opened = loop;
      await refresh(loop, scope);
      if (restored) await send(scope, KIND.restore, { tool: restored.tool, inputs: restored.inputs });
    },

    async handle(message, scope) {
      const current = opened;
      if (!current) return false;
      switch (message.kind) {
        case "control-studio.analysis.tools": {
          // The page names the answers it holds with the revision of the model each was calculated for; the engine says which are stale.
          const held = record(message.payload)?.held;
          await refresh(current, scope, Array.isArray(held) ? held as HeldAnswer[] : undefined);
          return true;
        }
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

    reset() {
      opened = undefined;
      stopWatching();
    },
  };
}

const UNEXPECTED_ANSWER = "The engine answered the analysis with a result the page does not show.";
const NO_ANSWER = "The engine did not answer the analysis.";

/**
 * The analysis the page names as the one it shows, for `controlStudio/page` (B9.9): its tool and the inputs it holds for it, or none when it
 * shows none. The page's navigation and its tool selection both name it this way.
 */
export function analysisPageOf(value: unknown): AnalysisPage | undefined {
  const named = record(value);
  return typeof named?.tool === "string" ? { tool: named.tool as AnalysisTool, inputs: (record(named.inputs) ?? {}) as AnalysisInputs } : undefined;
}

/** The page's fields of one operation update: its state, its progress, and its calculation, failure or cancellation sentence once it ends. */
function updateFields(operationId: string, update: OperationUpdate): Record<string, unknown> {
  const failure: Failure | undefined = update.failure ?? undefined;
  return {
    operationId,
    state: update.state,
    ...(update.progress ? { progress: update.progress } : {}),
    ...(update.message ? { message: update.message } : {}),
    ...(update.state === "finished" && update.result ? { envelope: update.result as unknown as AnalysisEnvelope } : {}),
    ...(failure ? { failure } : {}),
  };
}
