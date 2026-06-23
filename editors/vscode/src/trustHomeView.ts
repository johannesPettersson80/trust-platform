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
  type RemoteRuntime,
  type RuntimeModelSnapshot,
  type SelectedRuntime,
} from "./trustHomeModel";
import {
  getSelectedRuntimeId,
  onDidChangeSelectedRuntime,
  setSelectedRuntimeId,
} from "./selectedRuntime";
import {
  listManagedRuntimes,
  onDidChangeManagedRuntimes,
  startManagedRuntime,
  stopManagedRuntime,
} from "./localRuntime";
import type { ManagedRuntime } from "./localRuntimeModel";

// §UX v5 (vscode-ux-overhaul-plan.md §0.5) — the ONE truST panel (WebviewView `trust.home`, no visible
// "Home"). It has TWO states:
//   • No project open  → ONLY the Project welcome: Create project · Open project · Start from example.
//   • Project open     → the Run bar (select-only `Run target:` + ONE state-specific action + passive
//                        validity line) followed by nav launchers: Project · Devices & Connections ·
//                        Live Values · HMI.
// The dropdown is select-only (no Add/Connect). A remote NEVER renders Start/Stop (only Connect/
// Disconnect) — its process lifecycle lives on its Devices & Connections node, never here.

interface ValidityLine {
  readonly ok: boolean;
  readonly label: string;
}

class TrustHomeProvider implements vscode.WebviewViewProvider {
  static readonly viewType = "trust.home";

  private view?: vscode.WebviewView;
  // "Apply changes" (sim-only): true once an .st/.pou file is saved after Start, cleared on Start/Apply.
  // This is honest save-based change detection — never claim "changed" without an actual save.
  private sourceChanged = false;

  constructor(private readonly context: vscode.ExtensionContext) {}

