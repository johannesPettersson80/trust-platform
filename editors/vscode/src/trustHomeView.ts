import * as vscode from "vscode";

import {
  runtimeLifecycleService,
  type RuntimeLifecycleResult,
  type RuntimeLifecycleSnapshot,
} from "./runtimeLifecycle";
import {
  remoteLabelFromEndpoint,
  runtimeOptions,
  selectedRuntime,
  SIMULATOR_RUNTIME_ID,
  withPrimaryActionGate,
  type RemoteRuntime,
  type RuntimeModelSnapshot,
  type SelectedRuntime,
} from "./trustHomeModel";
import {
  getSelectedRuntimeId,
  onDidChangeSelectedRuntime,
  setSelectedRuntimeId,
} from "./selectedRuntime";
import { CHECK_PROGRAM_COMMAND, onDidCheckProgram } from "./checkProgram";
import { onDidDebugReload } from "./debug";
import {
  compileGateReason,
  isConfigDiagnosticPath,
  validityLine,
  type ValidityLine,
} from "./compileGate";
import {
  summarizeCheck,
  type CheckProgramResponse,
} from "./checkProgramModel";
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

// The ONE truST sidebar (WebviewView `trust.home`). It keeps one fixed layout:
//   • No project open  → Examples-first onboarding; no transport controls.
//   • Project open     → project label, Target picker, compact Compile/Run/Debug/Deploy actions,
//                        then visible truST destinations.
// Target selection is select-only (no Add/Connect sentinel). A remote NEVER renders Stop; it renders
// Disconnect because we only own our attach session.
const SIDEBAR_ACTION_TIMEOUT_MS = 8000;

type CompileState =
  | { readonly kind: "unknown" }
  | { readonly kind: "dirty" }
  | { readonly kind: "clean"; readonly summary: string }
  | {
      readonly kind: "failed";
      readonly summary: string;
      readonly errors: number;
      readonly sourceErrors: number;
      readonly configErrors: number;
    };

type ButtonTone = "neutral" | "primary" | "success" | "warning" | "danger" | "disabled";
type ButtonVariant = "outline" | "filled";

interface SidebarButtonState {
  readonly state: string;
  readonly label: string;
  readonly title: string;
  readonly icon: string;
  readonly tone: ButtonTone;
  readonly variant: ButtonVariant;
  readonly enabled: boolean;
}

interface WorkspaceProjectState {
  readonly kind: "none" | "nonTrust" | "trust";
  readonly folder?: vscode.WorkspaceFolder;
}

class TrustHomeProvider implements vscode.WebviewViewProvider {
  static readonly viewType = "trust.home";

  private view?: vscode.WebviewView;
  // "Update running simulation" (sim-only): true once an .st/.pou file is saved after Start, cleared only after a
  // confirmed successful Apply.
  // This is honest save-based change detection — never claim "changed" without an actual save.
  private sourceChanged = false;
  private applyMessage = "";
  private applyMessageKind: "success" | "error" | "" = "";
  private compileState: CompileState = { kind: "unknown" };

