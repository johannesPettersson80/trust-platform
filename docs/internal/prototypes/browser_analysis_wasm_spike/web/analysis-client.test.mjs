import test from "node:test";
import assert from "node:assert/strict";
import { TrustWasmAnalysisClient } from "./analysis-client.js";

class FakeWorker {
  constructor(name) {
    this.name = name;
    this.listeners = new Map();
    this.sent = [];
    this.terminated = false;
  }

  addEventListener(type, listener) {
    if (!this.listeners.has(type)) {
      this.listeners.set(type, []);
    }
    this.listeners.get(type).push(listener);
  }

  postMessage(payload) {
    this.sent.push(payload);
  }

  terminate() {
    this.terminated = true;
  }

  emitMessage(data) {
    const listeners = this.listeners.get("message") || [];
    for (const listener of listeners) {
      listener({ data });
    }
  }

  emitError(message) {
    const listeners = this.listeners.get("error") || [];
    for (const listener of listeners) {
      listener({ message });
    }
  }
}

function tick(ms = 0) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

test("analysis client bootstraps worker so ready resolves without external request", async () => {
  const worker = new FakeWorker("bootstrap");
  const client = new TrustWasmAnalysisClient({
    workerFactory: () => worker,
    autoRestart: false,
  });

  // Before the fix, no message was sent on startup, so ready() never resolved
  // unless another API call triggered the first worker request.
  const readyOrTimeout = Promise.race([
    client.ready().then(() => "ready"),
    tick(50).then(() => "timeout"),
  ]);

  await tick(0);
  assert.equal(worker.sent.length, 1, "expected startup bootstrap request");
  assert.equal(worker.sent[0].method, "status");
  worker.emitMessage({ type: "ready" });

  assert.equal(await readyOrTimeout, "ready");
  client.dispose();
});

test("analysis client sends request and resolves response", async () => {
  const worker = new FakeWorker("w1");
  const client = new TrustWasmAnalysisClient({
    workerFactory: () => worker,
    autoRestart: false,
  });

  worker.emitMessage({ type: "ready" });
  await client.ready();

  const pending = client.status();
  assert.equal(worker.sent.length, 2);
  const request = worker.sent[1];
  assert.equal(request.method, "status");

  worker.emitMessage({ id: request.id, result: { document_count: 1, uris: ["memory:///main.st"] } });
  const result = await pending;
  assert.equal(result.document_count, 1);

  client.dispose();
});

test("analysis client restarts worker after startup error", async () => {
  const workers = [new FakeWorker("w1"), new FakeWorker("w2")];
  let index = 0;
  const statusEvents = [];

  const client = new TrustWasmAnalysisClient({
    workerFactory: () => workers[index++],
    autoRestart: true,
    maxRestarts: 2,
    restartDelayMs: 0,
  });
  client.onStatus((status) => statusEvents.push(status.type));

  workers[0].emitMessage({ type: "startup_error", error: "boom" });
  await tick(0);

  workers[1].emitMessage({ type: "ready" });
  await client.ready();

  assert.ok(statusEvents.includes("startup_error"));
  assert.ok(statusEvents.includes("restarting"));
  assert.ok(statusEvents.includes("ready"));

  client.dispose();
});

test("analysis client rejects in-flight request on worker crash", async () => {
  const workers = [new FakeWorker("w1"), new FakeWorker("w2")];
  let index = 0;

  const client = new TrustWasmAnalysisClient({
    workerFactory: () => workers[index++],
    autoRestart: true,
    maxRestarts: 2,
    restartDelayMs: 0,
  });

  workers[0].emitMessage({ type: "ready" });
  await client.ready();

  const pending = client.diagnostics("memory:///main.st");
  workers[0].emitError("simulated crash");

  await assert.rejects(pending, /simulated crash/i);

  await tick(0);
  workers[1].emitMessage({ type: "ready" });
  await client.ready();

  client.dispose();
});
