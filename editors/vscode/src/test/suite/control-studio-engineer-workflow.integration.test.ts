import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import { getSelectedRuntimeId } from "../../selectedRuntime";
import { attachControlStudio, engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { stageTabs, type EvaluateWebview } from "./control-studio-navigation-assertions";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

/** The reported gaps: checks the three-page route does not pass yet. Each runs where it stands and fails the journey at its end. */
const REPORTED_GAPS = "Checks the three-page route does not pass yet (reported gaps)";
/** The Tune page's gap, worded as the three-page reference suite words it (it reports the candidate as not shown on the Tune page). */
const TUNE_STAGE_GAP = "Tune stage check: the engine's candidate on the Tune page (engineOpened.ts, stage \"engineer\"; threePageWorkflow.ts router)";
/** The normal workspace's three pages (specification 35, B9.1). Apply and Restore previous settings belong to Validate. */
const STAGE_TABS = [
  { stage: "model", label: "Identify", selected: "true", disabled: false },
  { stage: "tune", label: "Tune", selected: "false", disabled: false },
  { stage: "verify", label: "Validate", selected: "false", disabled: false },
];
/** The number of engine answers the page has recorded (`window.__engineAnswers`, control-studio-rendered-harness.ts). */
const ANSWER_COUNT = "return String((d.defaultView.__engineAnswers||[]).length);";
/** The values of the open Requirements editor by their keys (threePageRequirements.ts, `data-studio-requirement`). */
const EDITOR_VALUES = "return JSON.stringify(Object.fromEntries([...d.querySelectorAll('[data-trust-compact-editor] [data-studio-requirement]')].map(input=>[input.dataset.studioRequirement,input.value])));";

/** Runs one check of a gap the three-page route does not close yet, logs it when it fails, and lets the journey go on. */
async function reportedGap(gaps: string[], name: string, check: () => unknown): Promise<void> {
  try { await check(); }
  catch (error) {
    const gap = `${name}: ${error instanceof Error ? error.message : String(error)}`;
    gaps.push(gap);
    console.error(`Reported gap: ${gap}`);
  }
}

/**
 * A control request to the started simulator, sent as the deleted runtime connection sent it (`bindRuntimeConnection` at HEAD): the
 * selected runtime, then the lifecycle's endpoint and its control token, with the same five-second timeout.
 */
async function runtimeRequest(runtimeId: string, operation: string, params: Record<string, unknown>): Promise<any> {
  if (getSelectedRuntimeId() !== runtimeId) throw new Error("The selected runtime does not match this loop.");
  const lifecycle = await runtimeLifecycleService.snapshot();
  if (!lifecycle.status.running) throw new Error("Start the bound runtime before reviewing this operation.");
  const endpoint = lifecycle.status.endpoint;
  const token = await runtimeLifecycleService.controlAuthToken(endpoint);
  return sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 5000 });
}

/**
 * The last recorded answer of `method` after the mark `from` (control-studio-rendered-harness.ts projects every engine answer): its
 * status, a Tune candidate's family and settings, and what a Validate answer holds of its evidence, its plot and its table.
 * An answer the page never recorded reads as `null`.
 */
function lastAnswerOf(method: string, from: number): string {
  return `const list=(d.defaultView.__engineAnswers||[]).slice(${from}).filter(e=>e.method===${JSON.stringify(method)});const e=list[list.length-1];`
    + "if(!e)return 'null';const r=e.result||{};const plot=r.plot||{};const scenarios=Array.isArray(r.scenarios)?r.scenarios:[];"
    + "const evidence=[...new Set(scenarios.flatMap(s=>[s.current&&s.current.evidence,s.candidate&&s.candidate.evidence]).filter(Boolean))];"
    + "return JSON.stringify({status:e.status,family:e.family,values:e.values,evidence,"
    + "plotNames:[plot.tuned&&plot.tuned.name,plot.current&&plot.current.name].filter(Boolean),"
    + "series:(plot.tuned?1:0)+(plot.current?1:0),rows:Array.isArray(r.table)?r.table.length:0});";
}