  constructor(private readonly context: vscode.ExtensionContext) {
    this.context.subscriptions.push(
      onDidCheckProgram((result) => {
        this.setCompileState(result);
        void this.render();
      })
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
      })
    );
  }

  markSourceChanged(): void {
    this.sourceChanged = true;
    this.applyMessage = "";
    this.applyMessageKind = "";
    if (this.compileState.kind === "clean" || this.compileState.kind === "failed") {
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
    webviewView.webview.html = this.html(webviewView.webview);
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

  private readRemotes(): RemoteRuntime[] {
    const endpoints =
      vscode.workspace
        .getConfiguration("trust-lsp")
        .get<string[]>("runtime.fleetEndpoints", []) ?? [];
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
    managed: ManagedRuntime[]
  ): string {
    // Read the ONE shared store (§0.5.11) — written by this dropdown AND by graph nodes (Connect / Set
    // as run target). Fall back to the simulator if the stored target is no longer in the inventory.
    const stored = getSelectedRuntimeId();
    const valid = runtimeOptions(remotes, managed).some(
      (option) => option.id === stored
    );
    return valid ? stored : SIMULATOR_RUNTIME_ID;
  }

  private resolveSelected(
    snapshot: RuntimeLifecycleSnapshot,
    remotes: RemoteRuntime[],
    managed: ManagedRuntime[]
  ): SelectedRuntime {
    return selectedRuntime({
      snapshot: toModelSnapshot(snapshot),
      remotes,
      managed,
      selectedId: this.storedSelectedId(remotes, managed),
    });
  }

  private async render(): Promise<void> {
    if (!this.view) {
      return;
    }
    const workspaceState = await getWorkspaceProjectState();
    const projectOpen = workspaceState.kind === "trust";
    const snapshot = await runtimeLifecycleService.snapshot();
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const options = runtimeOptions(remotes, managed);
    const selectedRaw = this.resolveSelected(snapshot, remotes, managed);
    const diagnostics = validityLine();
    const primaryGate = primaryActionGateReason(
      selectedRaw,
      this.compileState,
      diagnostics
    );
    const selected = withPrimaryActionGate(
      selectedRaw,
      primaryGate ? { reason: primaryGate } : undefined
    );
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
      snapshot.failure &&
      selected.kind === "simulator" &&
      selected.status === "stopped"
        ? actionFailureMessage(selected, { ok: false, failure: snapshot.failure })
        : "";
    const applyMessage = updateGate
      ? updateGate
      : this.applyMessageKind === "error" ||
          (selected.kind === "simulator" && selected.status === "running")
        ? this.applyMessage
        : lifecycleFailureMessage;
    const applyMessageKind =
      updateGate ? "error" : this.applyMessageKind || (lifecycleFailureMessage ? "error" : "");
    const visibleApplyMessage =
      updateGate ||
      applyMessageKind === "error" ||
      (selected.kind === "simulator" && selected.status === "running")
        ? applyMessage
        : "";
    const compile = compileButtonState(this.compileState, diagnostics);
    const buttons = {
      compile,
      action: runtimeActionButtonState(selected),
      debug: debugButtonState(
        selected,
        compileGateReason(this.compileState, diagnostics, "debug")
      ),
      deploy: deployButtonState(selected, this.compileState),
    };
    const actionHint = !buttons.action.enabled ? buttons.action.title : selected.primary.hint ?? "";
    void this.view.webview.postMessage({
      type: "state",
      projectOpen,
      workspaceKind: workspaceState.kind,
      workspaceName: displayProjectName(workspaceState.folder?.name ?? ""),
      options,
      selectedId: selected.id,
      selected,
      buttons,
      actionHint,
      canApply,
      applyEnabled: canApply && !updateGate,
      applyTitle: updateGate || "Update running simulation",
      applyMessage: visibleApplyMessage,
      applyMessageKind,
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
      case "debug":
        await vscode.commands.executeCommand("trust-lsp.debug.start");
        return;
      case "deploy":
        void vscode.window.showInformationMessage(
          "Deploy is not available for this target yet."
        );
        return;
      case "applyChanges":
        await this.applyChanges();
        return;
      // No-project welcome
      case "createProject":
        await this.createProjectFromWelcome();
        return;
      case "openProject":
        await vscode.commands.executeCommand("workbench.action.files.openFolder");
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
    if (id) {
      await setSelectedRuntimeId(id);
    }
    this.applyMessage = "";
    this.applyMessageKind = "";
    await this.render();
  }

  private async chooseTarget(): Promise<void> {
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
      const groupOptions = options.filter((option) => option.kind === group.kind);
      if (!groupOptions.length) {
        continue;
      }
      items.push({ label: group.label, kind: vscode.QuickPickItemKind.Separator });
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
    const result = await vscode.commands.executeCommand<CheckProgramResponse | undefined>(
      CHECK_PROGRAM_COMMAND
    );
    if (result) {
      this.setCompileState(result);
    }
    await this.render();
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
    const selectedRaw = this.resolveSelected(snapshot, remotes, managed);
    const gateReason = primaryActionGateReason(
      selectedRaw,
      this.compileState,
      validityLine()
    );
    const selected = withPrimaryActionGate(
      selectedRaw,
      gateReason ? { reason: gateReason } : undefined
    );
    if (!selected.primary.enabled && selected.primary.hint) {
      this.applyMessage = selected.primary.hint;
      this.applyMessageKind = "error";
      await this.render();
      return;
    }
    // A managed local runtime is OURS — Start/Stop via the fleet lifecycle, not the debug simulator.
    if (selected.kind === "local") {
      await this.runManagedAction(selected);
      await this.render();
      return;
    }
    if (selected.primary.action === "start") {
      const compile = await vscode.commands.executeCommand<
        CheckProgramResponse | undefined
      >(CHECK_PROGRAM_COMMAND);
      if (compile) {
        this.setCompileState(compile);
        if (!compile.ok) {
          const summary = compileSummary(compile);
          this.applyMessage = summary;
          this.applyMessageKind = "error";
          await this.render();
          void this.showWarning(summary);
          return;
        }
      }
    }
    const dispatched = this.dispatch(selected);
    const result =
      selected.primary.action === "start" && dispatched
        ? await withSidebarActionTimeout(
            dispatched,
            SIDEBAR_ACTION_TIMEOUT_MS,
            "Start timed out. Check the runtime port or target settings."
          )
        : await dispatched;
    if (result && !result.ok) {
      const failureMessage = actionFailureMessage(selected, result);
      this.applyMessage = failureMessage;
      this.applyMessageKind = "error";
      await this.render();
      if (selected.primary.action === "connect") {
        const choices = connectFailureChoices(result);
        const choice = await this.showWarning(
          failureMessage,
          ...choices
        );
        if (choice === SET_AUTH_TOKEN_ACTION) {
          await vscode.commands.executeCommand("trust-lsp.runtime.setAuthToken", {
            endpoint: selected.id,
          });
        } else if (choice === OPEN_DEVICES_ACTION) {
          await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
        }
      } else {
        void this.showWarning(failureMessage);
      }
    } else if (result?.ok) {
      this.applyMessage = "";
      this.applyMessageKind = "";
      if (selected.primary.action === "start") {
        // A fresh Start compiles current source — nothing pending to apply.
        this.sourceChanged = false;
      }
      if (
        selected.primary.action === "start" ||
        selected.primary.action === "connect"
      ) {
        // Auto-reveal Live Values when the user starts the sim or connects a remote (§0.5.5).
        void vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
      }
    }
    await this.render();
  }

  private async runManagedAction(selected: SelectedRuntime): Promise<void> {
    const result =
      selected.primary.action === "stop"
        ? await stopManagedRuntime(this.context, selected.id)
        : await startManagedRuntime(this.context, selected.id);
    if (!result.ok) {
      // Honest: the backend can report "starting"/"stopping" (didn't reach the target state) — surface
      // its message, don't pretend it worked, and don't auto-open Live Values.
      const reason =
        result.message ||
        `Could not ${selected.primary.action} ${selected.label}.`;
      void this.showWarning(
        `${reason} Check it in Devices & Connections.`
      );
      return;
    }
    // Auto-reveal Live Values only when Start actually reached "running".
    if (selected.primary.action === "start") {
      const attach = await attachManagedRuntimeAfterStart(selected.id, result);
      if (!attach.ok) {
        void this.showWarning(
          attach.message || `Could not connect Live Values for ${selected.label}.`
        );
        return;
      }
      void vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
    } else if (selected.primary.action === "stop") {
      await disconnectManagedRuntimeAfterStop(selected.id, result);
    }
  }

  private async applyChanges(): Promise<void> {
    // Sim-only hot reload (§0.6.6). The button is only shown when canApply, but guard anyway.
    const gateReason = compileGateReason(this.compileState, validityLine(), "update");
    if (gateReason) {
      this.applyMessage = gateReason;
      this.applyMessageKind = "error";
      await this.render();
      return;
    }
    const result = await vscode.commands.executeCommand<unknown>("trust-lsp.debug.reload");
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
    selected: SelectedRuntime
  ): Promise<RuntimeLifecycleResult> | undefined {
    switch (selected.primary.action) {
      case "start":
        return runtimeLifecycleService.startLocalSimulator();
      case "stop":
        return runtimeLifecycleService.stopRuntime();
      case "connect":
        return runtimeLifecycleService.connectRemote(selected.id, selected.label);
      case "disconnect":
        // Disconnect ends our attach session — it does NOT kill a remote we don't own.
        return runtimeLifecycleService.stopRuntime();
      case "none":
      default:
        return undefined;
    }
  }

  private html(webview: vscode.Webview): string {
    const nonce = makeNonce();
    const themeUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.context.extensionUri, "src", "webview", "theme.css")
    );
    const codiconsUri = webview.asWebviewUri(
      vscode.Uri.joinPath(
        this.context.extensionUri,
        "node_modules",
        "@vscode",
        "codicons",
        "dist",
        "codicon.css"
      )
    );
    const csp = `default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; font-src ${webview.cspSource}; script-src 'nonce-${nonce}';`;
    return `<!DOCTYPE html>
	<html lang="en">
	<head>
	<meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
	<meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link rel="stylesheet" href="${themeUri}" />
  <link rel="stylesheet" href="${codiconsUri}" />
	<style>
	  * { box-sizing: border-box; }
	  body {
      margin: 0;
      padding: 10px 11px;
      font-family: var(--vscode-font-family);
      color: var(--trust-text);
      background: var(--vscode-sideBar-background, var(--trust-canvas));
    }
    .top {
      border-bottom: 1px solid var(--trust-border);
      padding-bottom: 10px;
      margin-bottom: 8px;
    }
    .project-name {
      color: var(--trust-text);
      font-size: 13px;
      font-weight: 700;
      line-height: 1.25;
      margin-bottom: 8px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .hint {
      color: var(--trust-text-muted);
      font-size: 11.5px;
      line-height: 1.4;
      margin: 0 0 9px;
    }
    button {
      font-family: var(--vscode-font-family);
    }
    .primary-start,
    .secondary-start,
    .target-button,
    .action-button,
    .update-button,
    .nav-button {
      border-radius: var(--trust-radius);
      cursor: pointer;
      min-width: 0;
      transition: background var(--trust-ease), border-color var(--trust-ease), color var(--trust-ease);
    }
    .primary-start,
    .secondary-start {
      align-items: center;
      display: flex;
      justify-content: center;
      min-height: 31px;
      width: 100%;
      margin-top: 7px;
      padding: 7px 9px;
      font-size: 12px;
      font-weight: 650;
    }
    .primary-start {
      background: var(--trust-action-primary-bg);
      border: 1px solid var(--trust-action-primary-bg);
      color: var(--trust-action-primary-fg);
    }
    .secondary-start {
      background: transparent;
      border: 1px solid var(--trust-border);
      color: var(--trust-text);
    }
    .target-label {
      color: var(--trust-text-muted);
      font-size: 10px;
      font-weight: 750;
      letter-spacing: 0.5px;
      margin: 0 0 4px;
      text-transform: uppercase;
    }
    .target-button {
      align-items: center;
      background: var(--trust-surface);
      border: 1px solid var(--trust-border);
      color: var(--trust-text);
      display: flex;
      gap: 7px;
      justify-content: space-between;
      min-height: 30px;
      padding: 6px 8px;
      width: 100%;
    }
    .target-button .value {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .action-row {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 5px;
      margin-top: 8px;
    }
    .action-button {
      align-items: center;
      background: transparent;
      border: 1px solid var(--trust-border);
      color: var(--trust-text);
      display: inline-flex;
      flex-direction: column;
      gap: 2px;
      justify-content: center;
      min-height: 42px;
      padding: 5px 3px;
    }
    .action-button .icon {
      font-size: 14px;
      line-height: 1;
    }
    .action-button .codicon {
      font-size: 15px;
    }
    .action-button .label {
      font-size: 10.5px;
      line-height: 1.1;
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .action-button.label-clipped .label { display: none; }
    .action-button[data-variant="filled"] {
      background: var(--trust-action-primary-bg);
      border-color: var(--trust-action-primary-bg);
      color: var(--trust-action-primary-fg);
      font-weight: 700;
    }
    .action-button[data-tone="success"] {
      border-color: color-mix(in srgb, var(--trust-ok) 55%, var(--trust-border));
      color: var(--trust-ok);
    }
    .action-button[data-tone="danger"] {
      border-color: color-mix(in srgb, var(--trust-danger) 58%, var(--trust-border));
      color: var(--trust-danger);
    }
    .action-button[data-tone="warning"] {
      border-color: color-mix(in srgb, var(--trust-warn) 58%, var(--trust-border));
      color: var(--trust-warn);
    }
    .action-button[data-tone="disabled"] {
      color: var(--trust-text-subtle);
    }
    button:hover:not(:disabled) {
      background: var(--trust-selected-bg);
      border-color: var(--trust-accent);
    }
    .action-button[data-variant="filled"]:hover:not(:disabled),
    .primary-start:hover:not(:disabled) {
      background: var(--trust-action-primary-hover-bg);
      border-color: var(--trust-action-primary-hover-bg);
      color: var(--trust-action-primary-fg);
    }
    button:disabled {
      color: var(--trust-text-subtle);
      cursor: not-allowed;
      opacity: 0.62;
    }
    .update-button {
      background: var(--trust-action-primary-bg);
      border: 1px solid var(--trust-action-primary-bg);
      color: var(--trust-action-primary-fg);
      display: none;
      font-size: 12px;
      font-weight: 650;
      margin-top: 7px;
      min-height: 30px;
      width: 100%;
    }
    .message {
      color: var(--trust-text-muted);
      display: none;
      font-size: 11px;
      line-height: 1.4;
      margin-top: 7px;
    }
    .message.success { color: var(--trust-ok); }
    .message.error { color: var(--trust-danger); }
    .hint-line {
      color: var(--trust-warn);
      display: none;
      font-size: 11px;
      line-height: 1.35;
      margin-top: 7px;
    }
    .nav {
      display: flex;
      flex-direction: column;
      gap: 3px;
    }
    .nav-button {
      align-items: center;
      background: transparent;
      border: 1px solid transparent;
      color: var(--trust-text);
      display: flex;
      gap: 8px;
      min-height: 31px;
      padding: 6px 7px;
      text-align: left;
      width: 100%;
    }
    .nav-button .nav-icon {
      color: var(--trust-text-muted);
      flex: 0 0 auto;
      text-align: center;
      width: 18px;
    }
    .nav-button:disabled {
      color: var(--trust-text-subtle);
      opacity: 0.72;
    }
    .nav-button:hover:not(:disabled) {
      background: var(--trust-selected-bg);
      border-color: var(--trust-border);
    }
    .disabled-reason {
      color: var(--trust-text-subtle);
      font-size: 10.5px;
      line-height: 1.25;
      margin: -1px 0 4px 26px;
    }
    @media (max-width: 245px) {
      .action-button .label { display: none; }
      .action-button { min-height: 32px; }
    }
	  .hidden { display: none; }
	</style>
	</head>
	<body>
	  <!-- No project open: same sidebar shell, onboarding top region only. -->
	  <section id="welcome" class="hidden">
      <div class="top">
        <div class="project-name" id="welcomeTitle">No project</div>
        <p class="hint" id="welcomeText">Start with a runnable example, create a blank project, or open an existing folder.</p>
        <button id="startExample" class="primary-start">▦ Start from example</button>
        <button id="createProject" class="secondary-start">+ Create project</button>
        <button id="openProject" class="secondary-start">Open project</button>
      </div>
      <nav class="nav" aria-label="truST destinations disabled until a project is open">
        <button class="nav-button" disabled><span class="nav-icon">▤</span><span>Devices &amp; Connections</span></button>
        <div class="disabled-reason">Open a project to use this.</div>
        <button class="nav-button" disabled><span class="nav-icon">▦</span><span>Libraries</span></button>
        <div class="disabled-reason">Open a project to use this.</div>
        <button class="nav-button" disabled><span class="nav-icon">◉</span><span>Live Values</span></button>
        <div class="disabled-reason">Start a project to watch values.</div>
        <button class="nav-button" disabled><span class="nav-icon">▭</span><span>Create HMI</span></button>
        <div class="disabled-reason">Open a project to use this.</div>
      </nav>
	  </section>

	  <!-- Project open: compact action surface + visible truST destinations. -->
	  <section id="project" class="hidden">
      <div class="top">
        <div class="project-name" id="projectName">Project</div>
        <div class="target-label">Target</div>
        <button id="targetButton" class="target-button" type="button">
          <span class="value" id="targetValue">Simulator</span>
          <span aria-hidden="true">▾</span>
        </button>
        <div class="action-row" aria-label="Run controls">
          <button id="compile" class="action-button" type="button" title="Compile project">
            <span class="icon codicon codicon-tools" id="compileIcon" aria-hidden="true"></span><span class="label" id="compileLabel">Compile</span>
          </button>
          <button id="action" class="action-button" type="button" disabled title="Selected target action">
            <span class="icon codicon codicon-play" id="actionIcon" aria-hidden="true"></span><span class="label" id="actionLabel">Run</span>
          </button>
          <button id="debug" class="action-button" type="button" title="Debug">
            <span class="icon codicon codicon-debug-alt" id="debugIcon" aria-hidden="true"></span><span class="label" id="debugLabel">Debug</span>
          </button>
          <button id="deploy" class="action-button" type="button" disabled title="Deploy is not available for this target yet">
            <span class="icon codicon codicon-rocket" id="deployIcon" aria-hidden="true"></span><span class="label" id="deployLabel">Deploy</span>
          </button>
        </div>
        <button id="apply" class="update-button" type="button">Update running simulation</button>
        <div class="message" id="applyMessage"></div>
        <div class="hint-line" id="hint"></div>
      </div>

      <nav class="nav" aria-label="truST destinations">
        <button class="nav-button" id="navDevices"><span class="nav-icon">▤</span><span>Devices &amp; Connections</span></button>
        <button class="nav-button" id="navLibraries"><span class="nav-icon">▦</span><span>Libraries</span></button>
        <button class="nav-button" id="navLiveValues"><span class="nav-icon">◉</span><span>Live Values</span></button>
        <button class="nav-button" id="navHmi"><span class="nav-icon">▭</span><span id="navHmiLabel">HMI</span></button>
      </nav>
	  </section>
	<script nonce="${nonce}">
	  const vscode = acquireVsCodeApi();
	  const welcomeEl = document.getElementById("welcome");
	  const projectEl = document.getElementById("project");
	  const welcomeTitle = document.getElementById("welcomeTitle");
	  const welcomeText = document.getElementById("welcomeText");
	  const createProjectEl = document.getElementById("createProject");
    const projectNameEl = document.getElementById("projectName");
    const targetButton = document.getElementById("targetButton");
    const targetValue = document.getElementById("targetValue");
    const compileEl = document.getElementById("compile");
    const compileIcon = document.getElementById("compileIcon");
    const compileLabel = document.getElementById("compileLabel");
	  const actionEl = document.getElementById("action");
    const actionIcon = document.getElementById("actionIcon");
    const actionLabel = document.getElementById("actionLabel");
    const debugEl = document.getElementById("debug");
    const debugIcon = document.getElementById("debugIcon");
    const debugLabel = document.getElementById("debugLabel");
    const deployEl = document.getElementById("deploy");
    const deployIcon = document.getElementById("deployIcon");
    const deployLabel = document.getElementById("deployLabel");
	  const applyEl = document.getElementById("apply");
	  const applyMessageEl = document.getElementById("applyMessage");
	  const hintEl = document.getElementById("hint");
	  const navHmiLabel = document.getElementById("navHmiLabel");

	  function post(type) { return () => vscode.postMessage({ type }); }
    function setButton(button, icon, label, view) {
      button.disabled = !view.enabled;
      button.title = view.title;
      button.dataset.baseTitle = view.title || "";
      button.dataset.state = view.state;
      button.dataset.tone = view.tone;
      button.dataset.variant = view.variant;
      label.textContent = view.label;
      icon.className = "icon codicon " + view.icon;
    }
    // CROSS-09: collapse an action label to icon-only (with the full label in the tooltip) when it would
    // clip in its 1/4 column, so "Disconnect"/"Connecting…" never render as "Disconn…".
    function measureLabelTextWidth(label) {
      const text = label.textContent || "";
      if (!text) { return 0; }
      const probe = document.createElement("span");
      const style = getComputedStyle(label);
      probe.textContent = text;
      probe.style.position = "absolute";
      probe.style.visibility = "hidden";
      probe.style.whiteSpace = "nowrap";
      probe.style.font = style.font;
      probe.style.letterSpacing = style.letterSpacing;
      document.body.appendChild(probe);
      const width = probe.getBoundingClientRect().width;
      probe.remove();
      return width;
    }
    function fitActionLabels() {
      document.querySelectorAll(".action-row .action-button").forEach((btn) => {
        const label = btn.querySelector(".label");
        if (!label) { return; }
        btn.classList.remove("label-clipped");
        const base = btn.dataset.baseTitle || "";
        const buttonStyle = getComputedStyle(btn);
        const available = btn.clientWidth -
          parseFloat(buttonStyle.paddingLeft || "0") -
          parseFloat(buttonStyle.paddingRight || "0");
        const measuredText = measureLabelTextWidth(label);
        // Chromium can still apply text-overflow ellipsis when measured text is only
        // fractionally below the content box. Keep a small reserve so long transport
        // labels collapse before they visibly truncate in the four-column action row.
        if (measuredText > Math.max(0, available - 4)) {
          btn.classList.add("label-clipped");
          const text = label.textContent || "";
          btn.title = text ? (base && base !== text ? text + " — " + base : text) : base;
        } else if (base) {
          btn.title = base;
        }
      });
    }
    window.addEventListener("resize", fitActionLabels);
    targetButton.addEventListener("click", post("chooseTarget"));
    compileEl.addEventListener("click", post("compile"));
	  actionEl.addEventListener("click", () => { if (!actionEl.disabled) { vscode.postMessage({ type: "action" }); } });
      debugEl.addEventListener("click", post("debug"));
    deployEl.addEventListener("click", () => { if (!deployEl.disabled) { vscode.postMessage({ type: "deploy" }); } });
	  applyEl.addEventListener("click", () => vscode.postMessage({ type: "applyChanges" }));
	  createProjectEl.addEventListener("click", post("createProject"));
	  document.getElementById("openProject").addEventListener("click", post("openProject"));
	  document.getElementById("startExample").addEventListener("click", post("startExample"));
	  document.getElementById("navDevices").addEventListener("click", post("navDevices"));
	  document.getElementById("navLibraries").addEventListener("click", post("navLibraries"));
	  document.getElementById("navLiveValues").addEventListener("click", post("navLiveValues"));
	  document.getElementById("navHmi").addEventListener("click", post("navHmi"));

  window.addEventListener("message", (event) => {
    const msg = event.data;
    if (!msg || msg.type !== "state") { return; }
	    welcomeEl.classList.toggle("hidden", msg.projectOpen);
	    projectEl.classList.toggle("hidden", !msg.projectOpen);
	    if (!msg.projectOpen) {
	      if (msg.workspaceKind === "nonTrust") {
	        const name = msg.workspaceName ? "“" + msg.workspaceName + "”" : "This folder";
	        welcomeTitle.textContent = "No truST project";
	        welcomeText.textContent = name + " does not contain a truST project yet. Initialize it here, open an existing project, or start from an example.";
	        createProjectEl.textContent = "Initialize truST here";
	      } else {
	        welcomeTitle.textContent = "No project";
	        welcomeText.textContent = "Start with a runnable example, create a blank project, or open an existing folder.";
	        createProjectEl.textContent = "+ Create project";
	      }
	      return;
	    }

      projectNameEl.textContent = msg.workspaceName || "truST project";
      projectNameEl.title = msg.workspaceName || "truST project";
      targetValue.textContent = msg.selected.label;
      targetButton.title = "Target: " + msg.selected.label + " — " + msg.selected.statusLabel;
      setButton(compileEl, compileIcon, compileLabel, msg.buttons.compile);
      setButton(actionEl, actionIcon, actionLabel, msg.buttons.action);
      setButton(debugEl, debugIcon, debugLabel, msg.buttons.debug);
      setButton(deployEl, deployIcon, deployLabel, msg.buttons.deploy);
      fitActionLabels();
	    applyEl.style.display = msg.canApply ? "block" : "none";
	    applyEl.disabled = !msg.applyEnabled;
	    applyEl.title = msg.applyTitle || "Update running simulation";
	    const applyMessage = msg.applyMessage || "";
	    applyMessageEl.textContent = applyMessage;
	    applyMessageEl.className = "message " + (msg.applyMessageKind || "");
	    applyMessageEl.style.display = applyMessage ? "block" : "none";
	    const hint = msg.actionHint || msg.selected.primary.hint || "";
	    hintEl.textContent = hint ? "⚠ " + hint : "";
	    hintEl.style.display = hint ? "block" : "none";
	    navHmiLabel.textContent = msg.hmiLabel || "HMI";
	  });

  vscode.postMessage({ type: "ready" });
</script>
</body>
</html>`;
  }
}

export function registerTrustHome(context: vscode.ExtensionContext): void {
  const provider = new TrustHomeProvider(context);
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.openSettings", () =>
      vscode.commands.executeCommand(
        "workbench.action.openSettings",
        "@ext:trust-platform.trust-lsp"
      )
    )
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("trust-lsp.visual.newDiagram", () =>
      newDiagramMenu()
    )
  );
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(
      TrustHomeProvider.viewType,
      provider,
      { webviewOptions: { retainContextWhenHidden: true } }
    )
  );
  context.subscriptions.push(
    runtimeLifecycleService.onDidChange(() => provider.refresh())
  );
  // Reflect run-target changes made on a graph node (Connect / Set as run target) in the dropdown.
  context.subscriptions.push(
    onDidChangeSelectedRuntime(() => provider.refresh())
  );
  // A managed local runtime starting/stopping (from here or a graph node) updates its Run-bar state.
  context.subscriptions.push(
    onDidChangeManagedRuntimes(() => provider.refresh())
  );
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("trust-lsp.runtime.fleetEndpoints")) {
        provider.refresh();
      }
    })
  );
  // The two sidebar states (no-project vs project-open) flip when folders open/close or a
  // trust-lsp.toml appears/disappears, and the validity line tracks diagnostics.
  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(() => provider.refresh())
  );
  const projectWatcher = vscode.workspace.createFileSystemWatcher("**/trust-lsp.toml");
  projectWatcher.onDidCreate(() => provider.refresh());
  projectWatcher.onDidDelete(() => provider.refresh());
  context.subscriptions.push(projectWatcher);
  const hmiWatcher = vscode.workspace.createFileSystemWatcher("**/hmi/*.toml");
  hmiWatcher.onDidCreate(() => provider.refresh());
  hmiWatcher.onDidDelete(() => provider.refresh());
  context.subscriptions.push(hmiWatcher);
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => provider.refresh())
  );
  // Saving an ST source while the sim runs enables the sim-only update action.
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (/\.(st|pou)$/i.test(doc.uri.fsPath)) {
        provider.markSourceChanged();
      }
    })
  );
}

