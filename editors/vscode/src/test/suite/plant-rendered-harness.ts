import * as fs from "fs";
import * as http from "http";
import * as path from "path";
import * as vscode from "vscode";
import { execFileSync } from "child_process";

export const sleep = (milliseconds: number) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

export type CdpConnection = {
  send(method: string, params?: Record<string, unknown>, sessionId?: string): Promise<any>;
  close(): void;
};

export type WebviewEvaluator = (body: string) => Promise<string>;

export type RenderedPlantSession = {
  connection: CdpConnection;
  inner: WebviewEvaluator;
  setValue(selector: string, value: string): Promise<string>;
  pressKey(key: string, code?: string): Promise<void>;
  pressTab(): Promise<void>;
  close(): void;
};

export async function openRenderedPlantSession(): Promise<RenderedPlantSession> {
  const userDataDir = requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR");
  const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
  if (extension) await extension.activate();
  const theme = process.env.TRUST_PLANT_RENDERED_THEME?.trim();
  if (theme) {
    await vscode.workspace.getConfiguration("workbench").update(
      "colorTheme",
      theme,
      vscode.ConfigurationTarget.Global
    );
    await sleep(500);
  }
  await vscode.commands.executeCommand("trust-lsp.networkCanvas.open");
  if (process.env.TRUST_PLANT_RENDERED_SPLIT === "1") {
    await vscode.commands.executeCommand("workbench.action.editorLayoutTwoColumns");
    const companion = await vscode.workspace.openTextDocument({
      content: "Narrow rendered temperature example companion editor",
      language: "plaintext",
    });
    await vscode.window.showTextDocument(companion, {
      viewColumn: vscode.ViewColumn.Two,
      preview: false,
    });
    await vscode.commands.executeCommand("workbench.action.focusFirstEditorGroup");
  }

  const port = await devToolsPort(userDataDir, 15_000);
  const version = await pollValue(
    "CDP version endpoint",
    async () => httpJson(port, "/json/version"),
    (value) => typeof value?.webSocketDebuggerUrl === "string",
    15_000
  );
  const connection = await connectCdp(version.webSocketDebuggerUrl);
  try {
    const sessionId = await attachNetworkCanvas(connection, port, 20_000);
    const inner = (body: string) => evaluateInWebview(connection, sessionId, body);
    return {
      connection,
      inner,
      setValue: (selector, value) => setWebviewValue(inner, selector, value),
      pressKey: async (key, code = key) => {
        await connection.send("Input.dispatchKeyEvent", {
          type: "keyDown", key, code,
        }, sessionId);
        await connection.send("Input.dispatchKeyEvent", {
          type: "keyUp", key, code,
        }, sessionId);
        await sleep(50);
      },
      pressTab: async () => {
        await connection.send("Input.dispatchKeyEvent", {
          type: "keyDown", key: "Tab", code: "Tab",
          windowsVirtualKeyCode: 9, nativeVirtualKeyCode: 9,
        }, sessionId);
        await connection.send("Input.dispatchKeyEvent", {
          type: "keyUp", key: "Tab", code: "Tab",
          windowsVirtualKeyCode: 9, nativeVirtualKeyCode: 9,
        }, sessionId);
        await sleep(50);
      },
      close: () => connection.close(),
    };
  } catch (error) {
    connection.close();
    throw error;
  }
}

export function captureRenderedWorkbench(outputPath: string): void {
  const resolved = path.resolve(outputPath);
  fs.mkdirSync(path.dirname(resolved), { recursive: true });
  execFileSync("/usr/bin/import", ["-window", "root", resolved], { stdio: "ignore" });
}