/** A message as the engine route posts it to the page: its kind, the page's scope and its fields (engineMessage in enginePanelRoute.ts). */
function engineMessage(kind: string, fields: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:${JSON.stringify(kind)},projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,...${JSON.stringify(fields)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}

/** The requirement values of the Requirements editor, read and closed with Cancel (threePageRequirements.ts). */
async function requirementValues(inner: EvaluateWebview): Promise<Record<string, string>> {
  await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
  await pollValue("requirements editor", () => inner("return String(Boolean(d.querySelector('[data-trust-compact-editor] [data-studio-requirement]')));"), value => value === "true", 10000);
  const values: Record<string, string> = JSON.parse(await inner(EDITOR_VALUES));
  await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'cancel requirements';");
  await pollValue("requirements editor closed", () => inner("return String(!d.querySelector('[data-trust-compact-editor]'));"), value => value === "true", 10000);
  return values;
}

/**
 * Identify again keeps the supplied model and the requirements across a page change (specification 35, B9.4): the model is the same
 * equation, the Open model action stays enabled, and the requirement values are the ones entered before.
 */
async function assertModelNavigationPreservesSettings(inner: EvaluateWebview, model: string, requirements: Record<string, string>): Promise<void> {
  await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify navigation';");
  assert.deepStrictEqual((await stageTabs(inner)).map(tab => tab.selected), ["true", "false", "false"],
    "clicking Identify from Tune must select Identify without changing the workflow state");
  assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify({text:d.querySelector('[data-engineer-find]')?.textContent?.trim(),disabled:d.querySelector('[data-engineer-find]')?.disabled,model:d.querySelector('[data-studio-model-equation]')?.textContent??''});")),
    { text: "Open model…", disabled: false, model }, "an existing supplied model must keep its Open model action enabled on Identify");
  await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune navigation';");
  assert.deepStrictEqual(await requirementValues(inner), requirements, "stage navigation must preserve the selected model and task settings");
}

/** Validate's recorded table and Apply offer survive a visit to another page: nothing is validated again (specification 35, B9.4). */
async function assertVerifiedNavigationPreservesEvidence(inner: EvaluateWebview, rows: number): Promise<void> {
  await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify after validate';");
  await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate after identify';");
  assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-validation-result] tbody tr').length);"), String(rows),
    "navigation must preserve the recorded verification table");
  assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false",
    "navigation must keep the verified Apply offer");
}

