import * as assert from "assert";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import {
  captureRenderedWorkbenchCdp,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  sleep,
} from "./plant-rendered-harness";

suite("Control Studio relay tuning rendered workflow", () => {
  test("shows qualified feedback cycles before an explicit unapplied PI calculation", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_RELAY_RENDERED !== "1") this.skip();
    this.timeout(60_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_PLANT_RENDERED_THEME?.trim();
    if (theme) {
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      await sleep(300);
    }
    const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(project, "the rendered relay test needs its copied ST project");
    const loop = require("fs").readFileSync(path.join(project, "control-loop.json"), "utf8");
    const definition = JSON.parse(loop);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: definition.projectId, runtimeId: definition.runtimeId }, loopDefinition: definition,
      });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await sleep(750);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Test').click();return 'Test';");
      const dispatch = (phase: string, payload: string) => inner(`
        var root=d.querySelector('[data-control-studio]');
        d.defaultView.dispatchEvent(new MessageEvent('message',{data:{schemaVersion:1,kind:'control-studio.experiment.result',messageId:crypto.randomUUID(),projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,phase:${JSON.stringify(phase)},payload:${payload}}}));
        return 'sent';
      `);
      await dispatch("terminal", JSON.stringify({
        status: "completed", stop_reason: "duration_complete", restoration: "restored",
        relayTuning: {
          status: "ready", cycles: 4,
          message: "Four measured feedback-relay cycles are ready. Calculate relay candidate to create an unapplied PI proposal.",
        },
      }));
      const ready = JSON.parse(await pollValue("rendered relay-ready evidence", () => inner(`
        var card=d.querySelector('[data-relay-tuning]');var button=d.querySelector('[data-relay-calculate]');
        return JSON.stringify({present:Boolean(card),hidden:card?.hidden,cycles:d.querySelector('[data-relay-cycles]')?.textContent,status:d.querySelector('[data-relay-tuning-status]')?.textContent,buttonHidden:button?.hidden,buttonDisabled:button?.disabled});
      `), value => {
        try { const state=JSON.parse(value); return state.present && !state.hidden && state.cycles === '4' && !state.buttonHidden && !state.buttonDisabled; }
        catch { return false; }
      }, 10_000));
      assert.match(ready.status, /feedback-relay cycles.*Calculate relay candidate/i);

      await dispatch("relay-candidate", JSON.stringify({ relayTuning: {
        status: "qualified", ultimateGain: 30.68047096, periodSeconds: 2,
        candidate: { proportionalGain: 13.80621193, integralTimeSeconds: 1.66666667, integralGain: 8.28372716, applied: false },
      } }));
      const calculated = JSON.parse(await pollValue("rendered relay candidate", () => inner(`
        var candidate=d.querySelector('[data-relay-candidate]');
        return JSON.stringify({hidden:candidate?.hidden,status:d.querySelector('[data-relay-tuning-status]')?.textContent,kp:d.querySelector('[data-relay-kp]')?.textContent,ti:d.querySelector('[data-relay-ti]')?.textContent,ki:d.querySelector('[data-relay-ki]')?.textContent});
      `), value => {
        try { const state=JSON.parse(value); return !state.hidden && /unapplied PI proposal/i.test(state.status) && Number(state.kp) > 13; }
        catch { return false; }
      }, 10_000));
      assert.ok(Number(calculated.ti) > 1 && Number(calculated.ki) > 8);

      const screenshot = process.env.TRUST_CONTROL_STUDIO_RELAY_SCREENSHOT?.trim();
      if (screenshot) {
        const visible = JSON.parse(await inner("var card=d.querySelector('[data-relay-tuning]');var workspace=card?.closest('.workspace');if(workspace&&card)workspace.scrollTop=card.offsetTop-workspace.offsetTop;card?.scrollIntoView({block:'start'});var box=card?.getBoundingClientRect();return JSON.stringify({top:box?.top,bottom:box?.bottom,height:innerHeight});"));
        assert.ok(visible.top >= 0 && visible.bottom <= visible.height,
          `the complete relay evidence card must be visible in the captured Test workspace: ${JSON.stringify(visible)}`);
        await sleep(300);
        await captureRenderedWorkbenchCdp(connection, screenshot);
      }
    } finally {
      connection.close();
    }
  });

  test("uses an actual ST feedback relay record before calculating its unapplied PI candidate", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_RELAY_ACTUAL !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_PLANT_RENDERED_THEME?.trim();
    if (theme) {
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      await sleep(300);
    }
    const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    assert.ok(project, "the actual relay test needs its copied ST project");
    const definition = JSON.parse(require("fs").readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await runtimeLifecycleService.stopRuntime();
      const start = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(start.ok, true, JSON.stringify(start));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: definition.projectId, runtimeId: definition.runtimeId }, loopDefinition: definition,
      });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await sleep(750);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Test').click();return 'Test';");
      await pollValue("fresh actual relay signals", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-quality]')?.textContent||'';");
      }, value => !/Unavailable|Waiting|stale/i.test(value), 15_000);
      await inner("var kind=d.querySelector('#test-kind');kind.value='relay';kind.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('#test-amplitude').value='1';d.querySelector('#test-duration').value='6';d.querySelector('[data-test-review]').click();return 'review';");
      await pollValue("actual relay review", () => inner("return d.querySelector('[data-test-state]')?.textContent||'';"),
        value => value.includes("Simulation test ready"), 15_000);
      await inner("d.querySelector('[data-test-approve]').click();return 'start';");
      await pollValue("actual relay running", () => inner("return d.querySelector('[data-test-state]')?.textContent||'';"),
        value => value.includes("Test running"), 15_000);
      const relayTerminal = await pollValue("actual relay terminal", () => inner("return d.querySelector('[data-relay-tuning-status]')?.textContent||'';"),
        value => /measured feedback-relay cycles.*Calculate relay candidate|unavailable/i.test(value), 30_000);
      assert.match(relayTerminal, /measured feedback-relay cycles.*Calculate relay candidate/i,
        "the reviewed amplitude and duration must produce repeatable, sufficiently sampled cycles");
      assert.strictEqual(await inner("return String(d.querySelector('[data-relay-candidate]')?.hidden);"), "true",
        "the terminal relay Test may qualify cycles but must not silently create controller settings");
      await inner("d.querySelector('[data-relay-calculate]').click();return 'calculate';");
      const calculated = await pollValue("actual relay candidate", () => inner("return JSON.stringify({status:d.querySelector('[data-relay-tuning-status]')?.textContent,kp:d.querySelector('[data-relay-kp]')?.textContent,candidateHidden:d.querySelector('[data-relay-candidate]')?.hidden});"), value => {
        try { const state = JSON.parse(value); return state.candidateHidden === false && /unapplied PI proposal/i.test(state.status) && Number(state.kp) > 0; }
        catch { return false; }
      }, 15_000);
      assert.ok(Number(JSON.parse(calculated).kp) > 0);
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RELAY_ACTUAL_SCREENSHOT?.trim();
      if (screenshot) {
        await inner("var card=d.querySelector('[data-relay-tuning]');var workspace=card?.closest('.workspace');if(workspace&&card)workspace.scrollTop=card.offsetTop-workspace.offsetTop;card?.scrollIntoView({block:'start'});return 'visible';");
        await sleep(300);
        await captureRenderedWorkbenchCdp(connection, screenshot);
      }
    } finally {
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
