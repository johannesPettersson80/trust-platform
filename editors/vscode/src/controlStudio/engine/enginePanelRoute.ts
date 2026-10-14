import type * as vscode from "vscode";
import { createEngineClient, type EngineAnswer } from "./engineClient";
import { engineExecutable, startEngine, type EngineProcess } from "./engineProcess";
import { engineIdentifyAction, type EngineAcquisitionSettings } from "./engineIdentify";
import type { Failure } from "../generated/Failure";
import type { IdentifyRequest, RuntimeConnection } from "../generated/IdentifyRequest";
import type { OpenRequest } from "../generated/OpenRequest";
import type { OpenResult } from "../generated/OpenResult";
import type { OperationUpdate } from "../generated/OperationUpdate";
import type { PageRequest } from "../generated/PageRequest";
import type { PreviewRequest } from "../generated/PreviewRequest";
import type { ControllerSettingsPins, Structure, TuneMethod, TuneRequest } from "../generated/TuneRequest";
import type { LoopMessageScope } from "../messages";
import {
  record,
  sameScope,
  scopeFields,
  unclassifiedFailure,
  type EngineLoop,
  type EngineMethod,
  type EngineRouteContext,
  type EngineStageRoute,
} from "./engineRouteContext";
import { createApplicationRoute } from "./routes/applicationRoute";
import { createExperimentRoute } from "./routes/experimentRoute";
import { createInputsRoute } from "./routes/inputsRoute";
import { createRunRoute } from "./routes/runRoute";
import { createValidateRoute } from "./routes/validateRoute";

export type { EngineLoop } from "./engineRouteContext";

export interface EnginePanelRouteOptions {
  /** The window's engine (see `controlStudioEngine`). */
  readonly engine: EngineProcess;
  /** The folder of the project that holds the loop. */
  readonly projectRoot: string;
  /** The folder of the operation journals (A8.3, A8.5). */
  readonly storageFolder: string;
  /** Posts one message to the page. */
  readonly post: (message: Record<string, unknown>) => PromiseLike<unknown> | void;
  /** False once the panel shows another loop; messages for the earlier loop are then dropped. */
  readonly isCurrent?: () => boolean;
  /** The study's runtime connection for a live identification (E11.1); none when no runtime runs. */
  readonly connection?: () => Promise<RuntimeConnection | undefined>;
  /** The engineer's acquisition settings for an identification run (B3.2); none when the study holds none. */
  readonly acquisitionSettings?: () => EngineAcquisitionSettings | undefined;
  /** The pins and the controller choice that the TypeScript session holds, which a manual preview is answered with (B4.2, B4.5). */
  readonly tuningInputs?: () => EngineTuningInputs | undefined;
}

/** What the TypeScript session holds of the engineer's tuning inputs; the study keeps them in the engine once S7 is done. */
export interface EngineTuningInputs {
  /** The pins the page keeps, with the family they were set for and their values. */
  readonly settingsPins?: { readonly family: string; readonly loopRevision: string; readonly values: Readonly<Record<string, number>> };
  /** The controller the engineer chose: a family, or automatic. */
  readonly controllerChoice?: string;
}

/**
 * The engine's side of one open Control Studio loop (specification 35, §0.1a). The engine serves the loops it runs:
 * `open` answers the page, `navigation` stores the page the engineer views, and `identify` takes the page's Identify
 * commands and the study reset, `tune` the Tune command and its Abort, and `preview` the manual preview. A loop the engine
 * parks (`controller-capability`, until V7) stays on the TypeScript route, and the route stays out of the way for the rest
 * of the panel's life.
 */
