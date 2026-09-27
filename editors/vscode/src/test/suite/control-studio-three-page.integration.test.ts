import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { recordingArchive } from "./control-studio-recording-flow.test";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

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
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.defaultView.__toolEvents=[];d.defaultView.addEventListener('message',e=>{if(e.data.kind==='control-studio.engineering.result')d.defaultView.__toolEvents.push(e.data);});return 'tools';");
      const choose = async (tool: string) => {
        assert.strictEqual(await inner(`const option=d.querySelector('[data-engineer-tuning-method] option[value="analysis:${tool}"]');return String(!!option&&!option.disabled&&!option.hidden);`), "true", `${tool} must be reachable`);
        await inner(`const method=d.querySelector('[data-engineer-tuning-method]');method.value='analysis:${tool}';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.defaultView.__toolEvents=[];return 'selected';`);
      };
      const calculate = async (field: string) => {
        await inner("d.querySelector('[data-engineer-tune]').click();return 'calculate';");
        return JSON.parse(await pollValue(`visible ${field} result`, () => inner(`return JSON.stringify(d.defaultView.__toolEvents.filter(e=>e.payload?.saved?.${field}).map(e=>e.payload.saved.${field}));`), value => JSON.parse(value).length > 0, 15000))[0];
      };
      await choose("response");
      await inner("for(const [key,value] of Object.entries({'response-step':'0.1','response-duration':'2','response-initial':'1,0','response-input':'1'})){const field=d.querySelector('[data-analysis-'+key+']');field.value=value;field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));}return 'response inputs';");
      const response = await calculate("response");
      assert.strictEqual(response.points.length, 21);
      assert.ok(response.points.some((point: any) => point.initialCondition[0] !== 0));
      await capture("initial-response");
      await choose("poles");
      assert.strictEqual(await inner("return d.querySelector('[data-analysis-pole-view]').value;"), "unreduced");
      const full = await calculate("poleView");
      assert.deepStrictEqual(full.unreduced.denominator, [1, 0, -1]);
      await inner("const view=d.querySelector('[data-analysis-pole-view]');view.value='minimal';view.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.defaultView.__toolEvents=[];return 'minimal';");
      const minimal = await calculate("poleView");
      assert.deepStrictEqual(minimal.minimal.denominator, [1, 1]);
      assert.strictEqual(minimal.hiddenUnstableMode, true);
      assert.match(await inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;"), /hidden|unstable/i);
      await capture("minimal-poles");
      await choose("lead-lag");
      await inner("d.querySelector('[data-analysis-lead-lag-crossover]').value='1';d.querySelector('[data-analysis-lead-lag-phase]').value='30';d.querySelector('[data-engineer-tune]').click();return 'lead';");
      const lead = JSON.parse(await pollValue("host lead design", () => inner("return JSON.stringify(d.defaultView.__toolEvents.filter(e=>e.stage==='lead-lag'&&e.payload.status==='complete').map(e=>e.payload.saved));"), value => JSON.parse(value).length > 0, 15000))[0];
      assert.strictEqual(lead.mode, "lead"); assert.ok(lead.result.gain > 0);
      assert.ok(!(await inner("return d.querySelector('[data-three-page-analysis-summary]').textContent;" )).includes("[object Object]"));
      await capture("lead-design");
      await inner("const mode=d.querySelector('[data-analysis-lead-lag-mode]');mode.value='lag';mode.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.querySelector('[data-analysis-lead-lag-ratio]').value='5';d.defaultView.__toolEvents=[];d.querySelector('[data-engineer-tune]').click();return 'lag';");
      await pollValue("host lag design", () => inner("return JSON.stringify(d.defaultView.__toolEvents.filter(e=>e.stage==='lead-lag'&&e.payload.status==='complete').map(e=>e.payload.saved.mode));"), value => JSON.parse(value).includes("lag"), 15000);
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune again';");
      await choose("lead-lag");
      assert.strictEqual(await inner("return d.querySelector('[data-analysis-lead-lag-mode]').value;"), "lag");
      await capture("lag-restored");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Analytical tools do not start or write the runtime");
    });
  });
  test("executes the project-owned sequence and retains its restored native recording", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_PROJECT_NATIVE !== "1") this.skip();
    this.timeout(90000);
    try { await withStudio(async inner => {
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='project';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.defaultView.__projectEvents=[];d.defaultView.addEventListener('message',event=>{if(event.data.kind==='control-studio.experiment.result')d.defaultView.__projectEvents.push(event.data);});d.querySelector('[data-engineer-find]').click();return 'review';");
      const reviewed = JSON.parse(await pollValue("native project review", () => inner("return JSON.stringify(d.defaultView.__projectEvents);"), value => JSON.parse(value).some((message: any) => message.phase === "review"), 45000));
      assert.strictEqual(reviewed.find((message: any) => message.phase === "review")?.payload.kind, "accepted", JSON.stringify(reviewed));
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Confirm and start");
      await inner("d.querySelector('[data-engineer-find]').click();return 'start';");
      const completed = JSON.parse(await pollValue("native project terminal", () => inner("return JSON.stringify(d.defaultView.__projectEvents);"), value => JSON.parse(value).some((message: any) => message.phase === "terminal" || message.phase === "failed"), 20000));
      const terminal = completed.find((message: any) => message.phase === "terminal")?.payload;
      assert.strictEqual(terminal?.status, "completed", JSON.stringify(completed));
      assert.strictEqual(terminal?.restoration, "restored", JSON.stringify(terminal));
      assert.ok(terminal.recording.rawSamples.length >= 10, "Retain runtime-scan data from the project sequence");
      await pollValue("native measured project response", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => Number(value) >= 2, 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-acquisition-method]').disabled);"), "false", "Finishing restoration must unlock method selection");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-import]').disabled);"), "false", "The recording action must unlock after the project test");
    }, { capture: "native-project", loop: loop => { loop.experiment.durationSeconds = 2; } });
    } finally { if (vscode.debug.activeDebugSession) await vscode.debug.stopDebugging(vscode.debug.activeDebugSession); }
  });
  test("fits a manually selected model from independent recordings in Identify", async function () {
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
        await pollValue("manual fit available", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Fit plant model", 10000);
        await inner("d.querySelector('[data-engineer-find]').click();return 'fit';");
        await pollValue("manual fit complete", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => /Open validation recording|Use fitted model/.test(value), 15000);
      }
      await inner("d.querySelector('[data-engineer-find]').click();return 'accept';");
      await pollValue("accepted manual model", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Model ready", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    }, { capture: "manual-identification" });
  });
  test("offers the declared project experiment and plots selected recorded signals in their own units", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-acquisition-method] option[value=project]'));"), "true", "Project-owned lessons have a reachable experiment method");
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='project';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'project';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-experiment]').checkVisibility());"), "true");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Review project test");
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const recording = JSON.parse(recordingArchive(loop, "project-recorded-signals", 2)).recording;
      await inner(`const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.recording.evidence',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,payload:{recording:${JSON.stringify(recording)}}},'*');return 'recorded';`);
      await pollValue("project experiment signals", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => Number(value) >= 2, 10000);
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /°C/);
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /%/);
    }, { capture: "project-signals", loop: loop => { loop.experiment = { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 20, seed: 1 }; loop.signals.manipulatedVariable.direction = "read"; } });
  });

  test("runs measured MIMO windows through the retained project recording and restores them", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_MIMO_RENDERED !== "1") this.skip();
    this.timeout(180000);
    await withStudio(async (inner, reopen) => {
      const loop = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      if (!loop.channels || loop.channels.inputs.length < 2) this.skip();
      await inner("d.defaultView.__mimoResult=undefined;d.defaultView.addEventListener('message',event=>{if(event.data.stage==='mimo'&&event.data.payload?.status==='complete')d.defaultView.__mimoResult=event.data.payload;});return 'observe MIMO';");
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='recorded-fit';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'recorded-fit';");
      const archive = mimoRecordingArchive(loop, "rendered-mimo");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(archive)}],'mimo-recording.json',{type:'application/json'}));const file=d.querySelector('[data-identify-import-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'open-recording';`);
      await pollValue("MIMO recording import", () => inner("return d.querySelector('[data-engineer-find]').textContent;"), value => value === "Check recording", 15000);
      assert.strictEqual(await inner("const option=d.querySelector('[data-engineer-tuning-method] option[value=\"analysis:measured-mimo\"]');return String(!!option && !option.hidden && !option.disabled);"), "true", "A retained declared multichannel recording must expose measured MIMO paths in Tune");
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const method=d.querySelector('[data-engineer-tuning-method]');method.value='analysis:measured-mimo';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'mimo';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-mimo-analysis]').checkVisibility());"), "true");
      assert.strictEqual(await inner("return String(d.documentElement.scrollWidth<=d.documentElement.clientWidth);"), "true", "MIMO fields must fit the actual editor width");
      assert.strictEqual(await inner("return d.defaultView.getComputedStyle(d.querySelector('[data-mimo-window]')).backgroundColor;"), await inner("return d.defaultView.getComputedStyle(d.querySelector('[data-engineer-tuning-method]')).backgroundColor;"), "MIMO fields must use the shared input theme");
      const values = ["1.9", "4.8", "5.2", "9.8", "10.2", "14.8", "15.2", "19.8"];
      await inner(`const fields=[...d.querySelectorAll('[data-mimo-window]')];${JSON.stringify(values)}.forEach((value,index)=>{fields[index].value=value;fields[index].dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));});return 'windows';`);
      await inner("d.querySelector('[data-engineer-tune]').click();return 'calculate';");
      await pollValue("measured MIMO result", () => inner("return String(d.defaultView.__mimoResult?.result?.status);"), value => value === "qualified", 15000);
      assert.ok((await inner("return d.querySelector('[data-three-page-mimo-analysis-gain]')?.textContent||'';" )).length > 0, "The host result must render the measured gain matrix");
      assert.ok((await inner("return d.querySelector('[data-three-page-mimo-analysis-pairing]')?.textContent||'';" )).includes("Pairing"));
      await inner("const field=d.querySelector('[data-mimo-window]');field.value='2.1';field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'edit-window';");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'recalculate';");
      await pollValue("recalculated measured MIMO result", () => inner("return String(d.defaultView.__mimoResult?.trials?.[0]?.baseline.startSeconds);"), value => value === "2.1", 15000);
      assert.ok((await inner("return d.querySelector('[data-three-page-mimo-analysis-gain]')?.textContent||'';" )).length > 0, "Recalculation must retain the host gain matrix");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const method=d.querySelector('[data-engineer-tuning-method]');method.value='analysis:measured-mimo';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'reopened';");
      await pollValue("restored MIMO windows", () => inner("return JSON.stringify([...d.querySelectorAll('[data-mimo-window]')].map(input=>input.value));"), value => value.includes("2.1") && value.includes("15.2"), 20000);
      assert.ok((await inner("return d.querySelector('[data-three-page-mimo-analysis-gain]')?.textContent||'';" )).length > 0, "Reopening must retain the measured gain matrix");
    }, { capture: "measured-mimo", loop: loop => {
      loop.signals.additional = [
        { id: "u2", channel: "u2", role: "MV", symbol: "P1.ManipulatedValue2", address: "%QL128", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: -10, maximum: 10 } },
        { id: "y2", channel: "y2", role: "PV", symbol: "P1.ProcessValue2", address: "%IL128", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      ];
      loop.channels = {
        inputs: [
          { modelChannelId: "u1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 0, maximum: 100 } },
          { modelChannelId: "u2", signalId: "u2", actuator: { kind: "continuous", minimum: -10, maximum: 10 } },
        ],
        outputs: [
          { modelChannelId: "y1", signalId: "processVariable" },
          { modelChannelId: "y2", signalId: "y2" },
        ],
      };
    } });
  });
  test("runs the declared root locus from Tune without a candidate or runtime write", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-tuning-method] option[value=\"analysis:root-locus\"]'));"), "true", "The existing tuning method selector exposes the declared root locus");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='analysis:root-locus';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'root locus';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false", "Read-only declared analysis does not require a tuned candidate");
      await inner("d.defaultView.__analysisMessages=[];d.defaultView.addEventListener('message',event=>{if(event.data.kind==='control-studio.engineering.result')d.defaultView.__analysisMessages.push(event.data);});d.querySelector('[data-engineer-tune]').click();return 'calculate';");
      const messages = await pollValue("root locus host result", () => inner("return JSON.stringify(d.defaultView.__analysisMessages);"), value => JSON.parse(value).some((message: any) => message.stage === "declared-root"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-three-page-analysis-result]')?.checkVisibility());"), "true", messages);
      assert.strictEqual(await inner("const message=d.querySelector('[data-studio-message=tune]');return message?.checkVisibility()?message.textContent:'';"), "", "A completed declared analysis must not still demand a plant model");
      assert.strictEqual(vscode.debug.activeDebugSession, undefined);
    }, { capture: "root-locus", loop: loop => { loop.analysis = {
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
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-acquisition-method]').value;"), "supplied-model", "Reopening preserves the supplied-model entry path");
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
      await inner(hostUpdate({ status: "failed", reason: "The load-change run was interrupted.", requiredScenarios: ["target-response", "load-change"],
        scenarioResults: [{ id: "target-response", result: { evaluation: { status: "Pass", requirements: [{id:"settling",kind:"settling",status:"Pass",value:4,limit:5,unit:"s"}], specification: [{id:"settling",kind:"settling",channel:"P1.ProcessValue"}] } } }] }));
      assert.strictEqual(await inner("return String(d.querySelector('.studio-response').getBoundingClientRect().height===0);"), "true", "A missing response does not reserve an empty chart above the action");
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
      const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
      const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
      for (const [id, amplitude] of [["manual-model-estimation", 2], ["manual-model-validation", 1.5]] as const) {
        await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(recordingArchive(loop, id, amplitude))}],'recording.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';`);
        await pollValue("opened model analysis", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 15000);
      }
      await pollValue("Tune requirements", () => inner("return String(!!d.querySelector('dialog[open]'));"), value => value === "true", 10000);
      await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'cancelled';");
      await inner(hostUpdate({ status: "candidate", family: "PI", availableFamilies: ["P", "PI", "PD", "PID"],
        values: { "P1.Parameters.ProportionalGain": 1.23456789012345, "P1.Parameters.IntegralGain": .5, "P1.Parameters.DerivativeGain": 0 },
        stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false } }));
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-manual-value]').length);"), "2", "Manual PI shows only its relevant settings");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-manual-value]').value;"), "1.23456789012345");
      assert.ok((await inner("return d.querySelector('[data-studio-tuning-result]').textContent;")).includes("Proportional gain (Kp)"));
      await inner("const input=d.querySelector('[data-studio-manual-value]');input.value='2.123456789';input.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));const pin=d.querySelector('[data-studio-manual-pin]');pin.checked=true;pin.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'pinned';");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='automatic';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'automatic';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-manual-value]').length);"), "0", "Automatic results stay plain and compact");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual again';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-manual-value]').value;"), "2.123456789");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-manual-pin]').checked);"), "true");
      await inner("const input=d.querySelector('[data-studio-manual-value]');input.value='';input.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));d.querySelector('[data-engineer-tune]').click();return 'incomplete';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-manual-value]').getAttribute('aria-invalid');"), "true");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.busy??'false';"), "false", "An incomplete manual value does not start Tune");
    }, { capture: "manual-settings", loop: loop => { loop.id += "-manual-fields"; loop.parameterBinding.loop_id = loop.id; } });
  });
  test("renders state-design policy and preserves finite design drafts across reopen", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async (inner, reopen) => {
      const loopSnapshot = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      const supplied = { kind: "continuous-state-space", revision: "state-design-rendered-model", source: "provided", validation: "validated", evidence: ["independent"], delaySeconds: 0,
        inputs: [{ id: "u", unit: loopSnapshot.signals.manipulatedVariable.unit, operatingRange: loopSnapshot.signals.manipulatedVariable.range }], outputs: [{ id: "y", unit: loopSnapshot.signals.processVariable.unit, operatingRange: loopSnapshot.signals.processVariable.range }], states: [{ id: "x", unit: loopSnapshot.signals.processVariable.unit, operatingRange: loopSnapshot.signals.processVariable.range }],
        a: [[-1]], b: [[1]], c: [[1]], d: [[0]], operatingPoint: { inputs: [0], outputs: [0] } };
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
      await inner("d.querySelector('[data-engineer-find]').click();return 'open model';");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(JSON.stringify(supplied))}],'state-model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'model imported';`);
      await pollValue("supplied state model", () => inner("return d.querySelector('[data-studio-model-equation]')?.textContent??'';"), value => value.length > 0, 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      await pollValue("state design requirements", () => inner("return String(!!d.querySelector('dialog[open]'));"), value => value === "true", 5000);
      await inner("const set=(key,value)=>{const field=d.querySelector(`[data-studio-requirement=\"${key}\"]`);field.value=String(value);field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));};set('referenceChange',1);set('maximumError',.1);set('settlingBand',.02);set('settlingTimeSeconds',2);set('durationSeconds',4);d.querySelector('[data-studio-editor-done]').click();return 'requirements set';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-tuning-method] option[value=state-design]'));"), "true", "State-space design must be an actual Tune method");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='state-design';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'state design';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-state-design-mode]'));"), "true");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=inputWeights]').value;"), "[[1]]", "State design must show the declared input weights used by the solver");
      await inner("d.defaultView.__stateDesignTrace=[];d.defaultView.addEventListener('message',e=>{if(e.data.kind==='control-studio.engineering.result'&&e.data.stage==='engineer'){const p=e.data.payload;d.defaultView.__stateDesignTrace.push({status:p.status,family:p.family,preferences:p.tuningPreferences,horizon:p.design?.horizon});}});return 'trace';");
      await inner("const mode=d.querySelector('[data-state-design-mode]');mode.value='finite';mode.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));d.querySelector('[data-state-design-field=\"horizonSteps\"]').value='3';d.querySelector('[data-state-design-field=\"horizonSteps\"]').dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));d.querySelector('[data-state-design-field=\"terminalWeights\"]').value='[[1]]';d.querySelector('[data-state-design-field=\"terminalWeights\"]').dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'finite';");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=\"horizonSteps\"]').value;"), "3");
      await inner("const q=d.querySelector('[data-state-design-field=\"stateWeights\"]');q.value='[[';q.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));d.querySelector('[data-engineer-tune]').click();return 'invalid';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-state-design-field=\"stateWeights\"]').getAttribute('aria-invalid'));"), "true", "Invalid state design must block Tune instead of reusing stale weights");
      await inner("const q=d.querySelector('[data-state-design-field=\"stateWeights\"]');q.value='[[1]]';q.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'repaired';");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune state model';");
      await pollValue("state-design candidate", () => inner("return d.querySelector('[data-studio-tuning-result]')?.textContent??'';"), value => /gain|Riccati|finite/i.test(value), 15000);
      // Automatic Tune continues to Validate. Review the candidate on its own tab.
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'review candidate';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-tuning-result]')?.textContent?.length>0);"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-design-policy]')?.checkVisibility()===true);"), "true", await inner("return 'Finite state design must expose its repeated-first-gain policy: '+JSON.stringify(d.defaultView.__stateDesignTrace);"));
      assert.match(await inner("return d.querySelector('[data-studio-design-policy]').textContent;"), /Repeated first gain/i);
      assert.ok(Number(await inner("return String(d.querySelectorAll('[data-studio-riccati-stage]').length);")) >= 2, "The finite design must expose N+1 Riccati stages");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const method=d.querySelector('[data-engineer-tuning-method]');method.value='state-design';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'reopened';");
      assert.strictEqual(await inner("return d.querySelector('[data-state-design-field=\"horizonSteps\"]').value;"), "3", "Finite draft survives reopening");
    }, { capture: "state-design", loop: loop => {
      loop.schemaVersion = 2;
      loop.controller.design = "project";
      loop.controller.type = "State feedback";
      loop.controller.law = undefined;
      delete loop.controller.pidInitialization;
      loop.controller.stateFeedback = { family: "LQR", convention: "sampled-deviation-negative-feedback", gainSymbols: [["P1.Kp"]], referenceGainSymbols: [["P1.N"]], stateWeights: [[1]], inputWeights: [[1]] };
      const gainUnit = `${loop.signals.manipulatedVariable.unit}/${loop.signals.processVariable.unit}`;
      loop.parameterBinding = { loop_id: loop.id, task_name: "Main", mode: { symbol: "P1.Mode", manual_value: false }, parameters: [
        { symbol: "P1.Kp", iec_type: "LREAL", unit: gainUnit, convention: "state feedback gain", minimum: -100, maximum: 100 },
        { symbol: "P1.N", iec_type: "LREAL", unit: gainUnit, convention: "reference gain", minimum: -100, maximum: 100 },
      ] };
    } });
  });
  test("asks for the missing experiment duration before live identification and leaves the field blank", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.querySelector('[data-engineer-find]').click();return 'identify';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "true", "Missing limits open before any live excitation");
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
      await inner("d.defaultView.__studioEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',event=>d.defaultView.__studioEvents.push(event.detail));return 'listening';");
      const first = archive("opened-extra-output-estimation", 0.25);
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(first)}],'recording-with-extra-output-estimation.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected';`);
      await pollValue("opened archive with extra output", () => inner("return d.querySelector('[data-studio-message=model]').textContent;"), value => value.includes("independent recording"), 15000);
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
      const events = JSON.parse(await inner("return JSON.stringify(d.defaultView.__studioEvents);")) as Array<{ status?: string; identificationPresentation?: { outputId?: string; outputFits?: Array<{ id: string }> } }>;
      const model = [...events].reverse().find(event => event.status === "model-ready");
      const presentation = model?.identificationPresentation;
      const selectedOutput = presentation?.outputId === extra.id || presentation?.outputFits?.some(output => output.id === extra.id);
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
      const checks = Array.from({ length: 32 }, (_, index) => ({ id: `controller-state:${index}`, kind: "state-bounds", status: "Pass", value: 0, limit: 0, unit: "°C", reason: "Within limits." }));
      checks.push({ id: "tracking-error", kind: "error", status: "Fail", value: 2, limit: 1, unit: "°C", reason: "The measured error exceeds the limit." });
      const evaluation = { status: "Fail", run: { channels: [{ id: "P1.ProcessValue", unit: "°C" }], samples: [{ timeSeconds: 0, values: { "P1.ProcessValue": { value: 20, quality: "good" } } }, { timeSeconds: 1, values: { "P1.ProcessValue": { value: 21, quality: "good" } } }] },
        requirements: checks, specification: checks.map(check => ({ id: check.id, channel: "P1.ProcessValue", kind: check.kind, minimum: 0, maximum: 100, window: { start: 0, end: 1 } })) };
      await inner(hostUpdate({ status: "verification-failed", candidate: evaluation, scenarioResults: [{ id: "nominal", result: { evaluation } }] }));
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'inspect results';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-validation-result] tbody tr').dataset.studioRequirement;"), "tracking-error", "Failures appear directly at the top of the results");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-validation-result] tbody tr').length);"), "33", "Keep every mandatory check");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').getBoundingClientRect().bottom<=d.defaultView.innerHeight-8);"), "true", "Apply remains visible beside the response, without scrolling through technical checks");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true", "A failed requirement keeps Apply unavailable");
    });
  });
  test("starts guarded application and reports rejection without claiming adoption", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner(hostUpdate({ status: "verified", stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: true } }));
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();d.defaultView.__application=[];d.defaultView.addEventListener('trust-control-studio-engineer',event=>{if(event.detail.applicationAction)d.defaultView.__application.push(event.detail);});return 'listen';");
      assert.strictEqual(await inner("d.querySelector('[data-engineer-apply]').click();return d.querySelector('[data-engineer-apply]').textContent;"), "Applying…");
      const messages = JSON.parse(await pollValue("unverified application rejection", () => inner("return JSON.stringify(d.defaultView.__application);"), text => JSON.parse(text).some((event: {status:string}) => event.status === "failed"), 10000));
      assert.ok(messages.some((event: {status:string}) => event.status === "failed"));
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-application] button').length);"), "1");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Apply");
      assert.ok(!(await inner("return d.querySelector('[data-studio-message=verify]').textContent;")).includes("Applied"));
    });
  });
  test("shows tuned settings and every mandatory validation requirement on their pages", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner(hostUpdate({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 5, "P1.Parameters.IntegralGain": 1 },
        stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false },
        plots: { timeResponse: [{ timeSeconds: 0, currentPv: 20, candidatePv: 20 }, { timeSeconds: 1, currentPv: 20.5, candidatePv: 21 }, { timeSeconds: 2, currentPv: 21, candidatePv: 21.8 }] } }));
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'inspect tuning';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-tuning-result]').checkVisibility());"), "true");
      const tuning = await inner("return d.querySelector('[data-studio-tuning-result]').textContent;");
      assert.match(tuning, /PI/); assert.match(tuning, /Proportional gain/i); assert.match(tuning, /5/); assert.match(tuning, /%/);
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "2", "Compare current and tuned predictions");
      const run = { channels: [{ id: "P1.ProcessValue", unit: "°C" }], samples: [{ timeSeconds: 0, values: { "P1.ProcessValue": { value: 20, quality: "good" } } }, { timeSeconds: 1, values: { "P1.ProcessValue": { value: 21, quality: "good" } } }] };
      const evaluation = { status: "Pass", run, requirements: [{ id: "nominal-error", kind: "error", status: "Pass", value: 0.02, limit: 0.1, unit: "°C", reason: "Within limit." }, { id: "nominal-settling", kind: "settling", status: "Pass", value: 4, limit: 8, unit: "s", reason: "Within deadline." }], specification: [{ id: "nominal-error", channel: "P1.ProcessValue", kind: "error", maximum: 0.1 }, { id: "nominal-settling", channel: "P1.ProcessValue", kind: "settling", deadlineSeconds: 8 }] };
      await inner(hostUpdate({ status: "verified", candidate: evaluation, scenarioResults: [{ id: "nominal", result: { run, evaluation } }], stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: true } }));
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'inspect validation';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-validation-result] tbody tr').length);"), "2", "Retain both mandatory requirements");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "1", "Show measured validation output");
      await inner(hostUpdate({ status: "edited", stageEvidence: { hasModel: true, hasCandidate: false, validationPassed: false } }));
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-validation-result]').hidden);"), "true", "Previous passing rows are no longer presented as current validation after inputs change");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "0", "Stale response is retained in history, not shown for current inputs");
    });
  });
  test("starts identification from its visible button with the actual Control.PID plant", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(120000);
    await withStudio(async (inner, reopen) => {
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__studioEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',event=>d.defaultView.__studioEvents.push(event.detail));d.querySelector('[data-engineer-find]').click();return 'started';");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Abort", "Identify starts the operation and offers Abort");
      const finished = JSON.parse(await pollValue("native identification result", () => inner("return JSON.stringify(d.defaultView.__studioEvents);"), text => JSON.parse(text).some((event: {status: string}) => ["model-ready", "failed", "paused"].includes(event.status)), 60000));
      const model = finished.find((event: {status: string}) => event.status === "model-ready");
      assert.ok(model?.model, JSON.stringify(finished));
      assert.ok(model.identificationPresentation?.predictedResponse?.length > 2, "The native model has measured-versus-predicted evidence");
      assert.ok(finished.slice(0, finished.indexOf(model)).some((event: any) => event.identificationExperiment?.inputs?.length && event.identificationExperiment?.outputs?.length), "Show the actual experiment recording before the model result");
      const experiment = model.identificationPresentation.experiment;
      assert.ok(experiment?.inputs?.length, "Retain the applied plant input with its model response");
      const inputValues = experiment.inputs[0].points.map((point: any) => point.value);
      assert.ok(Math.min(...inputValues) === 0 && Math.max(...inputValues) > 0, "The chart evidence includes the applied excitation");
      assert.strictEqual(experiment.inputs[0].unit, "%");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3", "Plot applied input, measured output and model prediction together");
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /Applied input.*%/);

      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'inspect identified model';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "true");
      await pollValue("initial identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      const terminalEvents = JSON.parse(await inner("return JSON.stringify(d.defaultView.__studioEvents.map(e=>({status:e.status,reason:e.reason,flowStage:e.flowStage})));"));
      assert.ok(terminalEvents.some((event: any) => event.status === "completed" && event.flowStage === "Identify"), JSON.stringify(terminalEvents));
      assert.ok(!terminalEvents.some((event: any) => event.status === "paused"), "Completed trial cleanup must not overwrite the model-ready result");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.defaultView.__studioEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',event=>d.defaultView.__studioEvents.push(event.detail));return 'reopened';");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3", "Reopening preserves the actual experiment input and model comparison");
      await inner("d.querySelector('[data-studio-reset-chart]').click();return 'reset';");
      await pollValue("reset study", () => inner("return String(d?.querySelector('[data-studio-three-page]')?.dataset.hostReady === 'true' && d?.querySelector('[data-studio-model-result]')?.hidden === true);").catch(() => "loading"), value => value === "true", 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-model-equation]').textContent;"), "", "Reset clears the model equation");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.pointCount;"), "0", "Reset clears the displayed experiment");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "true");
      await inner("d.defaultView.__studioEvents=[];d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__studioEvents.push(e.detail));return 'listen';");
      await inner("d.querySelector('[data-engineer-find]').click();return 'new identification';");
      const repeated = JSON.parse(await pollValue("repeated native identification result", () => inner("return JSON.stringify(d.defaultView.__studioEvents.map(e=>({status:e.status,flowStage:e.flowStage,reason:e.reason})));"), text => JSON.parse(text).some((event: {status: string}) => ["completed", "failed", "paused"].includes(event.status)), 60000));
      assert.ok(repeated.some((event: {status:string;flowStage:string}) => event.status === "completed" && event.flowStage === "Identify"), JSON.stringify(repeated));
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /Model prediction/, "The new result replaces the cleared chart");
    }, { capture: "identify-and-reset-chart" });
  });
  test("plots measured and predicted identification responses with their units", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engineering.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,stage:'engineer',payload:{status:'model-ready',identificationPresentation:{units:{output:'°C'},predictedResponse:[{timeSeconds:0,measured:20,predicted:20},{timeSeconds:1,measured:21,predicted:20.9},{timeSeconds:2,measured:21.8,predicted:21.7}]}}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`);
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
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog [aria-invalid=true]'));"), "true", "Missing requirements are identified in the editor");
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
      await inner(`d.querySelector('[data-engineer-stage-tab=model]').click();const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engineering.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,stage:'engineer',payload:{status:'model-ready',model:{kind:'transfer-function',numerator:[2],denominator:[4,1],delaySeconds:0.5,revision:'model-1',source:'fitted',validation:'validated',evidence:[],inputs:[{id:'input',unit:'%',operatingRange:{minimum:0,maximum:100}}],outputs:[{id:'output',unit:'°C',operatingRange:{minimum:0,maximum:100}}]}}},'*');await new Promise(resolve=>setTimeout(resolve,50));return 'received';`);
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
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), String(loop.experiment.durationSeconds));
      await inner("const input=d.querySelector('[data-studio-limit=durationSeconds]');input.value='12';input.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-studio-editor-cancel]').click();return 'cancelled';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      await inner("d.querySelector('[data-studio-limits]').click();return 'opened again';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), String(loop.experiment.durationSeconds), "Cancel preserves the prior duration");
      await inner("const input=d.querySelector('[data-studio-limit=durationSeconds]');input.value='12';input.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-studio-editor-done]').click();return 'saved';");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      await inner("d.querySelector('[data-studio-limits]').click();return 'reopened';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-limit=durationSeconds]').value;"), "12", "Done retains the edited duration");
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
  const decoded = require("../../controlStudio/loopDefinition").decodeControlLoopDefinition(loop);
  assert.strictEqual(decoded.kind, "accepted", `The rendered fixture must be valid before opening: ${JSON.stringify(decoded)}`);
  const validated = require("../../controlStudio/loopDefinition").validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId });
  assert.strictEqual(validated.kind, "accepted", `The rendered fixture bindings must be valid: ${JSON.stringify(validated)}`);
  await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
  const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
  try {
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    let session = await attachControlStudio(connection, port, "", loop.id);
    const inner = (body: string) => evaluateInWebview(connection, session, body);
    await pollValue("three-page handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
    const reopen = async () => {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port, "", loop.id);
      await pollValue("reopened host state", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
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

function hostUpdate(payload: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engineering.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,stage:'engineer',payload:${JSON.stringify(payload)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}
