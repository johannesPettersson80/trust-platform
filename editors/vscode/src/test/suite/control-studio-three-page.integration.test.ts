import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio, engineAnswerRecorderScript, type EngineAnswerRecord } from "./control-studio-rendered-harness";
import { recordingArchive } from "./control-studio-recording-archive";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";
import { formatDurationSeconds } from "../../controlStudio/webview/durationField";

suite("Control Studio three-page workflow", () => {
  test("calculates response, pole views and lead-lag designs through the visible model tools", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(90000);
    await withStudio(async (inner, reopen, capture) => {
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const model = { kind: "transfer-function", revision: "visible-model-tools", source: "provided", validation: "validated", evidence: ["analytical fixture"], delaySeconds: 0,
        numerator: [1, -1], denominator: [1, 0, -1], inputs: [{ id: "u", unit: loop.signals.manipulatedVariable.unit, operatingRange: loop.signals.manipulatedVariable.range }],
        outputs: [{ id: "y", unit: loop.signals.processVariable.unit, operatingRange: loop.signals.processVariable.range }] };
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(JSON.stringify(model))}],'model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'import';`);
      await pollValue("supplied model ready", () => inner("return d.querySelector('[data-studio-model-equation]')?.textContent??'';"), value => value.length > 0, 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.defaultView.__toolEvents=[];d.defaultView.addEventListener('message',e=>{if(typeof e.data?.kind==='string'&&e.data.kind.startsWith('control-studio.analysis.'))d.defaultView.__toolEvents.push(e.data);});return 'tools';");
      const choose = async (tool: string) => {
        // The engine answers the tools that apply to the model, so the option is awaited. An open list is closed and opened again, so it lists the current tools.
        await pollValue(`${tool} must be reachable`, () => inner(`const list=d.querySelector('[data-studio-analyze-list]');if(!list.hidden)d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze]').click();const option=d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool="${tool}"]');return String(!!option&&!option.disabled&&!option.hidden);`), value => value === "true", 15000);
        await inner(`d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool="${tool}"]').click();d.defaultView.__toolEvents=[];return 'selected';`);
      };
      // The engine answers a calculation with its envelope: a quick tool's `control-studio.analysis.answer`, or the `update` that
      // finishes an operation (B9.9). The tool's answer is `envelope.result[tool]`.
      const answerOf = async (tool: string) => {
        const envelope = JSON.parse(await pollValue(`visible ${tool} result`, () => inner(`return JSON.stringify(d.defaultView.__toolEvents.filter(e=>e.envelope?.tool==='${tool}'&&(e.kind==='control-studio.analysis.answer'||e.kind==='control-studio.analysis.update'&&e.state==='finished')).map(e=>e.envelope));`), value => JSON.parse(value).length > 0, 15000))[0];
        assert.ok(envelope.result, `${tool} calculates: ${JSON.stringify(envelope.failure)}`);
        return envelope;
      };
      const calculate = async (tool: string) => {
        await inner("d.querySelector('[data-engineer-tune]').click();return 'calculate';");
        return answerOf(tool);
      };
      await choose("initialState");
      await inner("for(const [key,value] of Object.entries({'response-step':'0.1','response-duration':'2','response-initial':'1,0','response-input':'1'})){const field=d.querySelector('[data-analysis-'+key+']');field.value=value;field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));}return 'response inputs';");
      const response = (await calculate("initialState")).result.initialState;
      assert.strictEqual(response.points.length, 21);
      assert.ok(response.points.some((point: any) => point.initialCondition[0] !== 0));
      await capture("initial-response");
      await choose("poleZero");
      assert.strictEqual(await inner("return d.querySelector('[data-analysis-pole-view]').value;"), "unreduced");
      const full = (await calculate("poleZero")).result.poleZero;
      assert.deepStrictEqual(full.unreduced.denominator, [1, 0, -1]);
      await inner("const view=d.querySelector('[data-analysis-pole-view]');view.value='minimal';view.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.defaultView.__toolEvents=[];return 'minimal';");
      const minimal = (await calculate("poleZero")).result.poleZero;
      assert.deepStrictEqual(minimal.minimal.denominator, [1, 1]);
      assert.strictEqual(minimal.hiddenUnstableMode, true);
      assert.match(await inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;"), /hidden|unstable/i);
      await capture("minimal-poles");
      await choose("leadLag");
      await inner("d.querySelector('[data-analysis-lead-lag-crossover]').value='1';d.querySelector('[data-analysis-lead-lag-phase]').value='30';d.querySelector('[data-engineer-tune]').click();return 'lead';");
      const lead = await answerOf("leadLag");
      assert.strictEqual(lead.inputs.mode, "lead"); assert.ok(lead.result.leadLag.design.gain > 0);
      assert.ok(!(await inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;" )).includes("[object Object]"));
      await capture("lead-design");
      await inner("const mode=d.querySelector('[data-analysis-lead-lag-mode]');mode.value='lag';mode.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.querySelector('[data-analysis-lead-lag-ratio]').value='5';d.defaultView.__toolEvents=[];d.querySelector('[data-engineer-tune]').click();return 'lag';");
      await pollValue("host lag design", () => inner("return JSON.stringify(d.defaultView.__toolEvents.filter(e=>e.envelope?.tool==='leadLag'&&(e.kind==='control-studio.analysis.answer'||e.kind==='control-studio.analysis.update'&&e.state==='finished')).map(e=>e.envelope.inputs.mode));"), value => JSON.parse(value).includes("lag"), 15000);
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune again';");
      // The reopened page shows the restored analysis as the chosen one (A8.6, B9.9): Analyze… offers its Close, and the Tune button names
      // the analysis's action. Wrong behaviour caught: only an engineer's choice named the tool, so the restored design had no Close.
      await pollValue("restored analysis named", () => inner("return String(!d.querySelector('[data-studio-analysis-close]').hidden)+'|'+d.querySelector('[data-engineer-tune]').textContent;"),
        value => value.startsWith("true|") && value !== "true|Tune controller", 10000);
      await choose("leadLag");
      // Reopening calculates the design again (B9.9), so the restored mode is awaited rather than read at once.
      await pollValue("restored lead-lag mode", () => inner("return d.querySelector('[data-analysis-lead-lag-mode]')?.value??'';"), value => value === "lag", 10000);
      await capture("lag-restored");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Analytical tools do not start or write the runtime");
    });
  });
  test("executes the project-owned sequence and retains its restored native recording", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_PROJECT_NATIVE !== "1") this.skip();
    this.timeout(90000);
    try { await withStudio(async inner => {
      // The review needs the running Simulator; without it the engine answers with its Start simulator action (A2.8).
      if (!(await runtimeLifecycleService.snapshot()).status.running) assert.ok((await runtimeLifecycleService.startLocalSimulator()).ok);
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='project';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.defaultView.__projectEvents=[];d.defaultView.addEventListener('message',event=>{if(event.data.kind==='control-studio.experiment.result')d.defaultView.__projectEvents.push(event.data);});d.querySelector('[data-engineer-find]').click();return 'review';");
      const reviewed = JSON.parse(await pollValue("native project review", () => inner("return JSON.stringify(d.defaultView.__projectEvents);"), value => JSON.parse(value).some((message: any) => message.phase === "review"), 45000));
      // The engine answers the review as `reviewed`, with its identity and the review the start will run (experimentRoute.ts:94).
      assert.strictEqual(reviewed.find((message: any) => message.phase === "review")?.payload.kind, "reviewed", JSON.stringify(reviewed));
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Confirm and start");
      await inner("d.querySelector('[data-engineer-find]').click();return 'start';");
      const completed = JSON.parse(await pollValue("native project terminal", () => inner("return JSON.stringify(d.defaultView.__projectEvents);"), value => JSON.parse(value).some((message: any) => message.phase === "engine-ended" || message.phase === "failed"), 20000));
      // The engine ends the test with `engine-ended` (experimentRoute.ts watch), its status and the restoration its finished campaign
      // confirmed (run.rs hold_restoration). The campaign is `finished` only after that restoration is confirmed (run.rs hold_end).
      const ended = completed.find((message: any) => message.phase === "engine-ended")?.payload;
      assert.strictEqual(ended?.status, "finished", JSON.stringify(completed));
      assert.strictEqual(ended?.restoration, "restored", "The engine names the restoration its campaign confirmed");
      await pollValue("the page says the outputs are restored", () => inner("return d.querySelector('[data-three-page-experiment-status]')?.textContent ?? '';"), value => value === "Project test completed. Outputs restored.", 10000);
      // The kept recording reaches the page in the identification the engine answers for the finished test (`control-studio.engine.result`,
      // method identify, the answer that carries the kept recording's samples, C12): its identifier, and one time per sample on the plot the
      // chart draws (identify_result.rs, IdentifyPlot.timeSeconds). The open answer's Recorded-data fit reaches the page as an identify
      // answer too (A8.6, B3.3), so the test's answer is the last one that carries recorded samples.
      const identified = JSON.parse(await pollValue("native project identification", () => inner("return JSON.stringify((d.defaultView.__engineAnswers||[]).filter(e=>e.kind==='control-studio.engine.result'&&e.method==='identify'&&e.result&&Array.isArray(e.result.recordedSamples)).map(e=>e.result));"), value => JSON.parse(value).length > 0, 10000)).pop();
      assert.ok(typeof identified.recordingId === "string" && identified.recordingId.length > 0, `The identification names the kept recording: ${JSON.stringify(identified).slice(0, 800)}`);
      // The recording's runtime scans reach the page (`recordedSamples`), whether or not its fit gave a model (C12).
      assert.ok((identified.recordedSamples?.length ?? 0) >= 10, "Retain runtime-scan data from the project sequence");
      // A fit that the recording supports draws its response plot; a refused fit answers its refusal instead (B3.5).
      if (identified.fitRefusal === undefined) assert.ok((identified.plot?.timeSeconds?.length ?? 0) >= 10, "A fit the recording supports draws its response plot");
      assert.strictEqual(identified.restoration, "restored", "The identification names the restoration of the project test");
      // The page shows a refused fit as the note under the completed test, and keeps no model (C12, B3.5).
      assert.strictEqual(await inner("return d.querySelector('[data-three-page-experiment-note]')?.textContent ?? '';"), identified.fitRefusal ?? "", "The page shows the fit's refusal as the note of the completed test");
      // The chart draws the recorded signals the engine kept (`control-studio.recording.evidence`), the default Signals selection
      // among them: the measurement and the setpoint in degC, and the applied output in %.
      await pollValue("native measured project response", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => Number(value) >= 2, 10000);
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /°C/, "The recorded measurement is drawn in its unit");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-acquisition-method]').disabled);"), "false", "Finishing restoration must unlock method selection");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-import]').disabled);"), "false", "The recording action must unlock after the project test");
    }, { capture: "native-project", loop: loop => { loop.experiment.durationSeconds = 2; } });
    } finally { if (vscode.debug.activeDebugSession) await vscode.debug.stopDebugging(vscode.debug.activeDebugSession); }
  });
  test("fits a manually selected model and checks it against an independent recording in Identify", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-acquisition-method] option[value=recorded-fit]'));"), "true", "Recorded fitting remains available as a manual identification method");
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='recorded-fit';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual fit';");
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      for (const [id, amplitude] of [["manual-estimation", 2], ["manual-independent", 1.5]] as const) {
        await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(recordingArchive(loop, id, amplitude))}],'recording.json',{type:'application/json'}));const file=d.querySelector('[data-identify-import-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'open';`);
        await pollValue("recording check available", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Check recording", 10000);
        await inner("const model=d.querySelector('[data-three-page-identification-model]');model.value='first_order';d.querySelector('[data-engineer-find]').click();return 'check';");
        // The first recording estimates the model; the independent one only scores the frozen model (Spec 35: no refit on validation data).
        const next = id === "manual-estimation" ? "Fit plant model" : "Compare with model";
        await pollValue(`${id} checked`, () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === next, 10000);
        await inner("d.querySelector('[data-engineer-find]').click();return 'fit';");
        await pollValue("manual fit complete", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => /Open validation recording|Use fitted model/.test(value), 15000);
      }
      await inner("d.querySelector('[data-engineer-find]').click();return 'accept';");
      await pollValue("accepted manual model", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Model ready", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false");
      // Tune designs on the accepted model: the engine's Tune answers a candidate for the study's current model (B3.7).
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune the accepted model';");
      await pollValue("candidate designed on the accepted manual model", () => inner("return d.querySelector('[data-studio-tuning-result]')?.textContent??'';"), value => /gain/i.test(value), 20000);
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    }, { capture: "manual-identification" });
  });
  test("offers the declared project experiment and plots selected recorded signals in their own units", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(120000);
    // The fixture is lesson 1's project (gatedJourneys in runTest.ts): its declared experiment keeps its activation, and the journey shortens only its duration.
    await withStudio(async inner => {
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-acquisition-method] option[value=project]'));"), "true", "Project-owned lessons have a reachable experiment method");
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='project';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'project';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-experiment]').checkVisibility());"), "true");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Review project test");
      // The engine runs the declared test and keeps its recording; the page draws the recorded signals from the engine's evidence
      // (`control-studio.recording.evidence`, enginePanelRoute.ts), not from a message the journey injects.
      const simulatorRuns = (await runtimeLifecycleService.snapshot()).status.running;
      await inner("d.defaultView.__projectEvents=[];d.defaultView.addEventListener('message',event=>{if(event.data.kind==='control-studio.experiment.result'||event.data.kind==='control-studio.recording.evidence')d.defaultView.__projectEvents.push(event.data);});d.querySelector('[data-engineer-find]').click();return 'review';");
      // On a stopped Simulator the engine answers the review with its next action Start simulator (A2.8, C12), which the page shows as the
      // refusal's button; the engineer presses it, and the started Simulator continues the test with its review.
      if (!simulatorRuns) {
        await pollValue("the engine's Start simulator action", () => inner("const action=d.querySelector('[data-studio-failure-action=model]');return action&&action.checkVisibility()?action.textContent:'';"), value => value === "Start simulator", 45000);
        await inner("d.querySelector('[data-studio-failure-action=model]').click();return 'start simulator';");
      }
      await pollValue("the engine reviews the project test", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Confirm and start", 45000);
      await inner("d.querySelector('[data-engineer-find]').click();return 'start';");
      await pollValue("the engine ends the project test", () => inner("return String(d.defaultView.__projectEvents.some(message=>message.phase==='engine-ended'));"), value => value === "true", 40000);
      await pollValue("the recorded signals of the engine", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => Number(value) >= 2, 10000);
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /°C/);
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /%/);
      // The Signals selector adds a recorded signal the test did not select by default (lesson 1 step 2); the chart
      // draws it in its own unit.
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-three-page-experiment-signals] option[value=\"door-open-indication\"]'));"), "true", "The declared door indication is a selectable recorded signal");
      const before = Number(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"));
      await inner("const option=d.querySelector('[data-three-page-experiment-signals] option[value=\"door-open-indication\"]');option.selected=true;d.querySelector('[data-three-page-experiment-signals]').dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'door';");
      await pollValue("the added recorded signal is drawn", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => Number(value) > before, 10000);
    }, { capture: "project-signals", loop: loop => { loop.experiment.durationSeconds = 20; } });
  });
  test("runs the declared root locus from Tune without a candidate or runtime write", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      // The engine answers the tools after the panel opens, so the list is read again until it lists the declared root locus.
      await pollValue("the declared root locus in the Analyze list", () => inner("const list=d.querySelector('[data-studio-analyze-list]');if(!list.hidden)d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze]').click();return String(!!d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"rootLocus\"]'));"), value => value === "true", 15000);
      await inner("d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"rootLocus\"]').click();return 'root locus';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false", "Read-only declared analysis does not require a tuned candidate");
      await inner("d.defaultView.__analysisMessages=[];d.defaultView.addEventListener('message',event=>{if(typeof event.data?.kind==='string'&&event.data.kind.startsWith('control-studio.analysis.'))d.defaultView.__analysisMessages.push(event.data);});d.querySelector('[data-engineer-tune]').click();return 'calculate';");
      // The root locus is an operation: the update that finishes it carries the envelope, whose answer is `result.rootLocus` (B9.9).
      const messages = await pollValue("root locus host result", () => inner("return JSON.stringify(d.defaultView.__analysisMessages);"), value => JSON.parse(value).some((message: any) => message.kind === "control-studio.analysis.update" && message.state === "finished" && message.envelope?.result?.rootLocus), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-analysis-result]')?.checkVisibility());"), "true", messages);
      assert.strictEqual(await inner("const message=d.querySelector('[data-studio-message=tune]');return message?.checkVisibility()?message.textContent:'';"), "", "A completed declared analysis must not still demand a plant model");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    }, { capture: "root-locus", loop: loop => { loop.topology = "open-loop"; delete loop.signals.setpoint; loop.controller.design = "project"; delete loop.controller.law; delete loop.controller.pidInitialization; loop.analysis = {
      plant: { kind: "transfer-function", numerator: [1], denominator: [1, 2, 0], delaySeconds: 0, inputUnit: "%", outputUnit: "°C", revision: "declared-root-model", source: "provided", validation: "validated" },
      controller: { numerator: [{ parameter: "Kp" }], denominator: [1], parameters: { Kp: { value: 2, minimum: 0, maximum: 10, unit: "%/°C" } } },
      rootLocus: { parameter: "Kp", selectedValue: 2, values: [0, 1, 2, 4, 8] },
    }; } });
  });
  test("opens a supplied plant model through the selected method and retains it for tuning after reopening", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const model = { kind: "transfer-function", numerator: [2], denominator: [5, 1], delaySeconds: 1,
      revision: "provided-course-model", source: "provided", validation: "validated", evidence: ["independent-course-measurement"],
      inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "y", unit: "°C", operatingRange: { minimum: 0, maximum: 100 } }],
      operatingPoint: { inputs: [20], outputs: [20] } };
    await withStudio(async (inner, reopen) => {
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-acquisition-method] option[value=supplied-model]'));"), "true", "Identify exposes supplied models through its method selector");
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Open model…");
      await inner("const file=d.querySelector('[data-studio-model-file]');d.defaultView.__modelPicker=0;file.addEventListener('click',event=>{event.preventDefault();d.defaultView.__modelPicker++;});d.querySelector('[data-engineer-find]').click();return 'open';");
      assert.strictEqual(await inner("return String(d.defaultView.__modelPicker);"), "1");
      const choose = (json: string) => inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(json)}],'plant.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'opened';`);
      await choose(JSON.stringify(model));
      await pollValue("supplied model result", () => inner("return d.querySelector('[data-studio-model-equation]').textContent;"), text => text.includes("5") && text.includes("2"), 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false", "The selected supplied model feeds the same Tune operation");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      // The chosen method is the study's acquisition input (A8.1), so reopening restores it and its entry path.
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-acquisition-method]').value;"), "supplied-model", "Reopening restores the chosen supplied-model method");
      assert.match(await inner("return d.querySelector('[data-studio-model-equation]').textContent;"), /5/);
      await choose("not json");
      await pollValue("invalid supplied model", () => inner("return d.querySelector('[data-studio-message=model]').textContent;"), text => /model|JSON/i.test(text), 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-message=model]').textContent;"), "Choose a valid plant model JSON file.");
      assert.match(await inner("return d.querySelector('[data-studio-model-equation]').textContent;"), /5/, "A failed import preserves the selected model");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    }, { capture: "supplied-model" });
  });
  test("shows retained checks and an inconclusive row for every unfinished mandatory scenario", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      // The engine answers an interrupted scenario inside its Validate result: the retained target-response check keeps its row, and
      // the unfinished scenario's runs are Inconclusive with the engine's reason (crates/trust-control/src/study/validate.rs
      // `job_outcome`, `not_run_requirements`, `failure_of`; validation/texts.rs `run_not_completed`, `scenario_label`).
      const interrupted = "The isolated run ended failed before it completed, so this requirement has no evidence. The runtime said: The load-change run was interrupted.";
      await inner(engineResult("validate", { scenarios: [
        scenarioOutcome("target-response", "Target response", "Pass", [requirementOutcome("settling", "Maximum settling time", "Pass", "s", "Within the limit.", { value: 4, limit: 5 })]),
        scenarioOutcome("load-change", "Load change", "Inconclusive", [requirementOutcome("settling", "Maximum settling time", "Inconclusive", "s", interrupted)]),
      ], status: "Inconclusive", applyAvailable: false, failure: { message: interrupted, cause: "runtime", nextAction: "none" },
      stageEvidence: { ...VALIDATION_EVIDENCE, validationPassed: false },
      stages: engineStages("complete", "complete", "needs-data"), labels: VALIDATE_LABELS, table: [
        validateTableRow({ title: "Load change · Current settings · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "—", limit: "—", verdict: "Inconclusive", verdictText: `Inconclusive · ${interrupted}`, requirement: "settling" }),
        validateTableRow({ title: "Load change · Candidate · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "—", limit: "—", verdict: "Inconclusive", verdictText: `Inconclusive · ${interrupted}`, requirement: "settling" }),
        validateTableRow({ title: "Target response · Current settings · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "4 s", limit: "≤ 5 s", verdict: "Pass", verdictText: "Pass", requirement: "settling" }),
        validateTableRow({ title: "Target response · Candidate · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "4 s", limit: "≤ 5 s", verdict: "Pass", verdictText: "Pass", requirement: "settling" }),
        validateTableRow({ title: "Candidate overall", measured: "—", limit: "—", verdict: "Inconclusive", verdictText: "Inconclusive", requirement: "overall" }),
      ] }));
      assert.strictEqual(await inner("const empty=d.querySelector('[data-studio-chart-empty]');return String(!!empty&&!empty.hidden&&['Start the runtime to see live values','Waiting for the first samples'].includes(empty.textContent)&&d.querySelector('[data-control-studio-chart]').dataset.seriesCount==='0');"), "true", "A missing response shows one line in the plot area instead of empty axes");
      const table = await inner("return d.querySelector('[data-studio-validation-result]').textContent;");
      assert.match(table, /Maximum settling time/);
      assert.match(table, /load change/i);
      assert.match(table, /Inconclusive/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
    }, { capture: "incomplete-validation" });
  });
  test("edits manual settings in the existing result area and preserves full values and pins when changing method", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      const gaps: string[] = [];
      const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
      const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
      for (const [id, amplitude] of [["manual-model-estimation", 2], ["manual-model-validation", 1.5]] as const) {
        // The file is read asynchronously before the host starts, so wait for that operation to run and finish; an edit
        // made while it runs would supersede it.
        await inner(`const root=d.querySelector('[data-studio-three-page]');d.defaultView.__recordingBusy=false;new d.defaultView.MutationObserver(()=>{if(root.dataset.busy==='true')d.defaultView.__recordingBusy=true;}).observe(root,{attributes:true,attributeFilter:['data-busy']});const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(recordingArchive(loop, id, amplitude))}],'recording.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';`);
        await pollValue("opened model analysis", () => inner("return String(d.defaultView.__recordingBusy && d.querySelector('[data-studio-three-page]').dataset.busy === 'false');"), value => value === "true", 15000);
      }
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false", "Opening recordings stays on Identify and never opens an empty Tune editor");
      // The engine answers Tune with the candidate's structure and settings (TuneResult; engineTune.ts tuneCandidatePayload gives the
      // page the family, the values and the stage evidence). The controller families come from the engine's open answer (controllerOffer,
      // threePageController.ts), not from the Tune answer. engineOpened.ts posts the candidate on the `engineer` stage, the stage the page's router takes for it.
      await inner(engineResult("tune", { candidate: { structure: "PI" },
        settings: { "P1.Parameters.ProportionalGain": 1.23456789012345, "P1.Parameters.IntegralGain": .5, "P1.Parameters.DerivativeGain": 0 },
        stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false },
        stages: engineStages("complete", "complete", "ready") }));
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual';");
      const manualRows = await inner("return String(d.querySelectorAll('[data-studio-manual-value]').length);");
      await reportedGap(gaps, `${TUNE_STAGE_GAP}: the manual rows of the candidate's family`, () => assert.strictEqual(manualRows, "2", "Manual PI shows only its relevant settings"));
      await reportedGap(gaps, `${TUNE_STAGE_GAP}: the candidate's settings`, async () => assert.strictEqual(await inner("return d.querySelector('[data-studio-manual-value]').value;"), "1.23456789012345"));
      await reportedGap(gaps, `${TUNE_STAGE_GAP}: the tuning result text`, async () => assert.ok((await inner("return d.querySelector('[data-studio-tuning-result]').textContent;")).includes("Proportional gain (Kp)")));
      if (manualRows === "0") {
        // A dependent chain: editing a setting, changing the method and the incomplete Tune all need the manual settings on the page.
        gaps.push(`${TUNE_STAGE_GAP}: the manual-settings steps not run, because the Tune page shows no manual settings`);
      } else {
        await inner("const input=d.querySelector('[data-studio-manual-value]');input.value='2.123456789';input.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'edited';");
        await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='automatic';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'automatic';");
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-manual-value]').length);"), "0", "Automatic results stay plain and compact");
        await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual again';");
        assert.strictEqual(await inner("return d.querySelector('[data-studio-manual-value]').value;"), "2.123456789");
        assert.strictEqual(await inner("return String(!!d.querySelector('[data-studio-manual-pin]'));"), "false", "Manual settings have no Pin checkboxes");
        await inner("const input=d.querySelector('[data-studio-manual-value]');input.value='';input.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));d.querySelector('[data-engineer-tune]').click();return 'incomplete';");
        // The page sends the entry as typed, and the engine refuses one that no number reads under the setting's own symbol (B4.2, B4.5), so the
        // refusal and the marked row come with the engine's answer, which the check waits for.
        const refused = await pollValue("the engine's refusal of the blank manual setting",
          () => inner("return JSON.stringify((d.defaultView.__engineAnswers||[]).filter(answer=>answer.method==='tune'&&answer.status==='failed').map(answer=>({reason:answer.reason,field:answer.failure&&answer.failure.field})));"),
          value => value !== "[]", 15000);
        assert.strictEqual(await inner("return d.querySelector('[data-studio-manual-value]').getAttribute('aria-invalid');"), "true", `The page marks the row that the engine names: ${refused}`);
        assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.busy??'false';"), "false", "An incomplete manual value does not leave Tune running");
      }
      assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
    }, { capture: "manual-settings", loop: loop => { loop.id += "-manual-fields"; loop.parameterBinding.loop_id = loop.id; } });
  });
  test("renders state-design policy and preserves finite design drafts across reopen", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    // The fixture is lesson 69's project (gatedJourneys in runTest.ts), so its supplied model is the lesson's own provided-model.json.
    this.timeout(120000);
    const modelText = fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "provided-model.json"), "utf8");
    await withStudio(async (inner, reopen) => {
      const loopSnapshot = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      assert.strictEqual(loopSnapshot.controller.stateFeedback?.family, "LQR", "The lesson's loop is an LQR state-feedback law");
      // control-loop.json declares no weights in controller.stateFeedback; README step 4 enters them on the Tune page.
      assert.strictEqual(loopSnapshot.controller.stateFeedback?.stateWeights, undefined, "The lesson's loop declares no state weights");
      assert.strictEqual(loopSnapshot.controller.stateFeedback?.inputWeights, undefined, "The lesson's loop declares no input weights");
      // README step 2: Identify, Supplied plant model, then open provided-model.json.
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
      await inner("d.querySelector('[data-engineer-find]').click();return 'open model';");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(modelText)}],'provided-model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'model imported';`);
      await pollValue("supplied state model", () => inner("return d.querySelector('[data-studio-model-equation]')?.textContent??'';"), value => value.length > 0, 10000);
      // README step 3: Requirements, then Done.
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      await pollValue("state design requirements", () => inner("return String(!!d.querySelector('dialog[open]'));"), value => value === "true", 5000);
      await inner("const set=(key,value)=>{const field=d.querySelector(`[data-studio-requirement=\"${key}\"]`);field.value=String(value);field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));};set('referenceChange',0.01);set('maximumError',0.002);set('settlingBand',0.001);set('settlingTimeSeconds',90);set('durationSeconds',120);d.querySelector('[data-studio-editor-done]').click();return 'requirements set';");
      // README step 4: State-space design, which the engine offers for this loop.
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-tuning-method] option[value=state-space]'));"), "true", "State-space design must be an actual Tune method");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-tuning-method] option[value=state-space]').textContent;"), "State-space design");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='state-space';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'state design';");
      // The engine lists the design form's fields in its answer, so the form appears once that answer arrives.
      await pollValue("state design form", () => inner("return String(!!d.querySelector('[data-state-design-mode]'));"), value => value === "true", 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-mode]').value;"), "infinite", "The solver policy starts at the infinite horizon");
      // The form shows the declared values only, and the lesson declares no weights, so both weight fields start empty.
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=stateWeights]').value;"), "", "The lesson's loop declares no state weights, so the form shows none");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=inputWeights]').value;"), "", "The lesson's loop declares no input weights, so the form shows none");
      await inner("d.defaultView.__stateDesignTrace=[];d.defaultView.addEventListener('message',e=>{if(e.data.kind==='control-studio.engineering.result'&&e.data.stage==='engineer'){const p=e.data.payload;d.defaultView.__stateDesignTrace.push({status:p.status,family:p.family,preferences:p.tuningPreferences,horizon:p.state?.horizon});}});return 'trace';");
      // README step 4 continued: Solver policy Finite-horizon (fixed policy), 20 steps, the weights and the terminal weights.
      await inner("const mode=d.querySelector('[data-state-design-mode]');mode.value='finite';mode.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'finite';");
      // The finite policy lists the horizon fields in the engine's next answer, so they appear after it.
      await pollValue("finite horizon fields", () => inner("return String(!!d.querySelector('[data-state-design-field=horizonSteps]'));"), value => value === "true", 15000);
      await inner("const set=(field,value)=>{const control=d.querySelector(`[data-state-design-field=\"${field}\"]`);control.value=value;control.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));};set('horizonSteps','20');set('terminalWeights','[[400, 0], [0, 400]]');set('inputWeights','[[40000]]');return 'finite values';");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=horizonSteps]').value;"), "20");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-mode]').value;"), "finite", "The solver policy is the finite horizon");
      // An unreadable weight matrix is refused under its own field, and Tune does not reuse the earlier weights.
      await inner("const q=d.querySelector('[data-state-design-field=stateWeights]');q.value='[[';q.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));d.querySelector('[data-engineer-tune]').click();return 'invalid';");
      await pollValue("refused state weights", () => inner("return d.querySelector('[data-state-design-error=stateWeights]')?.textContent??'';"), value => value.length > 0, 15000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-state-design-field=stateWeights]').getAttribute('aria-invalid'));"), "true", "Invalid state design must block Tune instead of reusing stale weights");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-error=stateWeights]').textContent;"), "State weights (Q) must be valid JSON.", "The refusal is shown under the field it names");
      // README step 4: the state weights Q, then Tune controller.
      await inner("const q=d.querySelector('[data-state-design-field=stateWeights]');q.value='[[400, 0], [0, 400]]';q.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));d.querySelector('[data-engineer-tune]').click();return 'tune state model';");
      await pollValue("state-design candidate", () => inner("return d.querySelector('[data-studio-tuning-result]')?.textContent??'';"), value => /gain|Riccati|finite/i.test(value), 15000);
      // Automatic Tune continues to Validate. Review the candidate on its own tab.
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'review candidate';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-tuning-result]')?.textContent?.length>0);"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-design-policy]')?.checkVisibility()===true);"), "true", await inner("return 'Finite state design must expose its repeated-first-gain policy: '+JSON.stringify(d.defaultView.__stateDesignTrace);"));
      assert.match(await inner("return d.querySelector('[data-studio-design-policy]').textContent;"), /Finite horizon: 20 steps.*Repeated first gain/);
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-riccati-stage]').length);"), "21", "The 20-step finite design must expose N+1 = 21 Riccati stages, from stage 0 to the terminal stage 20");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const method=d.querySelector('[data-engineer-tuning-method]');method.value='state-space';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'reopened';");
      await pollValue("reopened finite draft", () => inner("return String(!!d.querySelector('[data-state-design-field=horizonSteps]'));"), value => value === "true", 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=horizonSteps]').value;"), "20", "Finite draft survives reopening");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-mode]').value;"), "finite", "The finite solver policy survives reopening");
    }, { capture: "state-design" });
  });
  test("asks for the missing experiment duration before live identification and leaves the field blank", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.querySelector('[data-engineer-find]').click();return 'identify';");
      // The engine's run asks for the missing limit as the input it needs (B8, `needs: limits`), before it requires a runtime, and the page
      // opens Limits when that answer arrives: the dialog is awaited, not read at the press.
      await pollValue("Missing limits open before any live excitation", () => inner("return String(!!d.querySelector('dialog[open]'));"), value => value === "true", 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), "", "No arbitrary experiment duration is supplied");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').getAttribute('aria-invalid');"), "true", "The missing value is identified");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
      await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'cancelled';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.busy??'false';"), "false");
    }, { loop: loop => { loop.id += "-missing-duration"; loop.revision += "-missing-duration"; if (loop.parameterBinding) loop.parameterBinding.loop_id = loop.id; delete loop.experiment; } });
  });
  test("opens recorded data from its visible button and stops for model review without starting a runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-identify-import-file]'));"), "true", "Open recording has a file chooser");
      await inner("const input=d.querySelector('[data-identify-import-file]');d.defaultView.__picker=0;input.addEventListener('click',event=>{event.preventDefault();d.defaultView.__picker++;});d.querySelector('[data-identify-import]').click();return 'open';");
      assert.strictEqual(await inner("return String(d.defaultView.__picker);"), "1", "The visible button opens the recording chooser");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.busy??'false';"), "false", "Cancelling the picker starts no operation");
      const choose = (name: string, json: string) => inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(json)}],${JSON.stringify(name)},{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';`);
      await choose("invalid.json", "not json");
      await pollValue("recording error", () => inner("return d.querySelector('[data-studio-message=model]').textContent;"), text => text.includes("valid Control Studio recording"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "false", "A failed import leaves a usable action");
      await choose("estimation.json", recordingArchive(loop, "opened-estimation", 2));
      await pollValue("independent recording request", () => inner("return d.querySelector('[data-studio-message=model]').textContent;"), text => text.includes("independent recording"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-import]').disabled);"), "false");
      await choose("validation.json", recordingArchive(loop, "opened-validation", 1.5));
      await pollValue("recording model review", () => inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility() && d.querySelector('[data-studio-three-page]').dataset.busy === 'false');"), value => value === "true", 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Identify");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').classList.contains('trust-button--primary'));"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "true");
      await pollValue("recorded measured and model response", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => Number(value) >= 2, 10000);
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Opening recordings has no runtime startup or excitation");
    }, { capture: "opened-recording" });
  });
  test("offers actual archived output channels to the opened-recording selector without starting a runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    let scopedLoop: any;
    assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    await withStudio(async (inner, reopen) => {
      const extra = { id: "P1.RecordedTemperature", unit: scopedLoop.signals.processVariable.unit };
      const archive = (id: string, offset: number): string => {
        const value = JSON.parse(recordingArchive(scopedLoop, id, id.includes("validation") ? 1.5 : 2)) as any;
        const archivedRun = value.engineerStudy.campaigns[0].runs[0];
        archivedRun.run.channels.push(extra);
        archivedRun.request.observations.push({ symbol: extra.id, unit: extra.unit });
        for (const sample of archivedRun.run.samples) {
          sample.values[extra.id] = { value: sample.values[scopedLoop.signals.processVariable.symbol].value + offset, quality: "good" };
        }
        archivedRun.evaluation.run = structuredClone(archivedRun.run);
        return JSON.stringify(value);
      };
      // The engine's answer to each file (`control-studio.identification-recording`): its channel catalogue, its reason and its check.
      await inner("d.defaultView.__recordingAnswers=[];d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.identification-recording')d.defaultView.__recordingAnswers.push(event.data.payload);});return 'listening';");
      const first = archive("opened-extra-output-estimation", 0.25);
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(first)}],'recording-with-extra-output-estimation.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';`);
      await pollValue("opened archive with extra output", () => inner("return d.querySelector('[data-studio-message=model]').textContent;"), value => value.includes("independent recording"), 15000);
      const answer = JSON.parse(await inner("return JSON.stringify((d.defaultView.__recordingAnswers||[]).filter(p=>p.recordingCatalog)[0]||null);")) as { reason?: string; recordingCatalog: { channels: Array<{ id: string; unit: string }> } } | null;
      assert.ok(answer?.recordingCatalog.channels.some(channel => channel.id === extra.id && channel.unit === extra.unit),
        `The engine's recordingCatalog must offer the archived channel with its unit: ${JSON.stringify(answer)}`);
      assert.match(answer?.reason ?? "", /independent recording/, "The file that needs a second recording answers its reason");
      const labels = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-outputs] option')].map(option=>option.textContent));")) as string[];
      assert.ok(labels.some(label => label.includes("P1.RecordedTemperature") && label.includes("°C")),
        `The opened recording output selector must expose the archived channel with its unit: ${labels.join(", ")}`);
      const selected = JSON.parse(await inner("const select=d.querySelector('[data-studio-outputs]');for(const option of select.options)option.selected=false;const option=[...select.options].find(option=>option.textContent.includes('P1.RecordedTemperature'));option.selected=true;select.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return JSON.stringify([...select.selectedOptions].map(option=>option.value));")) as string[];
      assert.deepStrictEqual(selected, ["P1.RecordedTemperature"], "The archived output can be selected without retaining the configured PV");
      await pollValue("selected-channel refit awaits independent data", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      const second = archive("opened-extra-output-validation", 0.25);
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(second)}],'recording-with-extra-output-validation.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';`);
      await pollValue("selected recording model ready", () => inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility() && d.querySelector('[data-studio-three-page]').dataset.busy === 'false');"), value => value === "true", 15000);
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false", "Identification stays on model review");
      // The engine's Identify answer of the fitted pair names the response plot: its measurement is the selected archived output.
      const identifications = JSON.parse(await inner("return JSON.stringify((d.defaultView.__engineAnswers||[]).filter(e=>e.method==='identify'&&e.status==='model-ready').map(e=>e.result));")) as Array<{ plot?: { traces?: Array<{ role: string; symbol: string }> } }>;
      const measured = identifications[identifications.length - 1]?.plot?.traces?.find(trace => trace.role === "measurement");
      const selectedOutput = measured?.symbol === extra.id;
      const chartLabel = await pollValue("selected archived output plot", () => inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label')??'';"), value => value.includes(extra.id), 10000);
      assert.ok(selectedOutput || chartLabel.includes(extra.id), "The returned model or response plot must retain the selected archived output");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Opening a recording must not start a runtime");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'reopened';");
      assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify([...d.querySelector('[data-studio-outputs]').selectedOptions].map(option=>option.value));")), [extra.id],
        "Reopening retains the selected recording output and its signal catalogue");
    }, { loop: loop => { scopedLoop = loop; }, capture: "opened-recording-selection" });
  });
  test("keeps the validation response and Apply visible with many required checks and exposes failures first", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      const gaps: string[] = [];
      // The engine's Validate answer for the same campaign: 32 passing state-range guards, then the failing error check, in the order
      // the scenario declares them (RequirementOutcome rows; crates/trust-control/src/study/validate.rs `requirement_label`,
      // `failure_of`). The scenario's one requirement list judges both runs, so each run holds the 32 guards (guard: true) and the
      // failing check: the table names each run's failure first, counts the passing guards of both runs in one row, and ends with the
      // candidate's overall row (specification 35, B5.4 and B9.9; engineValidate.ts renderValidation).
      const checks = Array.from({ length: 32 }, (_, index) => requirementOutcome(`controller-state:${index}`, "State range", "Pass", "°C", "Within limits.",
        { measuredRange: { minimum: 0, maximum: 0 }, limitRange: { minimum: 0, maximum: 100 }, guard: true }));
      checks.push(requirementOutcome("tracking-error", "Maximum steady-state error", "Fail", "°C", "The measured error exceeds the limit.", { value: 2, limit: 1 }));
      await inner(engineResult("validate", { scenarios: [scenarioOutcome("nominal", "Nominal", "Fail", checks)], status: "Fail", applyAvailable: false,
        stageEvidence: { ...VALIDATION_EVIDENCE, validationPassed: false },
        failure: { message: "The measured error exceeds the limit.", cause: "settings", nextAction: "tune", nextActionLabel: "Tune controller" },
        stages: engineStages("complete", "complete", "failed"), labels: VALIDATE_LABELS, plot: MEASURED_RESPONSE, table: [
          validateTableRow({ title: "Current settings · Maximum steady-state error · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "2 °C", limit: "≤ 1 °C", verdict: "Fail", verdictText: "Fail · The measured error exceeds the limit.", requirement: "tracking-error" }),
          validateTableRow({ title: "Candidate · Maximum steady-state error · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "2 °C", limit: "≤ 1 °C", verdict: "Fail", verdictText: "Fail · The measured error exceeds the limit.", requirement: "tracking-error" }),
          validateTableRow({ title: "Ranges, states and transfer", measured: "64 checks", limit: "Within declared limits", verdict: "Pass", verdictText: "Pass", requirement: "limit-and-controller-checks" }),
          validateTableRow({ title: "Candidate overall", measured: "—", limit: "—", verdict: "Fail", verdictText: "Fail", requirement: "overall" }),
        ] }));
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'inspect results';");
      await reportedGap(gaps, "Validate order gap: failures are not shown first, because engineValidate.ts renderValidation draws the rows in the answer's order",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-studio-validation-result] tbody tr').dataset.studioRequirement;"), "tracking-error", "Failures appear directly at the top of the results"));
      await reportedGap(gaps, "Validate grouping gap: each passing generated guard has its own row, because engineValidate.ts renderValidation draws one row per requirement (threePageResults.ts grouped them on the TypeScript route)",
        async () => {
          const rows = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-validation-result] tbody tr')].map(row=>({id:row.dataset.studioRequirement,cells:[...row.cells].map(cell=>cell.textContent)})));")) as { id: string; cells: string[] }[];
          // The shape the engine answers on purpose: both runs' failing check, then one counted row of the passing guards of both runs,
          // then the candidate's overall row (B5.4 names each scenario's runs; B9.9 the counted row).
          assert.deepStrictEqual(rows.slice(1).map(row => row.id), ["tracking-error", "limit-and-controller-checks", "overall"],
            "The passing generated guards are counted in one row beneath the failures, and the overall row closes the table (CPREL-B13)");
          assert.deepStrictEqual(rows.find(row => row.id === "limit-and-controller-checks")?.cells.slice(0, 2), ["Ranges, states and transfer", "64 checks"],
            "The counted row names the guards of both runs (32 each)");
        });
      // The table scrolls in its own region (threePageStyles.ts `.studio-validation-scroll`), so Apply stays in view with every row.
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').getBoundingClientRect().bottom<=d.defaultView.innerHeight-8);"), "true", "Apply remains visible beside the response, without scrolling through technical checks");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true", "A failed requirement keeps Apply unavailable");
      assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
    });
  });
  test("starts guarded application and reports rejection without claiming adoption", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      // The engine route answers a passed validation as the Validate result with Apply available (engineValidate.ts relayApplyAnswer).
      await inner(engineResult("validate", { scenarios: [scenarioOutcome("target-response", "Target response", "Pass",
        [requirementOutcome("settling", "Maximum settling time", "Pass", "s", "Within the limit.", { value: 4, limit: 8 })])],
      status: "Pass", applyAvailable: true, stageEvidence: { ...VALIDATION_EVIDENCE, validationPassed: true },
        stages: engineStages("complete", "complete", "complete", true), labels: VALIDATE_LABELS, table: [
          validateTableRow({ title: "Current settings · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "4 s", limit: "≤ 8 s", verdict: "Pass", verdictText: "Pass", requirement: "settling" }),
          validateTableRow({ title: "Candidate · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "4 s", limit: "≤ 8 s", verdict: "Pass", verdictText: "Pass", requirement: "settling" }),
          validateTableRow({ title: "Candidate overall", measured: "—", limit: "—", verdict: "Pass", verdictText: "Pass", requirement: "overall" }),
        ] }));
      const before = Number(await inner(ENGINE_ANSWER_COUNT));
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      assert.strictEqual(await inner("d.querySelector('[data-engineer-apply]').click();return d.querySelector('[data-engineer-apply]').textContent;"), "Applying…");
      // The engine refuses the request (`control-studio.engine.failure`, method apply): the study holds no validation the engine ran
      // and no runtime is connected (crates/trust-control-studio/src/apply_action.rs). The engine route dispatches no application
      // event, so the wait reads the engine's answers (engineAnswerRecorderScript).
      const messages = (JSON.parse(await pollValue("unverified application rejection", () => inner(engineAnswersAfter(before)),
        text => (JSON.parse(text) as EngineAnswer[]).some(event => isApplicationAnswer(event) && event.status === "failed"), 10000)) as EngineAnswer[]).filter(isApplicationAnswer);
      assert.ok(messages.some(event => event.status === "failed"));
      assert.strictEqual(await inner("return String(Array.from(d.querySelectorAll('[data-studio-application] button')).filter(button=>!button.hidden).length);"), "1");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-keep]').hidden);"), "true", "Keep in project waits for a successful Apply");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Apply");
      assert.ok(!(await inner("return d.querySelector('[data-studio-message=verify]').textContent;")).includes("Applied"));
    });
  });
  test("shows tuned settings and every mandatory validation requirement on their pages", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      const gaps: string[] = [];
      // The engine's Tune answer: the candidate's structure and settings, its predicted measurement and the predicted measurement of the
      // current settings beside it (TuneResult; the traces `measurement` and `current-measurement`, TuneRole in crates/trust-control/src/messages/tune.rs).
      await inner(engineResult("tune", { candidate: { structure: "PI" }, settings: { "P1.Parameters.ProportionalGain": 5, "P1.Parameters.IntegralGain": 1 },
        outcome: { status: "meets" }, words: TUNE_WORDS,
        response: { timeSeconds: [0, 1, 2], traces: [{ role: "measurement", symbol: "P1.ProcessValue", unit: "°C", values: [20, 21, 21.8] },
          { role: "current-measurement", symbol: "P1.ProcessValue", unit: "°C", values: [20, 20.4, 20.9] }] },
        stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false },
        stages: engineStages("complete", "complete", "ready") }));
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'inspect tuning';");
      // The Tune-page checks are reported gaps; the Validate checks below depend only on the Validate answer, not on Tune.
      await reportedGap(gaps, `${TUNE_STAGE_GAP}: the tuning result`, async () => assert.strictEqual(await inner("return String(d.querySelector('[data-studio-tuning-result]').checkVisibility());"), "true"));
      const tuning = await inner("return d.querySelector('[data-studio-tuning-result]').textContent;");
      await reportedGap(gaps, `${TUNE_STAGE_GAP}: the tuning result text`, () => { assert.match(tuning, /PI/); assert.match(tuning, /Proportional gain/i); assert.match(tuning, /5/); assert.match(tuning, /%/); });
      await reportedGap(gaps, `${TUNE_STAGE_GAP}, and the current settings' predicted response beside the tuned one (TuneRole current-measurement)`,
        async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "2", "Compare current and tuned predictions"));
      // The engine's Validate answer: the nominal scenario's current settings and candidate runs, and the measured response of the
      // candidate's run (ValidateResult; its plot has no current run when that run has no sample).
      const requirements = [requirementOutcome("nominal-error", "Maximum steady-state error", "Pass", "°C", "Within limit.", { value: 0.02, limit: 0.1 }),
        requirementOutcome("nominal-settling", "Maximum settling time", "Pass", "s", "Within deadline.", { value: 4, limit: 8 })];
      await inner(engineResult("validate", { scenarios: [scenarioOutcome("nominal", "Nominal", "Pass", requirements)], status: "Pass", applyAvailable: true,
        stageEvidence: { ...VALIDATION_EVIDENCE, validationPassed: true },
        stages: engineStages("complete", "complete", "complete", true), labels: VALIDATE_LABELS, plot: MEASURED_RESPONSE, table: [
          validateTableRow({ title: "Current settings · Maximum steady-state error · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "0.02 °C", limit: "≤ 0.1 °C", verdict: "Pass", verdictText: "Pass", requirement: "nominal-error" }),
          validateTableRow({ title: "Current settings · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "4 s", limit: "≤ 8 s", verdict: "Pass", verdictText: "Pass", requirement: "nominal-settling" }),
          validateTableRow({ title: "Candidate · Maximum steady-state error · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "0.02 °C", limit: "≤ 0.1 °C", verdict: "Pass", verdictText: "Pass", requirement: "nominal-error" }),
          validateTableRow({ title: "Candidate · Maximum settling time · Measurement (°C)", symbol: "P1.ProcessValue", source: "temperature-process", measured: "4 s", limit: "≤ 8 s", verdict: "Pass", verdictText: "Pass", requirement: "nominal-settling" }),
          validateTableRow({ title: "Candidate overall", measured: "—", limit: "—", verdict: "Pass", verdictText: "Pass", requirement: "overall" }),
        ] }));
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'inspect validation';");
      // The engine's table holds each scenario's Current settings run, then its Candidate run, each with both mandatory requirements, and
      // ends with the candidate's overall row (engineValidate.ts renderValidation): every row is checked, in order.
      assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-validation-result] tbody tr')].map(row=>row.dataset.studioRequirement));")),
        ["nominal-error", "nominal-settling", "nominal-error", "nominal-settling", "overall"], "Retain both mandatory requirements");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-validation-result] tbody tr').length);"), "5", "The validation table holds no other rows");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "1", "Show measured validation output");
      // A requirement change leaves the validation stale, which the engine's inputs answer states (InputsResult `invalidated`,
      // engineInputs.ts). The stale plot's check is reported gap G6.
      await inner(engineResult("inputs", { stages: engineStages("complete", "complete", "ready"), invalidated: { validation: true } }));
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-validation-result]').hidden);"), "true", "Previous passing rows are no longer presented as current validation after inputs change");
      await reportedGap(gaps, "G6 the chart keeps the Tune and Validate plots after inputs change, because threePageChart.ts:116 keys on an edited status event that the engine route does not dispatch",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "0", "Stale response is retained in history, not shown for current inputs"));
      assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
    });
  });
  test("starts identification from its visible button with the actual Control.PID plant", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    // Identifying again waits, bounded, for the plant to settle after the first experiment.
    this.timeout(240000);
    await withStudio(async (inner, reopen, capture) => {
      const gaps: string[] = [];
      // Identify plant is one `controlStudio/run` through Identify on the engine route (engine/routes/runRoute.ts), and a live
      // identification needs a connected target (specification 35, B3.4 step 1; B3.9): without one the engine answers "Start the
      // runtime before running a live action." (crates/trust-control/src/identification/connected_run.rs). The TypeScript route
      // started the simulator itself (engineerActions/find.ts, runtimeSearchConnection.ts); here the engineer starts it first with the
      // same call, and withStudio stops it.
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.ok(started.ok, JSON.stringify(started));
      const before = Number(await inner(ENGINE_ANSWER_COUNT));
      await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__studioEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',event=>d.defaultView.__studioEvents.push(event.detail));${liveRecordingObserver(before)}d.querySelector('[data-engineer-find]').click();return 'started';`);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Abort", "Identify starts the operation and offers Abort");
      // The engine route dispatches no status events, so the wait reads the engine's answers: the run's result follows its last
      // update and carries Identify's answer as `identification` (RunResult in crates/trust-control/src/messages.rs).
      const finished = JSON.parse(await pollValue("native identification result", () => inner(engineAnswersAfter(before)),
        text => (JSON.parse(text) as EngineAnswer[]).some(endsIdentification), 60000)) as EngineAnswer[];
      const answered = finished.findIndex(answersIdentification);
      const identified = (answered < 0 ? null : JSON.parse(await inner(engineIdentificationAt(before + answered)))) as EngineIdentification | null;
      // The engine names the identified model by its type and equation; it answers no TypeScript model object (gap G4).
      assert.ok(identified?.modelType && identified.equation, JSON.stringify(finished));
      const presentations = JSON.parse(await inner("return JSON.stringify(d.defaultView.__studioEvents.filter(e=>e&&e.identificationPresentation).map(e=>(e.identificationPresentation.predictedResponse||[]).length));")) as number[];
      assert.ok(presentations.length > 0 && presentations[presentations.length - 1] > 2, "The native model has measured-versus-predicted evidence");
      // The engine route sends no liveIdentification event: while the engine reports the Identify stage running, the chart plots the
      // plant live from the loop monitor (threePageChart.ts), which is the recording the engineer sees before the model result.
      const live = JSON.parse(await inner("return JSON.stringify(d.defaultView.__liveRecording??null);")) as { answers: number } | null;
      assert.ok(live !== null && live.answers <= before + answered, "Show the live experiment recording before the model result");
      // The applied plant input is the plant-input trace of the engine's plot, taken from the same experiment record (IdentifyResult).
      const input = identified?.plantInput ?? null;
      assert.ok(input && input.count > 0, "Retain the applied plant input with its model response");
      assert.ok(input.minimum === 0 && input.maximum !== null && input.maximum > 0, "The chart evidence includes the applied excitation");
      assert.strictEqual(input.unit, "%");
      // Gap G2: engineIdentify.ts showPlot forwards only the measured and predicted traces, so the applied input is not drawn yet. Both
      // checks of it are reported gaps, so that the journey goes on to the reset on this live page.
      await reportedGap(gaps, "G2 applied input on the Identify plot", async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3", "Plot applied input, measured output and model prediction together"));
      // Specification 35 B9.9: the applied input is named by its role on its axis too, "Output (%)", not "Applied input".
      await reportedGap(gaps, "G2 applied input in the plot description", async () => assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /Model prediction, Output\. .*Output \(%\)/));

      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'inspect identified model';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "true");
      await pollValue("initial identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      // Every update of a run names the stage it is in (crates/trust-control-studio/src/operation.rs), so its end is completed on Identify.
      const terminalEvents = JSON.parse(await inner(engineAnswersAfter(before))) as EngineAnswer[];
      assert.ok(terminalEvents.some(event => event.status === "completed" && event.flowStage === "Identify"), JSON.stringify(terminalEvents));
      assert.ok(!terminalEvents.some(event => event.status === "paused"), "Completed trial cleanup must not overwrite the model-ready result");
      // Reset runs on this live page, which shows the identified model: a reopened page shows none (gap G3), so a reset there would
      // pass its checks on an empty page.
      await inner("d.querySelector('[data-studio-reset-chart]').click();Array.from(d.querySelectorAll('.studio-reset-confirm button')).find(b=>b.textContent==='Reset study').click();return 'reset';");
      // The engine answers the reset at once (`control-studio.engine.result`, method identify, without a model type) and the page
      // stays open, so the wait reads that answer as well as the page.
      await pollValue("reset study", () => inner("return String(d?.querySelector('[data-studio-three-page]')?.dataset.hostReady === 'true' && d?.querySelector('[data-studio-model-result]')?.hidden === true && (d.defaultView.__engineAnswers||[]).some(e=>e.status==='reset'));").catch(() => "loading"), value => value === "true", 15000);
      // Gap G1, the R3 failure: clearEngineIdentification (engineIdentify.ts) hides the model result but keeps its equation and type.
      await reportedGap(gaps, "G1 Reset keeps the model equation (engineIdentify.ts clearEngineIdentification)",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-studio-model-equation]').textContent;"), "", "Reset clears the model equation"));
      // Reset clears the recorded experiment and its model comparison, then resumes live observation (Identify spec).
      await reportedGap(gaps, "Behind G1: the chart still shows the model comparison after Reset (clearEngineIdentification sends the chart nothing)",
        async () => assert.doesNotMatch(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label') ?? '';"), /Model prediction/, "Reset clears the displayed experiment"));
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "true");
      // The TypeScript route reloaded the page after a reset; the engine route keeps it, so its reset must free Identify plant again.
      const identifyDisabled = await inner("return String(d.querySelector('[data-engineer-find]').disabled);");
      await reportedGap(gaps, "G5 the page stays busy after an engine Reset (threePageOperation.ts:92 ends the reset only on a failure)",
        () => assert.strictEqual(identifyDisabled, "false", "After the reset, Identify plant can start a new identification"));
      if (identifyDisabled === "false") {
        const again = Number(await inner(ENGINE_ANSWER_COUNT));
        await inner("d.querySelector('[data-engineer-find]').click();return 'new identification';");
        const repeated = JSON.parse(await pollValue("repeated native identification result", () => inner(engineAnswersAfter(again)),
          text => (JSON.parse(text) as EngineAnswer[]).some(endsIdentification), 150000)) as EngineAnswer[];
        assert.ok(repeated.some(event => event.status === "completed" && event.flowStage === "Identify"), JSON.stringify(repeated));
        assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /Model prediction/, "The new result replaces the cleared chart");
      } else {
        // A dependent step: the repeated identification needs Identify plant, so it is neither clicked nor waited for.
        gaps.push("G5 repeated identification not run: Identify plant disabled after Reset");
      }
      // The screenshot after the reset (and the new result when it ran), taken here because the reopen below now ends the journey.
      await capture("identify-and-reset-chart");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__studioEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',event=>d.defaultView.__studioEvents.push(event.detail));return 'reopened';");
      await capture("reopened-identification");
      // Gaps G3 and G2: the engine's open answer carries no identification (OpenResult), and the plot draws no applied input.
      await reportedGap(gaps, "G3 and G2 reopened identification plot", async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3", "Reopening preserves the actual experiment input and model comparison"));
      assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
    });
  });
  test("plots measured and predicted identification responses with their units", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      // The engine's Identify answer carries the response plot: the measurement and the model's prediction on one time base, each with
      // its unit (IdentifyResult.plot); engineIdentify.ts showPlot hands them to the chart as the identification presentation.
      await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();${engineResult("identify", identifyResult("First-order model", "G(s) = 0.2 / (1.5s + 1)", "Gain 0.2 °C per %, time constant 1.5 s",
        { timeSeconds: [0, 1, 2], traces: [{ role: "measurement", symbol: "P1.ProcessValue", unit: "°C", values: [20, 21, 21.8] }, { role: "prediction", symbol: "y", unit: "°C", values: [20, 20.9, 21.7] }] }))}`);
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "2", "The response plot includes measured data and model prediction");
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /°C/);
    });
  });
  test("edits missing control requirements without inventing targets", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'opened';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "true", "Requirements opens its compact editor");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-requirement=maximumError]').value;"), "", "Absent project requirements remain blank");
      await inner("d.querySelector('[data-studio-editor-done]').click();return 'submitted';");
      // Done sends the rows as typed and the engine refuses a blank one under its field (B4.3): the editor reopens with that refusal, so the
      // marked field is awaited, not read at the press.
      await pollValue("Missing requirements are identified in the editor", () => inner("return String(!!d.querySelector('dialog [aria-invalid=true]'));"), value => value === "true", 15000);
      await inner("for(const [key,value] of Object.entries({referenceChange:5,maximumError:1,settlingBand:0.2,settlingTimeSeconds:8,durationSeconds:16})){d.querySelector('[data-studio-requirement='+key+']').value=String(value);}d.querySelector('[data-studio-editor-done]').click();return 'saved';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      await inner("d.querySelector('[data-studio-requirements]').click();return 'reopened';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-requirement=maximumError]').value;"), "1");
    });
  });
  test("renders the identified plant equation from scoped host state", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      // The engine answers the identified model in its own words: the gain 2, the time constant 4 s and the delay 0.5 s of the model
      // as crates/trust-control/src/identification/presentation.rs writes them (IdentifyResult). The page shows them as they come.
      await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();${engineResult("identify", identifyResult("First-order model with delay", "G(s) = 2 e^(-0.5s) / (4s + 1)",
        "Gain 2 °C per %, time constant 4 s, delay 0.5 s", { timeSeconds: [], traces: [] }))}`);
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "true", "The actual identified model is visible on Identify");
      const equation = await inner("return d.querySelector('[data-studio-model-equation]').textContent;");
      for (const coefficient of ["2", "4", "0.5"]) assert.ok(equation.includes(coefficient), `The equation includes the actual ${coefficient} coefficient`);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false", "A qualified model enables tuning");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').classList.contains('trust-button--primary'));"), "true", "Identify plant stays the primary action for starting another identification");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Identify plant", "Keep the identification action label stable after a result");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-studio-continue-tune]'));"), "false", "The Tune tab is sufficient navigation; do not duplicate it in the action row");
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'Tune tab';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.selectedStage;"), "tune");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]') || d.querySelector('[data-studio-three-page]').dataset.busy === 'true');"), "false", "Navigation starts no work and opens no editor");
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'Identify tab';");
    });
  });
  test("edits experiment limits in one compact dialog and cancels without changing saved values", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("three-page handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await inner("d.querySelector('[data-studio-limits]').click();return 'opened';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "true", "Limits opens the compact editor without navigating away");
      // Durations show in their largest fitting unit, and a plain number still means seconds (Tune page spec).
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), formatDurationSeconds(loop.experiment.durationSeconds));
      await inner("const input=d.querySelector('[data-studio-limit=durationSeconds]');input.value='12';input.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-studio-editor-cancel]').click();return 'cancelled';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      await inner("d.querySelector('[data-studio-limits]').click();return 'opened again';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), formatDurationSeconds(loop.experiment.durationSeconds), "Cancel preserves the prior duration");
      await inner("const input=d.querySelector('[data-studio-limit=durationSeconds]');input.value='12';input.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-studio-editor-done]').click();return 'saved';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      await inner("d.querySelector('[data-studio-limits]').click();return 'reopened';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), "12", "Done retains the duration as it was typed (B9.9: an entry goes back and is shown as typed)");
      await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-limits.png"));
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
  test("shows Identify Tune Validate with page-specific controls and no runtime chrome", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
      });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("rendered stage navigation", () => inner("return String([...d.querySelectorAll('[role=tab]')].some(el=>el.checkVisibility()));"), value => value === "true", 10000);
      await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
      const tabs = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[role=tab]')].filter(el=>el.checkVisibility()).map(el=>el.textContent.trim()));"));
      assert.deepStrictEqual(tabs, ["Identify", "Tune", "Validate"], "The visible workspace has exactly the three agreed pages");
      // The page is ready when the engine's opened answer has selected its page (engineOpened.ts sets hostReady with the selection).
      await pollValue("three-page handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      const visible = (selector: string) => inner(`return String([...d.querySelectorAll(${JSON.stringify(selector)})].some(el=>el.checkVisibility()));`);
      for (const selector of [".values", ".plot-toolbar", "[data-control-studio-source]", "[data-engineer-results]"]) {
        assert.strictEqual(await visible(selector), "false", `${selector} must not add runtime chrome, a toolbar or a report to the page`);
      }
      assert.strictEqual(await inner("return d.querySelector('[role=tab][aria-selected=true]')?.textContent.trim();"), "Identify");
      await inner("[...d.querySelectorAll('[role=tab]')].find(el=>el.checkVisibility()&&el.textContent.trim()==='Tune').click();return 'selected';");
      assert.strictEqual(await inner("return d.querySelector('[role=tab][aria-selected=true]')?.textContent.trim();"), "Tune");
      assert.strictEqual(await visible("[data-engineer-tune]"), "true", "Tune retains its own action when prerequisites are missing");
      await inner("[...d.querySelectorAll('[role=tab]')].find(el=>el.checkVisibility()&&el.textContent.trim()==='Validate').click();return 'selected';");
      assert.strictEqual(await inner("return d.querySelector('[role=tab][aria-selected=true]')?.textContent.trim();"), "Validate");
      assert.strictEqual(await visible("[data-engineer-verify]"), "true");
      assert.strictEqual(await visible("[data-engineer-apply]"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true", "Opening Validate cannot enable an unvalidated application");
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});

let fixtureSequence = 0;

/** A decoder-valid retained recording opened through Identify's real import owner. */
function mimoRecordingArchive(loop: any, id: string): string {
  const value = JSON.parse(recordingArchive(loop, id, 0));
  const samples = value.recording.rawSamples as Array<Record<string, any>>;
  for (const sample of samples) {
    const time = Number(sample.elapsedSeconds);
    const u1 = time >= 5 && time < 10 ? 1 : 0;
    const u2 = time >= 15 && time < 20 ? 1 : 0;
    sample.appliedMv = u1;
    sample.requestedMv = u1;
    sample.pv = time >= 15 && time < 20 ? -1 : time >= 5 && time < 10 ? 2 : 0;
    sample.additional = [
      { id: "u2", channel: "u2", role: "MV", unit: "%", value: u2, quality: "good" },
      { id: "y2", channel: "y2", role: "PV", unit: "°C", value: time >= 15 && time < 20 ? 1.5 : time >= 5 && time < 10 ? -0.5 : 0, quality: "good" },
    ];
  }
  return JSON.stringify(value);
}

export async function withStudio(exercise: (inner: (body: string) => Promise<string>, reopen: () => Promise<void>, capture: (name: string) => Promise<void>) => Promise<void>, options: { loop?: (loop: any) => void; capture?: string } = {}): Promise<void> {
  const initialDebugSession = vscode.debug.activeDebugSession?.id;
  const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
  const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
  loop.id += `-rendered-${++fixtureSequence}`;
  if (loop.parameterBinding) loop.parameterBinding.loop_id = loop.id;
  options.loop?.(loop);
  // A study is kept in the project beside its loop (A8.3), so the fixture's own loop is saved into the project like any other.
  fs.mkdirSync(path.join(project, "control-loops"), { recursive: true });
  fs.writeFileSync(path.join(project, "control-loops", `${loop.id.replace(/[^a-z0-9-]+/gi, "-")}.json`), `${JSON.stringify(loop, null, 2)}\n`);
  await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
  const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
  try {
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    let session = await attachControlStudio(connection, port, "", loop.id);
    const inner = (body: string) => evaluateInWebview(connection, session, body);
    await pollValue("three-page handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
    // A served loop's answers reach the page as `control-studio.engine.*` messages, with no status events: record them for the waits.
    await inner(engineAnswerRecorderScript());
    const reopen = async () => {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port, "", loop.id);
      await pollValue("reopened host state", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      // A reopened editor is a new document, so its engine answers are recorded again.
      await inner(engineAnswerRecorderScript());
    };
    await exercise(inner, reopen, name => captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, `-${name}.png`)));
    if (options.capture) await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, `-${options.capture}.png`));
  } finally {
    connection.close();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    const ownedSession = vscode.debug.activeDebugSession;
    if (ownedSession && ownedSession.id !== initialDebugSession) {
      await vscode.debug.stopDebugging(ownedSession);
      await pollValue("owned simulator closed", async () => String(vscode.debug.activeDebugSession?.id !== ownedSession.id), value => value === "true", 10000);
    }
  }
}