export interface EnginePanelRoute {
  /**
   * Opens the loop when the page is ready. True when the engine serves it; false when the TypeScript route keeps it. The loop's
   * declared execution owner is read as the page reads it: a project-scripted loop offers the project test (C12).
   */
  open(loop: EngineLoop & { readonly experiment?: { readonly execution: string } }, scope: LoopMessageScope): Promise<boolean>;
  /** Stores the page the engineer views for a served loop. True when the message is consumed. */
  navigation(message: unknown, scope: LoopMessageScope): Promise<boolean>;
  /**
   * Routes one Identify command or the study reset to the engine (B3, A8.4). True when the engine serves the loop and
   * takes the command; false when the TypeScript route keeps it.
   */
  identify(message: unknown, scope: LoopMessageScope): Promise<boolean>;
  /**
   * Routes the page's Tune command (a flow started at Tune) and its Abort to the engine (B4.1, B8). True when the engine serves
   * the loop and takes the command; false when the TypeScript route keeps it.
   */
  tune(message: unknown, scope: LoopMessageScope): Promise<boolean>;
  /** Answers the page's manual preview with the engine's prediction (B4.5). True when the engine serves the loop and takes it. */
  preview(message: unknown, scope: LoopMessageScope): Promise<boolean>;
  /**
   * Routes a message of the other stages to the stage's own route (`engine/routes/`): Validate, the study inputs, the automatic
   * flow, and Apply, Restore and Keep. True when the engine serves the loop and a stage took the message.
   */
  handle(message: unknown, scope: LoopMessageScope): Promise<boolean>;
  /**
   * Hands the controller family that a binding selection chose before the panel opened the loop to the inputs route, which records
   * it as the loop's structure when the engine opens the loop (B1.3, `engine/routes/inputsRoute.ts`).
   */
  bindingFamily(family: string | undefined): void;
  dispose(): void;
}

/** The page names the webview sends, as the engine names its pages (D7). */
const ENGINE_PAGE_OF_STAGE = new Map<string, PageRequest["page"]>([
  ["model", "Identify"],
  ["tune", "Tune"],
  ["verify", "Validate"],
]);

/** The method the page's Tune command names, as the engine names it (B4.2); Automatic is none. A method the engine does not run is not here. */
const ENGINE_METHOD_OF_PAGE = new Map<string, TuneMethod | undefined>([
  ["automatic", undefined],
  ["SIMC", "SIMC"],
  ["AMIGO", "AMIGO"],
  ["relay", "relay"],
  ["manual", "manual"],
]);

interface Subscription {
  dispose(): void;
}

