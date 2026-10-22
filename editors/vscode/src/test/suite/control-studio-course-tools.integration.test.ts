import * as assert from "assert";
import * as vscode from "vscode";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue } from "./plant-rendered-harness";

type Inner = (body: string) => Promise<string>;
async function choose(inner: Inner, name: string): Promise<void> {
  // The engine answers the tools that apply to the model, so the option is awaited. An open list is closed and opened again, so it lists the current tools.
  const state = JSON.parse(await pollValue(`${name} reachable`, () => inner(`d.querySelector('[data-engineer-stage-tab=tune]').click();const list=d.querySelector('[data-studio-analyze-list]');if(!list.hidden)d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze]').click();const option=d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool="${name}"]');return JSON.stringify({reachable:!!option&&!option.disabled&&!option.hidden,tools:[...d.querySelectorAll('[data-studio-analyze-list] [data-studio-analysis-tool]')].map(o=>o.dataset.studioAnalysisTool),analyzeHidden:d.querySelector('[data-studio-analyze]').hidden,declared:Boolean(JSON.parse(d.querySelector('[data-studio-three-page]').dataset.studioLoop??'null')?.analysis)});`), value => JSON.parse(value).reachable === true, 15000));
  assert.strictEqual(state.reachable, true, `${name} is reachable: ${JSON.stringify(state)}`);
  await inner(`d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool="${name}"]').click();d.defaultView.__courseEvents=[];return 'selected';`);
}
async function listen(inner: Inner): Promise<void> {
  // The engine's analysis messages (`control-studio.analysis.*`, engine/routes/analyzeRoute.ts) reach the page as they are sent.
  await inner("d.defaultView.__courseEvents=[];d.defaultView.addEventListener('message',e=>{if(typeof e.data?.kind==='string'&&e.data.kind.startsWith('control-studio.analysis.'))d.defaultView.__courseEvents.push(e.data);});return 'listening';");
}
/**
 * The envelope of the calculation of `tool` that the engine finished (specification 35, B9.9): a quick tool's answer, or the update
 * that finishes an operation. The tool's answer is `result[tool]`; the model and the controller are named by their revisions.
 */
async function answerOf(inner: Inner, tool: string): Promise<any> {
  const envelope = JSON.parse(await pollValue(tool, () => inner(`return JSON.stringify(d.defaultView.__courseEvents.filter(e=>e.envelope?.tool==='${tool}'&&(e.kind==='control-studio.analysis.answer'||e.kind==='control-studio.analysis.update'&&e.state==='finished')).map(e=>e.envelope));`), value => JSON.parse(value).length > 0, 15000))[0];
  assert.ok(envelope.result, `${tool} calculates: ${JSON.stringify(envelope.failure)}`);
  return envelope;
}
async function calculate(inner: Inner, tool: string): Promise<any> {
  await inner("d.querySelector('[data-engineer-tune]').click();return 'calculate';");
  return answerOf(inner, tool);
}
/**
 * A declared analysis and a loop's channels are served on a loop without a controller law (C12, specification 35): the loop is an
 * open-loop observation of the plant. An open loop declares no setpoint (C12, Setup; the engine refuses a loop that does), so the
 * fixture's setpoint goes with its law.
 */
function observation(loop: any): void {
  loop.topology = "open-loop"; loop.controller.design = "project"; delete loop.controller.law; delete loop.controller.pidInitialization;
  delete loop.signals.setpoint;
}
/**
 * Opens a plant model through the Identify page's Supplied plant model method, as the engineer does (B3.7). The engine checks the
 * model against the import contract and keeps it as the study's model: a model it refuses leaves the equation empty and shows its
 * sentence, which the failure of the wait names.
 */