/** One engine answer as a journey reads it from the page's recorder (`engineAnswerRecorderScript`): a small projection. */
type EngineAnswer = Pick<EngineAnswerRecord, "kind" | "method" | "status" | "flowStage" | "reason" | "applicationAction">;

/** What a journey checks of an identification the engine answered (IdentifyResult): its model, and its applied plant input. */
interface EngineIdentification {
  modelType?: string;
  equation?: string;
  plantInput: { unit: string; count: number; minimum: number | null; maximum: number | null } | null;
}

/** A verdict of the engine's Validate answer (RequirementStatus in crates/trust-control/src/identification/independent_validation.rs). */
type Verdict = "Pass" | "Fail" | "Inconclusive";

/** A stage's status in the engine's answers (StageStatus in crates/trust-control/src/stages.rs). */
type StageStatus = "ready" | "needs-data" | "complete" | "failed";

const REPORTED_GAPS = "Checks the engine route cannot pass yet (reported gaps)";
/** The Tune page's check of the engine's candidate: engineOpened.ts hands the Tune result to the page's router as stage "engineer". */
const TUNE_STAGE_GAP = "Tune stage check: the engine's candidate on the Tune page (engineOpened.ts, stage \"engineer\"; threePageWorkflow.ts router)";
/**
 * Runs one check of a value the engine route does not show yet (a reported gap) where it stands, logs it when it fails, and lets the
 * journey go on, so that one run shows every gap; the test fails at its end when any gap failed.
 */
