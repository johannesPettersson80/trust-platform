export class TrustWasmAnalysisClient {
  constructor(options = {}) {
    this.defaultTimeoutMs = Number.isFinite(options.defaultTimeoutMs)
      ? options.defaultTimeoutMs
      : 10000;
    this.workerUrl = options.workerUrl || "/ide/wasm/worker.js";
    this.autoRestart = options.autoRestart !== false;
    this.maxRestarts = Number.isFinite(options.maxRestarts)
      ? Math.max(0, options.maxRestarts)
      : 3;
    this.restartDelayMs = Number.isFinite(options.restartDelayMs)
      ? Math.max(0, options.restartDelayMs)
      : 200;

    this.workerFactory =
      options.workerFactory ||
      (() => new Worker(this.workerUrl, { type: "module" }));

    this.worker = null;
    this.pending = new Map();
    this.requestSequence = 0;
    this.lastRequestId = null;
    this.statusListeners = new Set();
    this.restartCount = 0;
    this.disposed = false;

    this._resetReadyPromise();
    this._spawnWorker("initial");
  }

  _resetReadyPromise() {
    this.readySettled = false;
    this.readyPromise = new Promise((resolve, reject) => {
      this.resolveReady = resolve;
      this.rejectReady = reject;
    });
  }

  _spawnWorker(reason) {
    if (this.disposed) {
      return;
    }

    try {
      this.worker = this.workerFactory();
    } catch (error) {
      this._handleFatal(new Error(`worker_factory_failed: ${String(error)}`));
      return;
    }

    this.emitStatus({ type: "worker_spawned", reason, restartCount: this.restartCount });

    this.worker.addEventListener("message", (event) => {
      this.handleMessage(event.data);
    });

    this.worker.addEventListener("error", (event) => {
      const error = new Error(`worker crashed: ${event.message}`);
      this.emitStatus({ type: "worker_error", error: event.message });
      this.rejectAllPending(error);
      this._scheduleRestart(error);
    });
  }

  _terminateWorker() {
    if (!this.worker) {
      return;
    }
    try {
      this.worker.terminate();
    } catch {
      // Best effort.
    }
    this.worker = null;
  }

  _handleFatal(error) {
    this.rejectAllPending(error);
    if (!this.readySettled) {
      this.readySettled = true;
      this.rejectReady(error);
    }
    this.emitStatus({ type: "fatal", error: error.message });
  }

  _scheduleRestart(cause) {
    if (!this.autoRestart || this.disposed) {
      this._handleFatal(cause);
      return;
    }

    if (this.restartCount >= this.maxRestarts) {
      this._handleFatal(
        new Error(
          `worker restart budget exhausted (${this.maxRestarts}): ${cause.message}`,
        ),
      );
      return;
    }

    this.restartCount += 1;
    this._terminateWorker();
    this._resetReadyPromise();
    this.emitStatus({
      type: "restarting",
      reason: cause.message,
      restartCount: this.restartCount,
      maxRestarts: this.maxRestarts,
    });

    setTimeout(() => {
      if (this.disposed) {
        return;
      }
      this._spawnWorker("restart");
    }, this.restartDelayMs);
  }

  onStatus(listener) {
    if (typeof listener !== "function") {
      return () => {};
    }
    this.statusListeners.add(listener);
    return () => {
      this.statusListeners.delete(listener);
    };
  }

  emitStatus(status) {
    for (const listener of this.statusListeners) {
      try {
        listener(status);
      } catch {
        // Best-effort listener dispatch.
      }
    }
  }

  handleMessage(message) {
    if (!message || typeof message !== "object") {
      return;
    }

    if (message.type === "ready") {
      this.emitStatus({ type: "ready", restartCount: this.restartCount });
      if (!this.readySettled) {
        this.readySettled = true;
        this.resolveReady();
      }
      return;
    }

    if (message.type === "startup_error") {
      const error = new Error(String(message.error || "unknown startup error"));
      this.emitStatus({ type: "startup_error", error: error.message });
      this.rejectAllPending(error);
      this._scheduleRestart(error);
      return;
    }

    if (typeof message.id !== "string") {
      return;
    }

    const state = this.pending.get(message.id);
    if (!state) {
      return;
    }

    this.pending.delete(message.id);

    if (message.error) {
      const method = state.method || "request";
      const errorMessage = message.error.message || "unknown worker error";
      state.reject(new Error(`${method}: ${errorMessage}`));
      return;
    }

    state.resolve(message.result);
  }

  rejectAllPending(error) {
    for (const state of this.pending.values()) {
      state.reject(error);
    }
    this.pending.clear();
  }

  ready() {
    return this.readyPromise;
  }

  send(method, params = {}, timeoutMs = this.defaultTimeoutMs) {
    if (this.disposed) {
      return Promise.reject(new Error("client disposed"));
    }
    if (!this.worker) {
      return Promise.reject(new Error("worker unavailable"));
    }

    const id = `req-${++this.requestSequence}`;
    this.lastRequestId = id;

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, method });
      this.worker.postMessage({
        id,
        method,
        params,
        timeoutMs: Number.isFinite(timeoutMs) ? timeoutMs : this.defaultTimeoutMs,
      });
    });
  }

  cancel(requestId) {
    if (!requestId || typeof requestId !== "string" || !this.worker) {
      return;
    }
    this.worker.postMessage({ method: "cancel", params: { requestId } });
    const pending = this.pending.get(requestId);
    if (pending) {
      pending.reject(new Error(`request ${requestId} cancelled by client`));
      this.pending.delete(requestId);
    }
  }

  cancelLast() {
    if (!this.lastRequestId) {
      return;
    }
    this.cancel(this.lastRequestId);
  }

  applyDocuments(documents, timeoutMs = 15000) {
    return this.send("applyDocuments", { documents }, timeoutMs);
  }

  diagnostics(uri, timeoutMs = 12000) {
    return this.send("diagnostics", { uri }, timeoutMs);
  }

  hover(uri, position, timeoutMs = 10000) {
    return this.send("hover", { uri, position }, timeoutMs);
  }

  completion(uri, position, limit = 25, timeoutMs = 12000) {
    return this.send("completion", { uri, position, limit }, timeoutMs);
  }

  references(uri, position, includeDeclaration = true, timeoutMs = 12000) {
    return this.send("references", { uri, position, include_declaration: includeDeclaration }, timeoutMs);
  }

  definition(uri, position, timeoutMs = 12000) {
    return this.send("definition", { uri, position }, timeoutMs);
  }

  documentHighlight(uri, position, timeoutMs = 8000) {
    return this.send("documentHighlight", { uri, position }, timeoutMs);
  }

  rename(uri, position, newName, timeoutMs = 15000) {
    return this.send("rename", { uri, position, new_name: newName }, timeoutMs);
  }

  status(timeoutMs = 800) {
    return this.send("status", {}, timeoutMs);
  }

  dispose() {
    this.disposed = true;
    this.cancelLast();
    this.rejectAllPending(new Error("client disposed"));
    this._terminateWorker();
  }
}
