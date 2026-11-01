// Specification 25, section 5.1: Connect changes the run target only when it succeeds, a failed Connect stays on the
// runtime's node with its recovery, a runtime that answers but keeps debugging disabled is the selected target that
// Live Values explains, and the Target picker offers the workspace project's own runtime while it answers. The runtime
// is a fake control endpoint that answers as trust-runtime does, at the endpoint the workspace project's runtime.toml
// names, so it is the project's own runtime started outside VS Code; the lifecycle, Devices & Connections, the side
// panel, the status bar and Live Values are the extension's own.
import * as assert from "assert";
import * as fs from "fs";
import * as net from "net";
import * as path from "path";
import * as React from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import * as vscode from "vscode";

import { getTrustConfiguration } from "../../configuration";
import { resolveHmiRunTarget } from "../../hmiBuilder/editor/runTarget";
import { buildGraph } from "../../networkCanvas/webview/layout";
import type { NCGraph, NCRuntime } from "../../networkCanvas/webview/types";
import { getControlAuthToken, setControlAuthToken } from "../../runtimeAuth";
import { statusText } from "../../runtimeControls";
import { OnlineStatusReport, runtimeLifecycleService } from "../../runtimeLifecycle";
import { getSelectedRuntimeId, setSelectedRuntimeId } from "../../selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "../../trustHomeModel";

const { JSDOM } = require("jsdom") as {
  JSDOM: new (html: string, options: Record<string, unknown>) => any;
};
const { buildSync } = require("esbuild") as {
  buildSync(options: Record<string, unknown>): void;
};

const EXTENSION_ROOT = path.resolve(__dirname, "..", "..", "..");
// Made up for this test; the project's runtime.toml names the same token.
const TOKEN = "connect-target-test-token";
// Connect's text for a runtime that needs a token none was stored for (spec 25 section 5.6: distinct missing-token text).
const MISSING_TOKEN = "No auth token provided — this runtime requires one.";
const RUNTIME_SETTINGS = ["runtime.controlEndpoint", "runtime.controlEndpointEnabled", "runtime.mode"] as const;
// The Target picker's entry for the workspace project's own runtime (specification 25, section 5.1).
const PROJECT_ID = "project";
const PROJECT_LABEL = "This project's runtime";
const STATUS_CONTEXT = {
  subscriptions: [],
  extensionMode: vscode.ExtensionMode.Test,
  extensionPath: EXTENSION_ROOT,
  extensionUri: vscode.Uri.file(EXTENSION_ROOT),
} as unknown as vscode.ExtensionContext;

interface FakeRuntime {
  readonly endpoint: string;
  /** How the extension names the runtime: its host and port. */
  readonly hostPort: string;
  close(): Promise<void>;
}

/**
 * The tank example's runtime as a control endpoint that answers as trust-runtime does: one JSON request per line, one
 * reply per request (control.rs). It requires TOKEN: a request without it gets `missing_auth_token`, a wrong one
 * `invalid_auth_token` (spec 11, section 6.9.6). `status` returns the runtime's state, the fields of the status annex
 * (control-studio-runtime-protocol.md E1.3), `debug_enabled = false` as the example configures it, and the `access` of
 * the primary token (Admin, control.rs); any other request is unsupported.
 */
