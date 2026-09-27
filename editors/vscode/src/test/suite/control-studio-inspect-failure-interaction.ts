import * as assert from "assert";
import * as path from "path";
import * as vscode from "vscode";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { captureRenderedWorkbench, httpJson, pollValue, type CdpConnection } from "./plant-rendered-harness";

export async function inspectFailedController(connection: CdpConnection, port: number,
  inner: EvaluateWebview, project: string): Promise<void> {
  if (process.env.TRUST_ENGINEER_INSPECT_AUDIT !== "1") return;
  assert.strictEqual(await inner("const b=d.querySelector('[data-engineer-inspect]');const r=b.getBoundingClientRect();if(b.disabled||!r.width||!r.height)return 'unavailable';b.click();return 'clicked';"), "clicked");
  const targets = await httpJson(port, "/json");
  const target = targets.find((item: any) => item.type === "page" && /workbench/.test(item.url));
  assert.ok(target, "the real workbench must own the symbol picker");
  const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
  const session = attached.sessionId as string;
  const read = async (expression: string) => {
    const result = await connection.send("Runtime.evaluate", { expression, returnByValue: true }, session);
    return String(result.result?.value ?? "");
  };
  try {
    await connection.send("Runtime.enable", {}, session);
    await pollValue("controller symbol search", () => read("document.querySelector('.quick-input-widget input')?.value"), value => value === "#Main", 10000);
    await pollValue("matching controller implementation", () => read("document.querySelector('.quick-input-list')?.textContent"), value => value.includes("Main"), 10000);
    const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
    if (screenshot) captureRenderedWorkbench(screenshot.replace(/\.png$/, "-implementation-picker.png"));
    await connection.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 }, session);
    await connection.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 }, session);
    await pollValue("opened actual ST implementation", async () => vscode.window.activeTextEditor?.document.uri.fsPath ?? "",
      value => value === path.join(project, "src", "Main.st"), 10000);
    if (screenshot) captureRenderedWorkbench(screenshot.replace(/\.png$/, "-implementation-opened.png"));
    await vscode.commands.executeCommand("workbench.action.previousEditor");
    await pollValue("return to retained Control Studio", async () => vscode.window.tabGroups.activeTabGroup.activeTab?.label ?? "",
      value => value.includes("Control Studio"), 5000);
  } finally {
    await connection.send("Target.detachFromTarget", { sessionId: session });
  }
}
