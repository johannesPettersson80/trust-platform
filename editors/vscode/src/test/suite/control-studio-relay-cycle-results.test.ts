import * as assert from "assert";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { recordingFingerprint } from "../../controlStudio/independentModelValidation";
import { selectedModelRelayCycle } from "../../controlStudio/relayCycleEvidence";
import { renderThreePageStudioHtml } from "../../controlStudio/threePageTemplate";
import { renderRelayCycle } from "../../controlStudio/webview/threePageRelayCycle";
import { channel, pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** A connected feedback-relay recording: ±1 V relay, PV cycling ±0.2 m with a 0.8 s period. */
function relayRecord(): any {
  const pv = [-0.2, -0.1, 0, 0.1, 0.2, 0.1, 0, -0.1];
  const samples = Array.from({ length: 41 }, (_, k) => ({ timeSeconds: k / 10, sequence: k,
    values: { "Main.PV": { value: pv[k % 8], quality: "good" }, "Main.MV": { value: k % 8 < 4 ? 1 : -1, quality: "good" } } }));
  return { projectRevision: "project",
    request: { execution: "connected-runtime", experiment: { kind: "relay", relayFeedback: { lowOutput: -1, highOutput: 1 } } },
    run: { id: "relay-recording", evidence: "ST-executed", parameters: {}, conditions: {},
      channels: [{ id: "Main.PV", unit: "m" }, { id: "Main.MV", unit: "V" }], samples },
    evaluation: { status: "Pass" } };
}

function relayModel(record: any): any {
  return { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0, revision: "relay-fit",
    source: "fitted", validation: "validated", evidence: [recordingFingerprint(record.run)],
    inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] } };
}

function near(actual: unknown, expected: number, label: string): void {
  assert.ok(typeof actual === "number" && Math.abs(actual - expected) <= 1e-9 * Math.max(1, Math.abs(expected)),
    `${label}: expected ${expected}, got ${String(actual)}`);
}

const ultimateGain = 4 * 1 / (Math.PI * 0.2);
const proportionalGain = 0.45 * ultimateGain;
const integralTimeSeconds = 0.8 / 1.2;

