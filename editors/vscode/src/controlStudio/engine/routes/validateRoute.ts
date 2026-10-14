import type { ValidateRequest } from "../../generated/ValidateRequest";
import { record, type EngineRouteContext, type EngineStageRoute } from "../engineRouteContext";

/** The Validate stage's route: `handle` and `reset`, and `adopt`, which takes a Validate the engine kept running (A8.6). */
export interface ValidateStageRoute extends EngineStageRoute {
  /**
   * Takes the Validate the engine runs for the loop, found when the panel reopens the loop (A8.6): Cancel then stops it, as it
   * stops a Validate this route started.
   */
  adopt(operationId: string): void;
}

/**
 * Validate (specification 35, B5; §0.1a `validate`): the page's Validate start, `control-studio.flow.start` from Validate, goes
 * to `controlStudio/validate`, and its Cancel, `control-studio.flow.cancel`, to `controlStudio/cancel` while that operation runs.
 * The engine decides every rule, and its refusals name their next action; the route sends the request and relays the answer.
 */
export function createValidateRoute(context: EngineRouteContext): ValidateStageRoute {
  /** The Validate operation the engine runs for the page, or adopted on reopening; Cancel stops it. */
  let running: string | undefined;
  /** The subscription that ends `running` when the operation stops. */
  let watch: { dispose(): void } | undefined;

  const forget = (): void => {
    running = undefined;
    watch?.dispose();
    watch = undefined;
  };

  return {
    async handle(message, scope) {
      const loop = context.served();
      if (!loop) return false;
      if (message.kind === "control-studio.flow.cancel") {
        const operationId = running;
        if (!operationId) return false;
        const reply = await context.ask(scope, "validate", () => context.client.cancel({ operationId }));
        if (reply && "failure" in reply) await context.postFailure(scope, "validate", reply.failure);
        return true;
      }
      if (message.kind !== "control-studio.flow.start" || record(message.payload)?.from !== "verify") return false;
      const connection = await context.connection();
      const request: ValidateRequest = {
        ...(connection ? { connection } : {}),
        loopId: loop.id,
        projectRoot: context.projectRoot,
        runtimeId: loop.runtimeId,
        storageFolder: context.storageFolder,
      };
      const reply = await context.ask(scope, "validate", () => context.client.validate(request));
      // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
      if (!reply) return true;
      if ("failure" in reply) {
        await context.postFailure(scope, "validate", reply.failure);
        return true;
      }
      const operationId = reply.result.operationId;
      forget();
      running = operationId;
      context.follow(operationId, scope);
      watch = context.client.onUpdate(operationId, (update) => {
        if (update.state !== "running" && running === operationId) forget();
      });
      return true;
    },
    reset: forget,
    adopt(operationId: string): void {
      // The panel already relays the operation's updates (`follow` at open), so only the watch that ends `running` is started here.
      forget();
      running = operationId;
      watch = context.client.onUpdate(operationId, (update) => {
        if (update.state !== "running" && running === operationId) forget();
      });
    },
  };
}
