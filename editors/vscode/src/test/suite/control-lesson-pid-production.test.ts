import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { parseControlLesson } from "../../examples/model";
import { canonicalLessonReadmeHeadings, pedagogicalCoreWordCount, pedagogicalMarkdownText } from "./controlLessonAssertions";

const canonicalLessonHeadings = canonicalLessonReadmeHeadings;

function assertRetainedTuningEvidence(
  lessonRoot: string,
  source: string,
  stem: string,
): void {
  for (const heading of canonicalLessonHeadings) {
    assert.match(source, new RegExp(`^## ${heading}$`, "m"));
  }
  for (const suffix of ["svg", "trace.csv", "summary.csv"]) {
    const name = suffix === "svg" ? `${stem}.svg` : `${stem}-${suffix}`;
    assert.ok(fs.existsSync(path.join(lessonRoot, "figures", name)), `${name} must be retained`);
    assert.match(source, new RegExp(`figures/${name.replace(/[.]/g, "\\.")}`));
  }
  const svg = fs.readFileSync(path.join(lessonRoot, "figures", `${stem}.svg`), "utf8");
  const title = svg.match(/<title[^>]*>([^<]+)<\/title>/)?.[1] ?? "";
  assert.ok(title.length > 0 && title.length <= 55, "plot title must fit the fixed SVG canvas");
}

function assertCsvContract(file: string, requiredColumns: readonly string[]): void {
  const lines = fs.readFileSync(file, "utf8").trim().split(/\r?\n/);
  const header = lines[0].split(",");
  for (const column of requiredColumns) {
    assert.ok(header.includes(column), `${path.basename(file)} must retain ${column}`);
  }
  assert.ok(lines.slice(1).every((line) => line.split(",").length === header.length),
    `${path.basename(file)} must be one rectangular CSV table`);
}