suite("Control Studio relay cycle results", () => {
  test("measures the relay cycle in the recording the selected model was fitted from", () => {
    const loop = pidLoop();
    const record = relayRecord();
    const model = relayModel(record);
    const relay = selectedModelRelayCycle(loop, model, [{ runs: [record] }]);
    assert.strictEqual(relay?.status, "qualified", "A repeatable feedback-relay recording yields its cycle quantities");
    if (relay?.status !== "qualified") return;
    assert.strictEqual(relay.cycles, 4, "Only complete low-to-high cycles count");
    near(relay.outputAmplitude, 1, "relay amplitude d");
    near(relay.responseAmplitude, 0.2, "cycle amplitude a");
    near(relay.periodSeconds, 0.8, "cycle period Pu");
    near(relay.ultimateGain, ultimateGain, "Ku = 4d/(πa)");
    near(relay.proportionalGain, proportionalGain, "Kp = 0.45 Ku");
    near(relay.integralTimeSeconds, integralTimeSeconds, "Ti = Pu/1.2");
    near(relay.integralGain, proportionalGain / integralTimeSeconds, "Ki = Kp/Ti");
    assert.strictEqual(selectedModelRelayCycle(loop, { ...model, evidence: ["sha256:other"] }, [{ runs: [record] }]), undefined,
      "A model fitted from another recording has no relay cycle");
  });

  test("tunes the relay-rule PI candidate from the measured cycle", async () => {
    const loop = pidLoop();
    const record = relayRecord();
    const model = relayModel(record);
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    (session as any).state.campaigns.push({ id: "relay-campaign", status: "completed", runs: [record] });
    const scope = { projectId: loop.projectId, editorSessionId: "relay-rule", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune", projectId: scope.projectId,
        editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2,
          durationSeconds: 4, applicationMode: "auto", tuningPreferences: { method: "relay" } } },
      scope, async value => { posts.push(value); });
    } finally { session.dispose(); }
    const last = posts[posts.length - 1];
    assert.strictEqual(last?.status, "candidate", `The relay rule produces a candidate: ${last?.reason}`);
    assert.strictEqual(last.family, "PI", "The relay rule gives a PI controller");
    near(last.values["Main.Kp"], proportionalGain, "Kp = 0.45 Ku");
    near(last.values["Main.Ki"], proportionalGain / integralTimeSeconds, "Ki = Kp/(Pu/1.2)");
    assert.match(String(last.reason), /Relay rule/, "The result names the rule it used");
  });

  test("keeps the relay results when the selected relay-identified model is presented again", () => {
    const loop = pidLoop();
    const record = relayRecord();
    const model = relayModel(record);
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    (session as any).state.campaigns.push({ id: "relay-campaign", status: "completed", runs: [record] });
    try {
      const presented = session.modelChanged() as any;
      assert.strictEqual(presented.status, "model-ready");
      assert.strictEqual(presented.relayCycle?.status, "qualified", "The same relay-identified model keeps its relay results");
    } finally { session.dispose(); }
  });

  test("shows the relay cycle under the identified model and offers the relay rule in Tune", () => {
    const loop = pidLoop();
    const dom = new JSDOM(renderThreePageStudioHtml({ loopDefinition: loop } as any,
      value => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")));
    try {
      const root = dom.window.document.body;
      const relayCycle = { status: "qualified", cycles: 4, outputAmplitude: 1, responseAmplitude: 0.2, periodSeconds: 0.8,
        ultimateGain, proportionalGain, integralTimeSeconds, integralGain: proportionalGain / integralTimeSeconds };
      renderRelayCycle(root, loop, { status: "model-ready", model: {}, relayCycle });
      const line = root.querySelector("[data-studio-relay-cycle]");
      const option = root.querySelector('[data-engineer-tuning-method] option[value="relay"]');
      assert.ok(line && !line.hidden, "Identify shows the measured relay cycle");
      assert.strictEqual(line.textContent, "Relay cycle (4 cycles): d = 1 V, a = 0.2 m, Pu = 0.8 s, Ku = 4d/(πa) = 6.37 V/m");
      assert.ok(option && !option.hidden && !option.disabled, "Tune offers the relay rule once a relay cycle is measured");
      renderRelayCycle(root, loop, { status: "running", reason: "Validating…" });
      assert.ok(!line.hidden, "Progress messages without a model keep the relay results");
      renderRelayCycle(root, loop, { status: "model-ready", model: {}, relayCycle: { status: "not_qualified", reason: "Relay period spread exceeds 10% of its mean." } });
      assert.strictEqual(line.textContent, "Relay cycles not usable: Relay period spread exceeds 10% of its mean.");
      assert.ok(option.hidden && option.disabled, "Unusable relay cycles give no relay rule");
      renderRelayCycle(root, loop, { status: "model-ready", model: {} });
      assert.ok(line.hidden, "A model without relay cycles shows no relay results");
      assert.ok(option.hidden && option.disabled, "The relay rule is unavailable without relay cycles");
    } finally { dom.window.close(); }
  });

  test("keeps the relay results and the chosen relay rule while Validate updates the model", () => {
    const loop = pidLoop();
    const dom = new JSDOM(renderThreePageStudioHtml({ loopDefinition: loop } as any,
      value => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")));
    try {
      const root = dom.window.document.body;
      renderRelayCycle(root, loop, { status: "model-ready", model: {}, relayCycle: { status: "qualified", cycles: 4, outputAmplitude: 1,
        responseAmplitude: 0.2, periodSeconds: 0.8, ultimateGain, proportionalGain, integralTimeSeconds, integralGain: proportionalGain / integralTimeSeconds } });
      const method = root.querySelector("[data-engineer-tuning-method]");
      method.value = "relay";
      let changes = 0;
      method.addEventListener("change", () => changes++);
      renderRelayCycle(root, loop, { status: "running", model: {}, activity: "controller-check", reason: "Model checked. Validating the tuned controller…" });
      assert.ok(!root.querySelector("[data-studio-relay-cycle]").hidden, "The model check validates the same identification; its relay results stay");
      assert.strictEqual(method.value, "relay", "Validate must not change the chosen tuning method");
      assert.strictEqual(changes, 0, "Validate must not edit the tuning settings it is validating");
    } finally { dom.window.close(); }
  });

  test("writes relay units the way the rest of Control Studio does", () => {
    const loop = pidLoop();
    loop.signals.processVariable.unit = "degC"; loop.signals.manipulatedVariable.unit = "%";
    const dom = new JSDOM(renderThreePageStudioHtml({ loopDefinition: loop } as any,
      value => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;")));
    try {
      const root = dom.window.document.body;
      renderRelayCycle(root, loop, { status: "model-ready", model: {}, relayCycle: { status: "qualified", cycles: 3, outputAmplitude: 5,
        responseAmplitude: 0.52, periodSeconds: 11.6, ultimateGain: 20 / (Math.PI * 0.52), proportionalGain: 0, integralTimeSeconds: 0, integralGain: 0 } });
      assert.strictEqual(root.querySelector("[data-studio-relay-cycle]")?.textContent,
        "Relay cycle (3 cycles): d = 5 %, a = 0.52 °C, Pu = 11.6 s, Ku = 4d/(πa) = 12.24 %/°C");
    } finally { dom.window.close(); }
  });
});
