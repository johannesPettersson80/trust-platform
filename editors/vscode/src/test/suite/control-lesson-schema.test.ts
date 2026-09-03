import * as assert from "assert";
import { spawnSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { parseControlLesson } from "../../examples/model";
import { canonicalLessonReadmeHeadings } from "./controlLessonAssertions";

function readStructuredTextProject(lessonRoot: string): string {
  const sourceRoot = path.join(lessonRoot, "src");
  return fs.readdirSync(sourceRoot)
    .filter((name) => name.endsWith(".st"))
    .sort()
    .map((name) => fs.readFileSync(path.join(sourceRoot, name), "utf8"))
    .join("\n");
}

suite("Control lesson schema", () => {
  test("accepts and retains the complete version 3 engineering contract", () => {
    const lesson = completeLesson();

    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("requires the ordered theory-before-practice contract for pedagogical lessons", () => {
    const lesson = pedagogicalLesson();
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);

    for (const field of [
      "context", "reason", "theory", "workedPrediction", "practicalPreparation",
      "baseline", "change", "comparison", "validation", "explanation", "transfer",
    ]) {
      const incomplete = pedagogicalLesson() as any;
      delete incomplete.learning[field];
      assert.throws(
        () => parseControlLesson(incomplete, "control-test-heater"),
        new RegExp(`control lesson 'control-test-heater'.*learning\\.${field}`),
        `a pedagogical lesson must not omit ${field}`
      );
    }
  });

  test("presents pedagogical lesson stages in the specified causal order", () => {
    const api = require("../../examples/controlLesson") as {
      controlLessonPresentation?: (lesson: unknown) => Array<{ title: string; paragraphs: string[] }>;
    };
    const sections = api.controlLessonPresentation?.(pedagogicalLesson());

    assert.deepStrictEqual(sections?.map((section) => section.title), [
      "Context", "Reason", "Theory", "Worked prediction", "Practical preparation",
      "Baseline", "Change", "Comparison", "Validation", "Explanation", "Transfer",
    ]);
    assert.ok(sections?.every((section) => section.paragraphs.length > 0));
    assert.strictEqual(sections?.[2].paragraphs[0],
      "Identify SP, PV, MV, disturbance, feedback direction, limits, and safe state.");
  });

  test("derives the visible lesson position from the loaded curriculum size", () => {
    const api = require("../../examples/model") as {
      controlLessonPositionLabel?: (sequence: number, total: number) => string;
    };

    assert.strictEqual(api.controlLessonPositionLabel?.(1, 72), "Lesson 1 of 72");
    assert.strictEqual(api.controlLessonPositionLabel?.(72, 72), "Lesson 72 of 72");
  });

  test("rejects a version 3 lesson without an executable plant contract", () => {
    const lesson = completeLesson();
    delete (lesson as { plant?: unknown }).plant;

    assert.throws(
      () => parseControlLesson(lesson, "control-test-heater"),
      /control lesson 'control-test-heater' missing object 'plant'/
    );
  });

  test("accepts numeric plant state, operating point, and validity contracts", () => {
    const lesson = completeLesson();
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts the curriculum cold-feed flow unit", () => {
    const lesson = completeLesson() as any;
    lesson.signals[3].unit = "L/min";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts the curriculum cold-feed disturbance gain unit", () => {
    const lesson = completeLesson() as any;
    lesson.plant.parameters[0].unit = "degC/(L/min)";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts temperature IAE in degree-seconds", () => {
    const lesson = completeLesson() as any;
    lesson.acceptance[0].unit = "degC*s";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts integral gain in output percent per degree-second", () => {
    const lesson = completeLesson() as any;
    lesson.signals[3].unit = "%/(degC*s)";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts derivative gain in output-percent-seconds per degree", () => {
    const lesson = completeLesson() as any;
    lesson.signals[3].unit = "%*s/degC";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts heating-valve flow gain in litres per minute per output percent", () => {
    const lesson = completeLesson() as any;
    lesson.plant.parameters[0].unit = "L/min/%";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts temperature ISE and ITAE engineering units", () => {
    for (const unit of ["degC^2*s", "degC*s^2"]) {
      const lesson = completeLesson() as any;
      lesson.acceptance[0].unit = unit;
      assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
    }
  });

  test("accepts pump-pressure gain in bar per valve percent", () => {
    const lesson = completeLesson() as any;
    lesson.plant.parameters[0].unit = "bar/%";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts integrated pump-pressure lag in bar-seconds", () => {
    const lesson = completeLesson() as any;
    lesson.acceptance[0].unit = "bar*s";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts signal-to-noise ratio in decibels", () => {
    const lesson = completeLesson() as any;
    lesson.acceptance[0].unit = "dB";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts thermal energy-balance engineering units", () => {
    for (const unit of ["kJ/degC", "kW/degC"]) {
      const lesson = completeLesson() as any;
      lesson.plant.parameters[0].unit = unit;
      assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
    }
  });

  test("accepts rotational inertia in kilogram square metres", () => {
    const lesson = completeLesson() as any;
    lesson.plant.parameters[0].unit = "kg m2";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts rotational damping in newton-metre-seconds per radian", () => {
    const lesson = completeLesson() as any;
    lesson.plant.parameters[0].unit = "N m s/rad";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("accepts integrated position lag in radian-seconds", () => {
    const lesson = completeLesson() as any;
    lesson.acceptance[0].unit = "rad*s";
    assert.deepStrictEqual(parseControlLesson(lesson, "control-test-heater"), lesson);
  });

  test("rejects invalid version 3 engineering and proof fields deterministically", () => {
    expectRejected((lesson) => { lesson.signals[0].unit = "bananas"; },
      "signal 'SP' has unsupported unit 'bananas'");
    expectRejected((lesson) => { lesson.signals[0].role = "command"; },
      "signal has unsupported role 'command'");
    expectRejected((lesson) => { lesson.schemaVersion = 5; }, "unsupported schemaVersion");
    expectRejected((lesson) => { lesson.id = "control-other"; }, "id does not match manifest");
    expectRejected((lesson) => { lesson.signals = lesson.signals.filter((signal: any) => signal.role !== "MV"); },
      "signals missing required role 'MV'");
    expectRejected((lesson) => { lesson.signals[0].minimum = 90; }, "signal 'SP' has invalid range");
    expectRejected((lesson) => { lesson.signals[0].resolution = 0; }, "signal 'SP' has invalid resolution");
    expectRejected((lesson) => { lesson.signals[1].binding = lesson.signals[0].binding; },
      "signals have duplicate binding '%ML0'");
    expectRejected((lesson) => { lesson.signals[1].binding = ""; },
      "missing string 'signals\[1\].binding'");
    expectRejected((lesson) => { lesson.plant.modelId = ""; }, "plant missing string 'modelId'");
    expectRejected((lesson) => { lesson.plant.executable = ""; }, "missing string 'plant.executable'");
    expectRejected((lesson) => { lesson.plant.sampleTimeSeconds = 0; }, "plant has invalid sampleTimeSeconds");
    expectRejected((lesson) => { lesson.plant.parameters[0].value = Number.NaN; },
      "plant parameter 'gain' has nonfinite value");
    expectRejected((lesson) => { lesson.plant.initialState.outletTemperature = "twenty"; },
      "plant.initialState.outletTemperature is not finite");
    expectRejected((lesson) => { lesson.plant.operatingPoint.PV = Number.POSITIVE_INFINITY; },
      "plant.operatingPoint.PV is not finite");
    expectRejected((lesson) => { lesson.plant.validityRange[0].minimum = 100; },
      "plant validity range 'PV' is reversed");
    expectRejected((lesson) => { lesson.experiment.durationSeconds = -1; },
      "experiment has invalid durationSeconds");
    expectRejected((lesson) => { lesson.experiment.signal = "Unknown disturbance"; },
      "experiment names unknown signal 'Unknown disturbance'");
    expectRejected((lesson) => { lesson.acceptance[0].signal = "Unknown"; },
      "acceptance metric 'steady_state_error' names unknown signal 'Unknown'");
    expectRejected((lesson) => { lesson.acceptance[0].limit = Number.POSITIVE_INFINITY; },
      "acceptance metric 'steady_state_error' has nonfinite limit");
    expectRejected((lesson) => { lesson.acceptance[0].window = ""; },
      "missing string 'acceptance metric 'steady_state_error' window'");
    expectRejected((lesson) => { lesson.safety.hardLimits[0].minimum = 101; },
      "safety limit 'PV' has invalid range");
    expectRejected((lesson) => { delete lesson.safety.safeMv; },
      "has invalid safety.safeMv");
    expectRejected((lesson) => { lesson.workflowStages.push("Apply"); },
      "workflowStages contains unsupported stage 'Apply'");
    expectRejected((lesson) => { lesson.workflowStages = ["Monitor", "Validate", "Test"]; },
      "workflowStages are not in execution order");
    expectRejected((lesson) => { lesson.proof.nativeCommand = "cargo test unrelated"; },
      "proof nativeCommand does not name nativeTest 'control_test_heater'");
    expectRejected((lesson) => { lesson.proof.nativeCommand = "cargo test control_test_heater_typo"; },
      "proof nativeCommand does not name nativeTest 'control_test_heater' as an exact argument");
    expectRejected((lesson) => { lesson.video.publicationState = "public"; },
      "video has unsupported publicationState 'public'");
  });

  test("validates a unique gap-free sequence with only earlier prerequisites", () => {
    const api = require("../../examples/controlLesson") as {
      validateControlCurriculum?: (lessons: unknown[]) => unknown;
    };
    const first = { id: "control-first", sequence: 1, prerequisites: [] };
    const second = { id: "control-second", sequence: 2, prerequisites: ["control-first"] };

    assert.deepStrictEqual(api.validateControlCurriculum?.([first, second]), { kind: "accepted" });
    assert.deepStrictEqual(api.validateControlCurriculum?.([first, { ...second, sequence: 1 }]),
      { kind: "rejected", id: "control-second", reason: "duplicate_sequence", sequence: 1 });
    assert.deepStrictEqual(api.validateControlCurriculum?.([first, { ...second, sequence: 3 }]),
      { kind: "rejected", id: "control-second", reason: "sequence_gap", expected: 2, actual: 3 });
    assert.deepStrictEqual(api.validateControlCurriculum?.([
      { ...first, prerequisites: ["control-second"] }, second,
    ]), { kind: "rejected", id: "control-first", reason: "forward_prerequisite", prerequisite: "control-second" });
    assert.deepStrictEqual(api.validateControlCurriculum?.([
      first, { ...second, prerequisites: ["control-unknown"] },
    ]), { kind: "rejected", id: "control-second", reason: "unknown_prerequisite", prerequisite: "control-unknown" });
  });

  test("keeps incomplete lessons visible but unavailable until the version 3 contract is valid", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => unknown;
    };
    const legacy = {
      schemaVersion: 2,
      id: "control-test-heater",
      title: "Test heater",
      level: "Foundation",
      sequence: 1,
      equipment: "Recirculating water heater",
      principalTechnique: "Proportional feedback",
      prerequisites: [],
    };

    assert.deepStrictEqual(
      api.evaluateControlLessonAvailability?.(legacy, "control-test-heater"),
      {
        kind: "unavailable",
        reason: "Requires the complete version 3 engineering contract.",
        lesson: {
          sequence: 1,
          level: "Foundation",
          equipment: "Recirculating water heater",
          principalTechnique: "Proportional feedback",
          prerequisites: [],
        },
      }
    );

    const complete = completeLesson();
    assert.deepStrictEqual(
      api.evaluateControlLessonAvailability?.(complete, "control-test-heater"),
      { kind: "available", lesson: complete }
    );

    delete (complete as { plant?: unknown }).plant;
    assert.deepStrictEqual(
      api.evaluateControlLessonAvailability?.(complete, "control-test-heater"),
      {
        kind: "unavailable",
        reason: "Missing the executable plant contract.",
        lesson: {
          sequence: 1,
          level: "Foundation",
          equipment: "Recirculating water heater",
          principalTechnique: "Proportional feedback",
          prerequisites: [],
        },
      }
    );
  });

  test("allows project creation only for starters and available control lessons", () => {
    const api = require("../../examples/model") as {
      isExampleCopyable?: (entry: unknown) => boolean;
    };
    const starter = { id: "starter", title: "Starter", description: "", path: "starter", hardware: "none" };
    const lesson = { ...starter, id: "control-test", tags: ["control"], lesson: { sequence: 1 } };

    assert.strictEqual(api.isExampleCopyable?.(starter), true);
    assert.strictEqual(api.isExampleCopyable?.({
      ...lesson,
      lessonAvailability: { kind: "available", lesson: completeLesson() },
    }), true);
    assert.strictEqual(api.isExampleCopyable?.({
      ...lesson,
      lessonAvailability: { kind: "unavailable", reason: "Missing proof." },
    }), false);
  });

  test("publishes the repaired water-heater lesson without changing its curriculum identity", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const lessonPath = packagedLessonPath("01-feedback-foundations-and-temperature-control", "04-water-heater-p");
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(raw, "control-water-heater-p");

    assert.strictEqual(availability?.kind, "available");
    assert.strictEqual(availability.lesson.id, "control-water-heater-p");
    assert.strictEqual(availability.lesson.schemaVersion, 4);
    assert.strictEqual(availability.lesson.sequence, 4);
    assert.deepStrictEqual(availability.lesson.prerequisites, ["control-cold-storage-hysteresis-dwell"]);
    assert.strictEqual(availability.lesson.nextLesson, "control-water-heater-bias");
    assert.strictEqual(availability.lesson.baseline.settings.bias, 0);
    assert.strictEqual(availability.lesson.change.settings.bias, 0);
    assert.ok(availability.lesson.learning.theory.length >= 3);
    assert.ok(availability.lesson.learning.workedPrediction.length > 30);
    assert.strictEqual(availability.lesson.proof.nativeTest,
      "control_curriculum_water_heater_p_exposes_offset_gain_tradeoff_under_same_load");
  });

  test("keeps the P-control reference lesson focused and words before symbols", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
      controlLessonPresentation?: (lesson: unknown) => Array<{ title: string; paragraphs: string[] }>;
    };
    const lessonPath = packagedLessonPath("01-feedback-foundations-and-temperature-control", "04-water-heater-p");
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(raw, "control-water-heater-p");
    assert.strictEqual(availability?.kind, "available");

    assert.strictEqual(
      availability.lesson.objective,
      "Tune a proportional controller experimentally and explain why the best acceptable setting still leaves a steady-state temperature error."
    );

    const theory = pedagogicalText(availability.lesson.learning.theory.join("\n"));
    const requiredDefinitions = [
      /setpoint \(SP\).*desired outlet-water temperature/i,
      /process value \(PV\).*outlet-water temperature measured by the sensor/i,
      /error \(e\).*setpoint minus the process value/i,
      /proportional gain \(Kp\).*percent heater power per degree Celsius/i,
      /requested manipulated variable \(MV_requested\).*heater-power request/i,
      /applied manipulated variable \(MV_applied\).*heater-power command/i,
      /clamp.*physical range/i,
      /closed-loop feedback.*sensor.*controller/i,
      /zero-bias controller.*0.*100 percent.*proportional band \(PB\).*error range/i,
      /steady-state error.*remaining difference/i,
    ];
    for (const definition of requiredDefinitions) {
      assert.match(theory, definition);
    }
    const orderedTheory = theory.toLowerCase();
    assert.ok(
      orderedTheory.indexOf("setpoint (sp)") < orderedTheory.indexOf("e = sp - pv")
        && orderedTheory.indexOf("process value (pv)") < orderedTheory.indexOf("e = sp - pv")
        && orderedTheory.indexOf("error (e)") < orderedTheory.indexOf("e = sp - pv"),
      "SP, PV and e must each be introduced in words before e = SP - PV"
    );
    assert.ok(
      orderedTheory.indexOf("proportional gain (kp)") < orderedTheory.indexOf("mv_requested = kp")
        && orderedTheory.indexOf("requested manipulated variable (mv_requested)") < orderedTheory.indexOf("mv_requested = kp")
        && orderedTheory.indexOf("applied manipulated variable (mv_applied)") < orderedTheory.indexOf("mv_applied = clamp"),
      "Kp, requested MV and applied MV must be introduced before their control relationships"
    );
    assert.ok(
      orderedTheory.indexOf("proportional band (pb)") < orderedTheory.indexOf("pb = 100 / kp"),
      "PB must be introduced in words before its equation"
    );

    for (const laterLessonTheory of [
      "first-order-plus-dead-time",
      "first-order plus dead time",
      "fopdt",
      "static process gain",
      "time constant",
      "dead time",
      "model identification",
    ]) {
      assert.ok(
        !orderedTheory.includes(laterLessonTheory),
        `Lesson 4 must not teach later model theory: ${laterLessonTheory}`
      );
    }

    const sections = api.controlLessonPresentation?.(availability.lesson);
    const theorySection = sections?.find((section) => section.title === "Theory");
    assert.deepStrictEqual(theorySection?.paragraphs, availability.lesson.learning.theory);

    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const readmeSource = fs.readFileSync(
      path.join(repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "04-water-heater-p", "README.md"),
      "utf8"
    );
    assert.match(
      readmeSource,
      /Previous lesson:\s*\[Hysteresis and dwell protection\]\(\.\.\/03-cold-storage-hysteresis-dwell\/README\.md\)/
    );
    assert.match(
      readmeSource,
      /Next lesson: \[Operating-point bias\]\(\.\.\/05-water-heater-bias\/README\.md\)/
    );
    assert.doesNotMatch(
      readmeSource,
      /(?:Prerequisite|Next lesson):\s*`control-/,
      "learner navigation must use human lesson names instead of internal catalog IDs"
    );
    const readme = pedagogicalText(readmeSource);
    for (const definition of requiredDefinitions) {
      assert.match(readme, definition);
    }
    const requiredHeadings = canonicalLessonReadmeHeadings;
    assert.deepStrictEqual(
      [...readmeSource.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      requiredHeadings,
      "the reference lesson must have one concise teaching path without padded workflow sections"
    );
    for (const laterLessonTheory of [
      /FOPDT/i,
      /static process gain/i,
      /time constant/i,
      /dead time/i,
      /^## Mathematical model$/m,
      /^## Identify$/m,
      /^## Tune$/m,
      /^## Reproducing the figures$/m,
      /^## Advanced exercise$/m,
    ]) {
      assert.doesNotMatch(readmeSource, laterLessonTheory);
    }
    assert.ok(
      readmeSource.trim().split(/\s+/).length <= 2_100,
      "the rendered reference lesson must remain a focused page, not a multi-lesson wall of text"
    );
  });

  test("defines the P-control decision metrics and rejects unsettled error claims", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "04-water-heater-p");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));

    for (const requirement of [
      /self-regulating thermal process/i,
      /requested manipulated variable \(MV_requested\).*before.*physical output limit/i,
      /applied manipulated variable \(MV_applied\).*after.*clamp/i,
      /heater can add heat but cannot actively remove it/i,
      /because the thermal process responds with lag.*previous heater command/i,
      /for this zero-bias controller with a 0.*100 percent output range/i,
      /required steady heater output \(MV_required\)/i,
      /steady-state error \(e_ss\)/i,
      /MV_required = Kp(?: ×)? e_ss/i,
      /e_ss = MV_required \/ Kp/i,
      /cumulative process-value travel.*sum.*absolute.*successive/i,
      /cumulative applied-output travel.*sum.*absolute.*successive/i,
      /percentage points \(pp\)/i,
      /settled.*final 10-second.*process-value range.*0\.1 °C.*applied-output range.*1\.0 pp/i,
      /\| 40 %\/°C \| Not settled \|/i,
    ]) assert.match(readme, requirement);

    assert.match(source, /^## Implementation$/m);
    assert.match(source, /Parameters\.ProportionalGain := ProportionalGain;/);
    assert.match(source, /ManipulatedValue := Controller\.Update\(Input := Input\);/);
    assert.doesNotMatch(source, /Final temperature error|smallest final error/i);
    assert.deepStrictEqual(
      lesson.acceptance.slice(0, 2).map((item: any) => [item.metric, item.window, item.comparator]),
      [
        ["settled_mean_absolute_error_kp_2", "110 to 120 s after settling qualification", "<="],
        ["settled_mean_absolute_error_kp_8", "110 to 120 s after settling qualification", "<="],
      ]
    );
    assert.ok(lesson.proof.runArtifactFields.includes("settled"));
    assert.ok(lesson.proof.runArtifactFields.includes("meanAbsoluteError"));
  });

  test("makes the P-control gain sweep executable and both heater outputs traceable", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "04-water-heater-p");
    const source = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));

    assert.match(source, /RequestedManipulatedValue\s*:\s*LREAL/);
    assert.match(source, /RequestedManipulatedValue\s*:=\s*ProportionalGain\s*\*\s*\(Setpoint\s*-\s*ProcessValue\)/);
    assert.match(source, /Parameters\.ProportionalGain\s*:=\s*ProportionalGain;\s*Controller\.Configure/s);
    assert.match(source, /P1\.RequestedManipulatedValue AT %ML16 : LREAL/);
    assert.match(pedagogicalText(readme), /requested manipulated variable \(MV_requested\).*%ML16/is);
    assert.match(readme, /gain.*live.*next 100 ms scan/is);

    const requestedSignal = lesson.signals.find((signal: any) => signal.name === "Requested heater power");
    const appliedSignal = lesson.signals.find((signal: any) => signal.name === "Applied heater power");
    assert.strictEqual(requestedSignal?.binding, "%ML16");
    assert.strictEqual(appliedSignal?.binding, "%QL0");
    assert.strictEqual(
      lesson.acceptance.find((item: any) => item.metric === "selected_kp_8_mv_travel")?.unit,
      "pp"
    );
    assert.ok(!lesson.acceptance.some((item: any) => item.metric === "settled_mean_absolute_error_kp_2_and_8"));
    assert.deepStrictEqual(
      lesson.acceptance
        .filter((item: any) => item.metric.startsWith("settled_mean_absolute_error_kp_"))
        .map((item: any) => [item.metric, item.comparator, item.unit]),
      [
        ["settled_mean_absolute_error_kp_2", "<=", "degC"],
        ["settled_mean_absolute_error_kp_8", "<=", "degC"],
      ]
    );
  });

  test("ships reproducible teaching figures for the P-control lesson", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "04-water-heater-p");
    const readme = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const resultFigure = [
      "figures/p-gain-comparison.svg",
      "Water-heater responses for three proportional gains",
    ] as const;
    assert.ok(
      readme.includes(`![${resultFigure[1]}](${resultFigure[0]})`),
      "the lesson must use one real-results figure where the gain trade-off becomes clearer than prose"
    );
    assert.ok(fs.statSync(path.join(lessonRoot, resultFigure[0])).isFile());
    assert.strictEqual(
      [...readme.matchAll(/!\[[^\]]*\]\([^)]*\)/g)].length,
      1,
      "the reference lesson must not include redundant or decorative figures"
    );
    for (const obsoleteFigure of [
      "heater-loop.mmd",
      "heater-loop.svg",
      "heater-model-response.svg",
      "puppeteer-config.json",
    ]) {
      assert.ok(
        !fs.existsSync(path.join(lessonRoot, "figures", obsoleteFigure)),
        `the focused reference lesson must not retain unused figure ${obsoleteFigure}`
      );
    }

    const csvPath = path.join(lessonRoot, "figures", "p-gain-comparison.csv");
    const csv = fs.readFileSync(csvPath, "utf8").trim().split("\n");
    assert.strictEqual(csv[0], "time_s,setpoint_degC,load_event,low_kp_pv_degC,low_kp_mv_requested_percent,low_kp_mv_applied_percent,high_kp_pv_degC,high_kp_mv_requested_percent,high_kp_mv_applied_percent,aggressive_kp_pv_degC,aggressive_kp_mv_requested_percent,aggressive_kp_mv_applied_percent");
    assert.strictEqual(csv.length, 1_201, "the retained evidence must contain every 100 ms sample of the 120 s run");
    const samples = csv.slice(1).map((row) => row.split(",").map((value, index) => index === 2 ? value : Number(value)));
    const postEvent = samples.slice(600);
    const finalWindow = samples.slice(1_100);
    const totalVariation = (column: number): number => postEvent.slice(1).reduce(
      (sum, sample, index) => sum + Math.abs(Number(sample[column]) - Number(postEvent[index][column])), 0
    );
    const meanAbsoluteError = (column: number): number => finalWindow.reduce(
      (sum, sample) => sum + Math.abs(Number(sample[1]) - Number(sample[column])), 0
    ) / finalWindow.length;
    const range = (column: number): number => {
      const values = finalWindow.map((sample) => Number(sample[column]));
      return Math.max(...values) - Math.min(...values);
    };
    assert.ok(range(3) <= 0.1 && range(5) <= 1.0, "Kp 2 retained evidence must be settled");
    assert.ok(range(6) <= 0.1 && range(8) <= 1.0, "Kp 8 retained evidence must be settled");
    assert.ok(range(9) > 0.1 || range(11) > 1.0, "Kp 40 retained evidence must be reported as not settled");
    assert.strictEqual(samples[0][4], 30, "Kp 2 requested output must be retained");
    assert.strictEqual(samples[0][5], 30, "Kp 2 applied output must be retained");
    assert.strictEqual(samples[0][7], 120, "Kp 8 unclamped request must be retained");
    assert.strictEqual(samples[0][8], 100, "Kp 8 applied output must expose saturation");
    assert.ok(readme.includes(`| 2 %/°C | ${meanAbsoluteError(3).toFixed(1)} °C | ${totalVariation(3).toFixed(1)} °C | ${totalVariation(5).toFixed(1)} pp |`));
    assert.ok(readme.includes(`| 8 %/°C | ${meanAbsoluteError(6).toFixed(1)} °C | ${totalVariation(6).toFixed(1)} °C | ${totalVariation(8).toFixed(1)} pp |`));
    assert.ok(readme.includes(`| 40 %/°C | Not settled | ${totalVariation(9).toFixed(1)} °C | ${totalVariation(11).toFixed(1)} pp |`));
    const generator = fs.readFileSync(path.join(lessonRoot, "figures", "generate_figures.py"), "utf8");
    assert.doesNotMatch(generator, /model_response|time constant|dead time/i);

    const comparison = fs.readFileSync(path.join(lessonRoot, "figures", "p-gain-comparison.svg"), "utf8");
    assert.match(
      comparison,
      /<rect[^>]+class="surface"/,
      "the embedded plot needs its own high-contrast surface because VS Code theme state does not propagate into an external SVG"
    );
    assert.match(comparison, /\.surface\s*\{[^}]*fill:\s*#f8fafc/i);
    assert.match(
      comparison,
      /forced-color-adjust:\s*none/i,
      "the self-contained plot must preserve its qualified line distinctions in VS Code high contrast"
    );
    assert.doesNotMatch(
      comparison,
      /prefers-color-scheme/i,
      "the plot cannot depend on operating-system color preference when VS Code uses an independent editor theme"
    );
    assert.doesNotMatch(
      comparison,
      /<(?:text|title|desc)[^>]*>[^<]*(?:degC|12\.86|6\.92)/i,
      "visible plot text must use the lesson's engineering unit and one-decimal decision precision"
    );
    for (const label of ["Simulation evidence", "Time (s)", "Temperature (°C)", "Requested power (%)", "Applied power (%)", "Cold-water load at 60 s", "Physical maximum 100%", "Kp 40", "12.9 °C", "6.9 °C", "Reduced damping"]) {
      assert.ok(comparison.includes(label), `comparison graph must contain '${label}'`);
    }
  });

  test("teaches operating-point bias with the focused reference lesson structure", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "05-water-heater-bias");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const readmeSource = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalText(readmeSource);

    assert.strictEqual(
      lesson.objective,
      "Measure the nominal holding output, use it as controller bias, compare the same proportional controller under a changed load, and explain why fixed bias still leaves offset."
    );
    assert.deepStrictEqual(
      [...readmeSource.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonReadmeHeadings
    );
    assert.match(readmeSource, /Previous lesson:\s*\[Water-heater proportional control\]\(\.\.\/04-water-heater-p\/README\.md\)/);
    assert.match(readmeSource, /Next lesson:\s*\[Integral action in isolation\]\(\.\.\/06-water-heater-integral\/README\.md\)/);
    assert.doesNotMatch(readmeSource, /(?:Prerequisite|Next lesson):\s*`control-/);

    const definitions = [
      /setpoint \(SP\).*desired outlet-water temperature/i,
      /process value \(PV\).*measured outlet-water temperature/i,
      /requested manipulated variable \(MV_requested\).*controller calculates/i,
      /applied manipulated variable \(MV_applied\).*heater receives/i,
      /operating point.*outlet temperature and heater output remain steady/i,
      /controller bias \(u_?0\).*measured holding output/i,
      /temperature deviation.*current temperature minus the operating-point temperature/i,
      /proportional gain \(Kp\).*percent heater power per degree Celsius/i,
      /error \(e\).*setpoint minus the process value/i,
    ];
    for (const definition of definitions) {
      assert.match(readme, definition);
    }
    const lower = readme.toLowerCase();
    for (const [definition, equation] of [
      ["setpoint (sp)", "e = sp - pv"],
      ["process value (pv)", "e = sp - pv"],
      ["error (e)", "e = sp - pv"],
      ["controller bias (u_0)", "mv_requested = u_0 + kp"],
      ["proportional gain (kp)", "mv_requested = u_0 + kp"],
      ["requested manipulated variable (mv_requested)", "mv_requested = u_0 + kp"],
      ["applied manipulated variable (mv_applied)", "mv_applied = clamp"],
    ]) {
      assert.ok(lower.indexOf(definition) >= 0 && lower.indexOf(definition) < lower.indexOf(equation),
        `${definition} must be taught before ${equation}`);
    }
    assert.match(readmeSource, /75 (?:percent|%).*nominal|nominal.*75 (?:percent|%)/i);
    assert.match(readmeSource, /changed[- ]load/i);
    assert.match(readmeSource, /positive error/i);
    assert.doesNotMatch(readmeSource, /^## (?:Configure|Monitor|Test|Identify|Tune|Explain|Export)$/m);
    assert.ok(instructionalCoreWordCount(readmeSource) <= 1_600);
  });

  test("keeps every temperature-spine README on the focused teaching path", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonPaths = [
      "01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy",
      "01-feedback-foundations-and-temperature-control/02-cold-storage-on-off",
      "01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell",
      "01-feedback-foundations-and-temperature-control/04-water-heater-p",
      "01-feedback-foundations-and-temperature-control/05-water-heater-bias",
      "01-feedback-foundations-and-temperature-control/06-water-heater-integral",
      "01-feedback-foundations-and-temperature-control/07-water-heater-pi",
      "01-feedback-foundations-and-temperature-control/08-water-heater-saturation",
      "01-feedback-foundations-and-temperature-control/09-rate-limit",
      "01-feedback-foundations-and-temperature-control/10-saturation-antiwindup",
      "01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory",
      "01-feedback-foundations-and-temperature-control/12-reactor-setpoint-weighting",
      "01-feedback-foundations-and-temperature-control/13-measurement-noise",
      "01-feedback-foundations-and-temperature-control/14-sampling-interval",
      "01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance",
      "01-feedback-foundations-and-temperature-control/16-heater-step-identification",
      "01-feedback-foundations-and-temperature-control/17-independent-model-validation",
      "01-feedback-foundations-and-temperature-control/18-nominal-fopdt",
    ];
    const requiredHeadings = canonicalLessonReadmeHeadings;

    for (const lessonPath of lessonPaths) {
      const readme = fs.readFileSync(
        path.join(repositoryRoot, "examples", "control", lessonPath, "README.md"),
        "utf8"
      );
      assert.deepStrictEqual(
        [...readme.matchAll(/^## (.+)$/gm)]
          .map((match) => match[1]),
        requiredHeadings,
        `${lessonPath} must use the single ordered teaching path`
      );
      assert.doesNotMatch(readme, /(?:Prerequisite|Next lesson):\s*`control-/,
        `${lessonPath} must use learner-facing navigation rather than catalog IDs`);
    }
  });

  test("teaches integral-only and PI control as one causal words-before-symbols progression", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const readLesson = (name: string) => {
      const root = path.join(repositoryRoot, "examples", "control",
        "01-feedback-foundations-and-temperature-control", name);
      return {
        metadata: JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8")),
        readme: fs.readFileSync(path.join(root, "README.md"), "utf8"),
      };
    };
    const integral = readLesson("06-water-heater-integral");
    const pi = readLesson("07-water-heater-pi");

    assert.strictEqual(integral.metadata.objective,
      "Calculate the integral heater-output change per scan, then use an equivalent load experiment to explain why integral action removes constant-load offset but gives a slower transient response on its own.");
    assert.strictEqual(pi.metadata.objective,
      "Keep the proven proportional gain fixed, vary only integral gain in a bounded comparison, and select a PI controller that restores temperature promptly without excessive overshoot, saturation, or actuator movement.");

    const integralText = pedagogicalText(integral.readme).toLowerCase();
    for (const definition of [
      /setpoint \(sp\).*desired outlet-water temperature/,
      /process value \(pv\).*measured outlet-water temperature/,
      /error \(e\).*setpoint minus the process value/,
      /requested manipulated variable \(mv_requested\).*before physical limits/,
      /applied manipulated variable \(mv_applied\).*heater receives.*0.*100 percent/,
      /integral gain \(ki\).*percentage points.*per degree celsius-second/,
      /cycle time \((?:dt|δt)\).*seconds between controller updates/,
      /integral contribution \(ui\).*stored part.*heater command/,
    ]) {
      assert.match(integralText, definition);
    }
    const integralEquation = integralText.indexOf("ui,new = ui,old + ki × e × dt");
    assert.ok(integralEquation > 0, "Lesson 6 must present the per-scan integral equation");
    for (const definition of ["error (e)", "integral gain (ki)", "cycle time (dt)", "integral contribution (ui)"]) {
      assert.ok(integralText.indexOf(definition) < integralEquation,
        `${definition} must precede the integral equation`);
    }

    const piText = pedagogicalText(pi.readme).toLowerCase();
    for (const definition of [
      /proportional gain \(kp\).*requested-output percentage points per degree celsius/,
      /integral gain \(ki\).*stored-output change in percentage points per degree celsius-second/,
      /integral time \(t_?i\).*seconds.*integral action/,
      /requested manipulated variable \(mv_requested\).*preliminary request before limits/,
      /applied manipulated variable \(mv_applied\).*bounded heater command/,
    ]) {
      assert.match(piText, definition);
    }
    const piEquation = piText.indexOf("mv_requested = kp × e + ui");
    assert.ok(piEquation > 0, "Lesson 7 must present the parallel PI equation");
    for (const definition of ["proportional gain (kp)", "integral contribution (ui)", "requested manipulated variable (mv_requested)"]) {
      assert.ok(piText.indexOf(definition) >= 0 && piText.indexOf(definition) < piEquation,
        `${definition} must precede the PI equation`);
    }
    assert.match(pi.readme, /Previous lesson:\s*\[Integral action\]/);
    assert.match(pi.readme, /Next lesson:\s*\[Actuator saturation\]/);
  });

  test("teaches actuator constraints and anti-windup without borrowing later model theory", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const readme = (name: string) => fs.readFileSync(
      path.join(repositoryRoot, "examples", "control",
        "01-feedback-foundations-and-temperature-control", name, "README.md"), "utf8");
    const normalized = (source: string) => pedagogicalText(source).toLowerCase();
    const saturation = normalized(readme("08-water-heater-saturation"));
    const rate = normalized(readme("09-rate-limit"));
    const antiWindup = normalized(readme("10-saturation-antiwindup"));

    for (const definition of [
      /requested manipulated variable \(mv_requested\).*heater command.*controller calculates/,
      /applied manipulated variable \(mv_applied\).*command.*heater physically receives/,
      /saturation error \(e_sat\).*requested heater power minus applied heater power/,
      /target is therefore unreachable.*model/,
    ]) assert.match(saturation, definition);
    const saturationEquation = saturation.indexOf("e_sat = mv_requested - mv_applied");
    assert.ok(saturationEquation > saturation.indexOf("saturation error (e_sat)"));

    for (const definition of [
      /configured controller task interval \((?:dt|δt)\).*intended time between updates.*seconds/,
      /previous applied position \(mv_previous\).*last update.*current update.*percent/,
      /maximum opening rate \(r_open\).*percentage points per second/,
      /maximum closing rate \(r_close\).*percentage points per second/,
      /lowest admissible position \(mv_low\).*smallest position.*percent/,
      /highest admissible position \(mv_high\).*largest position.*percent/,
      /measured travel rate \(r_measured\).*percentage points per second/,
    ]) assert.match(rate, definition);
    assert.match(rate, /configured nominal interval.*not a scheduler-derived measurement/);
    assert.match(rate, /task interval is 100 ms.*cycle_interval_ms is 100.*configuredcycletime is 0\.1 second/);
    assert.match(rate, /lesson 14 adds measured elapsed time and jitter/);
    const rateEquation = rate.indexOf("mv_low = max(0%, mv_previous - r_close × dt)");
    assert.ok(rateEquation > rate.indexOf("lowest admissible position (mv_low)"));

    for (const definition of [
      /integral windup.*stored controller demand.*actuator cannot apply/,
      /tracking time \(t_?t\).*stored demand.*constrained output.*seconds/,
      /saturation error \(e_sat\).*nominal output minus saturated candidate.*percentage points/,
      /tracking coefficient \(alpha\).*fraction of saturation error.*one update/,
    ]) assert.match(antiWindup, definition);
    const backCalculation = antiWindup.indexOf("mv_stored = mv_nominal - alpha × e_sat");
    assert.ok(backCalculation > antiWindup.indexOf("tracking coefficient (alpha)"));

    for (const source of [readme("08-water-heater-saturation"), readme("09-rate-limit"), readme("10-saturation-antiwindup")]) {
      assert.doesNotMatch(source, /FOPDT|first-order-plus-dead-time|time constant|dead time/i,
        "Lessons 8-10 must not use model theory before the modelling spine");
      assert.match(source, /^### Control Studio actions$/m,
        "Each lesson must expose the exact continuous Control Studio path");
    }
  });

  test("teaches response metrics weighting filtering and timing before using their notation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const load = (name: string) => fs.readFileSync(
      path.join(repositoryRoot, "examples", "control",
        "01-feedback-foundations-and-temperature-control", name, "README.md"), "utf8");
    const normalize = (source: string) => pedagogicalText(source).toLowerCase();
    const objectives = normalize(load("11-reactor-servo-regulatory"));
    const weighting = normalize(load("12-reactor-setpoint-weighting"));
    const filtering = normalize(load("13-measurement-noise"));
    const timing = normalize(load("14-sampling-interval"));

    for (const definition of [
      /servo test.*changes the setpoint.*tracking/,
      /regulatory test.*setpoint remains fixed.*disturbance changes/,
      /integral of absolute error \(iae\).*degree-celsius seconds/,
      /integral of squared error \(ise\).*squared-degree- celsius seconds/,
      /time-weighted integral of absolute error \(itae\).*degree-celsius seconds squared/,
      /complete-event applied-\s*valve travel.*sum.*absolute.*mv_applied/,
    ]) assert.match(objectives, definition);
    assert.ok(objectives.indexOf("integral of absolute error (iae)") < objectives.indexOf("iae="));

    for (const definition of [
      /proportional setpoint weight \(b\).*dimensionless.*setpoint-change/,
      /true error \(e\).*setpoint minus the process value/,
      /proportional valve increment \((?:delta_|δ)mv_p\).*percentage points/,
      /integral valve increment \((?:delta_|δ)mv_i\).*percentage points/,
    ]) assert.match(weighting, definition);
    assert.ok(weighting.indexOf("proportional setpoint weight (b)") < weighting.indexOf("δmv_p ="));

    for (const definition of [
      /raw process value \(pv_raw\).*sampled transmitter value.*bar/,
      /controller process value \(pv_controller\).*after the selected measurement path/,
      /filter time constant \(τ_f\).*0\.1 second/,
      /filter coefficient \(a\).*fraction.*difference/,
      /continuous reference cutoff frequency \(fc_reference\).*continuous first-order pole/,
      /controller-pv increment rms.*root mean square.*consecutive.*differences/,
    ]) assert.match(filtering, definition);
    assert.ok(filtering.indexOf("filter coefficient (a)")
      < filtering.indexOf("a = dt / (τ_f + 0.5 × dt)"));

    for (const definition of [
      /actual cycle time \(dt_actual\).*elapsed timestamp time.*seconds/,
      /controller cycle time \(dt_controller\).*passed.*time-scaled calculations.*seconds/,
      /nominal cycle time \(dt_nominal\).*configured 0\.1-second schedule/,
      /sampling frequency \(fs\).*samples per second/,
      /nyquist frequency \(f_n\).*half the sampling frequency/,
      /timing jitter \(j\).*actual cycle time minus nominal cycle time/,
      /missed scan.*actual interval greater than 0\.25 second/,
    ]) assert.match(timing, definition);
    assert.ok(timing.indexOf("sampling frequency (fs)") < timing.indexOf("f_s = 1 / δt_actual"));

    for (const source of [load("11-reactor-servo-regulatory"), load("12-reactor-setpoint-weighting"), load("13-measurement-noise"), load("14-sampling-interval")]) {
      assert.doesNotMatch(source, /FOPDT|first-order-plus-dead-time|time constant \d|dead time/i,
        "Lessons 11-14 must not depend on plant-model terminology taught in Lesson 15");
      assert.match(source, /^### Control Studio actions$/m);
    }
  });

  test("teaches the heater modelling identification validation and tuning chain words first", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const read = (group: string, name: string) => fs.readFileSync(
      path.join(repositoryRoot, "examples", "control", group, name, "README.md"), "utf8");
    const normalize = (source: string) => pedagogicalText(source).toLowerCase();
    const physics = normalize(read("01-feedback-foundations-and-temperature-control", "15-water-heater-energy-balance"));
    const identify = normalize(read("01-feedback-foundations-and-temperature-control", "16-heater-step-identification"));
    const validate = normalize(read("01-feedback-foundations-and-temperature-control", "17-independent-model-validation"));
    const tune = normalize(read("01-feedback-foundations-and-temperature-control", "18-nominal-fopdt"));

    for (const definition of [
      /water temperature \(t\).*degrees celsius/,
      /thermal capacitance \(c\).*kilojoules per degree celsius/,
      /heater heat input \(Q_in\).*kilowatts/i,
      /heat-loss coefficient \(h\).*kilowatts per degree celsius/,
      /ambient temperature \(t_?ambient\).*degrees celsius/,
      /equilibrium temperature \(t_?eq\).*temperature.*stops changing/,
      /static gain \(k\).*degrees celsius per percent heater power/,
      /time constant \(τ\).*seconds.*63\.2 percent/,
      /apparent dead time \(l\).*seconds.*response begins/,
    ]) assert.match(physics, definition);
    assert.ok(physics.indexOf("thermal capacitance (c)") < physics.indexOf("cdt / dt = q_in"));

    for (const definition of [
      /baseline input \(u_?0\).*heater command before the step/,
      /baseline output \(y_?0\).*temperature before the step/,
      /(?:input )?step amplitude \((?:δu|delta_u)\).*difference.*new.*baseline (?:heater )?(?:command|input)/,
      /first-order-plus-dead-time model(?: \(fopdt\))?.*gain.*time constant.*(?:dead time|delay)/,
    ]) assert.match(identify, definition);
    assert.ok(identify.indexOf("first-order-plus-dead-time model") < identify.indexOf("t × dy/dt"));

    for (const definition of [
      /measured process value \(pv_measured\).*temperature.*degrees celsius/,
      /model-predicted process value \(pv_predicted\).*temperature calculated by the frozen model/,
      /prediction residual \(r_k\).*measured.*minus.*predicted/,
      /residual bias.*signed mean.*residual/,
      /prediction envelope.*lower-to-upper.*parameter.*interval/,
    ]) assert.match(validate, definition);
    assert.ok(validate.indexOf("prediction residual (r_k)") < validate.indexOf("r_k = pv_measured,k - pv_predicted,k"));

    for (const definition of [
      /desired closed-loop response time \(lambda\).*seconds/,
      /model gain \(k\).*degrees celsius per percentage point/,
      /model time constant \(t\).*seconds/,
      /model dead time \(l\).*seconds/,
      /proportional gain \(kp\).*heater percent per degree celsius/,
      /integral time \(t_?i\).*seconds/,
      /integral gain \(ki\).*heater percent per degree-celsius-second/,
    ]) assert.match(tune, definition);
    assert.ok(tune.indexOf("desired closed-loop response time (lambda)") < tune.indexOf("kp ="));

    for (const source of [read("01-feedback-foundations-and-temperature-control", "15-water-heater-energy-balance"), read("01-feedback-foundations-and-temperature-control", "16-heater-step-identification"), read("01-feedback-foundations-and-temperature-control", "17-independent-model-validation"), read("01-feedback-foundations-and-temperature-control", "18-nominal-fopdt")]) {
      assert.match(source, /^### Control Studio actions$/m);
      assert.doesNotMatch(source, /(?:Prerequisite|Next lesson):\s*`?control-/);
    }
  });

  test("publishes the motion progression and first production-PID lesson in causal order", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const expected = [
      [19, "02-derivative-action-and-dynamic-response/19-servo-position-p", "control-servo-position-p"],
      [20, "02-derivative-action-and-dynamic-response/20-servo-position-pd", "control-servo-position-pd"],
      [21, "02-derivative-action-and-dynamic-response/21-underdamped-pid", "control-loaded-position-pid"],
      [22, "02-derivative-action-and-dynamic-response/22-servo-derivative-kick", "control-servo-derivative-kick"],
      [23, "02-derivative-action-and-dynamic-response/23-servo-filtered-derivative", "control-servo-filtered-derivative"],
      [24, "02-derivative-action-and-dynamic-response/24-servo-mechanical-nonidealities", "control-servo-mechanical-nonidealities"],
      [25, "02-derivative-action-and-dynamic-response/25-servo-digital-realization", "control-servo-digital-realization"],
      [26, "03-production-quality-digital-pid/26-pid-forms", "control-pid-forms"],
    ] as const;
    const headings = canonicalLessonReadmeHeadings;

    for (const [sequence, folder, id] of expected) {
      const root = path.join(repositoryRoot, "examples", "control", folder);
      assert.ok(fs.existsSync(path.join(root, "src", "Main.st")), `${folder} needs executable ST`);
      assert.ok(fs.existsSync(path.join(root, "simulation.toml")), `${folder} needs an executable plant`);
      assert.ok(fs.existsSync(path.join(root, "lesson.json")), `${folder} needs a lesson descriptor`);
      assert.ok(fs.existsSync(path.join(root, "README.md")), `${folder} needs a pedagogical README`);
      const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
      const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
      assert.deepStrictEqual(parseControlLesson(lesson, id), lesson);
      assert.strictEqual(lesson.schemaVersion, 4);
      assert.strictEqual(lesson.sequence, sequence);
      assert.strictEqual(lesson.id, id);
      assert.deepStrictEqual([...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]), headings);
      assert.doesNotMatch(readme, /(?:Prerequisite|Next lesson):\s*`?control-/);
    }
  });

  test("teaches position P control with executable torque and metric definitions", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const root = path.join(repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response", "19-servo-position-p");
    const source = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const readme = pedagogicalText(source);

    for (const requirement of [
      /setpoint \(r or SP\).*desired table angle.*process value.*measured angle.*radians/i,
      /proportional gain \(Kp\).*requested motor torque per radian.*N.*m\/rad/i,
      /rotational inertia \(J\).*resistance to angular acceleration.*kg.*m/i,
      /viscous damping coefficient \(B\).*opposing torque.*angular velocity/i,
      /undamped natural frequency.*oscillation rate.*rad\/s/i,
      /damping ratio.*critical amount.*avoid oscillation/i,
      /90-percent rise time.*first retained sample.*PV.*0\.9 rad.*first arrival/i,
      /position overshoot.*largest positive distance.*1-rad target/i,
      /peak applied torque.*maximum absolute value.*positive acceleration.*negative braking/i,
    ]) assert.match(readme, requirement);

    assert.match(source, /J\\ddot\{\\theta\}\+B\\dot\{\\theta\}=\\tau_\{applied\}/);
    assert.match(source, /\\omega_n=\\sqrt\{K_p\/J\}/);
    assert.match(source, /\\zeta=B\/\[2\\sqrt\{JK_p\}\]/);
    assert.match(source, /^## Implementation$/m);
    assert.match(source, /Parameters\.ProportionalGain := ProportionalGain;/);
    assert.match(source, /MotorTorque := Controller\.Update\(Input := Input\);/);
    assert.doesNotMatch(source, /The \*\*manipulated variable \(MV\)\*\* is the motor torque requested by the\s+controller and applied by the simulated drive/);
    assert.ok(instructionalCoreWordCount(source) <= 2_200);
  });

  test("teaches PD as measured damping on the same positioning table", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const source = fs.readFileSync(path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response", "20-servo-position-pd", "README.md"
    ), "utf8");
    const readme = pedagogicalText(source);

    for (const requirement of [
      /angular velocity.*rate.*measured position changes.*radians per second/i,
      /derivative gain.*unit is N m s\/rad.*leaving N m/i,
      /positive.*table.*negative.*moves negatively.*positive.*opposes motion/is,
      /rotational torque balance.*J.*theta.*B.*theta.*T/is,
      /For PD control, substitute.*fixed target angle.*Move all motion terms/is,
      /standard second-order form.*matching the coefficient/is,
      /natural frequency.*contains Kp and J, but not Kd.*does not change.*stiffness/is,
      /damping ratio.*critical amount.*Derivative gain.*numerator/is,
      /sinusoidal displacement.*differentiation.*cos.*quarter cycle/is,
      /differentiate with respect to time.*substitution.*cancel.*energy decrease/is,
      /position overshoot.*complete run.*late position span.*5 to 15 seconds/is,
      /ratios are dimensionless because radians cancel.*same plant.*measurement windows/is,
      /sole reviewed write.*P1\.ExperimentEnabled/is,
      /learner workflow is exactly.*Monitor.*Test.*no separate.*Validate/is,
      /Observe without excitation.*Amplitude.*0.*15\.02/is,
    ]) assert.match(readme, requirement);
    assert.match(source, /J\\ddot\{x\}\+\(B\+K_d\)\\dot\{x\}\+K_p x=0/);
    assert.match(source, /\\omega_n=\\sqrt\{K_p\/J\}/);
    assert.match(source, /\\zeta=\(B\+K_d\)\/\(2\\sqrt\{JK_p\}\)/);
    assert.match(source, /\\frac\{dE\}\{dt\}=-\(B\+K_d\)\\omega\^2/);
    assert.match(source, /^## Implementation$/m);
    assert.match(source, /Parameters\.DerivativeSetpointWeight := LREAL#0\.0;/);
    assert.match(source, /MotorTorque := Controller\.Update\(Input := Input\);/);
    assert.ok(instructionalCoreWordCount(source) <= 2_600);
  });

  test("teaches loaded PID as equilibrium memory with qualified final error", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const source = fs.readFileSync(path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response", "21-underdamped-pid", "README.md"
    ), "utf8");
    const readme = pedagogicalText(source);

    for (const requirement of [
      /peak gravity-torque coefficient.*does not mean.*always applies 4 N m/is,
      /integral state.*accumulated error over time/is,
      /rectangle with height.*width.*state update/is,
      /Error has unit rad.*time has unit s.*integral gain.*product.*N m/is,
      /requested controller torque before the drive limit/is,
      /drive cannot deliver.*integral windup.*anti-windup/is,
      /At rest.*rotational balance.*motor torque.*gravity torque/is,
      /nonlinear equation.*no elementary algebraic solution.*small-angle approximation/is,
      /collect the angle terms.*remaining error/is,
      /PID equilibrium.*current error and velocity.*zero.*integral state.*nonzero/is,
      /linearize gravity.*constant.*cancelled.*characteristic polynomial/is,
      /Routh table.*first column.*All must be positive/is,
      /mean absolute error.*28 to 30 seconds.*positive and negative errors.*average/is,
      /quiet final sample alone is not enough/is,
      /50 consecutive samples.*one full second/is,
      /Observe without excitation.*30\.02.*P1\.ExperimentEnabled/is,
    ]) assert.match(readme, requirement);
    assert.match(source, /T_g=G\\sin\(\\theta\)/);
    assert.match(source, /\\theta_\{ss\}\\approx\\frac\{K_pr\}\{K_p\+G\}/);
    assert.match(source, /I_\{ss\}=\\frac\{G\\sin\(r\)\}\{K_i\}/);
    assert.match(source, /Js\^3\+\(B\+K_d\)s\^2\+\(K_p\+G\\cos r\)s\+K_i=0/);
    assert.match(source, /^## Implementation$/m);
    assert.match(source, /Parameters\.IntegralGain := IntegralGain;/);
    assert.match(source, /MotorTorque := Controller\.Update\(Input := Input\);/);
    assert.ok(instructionalCoreWordCount(source) <= 3_200);
  });

  test("teaches derivative kick without hiding the controller request behind saturation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const source = fs.readFileSync(path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response", "22-servo-derivative-kick", "README.md"
    ), "utf8");
    const readme = pedagogicalText(source);

    for (const requirement of [
      /derivative setpoint weight.*dimensionless.*0 to 1/is,
      /weighted derivative input.*chooses.*setpoint.*derivative path/is,
      /subtract the old weighted input.*On the first update.*table has not yet moved/is,
      /ideal continuous derivative.*step.*impulse.*sampled controller.*finite cycle/is,
      /inversely proportional.*cycle time.*Halving cycle time.*double/is,
      /derivative on full error.*derivative on measurement/is,
      /setpoint step.*no derivative torque.*physical PV motion.*damping/is,
      /two-degree-of-freedom PID/is,
      /first integral-state increment.*error times cycle time/is,
      /complete first request.*proportional.*integral.*derivative/is,
      /external clamp does not feed.*controller.*internal output limit/is,
      /first post-step request.*first update after SP changes/is,
      /final-sample position error.*only an endpoint tracking check/is,
      /Observe without excitation.*15\.02.*P1\.ExperimentEnabled/is,
    ]) assert.match(readme, requirement);
    assert.match(source, /\\Delta x_D=c\\,\\Delta SP-\\Delta PV/);
    assert.match(source, /T_D=K_d\\frac\{c\\,\\Delta SP\}\{\\Delta t\}/);
    assert.match(source, /T_\{requested\}=79\.02,\\ 41\.52,\\ 4\.02/);
    assert.match(source, /^## Implementation$/m);
    assert.match(source, /RequestedTorque := Controller\.Update\(Input := Input\);/);
    assert.match(source, /AppliedTorque := MAX\(MIN\(RequestedTorque, LREAL#10\.0\), LREAL#-10\.0\);/);
    assert.ok(instructionalCoreWordCount(source) <= 2_700);
  });

  test("teaches derivative filtering as a quantified noise and lag tradeoff", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const source = fs.readFileSync(path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response", "23-servo-filtered-derivative", "README.md"
    ), "utf8");
    const readme = pedagogicalText(source);

    for (const requirement of [
      /raw encoder position \(PV_raw\).*radians/i,
      /measurement noise \(n\).*unwanted variation/i,
      /standard deviation \(σ\).*0\.002 rad/i,
      /controller-facing position \(PV_control\)/i,
      /filter time constant \(τ\).*controls its cutoff/i,
      /τ.*0\.05 (?:s|seconds)/i,
      /filter order.*number of identical stages/i,
      /filter coefficient \(a\).*dimensionless/i,
      /a = dt \/ \(τ \+ 0\.5 × dt\)/i,
      /torque-increment root-mean-square value \(RMS_ΔT\).*15 to 20 seconds.*N m/i,
      /RMS\(ΔT\) = sqrt\(mean\(\(T_applied,k - T_applied,k-1\)\^2\)\)/i,
      /positive position-lag area \(A_lag\).*0 to 5 seconds.*radian-seconds/i,
      /A_lag = sum max\(PV_raw,k - PV_control,k, 0\) × dt/i,
      /more filtering is therefore not automatically better/i,
    ]) assert.match(readme, requirement);
    assert.match(source, /^## Implementation$/m);
    assert.match(source, /Input\.ProcessValue := ControllerPosition;/);
    assert.match(source, /MotorTorque := Controller\.Update\(Input := Input\);/);
    assert.ok(instructionalCoreWordCount(source) <= 1_600);
  });

  test("teaches mechanical diagnosis before servo retuning", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const source = fs.readFileSync(path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response",
      "24-servo-mechanical-nonidealities", "README.md"
    ), "utf8");
    const readme = pedagogicalText(source);

    for (const requirement of [
      /target position \(SP_target\).*desired final table angle.*radians/i,
      /profiled position \(SP_profile\).*acceleration- and velocity-limited/i,
      /load position \(PV_load\).*load-side encoder.*radians/i,
      /requested motor torque \(T_requested\).*controller.*newton metres/i,
      /applied motor torque \(T_applied\).*drive.*newton metres/i,
      /trajectory acceleration limit \(a_profile\).*radians per second squared/i,
      /trajectory velocity limit \(v_profile\).*radians per second/i,
      /Coulomb friction.*opposes motion/i,
      /command deadband.*small command change.*ignored/i,
      /backlash width \(w_backlash\).*total reversal travel.*radians/i,
      /stiction.*holds.*breakaway/i,
      /a_ideal = T_limit \/ J = 2\.5 N m \/ 1\.0 kg m\^2 = 2\.5 rad\/s\^2/i,
      /Δv_max = a_profile × dt = 1\.0 rad\/s\^2 × 0\.02 s = 0\.02 rad\/s/i,
      /Δposition_max = v_profile × dt = 0\.5 rad\/s × 0\.02 s = 0\.01 rad/i,
      /reversal delay.*first sample.*0\.020 rad/i,
      /requested-to-applied torque gap.*absolute.*T_requested minus T_applied/i,
      /retuning.*shall not.*mechanical repair/i,
    ]) assert.match(readme, requirement);
    assert.match(source, /^## Implementation$/m);
    assert.match(source, /StoppingVelocity := SQRT/);
    assert.match(source, /RequestedTorque := Controller\.Update\(Input := Input\);/);
    assert.ok(instructionalCoreWordCount(source) <= 1_600);
  });

  test("teaches an executed digital realization rather than renamed continuous PID", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response",
      "25-servo-digital-realization"
    );
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = readStructuredTextProject(lessonRoot);
    const readme = pedagogicalText(source);

    for (const requirement of [
      /controller sample interval \((?:dt|Δt)\).*seconds.*between controller updates/i,
      /sampling frequency \(f_s\).*updates per second/i,
      /Nyquist frequency \(f_N\).*half.*sampling frequency/i,
      /zero-order hold.*retains.*torque.*between updates/i,
      /quantization step \(q\).*0\.001 rad/i,
      /alias frequency \(f_alias\).*8 Hz.*10 Hz.*2 Hz/i,
      /computation delay \(T_delay\).*one update/i,
      /forward Euler.*previous error/i,
      /backward Euler.*current error/i,
      /Tustin.*average.*previous.*current error/i,
      /filtered derivative torque \(T_D\).*measurement-rate/i,
      /f_s = 1 \/ dt/i,
      /f_N = f_s \/ 2/i,
      /dt = 0\.10 s.*f_s = 10 Hz.*f_N = 5 Hz/i,
      /maximum absolute position error.*maximum.*absolute.*SP minus PV/i,
      /applied-torque total variation.*sum.*successive/i,
      /unstable|not settled/i,
      /LREAL.*coefficient.*state precision/i,
    ]) assert.match(readme, requirement);
    assert.match(source, /^## Implementation$/m);
    assert.match(program, /CASE MethodSelection OF/);
    assert.match(program, /RequestedTorque := Controller\.Update/);
    assert.match(program, /Between controller updates RequestedTorque is deliberately unchanged/);
    assert.ok(instructionalCoreWordCount(source) <= 1_600);
  });

  test("teaches why the production PID selects positional or incremental realization", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "03-production-quality-digital-pid", "26-pid-forms"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 26 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 26 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-pid-forms"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.id, "control-pid-forms");
    assert.strictEqual(lesson.sequence, 26);
    assert.deepStrictEqual(lesson.prerequisites, ["control-servo-digital-realization"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_combined_pid_selects_the_valid_form_from_integral_action");
    for (const requirement of [
      /controller output \(u\).*actuator request/i,
      /sample index \(k\).*current controller update/i,
      /current error \(e\[k\]\).*setpoint.*process value/i,
      /previous error \(e\[k-1\]\)/i,
      /error change \(Δe\[k\]\).*current error minus the previous error/i,
      /sample interval \(T_s\).*0\.02 seconds/i,
      /positional form.*complete output/i,
      /incremental form.*velocity form.*previous output/i,
      /u\[k\] = u_?0 \+ Kp × e\[k\]/i,
      /u\[k\] = u\[k-1\] \+ Kp × Δe\[k\] \+ Ki × T_s × e\[k\]/i,
      /constant.*error.*Δe\[k\] = 0/i,
      /parallel.*ideal.*series.*parameter/i,
      /rejected incremental-P.*never.*plant/i,
      /immediate gain-change output difference.*one controller update/i,
      /PI output discontinuity.*expected integral increment/i,
      /initialization.*Lesson 27/i,
    ]) assert.match(readme, requirement);
    assert.match(source, /^## Implementation$/m);
    assert.match(program, /PositionTorque := PositionController\.Update/);
    assert.match(program, /HeaterPower := HeaterController\.Update/);
    assert.match(program, /PositionParameters\.IntegralGain := LREAL#0\.0;/);
    assert.match(program, /HeaterParameters\.IntegralGain := HeaterIntegralGain;/);
    assert.match(program, /RejectedIncrementalPOutput := RejectedIncrementalPOutput/);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 2);
    assert.ok(instructionalCoreWordCount(source) <= 1_600);
  });

  test("publishes the cold-storage lesson only with its complete executable contract", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const lessonPath = path.resolve(
      __dirname,
      "..", "..", "..", "media", "examples", "control", "01-feedback-foundations-and-temperature-control", "02-cold-storage-on-off", "lesson.json"
    );
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(raw, "control-cold-storage-on-off");

    assert.strictEqual(availability?.kind, "available");
    assert.strictEqual(availability.lesson.schemaVersion, 4);
    assert.strictEqual(availability.lesson.id, "control-cold-storage-on-off");
    assert.strictEqual(availability.lesson.sequence, 2);
    assert.deepStrictEqual(availability.lesson.prerequisites, ["control-cold-storage-loop-anatomy"]);
    assert.strictEqual(availability.lesson.plant.modelId, "first_order");
    assert.strictEqual(availability.lesson.experiment.kind, "observation");
    assert.strictEqual(availability.lesson.experiment.signal, "Experiment authority");
    assert.strictEqual(availability.lesson.experiment.amplitude, 0);
    assert.strictEqual(availability.lesson.experiment.durationSeconds, 120.1);
    assert.deepStrictEqual(availability.lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(availability.lesson.proof.nativeTest,
      "control_curriculum_cold_storage_on_off_exports_regulation_and_chatter_evidence");
  });

  test("publishes the three cold-storage lessons as a causal pedagogical sequence", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..",
      );
    const sourceRoot = path.join(repositoryRoot, "examples");
    const catalog = JSON.parse(fs.readFileSync(path.join(sourceRoot, "catalog.json"), "utf8")) as
      Array<{ id: string; path: string }>;
    const expected = [
      ["control-cold-storage-loop-anatomy", 1, [], "control-cold-storage-on-off"],
      ["control-cold-storage-on-off", 2, ["control-cold-storage-loop-anatomy"],
        "control-cold-storage-hysteresis-dwell"],
      ["control-cold-storage-hysteresis-dwell", 3, ["control-cold-storage-on-off"],
        "control-water-heater-p"],
    ] as const;

    assert.deepStrictEqual(
      catalog.filter((entry) => entry.id.startsWith("control-")).slice(0, 3).map((entry) => entry.id),
      expected.map(([id]) => id)
    );
    for (const [id, sequence, prerequisites, nextLesson] of expected) {
      const entry = catalog.find((candidate) => candidate.id === id);
      assert.ok(entry, `catalog must contain ${id}`);
      const raw = JSON.parse(fs.readFileSync(path.join(sourceRoot, entry!.path, "lesson.json"), "utf8"));
      const availability = api.evaluateControlLessonAvailability?.(raw, id);
      assert.strictEqual(availability?.kind, "available", availability?.reason);
      assert.strictEqual(availability.lesson.schemaVersion, 4);
      assert.strictEqual(availability.lesson.sequence, sequence);
      assert.deepStrictEqual(availability.lesson.prerequisites, prerequisites);
      assert.strictEqual(availability.lesson.nextLesson, nextLesson);
      assert.ok(availability.lesson.objective.length > 30);
      assert.ok(availability.lesson.learning.theory.length > 0);
      assert.ok(availability.lesson.learning.practicalPreparation.length > 0);
    }
  });

  test("publishes the Lambda PI lesson only with its model-qualified contract", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const lessonPath = path.resolve(
      __dirname,
      "..", "..", "..", "media", "examples", "control", "01-feedback-foundations-and-temperature-control", "18-nominal-fopdt", "lesson.json"
    );
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(raw, "control-nominal-fopdt");

    assert.strictEqual(availability?.kind, "available");
    assert.strictEqual(availability.lesson.schemaVersion, 4);
    assert.strictEqual(availability.lesson.sequence, 18);
    assert.deepStrictEqual(availability.lesson.prerequisites,
      ["control-independent-heater-model-validation"]);
    assert.strictEqual(availability.lesson.plant.modelId, "validated_local_first_order_dead_time");
    assert.strictEqual(availability.lesson.experiment.kind, "observation");
    assert.strictEqual(availability.lesson.experiment.signal, "Experiment authority");
    assert.strictEqual(availability.lesson.experiment.amplitude, 0);
    assert.strictEqual(availability.lesson.experiment.durationSeconds, 120.1);
    assert.strictEqual(availability.lesson.change.settings.method, "Lambda/IMC");
    assert.strictEqual(availability.lesson.change.settings.lambdaSeconds, 4);
    assert.deepStrictEqual(availability.lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(availability.lesson.proof.nativeTest,
      "control_curriculum_lambda_pi_improves_declared_response_and_survives_model_error");
  });

  test("publishes the reactor lesson only with separate servo and regulatory evidence", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const lessonPath = path.resolve(
      __dirname,
      "..", "..", "..", "media", "examples", "control", "01-feedback-foundations-and-temperature-control", "11-reactor-servo-regulatory", "lesson.json"
    );
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(
      raw,
      "control-reactor-servo-regulatory"
    );

    assert.strictEqual(availability?.kind, "available");
    assert.strictEqual(availability.lesson.schemaVersion, 4);
    assert.strictEqual(availability.lesson.sequence, 11);
    assert.deepStrictEqual(availability.lesson.prerequisites, ["control-saturation-antiwindup"]);
    assert.strictEqual(availability.lesson.plant.modelId, "first_order_dead_time");
    assert.strictEqual(availability.lesson.baseline.executableId, "Control.PID.PI.ServoObjective");
    assert.strictEqual(availability.lesson.change.executableId, "Control.PID.PI.RegulatoryObjective");
    for (const field of ["proportionalGain", "integralGain", "derivativeGain"]) {
      assert.strictEqual(availability.lesson.baseline.settings[field],
        availability.lesson.change.settings[field]);
    }
    assert.strictEqual(availability.lesson.experiment.kind, "observation");
    assert.deepStrictEqual(availability.lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(availability.lesson.proof.nativeTest,
      "control_curriculum_reactor_retains_separate_servo_and_regulatory_evidence");
    assert.ok(availability.lesson.signals.some(
      (signal: { name: string; binding: string }) => signal.name === "Manual jacket-valve demand"
        && signal.binding === "%ML8"
    ));
    const finalWindow = availability.lesson.acceptance.find(
      (criterion: { metric: string }) => criterion.metric
        === "regulatory_final_window_mean_absolute_error"
    );
    assert.strictEqual(finalWindow.window,
      "110.1 through 120.0 s after recovery qualification");
    assert.ok(availability.lesson.proof.runArtifactFields.includes("safeRestoration"));
    assert.match(availability.lesson.safety.restorationMode,
      /Manual zero.*rate-constrained.*applied valve reaches zero/i);
  });

  test("publishes the sampling lesson only when actual cycle time is executable evidence", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const lessonPath = path.resolve(
      __dirname,
      "..", "..", "..", "media", "examples", "control", "01-feedback-foundations-and-temperature-control", "14-sampling-interval", "lesson.json"
    );
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(raw, "control-sampling-interval");

    assert.strictEqual(availability?.kind, "available");
    assert.strictEqual(availability.lesson.schemaVersion, 4);
    assert.strictEqual(availability.lesson.sequence, 14);
    assert.deepStrictEqual(availability.lesson.prerequisites, ["control-measurement-noise"]);
    assert.strictEqual(availability.lesson.plant.modelId, "first_order_dead_time");
    assert.strictEqual(
      availability.lesson.signals.find((signal: any) => signal.name === "Actual cycle time")?.binding,
      "%ML8"
    );
    assert.deepStrictEqual(availability.lesson.change.settings.intervalsMilliseconds, [50, 100, 200]);
    assert.strictEqual(availability.lesson.experiment.kind, "observation");
    assert.strictEqual(availability.lesson.experiment.signal, "Experiment authority");
    assert.strictEqual(availability.lesson.experiment.durationSeconds, 30.6);
    assert.strictEqual(
      availability.lesson.signals.find((signal: any) => signal.name === "Experiment authority")?.binding,
      "%MX80.0"
    );
    assert.deepStrictEqual(availability.lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(availability.lesson.proof.nativeTest,
      "control_curriculum_sampling_timing_contract");
  });

  test("publishes anti-windup only with identical unreachable-demand runs", () => {
    const api = require("../../examples/controlLesson") as {
      evaluateControlLessonAvailability?: (raw: unknown, id: string) => any;
    };
    const lessonPath = path.resolve(
      __dirname,
      "..", "..", "..", "media", "examples", "control", "01-feedback-foundations-and-temperature-control", "10-saturation-antiwindup", "lesson.json"
    );
    const raw = JSON.parse(fs.readFileSync(lessonPath, "utf8"));
    const availability = api.evaluateControlLessonAvailability?.(
      raw,
      "control-saturation-antiwindup"
    );

    assert.strictEqual(availability?.kind, "available");
    assert.strictEqual(availability.lesson.schemaVersion, 4);
    assert.strictEqual(availability.lesson.sequence, 10);
    assert.deepStrictEqual(availability.lesson.prerequisites, ["control-rate-limit"]);
    assert.strictEqual(availability.lesson.plant.modelId, "first_order_dead_time");
    assert.strictEqual(availability.lesson.baseline.settings.strategy, 0);
    assert.deepStrictEqual(availability.lesson.change.settings.strategies, [
      "1 signal clamping Tt=dt",
      "2 BackCalculation Tt=3.25 s",
      "3 Combined Tt=3.25 s",
    ]);
    assert.strictEqual(availability.lesson.experiment.kind, "observation");
    assert.deepStrictEqual(availability.lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(availability.lesson.proof.nativeTest,
      "control_curriculum_batch_vessel_retains_equivalent_pi_anti_windup_runs");
  });

  test("retains learner progress when a known lesson becomes unavailable", () => {
    const api = require("../../examples/controlLesson") as {
      retainControlLessonProgress?: (completed: string[], known: string[]) => string[];
    };

    assert.deepStrictEqual(
      api.retainControlLessonProgress?.(
        ["control-water-heater-p", "removed-lesson"],
        ["control-cold-storage-on-off", "control-water-heater-p"]
      ),
      ["control-water-heater-p"]
    );
  });

  test("uses the same lesson validator before packaging canonical source", () => {
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-lessons-"));
    const source = path.join(temporary, "examples");
    const output = path.join(temporary, "packaged");
    const project = path.join(source, "control", "invalid");
    fs.mkdirSync(project, { recursive: true });
    const catalog = [{ id: "control-test-heater", title: "Test heater", description: "Test",
      path: "control/invalid", hardware: "none", tags: ["control"] }];
    fs.writeFileSync(path.join(source, "catalog.json"), JSON.stringify(catalog));
    const invalid = completeLesson();
    delete (invalid as { plant?: unknown }).plant;
    fs.writeFileSync(path.join(project, "lesson.json"), JSON.stringify(invalid));
    const script = path.resolve(__dirname, "..", "..", "..", "scripts", "build-examples.js");
    try {
      const rejected = spawnSync(process.execPath, [script], {
        encoding: "utf8",
        env: { ...process.env, TRUST_EXAMPLES_SOURCE_ROOT: source, TRUST_EXAMPLES_OUTPUT_ROOT: output },
      });
      assert.notStrictEqual(rejected.status, 0);
      assert.match(rejected.stderr, /missing object 'plant'/);
      assert.ok(!fs.existsSync(output), "an invalid lesson must fail before package output is replaced");

      fs.writeFileSync(path.join(project, "lesson.json"), JSON.stringify(completeLesson()));
      const accepted = spawnSync(process.execPath, [script], {
        encoding: "utf8",
        env: { ...process.env, TRUST_EXAMPLES_SOURCE_ROOT: source, TRUST_EXAMPLES_OUTPUT_ROOT: output },
      });
      assert.strictEqual(accepted.status, 0, accepted.stderr);
      assert.deepStrictEqual(
        fs.readFileSync(path.join(output, "control", "invalid", "lesson.json")),
        fs.readFileSync(path.join(project, "lesson.json"))
      );
    } finally {
      fs.rmSync(temporary, { recursive: true, force: true });
    }
  });

  test("ships heater step identification as causal curriculum lesson 16", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonPath = path.join(
      repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "16-heater-step-identification", "lesson.json"
    );
    const lesson = parseControlLesson(
      JSON.parse(fs.readFileSync(lessonPath, "utf8")),
      "control-heater-step-identification"
    ) as any;

    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 16);
    assert.deepStrictEqual(lesson.prerequisites, ["control-water-heater-energy-balance"]);
    assert.strictEqual(lesson.nextLesson, "control-independent-heater-model-validation");
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
  });

  test("keeps the numbered heater identification lesson navigable from adjacent lessons", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const moduleRoot = path.join(
      repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control"
    );
    const expectedTargets = [
      path.join(moduleRoot, "16-heater-step-identification", "README.md"),
      path.join(moduleRoot, "16-heater-step-identification", "figures", "heater-identification-model-evidence.json"),
    ];

    for (const target of expectedTargets) {
      assert.ok(fs.existsSync(target), `numbered lesson target must exist: ${target}`);
    }

    const lesson15 = fs.readFileSync(path.join(moduleRoot, "15-water-heater-energy-balance", "README.md"), "utf8");
    const lesson17 = fs.readFileSync(path.join(moduleRoot, "17-independent-model-validation", "README.md"), "utf8");
    const lesson17Evidence = JSON.parse(fs.readFileSync(
      path.join(moduleRoot, "17-independent-model-validation", "figures", "heater-validation-evidence.json"), "utf8"
    ));
    const lesson18Evidence = JSON.parse(fs.readFileSync(
      path.join(moduleRoot, "18-nominal-fopdt", "figures", "lambda-tuning-evidence.json"), "utf8"
    ));

    assert.match(lesson15, /\.\.\/16-heater-step-identification\/README\.md/);
    assert.match(lesson17, /\.\.\/16-heater-step-identification\/README\.md/);
    assert.strictEqual(
      lesson17Evidence.modelSource.artifact,
      "../../16-heater-step-identification/figures/heater-identification-model-evidence.json"
    );
    assert.strictEqual(
      lesson18Evidence.modelSource.artifact,
      "../../16-heater-step-identification/figures/heater-identification-model-evidence.json"
    );
  });

  test("ships independent heater validation as causal curriculum lesson 17", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonPath = path.join(
      repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "17-independent-model-validation", "lesson.json"
    );
    const lesson = parseControlLesson(
      JSON.parse(fs.readFileSync(lessonPath, "utf8")),
      "control-independent-heater-model-validation"
    ) as any;

    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.id, "control-independent-heater-model-validation");
    assert.strictEqual(lesson.sequence, 17);
    assert.deepStrictEqual(lesson.prerequisites, ["control-heater-step-identification"]);
    assert.strictEqual(lesson.nextLesson, "control-nominal-fopdt");
  });

  test("ships model-based heater PI tuning as causal curriculum lesson 18", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonPath = path.join(
      repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control", "18-nominal-fopdt", "lesson.json"
    );
    const lesson = parseControlLesson(
      JSON.parse(fs.readFileSync(lessonPath, "utf8")),
      "control-nominal-fopdt"
    ) as any;

    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 18);
    assert.deepStrictEqual(lesson.prerequisites, ["control-independent-heater-model-validation"]);
    assert.strictEqual(lesson.nextLesson, "control-servo-position-p");
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
  });

  test("gives every lesson an ordered lesson-specific Control Studio workflow", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const curriculumRoot = path.join(repositoryRoot, "examples", "control");
    const lessonFiles: string[] = [];
    const visit = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const target = path.join(directory, entry.name);
        if (entry.isDirectory()) {
          visit(target);
        } else if (entry.name === "lesson.json") {
          lessonFiles.push(target);
        }
      }
    };
    visit(curriculumRoot);
    assert.strictEqual(lessonFiles.length, 84);

    const stageLabels: Record<string, string> = {
      Monitor: "**Monitor**",
      Test: "**Test**",
      Identify: "**Identify**",
      Tune: "**Tune / Design**",
      Validate: "**Validate**",
    };

    for (const lessonFile of lessonFiles) {
      const lesson = JSON.parse(fs.readFileSync(lessonFile, "utf8"));
      const loop = JSON.parse(fs.readFileSync(
        path.join(path.dirname(lessonFile), "control-loop.json"), "utf8"
      ));
      const readme = fs.readFileSync(path.join(path.dirname(lessonFile), "README.md"), "utf8");
      const heading = "### Control Studio actions";
      const start = readme.indexOf(heading);
      assert.ok(start >= 0, `${lesson.id} must explain its Control Studio workflow`);
      const nextHeading = readme.indexOf("\n## ", start + heading.length);
      const section = readme.slice(start, nextHeading < 0 ? readme.length : nextHeading);
      let previous = -1;
      for (const [index, stage] of lesson.workflowStages.entries()) {
        const label = stageLabels[stage];
        assert.ok(label, `${lesson.id} declares unsupported workflow stage ${stage}`);
        const position = section.indexOf(label);
        assert.ok(position > previous, `${lesson.id} must explain ${stage} in declared order`);
        assert.ok(
          section.includes(`${index + 1}. ${label}`),
          `${lesson.id} must number ${stage} as workflow step ${index + 1}`
        );
        previous = position;
      }
      for (const [stage, label] of Object.entries(stageLabels)) {
        if (!lesson.workflowStages.includes(stage)) {
          assert.ok(!section.includes(label), `${lesson.id} must not invent undeclared ${stage} work`);
        }
      }
      assert.match(
        section,
        /truST sidebar\s*(?:→|->)\s*Control Studio/,
        `${lesson.id} must give the complete visible path to Control Studio`
      );
      assert.match(section, /because|so that|in order to|why/i,
        `${lesson.id} must explain why the learner performs the selected action`);
      assert.match(section, /disabled|unavailable|prerequisite/i,
        `${lesson.id} must explain an unmet prerequisite or disabled action`);
      assert.match(section, /abort|reject|stale|unsafe|recover|retry|restore/i,
        `${lesson.id} must give an applicable failure or recovery action`);
      if (lesson.workflowStages.includes("Monitor")) {
        assert.match(section, /Response/,
          `${lesson.id} Monitor instructions must name the Response plot`);
        assert.match(section, /Start recording/,
          `${lesson.id} Monitor instructions must name Start recording`);
      }
      if (lesson.workflowStages.includes("Test")) {
        for (const label of ["Experiment", "Amplitude", "Maximum duration (s)", "Review test", "Confirm and start", "Abort test"]) {
          assert.ok(section.includes(label), `${lesson.id} Test instructions must name '${label}'`);
        }
        assert.ok(section.includes(String(lesson.experiment.durationSeconds)),
          `${lesson.id} Test instructions must retain its duration`);
      }
      if (lesson.workflowStages.includes("Identify")) {
        for (const label of ["Model selection", "Qualify recording", "Fit estimation run", "Accept model for design"]) {
          assert.ok(section.includes(label), `${lesson.id} Identify instructions must name '${label}'`);
        }
        if (loop.controller.design === "project") {
          assert.match(section, /low-order model/i,
            `${lesson.id} must explain the boundary of Control Studio's shared identification`);
          assert.match(section, /does not replace|is not.*design authority|must not be accepted/i,
            `${lesson.id} must not present a low-order preview as its advanced design model`);
        }
      }
      if (lesson.workflowStages.includes("Tune")) {
        for (const label of ["Design method", "Generate candidate"]) {
          assert.ok(section.includes(label), `${lesson.id} Tune instructions must name '${label}'`);
        }
        if (loop.controller.design === "pid") {
          assert.ok(section.includes("Response"),
            `${lesson.id} shared PID instructions must name the Response choice`);
        } else {
          assert.match(section, /designed by the runnable project/i,
            `${lesson.id} must identify the runnable project as design owner`);
          assert.match(section, /shared Control\.PID tuning is unavailable/i,
            `${lesson.id} must explain why the shared PID designer is disabled`);
          assert.doesNotMatch(section, /choose \*\*Manual Control\.PID\*\*/i,
            `${lesson.id} must not tell the learner to select an unrelated PID designer`);
        }
      }
      if (lesson.workflowStages.includes("Validate")) {
        for (const label of ["Run required validation", "Requirement details", "Save result"]) {
          assert.ok(section.includes(label), `${lesson.id} Validate instructions must name '${label}'`);
        }
        const sharedValidation = loop.controller.design === "pid" &&
          lesson.workflowStages.includes("Tune");
        if (!sharedValidation) {
          assert.match(section, /project-owned validation|validated by the runnable project/i,
            `${lesson.id} must identify where validation without a shared candidate is executed`);
          assert.match(section, /Run required validation.*disabled|disabled.*Run required validation/is,
            `${lesson.id} must not tell the learner to run validation without a shared candidate`);
        }
      }
      assert.match(section, /before|predict|confirm|verify/i, `${lesson.id} must include a pre-run or progression check`);
      assert.match(section, /record|retain|decide|conclude|reject|accept/i, `${lesson.id} must require an engineering decision`);
      for (const label of ["Baseline check", "Decision before action", "Successful evidence", "Recovery", "Record"]) {
        assert.ok(section.includes(`**${label}:**`),
          `${lesson.id} must organize the workflow around '${label}' rather than generic navigation prose`);
      }
      assert.ok(section.includes(lesson.experiment.signal),
        `${lesson.id} must name the exact experimented or observed signal`);
      assert.ok(section.includes(String(lesson.experiment.seed)),
        `${lesson.id} must retain the deterministic experiment seed`);
      assert.ok(section.includes(lesson.baseline.limitation),
        `${lesson.id} must connect the workflow to the baseline limitation being investigated`);
      assert.ok(section.includes(lesson.change.decision),
        `${lesson.id} must state the lesson-specific engineering decision the learner records`);
      assert.doesNotMatch(section, /worked prediction|this specific question/i,
        `${lesson.id} must replace generated placeholders with a concrete lesson decision`);
    }
  });

  test("binds every temperature-foundation lesson to its real Control Studio signals", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const moduleRoot = path.join(
      repositoryRoot, "examples", "control", "01-feedback-foundations-and-temperature-control"
    );
    const api = require("../../controlStudio/loopDefinition") as {
      decodeControlLoopDefinition(value: unknown): { kind: string; loop?: any };
    };
    const lessonDirectories = fs.readdirSync(moduleRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(moduleRoot, entry.name))
      .filter((directory) => fs.existsSync(path.join(directory, "lesson.json")));
    assert.strictEqual(lessonDirectories.length, 18);

    for (const directory of lessonDirectories) {
      const lesson = JSON.parse(fs.readFileSync(path.join(directory, "lesson.json"), "utf8"));
      const loopPath = path.join(directory, "control-loop.json");
      assert.ok(fs.existsSync(loopPath), `${lesson.id} must ship control-loop.json`);
      const rawLoop = JSON.parse(fs.readFileSync(loopPath, "utf8"));
      const decoded = api.decodeControlLoopDefinition(rawLoop);
      assert.strictEqual(decoded.kind, "accepted", `${lesson.id} loop must decode`);
      const loop = decoded.loop;
      assert.strictEqual(loop.id, lesson.id);
      assert.strictEqual(loop.projectId, `project:${lesson.id}`);
      assert.strictEqual(loop.runtimeId, "simulator");
      assert.ok(["pid", "project"].includes(loop.controller.design),
        `${lesson.id} must declare who owns controller design`);
      if (loop.controller.design === "pid") assert.strictEqual(loop.controller.type, "Control.PID");
      assert.ok(loop.experiment, `${lesson.id} must bind its Control Studio Test defaults`);
      assert.strictEqual(loop.experiment.durationSeconds, lesson.experiment.durationSeconds);
      assert.strictEqual(loop.experiment.seed, lesson.experiment.seed);
      if (loop.experiment.execution === "project-scripted") {
        assert.strictEqual(loop.experiment.kind, "observation",
          `${lesson.id} project-scripted Test must observe without writing a second excitation`);
        assert.strictEqual(loop.experiment.amplitude, 0);
      } else {
        assert.strictEqual(loop.experiment.kind, lesson.experiment.kind);
        assert.strictEqual(loop.experiment.amplitude, lesson.experiment.amplitude);
      }

      for (const [role, binding] of [
        ["SP", loop.signals.setpoint], ["PV", loop.signals.processVariable],
        ["MV", loop.signals.manipulatedVariable],
      ] as const) {
        if (!binding) continue;
        assert.ok(lesson.signals.some((signal: any) =>
          signal.role === role && signal.binding === binding.address && signal.unit === binding.unit),
        `${lesson.id} primary ${role} must be an explicit lesson signal`);
      }
      const source = fs.readdirSync(path.join(directory, "src"))
        .filter((name) => name.endsWith(".st"))
        .map((name) => fs.readFileSync(path.join(directory, "src", name), "utf8"))
        .join("\n");
      if (loop.topology !== "open-loop" && ["output_step", "mv_step", "mv_pulse", "relay", "prbs", "multisine"]
        .includes(loop.experiment.kind)) {
        assert.ok(loop.controller.modeSignal,
          `${lesson.id} manual-output experiment must declare its real controller mode signal`);
        assert.match(source, new RegExp(`AT\\s+${escapeRegExp(loop.controller.modeSignal)}`),
          `${lesson.id} manual-output experiment mode signal must be bound by the shipped program`);
      }
      for (const binding of [loop.signals.setpoint, loop.signals.processVariable,
        loop.signals.manipulatedVariable, loop.signals.actuator,
        ...(loop.signals.additional ?? [])].filter(Boolean)) {
        assert.match(source, new RegExp(`${escapeRegExp(binding.symbol)}\\s+AT\\s+${escapeRegExp(binding.address)}`),
          `${lesson.id} must bind ${binding.symbol} to its declared runtime address`);
      }
    }
  });

  test("binds every motion lesson to its executable loop and bounded Test defaults", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const moduleRoot = path.join(
      repositoryRoot, "examples", "control", "02-derivative-action-and-dynamic-response"
    );
    const loopApi = require("../../controlStudio/loopDefinition") as {
      decodeControlLoopDefinition(value: unknown): { kind: string; loop?: any };
    };
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(value: unknown): { kind: string; reason?: string };
    };
    const lessonDirectories = fs.readdirSync(moduleRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(moduleRoot, entry.name))
      .filter((directory) => fs.existsSync(path.join(directory, "lesson.json")));
    assert.strictEqual(lessonDirectories.length, 7);

    for (const directory of lessonDirectories) {
      const lesson = JSON.parse(fs.readFileSync(path.join(directory, "lesson.json"), "utf8"));
      const loopPath = path.join(directory, "control-loop.json");
      assert.ok(fs.existsSync(loopPath), `${lesson.id} must ship control-loop.json`);
      const decoded = loopApi.decodeControlLoopDefinition(JSON.parse(fs.readFileSync(loopPath, "utf8")));
      assert.strictEqual(decoded.kind, "accepted", `${lesson.id} loop must decode`);
      const loop = decoded.loop;
      assert.strictEqual(loop.experiment.kind, lesson.experiment.kind);
      assert.strictEqual(loop.experiment.amplitude, lesson.experiment.amplitude);
      assert.strictEqual(loop.experiment.durationSeconds, lesson.experiment.durationSeconds);
      assert.strictEqual(loop.experiment.seed, lesson.experiment.seed);
      const source = fs.readdirSync(path.join(directory, "src"))
        .filter((name) => name.endsWith(".st"))
        .map((name) => fs.readFileSync(path.join(directory, "src", name), "utf8")).join("\n");
      for (const binding of [loop.signals.setpoint, loop.signals.processVariable,
        loop.signals.manipulatedVariable, ...(loop.signals.additional ?? [])]) {
        assert.match(source, new RegExp(`${escapeRegExp(binding.symbol)}\\s+AT\\s+${escapeRegExp(binding.address)}`),
          `${lesson.id} must bind ${binding.symbol} to its declared runtime address`);
      }
      const mode = lesson.experiment.kind.startsWith("mv_") ? "Manual" : "Auto";
      const primaryValue = (binding: any, role: string) => role === "SP" && lesson.experiment.kind === "setpoint_step"
        ? lesson.experiment.baseline : role === "MV" && lesson.experiment.kind.startsWith("mv_")
          ? lesson.experiment.baseline : (binding.range.minimum + binding.range.maximum) / 2;
      const sample = {
        availability: "available", source: "Simulation", loopId: loop.id, loopRevision: loop.revision,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode },
        scan: 1, observedAtMs: 1,
        setpoint: { ...loop.signals.setpoint, value: primaryValue(loop.signals.setpoint, "SP"), quality: "good" },
        processVariable: { ...loop.signals.processVariable,
          value: primaryValue(loop.signals.processVariable, "PV"), quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable,
          value: primaryValue(loop.signals.manipulatedVariable, "MV"), quality: "good" },
        outputLimits: { minimum: loop.limits.outputMinimum, maximum: loop.limits.outputMaximum },
        rateLimits: {}, saturation: "none", recording: false,
      };
      const plan = experimentApi.buildControlExperimentPlan({ loop, sample,
        kind: loop.experiment.kind, amplitude: loop.experiment.amplitude,
        durationSeconds: loop.experiment.durationSeconds, seed: loop.experiment.seed });
      assert.strictEqual(plan.kind, "accepted", `${lesson.id} Test defaults rejected: ${plan.reason ?? "unknown"}`);
    }
  });

  test("binds every production-quality PID lesson without a second workflow", () => {
    assertControlLessonModuleBindings("03-production-quality-digital-pid", 5);
  });

  test("binds controller-selection and plant-model lessons without fake closed-loop signals", () => {
    assertControlLessonModuleBindings("04-controller-selection-and-plant-models", 6);
  });

  test("binds every experiment and identification lesson to its reviewed process input", () => {
    assertControlLessonModuleBindings("05-experiments-identification-and-validation", 5);
  });

  test("binds every PID tuning lesson to one visible reviewed experiment command", () => {
    assertControlLessonModuleBindings("06-pid-tuning-methods", 3);
  });

  test("binds every classical-analysis lesson to its physical signals and experiment", () => {
    assertControlLessonModuleBindings("07-classical-stability-and-frequency-design", 6);
  });

  test("binds tuning qualification and automation lessons to their reviewed evidence", () => {
    assertControlLessonModuleBindings("08-tuning-qualification-and-automation", 2);
  });

  test("binds every industrial-control lesson to its production signal structure", () => {
    assertControlLessonModuleBindings("09-industrial-control-structures", 8);
  });

  test("binds every multivariable lesson to named input-output channels", () => {
    assertControlLessonModuleBindings("10-multivariable-control", 3);
  });

  test("binds every state-space and predictive lesson to executable signals", () => {
    assertControlLessonModuleBindings("11-state-space-estimation-and-predictive-control", 5);
  });

  test("binds every nonlinear adaptive and robust lesson to executable signals", () => {
    assertControlLessonModuleBindings("12-nonlinear-adaptive-and-robust-control", 4);
  });

  test("binds every networked fault-aware and diagnostic lesson to executable signals", () => {
    assertControlLessonModuleBindings("13-networked-fault-aware-and-diagnostic-control", 3);
  });

  test("binds every capstone to its complete production-facing loop", () => {
    assertControlLessonModuleBindings("14-integrated-capstones", 9);
  });

  test("semantically validates every shipped control lesson loop", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const controlRoot = path.join(repositoryRoot, "examples", "control");
    const api = require("../../controlStudio/loopDefinition") as {
      validateControlLoopDefinition(
        value: unknown,
        scope: { projectId: string; runtimeId: string }
      ): { kind: string; reason?: string; signalIds?: string[] };
    };
    const loopPaths: string[] = [];
    const visit = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const entryPath = path.join(directory, entry.name);
        if (entry.isDirectory()) visit(entryPath);
        else if (entry.name === "control-loop.json") loopPaths.push(entryPath);
      }
    };
    visit(controlRoot);
    assert.strictEqual(loopPaths.length, 84);

    for (const loopPath of loopPaths.sort()) {
      const rawLoop = JSON.parse(fs.readFileSync(loopPath, "utf8"));
      const result = api.validateControlLoopDefinition(rawLoop, {
        projectId: rawLoop.projectId,
        runtimeId: rawLoop.runtimeId,
      });
      assert.strictEqual(
        result.kind,
        "accepted",
        `${path.relative(controlRoot, loopPath)} rejected: ${result.reason ?? "unknown"} ${result.signalIds?.join(",") ?? ""}`
      );
    }
  });
});

