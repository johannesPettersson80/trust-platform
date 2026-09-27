import { exerciseAutomaticTuneInteractions } from "./control-studio-automatic-tune-interactions";
import { exerciseImportedTuning } from "./control-studio-imported-tuning-interactions";
import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { exerciseEngineeringPlotViews } from "./control-studio-engineering-view-interactions";
import { exerciseNativeCandidateExport } from "./control-studio-native-export-interaction";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, captureRenderedWorkbench, type CdpConnection } from "./plant-rendered-harness";

suite("Control Studio pinned plot interactions", () => {
  test("selects two real tuned predictions without changing the current candidate", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_PINNED_PLOTS !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const messages = require("../../controlStudio/messages");
    const decode = messages.decodeEngineeringCommand;
    let commands = 0;
    messages.decodeEngineeringCommand = (...args: unknown[]) => { commands++; return decode(...args); };
    let connection: CdpConnection | undefined;
    try {
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
      connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection!, session, body);
      const capture = async (name: string) => {
        await new Promise(resolve => setTimeout(resolve, 250));
        captureRenderedWorkbench(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, `-${name}.png`));
      };
      const click = async (selector: string) => {
        assert.strictEqual(await inner(`const b=d.querySelector(${JSON.stringify(selector)});b?.scrollIntoView({block:'center'});const r=b?.getBoundingClientRect();if(!b||b.disabled||!r?.width||!r.height)return 'unavailable';b.click();return 'clicked';`), "clicked", selector);
      };
      await pollValue("handlers", () => inner("return d.querySelector('[data-finite-search]').dataset.ready;"), value => value === "true", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-chart-action=pin]').disabled);"), "true", "an empty study cannot be pinned");
      await exerciseAutomaticTuneInteractions(inner, click, capture);
      await exerciseNativeCandidateExport(inner, project, capture);
      await click("[data-auto-tune-close]");
      await inner("d.defaultView.addEventListener('message',e=>{const r=d.querySelector('[data-control-studio]');if(e.data?.kind==='control-studio.engineering.result'&&e.data.stage==='tune'&&e.data.editorSessionId===r.dataset.editorSessionId)d.defaultView.__legacyTune=e.data.payload;});[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Tune').click();return 'listen';");
      const gains: number[] = [];
      for (const [method, objective] of [["SIMC", "robust"], ["SIMC", "balanced"], ["SIMC", "fast"], ["AMIGO", "balanced"]]) {
        await inner(`d.defaultView.__legacyTune=undefined;const m=d.querySelector('#tune-method');m.value='${method}';m.dispatchEvent(new Event('change'));if('${method}'==='SIMC'){const o=d.querySelector('#tune-objective');o.value='${objective}';o.dispatchEvent(new Event('change'));}return 'method';`);
        await click("[data-tune-generate]");
        const result = JSON.parse(await pollValue(`${method} ${objective} candidate`, () => inner("return JSON.stringify(d.defaultView.__legacyTune??{});"), value => ["complete", "failed", "unavailable"].includes(JSON.parse(value).status), 15000));
        assert.strictEqual(result.status, "complete", result.reason);
        assert.strictEqual(result.candidate.source, method);
        assert.ok(Number.isFinite(result.candidate.parameters.proportionalGain));
        if (method === "SIMC") gains.push(result.candidate.parameters.proportionalGain);
        await capture(`method-${method}-${objective}`);
      }
      assert.ok(gains[0] < gains[1] && gains[1] < gains[2], "SIMC response preference must change the actual generated gain");
      await exerciseImportedTuning(connection, session, inner, project, click, capture);
      await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Tune').click();const method=d.querySelector('#tune-method');method.value='Manual';method.dispatchEvent(new Event('change'));return 'manual';");
      const tune = async (gain: number) => {
        await inner(`d.defaultView.__legacyTune=undefined;d.querySelector('[data-pid-setting=proportionalGain]').value='${gain}';return 'set gain';`);
        await click("[data-tune-generate]");
        await pollValue("real tuned candidate", () => inner("return JSON.stringify(d.defaultView.__legacyTune??{});"), value => { const r=JSON.parse(value);return r.status==='complete'&&r.candidate?.source==='Manual'&&r.candidate.parameters.proportionalGain===gain; }, 15000);
        await click("[data-chart-action=pin]");
      };
      await tune(1);
      await tune(2);
      await exerciseEngineeringPlotViews(inner, capture);
      const pins = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-pinned-select]')].map(b=>b.dataset.pinnedSelect));")) as string[];
      assert.strictEqual(pins.length, 2);
      assert.strictEqual(await inner("return JSON.parse(d.querySelector('[data-control-studio]').dataset.controlChartSession).boundary;"), `${loop.id}:${loop.revision}`, "saved chart sessions must retain the host's loop scope contract");
      assert.strictEqual(await inner("const row=d.querySelector('[data-control-designs]').getBoundingClientRect();return String([...d.querySelectorAll('[data-pinned-select]')].every(b=>{const r=b.getBoundingClientRect();return r.top>=row.top-1&&r.bottom<=row.bottom+1;}));"), "true", "pin actions must not be vertically clipped inside the comparison row: " + await inner("const row=d.querySelector('[data-control-designs]');return JSON.stringify({row:row.getBoundingClientRect().toJSON(),style:d.defaultView.getComputedStyle(row).cssText,host:d.defaultView.getComputedStyle(d.querySelector('[data-pinned-designs]')).display,buttons:[...d.querySelectorAll('[data-pinned-select]')].map(b=>b.getBoundingClientRect().toJSON())});"));
      const current = await inner("return d.querySelector('[data-active-design]').textContent;");
      const before = commands;
      const pixels = () => inner("return [...d.querySelectorAll('[data-engineering-time] canvas')].map(c=>c.toDataURL()).join('|');");
      await click(`[data-pinned-select="${pins[0]}"]`);
      await capture("first-pin");
      const first = await pixels();
      await click(`[data-pinned-select="${pins[1]}"]`);
      await pollValue("different cached prediction", pixels, value => value !== first, 3000);
      await capture("second-pin");
      await click("[data-pinned-return=current]");
      assert.strictEqual(await inner("return d.querySelector('[data-active-design]').textContent;"), current);
      await click(`[data-pinned-remove="${pins[0]}"]`);
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-pinned-select]').length);"), "1");
      await capture("pin-removed");
      assert.strictEqual(commands, before, "read-only comparisons must not issue host engineering commands");
      const shortcut = async (redo = false) => {
        await connection!.send("Input.dispatchKeyEvent", { type: "keyDown", key: redo ? "Z" : "z", code: "KeyZ", modifiers: redo ? 10 : 2, windowsVirtualKeyCode: 90 }, session);
        await connection!.send("Input.dispatchKeyEvent", { type: "keyUp", key: redo ? "Z" : "z", code: "KeyZ", modifiers: redo ? 10 : 2, windowsVirtualKeyCode: 90 }, session);
      };
      await inner("d.querySelector('[data-chart-action=pin]').focus();return 'focus';");
      await shortcut();
      await pollValue("keyboard undo candidate", () => inner("return d.querySelector('[data-active-design]').textContent;"), value => value.includes("Kp 1"), 10000);
      await capture("keyboard-undo");
      await shortcut(true);
      await pollValue("keyboard redo candidate", () => inner("return d.querySelector('[data-active-design]').textContent;"), value => value.includes("Kp 2"), 10000);
      await capture("keyboard-redo");
      await click("[data-chart-action=undo]");
      await pollValue("toolbar undo candidate", () => inner("return d.querySelector('[data-active-design]').textContent;"), value => value.includes("Kp 1"), 10000);
      await capture("toolbar-undo");
      await click("[data-chart-action=redo]");
      await pollValue("toolbar redo candidate", () => inner("return d.querySelector('[data-active-design]').textContent;"), value => value.includes("Kp 2"), 10000);
      await capture("toolbar-redo");
      const originalText = await inner("const t=d.querySelector('[data-search-request]');t.focus();t.select();return t.value;");
      await connection!.send("Input.insertText", { text: "field edit" }, session);
      await shortcut();
      await pollValue("Ctrl+Z inside a field retains native text undo", () => inner("return d.querySelector('[data-search-request]').value;"), value => value === originalText, 3000);
      assert.match(await inner("return d.querySelector('[data-active-design]').textContent;"), /Kp 2/);
      // Exercise the chart's scoped application-result boundary without writing runtime values.
      await inner("const r=d.querySelector('[data-control-studio]');d.defaultView.dispatchEvent(new MessageEvent('message',{data:{kind:'control-studio.application.result',phase:'applied',projectId:r.dataset.projectId,editorSessionId:r.dataset.editorSessionId,loopId:r.dataset.loopId,expectedLoopRevision:r.dataset.loopRevision}}));return 'application result';");
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-pinned-select]')].every(b=>b.disabled)&&d.querySelector('[data-chart-action=undo]').disabled);"), "true", "an observed application ends editable comparison history and fences old pins");
    } finally {
      messages.decodeEngineeringCommand = decode;
      connection?.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
