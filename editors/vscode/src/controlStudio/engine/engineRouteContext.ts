import type { ControlStudioEngine, EngineAnswer } from "./engineClient";
import type { Failure } from "../generated/Failure";
import type { RuntimeConnection } from "../generated/IdentifyRequest";
import type { LoopMessageScope } from "../messages";

/** The loop fields the route sends the engine. */
export interface EngineLoop {
  readonly id: string;
  readonly runtimeId: string;
}

/** The engine methods whose calls and failures the page shows; `start` is the engine's own start. */
export type EngineMethod =
  | "start"
  | "open"
  | "page"
  | "identify"
  | "tune"
  | "preview"
  | "validate"
  | "inputs"
  | "run"
  | "apply"
  | "restore"
  | "keep"
  | "cancel";

/**
 * What one stage's route needs from the panel's engine route (specification 35, §0.1a): the client, the loop the engine serves, the
 * page poster, one engine call with its start failure, the runtime connection, and the relay of an operation's updates. A stage
 * route only forwards the page's command to its engine method and the engine's answer to the page; the engine decides.
 */
export interface EngineRouteContext {
  readonly client: ControlStudioEngine;
  /** The folder of the project that holds the loop. */
  readonly projectRoot: string;
  /** The folder of the operation journals (A8.3, A8.5): empty, so that the engine uses its own. */
  readonly storageFolder: string;
  /** The loop the engine serves; none while the TypeScript route keeps the loop. */
  served(): EngineLoop | undefined;
  /** Posts one message to the page, with the loop's scope fields. */
  post(scope: LoopMessageScope, message: Record<string, unknown>): Promise<void>;
  /** Shows the engine's failure of a method on the page (`control-studio.engine.failure`). */
  postFailure(scope: LoopMessageScope, method: EngineMethod, failure: Failure): Promise<void>;
  /**
   * One engine call. Undefined when the engine cannot take it: its start failure, or the call's own failure, is then shown, and
   * the command is not repeated.
   */
  ask<T>(scope: LoopMessageScope, method: EngineMethod, run: () => Promise<EngineAnswer<T>>): Promise<EngineAnswer<T> | undefined>;
  /** The study's runtime connection for a live request; none when no runtime runs, and the engine then refuses the request. */
  connection(): Promise<RuntimeConnection | undefined>;
  /**
   * Relays an operation's updates to the page (`control-studio.engine.update`); a finished operation's result follows as
   * `control-studio.engine.result` with the operation's command as its method.
   */
  follow(operationId: string, scope: LoopMessageScope): void;
}

/**
 * One stage's route. It takes the page's messages of its stage, already checked to be of the served loop's scope, and returns
 * true when it consumed one; a message of another stage returns false.
 */
export interface EngineStageRoute {
  handle(message: Record<string, unknown>, scope: LoopMessageScope): Promise<boolean>;
  /** Forgets the stage's running operations when the panel opens a loop again. */
  reset?(): void;
}

/** The scope fields of a message to the page. */
export function scopeFields(scope: LoopMessageScope) {
  return {
    schemaVersion: 1,
    projectId: scope.projectId,
    editorSessionId: scope.editorSessionId,
    loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision,
  };
}

/** Whether a page message belongs to the loop's scope. */
export function sameScope(value: Record<string, unknown>, scope: LoopMessageScope): boolean {
  return value.schemaVersion === 1 && value.projectId === scope.projectId && value.editorSessionId === scope.editorSessionId
    && value.loopId === scope.loopId && value.expectedLoopRevision === scope.loopRevision;
}

/** A JSON object, or undefined for any other value. */
export function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}

/** A failure the engine did not classify: a call that could not be made, or an answer the page cannot show. */
export function unclassifiedFailure(error: unknown): Failure {
  const message = typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
  return { message, cause: "unclassified", nextAction: "none" };
}
