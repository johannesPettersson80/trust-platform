import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { parseControlLesson } from "../../examples/model";
import { canonicalLessonReadmeHeadings, pedagogicalCoreWordCount, pedagogicalMarkdownText } from "./controlLessonAssertions";

function readStructuredTextProject(lessonRoot: string): string {
  const sourceRoot = path.join(lessonRoot, "src");
  return fs.readdirSync(sourceRoot)
    .filter((name) => name.endsWith(".st"))
    .sort()
    .map((name) => fs.readFileSync(path.join(sourceRoot, name), "utf8"))
    .join("\n");
}

function assertRetainedCapstoneEvidence(
  lessonRoot: string,
  source: string,
  stem: string,
): void {
  for (const suffix of [".svg", "-trace.csv", "-summary.csv"]) {
    assert.ok(
      fs.existsSync(path.join(lessonRoot, "figures", `${stem}${suffix}`)),
      `capstone evidence is missing: ${stem}${suffix}`,
    );
  }
  assert.match(source, new RegExp(`!\\[[^\\]]+\\]\\(figures/${stem}\\.svg\\)`));
  assert.match(source, new RegExp(`\\[Complete trace\\]\\(figures/${stem}-trace\\.csv\\)`));
  assert.match(source, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
}

suite("Control-engineering capstone lessons", () => {
  test("commissions the cold room from requirements instead of a prescribed answer", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "76-cold-storage"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 76 executable cold-storage capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-cold-storage"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 76);
    assert.deepStrictEqual(lesson.prerequisites, ["control-loop-diagnosis"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-water-heater");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_cold_storage_capstone_selects_safe_discrete_control");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /walk-in cold-storage room.*simplest discrete controller.*product band.*short-cycling/i,
      /setpoint \(SP\).*process value \(PV\).*degrees Celsius/i,
      /requested manipulated variable \(MV_requested\).*applied manipulated variable \(MV_applied\)/i,
      /compressor.*remove heat.*cannot actively add heat.*0 percent.*100 percent/i,
      /single-threshold.*noise.*repeated transitions/i,
      /hysteresis width \(Delta_T_hys\).*T_on = SP \+ Delta_T_hys\/2.*T_off = SP - Delta_T_hys\/2/i,
      /minimum off time.*restart.*minimum on time/i,
      /starter-ready.*external.*not invent.*history/i,
      /safety band.*control band.*different/i,
      /2\.5 degrees Celsius.*1\.5 degrees Celsius.*0\.5 degrees Celsius/i,
      /single threshold.*hysteresis-only.*hysteresis-plus-dwell/i,
      /Control Studio.*Monitor.*Test/i,
      /maximum eligible temperature.*time outside.*transition count.*completed on.*off duration/i,
      /door.*60.*90 seconds.*sensor noise.*restart.*invalid.*manual.*recovery/i,
      /Simulation evidence.*not.*physical/i,
      /ON\/OFF control.*installed actuator is discrete.*continuous controller/i,
      /Lesson 77.*water heater/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "AutoMode", "TemperatureQualityGood", "StarterReady", "Setpoint",
      "HysteresisWidth", "MinimumOnTime", "MinimumOffTime", "MeasuredCycleTime",
      "ProcessValue", "CoolingRequested", "CompressorCommand", "CompressorRunning",
      "DwellElapsed", "DwellInhibit", "SwitchCount",
    ]) assert.ok(program.includes(required), `capstone ST must expose ${required}`);
    assert.ok(!/Control\.PID|PIDParameters/.test(program),
      "the discrete-compressor capstone must not hide an unnecessary PID");
    assert.match(simulation, /model\s*=\s*"first_order"/);
    assert.match(simulation, /standard_deviation\s*=\s*0\.03/);
    assert.match(simulation, /at_ms\s*=\s*60000[\s\S]*value\s*=\s*6\.0/);
    assert.match(simulation, /at_ms\s*=\s*90000[\s\S]*value\s*=\s*4\.0/);
    assertRetainedCapstoneEvidence(lessonRoot, source, "cold-storage-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 3_200);
  });

  test("commissions the recirculating heater from models through an unapplied PI decision", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "77-water-heater"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 77 executable water-heater capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-water-heater"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 77);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-cold-storage"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-servo");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_water_heater_capstone_commissions_unapplied_pi");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /recirculating electric water heater.*35 degrees Celsius.*0.*100 percent.*cannot actively cool/i,
      /setpoint \(SP\).*process value \(PV\).*error \(e\)/i,
      /requested manipulated variable \(MV_requested\).*applied manipulated variable \(MV_applied\)/i,
      /thermal equilibrium.*heat input.*heat loss.*incoming-water load/i,
      /MV_hold = \(SP - T_zero\)\/K = \(35 - 20\)\/0\.2 = 75 percent/i,
      /time constant.*dead time.*physical assumptions.*first-order-plus-dead-time/i,
      /safe power-step identification.*gain.*delay.*63\.2 percent/i,
      /fit data.*independent validation data.*not.*same record/i,
      /P.*bias.*changed-load offset.*integral-only.*PI.*simplest/i,
      /derivative.*rejected.*noise-qualified comparison/i,
      /combined anti-windup.*requested.*applied.*tracking.*bumpless/i,
      /Robust SIMC.*3\.26087.*0\.65217.*Balanced SIMC.*5\.76923.*1\.15385/i,
      /historical PI.*4.*0\.8.*not.*straw/i,
      /Control Studio.*Monitor.*Test/i,
      /physical model.*identified model.*independent.*residual/i,
      /final-window.*overshoot.*integrated absolute error.*total variation.*saturation.*robustness/i,
      /unapplied recommendation.*explicit approval.*rollback/i,
      /Simulation evidence.*not.*physical/i,
      /Lesson 78.*motion/i,
    ]) assert.match(readme, requirement);
    assert.match(equations, /e = SP - PV/i);
    for (const required of [
      "Control.PID", "PIDParameters", "TemperatureSetpoint", "CurrentTemperature",
      "CandidateTemperature", "CurrentRequested", "CandidateRequested",
      "CurrentApplied", "CandidateApplied", "ManualMode", "ManualOutput",
      "ControllerRevision", "CycleTime",
    ]) assert.ok(program.includes(required), `water-heater capstone ST must expose ${required}`);
    assert.match(simulation, /model\s*=\s*"first_order_dead_time"/);
    assert.ok((simulation.match(/model\s*=\s*"first_order_dead_time"/g) ?? []).length >= 4,
      "equivalent nominal and uncertainty validation copies are required");
    assertRetainedCapstoneEvidence(lessonRoot, source, "water-heater-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 3_800);
  });

  test("commissions a flexible indexing servo through time frequency and digital evidence", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "78-servo"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 78 executable servo capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-servo"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 78);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-water-heater"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-reactor");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_servo_capstone_qualifies_filtered_pid_digitally");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /flexible rotary indexing table.*minimum eligible structure.*unapplied/i,
      /setpoint \(SP\).*process value \(PV\).*position error \(e\)/i,
      /requested manipulated variable \(MV_requested\).*applied manipulated variable \(MV_applied\)/i,
      /motor inertia.*payload inertia.*shaft stiffness.*shaft damping.*11\.18 radians per second/i,
      /P.*lightly damped.*PD.*constant payload.*PID.*zero constant-load error/i,
      /derivative.*measurement.*setpoint kick.*low-pass filter.*0\.05 seconds/i,
      /raw encoder.*filtered position.*noise.*phase lag/i,
      /root locus.*closed-loop poles.*Bode.*gain margin.*phase margin/i,
      /sensitivity \(S\).*complementary sensitivity \(T\).*maximum sensitivity/i,
      /sampling.*zero-order hold.*one-update.*Nyquist.*314\.159.*31\.416/i,
      /Kp = 7.*Ki = 1\.2.*Kd = 2.*phase margin.*38.*gain margin.*1\.94/i,
      /3\.5 newton metres.*0\.2143 radians.*100 milliseconds.*reject/i,
      /Control Studio.*Monitor.*Test/i,
      /overshoot.*two-percent settling.*final-window.*total variation.*root-mean-square/i,
      /heavy payload.*reduced damping.*doubled encoder noise.*manual.*invalid quality.*controlled stop/i,
      /Simulation evidence.*not.*physical/i,
      /unapplied.*explicit approval.*rollback/i,
      /Lesson 79.*reactor/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "Control.PID", "Control.LowPass", "ServoCandidate", "PositionSetpoint",
      "RawPosition", "FilteredPosition", "RequestedTorque", "PendingTorque",
      "AppliedTorque", "LoadTorque", "FineUpdatePulse", "CoarseUpdatePulse",
      "MeasurementQualityGood", "AutomaticEnabled", "ControllerRevision",
    ]) assert.ok(program.includes(required), `servo capstone ST must expose ${required}`);
    assert.ok((simulation.match(/model\s*=\s*"state_space"/g) ?? []).length >= 4,
      "four equivalent flexible-table copies are required");
    assert.match(simulation, /frequency_hz\s*=\s*20\.0/);
    assertRetainedCapstoneEvidence(lessonRoot, source, "servo-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 4_200);
  });

  test("commissions one tracked reactor architecture across its operating sequence", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "79-reactor"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 79 executable reactor capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-reactor"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 79);
    assert.strictEqual(lesson.level, "Modern",
      "the integrated reactor journey must be reachable from the shipped curriculum");
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-servo"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-gravity-tank");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_reactor_capstone_coordinates_tracked_authority");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /jacketed stirred reactor.*startup.*production.*constraint.*shutdown.*unapplied/i,
      /reactor-temperature setpoint \(SP_r\).*reactor process value \(PV_r\)/i,
      /jacket-temperature setpoint \(SP_j\).*jacket process value \(PV_j\)/i,
      /signed utility demand \(U\).*steam.*cooling.*requested.*applied/i,
      /jacket.*2 seconds.*reactor.*12 seconds.*at least three times faster/i,
      /inner loop.*outer.*Track.*grant production authority/i,
      /feed thermal load \(D_feed\).*feedback waits.*feedforward.*0\.525/i,
      /positive demand.*steam.*negative demand.*cooling.*mutual.*exclusion/i,
      /dead zone.*overlap.*installed.*gain/i,
      /high-temperature.*low selector.*83\.5.*82\.5.*tracking/i,
      /InnerCommissioning.*Production.*Manual.*ControlledShutdown.*Safe/i,
      /Control Studio.*Monitor.*Test/i,
      /integrated absolute error.*maximum reactor departure.*onset delay.*total variation/i,
      /feedforward-quality loss.*steam-unavailable.*cooling-unavailable.*manual.*shutdown/i,
      /Simulation evidence.*not.*physical/i,
      /unapplied.*explicit approval.*rollback/i,
      /Lesson 80.*gravity/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "Control.PID", "ReactorTrain", "OperatingMode", "ReactorSetpoint",
      "ReactorTemperature", "JacketSetpoint", "JacketTemperature", "FeedThermalLoad",
      "FeedforwardContribution", "NormalUtilityDemand", "ProtectionDemand",
      "SelectedUtilityDemand", "HeatingRequested", "CoolingRequested",
      "HeatingApplied", "CoolingApplied", "ProtectionActive", "ActiveAuthority",
      "ArchitectureRevision",
    ]) assert.ok(program.includes(required), `reactor capstone ST must expose ${required}`);
    assert.ok((simulation.match(/model\s*=\s*"state_space"/g) ?? []).length >= 2,
      "equivalent feedback-only and integrated reactor copies are required");
    assertRetainedCapstoneEvidence(lessonRoot, source, "reactor-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 4_500);
  });

  test("commissions gravity-tank scheduling from local models through tracked recovery", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "80-gravity-tank"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 80 executable gravity-tank capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-gravity-tank"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 80);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-reactor"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-conveyor-oven");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_gravity_tank_capstone_qualifies_full_range_schedule");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /vented gravity-drained water tank.*0\.30.*0\.80 metres.*unapplied/i,
      /liquid level \(h\).*inlet pump flow \(q_in\).*gravity outlet flow \(q_out\)/i,
      /tank area \(A\).*outlet coefficient \(c\).*one-sided.*cannot.*negative/i,
      /q_out = c \* sqrt\(h\).*A \* dh\/dt = q_in - q_out/i,
      /0\.02739.*0\.03708.*0\.04472.*cubic metres per second/i,
      /equilibrium level \(h_eq\).*level deviation \(delta_h\).*pump-flow deviation \(delta_q\)/i,
      /local steady gain \(K_h\).*local time constant \(T_h\).*21\.91.*29\.66.*35\.78/i,
      /separately restored.*bounded pulse.*unused.*validation.*residual.*uncertainty/i,
      /proportional gain \(Kp\).*integral gain \(Ki\).*0\.00456.*0\.00337.*0\.00280/i,
      /scheduling variable \(rho\).*requested operating level.*not.*error/i,
      /scheduling variable.*interpolation fraction.*scheduled parameter/i,
      /2-second.*filter.*atomically.*no scan.*mixed/i,
      /requested pump flow \(q_requested\).*applied pump flow \(q_applied\)/i,
      /Automatic.*Manual.*ControlledStop.*Safe.*tracking.*0\.0005/i,
      /Control Studio.*Monitor.*Test/i,
      /30 seconds.*120 seconds.*240 seconds.*330 seconds.*420 seconds/i,
      /regional integrated absolute level error.*worst regional.*total variation.*authority return bump/i,
      /invalid level quality.*invalid schedule.*pump unavailable.*Manual.*ControlledStop.*restoration/i,
      /Simulation evidence.*not.*physical/i,
      /unapplied.*explicit approval.*rollback/i,
      /Lesson 81.*transport delay/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "Control.PID", "Control.PIDGainScheduler", "TankController", "OperatingMode",
      "LevelSetpoint", "MeasuredLevel", "LevelQualityGood", "SchedulingAvailable",
      "PumpAvailable", "ManualPumpFlow", "RequestedPumpFlow", "AppliedPumpFlow",
      "FilteredSchedulingLevel", "ActiveKp", "ActiveKi", "ScheduleValid",
      "ScheduleClamped", "TrackingActive", "FaultActive", "ControllerRevision",
    ]) assert.ok(program.includes(required), `gravity-tank capstone ST must expose ${required}`);
    assert.ok((simulation.match(/model\s*=\s*"gravity_tank"/g) ?? []).length >= 2,
      "equivalent fixed and scheduled nonlinear tanks are required");
    assert.match(simulation, /standard_deviation\s*=\s*0\.0005/);
    assertRetainedCapstoneEvidence(lessonRoot, source, "gravity-tank-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 4_000);
  });

  test("selects conveyor-oven delay compensation from time frequency and mismatch evidence", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "81-conveyor-oven"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 81 executable conveyor-oven capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-conveyor-oven"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 81);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-gravity-tank"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-distillation");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_conveyor_oven_capstone_selects_delay_architecture");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /continuous food-drying conveyor oven.*20.*35 degrees Celsius.*unapplied/i,
      /heater command \(u\).*downstream product temperature \(y\).*process gain \(K\).*time constant \(T\).*transport delay \(L\)/i,
      /Identify the delayed plant.*operating heater command/i,
      /10-percent heater step.*8-degree.*16 seconds.*12 seconds.*1\.333/i,
      /bounded heater step.*delay-free.*FOPDT.*unused.*validation.*residual.*uncertainty/i,
      /ordinary PI.*0\.46875.*0\.0390625.*aggressive.*0\.9375.*0\.078125/i,
      /Bode.*crossover.*phase margin.*gain margin.*delay margin.*35 degrees.*2 seconds/i,
      /Nyquist.*indented contour.*open-loop.*P.*encirclement.*N.*closed-loop.*Z/i,
      /upstream inlet-temperature deviation \(D_inlet\).*feedforward gain \(Kff\).*2\.5 percent/i,
      /delay-free model output \(y_model_now\).*delayed model output \(y_model_delayed\).*model correction \(c_model\)/i,
      /y_predicted = y_model_now \+ c_model.*6 seconds.*2\.5.*0\.2083333/i,
      /model mismatch.*0\.76.*13.*17.*0\.60.*18.*22.*fallback/i,
      /requested heater power \(MV_requested\).*applied heater power \(MV_applied\)/i,
      /Automatic.*Manual.*ControlledStop.*Safe.*tracking/i,
      /Control Studio.*Monitor.*Test/i,
      /10 seconds.*54 seconds.*70 seconds.*180 seconds/i,
      /setpoint-window integrated absolute temperature error.*regulatory IAE.*90-percent rise.*heater total variation/i,
      /invalid upstream quality.*invalid downstream quality.*incomplete history.*heater unavailable.*Manual.*ControlledStop/i,
      /Simulation evidence.*not.*physical/i,
      /unapplied.*explicit approval.*rollback/i,
      /Lesson 82.*interact/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "Control.PID", "Control.SmithPredictorFOPDT", "OvenController", "OperatingMode",
      "TemperatureSetpoint", "MeasuredTemperature", "PredictedTemperature",
      "UpstreamTemperatureDeviation", "UpstreamQualityGood", "TemperatureQualityGood",
      "ModelValid", "PredictorHistoryComplete", "PredictorActive", "PredictorFallback",
      "FeedforwardContribution", "RequestedHeaterPower", "AppliedHeaterPower",
      "DelayFreeModel", "DelayedModel", "ModelCorrection", "TrackingActive",
      "FaultActive", "OrdinaryThermalInput", "FeedforwardThermalInput",
      "IntegratedThermalInput", "ControllerRevision",
    ]) assert.ok(program.includes(required), `conveyor-oven capstone ST must expose ${required}`);
    assert.match(program,
      /PredictionEnabled THEN[\s\S]*ProportionalGain := LREAL#2\.5;[\s\S]*IntegralGain := LREAL#0\.2083333;/,
      "the executable Smith PI must use the tuning explained by the lesson");
    assert.ok((simulation.match(/model\s*=\s*"first_order_dead_time"/g) ?? []).length >= 3,
      "equivalent ordinary, feedforward and predictor ovens are required");
    assert.match(simulation,
      /command_sources\s*=\s*\["%QL24"\][\s\S]*command_sources\s*=\s*\["%QL32"\][\s\S]*command_sources\s*=\s*\["%QL40"\]/,
      "every equivalent oven must receive its independently composed net thermal input");
    assertRetainedCapstoneEvidence(lessonRoot, source, "conveyor-oven-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 4_500);
  });

  test("commissions a distillation architecture from interaction through feasibility evidence", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "82-distillation-column"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 82 executable distillation capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const nominal = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const stronger = fs.readFileSync(
      path.join(lessonRoot, "simulation-strong-interaction.toml"), "utf8"
    );

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-distillation"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 82);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-conveyor-oven"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-building-hvac");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_distillation_capstone_selects_robust_architecture");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /continuous binary distillation column.*reflux.*reboiler.*overhead.*bottoms/i,
      /input vector \(u\).*output vector \(y\).*dynamic matrix \(G\(s\)\)/i,
      /one input at a time.*other input fixed.*unused simultaneous-input data/i,
      /steady gain matrix.*Relative Gain Array/i,
      /1\.272727.*-0\.272727.*diagonal pairing.*crossed pairing/i,
      /direct decentralized PI.*static decoupling.*central constraint coordination/i,
      /not.*centralized MIMO control.*MPC/i,
      /singular value.*sigma_max.*sigma_min.*condition number.*inverse amplification/i,
      /0\.5364.*2\.73.*0\.2512.*6\.97.*0\.30/i,
      /sensitivity matrix \(S\).*inverse\(I \+ L\(s\)\).*0\.002.*2 radians per second/i,
      /5\.44.*5\.24.*10\.88.*10\.47.*Infeasible/i,
      /requested reflux.*applied reflux.*requested reboiler.*applied reboiler/i,
      /Automatic.*Manual.*ControlledStop.*Safe.*bumpless/i,
      /Control Studio.*Monitor.*Test/i,
      /final-window combined absolute error.*unintended cross peak.*combined IAE.*total variation/i,
      /invalid model.*invalid composition.*unavailable valve.*restoration.*rollback/i,
      /Simulation evidence.*not.*physical/i,
      /static decoupling.*simplest architecture.*Lesson 83/i,
    ]) assert.match(readme, requirement);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 2);
    assert.strictEqual((program.match(/Control\.StaticDecoupler2x2;/g) ?? []).length, 1);
    for (const required of [
      "OperatingMode", "ArchitectureRequested", "ArchitectureActive", "ArchitectureQualified",
      "OverheadSetpoint", "BottomsSetpoint", "OverheadProcessValue",
      "BottomsProcessValue", "VirtualOverheadCorrection", "VirtualBottomsCorrection",
      "RefluxRequested", "ReboilerRequested", "RefluxApplied", "ReboilerApplied",
      "CommonScale", "Saturated", "QualityGood", "RefluxAvailable",
      "ReboilerAvailable", "TrackingActive", "FaultActive", "ControllerRevision",
    ]) assert.ok(program.includes(required), `distillation capstone ST must expose ${required}`);
    assert.match(nominal, /model\s*=\s*"state_space"/);
    assert.match(stronger, /model\s*=\s*"state_space"/);
    assert.notStrictEqual(nominal, stronger,
      "nominal and stronger-interaction validation plants must be distinct");
    assertRetainedCapstoneEvidence(lessonRoot, source, "distillation-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 4_800);
  });

  test("decides whether predictive HVAC control earns its estimator and actuator cost", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "83-building-hvac"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 83 executable building-HVAC capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const nominal = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const mismatch = fs.readFileSync(
      path.join(lessonRoot, "simulation-outdoor-loss-mismatch.toml"), "utf8"
    );

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-building-hvac"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 83);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-distillation"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-independent");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_building_hvac_capstone_makes_evidence_based_decision");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /two thermally coupled office zones.*hydronic heating.*outdoor.*occupancy.*tariff/i,
      /sampled state-transition matrix \(A_d\).*sampled input matrix \(B_d\)/i,
      /unmeasured net thermal-load bias.*innovation \(nu\).*estimated.*bias.*d_hat/i,
      /unmeasured net thermal-load bias.*innovation.*estimator gain.*0\.4/i,
      /observer.*measured.*no information.*additive-load estimator/i,
      /cascade.*Ineligible.*no qualified inner variable/i,
      /incremental PI.*proportional gain.*20.*integral gain.*0\.02/i,
      /prediction horizon \(N=3\).*next three.*-20,0,\+20.*729/i,
      /temperature weight.*energy weight.*move weight.*objective/i,
      /receding horizon.*only the first.*solve.*again/i,
      /configuration validity.*completion.*feasible.*infeasible.*solver failure/i,
      /Control Studio.*Monitor.*Test/i,
      /158\.67.*146\.72.*46,624.*46,200.*18\.7.*160\.0/i,
      /tradeoff.*not.*automatic win/i,
      /invalid.*stale.*unbounded innovation.*unavailable heater.*Manual.*ControlledStop/i,
      /Simulation evidence.*not.*physical/i,
      /select.*simplest.*Lesson 84/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "OperatingMode", "QualityGood", "Heater1Available", "Heater2Available",
      "EstimatorInitialized", "EstimatorValid", "PreviousPrediction1",
      "PreviousPrediction2", "Innovation1", "Innovation2", "EstimatedBias1",
      "EstimatedBias2", "CandidateCount", "FeasibleCandidateCount", "BestObjective",
      "SolverComplete", "SolverFeasible", "Infeasible", "FallbackActive",
      "Zone1HeatRequested", "Zone2HeatRequested", "Zone1HeatApplied",
      "Zone2HeatApplied", "TrackingActive", "FaultActive", "ControllerRevision",
    ]) assert.ok(program.includes(required), `building-HVAC capstone ST must expose ${required}`);
    assert.ok((program.match(/FOR I\d\d := DINT#0 TO DINT#2 DO/g) ?? []).length >= 6,
      "the executable capstone must enumerate all six move decisions online");
    assert.match(nominal, /model\s*=\s*"state_space"/);
    assert.match(mismatch, /model\s*=\s*"state_space"/);
    assert.notStrictEqual(nominal, mismatch,
      "nominal and outdoor-loss mismatch plants must be distinct");
    assertRetainedCapstoneEvidence(lessonRoot, source, "building-hvac-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 5_000);
  });

  test("defends an independent compressed-air control-system recommendation", function () {
    this.timeout(20000);
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "14-integrated-capstones", "84-independent"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 84 executable independent capstone is missing");
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const program = readStructuredTextProject(lessonRoot);
    const nominal = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const uncertainty = fs.readFileSync(
      path.join(lessonRoot, "simulation-uncertainty.toml"), "utf8"
    );

    assert.deepStrictEqual(parseControlLesson(lesson, "control-capstone-independent"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 84);
    assert.deepStrictEqual(lesson.prerequisites, ["control-capstone-building-hvac"]);
    assert.strictEqual(lesson.nextLesson, null);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_independent_capstone_defends_selected_architecture");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /learner-selected.*platform-supported.*shipped reference.*compressed-air receiver/i,
      /header pressure.*controlled.*bar.*compressor-delivery request.*manipulated.*percent/i,
      /actual compressor delivery.*plant air demand.*measured disturbance/i,
      /pressure relief.*high-pressure trip.*not.*controller/i,
      /header pressure \(p\).*compressor delivery \(q_c\).*plant demand \(q_d\).*receiver rate gain \(K_r\)/i,
      /receiver stores compressed air.*delivery exceeds.*demand.*pressure rises/i,
      /delivery request \(u\).*delivery time constant \(T_c\).*dq_c\/dt = \(u-q_c\)\/T_c/i,
      /bounded.*request pulse.*delivery-demand imbalance.*unused.*validation/i,
      /ON\/OFF.*P.*PI.*PID.*feedforward.*cascade.*scheduling.*Smith.*MIMO.*MPC/i,
      /cascade.*ineligible.*installed drive.*inner loop/i,
      /pressure setpoint \(SP\).*header pressure \(PV\).*pressure error \(e\)/i,
      /MV_feedback.*MV_feedforward.*MV_requested.*MV_applied/i,
      /proportional gain.*40.*integral gain.*1(?:\.0)?.*one second/i,
      /closed-loop poles.*Bode.*phase margin.*gain margin.*delay margin.*sensitivity/i,
      /50.*65 percent.*-0\.075 bar\/s.*15 percentage points.*five-second/i,
      /Control Studio.*Monitor.*Test/i,
      /pressure integrated absolute error.*total variation.*30 percent.*25 percent/i,
      /invalid demand.*feedforward.*PI.*invalid.*pressure.*fail closed.*delivery.*authority/i,
      /Simulation evidence.*not.*physical/i,
      /reproducible engineering report.*requirements.*rollback.*maintenance/i,
    ]) assert.match(readme, requirement);
    for (const required of [
      "Control.PID", "FeedbackController", "IntegratedController", "OperatingMode",
      "PressureQualityGood", "DemandQualityGood", "DeliveryQualityGood",
      "CompressorAvailable", "PressureSetpoint", "FeedbackPressure",
      "IntegratedPressure", "MeasuredDemand", "FeedbackRequested",
      "IntegratedFeedback", "IntegratedFeedforward", "IntegratedRequested",
      "FeedbackApplied", "IntegratedApplied", "FeedbackActualDelivery",
      "IntegratedActualDelivery", "TrackingActive", "FaultActive",
      "ControllerRevision",
    ]) assert.ok(program.includes(required), `independent capstone ST must expose ${required}`);
    for (const forbidden of [
      "PressureIae", "AcceptedCandidate", "SelectedWinner", "PrecomputedTrace",
    ]) assert.ok(!program.includes(forbidden), `independent capstone ST embeds ${forbidden}`);
    assert.ok((nominal.match(/model\s*=\s*"state_space"/g) ?? []).length >= 2,
      "two equivalent nominal compressed-air plants are required");
    assert.ok((uncertainty.match(/model\s*=\s*"state_space"/g) ?? []).length >= 2,
      "two equivalent uncertainty compressed-air plants are required");
    assert.notStrictEqual(nominal, uncertainty,
      "nominal and uncertainty compressed-air plants must be distinct");
    assertRetainedCapstoneEvidence(lessonRoot, source, "independent-capstone-evidence");
    assert.ok(pedagogicalCoreWordCount(source) <= 5_000);
  });
});
