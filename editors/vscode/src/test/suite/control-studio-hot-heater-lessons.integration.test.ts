import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { ControlStudioWorkflowResult } from "../../controlStudio/workflowResult";
import { displayEngineeringUnit, formatEngineeringNumber } from "../../controlStudio/chartContract";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, setWebviewValue, sleep } from "./plant-rendered-harness";
import { assertHotHeaterRecording, captureHotHeaterPlot, chooseHotHeaterSignal, clickHotHeaterControl, hotHeaterSelectedSignals, hotHeaterResponseRaster } from "./control-studio-hot-heater-helpers";
import { revealControlForCapture } from "./control-studio-capture-visibility";

suite("Control Studio hot PID heater lessons", () => {
  for (const sequence of [7, 8]) {
    const abortNavigation = process.env.TRUST_HOT_HEATER_ABORT_NAVIGATION === "1";
    const layoutOnly = process.env.TRUST_HOT_HEATER_LAYOUT_ONLY === "1";
    const tableInspection = process.env.TRUST_HOT_HEATER_TABLE_INSPECTION === "1";
    const behavior = abortNavigation ? "keeps Abort available across views and restores the native actuator"
      : tableInspection ? "opens and closes the native sample table with selected signals"
      : layoutOnly ? "reviews compact Test layout and requested trace contrast"
        : "selects diagnostics and preserves the complete saved native experiment";
    test(`lesson ${sequence} ${behavior}`, async function () {
      if (process.env.TRUST_HOT_HEATER_LESSON !== String(sequence)) this.skip();
      this.timeout(180000);
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
      const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
      const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
      assert.strictEqual(loop.controller.type, "Control.PID");
      assert.strictEqual(loop.signals.processVariable.address, "%IL0");
      assert.strictEqual(loop.signals.manipulatedVariable.address, "%IL8");
      const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
      const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      let opened = false;
      const failures: string[] = [];
      const reopenOnly = process.env.TRUST_HOT_HEATER_REOPEN_ONLY === "1";
      try {
        assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
        opened = true;
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
        const session = await attachControlStudio(connection, port, "", loop.id);
        const inner = (body: string) => evaluateInWebview(connection, session, body);
        const click = (selector: string) => clickHotHeaterControl(inner, selector);
        const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
        const capture = async (name: string) => {
          await vscode.commands.executeCommand("notifications.clearAll"); await sleep(250);
          await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
          await captureHotHeaterPlot(inner, path.join(captures, name));
        };
        if (!reopenOnly) await pollValue("hot measured reset", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return JSON.stringify({pv:d.querySelector('[data-control-studio-pv]')?.textContent,mv:d.querySelector('[data-control-studio-mv]')?.textContent});");
        }, value => { const state = JSON.parse(value); return Number.parseFloat(state.pv) === 35 && Number.parseFloat(state.mv) === 75; }, 15000);
        await stage("Monitor");
        await click('[data-plot-tab="signals"]');
        await pollValue("current signal declarations", () => inner("return String(d.querySelectorAll('[data-control-signals] .signal-row').length);"), value => Number(value) >= 8, 15000);
        const auxiliary = ["requested-power", "stored-correction", "paired-requested-power", "unavailable-power", "controller-command",
          ...(sequence === 8 ? ["tracking-correction"] : [])];
        for (const id of auxiliary) {
          const binding = loop.signals.additional.find((signal: { id: string }) => signal.id === id);
          assert.ok(binding, `published lesson contains ${id}`);
          await chooseHotHeaterSignal(inner, binding.symbol, true);
        }
        const selected = ["sp", "pv", "mv", ...auxiliary];
        for (const id of selected) assert.ok((await hotHeaterSelectedSignals(inner)).includes(id));
        await capture("01-selected-diagnostics");
        if (layoutOnly) {
          const visibleFigureActions = () => inner(`return JSON.stringify(
            [...d.querySelectorAll('.plot-actions [data-chart-action]')]
              .filter(button=>!button.closest('[hidden]')&&button.getBoundingClientRect().width>0)
              .map(button=>button.dataset.chartAction));`);
          assert.deepStrictEqual(JSON.parse(await visibleFigureActions()), [],
            "Signals must not show actions that operate on a hidden plot");
          await click('[data-plot-tab="events"]');
          assert.deepStrictEqual(JSON.parse(await visibleFigureActions()), [],
            "Events must not show actions that operate on a hidden plot");
          await capture("01b-events-list");
          await click('[data-plot-tab="response"]');
          const responseActions = JSON.parse(await visibleFigureActions()) as string[];
          for (const action of ["inspect", "zoom-out", "zoom-in", "reset", "expand", "export-figure"]) {
            assert.ok(responseActions.includes(action), `Response restores ${action}`);
          }
          const idleHeader = JSON.parse(await inner(`return JSON.stringify({
            unsupported:d.querySelector('[data-control-studio-automatic-tune]')!==null,
            height:d.querySelector('.header').getBoundingClientRect().height
          });`));
          assert.strictEqual(idleHeader.unsupported, false,
            "the fixed lesson controller must not offer an unsupported Automatic Tune action");
          assert.ok(idleHeader.height <= 1, "an empty action header must not reserve a blank row above navigation");
        }
        await click('[data-plot-tab="response"]');
        if (tableInspection) {
          assert.strictEqual(await inner("return String(!!d.querySelector('[data-chart-action=table]'));"), "true",
            "Response must expose its accessible Samples table");
          await click('[data-chart-action="table"]');
          const table = JSON.parse(await inner(`
            const table=d.querySelector('[data-chart-table]'),rect=table.getBoundingClientRect();
            return JSON.stringify({visible:!table.hidden&&rect.height>0,expanded:d.querySelector('[data-chart-action=table]').getAttribute('aria-expanded'),
              headings:[...table.querySelectorAll('th')].map(cell=>cell.textContent),
              last:[...table.querySelector('tbody').lastElementChild.children].map(cell=>cell.textContent),
              bottom:rect.bottom,footer:d.querySelector('.values').getBoundingClientRect().top});
          `));
          assert.strictEqual(table.visible, true); assert.strictEqual(table.expanded, "true");
          assert.strictEqual(table.headings.length, selected.length + 1);
          assert.deepStrictEqual(table.headings.slice(0, 4), ["Elapsed time (s)", "SP (°C)", "PV (°C)", "MV (%)"]);
          assert.match(table.last[2], /^35.*Good$/); assert.match(table.last[3], /^75.*Good$/);
          assert.ok(table.bottom <= table.footer, "the table must fit above the live-value strip");
          await capture("02a-samples-all-signals");
          const requestedAddress = loop.signals.additional.find((signal: { id: string }) => signal.id === "requested-power").address;
          await vscode.commands.executeCommand("trust-lsp.debug.io.force", { address: requestedAddress, value: "110" });
          try {
            await pollValue("forced native diagnostic in the sample table", async () => {
              await runtimeLifecycleService.requestIoState();
              return inner("return d.querySelector('[data-chart-table] tbody').lastElementChild.children[4].textContent;");
            }, value => /^110.*Forced$/.test(value), 5000);
            await inner("const table=d.querySelector('[data-chart-table]');table.scrollTop=table.scrollHeight;return 'scrolled to latest retained reading';");
            await capture("02a-forced-sample-quality");
          } finally { await vscode.commands.executeCommand("trust-lsp.debug.io.release", { address: requestedAddress }); }
          await pollValue("released native diagnostic", async () => {
            await runtimeLifecycleService.requestIoState();
            return inner("return d.querySelector('[data-chart-table] tbody').lastElementChild.children[4].textContent;");
          }, value => /^75.*Good$/.test(value), 5000);
          await click('[data-chart-action="table"]');
          assert.strictEqual(await inner("return String(d.querySelector('[data-chart-table]').hidden);"), "true");
          await click('[data-chart-action="table"]');
          await click('[data-plot-tab="signals"]');
          assert.strictEqual(await inner("return String(d.querySelector('[data-chart-table]').hidden);"), "true",
            "changing views closes the old plot's table");
          for (const id of auxiliary) await chooseHotHeaterSignal(inner, loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol, false);
          await click('[data-plot-tab="response"]');
          assert.strictEqual(await inner("return d.querySelector('[data-chart-action=table]').getAttribute('aria-expanded');"), "false");
          await click('[data-chart-action="table"]');
          assert.strictEqual(await inner("return String(d.querySelectorAll('[data-chart-table] th').length);"), "4");
          await capture("02b-samples-primary-only");
          fs.writeFileSync(path.join(captures, "sample-table.json"), JSON.stringify(table, null, 2));
          return;
        }
        if (!reopenOnly) {
        await capture("02-hot-reset");
        await stage("Test");
        assert.strictEqual(await inner("return d.querySelector('#test-kind').value;"), "observation");
        await revealControlForCapture(inner, "#test-duration");
        await setWebviewValue(inner, "#test-duration", "70.1");
        await click("[data-test-review]");
        await pollValue("reviewed hot experiment", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 15000);
        await capture("03-reviewed-test");
        if (layoutOnly) {
          const layout = JSON.parse(await inner(`
            const signal=d.querySelector('[data-test-signal]');
            const range=d.createRange();range.selectNodeContents(signal);
            const text=range.getBoundingClientRect(),metric=signal.closest('.metric').getBoundingClientRect();
            const rootStyle=d.defaultView.getComputedStyle(d.documentElement);
            const canvas=d.querySelector('[data-control-studio-chart] canvas');
            const opacity=[];for(let node=canvas;node;node=node.parentElement){
              const style=d.defaultView.getComputedStyle(node);opacity.push({tag:node.tagName,class:node.className,opacity:style.opacity,filter:style.filter});
            }
            return JSON.stringify({textRight:text.right,metricRight:metric.right,
              colors:Object.fromEntries(['--vscode-charts-orange','--vscode-charts-purple','--vscode-charts-yellow','--vscode-editor-background','--vscode-editor-foreground'].map(name=>[name,rootStyle.getPropertyValue(name).trim()])),opacity});
          `));
          fs.writeFileSync(path.join(captures, "test-layout.json"), JSON.stringify(layout, null, 2));
          const visibleTest = JSON.parse(await inner(`
            const pane=d.querySelector('[data-workflow-pane="Test"]');
            return JSON.stringify({fields:[...pane.querySelectorAll('input[id],select[id]')]
              .filter(field=>field.getBoundingClientRect().width>0&&!field.closest('[hidden]')).map(field=>field.id),
              review:pane.querySelector('[data-test-preview]').innerText});
          `));
          assert.deepStrictEqual(visibleTest.fields, ["test-duration"],
            "a lesson-owned scenario must not show unusable experiment-kind or amplitude fields");
          assert.doesNotMatch(visibleTest.review, /authority|Deterministic seed/i,
            "the lesson review must omit internal authority units and an unused excitation seed");
          assert.ok(layout.textRight <= layout.metricRight - 4,
            "reviewed Signal text must remain inside its metric without covering the requested bound");
          await stage("Monitor"); await click('[data-plot-tab="signals"]');
          for (const id of auxiliary.filter(id => id !== "requested-power")) {
            await chooseHotHeaterSignal(inner, loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol, false);
          }
          await chooseHotHeaterSignal(inner, "P1.AppliedHeaterPower", false);
          await click('[data-plot-tab="response"]');
          await capture("10-requested-stroke-contrast");
          const opaquePixels = Number(await inner(`
            const probe=d.createElement('canvas');probe.width=probe.height=1;
            const c=probe.getContext('2d');c.fillStyle=d.defaultView.getComputedStyle(d.documentElement).getPropertyValue('--vscode-charts-orange').trim();c.fillRect(0,0,1,1);
            const rgb=c.getImageData(0,0,1,1).data;
            let count=0;for(const canvas of d.querySelectorAll('[data-control-studio-chart] canvas')){
              const pixels=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;
              for(let y=70;y<canvas.height-40;y++)for(let x=70;x<canvas.width-70;x++){
                const i=(y*canvas.width+x)*4;
                if(pixels[i+3]>250&&Math.abs(pixels[i]-rgb[0])<=2&&Math.abs(pixels[i+1]-rgb[1])<=2&&Math.abs(pixels[i+2]-rgb[2])<=2)count++;
              }
            }return String(count);
          `));
          fs.writeFileSync(path.join(captures, "stroke-contrast.json"), JSON.stringify({ opaquePixels }, null, 2));
          assert.ok(opaquePixels > 100, "the selected requested-output trace must be opaque in the actual canvas interior");
          return;
        }
        await click("[data-test-approve]");
        await pollValue("hot experiment running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Test running/.test(value), 10000);
        await pollValue("initial 45 degree request", () => inner("return d.querySelector('[data-control-studio-sp]').textContent;"), value => Number.parseFloat(value) === 45, 5000);
        await stage("Monitor");
        if (abortNavigation) {
          const assertAbortVisible = async (context: string) => {
            const state = JSON.parse(await inner(`
              const buttons=[...d.querySelectorAll('[data-test-abort]')],button=buttons[0],rect=button?.getBoundingClientRect();
              return JSON.stringify({count:buttons.length,enabled:button&&!button.disabled,
                visible:!!rect&&rect.width>0&&rect.height>0&&rect.top>=0&&rect.bottom<=d.defaultView.innerHeight&&!button.closest('[hidden]')});
            `));
            assert.strictEqual(state.count, 1, "one Abort action owns the active experiment");
            assert.strictEqual(state.visible && state.enabled, true, `Abort test must remain visible and enabled ${context}`);
          };
          await capture("04-running-monitor-before-abort");
          await assertAbortVisible("after switching to Monitor");
          await runtimeLifecycleService.requestIoState();
          await sleep(300);
          const recordingState = JSON.parse(await inner(`return JSON.stringify({
            text:d.querySelector('[data-control-studio-recording-evidence]').textContent,
            testStatus:d.querySelector('[data-control-studio-mode][aria-label^="Test,"]').dataset.status,
            manualVisible:[...d.querySelectorAll('[data-control-studio-record-start],[data-control-studio-record-stop]')].some(button=>button.getBoundingClientRect().width>0&&!button.closest('[hidden]'))
          });`));
          assert.match(recordingState.text, /Test recording in progress/,
            "Monitor must identify the active Test recording after fresh live observations");
          assert.strictEqual(recordingState.testStatus, "in-progress");
          assert.strictEqual(recordingState.manualVisible, false,
            "manual recording controls must not compete with the Test's Abort action");
          await click('[data-plot-tab="signals"]');
          await assertAbortVisible("while selecting Signals");
          await click('[data-plot-tab="response"]');
          await click('[data-chart-action="expand"]');
          await capture("05-running-expanded-before-abort");
          await assertAbortVisible("with the plot expanded");
          await click('[data-test-abort]');
          const terminal = await pollValue("native abort restoration", () => inner("return d.querySelector('[data-test-state]').textContent;"),
            value => /Aborted/.test(value) && /confirmed restored/.test(value), 15000);
          await pollValue("actual applied zero after abort", async () => {
            await runtimeLifecycleService.requestIoState();
            return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
          }, value => Number.parseFloat(value) === 0, 5000);
          assert.strictEqual(await inner("return String(d.querySelector('[data-test-abort]').hidden);"), "true");
          await click('[data-chart-action="expand"]');
          await stage("Test");
          await capture("06-aborted-restored");
          fs.writeFileSync(path.join(captures, "abort-navigation.json"), JSON.stringify({ sequence, terminal,
            appliedMv: 0, acquisition: "aborted by the visible action; not the full 70.1-second experiment" }, null, 2));
          return;
        }
        await pollValue("return to the feasible target", () => inner("return d.querySelector('[data-control-studio-sp]').textContent;"), value => Number.parseFloat(value) === 35, 18000);
        await capture("04-live-return-curves");
        await stage("Test");
        await pollValue("complete 70.1-second native Test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 85000);
        await stage("Monitor");
        await pollValue("native complete recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => /Recording retained/.test(value), 15000);
        await pollValue("following live applied-zero publication", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
        }, value => Number.parseFloat(value) === 0, 5000);
        await capture("05-complete-response");
        }
        // Observe actual host messages without replacing any browser API or
        // manufacturing a result. The listener disappears with this owned panel.
        await inner("const root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',event=>{const m=event.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened'&&m.loopId===root.dataset.loopId)root.dataset.hotHeaterOpenedResult=JSON.stringify(m.payload?.result);});return 'observing actual Open result';");
        await stage("Validate");
        if (!reopenOnly) {
        await click("[data-result-save]");
        await pollValue("saved native result", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /Saved revision/.test(value), 15000);
        }
        // Make persistence observable: change a visible live selection, then
        // require Open to restore the saved selection and its native data.
        await stage("Monitor"); await click('[data-plot-tab="signals"]');
        await chooseHotHeaterSignal(inner, "P1.RequestedHeaterPower", false);
        assert.ok(!(await hotHeaterSelectedSignals(inner)).includes("requested-power"));
        await stage("Validate");
        const savedDetails = "[data-workflow-pane=Validate] details:has([data-result-history]) > summary";
        if (await inner("return String(d.querySelector('[data-result-history]').closest('details').open);") !== "true") await click(savedDetails);
        await revealControlForCapture(inner, "[data-result-history]");
        const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(option=>option.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
        await setWebviewValue(inner, "[data-result-history]", revision);
        await click("[data-result-open]");
        const result = JSON.parse(await pollValue("opened saved native result", () => inner("return d.querySelector('[data-control-studio]').dataset.hotHeaterOpenedResult??'';"), value => value.length > 0, 15000)) as ControlStudioWorkflowResult;
        fs.writeFileSync(path.join(captures, "native-result.json"), JSON.stringify(result, null, 2));
        try {
          fs.writeFileSync(path.join(captures, "physical-comparison.json"), JSON.stringify(assertHotHeaterRecording(result, project, sequence, selected), null, 2));
        } catch (error) { failures.push(`Native physical/reference check: ${String(error)}`); }
        await stage("Monitor"); await click('[data-plot-tab="signals"]');
        for (const id of selected) assert.ok((await hotHeaterSelectedSignals(inner)).includes(id), `Open restores ${id}`);
        await capture("06-reopened-selections");
        const lastRecorded = result.recording.rawSamples[result.recording.rawSamples.length - 1];
        const expectedRows = [
          { symbol: loop.signals.setpoint.symbol, value: lastRecorded.sp, unit: result.units.sp, quality: lastRecorded.quality },
          { symbol: loop.signals.processVariable.symbol, value: lastRecorded.pv, unit: result.units.pv, quality: lastRecorded.quality },
          { symbol: loop.signals.manipulatedVariable.symbol, value: lastRecorded.appliedMv, unit: result.units.mv, quality: lastRecorded.quality },
          ...(lastRecorded.additional ?? []).map(signal => ({ ...signal,
            symbol: loop.signals.additional.find((binding: { id: string }) => binding.id === signal.id).symbol })),
        ];
        const visibleRows = JSON.parse(await inner(`return JSON.stringify([...d.querySelectorAll('[data-control-signals] .signal-row')].map(row=>({
          identity:row.children[2].textContent.trim(),reading:row.children[3].textContent.trim(),quality:row.children[4].textContent.trim()
        })));`)) as Array<{ identity: string; reading: string; quality: string }>;
        fs.writeFileSync(path.join(captures, "reopened-signal-values.json"), JSON.stringify({ expectedRows, visibleRows }, null, 2));
        for (const expected of expectedRows) {
          const row = visibleRows.find(row => row.identity.endsWith(expected.symbol));
          assert.ok(row, `the saved Signals view must retain ${expected.symbol}`);
          assert.strictEqual(row.reading,
            `${formatEngineeringNumber(expected.value)} ${displayEngineeringUnit(expected.unit)}`.trim(),
            `Signals must show the opened recording for ${expected.symbol}, not newer live values`);
          assert.strictEqual(row.quality, expected.quality, `retain recorded quality for ${expected.symbol}`);
        }
        for (const id of auxiliary) {
          const symbol = loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol;
          const checked = await inner(`const row=[...d.querySelectorAll('[data-control-signals] .signal-row')].find(row=>[...row.querySelectorAll('.signal-secondary')].some(label=>label.textContent.trim().endsWith(${JSON.stringify(symbol)})));return String(row?.querySelector('input').checked);`);
          if (checked !== "true") failures.push(`The reopened visible checkbox retains ${symbol}: expected true, got ${checked}.`);
        }
        await click('[data-plot-tab="response"]');
        await capture("07-reopened-curves");
        const showSignal = async (symbol: string, selected: boolean) => {
          await click('[data-plot-tab="signals"]');
          await chooseHotHeaterSignal(inner, symbol, selected);
          await click('[data-plot-tab="response"]');
        };
        for (const id of auxiliary) {
          const symbol = loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol;
          // Visible on/off transitions establish the isolation state even when
          // the captured restore defect left a stale checkbox presentation.
          await showSignal(symbol, true); await showSignal(symbol, false);
        }
        await showSignal("P1.AppliedHeaterPower", false);
        const primaryOnly = await hotHeaterResponseRaster(inner);
        assert.strictEqual(await hotHeaterResponseRaster(inner), primaryOnly, "reopened evidence must be stable before measuring curve changes");
        await showSignal("P1.AppliedHeaterPower", true);
        assert.notStrictEqual(await hotHeaterResponseRaster(inner), primaryOnly, "positive control: the real MV checkbox changes plotted pixels");
        await showSignal("P1.AppliedHeaterPower", false);
        const curveEvidence: Array<{ id: string; before: string; after: string; changed: boolean }> = [];
        for (const id of auxiliary) {
          const symbol = loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol;
          const before = await hotHeaterResponseRaster(inner);
          await showSignal(symbol, true);
          const after = await hotHeaterResponseRaster(inner);
          const changed = after !== before;
          curveEvidence.push({ id, before, after, changed });
          if (!changed) failures.push(`Selecting ${id} through visible Monitor controls changed no actual Response plot pixels; the MV positive control did.`);
          await capture(`08-isolated-${id}`);
          await showSignal(symbol, false);
        }
        fs.writeFileSync(path.join(captures, "reopened-curves.json"), JSON.stringify(curveEvidence, null, 2));
        await showSignal("P1.AppliedHeaterPower", true);
        for (const id of auxiliary) await showSignal(loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol, true);
        await capture("09-restored-selected-curves");
        fs.writeFileSync(path.join(captures, "journey-findings.json"), JSON.stringify({ sequence, failures }, null, 2));
        assert.deepStrictEqual(failures, [], "The published hot lesson requires actual selected auxiliary curves and preserved native data.");
      } finally {
        await runtimeLifecycleService.stopRuntime();
        if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
        connection.close();
      }
    });
  }
});
