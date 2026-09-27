import * as assert from "assert";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { setControlAuthToken } from "../../runtimeAuth";
import { setSelectedRuntimeId } from "../../selectedRuntime";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

const phase = process.env.TRUST_CONTROL_STUDIO_MANAGED_RECORDING_PHASE;

suite("Control Studio managed recording recovery", () => {
  test("retains a recording owner across two Extension Hosts", async function () {
    if (phase !== "start" && phase !== "recover") this.skip();
    this.timeout(60_000);
    const endpoint = requiredEnvironment("TRUST_CONTROL_STUDIO_MANAGED_ENDPOINT");
    const token = requiredEnvironment("TRUST_CONTROL_STUDIO_MANAGED_TOKEN");
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await setControlAuthToken(endpoint, token);
    await setSelectedRuntimeId("owner-recovery");
    const connected = await runtimeLifecycleService.connectRemote(endpoint, "owner-recovery");
    assert.strictEqual(connected.ok, true, JSON.stringify(connected));
    const loop = {
      schemaVersion: 2, id: "managed-owner-recovery", name: "Managed owner recovery", revision: "sha256:managed-owner-recovery-v1",
      projectId: "project:managed-owner-recovery", runtimeId: "owner-recovery",
      controller: { instance: "Main", type: "EngineerProportional", design: "project", revision: "sha256:managed-owner-controller", action: "direct", taskPeriodMs: 20 },
      signals: {
        setpoint: { symbol: "Main.SP", address: "%ML0", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
        processVariable: { symbol: "Main.PV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
        manipulatedVariable: { symbol: "Main.MV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "V", range: { minimum: -20, maximum: 20 } },
      }, limits: { outputMinimum: -20, outputMaximum: 20 }, observedSampleTiming: { nominalPeriodMs: 20 },
    };
    assert.strictEqual(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "accepted",
      "the managed-runtime journey must use a Control Studio-valid numeric loop");
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("managed recording handlers", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"),
        value => value === "true", 10_000);
      await pollValue("managed recording controls", () => inner("return String(Boolean(d.querySelector('[data-control-studio-record-start]')));"), value => value === "true", 10_000);
      if (phase === "start") {
        await inner("d.querySelector('[data-control-studio-record-start]').click();return 'start';");
        await pollValue("managed recording acknowledged", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => value.includes("Recording at runtime"), 15_000);
      } else {
        const state = JSON.parse(await pollValue("restored managed owner", () => inner("const start=d.querySelector('[data-control-studio-record-start]');const stop=d.querySelector('[data-control-studio-record-stop]');return JSON.stringify({text:d.querySelector('[data-control-studio-recording-evidence]').textContent,start:start.disabled,stop:stop.disabled});"), value => JSON.parse(value).text.includes("Recording recovery unconfirmed"), 15_000));
        assert.strictEqual(state.start, true); assert.strictEqual(state.stop, false);
        await inner("d.querySelector('[data-control-studio-record-stop]').click();return 'stop';");
        await pollValue("managed recovered record", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => value.includes("Complete recording"), 15_000);
      }
    } finally { connection.close(); }
  });

  test("recovers a Test-owned recording across two Extension Hosts without a replacement Test", async function () {
    if (phase !== "test-start" && phase !== "test-recover") this.skip();
    this.timeout(60_000);
    const endpoint = requiredEnvironment("TRUST_CONTROL_STUDIO_MANAGED_ENDPOINT");
    const token = requiredEnvironment("TRUST_CONTROL_STUDIO_MANAGED_TOKEN");
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await setControlAuthToken(endpoint, token);
    await setSelectedRuntimeId("owner-recovery");
    const connected = await runtimeLifecycleService.connectRemote(endpoint, "owner-recovery");
    assert.strictEqual(connected.ok, true, JSON.stringify(connected));
    const loop = {
      schemaVersion: 2, id: "managed-test-recovery", name: "Managed Test recovery", revision: "sha256:managed-test-recovery-v1",
      projectId: "project:managed-test-recovery", runtimeId: "owner-recovery",
      controller: { instance: "Main", type: "EngineerProportional", design: "project", revision: "sha256:managed-test-controller", action: "direct", taskPeriodMs: 20,
        modeSignal: "%MX8.0", modeWhenTrue: "Manual" },
      signals: {
        setpoint: { symbol: "Main.SP", address: "%ML0", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
        processVariable: { symbol: "Main.PV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "m", range: { minimum: -20, maximum: 20 } },
        manipulatedVariable: { symbol: "Main.MV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "V", range: { minimum: -20, maximum: 20 } },
      }, limits: { outputMinimum: -20, outputMaximum: 20 }, observedSampleTiming: { nominalPeriodMs: 20 },
    };
    assert.strictEqual(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "accepted");
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const stage = (name: string) => inner(`var b=Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(x=>x.textContent.trim()===${JSON.stringify(name)});b.click();return b.textContent;`);
      await pollValue("managed Test controls", () => inner("return String(Boolean(d.querySelector('[data-test-review]')));"), value => value === "true", 10_000);
      if (phase === "test-start") {
        await stage("Test");
        await pollValue("fresh managed Test sample", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-quality]').textContent;");
        }, value => !/Unavailable|Waiting/i.test(value), 15_000);
        await inner("d.querySelector('#test-kind').value='setpoint_step';d.querySelector('#test-amplitude').value='1';d.querySelector('#test-duration').value='10';var physical=d.querySelector('[data-physical-topology-reviewed]');if(physical){physical.checked=true;physical.dispatchEvent(new Event('change',{bubbles:true}));}d.querySelector('[data-test-review]').click();return 'review';");
        await pollValue("managed Test review", () => inner("return d.querySelector('[data-test-state]').textContent;"),
          value => value.includes("Simulation test ready") || value.includes("Physical test ready"), 15_000);
        await inner("d.querySelector('[data-test-approve]').click();return 'approve';");
        await pollValue("managed Test running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Test running"), 15_000);
        await sleep(5_000);
        return;
      }
      const terminal = await pollValue("recovered Test recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
        value => value.includes("Complete recording"), 20_000);
      assert.match(terminal, /runtime captured/);
      await stage("Identify");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-run]').disabled);"), "false",
        "the recovered Test buffer must unlock Identify without a new Test");
      await inner("d.querySelector('[data-identify-run]').click();return 'qualify';");
      const qualification = await pollValue("recovered Test qualification", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
        value => !/Qualifying the retained recording/.test(value), 15_000);
      assert.doesNotMatch(qualification, /Incomplete recording/i);
    } finally { connection.close(); }
  });
});
