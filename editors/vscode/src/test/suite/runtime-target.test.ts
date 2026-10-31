import * as assert from "assert";
import * as vscode from "vscode";

import { resolveRuntimeTargetFromSettings, RUNTIME_PANEL_COMMAND } from "../../runtimeTarget";
import { fakeRuntime, runtimeCommandsReady, type FakeRuntime } from "./fakeControlRuntime";

// The run target's status is the language server's answer (`trust-lsp.runTargets`, action `target`, specification 25
// §5.1); its Rust tests prove each status and credential channel. These tests prove the wiring: the settings reach the
// server, the server asks the runtime with the token, and the answer comes back.

suite("Runtime target", function () {
  this.timeout(60000);
  let runtime: FakeRuntime | undefined;

  suiteSetup(runtimeCommandsReady);

  teardown(async () => {
    await runtime?.close();
    runtime = undefined;
  });

  // Catches the configured endpoint contacted while the user simulates.
  test("the Simulator target asks no runtime", async () => {
    runtime = await fakeRuntime(() => ({ ok: true, result: { state: "running" } }));
    const target = await resolveRuntimeTargetFromSettings({ mode: "simulate", endpoint: runtime.endpoint });
    assert.strictEqual(target.status, "simulate");
    assert.strictEqual(target.reachable, false);
    assert.strictEqual(runtime.connections(), 0);
  });

  // Catches an online target named without asking the runtime, or asked without its token.
  test("an online target is asked for its status with its token", async () => {
    runtime = await fakeRuntime(() => ({ ok: true, result: { state: "running" } }));
    const target = await resolveRuntimeTargetFromSettings({
      mode: "online",
      endpoint: runtime.endpoint,
      authToken: "token",
    });
    assert.strictEqual(target.status, "online_reachable");
    assert.strictEqual(target.reachable, true);
    assert.strictEqual(target.endpoint, runtime.endpoint);
    assert.strictEqual(target.credentialChannel, "trusted_same_host");
    assert.strictEqual(target.authToken, "token");
    assert.deepStrictEqual(
      runtime.received.map(({ type, auth }) => ({ type, auth })),
      [{ type: "status", auth: "token" }]
    );
  });

  // Catches a refused credential shown as a runtime that does not answer.
  test("a runtime that refuses the credential is an authentication failure", async () => {
    runtime = await fakeRuntime(() => ({ ok: false, error: "missing auth token", error_code: "missing_auth_token" }));
    const target = await resolveRuntimeTargetFromSettings({ mode: "online", endpoint: runtime.endpoint });
    assert.strictEqual(target.status, "auth_failed");
    assert.strictEqual(target.authFailureKind, "missing");
    assert.strictEqual(target.reachable, true);
  });

  // Catches a status that reaches Live Values after it closed: the read ended after the panel was gone, and posting to it
  // threw an unhandled TypeError, "Cannot read properties of undefined (reading 'webview')" (release guard of 31 October
  // 2026). The panel opens and closes at once, so each read it starts ends after the panel is gone.
  test("Live Values closed while its status is read throws nothing", async () => {
    const rejections: unknown[] = [];
    const listener = (reason: unknown) => rejections.push(reason);
    process.on("unhandledRejection", listener);
    try {
      for (let round = 0; round < 5; round += 1) {
        await vscode.commands.executeCommand(RUNTIME_PANEL_COMMAND);
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      }
      await new Promise((resolve) => setTimeout(resolve, 3000));
    } finally {
      process.off("unhandledRejection", listener);
    }
    const fromPanel = rejections
      .map((reason) => (reason instanceof Error ? reason.stack ?? reason.message : String(reason)))
      .filter((text) => text.includes("sendRuntimeStatus"));
    assert.deepStrictEqual(fromPanel, []);
  });

  test("runtime pane command is the existing runtime panel command", () => {
    assert.strictEqual(RUNTIME_PANEL_COMMAND, "trust-lsp.debug.openIoPanel");
  });
});