async function reportedGap(gaps: string[], name: string, check: () => unknown): Promise<void> {
  try { await check(); }
  catch (error) {
    const gap = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    gaps.push(gap);
    console.error(`Reported gap: ${gap}`);
  }
}

/** The script that counts the engine answers the page has recorded: a later read takes the answers after this mark. */
const ENGINE_ANSWER_COUNT = "return String((d.defaultView.__engineAnswers||[]).length);";

/** The words the engine names the rows of the Validate table with (crates/trust-control/src/validation/texts.rs). */
const VALIDATE_LABELS = { currentSettings: "Current settings", candidate: "Candidate", candidateOverall: "Candidate overall", automatic: "Automatic", guards: "Ranges, states and transfer", guardsLimit: "Within declared limits" };

/** The stage evidence the engine answers a Validate result with: the model and the candidate it validated (StageEvidence, B9.4). */
const VALIDATION_EVIDENCE = { hasModel: true, hasCandidate: true, validationHasResult: true };

/**
 * The engine's words of a PI candidate's Tune answer (`TuneWords`, crates/trust-control/src/messages/tune_words.rs): its outcome sentence,
 * one row for each setting it names with the row's label and its unit, as the loop's parameter binding names them, and the names of the
 * three gains. The page draws the rows from these words and keeps no names or units of its own, so a row without its unit shows none.
 */
