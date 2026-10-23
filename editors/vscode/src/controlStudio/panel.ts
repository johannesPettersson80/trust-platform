import { ThreePageBindingRoute, type StudioBindingOpen } from "./threePageBindingRoute";
import { routeControlStudioRuntimeStart, createControlStudioRuntimeStartHandler } from "./runtimeStartAction";
import { postEngineeringResult } from "./panelWorkflowSupport";
import { randomUUID } from "crypto";
import * as path from "path";
import * as vscode from "vscode";
import { configureControlStudioCommand, openControlStudioCommand, registerControlStudioCommands } from "./openCommand";
import { completePlantHandoff, isLoopRequest, requestedLoopIdentity, text, type RequestedLoop } from "./panelBindings";
import { controlStudioHtml } from "./panelPresentation";
import type { PlantControlStudioHandoff } from "./handoff";
import type { LoopDefinition } from "./generated/ProjectLoopsResult";
import type { Failure } from "./generated/Failure";
import type { LoopMessageScope, LoopMonitorPayload } from "./messages";
import { onDidReadSimulationSpeed } from "../simulationSpeed";
import { runtimeLifecycleService } from "../runtimeLifecycle";
import { saveControlStudioChartExport, type ChartExportEngine, type ChartExportHost, type ChartExportScope } from "./chartExportHost";
import {
  decodeControlStudioHostCommand,
  decodeLoopReady,
  loopMonitorEnvelope,
  plantMonitorEnvelope,
  type ControlStudioHostCommand,
} from "./messages";
import { loadPlantMonitor } from "./plantMonitor";
import { controlStudioEngine, controlStudioStorageFolder, createEnginePanelRoute, type EnginePanelRoute } from "./engine/enginePanelRoute";
import { runtimeConnectionOf, studyRuntimeConnection } from "./engine/engineConnection";
import { createEngineClient, type ControlStudioEngine } from "./engine/engineClient";
import { projectLoops } from "./engine/engineProjectLoops";
import { scopeFields, unclassifiedFailure } from "./engine/engineRouteContext";
import { refusalText } from "./loopSetup/setupEngine";
import { getBinaryPath } from "../binary";

export const CONTROL_STUDIO_OPEN_COMMAND = "trust-lsp.controlStudio.open";
/** The Start simulator handler, which keeps the start in flight. The engine client is made on the first start, not at activation. */
let studioSimulatorStart: ReturnType<typeof createControlStudioRuntimeStartHandler> | undefined;

function simulatorStartOf(context: vscode.ExtensionContext): ReturnType<typeof createControlStudioRuntimeStartHandler> {
  studioSimulatorStart ??= createControlStudioRuntimeStartHandler(runtimeLifecycleService, createEngineClient(controlStudioEngine(context)));
  return studioSimulatorStart;
}

const CONTROL_STUDIO_VIEW_TYPE = "trust-control-studio";

let panel: vscode.WebviewPanel | undefined;
let activeHandoff: PlantControlStudioHandoff | undefined;
let activeLoop: LoopDefinition | undefined;
let openRequestGeneration = 0;
/** The engine's side of the open loop: every page message of the loop goes to it, and the engine answers the page (§0.1a). */
let engineRoute: EnginePanelRoute | undefined;
/** The binding page of the loop's project, which the engine discovers and selects (B1.2, B1.3). */
let bindingRoute: ThreePageBindingRoute | undefined;
let editorSessionId = "";
let refreshTimer: NodeJS.Timeout | undefined;
/** The engine's monitor of the open loop (`controlStudio/monitor`) or of the plant handoff (`controlStudio/plantMonitor`), and the project folder the loop is in (A8.3). */
let monitorEngine: ControlStudioEngine | undefined;
let activeProjectRoot: string | undefined;
let activeScopeKey = "";
/** The target label the page shows (B9.2), as the engine last answered it, and the subscription that asks the engine on each speed read. */
let speedReadSubscription: vscode.Disposable | undefined;
let currentTargetLabel: string | undefined;

/** The engine's answer to an open request's identity: the loop it opens and the folder that holds it, or its refusal (A8.3, B1.2). */
type OpeningAnswer = { kind: "opened"; loop: LoopDefinition; projectRoot: string } | { kind: "refused"; failure: Failure };

