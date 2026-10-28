import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, type CdpConnection } from "./plant-rendered-harness";

suite("Control Studio shared signal multi-select rendered", () => {
  test("renders immediate signal dropdowns and remains usable at narrow width", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
      });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      let session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("multi-select rendered handlers", () => inner("return String(Boolean(d.querySelector('[data-studio-three-page]')?.dataset.hostReady));"), value => value === "true", 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-multi-select-trigger]').length);"), "2", "Identify exposes compact input and output triggers");
      assert.strictEqual(await inner("return String([...d.querySelectorAll('select[multiple]')].every(x=>x.hidden&&x.tabIndex===-1));"), "true", "Native multi-selects remain available to host logic but do not require modifier keys");
      assert.match(await inner("return d.querySelector('[data-multi-select-trigger]').textContent;"), /P1\.ManipulatedValue/);
      assert.match(await inner("return d.querySelector('[data-multi-select-trigger]').textContent;"), /%|°C/);

      await inner("d.querySelector('[data-multi-select-trigger]').click();return 'opened';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-multi-select-dialog][open]')));"), "true");
      await inner("const dialog=d.querySelector('[data-multi-select-dialog]');dialog.querySelector('[data-multi-select-option]').click();return 'changed';");
      assert.strictEqual(await inner("return String(d.querySelector('select[multiple]').selectedOptions.length);"), "0", "Checkbox changes apply immediately");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-multi-select-done],[data-multi-select-cancel]').length);"), "0", "No confirmation buttons in a signal dropdown");
      await inner("d.querySelector('[data-multi-select-dialog] [data-multi-select-option]').click();d.body.dispatchEvent(new d.defaultView.PointerEvent('pointerdown',{bubbles:true}));return 'outside';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-multi-select-dialog][open]')));"), "false", "Outside click closes the choices");
      await inner("d.querySelector('[data-multi-select-trigger]').click();d.querySelector('[data-multi-select-trigger]').click();return 'toggle';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-multi-select-dialog][open]')));"), "false", "Trigger toggles the dropdown");
      await inner("const t=d.querySelector('[data-multi-select-trigger]');t.dispatchEvent(new d.defaultView.KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true}));d.querySelector('[data-multi-select-dialog]').dispatchEvent(new d.defaultView.KeyboardEvent('keydown',{key:'Escape',bubbles:true}));return 'escaped';");
      assert.strictEqual(await inner("return String(d.activeElement===d.querySelector('[data-multi-select-trigger]'));"), "true", "Escape restores trigger focus");
      const screenshot = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-signals.png");
      if (screenshot) {
        await inner("d.querySelector('[data-multi-select-trigger]').click();return 'picker';");
        await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-wide.png"));
        await setViewportWidth(connection, 680, 850);
        await pollValue("actual narrow webview", () => inner("return String(d.defaultView.innerWidth);"), value => Number(value) <= 650, 10000);
        await pollValue("open picker fits the resized editor", () => inner("const r=d.querySelector('[data-multi-select-dialog][open]').getBoundingClientRect();return JSON.stringify({left:r.left,right:r.right,width:d.defaultView.innerWidth});"), value => { const r=JSON.parse(value);return r.left>=0&&r.right<=r.width+0.5; }, 5000);
        await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-narrow.png"));
        await clearViewportWidth(connection);
        session = await attachControlStudio(connection, port, "", loop.id);
        await pollValue("restored full editor", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
        await inner("d.body.dispatchEvent(new d.defaultView.PointerEvent('pointerdown',{bubbles:true}));return 'closed';");
      }

      // Identify is a live campaign: without a running target the engine refuses it at once (specification 35, B3.9), and the page is busy for an
      // instant only. The engineer starts the local simulator first, so that Identify runs until Abort.
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.ok(started.ok, `The local simulator starts for the live identification: ${JSON.stringify(started)}`);
      await inner("d.querySelector('[data-engineer-find]').click();return 'busy';");
      await pollValue("Identify busy state", () => inner("return String(d.querySelector('[data-studio-three-page]').dataset.busy);"), value => value === "true", 5000);
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-multi-select-trigger]')].every(x=>x.disabled));"), "true", "Both signal pickers disable while Identify runs");
      await inner("d.querySelector('[data-engineer-find]').click();return 'abort';");
      await pollValue("Identify cancellation completes", () => inner("return String(d.querySelector('[data-studio-three-page]').dataset.busy);"), value => value === "false", 20000);
    } finally {
      await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-picker-final.png"));
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtimeLifecycleService.stopRuntime();
    }
  });
});

let scratchUri: string | undefined;
async function setViewportWidth(_connection: CdpConnection, _width: number, _height: number): Promise<void> {
  const preview = await vscode.workspace.openTextDocument({ content: "", language: "plaintext" });
  scratchUri = preview.uri.toString();
  await vscode.window.showTextDocument(preview, { viewColumn: vscode.ViewColumn.Beside, preview: true });
  await vscode.commands.executeCommand("vscode.setEditorLayout", { orientation: 0, groups: [{ size: 0.4 }, { size: 0.6 }] });
}
async function clearViewportWidth(_connection: CdpConnection): Promise<void> {
  const tab = vscode.window.tabGroups.all.flatMap(group => group.tabs).find(tab => tab.input instanceof vscode.TabInputText && tab.input.uri.toString() === scratchUri);
  if (tab) await vscode.window.tabGroups.close(tab);
  scratchUri = undefined;
  await vscode.commands.executeCommand("workbench.action.joinAllGroups");
}
