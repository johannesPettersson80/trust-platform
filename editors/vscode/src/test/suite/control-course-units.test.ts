import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { parseControlLessonContract } from "../../examples/controlLesson";

suite("Control course engineering units", () => {
  test("keeps Lesson 14 identification metadata and procedure aligned with the qualified source", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const lessonDir = path.join(root, "examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment");
    const lesson = parseControlLessonContract(JSON.parse(fs.readFileSync(path.join(lessonDir, "lesson.json"), "utf8")), "control-lesson-21") as any;
    const loop = JSON.parse(fs.readFileSync(path.join(lessonDir, "control-loop.json"), "utf8"));
    const main = fs.readFileSync(path.join(lessonDir, "src/Main.st"), "utf8");
    const readme = fs.readFileSync(path.join(lessonDir, "README.md"), "utf8");
    const procedure = `${lesson.learning.change}\n${readme.match(/## Experiment in Control Studio[\s\S]*?## Evidence and decision/)?.[0] ?? ""}`;
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.plant.seed, 20260829);
    assert.deepStrictEqual(lesson.plant.initialState, { temperatureC: 35, delayedInputPct: 75 });
    assert.strictEqual(lesson.experiment.durationSeconds, 35.1);
    const pv = lesson.signals.find((signal: { name: string }) => signal.name === "Measured temperature");
    assert.deepStrictEqual([pv?.minimum, pv?.maximum], [33, 39]);
    const pvValidity = lesson.plant.validityRange.find((range: { signal: string }) => range.signal === "Measured temperature");
    assert.deepStrictEqual([pvValidity?.minimum, pvValidity?.maximum], [33, 39]);
    const pvSafety = lesson.safety.hardLimits.find((range: { signal: string }) => range.signal === "Measured temperature");
    assert.deepStrictEqual([pvSafety?.minimum, pvSafety?.maximum], [33, 39]);
    assert.strictEqual(loop.topology, "open-loop");
    assert.strictEqual(loop.controller.type, "Main");
    assert.strictEqual(loop.observedSampleTiming.nominalPeriodMs, 100);
    assert.strictEqual(loop.experiment.durationSeconds, 35.1);
    assert.match(main, /initial_delayed_input|75\.0/);
    const diagnosticSymbols = [
      "P1.ProposedCommandPct", "P1.PredictedFinalC", "P1.BaselineRangeC", "P1.BaselineSlopeCPerS",
      "P1.BaselineQualified", "P1.Active", "P1.StartRejected", "P1.MeasurementQuality", "P1.SafetyAbort",
    ];
    for (const symbol of diagnosticSymbols) {
      assert.ok(loop.signals.additional.some((signal: { symbol: string }) => signal.symbol === symbol),
        `loop metadata must expose actual diagnostic ${symbol}`);
      assert.ok(lesson.signals.some((signal: { binding: string }) => signal.binding &&
        loop.signals.additional.find((candidate: { symbol: string; address: string }) => candidate.symbol === symbol)?.address === signal.binding),
        `lesson metadata must expose the binding for ${symbol}`);
    }
    assert.match(procedure, /Control Studio[\s\S]*Monitor[\s\S]*Test[\s\S]*Validate[\s\S]*Save result[\s\S]*Saved results[\s\S]*Open/i,
      "the published procedure must describe the complete retained public workflow");
    assert.doesNotMatch(procedure, /integral state|proportional correction|automatic controller|extra load D\s*=/i,
      "the public procedure must use the existing project owner without nonexistent controller controls");
  });
  test("keeps Lesson 13 metadata and teaching procedure aligned with its open-loop source", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const lessonDir = path.join(root, "examples/control/01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance");
    const lesson = parseControlLessonContract(JSON.parse(fs.readFileSync(path.join(lessonDir, "lesson.json"), "utf8")),
      "control-water-heater-energy-balance") as any;
    const loop = JSON.parse(fs.readFileSync(path.join(lessonDir, "control-loop.json"), "utf8"));
    const simulation = fs.readFileSync(path.join(lessonDir, "simulation.toml"), "utf8");
    const source = fs.readFileSync(path.join(lessonDir, "src/Main.st"), "utf8");
    const readme = fs.readFileSync(path.join(lessonDir, "README.md"), "utf8");
    assert.strictEqual(lesson.sequence, 13);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.plant.seed, 20260829);
    assert.deepStrictEqual(lesson.plant.initialState, { temperatureC: 35, delayedInputPct: 75 });
    const pvRange = lesson.safety.hardLimits.find((range: { signal: string }) => range.signal === "Measured temperature");
    const mvRange = lesson.safety.hardLimits.find((range: { signal: string }) => range.signal === "Applied heating command");
    assert.deepStrictEqual([pvRange?.minimum, pvRange?.maximum], [32, 39]);
    assert.deepStrictEqual([mvRange?.minimum, mvRange?.maximum], [0, 100]);
    assert.deepStrictEqual(lesson.signals.find((signal: { name: string }) => signal.name === "Safety abort")?.binding, "%MX40.1");
    assert.strictEqual(lesson.experiment.durationSeconds, 30.1);
    assert.strictEqual(lesson.experiment.seed, 20260829);
    assert.strictEqual(loop.topology, "open-loop");
    assert.strictEqual(loop.controller.instance, "P1");
    assert.strictEqual(loop.controller.type, "Main");
    assert.strictEqual(loop.signals.setpoint, undefined, "the project owns an open-loop input/output experiment");
    assert.ok(loop.signals.additional.some((signal: { symbol: string }) => signal.symbol === "P1.SafetyAbort"),
      "the existing SafetyAbort diagnostic must be visible in the same Signals view");
    assert.strictEqual(loop.observedSampleTiming.nominalPeriodMs, 100);
    assert.strictEqual(loop.experiment.durationSeconds, 30.1);
    assert.match(loop.operatingPoint, /35.*75/);
    assert.match(simulation, /seed\s*=\s*20260829/);
    assert.match(simulation, /initial_delayed_input\s*=\s*75(?:\.0)?/);
    assert.match(simulation, /initial_applied\s*=\s*75(?:\.0)?/);
    assert.match(source, /ModelTemperatureC\s*:\s*REAL\s*:=\s*35\.0/);
    assert.match(source, /CommandHistory\s*:\s*ARRAY\[0\.\.9\]\s+OF\s+REAL/);
    assert.doesNotMatch(readme.match(/## Implementation[\s\S]*?## Experiment in Control Studio/)?.[0] ?? "",
      /DeliveredPowerKW|TemperatureRateCPerS|DisturbanceKW/,
      "the learner snippet must use shipped Main.st names and no unbound disturbance input");
    assert.doesNotMatch(lesson.learning.change as string, /\bSP\b|\bD\s*=\s*0\b|disturbance/i,
      "the public workflow procedure must not invent a setpoint or writable disturbance input");
  });
  test("accepts the shaft's actual engineering units without a lesson-specific allowlist", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const lesson = JSON.parse(fs.readFileSync(path.join(root,
      "examples/control/07-classical-stability-and-frequency-design/45-root-locus/lesson.json"), "utf8"));
    assert.doesNotThrow(() => parseControlLessonContract(lesson, "control-root-locus"));
    const mv = lesson.signals.find((signal: {role: string}) => signal.role === "MV");
    assert.ok(mv);
    mv.unit = "kg·m²/s²";
    assert.doesNotThrow(() => parseControlLessonContract(lesson, "control-root-locus"));
    for (const unit of ["N\u0000m", "N\nm", "x".repeat(81)]) {
      mv.unit = unit;
      assert.throws(() => parseControlLessonContract(lesson, "control-root-locus"), /unit/i);
    }
  });
});