/** The page's chart export: the engine checks and names it, and the save dialog takes the name and filter the engine gives (B10.5). */
async function handleChartExportMessage(engine: ChartExportEngine, message: unknown, scope: ChartExportScope): Promise<Awaited<ReturnType<typeof saveControlStudioChartExport>> | undefined> {
  if (!message || typeof message !== "object" || (message as { kind?: unknown }).kind !== "control-studio.chart.export") return undefined;
  const host: ChartExportHost = {
    showSaveDialog: async options => {
      const defaultUri = vscode.workspace.workspaceFolders?.[0]
        ? vscode.Uri.joinPath(vscode.workspace.workspaceFolders[0].uri, options.defaultFileName) : undefined;
      return vscode.window.showSaveDialog({ title: "Save export", defaultUri, filters: { [options.filterLabel]: [options.extension] } });
    },
    writeFile: async (uri, bytes) => { await vscode.workspace.fs.writeFile(uri as vscode.Uri, bytes); },
  };
  return saveControlStudioChartExport(engine, message, scope, host);
}

export function registerControlStudioPanel(context: vscode.ExtensionContext): void {
  registerControlStudioCommands(context, {
    configure: () => configureControlStudioCommand(context, showControlStudio),
    open: request => openControlStudioCommand(context, request, showControlStudio),
    // The engine answers the export of the open loop (specification 35, B10.5); the engineer picks the format and the file.
    exportRecording: () => exportActiveLoop(context),
  });
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => scheduleMonitorRefresh()),
    // Each process-image update of the active simulator session refreshes the loop's monitor too (B2).
    runtimeLifecycleService.onDidIoStateChange(() => scheduleMonitorRefresh())
  );
}

