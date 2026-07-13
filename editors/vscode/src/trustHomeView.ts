import {
  affectsTrustConfiguration,
  getTrustConfiguration,
} from "./configuration";
import * as vscode from "vscode";

import {
  isStructuralRuntimeLifecycleChange,
  runtimeLifecycleService,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
} from "./runtimeLifecycle";
import { isRecord } from "./runtimeLifecycleModel";
import type { RuntimeLifecycleTarget } from "./runtimeLifecycleModel";
import {
  openSelectedRuntimeToml,
  openStructuredTextDebuggerLogs,
} from "./runtimeRecoveryActions";
import {
  remoteLabelFromEndpoint,
  runtimeOptions,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  type RemoteRuntime,
  type SelectedRuntime,
} from "./trustHomeModel";
import {
  getSelectedRuntimeId,
  onDidChangeSelectedRuntime,
  setSelectedRuntimeId,
} from "./selectedRuntime";
import { CHECK_PROGRAM_COMMAND, onDidCheckProgram } from "./checkProgram";
import { onDidDebugReload } from "./debug";
import { compileGateReason, validityLine } from "./compileGate";
import type { CheckProgramResponse } from "./checkProgramModel";
import {
  listManagedRuntimes,
  onDidChangeManagedRuntimes,
  startManagedRuntime,
  stopManagedRuntime,
} from "./localRuntime";
import {
  attachManagedRuntimeAfterStart,
  disconnectManagedRuntimeAfterStop,
} from "./managedRuntimeSession";
import type { ManagedRuntime } from "./localRuntimeModel";
import { getWorkspaceProjectState } from "./workspaceProject";
import { LatestOnlyRevision } from "./latestOnlyRevision";
import {
  effectiveLifecycleEntryFailure,
  lifecycleActionSucceeded,
  type LifecycleAction,
} from "./lifecycleEntryFailure";
import {
  runtimeAuthoritySelection,
  runtimeModelSnapshotForLifecycle,
} from "./runtimeAuthoritySelection";
import {
  lifecycleTargetForSelectedRuntime,
  lockedActionForSelectedRuntime,
  runtimeOperationBlockReason,
  type RuntimeLockedAction,
} from "./runtimeOperationPolicy";
import { trustHomeWebviewHtml } from "./trustHomeWebview";
import {
  classifyCompileIssues,
  compileButtonState,
  compileSummary,
  disabledButtonState,
  displayProjectName,
  runtimeActionButtonState,
  type CompileState,
} from "./trustHomePresentation";
import {
  hasHmiDescriptor,
  newDiagramMenu,
  openOrCreateHmi,
} from "./trustHomeNavigation";
import {
  OPEN_DEVICES_ACTION,
  OPEN_RUNTIME_LOGS_ACTION,
  OPEN_RUNTIME_TOML_ACTION,
  SET_AUTH_TOKEN_ACTION,
  actionFailureMessage,
  connectFailureChoices,
  isReloadSuccess,
  lifecycleFailureMatchesSelected,
  reloadFailureMessage,
  startFailureChoices,
} from "./trustHomeFailures";

export { startFailureChoices } from "./trustHomeFailures";

/** Test-only bridge through the exact WebviewView `action` message path. */
export const TEST_RUN_SIDEBAR_ACTION_COMMAND =
  "trust-lsp.test.runSidebarAction";

// The ONE truST sidebar (WebviewView `trust.home`). It keeps one fixed layout:
//   • No project open  → Examples-first onboarding; no transport controls.
//   • Project open     → project label, Target picker, compact Compile + lifecycle actions,
//                        then visible truST destinations.
// Target selection is select-only (no Add/Connect sentinel). A remote NEVER renders Stop; it renders
// Disconnect because we only own our attach session.

class TrustHomeProvider implements vscode.WebviewViewProvider {
  private readonly renderRevision = new LatestOnlyRevision();
  static readonly viewType = "trust.home";