function assertControlLessonModuleBindings(moduleName: string, expectedCount: number): void {
  const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
    ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
    : path.resolve(__dirname, "..", "..", "..", "..", "..");
  const moduleRoot = path.join(repositoryRoot, "examples", "control", moduleName);
  const api = require("../../controlStudio/loopDefinition") as {
    decodeControlLoopDefinition(value: unknown): { kind: string; loop?: any };
  };
  const directories = fs.readdirSync(moduleRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(moduleRoot, entry.name))
    .filter((directory) => fs.existsSync(path.join(directory, "lesson.json")));
  assert.strictEqual(directories.length, expectedCount);
  for (const directory of directories) {
    const lesson = JSON.parse(fs.readFileSync(path.join(directory, "lesson.json"), "utf8"));
    const loopPath = path.join(directory, "control-loop.json");
    assert.ok(fs.existsSync(loopPath), `${lesson.id} must ship control-loop.json`);
    const decoded = api.decodeControlLoopDefinition(JSON.parse(fs.readFileSync(loopPath, "utf8")));
    assert.strictEqual(decoded.kind, "accepted", `${lesson.id} loop must decode`);
    const loop = decoded.loop;
    assert.strictEqual(loop.projectId, `project:${lesson.id}`);
    assert.strictEqual(loop.runtimeId, "simulator");
    assert.strictEqual(loop.experiment.durationSeconds, lesson.experiment.durationSeconds);
    assert.strictEqual(loop.experiment.seed, lesson.experiment.seed);
    if (loop.experiment.activation) {
      assert.ok(
        lesson.workflowStages.includes("Test"),
        `${lesson.id} must teach Test because Test owns its reviewed experiment authority`
      );
    }
    const source = fs.readdirSync(path.join(directory, "src"))
      .filter((name) => name.endsWith(".st"))
      .map((name) => fs.readFileSync(path.join(directory, "src", name), "utf8")).join("\n");
    for (const binding of [loop.signals.setpoint, loop.signals.processVariable,
      loop.signals.manipulatedVariable, ...(loop.signals.additional ?? []), loop.experiment?.activation]) {
      if (!binding) continue;
      assert.match(source, new RegExp(`${escapeRegExp(binding.symbol)}\\s+AT\\s+${escapeRegExp(binding.address)}`),
        `${lesson.id} must bind ${binding.symbol} to its declared runtime address`);
    }
  }
}

