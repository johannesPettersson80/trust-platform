import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { connectRuntimeParameters } from "../../controlStudio/runtimeParameterConnection";
import { attachControlStudio, engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

/** A transfer's last transaction as the engine's Apply outcome reports it (`CommissioningResult.transfers`, crates/trust-control/src/commissioning/commissioning_result.rs). */
interface TransferTransaction {
  applied_values?: Record<string, number>;
  observed_values?: Record<string, unknown>;
  confirmation?: string;
}

/**
 * What the journey reads of one engine answer the page received. The recorder keeps each whole answer in the webview
 * (`__engineAnswers`, control-studio-rendered-harness.ts); `ANSWER_PROJECTION` reads these fields of it, so that a plot of
 * thousands of points never crosses CDP.
 */
interface JourneyAnswer {
  kind: string;
  method?: string;
  status: string;
  flowStage?: string;
  reason?: string;
  failure?: Record<string, unknown>;
  family?: string;
  values?: Record<string, number>;
  applicationAction?: string;
  /** An identification (`IdentifyResult`, alone or as a run's `identification`): its model type, equation and key numbers. */
  modelType?: string;
  equation?: string;
  keyNumbers?: string;
  /** A Validate answer (`ValidateResult`): each case's plant and the requirement ids of its current settings' and candidate's runs. */
  scenarios?: { id: string; plant: string; current: string[]; candidate: string[] }[];
  modelAgreement?: string;
  /** The Manual-to-Auto entry case of an Auto commissioning (`ValidateResult.initialization`): its status and its rows' ids. */
  initialization?: { status?: string; requirements?: { id: string; guard?: boolean }[] };
  /** An Apply outcome (`ApplyOutcome.result`): the transfers its commissioning made. */
  transfers?: { transaction?: TransferTransaction }[];
}

/** The webview function that reads a `JourneyAnswer` from one `__engineAnswers` record. */
const ANSWER_PROJECTION = "e=>{const r=e.result||{};const m=e.method==='run'?r.identification:e.method==='identify'?r:undefined;"
  + "return{kind:e.kind,method:e.method,status:e.status,flowStage:e.flowStage,reason:e.reason,failure:e.failure,family:e.family,"
  + "values:e.values,applicationAction:e.applicationAction,modelType:m&&m.modelType,equation:m&&m.equation,keyNumbers:m&&m.keyNumbers,"
  + "scenarios:Array.isArray(r.scenarios)?r.scenarios.map(s=>({id:s.id,plant:s.plant,"
  + "current:((s.current||{}).requirements||[]).map(q=>q.id),candidate:((s.candidate||{}).requirements||[]).map(q=>q.id)})):undefined,"
  + "modelAgreement:r.modelAgreement?r.modelAgreement.id:undefined,initialization:r.initialization,"
  + "transfers:r.result&&Array.isArray(r.result.transfers)?r.result.transfers.map(t=>({transaction:t.transaction?{applied_values:t.transaction.applied_values,"
  + "observed_values:t.transaction.observed_values,confirmation:t.transaction.confirmation}:undefined})):undefined};}";

/** The statuses that end a wait without its answer, as the TypeScript route's waits had them. */
const ENDING = ["failed", "verification-failed", "recovery-required"];

suite("Control Studio three-page native journey", () => {
  test("identifies tunes validates applies and restores the real library PID from the visible controls", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE_NATIVE !== "1") this.skip();
    // Includes multiple bounded native jobs and observed commissioning on ARM.
    // This is the test allowance; individual runtime budgets remain unchanged.
    this.timeout(600000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.type, "Control.PID");
    assert.strictEqual(loop.controller.pidInitialization.kind, "control-pid-v1");
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    let readEvents: (() => Promise<string>) | undefined;
    let answersSince: ((since: number) => Promise<JourneyAnswer[]>) | undefined;
    let answersBeforeReopen: JourneyAnswer[] = [];
    /**
     * The checks the engine route cannot pass yet (reported gaps). Each runs where the journey reaches it and is recorded when it
     * fails; the journey goes on, so one run reaches every later step, and the test fails at its end with all of them.
     */
    const gaps: string[] = [];
    const gap = async (check: () => unknown): Promise<void> => {
      try { await check(); } catch (error) { gaps.push(error instanceof Error ? error.message : String(error)); }
    };
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      let session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      // Retain the observations asserted below, not another full copy of every
      // accumulated recording on each progress event and every CDP poll.
      const observe = `d.defaultView.addEventListener('trust-control-studio-engineer', event => {
        const p=event.detail;
        d.defaultView.__journeyEvents.push({status:p.status,reason:p.reason,failure:p.failure,flowStage:p.flowStage,
          family:p.family,values:p.values,model:p.model,applicationAction:p.applicationAction,
          identificationPresentation:p.identificationPresentation?{predictedResponse:p.identificationPresentation.predictedResponse.slice(0,3)}:undefined,
          scenarioResults:p.scenarioResults?.map(s=>({id:s.id,result:{request:{simulation_override:s.result.request.simulation_override},evaluation:{requirements:s.result.evaluation.requirements}}})),
          initialization:p.initialization?{status:p.initialization.status,requirements:p.initialization.requirements}:undefined,
          commissioning:p.commissioning?{transfers:p.commissioning.transfers}:undefined});
      });`;
      readEvents = () => inner("return JSON.stringify(d.defaultView.__journeyEvents??[]);");
      // The engine route answers every flow as `control-studio.engine.*` messages and dispatches no status events
      // (threePageHostState.ts hands them to engineOpened.ts): the waits read the answers the recorder keeps.
      const answersText = (since: number) => inner(`return JSON.stringify((d.defaultView.__engineAnswers??[]).slice(${since}).map(${ANSWER_PROJECTION}));`);
      const readAnswers = async (since: number) => JSON.parse(await answersText(since)) as JourneyAnswer[];
      answersSince = readAnswers;
      const answerCount = async () => Number(await inner("return String((d.defaultView.__engineAnswers??[]).length);"));
      const capture = (name: string) => captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      const click = async (selector: string) => {
        const outcome = await inner(`const button=d.querySelector(${JSON.stringify(selector)});if(!button?.checkVisibility()||button.disabled)return JSON.stringify({page:d.querySelector('[data-engineer-stage-tab][aria-selected=true]')?.textContent,visible:button?.checkVisibility(),disabled:button?.disabled});button.click();return 'clicked';`);
        if (outcome !== "clicked") assert.fail(`${selector} is visible and enabled: ${outcome}; latest engine answers ${JSON.stringify((await readAnswers(0)).slice(-3))}`);
      };
      /** Waits for the engine's answer that `matches`, or for a failure, from answer `since` on. A refused manual preview shows in the Tune area and ends no flow (engineOpened.ts). */
      /** What the page shows when a wait ends without its answer: each page's message, the actions and whether the page is busy. */
      const pageState = () => inner("return JSON.stringify({messages:Object.fromEntries([...d.querySelectorAll('[data-studio-message]')].map(m=>[m.dataset.studioMessage,m.textContent])),actions:Object.fromEntries(['find','tune','verify','apply'].map(a=>{const b=d.querySelector('[data-engineer-'+a+']');return [a,b?{text:b.textContent,disabled:b.disabled}:null];})),tuningMethod:d.querySelector('[data-engineer-tuning-method]')?.value,manual:Object.fromEntries([...d.querySelectorAll('[data-studio-manual-value]')].map(i=>[i.dataset.studioManualValue,i.value])),controller:d.querySelector('[data-studio-controller]')?.selectedOptions[0]?.dataset.family,busy:d.querySelector('[data-studio-three-page]')?.dataset.busy});");
      const waitAnswer = async (label: string, matches: (answer: JourneyAnswer) => boolean, timeout = 60000, since = 0): Promise<JourneyAnswer> => {
        const ended = (answer: JourneyAnswer) => matches(answer) || (ENDING.includes(answer.status) && answer.method !== "preview");
        const answered = await pollValue(label, () => answersText(since), text => (JSON.parse(text) as JourneyAnswer[]).some(ended), timeout)
          .catch(async (error: unknown) => {
            throw new Error(`${error instanceof Error ? error.message : String(error)}; the page shows ${await pageState().catch(() => "nothing readable")}`);
          });
        const answers = JSON.parse(answered) as JourneyAnswer[];
        const result = answers.find(matches);
        assert.ok(result, JSON.stringify(answers)); return result;
      };
      const waitStatus = (status: string, timeout = 60000, since = 0) => waitAnswer(status, answer => answer.status === status, timeout, since);
      // Identify plant runs the engine's automatic flow through Identify (engine/routes/runRoute.ts): the run's result carries the
      // identification. An identification answered alone is an identify result; both state the model type.
      const identification = (answer: JourneyAnswer) => typeof answer.modelType === "string";
      /**
       * The answers of an Apply or a Restore the journey pressed, up to its outcome `status` or a failure. On a simulated target the
       * press is the confirmation (specification 35, A2.5 and B9.3): the page confirms the engine's review at once
       * (webview/threePageApplication.ts showReview). A page that shows a confirmation instead is a gap, recorded under `expectation`,
       * and the journey confirms the review so that the steps after it still run.
       */
      const reviewedOutcome = async (action: "apply" | "restore", status: string, expectation: string, timeout: number, since: number): Promise<JourneyAnswer[]> => {
        const ended = (answers: JourneyAnswer[]) => answers.some(answer => answer.status === status || ENDING.includes(answer.status));
        const confirmationText = () => inner(`return d.querySelector('[data-engine-confirm=${action}]')?.textContent??'';`);
        const first = JSON.parse(await pollValue(`${action} outcome`, async () => JSON.stringify({ answers: JSON.parse(await answersText(since)), confirmation: await confirmationText() }),
          text => { const state = JSON.parse(text) as { answers: JourneyAnswer[]; confirmation: string }; return ended(state.answers) || state.confirmation !== ""; }, timeout)) as { answers: JourneyAnswer[]; confirmation: string };
        if (ended(first.answers)) return first.answers;
        await gap(() => assert.fail(`${expectation}; the page asked to confirm the engine's review first ("${first.confirmation}")`));
        await click(`[data-engine-confirm=${action}]`);
        return JSON.parse(await pollValue(`${action} outcome`, () => answersText(since), text => ended(JSON.parse(text) as JourneyAnswer[]), timeout)) as JourneyAnswer[];
      };
      await pollValue("workflow ready", () => inner("return d.querySelector('[data-studio-three-page]').dataset.ready;"), value => value === "true", 10000);
      await pollValue("restored host state", () => inner("return d.querySelector('[data-studio-three-page]').dataset.hostReady;"), value => value === "true", 10000);
      await inner(engineAnswerRecorderScript());
      // Identify is a live campaign: without a running target the engine answers "Connect a runtime" (specification 35, B3.9 and
      // §0.1a). The TypeScript route started the local simulator itself (runtimeSearchConnection.ts); here the engineer starts it.
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.ok(started.ok, `The local simulator starts for the live identification: ${JSON.stringify(started)}`);
      await click("[data-engineer-stage-tab=model]");
      await inner(`d.defaultView.__journeyEvents=[];${observe}return 'listen';`);
      await capture("01-identify");
      const identifySince = await answerCount();
      await click("[data-engineer-find]");
      const identified = await waitAnswer("model-ready", identification, 60000, identifySince);
      // The engine route still draws the measured and predicted response through the page's identification presentation (engineIdentify.ts showPlot).
      const presentation = (events: Array<{ identificationPresentation?: { predictedResponse: unknown[] } }>) => events.filter(event => event.identificationPresentation).pop()?.identificationPresentation;
      assert.ok((presentation(JSON.parse(await readEvents()))?.predictedResponse.length ?? 0) > 2);
      await pollValue("identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Identify", "Keep the identified model visible for review");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false", "Identification must not open Tune requirements");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Identify plant", "Keep the identification action label stable");
      await capture("02-identify-review");
      const repeatSince = await answerCount();
      await click("[data-engineer-find]");
      const repeated = await waitAnswer("model-ready", identification, 90000, repeatSince);
      await pollValue("repeated identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      await capture("02b-identify-repeated");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Identify");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "true");
      // Reported gap (G2): the page plots the engine's measured and predicted traces only; the plant-input trace sets the input
      // unit and is not drawn (engineIdentify.ts showPlot).
      await gap(async () => assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3", "Include the applied plant input with the measured and predicted output"));
      await capture("03-identified-model");
      const equation = await inner("return d.querySelector('[data-studio-model-equation]').textContent;");
      // The engine answers no model object (G4). It states the identified model as text: G(s) with the identified coefficients, and
      // the key numbers, the gain first (crates/trust-control/src/identification/presentation.rs identified_model_equation and
      // identified_model_key_numbers). Every model it identifies is a transfer function, so its equation starts with "G(s) =".
      const transferFunction = (answer: JourneyAnswer) => answer.equation?.startsWith("G(s) =") === true;
      const gain = (answer: JourneyAnswer) => Number(/(?:^|, )(?:Rate gain|Gain) ([^ ,]+)/.exec(answer.keyNumbers ?? "")?.[1]);
      if (transferFunction(identified) && transferFunction(repeated)) {
        assert.ok(Math.abs(gain(repeated) - gain(identified)) <= 0.1 * Math.abs(gain(identified)),
          `Identifying again from a settled output reproduces the plant gain: ${gain(identified)} then ${gain(repeated)}; `
          + `models ${identified.equation} (${identified.keyNumbers}) then ${repeated.equation} (${repeated.keyNumbers})`);
      }
      // The page shows the model of the latest identification.
      if (transferFunction(repeated)) {
        // A transfer function relates deviations from the operating point; the page shows G(s) with its identified coefficients,
        // as the engine wrote it (engineIdentify.ts showEngineIdentification).
        assert.strictEqual(equation, repeated.equation, `The displayed G(s) shows the identified coefficients: ${equation}; key numbers ${repeated.keyNumbers}`);
        // The delay is written as e^(-Ls); the key numbers name it in s below two minutes, in min or h above (presentation.rs duration_text).
        const delay = /(?:^|, )delay ([^ ,]+) (s|min|h)(?:,|$)/.exec(repeated.keyNumbers ?? "");
        if (delay) assert.ok(equation.includes(delay[2] === "s" ? `e^(-${delay[1]}s)` : "e^(-"), `The displayed G(s) shows the identified delay: ${equation}`);
      } else {
        // Reported gap (G4): the engine's identification states no operating point (IdentifyResult, crates/trust-control/src/messages.rs).
        await gap(() => assert.fail(`The displayed plant equation includes its measured output operating point; the engine's identification states none: ${equation}`));
      }
      assert.strictEqual(await inner("const chart=d.querySelector('[data-control-studio-chart]').getBoundingClientRect();return String(chart.bottom<=d.defaultView.innerHeight-8);"), "true", "The identified response plot fits the available editor height");
      const client = (await connectRuntimeParameters(loop)).client;
      const before = await client.status();
      const beforeContinue = await answerCount();
      await click("[data-engineer-stage-tab=tune]");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Tune");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false", "The Tune tab is navigation only");
      assert.strictEqual((await readAnswers(beforeContinue)).some(answer => answer.status === "running"), false);
      await capture("03-tune-before-start");
      assert.strictEqual(await inner("const select=d.querySelector('[data-studio-controller]');const option=[...select.options].find(option=>option.dataset.family==='PI');if(!option)return 'unavailable';select.value=option.value;select.dispatchEvent(new Event('change',{bubbles:true}));return 'PI';"), "PI", "Choose PI for the actual P1.Controller instance");
      // Tune fills missing test settings automatically from the model and limits and never opens an empty editor.
      await click("[data-studio-requirements]");
      await pollValue("requirements editor", () => inner("return String(!!d.querySelector('[data-studio-requirement=maximumError]'));"), value => value === "true", 10000);
      // Reported gap: on the engine route Automatic fills the test criteria when Tune runs (B4.3), and no answer before the first
      // Tune carries them: the identification, the run's result and the inputs the study records hold no requirements, so the
      // editor shows the page's empty requirements (threePageRequirements.ts; only a Tune answer fills them, engineTune.ts).
      await gap(async () => assert.strictEqual(await inner("return String(['referenceChange','maximumError','settlingBand'].every(key => Number.isFinite(Number(d.querySelector('[data-studio-requirement='+key+']').value)) && d.querySelector('[data-studio-requirement='+key+']').value !== '') && ['settlingTimeSeconds','durationSeconds'].every(key => d.querySelector('[data-studio-requirement='+key+']').value !== ''));"), "true",
        "Requirements opens prefilled with the automatic test criteria"));
      await capture("04-requirements");
      await click("[data-studio-editor-done]");
      // While that gap stands, Done cannot save the blank criteria and the editor stays open: Cancel leaves the requirements as they
      // were, so that Tune fills them and the later pages are captured without the editor over them.
      if (await inner("return String(!!d.querySelector('dialog[open] [data-studio-editor-cancel]'));") === "true") await click("dialog[open] [data-studio-editor-cancel]");
      const tuneSince = await answerCount();
      await click("[data-engineer-tune]");
      const candidate = await waitStatus("candidate", 60000, tuneSince);
      assert.strictEqual(candidate.family, "PI", "Keep the explicitly selected controller configuration");
      // Reported gap: the engine's Tune answer predicts the candidate only (study/tune/prediction.rs builds its setpoint, measurement
      // and command traces), where B4.5 also shows the current settings; the chart draws "Tuned" alone (engineTune.ts predictedPlots).
      await gap(() => pollValue("tuned prediction", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => value === "2", 10000));
      await capture("05-tuned-controller");
      // Tuning stops on Tune; Validate controller on Validate starts the validation.
      const beforeValidation = await answerCount();
      await click("[data-engineer-stage-tab=verify]");
      await click("[data-engineer-verify]");
      const verified = await waitStatus("verified", 120000, beforeValidation);
      const scenarios = verified.scenarios ?? [];
      // The engine names the plant each case ran on: the configured plant by its id, or the model (validation/texts.rs configured_plant_name).
      assert.strictEqual(scenarios[0]?.plant, `configured plant ${loop.plantId}`,
        "Validate executes the configured plant; the fitted model belongs to the prediction comparison");
      // The Manual-to-Auto entry case of the Auto commissioning is the engine's (B5.3): the reported gap closes once the engine runs it.
      await gap(() => assert.strictEqual(verified.initialization?.status, "Pass", "Validate passes the controller transfer (Manual-to-Auto entry) check"));
      assert.deepStrictEqual((await client.status()).values, before.values, "Tuning and isolated validation do not apply runtime gains");
      // Every requirement of each run keeps its own row: the model agreement first, each case's current settings and candidate, the
      // Manual-to-Auto entry's rows (`initialization`), and the candidate's overall row last (engineValidate.ts renderValidation). A
      // "limit-and-controller-checks" row counts the passing guards (B9.9) and adds its count, so the guard rows of the entry count too.
      const entryRows = verified.initialization?.requirements ?? [];
      const expectedRows = scenarios.reduce((sum, scenario) => sum + scenario.current.length + scenario.candidate.length, 0)
        + (verified.modelAgreement ? 1 : 0) + entryRows.length;
      const requirementIds = new Set<string>([
        ...scenarios.flatMap(scenario => [...scenario.current, ...scenario.candidate]),
        ...(verified.modelAgreement ? [verified.modelAgreement] : []),
        ...entryRows.map(row => row.id),
      ]);
      const shown = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-validation-result] tbody tr')].map(row => ({ id: row.dataset.studioRequirement, measured: row.cells[1].textContent })));")) as { id: string; measured: string }[];
      const counted = shown.find(row => row.id === "limit-and-controller-checks");
      assert.strictEqual(shown.filter(row => requirementIds.has(row.id)).length + (counted ? Number(counted.measured.split(" ")[0]) : 0), expectedRows,
        "Show every mandatory actual-ST requirement the engine's Validate answer holds; the transfer is checked by the reported gap above");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false");
      await capture("06-validated-controller");
      // Validate's model check moved the live plant: one Apply waits until it rests at the validated point, then commissions.
      const applyStart = await answerCount();
      await click("[data-engineer-apply]");
      // A passed Apply is "applied" on the engine route (webview/engineApplication.ts applyOutcomePayload).
      const events = await reviewedOutcome("apply", "applied", "One Apply waits for the process and commissions without asking to apply again", 300000, applyStart);
      const applied = events.find(answer => answer.status === "applied" && answer.applicationAction === "apply");
      assert.ok(applied, `One Apply waits for the process and commissions without asking to apply again: ${JSON.stringify(events.filter(answer => answer.status !== "running"))}`);
      // The engine's Apply outcome carries the transfers of its commissioning (ApplyOutcome.result.transfers).
      const transaction = applied.transfers?.find(transfer => transfer.transaction?.applied_values?.["P1.Controller.StateValue.NominalOutput"] !== undefined)?.transaction;
      assert.ok(transaction, "The actual library state transfer was observed");
      for (const state of loop.controller.pidInitialization.states) {
        assert.ok(Number.isFinite(transaction.applied_values?.[state.symbol]), `${state.symbol} adopted`);
        assert.ok(Number.isFinite(transaction.observed_values?.[state.symbol]), `${state.symbol} observed after ST`);
      }
      assert.strictEqual(transaction.confirmation, "output-image");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Restore previous settings");
      await capture("07-applied");
      const eventsBeforeClose = await readEvents();
      answersBeforeReopen = await readAnswers(0);
      const activeBeforeClose = await client.status();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port, "", loop.id);
      await pollValue("reopened host state", () => inner("return d.querySelector('[data-studio-three-page]').dataset.hostReady;"), value => value === "true", 10000);
      await inner(`d.defaultView.__journeyEvents=${eventsBeforeClose};${observe}return 'reopened listener';`);
      // A reopened editor is a new document, so the recorder is installed again.
      await inner(engineAnswerRecorderScript());
      await click("[data-engineer-stage-tab=verify]");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Restore previous settings", "Reopening inspects the active receipt and offers its existing guarded restoration");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false");
      assert.deepStrictEqual(await client.status(), activeBeforeClose, "Reopening performs no parameter/state transaction");
      await capture("07-reopened-application");
      // Reopening shows the study's candidate on Tune (A8.6): its settings are the ones the manual method starts from below.
      await click("[data-engineer-stage-tab=tune]");
      await gap(async () => {
        const shown = await inner("const area=d.querySelector('[data-studio-tuning-result]');return JSON.stringify({visible:!!area?.checkVisibility(),text:area?.textContent??''});");
        const { visible, text } = JSON.parse(shown) as { visible: boolean; text: string };
        assert.ok(visible && text.includes(String(candidate.family)), `Reopening shows the study's candidate on Tune (A8.6): ${shown}`);
      });
      await click("[data-engineer-stage-tab=verify]");
      const beforeRestore = await answerCount();
      await click("[data-engineer-apply]");
      // A confirmed Restore is "commissioned" on the engine route (webview/engineApplication.ts restoreOutcomePayload).
      const restoring = await reviewedOutcome("restore", "commissioned", "One press of Restore previous settings restores the previous settings", 20000, beforeRestore);
      const restored = restoring.find(answer => answer.status === "commissioned");
      assert.ok(restored, JSON.stringify(restoring));
      assert.strictEqual(restored.applicationAction, "restore");
      assert.deepStrictEqual((await client.status()).values, before.values);
      await capture("08-restored");
      await click("[data-engineer-stage-tab=tune]");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual';");
      // In manual tuning the entered values are the settings; Keep toggles only fix values after automatic tuning (Tune page spec).
      const gainSymbol = loop.controller.law.proportionalGainSymbol;
      const enteredGain = candidate.values![gainSymbol] * 1.01;
      await inner(`const input=[...d.querySelectorAll('[data-studio-manual-value]')].find(field=>field.dataset.studioManualValue===${JSON.stringify(gainSymbol)});input.value=${JSON.stringify(String(enteredGain))};input.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));return 'manual gain entered';`);
      let since = await answerCount();
      await click("[data-engineer-tune]");
      const manual = await waitStatus("candidate", 20000, since);
      assert.strictEqual(manual.values![gainSymbol], enteredGain, "Manual Tune retains the entered value at full precision");
      since = await answerCount();
      await click("[data-engineer-stage-tab=verify]");
      await click("[data-engineer-verify]");
      await waitStatus("verified", 120000, since);
      assert.deepStrictEqual((await client.status()).values, before.values, "Manual Tune and Validate do not write the running controller");
      await click("[data-engineer-stage-tab=tune]");
      await capture("09-manual-tuned");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='automatic';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'automatic';");
      since = await answerCount();
      await click("[data-engineer-tune]");
      const automatic = await waitStatus("candidate", 20000, since);
      // After automatic tuning, Keep beside a setting fixes it for the next run (Tune page spec).
      await inner(`const keep=[...d.querySelectorAll('[data-studio-keep]')].find(item=>item.dataset.studioKeep===${JSON.stringify(gainSymbol)});keep.checked=true;keep.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'kept';`);
      since = await answerCount();
      await click("[data-engineer-tune]");
      const pinned = await waitStatus("candidate", 20000, since);
      assert.strictEqual(pinned.values![gainSymbol], automatic.values![gainSymbol], "Automatic Tune keeps the value its Keep toggle fixed");
      assert.strictEqual(await inner(`return String([...d.querySelectorAll('[data-studio-keep]')].find(item=>item.dataset.studioKeep===${JSON.stringify(gainSymbol)})?.checked);`), "true",
        "the kept setting stays marked Keep in the new result, so its pin went through the tuning run");
      since = await answerCount();
      await click("[data-engineer-stage-tab=verify]");
      await click("[data-engineer-verify]");
      await waitStatus("verified", 120000, since);
      assert.deepStrictEqual((await client.status()).values, before.values);
      await capture("10-pinned-validation");
      assert.deepStrictEqual(gaps, [], "The engine route does not pass these checks yet (reported gaps); the steps after them ran");
    } finally {
      if (readEvents) { try { fs.writeFileSync(path.join(captures, "three-page-native-events.json"), await readEvents()); } catch { /* Retain other evidence if the owned editor has closed. */ } }
      try {
        const later = answersSince ? await answersSince(0).catch((): JourneyAnswer[] => []) : [];
        fs.writeFileSync(path.join(captures, "three-page-native-engine-answers.json"), JSON.stringify({ answers: [...answersBeforeReopen, ...later], gaps }));
      } catch { /* Retain other evidence if the owned editor has closed. */ }
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtimeLifecycleService.stopRuntime();
    }
  });

});