  markSourceChanged(): void {
    this.sourceChanged = true;
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
    const projectOpen = await isTrustProjectOpen();
    const snapshot = await runtimeLifecycleService.snapshot();
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const options = runtimeOptions(remotes, managed);
    const selected = this.resolveSelected(snapshot, remotes, managed);
    // Apply changes is SIMULATOR-ONLY (§0.5.3/§0.6.6) and only when the running sim's source changed.
    // Remote apply/restart/deploy lives on the runtime node, never here.
    const canApply =
      selected.kind === "simulator" &&
      selected.status === "running" &&
      this.sourceChanged;
    void this.view.webview.postMessage({
      type: "state",
      projectOpen,
      options,
      selectedId: selected.id,
      selected,
      validity: validityLine(),
      canApply,
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
      case "action":
        await this.runAction();
        return;
      case "applyChanges":
        await this.applyChanges();
        return;
      // No-project welcome
      case "createProject":
        await vscode.commands.executeCommand("trust-lsp.newProject");
        return;
      case "openProject":
        await vscode.commands.executeCommand("workbench.action.files.openFolder");
        return;
      case "startExample":
        await vscode.commands.executeCommand("trust.examples.start");
        return;
      // Project-open nav launchers
      case "navProject":
        await projectActionsMenu();
        return;
      case "navDevices":
        await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
        return;
      case "navLiveValues":
        await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
        return;
      case "navHmi":
        await openOrCreateHmi();
        return;
      default:
        return;
    }
  }

  private async onSelect(id: string): Promise<void> {
    if (id) {
      await setSelectedRuntimeId(id);
    }
    await this.render();
  }

  private async runAction(): Promise<void> {
    const snapshot = await runtimeLifecycleService.snapshot();
    const remotes = this.readRemotes();
    const managed = await listManagedRuntimes(this.context);
    const selected = this.resolveSelected(snapshot, remotes, managed);
    // A managed local runtime is OURS — Start/Stop via the fleet lifecycle, not the debug simulator.
    if (selected.kind === "local") {
      await this.runManagedAction(selected);
      await this.render();
      return;
    }
    const result = await this.dispatch(selected);
    if (result && !result.ok) {
      if (selected.primary.action === "connect") {
        // A failed connect is often a missing/expired token — offer a SECURE (SecretStorage) entry.
        const choice = await vscode.window.showWarningMessage(
          actionFailureMessage(selected, result),
          "Set auth token"
        );
        if (choice === "Set auth token") {
          await vscode.commands.executeCommand("trust-lsp.runtime.setAuthToken", {
            endpoint: selected.id,
          });
        }
      } else {
        void vscode.window.showWarningMessage(actionFailureMessage(selected, result));
      }
    } else if (
      selected.primary.action === "start" ||
      selected.primary.action === "connect"
    ) {
      if (selected.primary.action === "start") {
        // A fresh Start compiles current source — nothing pending to apply.
        this.sourceChanged = false;
      }
      // Auto-reveal Live Values when the user starts the sim or connects a remote (§0.5.5).
      void vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
    }
    await this.render();
  }

  private async runManagedAction(selected: SelectedRuntime): Promise<void> {
    const ok =
      selected.primary.action === "stop"
        ? await stopManagedRuntime(this.context, selected.id)
        : await startManagedRuntime(this.context, selected.id);
    if (!ok) {
      void vscode.window.showWarningMessage(
        `Could not ${selected.primary.action} ${selected.label}. Check it in Devices & Connections.`
      );
    } else if (selected.primary.action === "start") {
      void vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
    }
  }

  private async applyChanges(): Promise<void> {
    // Sim-only hot reload (§0.6.6). The button is only shown when canApply, but guard anyway.
    await vscode.commands.executeCommand("trust-lsp.debug.reload");
    this.sourceChanged = false;
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
        return runtimeLifecycleService.connectRemote(selected.id);
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
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<style>
  body { padding: 10px 12px; font-family: var(--vscode-font-family); color: var(--vscode-foreground); }
  h2 { font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; opacity: 0.8; margin: 0 0 8px; }
  p.hint { font-size: 12px; opacity: 0.8; margin: 0 0 12px; }
  label { display: block; font-size: 12px; opacity: 0.85; margin: 10px 0 4px; }
  select {
    width: 100%; box-sizing: border-box; padding: 4px 6px;
    color: var(--vscode-dropdown-foreground); background: var(--vscode-dropdown-background);
    border: 1px solid var(--vscode-dropdown-border); border-radius: 2px; font-size: 13px;
  }
  .validity { font-size: 12px; margin: 2px 0 0; opacity: 0.85; }
  .validity.ok::before { content: "$(check) "; }
  .validity .ico { margin-right: 5px; }
  .validity.ok .ico { color: var(--vscode-testing-iconPassed, #2ea043); }
  .validity.warn .ico { color: var(--vscode-testing-iconFailed, #d13438); }
  .status { font-size: 12px; margin: 10px 0 2px; }
  .status .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; vertical-align: middle; background: var(--vscode-descriptionForeground); }
  .status .dot.running, .status .dot.connected { background: var(--vscode-testing-iconPassed, #2ea043); }
  .status .dot.starting { background: var(--vscode-charts-yellow, #d7a200); }
  .status .dot.unreachable { background: var(--vscode-testing-iconFailed, #d13438); }
  .status .value { font-weight: 600; }
  button {
    width: 100%; box-sizing: border-box; margin-top: 8px; padding: 6px 10px; cursor: pointer;
    color: var(--vscode-button-foreground); background: var(--vscode-button-background);
    border: 1px solid var(--vscode-button-border, transparent); border-radius: 2px; font-size: 13px;
  }
  button:hover:not(:disabled) { background: var(--vscode-button-hoverBackground); }
  button:disabled { opacity: 0.5; cursor: default; }
  button.secondary {
    color: var(--vscode-button-secondaryForeground, var(--vscode-foreground));
    background: var(--vscode-button-secondaryBackground, transparent);
    border-color: var(--vscode-button-border, var(--vscode-widget-border, rgba(128,128,128,0.35)));
  }
  button.secondary:hover:not(:disabled) { background: var(--vscode-button-secondaryHoverBackground, var(--vscode-list-hoverBackground)); }
  .hint { font-size: 11px; opacity: 0.8; margin-top: 6px; line-height: 1.4; }
  .nav { margin-top: 16px; border-top: 1px solid var(--vscode-widget-border, rgba(128,128,128,0.25)); padding-top: 8px; }
  .nav button {
    text-align: left; margin-top: 4px; padding: 6px 8px;
    background: transparent; color: var(--vscode-foreground); border: 1px solid transparent;
  }
  .nav button:hover { background: var(--vscode-list-hoverBackground); }
  .hidden { display: none; }
</style>
</head>
<body>
  <!-- No project open: the Project welcome ONLY -->
  <section id="welcome" class="hidden">
    <h2>truST</h2>
    <p class="hint">Create or open a project to get started.</p>
    <button id="createProject">Create project</button>
    <button id="openProject" class="secondary">Open project</button>
    <button id="startExample" class="secondary">Start from example</button>
  </section>

  <!-- Project open: Run bar + nav launchers -->
  <section id="project" class="hidden">
    <h2>Run</h2>
    <div class="validity" id="validity"><span class="ico" id="validityIco"></span><span id="validityText">—</span></div>
    <label for="runtime">Run target:</label>
    <select id="runtime"></select>
    <div class="status">Status: <span class="dot" id="dot"></span><span class="value" id="status">—</span></div>
    <button id="action" disabled>—</button>
    <button id="apply" class="secondary" style="display:none">Apply changes</button>
    <div class="hint" id="hint" style="display:none"></div>

    <nav class="nav">
      <button class="nav-item" id="navProject">Project</button>
      <button class="nav-item" id="navDevices">Devices &amp; Connections</button>
      <button class="nav-item" id="navLiveValues">Live Values</button>
      <button class="nav-item" id="navHmi">HMI</button>
    </nav>
  </section>
<script nonce="${nonce}">
  const vscode = acquireVsCodeApi();
  const welcomeEl = document.getElementById("welcome");
  const projectEl = document.getElementById("project");
  const runtimeEl = document.getElementById("runtime");
  const statusEl = document.getElementById("status");
  const dotEl = document.getElementById("dot");
  const actionEl = document.getElementById("action");
  const applyEl = document.getElementById("apply");
  const hintEl = document.getElementById("hint");
  const validityEl = document.getElementById("validity");
  const validityIco = document.getElementById("validityIco");
  const validityText = document.getElementById("validityText");

  function post(type) { return () => vscode.postMessage({ type }); }
  runtimeEl.addEventListener("change", () => vscode.postMessage({ type: "select", id: runtimeEl.value }));
  actionEl.addEventListener("click", () => { if (!actionEl.disabled) { vscode.postMessage({ type: "action" }); } });
  applyEl.addEventListener("click", () => vscode.postMessage({ type: "applyChanges" }));
  document.getElementById("createProject").addEventListener("click", post("createProject"));
  document.getElementById("openProject").addEventListener("click", post("openProject"));
  document.getElementById("startExample").addEventListener("click", post("startExample"));
  document.getElementById("navProject").addEventListener("click", post("navProject"));
  document.getElementById("navDevices").addEventListener("click", post("navDevices"));
  document.getElementById("navLiveValues").addEventListener("click", post("navLiveValues"));
  document.getElementById("navHmi").addEventListener("click", post("navHmi"));

  window.addEventListener("message", (event) => {
    const msg = event.data;
    if (!msg || msg.type !== "state") { return; }
    welcomeEl.classList.toggle("hidden", msg.projectOpen);
    projectEl.classList.toggle("hidden", !msg.projectOpen);
    if (!msg.projectOpen) { return; }

    validityText.textContent = msg.validity.label;
    validityIco.textContent = msg.validity.ok ? "✓" : "⚠";
    validityEl.className = "validity " + (msg.validity.ok ? "ok" : "warn");

    runtimeEl.innerHTML = "";
    for (const option of msg.options) {
      const el = document.createElement("option");
      el.value = option.id;
      el.textContent = option.label;
      if (option.id === msg.selectedId) { el.selected = true; }
      runtimeEl.appendChild(el);
    }
    statusEl.textContent = msg.selected.statusLabel;
    dotEl.className = "dot " + msg.selected.status;
    actionEl.textContent = msg.selected.primary.label;
    actionEl.disabled = !msg.selected.primary.enabled;
    applyEl.style.display = msg.canApply ? "" : "none";
    const hint = msg.selected.primary.hint || "";
    hintEl.textContent = hint;
    hintEl.style.display = hint ? "" : "none";
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
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => provider.refresh())
  );
  // Saving an ST source while the sim runs enables the sim-only "Apply changes" (§0.6.6).
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (/\.(st|pou)$/i.test(doc.uri.fsPath)) {
        provider.markSourceChanged();
      }
    })
  );
}

// "Project open" = a workspace folder is open AND it is a truST project (has a trust-lsp.toml). Anything
// else shows the Create/Open/Example welcome — there's nothing to run yet.
async function isTrustProjectOpen(): Promise<boolean> {
  if (!vscode.workspace.workspaceFolders?.length) {
    return false;
  }
  const found = await vscode.workspace.findFiles(
    "**/trust-lsp.toml",
    "**/node_modules/**",
    1
  );
  return found.length > 0;
}

// Passive validity (§0.5.6): diagnostics-derived "no known errors" — NOT an authoritative "build OK"
// (a real Check program backend doesn't exist yet — phase 8). Never a button.
function validityLine(): ValidityLine {
  let errors = 0;
  for (const [uri, diagnostics] of vscode.languages.getDiagnostics()) {
    if (!/\.(st|pou)$/i.test(uri.fsPath)) {
      continue;
    }
    errors += diagnostics.filter(
      (d) => d.severity === vscode.DiagnosticSeverity.Error
    ).length;
  }
  return errors === 0
    ? { ok: true, label: "No known errors" }
    : { ok: false, label: `${errors} error${errors === 1 ? "" : "s"} — see Problems` };
}

// HMI is adaptive (§0.5.13): open when a descriptor exists, otherwise scaffold then open. Never a dead
// disabled button.
async function openOrCreateHmi(): Promise<void> {
  const present = await vscode.workspace.findFiles(
    "**/hmi/*.toml",
    "**/node_modules/**",
    1
  );
  if (present.length > 0) {
    await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
    return;
  }
  await vscode.commands.executeCommand("trust-lsp.hmi.init");
  await vscode.commands.executeCommand("trust-lsp.hmi.openPreview");
}

// "Project" launcher → the project actions a user reaches without the palette.
async function projectActionsMenu(): Promise<void> {
  const pick = await vscode.window.showQuickPick(
    [
      { label: "$(file-directory-create) New project", command: "trust-lsp.newProject" },
      { label: "$(folder-opened) Open project", command: "workbench.action.files.openFolder" },
      { label: "$(library) Start from example", command: "trust.examples.start" },
      { label: "$(check-all) Check program", command: "trust-lsp.checkProgram" },
      { label: "$(type-hierarchy) New diagram…", command: "__newDiagram" },
      { label: "$(beaker) Run tests", command: "trust-lsp.test.runAll" },
    ],
    { title: "truST — Project", placeHolder: "Project actions" }
  );
  if (!pick) {
    return;
  }
  if (pick.command === "__newDiagram") {
    await newDiagramMenu();
    return;
  }
  await vscode.commands.executeCommand(pick.command);
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
      return `Could not connect to ${selected.label}: ${reason}`;
    case "disconnect":
      return `Could not disconnect: ${reason}`;
    default:
      return reason;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
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
