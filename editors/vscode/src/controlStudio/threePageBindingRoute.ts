import * as vscode from "vscode";
import * as fs from "fs";
import { randomUUID } from "crypto";
import { createControlStudioBindingSession, type WorkspaceControlProject } from "./hostBindingSession";
import { discoverControlStudioBindings, type ControlStudioBindingSelection } from "./hostBindingDiscovery";
import { controlLoopConfigurationPaths, type ProjectControlLoopOpenRequest } from "./projectLoop";
import { isControllerFamily, type ControllerFamily } from "./controllerFamilies";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopMessageScope } from "./messages";
import type { DiscoverControlSignals } from "./configureLoop";
import { storeEngineerDraft } from "./engineerDraftStorage";
import { storeEngineerStage } from "./engineerNavigationStorage";
import type { StudioAnalysisSessions } from "./studioAnalysisSessions";
import type { EngineerStudy } from "./engineerStudyPersistence";
import { connectedModelChannels } from "./loopChannels";
import type { ControlStudioEngine } from "./engine/engineClient";
import type { SetupDiscoverRequest } from "./generated/SetupDiscoverRequest";
import { engineAnswer, refusalText } from "./loopSetup/setupEngine";

export interface StudioBindingOpen {
  sessionId: string;
  family?: ControllerFamily;
  sourceLoop?: ControlLoopDefinition;
  sourceStudy?: EngineerStudy;
}

/** The engine route's hand-over of the controller family a binding selection chose, before the engine opens the loop (B1.3). */
export interface StudioBindingEngine {
  bindingFamily(family: ControllerFamily): void;
}

/**
 * Keep the same plant model when only the controller target changes. The controller family goes to the engine that serves the loop,
 * when the panel gives its port; otherwise it goes to the TypeScript session (B1.3).
 */
export async function adoptStudioBinding(context: vscode.ExtensionContext, loop: ControlLoopDefinition,
  sessions: StudioAnalysisSessions, scope: LoopMessageScope, selection: StudioBindingOpen, engine?: StudioBindingEngine): Promise<void> {
  const source = selection.sourceLoop, study = selection.sourceStudy, model = study?.selectedModel;
  if (model && source && source.projectId === loop.projectId && source.runtimeId === loop.runtimeId && source.plantId === loop.plantId) {
    let same = false;
    try {
      const identities = (binding: ControlLoopDefinition) => {
        const channels = connectedModelChannels(binding, model);
        return [...channels.inputs, ...channels.outputs].map(channel => channel.signal);
      };
      same = JSON.stringify(identities(source)) === JSON.stringify(identities(loop));
    } catch { /* A different plant/channel selection needs its own model. */ }
    if (same) {
      sessions.engineer?.assertModelReplacementAllowed();
      sessions.model.selectModel(model);
      const current = sessions.engineer?.snapshotStudy();
      sessions.engineer?.restoreStudy({ selectedModel: model, modelComparison: study.modelComparison,
        task: study.task, campaigns: study.campaigns, recordingCatalog: study.recordingCatalog, settingsPins: current?.settingsPins,
        controllerChoice: current?.controllerChoice, tuningPreferences: current?.tuningPreferences });
      sessions.engineer?.modelChanged();
    }
  }
  if (selection.family) {
    if (engine) engine.bindingFamily(selection.family);
    else await adoptStudioFamily(sessions, scope, selection.family);
  }
  await storeEngineerDraft(loop, sessions.engineer?.snapshotStudy());
  await storeEngineerStage(loop, { schemaVersion: 1, kind: "control-studio.navigation.select", ...scope,
    expectedLoopRevision: scope.loopRevision, payload: { stage: "tune" } }, scope);
}

/** Hands a controller family to the TypeScript session of a loop that the engine does not serve (until V7). */
export async function adoptStudioFamily(sessions: StudioAnalysisSessions, scope: LoopMessageScope, family: ControllerFamily): Promise<void> {
  await sessions.engineer?.handle({ schemaVersion: 1, kind: "control-studio.engineer.invalidate", messageId: randomUUID(),
    ...scope, expectedLoopRevision: scope.loopRevision, payload: { family } }, scope, async () => {});
}

