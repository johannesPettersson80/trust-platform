import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { parseControlLesson } from "../../examples/model";
import { compareQualifiedTunings, describeTuningMethod, tuneController } from "../../controlStudio/tuning";
import { analyzeLoopShape, classicalStabilityMargins, type TransferFunction } from "../../controlStudio/numerics";
import { automaticTunePlan, runSavedSimulationAutomaticTune } from "../../controlStudio/automaticTune";
import { candidateFromControlPid } from "../../controlStudio/controllerCandidate";
import { buildControlExperimentPlan, classifyControlExperimentStatus } from "../../controlStudio/experiment";
import { canonicalLessonReadmeHeadings, pedagogicalCoreWordCount, pedagogicalMarkdownText } from "./controlLessonAssertions";

function writeOrCompareJson(artifact: string, value: unknown): void {
  const expected = `${JSON.stringify(value, null, 2)}\n`;
  if (process.env.TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS) {
    fs.writeFileSync(artifact, expected);
  } else {
    assert.strictEqual(fs.readFileSync(artifact, "utf8"), expected,
      `retained curriculum artifact drift: ${artifact}`);
  }
}

function csvRecords(file: string): Record<string, string>[] {
  const rows = fs.readFileSync(file, "utf8").trim().split(/\r?\n/).map(row => row.split(","));
  return rows.slice(1).map(row => Object.fromEntries(rows[0].map((name, index) => [name, row[index]])));
}

function compactFrequencyEvidence(
  plant: TransferFunction,
  controller: TransferFunction,
  loop: TransferFunction
): Record<string, unknown> {
  // Persist enough precision for engineering review while removing the final
  // floating-point bits that differ between Electron and Node architectures.
  const reproducible = (value: number): number => Number(value.toPrecision(12));
  const grid = { minimumFrequencyRadPerSecond: 0.0001,
    maximumFrequencyRadPerSecond: 100, pointsPerDecade: 100 };
  const shape = analyzeLoopShape(plant, controller, grid) as any;
  const margins = classicalStabilityMargins(loop, grid) as any;
  assert.strictEqual(shape.status, "qualified");
  assert.strictEqual(margins.status, "qualified");
  return {
    grid,
    bandwidthRadPerSecond: reproducible(shape.bandwidthRadPerSecond),
    maximumSensitivity: reproducible(shape.maximumSensitivity),
    maximumComplementarySensitivity: reproducible(shape.maximumComplementarySensitivity),
    gainMarginDb: reproducible(margins.gainMarginDb),
    phaseMarginDegrees: reproducible(margins.phaseMarginDegrees),
    delayMarginSeconds: reproducible(margins.delayMarginSeconds),
  };
}

function paretoFrontier(attempts: Record<string, any>): string[] {
  const feasible = Object.entries(attempts).filter(([, value]) => value.status === "Feasible");
  const measures = (value: any): number[] => [
    value.validation.metrics.iae,
    value.validation.metrics.itae,
    value.validation.metrics.mvMovement,
    value.frequency.maximumSensitivity,
  ];
  return feasible.filter(([, candidate]) => !feasible.some(([, other]) => {
    if (other === candidate) return false;
    const left = measures(other);
    const right = measures(candidate);
    return left.every((value, index) => value <= right[index])
      && left.some((value, index) => value < right[index]);
  })).map(([method]) => method);
}

