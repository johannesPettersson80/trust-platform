import * as assert from "assert";
import { dapMessageLine, debugSessionLine } from "../../debug";

// Checklist G346: the "truST Debug" output channel is pasted into issues, so its lines name a session or a Debug Adapter
// Protocol message and never hold a configuration or a message's arguments or body, where a launch's
// `controlAuthToken` and an attach's `authToken` are.
suite("Debug channel lines hold no token (checklist G346)", () => {
  const token = "a-control-token-0123456789";

  test("an attach request and a launch request are named by their command, without their arguments", () => {
    // Catches the attach's token logged with the request's arguments, which every attach session wrote before, and a
    // launch's token, which the old redaction alone caught.
    const attach = dapMessageLine({
      seq: 2,
      type: "request",
      command: "attach",
      arguments: { endpoint: "unix:///work/line/.trust-runtime/control.sock", authToken: token },
    });
    assert.strictEqual(attach, "request attach seq=2");
    const launch = dapMessageLine({
      seq: 3,
      type: "request",
      command: "launch",
      arguments: { program: "/work/line/src/config.st", controlAuthToken: token },
    });
    assert.strictEqual(launch, "request launch seq=3");
  });

  test("a failed response keeps its error text, and an event drops its body", () => {
    // Catches a channel that loses the reason of a failed launch, the line a person reads first, and an event's body,
    // such as an output event's text, logged in full.
    assert.strictEqual(
      dapMessageLine({ seq: 7, type: "response", command: "launch", success: false, message: "cannot bind tcp://127.0.0.1:20444" }),
      'response launch seq=7 success=false message="cannot bind tcp://127.0.0.1:20444"'
    );
    assert.strictEqual(
      dapMessageLine({ seq: 9, type: "event", event: "output", body: { output: token } }),
      "event output seq=9"
    );
  });

  test("a session's line names it, its type and its request, without its configuration", () => {
    // Catches the session's configuration logged when it starts or ends, with the attach's token in it.
    const line = debugSessionLine("started", {
      name: "Attach to runtime",
      type: "structured-text",
      configuration: { type: "structured-text", name: "Attach to runtime", request: "attach", authToken: token },
    });
    assert.strictEqual(line, "Debug session started: Attach to runtime type=structured-text request=attach");
  });
});
