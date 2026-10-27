import * as assert from "assert";

import {
  parseProjectControlEndpoint,
  resolveProjectControlEndpoint,
  type ProjectCommandRunner,
} from "../../projectControlEndpoint";

suite("Project control endpoint (spec 11 §6.8.0a)", () => {
  test("asks trust-runtime for the project endpoint instead of reading its files", async () => {
    const calls: Array<{ bin: string; args: string[]; cwd: string }> = [];
    const run: ProjectCommandRunner = async (bin, args, cwd) => {
      calls.push({ bin, args: [...args], cwd });
      return JSON.stringify({
        endpoint: "unix:///work/plc/.trust-runtime/control.sock",
        auth_token: null,
        source: "default",
      });
    };

    const result = await resolveProjectControlEndpoint(
      "trust-runtime",
      "/work/plc",
      run
    );

    assert.deepStrictEqual(calls, [
      {
        bin: "trust-runtime",
        args: ["ctl", "--project", "/work/plc", "endpoint", "--json"],
        cwd: "/work/plc",
      },
    ]);
    assert.deepStrictEqual(result, {
      ok: true,
      value: {
        endpoint: "unix:///work/plc/.trust-runtime/control.sock",
        authToken: undefined,
        source: "default",
      },
    });
  });

  test("the endpoint file's generated token is returned with its source", () => {
    const result = parseProjectControlEndpoint(
      JSON.stringify({
        endpoint: "tcp://127.0.0.1:41234",
        auth_token: "generated-token",
        source: "endpoint_file",
      })
    );
    assert.deepStrictEqual(result, {
      ok: true,
      value: {
        endpoint: "tcp://127.0.0.1:41234",
        authToken: "generated-token",
        source: "endpoint_file",
      },
    });
  });

  test("a configured endpoint keeps its configured token", () => {
    const result = parseProjectControlEndpoint(
      JSON.stringify({
        endpoint: "tcp://127.0.0.1:5680",
        auth_token: "managed-secret",
        source: "configured",
      })
    );
    assert.deepStrictEqual(result, {
      ok: true,
      value: {
        endpoint: "tcp://127.0.0.1:5680",
        authToken: "managed-secret",
        source: "configured",
      },
    });
  });

  test("not_running keeps the runtime's message", () => {
    const result = parseProjectControlEndpoint(
      JSON.stringify({
        error: { code: "not_running", message: "The runtime is not running." },
      })
    );
    assert.deepStrictEqual(result, {
      ok: false,
      code: "not_running",
      message: "The runtime is not running.",
    });
  });

  test("every error object of trust-runtime keeps its code and message for display", () => {
    // Spec 11 §6.8.0a: `ctl endpoint --json` reports every failure as an error object.
    for (const code of [
      "no_project",
      "invalid_argument",
      "invalid_config",
      "endpoint_file_invalid",
      "endpoint_file_unreadable",
      "not_running",
      "default_unavailable",
    ]) {
      const message = `trust-runtime says ${code}`;
      const result = parseProjectControlEndpoint(
        JSON.stringify({ error: { code, message } })
      );
      assert.deepStrictEqual(result, { ok: false, code, message });
    }
  });

  test("a skipped endpoint file does not change the resolved endpoint", () => {
    const result = parseProjectControlEndpoint(
      JSON.stringify({
        endpoint: "tcp://127.0.0.1:5680",
        auth_token: "configured",
        source: "configured",
        endpoint_file_skipped: "read /plc/.trust-runtime/control.json: Permission denied",
      })
    );
    assert.deepStrictEqual(result, {
      ok: true,
      value: {
        endpoint: "tcp://127.0.0.1:5680",
        authToken: "configured",
        source: "configured",
      },
    });
  });

  test("missing, empty or non-JSON output is an invalid response", () => {
    for (const stdout of [
      undefined,
      "",
      "   ",
      "error: unrecognized subcommand 'endpoint'",
    ]) {
      const result = parseProjectControlEndpoint(stdout);
      assert.strictEqual(result.ok, false, `expected failure for ${JSON.stringify(stdout)}`);
      if (!result.ok) {
        assert.strictEqual(result.code, "invalid_response");
      }
    }
  });

  test("an answer without an endpoint or with an unknown source is rejected", () => {
    const missingEndpoint = parseProjectControlEndpoint(
      JSON.stringify({ source: "default" })
    );
    const unknownSource = parseProjectControlEndpoint(
      JSON.stringify({ endpoint: "unix:///x.sock", source: "guessed" })
    );
    assert.strictEqual(missingEndpoint.ok, false);
    assert.strictEqual(unknownSource.ok, false);
  });

  test("an empty token counts as no token", () => {
    const result = parseProjectControlEndpoint(
      JSON.stringify({
        endpoint: "tcp://127.0.0.1:9",
        auth_token: "",
        source: "configured",
      })
    );
    assert.deepStrictEqual(result, {
      ok: true,
      value: {
        endpoint: "tcp://127.0.0.1:9",
        authToken: undefined,
        source: "configured",
      },
    });
  });
});