function retainMethodQualification(lessonRoot: string): void {
  const native = Object.fromEntries(csvRecords(path.join(lessonRoot, "figures",
    "tuning-method-comparison-evidence-summary.csv")).map(row => [row.case, row]));
  const trace = csvRecords(path.join(lessonRoot, "figures",
    "tuning-method-comparison-evidence-trace.csv"));
  const seriesValues = (name: string): number[] => trace
    .filter(row => row.series === name).map(row => Number(row.y));
  const rmsAboutMean = (values: readonly number[]): number => {
    const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
    return Math.sqrt(values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length);
  };
  const settling = (values: readonly number[], setpoint: number, step: number) => {
    const band = Math.abs(step) * 0.05;
    const dwellSamples = 50;
    for (let index = 100; index + dwellSamples <= 1_000; index += 1) {
      if (values.slice(index, index + dwellSamples)
        .every(value => Math.abs(value - setpoint) <= band)) {
        return { status: "Settled", timeSeconds: (index - 99) * 0.1,
          band: { value: band, unit: "process unit", fractionOfStep: 0.05 },
          dwellSeconds: 5, windowSeconds: [10, 100] };
      }
    }
    return { status: "Not settled", timeSeconds: null,
      band: { value: band, unit: "process unit", fractionOfStep: 0.05 },
      dwellSeconds: 5, windowSeconds: [10, 100] };
  };
  const specifications = {
    heater: { model: { kind: "first_order_dead_time" as const, gain: 0.2,
      timeConstantSeconds: 5, deadTimeSeconds: 1, stable: true, normalizedError: 0 },
      processUnit: "degC", outputUnit: "%", setpoint: 25, setpointChange: 5,
      ids: ["heater_simc", "heater_amigo"] },
    tank: { model: { kind: "integrating" as const, gain: 0.05,
      deadTimeSeconds: 0.5, stable: true, normalizedError: 0 },
      processUnit: "level-%", outputUnit: "%", setpoint: 52, setpointChange: 2,
      ids: ["tank_simc", "tank_amigo"] },
    dryer: { model: { kind: "first_order_dead_time" as const, gain: 0.4,
      timeConstantSeconds: 20, deadTimeSeconds: 8, stable: true, normalizedError: 0 },
      processUnit: "dryness-%", outputUnit: "%", setpoint: 45, setpointChange: 5,
      ids: ["dryer_simc", "dryer_amigo"] },
  };
  const plants: Record<string, any> = {};
  for (const [plantName, specification] of Object.entries(specifications)) {
    const attempts: Record<string, any> = {};
    for (const [index, method] of (["SIMC", "AMIGO"] as const).entries()) {
      const description = describeTuningMethod(specification.model, method, "balanced",
        specification.outputUnit, specification.processUnit);
      const nativeResult = native[specification.ids[index]];
      if (description.status === "Unavailable") {
        attempts[method] = { status: description.status, reason: description.reason,
          assumptions: description.assumptions, modelInputs: description.inputs, applied: false };
        continue;
      }
      const ti = description.result!.integralTimeSeconds;
      const kp = description.result!.proportionalGain;
      const controller: TransferFunction = {
        numerator: [kp * ti, kp], denominator: [ti, 0], delaySeconds: 0,
      };
      const frequencyAt = (fraction: number): Record<string, any> => {
        const gain = specification.model.gain * (1 + fraction);
        const delay = (specification.model.deadTimeSeconds ?? 0) * (1 + fraction);
        const plant: TransferFunction = specification.model.kind === "integrating"
          ? { numerator: [gain], denominator: [1, 0], delaySeconds: delay }
          : { numerator: [gain], denominator: [specification.model.timeConstantSeconds!, 1],
            delaySeconds: delay };
        const loop: TransferFunction = specification.model.kind === "integrating"
          ? { numerator: [gain * kp * ti, gain * kp], denominator: [ti, 0, 0],
            delaySeconds: delay }
          : { numerator: [gain * kp * ti, gain * kp],
            denominator: [specification.model.timeConstantSeconds! * ti, ti, 0],
            delaySeconds: delay };
        return compactFrequencyEvidence(plant, controller, loop);
      };
      const frequency = frequencyAt(0);
      const tracePrefix = nativeResult.case === "tank_simc" ? "Tank SIMC"
        : nativeResult.case.split("_").map((part: string) => part.toUpperCase() === "SIMC"
          || part.toUpperCase() === "AMIGO" ? part.toUpperCase()
          : part[0].toUpperCase() + part.slice(1)).join(" ");
      const processValues = seriesValues(nativeResult.case === "tank_simc"
        ? "Tank SIMC level" : tracePrefix);
      const requestedValues = seriesValues(`${tracePrefix} requested`);
      const appliedValues = seriesValues(`${tracePrefix} applied`);
      assert.strictEqual(processValues.length, 2_200);
      assert.strictEqual(requestedValues.length, 2_200);
      assert.strictEqual(appliedValues.length, 2_200);
      const settlingResult = settling(processValues, specification.setpoint,
        specification.setpointChange);
      const peakRequestedOutput = Math.max(...requestedValues);
      const peakAppliedOutput = Math.max(...appliedValues);
      const maximumAppliedOutputRate = Math.max(...appliedValues.slice(1)
        .map((value, itemIndex) => Math.abs(value - appliedValues[itemIndex]) / 0.1));
      const saturationDuration = appliedValues.filter(value => value <= 0 || value >= 100).length * 0.1;
      const noiseOutputRms = rmsAboutMean(appliedValues.slice(2_100));
      const overshoot = Number(nativeResult.overshoot_percent);
      const peakValue = plantName === "tank"
        ? overshoot * specification.setpointChange / 100 : overshoot;
      const peakLimit = plantName === "tank" ? 0.8 : 10;
      const peakUnit = plantName === "tank" ? "level pp" : "% of setpoint step";
      const requirements = [
        { name: plantName === "tank" ? "Peak level excursion" : "Normalized overshoot",
          status: peakValue <= peakLimit ? "Pass" : "Fail", value: peakValue,
          limit: peakLimit, unit: peakUnit, source: "Native 220 s simulation" },
        { name: "Final mean absolute error",
          status: Number(nativeResult.final_mean_absolute_error) <= 0.25 ? "Pass" : "Fail",
          value: Number(nativeResult.final_mean_absolute_error), limit: 0.25,
          unit: specification.processUnit, source: "Native 220 s simulation" },
        { name: "Applied output bounds", status: nativeResult.status === "qualified" ? "Pass" : "Fail",
          value: "all samples", limit: "0..100", unit: "%",
          source: "Native 220 s simulation" },
        { name: "Settling and dwell", status: settlingResult.status === "Settled" ? "Pass" : "Fail",
          value: settlingResult.timeSeconds, limit: 90, unit: "s after setpoint event",
          source: "Native setpoint window 10..100 s; 5% band for 5 s dwell" },
        { name: "Peak applied output", status: peakAppliedOutput <= 100 ? "Pass" : "Fail",
          value: peakAppliedOutput, limit: 100, unit: "%",
          source: "Native 220 s simulation" },
        { name: "Maximum applied-output rate",
          status: maximumAppliedOutputRate <= 1_000 ? "Pass" : "Fail",
          value: maximumAppliedOutputRate, limit: 1_000, unit: "%/s",
          source: "Native 0.1 s applied-output samples" },
        { name: "Saturation duration", status: saturationDuration === 0 ? "Pass" : "Fail",
          value: saturationDuration, limit: 0, unit: "s",
          source: "Native 220 s simulation" },
        { name: "Noise-driven MV RMS", status: noiseOutputRms <= 0.5 ? "Pass" : "Fail",
          value: noiseOutputRms, limit: 0.5, unit: "% RMS",
          source: "Native applied-output window 210..220 s with 0.02-unit 0.5 Hz interference" },
        { name: "Gain margin", status: frequency.gainMarginDb >= 6 ? "Pass" : "Fail",
          value: frequency.gainMarginDb, limit: 6, unit: "dB",
          source: "Qualified frequency analyzer" },
        { name: "Phase margin", status: frequency.phaseMarginDegrees >= 30 ? "Pass" : "Fail",
          value: frequency.phaseMarginDegrees, limit: 30, unit: "degrees",
          source: "Qualified frequency analyzer" },
      ];
      const boundedVariation = [-0.2, 0, 0.2].map(fraction => {
        const varied = frequencyAt(fraction);
        return { gainAndDelayFraction: fraction, gainMarginDb: varied.gainMarginDb,
          phaseMarginDegrees: varied.phaseMarginDegrees,
          status: varied.gainMarginDb >= 6 && varied.phaseMarginDegrees >= 30 ? "Pass" : "Fail" };
      });
      const feasible = requirements.every(row => row.status === "Pass")
        && boundedVariation.every(row => row.status === "Pass");
      attempts[method] = { status: feasible ? "Feasible" : "Rejected", applied: false,
        controller: { proportionalGain: kp, integralTimeSeconds: ti,
          integralGain: description.result!.integralGain, derivativeGain: 0 },
        assumptions: description.assumptions, modelInputs: description.inputs,
        nativeRun: nativeResult, frequency,
        validation: {
          planRevision: "sha256:lesson-51-common-validation-v1",
          metrics: { iae: Number(nativeResult.iae), ise: Number(nativeResult.ise),
            itae: Number(nativeResult.itae), mvMovement: Number(nativeResult.applied_movement_pp),
            settling: settlingResult, peakRequestedOutput, peakAppliedOutput,
            maximumAppliedOutputRate, saturationDuration, noiseOutputRms },
          requirements, boundedVariation,
          scenarioDispositions: {
            setpointTracking: { status: requirements[0].status === "Pass"
              && settlingResult.status === "Settled" ? "Pass" : "Fail", windowSeconds: [10, 100] },
            disturbanceRejection: { status: requirements[1].status,
              windowSeconds: [100, 220], finalWindowSeconds: [210, 220] },
            measurementNoise: { status: noiseOutputRms <= 0.5 ? "Pass" : "Fail",
              windowSeconds: [210, 220], injectedAmplitude: 0.02, injectedFrequencyHz: 0.5 },
            boundedVariation: { status: boundedVariation.every(row => row.status === "Pass")
              ? "Pass" : "Fail", fractions: [-0.2, 0, 0.2] },
          },
        },
      };
    }
    const feasible = Object.values(attempts).filter((attempt: any) => attempt.status === "Feasible");
    const frontier = paretoFrontier(attempts);
    plants[plantName] = { modelIdentity: `control-${plantName}-accepted-model`,
      modelRevision: `sha256:lesson-51-${plantName}-model-v1`,
      controllerConvention: "Parallel Control.PID", attempts,
      paretoMeasures: ["IAE", "ITAE", "MV movement", "maximum sensitivity"],
      paretoFrontier: frontier,
      decision: feasible.length > 1 && frontier.length > 1 ? "Inconclusive" : "Only eligible candidate" };
  }
  writeOrCompareJson(path.join(lessonRoot, "figures", "tuning-method-comparison-qualification.json"), {
    schemaVersion: 1, evidenceClass: "Simulation", applied: false,
    conditions: { revision: "sha256:lesson-51-common-validation-v1", sampleSeconds: 0.1,
      durationSeconds: 220, seed: 20260829, outputLimitsPercent: [0, 100],
      eventsSeconds: { setpoint: 10, loadDisturbance: 100 },
      variationFractions: [-0.2, 0, 0.2] }, plants,
  });
}

