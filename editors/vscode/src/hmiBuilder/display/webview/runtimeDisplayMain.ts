// The runtime display's browser entry (specification 39, §11.1): starts the display on the
// `/hmi` page with the browser's fetch, websocket and timers, and the `TrustHmiDrawing` global
// from hmi-drawing.js. `npm run build:hmi-drawing` writes it to the runtime as hmi-display.js.

import { browserSounds } from "../alarmSound";
import { DisplayHost, DrawingApi, SocketLike, startDisplay } from "../runtimeDisplay";

declare const TrustHmiDrawing: DrawingApi;

const host: DisplayHost = {
  location: window.location,
  // A browser's choices, such as its language (§14.5); storage can be off, then nothing is kept.
  remember(key, value) {
    try {
      window.localStorage.setItem(key, value);
    } catch {
      // Private windows and blocked storage keep nothing; the display works without it.
    }
  },
  recall(key) {
    try {
      return window.localStorage.getItem(key) ?? undefined;
    } catch {
      return undefined;
    }
  },
  async fetchJson(url) {
    const response = await fetch(url, { cache: "no-store", headers: { Accept: "application/json" } });
    const body: unknown = await response.json().catch(() => null);
    return { status: response.status, body };
  },
  async postJson(url, body) {
    const response = await fetch(url, {
      method: "POST",
      cache: "no-store",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(body),
    });
    const answer: unknown = await response.json().catch(() => null);
    return { status: response.status, body: answer };
  },
  openSocket(url) {
    const socket = new WebSocket(url);
    const stream: SocketLike = { onmessage: null, onclose: null, close: () => socket.close() };
    socket.onmessage = (event) => stream.onmessage?.({ data: event.data });
    socket.onclose = () => stream.onclose?.();
    return stream;
  },
  now: () => Date.now(),
  schedule: (callback) => {
    window.requestAnimationFrame(() => callback());
  },
  later: (milliseconds, callback) => {
    window.setTimeout(callback, milliseconds);
  },
  reload: () => window.location.reload(),
  navigate: (url) => window.location.assign(url),
  sounds: browserSounds(window),
  download(name, text) {
    const url = URL.createObjectURL(new Blob([text], { type: "text/csv" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = name;
    document.body.append(link);
    link.click();
    link.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  },
};

function start(): void {
  const container = document.getElementById("hmi");
  if (container) void startDisplay(container, host, TrustHmiDrawing);
}

if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
else start();