/** Convert authored Markdown/LaTeX notation to stable text for semantic prose assertions. */
function pedagogicalText(source: string): string {
  return source
    .replace(/\$+/g, "")
    .replace(/\\(?:,|;|!)/g, " ")
    .replace(/\\mathrm\{([^}]*)\}/g, "$1")
    .replace(/\\operatorname\{([^}]*)\}/g, "$1")
    .replace(/([eu])_\{(k(?:-1)?)\}/g, "$1[$2]")
    .replace(/\\Delta\s*e_\{(k(?:-1)?)\}/g, "Δe[$1]")
    .replace(/_\{([^}]*)\}/g, "_$1")
    .replace(/\^\{([^}]*)\}/g, "^$1")
    .replace(/\\frac\{([^{}]+)\}\{([^{}]+)\}/g, "$1 / $2")
    .replace(/K_([pid])\b/g, "K$1")
    .replace(/\\times|\\cdot/g, "×")
    .replace(/\\sqrt\{([^{}]+)\}/g, "sqrt($1)")
    .replace(/\\sqrt/g, "sqrt")
    .replace(/\\left|\\right/g, "")
    .replace(/\\omega/g, "ω")
    .replace(/\\zeta/g, "ζ")
    .replace(/\\tau/g, "τ")
    .replace(/\\Delta/g, "Δ")
    .replace(/Δ\s+/g, "Δ")
    .replace(/[{}\\]/g, "")
    .replace(/[*`]/g, "")
    .replace(/\s+/g, " ");
}

function instructionalCoreWordCount(source: string): number {
  const workflowStart = source.indexOf("### Control Studio actions");
  const nextLessonSection = source.indexOf("\n## Evidence and decision", workflowStart);
  const core = workflowStart >= 0 && nextLessonSection > workflowStart
    ? `${source.slice(0, workflowStart)}${source.slice(nextLessonSection)}`
    : source;
  const prose = core.replace(/^#{1,6}\s+.+$/gm, "");
  return prose.trim().split(/\s+/).length;
}

function expectRejected(mutate: (lesson: any) => void, reason: string): void {
  const lesson = completeLesson() as any;
  mutate(lesson);
  assert.throws(
    () => parseControlLesson(lesson, "control-test-heater"),
    new RegExp(`control lesson 'control-test-heater' ${reason.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`)
  );
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function packagedLessonPath(level: string, lesson: string): string {
  const extensionRoot = process.env.TRUST_REPOSITORY_ROOT
    ? path.join(path.resolve(process.env.TRUST_REPOSITORY_ROOT), "editors", "vscode")
    : path.resolve(__dirname, "..", "..", "..");
  return path.join(extensionRoot, "media", "examples", "control", level, lesson, "lesson.json");
}

function completeLesson(): Record<string, unknown> {
  return {
    schemaVersion: 3,
    id: "control-test-heater",
    title: "Test heater",
    level: "Foundation",
    sequence: 1,
    equipment: "Recirculating water heater",
    physicalPurpose: "Maintain outlet-water temperature under flow changes.",
    principalTechnique: "Proportional feedback",
    prerequisites: [],
    signals: [
      { role: "SP", name: "Temperature setpoint", unit: "degC", minimum: 10, maximum: 80,
        resolution: 0.1, binding: "%ML0" },
      { role: "PV", name: "Outlet temperature", unit: "degC", minimum: 0, maximum: 100,
        resolution: 0.1, binding: "%IL0" },
      { role: "MV", name: "Heater power", unit: "%", minimum: 0, maximum: 100,
        resolution: 0.1, binding: "%QL0" },
      { role: "disturbance", name: "Flow disturbance", unit: "%", minimum: 0, maximum: 100,
        resolution: 0.1, binding: "%QL8" },
    ],
    plant: {
      modelId: "first_order_dead_time",
      executable: "simulation.toml#plant",
      parameters: [{ name: "gain", value: 0.8, unit: "degC/%" }],
      sampleTimeSeconds: 0.1,
      initialState: { outletTemperature: 20 },
      seed: 1,
      operatingPoint: { PV: 20, MV: 0 },
      validityRange: [{ signal: "PV", minimum: 10, maximum: 80, unit: "degC" }],
      simplifications: ["constant flow"],
    },
    baseline: { executableId: "HeaterP", settings: { proportionalGain: 2 },
      limitation: "The outlet settles below the setpoint." },
    change: { executableId: "HeaterPI", settings: { proportionalGain: 2, integralGain: 0.4 },
      decision: "Add integral action." },
    experiment: { kind: "load_step", signal: "Flow disturbance", baseline: 0, amplitude: 1,
      durationSeconds: 30, settlingSeconds: 10, disturbances: [], seed: 1, restorationValue: 0 },
    acceptance: [{ metric: "steady_state_error", signal: "PV", window: "last 5 s",
      comparator: "<=", limit: 0.2, unit: "degC", tolerance: 0.01, evidenceClass: "simulation" }],
    safety: { hardLimits: [{ signal: "PV", minimum: 0, maximum: 100, unit: "degC" }],
      abortPredicates: ["PV outside hard limit"], safeMv: 0, restorationMode: "Manual",
      physicalApprovalRole: "controls engineer", simulationOnlyRestrictions: [] },
    workflowStages: ["Monitor", "Test", "Validate"],
    proof: { specification: "docs/specs/38-control-engineering-curriculum.md#51-feedback-foundations",
      nativeTest: "control_test_heater", nativeCommand: "cargo test control_test_heater",
      projectPath: "examples/control/test-heater", runArtifactFields: ["samples"],
      renderedJourneyId: "control-test-heater" },
    video: { viewerOutcome: "Explain proportional offset.", requiredVisibleEvidence: ["SP", "PV", "MV"],
      durationClass: "standard", scriptPath: "video/script.md", captionPath: "video/captions.en.vtt",
      thumbnailPath: "video/thumbnail.svg", publicationState: "not_started" },
  };
}

function pedagogicalLesson(): Record<string, unknown> {
  return {
    ...completeLesson(),
    schemaVersion: 4,
    objective: "Explain the loop, predict its response, and perform a bounded experiment.",
    nextLesson: "control-test-heater-next",
    learning: {
      context: "A recirculating heater must maintain outlet temperature under changing flow.",
      reason: "Observation alone cannot regulate the outlet temperature.",
      theory: ["Identify SP, PV, MV, disturbance, feedback direction, limits, and safe state."],
      workedPrediction: "Increasing heater power must raise PV after the plant delay.",
      practicalPreparation: ["Confirm units, initial state, operating region, abort, and restoration."],
      baseline: "Observe the plant in manual mode before enabling feedback.",
      change: "Apply one bounded heater-power change.",
      comparison: "Compare the prediction with the recorded delayed response.",
      validation: "Verify finite signals, declared limits, and safe restoration.",
      explanation: "Explain each loop element and the direction of the response.",
      transfer: "Identify the same elements in a different temperature loop.",
    },
  };
}
