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

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function workspaceRoot(): string {
  return path.resolve(extensionRoot(), "..", "..");
}

function loadPackageJson(): Pkg {
  return JSON.parse(
    fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8")
  ) as Pkg;
}

function readSrc(file: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", file), "utf8");
}

function paletteHidden(pkg: Pkg, command: string): boolean {
  const entries = pkg.contributes?.menus?.commandPalette ?? [];
  return entries.some((item) => item.command === command && item.when === "false");
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
  "trust-lsp.debug.reload", // "Hot Reload" — Update running simulation drives this
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

suite("Phase 1 — palette cleanup (v5 shell)", () => {
  test("the leaked core commands are hidden from the command palette (when:false)", () => {
    const pkg = loadPackageJson();
    for (const command of HIDDEN_FROM_PALETTE) {
      assert.ok(
        paletteHidden(pkg, command),
        `${command} must be hidden from the command palette (when:false)`
      );
    }
  });

  test("retired Communication and ADS panel commands are removed, not hidden escapes", () => {
    const pkg = loadPackageJson();
    const titles = commandTitles(pkg);
    const activationEvents = JSON.stringify(pkg.activationEvents ?? []);
    const menuSurface = JSON.stringify(pkg.contributes?.menus ?? {});
    for (const command of RETIRED_COMMUNICATION_COMMANDS) {
      assert.ok(!titles.has(command), `${command} must not remain a contributed command`);
      assert.ok(
        !activationEvents.includes(command),
        `${command} must not remain an activation event`
      );
      assert.ok(!menuSurface.includes(command), `${command} must not remain in menus`);
    }
  });

  test("hidden commands are still registered where the surface contract keeps them", () => {
    const titles = commandTitles(loadPackageJson());
    for (const command of HIDDEN_FROM_PALETTE) {
      assert.ok(
        titles.has(command),
        `${command} must remain a registered command (hidden from palette, not removed)`
      );
    }
  });

  test("retired 3D twin surface is absent from the VS Code product surface", () => {
    const retiredSlug = ["trust", "twin"].join("-");
    const retiredCamel = ["trust", "Twin"].join("");
    const retiredSnake = ["trust", "twin"].join("_");
    const retiredPattern = new RegExp(
      `${retiredSlug}|${retiredCamel}|${retiredSnake}`,
      "i"
    );
    const pkg = loadPackageJson();
    const packageSurface = JSON.stringify({
      activationEvents: pkg.activationEvents,
      commands: pkg.contributes?.commands,
      languageModelTools: pkg.contributes?.languageModelTools,
      menus: pkg.contributes?.menus,
      scripts: pkg.scripts,
    });
    assert.ok(
      !retiredPattern.test(packageSurface),
      "retired 3D twin surface must not be contributed as an activation event, command, LM tool, menu, or build script"
    );
    for (const removedFile of [
      `${retiredCamel}Panel.ts`,
      path.join("lm-tools", `${retiredCamel}Tools.ts`),
    ]) {
      assert.ok(
        !fs.existsSync(path.join(extensionRoot(), "src", removedFile)),
        `${removedFile} must not remain in the VS Code source tree`
      );
    }
    for (const removedOutput of [
      `${retiredCamel}Panel.js`,
      `${retiredCamel}Panel.js.map`,
      path.join("lm-tools", `${retiredCamel}Tools.js`),
      path.join("lm-tools", `${retiredCamel}Tools.js.map`),
    ]) {
      assert.ok(
        !fs.existsSync(path.join(extensionRoot(), "out", removedOutput)),
        `${removedOutput} must not remain in the packaged VS Code output tree`
      );
    }
    assert.ok(
      !fs.existsSync(path.join(extensionRoot(), "media", retiredSlug)),
      "retired 3D twin media assets must not be packaged by the extension"
    );
  });

  test("no palette-visible command embeds a 'Structured Text:' category prefix (one truST category)", () => {
    const pkg = loadPackageJson();
    for (const cmd of pkg.contributes?.commands ?? []) {
      if (!cmd.command || paletteHidden(pkg, cmd.command)) {
        continue;
      }
      assert.ok(
        !(cmd.title ?? "").startsWith("Structured Text:"),
        `${cmd.command} (palette-visible) must not embed a category prefix in its title: "${cmd.title}"`
      );
    }
  });

  test("advanced refactor commands use self-explanatory Structured Text wording", () => {
    const pkg = loadPackageJson();
    const command = (pkg.contributes?.commands ?? []).find(
      (cmd: { command?: string }) => cmd.command === "trust-lsp.moveNamespace.ui"
    );
    assert.strictEqual(
      command?.title,
      "Move Structured Text Namespace",
      "namespace refactor command must not appear as the ambiguous bare title 'Move Namespace'"
    );
    const source = readSrc("namespaceMove.ts");
    assert.ok(
      source.includes("Move Structured Text Namespace") && !source.includes('"Move Namespace"'),
      "the editor code action must use the same self-explanatory namespace wording"
    );
    assert.ok(
      source.includes("return_edit: true") &&
        source.includes("applyLspWorkspaceEdit") &&
        source.includes("ensureTargetFile") &&
        source.includes("removeCreatedTargetIfEmpty"),
      "the command must request an edit, apply it in VS Code, pre-create missing targets, and clean them up on failure"
    );
  });

  test("the Communication panel is no longer user-facing", () => {
    const pkg = loadPackageJson();
    const titles = commandTitles(pkg);
    assert.ok(
      !titles.has("trust-lsp.communication.openPanel"),
      "Communication must not remain a contributed command"
    );
    const menus = pkg.contributes?.menus ?? {};
    const surfaced = [
      ...(menus["editor/title"] ?? []),
      ...(menus["view/title"] ?? []),
      ...(menus["view/item/context"] ?? []),
    ].some((item) => item.command === "trust-lsp.communication.openPanel");
    assert.ok(
      !surfaced,
      "Communication must not be reachable from any visible menu surface"
    );
  });
});

suite("Phases 2–3 — naming + nav (v5 shell)", () => {
  test("the graph is user-facing 'Devices & Connections', never 'Network Canvas'", () => {
    const titles = commandTitles(loadPackageJson());
    assert.strictEqual(
      titles.get("trust-lsp.networkCanvas.open"),
      "Open Devices & Connections",
      "the canvas command title must use the same Open-verb pattern as other destinations"
    );
  });

  test("NO user-facing command title contains the jargon 'Network Canvas'", () => {
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/network canvas/i.test(title),
        `${command} title must not contain 'Network Canvas' (got "${title}")`
      );
    }
  });

  test("native Testing view explains empty Structured Text test workspaces", () => {
    const welcomes = loadPackageJson().contributes?.viewsWelcome ?? [];
    const testingWelcome = welcomes.find(
      (entry) => entry.view === "workbench.view.testing"
    );
    assert.ok(
      testingWelcome,
      "truST must use the native Testing view welcome area for the no-tests state"
    );
    assert.match(
      testingWelcome.contents ?? "",
      /No Structured Text tests found\./,
      "the no-tests state must be honest and specific to Structured Text tests"
    );
    assert.match(
      testingWelcome.contents ?? "",
      /TEST_PROGRAM|TEST_FUNCTION_BLOCK/,
      "the no-tests state must tell a first-time user how to add an ST test"
    );
  });

  test("package contribution labels and descriptions use current product names", () => {
    const pkg = loadPackageJson();
    const contributedText = JSON.stringify({
      commands: (pkg.contributes?.commands ?? []).map((command: { command?: string; title?: string; category?: string }) => ({
        command: command.command,
        title: command.title,
        category: command.category,
      })),
      configuration: pkg.contributes?.configuration,
    });
    for (const forbidden of ["Network Canvas", "Runtime Panel", "Structured Text Runtime"]) {
      assert.ok(
        !contributedText.includes(forbidden),
        `package contribution text must not expose stale product wording: ${forbidden}`
      );
    }
  });

  test("Discover exposes Modbus host and subnet targets separately", () => {
    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");
    assert.match(
      source,
      /key:\s*"modbus-host"[\s\S]*protocol:\s*"modbus_tcp"[\s\S]*label:\s*"Modbus device"[\s\S]*input:\s*"host"/,
      "Discover must let a user scan one known Modbus host:port"
    );
    assert.match(
      source,
      /key:\s*"modbus-custom"[\s\S]*protocol:\s*"modbus_tcp"[\s\S]*label:\s*"Modbus \(custom subnet\)"[\s\S]*input:\s*"cidr"/,
      "Discover must keep the Modbus subnet scan for OT LAN sweeps"
    );
  });

	  test("Discover result cards show runtime endpoints and candidate confidence", () => {
	    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");
	    assert.ok(
	      source.includes("c.params.control_endpoint") &&
	        source.includes("c.params.host") &&
	        source.includes("formatDiscoveredEndpoint(endpoint)"),
	      "runtime discovery results must show a user-facing host:port address so Adopt is understandable"
	    );
    assert.ok(
      source.includes("c.confidence"),
      "non-runtime discovery results must still render confidence such as observed instead of hiding it"
    );
    assert.ok(
      source.includes("protocolName(c.protocol)") &&
        !source.includes("[c.protocol, c.source, c.confidence]"),
      "Discover results must display user-facing protocol names, not raw ids such as modbus_tcp/discovery"
    );
	    assert.ok(
	      source.includes('overflowWrap: "anywhere"'),
	      "runtime discovery endpoint detail must wrap instead of clipping the control endpoint"
	    );
	    assert.ok(
	      source.includes('value.startsWith("tcp://")') &&
	        source.includes('value.slice("tcp://".length)'),
	      "runtime discovery must not expose tcp:// in the visible result card"
	    );
	    assert.ok(
	      source.includes("runtimeDiscoveryDetail(host, displayEndpoint)") &&
	        source.includes("return cleanEndpoint || cleanHost"),
	      "runtime discovery must show one actionable address, not conflicting host plus endpoint details"
	    );
	  });

  test("Discover copy stays first-user-facing and avoids rejected network jargon", () => {
    const source = readSrc("networkCanvas/webview/DiscoverPane.tsx");
	    for (const required of [
	      "trust-inspector",
	      "trust-inspector__header",
	      "trust-inspector__eyebrow",
	      "trust-section",
	      "trust-input",
	      "trust-button",
	    ]) {
	      assert.ok(source.includes(required), `Discover pane must use shared product chrome: ${required}`);
	    }
	    for (const forbidden of [
	      "Field devices",
	      "origin's local subnet",
	      "connect-only",
	      "Targeted (needs a host/subnet)",
	      "Runtime-only",
      "var(--vscode-editorHoverWidget-background",
      "var(--vscode-editorWidget-border",
      "Discovery needs a runtime that serves it",
    ]) {
      assert.ok(
        !source.includes(forbidden),
        `Discover pane must not expose rejected first-user wording: ${forbidden}`
      );
    }
    assert.ok(
      source.includes("device is powered on") &&
        source.includes("same network") &&
        source.includes("port or firewall") &&
        source.includes("address or subnet"),
      "empty discovery results must give concrete recovery checks instead of a vague runtime hint"
    );
  });

  test("Discover hardware scans are disabled with a reason until an origin can run them", () => {
    const pane = readSrc("networkCanvas/webview/DiscoverPane.tsx");
    assert.ok(
      pane.includes("runtimeDiscoveryReady") &&
        pane.includes("selectedStoppedRuntimeReason") &&
        pane.includes('selectedOrigin.id !== "this_host"') &&
        pane.includes("runtimeScanDisabledReason") &&
        pane.includes("disabled={Boolean(disabledReason)}") &&
        pane.includes("selectedScanRows") &&
        pane.includes('className={scanDisabled ? "trust-button" : "trust-button trust-button--primary"}') &&
        pane.includes("Start or connect a runtime before scanning EtherCAT or GPIO."),
      "runtime-only scans must stay visible but disabled-with-reason, and stopped runtime origins must disable all scan rows"
    );
    const theme = readSrc("webview/theme.css");
    assert.ok(
      theme.includes("button.trust-button:disabled") &&
        theme.includes(".trust-inspector button.trust-button:disabled") &&
        /\.trust-button:disabled[\s\S]*background:\s*var\(--trust-surface-raised\)\s*!important/.test(theme) &&
        /background-color:\s*var\(--trust-surface-raised\)\s*!important/.test(theme) &&
        /border:\s*1px solid var\(--trust-border\)\s*!important/.test(theme) &&
        /color:\s*var\(--trust-text-subtle\)\s*!important/.test(theme) &&
        /transition:\s*none\s*!important/.test(theme),
      "disabled buttons must render as neutral disabled controls using shared trust tokens, not VS Code primary blue"
    );

    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(
      app.includes("runtimeDiscoveryReady") &&
        app.includes('health === "connected"') &&
        app.includes('health === "running"') &&
        app.includes("before scanning from it") &&
        app.includes("Choose a running runtime for EtherCAT or GPIO scans."),
      "Discover origins must derive hardware-scan readiness from the rendered runtime node state, not from hardcoded availability"
    );
  });

  test("Discover Adopt preserves the runtime label and focuses the adopted node", () => {
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(
      app.includes('post({ type: "addHost", endpoint, label })'),
      "Adopt must pass the discovered runtime label to the extension"
    );
    assert.ok(
      app.includes('msg.type === "focusNode"') && app.includes("setSelectedId(msg.nodeId)"),
      "the canvas must select the adopted runtime after the extension refreshes the graph"
    );

    const panel = readSrc("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      panel.includes("fleetEndpointLabels") && panel.includes("label: fleetEndpointLabels.get(endpoint)"),
      "fleet targets must keep the discovered runtime label when rendering the configured peer"
    );
    assert.ok(
      panel.includes("pendingFocusNodeId = fleetRuntimeNodeId(endpoint)") &&
        panel.includes('type: "focusNode"'),
      "adopting a runtime must return to the graph with the new runtime node selected"
    );
  });
});

