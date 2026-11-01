// The workbench of the VS Code under test, reached over the Chrome DevTools Protocol: its page and webviews, screenshots of
// the rendered window, clicks and reads inside a webview's document, the quick input, notifications and the status bar
// (specification 22, "Acceptance journeys on the packaged extension"). The connection primitives are those of the extension's
// rendered tests.
import * as fs from "fs";
import * as path from "path";

import {
  CdpConnection,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  sleep,
} from "../../test/suite/plant-rendered-harness";

export { sleep };

/** The CDP connection of a session and its workbench page. */
export type Workbench = { port: number; connection: CdpConnection; pageSession: string };

/** A quick pick or input box while it is open. */
export type QuickInput = { title: string; placeholder: string; message: string; items: string[] };

type Target = { id: string; type: string; url?: string };

const isWebview = (target: Target) => /index\.html/.test(target.url ?? "");

/** Reads `read` until `accept` holds or `timeoutMs` pass; a read that fails counts as not yet, so a webview still loading is
 * read again. */
export async function pollValue<T>(
  label: string,
  read: () => Promise<T>,
  accept: (value: T) => boolean,
  timeoutMs: number
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let last = "";
  while (Date.now() < deadline) {
    try {
      const value = await read();
      if (accept(value)) return value;
      last = String(value);
    } catch (error) {
      last = `error: ${(error as Error).message}`;
    }
    await sleep(200);
  }
  throw new Error(`Timed out waiting for ${label}: ${last.slice(0, 300)}`);
}

/** Connects to the window whose profile is `userDataDir` and attaches to its workbench page. */
export async function workbench(userDataDir: string): Promise<Workbench> {
  const port = await devToolsPort(userDataDir, 20_000);
  const version = await httpJson(port, "/json/version");
  // A screenshot of the whole window under a software renderer can take longer than the rendered tests' requests.
  const connection = await connectCdp(version.webSocketDebuggerUrl, 10_000);
  const targets: Target[] = await httpJson(port, "/json");
  const page = targets.find((target) => target.type === "page" && !isWebview(target) && /vscode-file|workbench/i.test(target.url ?? ""));
  if (!page) throw new Error("no workbench page target");
  const attached = await connection.send("Target.attachToTarget", { targetId: page.id, flatten: true });
  await connection.send("Runtime.enable", {}, attached.sessionId);
  return { port, connection, pageSession: attached.sessionId };
}

/** Evaluates `expression` in the workbench page and returns its value. */
export async function evalPage<T = unknown>(wb: Workbench, expression: string): Promise<T> {
  const result = await wb.connection.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, wb.pageSession);
  if (result.exceptionDetails) throw new Error(`page eval: ${JSON.stringify(result.exceptionDetails).slice(0, 300)}`);
  return result.result?.value as T;
}

/** Runs `body`, which sees the webview's own document as `d`, in the webview session `sessionId`. */
export function evalWebview(wb: Workbench, sessionId: string, body: string): Promise<string> {
  return evaluateInWebview(wb.connection, sessionId, body);
}

/** Attaches to each webview target that `select` keeps, in turn, until `found` holds for one; gives its session. */
async function attachFirst(
  wb: Workbench,
  label: string,
  select: (target: Target) => boolean,
  found: string,
  timeoutMs: number
): Promise<string> {
  return pollValue(label, async () => {
    const targets: Target[] = await httpJson(wb.port, "/json");
    for (const target of targets.filter((entry) => isWebview(entry) && select(entry))) {
      let sessionId: string | undefined;
      try {
        const attached = await wb.connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        sessionId = attached.sessionId as string;
        await wb.connection.send("Runtime.enable", {}, sessionId);
        if ((await evalWebview(wb, sessionId, `return String(Boolean(${found}));`)) === "true") return sessionId;
        await wb.connection.send("Target.detachFromTarget", { sessionId });
      } catch {
        if (sessionId) await wb.connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
    }
    return "";
  }, (sessionId) => sessionId.length > 0, timeoutMs);
}

/** Attaches to the webview whose document answers `marker`, a JavaScript condition over `d`. */
export function attachWebview(wb: Workbench, label: string, marker: string, timeoutMs = 30_000): Promise<string> {
  return attachFirst(wb, `${label} webview`, () => true, marker, timeoutMs);
}

/** The ids of the webview targets now open. */
export async function webviewTargetIds(wb: Workbench): Promise<Set<string>> {
  const targets: Target[] = await httpJson(wb.port, "/json");
  return new Set(targets.filter(isWebview).map((target) => target.id));
}

/** Attaches to a webview that was not open `before` and whose document shows text. */
export function attachNewWebview(wb: Workbench, label: string, before: Set<string>, timeoutMs = 30_000): Promise<string> {
  return attachFirst(wb, `${label} new webview`, (target) => !before.has(target.id),
    "d.body && d.body.innerText.trim().length > 0", timeoutMs);
}

/** Saves a screenshot of the rendered workbench as `file`. */
export async function screenshot(wb: Workbench, file: string): Promise<string> {
  const metrics = await wb.connection.send("Page.getLayoutMetrics", {}, wb.pageSession);
  const view = metrics.cssVisualViewport;
  const zoom = view.zoom || 1;
  const result = await wb.connection.send("Page.captureScreenshot", {
    format: "png",
    fromSurface: true,
    clip: {
      x: view.pageX * zoom,
      y: view.pageY * zoom,
      width: Math.round(view.clientWidth * zoom),
      height: Math.round(view.clientHeight * zoom),
      scale: 1,
    },
    captureBeyondViewport: false,
  }, wb.pageSession);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, Buffer.from(result.data, "base64"));
  return file;
}