// "Project open" = a workspace folder is open AND it is a truST project (has a trust-lsp.toml). Keep
// "no folder" and "non-truST folder" distinct so the first-run UI can explain exactly what happened.
async function getWorkspaceProjectState(): Promise<WorkspaceProjectState> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    return { kind: "none" };
  }
  const found = await vscode.workspace.findFiles(
    "**/trust-lsp.toml",
    "**/node_modules/**",
    1
  );
  return found.length > 0 ? { kind: "trust", folder } : { kind: "nonTrust", folder };
}

function displayProjectName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) {
    return "";
  }
  if (/^network[_-]+canvas[_-]+demo$/i.test(trimmed)) {
    return "Conveyor Demo";
  }
  if (!/[_-]/.test(trimmed)) {
    return trimmed;
  }
  return trimmed
    .split(/[_-]+/)
    .filter((part) => part.length > 0)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

async function hasHmiDescriptor(): Promise<boolean> {
  const found = await vscode.workspace.findFiles(
    "**/hmi/*.toml",
    "**/node_modules/**",
    1
  );
  return found.length > 0;
}

function compileButtonState(
  state: CompileState,
  diagnostics: ValidityLine
): SidebarButtonState {
  if (!diagnostics.ok) {
    return {
      state: "diagnostics-failed",
      label: `Compile ${diagnostics.errors}`,
      title: diagnostics.label,
      icon: "codicon-error",
      tone: "danger",
      variant: "outline",
      enabled: true,
    };
  }
  switch (state.kind) {
    case "clean":
      return {
        state: "clean",
        label: "Compile",
        title: state.summary,
        icon: "codicon-check",
        tone: "neutral",
        variant: "outline",
        enabled: true,
      };
    case "failed":
      return {
        state: "failed",
        label: `Compile ${state.errors}`,
        title: state.summary,
        icon: "codicon-error",
        tone: "danger",
        variant: "outline",
        enabled: true,
      };
    case "dirty":
      return {
        state: "dirty",
        label: "Compile",
        title: "Source changed — compile again.",
        icon: "codicon-warning",
        tone: "warning",
        variant: "outline",
        enabled: true,
      };
    case "unknown":
    default:
      return {
        state: "unknown",
        label: "Compile",
        title: "Compile the project and show Problems if it fails.",
        icon: "codicon-tools",
        tone: "neutral",
        variant: "outline",
        enabled: true,
      };
  }
}