suite("Production PID curriculum lessons", () => {
  test("teaches booster-pump state initialization before automatic authority", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "03-production-quality-digital-pid", "27-booster-pump-modes"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 27 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 27 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-booster-pump-modes"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 27);
    assert.deepStrictEqual(lesson.prerequisites, ["control-pid-forms"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_booster_pump_initializes_from_actual_state_before_auto");
    for (const requirement of [
      /raw discharge pressure \(PV_raw\).*pressure sensor.*bar/i,
      /filtered discharge pressure \(PV_filtered\).*controller.*bar/i,
      /actual valve position \(MV_actual\).*independent.*percent/i,
      /requested valve command \(MV_requested\).*controller.*percent/i,
      /nominal controller output \(u_nominal\).*before.*limits/i,
      /previous setpoint \(SP_previous\).*previous process value \(PV_previous\)/i,
      /initialization.*copies.*present.*performs no control calculation/i,
      /Manual.*manual output.*Auto.*feedback.*Track.*retained nominal output/i,
      /Disabled.*station-level supervision.*not.*PIDMode/i,
      /filter-settled indication.*three consecutive.*0\.01 bar/i,
      // L27-4: PV_eq is a distinct, explicitly introduced symbol for the
      // model's equilibrium relationship (README "The new idea"), not the
      // general dynamic PV also defined in this lesson.
      /PV_eq = 2 bar \+ 0\.1 bar\/% × MV/i,
      /20 percent.*4 bar.*40 percent.*6 bar.*70 percent.*9 bar/i,
      /first-update valve mismatch.*absolute.*MV_requested.*MV_actual.*percentage points/i,
      /filter initialization error.*absolute.*PV_filtered.*PV_raw.*bar/i,
      /zero-state.*10\.000 percent.*0\.198 bar/i,
      /Lesson 28.*bumpless/i,
    ]) assert.match(readme, requirement);
    assert.match(source, /^## Implementation$/m);
    assert.match(program, /PressureFilter\.Initialize\(Input := RawPressure\);/);
    assert.match(program, /InitialState\.NominalOutput := ActualValvePosition;/);
    assert.match(program, /InitialState\.SaturatedOutput := ActualValvePosition;/);
    assert.match(program, /ValveCommand := Controller\.Update\(Input := Input\);/);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 1,
      "the lesson must use one product PID and no parallel controller path");
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches bumpless authority transfer as a shared tracking contract", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "03-production-quality-digital-pid", "28-booster-pump-bumpless-transfer"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 28 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 28 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(
      parseControlLesson(lesson, "control-booster-pump-bumpless-transfer"), lesson
    );
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 28);
    assert.deepStrictEqual(lesson.prerequisites, ["control-booster-pump-modes"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_booster_pump_tracking_makes_authority_transfer_bumpless");
    for (const requirement of [
      /authority owner.*station.*command.*valve/i,
      /external applied valve output \(MV_external_applied\).*percent/i,
      /tracking input \(MV_track\).*applied.*percent/i,
      /retained nominal output \(u_nominal\).*controller state/i,
      /last applied output \(MV_last\).*transition/i,
      /transition instant \(t_transfer\)/i,
      /bumpless transfer.*first requested output.*last physically applied output/i,
      /Track.*integral action.*writes.*retained nominal output/i,
      /inactive without tracking.*stale/i,
      /Auto-to-Manual.*manual station.*adopt/i,
      /P or PD.*tracking.*no aligning effect/i,
      /stale.*applied-output|applied-output.*stale/i,
      /tracked.*60\.000.*60\.000.*0\.000/i,
      /untracked.*40\.000.*20\.000/i,
      /unaligned.*30\.000.*aligned.*0\.000/i,
      /authority-transition output step.*absolute.*first requested.*last applied.*percentage points/i,
      /retained-state alignment error.*u_nominal.*MV_external_applied/i,
      /Lesson 29.*atomic/i,
    ]) assert.match(readme, requirement);
    assert.match(source, /^## Implementation$/m);
    assert.match(program, /Input\.Mode := PIDMode#Track;/);
    assert.match(program, /Input\.TrackingOutput := TrackingOutput;/);
    assert.match(program, /ValveCommand := ExternalOutput;/);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 1);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches atomic configuration and one constraint-aware feedforward path", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "03-production-quality-digital-pid", "29-booster-pump-atomic-update"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 29 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 29 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(
      parseControlLesson(lesson, "control-booster-pump-atomic-update"), lesson
    );
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 29);
    assert.deepStrictEqual(lesson.prerequisites, ["control-booster-pump-bumpless-transfer"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_booster_pump_applies_atomic_parameters_and_orders_feedforward");
    for (const requirement of [
      /candidate configuration.*complete proposed.*PID/i,
      /active configuration.*controller.*current scan/i,
      /configuration revision.*whole active record/i,
      /coherent snapshot.*complete old.*complete new/i,
      /scan boundary.*one controller update/i,
      /feedback contribution.*output.*percentage points/i,
      /feedforward contribution.*measured disturbance.*percentage points/i,
      /nominal requested output \(MV_requested\).*before.*constraints/i,
      /applied output \(MV_applied\).*after.*constraints/i,
      /previous applied output \(MV_previous_applied\).*rate/i,
      /atomic.*record assignment.*one Configure call/i,
      /feedforward.*PIDInput\.FeedForward.*before.*anti-windup.*limits/i,
      /15\.3 percentage points.*15\.1 percentage points/i,
      /42\.000 percent.*60\.000 percent/i,
      /configuration coherence.*either.*complete old.*complete new/i,
      /first-update increment.*immediately before.*after/i,
      /external-sum limit violation.*percentage points/i,
      /applied-output rate.*difference.*cycle time/i,
      /Lesson 30.*invalid/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^## ${heading}$`, "m"));
    assert.match(program, /ActiveParameters := CandidateParameters;/);
    assert.match(program, /Controller\.Configure\(Parameters := ActiveParameters\);/);
    assert.match(program, /Input\.FeedForward := FeedForward;/);
    assert.match(program, /ValveCommand := Controller\.Update\(Input := Input\);/);
    assert.match(program, /RejectedExternalSum := ActualValvePosition \+ FeedForward;/);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 1);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches deterministic invalid-data handling and qualified recovery", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "03-production-quality-digital-pid", "30-booster-pump-invalid-data"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 30 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 30 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(
      parseControlLesson(lesson, "control-booster-pump-invalid-data"), lesson
    );
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 30);
    assert.deepStrictEqual(lesson.prerequisites, ["control-booster-pump-atomic-update"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_booster_pump_rejects_invalid_scans_and_requires_qualified_recovery");
    for (const requirement of [
      /valid numeric domain.*finite.*physical plausibility range/i,
      /data quality.*measurement age.*freshness limit/i,
      /measured cycle time.*deadline.*watchdog/i,
      /first-fault latch.*first detected cause/i,
      /hold-last output.*preserve.*hazard.*safe output.*30 percent/i,
      /acknowledgement.*operator.*does not clear/i,
      // L30-4: the station's own range guard (one ordered-comparison
      // conjunction) rejects nonfinite and implausible values alike, per
      // src/Main.st's own comment; the README no longer attributes NaN/Inf
      // rejection to an unexplained separate "runtime typed I/O boundary".
      /station's own range guard rejects.*nonfinite.*before.*PID arithmetic/i,
      /0 through 12 bar.*0 through 100 percent/i,
      /code 1.*numeric.*code 2.*stale.*code 3.*cycle.*code 4.*deadline.*code 5.*configuration/i,
      /invalid scan.*does not call.*PID.*does not call.*Configure/i,
      /three consecutive valid scans/i,
      /reset.*present setpoint.*process value.*actual valve/i,
      /separate renewed Auto request.*later scan/i,
      /diagnostic correctness.*exact equality.*injected.*published code/i,
      /invalid-scan state mutation.*required value is zero/i,
      /safe-output error.*absolute.*30 percent.*percentage points/i,
      /recovery output step.*first renewed-Auto output.*reset-scan/i,
      /30\.000 percent.*30\.000 percent/i,
      /Lesson 31.*minimum sufficient controller/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^## ${heading}$`, "m"));
    assert.match(program, /IF FaultLatched THEN\s+ValveCommand := LREAL#30\.0;/);
    assert.match(program, /IF ResetRequest AND FaultAcknowledged AND RecoveryReady THEN/);
    assert.match(program, /Controller\.Initialize\(State := InitialState\);/);
    assert.match(program, /ValveCommand := Controller\.Update\(Input := Input\);/);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 1);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches manual and historical heat-exchanger PI tuning", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "06-pid-tuning-methods", "42-heat-exchanger-tuning-methods"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-manual-historical-tuning");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-manual-historical-tuning"), lesson);
    assert.ok(catalogEntry, "Lesson 42 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/06-pid-tuning-methods/42-heat-exchanger-tuning-methods");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 42);
    assert.deepStrictEqual(lesson.prerequisites, ["control-independent-model-validation"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_manual_and_historical_pi_expose_their_tradeoffs");
    assertRetainedTuningEvidence(lessonRoot, source, "manual-historical-tuning-comparison");
    assertCsvContract(path.join(lessonRoot, "figures", "manual-historical-tuning-comparison-summary.csv"), [
      "applied", "controller_form", "bias_percent", "sample_time_s", "seed",
      "settling_status", "load_iae_degC_s", "peak_applied_mv_percent",
      "saturation_duration_s", "critical_peak_count", "invalid_sample_count",
      "abort_reason", "restoration_status",
    ]);
    assertCsvContract(path.join(lessonRoot, "figures", "manual-historical-tuning-comparison-trace.csv"), [
      "case", "pv_degC", "requested_mv_percent", "applied_mv_percent", "mode",
      "quality", "saturated", "event",
    ]);

    for (const requirement of [
      /four independent copies.*manual proportional-only.*manual proportional-integral.*critical proportional-only.*historical proportional-integral/i,
      /control error \(e\).*setpoint \(SP\).*measured outlet temperature \(PV\)/i,
      /operating-point bias.*valve position.*zero error/i,
      /proportional gain \(Kp\).*percentage points.*degree Celsius/i,
      /integral contribution \(I\).*accumulated past error/i,
      /integral gain \(Ki\).*percentage points.*degree Celsius.*second/i,
      /integral time \(Ti\).*proportional gain divided by integral gain/i,
      /derivative gain \(Kd\).*seconds per degree Celsius.*derivative time \(Td\)/i,
      /derivative action.*damping.*amplifies measurement noise.*does not add/i,
      /ideal parallel form.*bias.*present-error.*accumulated-error.*measured-rate/i,
      /Lesson 26.*incremental realization/i,
      /applied manipulated variable \(MV_applied\).*saturation/i,
      /manual tuning.*one term at a time.*proportional.*integral.*derivative/i,
      /ultimate gain \(Ku\).*constant-amplitude oscillation/i,
      /ultimate period \(Pu\).*equivalent peaks/i,
      /Ku = 22\.24.*Pu = 11\.35/i,
      /Ziegler-Nichols.*Kp = 0\.45.*Ti = Pu\/1\.2.*Ki = Kp\/Ti/i,
      /quarter-amplitude-decay.*one quarter.*aggressive.*not.*default/i,
      /manual proportional.*Kp = 2\.0.*1\.25 degrees Celsius/i,
      /manual PI.*Ki = 0\.05.*Ti = 40.*miss.*Ki = 0\.10.*Ti = 20/i,
      /historical PI.*Kp = 10\.01.*Ti = 9\.46.*Ki = 1\.06/i,
      /Control\.PID.*manual PI.*historical PI/i,
      /common 10-second baseline.*100-second load.*180 seconds/i,
      /Do not.*physical plant.*Apply/i,
      /rise time.*overshoot.*settling time.*load integral absolute error \(IAE\).*cumulative valve movement/i,
      /Not settled/i,
      /at least six alternating.*9 to 14 seconds/i,
      /cumulative valve movement.*at least twice/i,
      /Lesson 43.*relay/i,
    ]) assert.match(readme, requirement);
    assert.match(readme, /Ti = Kp \/ Ki/);
    assert.match(readme, /MV_requested = Bias \+ Kp\s*\*\s*e \+ I - Kd\s*\*\s*d\(PV\)\/dt/);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 2);
    assert.match(program, /HistoricalKp\s*:\s*LREAL\s*:=\s*10\.005/);
    assert.match(program, /HistoricalKi\s*:\s*LREAL\s*:=\s*1\.057/);
    assert.match(program, /ManualPiParameters\.IntegralGain := LREAL#0\.10/);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 4);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("teaches qualified feedback-relay autotuning without automatic application", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "06-pid-tuning-methods", "43-relay-autotuning"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 43 must ship a real feedback-relay project before it can be presented");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-relay-autotuning");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-relay-autotuning"), lesson);
    assert.ok(catalogEntry, "Lesson 43 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/06-pid-tuning-methods/43-relay-autotuning");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 43);
    assert.deepStrictEqual(lesson.prerequisites, ["control-manual-historical-tuning"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_feedback_relay_qualifies_cycles_before_producing_candidate");
    assertRetainedTuningEvidence(lessonRoot, source, "relay-autotuning-comparison");
    assertCsvContract(path.join(lessonRoot, "figures", "relay-autotuning-comparison-summary.csv"), [
      "transition_count", "crossing_times_s", "upper_extrema_degC", "lower_extrema_degC",
      "saturation_duration_s", "invalid_sample_count", "stop_reason", "failed_criteria",
      "candidate_kp_percent_per_degC", "candidate_ti_s", "candidate_ki_percent_per_degC_s",
      "candidate_applied", "restoration_status",
    ]);
    assertCsvContract(path.join(lessonRoot, "figures", "relay-autotuning-comparison-trace.csv"), [
      "case", "pv_degC", "requested_mv_percent", "applied_mv_percent", "mode",
      "quality", "saturated", "event",
    ]);

    for (const requirement of [
      /feedback relay.*not.*time-driven square wave/i,
      /relay bias.*20 percent.*relay amplitude \(d\).*percentage points/i,
      /high output.*bias plus.*low output.*bias minus/i,
      /relay center.*outlet temperature.*degrees Celsius/i,
      /hysteresis half-width \(epsilon\).*temperature distance/i,
      /retains.*inside the band.*upper threshold.*low.*lower threshold.*high/i,
      /hysteresis.*measurement noise.*false switching/i,
      /limit cycle.*repeatable closed-loop oscillation/i,
      /measured process amplitude \(a\).*half.*upper.*lower.*extrema/i,
      /measured relay period \(Pu\).*equivalent-direction.*crossings/i,
      /at least six complete.*amplitude.*period variation.*10 percent/i,
      /describing-function approximation.*assumes.*repeatable.*hysteresis.*sampling/i,
      /signal-to-noise ratio.*amplitude.*noise.*at least 5/i,
      /0\.20.*relay amplitude.*rejected.*No tuning candidate/i,
      /38 percent.*2 percent.*50\.08.*49\.92/i,
      /Control Studio.*Monitor.*Test.*Tune/i,
      /Do not.*physical plant.*Apply/i,
      /restor.*20 percent/i,
      /Lesson 44.*robustness-speed/i,
    ]) assert.match(readme, requirement);
    assert.match(readme, /Ku = 4\s*\*\s*d\/\(pi\s*\*\s*a\)/);
    assert.match(program, /QualifiedTemperature >= LREAL#50\.08/);
    assert.match(program, /QualifiedTemperature <= LREAL#49\.92/);
    assert.match(program,
      /IF QualifiedRelayHigh THEN\s+QualifiedRelayApplied := LREAL#20\.0 \+ RelayAmplitude;/);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 0);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 2);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("teaches model-qualified SIMC targets across self-regulating and integrating plants", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "06-pid-tuning-methods", "44-model-based-tuning"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 44 must ship its executable heater, tank, and dryer project");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-model-based-tuning");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-model-based-tuning"), lesson);
    assert.ok(catalogEntry, "Lesson 44 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/06-pid-tuning-methods/44-model-based-tuning");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 44);
    assert.deepStrictEqual(lesson.prerequisites, ["control-relay-autotuning"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.change.applied, false);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_simc_targets_are_model_qualified_and_remain_unapplied");
    assertRetainedTuningEvidence(lessonRoot, source, "model-based-tuning-comparison");
    assertCsvContract(path.join(lessonRoot, "figures", "model-based-tuning-comparison-summary.csv"), [
      "record_type", "variation", "candidate", "plant", "model_kind", "lambda_s", "kp",
      "ti_s", "ki", "controller_form", "operating_region", "settling_status",
      "settling_time_s", "overshoot_percent", "final_window_error", "cumulative_mv_movement_pp",
      "peak_requested_mv_percent", "peak_applied_mv_percent", "time_at_limit_s",
      "qualification", "applied",
    ]);
    assertCsvContract(path.join(lessonRoot, "figures", "model-based-tuning-comparison-trace.csv"), [
      "candidate", "plant", "model_kind", "pv", "pv_unit", "normalized_pv",
      "requested_mv_percent", "applied_mv_percent", "quality", "event",
    ]);

    for (const requirement of [
      /## What we need[\s\S]*calculate and validate.*Fast.*Balanced.*Robust.*eligible models.*unapplied/i,
      /desired closed-loop time \(lambda\).*seconds.*smaller.*faster.*larger.*robust/i,
      /self-regulating plant gain \(K\).*output change.*input change/i,
      /integrating rate gain \(Kprime\).*per second.*not.*steady-state gain/i,
      /dominant time constant \(T\).*seconds.*effective dead time \(L\).*seconds/i,
      /Kp = T \/ \(K.*lambda \+ L\)/i,
      /Kp = 1 \/ \(Kprime.*lambda \+ L\)/i,
      /Ti = 4.*lambda \+ L/i,
      /parallel integral gain \(Ki\).*Ki = Kp \/ Ti/i,
      /internal-model-control.*feedback PI.*not.*model-predictive/i,
      /Fast.*Balanced.*Robust.*numeric.*seconds/i,
      /heater.*0\.20.*5\.0.*1\.0/i,
      /buffer tank.*0\.05.*0\.5.*1\.0.*valve lag/i,
      /dryer.*0\.40.*20\.0.*8\.0/i,
      /Control Studio.*Monitor.*Tune.*Validate/i,
      /change only.*response target.*same.*plant.*initial state.*events.*sample/i,
      /rise time.*10.*90.*settling.*Not settled/i,
      /cumulative applied.*sum.*absolute.*percentage points/i,
      /20 percent.*plant gain.*50 percent.*effective delay/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 45.*stability/i,
    ]) assert.match(readme, requirement);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 3);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.doesNotMatch(program, /AcceptedCandidate|Winner|RiseTime|SettlingTime/);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_200);
  });
});
