import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, captureRenderedWorkbench } from "./plant-rendered-harness";

suite("Control Studio captured cancellation evidence", () => {
  test("retains streamed samples when cancelling actual ST verification", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const sourcePath = path.join(project, "src", "Main.st");
    fs.writeFileSync(sourcePath, fs.readFileSync(sourcePath, "utf8")
      .replace("END_VAR", "    Work : DINT;\n    Scratch : LREAL;\nEND_VAR")
      .replace("IF Manual THEN", "IF NOT Manual THEN\n    FOR Work := 1 TO 500 DO Scratch := Scratch + 0.0001; END_FOR;\nEND_IF;\nIF Manual THEN"));
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.controller.modeSignal = "Main.Manual"; loop.controller.modeWhenTrue = "Manual";
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const connector = require("../../controlStudio/runtimeSearchConnection");
    const original = connector.connectRuntimeSearch;
    const recorder = require("../../controlStudio/runtimeRecording");
    const originalStop = recorder.RuntimeRecordingSession.prototype.stop;
    const persistence = require("../../controlStudio/panelPersistence");
    const originalRestore = persistence.restoreWorkflowResultHistory;
    let restoredHistory: any;
    persistence.restoreWorkflowResultHistory = (...args: unknown[]) => {
      const result = originalRestore(...args); restoredHistory = result; return result;
    };
    let projectedRecording: any;
    recorder.RuntimeRecordingSession.prototype.stop = async function () {
      const result = await originalStop.call(this); projectedRecording = result; return result;
    };
    let releaseStart: (() => void) | undefined;
    let cleanupPendingStart = async () => {};
    try {
      let session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("workflow ready", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
      await inner("d.querySelector('[data-control-studio-record-start]').click();return 'record mode';");
      await pollValue("runtime recording started", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
        value => value.includes("Recording at runtime"), 10000);
      await new Promise(resolve => setTimeout(resolve, 300));
      await inner("d.querySelector('[data-control-studio-record-stop]').click();return 'stop record';");
      const recordingStatus = await pollValue("recording completes or explains failure", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
        value => /^(Complete|Incomplete) recording/.test(value), 10000);
      console.log("Symbolic recording outcome", recordingStatus);
      assert.match(recordingStatus, /Complete recording/, "Record the actual symbolic Manual mode without an invented I/O address");
      assert.ok(projectedRecording.rawSamples.length >= 2);
      assert.ok(projectedRecording.rawSamples.every((sample: any) => sample.controllerMode === "Manual"));
      assert.strictEqual(typeof projectedRecording.evidence.programRevision, "string");
      await inner("d.querySelector('[data-engineer-supply]').click();return 'supply';");
      const modelPath = path.join(project, "supplied-model.json");
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
      fs.writeFileSync(modelPath, JSON.stringify({ kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        source: "provided", validation: "validated", evidence: ["bench"], revision: "cancellation-model",
        inputs: [channel("u", "V")], outputs: [channel("y", "m")] }));
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const input = tree.nodes.find((node: any) => node.nodeName === "INPUT" && node.attributes?.includes("general-model-file"));
      assert.ok(input);
      await connection.send("DOM.setFileInputFiles", { files: [modelPath], backendNodeId: input.backendNodeId }, session);
      await pollValue("model selected", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("P control"), 10000);
      await inner("d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__prefixState=e.detail);d.querySelector('[data-engineer-duration]').value='20';d.querySelector('[data-engineer-tune]').click();return 'tune';");
      await pollValue("candidate", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Candidate ready"), 10000);
      let job: { job_id: string; project_revision: string } | undefined;
      let requestCount = 0;
      let startEntered = false;
      const pendingStart = new Promise<void>(resolve => { releaseStart = resolve; });
      cleanupPendingStart = async () => {
        if (!releaseStart) return;
        const release = releaseStart; releaseStart = undefined;
        await inner("d.querySelector('[data-engineer-cancel]').click();return 'cleanup cancel';");
        release();
        await pollValue("cancel pending start", () => inner("return String(d.defaultView.__prefixState?.status);"), value => value === "cancelled", 15000);
      };
      connector.connectRuntimeSearch = async (...args: unknown[]) => {
        const bound = await original(...args);
        return { ...bound, request: async (operation: string, params: any) => {
          if (operation === "control.simulation.run.start") { startEntered = true; await pendingStart; }
          const result = await bound.request(operation, params);
          if (operation === "control.simulation.run.start") { requestCount++; job ??= result; }
          return result;
        } };
      };
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      await pollValue("Verify preparing its owned job", async () => String(startEntered), value => value === "true", 10000);
      const replacementPath = path.join(project, "replacement-model.json");
      fs.writeFileSync(replacementPath, JSON.stringify({ ...JSON.parse(fs.readFileSync(modelPath, "utf8")),
        revision: "replacement", validation: "unvalidated", evidence: [], numerator: [2] }));
      await inner("d.querySelector('[data-engineer-supply]').click();return 'attempt replacement';");
      await connection.send("DOM.setFileInputFiles", { files: [replacementPath], backendNodeId: input.backendNodeId }, session);
      const rejectedImport = await pollValue("model import rejected during Verify", () => inner("return d.querySelector('[data-general-model-status]').textContent;"),
        value => /running operation|replacement/.test(value), 10000);
      assert.match(rejectedImport, /running operation/);
      assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Tune').dataset.status;"), "complete",
        "Rejected import must preserve current candidate readiness");
      assert.match(rejectedImport, /Supply model/, "Recovery names the actual operation to retry");
      releaseStart!(); releaseStart = undefined;
      const bound = await original(loop);
      await pollValue("owned verification job", async () => job ? "ready" : "waiting", value => value === "ready", 10000);
      let received = 0;
      await pollValue("actual worker sample delivery", async () => {
        const status = await bound.request("control.simulation.run.status", { job_id: job!.job_id,
          expected_project_revision: job!.project_revision, include_results: false });
        assert.strictEqual(typeof status.captured_sample_count, "number", "Runtime must report received sample frames");
        assert.ok(["queued", "running"].includes(status.status), `The cancellation must interrupt an unfinished actual-ST run: ${JSON.stringify(status)}`);
        received = status.captured_sample_count;
        return received >= 3 ? "captured" : "waiting";
      }, value => value === "captured", 10000);
      await inner("d.querySelector('[data-engineer-cancel]').click();return 'cancel';");
      await pollValue("confirmed cancellation", () => inner("return String(d.defaultView.__prefixState?.status);"), value => value === "cancelled", 15000);
      const state = JSON.parse(await inner("return JSON.stringify(d.defaultView.__prefixState);"));
      const outcome = state.verificationAttempts[0];
      assert.strictEqual(outcome.status, "cancelled");
      assert.strictEqual(outcome.run.evidence, "ST-executed");
      assert.ok(outcome.run.samples.length >= received);
      assert.ok(outcome.run.samples.some((sample: any) => sample.timeSeconds > 0
        && sample.values["Main.MV"].value !== outcome.run.samples[0].values["Main.MV"].value),
        "Retain actual controller updates as well as the initial simulator sample");
      assert.ok(outcome.run.samples.length < outcome.run.conditions.sampleTimesSeconds.length);
      assert.strictEqual(requestCount, 1, "Cancellation prevents the candidate scenario starting after the baseline");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
      const colors = JSON.parse(await inner("const b=d.querySelector('[data-engineer-apply]');const token=d.createElement('span');token.style.backgroundColor='var(--vscode-button-secondaryBackground)';d.body.append(token);const result={actual:d.defaultView.getComputedStyle(b).backgroundColor,expected:d.defaultView.getComputedStyle(token).backgroundColor};token.remove();return JSON.stringify(result);"));
      assert.strictEqual(colors.actual, colors.expected, "Disabled Engineer controls use the shared button background so their labels remain readable");
      await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Validate').click();return 'inspect';");
      assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-verification-history] tbody tr').length);")), outcome.run.samples.length);
      const interruptedChart = JSON.parse(await inner("const host=d.querySelector('[data-engineer-recorded-responses]');const plot=host?.querySelector('[data-engineer-recorded-plot=interrupted]');return JSON.stringify({visible:Boolean(host&&!host.hidden&&plot),text:plot?.textContent??'',rows:plot?.querySelectorAll('[data-engineer-recorded-values] tbody tr').length??0});"));
      assert.strictEqual(interruptedChart.visible, true,
        `A cancelled actual-ST prefix must render one interrupted response chart instead of only raw history rows: ${JSON.stringify({ interruptedChart, attempt: outcome })}`);
      assert.match(interruptedChart.text, /ST-executed/i);
      assert.match(interruptedChart.text, /cancelled/i);
      assert.match(interruptedChart.text, /no terminal point/i);
      assert.strictEqual(interruptedChart.rows, outcome.run.samples.length,
        "The interrupted chart must show exactly the received native prefix without inventing a comparison run.");
      await inner("const h=d.querySelector('[data-engineer-verification-history]');h.querySelectorAll('details').forEach(e=>e.open=true);const chart=d.querySelector('[data-engineer-recorded-plot=interrupted]');chart.scrollIntoView({block:'center'});return 'inspect interrupted prefix';");
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) { await new Promise(resolve => setTimeout(resolve, 150)); captureRenderedWorkbench(screenshot); }
      await inner("d.querySelector('[data-result-save]').click();return 'save';");
      await pollValue("saved cancelled samples", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => value.includes("Saved revision"), 10000);
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port);
      await pollValue("reopened cancellation prefix", () => inner("return d.querySelector('[data-engineer-verification-history]')?.textContent??'';"), value => value.includes(outcome.jobId), 10000);
      assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-verification-history] tbody tr').length);")), outcome.run.samples.length);
      const restoredRecording = restoredHistory.history.at(-1).recording;
      assert.strictEqual(restoredRecording.acquisition.programRevision, projectedRecording.evidence.programRevision,
        "Save and reopen retain the actual recording program identity");
      assert.deepStrictEqual(restoredRecording.rawSamples, JSON.parse(JSON.stringify(projectedRecording.rawSamples)),
        "Save and reopen preserve every recorded value, mode and quality");
      console.log("Reopened symbolic recording", JSON.stringify({ programRevision: restoredRecording.acquisition.programRevision,
        samples: restoredRecording.rawSamples.length }));
      await inner("d.querySelector('[data-engineer-supply]').click();return 'retry replacement';");
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const reopenedTree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const reopenedInput = reopenedTree.nodes.find((node: any) => node.nodeName === "INPUT" && node.attributes?.includes("general-model-file"));
      await connection.send("DOM.setFileInputFiles", { files: [replacementPath], backendNodeId: reopenedInput.backendNodeId }, session);
      await pollValue("retry imports the new model after confirmed cancellation", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("replacement"), 10000);
      console.log("Retained actual cancelled prefix", JSON.stringify({ received, rows: outcome.run.samples.length,
        planned: outcome.run.conditions.sampleTimesSeconds.length, jobId: outcome.jobId }));
    } finally {
      try { await cleanupPendingStart(); } catch (error) { console.error("Pending-start cleanup", error); }
      recorder.RuntimeRecordingSession.prototype.stop = originalStop;
      persistence.restoreWorkflowResultHistory = originalRestore;
      connector.connectRuntimeSearch = original;
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
