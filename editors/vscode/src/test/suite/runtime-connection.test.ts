import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import type { ProjectCommandRunner } from "../../projectControlEndpoint";
import { connectionFor } from "../../runtimeConnection";
import { resolveRuntimeTargetFromSettings } from "../../runtimeTarget";

// Spec 11 §6.8.0a: the surfaces that talk to the workspace's runtime (Live Values status, the
// runtime lifecycle, Devices & Connections) ask trust-runtime for its endpoint and token at each
// connection attempt. These tests prove the wiring only; the resolution itself is Rust's.

/** A runner that answers each `ctl endpoint --json` with the next reply and records the calls. */
function scriptedRunner(replies: unknown[]): {
  readonly run: ProjectCommandRunner;
  readonly calls: Array<{ bin: string; args: string[]; cwd: string }>;
} {
  const calls: Array<{ bin: string; args: string[]; cwd: string }> = [];
  const run: ProjectCommandRunner = async (bin, args, cwd) => {
    calls.push({ bin, args: [...args], cwd });
    const reply = replies[Math.min(calls.length - 1, replies.length - 1)];
    return JSON.stringify(reply);
  };
  return { run, calls };
}

suite("Runtime connection (spec 11 §6.8.0a)", () => {
  test("each connection attempt asks trust-runtime again and follows a restarted runtime", async () => {
    const { run, calls } = scriptedRunner([
      {
        endpoint: "tcp://127.0.0.1:50001",
        auth_token: "first-token",
        source: "endpoint_file",
      },
      {
        endpoint: "tcp://127.0.0.1:50002",
        auth_token: "second-token",
        source: "endpoint_file",
      },
    ]);

    const first = await connectionFor("", "/work/plc", { bin: "trust-runtime", run });
    const second = await connectionFor("", "/work/plc", { bin: "trust-runtime", run });

    assert.deepStrictEqual(first, {
      endpoint: "tcp://127.0.0.1:50001",
      authToken: "first-token",
    });
    assert.deepStrictEqual(second, {
      endpoint: "tcp://127.0.0.1:50002",
      authToken: "second-token",
    });
    assert.deepStrictEqual(
      calls.map((call) => call.args),
      [
        ["ctl", "--project", "/work/plc", "endpoint", "--json"],
        ["ctl", "--project", "/work/plc", "endpoint", "--json"],
      ]
    );
  });

  test("trust-runtime's reason is kept when the runtime is not running", async () => {
    const { run } = scriptedRunner([
      {
        error: {
          code: "not_running",
          message: "runtime is not running: no endpoint file at /work/plc/.trust-runtime/control.json",
        },
      },
    ]);

    const connection = await connectionFor("", "/work/plc", { bin: "trust-runtime", run });

    assert.deepStrictEqual(connection, {
      problem: {
        code: "not_running",
        message: "runtime is not running: no endpoint file at /work/plc/.trust-runtime/control.json",
      },
    });
  });

  test("an endpoint named in the settings is used as it is, without asking trust-runtime", async () => {
    const { run, calls } = scriptedRunner([]);

    const connection = await connectionFor("tcp://10.0.0.2:9901", "/work/plc", {
      bin: "trust-runtime",
      run,
    });

    assert.strictEqual(connection.endpoint, "tcp://10.0.0.2:9901");
    assert.strictEqual(calls.length, 0);
  });

  test("without a project folder there is nothing to ask", async () => {
    const { run, calls } = scriptedRunner([]);

    const connection = await connectionFor("", undefined, { bin: "trust-runtime", run });

    assert.deepStrictEqual(connection, {});
    assert.strictEqual(calls.length, 0);
  });

  test("a workspace runtime that is not running is unreachable, not a missing endpoint", async () => {
    const target = await resolveRuntimeTargetFromSettings({
      mode: "online",
      endpoint: "",
      endpointEnabled: true,
      unavailableReason: "runtime is not running: no endpoint file",
    });

    assert.strictEqual(target.status, "online_unreachable");
    assert.strictEqual(target.reachable, false);
  });

  test("the online surfaces resolve their endpoint through the runtime connection", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    for (const file of ["runtimeTarget.ts", "runtimeLifecycle.ts", "io-panel/status.ts"]) {
      const source = fs.readFileSync(path.join(extensionRoot, "src", file), "utf8");
      assert.ok(
        source.includes("await resolveRuntimeConnection("),
        `${file} must resolve the workspace runtime at each connection attempt`
      );
    }
  });
});
