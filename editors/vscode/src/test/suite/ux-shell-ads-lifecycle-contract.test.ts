import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  exampleQuickPickItems,
  hardwareBadge,
  parseManifest,
} from "../../examples/model";
import {
  setUpRuntimeOptions,
  V1_SETUP_CAPS,
} from "../../networkCanvas/webview/setUpRuntime";
import { pickAuthToken } from "../../runtimeAuthModel";
import { CHECK_PROGRAM_COMMAND } from "../../checkProgram";
import { summarizeCheck } from "../../checkProgramModel";
import {
  protocolColor,
  protocolName,
} from "../../networkCanvas/webview/protocolMeta";

// v5 "complete PLC IDE shell" contract guards (vscode-ux-overhaul-plan.md §0.5/§0.6/§9). This file holds
// the package.json + source invariants for the shell: palette cleanup, no user-facing Communication
// panel, the two sidebar states, no "Network Canvas" jargon, examples manifest, etc. The Run-card MODEL
// assertions live in runtime-controls-contract.test.ts.

type MenuItem = { command?: string; when?: string; group?: string };
type Pkg = {
  activationEvents?: string[];
  contributes?: {
    commands?: Array<{ command?: string; title?: string; category?: string }>;
    configuration?: unknown;
    languageModelTools?: Array<{ name?: string; displayName?: string }>;
    menus?: {
      commandPalette?: MenuItem[];
      "editor/title"?: MenuItem[];
      "view/title"?: MenuItem[];
      "view/item/context"?: MenuItem[];
    };
    viewsContainers?: { activitybar?: Array<{ id?: string }> };
    views?: Record<string, Array<{ id?: string; type?: string }>>;
    viewsWelcome?: Array<{ view?: string; contents?: string }>;
    debuggers?: Array<{
      type?: string;
      label?: string;
      initialConfigurations?: Array<{ name?: string; request?: string }>;
    }>;
  };
  scripts?: Record<string, string>;
};

type ConfigurationContribution = {
  title?: string;
  properties?: Record<
    string,
    {
      title?: string;
      description?: string;
      markdownDescription?: string;
    }
  >;
};

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function workspaceRoot(): string {
  return path.resolve(extensionRoot(), "..", "..");
}

function loadPackageJson(): Pkg {
  return JSON.parse(
    fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8"),
  ) as Pkg;
}

function readSrc(file: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", file), "utf8");
}

function readSrcSet(...files: string[]): string {
  return files.map((file) => readSrc(file)).join("\n");
}

function readTrustHomeSource(): string {
  return readSrcSet(
    "trustHomeView.ts",
    "trustHomeWebview.ts",
    "trustHomePresentation.ts",
    "trustHomeNavigation.ts",
  );
}

function paletteHidden(pkg: Pkg, command: string): boolean {
  const entries = pkg.contributes?.menus?.commandPalette ?? [];
  return entries.some(
    (item) => item.command === command && item.when === "false",
  );
}

function commandTitles(pkg: Pkg): Map<string, string> {
  const map = new Map<string, string>();
  for (const command of pkg.contributes?.commands ?? []) {
    if (command.command) {
      map.set(command.command, command.title ?? "");
    }
  }
  return map;
}

// The leaked palette commands the v5 cleanup hides (§0.5.6). Each stays REGISTERED (escape hatch) but is
// hidden from Ctrl+Shift+P so core flows route through visible surfaces, not the palette.
const HIDDEN_FROM_PALETTE = [
  "trust-lsp.debug.openIoPanel", // "Open Live Values" — reached via the Live Values launcher
  "trust-lsp.debug.start", // "Start Debugging" — F5 uses the debugger, not the palette
  "trust-lsp.debug.attach", // "Attach Debugger"
  "trust-lsp.debug.ensureConfiguration", // target selection lives in the sidebar, not the palette
  "trust-lsp.debug.reload", // Update running simulation drives this internal command
  "trust-lsp.test.runAll", // tests live in VS Code's native Testing view
  "trust-lsp.test.runOne", // tests live in VS Code's native Testing view
  "trust-lsp.hmi.init", // raw HMI init — reached via the adaptive HMI launcher
  "trust-lsp.hmi.refreshFromDescriptor", // raw HMI refresh
];

const RETIRED_COMMUNICATION_COMMANDS = [
  "trust-lsp.communication.openPanel",
  "trust-lsp.ads.openPanel",
  "trust-lsp.ads.server.openPanel",
  "trust-lsp.ads.addDevice",
  "trust-lsp.ads.diagnose",
  "trust-lsp.ads.importSymbols",
  "trust-lsp.ads.addRoute",
];