async function showControlStudio(
  context: vscode.ExtensionContext,
  request?: unknown, bindingOpen?: StudioBindingOpen
): Promise<void> {
  // The panel reads the loop's identity from the request and nothing else: the engine checks the loop file and answers what opens.
  const identity = requestedLoopIdentity(request);
  if (isLoopRequest(request) && !identity) {
    void vscode.window.showErrorMessage("Control Studio could not open: the open request does not name a loop.");
    return;
  }
  const handoff = identity ? undefined : completePlantHandoff(request);
  const nextScopeKey = controlStudioScopeKey(identity, handoff);
  if (panel && nextScopeKey === activeScopeKey) {
    panel.reveal(vscode.ViewColumn.Active);
    scheduleMonitorRefresh();
    return;
  }
  const openTicket = ++openRequestGeneration;
  const current = () => openTicket === openRequestGeneration;
  // A8.3: the engine decides the loop and the folder that holds it, and a refusal is shown with the engine's words and next action.
  const opening = identity ? await openingOf(context, identity) : undefined;
  if (!current()) return;
  if (opening?.kind === "refused") {
    void vscode.window.showErrorMessage(refusalText(opening.failure));
    return;
  }
  disposeEngineRoute();
  editorSessionId = bindingOpen?.sessionId ?? randomUUID();
  activeLoop = opening?.loop;
  activeHandoff = handoff;
  activeScopeKey = nextScopeKey;
  activeProjectRoot = opening?.projectRoot;
  engineRoute = opening ? createEnginePanelRoute({
    engine: controlStudioEngine(context),
    projectRoot: opening.projectRoot,
    storageFolder: controlStudioStorageFolder(),
    post: message => panel?.webview.postMessage(message),
    isCurrent: current,
    connection: () => studyRuntimeConnection(),
    runtimeMode: async () => (await runtimeLifecycleService.snapshot()).status.runtimeMode,
  }) : undefined;
  engineRoute?.bindingFamily(bindingOpen?.family);
  if (opening) {
    const loop = opening.loop;
    bindingRoute = new ThreePageBindingRoute(loop, loopMessageScope(loop), () => current() && activeLoop === loop,
      (request, sessionId, family) => showControlStudio(context, request, { sessionId, family }),
      { engine: createEngineClient(controlStudioEngine(context)), projectRoot: opening.projectRoot, languageTool: getBinaryPath(context, "trust-lsp", "server.path") });
  } else {
    bindingRoute = undefined;
  }
  monitorEngine = opening || handoff ? createEngineClient(controlStudioEngine(context)) : undefined;
  pinSpeedReads(activeLoop);
  const title = activeLoop?.name ?? text(activeHandoff?.plantId, "Unbound loop");
  if (panel) {
    panel.title = `Control Studio · ${title}`;
    panel.webview.html = controlStudioHtml(panel.webview, context, activeHandoff, activeLoop, editorSessionId, currentTargetLabel);
    panel.reveal(vscode.ViewColumn.Active);
    return;
  }
  panel = vscode.window.createWebviewPanel(
    CONTROL_STUDIO_VIEW_TYPE,
    `Control Studio · ${title}`,
    vscode.ViewColumn.Active,
    {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [vscode.Uri.file(path.join(context.extensionPath, "media")),
        vscode.Uri.file(path.join(context.extensionPath, "src", "webview"))],
    }
  );
  panel.webview.html = controlStudioHtml(panel.webview, context, activeHandoff, activeLoop, editorSessionId, currentTargetLabel);
  panel.webview.onDidReceiveMessage(async (message) => {
    if (message?.kind === "control-studio.chart.export") {
      const ownerPanel = panel;
      const ownerEditor = editorSessionId;
      const scope = activeLoop ? loopMessageScope(activeLoop) : activeHandoff ? {
        projectId: activeHandoff.projectId, editorSessionId, plantId: activeHandoff.plantId,
        plantRevision: activeHandoff.savedConfigurationRevision,
      } : undefined;
      if (!scope) return;
      const result = await handleChartExportMessage(createEngineClient(controlStudioEngine(context)), message, scope);
      if (panel === ownerPanel && editorSessionId === ownerEditor) await ownerPanel?.webview.postMessage({
        schemaVersion: 1, kind: "control-studio.chart.export.result", projectId: scope.projectId, editorSessionId: ownerEditor,
        ...("loopId" in scope ? { loopId: scope.loopId, expectedLoopRevision: scope.loopRevision }
          : { plantId: scope.plantId, expectedPlantRevision: scope.plantRevision }),
        requestId: message.payload?.requestId, result,
      });
      return;
    }

    // A plant handoff has no loop: its page is ready when it says so, and the engine's summary of the plant follows (B1.1).
    if (!activeLoop && activeHandoff && message?.kind === "control-studio.ready" && message.projectId === activeHandoff.projectId
      && message.editorSessionId === editorSessionId) {
      scheduleMonitorRefresh();
      return;
    }
    const loop = activeLoop;
    if (!loop) return;
    // Every other page message of the loop goes to the engine route. A message the route does not take is the page's own (a
    // navigation, a host command) or none of the engine's, and no other route takes it.
    const expected = loopMessageScope(loop);
    const route = engineRoute;
    const ownerPanel = panel;
    const stillCurrent = () => panel === ownerPanel && activeLoop === loop && editorSessionId === expected.editorSessionId;
    if (await routeControlStudioRuntimeStart(message, expected, loop.runtimeId, simulatorStartOf(context),
      async value => { if (stillCurrent()) await ownerPanel?.webview.postMessage(value); })) return;
    if (await route?.importFile(message, expected)) return;
    const hostCommand = decodeControlStudioHostCommand(message, expected);
    if (hostCommand) { await runControlStudioHostCommand(hostCommand); return; }
    if (await route?.preview(message, expected)) return;
    if (await bindingRoute?.handle(message, payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "bindings", payload))) return;
    if (await route?.navigation(message, expected)) return;
    if (await route?.identify(message, expected)) return;
    if (await route?.tune(message, expected)) return;
    if (await route?.handle(message, expected)) return;
    if (decodeLoopReady(message, expected)) await openLoopOnEngine(loop, expected, ownerPanel);
  }, undefined, context.subscriptions);
  panel.onDidDispose(() => {
    openRequestGeneration++;
    disposeEngineRoute();
    if (refreshTimer) clearTimeout(refreshTimer);
    speedReadSubscription?.dispose();
    speedReadSubscription = undefined;
    monitorEngine = undefined;
    activeProjectRoot = undefined;
    refreshTimer = undefined;
    activeHandoff = undefined;
    activeLoop = undefined;
    activeScopeKey = "";
    editorSessionId = "";
    panel = undefined;
  }, undefined, context.subscriptions);
}

/**
 * The loop an identity opens, as the engine answers it (`controlStudio/projectLoops` with the identity): the accepted loop, the folder
 * that declares it, or the refusal. The panel takes the engine's checked loop and never reads the loop file.
 */