async function startFakeRuntime(): Promise<FakeRuntime> {
  const sockets = new Set<net.Socket>();
  const answer = (request: { id?: number; type?: string; auth?: string }): Record<string, unknown> => {
    const id = request.id ?? 0;
    if (request.auth !== TOKEN) {
      return request.auth === undefined
        ? { id, ok: false, error: "missing auth token", error_code: "missing_auth_token" }
        : { id, ok: false, error: "invalid auth token", error_code: "invalid_auth_token" };
    }
    if (request.type !== "status") {
      return { id, ok: false, error: "unsupported request" };
    }
    return {
      id,
      ok: true,
      result: {
        state: "running",
        resource: "Cpu",
        debug_enabled: false,
        session_epoch: 1,
        simulation_enabled: true,
        simulation_only_io: true,
        simulated: true,
        simulation_time_scale: 1,
        simulation_achieved_time_scale: null,
        simulation_speed_text: "",
        simulation_speed_behind: false,
        program_revision: `sha256:${"0".repeat(64)}`,
        tasks: [{ name: "MainTask", interval_ms: 100 }],
        cycle_interval_ms: 100,
        access: { role: "admin", io: { write: true, force: true, release: true }, reason: null },
      },
    };
  };
  const server = net.createServer((socket) => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
    socket.on("error", () => undefined);
    let buffer = "";
    socket.on("data", (chunk) => {
      buffer += chunk.toString("utf8");
      for (let newline = buffer.indexOf("\n"); newline >= 0; newline = buffer.indexOf("\n")) {
        const line = buffer.slice(0, newline).trim();
        buffer = buffer.slice(newline + 1);
        if (line) {
          socket.write(`${JSON.stringify(answer(JSON.parse(line)))}\n`);
        }
      }
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as net.AddressInfo).port;
  return {
    endpoint: `tcp://127.0.0.1:${port}`,
    hostPort: `127.0.0.1:${port}`,
    close: async () => {
      sockets.forEach((socket) => socket.destroy());
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}

/** The tank example's project at the workspace root, whose runtime is the fake: Devices & Connections reads it there. */
function writeRootProject(root: string, endpoint: string): () => void {
  const files: Record<string, string> = {
    "runtime.toml": [
      "[bundle]", "version = 1", "",
      "[resource]", 'name = "Cpu"', "cycle_interval_ms = 100", "",
      "[runtime.control]", `endpoint = "${endpoint}"`, `auth_token = "${TOKEN}"`, 'mode = "production"', "debug_enabled = false", "",
      "[runtime.log]", 'level = "info"', "",
      "[runtime.retain]", 'mode = "none"', "save_interval_ms = 1000", "",
      "[runtime.watchdog]", "enabled = false", "timeout_ms = 5000", 'action = "halt"', "",
      "[runtime.fault]", 'policy = "halt"', "",
      "[runtime.web]", "enabled = false", 'listen = "127.0.0.1:8080"', 'auth = "local"', "tls = false", "",
      "[runtime.tls]", 'mode = "disabled"', "",
    ].join("\n"),
    "io.toml": '[io]\ndriver = "simulated"\nparams = {}\n',
    "trust-lsp.toml": '[project]\ninclude_paths = ["src"]\n',
  };
  const previous = new Map<string, string | undefined>();
  for (const [name, text] of Object.entries(files)) {
    const file = path.join(root, name);
    previous.set(file, fs.existsSync(file) ? fs.readFileSync(file, "utf8") : undefined);
    fs.writeFileSync(file, text);
  }
  return () => {
    for (const [file, text] of previous) {
      if (text === undefined) {
        fs.rmSync(file, { force: true });
      } else {
        fs.writeFileSync(file, text);
      }
    }
  };
}

interface TargetState {
  readonly settings: Record<string, unknown>;
  readonly selected: string;
  readonly statusBar: string;
}

/** The run target as the user sees it: the runtime settings, the selected target and the status bar. */
async function targetState(folder: vscode.WorkspaceFolder): Promise<TargetState> {
  const config = getTrustConfiguration(folder.uri);
  return {
    settings: Object.fromEntries(RUNTIME_SETTINGS.map((key) => [key, config.get<unknown>(key)])),
    selected: getSelectedRuntimeId(),
    statusBar: (await statusText(STATUS_CONTEXT, await runtimeLifecycleService.snapshot())).text,
  };
}

async function waitFor<T>(what: string, read: () => T | undefined | false | Promise<T | undefined | false>): Promise<T> {
  const deadline = Date.now() + 30_000;
  for (;;) {
    const value = await read();
    if (value) {
      return value;
    }
    if (Date.now() > deadline) {
      throw new Error(`Timed out waiting for ${what}.`);
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

interface CapturedPanel {
  readonly viewType: string;
  readonly posted: unknown[];
  /** A message from the webview to the extension, as the real webview sends it. */
  receive(message: unknown): void;
  dispose(): void;
}

/** Lets the extension's real panels post into memory instead of a webview. */
function captureWebviewPanels(): { readonly panels: CapturedPanel[]; restore(): void } {
  const create = vscode.window.createWebviewPanel;
  const panels: CapturedPanel[] = [];
  (vscode.window as any).createWebviewPanel = (viewType: string) => {
    const posted: unknown[] = [];
    const receivers: Array<(message: unknown) => void> = [];
    const disposers: Array<() => void> = [];
    const none = new vscode.Disposable(() => undefined);
    const dispose = () => disposers.splice(0).forEach((listener) => listener());
    panels.push({
      viewType,
      posted,
      receive: (message) => receivers.forEach((listener) => listener(message)),
      dispose,
    });
    return {
      viewType,
      visible: true,
      active: true,
      webview: {
        html: "",
        options: {},
        cspSource: "vscode-webview://connect-target-test",
        asWebviewUri: (uri: vscode.Uri) => uri,
        postMessage: async (message: unknown) => {
          posted.push(message);
          return true;
        },
        onDidReceiveMessage: (listener: (message: unknown) => void) => {
          receivers.push(listener);
          return none;
        },
      },
      reveal: () => undefined,
      onDidDispose: (listener: () => void) => {
        disposers.push(listener);
        return none;
      },
      onDidChangeViewState: () => none,
      dispose,
    };
  };
  return {
    panels,
    restore: () => {
      panels.forEach((panel) => panel.dispose());
      (vscode.window as any).createWebviewPanel = create;
    },
  };
}

function postedGraphs(panel: CapturedPanel): NCGraph[] {
  return panel.posted.flatMap((message) => {
    const posted = message as { type?: string; graph?: NCGraph };
    return posted.type === "graph" && posted.graph ? [posted.graph] : [];
  });
}

function runtimeAt(graph: NCGraph, endpoint: string): NCRuntime | undefined {
  return graph.hosts
    .flatMap((host) => [...host.runtimes, ...host.containers.flatMap((container) => container.runtimes)])
    .find((runtime) => runtime.controlEndpoint === endpoint);
}

/** Opens Devices & Connections and waits until it shows the project's runtime. */
async function openCanvas(capture: { readonly panels: CapturedPanel[] }, endpoint: string): Promise<CapturedPanel> {
  await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
  const canvas = await waitFor("Devices & Connections", () =>
    capture.panels.find((panel) => panel.viewType === "trust-network-canvas"));
  await waitFor("the project's runtime in Devices & Connections", () =>
    postedGraphs(canvas).some((graph) => runtimeAt(graph, endpoint) !== undefined));
  return canvas;
}

interface RenderedInspector {
  readonly posted: unknown[];
  primary(): string;
  click(label: string): Promise<void>;
  close(): Promise<void>;
}

/** The canvas's node inspector, bundled from its source as the canvas bundle builds it. */
function loadNodeInspector(): React.ComponentType<Record<string, unknown>> {
  const temp = fs.mkdtempSync(path.join(EXTENSION_ROOT, "node_modules", ".trust-node-inspector-test-"));
  const outfile = path.join(temp, "node-inspector.cjs");
  try {
    buildSync({
      entryPoints: [path.join(EXTENSION_ROOT, "src", "networkCanvas", "webview", "NodeInspector.tsx")],
      outfile,
      bundle: true,
      platform: "node",
      format: "cjs",
      jsx: "automatic",
      external: ["react", "react-dom", "vscode"],
      logLevel: "silent",
    });
    return require(outfile).NodeInspector as React.ComponentType<Record<string, unknown>>;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/** Renders the canvas's real node inspector for one rendered graph node. */
async function renderNodeInspector(node: { id: string; type?: string; data: Record<string, unknown> }): Promise<RenderedInspector> {
  const dom = new JSDOM('<!doctype html><body><div id="root"></div></body>', { url: "https://devices.test/" });
  const globals = globalThis as any;
  const previous = { window: globals.window, document: globals.document, act: globals.IS_REACT_ACT_ENVIRONMENT };
  const restoreGlobals = () => {
    globals.window = previous.window;
    globals.document = previous.document;
    globals.IS_REACT_ACT_ENVIRONMENT = previous.act;
    dom.window.close();
  };
  globals.window = dom.window;
  globals.document = dom.window.document;
  globals.IS_REACT_ACT_ENVIRONMENT = true;
  let root: Root | undefined;
  try {
    const NodeInspector = loadNodeInspector();
    const posted: unknown[] = [];
    const container = dom.window.document.getElementById("root");
    const mounted: Root = createRoot(container);
    root = mounted;
    await act(async () => {
      mounted.render(
        React.createElement(NodeInspector, {
          node,
          reachable: false,
          onClose: () => undefined,
          onFocus: () => undefined,
          post: (message: unknown) => posted.push(message),
        }),
      );
    });
    const button = (label: string): any =>
      [...container.querySelectorAll("button")].find((candidate: any) => candidate.textContent === label);
    return {
      posted,
      primary: () => container.querySelector("button.trust-button--primary")?.textContent ?? "",
      click: async (label: string) => {
        const target = button(label);
        assert.ok(target, `the inspector shows ${label}`);
        assert.strictEqual(target.disabled, false, `${label} is enabled`);
        await act(async () => {
          target.dispatchEvent(new dom.window.MouseEvent("click", { bubbles: true }));
        });
      },
      close: async () => {
        await act(async () => mounted.unmount());
        restoreGlobals();
      },
    };
  } catch (error) {
    const mounted = root;
    if (mounted) {
      await act(async () => mounted.unmount());
    }
    restoreGlobals();
    throw error;
  }
}

interface SidebarState {
  readonly selectedId: string;
  readonly selected: {
    readonly id: string;
    readonly label: string;
    readonly status: string;
    readonly statusLabel: string;
    readonly primary: { readonly enabled: boolean };
  };
  readonly options: ReadonlyArray<{ readonly id: string; readonly label: string; readonly kind: string }>;
}

/** The real truST side panel, resolved into a view that keeps what it posts. */
function openSidebar(): {
  states(): SidebarState[];
  pickerItems(): Promise<Array<vscode.QuickPickItem & { id?: string }>>;
  select(id: string): Promise<void>;
  press(): Promise<void>;
  dispose(): void;
} {
  const { registerTrustHome } = require("../../trustHomeView");
  const registerProvider = vscode.window.registerWebviewViewProvider;
  const registerCommand = vscode.commands.registerCommand;
  const subscriptions: vscode.Disposable[] = [];
  const none = new vscode.Disposable(() => undefined);
  let provider: any;
  (vscode.window as any).registerWebviewViewProvider = (_id: string, value: unknown) => {
    provider = value;
    return none;
  };
  (vscode.commands as any).registerCommand = () => none;
  try {
    registerTrustHome({
      subscriptions,
      extensionMode: vscode.ExtensionMode.Test,
      extensionPath: EXTENSION_ROOT,
      extensionUri: vscode.Uri.file(EXTENSION_ROOT),
    } as unknown as vscode.ExtensionContext);
  } finally {
    (vscode.window as any).registerWebviewViewProvider = registerProvider;
    (vscode.commands as any).registerCommand = registerCommand;
  }
  assert.ok(provider, "the side panel registers its view");
  const posted: unknown[] = [];
  provider.resolveWebviewView({
    visible: true,
    webview: {
      html: "",
      options: {},
      cspSource: "vscode-webview://connect-target-test",
      asWebviewUri: (uri: vscode.Uri) => uri,
      postMessage: async (message: unknown) => {
        posted.push(message);
        return true;
      },
      onDidReceiveMessage: () => none,
    },
    onDidChangeVisibility: () => none,
  });
  return {
    states: () => posted.filter((message): message is SidebarState => (message as { type?: string }).type === "state"),
    pickerItems: async () => {
      const showQuickPick = vscode.window.showQuickPick;
      let items: Array<vscode.QuickPickItem & { id?: string }> = [];
      (vscode.window as any).showQuickPick = async (offered: typeof items) => {
        items = offered;
        return undefined;
      };
      try {
        await provider.onMessage({ type: "chooseTarget" });
      } finally {
        (vscode.window as any).showQuickPick = showQuickPick;
      }
      return items;
    },
    // The Target picker's choice, as the side panel's webview sends it.
    select: async (id: string) => provider.onMessage({ type: "select", id }),
    // The selected target's one action, as the side panel's button sends it.
    press: async () => provider.onMessage({ type: "action" }),
    dispose: () => subscriptions.forEach((subscription) => subscription.dispose()),
  };
}

/** Renders what Live Values posted in its real webview script. */
function renderLiveValues(posted: readonly unknown[]): { status: string; target: string; state: string } {
  const dom = new JSDOM(
    `<!doctype html><html><body>
      <span id="runtimeStatusText"></span><span id="targetLabel"></span>
      <div id="status"></div><div id="sections"></div>
    </body></html>`,
    { runScripts: "outside-only", url: "https://live-values.test/" },
  );
  try {
    dom.window.acquireVsCodeApi = () => ({ postMessage: () => undefined });
    dom.window.eval(fs.readFileSync(path.join(EXTENSION_ROOT, "src", "ioPanel.webview.js"), "utf8"));
    for (const data of posted) {
      dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data }));
    }
    const document = dom.window.document;
    return {
      status: document.getElementById("status").textContent,
      target: document.getElementById("targetLabel").textContent,
      state: document.getElementById("runtimeStatusText").textContent,
    };
  } finally {
    dom.window.close();
  }
}

function lastStatus(panel: CapturedPanel): string | undefined {
  const statuses = panel.posted.filter((message) => (message as { type?: string }).type === "status");
  const last = statuses[statuses.length - 1] as { payload?: unknown } | undefined;
  return typeof last?.payload === "string" ? last.payload : undefined;
}

suite("Connect and the run target (spec 25 section 5.1)", function () {
  this.timeout(120_000);

  let folder: vscode.WorkspaceFolder;
  let fake: FakeRuntime;
  let restoreProject: (() => void) | undefined;
  let baseline: { readonly settings: Record<string, unknown>; readonly selected: string };
  const cleanups: Array<() => unknown> = [];

  suiteSetup(async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "the tests need a workspace folder");
    folder = workspaceFolder;
    assert.strictEqual(
      getTrustConfiguration().get<string>("runtime.controlAuthToken", "").trim(),
      "",
      "a plaintext fallback token would supply the token Connect must find missing",
    );
    fake = await startFakeRuntime();
    restoreProject = writeRootProject(folder.uri.fsPath, fake.endpoint);
    const config = getTrustConfiguration(folder.uri);
    baseline = {
      settings: Object.fromEntries(RUNTIME_SETTINGS.map((key) => [key, config.inspect<unknown>(key)?.workspaceFolderValue])),
      selected: getSelectedRuntimeId(),
    };
  });

  setup(async () => {
    // The extension's own panels close, so Devices & Connections and Live Values open into the captured ones.
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  });

  teardown(async () => {
    for (const cleanup of cleanups.splice(0).reverse()) {
      await cleanup();
    }
    const config = getTrustConfiguration(folder.uri);
    for (const key of RUNTIME_SETTINGS) {
      await config.update(key, baseline.settings[key], vscode.ConfigurationTarget.WorkspaceFolder);
    }
    await setSelectedRuntimeId(baseline.selected);
    await setControlAuthToken(fake.endpoint, "");
    if (!runtimeLifecycleService.getStructuredTextSession()) {
      // Clears what the lifecycle reported about the fake runtime.
      await runtimeLifecycleService.stopRuntime();
    }
  });

  suiteTeardown(async () => {
    restoreProject?.();
    await fake?.close();
  });

  test("failed connect leaves the target, settings and status bar unchanged", async () => {
    // Catches a Connect that writes the endpoint and online mode before the runtime answers, so that after a refusal
    // the status bar says "<endpoint> connected" (acceptance finding F20).
    // The runtime needs a token and none is stored for its endpoint, so it refuses Connect.
    const before = await targetState(folder);
    assert.ok(
      !before.statusBar.includes(fake.hostPort) && !before.statusBar.includes(PROJECT_LABEL),
      `the status bar starts on another target: ${before.statusBar}`,
    );

    const result = await runtimeLifecycleService.connectRemote(fake.endpoint, "Cpu");
    if (result.ok) {
      assert.fail("a runtime that refuses the missing token must fail Connect");
    }
    assert.strictEqual(result.failure.message, MISSING_TOKEN);
    assert.deepStrictEqual(
      await targetState(folder),
      before,
      "a refused Connect changes neither the runtime settings, nor the selected target, nor the status bar",
    );
  });

  test("connect without a token keeps the node and offers the token action", async () => {
    // Catches a failed Connect that leaves the runtime's node unchanged ("Stopped") with its issue aimed at a node that
    // is not in the graph, so no token action appears and only the plaintext fallback setting helps (F14, F19).
    const capture = captureWebviewPanels();
    cleanups.push(() => capture.restore());
    const canvas = await openCanvas(capture, fake.endpoint);

    canvas.receive({ type: "runtimeConnect", endpoint: fake.endpoint, label: "Cpu" });
    const failed = await waitFor("the failed Connect in Devices & Connections", () =>
      postedGraphs(canvas).find((graph) => graph.faults.some((fault) => fault.label === MISSING_TOKEN)));
    const runtime = runtimeAt(failed, fake.endpoint);
    assert.ok(runtime, "the runtime's node stays in the graph after the failed Connect");
    assert.strictEqual(runtime.health, "error", "the node shows the failure");
    assert.strictEqual(runtime.detail, MISSING_TOKEN);
    assert.strictEqual(
      failed.faults.find((fault) => fault.label === MISSING_TOKEN)?.targetNodeId,
      runtime.id,
      "the issue focuses the runtime's node",
    );

    // The node's inspector offers the token action for this endpoint first, and Connect as the retry.
    const node = buildGraph(failed).nodes.find((candidate) => candidate.id === runtime.id);
    assert.ok(node, "the node is rendered");
    const inspector = await renderNodeInspector({ id: node.id, type: node.type, data: node.data as Record<string, unknown> });
    cleanups.push(() => inspector.close());
    assert.strictEqual(inspector.primary(), "Set auth token");
    await inspector.click("Set auth token");
    const setToken = inspector.posted[inspector.posted.length - 1];
    assert.deepStrictEqual(setToken, { type: "setRuntimeAuthToken", endpoint: fake.endpoint });

    // Storing a token through that action lets Connect be pressed again, and the runtime then answers.
    const showInputBox = vscode.window.showInputBox;
    (vscode.window as any).showInputBox = async () => TOKEN;
    cleanups.push(() => {
      (vscode.window as any).showInputBox = showInputBox;
    });
    canvas.receive(setToken);
    await waitFor("the token stored for the endpoint", async () => (await getControlAuthToken(fake.endpoint)) === TOKEN);
    await inspector.click("Connect");
    const connect = inspector.posted[inspector.posted.length - 1];
    assert.deepStrictEqual(connect, { type: "runtimeConnect", endpoint: fake.endpoint, label: "Cpu" });
    canvas.receive(connect);
    await waitFor("the runtime selected by the second Connect", () => getSelectedRuntimeId() === fake.endpoint);
  });

  test("successful connect selects the runtime in the side panel and the status bar", async () => {
    // Catches a Connect after which the side panel's Target still says Simulator and its picker lists only the
    // Simulator, while the status bar names the runtime (F20). The runtime is the project's own, so every surface names
    // it as that runtime, in one entry (F23).
    // As in the acceptance run: a token is stored for the runtime, which keeps debugging disabled.
    await setControlAuthToken(fake.endpoint, TOKEN);
    const sidebar = openSidebar();
    cleanups.push(() => sidebar.dispose());
    const capture = captureWebviewPanels();
    cleanups.push(() => capture.restore());
    const canvas = await openCanvas(capture, fake.endpoint);

    canvas.receive({ type: "runtimeConnect", endpoint: fake.endpoint, label: "Cpu" });
    await waitFor("the runtime selected by Connect", () => getSelectedRuntimeId() === fake.endpoint);

    const state = await waitFor("the side panel's Target on the runtime", () =>
      sidebar.states().reverse().find((candidate) => candidate.selectedId === PROJECT_ID));
    assert.strictEqual(state.selected.label, PROJECT_LABEL, "the side panel's Target names the runtime");
    assert.ok(
      state.options.some((option) => option.id === PROJECT_ID && option.kind === "remote"),
      "the runtime is one of the side panel's targets",
    );
    assert.ok(
      !state.options.some((option) => option.id === fake.endpoint),
      `the runtime has one entry: ${JSON.stringify(state.options)}`,
    );
    const items = await sidebar.pickerItems();
    assert.ok(
      items.some((item) => item.id === PROJECT_ID && item.label === PROJECT_LABEL && item.description === "selected"),
      `the Target picker lists the runtime as selected: ${JSON.stringify(items)}`,
    );
    const statusBar = (await statusText(STATUS_CONTEXT, await runtimeLifecycleService.snapshot())).text;
    assert.ok(statusBar.includes(PROJECT_LABEL), `the status bar names the same runtime: ${statusBar}`);
  });

  test("the project's own runtime is offered while it answers, and selecting it follows the project", async () => {
    // Catches a runtime started from the command line that the Target picker never offers (acceptance run of
    // 29 October: the picker listed only the Simulator), and a selection that writes the runtime's endpoint of today into
    // the settings, so that a runtime restarted on another port is lost (specification 25, section 5.1).
    const config = getTrustConfiguration(folder.uri);
    await config.update("runtime.controlEndpoint", "tcp://127.0.0.1:9", vscode.ConfigurationTarget.WorkspaceFolder);
    await config.update("runtime.mode", "online", vscode.ConfigurationTarget.WorkspaceFolder);
    const sidebar = openSidebar();
    cleanups.push(() => sidebar.dispose());
    const items = await waitFor("the project's runtime in the Target picker", async () => {
      const offered = await sidebar.pickerItems();
      return offered.some((item) => item.id === PROJECT_ID && item.label === PROJECT_LABEL) && offered;
    });
    assert.ok(
      !items.some((item) => item.id === PROJECT_ID && item.description === "selected"),
      "the project's runtime is offered, not yet selected",
    );

    await sidebar.select(PROJECT_ID);
    const state = await targetState(folder);
    assert.strictEqual(state.selected, PROJECT_ID);
    assert.strictEqual(state.settings["runtime.mode"], "online", "the runtime settings are online");
    assert.strictEqual(
      state.settings["runtime.controlEndpoint"],
      "",
      "the settings name no endpoint, so each connection asks trust-runtime for it",
    );
    const shown = await waitFor("the side panel's Target on the project's runtime", () =>
      sidebar.states().reverse().find((candidate) => candidate.selectedId === PROJECT_ID));
    assert.strictEqual(shown.selected.label, PROJECT_LABEL);
    assert.ok(
      !shown.options.some((option) => option.id === fake.endpoint),
      `the runtime has one entry: ${JSON.stringify(shown.options)}`,
    );
  });

  test("Connect on the project's own runtime connects with the endpoint and token trust-runtime reports", async () => {
    // Catches the side panel's Connect sending the target's id, "project", as the endpoint, so that Connect failed as not
    // reachable while the project's runtime answered (acceptance run of 30 October on the packaged 0.26.0 candidate;
    // specification 25, section 5.1). The fake runtime keeps debugging disabled, so a Connect that reaches it is
    // reported as debugging disabled for its endpoint.
    const config = getTrustConfiguration(folder.uri);
    await config.update("runtime.mode", "online", vscode.ConfigurationTarget.WorkspaceFolder);
    const sidebar = openSidebar();
    cleanups.push(() => sidebar.dispose());
    await waitFor("the project's runtime in the Target picker", async () =>
      (await sidebar.pickerItems()).some((item) => item.id === PROJECT_ID));
    await sidebar.select(PROJECT_ID);
    await sidebar.press();
    const failure = await waitFor("the project's runtime answering Connect", () =>
      runtimeLifecycleService.debugDisabledFailure(fake.endpoint));
    assert.strictEqual(failure.kind, "debug_disabled");
    assert.strictEqual(
      getTrustConfiguration(folder.uri).get<string>("runtime.controlEndpoint"),
      "",
      "the settings still name no endpoint, so the next Connect asks trust-runtime again",
    );
  });

  test("a project runtime that stops outside VS Code is not running in the side panel", async () => {
    // Catches a side panel that keeps a stopped runtime as answering until some other event refreshes it (acceptance run
    // of 30 October on the packaged 0.26.0 candidate: unchanged 30 s after the runtime stopped; specification 25, section
    // 5.1). A second fake runtime is the project's for this test and stops while the side panel shows it.
    const second = await startFakeRuntime();
    const restore = writeRootProject(folder.uri.fsPath, second.endpoint);
    cleanups.push(() => restore());
    cleanups.push(() => second.close().catch(() => undefined));
    const config = getTrustConfiguration(folder.uri);
    await config.update("runtime.mode", "online", vscode.ConfigurationTarget.WorkspaceFolder);
    await config.update("runtime.controlEndpoint", "", vscode.ConfigurationTarget.WorkspaceFolder);
    await setSelectedRuntimeId(PROJECT_ID);
    const sidebar = openSidebar();
    cleanups.push(() => sidebar.dispose());
    await waitFor("the project's runtime answering in the side panel", () =>
      sidebar.states().reverse().find((state) => state.selectedId === PROJECT_ID && state.selected.status === "disconnected"));

    await second.close();
    const stopped = await waitFor("the stopped runtime in the side panel", () =>
      sidebar.states().reverse().find((state) => state.selectedId === PROJECT_ID && state.selected.statusLabel === "Not running"));
    assert.strictEqual(stopped.selected.primary.enabled, false, "Connect waits for the runtime to run again");
  });

  test("the online watch tells the surfaces its first read, so a runtime that stopped before it is not shown answering", () => {
    // Catches a watch that keeps its first read after arming to itself: a project runtime that stopped between the side
    // panel's last read and that first read stayed shown as answering, and the test above waited in vain (CI of the
    // 0.28.0 release pull request, 1 November 2026, checklist W18; specification 25, section 5.1).
    const report = new OnlineStatusReport();
    const answering = { endpoint: "", endpointReachable: false, projectEndpoint: "tcp://127.0.0.1:1", projectReachable: true };
    const stopped = { ...answering, projectReachable: false };
    assert.strictEqual(report.changed(stopped), true, "the first read after arming is news");
    assert.strictEqual(report.changed(stopped), false, "the same status again is not");
    assert.strictEqual(report.changed(answering), true, "a runtime that answers again is");
    report.reset();
    assert.strictEqual(report.changed(answering), true, "after a rest the first read is news again");
  });

  test("a runtime no probe asked is not connected, not unreachable", async () => {
    // Catches the side panel calling a runtime not reachable, with Connect disabled, when Live Values did not probe it
    // because its endpoint is switched off, and an extension that does not pass on whether it probed (specification 25,
    // section 5.1). Nothing listens on port 9, so the probe fails while the endpoint is on.
    const config = getTrustConfiguration(folder.uri);
    const silent = "tcp://127.0.0.1:9";
    await config.update("runtime.controlEndpoint", silent, vscode.ConfigurationTarget.WorkspaceFolder);
    await config.update("runtime.controlEndpointEnabled", true, vscode.ConfigurationTarget.WorkspaceFolder);
    await config.update("runtime.mode", "online", vscode.ConfigurationTarget.WorkspaceFolder);
    await setSelectedRuntimeId(silent);
    const sidebar = openSidebar();
    cleanups.push(() => sidebar.dispose());
    const probed = await waitFor("the failed probe in the side panel", () =>
      sidebar.states().reverse().find((state) => state.selectedId === silent && state.selected.statusLabel === "Not reachable"));
    assert.strictEqual(probed.selected.primary.enabled, false, "Connect waits for a runtime that answers");

    await config.update("runtime.controlEndpointEnabled", false, vscode.ConfigurationTarget.WorkspaceFolder);
    const unprobed = await waitFor("the unprobed runtime in the side panel", () =>
      sidebar.states().reverse().find((state) => state.selectedId === silent && state.selected.statusLabel === "Not connected"));
    assert.strictEqual(unprobed.selected.primary.enabled, true, "Connect, which probes first, stays available");
  });

  test("selecting the Simulator after Connect puts every surface back on the Simulator", async () => {
    // Catches a selection that changes only the side panel: the runtime settings stay online, the status bar keeps
    // naming the runtime and HMI Live keeps operating it while the side panel shows the Simulator (found while fixing
    // F20; specification 25, section 5.1).
    await setControlAuthToken(fake.endpoint, TOKEN);
    const sidebar = openSidebar();
    cleanups.push(() => sidebar.dispose());
    const connected = await runtimeLifecycleService.connectRemote(fake.endpoint, "Cpu");
    assert.ok(connected.ok, "the runtime answers Connect");
    // As the node's Connect does after a successful Connect.
    await setSelectedRuntimeId(fake.endpoint);
    // The fake keeps debugging disabled, so Connect holds no session: the runtime answers online and is named by its
    // endpoint (specification 39, section 11 item 6).
    assert.deepStrictEqual(
      await resolveHmiRunTarget(await runtimeLifecycleService.snapshot()),
      { endpoint: fake.endpoint, label: fake.endpoint, simulator: false },
      "after Connect HMI Live operates the runtime",
    );

    await sidebar.select(SIMULATOR_RUNTIME_ID);
    const state = await targetState(folder);
    assert.strictEqual(state.selected, SIMULATOR_RUNTIME_ID);
    assert.strictEqual(state.settings["runtime.mode"], "simulate", "the runtime settings leave online mode");
    assert.ok(
      !state.statusBar.includes(fake.hostPort) && !state.statusBar.includes(PROJECT_LABEL),
      `the status bar no longer names the runtime: ${state.statusBar}`,
    );
    const snapshot = await runtimeLifecycleService.snapshot();
    assert.strictEqual(
      await resolveHmiRunTarget(snapshot),
      undefined,
      "HMI Live has no runtime to operate until the Simulator starts",
    );
    assert.strictEqual(snapshot.failure, undefined, "the runtime's disabled debugging is no reason of the Simulator's");
  });

  test("live values names disabled debugging on a connected runtime", async () => {
    // Catches Live Values asking to "Connect to the selected runtime" on a runtime that answered but keeps debugging
    // disabled, and a canvas issue that tells the user to open Devices & Connections from inside it (F21).
    await setControlAuthToken(fake.endpoint, TOKEN);
    const capture = captureWebviewPanels();
    cleanups.push(() => capture.restore());
    const canvas = await openCanvas(capture, fake.endpoint);
    canvas.receive({ type: "runtimeConnect", endpoint: fake.endpoint, label: "Cpu" });
    await waitFor("the runtime selected by Connect", () => getSelectedRuntimeId() === fake.endpoint);

    // Live Values says why the connected runtime shows no values, and which setting enables them.
    await vscode.commands.executeCommand("trust-lsp.debug.openIoPanel");
    const live = await waitFor("Live Values", () => capture.panels.find((panel) => panel.viewType === "trust-io-panel"));
    const reason = await waitFor("the reason in Live Values", () => {
      const status = lastStatus(live);
      return status && /debugging is disabled/i.test(status) ? status : undefined;
    });
    for (const part of ["debug_enabled", "[runtime.control]", "runtime.toml"]) {
      assert.ok(reason.includes(part), `Live Values names ${part}: ${reason}`);
    }
    // Live Values republishes its unavailable state at 100 and 500 ms; the reason must survive all of it.
    await new Promise((resolve) => setTimeout(resolve, 700));
    assert.strictEqual(lastStatus(live), reason, "no later message replaces the reason with a request to connect");
    const rendered = renderLiveValues(live.posted);
    assert.strictEqual(rendered.status, reason, "the panel shows the reason");
    assert.strictEqual(rendered.target, `Runtime at ${fake.hostPort}`, "beside the runtime it is about");
    // The re-run of 26 October: the status bar said "connected" while Live Values said "Not connected". Both name the
    // state instead (specification 25, section 4.4).
    assert.strictEqual(rendered.state, "Debugging disabled");
    const statusBar = (await statusText(STATUS_CONTEXT, await runtimeLifecycleService.snapshot())).text;
    assert.ok(statusBar.endsWith("· debugging disabled") && !/connected/.test(statusBar), `the status bar names the state: ${statusBar}`);

    // Devices & Connections gives the same reason without sending the user to itself.
    const issue = await waitFor("the reason in Devices & Connections", () =>
      postedGraphs(canvas).flatMap((graph) => graph.faults).find((fault) => fault.label === reason));
    assert.ok(!issue.label.includes("Devices & Connections"), `the issue does not point at its own view: ${issue.label}`);
  });
});