const TUNE_WORDS = {
  outcome: "Meets the requirements",
  settingLabels: [
    { symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)", unit: "%/°C" },
    { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)", unit: "%/(°C*s)" },
  ],
  gainLabels: { proportional: "Proportional gain (Kp)", integral: "Integral gain (Ki)", derivative: "Derivative gain (Kd)" },
};

/** The engine's Validate plot of a candidate run whose measurement rose from 20 to 21 °C, with no current run (ValidationPlot). */
const MEASURED_RESPONSE = { title: "Measured response", role: "Measurement", unit: "°C", tuned: { name: "Tuned", timeSeconds: [0, 1], values: [20, 21] } };

/** The script that reads the engine answers recorded after the first `from`, as small projections (a plot can hold many points). */
function engineAnswersAfter(from: number): string {
  return `return JSON.stringify((d.defaultView.__engineAnswers||[]).slice(${from}).map(e=>({kind:e.kind,method:e.method,status:e.status,flowStage:e.flowStage,reason:e.reason,applicationAction:e.applicationAction})));`;
}

/**
 * Whether an engine answer ends an identification: Identify's own result (`model-ready`), the result of the automatic flow that
 * Identify plant starts (`completed`, or `paused` for an input it waits for; engine/routes/runRoute.ts), or a failure.
 */
