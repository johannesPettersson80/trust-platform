import {
  assert,
  fs,
  path,
  extensionRoot,
  workspaceRoot,
  readSrc,
  readSrcSet,
} from "./ux-shell-contract-fixtures";

suite("VIS — visual editors follow the shared Run + Live Values model", () => {
  const visualEditorFiles = [
    "sfc/webview/SfcEditor.tsx",
    "statechart/webview/StateChartEditor.tsx",
    "ladder/webview/LadderEditor.tsx",
    "blockly/webview/BlocklyEditor.tsx",
  ];

  test("visual editors do not render the legacy embedded runtime/I/O panel", () => {
    for (const file of visualEditorFiles) {
      const src = readSrc(file);
      assert.ok(
        !src.includes("StRuntimePanel"),
        `${file} must not import/render StRuntimePanel; use the shared sidebar + Live Values surfaces`
      );
      assert.ok(
        !/rightPaneView\s*===\s*"io"|setRightPaneView\("io"\)|>\s*I\/O\s*</.test(src),
        `${file} must not expose a local I/O tab`
      );
      assert.ok(
        !/rightPaneView\s*===\s*"settings"|setRightPaneView\("settings"\)|>\s*Settings\s*</.test(src),
        `${file} must not expose a local runtime settings tab`
      );
      assert.ok(
        !/Open Runtime Panel|Compile Diagnostics/.test(src),
        `${file} must not route users to the old Runtime Panel mental model`
      );
      assert.ok(
        !/MiniMap|<Panel\b/.test(src),
        `${file} must not render default minimap/stat overlays that obscure the program`
      );
    }
  });
  test("product webviews share the same truST theme source", () => {
    assert.ok(
      fs.existsSync(path.join(extensionRoot(), "src", "webview", "theme.ts")),
      "shared React style tokens must live in src/webview/theme.ts"
    );
    assert.ok(
      fs.existsSync(path.join(extensionRoot(), "src", "webview", "theme.css")),
      "shared CSS tokens must live in src/webview/theme.css"
    );
    assert.ok(
      !fs.existsSync(path.join(extensionRoot(), "src", "networkCanvas", "webview", "theme.css")),
      "Devices & Connections must not keep a parallel CSS theme file"
    );

    const expectedImport = 'import "../../webview/theme.css";';
    for (const file of [
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "sfc/webview/main.tsx",
      "statechart/webview/main.tsx",
      "ladder/webview/main.tsx",
      "blockly/webview/main.tsx",
    ]) {
      const src = readSrc(file);
      assert.ok(
        src.includes(expectedImport),
        `${file} must import the shared CSS theme, not a local theme copy`
      );
      assert.ok(
        !src.includes("trustTheme.css"),
        `${file} must not import the retired private visual-editor theme`
      );
    }

    assert.strictEqual(
      readSrc("networkCanvas/webview/theme.ts").trim(),
      'export { t, tint } from "../../webview/theme";',
      "Devices & Connections must use the shared React style token module"
    );
  });
  test("primary buttons use VS Code button tokens, not the focus/accent token as fill", () => {
    const theme = readSrc("webview/theme.css");
    assert.ok(
      theme.includes("--trust-action-primary-bg: var(--vscode-button-background") &&
        theme.includes("--trust-action-primary-hover-bg: var(--vscode-button-hoverBackground") &&
        theme.includes("--trust-action-primary-fg: var(--vscode-button-foreground"),
      "shared primary action tokens must map to VS Code button colors"
    );
    assert.ok(
      theme.includes("background: var(--trust-action-primary-bg)") &&
        !/\\.trust-button--primary\\s*{[^}]*background:\\s*var\\(--trust-accent\\)/s.test(theme),
      "filled primary buttons must not use focusBorder/accent as their background"
    );
    const sidebar = readSrc("trustHomeWebview.ts");
    assert.ok(
      sidebar.includes("background: var(--trust-action-primary-bg)") &&
        !/\\.action-button(?:\\.primary|\\[data-variant=\"filled\"\\])\\s*{[^}]*background:\\s*var\\(--trust-accent\\)/s.test(sidebar),
      "sidebar Run/Start buttons must use the shared primary action tokens"
    );
  });
  test("shared truST theme has an explicit high-contrast token contract", () => {
    const themeCss = readSrc("webview/theme.css");
    const themeTs = readSrc("webview/theme.ts");

    for (const selector of [
      ":root.vscode-high-contrast",
      "body.vscode-high-contrast",
      ':root[data-vscode-theme-kind="vscode-high-contrast"]',
      "body.vscode-high-contrast-light",
      "@media (forced-colors: active)",
    ]) {
      assert.ok(themeCss.includes(selector), `shared theme must define ${selector}`);
    }

    for (const token of [
      "--trust-canvas: #000000",
      "--trust-surface: #000000",
      "--trust-overlay: #000000",
      "--trust-input-bg: #000000",
      "--trust-canvas: #ffffff",
      "--trust-surface: #ffffff",
      "--trust-overlay: #ffffff",
      "--trust-input-bg: #ffffff",
      "--trust-border: var(--vscode-contrastBorder",
      "--trust-action-primary-bg: var(--vscode-button-background",
      "--trust-role-host-bg: #000000",
      "--trust-role-runtime-bg: #000000",
      "--trust-role-endpoint-bg: #000000",
      "--trust-role-external-bg: #000000",
      "--trust-role-host-bg: #ffffff",
      "--trust-role-runtime-bg: #ffffff",
      "--trust-role-endpoint-bg: #ffffff",
      "--trust-role-external-bg: #ffffff",
      "outline: 2px solid var(--trust-accent)",
    ]) {
      assert.ok(themeCss.includes(token), `high-contrast theme must include ${token}`);
    }

    for (const token of [
      'canvas: v("--trust-canvas"',
      'surface: v("--trust-surface"',
      'surfaceRaised: v("--trust-surface-raised"',
      'overlay: v("--trust-overlay"',
      'text: v("--trust-text"',
      'border: v("--trust-border"',
      'accent: v("--trust-accent"',
      'inputBg: v("--trust-input-bg"',
      'inputBorder: v("--trust-input-border"',
    ]) {
      assert.ok(
        themeTs.includes(token),
        `React/Canvas inline styles must consume shared CSS token ${token}`
      );
    }
  });
  test("VS Code extension test runner honors CARGO_TARGET_DIR", () => {
    const src = readSrc("test/runTest.ts");
    assert.ok(
      src.includes("function cargoTargetDir") &&
        src.includes("process.env.CARGO_TARGET_DIR") &&
        src.includes('path.join(cargoTargetDir(repoRoot), "debug", binaryName)'),
      "runTest.ts must find built trust binaries in CARGO_TARGET_DIR for remote-builder gates"
    );
    assert.ok(
      !src.includes('path.join(\n    repoRoot,\n    "target",\n    "debug"'),
      "runTest.ts must not hardcode repoRoot/target/debug while remote gates use CARGO_TARGET_DIR"
    );
  });
  test("Windows packaged ADS lane runs the prepared release debug adapter", () => {
    const runner = readSrc("test/runTest.ts");
    const workflow = fs.readFileSync(
      path.join(workspaceRoot(), ".github", "workflows", "ci.yml"),
      "utf8"
    );
    const releaseWorkflow = fs.readFileSync(
      path.join(workspaceRoot(), ".github", "workflows", "release.yml"),
      "utf8"
    );
    assert.ok(
      runner.includes("process.env.ST_DEBUG_TEST_BIN") &&
        runner.includes("ST_DEBUG_TEST_BIN: debugPath") &&
        runner.includes("ST_DEBUG_TEST_BIN not found"),
      "the Extension Host runner must accept and validate an explicit debug adapter binary"
    );
    assert.ok(
      workflow.includes(
        "ST_DEBUG_TEST_BIN: ${{ github.workspace }}/editors/vscode/bin/trust-debug.exe"
      ) &&
        workflow.includes(
          "--staged-debug editors/vscode/bin/trust-debug.exe"
        ),
      "the Windows packaged lane must exercise its prepared release trust-debug.exe"
    );
    assert.ok(
      releaseWorkflow.includes(
        "--staged-debug editors/vscode/bin/trust-debug.exe"
      ),
      "the release gate must byte-bind the packaged adapter to its staged release binary"
    );
  });
  test("development binary resolver honors CARGO_TARGET_DIR", () => {
    const src = readSrc("binary.ts");
    assert.ok(
      src.includes("process.env.CARGO_TARGET_DIR") &&
        src.includes("configuredDebugCandidate") &&
        src.includes("configuredReleaseCandidate"),
      "development/test binary lookup must resolve trust-lsp, trust-runtime, and trust-debug from CARGO_TARGET_DIR"
    );
    assert.ok(
      src.indexOf("process.env.CARGO_TARGET_DIR") <
        src.indexOf('path.join(\n    repoRoot,\n    "target",\n    "debug"'),
      "CARGO_TARGET_DIR must be checked before falling back to repoRoot/target/debug"
    );
  });
  test("HMI preview uses shared truST product theme roles", () => {
    const src = readSrc("hmi-panel/view.ts");
    for (const token of [
      "--trust-canvas",
      "--trust-surface",
      "--trust-text",
      "--trust-text-muted",
      "--trust-border",
      "--trust-accent",
      "--trust-input-bg",
      "--trust-selected-bg",
    ]) {
      assert.ok(src.includes(token), `HMI preview must define and consume ${token}`);
    }

    for (const selector of [
      "button {",
      ".tab.active",
      ".widget {",
      ".section-card {",
      ".process-panel {",
      ".hmi-empty--state",
      "#status {",
    ]) {
      assert.ok(src.includes(selector), `HMI preview must style ${selector} as product chrome`);
    }
    assert.ok(
      src.includes("Start the runtime to see live HMI data") &&
        src.includes("Use Start in the truST sidebar"),
      "HMI stopped state must render a beginner-facing empty-state body, not only a toolbar status"
    );
    assert.ok(
      src.includes("renderProcessPage(page, allWidgets)") &&
        !src.includes("renderProcessPage(page, visible)"),
      "HMI process bindings must resolve against all schema widgets, not only widgets visible on the process page"
    );
    assert.ok(
      src.includes("function applyProcessSvgTheme") &&
        src.includes("trust-process-svg") &&
        src.includes("svg.trust-process-svg > rect:first-of-type") &&
        src.includes("svg.trust-process-svg .pid-title") &&
        src.includes("svg.trust-process-svg .pid-value") &&
        src.includes("var(--trust-surface-raised)") &&
        src.includes("var(--trust-text)") &&
        src.includes("var(--trust-accent)"),
      "HMI process SVG embedding must normalize generated process SVGs to shared theme roles"
    );

    for (const legacyPattern of [
      "border: 1px solid var(--vscode-panel-border",
      "background: var(--vscode-editor-background",
      "color: var(--vscode-editor-foreground",
      "border-color: var(--vscode-focusBorder",
      "background: color-mix(in srgb, var(--vscode-focusBorder",
    ]) {
      assert.ok(
        !src.includes(legacyPattern),
        `HMI preview must not keep private raw VS Code chrome: ${legacyPattern}`
      );
    }
  });
  test("HMI preview formats live values like the rest of truST", () => {
    const src = readSrc("hmi-panel/view.ts");
    assert.ok(
      src.includes("function formatHmiLiteral") &&
        src.includes('return "TRUE";') &&
        src.includes('return "FALSE";'),
      "HMI preview must format BOOL values as IEC TRUE/FALSE, matching Live Values"
    );
    assert.ok(
      src.includes("function formatRealValue") && src.includes("numeric.toFixed(1)"),
      "HMI preview must keep at least one decimal for REAL/LREAL values"
    );
    assert.ok(
      src.includes("function processMapKeys") &&
        src.includes('keys.push(value ? "true" : "false")'),
      "HMI process maps must remain compatible with existing lowercase true/false map keys"
    );
  });
  test("HMI preview schedules descriptor refreshes from edit save and watcher events", () => {
    const src = readSrc("hmiPanel.ts");
    for (const token of [
      "vscode.workspace.onDidChangeTextDocument",
      "vscode.workspace.onDidSaveTextDocument",
      'vscode.workspace.createFileSystemWatcher("**/hmi/*.{toml,svg}")',
      'vscode.workspace.createFileSystemWatcher("**/hmi/views/*.view.toml")',
      "scheduleSchemaRefresh();",
      "DESCRIPTOR_REFRESH_DEBOUNCE_MS",
    ]) {
      assert.ok(src.includes(token), `HMI preview must keep descriptor refresh wiring: ${token}`);
    }
  });
  test("React Flow canvas controls use the shared Devices & Connections treatment", () => {
    const themeCss = readSrc("webview/theme.css");
    for (const selector of [
      ".react-flow__controls",
      ".react-flow__controls button",
      ".react-flow__controls button:hover",
      ".trust-canvas-summary",
    ]) {
      assert.ok(
        themeCss.includes(selector),
        `shared webview theme must define ${selector} for canvas navigation chrome`
      );
    }
    assert.ok(
      themeCss.includes("left: 58px;") && themeCss.includes("max-width: calc(100% - 90px);"),
      "the canvas summary/count label must sit beside the React Flow controls, not cover zoom/fit buttons"
    );

    const app = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/NetworkCanvasOverlays.tsx"
    );
    assert.ok(
      app.includes('className="trust-canvas-summary"'),
      "Devices & Connections must use the shared canvas summary style instead of private inline positioning"
    );

    const localControlCss = [
      "sfc/webview/sfcEditor.css",
      "statechart/webview/index.html",
      "statechart/stateChartEditor.ts",
    ];
    for (const file of localControlCss) {
      const src = readSrc(file);
      assert.ok(
        !src.includes("--vscode-button-background") &&
          !src.includes("--vscode-button-hoverBackground"),
        `${file} must not restyle canvas controls as primary buttons`
      );
      assert.ok(
        !/\\.react-flow__controls\\s*\\{/.test(src) &&
          !/\\.react-flow__controls button/.test(src),
        `${file} must not keep a private React Flow controls theme; use src/webview/theme.css`
      );
    }
  });
  test("Devices & Connections protocol identity colors use shared theme roles", () => {
    const nodes = readSrc("networkCanvas/webview/nodes.tsx");
    const busNode = readSrc("networkCanvas/webview/BusNode.tsx");
    const protocolMeta = readSrc("networkCanvas/webview/protocolMeta.ts");
    const theme = readSrc("webview/theme.ts");
    const css = readSrc("webview/theme.css");
    assert.ok(
      protocolMeta.includes("t.protocolBlue") &&
        protocolMeta.includes("t.protocolOrange") &&
        protocolMeta.includes("t.protocolCyan") &&
        protocolMeta.includes("t.protocolPurple") &&
        protocolMeta.includes("t.protocolMuted"),
      "protocol colors must be consumed from shared theme roles"
    );
    assert.ok(
      nodes.includes('from "./protocolMeta"'),
      "network canvas nodes must consume protocol identity from the shared protocol metadata module"
    );
    assert.ok(
      !/#[0-9a-fA-F]{3,8}\b/.test(nodes) &&
        !/#[0-9a-fA-F]{3,8}\b/.test(protocolMeta),
      "network canvas protocol identity must not define private hex colors"
    );
    for (const role of [
      "protocolBlue",
      "protocolOrange",
      "protocolGreen",
      "protocolCyan",
      "protocolRed",
      "protocolPurple",
      "protocolMuted",
      "roleHostBg",
      "roleHostBorder",
      "roleRuntimeBg",
      "roleRuntimeBorder",
      "roleEndpointBg",
      "roleExternalBg",
      "roleExternalBorder",
    ]) {
      assert.ok(theme.includes(role), `theme.ts must expose ${role}`);
    }
    for (const token of [
      "--trust-protocol-blue",
      "--trust-protocol-orange",
      "--trust-protocol-green",
      "--trust-protocol-cyan",
      "--trust-protocol-red",
      "--trust-protocol-purple",
      "--trust-protocol-muted",
      "--trust-role-host-bg",
      "--trust-role-host-border",
      "--trust-role-runtime-bg",
      "--trust-role-runtime-border",
      "--trust-role-endpoint-bg",
      "--trust-role-external-bg",
      "--trust-role-external-border",
    ]) {
      assert.ok(css.includes(token), `theme.css must define ${token}`);
    }
    assert.ok(
      css.includes(".trust-button:disabled") &&
        css.includes("button.trust-button:disabled") &&
        css.includes(".trust-button--primary:disabled") &&
        css.includes("button.trust-button.trust-button--primary:disabled") &&
        /background:\s*var\(--trust-surface-raised\)\s*!important/.test(css) &&
        /background-color:\s*var\(--trust-surface-raised\)\s*!important/.test(css) &&
        /border:\s*1px solid var\(--trust-border\)\s*!important/.test(css) &&
        /transition:\s*none\s*!important/.test(css),
      "disabled actions must use shared neutral styling, not a live-looking accent button"
    );
    for (const role of ["t.roleHostBg", "t.roleRuntimeBg", "t.roleEndpointBg", "t.roleExternalBg"]) {
      assert.ok(nodes.includes(role), `network canvas nodes must use shared role tint ${role}`);
    }
    assert.ok(
      nodes.includes("const statusTone = draftLike ? t.protocolMuted : healthColor(d.health)") &&
        nodes.includes("background: statusTone"),
      "draft endpoints must use the shared muted draft role for every status indicator, not a separate health colour"
    );
    assert.ok(
      busNode.includes("trust-edge-label-knockout") &&
        busNode.includes("trust-bus-draft-chip") &&
        busNode.includes("boxShadow: `0 0 0 4px ${t.canvas}`"),
      "mesh bus labels must have an opaque knockout and a separate DRAFT chip so wires never run through label text"
    );
    assert.ok(
      !busNode.includes(" · DRAFT"),
      "draft state must render as a separate chip, not as suffix text inside the bus label"
    );
  });
  test("Devices & Connections refits when endpoint children appear after managed Start", () => {
    const src = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/NetworkCanvasHeader.tsx",
      "networkCanvas/webview/useDiscoverPane.ts"
    );
    assert.ok(
      src.includes("child endpoints") && src.includes(".map((n) => n.id)"),
      "canvas fit signature must include child endpoint node IDs, not only host IDs"
    );
    assert.ok(
      !src.includes(".filter((n) => !n.parentId)\n      .map((n) => n.id)"),
      "managed Start can add endpoints under an existing host; top-level-only fit signatures leave a blank-looking canvas"
    );
    assert.ok(
      src.includes("setFocusTargetId(node.id)") && src.includes("selection and the add-flow share the right drawer"),
      "opening an inspector from a node click must refit the selected node into the narrowed canvas instead of leaving a blank-looking graph"
    );
    assert.ok(
      src.includes('post({ type: "focus", nodeId })') &&
        src.includes("void fitView({ duration: 500, padding: 0.2, maxZoom: 1.2 })"),
      "the inspector Focus action must preserve graph context instead of panning to an empty-looking canvas"
    );
    assert.ok(
      src.includes('window.addEventListener("resize", refit)') &&
        src.includes('window.addEventListener("focus", refit)') &&
        src.includes('document.addEventListener("visibilitychange", onVisibility)'),
      "Devices & Connections must re-fit when VS Code splits/focuses editor groups so the visible canvas cannot go blank beside Live Values"
    );
    assert.ok(
      src.includes('querySelectorAll<HTMLElement>(".react-flow__node")') &&
        src.includes("nodesAreVisible") &&
        src.includes("!nodesAreVisible()") &&
        src.includes("window.setInterval"),
      "Devices & Connections must recover if graph nodes exist in the DOM but none intersect the visible canvas"
    );
    assert.ok(
      src.includes("const editSlotsVisible =") &&
        src.includes("editMode && !draft && !selectedId && !browseTags && !discoverOpen && !addSlot && !filterOpen") &&
        src.includes("editSlotsVisible"),
      "edit-mode add/setup/host slots must hide while a right drawer is open so background affordances cannot overlap the active workflow"
    );
    assert.ok(
      src.includes("const toolbarAddTarget = useMemo") &&
        src.includes("LOCAL_RUNTIME_NODE_ID") &&
        src.includes("const openAddPicker = useCallback") &&
        src.includes('setAddSlot({ kind: "device", targetId: toolbarAddTarget.id })') &&
        src.includes("onAdd={openAddPicker}") &&
        /<button[\s\S]*onClick=\{onAdd\}[\s\S]*\+ Add[\s\S]*<\/button>/.test(src),
      "Devices & Connections must expose a first-class + Add toolbar action that opens the picker for the selected/default runtime"
    );
    assert.ok(
      !/\bMiniMap\b|<MiniMap\b/.test(src),
      "Devices & Connections must use the shared low-prominence zoom/fit/count controls, not a separate minimap panel that clutters small graphs"
    );
    assert.ok(
      /const adopt = useCallback[\s\S]*close\(\);[\s\S]*setEditMode\(false\)/.test(src) &&
        /<AddHostPanel[\s\S]*onSaved=\{\(\) => setEditMode\(false\)\}/.test(src),
      "successful Connect existing / Adopt runtime flows must return to a clean result graph instead of leaving edit-mode placeholders visible"
    );
    assert.ok(
      !src.includes("Devices &amp; Connections"),
      "the webview header must not repeat the VS Code tab title; the page name belongs in the panel/tab chrome"
    );
  });
  test("endpoint removal is a deliberate two-step action", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      src.includes("confirmRemove") && src.includes("Confirm remove"),
      "endpoint Remove must arm a confirmation state before writing config"
    );
    assert.ok(
      src.includes("Remove this endpoint from the project?"),
      "the confirmation state must explain what is about to happen"
    );
    assert.ok(
      src.includes("endpoint-remove-confirmation") && src.includes("Cancel"),
      "the remove confirmation warning must be visible in the action footer with a cancellation path"
    );
    assert.ok(
      src.includes('if (!confirmRemove)') && src.includes('send("commRemove")'),
      "commRemove must only be sent from the confirmed branch"
    );
  });
  test("empty runtime guidance points to + Add, not hidden Edit mode", () => {
    const nodes = readSrc("networkCanvas/webview/nodes.tsx");
    assert.ok(
      nodes.includes(">Discover ADS devices</span> to find ADS devices already running") &&
        nodes.includes(">+ Add</span> to configure one") &&
        !nodes.includes(">Edit</span> to add one"),
      "a first-time user must see Discover ADS devices first and + Add for manual setup, never hidden Edit mode"
    );
  });
  test("endpoint edit drafts are not reset by identical topology refreshes", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      src.includes("const paramsKey = JSON.stringify(params ?? {})"),
      "endpoint edit reset logic must compare params by content"
    );
    assert.ok(
      src.includes("const schemaKey = `${protoSchema.id}:${protoSchema.fields.map((field) => field.id).join(\"|\")}`"),
      "endpoint edit reset logic must compare schema by a stable signature"
    );
    assert.ok(
      src.includes("}, [node.id, schemaKey, paramsKey]);"),
      "endpoint edit drafts must not depend on changing schema or params object identities"
    );
    assert.ok(
      !src.includes("}, [node.id, protoSchema, paramsKey, params]);"),
      "topology polling must not reset in-progress endpoint edits with identical params"
    );
    assert.ok(
      !src.includes("}, [node.id, protoSchema, paramsKey]);"),
      "schema refreshes with equivalent content must not reset in-progress endpoint edits"
    );
  });
  test("endpoint disable is available from the inspector and writes through offline comm apply", () => {
    const inspector = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      inspector.includes('protoSchema.actions.includes("disable")'),
      "endpoint Disable must be driven by the backend schema action"
    );
    assert.ok(
      inspector.includes('send("commDisable"') && inspector.includes("Disable"),
      "endpoint edit inspector must expose a Disable button"
    );
    assert.ok(
      inspector.includes("This endpoint is disabled.") && inspector.includes("Use Enable to turn it back on"),
      "disabled endpoints must explain the visible Enable action"
    );
    assert.ok(
      inspector.includes('{isDisabled ? "Enable" : "Save"}'),
      "disabled endpoints must expose an explicit Enable primary action instead of making users infer it from Save"
    );
    const nodes = readSrc("networkCanvas/webview/nodes.tsx");
    assert.ok(
      nodes.includes('"unknown", "disabled"') &&
        nodes.includes('d.health === "disabled"') &&
        nodes.includes("<StatusPill") &&
        nodes.includes("health={d.health}"),
      "disabled endpoints must render a visible Disabled state in the graph, not only a color dot"
    );

    const panel = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/configurationActions.ts"
    );
    assert.ok(
      panel.includes('case "commDisable"') &&
        panel.includes('configurationActions.save(message, "disable")') &&
        panel.includes("offlineCommApply("),
      "Disable must write the project config through the same offline comm apply path as Save/Remove"
    );
    const offline = readSrc("networkCanvas/offlineComm.ts");
    assert.ok(
      offline.includes('"add" | "upsert" | "remove" | "disable"'),
      "offlineCommApply must allow the disable action"
    );
  });
});