/** The visible texts of a webview, as JSON: headings, buttons with their disabled state, inputs, alerts and the body. */
export function describeWebview(wb: Workbench, sessionId: string): Promise<string> {
  return evalWebview(wb, sessionId, [
    "var text = function (el) { return (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim(); };",
    "var visible = function (el) { return el.offsetParent !== null; };",
    "var out = {",
    "  title: d.title,",
    "  headings: [...d.querySelectorAll('h1,h2,h3,[role=heading]')].filter(visible).map(text).filter(Boolean).slice(0, 30),",
    "  buttons: [...d.querySelectorAll('button,[role=button]')].filter(visible).map(function (b) { return (b.disabled ? '(disabled) ' : '') + (text(b) || b.getAttribute('aria-label') || b.title || '?'); }).slice(0, 80),",
    "  inputs: [...d.querySelectorAll('input,select,textarea')].filter(visible).map(function (i) { return (i.getAttribute('aria-label') || i.name || i.id || i.placeholder || i.type) + '=' + (i.value || ''); }).slice(0, 40),",
    "  alerts: [...d.querySelectorAll('[role=alert],[role=status],.error,.warning')].filter(visible).map(text).filter(Boolean).slice(0, 20),",
    "  body: text(d.body).slice(0, 2000),",
    "};",
    "return JSON.stringify(out);",
  ].join("\n"));
}

/** Clicks the first visible button-like element of the webview whose text matches `pattern`; gives `CLICKED:<text>`,
 * `DISABLED:<reason>` or `NO_BUTTON`. */
export function clickButton(wb: Workbench, sessionId: string, pattern: string): Promise<string> {
  return evalWebview(wb, sessionId, [
    `var re = new RegExp(${JSON.stringify(pattern)}, 'i');`,
    "var all = [...d.querySelectorAll('button,[role=button],a,[role=tab],[role=menuitem],summary')].filter(function (x) { return x.offsetParent !== null; });",
    "var b = all.find(function (x) { return re.test((x.innerText || x.textContent || x.getAttribute('aria-label') || x.title || '').trim()); });",
    "if (!b) return 'NO_BUTTON';",
    "if (b.disabled) return 'DISABLED:' + (b.title || b.getAttribute('aria-disabled') || '');",
    "b.scrollIntoView({block: 'center'});",
    "b.click();",
    "return 'CLICKED:' + (b.innerText || b.textContent || '').trim().slice(0, 60);",
  ].join("\n"));
}

/** The quick input while it is open, or null. */
export function quickInput(wb: Workbench): Promise<QuickInput | null> {
  return evalPage<QuickInput | null>(wb, [
    "(function () {",
    "  var w = document.querySelector('.quick-input-widget');",
    "  if (!w || w.style.display === 'none' || w.offsetParent === null) return null;",
    "  var text = function (el) { return el ? (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim() : ''; };",
    "  return {",
    "    title: text(w.querySelector('.quick-input-title')),",
    "    placeholder: (w.querySelector('input') || {}).placeholder || '',",
    "    message: text(w.querySelector('.quick-input-message')),",
    "    items: [...w.querySelectorAll('.monaco-list-row')].map(text).slice(0, 30),",
    "  };",
    "})()",
  ].join("\n"));
}

const KEY_CODES: Record<string, number> = { Enter: 13, Escape: 27, Tab: 9, ArrowDown: 40, ArrowUp: 38 };

/** Presses `key` in the workbench. */
export async function pressKey(wb: Workbench, key: string): Promise<void> {
  for (const type of ["keyDown", "keyUp"]) {
    await wb.connection.send("Input.dispatchKeyEvent", {
      type,
      key,
      code: key,
      windowsVirtualKeyCode: KEY_CODES[key],
      nativeVirtualKeyCode: KEY_CODES[key],
      ...(key === "Enter" && type === "keyDown" ? { text: "\r" } : {}),
    }, wb.pageSession);
  }
}

/** Picks the quick-input item whose text matches `pattern` (`PICKED:<text>` or `NO_ITEM`), or types `text` and accepts. */
export async function answerQuickInput(wb: Workbench, answer: { pattern?: string; text?: string }): Promise<string> {
  if (answer.text !== undefined) {
    await evalPage(wb, "(function () { var i = document.querySelector('.quick-input-widget input'); i.value = " +
      JSON.stringify(answer.text) + "; i.dispatchEvent(new Event('input', {bubbles: true})); return true; })()");
    await sleep(400);
    await pressKey(wb, "Enter");
    return "typed";
  }
  return evalPage<string>(wb, [
    "(function () {",
    `  var re = new RegExp(${JSON.stringify(answer.pattern ?? "")}, 'i');`,
    "  var row = [...document.querySelectorAll('.quick-input-widget .monaco-list-row')].find(function (r) { return re.test(r.innerText || ''); });",
    "  if (!row) return 'NO_ITEM';",
    "  row.click();",
    "  return 'PICKED:' + (row.innerText || '').replace(/\\s+/g, ' ').slice(0, 80);",
    "})()",
  ].join("\n"));
}

/** The texts of the notifications on screen. */
export function notifications(wb: Workbench): Promise<string[]> {
  return evalPage<string[]>(wb, "[...document.querySelectorAll('.notification-toast')].map(function (n) { return (n.innerText || '').replace(/\\s+/g, ' ').trim(); }).filter(Boolean).slice(0, 10)");
}

/** The texts of the status bar's items. */
export function statusBar(wb: Workbench): Promise<string[]> {
  return evalPage<string[]>(wb, "[...document.querySelectorAll('.statusbar-item')].map(function (n) { return (n.innerText || '').trim(); }).filter(Boolean)");
}
