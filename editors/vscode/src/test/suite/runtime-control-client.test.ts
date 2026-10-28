import * as assert from "assert";

import {
  probeRuntimeControlEndpoint,
  RuntimeControlError,
  RuntimeTransportError,
  sendRuntimeControlRequest,
} from "../../runtimeControlClient";
import { closedEndpoint, fakeRuntime, runtimeCommandsReady, type FakeRuntime } from "./fakeControlRuntime";

// The language server sends, reads and decodes every control request of the extension (`trust-lsp.runtimeRequest`,
// runtime protocol annex E0.1); its Rust tests prove the decoding. These tests prove the wiring to a runtime on a
// loopback port: what the extension asks reaches the runtime, and the runtime's answer comes back as a result or an
// error.

suite("Runtime control client", function () {
  this.timeout(60000);
  let runtime: FakeRuntime | undefined;

  suiteSetup(runtimeCommandsReady);

  teardown(async () => {
    await runtime?.close();
    runtime = undefined;
  });

  // Catches a request that does not reach the runtime, or reaches it without its token or its parameters.
  test("a request reaches the runtime with its type, token and parameters, and its result comes back", async () => {
    runtime = await fakeRuntime(() => ({ ok: true, result: { state: "running" } }));
    const result = await sendRuntimeControlRequest(runtime.endpoint, "auth-token", "status", { verbose: true });
    assert.deepStrictEqual(result, { state: "running" });
    assert.deepStrictEqual(
      runtime.received.map(({ type, auth, params }) => ({ type, auth, params })),
      [{ type: "status", auth: "auth-token", params: { verbose: true } }]
    );
  });

  // Catches a refusal that comes back as a result, or loses its code, the result it carries or the credential failure
  // its code names.
  test("a refusal comes back as an error with its code, its result and its credential failure", async () => {
    const recovery = { contract: "trust.plant", configuration_revision: "sha256:current" };
    runtime = await fakeRuntime((request) =>
      request.type === "plant.save"
        ? { ok: false, error: "saved configuration changed", error_code: "revision_conflict", result: recovery }
        : { ok: false, error: "invalid auth token", error_code: "invalid_auth_token" }
    );
    await assert.rejects(sendRuntimeControlRequest(runtime.endpoint, "token", "plant.save", {}), (error) => {
      assert.ok(error instanceof RuntimeControlError);
      assert.strictEqual(error.message, "revision_conflict: saved configuration changed");
      assert.strictEqual(error.code, "revision_conflict");
      assert.deepStrictEqual(error.result, recovery);
      assert.strictEqual(error.authFailure, undefined);
      return true;
    });
    await assert.rejects(sendRuntimeControlRequest(runtime.endpoint, "stale", "status"), (error) => {
      assert.ok(error instanceof RuntimeControlError);
      assert.strictEqual(error.code, "invalid_auth_token");
      assert.strictEqual(error.authFailure, "rejected");
      return true;
    });
  });

  // Catches a runtime that does not answer reported as a refusal or as an empty result.
  test("a runtime that does not answer is a transport error", async () => {
    await assert.rejects(
      sendRuntimeControlRequest(await closedEndpoint(), undefined, "status"),
      (error) => error instanceof RuntimeTransportError
    );
  });

  // Catches a probe that sends a request, and a closed endpoint reported reachable.
  test("a probe connects without sending a request", async () => {
    runtime = await fakeRuntime(() => ({ ok: true, result: null }));
    assert.strictEqual(await probeRuntimeControlEndpoint(runtime.endpoint), true);
    assert.deepStrictEqual(runtime.received, []);
    assert.strictEqual(await probeRuntimeControlEndpoint(await closedEndpoint()), false);
  });
});