/** Capture the workbench page that contains the real rendered webview. */
export async function captureRenderedWorkbenchCdp(
  connection: CdpConnection,
  outputPath: string,
): Promise<void> {
  const targets = await connection.send("Target.getTargets");
  const target = (targets.targetInfos as unknown[] | undefined)?.find((value): value is { targetId: string; type: string } => {
    if (!value || typeof value !== "object") return false;
    const candidate = value as Record<string, unknown>;
    return candidate.type === "page" && typeof candidate.targetId === "string";
  });
  if (!target) throw new Error("CDP did not expose a top-level VS Code workbench page.");
  const attached = await connection.send("Target.attachToTarget", { targetId: target.targetId, flatten: true });
  const sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
  if (!sessionId) throw new Error("CDP did not attach to the VS Code workbench page.");
  try {
    const metrics = await connection.send("Page.getLayoutMetrics", {}, sessionId);
    const viewport = metrics.cssVisualViewport;
    const zoom = viewport?.zoom ?? 1;
    if (!viewport || ![viewport.pageX, viewport.pageY, viewport.clientWidth, viewport.clientHeight, zoom].every(Number.isFinite) ||
        viewport.clientWidth <= 0 || viewport.clientHeight <= 0 || zoom <= 0) {
      throw new Error("CDP did not expose a measured workbench viewport.");
    }
    const result = await connection.send("Page.captureScreenshot", {
      format: "png",
      fromSurface: true,
      // CDP's screenshot clip uses device-independent coordinates. Convert the
      // CSS viewport by the browser zoom so Electron does not crop its surface.
      clip: { x: viewport.pageX * zoom, y: viewport.pageY * zoom,
        width: Math.round(viewport.clientWidth * zoom), height: Math.round(viewport.clientHeight * zoom), scale: 1 },
      captureBeyondViewport: false,
    }, sessionId);
    if (typeof result?.data !== "string" || result.data.length === 0) {
      throw new Error("CDP did not return a rendered screenshot.");
    }
    const resolved = path.resolve(outputPath);
    fs.mkdirSync(path.dirname(resolved), { recursive: true });
    fs.writeFileSync(resolved, Buffer.from(result.data, "base64"));
  } finally {
    await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
  }
}

export async function beginPlantDraft(
  session: RenderedPlantSession
): Promise<{ add: string; plant: string }> {
  const add = await pollValue(
    "enabled Add action",
    async () => session.inner("var button=d.querySelector('[data-canvas-add]');if(!button||button.disabled)return 'WAIT';button.click();return 'ADD';"),
    (value) => value === "ADD",
    10_000
  );
  const plant = await pollValue(
    "simulated plant picker",
    async () => session.inner("var button=[...d.querySelectorAll('button,[role=button]')].find((x)=>/simulated plant/i.test(x.textContent||''));if(!button)return 'WAIT';button.click();return 'PLANT';"),
    (value) => value === "PLANT",
    10_000
  );
  await pollValue(
    "plant model selector",
    async () => session.inner("return String(Boolean(d.querySelector('select[name=plant-model]')));"),
    (value) => value === "true",
    10_000
  );
  return { add, plant };
}

export async function bindFirstAddressedSignals(
  session: RenderedPlantSession
): Promise<{ command: string; feedback: string }> {
  const choices = JSON.parse(await pollValue(
    "addressed signal choices",
    async () => session.inner("var command=d.querySelector('select[name=plant-command-source]');var feedback=d.querySelector('select[name=plant-feedback-target]');return JSON.stringify({commands:command?[...command.options].map((x)=>x.value):[],feedbacks:feedback?[...feedback.options].map((x)=>x.value):[],text:(d.body&&d.body.textContent||'').slice(-800)});"),
    (value) => {
      try {
        const parsed = JSON.parse(value);
        return Boolean(parsed.commands[1] && parsed.feedbacks[1]);
      } catch {
        return false;
      }
    },
    20_000
  ));
  const command = choices.commands[1];
  const feedback = choices.feedbacks[1];
  await session.setValue("select[name=plant-command-source]", command);
  await session.setValue("select[name=plant-feedback-target]", feedback);
  return { command, feedback };
}

export function setWebviewChecked(
  inner: WebviewEvaluator,
  selector: string,
  checked: boolean
): Promise<string> {
  return inner(`var el=d.querySelector(${JSON.stringify(selector)});if(!el)return 'NO_FIELD';if(Boolean(el.checked)!==${checked})el.click();return String(el.checked);`);
}

export function requiredEnvironment(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required for the rendered plant test.`);
  return value;
}

export async function devToolsPort(userDataDir: string, timeoutMs: number): Promise<number> {
  const activePort = path.join(userDataDir, "DevToolsActivePort");
  return pollValue(
    "Chromium DevToolsActivePort",
    async () => {
      try {
        return Number(fs.readFileSync(activePort, "utf8").split(/\r?\n/, 1)[0]);
      } catch {
        return 0;
      }
    },
    (value) => Number.isInteger(value) && value > 0,
    timeoutMs
  );
}

export function httpJson(port: number, requestPath: string): Promise<any> {
  return new Promise((resolve, reject) => {
    const request = http.get(`http://127.0.0.1:${port}${requestPath}`, (response) => {
      let body = "";
      response.on("data", (part) => { body += part; });
      response.on("end", () => {
        try { resolve(JSON.parse(body)); } catch (error) { reject(error); }
      });
    });
    request.on("error", reject);
    request.setTimeout(5_000, () => request.destroy(new Error(`CDP HTTP timeout: ${requestPath}`)));
  });
}

