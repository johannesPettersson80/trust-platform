import * as assert from "assert";
import { EventEmitter } from "events";

import {
  isRuntimeControlAuthError,
  probeRuntimeControlEndpoint,
  RuntimeControlError,
  type RuntimeControlSocket,
  sendRuntimeControlRequest,
} from "../../runtimeControlClient";

type RuntimeRequest = {
  id: number;
  type: string;
  auth?: string;
  params?: unknown;
};

class FakeRuntimeControlSocket
  extends EventEmitter
  implements RuntimeControlSocket
{
  public destroyed = false;
  public writes: string[] = [];
  private timeoutHandle: NodeJS.Timeout | undefined;

  constructor(
    private readonly responder?: (request: RuntimeRequest) => unknown
  ) {
    super();
    setImmediate(() => this.emit("connect"));
  }

  setTimeout(timeout: number, callback?: () => void): this {
    if (callback) {
      this.timeoutHandle = setTimeout(callback, timeout);
    }
    return this;
  }

  write(data: string, callback?: (error?: Error | null) => void): boolean {
    this.writes.push(data);
    callback?.(null);
    if (this.responder) {
      const request = JSON.parse(data) as RuntimeRequest;
      const response = this.responder(request);
      if (response !== undefined) {
        setImmediate(() => {
          this.emit("data", Buffer.from(`${JSON.stringify(response)}\n`));
        });
      }
    }
    return true;
  }

  destroy(): void {
    this.destroyed = true;
    if (this.timeoutHandle) {
      clearTimeout(this.timeoutHandle);
      this.timeoutHandle = undefined;
    }
  }
}

suite("Runtime control client", function () {
  test("sends one JSON-line request and resolves successful responses", async () => {
    const socket = new FakeRuntimeControlSocket((request) => ({
      id: request.id,
      ok: true,
      result: { state: "running" },
    }));

    const result = await sendRuntimeControlRequest(
      "tcp://127.0.0.1:9901",
      "auth-token",
      "status",
      { verbose: true },
      { socketFactory: () => socket, timeoutMs: 100 }
    );

    assert.deepStrictEqual(result, { state: "running" });
    assert.strictEqual(socket.destroyed, true);
    assert.strictEqual(socket.writes.length, 1);
    const request = JSON.parse(socket.writes[0]) as RuntimeRequest;
    assert.strictEqual(request.type, "status");
    assert.strictEqual(request.auth, "auth-token");
    assert.deepStrictEqual(request.params, { verbose: true });
  });

  test("classifies auth failures from control responses", async () => {
    const socket = new FakeRuntimeControlSocket((request) => ({
      id: request.id,
      ok: false,
      code: "AUTH_FAILED",
      error: "invalid control token",
    }));

    await assert.rejects(
      sendRuntimeControlRequest("tcp://127.0.0.1:9901", undefined, "status", undefined, {
        socketFactory: () => socket,
        timeoutMs: 100,
      }),
      (error) => {
        assert.ok(error instanceof RuntimeControlError);
        assert.strictEqual(isRuntimeControlAuthError(error), true);
        return true;
      }
    );
  });

  test("probes endpoint reachability without sending a request", async () => {
    const socket = new FakeRuntimeControlSocket();

    const reachable = await probeRuntimeControlEndpoint("tcp://127.0.0.1:9901", {
      socketFactory: () => socket,
      timeoutMs: 100,
    });

    assert.strictEqual(reachable, true);
    assert.strictEqual(socket.destroyed, true);
    assert.deepStrictEqual(socket.writes, []);
  });
});
