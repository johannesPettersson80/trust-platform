import { pollValue, httpJson, evaluateInWebview, type CdpConnection } from "./plant-rendered-harness";
export async function attachControlStudio(connection: CdpConnection, port: number, previousEditorSession = "", expectedLoopId = ""): Promise<string> {
  return pollValue("Control Studio webview", async () => {
    const targets = await httpJson(port, "/json");
    for (const target of targets.filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
      let sessionId: string | undefined;
      try {
        const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
        if (!sessionId)
          continue;
        await connection.send("Runtime.enable", {}, sessionId);
        const marker = await evaluateInWebview(connection, sessionId, `var root=d.querySelector('[data-control-studio]');return String(Boolean(root&&root.dataset.editorSessionId&&root.dataset.editorSessionId!==${JSON.stringify(previousEditorSession)}&&(!${JSON.stringify(expectedLoopId)}||root.dataset.loopId===${JSON.stringify(expectedLoopId)})));`);
        if (marker === "true")
          return sessionId;
        await connection.send("Target.detachFromTarget", { sessionId });
      }
      catch {
        if (sessionId)
          await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
    }
    return "";
  }, (value) => value.length > 0, 20000);
}