function endsIdentification(answer: EngineAnswer): boolean {
  return answer.status === "failed" || (answer.kind === "control-studio.engine.result" && ["model-ready", "completed", "paused"].includes(answer.status));
}

/** Whether an engine answer carries an identification: Identify's result, or the automatic flow's result with its `identification`. */
function answersIdentification(answer: EngineAnswer): boolean {
  return answer.kind === "control-studio.engine.result" && (answer.status === "model-ready" || answer.method === "run");
}

/** Whether an engine answer belongs to Apply, Restore or Keep in project (engineApplication.ts names them by their method). */
function isApplicationAnswer(answer: EngineAnswer): boolean {
  return answer.applicationAction !== undefined || ["apply", "restore", "keep"].includes(answer.method ?? "");
}

/**
 * The script that reads the identification of the engine answer recorded at `index`: the model's type and equation, and a summary of
 * the plot's plant-input trace (its unit, its number of values, its lowest and its highest value), so that the read stays small.
 */
function engineIdentificationAt(index: number): string {
  return `const answer=(d.defaultView.__engineAnswers||[])[${index}];const result=answer&&answer.result||{};`
    + "const found=answer&&answer.method==='run'?result.identification:result;if(!found||typeof found!=='object')return 'null';"
    + "const input=(found.plot&&Array.isArray(found.plot.traces)?found.plot.traces:[]).find(trace=>trace.role==='plant-input');"
    + "const values=input&&Array.isArray(input.values)?input.values.filter(value=>typeof value==='number'):[];"
    + "return JSON.stringify({modelType:found.modelType,equation:found.equation,plantInput:input?{unit:input.unit,count:values.length,"
    + "minimum:values.length?values.reduce((low,value)=>Math.min(low,value)):null,maximum:values.length?values.reduce((high,value)=>Math.max(high,value)):null}:null});";
}

