import type { EngineProcess } from "../../controlStudio/engine/engineProcess";
import type { LoopMessageScope } from "../../controlStudio/messages";

/**
 * The helpers that the suites of the engine route share: the fake engine process, the engine's answer to an open, and the page's
 * messages. The suites are `control-studio-engine-route.test.ts` and `control-studio-engine-route-tune.test.ts`.
 */

/**
 * An engine process without a binary: it answers each method the test names, records each request, and relays the update
 * notifications the test sends. The route's own functions run unchanged over it.
 */
export function fakeEngine(answers: Record<string, unknown>): {
  readonly engine: EngineProcess;
  readonly requests: Array<{ method: string; params: unknown }>;
  notify(params: unknown): void;
} {
  const requests: Array<{ method: string; params: unknown }> = [];
  const listeners: Array<(params: unknown) => void> = [];
  // A loop without a model offers no analysis tool: the open answer asks for the tools, so each fake answers them (B9.9).
  const scripted: Record<string, unknown> = { "controlStudio/analyze": { result: { tools: [] } }, ...answers };
  const connection = {
    sendRequest: async (method: string, params: unknown) => {
      requests.push({ method, params });
      const answer = scripted[method];
      // An answer the test computes, such as one it holds back until it releases it.
      return typeof answer === "function" ? (answer as () => unknown)() : answer;
    },
  };
  const engine = {
    connection: async () => connection,
    onNotification: (_method: string, listener: (params: unknown) => void) => {
      listeners.push(listener);
      return { dispose: () => undefined };
    },
    dispose: () => undefined,
  };
  return {
    engine: engine as unknown as EngineProcess,
    requests,
    notify: (params) => {
      for (const listener of listeners) listener(params);
    },
  };
}

/** A page command or the study reset, with the scope fields every page message carries. */
export function commandMessage(scope: LoopMessageScope, kind: string, payload: Record<string, unknown>): Record<string, unknown> {
  return {
    schemaVersion: 1,
    kind,
    messageId: "message-test",
    projectId: scope.projectId,
    editorSessionId: scope.editorSessionId,
    loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision,
    payload,
  };
}

/** The page's Tune command: the flow starts at Tune with the page's method, pins and task, and a family only when the engineer chose one (B4.1, B4.2). */
export function tuneMessage(scope: LoopMessageScope, payload: Record<string, unknown> = {}): Record<string, unknown> {
  return commandMessage(scope, "control-studio.flow.start", {
    from: "tune",
    applicationMode: "auto",
    pinnedValues: null,
    tuningPreferences: { method: "automatic" },
    task: {},
    ...payload,
  });
}

/**
 * The engine's answer to `controlStudio/open` for a served loop: the operations it runs, the journaled ones it reattaches, and the
 * study's tune section when the study holds a candidate.
 */
export function openedAnswer(running: string[], reattached: string[] = [], tune?: Record<string, unknown>, projectTest = false): Record<string, unknown> {
  return {
    result: {
      kind: "opened",
      loop: { id: "tank" },
      page: "Identify",
      stages: [],
      reattached,
      running,
      inputs: {},
      restoreAvailable: false,
      keepAvailable: false,
      applyObservationResumable: false,
      ...(tune ? { tune } : {}),
      // The engine's rule for the project test (C12): a loop that declares a project-scripted experiment.
      ...(projectTest ? { projectTest: true } : {}),
    },
  };
}