/** The engine that discovers and selects the loops of the panel's project (B1.2, B1.3), and the project folder it answers for. */
export interface EngineBindingPort {
  readonly engine: ControlStudioEngine;
  readonly projectRoot: string;
  /** The language tool that reads the compiled project, when the extension names it (§0.1a). */
  readonly languageTool?: string;
}

/** Adapts the existing scoped webview messages to compiled binding discovery. */
export class ThreePageBindingRoute {
  private readonly session;
  private readonly handled = new Set<string>();
  private selecting = false;
  private family: ControllerFamily | undefined;
  constructor(private readonly loop: ControlLoopDefinition, private readonly scope: LoopMessageScope,
    discover: DiscoverControlSignals | undefined, private readonly current: () => boolean,
    private readonly assertCanChange: () => void,
    private readonly open: (request: ProjectControlLoopOpenRequest, sessionId: string, family?: ControllerFamily) => Promise<void>,
    /** When given, the engine discovers and selects the bindings; otherwise the TypeScript session does (until stage V7). */
    private readonly engineBindings?: EngineBindingPort) {
    this.session = createControlStudioBindingSession({
      scope: { projectId: scope.projectId, runtimeId: loop.runtimeId, sessionId: scope.editorSessionId },
      workspaceProjects: () => (vscode.workspace.workspaceFolders ?? []).map(folder => ({ id: loop.projectId, name: folder.name, rootUri: folder.uri.toString() })),
      selectWorkspaceProject: projects => this.project(projects),
      discover: async uri => {
        if (!discover) throw new Error("Compiled signal discovery is not ready. Reopen the controller selector after the project loads.");
        return discoverControlStudioBindings(vscode.Uri.parse(uri).fsPath, loop.runtimeId, await discover(uri));
      },
      newSessionId: () => randomUUID(),
      openNewSession: async value => {
        if (!this.current()) return;
        this.assertCanChange();
        await open(value.request, value.sessionId, this.family);
      },
    });
  }
  async publish(post: (payload: Record<string, unknown>) => Promise<void>): Promise<void> {
    if (this.engineBindings) return this.publishFromEngine(this.engineBindings, post);
    try {
      const result = await this.session.handle({ schemaVersion: 1, kind: "control-studio.binding.list", scope: this.bindingScope() });
      if (this.current()) await post({ ...result, status: result.status === "accepted" ? "bindings" : "failed" });
    } catch (error) { if (this.current()) await post({ status: "failed", reason: reason(error) }); }
  }
  async handle(value: unknown, post: (payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!record(value) || !["control-studio.binding.list", "control-studio.binding.select"].includes(String(value.kind))) return false;
    if (value.schemaVersion !== 1 || value.projectId !== this.scope.projectId || value.editorSessionId !== this.scope.editorSessionId
      || value.loopId !== this.scope.loopId || value.expectedLoopRevision !== this.scope.loopRevision || !record(value.payload)
      || typeof value.messageId !== "string" || !value.messageId || !this.current()) return true;
    if (this.handled.has(value.messageId) || this.selecting) return true;
    if (this.handled.size >= 4096) { await post({ status: "failed", reason: "Reopen Control Studio before changing its binding again." }); return true; }
    this.handled.add(value.messageId);
    if (value.kind === "control-studio.binding.list") { await this.publish(post); return true; }
    const payload = value.payload;
    try {
      if (this.engineBindings) { await this.selectWithEngine(this.engineBindings, payload, post); return true; }
      if (!selection(payload.selection) || payload.family !== undefined && !isControllerFamily(payload.family)) throw new Error("Choose a declared controller and its complete signal mapping.");
      const selectedBinding = payload.selection;
      this.assertCanChange(); this.selecting = true;
      this.family = isControllerFamily(payload.family) ? payload.family : undefined;
      // Recheck compiled identities and configuration immediately before selection.
      const listed = await this.session.handle({ schemaVersion: 1, kind: "control-studio.binding.list", scope: this.bindingScope() });
      if (!this.current()) return true;
      if (listed.status !== "accepted") throw new Error(listed.reason ?? "The configured project is unavailable.");
      if (this.family && "targets" in listed) {
        const compatible = listed.targets.find(target => target.loopId === selectedBinding.loopId)?.controllerCompatibility;
        if (compatible?.kind !== "accepted" || !compatible.families.includes(this.family)) throw new Error("Choose a configuration supported by that controller's declared adapter.");
      }
      const selected = await this.session.handle({ schemaVersion: 1, kind: "control-studio.binding.select", scope: this.bindingScope(), selection: selectedBinding });
      if (selected.status !== "accepted" && this.current()) await post({ status: "failed", page: payload.page, reason: selected.reason });
    } catch (error) { if (this.current()) await post({ status: "failed", page: payload.page, reason: reason(error) }); }
    finally { this.selecting = false; }
    return true;
  }
  private bindingScope() { return { projectId: this.scope.projectId, runtimeId: this.loop.runtimeId, sessionId: this.scope.editorSessionId }; }
  /** Lists the project's loops as the engine discovers them (B1.2): the loops it lists, and its refusals with their next action. */
  private async publishFromEngine(port: EngineBindingPort, post: (payload: Record<string, unknown>) => Promise<void>): Promise<void> {
    const answer = await engineAnswer(() => port.engine.setupDiscover({ projectRoot: port.projectRoot, runtimeId: this.loop.runtimeId,
      ...(port.languageTool ? { languageTool: port.languageTool } : {}) }));
    if (!this.current()) return;
    if ("failure" in answer) await post({ status: "failed", reason: refusalText(answer.failure) });
    else await post({ status: "bindings", targets: answer.result.loops, rejected: answer.result.rejected });
  }
  /**
   * Selects a binding through the engine (B1.3, C0): the engine checks the selection and the family against the loops it lists and names
   * the loop's file, which opens as a new session. A refusal is posted to the page with its next action.
   */
  private async selectWithEngine(port: EngineBindingPort, payload: Record<string, unknown>,
    post: (payload: Record<string, unknown>) => Promise<void>): Promise<void> {
    this.assertCanChange();
    this.selecting = true;
    const answer = await engineAnswer(() => port.engine.setupDiscover({
      projectRoot: port.projectRoot,
      runtimeId: this.loop.runtimeId,
      ...(port.languageTool ? { languageTool: port.languageTool } : {}),
      selection: payload.selection as SetupDiscoverRequest["selection"],
      family: payload.family as string | undefined,
    }));
    if (!this.current()) return;
    if ("failure" in answer) { await post({ status: "failed", page: payload.page, reason: refusalText(answer.failure) }); return; }
    const file = answer.result.selectedFile;
    if (!file) { await post({ status: "failed", page: payload.page, reason: "The engine did not name the selected loop." }); return; }
    this.family = isControllerFamily(payload.family) ? payload.family : undefined;
    const loopDefinition = JSON.parse(await fs.promises.readFile(file, "utf8"));
    const request: ProjectControlLoopOpenRequest = { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId }, loopDefinition };
    if (!this.current()) return;
    this.assertCanChange();
    await this.open(request, randomUUID(), this.family);
  }
  private async project(projects: readonly WorkspaceControlProject[]): Promise<WorkspaceControlProject | undefined> {
    if (projects.length === 1) return projects[0];
    const matches: WorkspaceControlProject[] = [];
    for (const project of projects) {
      for (const file of await controlLoopConfigurationPaths(vscode.Uri.parse(project.rootUri).fsPath)) {
        try {
          if ((await fs.promises.stat(file)).size > 512 * 1024) continue;
          const value = JSON.parse(await fs.promises.readFile(file, "utf8"));
          if (value.id === this.loop.id && value.projectId === this.loop.projectId && value.runtimeId === this.loop.runtimeId) { matches.push(project); break; }
        } catch { /* Missing/invalid definitions are reported by the owning discovery. */ }
      }
    }
    if (matches.length !== 1) throw new Error("Open Control Studio from the configured project to identify its controller bindings.");
    return matches[0];
  }
}
function record(value: unknown): value is Record<string, unknown> { return !!value && typeof value === "object" && !Array.isArray(value); }
function selection(value: unknown): value is ControlStudioBindingSelection {
  const ids = (input: unknown) => Array.isArray(input) && input.length > 0 && input.length <= 16 && input.every(id => typeof id === "string" && id.length > 0);
  return record(value) && typeof value.loopId === "string" && typeof value.controllerInstance === "string" && ids(value.inputIds) && ids(value.outputIds);
}
function reason(error: unknown): string { return error instanceof Error ? error.message : String(error); }