  private view?: vscode.WebviewView;
  // "Update running simulation" (sim-only): true once an .st/.pou file is saved after Start, cleared only after a
  // confirmed successful Apply.
  // This is honest save-based change detection — never claim "changed" without an actual save.
  private sourceChanged = false;
  private applyMessage = "";
  private applyMessageKind: "success" | "error" | "" = "";
  private lifecycleActionFailure:
    { readonly message: string; readonly action: LifecycleAction } | undefined;
  private compileState: CompileState = { kind: "unknown" };

  constructor(private readonly context: vscode.ExtensionContext) {
    this.context.subscriptions.push(
      onDidCheckProgram((result) => {
        this.setCompileState(result);
        void this.render();
      }),
    );
    this.context.subscriptions.push(
      onDidDebugReload((result) => {
        if (result.ok) {
          this.sourceChanged = false;
          this.applyMessage = "Running simulation updated.";
          this.applyMessageKind = "success";
        } else if (this.sourceChanged) {
          this.applyMessage = `Update failed: ${reloadFailureMessage(result, validityLine())}`;
          this.applyMessageKind = "error";
        }
        void this.render();
      }),
    );
  }

  markSourceChanged(): void {
    this.sourceChanged = true;
    this.applyMessage = "";
    this.applyMessageKind = "";
    if (
      this.compileState.kind === "clean" ||
      this.compileState.kind === "failed"
    ) {
      this.compileState = { kind: "dirty" };
    }
    void this.render();
  }

