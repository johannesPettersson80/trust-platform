import { exportRecordedComparisonThroughHost } from "./control-studio-export-interaction";
import { exerciseControlStudioScenarioSettings } from "./control-studio-settings-interactions";
import { captureControlStudioViews } from "./control-studio-view-gallery";
import { inspectFailedController } from "./control-studio-inspect-failure-interaction";
import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, captureRenderedWorkbench } from "./plant-rendered-harness";
import { assertCleanHierarchy, assertInitialStageTabs, assertModelNavigationPreservesSettings, assertVerifiedNavigationPreservesEvidence, stageTabs } from "./control-studio-navigation-assertions";

suite("Control Studio ordinary engineer workflow", () => {
  test(process.env.TRUST_ENGINEER_GAIN_ACK_LOSS === "1" ? "recovers committed gains after lost application acknowledgement" : process.env.TRUST_ENGINEER_CANCEL_COMMISSIONING === "1" ? "cancels Auto commissioning and confirms Manual recovery" : process.env.TRUST_ENGINEER_SAVE_COMMISSIONING === "1" ? "retains Auto commissioning records without restoring write authority" : process.env.TRUST_ENGINEER_RUNTIME_FAULT === "1" ? "retains actual ST fault samples through save and reopen" : "supplies a model tunes verifies actual ST and applies then rolls back a proportional loop", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENGINEER_WORKFLOW !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const operatingPoint = process.env.TRUST_ENGINEER_OPERATING_POINT === "1";
    const recordingAckLoss = process.env.TRUST_ENGINEER_RECORDING_ACK_LOSS === "1";
    let blockRecordingStop = process.env.TRUST_ENGINEER_RECORDING_STOP_FAULT === "1";
    let lostRecordingId: string | undefined;
    let startsAfterLostAcknowledgement = 0;
    const recordingStops: string[] = [];
    const recordingContention = ["1", "exhaust"].includes(process.env.TRUST_ENGINEER_RECORDING_CONTENTION ?? "");
    const exhaustRecordingRetries = process.env.TRUST_ENGINEER_RECORDING_CONTENTION === "exhaust";
    const manualTolerance = process.env.TRUST_ENGINEER_TRANSFER_TOLERANCE === "1";
    const commissioning = process.env.TRUST_ENGINEER_COMMISSIONING === "1";
    const runtimeFault = process.env.TRUST_ENGINEER_RUNTIME_FAULT === "1";
    const emptyRuntimeFault = process.env.TRUST_ENGINEER_RUNTIME_FAULT_EMPTY === "1";
    const faultBadQuality = process.env.TRUST_ENGINEER_FAULT_BAD_QUALITY === "1";
    if (runtimeFault) {
      const file = path.join(project, "src", "Main.st");
      const source = fs.readFileSync(file, "utf8");
      fs.writeFileSync(file, source.replace("END_VAR", "    VerifyCycle : DINT;\n    VerifyDivisor : DINT := 1;\n    VerifyQuotient : DINT;\nEND_VAR")
        .replace("IF Manual THEN", `IF NOT Manual THEN
    VerifyCycle := VerifyCycle + 1;
    IF VerifyCycle >= ${emptyRuntimeFault ? "1" : "3"} THEN VerifyDivisor := 0; END_IF;
    VerifyQuotient := 100 / VerifyDivisor;
END_IF;
IF Manual THEN`));
    }
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    if (process.env.TRUST_ENGINEER_SAVE_OWNER_RACE === "1") loop.plantId = "engineer-first-order";
    loop.controller.modeSignal = "Main.Manual"; loop.controller.modeWhenTrue = "Manual";
    loop.controller.manualOutputType = "LREAL"; // Matches this fixture's declared Main.ManualCommand.
    const entry = process.env.TRUST_ENGINEER_MODEL_ENTRY ?? "supply";
    if (entry === "find") {
      // This journey asserts physical gain/time-constant accuracy, so request
      // a tighter independent fit than the general exploratory model default.
      loop.identificationPolicy = { openLoopPermitted: true, sustainedHoldPermitted: true,
        information: "basic", maximumNormalizedError: 0.02 };
      loop.analysis.plant.numerator = [999];
      loop.analysis.plant.denominator = [9, 1];
      loop.analysis.plant.revision = "sha256:deliberately-wrong-supplied-coefficients";
    }
    if (process.env.TRUST_ENGINEER_MODEL_FREE_LAW === "1") {
      delete loop.analysis; delete loop.analysisParameterSymbols;
      loop.controller.law = { kind: "proportional", gainSymbol: "Main.Kp" };
    }
    if (operatingPoint) {
      const plantPath = path.join(project, "simulation.toml");
      fs.writeFileSync(plantPath, fs.readFileSync(plantPath, "utf8").replace("initial_input = [0.0]",
        "initial_input = [0.0]\ninput_offset = [-2.0]\noutput_offset = [10.0]"));
      const stPath = path.join(project, "src", "Main.st");
      fs.writeFileSync(stPath, fs.readFileSync(stPath, "utf8").replace(/ManualCommand( AT %ML8)? : LREAL := LREAL#0\.0;/,
        "ManualCommand$1 : LREAL := LREAL#2.0;").replace("MV AT %QL0 : LREAL;", "MV AT %QL0 : LREAL := LREAL#2.0;")
        .replace("END_VAR", "    OperatingPointInitialized : BOOL := FALSE;\nEND_VAR")
        .replace("IF Manual THEN", "IF NOT OperatingPointInitialized THEN ManualCommand := LREAL#2.0; OperatingPointInitialized := TRUE; END_IF;\nIF Manual THEN"));
    }
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let cleanupCommissioning: (() => Promise<void>) | undefined;
    const gainAckLoss = process.env.TRUST_ENGINEER_GAIN_ACK_LOSS === "1";
    let lostGainAcknowledgement = false;
    const recoveryFault = process.env.TRUST_ENGINEER_RECOVERY_FAULT === "1";
    const transport = require("../../runtimeControlClient");
    const originalSend = transport.sendRuntimeControlRequest;
    const search = require("../../controlStudio/runtimeSearchConnection");
    const originalSearch = search.connectRuntimeSearch;
    let injectedEmptyTerminal = false;
    if (faultBadQuality || emptyRuntimeFault) search.connectRuntimeSearch = async (...args: unknown[]) => {
      const bound = await originalSearch(...args);
      return { ...bound, request: async (operation: string, params: Record<string, unknown>) => {
        const response = await bound.request(operation, params);
        if (operation !== "control.simulation.run.status" || params.include_results !== true || !response || typeof response !== "object") return response;
        const record = response as any;
        if (record.status !== "failed" || !record.run || !Array.isArray(record.run.samples)) return response;
        if (emptyRuntimeFault && !injectedEmptyTerminal) {
          injectedEmptyTerminal = true;
          return { ...record, run: { ...record.run, samples: [] } };
        }
        const samples = record.run.samples.map((sample: any) => ({ ...sample, values: { ...sample.values } }));
        if (samples[1]?.values?.["Main.PV"]) samples[1].values["Main.PV"] = { ...samples[1].values["Main.PV"], quality: "bad" };
        return { ...record, run: { ...record.run, samples } };
      } };
    };
    let displacedInspection = false;
    let rejectedRecordingStarts = 0;
    if (recordingContention || recordingAckLoss) transport.sendRuntimeControlRequest = async (...args: any[]) => {
      const [endpoint, token, operation, params, options] = args;
      if (recordingAckLoss && operation === "control.recording.start") {
        if (lostRecordingId) startsAfterLostAcknowledgement++;
        const result = await originalSend(...args);
        if (!lostRecordingId) {
          lostRecordingId = params.recording_id;
          await new Promise(resolve => setTimeout(resolve, 60));
          throw new Error("controlled lost recording Start acknowledgement");
        }
        return result;
      }
      if (recordingAckLoss && operation === "control.recording.stop") {
        recordingStops.push(params.recording_id);
        if (blockRecordingStop) throw new Error("controlled recording Stop transport interruption");
      }
      if (recordingContention && operation === "control.recording.start" && (!displacedInspection || exhaustRecordingRetries)) {
        displacedInspection = true;
        await originalSend(endpoint, token, "control.signals.inspect", {
          loop_revision: params.loop_revision, expected_program_revision: params.program_revision,
          bindings: params.bindings.map((binding: any) => ({ ...binding, id: `other-${binding.id}` })),
        }, options);
      }
      try { return await originalSend(...args); }
      catch (error) {
        if (operation === "control.recording.start" && String(error).includes("current coherent inspection")) rejectedRecordingStarts++;
        throw error;
      }
    };
    const connections = require("../../controlStudio/runtimeConnection");
    const originalBind = connections.bindRuntimeConnection;
    const commissioningPrototype = require("../../controlStudio/proportionalCommissioning").ProportionalCommissioning.prototype;
    const originalCancel = commissioningPrototype.cancel;
    let blockRecovery = recoveryFault;
    let autoRequests = 0;
    const gainApplyIds: string[] = [];
    const transferTolerances: number[] = [];
    const cancelResults: any[] = [];
    if (recoveryFault || gainAckLoss || manualTolerance || recordingContention || recordingAckLoss || process.env.TRUST_ENGINEER_REOPEN_ROLLBACK === "1") {
      connections.bindRuntimeConnection = async (...args: unknown[]) => {
        const bound = await originalBind(...args);
        return { ...bound, request: async (operation: string, params: any) => {
          if (gainAckLoss && operation === "control.parameters.apply") gainApplyIds.push(params.transaction_id);
          if (operation === "control.transfer.apply") {
            transferTolerances.push(params.maximum_output_change);
            if (!params.manual) autoRequests++;
            if (params.manual && blockRecovery) throw new Error("controlled recovery transport interruption");
          }
          const response = await bound.request(operation, params);
          if (gainAckLoss && operation === "control.parameters.apply" && !lostGainAcknowledgement) {
            lostGainAcknowledgement = true;
            await pollValue("native gain commit before dropped acknowledgement", async () => {
              const status = await bound.request("control.parameters.status", { binding: params.binding });
              return JSON.stringify(status);
            }, value => JSON.parse(value).status === "applied", 5000);
            throw new Error("controlled lost gain application acknowledgement");
          }
          return response;
        } };
      };
      commissioningPrototype.cancel = async function () {
        const result = await originalCancel.call(this); cancelResults.push(result); return result;
      };
    }
    try {
      if (process.env.TRUST_ENGINEER_WIDE === "1") {
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      }
      let session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-tune]')));"), "true",
        "An ordinary ST project must expose the connected model, controller, Tune, Verify and Apply workflow");
      await pollValue("engineer handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), s => s === "true", 10000);
      const workflowLayout = async () => JSON.parse(await inner("return JSON.stringify((()=>{const root=d.querySelector('[data-engineer-workflow]');const visible=[...root.querySelectorAll('button')].filter(b=>{const r=b.getBoundingClientRect();return !b.hidden&&getComputedStyle(b).display!=='none'&&r.width>0&&r.height>0;});const primary=visible.filter(b=>b.classList.contains('trust-button--primary'));const task=root.querySelector('[data-engineer-task-summary]');const taskRect=task?.getBoundingClientRect();return {stage:root.dataset.stage,primary:primary.map(b=>b.textContent.trim()),options:root.querySelector('[data-engineer-options]')?.open??false,scenarioControls:Boolean(root.querySelector('[data-engineer-scenario-controls]')?.open&&!root.querySelector('[data-engineer-scenario-controls]')?.closest('details:not([open])')),acquisitionControls:root.querySelector('[data-engineer-acquisition-controls]')?.open??false,taskSummaryVisible:Boolean(task&&!task.hidden&&!task.closest('details:not([open])')&&taskRect&&taskRect.width>0&&taskRect.height>0),overflow:d.documentElement.scrollWidth-d.documentElement.clientWidth};})());"));
      const captureWorkflowStage = async (suffix: string) => {
        const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
        if (!screenshot) return;
        await new Promise(resolve => setTimeout(resolve, 200));
        await vscode.commands.executeCommand("notifications.clearAll");
        captureRenderedWorkbench(screenshot.replace(/\.png$/, `${suffix}.png`));
      };
      await captureWorkflowStage("-start");
      const initialLayout = await workflowLayout();
      const chrome = JSON.parse(await inner("return JSON.stringify({duplicateTitle:!!d.querySelector('.header .identity'),intro:d.querySelector('[data-engineer-status]')?.textContent.trim(),toolsInSettings:!!d.querySelector('[data-engineer-options] [data-engineer-tools]'),scenariosInSettings:!!d.querySelector('[data-engineer-options] [data-engineer-scenario-controls]')});"));
      assert.strictEqual(chrome.duplicateTitle, false, "The editor tab owns the title; Control Studio must not repeat it");
      assert.strictEqual(chrome.intro, "", "Resting actions must not be repeated as introductory prose");
      assert.strictEqual(chrome.toolsInSettings, true, "Manual engineering tools belong inside Settings");
      assert.strictEqual(chrome.scenariosInSettings, true, "Scenario editing belongs inside Settings");
      assert.deepStrictEqual(initialLayout.primary, ["Identify model"], "The initial Workflow 1 state must expose exactly one visible primary next action");
      if (theme?.includes("High Contrast")) {
        const primaryStyle = JSON.parse(await inner("const b=d.querySelector('[data-engineer-find]');const s=d.defaultView.getComputedStyle(b);return JSON.stringify({border:s.borderTopColor,background:s.backgroundColor});"));
        assert.notStrictEqual(primaryStyle.border, primaryStyle.background,
          "High-contrast primary actions must retain a visible button boundary");
      }
      assert.strictEqual(initialLayout.stage, "start", "The initial primary action must match the start stage");
      await assertInitialStageTabs(inner);
      await assertCleanHierarchy(inner);
      assert.strictEqual(initialLayout.options, false, "Secondary engineering settings must start collapsed");
      assert.strictEqual(initialLayout.scenarioControls, false, "Manual scenario controls must start collapsed");
      assert.strictEqual(initialLayout.acquisitionControls, false, "Identification controls must start collapsed");
      assert.strictEqual(initialLayout.taskSummaryVisible, false, "task requirements stay hidden until a model is ready");
      assert.ok(initialLayout.overflow <= 1, `Workflow 1 must fit the available viewport without horizontal overflow: ${initialLayout.overflow}px`);
      if (commissioning) {
        await inner("d.defaultView.addEventListener('trust-control-studio-engineer',e=>{if(e.detail.commissioning)d.defaultView.__commissionResult=e.detail.commissioning;});return 'retain commissioning evidence';");
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-application-mode]')?.value ?? '';"), "auto",
          "Apply must default to the reviewed Auto commissioning workflow");
      } else {
        await inner("const mode=d.querySelector('[data-engineer-application-mode]');if(mode)mode.value='manual';return 'manual staging coverage';");
      }
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-control-studio-automatic-tune]')));"), "false",
        "The connected engineer workflow must have one Tune action, not a duplicate Automatic Tune header");
      const runtime = await (require("../../controlStudio/runtimeConnection").bindRuntimeConnection(loop.runtimeId));
      let inspected: any;
      for (let attempt = 0; attempt < 20; attempt++) {
        inspected = await runtime.request("control.signals.inspect", { loop_revision: loop.revision,
          bindings: [ { id: "sp", address: loop.signals.setpoint.address, iec_type: loop.signals.setpoint.iecType },
            { id: "pv", address: loop.signals.processVariable.address, iec_type: loop.signals.processVariable.iecType },
            { id: "mv", address: loop.signals.manipulatedVariable.address, iec_type: loop.signals.manipulatedVariable.iecType },
            { id: "mode", symbol: "Main.Manual", iec_type: "BOOL" }] });
        if (inspected.available) break;
        await new Promise(resolve => setTimeout(resolve, 50));
      }
      assert.strictEqual(inspected.available, true, "The actual runtime must publish a coherent symbolic scan");
      assert.strictEqual(inspected.values[3], 1, "Actual Main.Manual starts TRUE in this ST fixture");
      if (operatingPoint) {
        assert.strictEqual(inspected.values[2], 2, "The nonzero fixture must hold its declared input before design");
        assert.ok(Math.abs(inspected.values[1] - 10) < 1e-6, "The nonzero fixture must begin at its declared physical equilibrium");
      }
      console.log("Actual symbolic inspection", JSON.stringify(inspected));
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-freshness]').textContent;"), "Live", "Live status must not expose the internal scan counter");
      await new Promise(resolve => setTimeout(resolve, 600));
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), "Manual",
        "The live header must display the observed symbolic mode from a coherent scan");
      console.log(`Engineer workflow model entry: ${entry}`);
      await inner("d.defaultView.__engineerLatest=undefined;d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__engineerLatest=e.detail);return 'listen for model handoff';");
      if (entry === "find") {
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "false",
          "Find model must execute identification and independent validation from the declared manual-output binding");
        await inner("d.querySelector('[data-engineer-find]').click();return 'find';");
        const findStatus = await pollValue("independently identified model", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"),
          text => { try { const event=JSON.parse(text); return event.status === "model-ready" || String(event.status).includes("failed"); } catch { return false; } }, 30000);
        console.log("Find result", findStatus);
        const findEvent = JSON.parse(findStatus);
        assert.strictEqual(findEvent.status, "model-ready", `Find must hand off a validated model through the host event: ${findStatus}`);
        assert.match(String(findEvent.reason), /model|tune/i);
        const identified = findEvent.model;
        console.log("Independently identified model", JSON.stringify(identified));
        assert.strictEqual(identified.source, "fitted");
        await captureWorkflowStage("-identified-model");
        assert.strictEqual(await inner("return d.querySelector('[data-control-plot-tabs] [aria-selected=true]').dataset.plotTab;"), "fit", "Identify must show measured versus predicted response before frequency analysis");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identification-fit-plot]').getBoundingClientRect().height>100);"), "true");
        if (operatingPoint) assert.deepStrictEqual(identified.operatingPoint, { inputs: [2], outputs: [10] });
        assert.strictEqual(identified.validation, "validated");
        assert.ok(Math.abs(identified.numerator[0] / identified.denominator[1] - 1) < 0.1,
          "Find model must recover the measured simulator gain, ignoring the deliberately wrong supplied gain999");
        assert.ok(Math.abs(identified.denominator[0] / identified.denominator[1] - 1) < 0.1,
          "Find model must recover the measured time constant, ignoring the supplied time constant9");

      } else {
      await inner("d.querySelector('[data-engineer-supply]').click();return 'supply';");
      const modelPath = path.join(project, "supplied-model.json");
      const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
      fs.writeFileSync(modelPath, JSON.stringify({ kind: "transfer-function", numerator: [1], denominator: [1,1],
        delaySeconds: 0, source: "provided", validation: "validated", evidence: ["independent bench model reference"],
        ...(operatingPoint ? { operatingPoint: { inputs: [2], outputs: [10] } } : {}),
        revision: "sha256:bench-model", inputs: [channel("drive", "V")], outputs: [channel("position", "m")] }));
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const node = tree.nodes.find((n: any) => n.nodeName === "INPUT" && n.attributes?.includes("general-model-file"));
      assert.ok(node);
      await connection.send("DOM.setFileInputFiles", { files: [modelPath], backendNodeId: node.backendNodeId }, session);
      const suppliedStatus = await pollValue("controller recommendation", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"),
        text => { try { const event=JSON.parse(text); return event.status === "model-ready" || String(event.status).includes("failed"); } catch { return false; } }, 10000);
      const suppliedEvent = JSON.parse(suppliedStatus);
      assert.strictEqual(suppliedEvent.status, "model-ready", `Supply must hand off the selected model through the host event: ${suppliedStatus}`);
      assert.notStrictEqual(await inner("return String(d.querySelector('[data-control-studio]')?.dataset.manualWorkspace);"), "true",
        "supplying a model must remain in the connected Workflow 1 surface instead of opening the legacy manual Tune workspace");
      assert.strictEqual(await inner("const b=[...d.querySelectorAll('[data-engineer-actions] .trust-button--primary')].find(b=>!b.hidden);const r=b?.getBoundingClientRect();return String(Boolean(b&&r&&r.width>0&&r.height>0&&r.top>=0&&r.bottom<=d.defaultView.innerHeight));"), "true",
        "the supplied model state must keep its single next action visible in the workflow viewport");
      }
      assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Identify').dataset.status;"), "complete");
      assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Tune').dataset.status;"), "ready");
      const taskSummary = await inner("const e=d.querySelector('[data-engineer-task-summary]');const r=e?.getBoundingClientRect();return JSON.stringify({text:e?.textContent??'',visible:Boolean(e&&!e.hidden&&!e.closest('details:not([open])')&&r&&r.width>0&&r.height>0)});");
      const parsedTaskSummary = JSON.parse(taskSummary);
      assert.strictEqual(parsedTaskSummary.visible, false, "Model keeps the task summary hidden until the model is used");
      const selectedModelPlot = entry === "find" ? "fit" : "general-model";
      await pollValue("selected model graph", () => inner(`return String([...d.querySelectorAll('[data-plot-view=${selectedModelPlot}] canvas')].some(c=>{const r=c.getBoundingClientRect();return r.width>100&&r.height>50;}));`), value => value === "true", 10000);
      if (process.env.TRUST_ENGINEER_CAPTURE_VIEWS === "1") { await captureControlStudioViews(inner, captureWorkflowStage, "model-ready", undefined, true); await exerciseControlStudioScenarioSettings(inner, captureWorkflowStage); }
      const modelReadyLayout = await workflowLayout();
      assert.deepStrictEqual(modelReadyLayout.primary, ["Continue to Tune"],
        "a ready model must expose Use model as the explicit next action before Tune");
      await inner("d.querySelector('[data-engineer-use-model]')?.click();return 'use model';");
      const goalForm = JSON.parse(await inner("const g=d.querySelector('[data-engineer-goal-fields]');return JSON.stringify({visible:!g.hidden&&g.getBoundingClientRect().height>0,disabled:g.disabled,values:[...g.querySelectorAll('input')].map(i=>i.value)});"));
      assert.strictEqual(goalForm.visible, true, "Tune must expose its response target inputs directly");
      assert.strictEqual(goalForm.disabled, false, "A selected model enables response target editing");
      assert.deepStrictEqual(goalForm.values, ["1", "0.1", "2"]);
      const reopenStudy = process.env.TRUST_ENGINEER_SAVE_REOPEN === "1";
      const manualPin = process.env.TRUST_ENGINEER_MANUAL_PIN === "1";
      await inner("d.defaultView.__engineerLatest=undefined;d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__engineerLatest=e.detail);return 'listen';");
      await inner("d.querySelector('[data-engineer-reference]').value='1';d.querySelector('[data-engineer-error]').value='0.1';d.querySelector('[data-engineer-band]').value='0.02';d.querySelector('[data-engineer-settling]').value='2';d.querySelector('[data-engineer-duration]').value='4';return 'task';");
      if (operatingPoint) {
        // P control without bias needs a feasible residual-error allowance at
        // nonzero holding output; this case qualifies coordinate-preserving transfer.
        await inner("d.querySelector('[data-engineer-error]').value='0.2';return 'nonzero operating-point task';");
      }
      if (reopenStudy) {
        await inner("d.querySelector('[data-engineer-reference]').value='1.2';d.querySelector('[data-engineer-error]').value='0.15';d.querySelector('[data-engineer-band]').value='0.025';d.querySelector('[data-engineer-settling]').value='1.5';d.querySelector('[data-engineer-duration]').value='3';return 'distinct-task';");
      }
      if (manualPin) {
        assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-gain-mode]')&&d.querySelector('[data-engineer-gain-value]')));"), "true",
          "The engineer must be able to choose and pin a manual gain while automating the rest");
        await inner("const mode=d.querySelector('[data-engineer-gain-mode]');mode.closest('details').open=true;mode.value='manual';mode.dispatchEvent(new d.defaultView.Event('change'));d.querySelector('[data-engineer-gain-value]').value='1';d.querySelector('[data-engineer-tune]').click();return 'manual poor candidate';");
        const manualCandidateEvent = JSON.parse(await pollValue("manual exploratory candidate", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => {
          try { const event = JSON.parse(text); return ["candidate", "failed"].includes(event.status); } catch { return false; }
        }, 10000));
        assert.strictEqual(manualCandidateEvent.status, "candidate");
        assert.strictEqual(await inner("return String(d.defaultView.__engineerLatest.design.candidate.proportionalGain);"), "1", "Tune must not replace the entered gain to meet a target");
        await inner("d.querySelector('[data-engineer-verify]').click();return 'verify poor gain';");
        const failedManual = JSON.parse(await pollValue("failed manual requirement evidence", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => {
          try { const event = JSON.parse(text); return ["verification-failed", "failed"].includes(event.status); } catch { return false; }
        }, 15000));
        assert.ok(["verification-failed", "failed"].includes(failedManual.status));
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
        assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Validate').dataset.status;"), "failed");
        await inner("d.querySelector('[data-engineer-gain-value]').value='20';return 'pin qualifying gain';");
      }
      if (manualTolerance) {
        assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-transfer-tolerance]')));"), "true",
          "The engineer must be able to choose an Auto-entry tolerance in output units");
        await inner("const input=d.querySelector('[data-engineer-transfer-tolerance]');input.value='0.00001';input.dispatchEvent(new d.defaultView.Event('change'));return 'manual continuity tolerance';");
      }
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      const tuned = JSON.parse(await pollValue("unapplied candidate", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => {
        try { const event = JSON.parse(text); return ["candidate", "failed"].includes(event.status); } catch { return false; }
      }, 10000));
      assert.strictEqual(tuned.status, "candidate", `Tune must use the independently declared controller law and selected model: ${tuned.reason}`);
      const settingsBeforeStageNavigation = JSON.parse(await inner("return JSON.stringify({reference:d.querySelector('[data-engineer-reference]')?.value,error:d.querySelector('[data-engineer-error]')?.value,band:d.querySelector('[data-engineer-band]')?.value,settling:d.querySelector('[data-engineer-settling]')?.value,duration:d.querySelector('[data-engineer-duration]')?.value,model:d.querySelector('[data-general-model-provenance]')?.textContent??''});"));
      assert.deepStrictEqual((await stageTabs(inner)).map(tab => tab.selected), ["false", "false", "true", "false"],
        "Tune completion must select the Verify navigation tab");
      await assertModelNavigationPreservesSettings(inner, settingsBeforeStageNavigation);
      if (process.env.TRUST_ENGINEER_SWITCH_LOOP === "1") {
        const storage = require("../../controlStudio/engineerDraftStorage");
        const originalStore = storage.storeEngineerDraft;
        let retained: any;
        const otherLoop = { ...structuredClone(loop), id: `${loop.id}-other`, name: "Other loop", revision: `${loop.revision}-other`,
          parameterBinding: { ...structuredClone(loop.parameterBinding), loop_id: `${loop.id}-other` } };
        storage.storeEngineerDraft = async (...args: any[]) => {
          if (args[0].id === otherLoop.id) retained = structuredClone(args[1]);
          return originalStore(...args);
        };
        try {
          await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
            scope: { projectId: otherLoop.projectId, runtimeId: otherLoop.runtimeId }, loopDefinition: otherLoop });
          session = await attachControlStudio(connection, port, "", otherLoop.id);
          await pollValue("reused panel handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
          await inner("d.querySelector('[data-engineer-options]').open=true;const input=d.querySelector('[data-engineer-reference]');input.value='2';input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'edit new loop';");
          await pollValue("new loop draft retained", async () => String(Boolean(retained)), value => value === "true", 10000);
          assert.strictEqual(retained.task.referenceChange, 2, "a reused panel must save its current session's task");
          assert.strictEqual(retained.draft, undefined, "the previous loop's candidate cannot enter the new loop's draft");
          assert.strictEqual(retained.selectedModel, undefined, "the new loop has not selected a model");
          return;
        } finally { storage.storeEngineerDraft = originalStore; }
      }
      await inner("d.querySelector('[data-engineer-stage-tab=verify]')?.click();return 'verify navigation';");
      const candidateLayout = await workflowLayout();
      assert.deepStrictEqual(candidateLayout.primary, ["Run verification"], "A tuned candidate must expose Verify as the single primary next action");
      await captureWorkflowStage("-candidate");
      if (process.env.TRUST_ENGINEER_CAPTURE_VIEWS === "1") await captureControlStudioViews(inner, captureWorkflowStage, "candidate", ["tune", "verify"]);
      if (operatingPoint) {
        const starting = await inner("const e=d.querySelector('[data-engineer-operating-point]');if(!e)return '';const details=e.closest('details');if(details)details.open=true;return e.textContent;");
        assert.match(starting, /MV 2 V.*PV 10 m.*target 11 m/,
          "Show the physical starting point and absolute target before Verify");
        assert.strictEqual(await inner("const r=d.querySelector('[data-engineer-operating-point]').getBoundingClientRect();return String(r.width>0&&r.height>0&&r.top>=0&&r.bottom<=d.defaultView.innerHeight);"), "true");
        const startingScreenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
        if (startingScreenshot) { await new Promise(resolve => setTimeout(resolve, 150)); captureRenderedWorkbench(startingScreenshot.replace(/\.png$/, "-starting.png")); }
        await inner("d.querySelector('[data-engineer-operating-point]').closest('details').open=false;return 'reviewed';");
      }
      if (manualPin) assert.strictEqual(await inner("return String(d.defaultView.__engineerLatest.design.candidate.proportionalGain);"), "20");
      if (manualPin) await inner("d.querySelector('[data-engineer-gain-mode]').closest('details').open=false;return 'inspect responses';");
      const candidateState = JSON.parse(await inner("return JSON.stringify({family:d.querySelector('[data-engineer-family]')?.value,applied:d.defaultView.__engineerLatest?.design?.candidate?.applied,applyDisabled:d.querySelector('[data-engineer-apply]')?.disabled});"));
      assert.strictEqual(candidateState.family, "P", "the compact workflow must keep the selected controller family visible");
      assert.strictEqual(candidateState.applied, false, "Tune must retain an unapplied candidate until Verify and Apply");
      assert.strictEqual(candidateState.applyDisabled, true, "Apply must remain unavailable before actual-ST Verify");
      const parametersBeforeVerify = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      const verifiedEvent = JSON.parse(await pollValue("actual ST verification", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});") , text => {
        try { const event = JSON.parse(text); return ["verified", "verification-failed", "failed"].includes(event.status); } catch { return false; }
      }, 40000));
      const verified = String(verifiedEvent.reason ?? verifiedEvent.status);
      if (runtimeFault) {
        assert.match(verified, /division by zero/i, "Verify must report the actual ST fault rather than a mathematical prediction");
        const outcome = JSON.parse(await inner("return JSON.stringify(d.defaultView.__engineerLatest.verificationAttempts[0]);"));
        assert.strictEqual(outcome.status, "failed");
        if (emptyRuntimeFault) {
          assert.strictEqual(outcome.run.samples.length, 0,
            "An isolated ST fault with an empty terminal response must retain no fabricated samples");
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
          const unavailableText = await pollValue("unavailable empty actual ST response", () => inner("return d.querySelector('[data-engineer-recorded-responses]')?.textContent ?? '';"),
            text => /unavailable|no samples/i.test(text), 10000);
          assert.match(unavailableText, /unavailable|no samples/i,
            "A terminal failed run with no samples must explain that recorded evidence is unavailable");
          assert.strictEqual(await inner("const e=d.querySelector('[data-engineer-recorded-responses]');const r=e?.getBoundingClientRect();return String(Boolean(e&&!e.hidden&&getComputedStyle(e).display!=='none'&&r&&r.width>0&&r.height>0));"), "true",
            "The no-samples recovery state must be reachable in the visible recorded-response workspace");
          assert.strictEqual(await inner("return String(d.querySelector('.plot-toolbar').getBoundingClientRect().height===0);"), "true", "An empty terminal response must hide chart tools that have no samples to inspect");
          assert.strictEqual(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot]').length);"), "0",
            "A terminal failed run with no samples must not render an interrupted or fabricated chart");
          const retry = JSON.parse(await inner("const e=d.querySelector('[data-engineer-retry-verify]');return JSON.stringify({present:Boolean(e),text:e?.textContent,disabled:e?.disabled,className:e?.className});"));
          assert.deepStrictEqual(retry, { present: true, text: "Retry Verify", disabled: false, className: "secondary" },
            "The visible empty-response state must offer exactly one enabled action to collect fresh verification evidence");
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-verify]').getBoundingClientRect().height===0);"), "true",
            "an empty response must not expose a duplicate Run verification action alongside Retry Verify");
          const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
          if (screenshot) {
            await inner("d.querySelector('[data-engineer-recorded-responses]').scrollIntoView({block:'center'});return 'show unavailable response';");
            await new Promise(resolve => setTimeout(resolve, 250)); captureRenderedWorkbench(screenshot);
          }
          await inspectFailedController(connection, port, inner, project);
          await inner("d.querySelector('[data-engineer-retry-verify]').click();return 'retry Verify';");
          const retryAttempt = JSON.parse(await pollValue("fresh verification after empty response", () => inner("return JSON.stringify(d.defaultView.__engineerLatest.verificationAttempts);"),
            value => { try { const attempts = JSON.parse(value); return attempts.length === 2 && attempts[1]?.run?.samples?.length > 0; } catch { return false; } }, 40000));
          assert.strictEqual(retryAttempt[0].run.samples.length, 0,
            "Retry must preserve the terminal record that contained no samples");
          assert.ok(retryAttempt[1].run.samples.length > 0,
            "Retry must create a fresh actual-ST verification attempt rather than inventing evidence for the old one");
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true",
            "A failed retry must not grant Apply authority");
          return;
        }
        assert.deepStrictEqual(outcome.run?.samples.map((sample: any) => sample.timeSeconds), [0, 0.02, 0.04],
          "Scheduled execution retains the initial point and two completed task updates before the fault");
        assert.strictEqual(outcome.run.samples[0].values["Main.MV"].value, 0);
        assert.strictEqual(outcome.run.samples[1].values["Main.MV"].value, 1);
        assert.ok(outcome.run.conditions.sampleTimesSeconds.length > outcome.run.samples.length);
        if (faultBadQuality) {
          assert.strictEqual(outcome.run.samples[1].values["Main.PV"].quality, "bad",
            "The actual terminal runtime response must retain the injected PV quality rather than promoting it to good");
        }
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
        const rawFaultValues = outcome.run.samples.map((sample: any) => sample.values);
        const interruptedText = await pollValue("interrupted actual ST response", () => inner("return d.querySelector('[data-engineer-recorded-responses]')?.textContent ?? '';"),
          text => /Interrupted actual recorded response/.test(text) && /division by zero/i.test(text), 10000);
        assert.match(interruptedText, /one interrupted ST run, not a baseline\/candidate comparison/i,
          "A runtime-fault prefix must not be presented as a completed comparison");
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot=interrupted]').length);"), "1",
          "The retained fault prefix must render as one interrupted actual-ST chart");
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot=baseline],[data-engineer-recorded-plot=candidate]').length);"), "0",
          "An interrupted run must not fabricate terminal, baseline, or candidate curves");
        assert.match(await inner("return d.querySelector('[data-engineer-recorded-plot=interrupted]')?.textContent ?? '';"), /gap|missing|not good quality/i,
          "The interrupted chart must explain that missing or bad-quality values are gaps");
        if (faultBadQuality) {
          assert.match(await inner("return d.querySelector('[data-engineer-recorded-plot=interrupted]')?.textContent ?? '';"), /Gap \(bad\)/,
            "The rendered interrupted chart table must show the retained bad-quality PV as a gap");
          const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
          if (screenshot) {
            await inner("d.querySelector('[data-engineer-recorded-plot=interrupted]').scrollIntoView({block:'center'});return 'show bad-quality chart';");
            await new Promise(resolve => setTimeout(resolve, 250)); captureRenderedWorkbench(screenshot);
          }
        }
        await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();return 'inspect fault';");
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-engineer-verification-history] tbody tr').length);"), "3");
        await inner("d.querySelector('[data-result-save]').click();return 'save failed prefix';");
        await pollValue("saved fault prefix", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => text.includes("Saved revision"), 10000);
        await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
        session = await attachControlStudio(connection, port);
        await pollValue("reopened fault evidence", () => inner("return d.querySelector('[data-engineer-verification-history]')?.textContent??'';"), text => text.includes(outcome.jobId), 10000);
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-engineer-verification-history] tbody tr').length);"), "3");
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-verification-history]')?.textContent.includes('bad'));"), "true",
          "Save and reopen must retain the actual bad-quality fault sample without interpolation");
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
        return;
      }
      assert.strictEqual(verifiedEvent.status, "verified");
      assert.deepStrictEqual((await stageTabs(inner)).map(tab => tab.selected), ["false", "false", "false", "true"],
        "Verify completion must select the Apply navigation tab");
      const recordedBeforeStageNavigation = await inner("return JSON.stringify({settings:{reference:d.querySelector('[data-engineer-reference]')?.value,error:d.querySelector('[data-engineer-error]')?.value,band:d.querySelector('[data-engineer-band]')?.value,settling:d.querySelector('[data-engineer-settling]')?.value,duration:d.querySelector('[data-engineer-duration]')?.value},evidence:d.defaultView.__engineerLatest?.verificationAttempts,plots:d.querySelectorAll('[data-engineer-recorded-plot]').length});");
      await assertVerifiedNavigationPreservesEvidence(inner, recordedBeforeStageNavigation);
      const verifiedLayout = await workflowLayout();
      assert.deepStrictEqual(verifiedLayout.primary, ["Review changes"], "A verified candidate must expose Apply as the single primary next action");
      await captureWorkflowStage("-verified");
      if (process.env.TRUST_ENGINEER_CAPTURE_VIEWS === "1") await captureControlStudioViews(inner, captureWorkflowStage, "verified", ["verify", "apply"]);
      assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Tune').dataset.status;"), "complete");
      assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Validate').dataset.status;"), "complete");
      if (operatingPoint) {
        const evidence = JSON.parse(await inner("return JSON.stringify(d.defaultView.__engineerLatest);"));
        const identified = entry === "find" ? JSON.parse(await inner("return JSON.stringify(d.defaultView.__identifiedModel);")) : undefined;
        const gain = identified ? identified.numerator[0] / identified.denominator[1] : 1;
        const expectedKp = ((1 + 2 * gain) / (0.2 - 0.02) - 1) / gain;
        assert.ok(Math.abs(evidence.candidate.run.parameters["Main.Kp"] - expectedKp) < 1e-9,
          "Gain must follow the independently fitted model and the operating command, not exact fixture coefficients");
        for (const result of [evidence.baseline, evidence.candidate]) {
          assert.strictEqual(result.run.conditions.initialState["Main.SP"], 11);
          assert.strictEqual(result.run.conditions.initialState["Main.MV"], 2);
          assert.strictEqual(result.run.samples[0].values["Main.MV"].value, 2);
          assert.strictEqual(result.run.samples[0].values["Main.PV"].value, 10);
        }
        const last = evidence.candidate.run.samples[evidence.candidate.run.samples.length - 1];
        assert.ok(Math.abs(last.values["Main.PV"].value - 11) <= 0.2);
        console.log("OPERATING POINT actual ST", JSON.stringify({ gain: evidence.candidate.run.parameters["Main.Kp"],
          initial: evidence.candidate.run.samples[0], final: last }));
      }
      if (manualPin) assert.strictEqual(await inner("return String(d.defaultView.__engineerLatest.candidate.run.parameters['Main.Kp']);"), "20", "Actual ST must execute the pinned gain");
      const verifiedState = JSON.parse(await inner("return JSON.stringify({family:d.querySelector('[data-engineer-family]')?.value,applyDisabled:d.querySelector('[data-engineer-apply]')?.disabled});"));
      assert.strictEqual(verifiedState.family, "P");
      assert.strictEqual(verifiedState.applyDisabled, false, "a verified candidate must expose Apply without implying it has already been applied");
      const parametersAfterVerify = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
      assert.deepStrictEqual(parametersAfterVerify.values, parametersBeforeVerify.values,
        "actual-ST Verify must not write controller parameters before the explicit Apply action");
      if (reopenStudy) {
        await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();return 'save-stage';");
        assert.strictEqual(await inner("const e=d.querySelector('[data-result-save]');e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return String(!e.disabled&&r.width>0&&r.height>0&&r.top>=0&&r.bottom<=d.defaultView.innerHeight);"), "true",
          "Manual workflow controls must remain reachable after inspecting recorded responses");
        await inner("d.querySelector('[data-result-save]').click();return 'save';");
        await pollValue("saved supplied study", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => text.includes("Saved revision"), 10000);
        await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
        session = await attachControlStudio(connection, port);
        await inner("d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__engineerLatest=e.detail);return 'listen restored';");
        await pollValue("restored model and study", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), text => text.includes("Restored"), 10000);
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-application-mode]').value;"), commissioning ? "auto" : "manual",
          "Reopening preserves the engineer's application-mode choice");
        assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Tune').dataset.status;"), "ready");
        assert.strictEqual(await inner("return [...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()==='Validate').dataset.status;"), "needs-data",
          "Historical verification must not become current qualification when a study reopens");
        const restoredTask = JSON.parse(await inner("return JSON.stringify(['reference','error','band','settling','duration'].map(n=>Number(d.querySelector('[data-engineer-'+n+']').value)));"));
        assert.deepStrictEqual(restoredTask, [1.2, 0.15, 0.025, 1.5, 3], "Reopening must restore the exact declared task controls");
        if (manualPin) {
          assert.strictEqual(await inner("return d.querySelector('[data-engineer-gain-mode]').value;"), "manual");
          assert.strictEqual(await inner("return d.querySelector('[data-engineer-gain-value]').value;"), "20", "Reopening must retain the exact gain pin");
          await inner("d.querySelector('[data-engineer-gain-mode]').value='automatic';d.querySelector('[data-engineer-tune]').click();return 'automatic with retained pin';");
          await pollValue("automatic candidate with retained pin", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => { try { return JSON.parse(text).status === "candidate"; } catch { return false; } }, 10000);
          assert.strictEqual(await inner("return d.querySelector('[data-engineer-gain-value]').value;"), "20");
          assert.notStrictEqual(await inner("return String(d.defaultView.__engineerLatest.design.candidate.proportionalGain);"), "20");
          await inner("d.querySelector('[data-engineer-gain-mode]').value='manual';return 'return to pinned gain';");
        }
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
        await inner("d.querySelector('[data-engineer-tune]').click();return 'retune';");
        await pollValue("retuned restored model", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => { try { return JSON.parse(text).status === "candidate"; } catch { return false; } }, 10000);
        await inner("d.querySelector('[data-engineer-verify]').click();return 'reverify';");
        await pollValue("current runtime reverified", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => { try { return JSON.parse(text).status === "verified"; } catch { return false; } }, 15000);
        if (manualPin) assert.strictEqual(await inner("return String(d.defaultView.__engineerLatest.candidate.run.parameters['Main.Kp']);"), "20", "Switching back to Manual must execute the retained pin");
        await new Promise(resolve => setTimeout(resolve, 600));
        assert.match(await inner("return d.querySelector('[data-control-studio-freshness]').textContent;"), /^Live$/,
          "Fresh explicit verification must resume live monitor readings instead of leaving the study labelled historical");
        assert.notStrictEqual(await inner("return String(d.querySelector('[data-control-studio]').dataset.controlStudioHistoricalResult);"), "true");
      }

      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'inspect verification';");
      assert.match(await inner("return d.querySelector('[data-engineer-evidence]').textContent;"), /ST-executed/);
      await inner("const monitor=d.querySelector('[data-control-studio-monitor]');d.scrollingElement?.scrollTo(0,monitor.offsetTop);monitor.scrollIntoView({block:'start'});return 'recorded-workspace';");
      await new Promise(resolve => setTimeout(resolve, 100));
      const recordedResponse = JSON.parse(await inner("const panel=d.querySelector('[data-engineer-recorded-responses]');const monitor=d.querySelector('[data-control-studio-monitor]');const viewport=d.defaultView.innerHeight;const canvases=[...panel?.querySelectorAll('canvas')??[]].map(canvas=>{const rect=canvas.getBoundingClientRect();return {width:rect.width,height:rect.height,top:rect.top,bottom:rect.bottom,viewport,visible:rect.top>=0&&rect.bottom<=viewport};});const values=d.querySelector('.values')?.getBoundingClientRect();return JSON.stringify({present:Boolean(panel),monitorPosition:getComputedStyle(monitor).position,workspace:panel?.dataset.engineerResponseWorkspace,text:panel?.textContent,plotCount:panel?.querySelectorAll('[data-engineer-recorded-plot]').length,canvasCount:panel?.querySelectorAll('canvas').length,canvases,liveValuesVisible:Boolean(values&&values.top>=0&&values.bottom<=viewport),height:panel?.querySelector('[data-engineer-recorded-plot] .engineering-plot')?.style.height,zoom:panel?.querySelector('[data-engineer-response-zoom]')?.textContent,reset:panel?.querySelector('[data-engineer-response-reset]')?.textContent,table:panel?.querySelector('[data-engineer-recorded-values]')?.textContent});"));
      console.log("Recorded layout boxes", await inner("return JSON.stringify(['.header','.modes','.values','[data-engineer-recorded-responses]','[data-engineer-recorded-plot]','[data-engineer-recorded-plot] .engineering-plot'].map(s=>{const e=d.querySelector(s);const r=e?.getBoundingClientRect();return {s,top:r?.top,height:r?.height,display:e?getComputedStyle(e).display:null};}));"));
      assert.strictEqual(await inner("const t=d.querySelector('.plot-toolbar');return String(t.scrollWidth<=t.clientWidth+1);"), "true", "The plot toolbar must reflow without clipping controls in a narrow editor");
      assert.strictEqual(await inner("return getComputedStyle(d.querySelector('.workspace')).display;"), "none", "Guided Verify must not expose the legacy Monitor page");
      assert.strictEqual(recordedResponse.present, true, "actual ST verification must present baseline and candidate recorded responses in the Engineer workspace");
      if (process.env.TRUST_ENGINEER_THEME_SWITCH === "1") {
        const background = await inner("return getComputedStyle(d.body).getPropertyValue('--vscode-editor-background');");
        await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Light Modern", vscode.ConfigurationTarget.Global);
        await pollValue("changed active theme", () => inner("return getComputedStyle(d.body).getPropertyValue('--vscode-editor-background');"), color => color !== background, 10000);
        await new Promise(resolve => setTimeout(resolve, 150));
        assert.strictEqual(await inner("const p=d.createElement('canvas');p.width=p.height=1;const c=p.getContext('2d');c.fillStyle=getComputedStyle(d.body).getPropertyValue('--vscode-editor-background');c.fillRect(0,0,1,1);const expected=[...c.getImageData(0,0,1,1).data];return String([...d.querySelectorAll('[data-engineer-recorded-responses] canvas')].every(canvas=>[...canvas.getContext('2d').getImageData(0,0,1,1).data].every((v,i)=>v===expected[i])));"), "true",
          "Changing theme must repaint both retained response charts without rerunning Verify");
      } else {
        assert.match(await inner("return d.querySelector('[data-engineer-recorded-limits]')?.textContent??'';"), /Main.MV.*-20.*20.*V/,
          "Recorded responses must explain their actual output bounds and units");
        const scheduled = JSON.parse(await inner("return JSON.stringify(d.defaultView.__engineerLatest.candidate.run.conditions.events);"));
        const eventsText = await inner("return d.querySelector('[data-engineer-recorded-events]')?.textContent??'';");
        if (!scheduled.length) assert.match(eventsText, /No scheduled events/, "Initial values must not be invented as scheduled events");
        else for (const event of scheduled) assert.ok(eventsText.includes(event.channel), "Recorded events must preserve their actual channel identity");
      }

      assert.strictEqual(await inner("const t=d.querySelector('.plot-toolbar');const r=t.getBoundingClientRect();return String(r.width>0&&r.height>0);"), "true",
        "Recorded verification must use the visible shared plot toolbar");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-plot-tab=recorded-responses]')));"), "true",
        "Retained verification responses need a shared plot tab for returning without rerunning Verify");
      await inner("d.defaultView.__recordedBefore=[...d.querySelectorAll('[data-engineer-recorded-responses] canvas')].map(c=>c.toDataURL());d.querySelector('[data-chart-action=zoom-in]').click();return 'zoom comparison';");
      await pollValue("linked recorded zoom", () => inner("return d.querySelector('[data-engineer-response-status]')?.textContent??'';"), text => /window/i.test(text), 10000);
      await new Promise(resolve => setTimeout(resolve, 150));
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-engineer-recorded-responses] canvas')].every((c,i)=>c.toDataURL()!==d.defaultView.__recordedBefore[i]));"), "true",
        "Shared zoom must redraw both actual response plots");
      await inner("d.querySelector('[data-chart-action=reset]').click();d.querySelector('[data-chart-action=inspect]').click();return 'inspect';");
      assert.match(await inner("return d.activeElement?.id??'';"), /engineer-recorded/, "Inspect must focus an actual recorded plot");
      await inner("d.activeElement.dispatchEvent(new d.defaultView.KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));return 'next sample';");
      assert.match(await inner("return d.querySelector('[data-engineer-response-status]').textContent;"), /Baseline.*Main.PV.*Candidate.*Main.PV/s,
        "Keyboard inspection must announce both actual sample values");
      const png = await exportRecordedComparisonThroughHost(inner, project);
      assert.strictEqual(png.subarray(1, 4).toString(), "PNG");
      const maximumCssWidth = Math.max(...recordedResponse.canvases.map((canvas: { width: number }) => canvas.width));
      const totalCssHeight = recordedResponse.canvases.reduce((sum: number, canvas: { height: number }) => sum + canvas.height, 0);
      assert.ok(png.readUInt32BE(16) >= maximumCssWidth * 2, "Export width must scale from the actual responsive plot width");
      assert.ok(png.readUInt32BE(20) > totalCssHeight * 2, "Export height must include both responsive plots and their labels");

      await inner("d.querySelector('[data-engineer-stage-tab=apply]').click();return 'live response';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-recorded-responses]').hidden);"), "true");
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'retained comparison';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-recorded-responses]').hidden);"), "false");

      assert.strictEqual(recordedResponse.workspace, "monitor", "recorded responses must use the existing shared plot workspace instead of the header");
      assert.strictEqual(recordedResponse.plotCount, 2, `baseline and candidate responses need separate recorded-response plots: ${JSON.stringify(recordedResponse)}`);
      assert.strictEqual(recordedResponse.canvasCount, 2, "each recorded response must render through the shared engineering chart surface");
      assert.ok(recordedResponse.canvases.every((canvas: { width: number; height: number }) => canvas.width > 0 && canvas.height >= 300), "recorded response canvases must have usable responsive dimensions");
      assert.strictEqual(await inner("const cards=[...d.querySelectorAll('[data-engineer-recorded-plot]')].map(e=>{const r=e.getBoundingClientRect();return {top:r.top,bottom:r.bottom,left:r.left,right:r.right,width:r.width,height:r.height};});return String(cards.length===2&&cards.every(card=>card.width>0&&card.height>=300)&&(cards[0].bottom<=cards[1].top||cards[1].bottom<=cards[0].top||cards[0].right<=cards[1].left||cards[1].right<=cards[0].left));"), "true",
        "responsive recorded plots must remain separate and non-overlapping");
      for (let index = 0; index < 2; index += 1) {
        await inner(`const card=d.querySelectorAll('[data-engineer-recorded-plot]')[${index}];card.scrollIntoView({block:'center'});return 'reachable';`);
        await new Promise(resolve => setTimeout(resolve, 80));
        assert.strictEqual(await inner(`const card=d.querySelectorAll('[data-engineer-recorded-plot]')[${index}];const r=card.getBoundingClientRect();return String(r.top>=0&&r.bottom<=d.defaultView.innerHeight&&r.width>0&&r.height>=300);`), "true",
          `recorded plot ${index} must be reachable and fully visible after scrolling`);
      }
      const themedOutputPixels = JSON.parse(await inner("const s=getComputedStyle(d.documentElement);const color=s.getPropertyValue('--vscode-charts-red').trim()||s.getPropertyValue('--vscode-charts-orange').trim()||s.getPropertyValue('--vscode-editor-foreground').trim();const probe=d.createElement('canvas');probe.width=probe.height=1;const c=probe.getContext('2d');c.fillStyle=color;c.fillRect(0,0,1,1);const rgb=c.getImageData(0,0,1,1).data;return JSON.stringify([...d.querySelectorAll('[data-engineer-recorded-responses] canvas')].map(canvas=>{const a=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;let n=0;for(let i=0;i<a.length;i+=4)if(a[i]===rgb[0]&&a[i+1]===rgb[1]&&a[i+2]===rgb[2]&&a[i+3]>240)n++;return n;}));"));
      assert.ok(themedOutputPixels.every((count: number) => count > 30), "Recorded MV curves must use the shared theme output color for visible contrast");
      assert.strictEqual(recordedResponse.liveValuesVisible, true, "the shared live-values strip must remain visible with recorded verification plots");
      assert.strictEqual(recordedResponse.height, "420px", "recorded response needs a full-height visible engineering chart");
      assert.match(recordedResponse.text, /Baseline.*ST-executed.*Candidate/s, "recorded-response labels must state the compared roles and evidence class");
      assert.match(recordedResponse.text, /gap|missing/i, "recorded-response plots must explain missing or non-good-quality samples as gaps");
      assert.match(recordedResponse.table, /Time/);
      for (const [role, symbol, unit] of [["SP", "Main.SP", "m"], ["PV", "Main.PV", "m"], ["MV", "Main.MV", "V"]]) {
        assert.ok(recordedResponse.table.includes(`${role} · ${symbol} (${unit})`),
          `recorded values must preserve the ${role} channel identity and unit independently of observation order`);
      }
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("const monitor=d.querySelector('[data-control-studio-monitor]');d.scrollingElement?.scrollTo(0,monitor.offsetTop);monitor.scrollIntoView({block:'start'});return 'recorded-workspace';");
        await new Promise(resolve => setTimeout(resolve, 250));
        await vscode.commands.executeCommand("notifications.clearAll"); captureRenderedWorkbench(screenshot);
      }
      await inner("d.querySelector('[data-engineer-stage-tab=apply]').click();d.querySelector('[data-engineer-apply]').click();return 'review-apply';");
      await pollValue("reviewed application", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), s => s === "true", 10000);
      if (process.env.TRUST_ENGINEER_CAPTURE_VIEWS === "1") await captureWorkflowStage("-gallery-apply-review");
      if (manualTolerance) assert.match(await inner("return d.querySelector('[data-engineer-status]').textContent;"), /Maximum Auto-entry change: 0.00001 V/);
      if (commissioning) await inner("d.defaultView.__recordedCanvasText=[];const proto=d.defaultView.CanvasRenderingContext2D.prototype;const original=proto.fillText;proto.fillText=function(text,...args){d.defaultView.__recordedCanvasText.push(String(text));return original.call(this,text,...args);};return 'observe real chart text painting';");
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm';");
      if (recordingAckLoss) {
        await pollValue("unconfirmed recording Start reported", () => inner("return d.defaultView.__commissionResult?.status??'';"), value => value === "failed", 15000);
        let report = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
        assert.ok(lostRecordingId);
        if (blockRecordingStop) {
          assert.strictEqual(report.recoveryConfirmed, false);
          assert.strictEqual(report.recording, undefined);
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true");
          assert.deepStrictEqual(recordingStops, [lostRecordingId, lostRecordingId]);
          blockRecordingStop = false;
          await inner("d.querySelector('[data-engineer-cancel]').click();return 'retry owned recording Stop';");
          await pollValue("recording Stop recovery", () => inner("return String(d.defaultView.__commissionResult?.recoveryConfirmed);"), value => value === "true", 10000);
          report = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
          assert.deepStrictEqual(recordingStops, [lostRecordingId, lostRecordingId, lostRecordingId]);
        } else assert.deepStrictEqual(recordingStops, [lostRecordingId], "An unacknowledged Start must Stop its existing native recording ID");
        assert.strictEqual(startsAfterLostAcknowledgement, 0, "Never replay Start after losing its acknowledgement");
        assert.ok(report.recording?.rawSamples.length > 0, "Retain the actual samples returned by Stop after a lost Start reply");
        assert.strictEqual(report.recoveryConfirmed, true);
        assert.strictEqual(report.parameters.values["Main.Kp"], 1);
        assert.strictEqual(autoRequests, 0);
        assert.match(report.recording.evidence.reason, /acknowledg/i);
        console.log("Lost recording Start retained", report.recording.rawSamples.length, report.reason);
        return;
      }
      if (process.env.TRUST_ENGINEER_CLOSE_FIRST_CAMPAIGN === "1") {
        await pollValue("first actual Auto target before close", () => inner("return String(d.querySelector('[data-control-studio-controller-mode]').textContent==='Auto'&&Number(d.querySelector('[data-control-studio-pv]').textContent.split(' ')[0])>10.1);"), value => value === "true", 10000);
        await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
        const persistence = require("../../controlStudio/panelPersistence");
        const originalRestore = persistence.restoreWorkflowResultHistory;
        let receipt: any;
        persistence.restoreWorkflowResultHistory = (...args: any[]) => {
          const value = originalRestore(...args); receipt = value.history[value.history.length - 1]; return value;
        };
        try {
          await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
            scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
          session = await attachControlStudio(connection, port);
          await pollValue("first closed campaign editor ready", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 15000);
        } finally { persistence.restoreWorkflowResultHistory = originalRestore; }
        assert.ok(receipt?.commissioning, "Closing the first active campaign must retain its real terminal result");
        assert.strictEqual(receipt.commissioning.recoveryConfirmed, false);
        assert.strictEqual(receipt.commissioning.gainRecovery.origin.values["Main.Kp"], 1);
        assert.strictEqual(receipt.commissioning.gainRecovery.candidate["Main.Kp"], 36.5);
        assert.ok(receipt.commissioning.recording.rawSamples.length > 1 && receipt.commissioning.recording.rawSamples.length < 200);
        const decode = require("../../controlStudio/workflowResult").decodeWorkflowResult;
        assert.ok(decode(receipt));
        for (const corrupt of [
          (value: any) => { value.commissioning.gainRecovery.origin.values["Main.Kp"] = "1"; },
          (value: any) => { value.commissioning.gainRecovery.origin.program_revision = "sha256:other-program"; },
          (value: any) => { delete value.commissioning.gainRecovery.candidate["Main.Kp"]; },
          (value: any) => { value.commissioning.gainRecovery.origin.extra = true; },
        ]) {
          const invalid = structuredClone(receipt); corrupt(invalid); assert.strictEqual(decode(invalid), undefined);
        }
        const parameterConnection = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
        const currentReview = await parameterConnection.client.reviewRollback();
        const wrongOrigin = structuredClone(receipt.commissioning);
        wrongOrigin.gainRecovery.origin.values["Main.Kp"] = 2;
        const beforeReview = transferTolerances.length;
        await assert.rejects(() => require("../../controlStudio/proportionalCommissioning").ProportionalCommissioning.reviewRecovery(
          loop, receipt.engineerStudy.selectedModel, receipt.engineerStudy.task, parameterConnection.client, currentReview, wrongOrigin), /changed/);
        assert.strictEqual(transferTolerances.length, beforeReview, "A forged original gain cannot cause a Manual transfer");
        blockRecovery = false;
        await inner("d.querySelector('[data-engineer-rollback]').click();return 'review first closed campaign recovery';");
        await pollValue("first campaign fresh recovery review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden&&!d.querySelector('[data-engineer-confirm]').disabled);"), value => value === "true", 10000);
        await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm first campaign recovery';");
        await pollValue("first campaign Manual recovery", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Previous values restored"), 10000);
        const observed = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
        assert.strictEqual(observed.values["Main.Kp"], 1);
        assert.strictEqual(autoRequests, 2);
        console.log("First interrupted campaign retained", receipt.commissioning.recording.rawSamples.length, receipt.commissioning.reason);
        return;
      }
      if (exhaustRecordingRetries) {
        await pollValue("bounded recording contention failure", () => inner("return d.defaultView.__commissionResult?.status??'';"), value => value === "failed", 15000);
        const report = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
        assert.strictEqual(rejectedRecordingStarts, 5, "Coherence retry must stop after five rejected starts");
        assert.strictEqual(autoRequests, 0);
        assert.strictEqual(report.recording, undefined, "Rejected Start cannot manufacture a recording");
        assert.strictEqual(report.recoveryConfirmed, true);
        assert.strictEqual(report.parameters.values["Main.Kp"], 1);
        console.log("Recording retry exhaustion retains failure", rejectedRecordingStarts, report.reason);
        return;
      }
      if (gainAckLoss) {
        await pollValue("commissioning reconciled after lost gain acknowledgement", () => inner("return d.defaultView.__commissionResult?.status??'';"), value => value === "passed" || value === "failed", 15000);
        const report = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
        assert.strictEqual(lostGainAcknowledgement, true);
        assert.strictEqual(report.status, "passed", report.reason);
        assert.deepStrictEqual(report.parameters.values, verifiedEvent.candidate.run.parameters,
          "Status reconciliation must confirm the exact verified gains after a lost acknowledgement");
        assert.strictEqual(gainApplyIds.length, 1, "Acknowledgement recovery must not resubmit the parameter write");
        assert.ok(typeof gainApplyIds[0] === "string" && gainApplyIds[0].length > 0);
        assert.strictEqual(report.parameters.transaction_id, gainApplyIds[0]);
        assert.strictEqual(report.recoveryConfirmed, true);
        assert.ok(autoRequests > 0, "Auto may proceed after observed parameter adoption is confirmed");
        assert.ok(report.recording.rawSamples.length > 0);
        console.log("Lost gain acknowledgement reconciled without replay", report.parameters.values);
        return;
      }
      if (process.env.TRUST_ENGINEER_CANCEL_COMMISSIONING === "1") {
        cleanupCommissioning = async () => {
          await inner("d.querySelector('[data-engineer-cancel]').click();return 'cancel active commissioning';");
          await pollValue("terminal commissioning recovery", () => inner("return d.defaultView.__commissionResult?.status??'';"), value => value === "cancelled" || value === "failed", 15000);
        };
        await pollValue("actual Auto target response before cancellation", () => inner("return String(d.querySelector('[data-control-studio-controller-mode]').textContent==='Auto'&&Number(d.querySelector('[data-control-studio-pv]')?.textContent?.split(' ')[0])>0.1);"), value => value === "true", 10000);
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-confirm]').disabled);"), "true", "An active commissioning operation locks its reviewed controls");
        await cleanupCommissioning();
        let cancelled = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
        if (recoveryFault) {
          assert.strictEqual(cancelled.recoveryConfirmed, false);
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true");
          await pollValue("first Cancel finished", async () => String(cancelResults.length), value => Number(value) >= 1, 15000);
          blockRecovery = false;
          const count = cancelResults.length;
          await inner("d.querySelector('[data-engineer-cancel]').click();return 'retry owned recovery';");
          await pollValue("recovery retry finished", async () => String(cancelResults.length), value => Number(value) > count, 15000);
          assert.strictEqual(cancelResults[cancelResults.length - 1]?.recoveryConfirmed, true,
            "Cancel must retry the failed recovery, not return its previous unresolved result");
          await pollValue("recovery confirmation rendered", () => inner("return String(d.defaultView.__commissionResult?.recoveryConfirmed);"), value => value === "true", 5000);
          cancelled = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
          assert.strictEqual(autoRequests, 2, "Recovery retry cannot start another Auto entry or target step");
        }
        cleanupCommissioning = undefined;
        assert.strictEqual(cancelled.status, "cancelled");
        assert.strictEqual(cancelled.recoveryConfirmed, true);
        assert.strictEqual(cancelled.parameters.values["Main.Kp"], 1);
        assert.ok(cancelled.recording.rawSamples.length > 1);
        const recovery = cancelled.transfers[cancelled.transfers.length - 1].transaction;
        assert.strictEqual(recovery.observed_values["Main.Manual"], true);
        assert.ok(Math.abs(recovery.observed_values["Main.MV"] - recovery.initial_values["Main.MV"]) <= 0.00004);
        await pollValue("Manual after cancelled commissioning", () => inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), value => value === "Manual", 5000);
        console.log("Cancelled Auto recovered", cancelled.recording.rawSamples.length, recovery.observed_values);
        return;
      }
      const application = await pollValue("observed application", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        s => /Auto commissioning passed|observed after ST|failed/i.test(s), 15000);
      assert.ok(application.includes(commissioning ? "Auto commissioning passed" : "observed after ST"), application);
      if (commissioning) {
        assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), "Auto");
        assert.match(await inner("return d.querySelector('[data-engineer-evidence]').textContent;"), /Transfer verification.*Pass/s);
        const report = JSON.parse(await inner("return JSON.stringify(d.defaultView.__commissionResult);"));
        if (recordingContention) {
          assert.strictEqual(displacedInspection, true);
          assert.ok(rejectedRecordingStarts >= 1 && rejectedRecordingStarts < 5,
            "The real runtime must reject a displaced inspection before Studio obtains a fresh one");
        }
        assert.strictEqual(report.status, "passed");
        assert.strictEqual(report.recording.evidence.completeness, "complete");
        assert.ok(report.recording.rawSamples.length >= 200);
        assert.strictEqual(report.transfers[0].transaction.confirmation, "output-image");
        assert.strictEqual(report.transfers[0].transaction.program_revision, report.recording.evidence.programRevision);
        assert.ok(Math.abs(report.transfers[0].transaction.observed_values["Main.MV"] - (operatingPoint ? 2 : 0)) <= 0.00004);
        assert.ok(report.maximumError <= (operatingPoint ? 0.2 : 0.1));
        assert.strictEqual(await inner("return String(d.defaultView.__recordedCanvasText.some(text=>text.includes('Main.SP →')));"), "false",
          "Reference-change labels must not collide with chart axes; retain them in the readable event note");
        assert.match(await inner("return d.querySelector('[data-engineer-recorded-plot=commissioning] [data-engineer-recorded-events]').textContent;"), /Main.SP →/);
        console.log("Observed Auto commissioning", JSON.stringify({ samples: report.recording.rawSamples.length,
          maximumError: report.maximumError, firstAuto: report.transfers[0].transaction.observed_values,
          programRevision: report.recording.evidence.programRevision }));
        assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-recorded-plot=commissioning] canvas')));"), "true",
          "Commissioning must plot the complete native recording, including the short output peak");
        assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot=commissioning] tbody tr').length);")), report.recording.rawSamples.length);
        await inner("const choice=d.querySelector('[data-engineer-recorded-source]');choice.value='verification';choice.dispatchEvent(new d.defaultView.Event('change'));return 'verification record';");
        assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot]').length);")), 2);
        await inner("const choice=d.querySelector('[data-engineer-recorded-source]');choice.value='commissioning';choice.dispatchEvent(new d.defaultView.Event('change'));return 'commissioning record';");
        assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot=commissioning] tbody tr').length);")), report.recording.rawSamples.length);
        assert.match(application, /error -?\d+(?:\.\d{1,6})? m/, "display concise engineering precision while retaining exact recorded values");
        assert.strictEqual(await inner("const identity=d.querySelector('[data-engineer-recorded-identity]');return String(identity?.tagName==='DETAILS'&&!identity.open);"), "true",
          "Keep full provenance accessible without crowding the response plot");
        if (screenshot) { await new Promise(resolve => setTimeout(resolve, 200)); captureRenderedWorkbench(screenshot.replace(/\.png$/, "-auto.png")); }
        if (process.env.TRUST_ENGINEER_SAVE_OWNER_RACE === "1") {
          const snapshots = require("../../controlStudio/workflowSnapshots");
          const results = require("../../controlStudio/workflowResult");
          const originalLoad = snapshots.loadWorkflowSimulationPlant;
          const originalSave = results.saveWorkflowResultRevision;
          let release!: () => void;
          const blocked = new Promise<void>(resolve => { release = resolve; });
          let waiting = false; let readFinished = false; let saved: any;
          const saveFailure = process.env.TRUST_ENGINEER_SAVE_OWNER_FAILURE === "1";
          snapshots.loadWorkflowSimulationPlant = async (...args: any[]) => {
            if (args[1].id === loop.id) { waiting = true; await blocked; }
            const loaded = await originalLoad(...args);
            readFinished = true;
            if (saveFailure) throw new Error("controlled old Save metadata failure");
            return loaded;
          };
          results.saveWorkflowResultRevision = async (...args: any[]) => {
            const history = await originalSave(...args); saved = args[2]; return history;
          };
          try {
            await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();d.querySelector('[data-result-save]').click();return 'start owned Save';");
            await pollValue("Save waiting for plant metadata", async () => String(waiting), value => value === "true", 10000);
            await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
            const otherLoop = { ...structuredClone(loop), id: `${loop.id}-other`, name: "Other loop", revision: `${loop.revision}-other`,
              parameterBinding: { ...structuredClone(loop.parameterBinding), loop_id: `${loop.id}-other` } };
            await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
              scope: { projectId: otherLoop.projectId, runtimeId: otherLoop.runtimeId }, loopDefinition: otherLoop });
            session = await attachControlStudio(connection, port);
            await pollValue("other editor ready", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
            release();
            if (saveFailure) {
              await pollValue("original Save read rejected", async () => String(readFinished), value => value === "true", 10000);
              await new Promise(resolve => setTimeout(resolve, 250));
              assert.ok(!(await inner("return d.body.textContent;")).includes("controlled old Save metadata failure"),
                "A failed old Save cannot publish its error into the new editor");
              assert.strictEqual(saved, undefined);
              console.log("Old Save failure stayed outside new editor");
              return;
            }
            await pollValue("original Save persisted", async () => String(Boolean(saved)), value => value === "true", 10000);
            assert.strictEqual(saved.loop.id, loop.id);
            assert.strictEqual(saved.engineerStudy?.selectedModel?.revision, report.modelRevision,
              "Pending Save must retain its original model and study after a different editor opens");
            assert.strictEqual(saved.commissioning?.recording.rawSamples.length, report.recording.rawSamples.length);
            await new Promise(resolve => setTimeout(resolve, 250));
            assert.ok(!(await inner("return d.querySelector('[data-result-state]').textContent;")).includes("Saved revision"),
              "An old Save must not replace the new editor's current result or history");
            console.log("Save retained original owner", saved.loop.id, saved.engineerStudy.selectedModel.revision);
          } finally { release(); snapshots.loadWorkflowSimulationPlant = originalLoad; results.saveWorkflowResultRevision = originalSave; }
          return;
        }
        if (process.env.TRUST_ENGINEER_SAVE_COMMISSIONING === "1") {
          await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();d.querySelector('[data-result-save]').click();return 'save commissioning';");
          await pollValue("saved commissioning", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => value.includes("Saved revision"), 10000);
          await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
          const persistence = require("../../controlStudio/panelPersistence");
          const originalRestore = persistence.restoreWorkflowResultHistory;
          const originalCheckpoint = persistence.restoreCommissioningCheckpoint;
          let checkpoint: any;
          persistence.restoreCommissioningCheckpoint = (...args: unknown[]) => {
            checkpoint = originalCheckpoint(...args); return checkpoint;
          };
          let restored: any;
          persistence.restoreWorkflowResultHistory = (...args: unknown[]) => {
            const result = originalRestore(...args); restored = result.history[result.history.length - 1]; return result;
          };
          try {
            await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
              scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
          } finally { persistence.restoreWorkflowResultHistory = originalRestore; persistence.restoreCommissioningCheckpoint = originalCheckpoint; }
          console.log("Restored commissioning identity", JSON.stringify({
            receipt: { status: restored?.commissioning?.status, model: restored?.commissioning?.modelRevision,
              parameters: restored?.commissioning?.parameters, transfers: restored?.commissioning?.transfers.map((transfer: any) => ({ id: transfer.transaction?.id, program: transfer.transaction?.program_revision, binding: transfer.transaction?.binding_revision, confirmation: transfer.transaction?.confirmation })) },
            checkpoint: checkpoint && { stage: checkpoint.stage, model: checkpoint.model.revision, origin: checkpoint.origin,
              candidate: checkpoint.candidate, parameterOperation: checkpoint.parameterOperation, transfer: checkpoint.transfer },
            matches: checkpoint && require("../../controlStudio/commissioningCheckpoint").completedCommissioningMatchesCheckpoint(restored?.commissioning, checkpoint),
          }));
          session = await attachControlStudio(connection, port);
          await pollValue("restored commissioning study", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.stage;"), value => value === "restored", 10000);
          assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
          assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot=commissioning] tbody tr').length);")), report.recording.rawSamples.length,
            "Reopening must retain every native commissioning sample and its plot");
          if (manualTolerance) {
            assert.strictEqual(await inner("return d.querySelector('[data-engineer-transfer-tolerance]').value;"), "0.00001");
            assert.strictEqual(restored.engineerStudy.task.transferTolerance, 0.00001);
            const continuity = restored.engineerStudy.initialization.evaluation.specification.find((item: any) => item.id === "transfer-continuity");
            assert.strictEqual(continuity.minimum, (operatingPoint ? 2 : 0) - 0.00001);
            assert.strictEqual(continuity.maximum, (operatingPoint ? 2 : 0) + 0.00001);
            assert.strictEqual(transferTolerances[0], 0.00001, "Native Auto transfer must use the same manual tolerance as actual-ST Verify");
            await inner("d.querySelector('[data-engineer-transfer-tolerance]').closest('details').open=true;return 'show restored tolerance';");
            if (screenshot) { await new Promise(resolve => setTimeout(resolve, 250)); captureRenderedWorkbench(screenshot.replace(/\.png$/, "-restored-tolerance.png")); }
          }
          const decode = require("../../controlStudio/workflowResult").decodeWorkflowResult;
          assert.ok(decode(restored));
          for (const corrupt of [
            (value: any) => { value.commissioning.maximumError = 0; },
            (value: any) => { value.commissioning.recording.rawSamples.splice(10, 1); },
            (value: any) => { value.commissioning.transfers[0].transaction.program_revision = "sha256:other-program"; },
          ]) {
            const value = JSON.parse(JSON.stringify(restored)); corrupt(value);
            assert.strictEqual(decode(value), undefined, "Changed samples, evaluations or program identities cannot retain a saved Pass");
          }
          if (process.env.TRUST_ENGINEER_REOPEN_ROLLBACK === "1") {
            assert.strictEqual(autoRequests, 2, "Reopening must not reapply Auto or the target");
            await inner("d.querySelector('[data-engineer-rollback]').click();return 'review reopened rollback';");
            await pollValue("reopened rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
            const rollbackDescription = await inner("return d.querySelector('[data-engineer-status]').textContent;");
            const reopenRecoveryFault = process.env.TRUST_ENGINEER_REOPEN_RECOVERY_FAULT === "1";
            if (reopenRecoveryFault) blockRecovery = true;
            await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm reopened rollback';");
            let rollback = await pollValue("reopened rollback outcome", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /restored|failed/i.test(value), 10000);
            if (reopenRecoveryFault) {
              assert.match(rollback, /failed/);
              assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true",
                "A failed reopened rollback must retain its owner and lock dependent controls");
              if (process.env.TRUST_ENGINEER_CLOSE_RECOVERY === "1") {
                await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
                const recoveryPersistence = require("../../controlStudio/panelPersistence");
                const restoreRecovery = recoveryPersistence.restoreWorkflowResultHistory;
                let receipt: any;
                recoveryPersistence.restoreWorkflowResultHistory = (...args: any[]) => {
                  const value = restoreRecovery(...args); receipt = value.history[value.history.length - 1]; return value;
                };
                try {
                  await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
                    scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
                  session = await attachControlStudio(connection, port);
                  await pollValue("closed recovery history reopened", async () => String(Boolean(receipt)), value => value === "true", 15000);
                } finally { recoveryPersistence.restoreWorkflowResultHistory = restoreRecovery; }
                assert.strictEqual(receipt.commissioning.recoveryConfirmed, false,
                  "Closing must save the unresolved recovery instead of restoring an older successful outcome");
                assert.strictEqual(receipt.commissioning.recording.rawSamples.length, report.recording.rawSamples.length);
                await pollValue("unresolved recovery shown", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /recovery/i.test(value), 10000);
                assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true");
                assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-rollback]').disabled);"), "false");
                if (screenshot) { await new Promise(resolve => setTimeout(resolve, 250)); captureRenderedWorkbench(screenshot.replace(/\.png$/, "-unresolved-restored.png")); }
                blockRecovery = false;
                await inner("d.querySelector('[data-engineer-rollback]').click();return 'fresh closed recovery review';");
                await pollValue("fresh closed recovery confirmation", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden&&!d.querySelector('[data-engineer-confirm]').disabled);"), value => value === "true", 10000);
                await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm closed recovery';");
                rollback = await pollValue("closed recovery restored", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Previous values restored/.test(value), 10000);
                console.log("Closed unresolved recovery retained", receipt.resultRevision, receipt.commissioning.reason);
              } else {
                blockRecovery = false;
                await inner("d.querySelector('[data-engineer-cancel]').click();return 'retry reopened recovery';");
                rollback = await pollValue("reopened recovery retry", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Recovery confirmed/.test(value), 10000);
              }
            } else assert.match(rollback, /Previous values restored/);
            assert.match(rollbackDescription, /Manual/i,
              "A fresh rollback review must explain the Manual transfer");
            assert.match(rollbackDescription, /(original|previous).*(parameter|gain)/i,
              "A fresh rollback review must explain restoration of the original parameter set");
            const observed = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
            assert.strictEqual(observed.values["Main.Kp"], 1);
            assert.strictEqual(autoRequests, 2);
            assert.ok(transferTolerances.length >= 3, "Rollback must perform the native Manual tracking transfer");
            if (process.env.TRUST_ENGINEER_LATER_MANUAL === "1") {
              await inner("const mode=d.querySelector('[data-engineer-application-mode]');mode.value='manual';mode.dispatchEvent(new d.defaultView.Event('change'));d.querySelector('[data-engineer-tune]').click();return 'later manual candidate';");
              await pollValue("later candidate", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), value => { try { return JSON.parse(value).status === "candidate"; } catch { return false; } }, 10000);
              await inner("d.querySelector('[data-engineer-verify]').click();return 'verify later manual candidate';");
              await pollValue("later verification", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), value => { try { return JSON.parse(value).status === "verified"; } catch { return false; } }, 15000);
              await inner("d.querySelector('[data-engineer-apply]').click();return 'review later Manual staging';");
              await pollValue("later staging review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
              await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm later Manual staging';");
              await pollValue("later gain staged", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Candidate values staged"), 10000);
              const staged = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
              assert.ok(staged.values["Main.Kp"] > 1);
              await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Validate').click();d.querySelector('[data-result-save]').click();return 'save newer Manual operation';");
              await pollValue("saved newer Manual operation", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => value.includes("Saved revision 2"), 10000);
              await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
              await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
                scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
              session = await attachControlStudio(connection, port);
              await pollValue("restored newer Manual study", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /restored/i.test(value), 10000);
              assert.strictEqual(await inner("return d.querySelector('[data-engineer-application-mode]').value;"), "manual");
              assert.strictEqual(Number(await inner("return String(d.querySelectorAll('[data-engineer-recorded-plot=commissioning] tbody tr').length);")), report.recording.rawSamples.length,
                "The older commissioning recording must survive a newer Manual operation");
              await inner("d.querySelector('[data-engineer-rollback]').click();return 'review newer native gain history';");
              const laterReview = await pollValue("later rollback review", () => inner("const text=d.querySelector('[data-engineer-status]').textContent;return !d.querySelector('[data-engineer-confirm]').hidden||text.includes('failed')?text:'';"), value => /review|failed|already/i.test(value), 10000);
              assert.ok(!laterReview.includes("failed"), `Old commissioning history must not block newer Manual rollback: ${laterReview}`);
              await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm newer Manual rollback';");
              await pollValue("later gains restored", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Previous values restored"), 10000);
              const final = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
              assert.strictEqual(final.values["Main.Kp"], 1);
              assert.strictEqual(autoRequests, 2, "Later Manual staging and rollback cannot re-enter Auto");
              console.log("Newer Manual gain history restored", staged.values, final.values);
            }
            if (process.env.TRUST_ENGINEER_REPEATED_ROLLBACK === "1") {
              await inner("d.querySelector('[data-engineer-rollback]').click();return 'review rollback again';");
              const repeat = await pollValue("already recovered review", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /review|failed|already/i.test(value), 10000);
              assert.match(repeat, /already.*Manual/i, "A completed recovery cannot reuse its old owner or reverse the restored gain");
              assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-confirm]').hidden);"), "true");
              await inner("d.querySelector('[data-engineer-confirm]').click();return 'stale confirmation cannot write';");
              await pollValue("no confirmable review", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Review the exact application/.test(value), 10000);
              const unchanged = await runtime.request("control.parameters.status", { binding: loop.parameterBinding });
              assert.strictEqual(unchanged.values["Main.Kp"], 1);
            }
            console.log("Fresh reviewed historical rollback", observed.values);
          }
          console.log("Saved commissioning remains historical", report.recording.evidence.programRevision, report.recording.rawSamples.length);
          return;
        }
      }
      assert.strictEqual(await inner("return String(!d.querySelector('[data-engineer-view-results]').hidden);"), "true");
      await inner("d.querySelector('[data-engineer-view-results]').click();return 'view completed results';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-results]').open);"), "true");
      await captureWorkflowStage("-applied-details");
      await inner("d.querySelector('[data-engineer-results]').open=false;return 'close completed results';");
      await inner("d.querySelector('[data-engineer-rollback]').click();return 'review-rollback';");
      await pollValue("reviewed rollback", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), s => s === "true", 10000);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm-rollback';");
      const rollback = await pollValue("observed rollback", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), s => /Previous values restored|failed/i.test(s), 10000);
      assert.match(rollback, /Previous values restored/);
      if (commissioning) await pollValue("fresh observed Manual mode after rollback", () => inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), value => value === "Manual", 5000);
      assert.match(await inner("return d.querySelector('[data-engineer-evidence]').textContent;"), /Main.Kp.*1/);
      if (manualPin) {
        await inner("d.querySelector('[data-engineer-find]').click();return 'identify a new model';");
        await pollValue("new independently identified model", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("fitted") || value.startsWith("Identification failed"), 30000);
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-gain-mode]').value;"), "automatic",
          "A changed identified model invalidates the old manual gain pin just like a supplied model change");
      }
      const clearedAfterCancellation = await inner("d.defaultView.dispatchEvent(new d.defaultView.CustomEvent('trust-control-studio-engineer',{detail:{status:'cancelled',reason:'Find was cancelled; retained trials remain in the identification history.'}}));return String(!d.querySelector('[data-engineer-recorded-responses]').hasChildNodes());");
      assert.strictEqual(clearedAfterCancellation, "true", "a cancelled engineer operation must clear stale verification plots instead of presenting them as current");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled && d.querySelector('[data-engineer-verify]').disabled);"), "true", "Cancelled candidates must disable Verify and Apply");
      await runtimeLifecycleService.stopRuntime();
      await pollValue("stopped live acquisition", () => inner("return d.querySelector('[data-control-studio-quality]').textContent;"), text => text.includes("Unavailable"), 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), "Unavailable",
        "A stopped runtime must not retain its last Manual/Auto mode as current");
      await pollValue("contextual simulator start", () => inner("return String(!d.querySelector('[data-control-studio-start-runtime]').hidden);"), value => value === "true", 10000);
      if (screenshot) captureRenderedWorkbench(screenshot.replace(/\.png$/, "-stopped-simulator.png"));
      await inner("d.querySelector('[data-control-studio-start-runtime]').click();d.querySelector('[data-control-studio-start-runtime]').click();return 'start simulator from Control Studio';");
      await pollValue("restarted simulator from Control Studio", () => inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), value => value === "Manual", 15000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-control-studio-start-runtime]').hidden);"), "true");
    } finally {
      blockRecovery = false;
      try { await cleanupCommissioning?.(); } catch (error) { console.error("Commissioning cleanup", error); }
      transport.sendRuntimeControlRequest = originalSend;
      search.connectRuntimeSearch = originalSearch;
      connections.bindRuntimeConnection = originalBind;
      commissioningPrototype.cancel = originalCancel;
      connection.close(); await runtimeLifecycleService.stopRuntime();
    }
  });
});
