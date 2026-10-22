import type * as vscode from "vscode";
import { createEngineClient, type EngineAnswer, type IdentifyAnswer } from "./engineClient";
import { engineExecutable, startEngine, type EngineProcess } from "./engineProcess";
import { engineIdentifyAction, MANUAL_IDENTIFY_COMMANDS } from "./engineIdentify";
import type { Failure } from "../generated/Failure";
import type { IdentifyRequest, RuntimeConnection } from "../generated/IdentifyRequest";
import type { ManualIdentificationAnswer } from "../generated/ManualIdentificationAnswer";
import type { OpenRequest } from "../generated/OpenRequest";
import type { OpenResult } from "../generated/OpenResult";
import type { OperationUpdate } from "../generated/OperationUpdate";
import type { PageRequest } from "../generated/PageRequest";
import type { PreviewRequest } from "../generated/PreviewRequest";
import type { TuneMethod, TuneRequest } from "../generated/TuneRequest";
import {
  record,
  sameScope,
  scopeFields,
  unclassifiedFailure,
  type EngineLoop,
  type EngineMethod,
  type EngineRouteContext,
  type EngineScope,
  type EngineStageRoute,
} from "./engineRouteContext";
import { analysisPageOf, createAnalyzeRoute } from "./routes/analyzeRoute";
import type { AnalysisPage } from "../generated/PageRequest";
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
  /** The study's runtime connection for a live request (E11.1, A2.7); none when no runtime runs. */
  readonly connection?: () => Promise<RuntimeConnection | undefined>;
  /** The mode of the window's runtime lifecycle, which a live request without a connection carries (A2.8); none when unknown. */
  readonly runtimeMode?: () => Promise<string | undefined>;
}

/**
 * The engine's side of one open Control Studio loop (specification 35, §0.1a). The engine serves the loops it runs:
 * `open` answers the page, `navigation` stores the page the engineer views, `importFile` takes a recording file and a supplied
 * model, `identify` the study reset, a refit and the cancel, `tune` the Tune command and its Abort, and `preview` the manual preview.
 * A loop the engine refuses (`controller-capability`) shows the refusal and its next action on the page: the route answers the
 * page's stage messages of that loop with the refusal, and no other route opens (specification 35, §0.1a).
 */
