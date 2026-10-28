import * as vscode from "vscode";
import type { EngineAnswer } from "../engineClient";
import type { ApplicationAnswer } from "../../generated/ApplicationAnswer";
import type { ApplicationRequest } from "../../generated/ApplicationRequest";
import type { KeepReviewAnswer } from "../../generated/KeepReviewAnswer";
import type { OperationStarted } from "../../generated/OperationStarted";
import { record, type EngineLoop, type EngineRouteContext, type EngineScope, type EngineStageRoute } from "../engineRouteContext";

/** The methods that review a change and then confirm it with the review's identity (specification 35, §0.1b, B6). */
type ReviewedMethod = "apply" | "restore" | "keep";

/**
 * The answer of one of the three methods: the review, the started operation (`{operationId}`), the end of a Restore on a runtime that
 * was replaced since Apply (its outcome, B6.4), or the failure that refuses it.
 */
type ReviewedAnswer = ApplicationAnswer | KeepReviewAnswer | OperationStarted;

/** The page's message kind of each method (`webview/threePageApplication.ts`). */
const METHOD_OF_KIND = new Map<string, ReviewedMethod>([
  ["control-studio.application.apply", "apply"],
  ["control-studio.application.restore", "restore"],
  ["control-studio.application.keep", "keep"],
]);

/** The page's Abort of an Apply, a Restore or a Keep: only Cancel stops an operation. */
const CANCEL_KIND = "control-studio.engineer.cancel";

/** The operation this stage runs, which Abort stops; the watch ends once the operation has ended. */
interface RunningOperation {
  readonly operationId: string;
  readonly watch: vscode.Disposable;
}

/** The application stage's route: `handle` and `reset`, and `adopt`, which takes an Apply, Restore or Keep the engine kept running (A8.6). */
export interface ApplicationStageRoute extends EngineStageRoute {
  /**
   * Takes the Apply, Restore or Keep operation the engine runs for the loop, found when the panel reopens the loop (A8.6): Cancel
   * then stops it, as it stops one this route started.
   */
  adopt(operationId: string): void;
}

/**
 * Apply, Restore previous settings and Keep in project (specification 35, B6; §0.1a `apply`, `restore`, `keep`; §0.1b). A request
 * without a review identity asks the engine for the review, which writes nothing; a request with the identity of that review
 * confirms it, and the engine runs the change as an operation whose result follows through the engine route. The engine decides
 * when each action is allowed and refuses what it does not allow: this route only names the request's fields. The runtime
 * connection goes to the engine only (its token is a credential), the physical approval and the Resume of an observation are sent
 * as the page gives them (A2.5, A8.6; the engine refuses a Resume of a Restore), and Keep reads the editors' unsaved files (B6.5).
 */
export function createApplicationRoute(context: EngineRouteContext): ApplicationStageRoute {
  let running: RunningOperation | undefined;

  /** Ends the watch of the running operation, which is also what the panel opening the loop again does (`reset`). */
  function forgetOperation(): void {
    running?.watch.dispose();
    running = undefined;
  }

  /** Watches the operation's updates until it ends, so that Abort reaches the operation only while it runs. */
  function watch(operationId: string): void {
    forgetOperation();
    const subscription: vscode.Disposable = context.client.onUpdate(operationId, (update) => {
      if (update.state === "running") return;
      if (running?.operationId === operationId) running = undefined;
      subscription.dispose();
    });
    running = { operationId, watch: subscription };
  }

  /** The engine's call of one method: the review without a review identity, or the confirmation with it. */
  function send(
    method: ReviewedMethod,
    base: Omit<ApplicationRequest, "physicalApproved" | "resume">,
    payload: Record<string, unknown>
  ): Promise<EngineAnswer<ReviewedAnswer>> {
    if (method === "keep") return context.client.keep({ ...base, unsavedFiles: unsavedFiles() });
    const request: ApplicationRequest = {
      ...base,
      physicalApproved: payload.physicalApproved === true,
      // The Resume of a journaled observation (A8.5, A8.6), as the page asks for it; the engine refuses it for a Restore.
      ...(payload.resume === true ? { resume: true } : {}),
    };
    return method === "apply" ? context.client.apply(request) : context.client.restore(request);
  }

  /**
   * One request to the engine's method, with the review identity of its confirmation when there is one. A started operation is
   * followed; a review is posted to the page, which shows it before the engineer confirms it (B6.1, B6.4, B6.5), and so is the end of
   * a Restore that the engine answers at once on a replaced runtime (B6.4).
   */
  async function request(
    scope: EngineScope,
    loop: EngineLoop,
    method: ReviewedMethod,
    payload: Record<string, unknown>,
    review: string | undefined
  ): Promise<boolean> {
    const connection = await context.connection();
    const base = {
      loopId: loop.id,
      projectRoot: context.projectRoot,
      runtimeId: loop.runtimeId,
      storageFolder: context.storageFolder,
      ...(connection ? { connection } : {}),
      ...(review ? { review } : {}),
    };
    const reply = await context.ask(scope, method, () => send(method, base, payload));
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
    if (!reply) return true;
    if ("failure" in reply) {
      await context.postFailure(scope, method, reply.failure);
      return true;
    }
    const started = reply.result as { operationId?: unknown };
    if (typeof started.operationId === "string") {
      watch(started.operationId);
      context.follow(started.operationId, scope);
      return true;
    }
    await context.post(scope, { kind: "control-studio.engine.result", method, result: reply.result });
    return true;
  }

  /**
   * Abort stops the operation this stage runs. The page's Abort of a served loop is answered here: with no
   * operation running there is nothing to stop, so the command is consumed here.
   */
  async function cancel(scope: EngineScope): Promise<boolean> {
    const operationId = running?.operationId;
    if (operationId) {
      const reply = await context.ask(scope, "cancel", () => context.client.cancel({ operationId }));
      if (reply && "failure" in reply) await context.postFailure(scope, "cancel", reply.failure);
    }
    return true;
  }

  return {
    async handle(message, scope) {
      const loop = context.served();
      if (!loop) return false;
      const kind = message.kind;
      if (kind === CANCEL_KIND) return cancel(scope);
      const method = typeof kind === "string" ? METHOD_OF_KIND.get(kind) : undefined;
      if (!method) return false;
      const payload = record(message.payload) ?? {};
      return request(scope, loop, method, payload, typeof payload.review === "string" ? payload.review : undefined);
    },
    reset: forgetOperation,
    // The panel already relays the operation's updates (`follow` at open); the watch is what Cancel reads (`running`).
    adopt(operationId: string): void {
      watch(operationId);
    },
  };
}

/** The project files that have unsaved changes in an editor: a Keep must not change them (B6.5). */
function unsavedFiles(): string[] {
  return vscode.workspace.textDocuments
    .filter((document) => document.isDirty && document.uri.scheme === "file")
    .map((document) => document.uri.fsPath);
}