function runtimeActionButtonState(selected: SelectedRuntime): SidebarButtonState {
  const action = selected.primary.action;
  const enabled = selected.primary.enabled;
  const title = selected.primary.hint || selected.statusLabel || selected.primary.label;
  switch (action) {
    case "start":
      return {
        state: "start",
        label: selected.primary.label,
        title,
        icon: "codicon-play",
        tone: enabled ? "primary" : "disabled",
        variant: enabled ? "filled" : "outline",
        enabled,
      };
    case "connect":
      return {
        state: enabled ? "connect" : "connect-disabled",
        label: selected.primary.label,
        title,
        icon: "codicon-remote",
        tone: enabled ? "primary" : "disabled",
        variant: enabled ? "filled" : "outline",
        enabled,
      };
    case "stop":
      return {
        state: "stop",
        label: selected.primary.label,
        title,
        icon: "codicon-stop",
        tone: "neutral",
        variant: "outline",
        enabled,
      };
    case "disconnect":
      return {
        state: "disconnect",
        label: selected.primary.label,
        title,
        icon: "codicon-debug-disconnect",
        tone: "neutral",
        variant: "outline",
        enabled,
      };
    case "none":
    default:
      return {
        state: selected.status === "starting" ? "busy" : "disabled",
        label: selected.primary.label,
        title,
        icon: selected.status === "starting" ? "codicon-loading codicon-modifier-spin" : "codicon-circle-slash",
        tone: "disabled",
        variant: "outline",
        enabled: false,
      };
  }
}