  resolveWebviewView(webviewView: vscode.WebviewView): void {
    this.view = webviewView;
    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [this.context.extensionUri],
    };
    webviewView.webview.html = trustHomeWebviewHtml(
      webviewView.webview,
      this.context.extensionUri,
    );
    webviewView.webview.onDidReceiveMessage((message) => {
      void this.onMessage(message);
    });
    webviewView.onDidChangeVisibility(() => {
      if (webviewView.visible) {
        void this.render();
      }
    });
    void this.render();
  }

  refresh(): void {
    void this.render();
  }

  lifecycleChanged(): void {
    // Entry-point-local lifecycle errors are historical once any shared
    // lifecycle transition occurs. Clear synchronously before scheduling an
    // async latest-only render so an older render cannot mutate newer state.
    if (
      this.lifecycleActionFailure &&
      lifecycleActionSucceeded(
        this.lifecycleActionFailure.action,
        runtimeLifecycleService.phase(),
      )
    ) {
      this.lifecycleActionFailure = undefined;
    }
    void this.render();
  }

  async runSidebarActionForTest(): Promise<void> {
    if (this.context.extensionMode !== vscode.ExtensionMode.Test) {
      throw new Error("The sidebar action test bridge is available only in tests.");
    }
    await this.onMessage({ type: "action" });
  }

  private readRemotes(): RemoteRuntime[] {
    const endpoints =
      getTrustConfiguration(runtimeLifecycleService.runtimeConfigTarget()).get<
        string[]
      >("runtime.fleetEndpoints", []) ?? [];
    const seen = new Set<string>();
    const remotes: RemoteRuntime[] = [];
    for (const raw of endpoints) {
      const endpoint = (raw ?? "").trim();
      if (!endpoint || seen.has(endpoint)) {
        continue;
      }
      seen.add(endpoint);
      remotes.push({ id: endpoint, label: remoteLabelFromEndpoint(endpoint) });
    }
    return remotes;
  }

  private storedSelectedId(
    remotes: RemoteRuntime[],
    managed: ManagedRuntime[],
  ): string {
    // Read the ONE shared store (§0.5.11) — written by this dropdown AND by graph nodes (Connect / Set
    // as run target). Fall back to the simulator if the stored target is no longer in the inventory.
    const stored = getSelectedRuntimeId();
    const valid = runtimeOptions(remotes, managed).some(
      (option) => option.id === stored,
    );
    return valid ? stored : SIMULATOR_RUNTIME_ID;
  }

  private resolveSelected(
    snapshot: RuntimeLifecycleSnapshot,
    remotes: RemoteRuntime[],
    managed: ManagedRuntime[],
  ): {
    readonly selected: SelectedRuntime;
    readonly remotes: RemoteRuntime[];
    readonly authorityTarget?: RuntimeLifecycleTarget;
  } {
    const authority = runtimeAuthoritySelection(
      snapshot,
      remotes,
      managed,
      this.storedSelectedId(remotes, managed),
    );
    return {
      selected: selectedRuntime({
        snapshot: runtimeModelSnapshotForLifecycle(snapshot, authority.target),
        remotes: authority.remotes,
        managed,
        selectedId: authority.selectedId,
        managedSessionId: authority.managedSessionId,
      }),
      remotes: authority.remotes,
      authorityTarget: authority.target,
    };
  }

  private async render(): Promise<void> {
    const view = this.view;
    if (!view) {
      return;
    }
    const revision = this.renderRevision.begin();
    const workspaceState = await getWorkspaceProjectState();
    const projectOpen = workspaceState.kind === "trust";
    const snapshot = await runtimeLifecycleService.snapshot();
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const resolved = this.resolveSelected(snapshot, remotes, managed);
    const options = runtimeOptions(resolved.remotes, managed);
    const selected = resolved.selected;
    const diagnostics = validityLine();
    const hmiLabel = (await hasHmiDescriptor()) ? "HMI" : "Create HMI";
    // Update running simulation is SIMULATOR-ONLY and only when the running sim's source changed.
    // Remote apply/restart/deploy lives on the runtime node, never here.
    const canApply =
      selected.kind === "simulator" &&
      selected.status === "running" &&
      this.sourceChanged;
    const updateGate = canApply
      ? compileGateReason(this.compileState, diagnostics, "update")
      : undefined;
    const lifecycleFailureMessage =
      snapshot.failure && lifecycleFailureMatchesSelected(snapshot, selected)
        ? actionFailureMessage(selected, {
            ok: false,
            failure: snapshot.failure,
          })
        : undefined;
    const effectiveLifecycleMessage = effectiveLifecycleEntryFailure(
      this.lifecycleActionFailure?.message,
      lifecycleFailureMessage,
      this.lifecycleActionFailure?.action,
      snapshot.starting ? "starting" : snapshot.status.runtimeState,
    );
    let visibleApplyMessage = "";
    let applyMessageKind: "success" | "error" | "" = "";
    if (updateGate) {
      visibleApplyMessage = updateGate;
      applyMessageKind = "error";
    } else if (this.applyMessageKind === "error") {
      visibleApplyMessage = this.applyMessage;
      applyMessageKind = "error";
    } else if (
      selected.kind === "simulator" &&
      selected.status === "running" &&
      this.applyMessage
    ) {
      visibleApplyMessage = this.applyMessage;
      applyMessageKind = this.applyMessageKind;
    } else if (effectiveLifecycleMessage) {
      visibleApplyMessage = effectiveLifecycleMessage;
      applyMessageKind = "error";
    }
    const phase = runtimeLifecycleService.phase();
    const operationInProgress = snapshot.operation !== undefined;
    const compileReason = runtimeOperationBlockReason(
      phase,
      "compile",
      operationInProgress,
    );
    const targetReason = runtimeOperationBlockReason(
      phase,
      "select_target",
      operationInProgress,
    );
    const actionReason =
      selected.status === "starting"
        ? undefined
        : runtimeOperationBlockReason(
            phase,
            lockedActionForSelectedRuntime(selected),
            operationInProgress,
          );
    const applyReason = runtimeOperationBlockReason(
      phase,
      "apply_changes",
      operationInProgress,
    );
    const compile = disabledButtonState(
      compileButtonState(this.compileState, diagnostics),
      compileReason,
    );
    const buttons = {
      compile,
      action: disabledButtonState(
        runtimeActionButtonState(selected),
        actionReason,
      ),
    };
    const actionHint = actionReason || selected.primary.hint || "";
    if (!this.renderRevision.isCurrent(revision) || this.view !== view) {
      return;
    }
    void view.webview.postMessage({
      type: "state",
      projectOpen,
      workspaceKind: workspaceState.kind,
      workspaceName: displayProjectName(workspaceState.folder?.name ?? ""),
      workspaceIssue: workspaceState.issue ?? "",
      options,
      selectedId: selected.id,
      selected,
      targetEnabled: !targetReason,
      targetTitle:
        targetReason || `Target: ${selected.label} — ${selected.statusLabel}`,
      buttons,
      actionHint,
      canApply,
      applyEnabled: canApply && !updateGate && !applyReason,
      applyTitle: applyReason || updateGate || "Update running simulation",
      applyMessage: visibleApplyMessage,
      applyMessageKind,
      recoveryAction:
        snapshot.failure?.kind === "configuration" &&
        lifecycleFailureMatchesSelected(snapshot, selected)
          ? { label: OPEN_RUNTIME_TOML_ACTION, action: "openRuntimeToml" }
          : undefined,
      hmiLabel,
    });
  }

  private async onMessage(message: unknown): Promise<void> {
    if (!isRecord(message)) {
      return;
    }
    switch (message.type) {
      case "ready":
        await this.render();
        return;
      case "select":
        await this.onSelect(String(message.id ?? ""));
        return;
      case "chooseTarget":
        await this.chooseTarget();
        return;
      case "compile":
        await this.compileProject();
        return;
      case "action":
        await this.runAction();
        return;
      case "applyChanges":
        await this.applyChanges();
        return;
      case "openRuntimeToml":
        await openSelectedRuntimeToml();
        return;
      // No-project welcome
      case "createProject":
        await this.createProjectFromWelcome();
        return;
      case "openProject":
        await vscode.commands.executeCommand(
          "workbench.action.files.openFolder",
        );
        return;
      case "startExample":
        await vscode.commands.executeCommand("trust.examples.start");
        return;
      case "navDevices":
        await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
        return;
      case "navLibraries":
        await vscode.commands.executeCommand("trust-lsp.libraries.open");
        return;
      case "navLiveValues":
        await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
        return;
      case "navHmi":
        await openOrCreateHmi();
        await this.render();
        return;
      default:
        return;
    }
  }

  private async createProjectFromWelcome(): Promise<void> {
    const workspaceState = await getWorkspaceProjectState();
    if (workspaceState.kind === "nonTrust" && workspaceState.folder) {
      await vscode.commands.executeCommand("trust-lsp.newProject", {
        targetUri: workspaceState.folder.uri,
        openWorkspace: false,
      });
      await this.render();
      return;
    }
    await vscode.commands.executeCommand("trust-lsp.newProject");
  }

  private async onSelect(id: string): Promise<void> {
    if (await this.rejectBlockedOperation("select_target")) {
      return;
    }
    if (id) {
      await setSelectedRuntimeId(id);
    }
    this.applyMessage = "";
    this.applyMessageKind = "";
    this.lifecycleActionFailure = undefined;
    await this.render();
  }

  private async chooseTarget(): Promise<void> {
    if (await this.rejectBlockedOperation("select_target")) {
      return;
    }
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const options = runtimeOptions(remotes, managed);
    const currentId = getSelectedRuntimeId() || SIMULATOR_RUNTIME_ID;
    const items: Array<vscode.QuickPickItem & { id?: string }> = [];
    for (const group of [
      { label: "Simulator", kind: "simulator" },
      { label: "Managed on this computer", kind: "local" },
      { label: "Runtime on another computer", kind: "remote" },
    ] as const) {
      const groupOptions = options.filter(
        (option) => option.kind === group.kind,
      );
      if (!groupOptions.length) {
        continue;
      }
      items.push({
        label: group.label,
        kind: vscode.QuickPickItemKind.Separator,
      });
      for (const option of groupOptions) {
        items.push({
          id: option.id,
          label: option.label,
          description: option.id === currentId ? "selected" : undefined,
        });
      }
    }
    const pick = await vscode.window.showQuickPick(items, {
      title: "Target",
      placeHolder: "Choose where truST should run or connect",
    });
    if (pick?.id) {
      await this.onSelect(pick.id);
    }
  }

  private async compileProject(): Promise<void> {
    if (await this.rejectBlockedOperation("compile")) {
      return;
    }
    const snapshot = await runtimeLifecycleService.snapshot();
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const resolved = this.resolveSelected(snapshot, remotes, managed);
    const selected = resolved.selected;
    const operation = await runtimeLifecycleService.runExclusiveOperation(
      "compile",
      lifecycleTargetForSelectedRuntime(selected),
      async () =>
        await vscode.commands.executeCommand<CheckProgramResponse | undefined>(
          CHECK_PROGRAM_COMMAND,
        ),
    );
    if (!operation.acquired) {
      this.applyMessage = operation.reason;
      this.applyMessageKind = "error";
    } else if (operation.value) {
      this.setCompileState(operation.value);
    }
    await this.render();
  }

  private async rejectBlockedOperation(
    action: RuntimeLockedAction,
  ): Promise<boolean> {
    const reason = runtimeOperationBlockReason(
      runtimeLifecycleService.phase(),
      action,
      runtimeLifecycleService.operationState() !== undefined,
    );
    if (!reason) {
      return false;
    }
    this.applyMessage = reason;
    this.applyMessageKind = "error";
    await this.render();
    return true;
  }

  private setCompileState(result: CheckProgramResponse): void {
    const issueCounts = classifyCompileIssues(result);
    this.compileState = result.ok
      ? { kind: "clean", summary: compileSummary(result) }
      : {
          kind: "failed",
          summary: compileSummary(result),
          errors: result.errors ?? result.issues?.length ?? 1,
          sourceErrors: issueCounts.sourceErrors,
          configErrors: issueCounts.configErrors,
        };
  }

  private async showWarning(
    message: string,
    ...items: string[]
  ): Promise<string | undefined> {
    if (this.context.extensionMode === vscode.ExtensionMode.Test) {
      return undefined;
    }
    try {
      return await vscode.window.showWarningMessage(message, ...items);
    } catch {
      return undefined;
    }
  }

  private async runAction(): Promise<void> {
    const snapshot = await runtimeLifecycleService.snapshot();
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const resolved = this.resolveSelected(snapshot, remotes, managed);
    const selected = resolved.selected;
    if (
      await this.rejectBlockedOperation(
        lockedActionForSelectedRuntime(selected),
      )
    ) {
      return;
    }
    if (!selected.primary.enabled && selected.primary.hint) {
      this.lifecycleActionFailure = {
        message: selected.primary.hint,
        action:
          selected.primary.action === "none"
            ? "other"
            : selected.primary.action,
      };
      await this.render();
      return;
    }
    // A managed local runtime is OURS — Start/Stop via the fleet lifecycle, not the debug simulator.
    if (selected.kind === "local") {
      await this.runManagedAction(selected, resolved.authorityTarget);
      await this.render();
      return;
    }
    let result: RuntimeLifecycleResult | undefined;
    if (selected.primary.action === "start") {
      let compile: CheckProgramResponse | undefined;
      const startResult = await runtimeLifecycleService.startLocalSimulator(
        async (projectRoot) => {
          compile = await vscode.commands.executeCommand<
            CheckProgramResponse | undefined
          >(CHECK_PROGRAM_COMMAND, { silent: true, projectRoot });
          return compile;
        },
      );
      if (compile) {
        this.setCompileState(compile);
      }
      if (!startResult.ok && "validationRejected" in startResult) {
        const summary = compileSummary(startResult.validationRejected);
        this.lifecycleActionFailure = undefined;
        this.applyMessage = summary;
        this.applyMessageKind = "error";
        await this.render();
        void this.showWarning(summary);
        return;
      }
      result = startResult;
      if (compile && !compile.ok) {
        if (result.ok || result.failure.kind !== "configuration") {
          const summary = compileSummary(compile);
          this.applyMessage = summary;
          this.applyMessageKind = "error";
        }
      }
    }
    result ??= await this.dispatch(selected);
    if (result && !result.ok) {
      const failureMessage = actionFailureMessage(selected, result);
      this.lifecycleActionFailure = {
        message: failureMessage,
        action:
          selected.primary.action === "none"
            ? "other"
            : selected.primary.action,
      };
      await this.render();
      if (selected.primary.action === "connect") {
        const choices = connectFailureChoices(result);
        const choice = await this.showWarning(failureMessage, ...choices);
        if (choice === SET_AUTH_TOKEN_ACTION) {
          await vscode.commands.executeCommand(
            "trust-lsp.runtime.setAuthToken",
            {
              endpoint: selected.id,
            },
          );
        } else if (choice === OPEN_DEVICES_ACTION) {
          await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
        }
      } else if (selected.primary.action === "start") {
        const choice = await this.showWarning(
          failureMessage,
          ...startFailureChoices(result.failure),
        );
        if (choice === OPEN_RUNTIME_TOML_ACTION) {
          await openSelectedRuntimeToml();
        } else if (choice === OPEN_RUNTIME_LOGS_ACTION) {
          openStructuredTextDebuggerLogs();
        }
      } else {
        void this.showWarning(failureMessage);
      }
    } else if (result?.ok) {
      this.lifecycleActionFailure = undefined;
      this.applyMessage = "";
      this.applyMessageKind = "";
      if (selected.primary.action === "start") {
        // A fresh Start compiles current source — nothing pending to apply.
        this.sourceChanged = false;
      }
      // Starting or connecting changes lifecycle state only. Live Values is an
      // explicit destination and must not steal focus from the user's editor.
    }
    await this.render();
  }

  private async runManagedAction(
    selected: SelectedRuntime,
    validatedAuthority: RuntimeLifecycleTarget | undefined,
  ): Promise<void> {
    const starting = selected.primary.action !== "stop";
    const operationTarget =
      !starting &&
      validatedAuthority?.kind === "managed" &&
      validatedAuthority.id === selected.id
        ? validatedAuthority
        : { kind: "managed" as const, id: selected.id };
    const operation = await runtimeLifecycleService.runExclusiveOperation(
      starting ? "managed_start" : "managed_stop",
      operationTarget,
      async (operationId) => {
        const result = starting
          ? await startManagedRuntime(this.context, selected.id)
          : await stopManagedRuntime(this.context, selected.id);
        if (!result.ok) {
          return { result };
        }
        if (starting) {
          return {
            result,
            attach: await attachManagedRuntimeAfterStart(
              selected.id,
              result,
              operationId,
            ),
          };
        }
        return {
          result,
          disconnect: await disconnectManagedRuntimeAfterStop(
            selected.id,
            result,
            operationId,
            validatedAuthority,
          ),
        };
      },
    );
    if (!operation.acquired) {
      this.applyMessage = operation.reason;
      this.applyMessageKind = "error";
      return;
    }
    const { result, attach, disconnect } = operation.value;
    if (!result.ok) {
      // Honest: the backend can report "starting"/"stopping" (didn't reach the target state) — surface
      // its message, don't pretend it worked, and don't auto-open Live Values.
      const reason =
        result.message ||
        `Could not ${selected.primary.action} ${selected.label}.`;
      void this.showWarning(`${reason} Check it in Devices & Connections.`);
      return;
    }
    // Keep the managed runtime attached in the background after Start so the
    // explicit Live Values action is ready, without revealing or focusing it.
    if (starting) {
      if (!attach?.ok) {
        void this.showWarning(
          attach?.message ||
            `Could not connect Live Values for ${selected.label}.`,
        );
        return;
      }
    } else {
      const disconnectResult = disconnect ?? {
        ok: false as const,
        failure: {
          kind: "stale_runtime" as const,
          message: `Stopped ${selected.label}, but its Live Values session outcome was not reported.`,
        },
      };
      if (!disconnectResult.ok) {
        const message =
          disconnectResult.failure.message ||
          `Stopped ${selected.label}, but could not close its Live Values session.`;
        this.lifecycleActionFailure = { message, action: "disconnect" };
        void this.showWarning(message);
        return;
      }
    }
    this.lifecycleActionFailure = undefined;
  }

  private async applyChanges(): Promise<void> {
    // Sim-only update (§0.6.6). The button is only shown when canApply, but guard anyway.
    if (await this.rejectBlockedOperation("apply_changes")) {
      return;
    }
    const gateReason = compileGateReason(
      this.compileState,
      validityLine(),
      "update",
    );
    if (gateReason) {
      this.applyMessage = gateReason;
      this.applyMessageKind = "error";
      await this.render();
      return;
    }
    const operation = await runtimeLifecycleService.runExclusiveOperation(
      "apply_changes",
      { kind: "simulator" },
      async () =>
        await vscode.commands.executeCommand<unknown>("trust-lsp.debug.reload"),
    );
    if (!operation.acquired) {
      this.applyMessage = operation.reason;
      this.applyMessageKind = "error";
      await this.render();
      return;
    }
    const result = operation.value;
    if (isReloadSuccess(result)) {
      this.sourceChanged = false;
      this.applyMessage = "Running simulation updated.";
      this.applyMessageKind = "success";
    } else {
      const reason = reloadFailureMessage(result, validityLine());
      this.sourceChanged = true;
      this.applyMessage = `Update failed: ${reason}`;
      this.applyMessageKind = "error";
    }
    await this.render();
  }

  private dispatch(
    selected: SelectedRuntime,
  ): Promise<RuntimeLifecycleResult> | undefined {
    switch (selected.primary.action) {
      case "start":
        return runtimeLifecycleService.startLocalSimulator();
      case "stop":
        return runtimeLifecycleService.stopRuntime();
      case "connect":
        return runtimeLifecycleService.connectRemote(
          selected.id,
          selected.label,
        );
      case "disconnect":
        // Disconnect ends our attach session — it does NOT kill a remote we don't own.
        return runtimeLifecycleService.stopRuntime();
      case "none":
      default:
        return undefined;
    }
  }
}