suite("UX shell ADS and lifecycle contracts", function () {
  test("a genuinely empty topology is terminal and actionable, never indefinite loading", () => {
    const overlay = readSrc("networkCanvas/webview/NetworkCanvasOverlays.tsx");
    assert.ok(
      overlay.includes('data-role="canvas-empty-state"') &&
        overlay.includes("No devices or runtimes yet") &&
        overlay.includes(
          "Select Discover ADS devices to search this computer and the local",
        ) &&
        overlay.includes("Start the Simulator to show this project here."),
      "an empty graph must explain what it means and what to do next",
    );
    assert.ok(
      !overlay.includes("Loading your devices...") &&
        !overlay.includes("Loading your devices…"),
      "neither ASCII nor Unicode-ellipsis loading copy may return for a terminal empty graph",
    );
  });

  test("an empty Simulator card leads with Discover before manual Add", () => {
    const nodes = readSrc("networkCanvas/webview/nodes.tsx");
    assert.ok(
      nodes.includes(">Discover ADS devices</span> to find ADS devices already running") &&
        nodes.includes(">+ Add</span> to configure one"),
      "first-run Simulator guidance must lead with automatic discovery and explain manual Add",
    );
    assert.ok(
      !nodes.includes(">+ Add</span> to add one, or") &&
        !nodes.includes("Discover</span> to scan the network"),
      "retired Add-first copy must not return",
    );
  });

  test("Simulator status names and controls agree across the canvas", () => {
    const nodes = readSrc("networkCanvas/webview/nodes.tsx");
    const sidebar = readSrc("trustHomeWebview.ts");
    const inspector = readSrcSet(
      "networkCanvas/webview/NodeInspector.tsx",
      "networkCanvas/webview/NodeSummaryView.tsx",
    );
    const graph = readSrc("networkCanvas/graphData.ts");

    assert.ok(
      nodes.includes("const effectiveLabel = label ?? statusLabel(health)") &&
        nodes.includes("title={effectiveLabel}") &&
        nodes.includes("aria-label={effectiveLabel}"),
      "the visible Simulator Running label must also own its tooltip and accessible name",
    );
    assert.ok(
      sidebar.includes('targetValue.textContent = msg.selected.label + " · " + msg.selected.statusLabel') &&
        nodes.includes('["State", lifecycleState]') &&
        nodes.includes('["Target", targetKind]') &&
        !nodes.includes('[["mode", d.mode], ["health", d.health]'),
      "sidebar and canvas hover must visibly use the same user-facing lifecycle state",
    );
    assert.ok(
      nodes.includes("function runtimeCardSurface") &&
        nodes.includes('tone: "running"') &&
        nodes.includes('tone: "neutral"') &&
        nodes.includes('tone: "error"') &&
        nodes.includes("data-surface-tone={surface.tone}"),
      "Simulator card tint must be neutral while stopped/starting, error-toned on failure, and green only while running",
    );
    assert.ok(
      inspector.includes("const summaryHealthLabel =") &&
        inspector.includes("title={summaryHealthLabel}") &&
        inspector.includes("aria-label={summaryHealthLabel}"),
      "the runtime inspector health marker must expose Running, never raw connected",
    );
    assert.ok(
      inspector.includes('node.type === "runtime"') &&
        inspector.includes("runtimeNodeControlsForNode({") &&
        inspector.includes("nodeId: node.id") &&
        inspector.includes(
          "Use Start and Stop in the truST sidebar on the left.",
        ),
      "the local Simulator inspector must defer lifecycle control to the visible truST sidebar",
    );
    assert.ok(
      !/actions:\s*\[\{ label: "Start simulator"/.test(graph) &&
        !/actions:\s*\[\{ label: "Stop simulator"/.test(graph),
      "the canvas banner must not create a second Simulator Start/Stop surface",
    );
  });

  test("unsupported Deploy and duplicate Debug stay out of the novice action row", () => {
    // Not a registered palette command …
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/send to plc|deploy to/i.test(title),
        `${command} must not expose a deploy action before the backend exists`,
      );
    }
    const view = readTrustHomeSource();
    assert.ok(
      !view.includes('id="deploy"') &&
        !view.includes('id="debug"') &&
        !view.includes('case "deploy"') &&
        !view.includes('case "debug"'),
      "unsupported Deploy and a second local launch path must stay hidden until a selected target genuinely supports them",
    );
    assert.ok(
      !/send to plc/i.test(view),
      "the sidebar must not use the old Send to PLC wording",
    );
  });

  test("sidebar two-button state table is explicit and has one primary source of truth", () => {
    const view = readTrustHomeSource();
    for (const fn of ["compileButtonState", "runtimeActionButtonState"]) {
      assert.ok(
        view.includes(`function ${fn}`),
        `${fn} must own one sidebar button state table`,
      );
    }
    assert.ok(
      view.includes('case "start"') &&
        view.includes('case "connect"') &&
        view.includes('variant: enabled ? "filled" : "outline"') &&
        view.includes('tone: enabled ? "primary" : "disabled"'),
      "Start/Connect are the only runtime actions that become filled primary buttons",
    );
    assert.ok(
      view.includes('case "stop"') &&
        view.includes('case "disconnect"') &&
        view.includes('tone: "neutral"') &&
        view.includes('variant: "outline"'),
      "Stop/Disconnect must stay neutral outlined routine actions",
    );
    assert.ok(
      view.includes("setButton(compileEl") &&
        view.includes("setButton(actionEl") &&
        !view.includes("setButton(debugEl") &&
        !view.includes("setButton(deployEl"),
      "the two visible buttons must be projected from typed state without hidden duplicate controls",
    );
    assert.ok(
      !view.includes("🐞") &&
        !view.includes("⚒") &&
        !view.includes("⤓") &&
        !view.includes("▶"),
      "the action row must not use emoji/text glyphs; Codicons carry the shape",
    );
  });

  test("manual ADS setup stays advanced and uses explicit intent wording", () => {
    const paneSrc = readSrc("networkCanvas/webview/AddPane.tsx");
    const groupingSrc = readSrc("networkCanvas/webview/grouping.ts");
    const formSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");

    assert.ok(
      groupingSrc.includes('label: "ADS advanced setup"') &&
        groupingSrc.includes('title: "Connect using a known ADS address"') &&
        groupingSrc.includes('title: "Expose this truST runtime as an ADS server"') &&
        !groupingSrc.includes('ids: ["opcua_client", "ads"]') &&
        !groupingSrc.includes('ids: ["opcua", "ads_server"]'),
      "automatic discovery must be the novice path; manual ADS client/server setup stays advanced",
    );
    assert.ok(
      paneSrc.includes('data-role="add-picker-item"') &&
        paneSrc.includes("data-protocol={item.protocol.id}"),
      "rendered acceptance must be able to identify each intent without relying on layout",
    );
    assert.ok(
      formSrc.includes("protocolSelectionLocked") &&
        formSrc.includes('data-role="locked-protocol"') &&
        formSrc.includes('data-role="protocol-selector"') &&
        formSrc.includes("protocolSelectionLocked && protocol"),
      "a preselected connection type must render as locked text, with a selector only as a fallback",
    );
    assert.ok(
      !/TwinCAT/i.test(`${groupingSrc}\n${paneSrc}\n${formSrc}`),
      "the ADS add journey must stay protocol-first and vendor-neutral",
    );
    assert.strictEqual(protocolName("ads"), "Read from ADS");
    assert.strictEqual(protocolName("ads_server"), "Share over ADS");
    assert.strictEqual(protocolColor("ads_server"), protocolColor("opcua"));
  });

  test("ADS route recovery stays in the Browse pane and exposes honest route setup", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    const panel = readSrc("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      browse.includes("routeCreateAttempted") &&
        />\s*Route setup\s*<\/button>/.test(browse) &&
        !browse.includes(
          "artifacts.length === 0 && <button onClick={onCreateRoute}",
        ),
      "ADS missing-route recovery must always expose a visible Route setup action, even when generated route artifacts exist",
    );
    assert.ok(
      browse.includes("const routeWarningText = routeCreateAttempted") &&
        browse.includes(
          "Route needs administrator access. Run the generated route script on the ADS device, then select Retry browse.",
        ) &&
        browse.includes(
          "Automatic route creation is not available from this canvas in this build.",
        ) &&
        /Run the\s+generated PowerShell as Administrator on the ADS device/.test(
          browse,
        ) &&
        browse.includes("The remote ADS router needs a route back to truST."),
      "Route setup must explain the administrator/manual route requirement in the fixed warning strip and Browse pane",
    );
    assert.ok(
      !panel.includes("ADS panel's route doctor") &&
        panel.includes(
          "Run the generated ADS route PowerShell as Administrator on the remote ADS device",
        ),
      "the Route setup handler must not send users to a retired ADS panel",
    );
  });
});
