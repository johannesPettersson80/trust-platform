import * as assert from "assert";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { channel, pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function robustness(): any {
  try { return require("../../controlStudio/candidateRobustness"); } catch { return {}; }
}

type Payload = Record<string, any>;

/** A plant with a lag and a delay, so the loop has a phase crossover and every margin is finite. */
const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0.2, revision: "margins",
  source: "provided", validation: "validated", evidence: ["record"], inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
const task = { family: "PI", applicationMode: "manual", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
  settlingTimeSeconds: 4, durationSeconds: 8 };

function tune(session: EngineerWorkflowSession, loop: any, payload: Payload = task): Promise<Payload[]> {
  const scope = { projectId: loop.projectId, editorSessionId: "robustness", loopId: loop.id, loopRevision: loop.revision };
  const posts: Payload[] = [];
  return session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune", projectId: scope.projectId,
    editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
  async value => { posts.push(value); }).then(() => posts);
}

suite("Control Studio candidate robustness margins", () => {
  test("derives the delay margin from the phase margin and crossover and keeps missing margins unbounded", () => {
    const api = robustness();
    assert.strictEqual(typeof api.candidateRobustness, "function", "Tune needs one owner for a candidate's margins");
    const evidence = { gainMarginDb: 12, phaseMarginDegrees: 60, gainCrossover: { frequencyRadPerSecond: 0.5, magnitudeDb: 0, phaseDegrees: -120 },
      sensitivity: [{ frequencyRadPerSecond: 0.1, sensitivityMagnitudeDb: -10 }, { frequencyRadPerSecond: 1, sensitivityMagnitudeDb: 3 }] };
    const margins = api.candidateRobustness(evidence);
    assert.strictEqual(margins.gainMarginDb, 12);
    assert.strictEqual(margins.phaseMarginDegrees, 60);
    assert.ok(Math.abs(margins.delayMarginSeconds - (60 * Math.PI / 180) / 0.5) < 1e-12, String(margins.delayMarginSeconds));
    assert.ok(Math.abs(margins.peakSensitivity - 10 ** (3 / 20)) < 1e-12);
    const small = api.candidateRobustness({ ...evidence, gainMarginDb: "unbounded", phaseMarginDegrees: "unbounded", gainCrossover: undefined });
    assert.deepStrictEqual([small.gainMarginDb, small.phaseMarginDegrees, small.delayMarginSeconds], ["unbounded", "unbounded", "unbounded"],
      "a loop whose gain stays below one has no crossover, so no phase or delay limit");
  });

  test("Ms is the peak over every frequency, where a strictly proper loop's sensitivity tends to one", () => {
    const { engineerPrediction } = require("../../controlStudio/engineerPrediction");
    const loop = pidLoop();
    // A PI on 1/(8s + 1) with a 90° phase margin: |S| rises towards one only beyond the plotted crossover region.
    const lag: any = { ...model, delaySeconds: 0, denominator: [8, 1], revision: "lag" };
    const smooth = robustness().candidateRobustness(engineerPrediction(lag, loop, { proportionalGain: 3.75, integralGain: 0.469, derivativeGain: 0 }, task));
    assert.ok(smooth.peakSensitivity >= 1 - 1e-9, `sup |S| of a strictly proper loop is at least one: ${smooth.peakSensitivity}`);
    // A static plant under P control has L(∞) = K·Kp, so |S| = 1/(1 + K·Kp) everywhere: no clamp to one.
    const staticPlant: any = { ...model, delaySeconds: 0, denominator: [1], revision: "static" };
    const flat = robustness().candidateRobustness(engineerPrediction(staticPlant, loop, { proportionalGain: 1, integralGain: 0, derivativeGain: 0 }, task));
    assert.ok(Math.abs(flat.peakSensitivity - 0.5) < 1e-9, String(flat.peakSensitivity));
  });

  test("a Control.PID candidate carries its margins on the selected model", async () => {
    const loop = pidLoop();
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    try {
      const candidate = (await tune(session, loop)).find(post => post.status === "candidate");
      assert.ok(candidate, "Tune produces a candidate");
      const margins = candidate.robustness;
      assert.ok(margins && typeof margins.gainMarginDb === "number" && typeof margins.phaseMarginDegrees === "number"
        && typeof margins.delayMarginSeconds === "number" && margins.peakSensitivity >= 1, JSON.stringify(margins));
      const crossover = candidate.plots.gainCrossover.frequencyRadPerSecond;
      assert.ok(Math.abs(margins.delayMarginSeconds - margins.phaseMarginDegrees * Math.PI / 180 / crossover) < 1e-9);
    } finally { session.dispose(); }
  });

  test("the Tune page keeps the host's margins when it shows the candidate", () => {
    const dom = new JSDOM(`<main><select data-studio-controller></select><select data-engineer-tuning-method><option value="automatic" selected>Automatic</option></select>`
      + `<div data-studio-tuning-result hidden></div></main>`);
    const root = dom.window.document.querySelector("main");
    const previous = new Map<string, unknown>();
    for (const key of ["document", "window", "Option", "HTMLElement"]) { previous.set(key, (globalThis as any)[key]); (globalThis as any)[key] = key === "window" ? dom.window : dom.window[key]; }
    try {
      const { ThreePageTuning } = require("../../controlStudio/webview/threePageTuning");
      const tuning = new ThreePageTuning(root, pidLoop(), () => undefined, () => "PI");
      tuning.receive({ status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 },
        robustness: { gainMarginDb: 12.04, phaseMarginDegrees: 58.3, delayMarginSeconds: 1.27, peakSensitivity: 1.43 } });
      assert.strictEqual(root.querySelector("[data-studio-robustness]")?.textContent,
        "Gain margin 12.04 dB · phase margin 58.3° · delay margin 1.27 s · peak sensitivity Ms 1.43");
    } finally { for (const [key, value] of previous) (globalThis as any)[key] = value; dom.window.close(); }
  });

  test("a failed Tune leaves no earlier candidate on the Tune page", () => {
    const dom = new JSDOM(`<main><select data-studio-controller></select><select data-engineer-tuning-method><option value="automatic" selected>Automatic</option></select>`
      + `<div data-studio-tuning-result hidden></div></main>`);
    const root = dom.window.document.querySelector("main");
    const previous = new Map<string, unknown>();
    for (const key of ["document", "window", "Option", "HTMLElement"]) { previous.set(key, (globalThis as any)[key]); (globalThis as any)[key] = key === "window" ? dom.window : dom.window[key]; }
    try {
      const { ThreePageTuning } = require("../../controlStudio/webview/threePageTuning");
      const tuning = new ThreePageTuning(root, pidLoop(), () => undefined, () => "PI");
      tuning.receive({ status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 } });
      assert.strictEqual(root.querySelector("[data-studio-tuning-result]").hidden, false, "the candidate is shown");
      tuning.receive({ status: "failed", flowStage: "Tune", reason: "Tuning failed: PI cannot meet the requirements on the model." });
      const area = root.querySelector("[data-studio-tuning-result]");
      assert.ok(area.hidden && !area.textContent.includes("Proportional gain"),
        "the host dropped the candidate, so its gains no longer stand beside the refusal as if they were its result");
    } finally { for (const [key, value] of previous) (globalThis as any)[key] = value; dom.window.close(); }
  });

  test("the Tune page shows the margins beside the candidate", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const root = dom.window.document.querySelector("main");
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const loop = pidLoop();
      renderTunedController(root, loop, { status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 },
        robustness: { gainMarginDb: 12.04, phaseMarginDegrees: 58.3, delayMarginSeconds: 1.27, peakSensitivity: 1.43 } });
      const line = root.querySelector("[data-studio-robustness]");
      assert.ok(line, "the candidate shows its margins");
      assert.strictEqual(line.textContent, "Gain margin 12.04 dB · phase margin 58.3° · delay margin 1.27 s · peak sensitivity Ms 1.43");
      renderTunedController(root, loop, { status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 },
        robustness: { gainMarginDb: "unbounded", phaseMarginDegrees: 58.3, delayMarginSeconds: 1.27, peakSensitivity: 1.43 } });
      assert.match(root.querySelector("[data-studio-robustness]").textContent, /^Gain margin unbounded · /);
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });
});
