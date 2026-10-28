import type { LoopMessageScope } from "./messages";
import type { RuntimeLifecycleResult, RuntimeLifecycleSnapshot } from "../runtimeLifecycle";
import type { SimulatorStartAnswer } from "./generated/SimulatorStartAnswer";
import type { SimulatorStartOutcome } from "./generated/SimulatorStartOutcome";
import type { SimulatorStartReply } from "./generated/SimulatorStartReply";
import type { SimulatorStartRequest } from "./generated/SimulatorStartRequest";
import type { ControlStudioEngine, EngineAnswer } from "./engine/engineClient";

export interface ControlStudioRuntimeStartScope {
  schemaVersion: 1;
  projectId: string;
  editorSessionId: string;
  loopId: string;
  loopRevision: string;
  runtimeId: string;
  runtimeMode: "simulate";
}

/** The reply to the page's start: `ok` with its sentence, or the failure with its kind and sentence (specification 35, A2.8). */
export type ControlStudioRuntimeStartResult = SimulatorStartReply;

/** The start as the page's message names it: the runtime and the mode it asks for. */
export interface ControlStudioRuntimeStartRequest {
  runtimeId?: unknown;
  runtimeMode?: unknown;
}

export interface ControlStudioRuntimeLifecycle {
  snapshot(): Promise<Pick<RuntimeLifecycleSnapshot, "status" | "starting">>;
  startLocalSimulator(): Promise<RuntimeLifecycleResult>;
}

/** The engine's side of a start: the decision, and the reply to the outcome the lifecycle service reached. */
export type ControlStudioRuntimeStartEngine = Pick<ControlStudioEngine, "simulatorStart" | "simulatorStartResult">;

/**
 * Creates the one scoped host action the Control Studio Start simulator button uses. The engine decides each request (A2.8): it
 * refuses it at once, joins the start in flight, or lets the host start the local simulator through the runtime lifecycle service
 * and then reports the outcome. The host forwards the facts, starts the service and returns the engine's reply.
 */
export function createControlStudioRuntimeStartHandler(
  lifecycle: ControlStudioRuntimeLifecycle,
  engine: ControlStudioRuntimeStartEngine
) {
  let pending: Promise<ControlStudioRuntimeStartResult> | undefined;
  let pendingScope: string | undefined;

  return function start(
    request: ControlStudioRuntimeStartRequest,
    expected: ControlStudioRuntimeStartScope
  ): Promise<ControlStudioRuntimeStartResult> {
    const scopeKey = JSON.stringify(expected);
    if (pending) {
      return decideStart(lifecycle, engine, request, expected, pendingScope === scopeKey ? "sameScope" : "otherScope", pending);
    }
    // The start is claimed before the first await, so a second request that arrives meanwhile sees it in flight.
    const operation = decideStart(lifecycle, engine, request, expected, "idle", undefined).finally(() => {
      pending = undefined;
      pendingScope = undefined;
    });
    pending = operation;
    pendingScope = scopeKey;
    return operation;
  };
}

/** The text of a reply the engine's decision did not give, which the engine never sends: the start is not made. */
const UNDECIDED = "The simulator start could not be decided. Wait and retry.";

async function decideStart(
  lifecycle: ControlStudioRuntimeLifecycle,
  engine: ControlStudioRuntimeStartEngine,
  request: ControlStudioRuntimeStartRequest,
  expected: ControlStudioRuntimeStartScope,
  pending: SimulatorStartRequest["pending"],
  inFlight: Promise<ControlStudioRuntimeStartResult> | undefined
): Promise<ControlStudioRuntimeStartResult> {
  try {
    const snapshot = await lifecycle.snapshot();
    const answer: EngineAnswer<SimulatorStartAnswer> = await engine.simulatorStart({
      loopRuntimeId: expected.runtimeId,
      requestedRuntimeId: typeof request.runtimeId === "string" ? request.runtimeId : undefined,
      requestedRuntimeMode: typeof request.runtimeMode === "string" ? request.runtimeMode : undefined,
      runtimeMode: snapshot.status.runtimeMode,
      running: snapshot.status.running,
      starting: snapshot.starting,
      pending,
    });
    if ("failure" in answer) return failureReply(answer.failure.message);
    switch (answer.result.decision) {
      case "join":
        return inFlight ?? failureReply(UNDECIDED);
      case "reply":
        return answer.result.reply ?? failureReply(UNDECIDED);
      case "start":
        return await startThroughLifecycle(lifecycle, engine);
    }
  } catch (error) {
    return failureReply(errorText(error));
  }
}

/** Starts the simulator through the lifecycle service, then reports the outcome and returns the engine's reply to it. */
async function startThroughLifecycle(
  lifecycle: ControlStudioRuntimeLifecycle,
  engine: ControlStudioRuntimeStartEngine
): Promise<ControlStudioRuntimeStartResult> {
  let outcome: SimulatorStartOutcome;
  try {
    const result = await lifecycle.startLocalSimulator();
    outcome = result.ok
      ? { ok: true, message: result.message }
      : { ok: false, failureMessage: result.failure.message };
  } catch (error) {
    outcome = { ok: false, failureMessage: errorText(error) };
  }
  const reply = await engine.simulatorStartResult(outcome);
  return "failure" in reply ? failureReply(reply.failure.message) : reply.result;
}

/** A reply the engine could not give (the engine is unreachable or fails): the start failed, with the text of the failure. */
function failureReply(message: string): ControlStudioRuntimeStartResult {
  return { ok: false, failure: { kind: "start_failed", message } };
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export async function routeControlStudioRuntimeStart(message: unknown, scope: LoopMessageScope, runtimeId: string,
  start: ReturnType<typeof createControlStudioRuntimeStartHandler>, post: (value: Record<string, unknown>) => Promise<unknown>): Promise<boolean> {
  const raw = message as Record<string, unknown> | undefined;
  if (raw?.kind !== "control-studio.runtime.start") return false;
  if (raw.schemaVersion !== 1 || raw.projectId !== scope.projectId || raw.editorSessionId !== scope.editorSessionId || raw.loopId !== scope.loopId || raw.expectedLoopRevision !== scope.loopRevision) return true;
  const payload = raw.payload as Record<string, unknown> | undefined;
  const result = await start({ runtimeId: payload?.runtimeId, runtimeMode: payload?.runtimeMode },
    { schemaVersion: 1, ...scope, runtimeId, runtimeMode: "simulate" });
  await post({ kind: "control-studio.runtime.start.result", schemaVersion: 1, ...scope, runtimeId, result });
  return true;
}