function debugButtonState(
  selected: SelectedRuntime,
  launchGateReason?: string
): SidebarButtonState {
  const disabled =
    selected.status === "unreachable" ||
    selected.status === "starting" ||
    Boolean(launchGateReason);
  return {
    state: disabled ? "disabled" : "ready",
    label: "Debug",
    title: launchGateReason ||
      (disabled ? "Debug is unavailable until the target is reachable." : "Start debugging"),
    icon: "codicon-debug-alt",
    tone: disabled ? "disabled" : "neutral",
    variant: "outline",
    enabled: !disabled,
  };
}

function primaryActionGateReason(
  selected: SelectedRuntime,
  compileState: CompileState,
  diagnostics: ValidityLine
): string | undefined {
  if (selected.primary.action !== "start") {
    return undefined;
  }
  return compileGateReason(compileState, diagnostics, "start");
}

function classifyCompileIssues(response: CheckProgramResponse): {
  sourceErrors: number;
  configErrors: number;
} {
  let sourceErrors = 0;
  let configErrors = 0;
  for (const issue of response.issues ?? []) {
    if ((issue.severity ?? "").toLowerCase() !== "error") {
      continue;
    }
    const file = issue.file ?? "";
    const code = issue.code ?? "";
    if (isConfigDiagnosticPath(file) || /config/i.test(code)) {
      configErrors += 1;
    } else {
      sourceErrors += 1;
    }
  }
  return { sourceErrors, configErrors };
}

