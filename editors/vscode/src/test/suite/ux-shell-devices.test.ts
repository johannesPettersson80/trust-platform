import {
  assert,
  fs,
  path,
  setUpRuntimeOptions,
  V1_SETUP_CAPS,
  extensionRoot,
  readSrc,
  readSrcSet,
} from "./ux-shell-contract-fixtures";

suite("Phase 7 — Devices & Connections (shared run-target + naming)", () => {
  const panel = () => readSrc("networkCanvas/networkCanvasPanel.ts");

  test("the canvas panel is user-facing 'Devices & Connections', never 'Network Canvas'", () => {
    const src = panel();
    assert.ok(
      src.includes('"Devices & Connections"'),
      "the panel title must be 'Devices & Connections'"
    );
    assert.ok(
      !src.includes("Structured Text: Network Canvas") &&
        !src.includes("<title>Network Canvas"),
      "no user-facing 'Network Canvas' title"
    );
  });
  test("Devices & Connections never opens as a blank webview while loading", () => {
    const src = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/webviewHtml.ts",
      "networkCanvas/initialGraph.ts",
      "networkCanvas/refreshCoordinator.ts"
    );
    assert.ok(
      src.includes('class="initial-canvas"') &&
        src.includes("Devices &amp; Connections") &&
        src.includes("initialNetworkCanvasGraph(") &&
        src.includes("Loading configured connections in the background."),
      "the static webview HTML must render the lifecycle-owned first graph while configured connections load"
    );
    assert.ok(
      !src.includes('<div id="root"></div>'),
      "the root must not be empty on first paint"
    );
    assert.ok(
      src.includes("var(--trust-canvas") &&
        src.includes("var(--trust-text-muted") &&
        src.includes("var(--trust-text-subtle"),
      "the initial loading state must use the shared truST theme roles"
    );
    const openBody = src.slice(
      src.indexOf("async function showNetworkCanvasPanel"),
      src.indexOf("async function refreshNetworkCanvasPanel")
    );
    assert.ok(
      openBody.includes("void refreshNetworkCanvasPanel();"),
      "opening the panel must paint the static loading shell before the async topology refresh completes"
    );
    assert.ok(
      !openBody.includes("await refreshNetworkCanvasPanel();"),
      "opening the panel must not block on topology before the user sees progress"
    );
    assert.ok(
      src.includes("runtimeAuthority.beginFirstPaint(") &&
        src.includes("projectCanvasLifecycleAuthority(") &&
        src.includes('runtime.name = "Checking active runtime…"'),
      "first paint must project the shared lifecycle immediately and fail closed while a non-simulator authority is checked"
    );
    assert.ok(
      src.includes("TRUST_VSCODE_NETWORK_CANVAS_REFRESH_DELAY_MS") &&
        src.includes("Math.min(Math.floor(value), 10_000)"),
      "slow-source acceptance tests may delay topology refresh, but the hook must be explicit and bounded"
    );
  });
  test("no user-facing 'Network Canvas' anywhere it renders or reaches the user (bundle + runtime strings)", () => {
    // The BUILT webview bundle is esbuild output (comments stripped) — any match here is a real rendered
    // string. This is what caught the header/title leaks that source-only guards missed.
    const bundle = fs.readFileSync(
      path.join(extensionRoot(), "media", "networkCanvasWebview.js"),
      "utf8"
    );
    assert.ok(
      !bundle.includes("Network Canvas"),
      "the built Devices & Connections webview must not render 'Network Canvas'"
    );
    // Runtime-facing host strings: graph titles posted to the webview + user-visible messages. Match only
    // quoted/templated literals so internal identifiers (NETWORK_CANVAS_VIEW_TYPE, the command id) are fine.
    for (const file of ["networkCanvas/graphData.ts", "runtimeLifecycle.ts"]) {
      assert.ok(
        !/["'`][^"'`\n]*Network Canvas/.test(readSrc(file)),
        `${file} must not contain a user-facing 'Network Canvas' string`
      );
    }
  });
  test("ONE selected-run-target store, written by the dropdown AND the graph", () => {
    const store = readSrc("selectedRuntime.ts");
    assert.ok(
      store.includes("getSelectedRuntimeId") && store.includes("setSelectedRuntimeId"),
      "a shared selected-runtime store exists"
    );
    assert.ok(
      readSrc("trustHomeView.ts").includes("getSelectedRuntimeId"),
      "the sidebar reads the shared selected-target store"
    );
  });
  test("selected run target persists across VS Code restart with a workspace-scoped fallback", () => {
    const store = readSrc("selectedRuntime.ts");
    assert.ok(
      store.includes("workspaceState.get<string>(KEY)") &&
        store.includes("workspaceState.update(KEY, id)"),
      "workspaceState remains the primary selected-run-target store"
    );
    assert.ok(
      store.includes("globalState.get<string>(globalKey)") &&
        store.includes("globalState.update(globalKey, id)"),
      "a workspace-scoped globalState fallback must persist selection across VS Code restarts"
    );
    assert.ok(
      store.includes("const workspaceValue = ctx.workspaceState.get<string>(KEY)") &&
        store.includes("const globalValue = ctx.globalState.get<string>(globalKey)") &&
        store.includes("const persistedValue = readPersistedTargets()[globalKey]") &&
        store.includes(
          "workspaceValue === id && globalValue === id && persistedValue === id"
        ),
      "setSelectedRuntimeId must not skip writing the durable fallback just because the in-session store already has the id"
    );
    assert.ok(
      store.includes('const PERSIST_FILE = "selected-runtime-by-workspace.json"') &&
        store.includes("ctx.globalStorageUri.fsPath") &&
        store.includes("writePersistedTarget(globalKey, id)") &&
        store.includes("readPersistedTargets()[globalKey]"),
      "the selected target must also persist to extension global storage so real VS Code restarts keep it selected"
    );
    assert.ok(
      store.includes("createHash") && store.includes("vscode.workspace.workspaceFolders"),
      "the fallback key must be scoped to the workspace roots, not one global target for every project"
    );
  });
  test("Connect on a runtime node ALSO sets the active Target", () => {
    const src = readSrc("networkCanvas/lifecycleActions.ts");
    const connectIdx = src.indexOf("private async connectRemote");
    const targetIdx = src.indexOf("private async setAsRunTarget", connectIdx);
    const handler = src.slice(connectIdx, targetIdx);
    assert.ok(
      src.includes('case "runtimeConnect"') &&
        src.includes("await this.connectRemote(message)") &&
        handler.includes("const result = await this.dependencies.connectRemote(endpoint, label)") &&
        handler.includes("if (result.ok && endpoint)") &&
        handler.includes("await setSelectedRuntimeId(endpoint)"),
      "a successful runtime-node Connect must set that exact endpoint as the run target"
    );
  });
  test("Set as run target selects WITHOUT connecting", () => {
    const src = readSrc("networkCanvas/lifecycleActions.ts");
    assert.ok(src.includes('case "setAsRunTarget"'), "the panel handles setAsRunTarget");
    const start = src.indexOf("private async setAsRunTarget");
    const end = src.indexOf("private async runManagedAction", start);
    const handler = src.slice(start, end);
    assert.ok(
      handler.includes("await setSelectedRuntimeId(target)") &&
        !handler.includes("connectRemote("),
      "Set as run target must persist the target without opening a connection"
    );
    const ctrl = readSrc("networkCanvas/webview/runtimeNodeControls.ts");
    assert.ok(
      ctrl.includes('action: "setAsRunTarget"'),
      "a runtime node offers Set as run target"
    );
  });
  test("'Set up runtime…' wizard is capability-gated (Install/Docker gated in v1)", () => {
    const options = setUpRuntimeOptions(V1_SETUP_CAPS);
    const byId = (id: string) => options.find((option) => option.id === id);
    assert.ok(byId("connect")?.available, "Connect existing is available in v1");
    assert.ok(byId("local")?.available, "Run a runtime on this computer is available in v1");
    assert.ok(
      !byId("install")?.available && !!byId("install")?.reason,
      "Install truST runtime is gated with a reason (phase 11)"
    );
    assert.ok(
      !byId("docker")?.available && !!byId("docker")?.reason,
      "Run in Docker is gated with a reason (phase 12)"
    );
    assert.ok(
      byId("connect")?.detail.includes("another computer or controller"),
      "Connect existing copy must explain the user goal without naming only Pi/IPC hardware"
    );
    assert.ok(
      byId("local")?.detail.includes("select it as the Target and click Start") &&
        !byId("local")?.detail.includes("Run target"),
      "managed local runtime copy must use the current sidebar Target + Start wording"
    );
    assert.ok(
      byId("install")?.detail.includes("another computer over SSH"),
      "Install copy must stay generic to computers/controllers instead of implying Raspberry Pi / IPC only"
    );
    assert.ok(
      !options.map((option) => `${option.label} ${option.detail}`).join("\n").includes("IPC"),
      "setup wizard copy must not use narrow IPC jargon in the first-user flow"
    );
  });
  test("host runtime setup slot uses the self-explanatory setup wording", () => {
    const layout = readSrc("networkCanvas/webview/layout.ts");
    assert.ok(
      layout.includes('data: { label: "Set up runtime", slot: { add: "runtime"'),
      "the host runtime slot must say Set up runtime, not a raw +Runtime label"
    );
    assert.ok(
      layout.includes('data: { label: "Add connection", slot: { add: "device"'),
      "the runtime-local add slot must say Add connection, not just Add"
    );
    assert.ok(
      layout.includes('data: { label: "Add host", slot: { add: "host"'),
      "the host slot must say Add host, not just Host"
    );
    assert.ok(
      !layout.includes('data: { label: "Runtime", slot: { add: "runtime"'),
      "the old raw Runtime slot label must not return"
    );
    assert.ok(
      !layout.includes('data: { label: "Add", slot: { add: "device"'),
      "the old vague Add slot label must not return"
    );
    assert.ok(
      !layout.includes('data: { label: "Host", slot: { add: "host"'),
      "the old vague Host slot label must not return"
    );
    assert.ok(
      layout.includes("position: { x: hostX, y: HOST_HEADER }"),
      "the Add host slot must sit in the host body row, not overlap the host header or setup slot"
    );
  });
  test("'Set up runtime…' wizard uses the shared product inspector chrome", () => {
    const src = readSrc("networkCanvas/webview/SetUpRuntimePanel.tsx");
    for (const required of [
      "trust-inspector",
      "trust-inspector__header",
      "trust-inspector__eyebrow",
      "trust-inspector__title",
      "trust-section",
      "trust-button",
      "trust-button-grid",
      "trust-help",
      "Devices &amp; Connections / Runtime setup",
    ]) {
      assert.ok(src.includes(required), `SetUpRuntimePanel must render ${required}`);
    }
    assert.ok(
      !/var\(--vscode-[^)]+\)/.test(src) &&
        !/#[0-9a-fA-F]{3,8}\b/.test(src) &&
        !/background\s*:|border(?:Left)?\s*:|color\s*:/.test(src),
      "SetUpRuntimePanel must not define private raw VS Code colors/chrome"
    );
  });
  test("Connect existing runtime stores tokens securely and uses shared chrome", () => {
    const form = readSrc("networkCanvas/webview/AddHostPanel.tsx");
    for (const required of [
      'aria-label="Connect existing runtime"',
      "trust-inspector",
      "trust-inspector__header",
	      "trust-section",
	      "trust-field",
	      "trust-input",
	      "trust-button",
	      "type=\"password\"",
	      "Devices &amp; Connections / Runtime setup",
	      "Runtime address",
	      "10.0.0.5:5680",
	      "Runtime auth token (optional)",
	      'placeholder="Optional"',
	      "Paste the token configured for that runtime",
	      "Leave this empty when the runtime does not require one",
	      "If you do not know the address, use Discover instead.",
	      "Add runtime",
	      "authToken",
    ]) {
      assert.ok(form.includes(required), `AddHostPanel must render ${required}`);
    }
    for (const rejected of [
	      "Raspberry Pi, or an IPC",
	      'placeholder="tcp://10.0.0.5:5680"',
	      "Runtime auth token (if required)",
	      "Leave empty unless the runtime asks for one",
	      "Use the token that was configured when the runtime was started",
	      "Save runtime",
	    ]) {
      assert.ok(!form.includes(rejected), `AddHostPanel must not render confusing copy: ${rejected}`);
    }
    assert.ok(
      !/var\(--vscode-[^)]+\)/.test(form) &&
        !/#[0-9a-fA-F]{3,8}\b/.test(form) &&
        !/background\s*:|border(?:Left)?\s*:|color\s*:/.test(form),
      "AddHostPanel must not define private raw VS Code colors/chrome"
    );

    const host = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/fleetActions.ts",
      "networkCanvas/fleetTargetResolver.ts",
      "networkCanvas/networkCanvasWorkspace.ts"
    );
    assert.ok(host.includes("setControlAuthToken"), "remote tokens must use SecretStorage");
    assert.ok(
      host.includes("getControlAuthToken(endpoint)"),
      "fleet peer resolution must read the saved SecretStorage token before probing"
    );
    assert.ok(
      host.includes("workspaceConfigResource()") && host.includes("trustConfig()"),
      "fleet endpoint settings must be read with the active workspace resource"
    );
    assert.ok(host.includes("message.authToken"), "the host add path must receive the token field");
    assert.ok(
      host.includes("normalizeFleetControlEndpoint"),
      "host:port entries must normalize to a real control endpoint"
    );
    assert.ok(
      !host.includes("runtime.controlAuthToken"),
      "remote setup must not write the legacy plaintext token setting"
    );
    assert.ok(
      !host.includes("Added ${endpoint} to the fleet"),
      "successful remote-runtime setup must not use a global VS Code toast that covers the canvas result"
    );
  });
  test("runtime setup task panes keep the Devices & Connections breadcrumb", () => {
    const panes = [
      ["SetUpRuntimePanel", readSrc("networkCanvas/webview/SetUpRuntimePanel.tsx"), "Set up runtime"],
      ["AddHostPanel", readSrc("networkCanvas/webview/AddHostPanel.tsx"), "Connect existing runtime"],
    ] as const;

    for (const [name, src, title] of panes) {
      assert.ok(
        src.includes("trust-inspector__eyebrow") &&
          src.includes("Devices &amp; Connections / Runtime setup"),
        `${name} must render the shared Devices & Connections runtime setup breadcrumb`
      );
      assert.ok(src.includes(title), `${name} must keep its task-specific title`);
    }
  });
  test("endpoint edit pane uses task-name breadcrumbs instead of role badges", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      src.includes("function editBreadcrumb(protocol: string)") &&
        src.includes("return `Edit ${protocolName(protocol)}`;"),
      "endpoint edit panes must build a user-facing Edit <protocol> breadcrumb"
    );
    assert.ok(
      src.includes("Devices & Connections / {editBreadcrumb(protocol)}"),
      "endpoint edit breadcrumb must use the task name helper"
    );
    assert.ok(
      !src.includes("{roleWord(protocol, str(node.data.role))} edit"),
      "endpoint edit breadcrumb must not render role-badge copy such as CLIENT edit"
    );
  });
  test("refresh does not post through a disposed canvas panel", () => {
    const src = panel();
    assert.ok(
      src.includes("const panelRef = panel;"),
      "refresh must snapshot the current webview panel before any await"
    );
    assert.ok(
      src.includes("panel !== panelRef") &&
        src.includes("!panelRef.visible") &&
        src.includes("return;"),
      "refresh must stop if the panel was disposed/replaced while async work was in flight"
    );
    assert.ok(
      src.includes("panelRef.webview.postMessage"),
      "refresh must post through the stable panel reference, not the mutable global"
    );
  });
  test("node inspector maps raw health ids to user-facing labels", () => {
    const src = readSrcSet(
      "networkCanvas/webview/NodeInspector.tsx",
      "networkCanvas/webview/NodeSummaryView.tsx"
    );
    assert.ok(
      src.includes("function healthLabel"),
      "NodeInspector must map backend health ids before rendering inspector state rows"
    );
    assert.ok(
      /case "configured_policy":[\s\S]*return "Configured";/.test(src),
      "configured_policy must render as Configured, never as the raw backend enum"
    );
    assert.ok(
      src.includes("healthLabel(health)") &&
        !src.includes('`${health} · ${str(d.detail)}`'),
      "endpoint state rows must use healthLabel(health), not raw health ids"
    );
    assert.ok(
      src.includes("function stateSummary") &&
        src.includes("function runtimeModeLabel") &&
        /rows\.push\(\[\s*"State",\s*stateSummary\([\s\S]*?health,[\s\S]*?str\(d\.detail\)/.test(src) &&
        src.includes('rows.push(["Mode", mode])') &&
        !src.includes('rows.push(["mode"') &&
        !src.includes('rows.push(["status"') &&
        !src.includes('rows.push(["detail"'),
      "runtime/host inspector rows must render Title-Case product labels and keep lifecycle in one State row"
    );
    assert.ok(
      src.includes("function summaryLabelFor") &&
        src.includes('return "Connection file"') &&
        src.includes('return "Polling"') &&
        src.includes('return "Enabled"'),
      "endpoint summary rows must translate backend field labels into user-facing labels"
    );
    assert.ok(
      !src.includes("rows.push([field.label.toLowerCase(), v])"),
      "endpoint summaries must not render raw lower-cased schema labels"
    );
  });
  test("starting a new canvas drawer clears stale apply errors", () => {
    const host = panel();
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(
      host.includes('case "clearApplyResult"') &&
        host.includes("lastApplyResult = undefined"),
      "the canvas host must clear lastApplyResult on request so old faults disappear"
    );
    assert.ok(
      app.includes("function Canvas()") &&
        app.includes("const clearApplyResult = useCallback"),
      "the webview must centralize clearing transient apply state"
    );
    assert.ok(
      app.includes('post({ type: "clearApplyResult" })'),
      "the webview must tell the host to clear stale apply state, not only local React state"
    );
    assert.ok(
      /onPickSlot:[\s\S]*clearApplyResult\(\);[\s\S]*if \(slot\.add === "device"\)/.test(app),
      "opening a new Add flow must clear stale validation/fault banners"
    );
    assert.ok(
      /onChoose=\{\(protocol\) => \{[\s\S]*clearApplyResult\(\);[\s\S]*setDraft/.test(app),
      "choosing a new protocol form must clear stale validation/fault banners"
    );
  });
  test("EtherCAT channel browse saves through EtherCAT config, not ADS import", () => {
    const app = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/useBrowseSession.ts"
    );
    const host = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/protocolActions.ts"
    );
    const schema = readSrc("networkCanvas/webview/browseActions.ts");

    assert.ok(
      schema.includes('case "ethercat"') &&
        schema.includes('actionLabel: "Add channels"') &&
        schema.includes('kind: "channels"'),
      "EtherCAT browse must remain a channel picker, not a tag/import flow"
    );
    assert.ok(
      app.includes('panel.protocol === "ethercat"') &&
        app.includes('type: "addEthercatChannels"'),
      "the webview must route selected EtherCAT channels to the dedicated save message"
    );
    assert.ok(
      host.includes("async addEthercatChannels") &&
        host.includes('case "addEthercatChannels"') &&
        host.includes("selected_channels"),
      "the host must persist selected EtherCAT channels through comm.apply"
    );
    const ethercatBranch =
      /else if \(panel\.protocol === "ethercat"\) \{([\s\S]*?)\n\s*\} else \{/.exec(
        app
      )?.[1] ?? "";
    assert.ok(
      ethercatBranch.includes('type: "addEthercatChannels"') &&
        !ethercatBranch.includes('"addTags"'),
      "EtherCAT must not fall through to the ADS addTags handler"
    );
  });
});
