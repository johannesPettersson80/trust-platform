import * as assert from "assert";

import { RuntimeControlError } from "../../runtimeControlClient";
import {
  classifyRuntimeCredentialChannel,
  resolveRuntimeTargetFromSettings,
  RUNTIME_PANEL_COMMAND,
} from "../../runtimeTarget";

suite("Runtime target", function () {
  test("classifies simulate mode without probing endpoint", async () => {
    let probes = 0;
    const target = await resolveRuntimeTargetFromSettings(
      {
        mode: "simulate",
        endpoint: "tcp://192.168.10.20:9901",
      },
      {
        probeEndpoint: async () => {
          probes += 1;
          return true;
        },
      }
    );

    assert.strictEqual(probes, 0);
    assert.strictEqual(target.status, "simulate");
    assert.strictEqual(target.reachable, false);
    assert.strictEqual(target.credentialChannel, "untrusted_remote_plain_tcp");
  });

  test("reports missing endpoint when online runtime has no active endpoint", async () => {
    const target = await resolveRuntimeTargetFromSettings({
      mode: "online",
      endpoint: "",
      endpointEnabled: true,
    });

    assert.strictEqual(target.status, "missing_endpoint");
    assert.strictEqual(target.reachable, false);
    assert.strictEqual(target.credentialChannel, "unavailable");
  });

  test("reports missing endpoint when configured endpoint is disabled", async () => {
    const target = await resolveRuntimeTargetFromSettings({
      mode: "online",
      endpoint: "tcp://127.0.0.1:9901",
      endpointEnabled: false,
    });

    assert.strictEqual(target.status, "missing_endpoint");
    assert.strictEqual(target.endpoint, "tcp://127.0.0.1:9901");
    assert.strictEqual(target.credentialChannel, "unavailable");
  });

  test("reports online reachable when probe and status succeed", async () => {
    const target = await resolveRuntimeTargetFromSettings(
      {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
        authToken: "token",
      },
      {
        probeEndpoint: async () => true,
        requestStatus: async (_endpoint, authToken) => {
          assert.strictEqual(authToken, "token");
          return { state: "running" };
        },
      }
    );

    assert.strictEqual(target.status, "online_reachable");
    assert.strictEqual(target.reachable, true);
    assert.strictEqual(target.credentialChannel, "trusted_same_host");
  });

  test("reports online unreachable when endpoint probe fails", async () => {
    const target = await resolveRuntimeTargetFromSettings(
      {
        mode: "online",
        endpoint: "tcp://192.168.10.20:9901",
      },
      {
        probeEndpoint: async () => false,
      }
    );

    assert.strictEqual(target.status, "online_unreachable");
    assert.strictEqual(target.reachable, false);
  });

  test("reports online unreachable when status request fails after probe", async () => {
    const target = await resolveRuntimeTargetFromSettings(
      {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
      },
      {
        probeEndpoint: async () => true,
        requestStatus: async () => {
          throw new Error("control request timeout");
        },
      }
    );

    assert.strictEqual(target.status, "online_unreachable");
    assert.strictEqual(target.reachable, false);
  });

  test("reports auth failed when status request is rejected by auth", async () => {
    const target = await resolveRuntimeTargetFromSettings(
      {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
      },
      {
        probeEndpoint: async () => true,
        requestStatus: async () => {
          throw new RuntimeControlError("invalid auth token", "invalid_auth_token");
        },
      }
    );

    assert.strictEqual(target.status, "auth_failed");
    assert.strictEqual(target.authFailureKind, "rejected");
    assert.strictEqual(target.reachable, true);
  });

  test("reports missing auth token as a distinct auth failure", async () => {
    const target = await resolveRuntimeTargetFromSettings(
      {
        mode: "online",
        endpoint: "tcp://127.0.0.1:9901",
      },
      {
        probeEndpoint: async () => true,
        requestStatus: async () => {
          throw new RuntimeControlError("missing auth token", "missing_auth_token");
        },
      }
    );

    assert.strictEqual(target.status, "auth_failed");
    assert.strictEqual(target.authFailureKind, "missing");
    assert.strictEqual(target.reachable, true);
  });

  test("classifies credential forwarding trust from endpoint shape", () => {
    assert.strictEqual(
      classifyRuntimeCredentialChannel("unix:///tmp/trust.sock"),
      process.platform === "win32" ? "unavailable" : "trusted_same_host"
    );
    assert.strictEqual(
      classifyRuntimeCredentialChannel("tcp://localhost:9901"),
      "trusted_same_host"
    );
    assert.strictEqual(
      classifyRuntimeCredentialChannel("tcp://127.0.0.1:9901"),
      "trusted_same_host"
    );
    assert.strictEqual(
      classifyRuntimeCredentialChannel("tcp://192.168.10.20:9901"),
      "untrusted_remote_plain_tcp"
    );
    assert.strictEqual(classifyRuntimeCredentialChannel(""), "unavailable");
  });

  test("runtime pane command is the existing runtime panel command", () => {
    assert.strictEqual(RUNTIME_PANEL_COMMAND, "trust-lsp.debug.openIoPanel");
  });
});