export function connectCdp(url: string): Promise<CdpConnection> {
  const WebSocket = require("ws") as any;
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(url);
    let requestId = 0;
    const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void; timer: NodeJS.Timeout }>();
    socket.on("message", (data: { toString(): string }) => {
      const message = JSON.parse(data.toString());
      const request = pending.get(message.id);
      if (!request) return;
      clearTimeout(request.timer);
      pending.delete(message.id);
      if (message.error) request.reject(new Error(JSON.stringify(message.error)));
      else request.resolve(message.result);
    });
    socket.on("error", reject);
    socket.on("open", () => resolve({
      send(method, params = {}, sessionId) {
        return new Promise((resolveSend, rejectSend) => {
          const id = ++requestId;
          const timer = setTimeout(() => {
            pending.delete(id);
            rejectSend(new Error(`CDP timeout: ${method}`));
          }, 8_000);
          pending.set(id, { resolve: resolveSend, reject: rejectSend, timer });
          socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
        });
      },
      close() { socket.close(); },
    }));
  });
}

export async function attachNetworkCanvas(
  connection: CdpConnection,
  port: number,
  timeoutMs: number
): Promise<string> {
  return pollValue(
    "Network Canvas webview CDP target",
    async () => {
      const targets = await httpJson(port, "/json");
      for (const target of targets.filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
        let sessionId: string | undefined;
        try {
          const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
          sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
          if (!sessionId) continue;
          await connection.send("Runtime.enable", {}, sessionId);
          const marker = await evaluateInWebview(connection, sessionId, "return String(Boolean(d.querySelector('[data-canvas-add]')));");
          if (marker === "true") return sessionId;
          await connection.send("Target.detachFromTarget", { sessionId });
        } catch {
          if (sessionId) await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
        }
      }
      return "";
    },
    (value) => typeof value === "string" && value.length > 0,
    timeoutMs
  );
}

export async function evaluateInWebview(
  connection: CdpConnection,
  sessionId: string,
  body: string
): Promise<string> {
  const expression = `(async function(){try{var f=document.querySelector('iframe');var d=f&&f.contentDocument;if(!d)return 'NO_WEBVIEW_DOCUMENT';${body}}catch(error){return 'ERR:'+error.name+':'+error.message;}})()`;
  const evaluation = await connection.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId);
  if (evaluation.exceptionDetails) throw new Error(`Runtime.evaluate exception: ${JSON.stringify(evaluation.exceptionDetails)}`);
  const value = evaluation.result?.value;
  if (typeof value === "string" && value.startsWith("ERR:")) throw new Error(`Webview evaluation failed: ${value}`);
  return String(value);
}

export function setWebviewValue(
  inner: (body: string) => Promise<string>,
  selector: string,
  value: string
): Promise<string> {
  return inner(`var el=d.querySelector(${JSON.stringify(selector)});if(!el)return 'NO_FIELD';var w=el.ownerDocument.defaultView;var proto=el.tagName==='TEXTAREA'?w.HTMLTextAreaElement.prototype:el.tagName==='SELECT'?w.HTMLSelectElement.prototype:w.HTMLInputElement.prototype;var setter=Object.getOwnPropertyDescriptor(proto,'value').set;setter.call(el,${JSON.stringify(value)});el.dispatchEvent(new w.Event('input',{bubbles:true}));el.dispatchEvent(new w.Event('change',{bubbles:true}));return el.value;`);
}

export function fieldState(
  inner: (body: string) => Promise<string>,
  selector: string
): Promise<string> {
  return inner(`var field=d.querySelector(${JSON.stringify(selector)});return JSON.stringify({value:field&&field.value,invalid:field&&field.getAttribute('aria-invalid')});`);
}

export async function pollSave(
  inner: (body: string) => Promise<string>,
  disabled: boolean,
  label: string
): Promise<void> {
  await pollValue(
    `${label} Save eligibility`,
    async () => inner("var panel=[...d.querySelectorAll('aside')].find((x)=>/Plant properties/i.test(x.getAttribute('aria-label')||''));var save=panel&&[...panel.querySelectorAll('button')].find((x)=>(x.textContent||'').trim()==='Save');return JSON.stringify({disabled:save&&save.disabled,summary:(panel&&panel.querySelector('[data-plant-validation-summary]')?.textContent||'').trim()});"),
    (value) => JSON.parse(value).disabled === disabled,
    10_000
  );
}

export async function pollValue<T>(
  label: string,
  read: () => Promise<T>,
  accept: (value: T) => boolean,
  timeoutMs: number
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let value!: T;
  while (Date.now() < deadline) {
    value = await read();
    if (accept(value)) return value;
    await sleep(100);
  }
  throw new Error(`Timed out waiting for ${label}: ${String(value)}`);
}
