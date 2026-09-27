import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { createWorkflowResult, decodeWorkflowResult } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { exerciseStepFields } from "./control-studio-step-field-interaction";
import { exerciseStepLayout, exerciseStepPlots } from "./control-studio-step-layout-interaction";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

suite("Control Studio adverse step response", () => {
  test("recovers from incomplete windows and shows measured falling onset before and after fitting", async function () {
    if (process.env.TRUST_STEP_ADVERSE_RENDERED !== "1") this.skip();
    this.timeout(process.env.TRUST_STEP_LAYOUT === "1" || process.env.TRUST_STEP_PLOTS === "1" ? 120000 : process.env.TRUST_STEP_FIELDS === "1" ? 90000 : 45000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const root = path.resolve(__dirname, "../../../../..");
    const loop = JSON.parse(fs.readFileSync(path.join(root,
      "examples/control/05-experiments-identification-and-validation/23-step-response-identification/control-loop.json"), "utf8"));
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const units = { sp: "", pv: "°C", mv: "%" };
    // Analytical fixture exercises the real import/analysis UI; it is not native plant evidence.
    const result = createWorkflowResult({ resultId: "analytical-falling-step-fixture", resultRevision: 1,
      projectId: "project:analytical-fixture", source: "Simulation",
      loop: { id: "analytical-fixture", revision: "sha256:analytical-fixture", topology: "open-loop" },
      controller: { instance: "Fixture", revision: "sha256:analytical-fixture" },
      software: { extensionVersion: "0.24.65", numericalAdapters: {} }, units,
      recording: { rawSamples: Array.from({ length: 41 }, (_, i) => ({ elapsedSeconds: i / 10,
        observedAtMs: 1000 + i * 100, actualCycleSeconds: .1, scan: i + 1, source: "Simulation" as const,
        loopRevision: "sha256:analytical-fixture", sp: null,
        pv: i < 10 ? 35 : 35 - 2 * (1 - Math.exp(-Math.max(0, i / 10 - 1.1) / .2)),
        requestedMv: null, appliedMv: i >= 10 && i < 40 ? 65 : 75, units,
        quality: "good" as const, freshnessMs: 0, controllerMode: "Unavailable" as const,
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable" as const, events: [] })),
      selectedIntervals: [], acquisition: { method: "runtime_scan_buffer", completeness: "complete",
        programRevision: "sha256:analytical-fixture", observedSampleCount: 41, scanRange: { first: 1, last: 41 },
        missingScanRanges: [], droppedObservedSamples: 0, actualCycleTimingAvailable: true, freshnessAvailable: true,
        reason: "Analytical UI regression fixture; no native acquisition or physical qualification." } },
      evidence: { observation: "Not observed", model: "Not evaluated" }, conclusion: "Analytical UI regression fixture." }, "2026-09-13T00:00:00.000Z");
    assert.ok(decodeWorkflowResult(result), "the import fixture must satisfy the public result contract");
    const file = path.join(captures, "analytical-falling-step.json");
    fs.writeFileSync(file, JSON.stringify(result, null, 2));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const reviewFailures: string[] = [];
      const review = (condition: boolean, reason: string) => { if (!condition) reviewFailures.push(reason); };
      const click = (selector: string) => inner(`d.querySelector(${JSON.stringify(selector)}).click();return 'clicked';`);
      const capture = async (name: string) => { await sleep(250); await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`)); };
      await pollValue("handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), v => v === "true", 10000);
      await click('[data-control-studio-mode][aria-label^="Identify,"]');
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const node = tree.nodes.find((n: any) => n.nodeName === "INPUT" && n.attributes?.includes("identify-import-file"));
      assert.ok(node);
      await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session);
      await pollValue("import complete", () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => /Imported recording ready/.test(v), 10000);
      await click("[data-step-response-enabled]");
      const fields = { baselineDurationSeconds: 1.1, finalDurationSeconds: 1, "delay.maximum": .2,
        "timeConstant.minimum": .1, "timeConstant.maximum": .4, "timeConstant.step": .1 };
      const setField = (key: string, value: number) => inner(`var f=d.querySelector('[data-step-field="${key}"]');f.value=${JSON.stringify(String(value))};f.dispatchEvent(new Event('input',{bubbles:true}));return 'edited';`);
      for (const [key, value] of Object.entries(fields)) await setField(key, value);
      await click("[data-identify-run]");
      await pollValue("missing baseline rejected", () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => /baseline window.*coverage/i.test(v), 10000);
      review(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Identify,\"]').getAttribute('aria-label');") === "Identify, Failed",
        "rejected recording must show Identify, Failed in the visible/accessibility stage status");
      review(/Shorten the baseline window|longer complete recording/.test(await inner("return d.querySelector('[data-identify-state]').textContent;")),
        "missing baseline coverage must explain how to correct the window or recording");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-fit]').hidden);"), "true");
      await capture("00-incomplete-window");
      await setField("baselineDurationSeconds", 1);
      await click("[data-identify-run]");
      await pollValue("qualified after correction", () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => /Estimation recording qualified/.test(v), 10000);
      review(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Identify,\"]').getAttribute('aria-label');") === "Identify, Ready",
        "a successful recheck must clear the failed stage status");
      await click("[data-step-response-evidence] > summary");
      const row = (label: string) => inner(`return [...d.querySelectorAll('[data-step-response-report] tr')].find(r=>r.querySelector('th').textContent===${JSON.stringify(label)})?.querySelector('td').textContent??'';`);
      assert.match(await row("Detectable onset"), /^0\.2 s after step/);
      assert.match(await row("Departure / 63.2% level"), /^34\.85 /);
      review((await row("Final")).startsWith("33 °C"), "window means must use shared readable precision rather than six forced decimal places");
      review(/3 consecutive samples.*0\.15 °C below baseline/.test(await row("Onset criterion")),
        "the measurement summary must explain the configured departure and consecutive-sample criterion");
      const bounds = JSON.parse(await inner("var a=d.querySelector('[data-step-response-evidence]').getBoundingClientRect();var b=d.querySelector('[data-workflow-pane=Identify]').getBoundingClientRect();return JSON.stringify({left:a.left,paneLeft:b.left,width:a.width,paneWidth:b.width});"));
      review(bounds.left <= bounds.paneLeft + 1 && bounds.width >= bounds.paneWidth * .9,
        "expanded measurements must span the workspace below the input and qualification columns");
      await inner("d.querySelector('[data-step-response-evidence]').scrollIntoView({block:'end'});return 'measurement evidence';");
      await capture("01-falling-measurement");
      await click("[data-identify-fit]");
      await pollValue("fit preview", () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => /Preview only/.test(v), 10000);
      assert.match(await row("Detectable onset"), /^0\.2 s after step/);
      assert.match(await row("Departure / 63.2% level"), /^34\.85 /);
      assert.strictEqual(await inner("return d.querySelector('[data-plot-tab=fit]').getAttribute('aria-selected');"), "true");
      await inner("d.querySelector('[data-step-response-evidence]').scrollIntoView({block:'end'});return 'evidence';");
      await capture("02-falling-fit");
      assert.deepStrictEqual(reviewFailures, [], "Identify review must communicate rejection, recovery and measurement evidence clearly");
      await click('[data-control-studio-mode][aria-label^="Validate,"]');
      await click("[data-result-save]");
      await pollValue("preview saved", () => inner("return d.querySelector('[data-result-state]').textContent;"), v => /Saved revision 1/.test(v), 10000);
      await inner("var h=d.querySelector('[data-result-history]');h.closest('details').open=true;h.value='1';h.dispatchEvent(new Event('change',{bubbles:true}));return 'select saved preview';");
      await click("[data-result-open]");
      await pollValue("preview opened", () => inner("return d.querySelector('[data-result-state]').textContent;"), v => /^Opened/.test(v), 10000);
      await click('[data-control-studio-mode][aria-label^="Monitor,"]');
      await capture("03-monitor-retained-recording");
      assert.match(await inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
        /Retained recording.*Validate/, "Monitor must explain where the opened recording remains available");
      await click('[data-control-studio-mode][aria-label^="Validate,"]');
      assert.strictEqual(await inner("return d.querySelector('[data-plot-tab=recorded-response]').getAttribute('aria-selected');"), "true");
      await capture("04-return-to-recording");
      if (process.env.TRUST_STEP_FIELDS === "1") await exerciseStepFields(connection, inner, capture, captures);
      if (process.env.TRUST_STEP_LAYOUT === "1") await exerciseStepLayout(inner, capture, captures);
      if (process.env.TRUST_STEP_PLOTS === "1") await exerciseStepPlots(inner, capture, captures);
    } finally {
      connection.close();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
    }
  });
});
