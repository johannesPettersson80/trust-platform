import * as assert from "assert";

import {
  redactDapMessage,
  stringifyDebugSession,
} from "../../debug/sessionLogging";

suite("Debug log credential redaction", () => {
  test("redacts the runtime control token while retaining useful session configuration", () => {
    const launchSecret = "raw-session-control-token-must-not-leak";
    const attachSecret = "raw-attach-token-must-not-leak";
    const formatted = stringifyDebugSession({
      configuration: {
        type: "structured-text",
        request: "launch",
        name: "truST Simulator",
        program: "C:\\projects\\demo\\src\\config.st",
        controlEndpoint: "tcp://127.0.0.1:9902",
        controlAuthToken: launchSecret,
        authToken: attachSecret,
      },
    });
    const parsed = JSON.parse(formatted) as Record<string, unknown>;

    assert.ok(!formatted.includes(launchSecret));
    assert.ok(!formatted.includes(attachSecret));
    assert.strictEqual(parsed.controlAuthToken, "***");
    assert.strictEqual(parsed.authToken, "***");
    assert.strictEqual(parsed.request, "launch");
    assert.strictEqual(parsed.name, "truST Simulator");
    assert.strictEqual(parsed.program, "C:\\projects\\demo\\src\\config.st");
    assert.strictEqual(parsed.controlEndpoint, "tcp://127.0.0.1:9902");
  });

  test("redacts both launch and attach credentials in DAP traffic", () => {
    const launchSecret = "dap-launch-secret";
    const attachSecret = "dap-attach-secret";
    const formatted = JSON.stringify(
      redactDapMessage({
        type: "request",
        command: "launch",
        arguments: {
          request: "launch",
          program: "C:\\projects\\demo\\src\\config.st",
          controlAuthToken: launchSecret,
          authToken: attachSecret,
          configuration: {
            authToken: "nested-reverse-dap-secret",
            nested: [
              { control_auth_token: "nested-snake-control-secret" },
              { auth_token: "nested-snake-auth-secret" },
            ],
          },
        },
      })
    );
    const parsed = JSON.parse(formatted) as {
      arguments: Record<string, unknown> & {
        configuration: {
          authToken: string;
          nested: Array<Record<string, unknown>>;
        };
      };
    };

    assert.ok(!formatted.includes(launchSecret));
    assert.ok(!formatted.includes(attachSecret));
    assert.ok(!formatted.includes("nested-reverse-dap-secret"));
    assert.ok(!formatted.includes("nested-snake-control-secret"));
    assert.ok(!formatted.includes("nested-snake-auth-secret"));
    assert.strictEqual(parsed.arguments.controlAuthToken, "***");
    assert.strictEqual(parsed.arguments.authToken, "***");
    assert.strictEqual(parsed.arguments.configuration.authToken, "***");
    assert.strictEqual(
      parsed.arguments.configuration.nested[0].control_auth_token,
      "***"
    );
    assert.strictEqual(
      parsed.arguments.configuration.nested[1].auth_token,
      "***"
    );
    assert.strictEqual(parsed.arguments.request, "launch");
  });
});
