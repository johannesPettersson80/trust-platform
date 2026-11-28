import * as assert from "assert";
import * as vscode from "vscode";
import { attachedRuntimeOf } from "../../attachedRuntime";

// Specification 25 §6.8: the extension names an attach session's workspace folder and endpoint to the language server,
// and no token, so that inline values show the attached runtime's values (specification 14 §7.1.2).
suite("Attach session forwarded to the language server (specification 25 §6.8)", () => {
  const folder = { uri: vscode.Uri.file("/work/line"), name: "line", index: 0 };

  test("an attach session of truST is named by its folder and endpoint, never its token", () => {
    // Catches a forwarded token, which the language server finds itself, and a folder named by its URI string
    // instead of the path the server compares.
    const named = attachedRuntimeOf({
      type: "structured-text",
      configuration: {
        type: "structured-text",
        name: "Attach",
        request: "attach",
        endpoint: "unix:///work/line/.trust-runtime/control.sock",
        authToken: "a-secret-token",
      },
      workspaceFolder: folder,
    });
    assert.deepStrictEqual(named, {
      folder: vscode.Uri.file("/work/line").fsPath,
      endpoint: "unix:///work/line/.trust-runtime/control.sock",
    });
    assert.ok(!JSON.stringify(named).includes("a-secret-token"));
  });

  test("a launch, another debugger's session and an attach without an endpoint are not named", () => {
    // Catches a launch's simulator named as an attached runtime (the simulator record serves it), and a session of
    // another debugger forwarded.
    const session = (type: string, configuration: vscode.DebugConfiguration) => ({
      type,
      configuration,
      workspaceFolder: folder,
    });
    assert.strictEqual(
      attachedRuntimeOf(session("structured-text", { type: "structured-text", name: "Run", request: "launch" })),
      undefined
    );
    assert.strictEqual(
      attachedRuntimeOf(session("node", { type: "node", name: "Attach", request: "attach", endpoint: "tcp://x:1" })),
      undefined
    );
    assert.strictEqual(
      attachedRuntimeOf(session("structured-text", { type: "structured-text", name: "Attach", request: "attach" })),
      undefined
    );
  });
});
