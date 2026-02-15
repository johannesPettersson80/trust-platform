import * as assert from "assert";
import { EventEmitter } from "events";
import { RuntimeClient } from "../../statechart/runtimeClient";

class FakeSocket extends EventEmitter {
  public destroyed = false;

  write(_data: string, cb?: (err?: Error | null) => void): boolean {
    if (cb) {
      cb(null);
    }
    return true;
  }

  destroy(): void {
    this.destroyed = true;
  }
}

suite("RuntimeClient", function () {
  this.timeout(5000);

  test("cleans request listeners when a request times out", async () => {
    const client = new RuntimeClient({
      controlEndpoint: "unix:///tmp/test.sock",
      requestTimeoutMs: 25,
    });
    const socket = new FakeSocket();
    (client as any).socket = socket;

    await assert.rejects(client.readIo("%IX0.0"), /timed out/i);

    assert.strictEqual(socket.listenerCount("data"), 0);
    assert.strictEqual(socket.listenerCount("error"), 0);
    assert.strictEqual(socket.listenerCount("close"), 0);
  });
});
