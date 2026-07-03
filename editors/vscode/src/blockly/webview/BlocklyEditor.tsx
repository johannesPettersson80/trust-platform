import React, { useCallback, useEffect, useRef, useState } from "react";
import * as Blockly from "blockly";
import { useBlockly } from "./hooks/useBlockly";
import { registerPLCBlocks } from "./blocklyBlocks";
import { PropertiesPanel } from "./PropertiesPanel";
import { CodePanel } from "./CodePanel";
import { useRightPaneResize } from "../../visual/runtime/webview/useRightPaneResize";
import "./styles.css";
import "./blocklyTheme.css";
import "../../visual/runtime/webview/rightPaneResize.css";

/**
 * Main Blockly Editor Component
 * Provides visual programming interface for PLC programs
 */
export const BlocklyEditor: React.FC = () => {
  console.log("[BlocklyEditor webview] Component rendering");

  const {
    workspace,
    generatedCode,
    errors,
    saveWorkspace,
    validateWorkspace,
    generateCode,
  } = useBlockly();

  const workspaceRef = useRef<HTMLDivElement>(null);
  const blocklyWorkspaceRef = useRef<Blockly.WorkspaceSvg | null>(null);
  const [selectedBlockId, setSelectedBlockId] = useState<string | null>(null);
  const [showCode, setShowCode] = useState(false);
  const [showProperties, setShowProperties] = useState(false);
  const [blockCount, setBlockCount] = useState(0);
  const {
    rightPaneStyle,
    resizeHandleClassName,
    resizeHandleProps,
  } = useRightPaneResize("blockly");

  const applyBlocklyTheme = useCallback(() => {
    const root = workspaceRef.current;
    if (!root) {
      return;
    }
    const styleHost = document.body ?? document.documentElement;
    const computed = getComputedStyle(styleHost);
    const canvas =
      computed.getPropertyValue("--trust-canvas").trim() ||
      computed.getPropertyValue("--vscode-editor-background").trim();
    if (!canvas) {
      return;
    }
    root.style.backgroundColor = canvas;
    const svg = root.querySelector<SVGSVGElement>(".blocklySvg");
    if (svg) {
      svg.style.backgroundColor = canvas;
    }
    root.querySelectorAll<SVGElement>(".blocklyMainBackground").forEach((background) => {
      background.style.fill = canvas;
      background.setAttribute("fill", canvas);
    });
  }, []);

  const refreshBlockCount = useCallback(() => {
    setBlockCount(blocklyWorkspaceRef.current?.getAllBlocks(false).length ?? 0);
  }, []);

  // Initialize Blockly workspace
  useEffect(() => {
    if (!workspaceRef.current || blocklyWorkspaceRef.current) {
      return;
    }

    registerPLCBlocks();

    const blocklyWorkspace = Blockly.inject(workspaceRef.current, {
      toolbox: getToolboxXML(),
      grid: {
        spacing: 20,
        length: 3,
        colour: "var(--trust-grid-line)",
        snap: true,
      },
      zoom: {
        controls: true,
        wheel: true,
        startScale: 1.0,
        maxScale: 3,
        minScale: 0.3,
        scaleSpeed: 1.2,
      },
      trashcan: true,
      move: {
        scrollbars: {
          horizontal: true,
          vertical: true,
        },
        drag: true,
        wheel: true,
      },
    });

    blocklyWorkspaceRef.current = blocklyWorkspace;
    (window as any).blocklyWorkspace = blocklyWorkspace;
    requestAnimationFrame(applyBlocklyTheme);
    console.log("[BlocklyEditor] Blockly workspace stored in window");

    const themeObserver = new MutationObserver(() => {
      requestAnimationFrame(applyBlocklyTheme);
    });
    themeObserver.observe(document.body, {
      attributes: true,
      attributeFilter: ["class", "style"],
    });

    blocklyWorkspace.addChangeListener((event: Blockly.Events.Abstract) => {
      if (
        event.type === Blockly.Events.BLOCK_CREATE ||
        event.type === Blockly.Events.BLOCK_DELETE ||
        event.type === Blockly.Events.BLOCK_CHANGE ||
        event.type === Blockly.Events.BLOCK_MOVE
      ) {
        if (Blockly.Events.getGroup()) {
          return;
        }

        const json = Blockly.serialization.workspaces.save(blocklyWorkspace);
        saveWorkspace({
          blocks: json.blocks || {},
          variables: json.variables || [],
          metadata: workspace?.metadata || { name: "Untitled", description: "" },
        });
        refreshBlockCount();
      }
    });

    console.log("Blockly workspace initialized");

    return () => {
      themeObserver.disconnect();
      blocklyWorkspace.dispose();
      blocklyWorkspaceRef.current = null;
      (window as any).blocklyWorkspace = null;
      console.log("Blockly workspace cleanup");
    };
  }, [applyBlocklyTheme, refreshBlockCount]);

  // Update workspace when data changes
  useEffect(() => {
    if (!workspace || !blocklyWorkspaceRef.current) {
      return;
    }

    try {
      blocklyWorkspaceRef.current.clear();
      const blocklyState = {
        blocks: workspace.blocks,
        variables: workspace.variables || [],
      };

      console.log("Loading workspace from JSON:", blocklyState);
      Blockly.Events.disable();
      Blockly.serialization.workspaces.load(
        blocklyState,
        blocklyWorkspaceRef.current
      );
      Blockly.Events.enable();

      console.log("✅ Workspace loaded successfully");
      console.log(
        "Total blocks in workspace:",
        blocklyWorkspaceRef.current.getAllBlocks(false).length
      );
      refreshBlockCount();
    } catch (error) {
      Blockly.Events.enable();
      console.error("❌ Failed to load workspace:", error);
      console.error("Workspace data:", workspace);
    }
  }, [workspace, refreshBlockCount]);

  const handleGenerateCode = () => {
    generateCode();
    setShowCode(true);
  };

  const handleSaveWorkspace = () => {
    const blocklyWorkspace = blocklyWorkspaceRef.current;
    if (!blocklyWorkspace) {
      return;
    }
    const json = Blockly.serialization.workspaces.save(blocklyWorkspace);
    saveWorkspace({
      blocks: json as any,
      variables: blocklyWorkspace.getAllVariables().map((variable) => ({
        id: variable.getId(),
        name: variable.name,
        type: variable.type || "",
      })),
      metadata: workspace?.metadata || { name: "BlocklyProgram" },
    });
    refreshBlockCount();
  };

  const getToolboxXML = () => {
    return {
      kind: "categoryToolbox",
      contents: [
        {
          kind: "category",
          name: "Logic",
          colour: "210",
          contents: [
            { kind: "block", type: "controls_if" },
            { kind: "block", type: "logic_compare" },
            { kind: "block", type: "logic_operation" },
            { kind: "block", type: "logic_negate" },
            { kind: "block", type: "logic_boolean" },
          ],
        },
        {
          kind: "category",
          name: "Loops",
          colour: "120",
          contents: [
            { kind: "block", type: "controls_whileUntil" },
            { kind: "block", type: "controls_for" },
            { kind: "block", type: "controls_forEach" },
            { kind: "block", type: "controls_flow_statements" },
          ],
        },
        {
          kind: "category",
          name: "Math",
          colour: "230",
          contents: [
            { kind: "block", type: "math_number" },
            { kind: "block", type: "math_arithmetic" },
            { kind: "block", type: "math_single" },
            { kind: "block", type: "math_trig" },
            { kind: "block", type: "math_constant" },
            { kind: "block", type: "math_number_property" },
            { kind: "block", type: "math_change" },
            { kind: "block", type: "math_round" },
          ],
        },
        {
          kind: "category",
          name: "Variables",
          colour: "330",
          custom: "VARIABLE",
        },
        {
          kind: "category",
          name: "Functions",
          colour: "290",
          custom: "PROCEDURE",
        },
        {
          kind: "category",
          name: "PLC I/O",
          colour: "160",
          contents: [
            { kind: "block", type: "io_digital_write" },
            { kind: "block", type: "io_digital_read" },
          ],
        },
        {
          kind: "category",
          name: "PLC Timers",
          colour: "65",
          contents: [{ kind: "block", type: "timer_ton" }],
        },
        {
          kind: "category",
          name: "PLC Counters",
          colour: "20",
          contents: [{ kind: "block", type: "counter_ctu" }],
        },
        {
          kind: "category",
          name: "Comments",
          colour: "160",
          contents: [{ kind: "block", type: "comment" }],
        },
      ],
    };
  };

  return (
    <div className="blockly-editor-container trust-product-shell">
      <header className="trust-product-header" aria-label="Blockly editor header">
        <div className="trust-product-brand">
          tru<span className="trust-product-brand__accent">ST</span>
          <span className="trust-product-brand__separator">·</span>
          <span className="trust-product-brand__surface">Blockly editor</span>
        </div>
        <div className="trust-product-header__meta">Block-based Structured Text program</div>
      </header>

      <div className="blockly-content trust-product-workspace">
        <div className="blockly-workspace-container trust-canvas-pane">
          {showCode ? (
            <CodePanel code={generatedCode} errors={errors} />
          ) : (
            <div ref={workspaceRef} className="blockly-workspace" id="blocklyDiv">
              {!workspace && (
                <div className="workspace-placeholder">
                  <p>Loading Blockly workspace...</p>
                </div>
              )}
            </div>
          )}
        </div>

        <div className={resizeHandleClassName} {...resizeHandleProps} />

        <div className="trust-inspector blockly-right-panel right-pane-resizable" style={rightPaneStyle}>
          <div className="trust-inspector__header" aria-label="Blockly editor">
            <div className="trust-inspector__title">Blockly editor</div>
          </div>

          <section className="trust-section" aria-label="Blockly tools">
            <div className="trust-section__title">Tools</div>
            {workspace?.metadata?.name && (
              <div className="trust-help" style={{ marginBottom: 8 }}>
                {workspace.metadata.name}
              </div>
            )}
            <div className="trust-button-grid">
              <button
                type="button"
                className="trust-button"
                onClick={validateWorkspace}
                disabled={!workspace}
                title="Validate generated Structured Text"
              >
                Validate
              </button>
              <button
                type="button"
                className="trust-button"
                onClick={handleGenerateCode}
                disabled={!workspace}
                title="Generate Structured Text code"
              >
                Generate ST
              </button>
              <button
                type="button"
                className="trust-button trust-button--primary"
                onClick={handleSaveWorkspace}
                disabled={!workspace}
                title="Save Blockly workspace"
              >
                Save
              </button>
            </div>
            <div className="trust-section__title" style={{ marginTop: 10 }}>View</div>
            <div className="trust-button-grid trust-button-grid--single">
              <button
                type="button"
                className="trust-button"
                onClick={() => setShowCode(!showCode)}
                title="Toggle code view"
              >
                {showCode ? "Show Blocks" : "Show Code"}
              </button>
              <button
                type="button"
                className="trust-button"
                onClick={() => setShowProperties(!showProperties)}
                title="Toggle properties panel"
              >
                {showProperties ? "Hide Properties" : "Show Properties"}
              </button>
            </div>
          </section>
          {showProperties && (
            <div className="blockly-properties-container">
              <PropertiesPanel
                workspace={workspace}
                selectedBlockId={selectedBlockId}
                onWorkspaceChange={saveWorkspace}
              />
            </div>
          )}
        </div>
      </div>

      <div className="blockly-status-bar trust-visual-status">
        <span>
          Blocks: {blockCount} | Variables:{" "}
          {workspace?.variables?.length || 0}
        </span>
        {errors.length > 0 && (
          <span className="error-count">⚠️ {errors.length} warnings</span>
        )}
      </div>
    </div>
  );
};
