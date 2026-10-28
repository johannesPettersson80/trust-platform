import * as vscode from "vscode";
import {
  captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment,
} from "./plant-rendered-harness";

/** Attach to the shipped gallery, never to a separately rendered HTML fixture. */
export async function openRenderedExamplesGallery() {
  await vscode.commands.executeCommand("trust.examples.start");
  const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
  const version = await httpJson(port, "/json/version");
  const connection = await connectCdp(version.webSocketDebuggerUrl);
  try {
    const initialTargets = await httpJson(port, "/json") as Array<{ id: string; type?: string; url: string }>;
    const workbenchTarget = initialTargets.find((target) =>
      target.type === "page" && !/index\.html/.test(target.url) && /vscode-file|workbench/i.test(target.url)
    );
    const workbenchSession = workbenchTarget
      ? await connection.send("Target.attachToTarget", { targetId: workbenchTarget.id, flatten: true })
      : undefined;
    if (workbenchSession?.sessionId) await connection.send("Runtime.enable", {}, workbenchSession.sessionId);
    const attachGallery = () => pollValue("examples webview", async () => {
      const targets = await httpJson(port, "/json") as Array<{ id: string; url: string }>;
      for (const target of targets.filter((item) => /index\.html/.test(item.url))) {
        const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        await connection.send("Runtime.enable", {}, attached.sessionId);
        const marker = await evaluateInWebview(connection, attached.sessionId, "return String(Boolean(d.querySelector('#grid') && d.querySelector('#title')));");
        if (marker === "true") return attached.sessionId as string;
        await connection.send("Target.detachFromTarget", { sessionId: attached.sessionId });
      }
      return "";
    }, Boolean, 20_000);
    let sessionId = await attachGallery();
    return {
      reconnect: async () => { sessionId = await attachGallery(); },
      inner: (body: string) => evaluateInWebview(connection, sessionId, body),
      capture: (outputPath: string) => captureRenderedWorkbenchCdp(connection, outputPath),
      surfaceMetrics: async () => {
        if (!workbenchSession?.sessionId) throw new Error("The workbench surface is unavailable.");
        const window = await connection.send("Runtime.evaluate", {
          expression: "({width:innerWidth,height:innerHeight,scale:devicePixelRatio,outerWidth,outerHeight,screenWidth:screen.width,screenHeight:screen.height})",
          returnByValue: true,
        }, workbenchSession.sessionId);
        return { window: window.result?.value, layout: await connection.send("Page.getLayoutMetrics", {}, workbenchSession.sessionId) };
      },
      focusWebview: async () => {
        if (!workbenchSession?.sessionId) return;
        await connection.send("Page.bringToFront", {}, workbenchSession.sessionId);
        await connection.send("Runtime.evaluate", {
          expression: "document.querySelector('iframe')?.focus();",
        }, workbenchSession.sessionId);
      },
      webviewFrameRect: async () => {
        const targetSession = workbenchSession?.sessionId;
        if (!targetSession) return undefined;
        const result = await connection.send("Runtime.evaluate", {
          expression: "(()=>{const f=document.querySelector('iframe');if(!f)return undefined;const r=f.getBoundingClientRect();return {x:r.x,y:r.y,w:r.width,h:r.height};})()",
          returnByValue: true,
        }, targetSession);
        return result.result?.value;
      },
      clickWebviewPoint: async (x: number, y: number) => {
        const targetSession = workbenchSession?.sessionId ?? sessionId;
        await connection.send("Input.dispatchMouseEvent", {
          type: "mousePressed", x, y, button: "left", clickCount: 1,
        }, targetSession);
        await connection.send("Input.dispatchMouseEvent", {
          type: "mouseReleased", x, y, button: "left", clickCount: 1,
        }, targetSession);
        await new Promise((resolve) => setTimeout(resolve, 100));
      },
      pressKey: async (key: string, code = key) => {
        const targetSession = workbenchSession?.sessionId ?? sessionId;
        const virtualKeyCodes: Record<string, number> = { Enter: 13, " ": 32, ArrowDown: 40, ArrowUp: 38, ArrowLeft: 37, ArrowRight: 39 };
        const text = key === "Enter" ? "\r" : key.length === 1 ? key : undefined;
        await connection.send("Input.dispatchKeyEvent", {
          type: "keyDown", key, code, windowsVirtualKeyCode: virtualKeyCodes[key], nativeVirtualKeyCode: virtualKeyCodes[key],
          text,
          unmodifiedText: text,
        }, targetSession);
        await connection.send("Input.dispatchKeyEvent", {
          type: "keyUp", key, code, windowsVirtualKeyCode: virtualKeyCodes[key], nativeVirtualKeyCode: virtualKeyCodes[key],
        }, targetSession);
        await new Promise((resolve) => setTimeout(resolve, 50));
      },
      close: () => connection.close(),
    };
  } catch (error) {
    connection.close();
    throw error;
  }
}