/**
 * The script that records, once, the chart plotting the plant live (a series and no model prediction, threePageChart.ts) while a flow
 * runs and the engine has reported the Identify stage running after the mark `from`, with the number of engine answers recorded by
 * then. The loop monitor feeds that live plot; the engine route sends no live recording of its own.
 */
function liveRecordingObserver(from: number): string {
  return "d.defaultView.__liveRecording=null;{const root=d.querySelector('[data-studio-three-page]');const chart=d.querySelector('[data-control-studio-chart]');"
    + "new d.defaultView.MutationObserver(()=>{const answers=d.defaultView.__engineAnswers||[];const label=chart.getAttribute('aria-label')||'';"
    + "if(d.defaultView.__liveRecording||root.dataset.busy!=='true'||Number(chart.dataset.seriesCount)<1||/Model prediction/.test(label))return;"
    + `if(answers.slice(${from}).some(e=>e.status==='running'&&e.flowStage==='Identify'))d.defaultView.__liveRecording={answers:answers.length,series:chart.dataset.seriesCount,label};`
    + "}).observe(chart,{attributes:true,attributeFilter:['aria-label']});}";
}

/** The script that posts one message as the engine route posts it to the page: its kind, the page's scope and its fields (`scopeFields`). */
function engineMessage(kind: string, fields: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:${JSON.stringify(kind)},projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,...${JSON.stringify(fields)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}

/** The script that posts the engine's answer to `method` (`control-studio.engine.result`, enginePanelRoute.ts). */
function engineResult(method: string, result: Record<string, unknown>): string {
  return engineMessage("control-studio.engine.result", { method, result });
}

/** The stage tabs' statuses as the engine answers them, each with its words (StageAvailability, crates/trust-control/src/stages.rs). */
function engineStages(identify: StageStatus, tune: StageStatus, validate: StageStatus, apply = false): Array<Record<string, string | boolean>> {
  const words: Record<StageStatus, string> = { ready: "Ready", "needs-data": "Needs data", complete: "Complete", failed: "Failed" };
  // `available` is the engine's rule (`stage_available`, crates/trust-control/src/stages.rs): Identify always, Tune with a basis, Validate
  // with a candidate. The fixtures' Tune is complete exactly when a candidate exists. The engine states the Apply stage in every answer
  // that states the stages: it has no tab, and the Validate page's application area keeps whether Apply can start, which in every answer
  // that states the offers is exactly when Apply is offered (`apply`, `application_offers::offered_stages`, B7, B9.4).
  const available: Record<string, boolean> = { Identify: true, Tune: tune === "ready" || tune === "complete", Validate: tune === "complete" };
  return [
    ...([["Identify", identify], ["Tune", tune], ["Validate", validate]] as const).map(([stage, status]) => ({ stage, status, label: words[status], available: available[stage] })),
    { stage: "Apply", status: apply ? "ready" : "needs-data", label: apply ? words.ready : words["needs-data"], available: apply },
  ];
}

/** An Identify answer in the engine's words (IdentifyResult): the model type, the equation, the key numbers and the response plot. */
function identifyResult(modelType: string, equation: string, keyNumbers: string, plot: Record<string, unknown>): Record<string, unknown> {
  return { kind: "identification", modelType, equation, keyNumbers, provisional: false, text: [modelType, equation, keyNumbers].join("\n"), plot,
    recordingId: "rec-rendered-fixture", stageEvidence: { hasModel: true, hasCandidate: false, validationPassed: false },
    stages: engineStages("complete", "ready", "ready") };
}

/** One requirement's row of a Validate answer, on the loop's measurement (RequirementOutcome); `measured` adds its value and limit or ranges. */
function requirementOutcome(id: string, label: string, status: Verdict, unit: string, reason: string, measured: Record<string, unknown> = {}): Record<string, unknown> {
  return { id, label, channel: { role: "Measurement", unit: "°C", symbol: "P1.ProcessValue" }, limitSide: "atMost", status, ...measured, unit,
    source: "temperature-process", reason, automatic: false };
}

/** One scenario of a Validate answer (ScenarioOutcome): the current settings' run and the candidate's, both judged by `requirements`. */
function scenarioOutcome(id: string, label: string, status: Verdict, requirements: Array<Record<string, unknown>>): Record<string, unknown> {
  const run = (settings: string) => ({ runId: `rendered-${id}-${settings}`, projectRevision: "rendered-fixture", evidence: "ST-executed", status, requirements });
  return { id, plant: "temperature-process", label, current: run("current"), candidate: run("candidate") };
}

/** One row of the engine's Validate table as the engine writes it (ValidateRow, validate_answer.rs); its verdict class is the word in lower case. */
function validateTableRow(row: Record<string, string>): Record<string, string> {
  return { symbol: "", source: "", ...row, verdictClass: String(row.verdict).toLowerCase() };
}
