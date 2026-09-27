import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import {
  captureRenderedWorkbench,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  sleep,
} from "./plant-rendered-harness";

suite("Control Studio saturation and windup actual rendered workflow", () => {
  test("retains and explains the 35 to 45 to 35 degree actual-ST saturation-return record", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_SATURATION_WINDUP !== "1") this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(project, "the saturation runner must open the copied heater project");
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, "control-lesson-06");
    loop.controller.saturationEvidence = { requestedOutputId: "requested-power", integralStateId: "stored-correction" };
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await runtimeLifecycleService.stopRuntime();
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(started.ok, true, JSON.stringify(started));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
      });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const stage = (name: "Test" | "Monitor") => inner(
        `var b=[...d.querySelectorAll('[data-control-studio-mode]')].find(x=>x.textContent.trim()===${JSON.stringify(name)});if(!b)return 'missing';b.click();return b.textContent.trim();`
      );
      await pollValue("fresh hot heater state", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-quality]')?.textContent||'';");
      }, value => !/Unavailable|Waiting|stale/i.test(value), 15_000);
      assert.strictEqual(await stage("Test"), "Test");
      await inner("d.querySelector('[data-test-review]').click();return 'review';");
      await pollValue("saturation test review", () => inner("return d.querySelector('[data-test-state]').textContent;"),
        value => value.includes("Simulation test ready"), 15_000);
      await inner("d.querySelector('[data-test-approve]').click();return 'start';");
      await pollValue("saturation test terminal", () => inner("return d.querySelector('[data-test-state]').textContent;"),
        value => /Completed|restor/i.test(value), 95_000);
      await sleep(300);

      const rendered = JSON.parse(await inner(`
        var card=d.querySelector('[data-saturation-windup-evidence]');
        return JSON.stringify({present:Boolean(card),hidden:card?.hidden,
          text:(card?.textContent||'').trim(),requested:d.querySelector('[data-saturation-windup-requested]')?.textContent,
          applied:d.querySelector('[data-saturation-windup-applied]')?.textContent,
          recovery:d.querySelector('[data-saturation-windup-recovery]')?.textContent});
      `));
      assert.strictEqual(rendered.present, true, "The Test surface must show saturation evidence from the actual ST record.");
      assert.strictEqual(rendered.hidden, false);
      assert.match(rendered.text, /requested.*applied|saturation|windup/i);
      assert.ok(Number(rendered.requested) > 100, `unlimited request must remain visible: ${JSON.stringify(rendered)}`);
      assert.strictEqual(Number(rendered.applied), 100, `applied heater power must remain clamped: ${JSON.stringify(rendered)}`);
      assert.ok(Number(rendered.recovery) > 0, `the record must show delayed release after the target returns: ${JSON.stringify(rendered)}`);
      const screenshot = process.env.TRUST_CONTROL_STUDIO_SATURATION_WINDUP_SCREENSHOT;
      if (screenshot) {
        const visible = JSON.parse(await inner("var card=d.querySelector('[data-saturation-windup-evidence]');var workspace=card?.closest('.workspace');if(workspace&&card)workspace.scrollTop=card.offsetTop-workspace.offsetTop;card?.scrollIntoView({block:'start'});var box=card?.getBoundingClientRect();return JSON.stringify({text:card?.textContent||'',top:box?.top,bottom:box?.bottom,height:innerHeight});"));
        assert.ok(visible.top >= 0 && visible.bottom <= visible.height,
          `The complete saturation evidence card must be visible in its workspace: ${JSON.stringify(visible)}`);
        await sleep(300);
        captureRenderedWorkbench(screenshot);
      }
    } finally {
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