async function supply(inner: Inner, model: Record<string, unknown>): Promise<void> {
  model.evidence = ["analytical test fixture"];
  await inner("d.querySelector('[data-engineer-stage-tab=model]').click();const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
  await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(JSON.stringify(model))}],'model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'import';`);
  await pollValue("selected model", () => inner("return JSON.stringify({equation:d.querySelector('[data-studio-model-equation]')?.textContent??'',message:d.querySelector('[data-studio-message=model]')?.textContent??''});"),
    value => (JSON.parse(value) as { equation: string }).equation.length > 0, 10000);
}
function declared(loop: any): void {
  loop.analysis = {
    plant: { kind: "transfer-function", numerator: [1], denominator: [1, 3, 3, 10], delaySeconds: 0, inputUnit: loop.signals.manipulatedVariable.unit, outputUnit: loop.signals.processVariable.unit, revision: "course-tool-declaration", source: "provided", validation: "validated" },
    controller: { numerator: [{ parameter: "Kp" }, { parameter: "Kp" }, { parameter: "Kp", scale: 4 }], denominator: [1], parameters: { Kp: { value: 2, minimum: -20, maximum: 20, unit: `${loop.signals.manipulatedVariable.unit}/${loop.signals.processVariable.unit}` } } },
    rootLocus: { parameter: "Kp", selectedValue: 2, values: [0, 1, 2] },
  };
}
suite("Control Studio course tools in three pages", () => {
  test("exposes the existing recording export through the response context menu", async () => {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp")!;
    await extension.activate();
    assert.ok((await vscode.commands.getCommands(true)).includes("trust-lsp.controlStudio.exportRecording"),
      "A retained native recording must have a reachable export action");
    assert.ok(extension.packageJSON.contributes.menus["webview/context"].some((item: any) =>
      item.command === "trust-lsp.controlStudio.exportRecording" && item.when.includes("trust-control-studio")),
      "Recording export belongs in the response context menu");
  });
  test("calculates and restores exact Routh regions from the visible parameter field", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip(); this.timeout(60000);
    await withStudio(async (inner, reopen, capture) => {
      await listen(inner); await choose(inner, "routh");
      await inner("const field=d.querySelector('[data-analysis-parameter-value]');field.value='2';field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'gain';");
      const routh = (await calculate(inner, "routh")).result.routh;
      assert.strictEqual(routh.selectedValue, 2); assert.strictEqual(routh.interval.status, "qualified");
      assert.ok(routh.interval.intervals.length > 1);
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-analysis-plot]').checkVisibility());"), "false", "An algebraic result does not reserve blank plot space");
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-analysis-table-details]').open);"), "false");
      await capture("routh-regions");
      await inner("d.querySelector('[data-three-page-analysis-table-details] summary').click();return 'details';");
      assert.match(await inner("return d.querySelector('[data-three-page-analysis-table]').textContent;"), /Boundary polynomial/);
      const layout = JSON.parse(await inner("const view=d.defaultView;const tables=[...d.querySelectorAll('[data-three-page-analysis-table] table')];const touching=[...d.querySelectorAll('[data-three-page-analysis-table] :is(th,td)')].filter(cell=>parseFloat(view.getComputedStyle(cell).paddingRight)<12).map(cell=>cell.textContent);const gaps=tables.slice(1).map((table,index)=>table.getBoundingClientRect().top-tables[index].getBoundingClientRect().bottom);return JSON.stringify({tables:tables.length,touching,gaps});"));
      assert.ok(layout.tables >= 2, `the details show the Routh table and its boundary polynomials: ${layout.tables}`);
      assert.deepStrictEqual(layout.touching, [], "every Calculation-details column is separated from the next");
      assert.ok(layout.gaps.every((gap: number) => gap >= 8), `stacked Calculation-details tables are separated: ${layout.gaps}`);
      await capture("routh-details");
      await reopen(); await choose(inner, "routh");
      // Reopening calculates the reading again (B9.9), so its summary arrives after the choice.
      await pollValue("reopened Routh reading", () => inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;"), value => /Kp = 2/.test(value), 15000);
      assert.strictEqual(Boolean(vscode.debug.activeDebugSession), false, "Analysis must not start a simulator");
    }, { loop: loop => { observation(loop); declared(loop); } });
  });
  test("calculates channel frequency and declared loop plots using the selected model", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip(); this.timeout(60000);
    await withStudio(async (inner, _reopen, capture) => {
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      await supply(inner, { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, revision: "course-visible-frequency", source: "provided", validation: "validated", inputs: [{ id: "u", unit: loop.signals.manipulatedVariable.unit, operatingRange: loop.signals.manipulatedVariable.range }], outputs: [{ id: "y", unit: loop.signals.processVariable.unit, operatingRange: loop.signals.processVariable.range }] });
      await listen(inner); await choose(inner, "frequency");
      await inner("for(const [key,value] of Object.entries({'minimum-frequency':'0.1','maximum-frequency':'10','frequency-points':'32'})){const field=d.querySelector('[data-analysis-'+key+']');field.value=value;field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));}return 'grid';");
      // The model's revision is the envelope's `model.revision` (the engine names the model it analysed by its revision).
      const frequency = await calculate(inner, "frequency");
      assert.strictEqual(frequency.result.frequency.channels[0].points.length, 32);
      assert.strictEqual(frequency.model.revision, "course-visible-frequency");
      await capture("channel-frequency");
      await choose(inner, "feedback"); await calculate(inner, "feedback");
      for (const view of ["bode", "sensitivity", "nyquist"]) {
        await inner(`const field=d.querySelector('[data-analysis-feedback-view]');field.value='${view}';field.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'plot';`);
        assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-analysis-plot]').checkVisibility());"), "true");
        await capture(`loop-${view}`);
        if (view === "nyquist") assert.match(await inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;"), /stability|stable|unstable/i, "Nyquist must expose the existing stability conclusion or its inconclusive result");
        if (view === "nyquist") assert.ok(Number(await inner("let count=0;for(const canvas of d.querySelectorAll('[data-three-page-analysis-plot] canvas')){if(!canvas.width||!canvas.height)continue;const ctx=canvas.getContext('2d');if(!ctx)continue;const pixels=ctx.getImageData(Math.floor(canvas.width*.2),Math.floor(canvas.height*.12),Math.floor(canvas.width*.75),Math.floor(canvas.height*.32)).data;for(let i=0;i<pixels.length;i+=4)if(Math.max(pixels[i],pixels[i+1],pixels[i+2])>150&&Math.max(pixels[i],pixels[i+1],pixels[i+2])-Math.min(pixels[i],pixels[i+1],pixels[i+2])>70)count++;}return String(count);")) > 6, "The real-model Nyquist view must show its negative-frequency conjugate branch above the real axis, below the legend");
      }
      assert.strictEqual(Boolean(vscode.debug.activeDebugSession), false, "Analysis must not start a simulator");
    }, { loop: loop => { observation(loop); declared(loop); loop.analysis.controller.numerator = [{ parameter: "Kp" }]; } });
  });
  test("calculates multichannel state structure and RGA directly without a frequency detour", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip(); this.timeout(60000);
    await withStudio(async (inner, reopen, capture) => {
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const channel = (id: string, signal: any) => ({ id, unit: signal.unit, operatingRange: signal.range });
      await supply(inner, { kind: "continuous-state-space", revision: "course-visible-mimo", source: "provided", validation: "validated", delaySeconds: 0,
        inputs: [channel("u1", loop.signals.manipulatedVariable), channel("u2", loop.signals.additional[0])], outputs: [channel("y1", loop.signals.processVariable), channel("y2", loop.signals.additional[1])],
        states: [channel("x1", loop.signals.processVariable), channel("x2", loop.signals.additional[1])], a: [[-1, 0], [0, -2]], b: [[1, .2], [.4, 1]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]], operatingPoint: { inputs: [0, 0], outputs: [0, 0] } });
      await listen(inner); await choose(inner, "stateStructure");
      const state = await calculate(inner, "stateStructure");
      // The ranks are the engine's first sentence, "Controllability rank 2 of 2; observability rank 2 of 2." (model_views.rs), which the
      // result area shows; the matrices are `result.stateStructure.matrices`.
      assert.ok(state.result.stateStructure.summary.includes("Controllability rank 2 of 2; observability rank 2 of 2."), JSON.stringify(state.result.stateStructure.summary));
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-analysis-plot]').checkVisibility());"), "false");
      await capture("state-structure");
      await choose(inner, "relativeGain"); const rga = await calculate(inner, "relativeGain");
      // The array is qualified with its matrix (N4.5); a singular steady-state gain is `status: "unavailable"` instead.
      assert.strictEqual(rga.result.relativeGain.status.status, "qualified");
      assert.ok(Math.abs(rga.result.relativeGain.status.matrix[0][0] - 1 / .92) < 1e-8);
      assert.match(await inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;"), /u1.*u2.*y1.*y2/s);
      await capture("state-rga"); await reopen(); await choose(inner, "relativeGain");
      await pollValue("reopened relative gain array", () => inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;"), value => /u1.*u2.*y1.*y2/s.test(value), 15000);
      assert.strictEqual(Boolean(vscode.debug.activeDebugSession), false, "Analysis must not start a simulator");
    }, { loop: loop => {
      observation(loop);
      loop.signals.additional = [{ ...loop.signals.manipulatedVariable, id: "u2", channel: "u2", role: "MV", symbol: "P1.Mv2", address: "%QL128" }, { ...loop.signals.processVariable, id: "y2", channel: "y2", role: "PV", symbol: "P1.Pv2", address: "%IL128" }];
      const actuator = { kind: "continuous", minimum: loop.signals.manipulatedVariable.range.minimum, maximum: loop.signals.manipulatedVariable.range.maximum };
      loop.channels = { inputs: [{ modelChannelId: "u1", signalId: "manipulatedVariable", actuator }, { modelChannelId: "u2", signalId: "u2", actuator }], outputs: [{ modelChannelId: "y1", signalId: "processVariable" }, { modelChannelId: "y2", signalId: "y2" }] };
    } });
  });
});