async function openingOf(context: vscode.ExtensionContext, identity: RequestedLoop): Promise<OpeningAnswer> {
  const answer = await projectLoops(createEngineClient(controlStudioEngine(context)), {
    projectRoots: (vscode.workspace.workspaceFolders ?? []).map(folder => folder.uri.fsPath),
    runtimeId: identity.runtimeId,
    loopIdentity: { id: identity.id, projectId: identity.projectId, revision: identity.revision },
  });
  if ("failure" in answer) return { kind: "refused", failure: answer.failure };
  const { resolution, declaringRoot } = answer.result;
  if (resolution?.state === "accepted" && resolution.request && declaringRoot) {
    return { kind: "opened", loop: resolution.request.loopDefinition, projectRoot: declaringRoot };
  }
  return { kind: "refused", failure: resolution?.failure ?? unclassifiedFailure("Control Studio could not open this loop.") };
}

/**
 * Saves the open loop's results (B10.5). The engineer picks the format and the file; the engine writes the text of the export, and the
 * extension writes that text where the engineer chose. Nothing here decides what an export holds.
 */
async function exportActiveLoop(context: vscode.ExtensionContext): Promise<void> {
  const loop = activeLoop;
  const projectRoot = activeProjectRoot;
  if (!loop || !projectRoot) {
    void vscode.window.showInformationMessage("Open a Control Studio loop before exporting its results.");
    return;
  }
  const picked = await vscode.window.showQuickPick<{ label: string; detail: string; format: "json" | "csv" }>([
    { label: "JSON", detail: "The complete workflow result", format: "json" },
    { label: "CSV", detail: "The signal table of the recording the plant model was identified from", format: "csv" },
  ], { title: "Export Control Studio results" });
  if (!picked) return;
  const answer = await createEngineClient(controlStudioEngine(context)).exportLoop({
    projectRoot, runtimeId: loop.runtimeId, loopId: loop.id, format: picked.format,
  });
  if ("failure" in answer) {
    void vscode.window.showErrorMessage(answer.failure.detail || answer.failure.message);
    return;
  }
  const folder = vscode.workspace.workspaceFolders?.[0];
  const target = await vscode.window.showSaveDialog({
    title: "Save export",
    defaultUri: folder ? vscode.Uri.joinPath(folder.uri, answer.result.suggestedFileName) : undefined,
    filters: picked.format === "json" ? { "JSON data": ["json"] } : { "CSV data": ["csv"] },
  });
  if (!target) return;
  await vscode.workspace.fs.writeFile(target, new TextEncoder().encode(answer.result.text));
  void vscode.window.showInformationMessage(`Saved ${path.basename(target.fsPath)}.`);
}

/** The panel's scope key: the loop it shows, or the plant. */
function controlStudioScopeKey(loop: RequestedLoop | undefined, handoff: PlantControlStudioHandoff | undefined): string {
  if (loop) return `loop:${loop.projectId}:${loop.runtimeId}:${loop.id}`;
  if (handoff) return `plant:${handoff.projectId}:${handoff.runtimeId}:${handoff.plantId}`;
  return "unbound";
}

/**
 * The page is ready: the engine opens the loop and answers the page with its state, or with its refusal and the next action (§0.1a).
 * The binding page then publishes the project's loops.
 */
async function openLoopOnEngine(loop: LoopDefinition, expected: LoopMessageScope, ownerPanel: vscode.WebviewPanel | undefined): Promise<void> {
  void refreshMonitor();
  const route = engineRoute;
  if (!route) {
    await ownerPanel?.webview.postMessage({ ...scopeFields(expected), kind: "control-studio.engine.failure", method: "open",
      failure: unclassifiedFailure("Control Studio could not open this loop.") });
    return;
  }
  await route.open(loop, expected);
  await bindingRoute?.publish(payload => postEngineeringResult(ownerPanel, loop, expected.editorSessionId, "bindings", payload));
}

/** Drops the engine route of the panel's loop, and with it the binding page, when the panel opens another loop or closes. */
function disposeEngineRoute(): void {
  engineRoute?.dispose();
  engineRoute = undefined;
  bindingRoute = undefined;
}

/**
 * The page's host commands (A11.2) run the commands truST already has. Attaching to a runtime that is already running has
 * no command of its own yet, so connecting opens Devices & Connections, where that is done.
 */
async function runControlStudioHostCommand(command: ControlStudioHostCommand): Promise<void> {
  if (command === "connect-runtime") await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
  else await vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
}

function loopMessageScope(loop: LoopDefinition) {
  return {
    projectId: loop.projectId,
    editorSessionId,
    loopId: loop.id,
    loopRevision: loop.revision,
  };
}