suite("Control Studio ordinary engineer workflow", () => {
  test("supplies a model tunes verifies actual ST and applies then rolls back a proportional loop", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.controller.modeSignal = "Main.Manual"; loop.controller.modeWhenTrue = "Manual";
    loop.controller.manualOutputType = "LREAL"; // Matches this fixture's declared Main.ManualCommand.
    if (process.env.TRUST_ENGINEER_MODEL_FREE_LAW === "1") {
      delete loop.analysis; delete loop.analysisParameterSymbols;
      loop.controller.law = { kind: "proportional", gainSymbol: "Main.Kp" };
    }
    const gaps: string[] = [];
    const captureWorkflowStage = async (suffix: string): Promise<void> => {
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (!screenshot) return;
      await new Promise(resolve => setTimeout(resolve, 200));
      await vscode.commands.executeCommand("notifications.clearAll");
      captureRenderedWorkbench(screenshot.replace(/\.png$/, `${suffix}.png`));
    };
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      if (process.env.TRUST_ENGINEER_WIDE === "1") {
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      }
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner: EvaluateWebview = body => evaluateInWebview(connection, session, body);
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-tune]') && d.querySelector('[data-engineer-verify]') && d.querySelector('[data-engineer-apply]')));"), "true",
        "An ordinary ST project must expose the connected model, controller, Tune, Validate and Apply workflow");
      await pollValue("engineer handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
      await inner(engineAnswerRecorderScript());
      const workflowLayout = async (): Promise<{ stage: string; primary: string[]; overflow: number; options: boolean; scenarioControls: boolean; acquisitionControls: boolean; taskSummaryVisible: boolean }> =>
        JSON.parse(await inner("return JSON.stringify((()=>{const root=d.querySelector('[data-engineer-workflow]');const visible=[...root.querySelectorAll('button')].filter(b=>{const r=b.getBoundingClientRect();return !b.hidden&&getComputedStyle(b).display!=='none'&&r.width>0&&r.height>0;});const task=root.querySelector('[data-engineer-task-summary]');const taskRect=task?.getBoundingClientRect();return {stage:root.dataset.selectedStage,primary:visible.filter(b=>b.classList.contains('trust-button--primary')).map(b=>b.textContent.trim()),options:root.querySelector('[data-engineer-options]')?.open??false,scenarioControls:Boolean(root.querySelector('[data-engineer-scenario-controls]')?.open),acquisitionControls:root.querySelector('[data-engineer-acquisition-controls]')?.open??false,taskSummaryVisible:Boolean(task&&!task.hidden&&taskRect&&taskRect.width>0&&taskRect.height>0),overflow:d.documentElement.scrollWidth-d.documentElement.clientWidth};})());"));
      await captureWorkflowStage("-start");
      const initialLayout = await workflowLayout();
      const chrome = JSON.parse(await inner("return JSON.stringify({duplicateTitle:!!d.querySelector('.header .identity'),intro:d.querySelector('[data-studio-message=model]')?.textContent.trim()??''});"));
      assert.strictEqual(chrome.duplicateTitle, false, "The editor tab owns the title; Control Studio must not repeat it");
      assert.strictEqual(chrome.intro, "", "Resting actions must not be repeated as introductory prose");
      await reportedGap(gaps, "Manual engineering tools belong inside Settings (the three-page page has no Settings disclosure)",
        async () => assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-options] [data-engineer-tools]'));"), "true", "Manual engineering tools belong inside Settings"));
      await reportedGap(gaps, "Scenario editing belongs inside Settings (the three-page page has no Settings disclosure)",
        async () => assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-options] [data-engineer-scenario-controls]'));"), "true", "Scenario editing belongs inside Settings"));
      assert.deepStrictEqual(initialLayout.primary, ["Identify plant"], "The initial Identify page must expose exactly one visible primary next action");
      if (theme?.includes("High Contrast")) {
        const primaryStyle = JSON.parse(await inner("const b=d.querySelector('[data-engineer-find]');const s=d.defaultView.getComputedStyle(b);return JSON.stringify({border:s.borderTopColor,background:s.backgroundColor});"));
        assert.notStrictEqual(primaryStyle.border, primaryStyle.background, "High-contrast primary actions must retain a visible button boundary");
      }
      assert.strictEqual(initialLayout.stage, "model", "The page must start on Identify, its first page");
      assert.deepStrictEqual(await stageTabs(inner), STAGE_TABS, "The normal workspace must keep exactly Identify, Tune and Validate as always-visible navigable tabs (specification 35, B9.1)");
      // Legacy plot events must not replace the guided Identify page; the navigation holds no runtime status.
      await inner("d.defaultView.dispatchEvent(new d.defaultView.CustomEvent('trust-control-studio-stage',{detail:{stage:'Monitor'}}));return 'legacy stage event';");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-workflow]').dataset.selectedStage;"), "model", "Legacy plot events must not replace the guided Identify page");
      assert.strictEqual(await inner("return String(!!d.querySelector('.studio-navigation [role=status]'));"), "false", "Runtime status must not compete with navigation");
      await reportedGap(gaps, "Live status strip: runtime state and SP/PV/MV below the working plot (the three-page page has no live-status strip)",
        async () => assert.strictEqual(await inner("return String(!!d.querySelector('.values'));"), "true", "Runtime state and SP/PV/MV belong in one live-status strip"));
      assert.strictEqual(initialLayout.options, false, "Secondary engineering settings must start collapsed");
      assert.strictEqual(initialLayout.scenarioControls, false, "Manual scenario controls must start collapsed");
      assert.strictEqual(initialLayout.acquisitionControls, false, "Identification controls must start collapsed");
      assert.strictEqual(initialLayout.taskSummaryVisible, false, "task requirements stay hidden until a model is ready");
      assert.ok(initialLayout.overflow <= 1, `Workflow 1 must fit the available viewport without horizontal overflow: ${initialLayout.overflow}px`);
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-control-studio-automatic-tune]')));"), "false",
        "The connected engineer workflow must have one Tune action, not a duplicate Automatic Tune header");
      // The actual runtime publishes a coherent symbolic scan, with Main.Manual TRUE, before any design.
      let inspected: any;
      for (let attempt = 0; attempt < 20; attempt++) {
        inspected = await runtimeRequest(loop.runtimeId, "control.signals.inspect", { loop_revision: loop.revision,
          bindings: [ { id: "sp", address: loop.signals.setpoint.address, iec_type: loop.signals.setpoint.iecType },
            { id: "pv", address: loop.signals.processVariable.address, iec_type: loop.signals.processVariable.iecType },
            { id: "mv", address: loop.signals.manipulatedVariable.address, iec_type: loop.signals.manipulatedVariable.iecType },
            { id: "mode", symbol: "Main.Manual", iec_type: "BOOL" }] });
        if (inspected.available) break;
        await new Promise(resolve => setTimeout(resolve, 50));
      }
      assert.strictEqual(inspected.available, true, "The actual runtime must publish a coherent symbolic scan");
      assert.strictEqual(inspected.values[3], 1, "Actual Main.Manual starts TRUE in this ST fixture");
      console.log("Actual symbolic inspection", JSON.stringify(inspected));
      await reportedGap(gaps, "Live header freshness reads Live (the three-page page shows no freshness indicator)",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-freshness]')?.textContent??'';"), "Live", "Live status must not expose the internal scan counter"));
      await reportedGap(gaps, "Live header controller mode reads Manual (the three-page page shows no controller-mode indicator)", async () => {
        await new Promise(resolve => setTimeout(resolve, 600));
        assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]')?.textContent??'';"), "Manual", "The live header must display the observed symbolic mode from a coherent scan");
      });
      // The engineer supplies a plant model through Identify's method (threePageSuppliedModel.ts): Open model opens the chosen file.
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Open model…", "Identify must offer Open model once supplied-model is the method");
      const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, source: "provided", validation: "validated",
        evidence: ["independent bench model reference"], revision: "sha256:bench-model",
        inputs: [{ id: "drive", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
        outputs: [{ id: "position", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(JSON.stringify(model))}],'supplied-model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'import';`);
      const equationText = await pollValue("supplied model equation", () => inner("return d.querySelector('[data-studio-model-equation]')?.textContent??'';"), text => text.trim().length > 0, 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-workflow]').dataset.selectedStage;"), "model",
        "supplying a model must remain on the Identify page instead of opening Tune");
      assert.strictEqual(await inner("const b=d.querySelector('[data-engineer-find]');const r=b.getBoundingClientRect();return String(r.width>0&&r.height>0&&r.top>=0&&r.bottom<=d.defaultView.innerHeight);"), "true",
        "the supplied model state must keep its single next action visible in the workflow viewport");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=model]').dataset.status;"), "complete", "Identify is complete once a model is supplied");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=tune]').dataset.status;"), "ready", "Tune is ready once a model is supplied");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false", "The selected supplied model feeds the same Tune operation");
      await reportedGap(gaps, "A supplied model's own response is drawn on Identify (the three-page chart draws the live recording or an identification's prediction)",
        async () => assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label')??'';"), /Model prediction/));
      assert.strictEqual((await workflowLayout()).taskSummaryVisible, false, "Model keeps the task summary hidden until the model is used");
      // Tune's requirements: the Requirements editor holds the test values (threePageRequirements.ts keys, More for the test design).
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      await pollValue("requirements editor", () => inner("return String(Boolean(d.querySelector('[data-trust-compact-editor] [data-studio-requirement=maximumError]')));"), value => value === "true", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-trust-compact-editor] [data-studio-requirement=maximumError]').disabled);"), "false", "A selected model enables response target editing");
      assert.strictEqual(await inner("const i=d.querySelector('[data-trust-compact-editor] [data-studio-requirement=maximumError]');return String(i.getBoundingClientRect().height>0);"), "true",
        "Tune must expose its response target inputs directly");
      await reportedGap(gaps, "Default requirement values are 1, 0.1 and 2 s (the editor shows the engine's defaults, and the fixture declares no requirements)",
        async () => {
          const values = JSON.parse(await inner(EDITOR_VALUES));
          assert.deepStrictEqual([values.referenceChange, values.maximumError, values.settlingTimeSeconds], ["1", "0.1", "2 s"], "The requirement editor opens with the reference, error and settling values");
        });
      await inner("const set=(key,value)=>{d.querySelector('[data-trust-compact-editor] [data-studio-requirement='+key+']').value=value;};set('referenceChange','1');set('maximumError','0.1');set('settlingBand','0.02');set('settlingTimeSeconds','2 s');d.querySelector('[data-trust-compact-more]').open=true;set('durationSeconds','4 s');return 'task';");
      await inner("d.querySelector('[data-studio-editor-done]').click();return 'requirements done';");
      await pollValue("requirements saved", () => inner("return String(!d.querySelector('[data-trust-compact-editor]'));"), value => value === "true", 10000);
      // Tune designs a candidate from the supplied model and the requirements (the engine's answer is recorded as `tune`).
      const tuneFrom = Number(await inner(ANSWER_COUNT));
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      const tuned = JSON.parse(await pollValue("unapplied candidate", () => inner(lastAnswerOf("tune", tuneFrom)),
        text => ["candidate", "failed"].includes(JSON.parse(text)?.status), 60000));
      assert.strictEqual(tuned.status, "candidate", `Tune must use the independently declared controller law and selected model: ${JSON.stringify(tuned)}`);
      assert.strictEqual(tuned.family, "P", "The candidate must keep the selected controller family, P");
      await reportedGap(gaps, `${TUNE_STAGE_GAP}: the candidate's family and gain on the Tune page`, async () => {
        assert.strictEqual(await inner("return String(d.querySelector('[data-studio-tuning-result]').checkVisibility());"), "true", "The Tune page shows the candidate");
        assert.match(await inner("return d.querySelector('[data-studio-tuning-result]').textContent;"), /Proportional/i, "The candidate names its controller family");
      });
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Apply", "Tune must leave the candidate unapplied until Validate and Apply");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true", "Apply must remain unavailable before actual-ST Validate");
      assert.deepStrictEqual((await stageTabs(inner)).map(tab => tab.selected), ["false", "true", "false"],
        "Tune completion keeps the Tune page: the three-page flow has no automatic page change, since Tune stops with its candidate");
      const requirements = await requirementValues(inner);
      await assertModelNavigationPreservesSettings(inner, equationText, requirements);
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-verify]').disabled);"), "false", "A tuned candidate must enable Validate controller");
      await reportedGap(gaps, "Validate controller is the single primary next action after Tune (threePageTemplate.ts marks Validate controller as a plain button)",
        async () => assert.deepStrictEqual((await workflowLayout()).primary, ["Validate controller"], "A tuned candidate must expose Validate as the single primary next action"));
      await captureWorkflowStage("-candidate");
      const parametersBeforeVerify = await runtimeRequest(loop.runtimeId, "control.parameters.status", { binding: loop.parameterBinding });
      // Validate runs the candidate on the actual ST runtime and answers its evidence (the engine's `validate` answer).
      const verifyFrom = Number(await inner(ANSWER_COUNT));
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      const verified = JSON.parse(await pollValue("actual ST verification", () => inner(lastAnswerOf("validate", verifyFrom)),
        text => ["verified", "verification-failed", "failed"].includes(JSON.parse(text)?.status), 40000));
      assert.strictEqual(verified.status, "verified", `Validate must pass the candidate on the actual ST runtime: ${JSON.stringify(verified)}`);
      assert.ok(verified.evidence.includes("ST-executed"), "Validate must report its runs as executed on the actual ST runtime");
      await reportedGap(gaps, "The Validate page names the runs' evidence as ST-executed (the three-page Validate page has no evidence line)",
        async () => assert.match(await inner("return d.querySelector('[data-studio-validation-result]').closest('.studio-result')?.textContent??'';"), /ST-executed/));
      const parametersAfterVerify = await runtimeRequest(loop.runtimeId, "control.parameters.status", { binding: loop.parameterBinding });
      assert.deepStrictEqual(parametersAfterVerify.values, parametersBeforeVerify.values,
        "actual-ST Verify must not write controller parameters before the explicit Apply action");
      assert.deepStrictEqual((await stageTabs(inner)).map(tab => tab.selected), ["false", "false", "true"],
        "Validate completion keeps the Validate page, where Apply is the next action (specification 35, B9.1)");
      await assertVerifiedNavigationPreservesEvidence(inner, verified.rows);
      const verifiedLayout = await workflowLayout();
      assert.deepStrictEqual(verifiedLayout.primary, ["Apply"], "A verified candidate must expose Apply as the single primary next action");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false",
        "a verified candidate must expose Apply without implying it has already been applied");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=tune]').dataset.status;"), "complete", "Tune is complete after its candidate is validated");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=verify]').dataset.status;"), "complete", "Validate is complete after its run passes");
      await captureWorkflowStage("-verified");
      // The Validate plot is the engine's measured output: its tuned run and its current-settings run, as two series of one group (B9.6).
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), String(verified.series),
        "The Validate plot draws the engine's tuned run and, when it has one, its current-settings run as its series (specification 35, B9.6)");
      const plotLabel: string = await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label')??'';");
      for (const name of verified.plotNames as string[]) assert.ok(plotLabel.includes(name), `The response plot names ${name}: ${plotLabel}`);
      assert.ok(Number(await inner("return String(Math.round(d.querySelector('[data-control-studio-chart]').getBoundingClientRect().height));")) >= 240,
        "The response plot keeps the page's minimum height for its plot row");
      await reportedGap(gaps, "Recorded responses: the plot toolbar reflows without clipping its controls, and a shared recorded-responses plot tab returns to them (the three-page chart has neither)",
        async () => assert.strictEqual(await inner("const t=d.querySelector('.plot-toolbar');return String(Boolean(t)&&t.scrollWidth<=t.clientWidth+1&&Boolean(d.querySelector('[data-plot-tab=recorded-responses]')));"), "true", "The plot toolbar must reflow without clipping controls in a narrow editor"));
      assert.strictEqual(await inner("return String(!d.querySelector('.workspace') || getComputedStyle(d.querySelector('.workspace')).display === 'none');"), "true",
        "Guided Verify must not expose the legacy Monitor page");
      await reportedGap(gaps, "Recorded responses: missing or bad-quality samples are explained as gaps in the plot (the chart label says nothing about gaps)",
        async () => assert.match(plotLabel, /gap|missing/i));
      await reportedGap(gaps, "Recorded responses: zoom, reset and keyboard inspection of the plots (the three-page chart has no zoom or inspect controls)",
        async () => assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-chart-action=zoom-in]')));"), "true", "Shared zoom must redraw the actual response plot"));
      await reportedGap(gaps, "Recorded responses: PNG export of the comparison through the host (the three-page chart has no export control)",
        async () => assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-chart-action=export-figure]')));"), "true", "Export must write the recorded comparison as a PNG through the host"));
      await reportedGap(gaps, "Recorded responses: recorded limits, scheduled events and a table of the sample values with each signal's identity and unit (the three-page page shows none)",
        async () => assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-recorded-values]') && d.querySelector('[data-engineer-recorded-limits]')));"), "true", "Recorded responses must explain their actual output bounds and units"));
      await reportedGap(gaps, "Recorded MV curves use the theme's output colour for contrast (the pixel check reads recorded-response canvases, which the three-page page does not have)",
        async () => assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-recorded-responses]')));"), "true", "Recorded MV curves must use the shared theme output color for visible contrast"));
      // The Apply block belongs to Validate only; changing page keeps the Validate plot and its series.
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-application]').hidden);"), "true", "Apply belongs to the Validate page only");
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-application]').hidden);"), "false", "Validate offers Apply");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), String(verified.series),
        "Changing page must keep the retained Validate plot");
      // Apply writes the validated candidate; on a simulated target the press of Apply confirms its review at once (threePageApplication.ts).
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-engine-confirm]'));"), "false", "A simulated target needs no confirmation of its Apply");
      await inner("d.querySelector('[data-engineer-apply]').click();return 'apply';");
      const applied = await pollValue("applied notice", () => inner("return d.querySelector('[data-studio-message=verify]').textContent;"), text => /Applied|failed|could not/i.test(text), 30000);
      assert.match(applied, /^Applied/, `Apply must report the applied values once the runtime observed them: ${applied}`);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Restore previous settings",
        "After Apply, Validate offers Restore previous settings in the Apply position (specification 35, B7)");
      await reportedGap(gaps, "Completed results: a View results disclosure with the applied values (the three-page Validate page has none)",
        async () => assert.strictEqual(await inner("return String(!!d.querySelector('[data-engineer-view-results]'));"), "true", "View the completed results"));
      // Rollback: Restore previous settings returns the runtime to the values it held before the Apply.
      await inner("d.querySelector('[data-engineer-apply]').click();return 'restore previous settings';");
      const restored = await pollValue("restored notice", () => inner("return d.querySelector('[data-studio-message=verify]').textContent;"), text => /Restored|failed|could not/i.test(text), 30000);
      assert.match(restored, /^Restored/, `Restore previous settings must return the previous values: ${restored}`);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Apply",
        "After Restore the page offers Apply again for a validated candidate (specification 35, B7)");
      const observed = await runtimeRequest(loop.runtimeId, "control.parameters.status", { binding: loop.parameterBinding });
      assert.strictEqual(observed.values["Main.Kp"], 1, "Restore previous settings must return the runtime to the original proportional gain");
      await reportedGap(gaps, "After Restore, Validate shows the current gains (the three-page page has no evidence line)",
        async () => assert.match(await inner("return d.querySelector('[data-engineer-evidence]')?.textContent??'';"), /Main.Kp.*1/));
      // A cancelled operation: the page must stop its busy state and let no stale plot or candidate stand (engineOpened.ts, threePageChart.ts).
      await inner(engineMessage("control-studio.engine.update", { update: { command: "validate", state: "cancelled", message: "Validation was cancelled." } }));
      await reportedGap(gaps, "A cancelled operation clears the stale verification plot (threePageChart.ts clears its plots only on edited, model-ready and reset)",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "0", "A cancelled engineer operation must clear stale verification plots instead of presenting them as current"));
      await reportedGap(gaps, "A cancelled operation disables Verify and Apply (threePageOperation.ts clears the candidate only on edited)",
        async () => assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled && d.querySelector('[data-engineer-verify]').disabled);"), "true", "Cancelled candidates must disable Verify and Apply"));
      // The runtime stops: the page must show that live values are unavailable, and offer to start the simulator again.
      await runtimeLifecycleService.stopRuntime();
      await reportedGap(gaps, "Live quality reads Unavailable after the runtime stops (the three-page page shows no quality indicator)",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-quality]')?.textContent??'';"), "Unavailable", "A stopped runtime must show its live values as unavailable"));
      await reportedGap(gaps, "Controller mode reads Unavailable after the runtime stops (the three-page page shows no controller-mode indicator)",
        async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]')?.textContent??'';"), "Unavailable", "A stopped runtime must not retain its last Manual/Auto mode as current"));
      await reportedGap(gaps, "Contextual simulator start appears after the runtime stops (the three-page page has no start-runtime control)",
        async () => assert.strictEqual(await inner("const b=d.querySelector('[data-control-studio-start-runtime]');return String(Boolean(b&&!b.hidden));"), "true", "The page must offer to start the simulator again"));
      await reportedGap(gaps, "Starting the simulator from Control Studio restores its Manual mode and hides the start control (there is no start-runtime control to press)", async () => {
        assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]')?.textContent??'';"), "Manual", "The simulator started from Control Studio must show its Manual mode again");
        assert.strictEqual(await inner("const b=d.querySelector('[data-control-studio-start-runtime]');return String(Boolean(b&&b.hidden));"), "true", "The start control must hide once the simulator runs");
      });
    } finally {
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
    assert.deepStrictEqual(gaps, [], REPORTED_GAPS);
  });
});

suite("Control Studio connected PID engineer path", () => {
  test("exposes all typed PID gains through the actual ST runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(60_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    if (!loop.parameterBinding) throw new Error("Declare the ST parameter binding before application.");
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    try {
      const status = await runtimeRequest(loop.runtimeId, "control.parameters.status", { binding: loop.parameterBinding });
      assert.deepStrictEqual(status.values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
    } finally { await runtimeLifecycleService.stopRuntime(); }
  });
});