export interface EnginePanelRoute {
  /**
   * Opens the loop when the page is ready. True when the engine serves it; false when it refuses the loop, and the refusal is then
   * shown on the page. The open answer says whether the loop offers the project test (`projectTest`, C12).
   */
  open(loop: EngineLoop, scope: EngineScope): Promise<boolean>;
  /** Stores the page the engineer views for a served loop. True when the message is consumed. */
  navigation(message: unknown, scope: EngineScope): Promise<boolean>;
  /**
   * Routes the study reset and the engine's refit of a kept recording (`control-studio.recording.refit`) to the engine, and the
   * cancel of an identification the engine runs (B3, A8.4, B3.6). True when the engine serves the loop and takes the command;
   * false when the loop is not served. The Recorded-data fit's steps (`control-studio.identify.edit`, `run`, `fit`,
   * `choose-structure`, `accept`) are the engine's (B3.3).
   */
  identify(message: unknown, scope: EngineScope): Promise<boolean>;
  /**
   * Opens a recording file (`control-studio.recording.import`), a supplied plant model (`control-studio.general-model.import`) or the
   * Recorded-data fit's file (`control-studio.identify.import`, B3.3) on a served loop (B3.6, B3.7): the engine checks it and keeps it,
   * and answers the page. True when the engine serves the loop and takes the file; false when the loop is not served.
   */
  importFile(message: unknown, scope: EngineScope): Promise<boolean>;
  /**
   * Routes the page's Tune command (a flow started at Tune) and its Abort to the engine (B4.1, B8). True when the engine serves
   * the loop and takes the command; false when the loop is not served.
   */
  tune(message: unknown, scope: EngineScope): Promise<boolean>;
  /** Answers the page's manual preview with the engine's prediction (B4.5). True when the engine serves the loop and takes it. */
  preview(message: unknown, scope: EngineScope): Promise<boolean>;
  /**
   * Routes a message of the other stages to the stage's own route (`engine/routes/`): Validate, the study inputs, the automatic
   * flow, and Apply, Restore and Keep. True when the engine serves the loop and a stage took the message. The page's analysis
   * (`control-studio.analysis.*`) is taken for every loop (B9.9).
   */
  handle(message: unknown, scope: EngineScope): Promise<boolean>;
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

interface Subscription {
  dispose(): void;
}

/** An engine answer to a recording that is not a recording's check, so the page has nothing it can show. */
const UNEXPECTED_RECORDING_ANSWER = "The engine answered the recording with a result the page does not show.";
/** A command of the Recorded-data fit the page sent in a form the engine has no action for. */
const UNSUPPORTED_COMMAND = "The engine does not take this command of the page.";
/**
 * The page's messages of the stages that a refused loop answers with the engine's refusal (§0.1a): each would reach a stage route of
 * a served loop. The page's navigation, its analysis tools (`control-studio.analysis.`), its bindings and the panel's own messages are
 * not among them.
 */
const REFUSED_STAGE_PREFIXES: readonly string[] = [
  "control-studio.identify.",
  "control-studio.recording.",
  "control-studio.general-model.",
  "control-studio.study.",
  "control-studio.flow.",
  "control-studio.engineer.",
  "control-studio.application.",
  "control-studio.experiment.",
  "control-studio.parameters.",
];

/** A Recorded-data fit's answer (`ManualIdentificationAnswer`, B3.3), by the kind the engine names (§0.1a). */
function isManualAnswer(value: unknown): value is ManualIdentificationAnswer {
  return record(value)?.kind === "manualIdentification";
}

export function createEnginePanelRoute(options: EnginePanelRouteOptions): EnginePanelRoute {
  const client = createEngineClient(options.engine);
  /** The refusal the engine gave the open loop (`controller-capability`): the page's stage messages of that loop are answered with it. */
  let refusal: Failure | undefined;
  let served: EngineLoop | undefined;
  /** The page the engine last stored for the served loop. */
  let storedPage: string | undefined;
  /** The Identify method the page showed last, as the study keeps it with the page (view state, A8.1). */
  let storedMethod: string | undefined;
  /** The identification the engine runs for the page; Cancel stops it. */
  let runningOperation: string | undefined;
  /** The recording of the study's last identification, which Fit refits (B3.6). */
  let recordingId: string | undefined;
  /** The Tune the engine runs for the page; Abort stops it. */
  let runningTune: string | undefined;
  /** The number of the latest manual preview the page asked for (B4.5). */
  let previewSerial = 0;
  let subscriptions: Subscription[] = [];
  let disposed = false;

  const send = (message: Record<string, unknown>): Promise<void> => {
    if (disposed || options.isCurrent?.() === false) return Promise.resolve();
    return Promise.resolve(options.post(message)).then(() => undefined);
  };

  const postFailure = (scope: EngineScope, method: EngineMethod, failure: Failure): Promise<void> =>
    send({ ...scopeFields(scope), kind: "control-studio.engine.failure", method, failure });

  /** The request of one identify action on a served loop (`controlStudio/identify`). */
  function identifyRequest(loop: EngineLoop, action: IdentifyRequest["action"]): IdentifyRequest {
    return { action, loopId: loop.id, projectRoot: options.projectRoot, runtimeId: loop.runtimeId, storageFolder: options.storageFolder };
  }

  /**
   * Shows a recording's check or fit (`ImportedRecording`, B3.6, B5.2) as `control-studio.identification-recording`: the status the engine
   * gives (`checked` or `recording-needed`), the recording the study keeps with its channels, and what the file still needs or the check
   * it passed. A fit the answer holds is an identification, sent as an Identify answer. Undefined when the answer is no recording's
   * check; the page is then told the engine's answer is not one it shows.
   */
  async function showRecording(scope: EngineScope, messageId: unknown, answer: Record<string, unknown>): Promise<string | undefined> {
    const kept = answer.recordingId;
    if (typeof kept !== "string" || answer.kind !== "importedRecording") {
      await postFailure(scope, "identify", unclassifiedFailure(UNEXPECTED_RECORDING_ANSWER));
      return undefined;
    }
    recordingId = kept;
    // The identification goes first: its result ends the page's flow, and the check's sentence then stays in the model message.
    const identification = record(answer.identification);
    if (identification) await send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result: identification });
    await send({ ...scopeFields(scope), kind: "control-studio.identification-recording", messageId, payload: { status: answer.status, recordingId: kept,
      ...(answer.recordingCatalog !== undefined ? { recordingCatalog: answer.recordingCatalog } : {}),
      ...(typeof answer.reason === "string" ? { reason: answer.reason } : {}),
      ...(typeof answer.validation === "string" ? { validation: answer.validation } : {}) } });
    return kept;
  }

  /**
   * Shows the page one identify answer (`controlStudio/identify`, B3.8, B3.6): an operation the page follows, a recording's check or fit,
   * the study's reset, or the engine's refusal. A failure is the engine's failure, shown as such.
   */
  async function answerIdentify(scope: EngineScope, messageId: unknown, reply: EngineAnswer<IdentifyAnswer>): Promise<void> {
    if ("failure" in reply) {
      await postFailure(scope, "identify", reply.failure);
      return;
    }
    // Each answer names its kind (§0.1a), and the route shows it by that word.
    const answer = record(reply.result) ?? {};
    switch (answer.kind) {
      case "operationStarted":
        if (typeof answer.operationId === "string") followIdentification(answer.operationId, scope);
        return;
      case "manualIdentification":
        await showManual(scope, answer as unknown as ManualIdentificationAnswer);
        return;
      case "importedRecording":
        await showRecording(scope, messageId, answer);
        return;
      case "reset": {
        recordingId = undefined;
        await send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result: answer });
        // A reset answers the Recorded-data fit of the new study too (B3.3): the page shows it as it shows the fit's answers.
        const kept = record(answer.manualIdentification);
        if (kept) await showManual(scope, kept as unknown as ManualIdentificationAnswer);
        return;
      }
    }
    await postFailure(scope, "identify", unclassifiedFailure("The engine answered the identification with a result the page does not show."));
  }

  /**
   * Shows the page the Recorded-data fit's answer (B3.3): its stage, status, primary action and label, its checks and model, as the engine
   * answers them. The route keeps nothing of it.
   */
  async function showManual(scope: EngineScope, answer: ManualIdentificationAnswer): Promise<void> {
    await send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result: answer });
  }

  /**
   * The Recorded-data fit's command of the page on a served loop (B3.3, B3.6): `identify.edit`, `run`, `fit`, `choose-structure`,
   * `accept`, and the file the page opened (`identify.import`). A check names no recording: the engine reads the study's last opened
   * recording, or refuses when it keeps none. The route claims every such command of a served loop, so no other route takes one.
   */
  async function manualIdentify(loop: EngineLoop, value: Record<string, unknown>, scope: EngineScope): Promise<boolean> {
    const action = engineIdentifyAction(String(value.kind), record(value.payload) ?? {});
    if (!action) {
      await postFailure(scope, "identify", unclassifiedFailure(UNSUPPORTED_COMMAND));
      return true;
    }
    const reply = await ask(scope, "identify", () => client.identify(identifyRequest(loop, action)));
    // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
    if (reply) await answerIdentify(scope, value.messageId, reply);
    return true;
  }

  /**
   * `importRecording` (B3.6, §0.1a): the engine checks the file and keeps it as a recording of the study, and the page shows the answer.
   * A refused file keeps nothing, and its refusal is the same message with status `failed`. A checked recording is fitted by the engine
   * in the same answer, which names the fit's operation (`operationId`) when it starts one; the page follows that operation (B3.6).
   */
  async function importRecording(loop: EngineLoop, value: Record<string, unknown>, scope: EngineScope): Promise<boolean> {
    const json = record(value.payload)?.json;
    if (typeof json !== "string") return false;
    const reply = await ask(scope, "identify", () => client.identify(identifyRequest(loop, { action: "importRecording", json })));
    if (!reply) return true;
    if ("failure" in reply) {
      await send({ ...scopeFields(scope), kind: "control-studio.identification-recording", messageId: value.messageId,
        payload: { status: "failed", reason: reply.failure.message } });
      return true;
    }
    const answer = record(reply.result);
    if (!answer) {
      await postFailure(scope, "identify", unclassifiedFailure(UNEXPECTED_RECORDING_ANSWER));
      return true;
    }
    const kept = await showRecording(scope, value.messageId, answer);
    // The engine's fit of the kept recording, when it started one, is followed as any identification is (B3.6).
    if (kept !== undefined && typeof answer.operationId === "string") followIdentification(answer.operationId, scope);
    return true;
  }

  /** Follows an identification the engine started for the page: its updates reach the page, and the page's Cancel stops it (B3.8). */
  function followIdentification(operationId: string, scope: EngineScope): void {
    runningOperation = operationId;
    subscriptions.push(relay(operationId, scope));
  }

  /**
   * `importModel` (B3.7, §0.1a): the engine checks the supplied model and keeps it as the study's model. Its answer is the
   * identification the page shows, as an Identify answer is.
   */
  async function importModel(loop: EngineLoop, value: Record<string, unknown>, scope: EngineScope): Promise<boolean> {
    const json = record(value.payload)?.json;
    if (typeof json !== "string") return false;
    const reply = await ask(scope, "identify", () => client.identify(identifyRequest(loop, { action: "importModel", json })));
    if (!reply) return true;
    if ("failure" in reply) {
      await postFailure(scope, "identify", reply.failure);
      return true;
    }
    recordingId = undefined;
    await send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result: reply.result });
    return true;
  }

  /**
   * One engine call. A call the engine cannot take is the engine's start failure, not the loop's: it is shown as
   * such.
   */
  async function ask<T>(
    scope: EngineScope,
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

  /**
   * A stage message of a loop the engine refused is answered with the engine's refusal, shown as the page shows any engine failure
   * (`control-studio.engine.failure`), so that the page shows the sentence and the next action and waits for no reply. Other messages
   * are left to the other routes (false).
   */
  async function answerRefusal(scope: EngineScope, value: Record<string, unknown>): Promise<boolean> {
    const kind = value.kind;
    if (!refusal || typeof kind !== "string" || !REFUSED_STAGE_PREFIXES.some(prefix => kind.startsWith(prefix))) return false;
    await postFailure(scope, "open", refusal);
    return true;
  }

  /**
   * Relays one operation's updates to the page; an identification's final result follows its last update (B3.8). An operation the
   * page started catches up on a final update sent before the subscription stood; one a reopened panel follows does not (A8.6).
   */
  function relay(operationId: string, scope: EngineScope, catchUp = true): Subscription {
    return client.onUpdate(operationId, (update: OperationUpdate) => {
      // An analysis operation's updates belong to the analysis route, which answers the page's analysis (B9.9).
      if (update.command === "analyze") return;
      if (update.state !== "running") {
        if (runningOperation === operationId) runningOperation = undefined;
        if (runningTune === operationId) runningTune = undefined;
      }
      void send({ ...scopeFields(scope), kind: "control-studio.engine.update", operationId, update });
      const result = update.result;
      if (update.state === "finished" && update.command === "identify" && result) {
        // The Recorded-data fit's final answer is the page's manual state (B3.3), where a fit's operation ends.
        if (isManualAnswer(result)) {
          void showManual(scope, result);
        } else {
          // A campaign keeps a recording of its own, which Fit refits (B3.6).
          if (typeof result.recordingId === "string") recordingId = result.recordingId;
          void send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "identify", result });
          // The kept recording's samples: the page draws the recorded signals it selects from them (`control-studio.recording.evidence`).
          if (Array.isArray(result.recordedSamples)) {
            void send({ ...scopeFields(scope), kind: "control-studio.recording.evidence", payload: { recording: {
              rawSamples: result.recordedSamples, evidence: { recordingId: result.recordingId } } } });
          }
        }
      }
      if (update.state === "finished" && update.command === "tune" && result) {
        void send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: "tune", result });
      }
      // The other stages' operations (Validate, the automatic flow, Apply, Restore, Keep): their result follows their last update.
      if (update.state === "finished" && update.command !== "identify" && update.command !== "tune" && result) {
        void send({ ...scopeFields(scope), kind: "control-studio.engine.result", method: update.command, result });
      }
    }, { catchUp });
  }

  /** Relays the operation's updates to the page until the next open or the route's end. */
  function follow(operationIds: readonly string[], scope: EngineScope): void {
    for (const subscription of subscriptions.splice(0)) subscription.dispose();
    subscriptions = operationIds.map((operationId) => relay(operationId, scope, false));
  }

  /**
   * Shows the page the latest update of each operation the engine runs for the loop that is still running (A8.6), so that a reopened
   * panel shows the progress it missed, and the page's Cancel reaches it. The route that owns the operation's kind adopts it: the
   * identification and the Tune here, the automatic flow, Validate, and Apply, Restore and Keep in their routes. On a project-scripted
   * loop (`projectTest`, C12) the identification campaign is the project test, which the experiment route adopts as well. An operation
   * that has ended shows nothing here: its result is in the study, which `controlStudio/open` answered. A status the engine cannot give
   * is not shown, and the updates still follow.
   */
  async function showRunning(operationIds: readonly string[], scope: EngineScope, projectTest: boolean): Promise<void> {
    for (const operationId of operationIds) {
      const reply = await client.status({ operationId }).catch(() => undefined);
      if (!reply || !("result" in reply) || reply.result.state !== "running") continue;
      const update = reply.result;
      if (update.command === "analyze") { analysis.adopt(operationId, scope, update); continue; }
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
   * The engine's request of the page's Tune command (B4.1, B4.2): the page's words, as the page sent them. The route decides none of
   * them. `automatic` is the page's word for no method, and is sent as none. The page sends a family only when the engineer chose one,
   * and it goes as a word, which the engine reads: `P`, `PI`, `PD` and `PID` fix that structure, and any other word fixes none (B4.4);
   * a request without one designs for the structure the study records (B4.2). The pins go
   * without a family, with the loop revision they belong to: the engine gives them the family of the structure, of the study's candidate
   * or of the study's pins, and ignores the pins of a state law (B4.2, C4). The settings go when the page sends them, which it does with
   * the manual method alone; the engine refuses them with another method. The response is not sent: the engine reads the one the study
   * declares (B4.3).
   */
  function tuneRequest(loop: EngineLoop, scope: EngineScope, payload: Record<string, unknown>): TuneRequest {
    const preferences = record(payload.tuningPreferences);
    const method = preferences?.method;
    const family = payload.family;
    const pinned = record(payload.pinnedValues);
    const settings = record(preferences?.values);
    return {
      ...(typeof method === "string" && method !== "automatic" ? { method: method as TuneMethod } : {}),
      ...(typeof family === "string" ? { structure: family as TuneRequest["structure"] } : {}),
      ...(pinned && Object.keys(pinned).length > 0
        ? { pins: { loopRevision: scope.loopRevision, values: { ...(pinned as Record<string, number>) } } }
        : {}),
      ...(settings ? { settings: settings as Record<string, number> } : {}),
      loopId: loop.id,
      projectRoot: options.projectRoot,
      runtimeId: loop.runtimeId,
      storageFolder: options.storageFolder,
    };
  }

  /**
   * The engine's request of the page's manual preview (B4.5): the settings as the page sent them. The engine reads the study's structure
   * and pins (B4.2, B4.5) and refuses settings it cannot predict, so the route sends nothing else: a preview keeps no journal. With a
   * runtime connected, the engine also reads the running controller's settings for the comparison (B4.5); the route then sends that
   * connection with the request (`preview` in `handle`).
   */
  function previewRequest(loop: EngineLoop, payload: Record<string, unknown>): PreviewRequest {
    return {
      loopId: loop.id,
      projectRoot: options.projectRoot,
      runtimeId: loop.runtimeId,
      settings: payload.settings as PreviewRequest["settings"],
    };
  }

  /** Cancels the Tune the engine runs for the page (Abort); false when none runs. */
  async function cancelTune(scope: EngineScope): Promise<boolean> {
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
    runtimeMode: () => options.runtimeMode ? options.runtimeMode().catch(() => undefined) : Promise.resolve(undefined),
    follow: (operationId, scope) => {
      subscriptions.push(relay(operationId, scope));
    },
  };
  /** The study inputs route: the page's changes of the inputs go to the engine in the order the page made them (S7). */
  const inputsRoute = createInputsRoute(context);
  /**
   * The page's analysis (B9.9), for every loop: the engine analyses the study's model, and a loop it refuses offers no tool. The inputs
   * the page changed are recorded first, as the other stages' calls wait for them.
   */
  const analysis = createAnalyzeRoute({
    client,
    projectRoot: options.projectRoot,
    post: message => send(message),
    served: () => served !== undefined,
    settled: () => inputsRoute.settled(),
  });
  // The stage routes that adopt a reopened operation (A8.6) are kept apart, so that `showRunning` can name each one.
  /** The automatic flow's route: a run the engine keeps running is adopted here when the panel reopens the loop (A8.6). */
  const runRoute = createRunRoute(context);
  const validateRoute = createValidateRoute(context);
  const applicationRoute = createApplicationRoute(context);
  const experimentRoute = createExperimentRoute(context);
  const stages: readonly EngineStageRoute[] = [validateRoute, inputsRoute, runRoute, applicationRoute, experimentRoute];

  /** Cancels the identification the engine runs for the page (Cancel, B9.4); false when none runs. */
  async function cancelIdentification(scope: EngineScope): Promise<boolean> {
    const operationId = runningOperation;
    if (!operationId) return false;
    const reply = await ask(scope, "identify", () => client.cancel({ operationId }));
    if (reply && "failure" in reply) await postFailure(scope, "identify", reply.failure);
    return true;
  }

  return {
    async open(loop, scope) {
      served = undefined;
      refusal = undefined;
      runningOperation = undefined;
      recordingId = undefined;
      runningTune = undefined;
      for (const stage of stages) stage.reset?.();
      analysis.reset();
      const request: OpenRequest = {
        loopId: loop.id,
        projectRoot: options.projectRoot,
        runtimeId: loop.runtimeId,
        storageFolder: options.storageFolder,
      };
      const reply = await ask(scope, "open", () => client.open(request));
      if (!reply) return false;
      if ("failure" in reply) {
        // A loop the engine refuses shows the refusal and its next action, and the page's stage messages get the same answer (§0.1a).
        refusal = reply.failure;
        await postFailure(scope, "open", reply.failure);
        await analysis.opened(loop, scope, undefined);
        return false;
      }
      const result: OpenResult = reply.result;
      if (result.kind !== "opened") {
        refusal = unclassifiedFailure("The engine answered the project's loops, not the loop.");
        await postFailure(scope, "open", refusal);
        await analysis.opened(loop, scope, undefined);
        return false;
      }
      served = loop;
      storedPage = result.page;
      storedMethod = result.pageView?.identifyMethod ?? undefined;
      // The recording the study's model was fitted from is the one a reopened page's Fit refits (B3.6, A8.6); none when it has no model.
      recordingId = typeof result.identification?.recordingId === "string" ? result.identification.recordingId : undefined;
      // The page follows the operations the engine runs for the loop (A8.6); the journaled operations are not updates it answers.
      follow(result.running, scope);
      // The Recorded-data fit the study keeps, or the engine's start state, comes with the open answer (A8.6, B3.3). It reaches the page once,
      // as the fit's answers do, and not again inside the open answer.
      const { manualIdentification: kept, ...shown } = result;
      await send({ ...scopeFields(scope), kind: "control-studio.engine.opened", result: shown });
      await inputsRoute.opened(result.inputs, scope);
      // The page shows the fit when it shows that method; the updates of an operation that runs follow it and replace it.
      if (kept) await showManual(scope, kept as ManualIdentificationAnswer);
      await showRunning(result.running, scope, result.projectTest === true);
      // The analysis the Tune page showed last, with its inputs, as the open answer names it (`pageAnalysis`, B9.9, A8.6).
      await analysis.opened(loop, scope, (result.pageAnalysis ?? undefined) as AnalysisPage | undefined);
      return true;
    },

    async navigation(message, scope) {
      const loop = served;
      if (!loop) return false;
      const value = record(message);
      if (!value || value.kind !== "control-studio.navigation.select" || !sameScope(value, scope)) return false;
      const stage = record(value.payload)?.stage;
      const page = typeof stage === "string" ? ENGINE_PAGE_OF_STAGE.get(stage) : undefined;
      const view = record(record(value.payload)?.view);
      const method = typeof view?.acquisitionMethod === "string" ? view.acquisitionMethod : undefined;
      // The page posts its selection again with each progress update; only a change of page or of its method is stored.
      if (page && (page !== storedPage || method !== storedMethod)) {
        storedPage = page;
        storedMethod = method;
        // The Tune page records the analysis it shows with the page, as the page names it in this message (B9.9); the other pages record none.
        const shown = page === "Tune" ? analysisPageOf(record(value.payload)?.analysis) : undefined;
        const reply = await ask(scope, "page", () =>
          client.page({
            loopId: loop.id, page, projectRoot: options.projectRoot, runtimeId: loop.runtimeId,
            ...(shown ? { analysis: shown } : {}), ...(method ? { view: { identifyMethod: method } } : {}),
          })
        );
        if (reply && "failure" in reply) {
          storedPage = undefined;
          storedMethod = undefined;
          await postFailure(scope, "page", reply.failure);
        }
      }
      return true;
    },

    async importFile(message, scope) {
      const loop = served;
      const value = record(message);
      if (!value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      if (!loop) return answerRefusal(scope, value);
      if (value.kind === "control-studio.recording.import") return importRecording(loop, value, scope);
      if (value.kind === "control-studio.general-model.import") return importModel(loop, value, scope);
      // The Recorded-data fit's file (B3.6) is checked and kept by the engine as the manual import.
      if (value.kind === "control-studio.identify.import") return manualIdentify(loop, value, scope);
      return false;
    },

    async identify(message, scope) {
      const loop = served;
      const value = record(message);
      if (!value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      if (!loop) return answerRefusal(scope, value);
      if (value.kind === "control-studio.identify.cancel") return cancelIdentification(scope);
      if (MANUAL_IDENTIFY_COMMANDS.has(value.kind)) return manualIdentify(loop, value, scope);
      // A refit names the recording the route holds, if any; the engine fits the study's last kept recording otherwise, or refuses (B3.6).
      const action = engineIdentifyAction(value.kind, record(value.payload) ?? {}, recordingId);
      if (!action) return false;
      const reply = await ask(scope, "identify", () => client.identify(identifyRequest(loop, action)));
      // The engine serves the loop even when it cannot be reached: its start failure is shown, and the command is not repeated.
      if (reply) await answerIdentify(scope, value.messageId, reply);
      return true;
    },

    async tune(message, scope) {
      const loop = served;
      const value = record(message);
      if (!value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      if (!loop) return answerRefusal(scope, value);
      if (value.kind === "control-studio.flow.cancel") return cancelTune(scope);
      if (value.kind !== "control-studio.flow.start") return false;
      const payload = record(value.payload);
      if (!payload || payload.from !== "tune") return false;
      // A served loop's Tune is the engine's (C2): the route claims it and sends the page's words; the engine refuses what it cannot run.
      const request = tuneRequest(loop, scope, payload);
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
      if (!value || value.kind !== "control-studio.engineer.manual-preview" || !sameScope(value, scope)) return false;
      if (!loop) return answerRefusal(scope, value);
      const request = previewRequest(loop, record(value.payload) ?? {});
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
      if (!value || typeof value.kind !== "string" || !sameScope(value, scope)) return false;
      // The study inputs are queued before any other step awaits (§0.1a `inputs`): a request the page made after them, such as a
      // Tune, then waits for them. Only the inputs route takes this message kind, so the order of the others is unchanged.
      const change = inputsRoute.handle(value, scope);
      if (await analysis.handle(value, scope)) return true;
      if (!served) return answerRefusal(scope, value);
      if (await change) return true;
      for (const stage of stages) {
        if (stage !== inputsRoute && await stage.handle(value, scope)) return true;
      }
      // The page's Abort of an identification the engine runs (a refit's fit, B3.6): the stages above take the Abort of the
      // operations they run, so this is the last of them.
      if (value.kind === "control-studio.flow.cancel") return cancelIdentification(scope);
      return false;
    },

    bindingFamily(family) {
      inputsRoute.bindingFamily(family);
    },

    dispose(): void {
      disposed = true;
      analysis.reset();
      for (const stage of stages) stage.reset?.();
      served = undefined;
      refusal = undefined;
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
