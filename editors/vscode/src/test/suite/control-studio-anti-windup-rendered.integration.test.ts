import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, captureRenderedWorkbenchCdp } from "./plant-rendered-harness";

suite("Control Studio anti-windup rendered comparison", () => {
  test("renders a qualified Review to matched actual-ST comparison sequence", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ANTI_WINDUP_RENDERED !== "1") this.skip();
    this.timeout(60_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(project);
    const loop = JSON.parse(fs.readFileSync(path.join(project!, "control-loop.json"), "utf8"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("advanced handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      await pollValue("fresh anti-windup loop", () => inner("return d.querySelector('[data-control-studio-quality]')?.textContent||'';"), value => !/Unavailable|Waiting|stale/i.test(value), 15_000);
      await inner("const tools=d.querySelector('[data-engineer-tools]');if(tools){const parents=[];for(let p=tools.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')parents.push(p);for(const p of parents.reverse())if(!p.open)p.querySelector(':scope > summary').click();tools.click();}return 'advanced controls';");
      await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Test').click();return 'Test';");
      await pollValue("visible Test controls", () => inner("const b=d.querySelector('[data-test-review]');const r=b?.getBoundingClientRect();return JSON.stringify({visible:Boolean(r?.width&&r.height),manual:d.querySelector('[data-control-studio]').dataset.manualWorkspace,pressed:d.querySelector('[data-engineer-tools]')?.getAttribute('aria-pressed'),pane:d.querySelector('[data-workflow-pane=Test]')?.hidden,mode:[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.getAttribute('aria-selected')==='true')?.textContent});"), value => JSON.parse(value).visible, 5000);
      await inner("var w=d.defaultView;w.__antiWindupTerminal=[];w.addEventListener('message',e=>{if(e.data?.kind==='control-studio.experiment.result'&&e.data?.phase==='terminal')w.__antiWindupTerminal.push(e.data.payload)});return 'listening';");
      await inner("d.querySelector('[data-test-review]')?.click();return 'review';");
      await pollValue("reviewed bounded saturation test", () => inner("return d.querySelector('[data-test-state]')?.textContent||'';"), value => /Simulation test ready/i.test(value), 10_000);
      await inner("d.querySelector('[data-test-approve]')?.click();return 'start';");
      await pollValue("completed saturation-return Test", () => inner("return d.querySelector('[data-test-state]')?.textContent||'';"), value => /Completed|restored/i.test(value), 30_000);
      await pollValue("qualified anti-windup review", () => inner("var card=d.querySelector('[data-saturation-windup-evidence]'),w=d.defaultView;return JSON.stringify({terminal:w.__antiWindupTerminal,review:!d.querySelector('[data-anti-windup-review]')?.hidden,visible:!d.querySelector('[data-anti-windup-actions]')?.hidden,cardHidden:card?.hidden});"), value => {
        try { const parsed=JSON.parse(value); return parsed.review && parsed.visible && !parsed.cardHidden; } catch { return false; }
      }, 10_000);
      await inner("d.querySelector('[data-anti-windup-tt]').value='0';d.querySelector('[data-anti-windup-review]').click();return 'invalid tracking time';");
      assert.match(await inner("return d.querySelector('[data-anti-windup-status]').textContent;"), /positive tracking time/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-anti-windup-verify]').hidden);"), "true");
      await inner("var input=d.querySelector('[data-anti-windup-tt]');input.value='4';d.querySelector('[data-anti-windup-review]')?.click();return 'candidate review';");
      await pollValue("matched verification control", () => inner("return JSON.stringify({verify:!d.querySelector('[data-anti-windup-verify]')?.hidden,text:d.querySelector('[data-anti-windup-status]')?.textContent||''});"), value => {
        try { const parsed=JSON.parse(value); return parsed.verify && /matched actual-ST/i.test(parsed.text); } catch { return false; }
      }, 15_000);
      await inner("d.querySelector('[data-anti-windup-verify]')?.click();return 'verify';");
      const text = await pollValue("rendered measured comparison", () => inner("return d.querySelector('[data-anti-windup-status]')?.textContent||'';"), value => /Matched actual-ST comparison.*tracking/i.test(value), 30_000);
      assert.match(text, /retained-integral change/i);
      assert.strictEqual(await inner("return String(d.querySelector('[data-anti-windup-rollback]').hidden);"), "true", "isolated verification must not offer a live rollback when it has not staged live parameters");
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        const visible = JSON.parse(await inner("var tab=[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Test');tab?.click();var pane=d.querySelector('[data-workflow-pane=Test]'),card=d.querySelector('[data-saturation-windup-evidence]');card?.scrollIntoView({block:'center'});var box=card?.getBoundingClientRect();return JSON.stringify({testVisible:Boolean(pane&&!pane.hidden),cardVisible:Boolean(box&&box.top>=0&&box.bottom<=d.defaultView.innerHeight),text:card?.textContent||''});"));
        assert.strictEqual(visible.testVisible, true, JSON.stringify(visible));
        assert.strictEqual(visible.cardVisible, true, JSON.stringify(visible));
        assert.match(visible.text, /Matched actual-ST comparison/i);
        await new Promise(resolve => setTimeout(resolve, 250));
        await captureRenderedWorkbenchCdp(connection, screenshot);
      }
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });
});
