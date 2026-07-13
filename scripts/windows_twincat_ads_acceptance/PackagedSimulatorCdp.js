"use strict";

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function fetchJson(url, timeoutMs = 5_000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { signal: controller.signal });
    if (!response.ok) throw new Error(`CDP HTTP ${response.status}.`);
    return await response.json();
  } finally {
    clearTimeout(timer);
  }
}

async function waitForCdpJson(port, pathname, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      return await fetchJson(`http://127.0.0.1:${port}${pathname}`);
    } catch (error) {
      lastError = error;
      await sleep(150);
    }
  }
  throw lastError || new Error(`Timed out waiting for CDP ${pathname}.`);
}

async function connectCdp(port) {
  if (typeof WebSocket !== "function") {
    throw new Error("This VS Code Extension Host does not expose WebSocket for CDP acceptance.");
  }
  const version = await waitForCdpJson(port, "/json/version");
  return await new Promise((resolve, reject) => {
    const socket = new WebSocket(version.webSocketDebuggerUrl);
    let nextId = 0;
    const pending = new Map();
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (!message.id || !pending.has(message.id)) return;
      const callback = pending.get(message.id);
      pending.delete(message.id);
      callback(message);
    });
    socket.addEventListener("error", () => reject(new Error("CDP WebSocket failed.")));
    socket.addEventListener("open", () => {
      resolve({
        send(method, params = {}, sessionId) {
          return new Promise((done, fail) => {
            const id = ++nextId;
            const timer = setTimeout(() => {
              pending.delete(id);
              fail(new Error(`CDP timeout: ${method}`));
            }, 15_000);
            pending.set(id, (message) => {
              clearTimeout(timer);
              if (message.error) {
                fail(new Error(`${method} failed.`));
              } else {
                done(message);
              }
            });
            socket.send(JSON.stringify({ id, method, params, sessionId }));
          });
        },
        close() {
          socket.close();
        },
      });
    });
  });
}

module.exports = { connectCdp, waitForCdpJson };
