import { record, type EngineRouteContext, type EngineScope, type EngineStageRoute } from "../engineRouteContext";
import type { RunRequest } from "../../generated/RunRequest";

/** The automatic flow's stage route: `handle` and `reset`, and `adopt`, which takes a run the engine kept running (A8.6). */
export interface RunStageRoute extends EngineStageRoute {
  /**
   * Takes a run the engine runs for the loop, found when the panel reopens the loop (A8.6): Abort then stops it, as it stops a run
   * this route started.
   */
  adopt(operationId: string): void;
}

/**
 * The automatic flow (specification 35, B8 and §0.1a `run`). The Identify page's start, Identify plant, is one
 * `controlStudio/run` through Identify that identifies again (`identifyAgain`). Tune's start is the Tune route's, Validate's
 * start the Validate route's. Abort stops the run this route started or adopted, with `controlStudio/cancel`.
 */
export function createRunRoute(context: EngineRouteContext): RunStageRoute {
  /** The run the engine started for the page, or adopted on reopening; Abort stops it while it runs. */
  let running: string | undefined;

  /** Stops the run the route started. False when no run is running, or the run ended on its own, so the other routes keep the command. */
  async function cancel(scope: EngineScope): Promise<boolean> {
    const operationId = running;
    if (!operationId) return false;
    const reply = await context.ask(scope, "run", () => context.client.cancel({ operationId }));
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the Abort is not repeated.
    if (!reply) return true;
    if ("failure" in reply) {
      running = undefined;
      await context.postFailure(scope, "run", reply.failure);
      return true;
    }
    if (reply.result.state !== "running") {
      running = undefined;
      return false;
    }
    return true;
  }

  return {
    async handle(message, scope) {
      const loop = context.served();
      if (!loop) return false;
      if (message.kind === "control-studio.flow.cancel") return cancel(scope);
      if (message.kind !== "control-studio.flow.start" || record(message.payload)?.from !== "find") return false;
      const connection = await context.connection();
      const request: RunRequest = {
        projectRoot: context.projectRoot,
        runtimeId: loop.runtimeId,
        loopId: loop.id,
        storageFolder: context.storageFolder,
        ...(connection ? { connection } : {}),
        through: "identify",
        identifyAgain: true,
      };
      const reply = await context.ask(scope, "run", () => context.client.run(request));
      // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
      if (!reply) return true;
      if ("failure" in reply) {
        await context.postFailure(scope, "run", reply.failure);
        return true;
      }
      const operationId = reply.result.operationId;
      running = operationId;
      context.follow(operationId, scope);
      return true;
    },
    reset(): void {
      running = undefined;
    },
    adopt(operationId: string): void {
      running = operationId;
    },
  };
}