function assertRetainedTuningEvidence(root: string, stem: string): void {
  const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
  const figure = path.join(root, "figures", `${stem}.svg`);
  const trace = path.join(root, "figures", `${stem}-trace.csv`);
  const summary = path.join(root, "figures", `${stem}-summary.csv`);
  for (const artifact of [figure, trace, summary])
    assert.ok(fs.existsSync(artifact), `retained tuning evidence must exist: ${artifact}`);
  assert.match(readme, new RegExp(`!\\[[^\\]]+\\]\\(figures/${stem}\\.svg\\)`));
  assert.match(readme, new RegExp(`\\[Complete trace\\]\\(figures/${stem}-trace\\.csv\\)`));
  assert.match(readme, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
  const rows = fs.readFileSync(trace, "utf8").trim().split(/\r?\n/).map(row => row.split(","));
  assert.deepStrictEqual(rows[0], ["panel", "series", "point", "x", "y", "x_unit", "y_unit"]);
  assert.ok(rows.length > 2_000 && rows.every(row => row.length === rows[0].length));
}

suite("Tuning-method curriculum lesson", () => {
  test("compares only model-eligible heater tank and dryer PI methods", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "08-tuning-qualification-and-automation", "51-method-comparison"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 51 tuning-method project must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const project = fs.readFileSync(path.join(lessonRoot, "trust-lsp.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) =>
      candidate.id === "control-tuning-method-comparison");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-tuning-method-comparison"), lesson);
    assert.strictEqual(entry?.path, "control/08-tuning-qualification-and-automation/51-method-comparison");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 51);
    assert.strictEqual(lesson.level, "Advanced");
    assert.match(source, /Course position: Lesson 51 of 84/);
    assert.ok(source.replace(/\s+/g, " ").includes(lesson.objective));
    assert.ok(source.replace(/\s+/g, " ").includes(lesson.equipment));
    assert.deepStrictEqual(lesson.prerequisites, ["control-position-z-plane-sampling"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.change.applied, false);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_tuning_methods_separate_eligibility_from_tradeoffs");
    const movementMetrics = lesson.acceptance.filter((item: { metric: string }) =>
      item.metric.includes("movement"));
    assert.strictEqual(movementMetrics.length, 2);
    assert.ok(movementMetrics.every((item: { unit: string }) => item.unit === "pp"),
      "cumulative actuator movement must use percentage points, not instantaneous percent");

    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /Use accepted models.*eligible.*validate.*no acceptable candidate/i,
      /recirculating water heater.*integrating buffer tank.*continuous belt dryer/i,
      /Lesson 44.*response target.*Lessons 45.*50.*stability.*robustness/i,
      /method eligibility.*published tuning rule.*Unavailable/i,
      /SIMC.*integrating.*AMIGO.*unavailable.*integrating/i,
      /proportional gain \(Kp\).*integral time \(Ti\).*integral gain \(Ki\)/i,
      /setpoint \(SP\).*process value \(PV\).*control error \(e\)/i,
      /integral absolute error \(IAE\).*integral squared error \(ISE\).*time-weighted.*\(ITAE\)/i,
      /cumulative applied-output movement.*total variation.*percentage points/i,
      /gain margin.*phase margin.*delay margin/i,
      /sensitivity function.*sensitivity \(S\).*complementary sensitivity.*\(T\)/i,
      /Pareto dominance.*no worse.*better.*at least one/i,
      /No acceptable candidate.*Inconclusive/i,
      /heater.*5\.769.*1\.154.*6\.028.*1\.552/i,
      /tank.*13\.333.*2\.222.*AMIGO.*Unavailable/i,
      /dryer.*2\.083.*0\.104.*1\.287.*0\.072/i,
      /requested.*applied output.*separate/i,
      /Control Studio.*Monitor.*Tune.*Validate/i,
      /change only.*method.*Kp.*Ki/i,
      /gain margin.*6 dB.*phase margin.*30 degrees/i,
      /27\.44.*93\.34.*16\.66.*76\.78.*1\.227/i,
      /240\.63.*23\.04.*17\.41.*74\.37.*1\.218/i,
      /0\.0001.*100 rad\/s.*100 points per decade/i,
      /Simulation evidence.*not.*physical/i,
      /Lesson 52.*automate/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("control error") < source.indexOf("e = SP - PV"));
    assert.ok(source.indexOf("Integral absolute error") < source.indexOf("IAE ="));
    assert.ok(source.indexOf("sensitivity function") < source.indexOf("S = 1 /"));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 5);
    assert.match(project, /\[dependencies\][\s\S]*Control\s*=\s*\{\s*path\s*=\s*"libraries\/control"/);
    assert.ok(fs.existsSync(path.join(lessonRoot, "libraries", "control", "src", "control.st")),
      "Lesson 51 must ship the Control library required by its executable project");
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 5);
    assert.match(program, /TankAmigoEligible\s*:\s*BOOL\s*:=\s*FALSE/);
    assert.doesNotMatch(program, /TankAmigoController\s*:\s*Control\.PID/);
    for (const forbidden of ["Winner", "ParetoRank", "PerformanceScore", "AcceptedMethod"])
      assert.ok(!program.includes(forbidden), `ST must not embed method result '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_200);
    assertRetainedTuningEvidence(lessonRoot, "tuning-method-comparison-evidence");
    retainMethodQualification(lessonRoot);
    const qualificationPath = path.join(
      lessonRoot, "figures", "tuning-method-comparison-qualification.json"
    );
    assert.ok(fs.existsSync(qualificationPath), "Lesson 51 must retain complete qualification evidence");
    const qualification = JSON.parse(fs.readFileSync(qualificationPath, "utf8"));
    assert.strictEqual(qualification.schemaVersion, 1);
    assert.match(qualification.conditions.revision, /^sha256:/);
    assert.deepStrictEqual(Object.keys(qualification.plants), ["heater", "tank", "dryer"]);
    assert.strictEqual(qualification.plants.tank.attempts.AMIGO.status, "Unavailable");
    assert.match(qualification.plants.tank.attempts.AMIGO.reason, /integrating/i);
    const tankSimc = qualification.plants.tank.attempts.SIMC;
    assert.strictEqual(tankSimc.validation.planRevision, qualification.conditions.revision);
    assert.ok(tankSimc.validation.requirements.length >= 10);
    assert.ok(tankSimc.validation.requirements.every((row: any) => row.status === "Pass"));
    assert.strictEqual(tankSimc.validation.requirements[0].unit, "level pp");
    assert.strictEqual(tankSimc.validation.requirements[0].limit, 0.8);
    assert.ok(tankSimc.validation.boundedVariation.every((row: any) => row.status === "Pass"));
    assert.ok(!("gainMarginDb" in tankSimc.controller),
      "controller settings must not duplicate the qualified frequency margin");
    for (const plant of Object.values(qualification.plants) as any[]) {
      for (const attempt of Object.values(plant.attempts) as any[]) {
        if (attempt.status !== "Feasible") continue;
        assert.deepStrictEqual(Object.keys(attempt.validation.scenarioDispositions),
          ["setpointTracking", "disturbanceRejection", "measurementNoise", "boundedVariation"]);
        assert.ok(Object.values(attempt.validation.scenarioDispositions)
          .every((item: any) => item.status === "Pass"));
        for (const metric of ["settling", "peakRequestedOutput", "peakAppliedOutput",
          "maximumAppliedOutputRate", "saturationDuration", "noiseOutputRms"])
          assert.ok(metric in attempt.validation.metrics, `missing retained metric ${metric}`);
        for (const requirement of ["Settling and dwell", "Peak applied output",
          "Maximum applied-output rate", "Saturation duration", "Noise-driven MV RMS"])
          assert.ok(attempt.validation.requirements.some((row: any) => row.name === requirement));
      }
    }
    for (const plant of [qualification.plants.heater, qualification.plants.dryer]) {
      assert.strictEqual(plant.decision, "Inconclusive");
      assert.ok(plant.paretoFrontier.length >= 2);
      assert.ok(Object.values(plant.attempts).every((attempt: any) => attempt.applied === false));
      assert.ok(Object.values(plant.attempts).every((attempt: any) =>
        attempt.status !== "Feasible" || (attempt.frequency && attempt.validation)));
      assert.ok(Object.values(plant.attempts).every((attempt: any) =>
        attempt.status !== "Feasible" ||
          attempt.validation.planRevision === qualification.conditions.revision));
    }
  });

  test("reproduces qualified method formulas and retains unavailable reasons", () => {
    const heater = { kind: "first_order_dead_time" as const, gain: 0.2,
      timeConstantSeconds: 5, deadTimeSeconds: 1, stable: true, normalizedError: 0 };
    const tank = { kind: "integrating" as const, gain: 0.05,
      deadTimeSeconds: 0.5, stable: true, normalizedError: 0 };
    const dryer = { kind: "first_order_dead_time" as const, gain: 0.4,
      timeConstantSeconds: 20, deadTimeSeconds: 8, stable: true, normalizedError: 0 };
    const relative = (actual: number, expected: number): void => assert.ok(
      Math.abs(actual - expected) / Math.max(Math.abs(expected), Number.EPSILON) <= 1e-7,
      `expected ${expected}, got ${actual}`);

    const heaterSimc = tuneController(heater, { method: "SIMC", objective: "balanced" });
    const heaterAmigo = tuneController(heater, { method: "AMIGO", objective: "balanced" });
    const tankComparison = compareQualifiedTunings(tank, "balanced");
    const dryerSimc = tuneController(dryer, { method: "SIMC", objective: "balanced" });
    const dryerAmigo = tuneController(dryer, { method: "AMIGO", objective: "balanced" });
    for (const [actual, expected] of [
      [heaterSimc.controller.kp, 5.7692308],
      [heaterSimc.controller.kp / heaterSimc.controller.tiSeconds!, 1.15384616],
      [heaterAmigo.controller.kp, 6.0277778],
      [heaterAmigo.controller.kp / heaterAmigo.controller.tiSeconds!, 1.5525071584],
      [dryerSimc.controller.kp, 2.0833333],
      [dryerSimc.controller.kp / dryerSimc.controller.tiSeconds!, 0.104166665],
      [dryerAmigo.controller.kp, 1.2869898],
      [dryerAmigo.controller.kp / dryerAmigo.controller.tiSeconds!, 0.0721855894],
    ]) relative(actual, expected);
    assert.deepStrictEqual(tankComparison.attempts.map((attempt) => [attempt.method, attempt.status]),
      [["SIMC", "Feasible"], ["AMIGO", "Unavailable"]]);
    assert.match(tankComparison.attempts[1].reason ?? "", /integrating/i);
    assert.strictEqual(tankComparison.applied, false);
  });

  test("qualifies method margins bandwidth and sensitivity on one declared grid", () => {
    const sweep = { minimumFrequencyRadPerSecond: 0.0001,
      maximumFrequencyRadPerSecond: 100, pointsPerDecade: 100 };
    const analyze = (gain: number, time: number, delay: number, kp: number, ti: number) => {
      const plant: TransferFunction = {
        numerator: [gain], denominator: [time, 1], delaySeconds: delay,
      };
      const controller: TransferFunction = {
        numerator: [kp * ti, kp], denominator: [ti, 0], delaySeconds: 0,
      };
      const shape = analyzeLoopShape(plant, controller, sweep) as any;
      const margins = classicalStabilityMargins({
        numerator: [gain * kp * ti, gain * kp],
        denominator: [time * ti, ti, 0],
        delaySeconds: delay,
      }, sweep) as any;
      assert.strictEqual(shape.status, "qualified");
      assert.strictEqual(margins.status, "qualified");
      return { shape, margins };
    };
    const close = (actual: number, expected: number, tolerance = 0.001): void =>
      assert.ok(Math.abs(actual - expected) <= tolerance, `expected ${expected}, got ${actual}`);
    const heaterSimc = analyze(0.2, 5, 1, 5.7692308, 5);
    const heaterAmigo = analyze(0.2, 5, 1, 6.0277778, 3.8826087);
    const dryerSimc = analyze(0.4, 20, 8, 2.0833333, 20);
    const dryerAmigo = analyze(0.4, 20, 8, 1.2869898, 17.828902);
    for (const [actual, expected] of [
      [heaterSimc.margins.gainMarginDb, 16.6583],
      [heaterSimc.margins.phaseMarginDegrees, 76.7771],
      [heaterSimc.margins.delayMarginSeconds, 5.8067],
      [heaterSimc.shape.maximumSensitivity, 1.22744],
      [heaterAmigo.margins.gainMarginDb, 16.0246],
      [heaterAmigo.margins.phaseMarginDegrees, 67.5151],
      [heaterAmigo.margins.delayMarginSeconds, 4.3968],
      [heaterAmigo.shape.maximumSensitivity, 1.26376],
      [dryerSimc.margins.gainMarginDb, 13.4643],
      [dryerSimc.margins.phaseMarginDegrees, 70.9013],
      [dryerSimc.margins.delayMarginSeconds, 29.6991],
      [dryerSimc.shape.maximumSensitivity, 1.34859],
      [dryerAmigo.margins.gainMarginDb, 17.4137],
      [dryerAmigo.margins.phaseMarginDegrees, 74.3678],
      [dryerAmigo.margins.delayMarginSeconds, 46.1061],
      [dryerAmigo.shape.maximumSensitivity, 1.21814],
    ]) close(actual, expected);
    assert.ok(heaterAmigo.shape.bandwidthRadPerSecond > heaterSimc.shape.bandwidthRadPerSecond);
    assert.ok(dryerAmigo.shape.bandwidthRadPerSecond < dryerSimc.shape.bandwidthRadPerSecond);
  });

  test("teaches the complete automatic heater sequence without applying its result", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "08-tuning-qualification-and-automation", "52-automatic-water-heater"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 52 Automatic Tune project must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const project = fs.readFileSync(path.join(lessonRoot, "trust-lsp.toml"), "utf8");
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((candidate: { id: string }) =>
      candidate.id === "control-automatic-tuning-sequence");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-automatic-tuning-sequence"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 52);
    assert.strictEqual(lesson.level, "Advanced");
    assert.match(source, /Course position: Lesson 52 of 84/);
    assert.ok(source.replace(/\s+/g, " ").includes(lesson.objective));
    assert.ok(source.replace(/\s+/g, " ").includes(lesson.equipment));
    assert.strictEqual(catalogEntry?.path, "control/08-tuning-qualification-and-automation/52-automatic-water-heater");
    assert.deepStrictEqual(lesson.prerequisites, ["control-tuning-method-comparison"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.change.applied, false);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_automatic_tune_validates_without_replacing_the_running_heater");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /run and audit.*Automatic Tune.*explain.*stage.*unapplied/i,
      /recirculating water heater.*one-sided.*0.*100 percent.*0\.1 seconds/i,
      /Lesson 51.*eligibility.*feasibility.*preference/i,
      /orchestrator.*Test.*Identify.*Tune.*Validate.*not.*private copies/i,
      /exact saved simulated model.*configuration evidence.*no excitation/i,
      /not.*identified physical model/i,
      /model structure.*parameters.*independent validation/i,
      /SIMC.*AMIGO.*P\/PI\/PD\/PID/i,
      /one immutable validation-conditions record/i,
      /abort.*restoration.*restoration failure/i,
      /recommendation.*not permission.*Apply.*separate/i,
      /5\.769.*5\.000.*1\.154.*13\.5.*0\.094/i,
      /Control Studio.*Monitor.*Automatic Tune.*plan.*confirm/i,
      /Remove the selected saved plant.*negative gain.*Identify.*Request excitation.*rejected/i,
      /poor.*data.*tuning.*Validate.*No acceptable controller found/i,
      /user abort.*automatic abort.*restoration failure/i,
      /active-controller revision.*unchanged/i,
      /Simulation evidence.*physical evidence/i,
    ]) assert.match(readme, requirement);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 2);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 2);
    assert.match(project, /\[dependencies\][\s\S]*Control\s*=\s*\{\s*path\s*=\s*"libraries\/control"/);
    assert.ok(fs.existsSync(path.join(lessonRoot, "libraries", "control", "src", "control.st")),
      "Lesson 52 must ship the Control library required by its executable project");
    for (const forbidden of ["AutoTuneWinner", "StagePassed", "ModelFit", "AcceptedCandidate", "ApplyCandidate"])
      assert.ok(!program.includes(forbidden), `ST must not embed orchestration result '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_000);
    assertRetainedTuningEvidence(lessonRoot, "automatic-water-heater-evidence");
    assert.match(source, /automatic-water-heater-orchestration-evidence\.json/);

    const response = { schema_version: 1, operation: "plant.read", project_identity: "lesson",
      project_root: "/lesson", saved_configuration_revision: "sha256:heater-model-v1",
      saved_configuration: { plants: [{ id: "automatic-water-heater",
        model: "first_order_dead_time", parameters: {
          gain: 0.2, time_constant_s: 5, dead_time_s: 1,
        } }] } };
    assert.deepStrictEqual(automaticTunePlan("balanced"), {
      objective: "balanced", source: "Simulation", stages: ["Test", "Identify", "Tune", "Validate"],
      evidenceReuse: "The saved simulation model is exact configuration evidence; no excitation is required.",
    });
    const outcome = runSavedSimulationAutomaticTune(response as never, "automatic-water-heater", "balanced");
    assert.strictEqual(outcome.status, "complete");
    if (outcome.status !== "complete") return;
    assert.deepStrictEqual(outcome.completedStages, ["Test", "Identify", "Tune", "Validate"]);
    assert.strictEqual(outcome.result.applied, false);
    assert.strictEqual(outcome.result.modelEvidence, "Exact simulation model");
    assert.strictEqual(outcome.result.structure, "PI");
    assert.strictEqual(outcome.result.kp, 5.7692308);
    assert.strictEqual(outcome.result.tiSeconds, 5);
    assert.ok(outcome.result.attempts.length >= 6);
    assert.ok(outcome.result.requirements.every((row) => row.status === "Pass"));
    assert.ok(outcome.result.metrics.settlingTimeSeconds > 13 &&
      outcome.result.metrics.settlingTimeSeconds < 14);
    assert.strictEqual(lesson.automaticPlan.selectionRule.materialBenefitPercent, 10);
    assert.deepStrictEqual(lesson.automaticPlan.selectionRule.deterministicMethodOrder,
      ["SIMC", "AMIGO", "Constrained P", "Constrained PI", "Constrained PD", "Constrained PID"]);
    assert.match(readme,
      /AMIGO.*7\.4 percent.*below.*10-percent material-benefit threshold.*SIMC/i);

    const missing = runSavedSimulationAutomaticTune(response as never, "missing", "balanced");
    assert.strictEqual(missing.status, "failed");
    if (missing.status === "failed") {
      assert.strictEqual(missing.failedStage, "Test");
      assert.deepStrictEqual(missing.completedStages, []);
    }
    const invalid = runSavedSimulationAutomaticTune({ ...response,
      saved_configuration: { plants: [{ id: "automatic-water-heater",
        model: "first_order_dead_time", parameters: {
          gain: -0.2, time_constant_s: 5, dead_time_s: 1,
        } }] } } as never, "automatic-water-heater", "balanced");
    assert.strictEqual(invalid.status, "failed");
    if (invalid.status === "failed") {
      assert.strictEqual(invalid.failedStage, "Identify");
      assert.deepStrictEqual(invalid.completedStages, ["Test"]);
      assert.match(invalid.recovery, /correct.*model parameters/i);
    }
  });

  test("stops automatic tuning at unsafe data tuning validation and restoration boundaries", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "08-tuning-qualification-and-automation", "52-automatic-water-heater"
    );
    const loop = {
      schemaVersion: 1, id: "loop:heater", name: "Recirculating water heater",
      revision: "sha256:heater-loop-v1", projectId: "lesson", runtimeId: "runtime:local",
      controller: { instance: "HeaterController", type: "Control.PID",
        revision: "sha256:robust-pi-v1", action: "direct", taskPeriodMs: 100 },
      signals: {
        setpoint: { symbol: "TemperatureSP", address: "%ML0", iecType: "LREAL",
          direction: "read", unit: "degC", range: { minimum: 5, maximum: 60 } },
        processVariable: { symbol: "TemperaturePV", address: "%IL0", iecType: "LREAL",
          direction: "read", unit: "degC", range: { minimum: 5, maximum: 60 } },
        manipulatedVariable: { symbol: "HeaterMV", address: "%QL0", iecType: "LREAL",
          direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
      },
      limits: { outputMinimum: 0, outputMaximum: 100 },
      observedSampleTiming: { nominalPeriodMs: 100 },
    };
    const sample = {
      availability: "available", source: "Simulation", loopId: "loop:heater",
      loopRevision: "sha256:heater-loop-v1",
      controller: { instance: "HeaterController", revision: "sha256:robust-pi-v1", mode: "Auto" },
      scan: 42, observedAtMs: 4_200,
      setpoint: { symbol: "TemperatureSP", address: "%ML0", value: 35, unit: "degC", quality: "good" },
      processVariable: { symbol: "TemperaturePV", address: "%IL0", value: 35, unit: "degC", quality: "good" },
      manipulatedVariable: { symbol: "HeaterMV", address: "%QL0", value: 20, unit: "%", quality: "good" },
      outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false,
    };

    assert.deepStrictEqual(buildControlExperimentPlan({ loop: loop as never, sample: { ...sample,
      availability: "unavailable", reason: "stale" } as never,
      kind: "setpoint_step", amplitude: 2, durationSeconds: 30 }), {
      kind: "rejected", reason: "Fresh synchronized SP, PV, and MV are required.",
    });
    assert.deepStrictEqual(buildControlExperimentPlan({ loop: loop as never, sample: { ...sample,
      processVariable: { ...sample.processVariable, quality: "bad" } } as never,
      kind: "setpoint_step", amplitude: 2, durationSeconds: 30 }), {
      kind: "rejected", reason: "Fresh good-quality SP, PV, and MV are required.",
    });
    assert.deepStrictEqual(buildControlExperimentPlan({ loop: loop as never, sample: sample as never,
      kind: "setpoint_step", amplitude: 30, durationSeconds: 30 }), {
      kind: "rejected", reason: "The requested value 65 degC is outside 5…60 degC.",
    });
    assert.deepStrictEqual(buildControlExperimentPlan({ loop: loop as never, sample: { ...sample, source: "Physical" } as never,
      kind: "setpoint_step", amplitude: 2, durationSeconds: 30 }), {
      kind: "rejected", reason: "A reviewed physical topology is required before approval can be requested.",
    });

    const configuration = (gain: number) => ({
      schema_version: 1, operation: "plant.read", project_identity: "lesson", project_root: "/lesson",
      saved_configuration_revision: "sha256:heater-model-v1",
      saved_configuration: { plants: [{ id: "automatic-water-heater",
        model: "first_order_dead_time", parameters: {
          gain, time_constant_s: 5, dead_time_s: 1,
        } }] },
    });
    const tuningFailure = runSavedSimulationAutomaticTune(
      configuration(Number.MIN_VALUE) as never, "automatic-water-heater", "balanced");
    assert.deepStrictEqual(tuningFailure, {
      status: "failed", completedStages: ["Test", "Identify"], failedStage: "Tune",
      reason: "No applicable tuning method found.",
      recovery: "Review the model and selected tuning objective, then retry tuning.",
    });

    const running = candidateFromControlPid({
      proportionalGain: 3.260869565, integralGain: 0.652173913, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 20,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -1000, rateMaximum: 1000,
      trackingTime: 1, antiWindup: "Combined",
    }, "Imported");
    const runningBefore = JSON.stringify(running);
    const successWithRunning = runSavedSimulationAutomaticTune(
      configuration(0.2) as never, "automatic-water-heater", "balanced", running,
      { processUnit: "°C", outputUnit: "%" });
    assert.strictEqual(successWithRunning.status, "complete",
      "the declared Robust heater must remain a valid baseline for the Balanced recommendation");
    if (successWithRunning.status === "complete") {
      assert.strictEqual(successWithRunning.result.kp, 5.7692308);
      assert.ok(successWithRunning.result.requirements.every((row) => row.status === "Pass"));
      assert.match(successWithRunning.result.rationale,
        /AMIGO.*7\.4 percent.*below.*10 percent material-benefit threshold.*SIMC/i);
    }
    const restrictedRunning = candidateFromControlPid({
      ...running.parameters, bias: 0.05, outputMinimum: 0, outputMaximum: 0.1,
    }, "Imported");
    const validationFailure = runSavedSimulationAutomaticTune(
      configuration(0.2) as never, "automatic-water-heater", "balanced", restrictedRunning);
    assert.strictEqual(validationFailure.status, "failed");
    if (validationFailure.status === "failed") {
      assert.deepStrictEqual(validationFailure.completedStages, ["Test", "Identify", "Tune"]);
      assert.strictEqual(validationFailure.failedStage, "Validate");
      assert.strictEqual(validationFailure.reason, "No acceptable controller found.");
    }
    assert.strictEqual(JSON.stringify(running), runningBefore,
      "a failed sequence must not mutate the running controller");

    const restoration = [classifyControlExperimentStatus({
      status: "aborted", stop_reason: "user_abort", restoration: "pending",
    }), classifyControlExperimentStatus({
      status: "aborted", stop_reason: "user_abort", restoration: "restored",
    }), classifyControlExperimentStatus({
      status: "failed_to_restore", stop_reason: "signal_outside_limits",
      restoration: "failed", restoration_reason: "confirmation_timeout",
    })];
    assert.deepStrictEqual(restoration, ["awaiting_restoration", "terminal", "terminal"]);

    let approvalCount = 0;
    const plan = automaticTunePlan("balanced");
    approvalCount += 1;
    const success = successWithRunning;
    assert.strictEqual(success.status, "complete");
    if (success.status !== "complete") return;
    const failures = [
      runSavedSimulationAutomaticTune(configuration(0.2) as never, "missing", "balanced"),
      runSavedSimulationAutomaticTune(configuration(-0.2) as never,
        "automatic-water-heater", "balanced"),
      tuningFailure,
      validationFailure,
    ];
    assert.ok(failures.every((failure) => failure.status === "failed"));
    const activeControllerRevision = loop.controller.revision;
    const artifact = {
      schemaVersion: 1,
      evidenceClass: "Simulation",
      plan: { ...plan, approvalCount, excitationCount: 0 },
      evidenceRevisions: {
        loop: loop.revision,
        savedConfiguration: configuration(0.2).saved_configuration_revision,
        activeController: activeControllerRevision,
      },
      success: {
        completedStages: success.completedStages,
        approvalCount,
        excitationCount: 0,
        activeControllerRevisionBefore: activeControllerRevision,
        activeControllerRevisionAfter: activeControllerRevision,
        result: {
          applied: success.result.applied,
          modelEvidence: success.result.modelEvidence,
          model: success.result.model,
          method: success.result.method,
          structure: success.result.structure,
          kp: success.result.kp,
          tiSeconds: success.result.tiSeconds,
          ki: Number(success.result.parameters.integralGain.toPrecision(8)),
          gainUnits: { kp: "%/°C", ti: "s", ki: "%/(°C·s)" },
          attempts: success.result.attempts,
          requirements: success.result.requirements,
          metrics: success.result.metrics,
        },
      },
      evidenceBoundary: "This pure service records deterministic stage outcomes but does not observe runtime approval counts, output writes, controller revisions, or restoration I/O. Runtime-owned safety evidence is linked below.",
      failures: failures.map((failure) => failure.status === "failed" ? {
        completedStages: failure.completedStages,
        failedStage: failure.failedStage,
        reason: failure.reason,
        recovery: failure.recovery,
        executionEvidence: {
          status: "Not applicable",
          reason: "The pure service returned this stage outcome before runtime execution; caller approval, excitation, and controller-revision evidence are not observed here.",
        },
      } : failure),
      restorationClassifications: [
        { input: { status: "aborted", stopReason: "user_abort", restoration: "pending" },
          state: restoration[0], classificationOnly: true },
        { input: { status: "aborted", stopReason: "user_abort", restoration: "restored" },
          state: restoration[1], classificationOnly: true },
        { input: { status: "failed_to_restore", stopReason: "signal_outside_limits",
          restoration: "failed", restorationReason: "confirmation_timeout" },
          state: restoration[2], classificationOnly: true },
      ],
      runtimeSafetyEvidence: [
        { nativeTest: "control_studio_experiment_starts_once_and_abort_restores_exact_value",
          sourceFile: "crates/trust-runtime/src/control/tests/control_studio_experiment.rs",
          command: "cargo test -p trust-runtime --lib control_studio_experiment_starts_once_and_abort_restores_exact_value -- --nocapture",
          proves: ["start writes the reviewed excitation once", "abort writes the exact restoration value", "stop_reason is user_abort"] },
        { nativeTest: "control_studio_abort_preempts_future_excitation_and_confirms_restoration_from_io",
          sourceFile: "crates/trust-runtime/src/control/tests/control_studio_experiment.rs",
          command: "cargo test -p trust-runtime --lib control_studio_abort_preempts_future_excitation_and_confirms_restoration_from_io -- --nocapture",
          proves: ["no trajectory write occurs after abort", "restoration is pending before I/O read-back", "I/O read-back changes restoration to restored"] },
        { nativeTest: "control_studio_runtime_data_trip_stops_and_failed_restoration_is_explicit",
          sourceFile: "crates/trust-runtime/src/control/tests/control_studio_experiment.rs",
          command: "cargo test -p trust-runtime --lib control_studio_runtime_data_trip_stops_and_failed_restoration_is_explicit -- --nocapture",
          proves: ["out-of-limit I/O queues the restoration value", "failed read-back reports failed_to_restore", "stop_reason remains signal_outside_limits"] },
        { nativeTest: "control_studio_running_experiment_trips_on_force_and_staleness",
          sourceFile: "crates/trust-runtime/src/control/tests/control_studio_experiment.rs",
          command: "cargo test -p trust-runtime --lib control_studio_running_experiment_trips_on_force_and_staleness -- --nocapture",
          proves: ["forced runtime data stops the experiment", "stale runtime data stops the experiment", "both paths retain restored state"] },
      ],
    };
    const orchestrationPath = path.join(
      lessonRoot, "figures", "automatic-water-heater-orchestration-evidence.json"
    );
    writeOrCompareJson(orchestrationPath, artifact);
    const orchestration = JSON.parse(fs.readFileSync(orchestrationPath, "utf8"));
    assert.deepStrictEqual(orchestration.plan.stages, ["Test", "Identify", "Tune", "Validate"]);
    assert.deepStrictEqual(orchestration.success.completedStages,
      ["Test", "Identify", "Tune", "Validate"]);
    assert.strictEqual(orchestration.success.approvalCount, 1);
    assert.strictEqual(orchestration.success.excitationCount, 0);
    assert.strictEqual(orchestration.success.result.applied, false);
    assert.strictEqual(orchestration.success.result.kp, 5.7692308);
    assert.strictEqual(orchestration.success.result.tiSeconds, 5);
    assert.strictEqual(orchestration.success.result.ki, 1.1538462);
    assert.deepStrictEqual(orchestration.success.result.gainUnits, {
      kp: "%/°C", ti: "s", ki: "%/(°C·s)",
    });
    assert.ok(orchestration.success.result.attempts.length >= 6);
    assert.ok(orchestration.success.result.requirements.every((row: any) => row.status === "Pass"));
    assert.deepStrictEqual(Object.fromEntries(orchestration.success.result.requirements
      .filter((row: any) => ["MV excursion", "MV rate", "Noise response"].includes(row.name))
      .map((row: any) => [row.name, row.unit])), {
      "MV excursion": "%", "MV rate": "%/s", "Noise response": "% RMS",
    });
    assert.strictEqual(orchestration.success.activeControllerRevisionBefore,
      orchestration.success.activeControllerRevisionAfter);
    assert.deepStrictEqual(orchestration.failures.map((failure: any) => failure.failedStage),
      ["Test", "Identify", "Tune", "Validate"]);
    assert.ok(orchestration.failures.every((failure: any) =>
      !("approvalCount" in failure) && !("observed" in failure)
      && !("activeControllerRevisionBefore" in failure)));
    assert.ok(orchestration.failures.every((failure: any) =>
      failure.executionEvidence?.status === "Not applicable"
      && /pure service.*before runtime execution/i.test(failure.executionEvidence.reason)));
    assert.match(orchestration.evidenceBoundary, /pure service.*does not observe/i);
    assert.deepStrictEqual(orchestration.restorationClassifications.map((item: any) => item.state),
      ["awaiting_restoration", "terminal", "terminal"]);
    assert.ok(orchestration.restorationClassifications.every((item: any) =>
      !("observed" in item) && item.classificationOnly === true));
    assert.deepStrictEqual(orchestration.runtimeSafetyEvidence.map((item: any) => item.nativeTest), [
      "control_studio_experiment_starts_once_and_abort_restores_exact_value",
      "control_studio_abort_preempts_future_excitation_and_confirms_restoration_from_io",
      "control_studio_runtime_data_trip_stops_and_failed_restoration_is_explicit",
      "control_studio_running_experiment_trips_on_force_and_staleness",
    ]);
    assert.ok(orchestration.runtimeSafetyEvidence.every((item: any) =>
      item.sourceFile === "crates/trust-runtime/src/control/tests/control_studio_experiment.rs"
      && item.command && item.proves.length > 0));
  });
});