suite("Phase 5b — examples manifest + bundle (v5 shell)", () => {
  const EXAMPLES_DIR = path.join(extensionRoot(), "media", "examples");

  function manifestEntries() {
    const raw = JSON.parse(
      fs.readFileSync(path.join(EXAMPLES_DIR, "manifest.json"), "utf8")
    );
    return parseManifest(raw);
  }

  function readLaunchConfig(dir: string): {
    configurations?: Array<{
      type?: string;
      request?: string;
      name?: string;
      program?: string;
    }>;
  } {
    return JSON.parse(
      fs.readFileSync(path.join(dir, ".vscode", "launch.json"), "utf8")
    ) as {
      configurations?: Array<{
        type?: string;
        request?: string;
        name?: string;
        program?: string;
      }>;
    };
  }

  test("the manifest parses and ships the curated starters", () => {
    const ids = manifestEntries().map((entry) => entry.id);
    for (const id of [
      "empty-simulator",
      "conveyor",
      "twincat-ads",
      "raspberry-pi",
      "hmi-starter",
      "plcopen-motion-single-axis",
    ]) {
      assert.ok(ids.includes(id), `manifest must include the '${id}' starter`);
    }
  });

  test("every example folder is a runnable scaffold (the 4 files), bundled in media/", () => {
    for (const entry of manifestEntries()) {
      const dir = path.join(EXAMPLES_DIR, entry.path);
      for (const file of [
        "trust-lsp.toml",
        "runtime.toml",
        "io.toml",
        path.join(".vscode", "launch.json"),
        path.join("src", "Main.st"),
      ]) {
        assert.ok(
          fs.existsSync(path.join(dir, file)),
          `example '${entry.id}' must bundle ${file}`
        );
      }
    }
  });

  test("every bundled example has a native truST Simulator debug configuration", () => {
    for (const entry of manifestEntries()) {
      const dir = path.join(EXAMPLES_DIR, entry.path);
      const launch = readLaunchConfig(dir);
      const config = launch.configurations?.find(
        (candidate) =>
          candidate.type === "structured-text" &&
          candidate.request === "launch" &&
          candidate.name === "truST Simulator"
      );
      assert.ok(
        config,
        `example '${entry.id}' must give VS Code a native truST Simulator launch configuration`
      );
      assert.ok(
        typeof config?.program === "string" &&
          config.program.startsWith("${workspaceFolder}/src/"),
        `example '${entry.id}' launch config must point at its bundled CONFIGURATION source`
      );
      const relativeProgram = config!.program!.replace("${workspaceFolder}/", "");
      assert.ok(
        fs.existsSync(path.join(dir, relativeProgram)),
        `example '${entry.id}' launch program must exist: ${relativeProgram}`
      );
    }
  });

  test("new project scaffolding writes the same native debug configuration", () => {
    const source = readSrc("newProject.ts");
    assert.ok(
      source.includes('const LAUNCH_JSON_SOURCE = `') &&
        source.includes('"name": "truST Simulator"') &&
        source.includes('"program": "\\${workspaceFolder}/src/config.st"') &&
        source.includes('vscode.Uri.joinPath(targetUri, ".vscode")') &&
        source.includes('vscode.Uri.joinPath(vscodeUri, "launch.json")'),
      "Create project must write .vscode/launch.json so VS Code does not show No Configurations"
    );
  });

  test("debug journey fixture has a native truST Simulator launch configuration", () => {
    const dir = path.join(workspaceRoot(), "examples", "network_canvas_demo");
    const launch = readLaunchConfig(dir);
    assert.ok(
      launch.configurations?.some(
        (config) =>
          config.type === "structured-text" &&
          config.request === "launch" &&
          config.name === "truST Simulator" &&
          config.program === "${workspaceFolder}/src/config.st"
      ),
      "network_canvas_demo must not leave the native debug selector at No Configurations"
    );
  });

  test("every example runtime.toml has the sections the runtime parser requires", () => {
    // The parser requires retain/watchdog/fault (Codex review) — a compact file fails to load offline.
    for (const entry of manifestEntries()) {
      const toml = fs.readFileSync(
        path.join(EXAMPLES_DIR, entry.path, "runtime.toml"),
        "utf8"
      );
      for (const section of [
        "[runtime.control]",
        "[runtime.retain]",
        "[runtime.watchdog]",
        "[runtime.fault]",
      ]) {
        assert.ok(
          toml.includes(section),
          `example '${entry.id}' runtime.toml must declare ${section}`
        );
      }
    }
  });

  test("every example instantiates its program in a configuration", () => {
    for (const entry of manifestEntries()) {
      const srcDir = path.join(EXAMPLES_DIR, entry.path, "src");
      const stFiles = fs
        .readdirSync(srcDir)
        .filter((name) => name.toLowerCase().endsWith(".st"));
      const source = stFiles
        .map((name) => fs.readFileSync(path.join(srcDir, name), "utf8"))
        .join("\n");
      assert.ok(
        /\bCONFIGURATION\b/i.test(source),
        `example '${entry.id}' must include a CONFIGURATION so first open is warning-free`
      );
      assert.ok(
        /\bPROGRAM\s+\w+\s+WITH\b/i.test(source),
        `example '${entry.id}' must bind a program instance to a task`
      );
    }
  });

  test("hardware badges map to the user-facing requirement labels", () => {
    assert.strictEqual(hardwareBadge("none"), "No hardware");
    assert.strictEqual(hardwareBadge("twincat"), "Requires TwinCAT");
    assert.strictEqual(hardwareBadge("raspberrypi"), "Requires Raspberry Pi");
  });

  test("example gallery entries carry hardware badges", () => {
    const items = exampleQuickPickItems(manifestEntries());
    const conveyor = items.find((item) => item.id === "conveyor");
    assert.ok(conveyor, "conveyor must be offered");
    assert.strictEqual(conveyor?.description, "No hardware");
    const ads = items.find((item) => item.id === "twincat-ads");
    assert.strictEqual(ads?.description, "Requires TwinCAT");
  });

  test("example gallery has scalable search plus hardware and tag filters", () => {
    const source = readSrc("examples.ts");
    assert.ok(
      source.includes('placeholder="Search examples"') &&
        source.includes("searchWrap.hidden = false"),
      "the example gallery must always expose search; it cannot become unusable when the catalog grows"
    );
    assert.ok(
      source.includes('"no-hardware"') &&
        source.includes('"hardware"') &&
        source.includes("for (const tag of example.tags"),
      "the example gallery must generate hardware and category/tag chips instead of a flat unfiltered list"
    );
    assert.ok(
      source.includes('dataset.filterKind = "hardware"') &&
        source.includes('dataset.filterKind = "category"') &&
        source.includes("hardwareFilter") &&
        source.includes("tagFilter"),
      "hardware and category filters must be separate state so users can combine No hardware with a category"
    );
    assert.ok(
      source.includes("Clear search and filters") &&
        source.includes('hardwareFilter = "all"') &&
        source.includes('tagFilter = "all"') &&
        source.includes('searchInput.value = ""'),
      "the no-match state must provide a visible reset instead of a dead empty surface"
    );
  });

  test("example copy keeps native prompts and exposes only an acceptance-runner prompt override", () => {
    const source = readSrc("examples.ts");
    assert.ok(source.includes("showOpenDialog"), "normal users must still choose the destination through native VS Code UI");
    assert.ok(source.includes("showInputBox"), "normal users must still name the editable example copy through native VS Code UI");
    assert.ok(
      source.includes("TRUST_UX_EXAMPLE_DESTINATION") &&
        source.includes("TRUST_UX_EXAMPLE_NAME") &&
        source.includes("TRUST_UX_EXAMPLE_OPEN_FOLDER"),
      "the J-01 runner may supply native prompt answers without monkeypatching VS Code APIs"
    );
  });

  test("starter descriptions are compact enough for gallery cards", () => {
    for (const item of exampleQuickPickItems(manifestEntries())) {
      assert.ok(
        item.detail.length <= 80,
        `example '${item.id}' detail is too long for the first-run gallery card: ${item.detail.length} chars`
      );
    }
  });

  test("the PLCopen Motion starter is portable outside the repository", () => {
    const entry = manifestEntries().find((candidate) => candidate.id === "plcopen-motion-single-axis");
    assert.ok(entry, "PLCopen Motion starter must be offered");
    const dir = path.join(EXAMPLES_DIR, entry.path);
    const manifest = fs.readFileSync(path.join(dir, "trust-lsp.toml"), "utf8");
    assert.ok(
      manifest.includes('PLCopenMotionSingleAxis = { path = "libraries/plcopen_motion", version = "0.1.0" }'),
      "PLCopen Motion starter must use a project-relative vendored dependency"
    );
    assert.ok(
      fs.existsSync(path.join(dir, "libraries", "plcopen_motion", "trust-lsp.toml")),
      "PLCopen Motion starter must include the vendored library"
    );
    const source = fs.readFileSync(path.join(dir, "src", "Main.st"), "utf8");
    assert.ok(source.includes("MC_MoveAbsolute"), "starter must show an MC_ motion block");
    assert.ok(source.includes("motion_demo_cycles"), "starter must expose mapped proof values");
    assert.ok(
      !fs.existsSync(path.join(dir, "src", "Globals.st")),
      "starter should stay small enough to read on first open"
    );
  });

  test("no secrets in example fixtures (no tokens/passwords/keys)", () => {
    const offenders: string[] = [];
    const walk = (dir: string) => {
      for (const name of fs.readdirSync(dir)) {
        const full = path.join(dir, name);
        if (fs.statSync(full).isDirectory()) {
          walk(full);
          continue;
        }
        const text = fs.readFileSync(full, "utf8");
        // A real secret has a non-empty value; `auth_token = ""` (empty default in runtime.toml) is fine.
        if (/(auth_token|password|secret|api[_-]?key)\s*[:=]\s*["']?[^"'\s]/i.test(text)) {
          offenders.push(full);
        }
      }
    };
    walk(EXAMPLES_DIR);
    assert.deepStrictEqual(offenders, [], "example fixtures must not contain secrets");
  });
});

suite("Phase 0 — packaged runtime tools (v5 shell)", () => {
  test("release VSIX bundles trust-runtime beside trust-lsp and trust-debug", () => {
    const releaseWorkflow = fs.readFileSync(
      path.join(workspaceRoot(), ".github", "workflows", "release.yml"),
      "utf8"
    );
    for (const binary of ["trust-lsp", "trust-debug", "trust-runtime"]) {
      assert.ok(
        releaseWorkflow.includes(`cp target/\${{ matrix.target }}/release/${binary} editors/vscode/bin/`),
        `Unix VSIX packaging must copy ${binary} into editors/vscode/bin`
      );
      assert.ok(
        releaseWorkflow.includes(`cp target/\${{ matrix.target }}/release/${binary}.exe editors/vscode/bin/`),
        `Windows VSIX packaging must copy ${binary}.exe into editors/vscode/bin`
      );
    }
  });
});

suite("Phase 4 — Live Values (v5 shell)", () => {
  test("the values surface is named 'Live Values' (not 'Structured Text Runtime')", () => {
    const host = readSrc("ioPanel.ts");
    const legacy = readSrc("io-panel/view.ts");
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      /createWebviewPanel\(\s*"trust-io-panel",\s*"Live Values"/.test(host),
      "the panel title must be 'Live Values'"
    );
    assert.ok(host.includes("<title>Live Values</title>"), "the HTML title must be 'Live Values'");
    assert.ok(legacy.includes("<title>Live Values</title>"), "legacy HTML title must also be 'Live Values'");
    assert.ok(!legacy.includes("Structured Text Runtime"), "legacy HTML must not reintroduce the old Runtime wording");
    for (const [file, text] of [
      ["ioPanel.ts", host],
      ["io-panel/view.ts", legacy],
      ["ioPanel.webview.js", web],
    ] as const) {
      assert.ok(
        !/Runtime panel/i.test(text),
        `${file} must not reintroduce old Runtime panel wording`
      );
    }
  });

  test("write / force / release are preserved (NOT read-only)", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(host.includes("trust-lsp.debug.io.write"), "write preserved");
    assert.ok(host.includes("trust-lsp.debug.io.force"), "force preserved");
    assert.ok(host.includes("trust-lsp.debug.io.release"), "release preserved");
  });

  test("Live Values does not force a stale split beside Devices & Connections", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("function liveValuesViewColumn") &&
        host.includes('activeTab?.label === "Devices & Connections"') &&
        host.includes("return vscode.ViewColumn.Active") &&
        host.includes("return vscode.ViewColumn.Two") &&
        host.includes("liveValuesViewColumn()"),
      "opening Live Values from Devices & Connections must use the active editor group instead of forcing a blank side-by-side canvas"
    );
  });

  test("Release all forces exists end-to-end (button + message + host loop)", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    assert.ok(host.includes('id="releaseAllForces"'), "toolbar has the Release all forces button");
    assert.ok(host.includes("async function releaseAllForces"), "host releases every force");
    assert.ok(host.includes('case "releaseAllForces"'), "host handles the releaseAllForces message");
    assert.ok(
      web.includes('type: "releaseAllForces"'),
      "the webview posts releaseAllForces with the forced addresses"
    );
  });

  test("row write force and release wait for the next runtime scan before refreshing rows", () => {
    const host = readSrc("ioPanel.ts");
    for (const [name, successText] of [
      ["writeInput", "I/O write queued for"],
      ["forceInput", "I/O force active at"],
      ["releaseInput", "I/O force released at"],
    ] as const) {
      const start = host.indexOf(`async function ${name}`);
      assert.ok(start >= 0, `${name} must exist`);
      const end = host.indexOf("\nasync function", start + 1);
      const body = host.slice(start, end >= 0 ? end : undefined);
      assert.ok(body.includes(successText), `${name} must post success feedback`);
      assert.ok(
        body.includes("const previousScan = await currentIoScan();") &&
          body.includes("void requestIoStateAfterScan(previousScan);"),
        `${name} must wait for a newer scan before refreshing visible rows`
      );
    }
  });

  test("Force/Unforce work on remote attach too — the old 'not available' gate is removed", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    // The backend now forwards io.force/io.unforce via attach (bbe4dacf2), so the remote-only block is
    // gone — leaving it would be a FALSE limitation. Force/release flow on sim AND remote; the runtime
    // authorizes by role and the catch surfaces any error.
    assert.ok(
      !host.includes("REMOTE_FORCE_UNAVAILABLE") && !host.includes("isRemoteTarget"),
      "the remote-only force/release block must be removed"
    );
    assert.ok(
      !/not available for remote targets yet/i.test(host) &&
        !/not available for remote targets yet/i.test(web),
      "no stale 'not available for remote targets yet' copy remains"
    );
    assert.ok(
      !/allowForce:\s*!remote/.test(web) && !/allowRelease:\s*!remote/.test(web),
      "the webview must NOT disable force/release for remote targets"
    );
    // Still wired (sim + remote) and still surfaces backend errors honestly.
    assert.ok(
      host.includes("trust-lsp.debug.io.force") &&
        host.includes("trust-lsp.debug.io.release"),
      "force/release commands stay wired"
    );
  });

  test("viewer Live Values permissions disable Write/Force before a backend rejection", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    const runtime = fs.readFileSync(
      path.join(workspaceRoot(), "crates", "trust-runtime", "src", "control.rs"),
      "utf8"
    );

    assert.ok(
      runtime.includes('"access"') &&
        runtime.includes('"io"') &&
        runtime.includes('"write"') &&
        runtime.includes('"force"') &&
        runtime.includes('"release"') &&
        runtime.includes("connect with an engineer token"),
      "runtime status must expose role-derived I/O capabilities"
    );
    assert.ok(
      status.includes("normalizeRuntimeAccess") &&
        status.includes("controlAuthToken") &&
        status.includes("controlEndpoint") &&
        status.includes("access,"),
      "Live Values status payload must carry the active session's access capabilities"
    );
    assert.ok(
      web.includes("let currentAccess") &&
        web.includes("allowWrite: currentAccess.allowWrite") &&
        web.includes("allowForce: currentAccess.allowForce") &&
        web.includes("allowRelease: currentAccess.allowRelease"),
      "the webview must render row controls from runtime-reported capabilities"
    );
    assert.ok(
      web.includes("writeButton.disabled = !canWrite") &&
        web.includes("forceButton.disabled = !canForce") &&
        web.includes("releaseButton.disabled = !canRelease") &&
        web.includes("releaseAllForcesBtn.disabled"),
      "denied write/force/release controls must be disabled before the user clicks"
    );
    assert.ok(
      web.includes("writeDisabledReason || remoteReason || \"Write is not available for this value.\"") &&
        web.includes("\"Release force before writing this value.\"") &&
        web.includes("if (!canForce && remoteReason)") &&
        web.includes("forceButton.title = remoteReason") &&
        web.includes("setStatusText(currentAccess.reason)"),
      "denied controls must carry a visible reason, and forced rows must explain why Write is disabled"
    );
  });

  test("non-simulator force is explicitly armed before pinning a value", () => {
    const web = readSrc("ioPanel.webview.js");
    const host = readSrc("ioPanel.ts");
    const legacy = readSrc("io-panel/view.ts");
    assert.ok(
      web.includes("function forceRequiresArming") &&
        web.includes('currentMode !== "simulate"') &&
        web.includes("currentRuntimeState === \"connected\""),
      "Live Values must distinguish simulator one-click force from managed/remote force arming"
    );
    assert.ok(
      web.includes("function armForceForTarget") &&
        web.includes("Force armed for this target. Click Force again to pin a value."),
      "the arming step must be visible in the sticky status banner"
    );
    assert.ok(
      web.includes("function updateForcePolicy") &&
        web.includes("simulator pins immediately") &&
        web.includes("managed/remote targets require Arm force first") &&
        web.includes("this target requires Arm force first"),
      "the simulator-vs-managed force ceremony difference must be explained in the rendered panel"
    );
    assert.ok(
      host.includes('id="forcePolicy"') &&
        host.includes(".force-policy") &&
        host.includes("Force policy: simulator pins immediately; managed/remote targets require Arm force first."),
      "the force policy explanation must exist in the real Live Values webview HTML/CSS"
    );
    assert.ok(
      web.includes("Force remains armed for this target."),
      "release feedback must explain when the target remains armed for the session"
    );
    assert.ok(
      web.includes('forceButton.textContent = needsForceArm ? "Arm force" : "Force"'),
      "non-simulator targets must expose an Arm force first click"
    );
    assert.ok(
      /action === "force"[\s\S]*forceRequiresArming\(\)[\s\S]*!forceArmed[\s\S]*armForceForTarget\(\)/.test(web),
      "the first non-simulator Force click must arm instead of posting io.force"
    );
    for (const [name, source] of [
      ["ioPanel.ts", host],
      ["io-panel/view.ts", legacy],
    ] as const) {
      assert.ok(source.includes(".mini-btn.armed"), `${name} must style the armed force state`);
      assert.ok(
        source.includes("background: color-mix(in srgb, var(--trust-warn) 14%, var(--trust-surface))") &&
          source.includes("box-shadow: inset 2px 0 0 var(--trust-warn)"),
        `${name} must use a quiet amber treatment for force arming, not a solid action fill`
      );
    }
  });

  test("forced values are always visibly marked", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    assert.ok(host.includes(".state-badge.forced"), "CSS marks forced values in the State column");
    assert.ok(
      web.includes('"state-badge forced"') && web.includes('"FORCED"'),
      "the webview renders a FORCED state badge on forced rows"
    );
  });

  test("Live Values exposes a forced-values inventory filter", () => {
    const web = readSrc("ioPanel.webview.js");
    for (const [name, source] of [
      ["ioPanel.ts", readSrc("ioPanel.ts")],
      ["io-panel/view.ts", readSrc("io-panel/view.ts")],
    ] as const) {
      assert.ok(
        source.includes('id="forcedFilter"') &&
          source.includes("Forced") &&
          source.includes(".forced-filter") &&
          source.includes('aria-pressed="false"'),
        `${name} must render the Forced (N) filter chip in the Live Values header`
      );
      assert.ok(
        source.includes("var(--trust-warn)") &&
          source.includes(".forced-filter.active") &&
          source.includes("white-space: nowrap"),
        `${name} must style the active Forced filter with the shared force/warning role`
      );
    }
    assert.ok(
      web.includes("const forcedFilterBtn = document.getElementById(\"forcedFilter\")") &&
        web.includes("let forcedOnly = false") &&
        web.includes("function updateForcedFilter") &&
        web.includes("Forced (\" + count + \")") &&
        web.includes("forcedFilterBtn.setAttribute(\"aria-pressed\"") &&
        web.includes("forcedFilterBtn.addEventListener(\"click\"") &&
        web.includes("forcedOnly && !entry.forced") &&
        web.includes("function appendIoSection") &&
        web.includes("forcedOnly && !hasForcedEntry(entries)"),
      "the webview must count forced rows, toggle the chip, and filter to forced rows only without empty groups"
    );
  });

  test("Live Values uses explicit safety verbs for row actions", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.match(
        source,
        /writeButton\.textContent\s*=\s*[^;]*["`]Write\b/s,
        `${name} must label write actions with the explicit Write verb`
      );
      assert.match(
        source,
        /forceButton\.textContent\s*=\s*[^;]*["`]Force\b/s,
        `${name} must label force actions`
      );
      assert.ok(source.includes('textContent = "Release"'), `${name} must label release actions`);
      assert.ok(!/textContent\s*=\s*"W"/.test(source), `${name} must not use W as a safety action label`);
      assert.ok(!/textContent\s*=\s*"R"/.test(source), `${name} must not use R as a safety action label`);
      assert.ok(!/\?\s*"F\*"\s*:\s*"F"/.test(source), `${name} must not use F/F* as a safety action label`);
    }
  });

  test("Live Values explains disabled program-driven writes", () => {
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes("Outputs and memory are program-driven") &&
        web.includes("use Force to override"),
      "Live Values must show a visible hint explaining why outputs/memory Write is disabled"
    );
    assert.ok(
      web.includes("writeDisabledReason") &&
        web.includes("Program-driven") &&
        web.includes("Write is not available for this value."),
      "disabled Write buttons must carry a concrete tooltip reason"
    );
  });

  test("Live Values renders visible data-type labels instead of hidden value inference", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.ok(source.includes("typeFromAddress"), `${name} must derive BOOL/WORD-style types from I/O addresses`);
      assert.ok(
        source.includes("valueType") && source.includes("typeFromAddress(entry"),
        `${name} must prefer backend-provided I/O value types before address fallback`
      );
      assert.ok(
        source.includes('source.className = "source-subtitle"') &&
          source.includes("nameCell.appendChild(source)"),
        `${name} must render source as muted name-cell context instead of a width-consuming column`
      );
      assert.ok(source.includes('typeCell.className = "type-cell"'), `${name} must render type in its own column`);
      assert.ok(source.includes('typeCell.textContent = displayType || "—"'), `${name} must show a stable type-cell value`);
      assert.ok(source.includes('stateCell.className = "state-cell"'), `${name} must render state in its own column`);
      assert.ok(source.includes("state-badge"), `${name} must use explicit state badges`);
    }
  });

  test("Live Values keeps BOOL rows compact and contextual", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.ok(source.includes('displayType === "BOOL"'), `${name} must branch from the visible data type`);
      // BOOL rows expose an explicit TRUE/FALSE chooser in the write slot (parity with the numeric
      // write-box), so the operator picks what to write/force instead of an implicit hidden value.
      assert.ok(
        source.includes("createBoolToggle") && source.includes('"value-input bool-toggle"'),
        `${name} must give BOOL rows a TRUE/FALSE chooser in the write slot`
      );
      assert.ok(
        source.includes('toggle.value === "TRUE" ? "FALSE" : "TRUE"'),
        `${name} BOOL chooser must toggle between TRUE and FALSE`
      );
      assert.match(
        source,
        /writeButton\.textContent\s*=\s*["`]Write["`]/,
        `${name} must keep the visible BOOL Write action compact`
      );
      assert.ok(
        source.includes("valueControl.value"),
        `${name} must write/force the value chosen in the row control (BOOL toggle or numeric input)`
      );
      assert.ok(
        source.includes("if (isForced)") &&
          source.includes("actions.appendChild(releaseButton)") &&
          source.includes("actions.appendChild(forceButton)"),
        `${name} must show Release only for forced rows and Force otherwise`
      );
      assert.ok(
        /const valueControl[\s\S]*(isForced|forced)[\s\S]*\?\s*null/.test(source) &&
          source.includes("Release force before writing this value."),
        `${name} must not crowd forced rows with an editable value control beside the FORCED badge`
      );
    }
    for (const [name, source] of [
      ["ioPanel.ts", readSrc("ioPanel.ts")],
      ["io-panel/view.ts", readSrc("io-panel/view.ts")],
      ["visual/runtime/webview/stRuntimePanel.css", readSrc("visual/runtime/webview/stRuntimePanel.css")],
    ] as const) {
      assert.ok(source.includes(".value-input"), `${name} must style the value editor`);
      assert.ok(source.includes("height: 24px"), `${name} must keep row controls aligned`);
    }
  });

  test("Live Values keeps operation feedback visible in the sticky header", () => {
    const web = readSrc("ioPanel.webview.js");
    const visualRuntime = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.ts", readSrc("ioPanel.ts")],
      ["io-panel/view.ts", readSrc("io-panel/view.ts")],
    ] as const) {
      assert.ok(
        /<header>[\s\S]*<div class="status" id="status">/.test(source),
        `${name} must render operation feedback inside the sticky header`
      );
      assert.ok(
        !/<\/header>[\s\S]*<div class="status" id="status">/.test(source),
        `${name} must not hide operation feedback below the value list`
      );
      assert.ok(source.includes(".status:not(:empty)"), `${name} must hide only empty status text`);
      assert.ok(source.includes(".status.status-error"), `${name} must style failed writes/forces visibly`);
      assert.ok(source.includes(".status.status-warn"), `${name} must style armed/active force feedback as warning`);
    }
    assert.ok(
      web.includes('if (message.type === "status")') &&
        web.includes("const payload = String(message.payload || \"\")") &&
        web.includes("setStatusText(payload"),
      "status messages must go through the styled status renderer"
    );
    assert.ok(web.includes("status-error"), "webview must mark failed operations as error status");
    assert.ok(
      web.includes("status-warn") &&
        web.includes("isPermissionGuidanceText") &&
        web.includes("force armed|force active|force remains armed") &&
        web.includes("!isWarning && /queued|released|cleared/i.test(text)") &&
        web.includes('status.classList.toggle("status-error", isError)'),
      "force armed/active feedback and permission guidance must be amber warning, not green success or alarm red"
    );
    assert.ok(
      web.includes("updateForceStatusFromState") &&
        web.includes("forcedAddresses(state)") &&
        web.includes('"I/O force active at " + addresses[0]') &&
        web.includes("updateForceStatusFromState(currentState)"),
      "active forces from runtime snapshots must render a standing amber warning, even without a fresh button click"
    );
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual/runtime/webview/stRuntimePanelController.ts", visualRuntime],
    ] as const) {
      assert.ok(
        source.includes("isTransientStatusText"),
        `${name} must clear only startup/unavailable guidance when live values arrive`
      );
      assert.ok(
        source.includes("Start the runtime to see live values"),
        `${name} must clear stale empty-state guidance after live values arrive`
      );
      assert.ok(
        source.includes("TRANSIENT_STATUS_CLEAR_MS = 5000") &&
          source.includes("isAutoExpiringStatusText") &&
          source.includes("force released at") &&
          source.includes("Released \\d+ forces?") &&
          source.includes("No forces to release") &&
          /status(?:\?\.|\.)textContent === (text|message)/.test(source),
        `${name} must auto-expire short success feedback without clearing newer status`
      );
      assert.ok(
        !source.includes("I/O force active at .+"),
        `${name} must not auto-expire force-active standing-state banners`
      );
      assert.ok(
        !/if \(message\.type === "ioState"\) \{\s*setStatusText\(""\);/.test(source),
        `${name} must not clear operation feedback on every value refresh`
      );
    }
  });

  test("Live Values makes the active target and table columns visible", () => {
    const host = readSrc("ioPanel.ts");
    const legacy = readSrc("io-panel/view.ts");
    const web = readSrc("ioPanel.webview.js");
    for (const [name, source] of [
      ["ioPanel.ts", host],
      ["io-panel/view.ts", legacy],
    ] as const) {
      assert.ok(
        source.includes('aria-label="Active Live Values target"') &&
          source.includes('id="targetLabel"') &&
          source.includes(".target-strip") &&
          source.includes(".target-label"),
        `${name} must render the active Live Values target above the table`
      );
      assert.ok(
        source.includes('id="scanLabel"') &&
          source.includes("scan --") &&
          source.includes(".scan-label"),
        `${name} must render the runtime scan number above the table`
      );
      assert.ok(
        source.includes(".row-header") &&
          source.includes(".actions-heading"),
        `${name} must style visible table headers for value rows`
      );
      assert.ok(
        source.includes('aria-label="Numeric display format"') &&
          source.includes('data-numeric-format="dec"') &&
          source.includes('data-numeric-format="hex"') &&
          source.includes('data-numeric-format="bin"') &&
          source.includes(".numeric-format") &&
          source.includes(".format-toggle"),
        `${name} must expose the DEC/HEX/BIN numeric display toggle in the Live Values header`
      );
    }
    assert.ok(
      web.includes("targetLabelForStatus") &&
        web.includes("Simulator (this computer)") &&
        web.includes('runtimeState === "connected"') &&
        web.includes("Connected runtime") &&
        web.includes("Runtime at ") &&
        web.includes("Local runtime (control socket)") &&
        web.includes('"local control socket"') &&
        !web.includes('"local socket "'),
      "the webview must label simulator and attached runtime targets in user-facing words"
    );
    assert.ok(
      web.includes("function updateScanLabel") &&
        web.includes('"scan #" + scan') &&
        web.includes("Rows are from runtime scan #"),
      "the webview must update the visible scan number from each I/O state payload"
    );
    for (const label of ["Name", "Value", "Type", "State", "Actions"]) {
      assert.ok(web.includes(`textContent = "${label}"`), `Live Values rows must label ${label}`);
    }
    assert.ok(
      !web.includes('textContent = "Source"') && web.includes("source-subtitle"),
      "source provenance must stay visible as row context without adding a sixth table column"
    );
  });

  test("Live Values can display word-like values as decimal hex or binary", () => {
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes('let numericDisplayBase = "dec"') &&
        web.includes("setNumericDisplayBase") &&
        web.includes("formatIntegerForBase") &&
        web.includes("displayValueForEntry"),
      "the webview must keep numeric display format as explicit panel state"
    );
    assert.ok(
      web.includes('return "16#" + normalized.toString(16).toUpperCase().padStart(width, "0")') &&
        web.includes('return "2#" + normalized.toString(2).padStart(bits, "0")'),
      "the webview must render IEC-style HEX/BIN literals for word-like values"
    );
    for (const type of ['case "BYTE":', 'case "WORD":', 'case "DWORD":']) {
      assert.ok(web.includes(type), `numeric display toggle must cover ${type}`);
    }
  });

  test("Live Values action buttons do not wrap safety verbs", () => {
    for (const [name, source] of [
      ["ioPanel.ts", readSrc("ioPanel.ts")],
      ["io-panel/view.ts", readSrc("io-panel/view.ts")],
      ["visual/runtime/webview/stRuntimePanel.css", readSrc("visual/runtime/webview/stRuntimePanel.css")],
    ] as const) {
      assert.ok(source.includes("white-space: nowrap"), `${name} must keep Write/Force/Release on one line`);
      assert.ok(source.includes(".mini-btn"), `${name} must style action buttons explicitly`);
      assert.ok(
        source.includes("secondary") || source.includes("button-secondary"),
        `${name} must render row Write/Force controls as quiet secondary actions`
      );
      assert.ok(
        !/\.mini-btn\s*\{[\s\S]*background:\s*var\(--trust-accent\)/.test(source) &&
          !/\.mini-btn\s*\{[\s\S]*background:\s*var\(--button-bg\)/.test(source),
        `${name} must not render every row Write/Force action as a filled primary button`
      );
      assert.ok(
        source.includes("minmax(160px, max-content)") &&
          source.includes("column-gap: 6px") &&
          source.includes("width: 46px") &&
          source.includes("width: 62px"),
        `${name} must reserve enough fixed action-column width for the write/force/release controls`
      );
    }
  });

  test("Live Values long signal names cannot collapse the table columns", () => {
    const host = readSrc("ioPanel.ts");
    const legacy = readSrc("io-panel/view.ts");
    const visual = readSrc("visual/runtime/webview/stRuntimePanel.css");
    for (const [name, source] of [
      ["ioPanel.ts", host],
      ["io-panel/view.ts", legacy],
      ["visual/runtime/webview/stRuntimePanel.css", visual],
    ] as const) {
      assert.ok(
        source.includes("minmax(116px, 1fr)") &&
          source.includes("minmax(52px, max-content)") &&
          source.includes("minmax(38px, max-content)") &&
          source.includes("minmax(64px, max-content)") &&
          source.includes("minmax(160px, max-content)"),
        `${name} must keep name/value/type/state/actions visible on narrow panes`
      );
      assert.ok(source.includes("overflow-x: auto"), `${name} must stay usable in narrow panes`);
      assert.ok(
        source.includes("text-overflow: ellipsis") && source.includes("white-space: nowrap"),
        `${name} must ellipsize long names instead of letting them push into other columns`
      );
    }
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes("[entry.name, entry.address].filter(Boolean).join") &&
        web.includes("nameCell.title = nameTitle"),
      "Live Values rows must expose the full signal name and address in the title when visible text is ellipsized"
    );
    for (const [name, source] of [
      ["ioPanel.ts", host],
      ["io-panel/view.ts", legacy],
      ["visual/runtime/webview/stRuntimePanel.css", visual],
    ] as const) {
      assert.ok(
        source.includes("overflow-wrap: anywhere") && source.includes("white-space: normal"),
        `${name} must wrap source provenance in pixels instead of hiding it behind ellipsis`
      );
    }
  });

  test("Live Values uses the shared truST product theme tokens", () => {
    const active = readSrc("ioPanel.ts");
    const legacy = readSrc("io-panel/view.ts");
    for (const [name, source] of [
      ["ioPanel.ts", active],
      ["io-panel/view.ts", legacy],
    ] as const) {
      assert.ok(
        source.includes("--trust-canvas") &&
          source.includes("--trust-text") &&
          source.includes("--trust-accent"),
        `${name} must use the shared --trust-* product theme roles`
      );
      assert.ok(
        !/--(?:bg|text|muted|border|panel|table-header|row-hover|row-alt|button-bg|button-fg|button-hover|input-bg|input-fg|input-border|error|warning)\s*:/.test(
          source
        ),
        `${name} must not define a private Live Values color token layer`
      );
      assert.ok(
        !/var\(--(?:bg|text|muted|border|panel|table-header|row-hover|row-alt|button-bg|button-fg|button-hover|input-bg|input-fg|input-border|error|warning)\)/.test(
          source
        ),
        `${name} must not consume private Live Values color tokens`
      );
    }
  });

  test("stopped/no-session state is beginner-facing and clears stale values", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("Start the runtime to see live values."),
      "Live Values must explain the stopped state in user-facing language"
    );
    assert.ok(
      host.includes("function postEmptyIoState"),
      "Live Values must have a single helper for clearing stale I/O rows"
    );
    const requestIoStateBody = host.slice(
      host.indexOf("async function requestIoState"),
      host.indexOf("async function writeInput")
    );
    assert.ok(
      requestIoStateBody.includes("postUnavailableLiveValues(status);"),
      "a no-session request must clear stale rows and publish stopped guidance through the unavailable helper"
    );
    const terminateBody = host.slice(
      host.indexOf("vscode.debug.onDidTerminateDebugSession"),
      host.indexOf("vscode.debug.onDidChangeActiveDebugSession")
    );
    assert.ok(
      terminateBody.includes("postUnavailableLiveValues(terminatedSessionStatus(session));"),
      "debug session termination must clear stale rows through the unavailable helper"
    );
    assert.ok(
      !/payload:\s*"No active Structured Text debug session\."/.test(host),
      "Live Values must not display the raw debug-adapter no-session message"
    );
    assert.ok(
      /No debugger available/i.test(host) && /stIoState/i.test(host),
      "Live Values must map disconnected attach-mode stIoState failures to the same beginner-facing empty state"
    );
    assert.ok(
      host.includes("Connect to the selected runtime to see live values."),
      "Live Values must tell disconnected remote users to Connect, not Start"
    );
    assert.ok(
      host.includes("runtimeMode === \"online\"") &&
        host.includes("runtimeState !== \"connected\""),
      "Live Values disconnected guidance must branch on the selected target state"
    );
  });

  test("Live Values does not expose runtime lifecycle controls", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      !host.includes('id="runtimeStart"'),
      "Live Values must not render a Start/Stop/Connect/Disconnect lifecycle button"
    );
    assert.ok(
      !host.includes('aria-label="Runtime mode"') && !host.includes('class="mode-toggle"'),
      "Live Values must not render a Local/External target selector"
    );
    assert.ok(
      host.includes('id="releaseAllForces"'),
      "Live Values must keep value-safety controls such as Release all forces"
    );
  });

  test("attached runtimes are labelled Connected, not Stopped or Running", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    const visualController = readSrc("visual/runtime/webview/stRuntimePanelController.ts");
    for (const [name, source] of [
      ["ioPanel.webview.js", web],
      ["visual runtime controller", visualController],
    ] as const) {
      assert.ok(
        /runtimeState\s*===\s*"connected"[\s\S]{0,120}\?\s*"Connected"/.test(source),
        `${name} must show Connected for attach-mode Live Values sessions`
      );
      assert.ok(
        !source.includes('const label = isRunning ? "Running" : "Stopped"') &&
          !source.includes('runtimeStatusText.textContent = isRunning ? "Running" : "Stopped"'),
        `${name} must not use the old connected-as-running label pattern`
      );
    }
    assert.ok(
      status.includes('request === "attach"') &&
        status.includes("session.configuration.endpoint.trim()"),
      "Live Values status must source the active attach endpoint from the debug session"
    );
  });

  test("Live Values lifecycle pill is lifecycle-only and does not fake remote running", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    assert.ok(
      web.includes("runtimeStatusText.textContent = label"),
      "Live Values pill must render only the lifecycle label"
    );
    assert.ok(
      web.includes('payload.runtimeMode === "online"') && web.includes('"Not connected"'),
      "Live Values must label an unattached online target as Not connected"
    );
    assert.ok(
      !web.includes("`${label} · ${adsText}`") && !web.includes("payload.ads && payload.ads.text"),
      "Live Values pill must not append ADS/protocol commentary to lifecycle state"
    );
    const onlineReachableBranch = status.slice(
      status.indexOf('if (!running && runtimeMode === "online"'),
      status.indexOf("if (!access)")
    );
    assert.ok(
      onlineReachableBranch.includes("endpointReachable = await probeEndpointReachable(endpoint)") &&
        onlineReachableBranch.includes("fetchRuntimeStatusReport(endpoint, authToken)"),
      "unattached online targets may be probed for access/reachability"
    );
    assert.ok(
      !onlineReachableBranch.includes('runtimeState = "running"'),
      "a reachable remote without an attached Live Values session must stay Not connected, not Running"
    );
  });

  test("Live Values clears connected UI immediately when a debug session terminates", () => {
    const host = readSrc("ioPanel.ts");
    const terminateBody = host.slice(
      host.indexOf("vscode.debug.onDidTerminateDebugSession((session) =>"),
      host.indexOf("vscode.debug.onDidChangeActiveDebugSession")
    );
    assert.ok(
      host.includes("function terminatedSessionStatus") &&
        host.includes("function postUnavailableLiveValues") &&
        host.includes("postUnavailableLiveValues(terminatedSessionStatus(session))") &&
        host.indexOf("postUnavailableLiveValues(terminatedSessionStatus(session))") <
          host.indexOf("vscode.debug.onDidChangeActiveDebugSession"),
      "terminated debug sessions must immediately publish a disconnected runtime status before clearing rows"
    );
    const unavailableBody = host.slice(
      host.indexOf("function postUnavailableLiveValues"),
      host.indexOf("function terminatedSessionStatus")
    );
    assert.ok(
      unavailableBody.includes("postEmptyIoState();") &&
        unavailableBody.includes("message || liveValuesUnavailableMessage(status)") &&
        unavailableBody.includes("payload: statusMessage"),
      "terminated sessions must also clear stale rows and replace stale role/success banners with the correct unavailable message"
    );
    assert.ok(
      host.includes("/I\\/O state request failed:\\s*Canceled/i.test(message)"),
      "a canceled I/O request during Stop must clear stale Live Values instead of leaving old LIVE rows"
    );
    assert.ok(
      !terminateBody.includes("sendRuntimeStatus()"),
      "termination must not immediately recompute runtime status from a stale lifecycle snapshot"
    );
    const web = readSrc("ioPanel.webview.js");
    assert.ok(
      web.includes("function clearUnavailableRuntimeStatus") &&
        web.includes("Start the runtime to see live values") &&
        web.includes('runtimeState: "stopped"') &&
        web.includes("clearUnavailableRuntimeStatus(payload)"),
      "the webview must clear stale Connected pills when the host reports Live Values unavailable"
    );
    assert.ok(
      host.includes("postUnavailableLiveValues();") &&
        host.includes("vscode.debug.onDidChangeActiveDebugSession"),
      "active debug-session loss must clear Live Values even before a later poll fails"
    );
  });

  test("Structured Text Stop waits for termination before callers capture the UI", () => {
    const debug = readSrc("debug.ts");
    const stopBody = debug.slice(
      debug.indexOf('vscode.commands.registerCommand("trust-lsp.debug.stop"'),
      debug.indexOf('"trust-lsp.debug.io.write"')
    );
    assert.ok(
      debug.includes("function waitForStructuredTextSessionTerminated") &&
        debug.includes("vscode.debug.onDidTerminateDebugSession"),
      "Stop must have an explicit termination wait helper"
    );
    assert.ok(
      debug.includes("const structuredTextSessions = new Map") &&
        debug.includes("structuredTextSessions.set(structuredTextSessionKey(session), session)") &&
        debug.includes("for (const session of structuredTextSessions.values())") &&
        debug.includes("structuredTextSessions.delete(structuredTextSessionKey(session))"),
      "Stop must fall back to a tracked Structured Text session when VS Code has no active debug session"
    );
    assert.ok(
      stopBody.includes("const terminated = waitForStructuredTextSessionTerminated(session)") &&
        stopBody.includes("await vscode.debug.stopDebugging(session)") &&
        stopBody.includes("const stopped = await terminated") &&
        stopBody.includes("await sleep(DEBUG_STOP_UI_SETTLE_MS)"),
      "Stop command must not resolve before the structured-text session termination event and UI settle"
    );
  });

  test("Structured Text debugger exposes a named truST simulator configuration", () => {
    const pkg = loadPackageJson();
    const debug = readSrc("debug.ts");
    const structuredTextDebugger = pkg.contributes?.debuggers?.find(
      (entry) => entry.type === "structured-text"
    );
    assert.ok(structuredTextDebugger, "package.json must contribute the ST debugger");
    assert.ok(
      structuredTextDebugger?.initialConfigurations?.some(
        (config) => config.name === "truST Simulator" && config.request === "launch"
      ),
      "the native Run and Debug selector must have a user-facing truST Simulator launch option"
    );
    assert.ok(
      debug.includes("provideDebugConfigurations") &&
        debug.includes('name: "truST Simulator"') &&
        debug.includes("DebugConfigurationProviderTriggerKind.Dynamic"),
      "the debug configuration provider must supply a dynamic truST Simulator option, not leave VS Code at No Configurations"
    );
  });

  test("Live Values uses the selected runtime label instead of exposing raw endpoints", () => {
    const web = readSrc("ioPanel.webview.js");
    const status = readSrc("io-panel/status.ts");
    const lifecycle = readSrc("runtimeLifecycle.ts");
    const managedAttach = readSrc("managedRuntimeSession.ts");
    const home = readSrc("trustHomeView.ts");
    const canvas = readSrc("networkCanvas/networkCanvasPanel.ts");
    const inspector = readSrc("networkCanvas/webview/NodeInspector.tsx");

    assert.ok(
      web.includes("payload.targetLabel") &&
        web.indexOf("payload.targetLabel") <
          web.indexOf('return endpoint ? "Runtime at " + endpoint'),
      "Live Values must prefer the friendly selected target label before falling back to endpoint text"
    );
    assert.ok(
      status.includes("session?.configuration?.targetLabel") &&
        status.includes("targetLabel,"),
      "runtime status must carry the active attach target label"
    );
    assert.ok(
      lifecycle.includes("targetLabel?: string") &&
        lifecycle.includes("targetLabel,") &&
        lifecycle.includes("async connectRemote(") &&
        lifecycle.includes("endpoint: string"),
      "the shared lifecycle attach path must accept and pass a friendly target label"
    );
    assert.ok(
      managedAttach.includes("managedRuntimeLabel(name)") &&
        managedAttach.includes("runtimeLifecycleService.connectRemote("),
      "managed Start must label Live Values with the same name shown in the sidebar Target"
    );
    assert.ok(
      home.includes("runtimeLifecycleService.connectRemote(selected.id, selected.label)"),
      "sidebar Connect must pass its selected target label into Live Values"
    );
    assert.ok(
      inspector.includes('type: "runtimeConnect"') &&
        inspector.includes("label: str(node.data.label)") &&
        canvas.includes("typeof message.label === \"string\"") &&
        canvas.includes("runtimeLifecycleService.connectRemote(endpoint, label)"),
      "canvas Connect must pass the selected node label into Live Values"
    );
  });

  test("Live Values refreshes from the shared runtime lifecycle service", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("runtimeLifecycleService.onDidChange"),
      "Live Values must subscribe to the same lifecycle model as the sidebar and Devices canvas"
    );
    const subscriptionBody = host.slice(
      host.indexOf("runtimeLifecycleService.onDidChange"),
      host.indexOf("const activeSession = vscode.debug.activeDebugSession")
    );
    assert.ok(
      subscriptionBody.includes("void requestIoState();") &&
        subscriptionBody.includes("void sendRuntimeStatus();"),
      "lifecycle changes must refresh both the values table and the status badge"
    );
  });
});

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
    const src = panel();
    assert.ok(
      src.includes('class="initial-loading"') &&
        src.includes("Loading your devices...") &&
        src.includes("Reading the project's runtime and connections."),
      "the static webview HTML must render a loading state before the React bundle mounts"
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
    const src = panel();
    // In the runtimeConnect handler, after a successful connect, the target is set.
    const connectIdx = src.indexOf('case "runtimeConnect"');
    const disconnectIdx = src.indexOf('case "runtimeDisconnect"');
    const handler = src.slice(connectIdx, disconnectIdx);
    assert.ok(
      handler.includes("setSelectedRuntimeId"),
      "connecting must set the run target"
    );
  });

  test("Set as run target selects WITHOUT connecting", () => {
    const src = panel();
    assert.ok(src.includes('case "setAsRunTarget"'), "the panel handles setAsRunTarget");
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
      "trust-inspector__title",
      "trust-section",
      "trust-button",
      "trust-button-grid",
      "trust-help",
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
	      "Set up runtime",
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

    const host = panel();
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

  test("refresh does not post through a disposed canvas panel", () => {
    const src = panel();
    assert.ok(
      src.includes("const panelRef = panel;"),
      "refresh must snapshot the current webview panel before any await"
    );
    assert.ok(
      src.includes("if (panel !== panelRef)") && src.includes("return;"),
      "refresh must stop if the panel was disposed/replaced while async work was in flight"
    );
    assert.ok(
      src.includes("panelRef.webview.postMessage"),
      "refresh must post through the stable panel reference, not the mutable global"
    );
  });

  test("node inspector maps raw health ids to user-facing labels", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
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
        src.includes('rows.push(["State", stateSummary(health, str(d.detail))])') &&
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
      app.includes('vscode.postMessage({ type: "clearApplyResult" })'),
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
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    const host = panel();
    const schema = readSrc("networkCanvas/webview/browseActions.ts");

    assert.ok(
      schema.includes('case "ethercat"') &&
        schema.includes('actionLabel: "Add channels"') &&
        schema.includes('kind: "channels"'),
      "EtherCAT browse must remain a channel picker, not a tag/import flow"
    );
    assert.ok(
      app.includes('browseTags.protocol === "ethercat"') &&
        app.includes('type: "addEthercatChannels"'),
      "the webview must route selected EtherCAT channels to the dedicated save message"
    );
    assert.ok(
      host.includes("async function handleAddEthercatChannels") &&
        host.includes('case "addEthercatChannels"') &&
        host.includes("selected_channels"),
      "the host must persist selected EtherCAT channels through comm.apply"
    );
    const ethercatBranch =
      /else if \(browseTags\.protocol === "ethercat"\) \{([\s\S]*?)\n        \} else \{/.exec(
        app
      )?.[1] ?? "";
    assert.ok(
      ethercatBranch.includes('type: "addEthercatChannels"') &&
        !ethercatBranch.includes('"addTags"'),
      "EtherCAT must not fall through to the ADS addTags handler"
    );
  });
});

suite("Phases 8–10 — honest backend gating (no fakes, no dead buttons)", () => {
  test("Deploy is visible in the action row but disabled with a reason until supported", () => {
    // Not a registered palette command …
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/send to plc|deploy to/i.test(title),
        `${command} must not expose a deploy action before the backend exists`
      );
    }
    const view = readSrc("trustHomeView.ts");
    assert.ok(
      view.includes('id="deploy"') &&
        view.includes("Deploy is not available for this target yet.") &&
        view.includes("enabled: false"),
      "Deploy must keep its fixed action-row position but be disabled with a plain reason until backend support is real"
    );
    assert.ok(!/send to plc/i.test(view), "the sidebar must not use the old Send to PLC wording");
  });

  test("Compile state uses icon + token role, and clean compile settles to neutral", () => {
    // Ignore comments — only code/UI strings count.
    const code = readSrc("trustHomeView.ts")
      .split("\n")
      .filter((line) => !line.trim().startsWith("//"))
      .join("\n");
    assert.ok(
      code.includes('case "clean"') &&
        code.includes('icon: "codicon-check"') &&
        code.includes('tone: "neutral"') &&
        code.includes('variant: "outline"'),
      "a clean compile must show a check icon in the neutral outlined Compile button, not a persistent green button"
    );
    assert.ok(
      code.includes('case "dirty"') &&
        code.includes('icon: "codicon-warning"') &&
        code.includes('tone: "warning"'),
      "dirty state must be icon + warning token, not color alone"
    );
    assert.ok(
      code.includes('icon: "codicon-error"') &&
        code.includes('tone: "danger"'),
      "compile failures must be icon + danger token, not color alone"
    );
    assert.ok(
      !/build ok|build succeeded|build successful/i.test(code),
      "must NOT claim an authoritative build from sidebar diagnostics"
    );
  });

  test("sidebar four-button state table is explicit and has one primary source of truth", () => {
    const view = readSrc("trustHomeView.ts");
    for (const fn of [
      "compileButtonState",
      "runtimeActionButtonState",
      "debugButtonState",
      "deployButtonState",
    ]) {
      assert.ok(view.includes(`function ${fn}`), `${fn} must own one sidebar button state table`);
    }
    assert.ok(
      view.includes('case "start"') &&
        view.includes('case "connect"') &&
        view.includes('variant: enabled ? "filled" : "outline"') &&
        view.includes('tone: enabled ? "primary" : "disabled"'),
      "Start/Connect are the only runtime actions that become filled primary buttons"
    );
    assert.ok(
      view.includes('case "stop"') &&
        view.includes('case "disconnect"') &&
        view.includes('tone: "neutral"') &&
        view.includes('variant: "outline"'),
      "Stop/Disconnect must stay neutral outlined routine actions"
    );
    assert.ok(
      view.includes("setButton(compileEl") &&
        view.includes("setButton(actionEl") &&
        view.includes("setButton(debugEl") &&
        view.includes("setButton(deployEl"),
      "all four buttons must be projected from typed button-state objects"
    );
    assert.ok(
      !view.includes("🐞") && !view.includes("⚒") && !view.includes("⤓") && !view.includes("▶"),
      "the four-button row must not use emoji/text glyphs; Codicons carry the shape"
    );
  });

  test("Live Values does not show stale compile diagnostics before a real result", () => {
    const html = readSrc("ioPanel.ts");
    const script = readSrc("ioPanel.webview.js");
    assert.ok(
      !html.includes("Compile Diagnostics"),
      "Live Values must not show the old Runtime Panel compile-diagnostics card"
    );
    assert.ok(
      !html.includes("No compile run yet") && !script.includes("No compile run yet"),
      "Live Values must not contradict Compile with a stale no-compile state"
    );
    assert.ok(
      /id="diagnostics"[^>]*display:none/.test(html),
      "diagnostics details stay hidden until a real compile/reload result exists"
    );
  });

  test("managed local runtimes are projected into the sidebar Target from the fleet lifecycle", () => {
    const src = readSrc("trustHomeView.ts");
    // The sidebar lists real managed runtimes + drives Start/Stop
    // through the fleet lifecycle — no fake static "Local runtime" entry, no false advertising.
    assert.ok(
      src.includes("listManagedRuntimes"),
      "the sidebar must list managed runtimes from the fleet lifecycle"
    );
    assert.ok(
      src.includes("startManagedRuntime") && src.includes("stopManagedRuntime"),
      "a selected managed runtime Start/Stop must drive the fleet lifecycle (we own it)"
    );
    assert.ok(
      src.includes("attachManagedRuntimeAfterStart"),
      "managed Start must use the shared attach helper so Live Values can write/force without manual token setup"
    );
    const helper = readSrc("managedRuntimeSession.ts");
    assert.ok(
      helper.includes("runtimeLifecycleService.connectRemote(") &&
        helper.includes("result.controlEndpoint") &&
        helper.includes("managedRuntimeLabel(name)") &&
        helper.includes("setSelectedRuntimeId(name)"),
      "the shared managed-runtime attach helper must attach to the reached endpoint and set the Target"
    );
    assert.ok(
      !/LOCAL_RUNTIME_SUPPORTED/.test(src),
      "the stale static local-runtime flag must be gone"
    );
  });

  test("managed runtime tokens are imported into SecretStorage before attach", () => {
    const src = readSrc("localRuntime.ts");
    assert.ok(
      src.includes("parseRuntimeControlAuthToken"),
      "managed runtime token must be read from that runtime project's runtime.toml"
    );
    assert.ok(
      src.includes("setControlAuthToken"),
      "managed runtime token must be saved to SecretStorage, not plaintext settings"
    );
    assert.ok(
      !/runtime\.controlAuthToken/.test(src),
      "managed runtime token import must not write the legacy plaintext setting"
    );
  });
});

suite("Phase 6 — Update running simulation (simulator-only)", () => {
  test("Update running simulation is sim-only, gated on a real source change, wired to hot reload", () => {
    const src = readSrc("trustHomeView.ts");
    // sim-only + running + an actual change
    assert.ok(
      /selected\.kind === "simulator"/.test(src) &&
        /selected\.status === "running"/.test(src) &&
        /this\.sourceChanged/.test(src),
      "canApply must require simulator + running + a real source change"
    );
    // wired to the existing hot reload, not a fake
    assert.ok(
      src.includes("trust-lsp.debug.reload"),
      "Update running simulation must drive the hot-reload command"
    );
    assert.ok(
      src.includes("isReloadSuccess") &&
        src.includes("Running simulation updated.") &&
        src.includes("Update failed:"),
      "Update running simulation must expose success/failure status instead of silently hiding failures"
    );
    assert.ok(
      src.includes("Fix the errors shown in Problems, then try again.") &&
        src.includes("summarizeReloadMessage"),
      "Update running simulation must summarize compiler failures for the compact sidebar instead of dumping raw paths"
    );
    assert.ok(
      /if \(isReloadSuccess\(result\)\)[\s\S]*this\.sourceChanged = false/.test(src) &&
        /else[\s\S]*this\.sourceChanged = true/.test(src),
      "Update running simulation must clear pending state only after a successful reload and keep retry visible on failure"
    );
    // change detection is save-based (honest), and reset on Start/Apply
    assert.ok(
      src.includes("onDidSaveTextDocument") && src.includes("markSourceChanged"),
      "source-change must be detected from an actual ST save"
    );
  });

  test("debug reload LM tool reports command failure honestly", () => {
    const src = readSrc("lm-tools/debugTools.ts");
    assert.ok(
      src.includes("executeCommand<CommandResult>") &&
        src.includes("trust-lsp.debug.reload"),
      "the LM reload tool must inspect the structured reload command result"
    );
    assert.ok(
      src.includes("result.ok === false") &&
        src.includes("Failed to reload debugger:"),
      "the LM reload tool must not report success when hot reload failed"
    );
  });
});

suite("R4 — runtime auth tokens in SecretStorage (security)", () => {
  test("pickAuthToken: SecretStorage value wins; empty falls back to the legacy setting", () => {
    assert.strictEqual(pickAuthToken("sek", "legacy"), "sek");
    assert.strictEqual(pickAuthToken("", "legacy"), "legacy");
    assert.strictEqual(pickAuthToken(undefined, "legacy"), "legacy");
    assert.strictEqual(pickAuthToken("  ", " legacy "), "legacy");
    assert.strictEqual(pickAuthToken(undefined, undefined), undefined);
    assert.strictEqual(pickAuthToken("", ""), undefined);
  });

  test("token read paths use the SecretStorage-backed store, not the raw plaintext setting", () => {
    for (const file of ["runtimeTarget.ts", "runtimeLifecycle.ts", "io-panel/status.ts"]) {
      const src = readSrc(file);
      assert.ok(
        src.includes("getControlAuthToken"),
        `${file} must read tokens via getControlAuthToken`
      );
      assert.ok(
        !/config\.get<[^>]*>\("runtime\.controlAuthToken"/.test(src),
        `${file} must not read the plaintext controlAuthToken setting directly`
      );
    }
  });

  test("the legacy plaintext token setting is marked legacy + points to the secret store", () => {
    const pkg = fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8");
    const idx = pkg.indexOf("trust-lsp.runtime.controlAuthToken");
    assert.ok(idx >= 0, "the setting still exists (as a fallback)");
    const block = pkg.slice(idx, idx + 400);
    assert.ok(/legacy/i.test(block), "setting description must flag it as legacy");
    assert.ok(/secret store/i.test(block), "setting must point users to the secret store");
  });
});

suite("Phase 8 — Compile (authoritative project validation)", () => {
  test("summarizeCheck: passed vs failed wording", () => {
    assert.strictEqual(
      summarizeCheck({ ok: true, status: "ok", errors: 0, warnings: 0, issues: [], source_count: 3 }),
      "Compile passed — 3 sources, no errors."
    );
    assert.strictEqual(
      summarizeCheck({ ok: true, status: "ok", errors: 0, warnings: 0, issues: [], source_count: 1 }),
      "Compile passed — 1 source, no errors."
    );
    assert.strictEqual(
      summarizeCheck({ ok: false, status: "failed", errors: 2, warnings: 1, issues: [] }),
      "Compile failed — 2 errors, 1 warning."
    );
    assert.strictEqual(
      summarizeCheck(
        { ok: false, status: "failed", errors: 1, warnings: 0, issues: [] },
        { errors: 2, warnings: 0 }
      ),
      "Compile failed — 2 errors, 0 warnings."
    );
  });

  test("Compile is a fixed sidebar action plus palette escape hatch, not a Project bucket item", () => {
    assert.strictEqual(
      commandTitles(loadPackageJson()).get("trust-lsp.checkProgram"),
      "Compile"
    );
    const view = readSrc("trustHomeView.ts");
    assert.ok(
      view.includes('id="compile"') &&
        view.includes("CHECK_PROGRAM_COMMAND") &&
        CHECK_PROGRAM_COMMAND === "trust-lsp.checkProgram",
      "the sidebar Compile control must invoke the backend check command"
    );
    assert.ok(
      !view.includes("projectActionsMenu") && !view.includes("truST — Project"),
      "the retired Project menu must not remain"
    );
  });

  test("project switching is not hidden in the sidebar project name", () => {
    const view = readSrc("trustHomeView.ts");
    assert.ok(view.includes('id="projectName"'), "the open project name is rendered");
    assert.ok(
      !view.includes('projectNameEl.addEventListener') &&
        !view.includes('type: "projectMenu"'),
      "the project name must not become a hidden Open/Create/Example dropdown"
    );
  });

  test("truST sidebar title exposes only Settings as a visible icon; New diagram stays in overflow", () => {
    const viewTitle = loadPackageJson().contributes?.menus?.["view/title"] ?? [];
    const settings = viewTitle.find((item) => item.command === "trust-lsp.openSettings");
    const newDiagram = viewTitle.find((item) => item.command === "trust-lsp.visual.newDiagram");
    assert.ok(settings, "Settings must be available from the truST view title");
    assert.strictEqual(settings?.group, "navigation@1", "Settings is the single visible view-title icon");
    assert.ok(newDiagram, "New diagram remains available as a secondary action");
    assert.ok(
      !String(newDiagram?.group || "").startsWith("navigation"),
      "New diagram must stay in the view-title overflow, not as an unexplained second icon"
    );
  });

  test("action row has a real narrow-width collapse rule", () => {
    const view = readSrc("trustHomeView.ts");
    assert.ok(
      /@media\s*\(max-width:\s*245px\)\s*\{[\s\S]*\.action-button \.label\s*\{\s*display:\s*none;\s*\}/.test(view),
      "action labels must collapse below the sidebar width threshold instead of wrapping or clipping"
    );
    assert.ok(
      /@media\s*\(max-width:\s*245px\)\s*\{[\s\S]*\.action-button\s*\{[\s\S]*min-height:\s*32px/.test(view),
      "action buttons must tighten vertically in the narrow sidebar state"
    );
  });
});

suite("S-24 — Libraries surface contract", () => {
  test("Libraries is reachable from the sidebar and the command palette escape hatch", () => {
    assert.strictEqual(
      commandTitles(loadPackageJson()).get("trust-lsp.libraries.open"),
      "Open Libraries"
    );
    const pkg = loadPackageJson();
    assert.ok(
      pkg.contributes?.commands?.some(
        (command) => command.command === "trust-lsp.libraries.open"
      ),
      "Libraries command must be contributed as the palette escape hatch"
    );
    const view = readSrc("trustHomeView.ts");
    assert.ok(
      view.includes("trust-lsp.libraries.open") && view.includes('id="navLibraries"'),
      "the sidebar destination must invoke Libraries"
    );
    assert.ok(
      /Libraries/.test(view),
      "the sidebar destination must use the user-facing Libraries label"
    );
  });

  test("Libraries uses the shared truST theme instead of a private token layer", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes('"src", "webview", "theme.css"'),
      "Libraries must load the shared theme.css token layer"
    );
    assert.ok(
      !source.includes("--trust-canvas:"),
      "Libraries must not redeclare shared --trust-* theme tokens"
    );
    assert.ok(
      source.includes("trust-button--primary"),
      "Libraries must use shared product button roles"
    );
    const ignore = fs.readFileSync(path.join(extensionRoot(), ".vscodeignore"), "utf8");
    assert.ok(
      ignore.includes("!src/webview/theme.css"),
      "packaged VSIX must include the shared theme.css file used by Libraries"
    );
  });

  test("curated libraries are packaged and catalog remains gated", () => {
    const root = extensionRoot();
    for (const library of ["oscat", "plcopen_motion"]) {
      assert.ok(
        fs.existsSync(path.join(root, "media", "libraries", library, "trust-lsp.toml")),
        `${library} curated library must be packaged under media/libraries`
      );
    }
    const source = readSrc("libraries.ts");
    assert.ok(
      !/From library catalog/i.test(source),
      "catalog source must stay hidden until the ST-package-install backend contract exists"
    );
    assert.ok(
      source.includes("No libraries added. Add OSCAT, PLCopen Motion, or your own."),
      "empty state must be plain-language and must not mention trust-lsp.toml"
    );
  });

  test("Libraries failures stay visible with a recovery action but clear after success", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes('role="alert"') && source.includes("Fix and retry"),
      "failed add attempts must render an actionable in-surface error"
    );
    assert.ok(
      source.includes("if (result.ok || !result.message)"),
      "failed add attempts must not immediately clear the in-surface error"
    );
    assert.ok(
      source.includes("this.lastError = error;") && !source.includes("error || this.lastError"),
      "successful refreshes must clear stale library errors"
    );
  });

  test("curated library updates compare the vendored project copy", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes("const resolvedPath = curated ? manifestPath : projectInfo?.path ?? manifestPath"),
      "curated rows must use the project-vendored path, not a stale LSP project-info path"
    );
    assert.ok(
      source.includes("curated") && source.includes("packageVersion ?? dependency.version ?? projectInfo?.version"),
      "curated update state must prefer the vendored package version before project-info"
    );
  });

  test("library symbol counts use real singular/plural copy", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes('function countLabel(count: number, singular: string') &&
        source.includes('countLabel(symbols.length, "symbol")'),
      "Libraries must use a shared countLabel helper for symbol availability"
    );
    assert.ok(
      !source.includes('${symbols.length} symbols available'),
      "Libraries must not render awkward copy such as '1 symbols available'"
    );
  });
});

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
    const sidebar = readSrc("trustHomeView.ts");
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

    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
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
    const src = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
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
      src.includes('vscode.postMessage({ type: "focus", nodeId })') &&
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
        /<button[\s\S]*onClick=\{openAddPicker\}[\s\S]*\+ Add[\s\S]*<\/button>/.test(src),
      "Devices & Connections must expose a first-class + Add toolbar action that opens the picker for the selected/default runtime"
    );
    assert.ok(
      !/\bMiniMap\b|<MiniMap\b/.test(src),
      "Devices & Connections must use the shared low-prominence zoom/fit/count controls, not a separate minimap panel that clutters small graphs"
    );
    assert.ok(
      /onDiscoverAdopt[\s\S]*setDiscoverOpen\(false\);[\s\S]*setEditMode\(false\)/.test(src) &&
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
      nodes.includes(">+ Add</span> to add one") &&
        !nodes.includes(">Edit</span> to add one"),
      "a first-time user must see + Add as the primary path from an empty runtime"
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
      nodes.includes('"unknown", "disabled"') && nodes.includes('d.health === "disabled"') && nodes.includes("<StatusPill health={d.health} />"),
      "disabled endpoints must render a visible Disabled state in the graph, not only a color dot"
    );

    const panel = readSrc("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      panel.includes('case "commDisable"') && panel.includes('saveNetworkCanvasSetup(message, "disable")'),
      "Disable must write the project config through the same offline comm apply path as Save/Remove"
    );
    const offline = readSrc("networkCanvas/offlineComm.ts");
    assert.ok(
      offline.includes('"add" | "upsert" | "remove" | "disable"'),
      "offlineCommApply must allow the disable action"
    );
  });

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
        src.includes("Preview generated ST without saving the companion file"),
        `${file} must explain Show Code as a preview, distinct from Generate ST`
      );
    }
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

  test("Devices & Connections add pane uses the shared product chrome baseline", () => {
    const src = readSrc("networkCanvas/webview/AddPane.tsx");
    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-section"',
      'className="trust-button trust-button--primary"',
    ]) {
      assert.ok(
        src.includes(required),
        `AddPane must use shared product chrome: missing ${required}`
      );
    }

    for (const forbidden of [
      "--vscode-foreground",
      "--vscode-descriptionForeground",
      "--vscode-editorWidget-border",
      "--vscode-editorHoverWidget-background",
      "--vscode-input-background",
      "--vscode-input-border",
    ]) {
      assert.ok(
        !src.includes(forbidden),
        `AddPane product chrome must use shared --trust-* tokens/classes, not ${forbidden}`
      );
    }
  });

  test("Devices & Connections add pane follows the accepted S-09 picker taxonomy", () => {
    const paneSrc = readSrc("networkCanvas/webview/AddPane.tsx");
    const groupingSrc = readSrc("networkCanvas/webview/grouping.ts");

    for (const required of [
      "Add device or connection",
      "Discover devices and runtimes",
      "Devices and I/O",
      "Read tags from another PLC or server",
      "Share truST values",
      "Send and receive messages",
      "Advanced integrations",
    ]) {
      assert.ok(
        `${paneSrc}\n${groupingSrc}`.includes(required),
        `Add picker must include S-09 label: ${required}`
      );
    }

    for (const forbidden of [
      "Search protocols",
      "Field devices",
      "Supervisory services",
      "Peer links",
      "groupByCategory",
    ]) {
      assert.ok(
        !`${paneSrc}\n${groupingSrc}`.includes(forbidden),
        `Add picker must not regress to rejected wording/search: ${forbidden}`
      );
    }
  });

  test("schema json_array fields render as list editors, not raw one-line JSON", () => {
    const fieldSrc = readSrc("networkCanvas/webview/SchemaFields.tsx");
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const themeSrc = readSrc("webview/theme.css");
    const runtimeFieldsSrc = fs.readFileSync(
      path.join(
        workspaceRoot(),
        "crates",
        "trust-runtime",
        "src",
        "control",
        "comm_handlers",
        "schema",
        "fields.rs"
      ),
      "utf8"
    );

    assert.ok(
      fieldSrc.includes('field.type === "json_array"') &&
        fieldSrc.includes("<JsonArrayField"),
      "json_array fields must use the shared list editor"
    );
    assert.ok(
      fieldSrc.includes('data-field-type="json_array"') &&
        fieldSrc.includes("trust-array__item") &&
        fieldSrc.includes("trust-array__empty"),
      "json_array list editor must render visible rows/empty states"
    );
    assert.ok(
      fieldSrc.includes('field.id === "expose"') &&
        fieldSrc.includes("No globals selected yet.") &&
        fieldSrc.includes('return "global"'),
      "exposed-global fields must use user-facing copy instead of generic JSON-array wording"
    );
    assert.ok(
      !fieldSrc.includes("No expose globals yet") && !fieldSrc.includes("Add expose global"),
      "exposed-global fields must not regress to the old generic wording"
    );
    assert.ok(
      fieldSrc.includes('const parsed = JSON.parse(raw || "[]")') ||
        fieldSrc.includes('JSON.parse(raw || "[]")'),
      "json_array values must still serialize back to real arrays for comm apply"
    );
    assert.ok(
      fieldSrc.includes("function BooleanControl") &&
        fieldSrc.includes('type="checkbox"') &&
        fieldSrc.indexOf('const isBooleanField = field.type === "bool" || field.type === "boolean"') <
          fieldSrc.indexOf("field.options && field.options.length > 0") &&
        fieldSrc.includes('checked={value === "true"}') &&
        fieldSrc.includes('onChange={(checked) => onChange(String(checked))}') &&
        !fieldSrc.includes('<option value="false">false</option>') &&
        !fieldSrc.includes('<option value="true">true</option>'),
      "boolean protocol fields must render native checkboxes with On/Off labels, not dropdowns or raw true/false"
    );
    assert.ok(
      fieldSrc.includes("function sentenceFieldLabel") &&
        fieldSrc.includes("/^[A-Z0-9]+$/.test(firstWord)") &&
        fieldSrc.includes("const label = sentenceFieldLabel(field);"),
      "generic array empty states must preserve acronym field labels such as TLS ALPN and CPU affinity"
    );
    assert.ok(
      runtimeFieldsSrc.includes("Existing saved passwords are not shown here.") &&
        !runtimeFieldsSrc.includes("It is never returned by schema defaults."),
      "secret-field help must use product copy instead of schema-defaults wording"
    );
    assert.ok(
      addSrc.includes('import { coerce, Field } from "./SchemaFields"'),
      "AddDevicePanel must share the same schema field renderer as the edit inspector"
    );
    assert.ok(
      themeSrc.includes(".trust-array") &&
        themeSrc.includes(".trust-checkbox") &&
        themeSrc.includes("var(--trust-surface)") &&
        themeSrc.includes("var(--trust-border)"),
      "array editor chrome must live in the shared --trust-* theme layer"
    );
  });

  test("browse tree shows plain access labels, not protocol shorthand", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(
      browse.includes('"read/write"') && browse.includes('"read-only"'),
      "browse tree must spell out writable/read-only state"
    );
    assert.ok(
      !browse.includes(">rd<") && !browse.includes('"rd"'),
      "browse tree must not use the cryptic rd access abbreviation"
    );
  });

  test("browse add action disables honestly when there is nothing valid to add", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(
      browse.includes("collectLeafKeys") &&
        browse.includes("selectableKeys") &&
        browse.includes("selectedAddKeys") &&
        browse.includes("setSelected((prev)") &&
        browse.includes("filter((key) => selectableKeys.has(key))"),
      "browse selections must be pruned when the tree empties, errors, or changes"
    );
    assert.ok(
      browse.includes('className={addDisabledReason ? "trust-button" : "trust-button trust-button--primary"}') &&
        browse.includes("disabled={Boolean(addDisabledReason)}") &&
        browse.includes("No symbols are available to add.") &&
        browse.includes("Select at least one symbol to add.") &&
        browse.includes("Resolve the browse error before adding tags."),
      "browse Add tags/Add nodes must stay visible but neutral-disabled with a reason when no valid selection exists"
    );
    assert.ok(
      browse.includes("writeToggleDisabled") &&
        browse.includes("disabled={writeToggleDisabled}") &&
        browse.includes('cursor: writeToggleDisabled ? "not-allowed" : "pointer"'),
      "browse write-mode toggle must not remain interactive when browse results cannot be added"
    );
    assert.ok(
      !browse.includes("const PRIMARY") &&
        !browse.includes("var(--vscode-focusBorder, #2f81f7)") &&
        !browse.includes("opacity: selected.size"),
      "browse footer must use the shared trust-button contract instead of a private blue opacity button"
    );
  });

  test("OPC UA browse auth warnings have an inline credential recovery action", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    const opcua = readSrc("networkCanvas/webview/opcuaClientModel.ts");
    assert.ok(
      opcua.includes('action: "credentials"') &&
        opcua.includes("Choose username authentication or update the saved OPC UA credentials"),
      "OPC UA auth browse failures must classify to a credential recovery action"
    );
    assert.ok(
      browse.includes("onEditCredentials?: () => void") &&
        browse.includes('error.action === "credentials"') &&
        browse.includes("Edit credentials"),
      "the browse warning must show an inline Edit credentials action, not only passive text"
    );
    assert.ok(
      app.includes("const onEditBrowseCredentials = useCallback") &&
        app.includes("setBrowseTags(undefined)") &&
        app.includes("protocol: browseTags.protocol") &&
        app.includes("prefillParams: browseTags.target") &&
        app.includes("onEditCredentials={onEditBrowseCredentials}"),
      "Edit credentials must reopen the protocol form prefilled with the failed OPC UA target"
    );
  });

  test("remote browse uses one configured client connection for ADS and OPC UA", () => {
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(
      app.includes('(protocol === "opcua_client" || protocol === "ads")') &&
        app.includes("Array.isArray(connections)") &&
        app.includes("connections[0]"),
      "ADS and OPC UA client browse must pass one connection target, not the whole endpoint section"
    );
  });

  test("server endpoint summaries hide advanced transport limits by default", () => {
    const inspector = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      inspector.includes("SUMMARY_FIELD_IDS") &&
        inspector.includes("ads_server") &&
        inspector.includes("includeSummaryField(protocol, field)"),
      "endpoint summaries must use a protocol-specific allowlist instead of dumping every schema field"
    );
    for (const advanced of [
      '"max_frame_bytes"',
      '"max_sumup_items"',
      '"max_write_bytes"',
      '"max_subscriptions_per_client"',
      '"max_total_subscriptions"',
    ]) {
      assert.ok(
        !inspector.includes(advanced),
        `${advanced} must stay out of the default ADS server summary allowlist`
      );
    }
  });

  test("ADS server allowed clients render through the humanized summary, not raw JSON pins", () => {
    const inspector = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      inspector.includes("formatAdsServerAllowedClients") &&
        inspector.includes("clients_summary") &&
        inspector.includes('protocol === "ads_server" && field.id === "clients"'),
      "ADS server Allowed clients must use the runtime's humanized clients_summary instead of dumping raw client pin JSON"
    );
    assert.ok(
      !inspector.includes('rows.push(["Allowed clients", JSON.stringify'),
      "the inspector must not render raw ADS client objects by stringifying the row"
    );
  });

  test("network-canvas notifications do not expose backend protocol ids or awkward plurals", () => {
    const panel = readSrc("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      panel.includes("protocolDisplayName(protocol)") &&
        panel.includes('countLabel(names.length, "global")') &&
        panel.includes('countLabel(count, "ADS tag")'),
      "network-canvas success toasts must use user-facing protocol names and real pluralization"
    );
    assert.ok(!panel.includes("global(s)") && !panel.includes("tag(s)"));
  });

  test("add-device form does not reset user edits on schema refresh", () => {
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");

    assert.ok(
      addSrc.includes("lastInitializedKey"),
      "AddDevicePanel must remember which protocol/prefill initialized the form"
    );
    assert.ok(
      addSrc.includes("preselectParamsKey"),
      "AddDevicePanel must compare prefill content, not object identity from refreshed props"
    );
    assert.ok(
      addSrc.includes("schema/meta stream can") &&
        addSrc.includes("must not wipe fields the user is actively editing"),
      "AddDevicePanel must document why schema refreshes cannot reset active user edits"
    );
    assert.ok(
      addSrc.includes("lastInitializedKey.current !== initKey") &&
        addSrc.includes("setValues(valuesWithPrefill(protocol, preselectParams))"),
      "AddDevicePanel must reset defaults only when the selected protocol/prefill actually changes"
    );
  });

  test("add-device Test success does not render raw lifecycle tokens", () => {
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");

    assert.ok(
      addSrc.includes('lifecycle_effect === "test_ok"'),
      "AddDevicePanel must still treat comm.test success as a positive result"
    );
    assert.ok(
      /!\["blocked", "test_ok"\]\.includes\(\w+ApplyResult\.lifecycle_effect\)/.test(addSrc),
      "AddDevicePanel must not render raw lifecycle tokens such as test_ok as user-facing detail"
    );
    assert.ok(
      addSrc.includes("{lifecycleDetail &&") &&
        addSrc.includes('{lifecycleDetail}</div>'),
      "AddDevicePanel must render only filtered lifecycle detail text"
    );
  });

  test("successful add-device Save lands on the saved node without clearing the result", () => {
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const appSrc = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    const panelSrc = readSrc("networkCanvas/networkCanvasPanel.ts");

    assert.ok(
      addSrc.includes("onSaved?: (nodeId?: string) => void") &&
        addSrc.includes("onSaved(applyResult.instance_id)"),
      "AddDevicePanel must report the saved instance id after a successful Save"
    );
    assert.ok(
      appSrc.includes("onSaved={(nodeId)") &&
        appSrc.includes("setSelectedId(nodeId)") &&
        appSrc.includes("setFocusTargetId(nodeId)") &&
        appSrc.includes('post({ type: "selectNode", nodeId })'),
      "NetworkCanvasApp must select/focus the saved node after add-save"
    );
    assert.ok(
      /<AddDevicePanel[\s\S]*onSaved=\{\(nodeId\) => \{[\s\S]*setDraft\(undefined\);[\s\S]*setSelectedId\(nodeId\)[\s\S]*onClose=\{\(\) => \{\s*clearApplyResult\(\);[\s\S]*setDraft\(undefined\);[\s\S]*\/>/.test(appSrc),
      "manual close clears the result, but add-save landing must preserve it for the selected-node message"
    );
    assert.ok(
      panelSrc.includes("findSavedEndpointId(topology, protocol, params)") &&
        panelSrc.includes("result.instance_id ??") &&
        panelSrc.includes("Secret/redacted fields are intentionally absent from topology."),
      "the host must resolve a saved endpoint id from topology when comm.apply omits instance_id"
    );
  });

  test("Devices & Connections header reports active form field errors", () => {
    const appSrc = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");

    assert.ok(
      appSrc.includes("fieldIssueCount") &&
        appSrc.includes("applyResult?.field_errors?.length"),
      "header issue pill must count active apply field errors, not only graph faults"
    );
    assert.ok(
      appSrc.includes("field issue") &&
        appSrc.includes("fix highlighted fields"),
      "header issue pill must use concise, non-truncating form-validation wording"
    );
    assert.ok(
      appSrc.includes("fieldIssueTitle") &&
        appSrc.includes("Fix the highlighted fields and try again."),
      "header issue pill must keep the full form-validation message as title/help text"
    );
    assert.ok(
      appSrc.includes("fieldIssueLabel ?") &&
        appSrc.includes(": fault &&"),
      "field-validation issues must take precedence over graph-fault fallback while a form is active"
    );
  });

  test("Devices & Connections filter panel uses plain status wording", () => {
    const src = readSrc("networkCanvas/webview/FilterPanel.tsx");
    assert.ok(
      src.includes("Filter status"),
      "filter panel must use a neutral status heading that also works when all protocols are visible"
    );
    assert.ok(
      src.includes("1 hidden item needs attention.") &&
        src.includes("hidden items need attention."),
      "filter panel must use grammatically correct hidden-warning copy"
    );
    assert.ok(
      !src.includes("still need attention"),
      "filter panel must not regress to the awkward 'still need attention' wording"
    );
  });

  test("Devices & Connections node summaries use the shared product chrome baseline", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-inspector__eyebrow"',
      'className="trust-section trust-section--grow"',
      'className="trust-button"',
    ]) {
      assert.ok(
        src.includes(required),
        `Node summary must use shared product chrome: missing ${required}`
      );
    }

    for (const forbidden of ["primaryBtn", "secondaryBtn", "dangerBtn"]) {
      assert.ok(
        !src.includes(forbidden),
        `Node summary must not keep a parallel inline button style via ${forbidden}`
      );
    }
  });

  test("protocol add/edit forms use the shared product chrome baseline", () => {
    const addPanel = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const schemaFields = readSrc("networkCanvas/webview/SchemaFields.tsx");

    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-section trust-section--grow"',
      'className="trust-field"',
      'className="trust-input"',
      'className="trust-button trust-button--primary"',
      "trust-message",
    ]) {
      assert.ok(
        addPanel.includes(required),
        `AddDevicePanel must use shared product chrome: missing ${required}`
      );
    }

    for (const required of [
      'className="trust-field"',
      "trust-input",
      "trust-input--error",
      "trust-field__message",
      "trust-field__message--error",
    ]) {
      assert.ok(
        schemaFields.includes(required),
        `SchemaFields must use shared product form chrome: missing ${required}`
      );
    }

    const files = new Map([
      ["AddDevicePanel", addPanel],
      ["SchemaFields", schemaFields],
    ]);
    for (const [name, src] of files) {
      for (const forbidden of [
        "--vscode-foreground",
        "--vscode-descriptionForeground",
        "--vscode-editorWidget-border",
        "--vscode-editorHoverWidget-background",
        "--vscode-input-background",
        "--vscode-input-border",
        "--vscode-errorForeground",
        "labelStyle",
        "inputStyle",
        "primaryBtn",
        "secondaryBtn",
      ]) {
        assert.ok(
          !src.includes(forbidden),
          `${name} must not keep a parallel protocol-form chrome via ${forbidden}; use shared trust-* classes`
        );
      }
    }
  });

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