function deployButtonState(
  _selected: SelectedRuntime,
  _compileState: CompileState
): SidebarButtonState {
  return {
    state: "unsupported",
    label: "Deploy",
    title: "Deploy is not available for this target yet.",
    icon: "codicon-rocket",
    tone: "disabled",
    variant: "outline",
    enabled: false,
  };
}

function compileSummary(response: CheckProgramResponse): string {
  return summarizeCheck(response);
}

async function withSidebarActionTimeout(
  promise: Promise<RuntimeLifecycleResult>,
  timeoutMs: number,
  timeoutMessage: string
): Promise<RuntimeLifecycleResult> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<RuntimeLifecycleResult>((resolve) => {
        timer = setTimeout(
          () =>
            resolve({
              ok: false,
              failure: {
                kind: "failed_spawn",
                message: timeoutMessage,
              },
            }),
          timeoutMs
        );
      }),
    ]);
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
  }
}

// HMI is adaptive (§0.5.13): open when a descriptor exists, otherwise scaffold then open. Never a dead
// disabled button.
async function openOrCreateHmi(): Promise<void> {
  if (await hasHmiDescriptor()) {
    await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
    return;
  }
  await vscode.commands.executeCommand("trust-lsp.hmi.init");
  await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
}

async function newDiagramMenu(): Promise<void> {
  const pick = await vscode.window.showQuickPick(
    [
      { label: "UML Statechart", command: "trust-lsp.statechart.new" },
      { label: "Blockly program", command: "trust-lsp.blockly.new" },
      { label: "Ladder Logic", command: "trust-lsp.ladder.new" },
      { label: "Sequential Function Chart (SFC)", command: "trust-lsp.sfc.new" },
    ],
    { title: "truST — New diagram", placeHolder: "Choose a visual editor" }
  );
  if (pick) {
    await vscode.commands.executeCommand(pick.command);
  }
}

