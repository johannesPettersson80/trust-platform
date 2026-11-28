import * as assert from "assert";
import * as vscode from "vscode";
import { attachedSessionEnded, attachedSessionStarted, type AttachedRuntimeNote } from "../../attachedRuntime";

// Specification 25 §6.8: the extension names an attach session, its workspace folder and its endpoint to the language
// server when the session starts, and the session with `endpoint: null` when it ends; never a token (specification 14
// §7.1.2).
suite("Attach session forwarded to the language server (specification 25 §6.8)", () => {
  const folder = { uri: vscode.Uri.file("/work/line"), name: "line", index: 0 };
  const attach = {
    id: "session-7",
    type: "structured-text",
    configuration: {
      type: "structured-text",
      name: "Attach",
      request: "attach",
      endpoint: "unix:///work/line/.trust-runtime/control.sock",
      authToken: "a-secret-token",
    },
    workspaceFolder: folder,
  };

  test("an attach session's start and end are sent with its identity, folder and endpoint, never its token", () => {
    // Catches a forwarded token, which the language server finds itself, an end that does not clear the record, and a
    // note without the session, which would let one session's end clear another's record.
    const sent: AttachedRuntimeNote[] = [];
    attachedSessionStarted(attach, (note) => sent.push(note));
    attachedSessionEnded(attach, (note) => sent.push(note));
    const path = vscode.Uri.file("/work/line").fsPath;
    assert.deepStrictEqual(sent, [
      { session: "session-7", folder: path, endpoint: "unix:///work/line/.trust-runtime/control.sock" },
      { session: "session-7", folder: path, endpoint: null },
    ]);
    assert.ok(!JSON.stringify(sent).includes("a-secret-token"));
  });

  test("a launch, another debugger's session and an attach without an endpoint send nothing", () => {
    // Catches a launch's simulator named as an attached runtime (the simulator record serves it), and a session of
    // another debugger forwarded.
    const sent: AttachedRuntimeNote[] = [];
    const record = (note: AttachedRuntimeNote) => sent.push(note);
    const session = (type: string, configuration: vscode.DebugConfiguration) => ({
      id: "other",
      type,
      configuration,
      workspaceFolder: folder,
    });
    for (const other of [
      session("structured-text", { type: "structured-text", name: "Run", request: "launch" }),
      session("node", { type: "node", name: "Attach", request: "attach", endpoint: "tcp://127.0.0.1:1" }),
      session("structured-text", { type: "structured-text", name: "Attach", request: "attach" }),
    ]) {
      attachedSessionStarted(other, record);
      attachedSessionEnded(other, record);
    }
    assert.deepStrictEqual(sent, []);
  });
});
