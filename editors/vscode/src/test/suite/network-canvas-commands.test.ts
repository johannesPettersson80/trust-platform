import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import {
  DEVICES_CANVAS_ACT_COMMAND,
  DEVICES_TOPOLOGY_COMMAND,
  presentCanvasReply,
  requestCanvasAct,
  requestCanvasTopology,
} from "../../networkCanvas/canvasCommands";
import { NetworkCanvasConfigurationActions } from "../../networkCanvas/configurationActions";
import { NetworkCanvasProtocolActions } from "../../networkCanvas/protocolActions";
import type { CommApplyResponse } from "../../communication/schemaForm";

// The canvas forwards each request to `trust-lsp.devicesTopology` or `trust-lsp.devicesCanvasAct`
// and shows the answer (specification 25 §5.15). The rules are the Rust tests'
// (crates/trust-devices/src/canvas/tests.rs); these tests prove that the requests reach the
// language server and that its answers reach the user and the webview.

async function languageServerReady(): Promise<void> {
  const deadline = Date.now() + 30000;
  for (;;) {
    const commands = await vscode.commands.getCommands(true);
    if (commands.includes(DEVICES_TOPOLOGY_COMMAND) && commands.includes(DEVICES_CANVAS_ACT_COMMAND)) {
      return;
    }
    if (Date.now() > deadline) assert.fail("the language server's devices commands are not available");
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

type WindowLike = Record<string, unknown>;

/** Replaces `vscode.window[name]` while `body` runs, recording each call. */
async function withMessages<T>(
  name: "showWarningMessage" | "showInformationMessage",
  answer: string | undefined,
  body: (calls: unknown[][]) => Promise<T>
): Promise<T> {
  const windowLike = vscode.window as unknown as WindowLike;
  const original = windowLike[name];
  const calls: unknown[][] = [];
  windowLike[name] = async (...args: unknown[]) => {
    calls.push(args);
    return answer;
  };
  try {
    return await body(calls);
  } finally {
    windowLike[name] = original;
  }
}

/** A webview panel stand-in that records what the canvas posts to it. */
function recordingPanel(): { panel: vscode.WebviewPanel; posted: unknown[] } {
  const posted: unknown[] = [];
  const panel = {
    visible: true,
    webview: {
      postMessage: async (message: unknown) => {
        posted.push(message);
        return true;
      },
    },
  } as unknown as vscode.WebviewPanel;
  return { panel, posted };
}

suite("Network Canvas — language server commands", function () {
  this.timeout(60000);
  suiteSetup(languageServerReady);

  test("a topology request reaches the server and its fleet view comes back", async () => {
    const answer = await requestCanvasTopology({
      peers: [{ endpoint: "tcp://10.0.0.9:5510", label: "cell1", reach: "unreachable" }],
      run_mode: "online",
      search_query: "",
      discovered: [],
    });
    assert.strictEqual(answer.topology_error, undefined);
    // Nothing reports the project's runtime, so the simulator stands for it first (§5.6).
    const peer = answer.fleet?.hosts.find((host) => host.hostname === "cell1");
    assert.strictEqual(answer.fleet?.hosts[0]?.hostname, "This computer");
    assert.strictEqual(peer?.runtimes[0]?.controlEndpoint, "tcp://10.0.0.9:5510");
    assert.deepStrictEqual(answer.discovered, []);
  });

  test("an action reaches the server, and a refused one warns with the server's reason", async () => {
    const port = await requestCanvasAct("ads_port", { text: " 301 " });
    assert.deepStrictEqual(port.result, { port: 301 });

    const unknown = await requestCanvasAct("format_disk", {});
    assert.strictEqual(unknown.applied, false);
    assert.strictEqual(unknown.notice?.tone, "warning");
    assert.match(unknown.notice?.text ?? "", /format_disk is not an action/);
  });

  test("a reply's notice is shown with its detail and actions, and the chosen action is answered", async () => {
    const chosen = await withMessages("showWarningMessage", "Start runtime", async (calls) => {
      const id = await presentCanvasReply({
        applied: false,
        refresh: false,
        notice: {
          tone: "warning",
          text: "Write-enabled ADS imports need a running runtime.",
          detail: "Start the runtime, then import again.",
          modal: true,
          actions: [{ id: "start_runtime", label: "Start runtime" }],
        },
      });
      assert.deepStrictEqual(calls, [
        [
          "Write-enabled ADS imports need a running runtime.",
          { modal: true, detail: "Start the runtime, then import again." },
          "Start runtime",
        ],
      ]);
      return id;
    });
    assert.strictEqual(chosen, "start_runtime");
  });

  test("a reply opens the generated documents it names so their diagnostics refresh", async () => {
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-canvas-reply-"));
    try {
      const generated = path.join(folder, "ads_generated.st");
      fs.writeFileSync(generated, "PROGRAM Generated\nEND_PROGRAM\n");
      await presentCanvasReply({ applied: true, refresh: true, open_documents: [generated] });
      assert.ok(
        vscode.workspace.textDocuments.some((document) => document.uri.fsPath === generated),
        "the generated document is open"
      );
    } finally {
      fs.rmSync(folder, { recursive: true, force: true });
    }
  });

  test("a form applied without a reachable runtime shows the server's blocked result", async () => {
    // The workspace of the tests runs the simulator, so no runtime target answers.
    const committed: Array<{ protocol: string; result: CommApplyResponse }> = [];
    let refreshed = 0;
    const actions = new NetworkCanvasConfigurationActions({
      schema: () => undefined,
      commit: (protocol, result) => committed.push({ protocol, result }),
      refresh: async () => {
        refreshed += 1;
      },
    });
    await actions.apply({ type: "commApplyLive", protocol: "Modbus-TCP", values: { address: "10.0.0.9:502" } });
    assert.strictEqual(committed.length, 1);
    assert.strictEqual(committed[0].protocol, "modbus_tcp");
    assert.strictEqual(committed[0].result.lifecycle_effect, "blocked");
    assert.strictEqual(
      committed[0].result.message,
      "Select a reachable online runtime before applying Communication setup."
    );
    assert.strictEqual(refreshed, 1, "the canvas reads its data again");
  });

  test("Identify of a runtime goes to the server, which sends nothing without a runtime that answers", async () => {
    // The request, the ten seconds and the notice texts are the Rust test
    // `identify_asks_the_runtime_for_ten_seconds_and_shows_its_answer`.
    const reply = await requestCanvasAct("identify", { name: "line" });
    assert.strictEqual(reply.notice?.tone, "warning");
    assert.strictEqual(reply.notice?.text, "line did not answer, so nothing was sent.");
  });

  test("a Save names the runtime it saves to, and the canvas selects the node the server answers", async () => {
    // Specification 25 §5.9; the node's identity is the Rust test
    // `a_saved_connection_is_selected_on_the_node_the_canvas_draws`.
    const commands = vscode.commands as unknown as Record<string, unknown>;
    const original = commands.executeCommand as (...args: unknown[]) => Thenable<unknown>;
    let sent: { op?: string; args?: { runtime_id?: string } } | undefined;
    commands.executeCommand = async (command: unknown, ...rest: unknown[]) => {
      if (command === DEVICES_CANVAS_ACT_COMMAND) {
        sent = rest[0] as typeof sent;
        return {
          applied: true,
          refresh: false,
          result: {
            schema_version: 1,
            protocol: "gpio",
            driver: "gpio",
            action: "upsert",
            applied: true,
            lifecycle_effect: "restart_required",
            message: "I/O configuration saved.",
            instance_id: "gpio:0",
            node_id: "trust-fleet-v1:saved",
          },
        };
      }
      return original.call(vscode.commands, command, ...rest);
    };
    try {
      const committed: CommApplyResponse[] = [];
      const actions = new NetworkCanvasConfigurationActions({
        schema: () => undefined,
        commit: (_protocol, result) => committed.push(result),
        refresh: async () => undefined,
      });
      await actions.save(
        { type: "commSave", protocol: "gpio", values: {}, runtimeId: "trust-fleet-v1:runtime" },
        "upsert"
      );
      assert.strictEqual(sent?.op, "comm_save");
      assert.strictEqual(sent?.args?.runtime_id, "trust-fleet-v1:runtime");
      assert.strictEqual(committed[0]?.node_id, "trust-fleet-v1:saved");
    } finally {
      commands.executeCommand = original;
    }
    const panel = fs.readFileSync(
      path.join(__dirname, "..", "..", "..", "src", "networkCanvas", "webview", "AddDevicePanel.tsx"),
      "utf8"
    );
    assert.ok(panel.includes("onSaved(applyResult.node_id)"), "the add form selects the server's node");
  });

  test("a typed ADS port is checked by the server and its answer goes back to the pane", async () => {
    const { panel, posted } = recordingPanel();
    const actions = new NetworkCanvasProtocolActions({
      panel: () => panel,
      extensionContext: () => undefined,
      topology: () => undefined,
      runtimeTarget: () => undefined,
      refresh: async () => undefined,
      startRuntime: async () => undefined,
    });
    await actions.checkAdsPort({ type: "checkAdsPort", requestId: 7, text: "65536" });
    await actions.checkAdsPort({ type: "checkAdsPort", requestId: 8, text: "" });
    assert.deepStrictEqual(posted, [
      {
        type: "adsPortChecked",
        requestId: 7,
        text: "65536",
        port: undefined,
        error: "ADS port must be between 1 and 65535.",
      },
      { type: "adsPortChecked", requestId: 8, text: "", port: 851, error: undefined },
    ]);
  });

  test("a tag removal's result goes back to the tag browser that asked for it", async () => {
    const { panel, posted } = recordingPanel();
    let refreshed = 0;
    const actions = new NetworkCanvasProtocolActions({
      panel: () => panel,
      extensionContext: () => undefined,
      topology: () => undefined,
      runtimeTarget: () => undefined,
      refresh: async () => {
        refreshed += 1;
      },
      startRuntime: async () => undefined,
    });
    await actions.removeAdsTag({
      type: "removeAdsTag",
      browseSessionId: "browse-1",
      adsImportRequestId: 3,
      target: { host: "192.168.77.11", target_net_id: "100.67.6.217.1.1" },
      selection: { port: 301, path: "Task 4.Inputs.Var 1" },
    });
    assert.strictEqual(posted.length, 1);
    const message = posted[0] as { type: string; browseSessionId: string; adsImportRequestId: number; result: { operation: string; summary: string } };
    assert.strictEqual(message.type, "adsTagsResult");
    assert.strictEqual(message.browseSessionId, "browse-1");
    assert.strictEqual(message.adsImportRequestId, 3);
    assert.strictEqual(message.result.operation, "remove");
    assert.strictEqual(typeof message.result.summary, "string");
    assert.ok(refreshed <= 1);
  });
});
