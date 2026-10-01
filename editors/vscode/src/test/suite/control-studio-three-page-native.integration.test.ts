import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { connectRuntimeParameters } from "../../controlStudio/runtimeParameterConnection";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

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
      const capture = (name: string) => captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      const click = async (selector: string) => {
        const outcome = await inner(`const button=d.querySelector(${JSON.stringify(selector)});if(!button?.checkVisibility()||button.disabled)return JSON.stringify({page:d.querySelector('[data-engineer-stage-tab][aria-selected=true]')?.textContent,visible:button?.checkVisibility(),disabled:button?.disabled});button.click();return 'clicked';`);
        if (outcome !== "clicked") assert.fail(`${selector} is visible and enabled: ${outcome}; latest events ${JSON.stringify(JSON.parse(readEvents ? await readEvents() : "[]").slice(-3))}`);
      };
      const waitStatus = async (status: string, timeout = 60000, since = 0) => {
        const events = JSON.parse(await pollValue(status, readEvents!, text => JSON.parse(text).slice(since).some((event: { status: string }) => [status, "failed", "verification-failed", "recovery-required"].includes(event.status)), timeout)).slice(since);
        const result = events.find((event: { status: string }) => event.status === status);
        assert.ok(result, JSON.stringify(events)); return result;
      };
      await pollValue("workflow ready", () => inner("return d.querySelector('[data-studio-three-page]').dataset.ready;"), value => value === "true", 10000);
      await pollValue("restored host state", () => inner("return d.querySelector('[data-studio-three-page]').dataset.hostReady;"), value => value === "true", 10000);
      await click("[data-engineer-stage-tab=model]");
      await inner(`d.defaultView.__journeyEvents=[];${observe}return 'listen';`);
      await capture("01-identify");
      await click("[data-engineer-find]");
      const identified = await waitStatus("model-ready");
      assert.ok(identified.identificationPresentation.predictedResponse.length > 2);
      await pollValue("identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Identify", "Keep the identified model visible for review");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false", "Identification must not open Tune requirements");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-find]').textContent;"), "Identify plant", "Keep the identification action label stable");
      await capture("02-identify-review");
      const repeatSince = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-find]");
      const repeated = await waitStatus("model-ready", 90000, repeatSince);
      await pollValue("repeated identification finished", () => inner("return d.querySelector('[data-studio-three-page]').dataset.busy;"), value => value === "false", 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Identify");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-model-result]').checkVisibility());"), "true");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), "3", "Include the applied plant input with the measured and predicted output");
      await capture("03-identified-model");
      const equation = await inner("return d.querySelector('[data-studio-model-equation]').textContent;");
      if (identified.model.kind === "transfer-function" && repeated.model.kind === "transfer-function") {
        const gain = (model: { numerator: number[]; denominator: number[] }) => model.numerator[0] / model.denominator[model.denominator.length - 1];
        assert.ok(Math.abs(gain(repeated.model) - gain(identified.model)) <= 0.1 * Math.abs(gain(identified.model)),
          `Identifying again from a settled output reproduces the plant gain: ${gain(identified.model)} then ${gain(repeated.model)}`);
      }
      // The page shows the model of the latest identification.
      if (repeated.model.kind === "transfer-function") {
        // A transfer function relates deviations from the operating point; the page shows G(s) with its identified coefficients.
        const { formatEngineeringNumber } = require("../../controlStudio/chartContract");
        for (const coefficient of [...repeated.model.numerator, ...repeated.model.denominator].filter((value: number) => value !== 0)) {
          assert.ok(equation.includes(formatEngineeringNumber(Math.abs(coefficient))), `The displayed G(s) shows the identified coefficient ${coefficient}: ${equation}`);
        }
        if (repeated.model.delaySeconds) assert.ok(equation.includes(`e^(−${formatEngineeringNumber(repeated.model.delaySeconds)}s)`), `The displayed G(s) shows the identified delay: ${equation}`);
      } else assert.ok(equation.includes(String(repeated.model.operatingPoint.outputs[0])), "The displayed plant equation includes its measured output operating point");
      assert.strictEqual(await inner("const chart=d.querySelector('[data-control-studio-chart]').getBoundingClientRect();return String(chart.bottom<=d.defaultView.innerHeight-8);"), "true", "The identified response plot fits the available editor height");
      const client = (await connectRuntimeParameters(loop)).client;
      const before = await client.status();
      const beforeContinue = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-stage-tab=tune]");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-stage-tab][aria-selected=true]').textContent;"), "Tune");
      assert.strictEqual(await inner("return String(!!d.querySelector('dialog[open]'));"), "false", "The Tune tab is navigation only");
      assert.strictEqual((JSON.parse(await readEvents!()) as Array<{status:string}>).slice(beforeContinue).some(event => event.status === "running"), false);
      await capture("03-tune-before-start");
      assert.strictEqual(await inner("const select=d.querySelector('[data-studio-controller]');const option=[...select.options].find(option=>option.dataset.family==='PI');if(!option)return 'unavailable';select.value=option.value;select.dispatchEvent(new Event('change',{bubbles:true}));return 'PI';"), "PI", "Choose PI for the actual P1.Controller instance");
      // Tune fills missing test settings automatically from the model and limits and never opens an empty editor.
      await click("[data-studio-requirements]");
      await pollValue("requirements editor", () => inner("return String(!!d.querySelector('[data-studio-requirement=maximumError]'));"), value => value === "true", 10000);
      assert.strictEqual(await inner("return String(['referenceChange','maximumError','settlingBand','settlingTimeSeconds','durationSeconds'].every(key => Number.isFinite(Number(d.querySelector('[data-studio-requirement='+key+']').value)) && d.querySelector('[data-studio-requirement='+key+']').value !== ''));"), "true",
        "Requirements opens prefilled with the automatic test criteria");
      await capture("04-requirements");
      await click("[data-studio-editor-done]");
      await click("[data-engineer-tune]");
      const candidate = await waitStatus("candidate");
      assert.strictEqual(candidate.family, "PI", "Keep the explicitly selected controller configuration");
      await pollValue("tuned prediction", () => inner("return d.querySelector('[data-control-studio-chart]').dataset.seriesCount;"), value => value === "2", 10000);
      await capture("05-tuned-controller");
      // Tuning stops on Tune; Validate controller on Validate starts the validation.
      const beforeValidation = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-stage-tab=verify]");
      await click("[data-engineer-verify]");
      const verified = await waitStatus("verified", 120000, beforeValidation);
      assert.strictEqual(verified.scenarioResults[0].result.request.simulation_override, undefined,
        "Validate executes the configured plant; the fitted model belongs to the prediction comparison");
      assert.strictEqual(verified.initialization.status, "Pass");
      assert.deepStrictEqual((await client.status()).values, before.values, "Tuning and isolated validation do not apply runtime gains");
      const expectedRows = verified.scenarioResults.reduce((sum: number, scenario: { result: { evaluation: { requirements: unknown[] } } }) => sum + scenario.result.evaluation.requirements.length, 0) + verified.initialization.requirements.length;
      // Declared and failing requirements keep their own rows; passing generated checks and the transfer run are counted in one row.
      const requirementIds = new Set<string>([...verified.scenarioResults.flatMap((scenario: { result: { evaluation: { requirements: { id: string }[] } } }) =>
        scenario.result.evaluation.requirements.map(requirement => requirement.id)), ...verified.initialization.requirements.map((requirement: { id: string }) => requirement.id)]);
      const shown = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-studio-validation-result] tbody tr')].map(row => ({ id: row.dataset.studioRequirement, measured: row.cells[1].textContent })));")) as { id: string; measured: string }[];
      const counted = shown.find(row => row.id === "limit-and-controller-checks");
      assert.strictEqual(shown.filter(row => requirementIds.has(row.id)).length + (counted ? Number(counted.measured.split(" ")[0]) : 0), expectedRows,
        "Show every mandatory actual-ST requirement, including transfer");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false");
      await capture("06-validated-controller");
      // Validate's model check moved the live plant: one Apply waits until it rests at the validated point, then commissions.
      const applyStart = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-apply]");
      const events = JSON.parse(await pollValue("apply outcome", readEvents!, text => JSON.parse(text).slice(applyStart)
        .some((event: { status: string }) => ["commissioned", "failed", "verification-failed", "recovery-required"].includes(event.status)), 300000)).slice(applyStart);
      const applied = events.find((event: { status: string }) => event.status === "commissioned");
      assert.ok(applied, `One Apply waits for the process and commissions without asking to apply again: ${JSON.stringify(events.filter((event: { status: string }) => event.status !== "running"))}`);
      const entry = applied.commissioning.transfers.find((transfer: { transaction?: { applied_values?: Record<string, number> } }) => transfer.transaction?.applied_values?.["P1.Controller.StateValue.NominalOutput"] !== undefined);
      assert.ok(entry, "The actual library state transfer was observed");
      for (const state of loop.controller.pidInitialization.states) {
        assert.ok(Number.isFinite(entry.transaction.applied_values[state.symbol]), `${state.symbol} adopted`);
        assert.ok(Number.isFinite(entry.transaction.observed_values[state.symbol]), `${state.symbol} observed after ST`);
      }
      assert.strictEqual(entry.transaction.confirmation, "output-image");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Restore previous settings");
      await capture("07-applied");
      const eventsBeforeClose = await readEvents();
      const activeBeforeClose = await client.status();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port, "", loop.id);
      await pollValue("reopened host state", () => inner("return d.querySelector('[data-studio-three-page]').dataset.hostReady;"), value => value === "true", 10000);
      await inner(`d.defaultView.__journeyEvents=${eventsBeforeClose};${observe}return 'reopened listener';`);
      await click("[data-engineer-stage-tab=verify]");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Restore previous settings", "Reopening inspects the active receipt and offers its existing guarded restoration");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false");
      assert.deepStrictEqual(await client.status(), activeBeforeClose, "Reopening performs no parameter/state transaction");
      await capture("07-reopened-application");
      const beforeRestore = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-apply]");
      const restored = await waitStatus("applied", 20000, beforeRestore);
      assert.strictEqual(restored.applicationAction, "restore");
      assert.deepStrictEqual((await client.status()).values, before.values);
      await capture("08-restored");
      await click("[data-engineer-stage-tab=tune]");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual';");
      const gainSymbol = loop.controller.law.proportionalGainSymbol;
      const enteredGain = candidate.values[gainSymbol] * 1.01;
      await inner(`const input=[...d.querySelectorAll('[data-studio-manual-value]')].find(field=>field.dataset.studioManualValue===${JSON.stringify(gainSymbol)});input.value=${JSON.stringify(String(enteredGain))};input.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));const pin=[...d.querySelectorAll('[data-studio-manual-pin]')].find(field=>field.dataset.studioManualPin===${JSON.stringify(gainSymbol)});pin.checked=true;pin.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual gain pinned';`);
      let since = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-tune]");
      const manual = await waitStatus("candidate", 20000, since);
      assert.strictEqual(manual.values[gainSymbol], enteredGain, "Manual Tune retains the entered value at full precision");
      since = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-stage-tab=verify]");
      await click("[data-engineer-verify]");
      await waitStatus("verified", 120000, since);
      assert.deepStrictEqual((await client.status()).values, before.values, "Manual Tune and Validate do not write the running controller");
      await click("[data-engineer-stage-tab=tune]");
      await capture("09-manual-tuned");
      await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='automatic';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'automatic with pin';");
      since = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-tune]");
      const pinned = await waitStatus("candidate", 20000, since);
      assert.strictEqual(pinned.values[gainSymbol], enteredGain, "Automatic Tune preserves the explicit pin");
      since = Number(await inner("return String(d.defaultView.__journeyEvents.length);"));
      await click("[data-engineer-stage-tab=verify]");
      await click("[data-engineer-verify]");
      await waitStatus("verified", 120000, since);
      assert.deepStrictEqual((await client.status()).values, before.values);
      await capture("10-pinned-validation");
    } finally {
      if (readEvents) { try { fs.writeFileSync(path.join(captures, "three-page-native-events.json"), await readEvents()); } catch { /* Retain other evidence if the owned editor has closed. */ } }
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtimeLifecycleService.stopRuntime();
    }
  });

});
