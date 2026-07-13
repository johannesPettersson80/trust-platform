import {
  assert,
  fs,
  path,
  extensionRoot,
  workspaceRoot,
  readSrc,
} from "./ux-shell-contract-fixtures";

suite("VIS — visual editors follow the shared Run + Live Values model", () => {
  const visualEditorFiles = [
    "sfc/webview/SfcEditor.tsx",
    "statechart/webview/StateChartEditor.tsx",
    "ladder/webview/LadderEditor.tsx",
    "blockly/webview/BlocklyEditor.tsx",
  ];

  test("SFC toolbar add actions reframe the canvas so the result is visible", () => {
    const src = readSrc("sfc/webview/SfcEditor.tsx");
    const hook = readSrc("sfc/webview/hooks/useSfc.ts");
    assert.ok(
      src.includes("requestFitView") &&
        /useEffect\([\s\S]*reactFlowInstance\.fitView/.test(src),
      "SFC toolbar Add actions must request a committed fitView after mutating the graph"
    );
    assert.ok(
      /const handleAddStep[\s\S]*addNodeAtPosition\("step"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Add Step must reframe after adding so the new step is visible"
    );
    assert.ok(
      /const handleAddParallelSplit[\s\S]*addNodeAtPosition\("parallelSplit"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Split must reframe after adding so the new node is visible"
    );
    assert.ok(
      /const handleAddParallelJoin[\s\S]*addNodeAtPosition\("parallelJoin"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Join must reframe after adding so the new node is visible"
    );
    assert.ok(
      hook.includes("nextNodePosition"),
      "SFC hook must calculate toolbar-added node placement from existing node positions"
    );
    assert.ok(
      !hook.includes("150 + nds.length * 100"),
      "SFC toolbar-added nodes must not use the old overlapping vertical placement formula"
    );
  });
  test("SFC transition routing avoids stacking non-linear labels through the center line", () => {
    const stepNode = readSrc("sfc/webview/StepNode.tsx");
    const hook = readSrc("sfc/webview/hooks/useSfc.ts");
    const editor = readSrc("sfc/webview/SfcEditor.tsx");
    const transitionEdge = readSrc("sfc/webview/TransitionEdge.tsx");
    assert.ok(
      editor.includes("TransitionEdge") && editor.includes("edgeTypes"),
      "SFC must use the custom transition edge renderer, not React Flow's default midpoint labels"
    );
    assert.ok(
      transitionEdge.includes("EdgeLabelRenderer") &&
        transitionEdge.includes("labelOffset(sourcePosition)") &&
        transitionEdge.includes("sourcePosition === Position.Bottom") &&
        transitionEdge.includes("sourcePosition === Position.Top") &&
        transitionEdge.includes("sfc-transition-marker") &&
        transitionEdge.includes("sfc-transition-bar") &&
        transitionEdge.includes("sfc-transition-label"),
      "SFC transitions must render an IEC-style bar plus an offset condition label inspectable by the VIS runner; dense vertical-chain labels must not sit on the center line"
    );
    assert.ok(
      stepNode.includes('data.type === "initial"') &&
        stepNode.includes("4px double") &&
        stepNode.includes("INITIAL"),
      "SFC initial steps must be visually distinct at a glance, not only a thicker generic border"
    );
    assert.ok(
      transitionEdge.includes("function transitionBarStyle") &&
        transitionEdge.includes("width: sideRouted ? 3 : 34") &&
        transitionEdge.includes("height: sideRouted ? 34 : 3"),
      "SFC transition bars must stay perpendicular to normal and side-routed links"
    );
    for (const handle of [
      "STEP_TARGET_LEFT",
      "STEP_TARGET_RIGHT",
      "STEP_SOURCE_LEFT",
      "STEP_SOURCE_RIGHT",
    ]) {
      assert.ok(
        stepNode.includes(handle),
        `SFC step nodes must expose ${handle} for readable side-routed transitions`
      );
      assert.ok(
        hook.includes(handle),
        `SFC import/connect routing must use ${handle} when a transition is not a simple downward edge`
      );
    }
    assert.ok(
      hook.includes("stepConnectionHandles"),
      "SFC edge routing must use the shared stepConnectionHandles helper"
    );
    assert.ok(
      /deltaY\s*<\s*0[\s\S]*STEP_SOURCE_LEFT[\s\S]*STEP_TARGET_LEFT/.test(hook),
      "backward SFC transitions must route to side handles instead of overlapping the vertical path"
    );
    assert.ok(
      /deltaY\s*>\s*expectedVerticalGap[\s\S]*STEP_SOURCE_RIGHT[\s\S]*STEP_TARGET_RIGHT/.test(hook),
      "skip SFC transitions must route to side handles instead of overlapping intermediate labels"
    );
  });
  test("Statechart import and add actions reframe the canvas inside the shared editor shell", () => {
    const src = readSrc("statechart/webview/StateChartEditor.tsx");
    const hook = readSrc("statechart/webview/hooks/useStateChart.ts");
    assert.ok(
      src.includes("STATECHART_FIT_VIEW_OPTIONS"),
      "Statechart editor must use explicit fitView options for predictable framing"
    );
    assert.ok(
      src.includes("requestFitView"),
      "Statechart editor must request fitView after graph mutations"
    );
    assert.ok(
      /importFromXState\(config\);[\s\S]*requestFitView\(\);/.test(src),
      "Statechart import must reframe after loading nodes"
    );
    assert.ok(
      /const handleAddState[\s\S]*addNewState\("normal"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Add State must reframe so the new state and existing small graph stay visible"
    );
    assert.ok(
      /const handleAutoLayout[\s\S]*autoLayout\(\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Auto Layout must reframe after moving nodes"
    );
    assert.ok(
      /const STATE_GRID_X = 2[2-9]0;/.test(hook) &&
        /const STATE_GRID_Y = 2[2-9]0;/.test(hook),
      "Statechart grid spacing must leave room for edge labels between cards"
    );
    assert.ok(
      hook.includes("transitionHandles") &&
        hook.includes("STATE_SOURCE_RIGHT") &&
        hook.includes("STATE_TARGET_LEFT"),
      "Statechart same-row transitions must use side handles so labels do not sit on cards"
    );
    const edge = readSrc("statechart/webview/StateTransitionEdge.tsx");
    assert.ok(
      edge.includes("EdgeLabelRenderer") &&
        edge.includes("statechart-transition-label") &&
        edge.includes("labelTranslateY") &&
        edge.includes("sourcePosition === Position.Left") &&
        edge.includes("targetPosition === Position.Right") &&
        edge.includes("sourcePosition === Position.Bottom") &&
        edge.includes("targetPosition === Position.Top"),
      "Statechart backward and row-crossing transitions must lift labels away from cards"
    );
  });
  test("visual-editor chrome does not add private hardcoded colours", () => {
    const allowedSharedThemeFiles = new Set(["webview/theme.ts", "webview/theme.css"]);
    const filesToCheck = [
      "sfc/webview/SfcEditor.tsx",
      "sfc/webview/StepNode.tsx",
      "sfc/webview/sfcEditor.css",
      "statechart/webview/StateChartEditor.tsx",
      "statechart/webview/StateNode.tsx",
      "statechart/webview/StateTransitionEdge.tsx",
      "ladder/webview/LadderEditor.tsx",
      "ladder/webview/nodeDrawing.ts",
      "ladder/webview/styles.css",
      "blockly/webview/BlocklyEditor.tsx",
      "blockly/webview/ToolboxPanel.tsx",
      "blockly/webview/styles.css",
      "blockly/webview/blocklyTheme.css",
    ];
    const hardcodedColor = /#[0-9a-fA-F]{3,8}|rgba?\(/;
    for (const file of filesToCheck) {
      if (allowedSharedThemeFiles.has(file)) {
        continue;
      }
      const src = readSrc(file)
        .split("\n")
        .filter((line) => !line.includes("color-mix("))
        .join("\n");
      assert.ok(
        !hardcodedColor.test(src),
        `${file} must use shared --trust-* or t.* tokens for product chrome/semantic colours`
      );
    }
  });
  test("canvas grid backgrounds use the shared truST product grid role", () => {
    const files = [
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "sfc/webview/SfcEditor.tsx",
      "statechart/webview/StateChartEditor.tsx",
    ];
    for (const file of files) {
      const src = readSrc(file);
      assert.ok(
        src.includes('color="var(--trust-grid-line)"'),
        `${file} must use the shared --trust-grid-line role for canvas dot/grid backgrounds`
      );
      assert.ok(
        !src.includes('color="var(--vscode-editorWidget-border)"') &&
          !src.includes("vscode-editorIndentGuide-background"),
        `${file} must not give the generic canvas grid a private raw VS Code color`
      );
    }
  });
  test("Blockly toolbox labels use normal foreground tokens, not accent-button text", () => {
    const blocklyTheme = readSrc("blockly/webview/blocklyTheme.css");
    for (const selector of [".blocklyToolboxCategory", ".blocklyTreeLabel"]) {
      const match = blocklyTheme.match(new RegExp(`${selector.replace(".", "\\.")}\\s*\\{([\\s\\S]*?)\\}`));
      assert.ok(match, `${selector} must have an explicit shared-theme style`);
      assert.ok(
        match[1].includes("var(--trust-text)"),
        `${selector} must use the normal shared foreground token`
      );
      assert.ok(
        !match[1].includes("--trust-on-accent"),
        `${selector} must not use --trust-on-accent; that is only readable on accent backgrounds`
      );
    }
  });
  test("Blockly generated-code actions use shared button chrome and no emoji glyphs", () => {
    const codePanel = readSrc("blockly/webview/CodePanel.tsx");
    const styles = readSrc("blockly/webview/styles.css");
    assert.ok(
      codePanel.includes("trust-button trust-button--primary"),
      "the Blockly generated-code Copy action must use the shared product button classes"
    );
    assert.ok(
      !/📋|🔀|🔁|➕|📦|⚙️|🔌|⏱️|🔢/.test(codePanel),
      "the generated-code panel must not render emoji glyphs as product action icons"
    );
    const copyRule = styles.match(/\.copy-button\s*\{([\s\S]*?)\}/);
    assert.ok(copyRule, "copy-button may keep layout-only CSS");
    assert.ok(
      !/background(?:-color)?\\s*:|\\bcolor\\s*:|\\bborder\\s*:|border-radius\\s*:/.test(copyRule[1]),
      "copy-button CSS must not override shared trust-button color/border/radius treatment"
    );
  });
  test("dead execution panels with embedded runtime controls are removed", () => {
    for (const file of [
      "sfc/webview/SfcExecutionPanel.tsx",
      "statechart/webview/ExecutionPanel.tsx",
    ]) {
      assert.ok(
        !fs.existsSync(path.join(extensionRoot(), "src", file)),
        `${file} must not remain as dead duplicate runtime UI`
      );
    }
  });
  test("visual editor parse errors use user-facing recovery language", () => {
    for (const file of [
      "sfc/sfcEditor.ts",
      "statechart/stateChartEditor.ts",
      "blockly/blocklyEditor.ts",
    ]) {
      const src = readSrc(file);
      assert.ok(
        !/Editor Error:/.test(src),
        `${file} must not show raw 'Editor Error' notifications`
      );
      assert.ok(
        /Could not open/.test(src),
        `${file} must tell the user the visual file could not be opened`
      );
    }
  });
  test("server-expose examples drive the exposed global from ST, not a static initializer", () => {
    const opcUaMain = fs.readFileSync(
      path.join(workspaceRoot(), "examples/communication/opcua/src/main.st"),
      "utf8"
    );
    const adsMain = fs.readFileSync(
      path.join(workspaceRoot(), "examples/communication/ads_server_basic/src/main.st"),
      "utf8"
    );
    assert.ok(
      /TankLevel\s*:=\s*TankLevel\s*\+\s*1\.0/.test(opcUaMain) &&
        /PumpRunning\s*:=\s*TankLevel\s*>\s*50\.0/.test(opcUaMain),
      "OPC UA server example must update TankLevel each scan before exposing it"
    );
    assert.ok(
      /TankLevel\s*:=\s*TankLevel\s*\+\s*1\.0/.test(adsMain) &&
        /PumpRunning\s*:=\s*TankLevel\s*>\s*40\.0/.test(adsMain),
      "ADS server example must update TankLevel each scan before exposing it"
    );

    const runnerRoot = path.join(
      workspaceRoot(),
      "docs/internal/testing/evidence/vscode-ui-ux-acceptance/2026-06-25/runners"
    );
    const opcRunnerPath = path.join(runnerRoot, "opcua-server-live-read-runner.js");
    if (fs.existsSync(opcRunnerPath)) {
      const runner = fs.readFileSync(opcRunnerPath, "utf8");
      assert.ok(
        runner.includes("clientProof.before.values.TankLevel.value") &&
          runner.includes("clientProof.after.values.TankLevel.value") &&
          !runner.includes("assert.strictEqual(clientProof.values.TankLevel.value, 42.5"),
        "OPC UA live-read runner must prove TankLevel changes, not only read the initializer"
      );
    }
    const adsRunnerPath = path.join(runnerRoot, "ads-server-expose-runner.js");
    if (fs.existsSync(adsRunnerPath)) {
      const runner = fs.readFileSync(adsRunnerPath, "utf8");
      assert.ok(
        runner.includes('selected: "global.TankLevel"') &&
          runner.includes('waitAndClickLeafCheckbox("TankLevel")') &&
          !runner.includes('selected: "global.Setpoint"'),
        "ADS server expose runner must select the ST-driven TankLevel, not an unrelated Setpoint"
      );
    }
  });
});