function scheduleMonitorRefresh(): void {
  if (!panel || (!activeHandoff && !activeLoop) || refreshTimer) return;
  refreshTimer = setTimeout(() => {
    refreshTimer = undefined;
    void refreshMonitor();
  }, 250);
}

async function refreshMonitor(): Promise<void> {
  await Promise.all([refreshPlantMonitor(), refreshLoopMonitor()]);
}

/** The plant summary of the plant handoff: the engine decides it from the runtime's state and `plant.status` (specification 35, §0.1a). */
async function refreshPlantMonitor(): Promise<void> {
  const currentPanel = panel;
  const handoff = activeHandoff;
  const sessionId = editorSessionId;
  const engine = monitorEngine;
  if (!currentPanel || !handoff || !sessionId || !engine) return;
  const snapshot = await loadPlantMonitor(handoff, {
    lifecycleSnapshot: () => runtimeLifecycleService.snapshot(),
    authToken: (endpoint) => runtimeLifecycleService.controlAuthToken(endpoint),
    monitor: (request) => engine.plantMonitor(request),
  });
  if (panel !== currentPanel || activeHandoff !== handoff || editorSessionId !== sessionId) return;
  await currentPanel.webview.postMessage(
    plantMonitorEnvelope(handoff, sessionId, randomUUID(), snapshot)
  );
}

/**
 * The loop's live sample (B2, A3.4). The engine reads the runtime and decides the sample: `controlStudio/monitor` answers it, and this
 * posts the answer. The connection and the debug session come from one lifecycle snapshot, so they describe one runtime.
 */
async function refreshLoopMonitor(): Promise<void> {
  const currentPanel = panel;
  const loop = activeLoop;
  const sessionId = editorSessionId;
  const engine = monitorEngine;
  const projectRoot = activeProjectRoot;
  if (!currentPanel || !loop || !sessionId || !engine || !projectRoot) return;
  let sample: LoopMonitorPayload;
  let targetLabel: string | null | undefined;
  try {
    const lifecycle = await runtimeLifecycleService.snapshot();
    const answer = await engine.monitor({
      projectRoot, runtimeId: loop.runtimeId, loopId: loop.id, editorSessionId: sessionId,
      connection: await runtimeConnectionOf(lifecycle), runtimeSessionId: lifecycle.sessionId,
    });
    if ("failure" in answer) {
      sample = unavailableLoopSample(loop, answer.failure.message);
    } else {
      // The sample is posted as the engine answered it, its own source included.
      sample = answer.result.sample;
      targetLabel = answer.result.targetLabel;
    }
  } catch (error) {
    sample = unavailableLoopSample(loop, error instanceof Error ? error.message : String(error));
  }
  // The panel may have moved to another loop while the engine answered; that loop's page must not receive this sample.
  if (panel !== currentPanel || activeLoop !== loop || editorSessionId !== sessionId) return;
  await currentPanel.webview.postMessage(loopMonitorEnvelope(loopMessageScope(loop), randomUUID(), sample));
  if (targetLabel) {
    currentTargetLabel = targetLabel;
    await postTargetLabel(loop, targetLabel);
  }
}

/**
 * A monitor call the engine answers with a failure (its arguments do not decode) shows as unavailable with that reason. The engine
 * answers every other case itself, refusals included. The failure names no source, so this sample names none.
 */
function unavailableLoopSample(loop: LoopDefinition, reason: string): LoopMonitorPayload {
  return { availability: "unavailable", loopId: loop.id, reason };
}

/**
 * Each speed read of the loop's runtime asks the engine's monitor again, which answers the target's label when it changed (B9.2).
 * The status bar reads the runtime's speed at its own cadence, so the label follows the achieved speed as it did before. The
 * subscription ends when the loop changes or the panel closes.
 */
function pinSpeedReads(loop: LoopDefinition | undefined): void {
  speedReadSubscription?.dispose();
  currentTargetLabel = undefined;
  speedReadSubscription = loop
    ? onDidReadSimulationSpeed(read => {
      if (read.runtimeId === loop.runtimeId && activeLoop === loop) scheduleMonitorRefresh();
    })
    : undefined;
}

/** Posts the target label in the envelope the other host-to-page messages use; the page checks that envelope's scope. */
async function postTargetLabel(loop: LoopDefinition, label: string): Promise<void> {
  await panel?.webview.postMessage({ schemaVersion: 1, kind: "control-studio.target-label", messageId: randomUUID(),
    projectId: loop.projectId, editorSessionId, loopId: loop.id, expectedLoopRevision: loop.revision, label });
}
