import {
  assert,
  fs,
  path,
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

  test("visual editor right panes use the shared product chrome, not private sidebars", () => {
    const themeCss = readSrc("webview/theme.css");
    for (const selector of [
      ".trust-product-shell",
      ".trust-product-header",
      ".trust-product-brand",
      ".trust-product-workspace",
      ".trust-canvas-pane",
      ".trust-inspector",
      ".trust-inspector__header",
      ".trust-section",
      ".trust-button",
      ".trust-input",
    ]) {
      assert.ok(
        themeCss.includes(selector),
        `shared webview theme must define ${selector} for product chrome`
      );
    }

    const editorShells = [
      "sfc/webview/SfcEditor.tsx",
      "statechart/webview/StateChartEditor.tsx",
      "ladder/webview/LadderEditor.tsx",
      "blockly/webview/BlocklyEditor.tsx",
    ];
    for (const file of editorShells) {
      const src = readSrc(file);
      assert.ok(
        src.includes("trust-inspector"),
        `${file} must render the same inspector/right-pane chrome as Devices & Connections`
      );
      assert.ok(
        src.includes("trust-product-shell") &&
          src.includes("trust-product-header") &&
          src.includes("trust-product-workspace") &&
          src.includes("trust-canvas-pane"),
        `${file} must render the same product shell/header/workspace structure as Devices & Connections`
      );
      assert.ok(
        src.includes("trust-inspector__title"),
        `${file} must render its right-pane heading with the shared primary inspector title treatment`
      );
      assert.ok(
        !src.includes(">Editor tools<") && !src.includes('"Editor tools"'),
        `${file} must not render a generic "Editor tools" right-pane title; use the surface name`
      );
      assert.ok(
        !/right-pane-view-title|blockly-right-pane-title/.test(src),
        `${file} must not use private right-pane title classes for the primary inspector heading`
      );
    }

    const expectedSurfaceTitles: Array<[string, string]> = [
      ["sfc/webview/SfcEditor.tsx", "SFC editor"],
      ["statechart/webview/StateChartEditor.tsx", "Statechart editor"],
      ["ladder/webview/LadderEditor.tsx", "Ladder editor"],
      ["blockly/webview/BlocklyEditor.tsx", "Blockly editor"],
    ];
    for (const [file, title] of expectedSurfaceTitles) {
      assert.ok(
        readSrc(file).includes(title),
        `${file} must use the product-surface title "${title}"`
      );
    }

    const productChromeFiles = [
      "sfc/webview/SfcToolsPanel.tsx",
      "sfc/webview/SfcCodePanel.tsx",
      "sfc/webview/sfcEditor.css",
      "statechart/webview/StatechartToolsPanel.tsx",
      "statechart/webview/PropertiesPanel.tsx",
      "statechart/webview/ActionMappingsPanel.tsx",
      "ladder/webview/styles.css",
      "blockly/webview/styles.css",
      "blockly/webview/blocklyTheme.css",
    ];
    const forbiddenPrivateChrome = [
      "vscode-button-secondaryBackground",
      "vscode-button-secondaryHoverBackground",
      "vscode-button-secondaryForeground",
      "vscode-sideBar-background",
      "vscode-sideBarSectionHeader-background",
      "vscode-panel-border, #2b2b2b",
    ];
    for (const file of productChromeFiles) {
      const src = readSrc(file);
      for (const forbidden of forbiddenPrivateChrome) {
        assert.ok(
          !src.includes(forbidden),
          `${file} must not define private visual-editor chrome with ${forbidden}; use shared --trust-* product tokens/classes`
        );
      }
    }

    const forbiddenVisualPanelSelectors = [
      ".blockly-toolbar",
      ".toolbar-button",
      ".toolbar-section",
      ".ladder-tools-panel__title",
      ".ladder-tools-panel__hint",
      ".ladder-tools-panel__section-title",
      ".ladder-tools-panel__grid",
      ".ladder-tools-panel__rungs",
      ".ladder-tools-panel__button",
      ".blockly-tools-panel",
      ".blockly-tools-panel__title",
      ".blockly-tools-panel__hint",
      ".blockly-tools-panel__grid",
      ".blockly-tools-panel__button",
    ];
    for (const [file, src] of [
      ["ladder/webview/styles.css", readSrc("ladder/webview/styles.css")],
      ["blockly/webview/styles.css", readSrc("blockly/webview/styles.css")],
    ] as const) {
      for (const selector of forbiddenVisualPanelSelectors) {
        assert.ok(
          !src.includes(selector),
          `${file} must not define private product chrome selector ${selector}; use shared trust-section/trust-button classes`
        );
      }
    }
  });
  test("ladder contacts and coils show symbols with addresses using neutral edit strokes", () => {
    const editor = readSrc("ladder/webview/LadderEditor.tsx");
    const nodes = readSrc("ladder/webview/nodeDrawing.ts");
    const themeCss = readSrc("webview/theme.css");
    const example = JSON.parse(
      fs.readFileSync(
        path.join(workspaceRoot(), "examples/ladder/ethercat-snake.ladder.json"),
        "utf8"
      )
    ) as {
      variables?: Array<{ name?: string; address?: string }>;
    };

    assert.ok(
      editor.includes("variableDisplayByReference") &&
        editor.includes("resolveVariableDisplay") &&
        editor.includes("register(address, display)"),
      "Ladder editor must resolve node labels through variables[] so address references display their symbols"
    );
    assert.ok(
      nodes.includes("drawVariableLabel") &&
        nodes.includes("display.primary") &&
        nodes.includes("display.secondary"),
      "Ladder contacts/coils must render the symbolic name and mapped address as separate label lines"
    );
    assert.ok(
      nodes.includes("const color = k(isActive ? t.ladderWireLive : t.ladderWire)") &&
        !nodes.includes("const color = k(isActive ? t.ladderWireLive : t.accent)"),
      "Ladder contact/coil edit strokes must use the neutral ladder wire token until live execution state drives power-flow colour"
    );
    assert.ok(
      /--trust-ladder-wire:\s*color-mix\(in srgb, var\(--trust-text\)/.test(
        themeCss
      ),
      "The edit-time ladder wire token must derive from text/border roles, not status green"
    );

    const mappedSymbol = example.variables?.find(
      (variable) => variable.address === "%MX1.0"
    );
    assert.equal(
      mappedSymbol?.name,
      "Step0Active",
      "EtherCAT ladder fixture must expose a named symbol for the %MX1.0 address capture"
    );
  });
  test("visual editors reserve dashed strokes for product draft semantics", () => {
    const editorFiles = [
      "statechart/webview/StateNode.tsx",
      "ladder/webview/LadderEditor.tsx",
      "ladder/webview/elements/Rung.tsx",
      "sfc/webview/SfcEditor.tsx",
      "blockly/webview/BlocklyEditor.tsx",
    ];
    for (const file of editorFiles) {
      const src = readSrc(file);
      assert.ok(!/borderStyle:\s*["']dashed["']/.test(src), `${file} must not render dashed borders for editor decoration`);
      assert.ok(!/strokeDasharray/.test(src), `${file} must not render dashed editor strokes`);
      assert.ok(!/\bdash\s*[:=]\s*\[/.test(src), `${file} must not render Konva dashed editor strokes`);
    }
    assert.ok(
      !readSrc("statechart/webview/StateChartEditor.tsx").includes("animated: true"),
      "Statechart transitions must not use React Flow animated edges because that renders dashed motion"
    );

    const dcNodes = readSrc("networkCanvas/webview/nodes.tsx");
    const dcEdges = readSrc("networkCanvas/webview/CasedEdge.tsx");
    assert.ok(
      dcNodes.includes('"dashed"') && dcEdges.includes("strokeDasharray"),
      "Devices & Connections must keep dashed treatment for draft/unproven topology"
    );
  });
  test("visual editor right panes share Tools Edit View IA and one zoom placement", () => {
    const panelFiles = [
      "sfc/webview/SfcToolsPanel.tsx",
      "statechart/webview/StatechartToolsPanel.tsx",
      "ladder/webview/LadderToolsPanel.tsx",
      "blockly/webview/BlocklyEditor.tsx",
    ];

    for (const file of panelFiles) {
      const src = readSrc(file);
      const tools = src.indexOf(">Tools<");
      const edit = src.indexOf(">Edit<");
      const view = src.indexOf(">View<");
      assert.ok(tools >= 0 && edit > tools && view > edit, `${file} must order sections as Tools → Edit → View`);
      assert.ok(!src.includes("Edit tools"), `${file} must use the shared Edit section label`);
      assert.ok(src.includes("Fit View"), `${file} must expose canvas fit/zoom from the shared View section`);
    }

    const sfc = readSrc("sfc/webview/SfcEditor.tsx");
    const statechart = readSrc("statechart/webview/StateChartEditor.tsx");
    const ladderEditor = readSrc("ladder/webview/LadderEditor.tsx");
    const blockly = readSrc("blockly/webview/BlocklyEditor.tsx");
    assert.ok(!sfc.includes("<Controls />"), "SFC must not keep a separate floating zoom-control placement");
    assert.ok(!statechart.includes("<Controls />"), "Statechart must not keep a separate floating zoom-control placement");
    assert.ok(blockly.includes("controls: false"), "Blockly must not keep its separate floating zoom-control placement");
    assert.ok(
      ladderEditor.indexOf("<LadderToolsPanel") >= 0 &&
        ladderEditor.indexOf("<ElementPropertiesPanel") > ladderEditor.indexOf("<LadderToolsPanel"),
      "Ladder must render the shared Tools/Edit/View panel before selection/rung properties"
    );

    for (const file of ["sfc/webview/SfcToolsPanel.tsx", "blockly/webview/BlocklyEditor.tsx"]) {
      const src = readSrc(file);
      assert.ok(
        src.includes("Preview ST") &&
        src.includes("Preview generated ST without saving the companion file"),
        `${file} must explain Preview ST as a preview, distinct from Generate ST`
      );
    }

    for (const file of [
      "sfc/webview/SfcToolsPanel.tsx",
      "ladder/webview/LadderToolsPanel.tsx",
      "statechart/webview/StatechartToolsPanel.tsx",
    ]) {
      const src = readSrc(file);
      assert.ok(
        src.includes("Write generated ST companion file"),
        `${file} must explain Generate ST as writing a companion file`
      );
    }
    assert.ok(
      blockly.includes("Generate Structured Text and ask whether to save it as a .st file"),
      "Blockly Generate ST must explain that saving the generated file is prompted"
    );
  });
  test("invalid visual model cards can escape to the text editor", () => {
    for (const file of [
      "statechart/webview/StateChartEditor.tsx",
      "sfc/webview/SfcEditor.tsx",
      "blockly/webview/BlocklyEditor.tsx",
    ]) {
      const src = readSrc(file);
      assert.ok(src.includes("Open as text"), `${file} must render an Open as text recovery button`);
    }

    for (const file of [
      "statechart/webview/StateChartEditor.tsx",
      "sfc/webview/SfcEditor.tsx",
      "blockly/webview/hooks/useBlockly.ts",
    ]) {
      const src = readSrc(file);
      assert.ok(
        src.includes('type: "openAsText"'),
        `${file} must post the openAsText recovery message`
      );
    }

    for (const file of [
      "statechart/stateChartEditor.ts",
      "sfc/sfcEditor.ts",
      "blockly/blocklyEditor.ts",
    ]) {
      const src = readSrc(file);
      assert.ok(src.includes('case "openAsText"'), `${file} must handle the openAsText recovery message`);
      assert.ok(
        src.includes('"vscode.openWith"') && src.includes('"default"'),
        `${file} must open the same file with VS Code's default text editor`
      );
    }
  });
  test("Blockly uses the shared truST theme instead of raw toy hues", () => {
    const editor = readSrc("blockly/webview/BlocklyEditor.tsx");
    const blocks = readSrc("blockly/webview/blocklyBlocks.ts");
    const css = readSrc("blockly/webview/blocklyTheme.css");
    assert.ok(
      editor.includes("Blockly.Theme.defineTheme(\"trust\"") &&
        editor.includes("theme: createTrustBlocklyTheme()"),
      "Blockly must inject a named truST Blockly theme"
    );
    assert.ok(
      editor.includes("workspaceBackgroundColour: resolvedThemeColor(t.canvas)") &&
        editor.includes("toolboxBackgroundColour: resolvedThemeColor(t.surface)") &&
        editor.includes("flyoutBackgroundColour: resolvedThemeColor(t.surfaceRaised)") &&
        editor.includes("mixedThemeColor(primary, t.surface, 0.72)") &&
        editor.includes("mixedThemeColor(primary, t.border, 0.58)"),
      "Blockly workspace, toolbox, and flyout surfaces must derive from shared truST tokens"
    );
    assert.ok(!/colour:\\s*\"\\d+\"/.test(editor), "Blockly toolbox categories must not use raw hue strings");
    assert.ok(!blocks.includes(".setColour("), "custom PLC Blockly blocks must use named block styles, not raw hue colours");
    assert.ok(
      css.includes("background-color: var(--trust-surface)") &&
        css.includes("fill: var(--trust-surface-raised)"),
      "Blockly toolbox and flyout CSS must stay on shared surface tokens"
    );
  });
  test("Blockly status counts visible blocks, not serialized top-level stacks", () => {
    const src = readSrc("blockly/webview/BlocklyEditor.tsx");
    assert.ok(
      src.includes("blockCount") &&
        src.includes("refreshBlockCount") &&
        src.includes("getAllBlocks(false).length"),
      "Blockly status must use the live Blockly workspace block count"
    );
    assert.ok(
      !src.includes("workspace?.blocks?.blocks?.length"),
      "Blockly status must not count only serialized top-level stacks"
    );
  });
});