function toModelSnapshot(snapshot: RuntimeLifecycleSnapshot): RuntimeModelSnapshot {
  return {
    runtimeMode: snapshot.status.runtimeMode,
    runtimeState: snapshot.status.runtimeState,
    endpoint: snapshot.status.endpoint,
    endpointConfigured: snapshot.status.endpointConfigured,
    endpointReachable: snapshot.status.endpointReachable,
    starting: snapshot.starting,
  };
}

function actionFailureMessage(
  selected: SelectedRuntime,
  result: RuntimeLifecycleResult & { ok: false }
): string {
  const reason = result.failure.message;
  switch (selected.primary.action) {
    case "start":
      return `Could not start the simulator: ${reason}`;
    case "stop":
      return `Could not stop: ${reason}`;
    case "connect":
      if (isRuntimeUnreachableFailure(reason)) {
        return `Could not connect to ${selected.label}. Runtime is not reachable. Open Devices & Connections to start or diagnose this runtime.`;
      }
      return `Could not connect to ${selected.label}: ${reason}`;
    case "disconnect":
      return `Could not disconnect: ${reason}`;
    default:
      return reason;
  }
}

const SET_AUTH_TOKEN_ACTION = "Set auth token";
const OPEN_DEVICES_ACTION = "Open Devices & Connections";