export function createEnginePanelRoute(options: EnginePanelRouteOptions): EnginePanelRoute {
  const client = createEngineClient(options.engine);
  let parked = false;
  let served: EngineLoop | undefined;
  /** The page the engine last stored for the served loop. */
  let storedPage: string | undefined;
  /** The identification the engine runs for the page; Cancel stops it. */
  let runningOperation: string | undefined;
  /** The recording of the study's last identification, which Fit refits (B3.6). */
  let recordingId: string | undefined;
  /** The Tune the engine runs for the page; Abort stops it. */
  let runningTune: string | undefined;
  /** The family of the candidate the engine last answered: the pins of a candidate the page keeps belong to that family (B4.2). */
  let lastFamily: Structure | undefined;
  /** The number of the latest manual preview the page asked for (B4.5). */
  let previewSerial = 0;
  let subscriptions: Subscription[] = [];
  let disposed = false;

  const send = (message: Record<string, unknown>): Promise<void> => {
    if (disposed || options.isCurrent?.() === false) return Promise.resolve();
    return Promise.resolve(options.post(message)).then(() => undefined);
  };

  const postFailure = (scope: LoopMessageScope, method: EngineMethod, failure: Failure): Promise<void> =>
    send({ ...scopeFields(scope), kind: "control-studio.engine.failure", method, failure });

  /**
   * One engine call. A call the engine cannot take is the engine's start failure, not the loop's: it is shown as
   * such, and the loop keeps its TypeScript route.
   */
  async function ask<T>(
    scope: LoopMessageScope,
    method: EngineMethod,
    run: () => Promise<EngineAnswer<T>>,
    /** False once the call is superseded: its failure is then not shown (B4.5). */
    current: () => boolean = () => true
  ): Promise<EngineAnswer<T> | undefined> {
    // The engine takes the page's requests in the order the page made them: a call waits until every study input the page changed
    // before it is recorded (§0.1a `inputs`). Otherwise a start could reach the engine first, and the engine would refuse the change
    // the page made before it, because the started operation then holds the loop.
    if (method !== "inputs") await inputsRoute.settled();
    try {
      await options.engine.connection();
    } catch (error) {
      if (current()) await postFailure(scope, "start", unclassifiedFailure(error));
      return undefined;
    }
    try {
      return await run();
    } catch (error) {
      if (current()) await postFailure(scope, method, unclassifiedFailure(error));
      return undefined;
    }
  }

  /** The runtime connection of a live request; a connection that cannot be made is none, and the engine refuses the request. */
  function connectionOf(): Promise<RuntimeConnection | undefined> {
    return options.connection ? options.connection().catch(() => undefined) : Promise.resolve(undefined);
  }

  /** Relays one operation's updates to the page; an identification's final result follows its last update (B3.8). */
  function relay(operationId: string, scope: LoopMessageScope): Subscription {
    return client.onUpdate(operationId, (update: OperationUpdate) => {
      if (update.state !== "running") {
        if (runningOperation === operationId) runningOperation = undefined;
        if (runningTune === operationId) runningTune = undefined;
      }
      void send({ ...scopeFields(scope), kind: "control-studio.engine.update", operationId, update });
      const result = update.result;
      if (update.state === "finished" && update.command === "identify" && result) {
        if (typeof result.recordingId === "string") recordingId = result.recordingId;
        void send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result });
      }
      if (update.state === "finished" && update.command === "tune" && result) {
        lastFamily = familyOf(record(result.candidate)?.structure);
        void send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "tune", result });
      }
      // The other stages' operations (Validate, the automatic flow, Apply, Restore, Keep): their result follows their last update.
      if (update.state === "finished" && update.command !== "identify" && update.command !== "tune" && result) {
        void send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: update.command, result });
      }
    });
  }

  /** Relays the operation's updates to the page until the next open or the route's end. */
  function follow(operationIds: readonly string[], scope: LoopMessageScope): void {
    for (const subscription of subscriptions.splice(0)) subscription.dispose();
    subscriptions = operationIds.map((operationId) => relay(operationId, scope));
  }

  /**
   * Shows the page the latest update of each operation the engine runs for the loop that is still running (A8.6), so that a reopened
   * panel shows the progress it missed, and the page's Cancel reaches it. The route that owns the operation's kind adopts it: the
   * identification and the Tune here, the automatic flow, Validate, and Apply, Restore and Keep in their routes. On a project-scripted
   * loop (`projectTest`, C12) the identification campaign is the project test, which the experiment route adopts as well. An operation
   * that has ended shows nothing here: its result is in the study, which `controlStudio/open` answered. A status the engine cannot give
   * is not shown, and the updates still follow.
   */
  async function showRunning(operationIds: readonly string[], scope: LoopMessageScope, projectTest: boolean): Promise<void> {
    for (const operationId of operationIds) {
      const reply = await client.status({ operationId }).catch(() => undefined);
      if (!reply || !("result" in reply) || reply.result.state !== "running") continue;
      const update = reply.result;
      if (update.command === "identify") {
        runningOperation = operationId;
        if (projectTest) await experimentRoute.adopt(operationId, scope);
      }
      if (update.command === "tune") runningTune = operationId;
      if (update.command === "run") runRoute.adopt(operationId);
      if (update.command === "validate") validateRoute.adopt(operationId);
      if (update.command === "apply" || update.command === "restore" || update.command === "keep") applicationRoute.adopt(operationId);
      await send({ ...scopeFields(scope), kind: "control-studio.engine.update", operationId, update });
    }
  }

  /**
   * The engine's request of the page's Tune command (B4.1, B4.2): the method, the structure the engineer fixed, the pins and the
   * entered settings. The response is not sent: the engine reads the one the study declares (B4.3). Undefined when the engine does
   * not run the method, or when pins have no family to belong to, so that the TypeScript route keeps the command.
   */
  function tuneRequest(loop: EngineLoop, scope: LoopMessageScope, payload: Record<string, unknown>): TuneRequest | undefined {
    const preferences = record(payload.tuningPreferences);
    const page = String(preferences?.method ?? "automatic");
    if (!ENGINE_METHOD_OF_PAGE.has(page)) return undefined;
    const method = ENGINE_METHOD_OF_PAGE.get(page);
    const structure = familyOf(payload.family);
    const pinned = record(payload.pinnedValues);
    let pins: ControllerSettingsPins | undefined;
    if (pinned && Object.keys(pinned).length > 0) {
      const family = structure ?? lastFamily ?? familyOf(options.tuningInputs?.()?.settingsPins?.family);
      if (!family) return undefined;
      pins = { family, loopRevision: scope.loopRevision, values: { ...(pinned as Record<string, number>) } };
    }
    const settings = method === "manual" ? record(preferences?.values) : undefined;
    return {
      ...(method ? { method } : {}),
      ...(structure ? { structure } : {}),
      ...(pins ? { pins } : {}),
      ...(settings ? { settings: settings as Record<string, number> } : {}),
      loopId: loop.id,
      projectRoot: options.projectRoot,
      runtimeId: loop.runtimeId,
      storageFolder: options.storageFolder,
    };
  }

  /**
   * The engine's request of the page's manual preview (B4.5): the settings as the page holds them, with the controller choice
   * and the pins that the TypeScript session holds. Nothing else is sent: a preview keeps no journal and reaches no runtime.
   */
  function previewRequest(loop: EngineLoop, scope: LoopMessageScope, payload: Record<string, unknown>): PreviewRequest | undefined {
    const settings = record(payload.settings);
    if (!settings || Object.keys(payload).join(",") !== "settings") return undefined;
    // The inputs the engine recorded come first; the TypeScript session's are kept for a study the engine has not recorded yet.
    const recorded = inputsRoute.recorded();
    const inputs = options.tuningInputs?.();
    const structure = recorded.structure ?? familyOf(inputs?.controllerChoice);
    const pinned = inputs?.settingsPins;
    const pinFamily = familyOf(pinned?.family);
    const pins: ControllerSettingsPins | undefined = recorded.pins ?? (pinned && pinFamily && Object.keys(pinned.values).length > 0
      ? { family: pinFamily, loopRevision: scope.loopRevision, values: { ...pinned.values } }
      : undefined);
    return {
      loopId: loop.id,
      projectRoot: options.projectRoot,
      runtimeId: loop.runtimeId,
      settings: settings as Record<string, number>,
      ...(structure ? { structure } : {}),
      ...(pins ? { pins } : {}),
    };
  }

  /** Cancels the Tune the engine runs for the page (Abort); false when none runs, so the TypeScript route keeps the command. */
  async function cancelTune(scope: LoopMessageScope): Promise<boolean> {
    const operationId = runningTune;
    if (!operationId) return false;
    const reply = await ask(scope, "tune", () => client.cancel({ operationId }));
    if (reply && "failure" in reply) await postFailure(scope, "tune", reply.failure);
    return true;
  }

  /** What the stages' own routes use of this route (`engine/routes/`). */
  const context: EngineRouteContext = {
    client,
    projectRoot: options.projectRoot,
    storageFolder: options.storageFolder,
    served: () => served,
    post: (scope, message) => send({ ...scopeFields(scope), ...message }),
    postFailure,
    ask: (scope, method, run) => ask(scope, method, run),
    connection: connectionOf,
    follow: (operationId, scope) => {
      subscriptions.push(relay(operationId, scope));
    },
  };
  /** The study inputs the engine recorded for the served loop: Identify's settings and the preview's pins come from here (S7). */
  const inputsRoute = createInputsRoute(context);
  // The stage routes that adopt a reopened operation (A8.6) are kept apart, so that `showRunning` can name each one.
  /** The automatic flow's route: a run the engine keeps running is adopted here when the panel reopens the loop (A8.6). */
  const runRoute = createRunRoute(context);
  const validateRoute = createValidateRoute(context);
  const applicationRoute = createApplicationRoute(context);
  const experimentRoute = createExperimentRoute(context);
  const stages: readonly EngineStageRoute[] = [validateRoute, inputsRoute, runRoute, applicationRoute, experimentRoute];

  /** Cancels the identification the engine runs for the page (Cancel, B9.4); false when none runs, so the TypeScript route keeps it. */
  async function cancelIdentification(scope: LoopMessageScope): Promise<boolean> {
    const operationId = runningOperation;
    if (!operationId) return false;
    const reply = await ask(scope, "identify", () => client.cancel({ operationId }));
    if (reply && "failure" in reply) await postFailure(scope, "identify", reply.failure);
    return true;
  }

  return {
    async open(loop, scope) {
      if (parked) return false;
      served = undefined;
      runningOperation = undefined;
      recordingId = undefined;
      runningTune = undefined;
      lastFamily = undefined;
      for (const stage of stages) stage.reset?.();
      const request: OpenRequest = {
        loopId: loop.id,
        projectRoot: options.projectRoot,
        runtimeId: loop.runtimeId,
        storageFolder: options.storageFolder,
      };
      const reply = await ask(scope, "open", () => client.open(request));
      if (!reply) return false;
      if ("failure" in reply) {
        if (reply.failure.cause === "controller-capability") {
          parked = true;
          return false;
        }
        await postFailure(scope, "open", reply.failure);
        return false;
      }
      const result: OpenResult = reply.result;
      if (result.kind !== "opened") {
        await postFailure(scope, "open", unclassifiedFailure("The engine answered the project's loops, not the loop."));
        return false;
      }
      served = loop;
      storedPage = result.page;
      // The page follows the operations the engine runs for the loop (A8.6); the journaled operations are not updates it answers.
      follow(result.running, scope);
      await send({ ...scopeFields(scope), kind: "control-studio.engine.opened", result });
      await inputsRoute.opened(result.inputs, scope);
      await showRunning(result.running, scope, loop.experiment?.execution === "project-scripted");
      return true;
    },

    async navigation(message, scope) {
      const loop = served;
      if (!loop) return false;
      const value = record(message);
      if (!value || value.kind !== "control-studio.navigation.select" || !sameScope(value, scope)) return false;
      const stage = record(value.payload)?.stage;
      const page = typeof stage === "string" ? ENGINE_PAGE_OF_STAGE.get(stage) : undefined;
      // The page posts its selection again with each progress update; only a change of page is stored.
      if (page && page !== storedPage) {
        storedPage = page;
        const reply = await ask(scope, "page", () =>
          client.page({ loopId: loop.id, page, projectRoot: options.projectRoot, runtimeId: loop.runtimeId })
        );
        if (reply && "failure" in reply) {
          storedPage = undefined;
          await postFailure(scope, "page", reply.failure);
        }
      }
      return true;
    },

    async identify(message, scope) {
      const loop = served;
      const value = record(message);
      if (!loop || !value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      if (value.kind === "control-studio.identify.cancel") return cancelIdentification(scope);
      const acquisition = inputsRoute.recorded().acquisition ?? options.acquisitionSettings?.();
      const action = engineIdentifyAction(value.kind, record(value.payload) ?? {}, acquisition, recordingId);
      if (!action) return false;
      const connection = action.action === "identify" ? await connectionOf() : undefined;
      const request: IdentifyRequest = {
        action,
        ...(connection ? { connection } : {}),
        loopId: loop.id,
        projectRoot: options.projectRoot,
        runtimeId: loop.runtimeId,
        storageFolder: options.storageFolder,
      };
      const reply = await ask(scope, "identify", () => client.identify(request));
      // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
      if (!reply) return true;
      if ("failure" in reply) {
        await postFailure(scope, "identify", reply.failure);
        return true;
      }
      const answer = reply.result;
      if ("operationId" in answer && typeof answer.operationId === "string") {
        runningOperation = answer.operationId;
        subscriptions.push(relay(answer.operationId, scope));
        return true;
      }
      if ("stages" in answer && !("modelType" in answer)) {
        recordingId = undefined;
        await send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result: answer });
        return true;
      }
      await postFailure(scope, "identify", unclassifiedFailure("The engine answered the identification with a result the page does not show."));
      return true;
    },

    async tune(message, scope) {
      const loop = served;
      const value = record(message);
      if (!loop || !value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      if (value.kind === "control-studio.flow.cancel") return cancelTune(scope);
      if (value.kind !== "control-studio.flow.start") return false;
      const payload = record(value.payload);
      if (!payload || payload.from !== "tune") return false;
      const request = tuneRequest(loop, scope, payload);
      if (!request) return false;
      // With a runtime connection the engine reads the running controller's settings for the comparison (B4.5); Tune never writes to it.
      const connection = await connectionOf();
      const reply = await ask(scope, "tune", () => client.tune(connection ? { ...request, connection } : request));
      // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
      if (!reply) return true;
      if ("failure" in reply) {
        await postFailure(scope, "tune", reply.failure);
        return true;
      }
      runningTune = reply.result.operationId;
      subscriptions.push(relay(reply.result.operationId, scope));
      return true;
    },

    async preview(message, scope) {
      const loop = served;
      const value = record(message);
      if (!loop || !value || value.kind !== "control-studio.engineer.manual-preview" || !sameScope(value, scope)) return false;
      const request = previewRequest(loop, scope, record(value.payload) ?? {});
      if (!request) return false;
      const serial = ++previewSerial;
      const current = () => serial === previewSerial;
      const connection = await connectionOf();
      const reply = await ask(scope, "preview", () => client.preview(connection ? { ...request, connection } : request), current);
      // Only the latest request is answered: an answer to a request that a newer one superseded is dropped (B4.5).
      if (!reply || !current()) return true;
      if ("failure" in reply) {
        await postFailure(scope, "preview", reply.failure);
        return true;
      }
      await send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "preview", result: reply.result });
      return true;
    },

    async handle(message, scope) {
      const value = record(message);
      if (!served || !value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      for (const stage of stages) {
        if (await stage.handle(value, scope)) return true;
      }
      return false;
    },

    bindingFamily(family) {
      inputsRoute.bindingFamily(family);
    },

    dispose(): void {
      disposed = true;
      for (const stage of stages) stage.reset?.();
      served = undefined;
      runningOperation = undefined;
      recordingId = undefined;
      runningTune = undefined;
      for (const subscription of subscriptions.splice(0)) subscription.dispose();
    },
  };
}

let windowEngine: EngineProcess | undefined;

/**
 * The engine of this window. The first Control Studio loop starts it, and it ends with the extension (specification
 * 35, §0.1a).
 */
export function controlStudioEngine(context: vscode.ExtensionContext): EngineProcess {
  if (!windowEngine) {
    windowEngine = startEngine({ executable: engineExecutable(context) });
    context.subscriptions.push(windowEngine);
  }
  return windowEngine;
}

/**
 * The folder of the operation journals that the extension asks for: empty, so that the engine uses its own, truST's folder in the
 * user's local state folder, where the command line keeps its journals too (specification 35, §0.1a, A8.3).
 */
export function controlStudioStorageFolder(): string {
  return "";
}

/** A controller family as the engine names it, or undefined for any other value. */
function familyOf(value: unknown): Structure | undefined {
  return value === "P" || value === "PI" || value === "PD" || value === "PID" ? value : undefined;
}