export function registerTrustHome(context: vscode.ExtensionContext): void {
  const provider = new TrustHomeProvider(context);
  if (context.extensionMode === vscode.ExtensionMode.Test) {
    context.subscriptions.push(
      vscode.commands.registerCommand(TEST_RUN_SIDEBAR_ACTION_COMMAND, () =>
        provider.runSidebarActionForTest(),
      ),
    );
  }
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.openSettings", () =>
      vscode.commands.executeCommand(
        "workbench.action.openSettings",
        "@ext:trust-platform.trust-lsp",
      ),
    ),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.visual.newDiagram", () =>
      newDiagramMenu(),
    ),
  );
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      TrustHomeProvider.viewType,
      provider,
      { webviewOptions: { retainContextWhenHidden: true } },
    ),
  );
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange((change) => {
      if (isStructuralRuntimeLifecycleChange(change)) {
        provider.lifecycleChanged();
      }
    }),
  );
  // Reflect target changes made on a graph node (Connect / Select as target) in the dropdown.
  context.subscriptions.push(
    onDidChangeSelectedRuntime(() => provider.refresh()),
  );
  // A managed local runtime starting/stopping (from here or a graph node) updates its Run-bar state.
  context.subscriptions.push(
    onDidChangeManagedRuntimes(() => provider.refresh()),
  );
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (affectsTrustConfiguration(event, "runtime.fleetEndpoints")) {
        provider.refresh();
      }
    }),
  );
  // The two sidebar states (no-project vs project-open) flip when folders open/close or a
  // trust-lsp.toml appears/disappears, and the validity line tracks diagnostics.
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(() => provider.refresh()),
  );
  const projectWatcher =
    vscode.workspace.createFileSystemWatcher("**/trust-lsp.toml");
  projectWatcher.onDidCreate(() => provider.refresh());
  projectWatcher.onDidDelete(() => provider.refresh());
  context.subscriptions.push(projectWatcher);
  const hmiWatcher = vscode.workspace.createFileSystemWatcher("**/hmi/*.toml");
  hmiWatcher.onDidCreate(() => provider.refresh());
  hmiWatcher.onDidDelete(() => provider.refresh());
  context.subscriptions.push(hmiWatcher);
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => provider.refresh()),
  );
  // Saving an ST source while the sim runs enables the sim-only update action.
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (/\.(st|pou)$/i.test(doc.uri.fsPath)) {
        provider.markSourceChanged();
      }
    }),
  );
}