function connectFailureChoices(
  result: RuntimeLifecycleResult & { ok: false }
): string[] {
  const text = `${result.failure.kind} ${result.failure.message} ${result.failure.detail ?? ""}`;
  if (isRuntimeUnreachableFailure(text)) {
    return [OPEN_DEVICES_ACTION];
  }
  if (isAuthTokenFailure(text)) {
    return [SET_AUTH_TOKEN_ACTION];
  }
  return [];
}

function isRuntimeUnreachableFailure(text: string): boolean {
  return /not reachable|unreachable|connection refused|econnrefused|timed out|timeout/i.test(text);
}

function isAuthTokenFailure(text: string): boolean {
  return /auth|token|credential|unauthori[sz]ed|permission denied/i.test(text) &&
    !isRuntimeUnreachableFailure(text);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isReloadSuccess(value: unknown): boolean {
  return isRecord(value) && value.ok === true;
}

function reloadFailureMessage(value: unknown, validity: ValidityLine): string {
  if (isRecord(value) && value.gated === true && typeof value.message === "string" && value.message.trim()) {
    return value.message;
  }
  if (!validity.ok) {
    return "Fix the errors shown in Problems, then try again.";
  }
  if (isRecord(value) && typeof value.message === "string" && value.message.trim()) {
    return summarizeReloadMessage(value.message);
  }
  return "Reload did not report success. Keep the simulator running, fix any compile errors, and try again.";
}

function summarizeReloadMessage(message: string): string {
  const firstLine = message.trim().split(/\r?\n/).find((line) => line.trim())?.trim() ?? "";
  if (!firstLine) {
    return "Reload did not report a reason.";
  }
  const sourceErrorCount = message
    .split(/\r?\n/)
    .filter((line) => /\.(st|pou):/i.test(line)).length;
  if (sourceErrorCount > 0) {
    return `Compile failed — ${sourceErrorCount} error${sourceErrorCount === 1 ? "" : "s"}. Open Problems, then try again.`;
  }
  if (firstLine.length <= 160) {
    return firstLine;
  }
  return `${firstLine.slice(0, 157).trimEnd()}...`;
}

function makeNonce(): string {
  const chars =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let nonce = "";
  for (let i = 0; i < 32; i += 1) {
    nonce += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return nonce;
}
