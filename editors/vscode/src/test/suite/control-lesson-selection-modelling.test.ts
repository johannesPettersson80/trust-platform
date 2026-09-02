import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { parseControlLesson } from "../../examples/model";
import { canonicalLessonReadmeHeadings, pedagogicalCoreWordCount, pedagogicalMarkdownText } from "./controlLessonAssertions";

function assertCanonicalRenderedLesson(source: string, lessonRoot: string, artifactStem: string): void {
  for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^## ${heading}$`, "m"));
  for (const suffix of [".svg", "-run.csv", "-summary.csv"]) {
    const artifact = path.join(lessonRoot, "figures", `${artifactStem}${suffix}`);
    assert.ok(fs.existsSync(artifact), `retained lesson artifact must exist: ${artifact}`);
  }
  const svg = fs.readFileSync(
    path.join(lessonRoot, "figures", `${artifactStem}.svg`),
    "utf8",
  );
  const title = svg.match(/<title id="title">([^<]+)<\/title>/)?.[1] ?? "";
  assert.ok(title.length > 0 && title.length <= 55, `plot title must fit the fixed SVG canvas: ${title}`);
  assert.match(source, new RegExp(`!\\[[^\\]]+\\]\\(figures/${artifactStem}\\.svg\\)`));
  assert.match(source, new RegExp(`\\[Retained run\\]\\(figures/${artifactStem}-run\\.csv\\)`));
  assert.match(source, new RegExp(`\\[Decision summary\\]\\(figures/${artifactStem}-summary\\.csv\\)`));
}

function assertSvgTitleFits(artifact: string): void {
  const svg = fs.readFileSync(artifact, "utf8");
  const title = svg.match(/<title id="title">([^<]+)<\/title>/)?.[1] ?? "";
  assert.ok(title.length > 0 && title.length <= 55,
    `plot title must fit the fixed SVG canvas: ${title}`);
}

function assertTraceSeriesHasNonzeroValue(trace: string, series: string): void {
  const rows = fs.readFileSync(trace, "utf8").trim().split(/\r?\n/).slice(1)
    .map(row => row.split(","));
  assert.ok(rows.some(row => row[1] === series && Math.abs(Number(row[4])) > 1e-9),
    `${series} must retain a nonzero measured value`);
}

function assertTraceContainsSeries(trace: string, series: string): void {
  const rows = fs.readFileSync(trace, "utf8").trim().split(/\r?\n/).slice(1)
    .map(row => row.split(","));
  assert.ok(rows.some(row => row[1] === series), `${trace} must retain ${series}`);
}

function assertClassicalEvidence(repositoryRoot: string, lesson: string, stem: string,
  summaryColumns: readonly string[], requiredSeries: readonly string[], extraStems: readonly string[] = []): void {
  const root = path.join(repositoryRoot, "examples", "control",
    "07-classical-stability-and-frequency-design", lesson);
  const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
  const figure = path.join(root, "figures", `${stem}.svg`);
  const trace = path.join(root, "figures", `${stem}-trace.csv`);
  const summary = path.join(root, "figures", `${stem}-summary.csv`);
  for (const artifact of [figure, trace, summary])
    assert.ok(fs.existsSync(artifact), `classical lesson evidence must exist: ${artifact}`);
  assertSvgTitleFits(figure);
  assert.match(readme, new RegExp(`!\\[[^\\]]+\\]\\(figures/${stem}\\.svg\\)`));
  assert.match(readme, new RegExp(`\\[Complete trace\\]\\(figures/${stem}-trace\\.csv\\)`));
  assert.match(readme, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
  const traceRows = fs.readFileSync(trace, "utf8").trim().split(/\r?\n/).map(row => row.split(","));
  assert.deepStrictEqual(traceRows[0], ["panel", "series", "point", "x", "y", "x_unit", "y_unit"]);
  assert.ok(traceRows.length > 10, `${lesson} must retain plotted numerical evidence`);
  assert.ok(traceRows.every(row => row.length === traceRows[0].length), `${lesson} trace must be rectangular`);
  const retainedSeries = new Set(traceRows.slice(1).map(row => row[1]));
  for (const series of requiredSeries)
    assert.ok(retainedSeries.has(series), `${lesson} must retain ${series}`);
  const summaryRows = fs.readFileSync(summary, "utf8").trim().split(/\r?\n/).map(row => row.split(","));
  assert.deepStrictEqual(summaryRows[0], summaryColumns);
  assert.ok(summaryRows.length >= 3, `${lesson} must retain competing cases`);
  assert.ok(summaryRows.every(row => row.length === summaryRows[0].length), `${lesson} summary must be rectangular`);
  for (const extraStem of extraStems) {
    const extraFigure = path.join(root, "figures", `${extraStem}.svg`);
    const extraTrace = path.join(root, "figures", `${extraStem}-trace.csv`);
    assert.ok(fs.existsSync(extraFigure), `${lesson} must retain ${extraStem}.svg`);
    assert.ok(fs.existsSync(extraTrace), `${lesson} must retain ${extraStem}-trace.csv`);
    assertSvgTitleFits(extraFigure);
    assert.match(readme, new RegExp(`!\\[[^\\]]+\\]\\(figures/${extraStem}\\.svg\\)`));
    assert.match(readme, new RegExp(`\\[Complete [^\\]]+ trace\\]\\(figures/${extraStem}-trace\\.csv\\)`));
  }
}

suite("Controller selection and modelling curriculum lessons", () => {
  test("retains classical stability and frequency-domain evidence for Lessons 45 through 50", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    assertClassicalEvidence(repositoryRoot, "45-crane-routh-stability", "crane-routh-evidence", [
      "case", "gain", "polynomial", "poles", "right_half_plane_poles", "predicted_class",
      "sample_s", "early_window_s", "late_window_s", "peak_position_m", "early_envelope_m",
      "late_envelope_m", "late_early_ratio", "peak_requested_force_percent",
      "peak_applied_force_percent", "applied_force_travel_pp", "limit_contact", "aborted",
      "stop_reason", "restoration", "seed", "applied", "evidence_class",
    ], ["K = 4 position", "K = 4 requested force", "K = 4 applied force",
      "K = 6 position", "K = 6 requested force", "K = 6 applied force",
      "K = 8 position", "K = 8 requested force", "K = 8 applied force"]);
    assertClassicalEvidence(repositoryRoot, "46-servo-root-locus", "servo-root-locus-evidence", [
      "case", "controller", "poles", "damping_ratio", "overshoot_percent", "settling_s",
      "final_window_s", "final_error_mm", "peak_requested_current_percent",
      "peak_applied_current_percent", "applied_current_travel_pp", "time_at_limit_s",
      "saturated", "stop_reason", "requirements_pass", "seed", "applied", "evidence_class",
    ], ["P root-locus upper", "P root-locus lower", "Lead root-locus branch 1",
      "Lead root-locus branch 2", "Lead root-locus branch 3", "P position",
      "P requested current", "P applied current", "Lead position", "Lead requested current",
      "Lead applied current"]);
    assertTraceSeriesHasNonzeroValue(path.join(repositoryRoot, "examples", "control", "07-classical-stability-and-frequency-design",
      "46-servo-root-locus", "figures", "servo-root-locus-evidence-trace.csv"), "P requested current");
    assertTraceSeriesHasNonzeroValue(path.join(repositoryRoot, "examples", "control", "07-classical-stability-and-frequency-design",
      "46-servo-root-locus", "figures", "servo-root-locus-evidence-trace.csv"), "Lead requested current");
    assertClassicalEvidence(repositoryRoot, "47-flexible-drive-bode", "flexible-drive-bode-evidence", [
      "case", "record_type", "controller", "model_revision", "controller_revision", "sample_s",
      "frequency_rad_s", "input_amplitude_percent",
      "output_amplitude_rpm", "magnitude_rpm_per_percent", "raw_phase_deg",
      "corrected_phase_deg", "fit_window_s", "coherence", "gain_crossovers_rad_s",
      "phase_margins_deg", "phase_crossover_rad_s", "gain_margin", "delay_margin_s",
      "resonance_peak_rpm_per_percent", "bandwidth_rad_s", "perturbation", "qualified",
      "stop_reason", "seed", "applied", "evidence_class",
    ], ["Low requested torque", "Low applied torque", "Low load speed",
      "Resonant requested torque", "Resonant applied torque", "Resonant load speed",
      "High requested torque", "High applied torque", "High load speed",
      "Unshaped magnitude", "Shaped magnitude", "Unshaped phase", "Shaped phase"],
      ["flexible-drive-controller-bode-evidence"]);
    assertClassicalEvidence(repositoryRoot, "48-delayed-thermal-nyquist", "delayed-thermal-nyquist-evidence", [
      "case", "delay_s", "model_revision", "controller_revision", "sample_s", "setpoint_deg_c",
      "predicted_equilibrium_deg_c", "crossover_rad_s", "wrapped_phase_deg",
      "unwrapped_phase_deg", "open_loop_rhp_poles", "ccw_encirclements",
      "closed_loop_rhp_poles", "sweep_min_rad_s", "sweep_max_rad_s", "points_per_decade",
      "endpoint_magnitude", "minimum_distance_to_minus_one", "early_window_s", "late_window_s",
      "early_envelope_deg_c", "late_envelope_deg_c", "peak_requested_heater_percent",
      "peak_applied_heater_percent", "limit_contact", "aborted", "stop_reason", "restoration",
      "seed", "predicted_class", "applied", "evidence_class",
    ], ["5 s Nyquist contour", "25 s Nyquist contour", "60 s Nyquist contour", "Critical -1 point",
      "5 s equilibrium error", "5 s requested heater", "5 s applied heater",
      "25 s equilibrium error", "25 s requested heater", "25 s applied heater",
      "60 s equilibrium error", "60 s requested heater", "60 s applied heater"]);
    assertClassicalEvidence(repositoryRoot, "49-motor-sensitivity", "motor-sensitivity-evidence", [
      "case", "model_revision", "controller_revision", "maximum_sensitivity",
      "maximum_complementary_sensitivity", "complementarity_residual", "crossover_rad_s",
      "phase_margin_deg", "bandwidth_rad_s", "s_at_0_01", "gs_at_0_01", "t_at_10",
      "cs_at_10", "overshoot_percent", "settling_s", "load_departure_rpm",
      "requested_torque_travel_pp", "applied_torque_travel_pp",
      "quiet_requested_torque_travel_pp", "limit_contact", "aborted", "stop_reason",
      "qualified", "seed", "applied", "evidence_class",
    ], ["Baseline L", "Baseline S", "Baseline T", "Baseline GS", "Baseline CS",
      "Shaped L", "Shaped S", "Shaped T", "Shaped GS", "Shaped CS",
      "Baseline true speed", "Baseline requested torque", "Baseline applied torque",
      "Shaped true speed", "Shaped requested torque", "Shaped applied torque"],
      ["motor-sensitivity-response-evidence"]);
    assertClassicalEvidence(repositoryRoot, "50-position-z-plane", "position-z-plane-evidence", [
      "case", "continuous_model_revision", "discrete_model_revision", "controller_revision",
      "plant_sample_s", "controller_sample_s", "hold_convention", "computation_delay_samples",
      "ad", "bd", "sampled_numerator", "sampled_denominator", "poles", "pole_magnitudes",
      "maximum_pole_magnitude", "unit_circle_clearance", "sampling_zero", "nyquist_rad_s",
      "aliased_frequency_rad_s", "overshoot_percent", "settling_s", "final_error_mm",
      "requested_force_travel_pp", "applied_force_travel_pp", "saturation_duration_s",
      "aborted", "abort_trigger", "controlled_stop_complete", "stop_reason", "qualified",
      "seed", "applied", "evidence_class",
    ], ["Unit circle", "Corrected root-locus branch 1", "Corrected root-locus branch 2",
      "Corrected root-locus branch 3", "10 ms position", "100 ms unchanged position",
      "100 ms corrected position"], ["position-z-plane-force-evidence"]);
    assertTraceContainsSeries(path.join(repositoryRoot, "examples", "control", "07-classical-stability-and-frequency-design",
      "50-position-z-plane", "figures", "position-z-plane-force-evidence-trace.csv"),
    "100 ms corrected pending");
  });
  test("teaches minimum-sufficient controller selection from requirements", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "04-controller-selection-and-plant-models", "31-controller-selection"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 31 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 31 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-controller-selection"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 31);
    assert.deepStrictEqual(lesson.prerequisites, ["control-booster-pump-invalid-data"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_selects_the_minimum_sufficient_structure_for_four_real_loops");
    for (const requirement of [
      /controlled-variable accuracy.*acceptable band.*permanent load error/i,
      /self-regulating plant.*integrating plant.*damping requirement/i,
      /actuator resolution.*binary.*continuous.*one-sided.*bidirectional/i,
      /servo objective.*regulatory objective.*noise sensitivity/i,
      /minimum sufficient controller.*least complex.*every declared requirement/i,
      /cold-storage room.*binary compressor.*ON\/OFF.*hysteresis.*five-second/i,
      /water heater.*self-regulating.*PI.*constant heat-loss/i,
      /buffer tank.*integrates net flow.*20 percent.*P control/i,
      /unknown.*outflow.*can justify PI/i,
      /positioning table.*four newton-metre.*PID.*damping.*zero final/i,
      /50 percent.*30 degrees/i,
      /2\.5 seconds/i,
      /0\.2 radian.*PD.*PID/i,
      /requirement coverage.*count.*requirements passed/i,
      /complexity.*enabled dynamic mechanisms.*eligible/i,
      /1\.492.*2\.502.*0\.0159.*0\.0000/i,
      /not.*automatic structure-selection algorithm/i,
      /Lesson 32.*plant response classes/i,
    ]) assert.match(readme, requirement);
    assertCanonicalRenderedLesson(source, lessonRoot, "controller-selection");
    const candidateMatrixPath = path.join(lessonRoot, "figures", "controller-selection-candidates.csv");
    assert.ok(fs.existsSync(candidateMatrixPath), "Lesson 31 must retain every candidate decision");
    const candidateRows = fs.readFileSync(candidateMatrixPath, "utf8").trim().split(/\r?\n/);
    assert.strictEqual(candidateRows.length, 21, "four plants by five candidate structures plus header");
    for (const structure of ["ON/OFF", "P", "PI", "PD", "PID"]) {
      assert.ok(candidateRows.some(row => row.includes(`,${structure},`)), `missing ${structure} candidate`);
    }
    assert.strictEqual(candidateRows.filter(row => row.includes(",true,selected,")).length, 4);
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 3,
      "heater, tank, and table each require one product PID instance");
    assert.match(program, /ColdRoomStructure := 1;/);
    assert.match(program, /HeaterStructure := 3;/);
    assert.match(program, /TankStructure := 2;/);
    assert.match(program, /TableStructure := 5;/);
    assert.match(program, /ColdRoomCompressor := LREAL#100\.0;/);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches physical plant-response classification before model fitting", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "04-controller-selection-and-plant-models", "32-plant-response-classes"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 32 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 32 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-plant-response-classes"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 32);
    assert.deepStrictEqual(lesson.prerequisites, ["control-controller-selection"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_classifies_four_physical_plant_responses_before_modelling");
    for (const requirement of [
      /operating point.*manipulated-input change.*process-output change.*control direction/i,
      /stored mass.*stored energy.*stored momentum/i,
      /time constant.*continuing slope.*integration.*response delay/i,
      /natural frequency.*damping.*resonance.*observation window/i,
      /self-regulating.*finite equilibrium.*integrating.*nonzero slope/i,
      /delayed.*no output response.*oscillatory.*crosses.*eventual value/i,
      /normalized experiment command.*20-percentage-point heater.*10-percentage-point.*tank.*one-newton-metre/i,
      /physical commands.*PV units/i,
      /54 degrees.*53\.93 degrees/i,
      /62 centimetres.*12 seconds/i,
      /four seconds.*84\.91 degrees/i,
      /52\.7 percent.*overshoot/i,
      /response delay.*first PV change.*sensor resolution/i,
      /late slope.*final window.*PV units per second/i,
      /crossing count.*steady value/i,
      /53\.927.*62\.000.*84\.908.*1\.527.*four crossings/i,
      /do not fit.*do not tune/i,
      /Lesson 33.*conservation laws/i,
    ]) assert.match(readme, requirement);
    assertCanonicalRenderedLesson(source, lessonRoot, "plant-response-classes");
    assert.ok(fs.existsSync(path.join(lessonRoot, "figures", "plant-response-classes-first-second.svg")),
      "Lesson 32 must retain the ambiguous first-second view");
    const classificationPath = path.join(lessonRoot, "figures", "plant-response-classes-worksheet.csv");
    assert.ok(fs.existsSync(classificationPath), "Lesson 32 must retain the first-second ambiguity record");
    const classifications = fs.readFileSync(classificationPath, "utf8");
    assert.match(classifications,
      /response_id,first_second_observation,classification_status,still_plausible_classes,learner_commitment,reveal_status/);
    assert.strictEqual((classifications.match(/,ambiguous,/gm) ?? []).length, 4);
    assert.strictEqual((classifications.match(/,not_revealed$/gm) ?? []).length, 4);
    assert.ok(!/heated vessel|buffer tank|conveyor oven|flexible drive/i.test(classifications),
      "the first-second worksheet must not contain the answer key");
    assert.match(source, /<details>[\s\S]*<summary>Reveal the plant identities[\s\S]*<\/details>/i);
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0,
      "open-loop classification must not hide a feedback controller");
    assert.match(program, /HeaterCommand := LREAL#20\.0 \+ LREAL#20\.0 \* BoundedExperimentInput;/);
    assert.match(program, /TankInflowTrim := LREAL#10\.0 \* BoundedExperimentInput;/);
    assert.match(program, /OvenCommand := LREAL#20\.0 \+ LREAL#20\.0 \* BoundedExperimentInput;/);
    assert.match(program, /DriveTorque := LREAL#1\.0 \* BoundedExperimentInput;/);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches tank mass and heater energy conservation before equations", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "04-controller-selection-and-plant-models", "33-tank-heater-conservation"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 33 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 33 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-tank-heater-conservation"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 33);
    assert.deepStrictEqual(lesson.prerequisites, ["control-plant-response-classes"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_conservation_balances_predict_tank_and_heater");

    for (const requirement of [
      /derive.*buffer tank.*heated vessel.*conservation laws/i,
      /stored quantity.*state variable.*accumulates.*entering.*leaving/i,
      /tank cross-sectional area \(A\).*liquid level \(h\).*inlet volumetric flow \(q_in\).*outlet volumetric flow \(q_out\)/i,
      /litres.*cubic metres.*mass-volume balance/i,
      /thermal capacitance \(C\).*vessel temperature \(T\).*ambient temperature \(T_ambient\).*heating power \(P_heater\).*heat-loss coefficient \(H\)/i,
      /thermal capacitance.*electrical heating power.*heat-loss coefficient.*energy balance/i,
      /equilibrium.*zero accumulation.*static gain.*time constant.*transport delay/i,
      /0\.5 square metres.*12 litres per second.*10 litres per second/i,
      /0\.002 cubic metres per second.*0\.4 centimetres per second.*58 centimetres/i,
      /equal.*10 litres per second.*zero.*slope/i,
      /50 kilojoules per degree Celsius.*10 kilowatts per degree Celsius.*200 kilowatts/i,
      /20-percent.*40 kilowatts.*24 degrees Celsius/i,
      /0\.2 degrees Celsius per percentage point.*five seconds.*22\.53 degrees Celsius/i,
      /constant area.*perfectly mixed.*no overflow/i,
      /sensor dynamics.*evaporation.*nonlinear gravity outflow.*absent/i,
      /simulation evidence.*not physical-plant evidence/i,
      /tank level-rate error.*late-window slope.*0\.4 centimetres per second/i,
      /equal-flow drift.*maximum level minus minimum level/i,
      /six-second prediction error.*final-window equilibrium error/i,
      /0\.4000.*58\.000.*0\.000000.*22\.528.*23\.988/i,
      /do not Identify, Tune, or Apply/i,
      /Lesson 34.*nonlinear gravity outflow.*local linear model/i,
    ]) assert.match(readme, requirement);
    assertCanonicalRenderedLesson(source, lessonRoot, "tank-heater-conservation");
    const conservationRun = fs.readFileSync(
      path.join(lessonRoot, "figures", "tank-heater-conservation-run.csv"), "utf8");
    assert.match(conservationRun.split(/\r?\n/, 1)[0], /units,quality,model_parameters/);
    assert.match(conservationRun, /tank_flow=L\/s;tank_level=cm;heater_command=percent;heater_temperature=degC,good,A=0\.5m2;C=50kJ\/degC;H=10kW\/degC;Pmax=200kW;dead_time=1s/);

    const tankNames = readme.indexOf("tank cross-sectional area (A)");
    const tankEquation = readme.indexOf("A * dh/dt = q_in - q_out");
    const heaterNames = readme.indexOf("thermal capacitance (C)");
    const heaterEquation = readme.indexOf("C * dT/dt = P_heater - H * (T - T_ambient)");
    assert.ok(tankNames >= 0 && tankNames < tankEquation,
      "tank quantities must be explained in words before the balance equation");
    assert.ok(heaterNames >= 0 && heaterNames < heaterEquation,
      "heater quantities must be explained in words before the balance equation");
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0,
      "first-principles modelling must not hide a feedback controller");
    assert.match(program, /TankNetFlowCubicMetresPerSecond := TankNetFlowLitresPerSecond \* LREAL#0\.001;/);
    assert.match(program, /HeaterTimeConstant :=\s*HeaterThermalCapacitance \/ HeaterHeatLossCoefficient;/);
    assert.match(simulation, /id = "constant-area-buffer-tank"[\s\S]*model = "integrator"/);
    assert.match(simulation, /id = "electrically-heated-vessel"[\s\S]*model = "first_order_dead_time"/);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches gravity-tank linearization as a validated local model", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "04-controller-selection-and-plant-models", "34-gravity-tank-linearization"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 34 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 34 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-gravity-tank-linearization"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 34);
    assert.deepStrictEqual(lesson.prerequisites, ["control-tank-heater-conservation"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_gravity_tank_linearization_is_local_and_unit_consistent");
    for (const requirement of [
      /derive.*first-order transfer function.*gravity-drained tank.*three operating points/i,
      /same inlet-flow change.*different equilibrium.*response speed/i,
      /tank area \(A\).*level \(h\).*inlet flow \(q\).*outlet coefficient \(c\)/i,
      /physical balance.*square root.*nonlinear/i,
      /operating point.*constant inlet flow.*constant level.*zero accumulation/i,
      /level deviation \(delta_h\).*inlet-flow deviation \(delta_q\)/i,
      /Taylor approximation.*curved outlet-flow.*tangent/i,
      /Laplace transform.*zero initial deviation.*DeltaH\(s\) \/ DeltaQ\(s\)/i,
      /transfer function.*gain.*time constant.*order.*pole.*zero.*dead time/i,
      /one pole.*-1\/tau.*no finite zero.*no dead time/i,
      /0\.25.*0\.50.*0\.81 metres/i,
      /0\.010000.*0\.014142.*0\.018000 cubic metres per second/i,
      /50\.000.*70\.711.*90\.000/i,
      /25\.000.*35\.355.*45\.000 seconds/i,
      /5\.000.*7\.071.*9\.000 centimetres/i,
      /0\.250 centimetres.*small-step/i,
      /local-model prediction error.*absolute.*nonlinear level.*local-model level/i,
      /0\.30 centimetres.*validity.*6\.241 centimetres.*outside/i,
      /do not Identify, Tune, or Apply/i,
      /not.*gain scheduling/i,
      /Lesson 35.*poles and zeros/i,
    ]) assert.match(readme, requirement);
    assertCanonicalRenderedLesson(source, lessonRoot, "gravity-tank-linearization");
    const linearizationRun = fs.readFileSync(
      path.join(lessonRoot, "figures", "gravity-tank-linearization-run.csv"), "utf8");
    assert.match(linearizationRun.split(/\r?\n/, 1)[0],
      /operating_point_cm,gain_cm_per_lps,time_constant_s,pole_per_s,prediction_error_cm,validity_decision/);
    assert.match(linearizationRun, /inside_declared_local_bound/);
    assert.match(linearizationRun, /outside_declared_local_bound/);
    assert.ok(readme.indexOf("tank area (A)") < readme.indexOf("A * dh/dt"));
    assert.ok(source.indexOf("level deviation ($\\Delta h$)") <
      source.indexOf("\\frac{d\\Delta h}{dt}"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.ok(!program.includes("DeadTime"));
    assert.match(program, /SQRT\(LowOperatingLevelMetres\)/);
    assert.strictEqual((simulation.match(/model = "gravity_tank"/g) ?? []).length, 3);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches pole-zero interpretation from six executable plant responses", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "04-controller-selection-and-plant-models", "35-pole-zero-plant-classes"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 35 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 35 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-pole-zero-plant-classes"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 35);
    assert.deepStrictEqual(lesson.prerequisites, ["control-gravity-tank-linearization"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_pole_zero_maps_predict_six_real_plant_responses");

    for (const requirement of [
      /heated vessel.*storage tank.*flexible.*drive.*overhead crane.*magnetic bearing.*boiler drum/i,
      /pole.*denominator.*natural mode.*zero.*numerator.*input.*output/i,
      /complex plane.*real part.*imaginary part.*left half-plane.*right half-plane.*origin/i,
      /negative-real pole.*decay.*pole at the origin.*integration.*positive-real pole.*growth/i,
      /complex-conjugate.*oscillation.*real part.*imaginary part/i,
      /natural frequency.*damping ratio.*dominant poles.*decay most slowly/i,
      /left-half-plane zero.*right-half-plane zero.*initial response.*opposite/i,
      /inverse response.*dead time.*immediate.*wrong direction.*no response/i,
      /pole-zero cancellation.*uncertain.*internally present.*unstable.*lightly damped/i,
      /heater.*-0\.2.*63\.2 percent.*five seconds/i,
      /storage tank.*pole at the origin.*0\.2 centimetres per second/i,
      /flexible drive.*-0\.2.*0\.980.*52\.7 percent/i,
      /overhead crane.*-0\.1.*-1.*4.*slow pole.*dominates/i,
      /magnetic bearing.*\+0\.4.*abort.*10 millimetres/i,
      /boiler drum.*-0\.05.*-0\.5.*\+0\.0625.*initially negative.*positive/i,
      /monotonicity.*successive output increments.*late slope.*overshoot.*crossing count/i,
      /dominant-tail decay rate.*unstable growth ratio.*inverse-response minimum/i,
      /Not settled.*tank.*magnetic bearing/i,
      /0\.632.*0\.200.*1\.527.*nine crossings.*0\.607.*9\.3.*-0\.505.*0\.554/i,
      /separate.*bearing.*abort-bounded/i,
      /do not Identify, Tune, or Apply/i,
      /Lesson 36.*state space.*internal stored quantities/i,
    ]) assert.match(readme, requirement);
    assertCanonicalRenderedLesson(source, lessonRoot, "pole-zero-plant-classes");
    const mapPath = path.join(lessonRoot, "figures", "pole-zero-plant-classes-map.svg");
    assert.ok(fs.existsSync(mapPath), "Lesson 35 must ship the hidden pole-zero matching worksheet");
    assert.match(source, /!\[[^\]]*pole-zero[^\]]*worksheet[^\]]*\]\(figures\/pole-zero-plant-classes-map\.svg\)/i);
    const poleZeroMap = fs.readFileSync(mapPath, "utf8");
    assert.strictEqual((poleZeroMap.match(/data-symbol="pole"/g) ?? []).length, 10);
    assert.strictEqual((poleZeroMap.match(/data-symbol="zero"/g) ?? []).length, 1);
    for (const label of ["A", "B", "C", "D", "E", "F"]) {
      assert.match(poleZeroMap, new RegExp(`>Map ${label}<`));
    }
    for (const hiddenIdentity of ["heater", "tank", "drive", "crane", "bearing", "boiler"]) {
      assert.ok(!poleZeroMap.toLowerCase().includes(hiddenIdentity),
        `matching worksheet must hide the ${hiddenIdentity} identity`);
    }
    assert.ok(readme.indexOf("pole (p)") < readme.indexOf("p = -0.2"));
    assert.ok(readme.indexOf("zero (z)") < readme.indexOf("z = +0.0625"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.match(program, /IF BearingExperimentEnabled THEN/);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 6);
    assert.match(simulation, /a = \[\[0\.4\]\]/);
    assert.match(simulation, /c = \[\[2\.0, -1\.0\]\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches state space by revealing hidden water in interconnected tanks", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "04-controller-selection-and-plant-models", "36-two-tank-state-space"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 36 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 36 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);

    assert.deepStrictEqual(parseControlLesson(lesson, "control-two-tank-state-space"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 36);
    assert.deepStrictEqual(lesson.prerequisites, ["control-pole-zero-plant-classes"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_two_tank_state_space_exposes_hidden_stored_water");
    for (const requirement of [
      /build.*state-space model.*two interconnected.*tanks.*internal levels/i,
      /same pump.*downstream-level reading.*different upstream.*different future/i,
      /state variable.*present value.*future input.*predict future behavior/i,
      /upstream level \(h1\).*downstream level \(h2\).*state vector \(x\)/i,
      /pump inlet flow \(u\).*output vector \(y\)/i,
      /vector.*ordered collection.*matrix.*table of coefficients.*units/i,
      /A1.*A2.*1 square metre.*k12.*0\.5.*k2.*0\.2/i,
      /upstream accumulation.*downstream accumulation.*equations/i,
      /state vector.*pump inlet flow.*output vector.*state equation.*output equation/i,
      /first row.*upstream.*second row.*downstream.*output rows/i,
      /equilibrium.*zero accumulation.*0\.1 cubic metres per second.*20 centimetres.*50 centimetres/i,
      /eigenvalue.*natural.*-0\.5.*-0\.2.*poles/i,
      /Empty.*\[0\.0, 0\.0\].*Upstream charged.*\[0\.2, 0\.0\]/i,
      /same initial downstream.*zero input.*10\.9 centimetres.*3\.05 seconds/i,
      /hidden-state downstream separation.*absolute difference/i,
      /20\.000.*50\.000.*10\.857.*3\.10/i,
      /do not Identify, Tune, or Apply/i,
      /no.*observer.*estimator.*state feedback.*optimal.*MPC/i,
      /Lesson 37.*safe.*informative.*experiment/i,
    ]) assert.match(readme, requirement);
    assertCanonicalRenderedLesson(source, lessonRoot, "two-tank-state-space");
    const predictionPath = path.join(lessonRoot, "figures", "two-tank-hidden-prediction.svg");
    assert.ok(fs.existsSync(predictionPath), "Lesson 36 must ship a downstream-only prediction view");
    const predictionPrompt = fs.readFileSync(predictionPath, "utf8");
    assert.match(predictionPrompt, /data-stage="pre-commit"/);
    assert.ok(!predictionPrompt.includes("<polyline"),
      "the pre-commit prompt must not reveal either future trace");
    const predictionRecord = fs.readFileSync(
      path.join(lessonRoot, "figures", "two-tank-hidden-prediction-summary.csv"), "utf8");
    assert.match(predictionRecord,
      /prediction_id,visible_case_a_downstream_cm,visible_case_b_downstream_cm,learner_commitment,learner_reason,reveal_status/);
    assert.match(predictionRecord, /A_vs_B,0\.000000,0\.000000,,,not_revealed/);
    assert.ok(!/rises|charged|peak/i.test(predictionRecord),
      "the pre-commit record must not contain the measured answer");
    const stateRun = fs.readFileSync(
      path.join(lessonRoot, "figures", "two-tank-state-space-run.csv"), "utf8");
    assert.match(stateRun.split(/\r?\n/, 1)[0],
      /state_order,matrix_a,matrix_b,matrix_c,matrix_d,equilibrium_error_cm,hidden_state_separation_cm/);
    assert.match(source, /<details>[\s\S]*<summary>Reveal the upstream states[\s\S]*<\/details>/i);
    assert.ok(readme.indexOf("state vector (x)") < readme.indexOf("dx/dt = A * x + B * u"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.ok(!/Observer|Estimator|MPC/.test(program));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.strictEqual((simulation.match(/a = \[\[-0\.5, 0\.0\], \[0\.5, -0\.2\]\]/g) ?? []).length, 3);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches safe heat-exchanger experiment design before data collection", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "05-experiments-identification-and-validation", "37-safe-heat-exchanger-experiment"
    );
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 37 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 37 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find(
      (entry: { id: string }) => entry.id === "control-safe-heat-exchanger-experiment"
    );

    assert.deepStrictEqual(parseControlLesson(lesson, "control-safe-heat-exchanger-experiment"), lesson);
    assert.ok(catalogEntry, "Lesson 37 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/05-experiments-identification-and-validation/37-safe-heat-exchanger-experiment");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 37);
    assert.deepStrictEqual(lesson.prerequisites, ["control-two-tank-state-space"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_safe_heat_exchanger_plan_rejects_executes_and_restores");

    for (const requirement of [
      /design.*review.*execute.*abort.*restore.*plate heat exchanger/i,
      /simulation evidence.*not physical-plant evidence/i,
      /engineering question.*how much.*how quickly.*outlet temperature/i,
      /baseline.*before excitation.*steady/i,
      /excitation amplitude.*change.*baseline valve position.*excitation duration/i,
      /confounding disturbance.*another input change.*attribut/i,
      /baseline valve position \(MV0\).*planned valve change \(delta_MV\)/i,
      /baseline outlet temperature \(PV0\).*local process gain \(K\)/i,
      /time constant \(T\).*dead time \(L\)/i,
      /delta_PV_final = K delta_MV/i,
      /10-percentage-point.*5-degree-Celsius.*55-degree-Celsius/i,
      /3-second dead time.*three time constants.*60 seconds.*95 percent/i,
      /complete hold.*63 seconds.*3 seconds of dead time.*60 seconds/i,
      /soft experiment bound.*hard limit.*immediate abort/i,
      /40.*80 percent.*45.*58 degrees Celsius/i,
      /bad or stale.*nonfinite.*undeclared disturbance.*abort/i,
      /restore 60 percent.*restoration.*baseline/i,
      /Plan A.*60 to 70 percent.*63 seconds.*Plan B.*60 to 85 percent.*62\.5 degrees Celsius/i,
      /requested valve position.*applied valve position.*separate/i,
      /baseline temperature range.*largest minus smallest.*10-second baseline/i,
      /baseline slope.*last minus first baseline temperature.*degrees Celsius per second/i,
      /prediction error.*absolute difference.*predicted.*observed/i,
      /Not qualified.*baseline.*quality.*disturbance.*duration.*limit/i,
      /late-window.*within 0\.35 degrees Celsius.*5-degree prediction/i,
      /54\.676 degrees Celsius.*50\.099 degrees Celsius.*reason code 1/i,
      /aborted run.*safety evidence.*invalid identification evidence/i,
      /do not use Identify, Tune, or Apply/i,
      /Lesson 38.*excitation shapes/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(source.indexOf("baseline valve position ($MV_{0}$)") <
      source.indexOf("\\Delta PV_{\\mathrm{final}} = K \\cdot \\Delta MV"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.match(program, /PlanRejectReason/);
    assert.match(program, /AbortReason/);
    assert.match(program, /PhysicalExecutionEligible := FALSE/);
    assert.match(simulation, /id = "plate-heat-exchanger"[\s\S]*model = "first_order_dead_time"/);
    for (const artifact of [
      "safe-experiment-comparison.svg", "safe-experiment-comparison-trace.csv",
      "safe-experiment-comparison-summary.csv", "safe-experiment-plans.csv",
    ]) {
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", artifact)),
        `Lesson 37 must retain ${artifact}`);
      if (artifact.endsWith(".svg")) assertSvgTitleFits(path.join(lessonRoot, "figures", artifact));
      assert.ok(source.includes(`figures/${artifact}`), `Lesson 37 README must link ${artifact}`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches plant-matched excitation with heater and flexible-drive evidence", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "05-experiments-identification-and-validation", "38-excitation-methods");
    const descriptorPath = path.join(lessonRoot, "lesson.json");
    const readmePath = path.join(lessonRoot, "README.md");
    assert.ok(fs.existsSync(descriptorPath), "Lesson 38 descriptor must be shipped");
    assert.ok(fs.existsSync(readmePath), "Lesson 38 pedagogical README must be shipped");

    const lesson = JSON.parse(fs.readFileSync(descriptorPath, "utf8"));
    const source = fs.readFileSync(readmePath, "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-excitation-methods");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-excitation-methods"), lesson);
    assert.ok(catalogEntry, "Lesson 38 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/05-experiments-identification-and-validation/38-excitation-methods");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 38);
    assert.deepStrictEqual(lesson.prerequisites, ["control-safe-heat-exchanger-experiment"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_excitation_methods_match_heater_and_flexible_drive_dynamics");

    for (const requirement of [
      /select.*excitation.*slow water heater.*resonant flexible-drive servo/i,
      /step.*sustained.*low-frequency.*gain.*delay.*lag/i,
      /pulse.*restoration.*less time away.*operating point/i,
      /doublet.*positive and negative.*zero net input area/i,
      /relay excitation.*feedback-dependent.*oscillation.*Lesson 43.*rejected/i,
      /pseudo-random binary sequence \(PRBS\).*deterministic.*two input levels/i,
      /dwell time.*slow plant.*longer.*fast plant/i,
      /chirp.*sinusoidal.*frequency.*start.*end/i,
      /persistent excitation.*enough independent temporal content.*model structure/i,
      /signal amplitude \(A_signal\).*noise amplitude \(A_noise\).*signal-to-noise ratio \(SNR\)/i,
      /SNR = A_signal \/ A_noise/i,
      /5 percentage points.*1 degree Celsius.*0\.02.*SNR.*50/i,
      /heater PRBS dwell.*2\.5 seconds.*5-second thermal lag/i,
      /servo PRBS dwell.*0\.3 second.*three 100-millisecond task scans/i,
      /0\.2 to 2 hertz.*0\.95-hertz flexible mode/i,
      /requested.*applied excitation.*separate/i,
      /input total variation.*sum of absolute successive/i,
      /signed input area.*sampled sum.*sample interval/i,
      /output span.*maximum minus minimum/i,
      /heater information ratio.*temperature span.*noise/i,
      /servo resonance peak frequency.*largest qualified shaft-position amplitude/i,
      /rejected or clipped.*Not qualified/i,
      /do not Identify, Tune, or Apply/i,
      /heater.*low-pass.*servo.*stores energy.*flexible mode/i,
      /Lesson 39.*model structures/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(source.indexOf("signal amplitude") <
      source.indexOf("SNR = A_{\\mathrm{signal}} / A_{\\mathrm{noise}}"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.match(program, /CASE MethodSelection OF/);
    assert.match(program, /RejectReason := 4/);
    assert.match(program, /SIN\(ChirpPhase\)/);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 2);
    assert.match(simulation, /id = "water-heater"[\s\S]*model = "first_order_dead_time"/);
    assert.match(simulation, /id = "flexible-drive"[\s\S]*model = "second_order"/);
    for (const artifact of [
      "excitation-heater-comparison.svg", "excitation-heater-comparison-trace.csv",
      "excitation-heater-comparison-summary.csv", "excitation-servo-comparison.svg",
      "excitation-servo-comparison-trace.csv", "excitation-servo-comparison-summary.csv",
    ]) {
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", artifact)),
        `Lesson 38 must retain ${artifact}`);
      if (artifact.endsWith(".svg")) assertSvgTitleFits(path.join(lessonRoot, "figures", artifact));
      assert.ok(source.includes(`figures/${artifact}`), `Lesson 38 README must link ${artifact}`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches conveyor-dryer model selection from independent prediction", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "05-experiments-identification-and-validation", "39-dryer-model-selection");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-dryer-model-selection");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-dryer-model-selection"), lesson);
    assert.ok(catalogEntry, "Lesson 39 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/05-experiments-identification-and-validation/39-dryer-model-selection");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 39);
    assert.deepStrictEqual(lesson.prerequisites, ["control-excitation-methods"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_dryer_selects_simplest_model_from_independent_prediction");

    for (const requirement of [
      /fit.*first-order.*first-order-plus-dead-time.*second-order.*integrating.*conveyor dryer/i,
      /model structure.*form and number.*parameter estimate.*numerical value/i,
      /estimation dataset.*choose parameters.*validation dataset.*withheld/i,
      /measured output \(y_measured\).*model-predicted output \(y_predicted\)/i,
      /residual \(r\).*measured minus predicted.*r = y_measured - y_predicted/i,
      /root-mean-square prediction error \(RMSE\).*mean squared residual/i,
      /residual bias.*mean residual.*residual autocorrelation.*sample lag/i,
      /model complexity.*number and freedom.*dynamic parameters/i,
      /underfit.*too simple.*overfit.*noise or a disturbance/i,
      /bias-variance trade-off.*systematic error.*estimates vary/i,
      /first-order candidate.*gain.*lag.*no delay.*immediately/i,
      /first-order-plus-dead-time \(FOPDT\).*gain.*lag.*delay/i,
      /second-order candidate.*natural frequency.*damping.*no explicit delay/i,
      /integrating candidate.*output rate.*finite.*equilibrium/i,
      /parameter uncertainty.*early and late.*subset.*spread/i,
      /material validation improvement.*10 percent.*repeatable residual.*physically plausible/i,
      /10-percentage-point.*4-degree-Celsius.*80 to 84 degrees Celsius/i,
      /first 4 seconds.*first-order.*early residual bias/i,
      /same estimation window.*same weighting.*sample inclusion/i,
      /independent validation.*5-percentage-point pulse/i,
      /delay-window absolute bias.*first 4 seconds/i,
      /Not applicable.*Not identifiable.*Rejected/i,
      /FOPDT.*gain.*0\.03.*time constant.*1\.5.*dead time.*0\.5/i,
      /validation RMSE.*0\.10.*residual bias.*0\.03/i,
      /do not Tune or Apply/i,
      /not because.*simulator configuration.*lowest training error/i,
      /Lesson 40.*unidentifiable/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(source.indexOf("measured output ($y_{\\mathrm{measured}}$)") <
      source.indexOf("r = y_{\\mathrm{measured}} - y_{\\mathrm{predicted}}"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.match(simulation, /model = "first_order_dead_time"/);
    assert.match(simulation, /dead_time_s = 4\.0/);
    for (const artifact of [
      "dryer-model-estimation.svg", "dryer-model-estimation-trace.csv",
      "dryer-model-estimation-summary.csv",
      "dryer-model-comparison.svg", "dryer-model-comparison-trace.csv",
      "dryer-model-comparison-summary.csv", "dryer-model-residual.svg",
      "dryer-model-residual-trace.csv", "dryer-model-residual-summary.csv",
    ]) {
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", artifact)),
        `Lesson 39 must retain ${artifact}`);
      if (artifact.endsWith(".svg")) assertSvgTitleFits(path.join(lessonRoot, "figures", artifact));
      assert.ok(source.includes(`figures/${artifact}`), `Lesson 39 README must link ${artifact}`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_500);
  });

  test("teaches why heat-exchanger records can be unidentifiable", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "05-experiments-identification-and-validation", "40-poor-excitation");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-poor-excitation");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-poor-excitation"), lesson);
    assert.ok(catalogEntry, "Lesson 40 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/05-experiments-identification-and-validation/40-poor-excitation");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 40);
    assert.deepStrictEqual(lesson.prerequisites, ["control-dryer-model-selection"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_poor_excitation_rejects_unidentifiable_records");

    for (const requirement of [
      /three independent copies.*same plate heat exchanger/i,
      /identifiability.*distinguish.*parameters/i,
      /identifiable parameter.*different plausible values.*measurably different predictions/i,
      /unidentifiable parameter.*indistinguishable.*measurement noise.*bandwidth/i,
      /input span.*maximum.*minus.*minimum.*percentage points/i,
      /expected response amplitude \(A_response\).*measurement-noise amplitude \(A_noise\)/i,
      /response-to-noise ratio \(SNR\).*compares those two amplitudes/i,
      /SNR.*at least 5.*input span.*at least 5 percentage points.*63 seconds/i,
      /collinearity.*same pattern.*separate effects/i,
      /fast sampling.*(?:more rows|adds rows).*(?:does not add|not).*independent information/i,
      /exogenous excitation.*independently.*measurement noise/i,
      /endogenous input.*feedback/i,
      /measurement noise \(n\).*controller-used temperature \(PV_used\)/i,
      /valve deviation \(delta_MV\).*applied valve position minus.*20-percent baseline/i,
      /correlation coefficient \(rho\).*minus one.*plus one.*not causation/i,
      /ordinary open-loop estimator.*biased.*closed-loop identification/i,
      /reason 1.*insufficient.*reason 2.*independent excitation.*reason 3.*clipping.*reason 0/i,
      /0\.2 percentage points.*0\.1 degree Celsius.*0\.67/i,
      /10-percentage-point.*5 degrees Celsius.*33\.33/i,
      /same physical parameters.*distinct.*noise streams/i,
      /requested and applied.*controller-used measurement/i,
      /Do not.*Tune.*Apply/i,
      /rejected record.*no accepted model.*parameter estimate/i,
      /Data inadequate/i,
      /Lesson 41.*independent/i,
    ]) assert.match(readme, requirement);
    assert.match(source,
      /\\Delta MV = 2\\left\(50 - PV_\{\\mathrm\{used\}\}\\right\)/i);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(source.indexOf("Expected response amplitude") <
      source.indexOf("SNR = \\frac{A_{\\mathrm{response}}}{A_{\\mathrm{noise}}}"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.match(program, /ClosedRequested := 20\.0 \+ 2\.0 \* \(50\.0 - ClosedMeasurementUsed\)/);
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.strictEqual((simulation.match(/model = "first_order_dead_time"/g) ?? []).length, 3);
    for (const artifact of [
      "poor-excitation-comparison.svg", "poor-excitation-comparison-trace.csv",
      "poor-excitation-comparison-summary.csv",
    ]) {
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", artifact)),
        `Lesson 40 must retain ${artifact}`);
      if (artifact.endsWith(".svg")) assertSvgTitleFits(path.join(lessonRoot, "figures", artifact));
      assert.ok(source.includes(`figures/${artifact}`), `Lesson 40 README must link ${artifact}`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_600);
  });

  test("teaches independent heat-exchanger validation and bounded model scope", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "05-experiments-identification-and-validation", "41-heat-exchanger-validation-uncertainty");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-independent-model-validation");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-independent-model-validation"), lesson);
    assert.ok(catalogEntry, "Lesson 41 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/05-experiments-identification-and-validation/41-heat-exchanger-validation-uncertainty");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 41);
    assert.deepStrictEqual(lesson.prerequisites, ["control-poor-excitation"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_heat_exchanger_validation_classifies_scope_and_uncertainty");

    for (const requirement of [
      /three independent.*plate heat exchanger.*local.*cold-flow.*high-flow/i,
      /estimation data.*select.*parameters.*validation data.*different.*fitting/i,
      /frozen model.*structure.*parameters.*intervals.*preprocessing.*cannot change/i,
      /holdout experiment.*predeclared.*acceptance/i,
      /measured outlet temperature \(PV_measured\).*model-predicted outlet temperature \(PV_predicted\)/i,
      /sample number \(k\).*update.*residual.*measured minus predicted/i,
      /root-mean-square prediction error \(RMSE\).*mean squared residual/i,
      /residual bias.*lag-one residual autocorrelation/i,
      /parameter uncertainty interval.*supported.*assumptions/i,
      /confidence level.*long-run coverage.*not.*guarantee/i,
      /prediction uncertainty envelope.*parameter intervals.*measurement-noise allowance/i,
      /envelope coverage.*fraction.*samples.*inside/i,
      /interpolation.*represented.*extrapolation.*beyond/i,
      /validity region.*inputs.*outputs.*disturbances.*operating points/i,
      /accepted.*stated purpose.*restricted.*subset.*rejected.*fails/i,
      /5-percentage-point.*2\.5 degrees Celsius.*1\.58 degrees Celsius/i,
      /cold flow.*minus-2-degree-Celsius residual bias/i,
      /high-flow.*1\.25 degrees Celsius.*minus 1\.25 degrees Celsius/i,
      /requested and applied.*0.*100 percent.*restore.*20 percent/i,
      /no PID.*fitted model.*prediction.*verdict/i,
      /common 10-second baseline.*cold-flow load.*90 seconds.*160 seconds/i,
      /Do not.*Tune.*Apply/i,
      /RMSE.*bias.*lag-one.*envelope coverage/i,
      /20 to 80 seconds.*130 to 160 seconds/i,
      /accepted.*restricted.*rejected/i,
      /Lesson 42.*local command model.*limitations/i,
    ]) assert.match(readme, requirement);
    assert.match(source,
      /r_k = PV_\{\\mathrm\{measured\},k\} - PV_\{\\mathrm\{predicted\},k\}/);
    for (const artifact of [
      "heat-exchanger-validation-response.svg", "heat-exchanger-validation-response-trace.csv",
      "heat-exchanger-validation-response-summary.csv", "heat-exchanger-validation-residual.svg",
      "heat-exchanger-validation-residual-trace.csv", "heat-exchanger-validation-residual-summary.csv",
    ]) {
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", artifact)),
        `Lesson 41 must retain ${artifact}`);
      if (artifact.endsWith(".svg")) assertSvgTitleFits(path.join(lessonRoot, "figures", artifact));
      assert.ok(source.includes(`figures/${artifact}`), `Lesson 41 README must link ${artifact}`);
    }
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(source.indexOf("measured outlet temperature") <
      source.indexOf("r_k = PV_{\\mathrm{measured},k} - PV_{\\mathrm{predicted},k}"));
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    for (const forbidden of ["PV_predicted", "Residual", "RMSE", "Envelope", "Verdict"]) {
      assert.ok(!program.includes(forbidden), `ST must not embed validation answer '${forbidden}'`);
    }
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.match(simulation, /at_ms = 90000[\s\S]*parameter = "output_bias"[\s\S]*value = 38\.0/);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("retains reproducible Module 5 experiment and identification evidence", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const evidence = (lesson: string, file: string): any => JSON.parse(fs.readFileSync(path.join(
      repositoryRoot, "examples", "control", "05-experiments-identification-and-validation",
      lesson, "figures", file,
    ), "utf8"));

    const safe = evidence("37-safe-heat-exchanger-experiment", "safe-experiment-evidence.json");
    assert.deepStrictEqual(safe.runs.map((run: any) => run.plan), [
      "Plan B rejected", "Plan A accepted", "Plan A bad quality",
    ]);
    assert.ok(safe.runs[0].samples.every((sample: any) => sample.appliedValve === 60));
    assert.ok(safe.runs[1].samples.some((sample: any) => sample.experimentState === "baseline"));
    assert.ok(safe.runs[1].samples.some((sample: any) => sample.experimentState === "restoring"));
    assert.ok(safe.runs[2].samples.some((sample: any) => sample.experimentState === "aborted"));
    for (const run of safe.runs) for (const field of [
      "seed", "sampleInterval", "requestedValve", "appliedValve", "outletTemperature",
      "quality", "disturbanceState", "experimentState", "reasonCode", "evidenceClass",
    ]) assert.ok(field in run.samples[0], `Lesson 37 sample must retain ${field}`);

    const excitation = evidence("38-excitation-methods", "excitation-methods-evidence.json");
    assert.deepStrictEqual(excitation.runs.map((run: any) => `${run.plant}:${run.method}`), [
      "water-heater:step", "water-heater:pulse", "water-heater:PRBS",
      "water-heater:chirp", "flexible-drive:doublet", "flexible-drive:PRBS",
      "flexible-drive:chirp", "water-heater:relay-rejected",
    ]);
    for (const run of excitation.runs) for (const field of [
      "requestedInput", "appliedInput", "output", "frequency", "noise", "limits",
      "qualification", "reasonCode", "seed", "evidenceClass",
    ]) assert.ok(field in run.samples[0], `Lesson 38 sample must retain ${field}`);
    const heaterSvg = fs.readFileSync(path.join(repositoryRoot, "examples", "control", "05-experiments-identification-and-validation",
      "38-excitation-methods", "figures", "excitation-heater-comparison.svg"), "utf8");
    assert.match(heaterSvg, /pulse/);
    assert.doesNotMatch(heaterSvg, /Measured heater temperature \(degC\)[\s\S]*>22\.00<\/text>/,
      "heater evidence axis must include the retained temperature range rather than clip it");

    const dryer = evidence("39-dryer-model-selection", "dryer-model-selection-evidence.json");
    assert.strictEqual(dryer.estimationRun.id, "dryer-estimation-step-seed-20260827");
    assert.strictEqual(dryer.validationRun.id, "dryer-validation-pulse-seed-20260827");
    assert.ok(dryer.estimationRun.samples.length > 0 && dryer.validationRun.samples.length > 0);
    assert.strictEqual(dryer.candidates.length, 4);
    for (const candidate of dryer.candidates) for (const field of [
      "structure", "parameters", "parameterUncertainty", "estimationRmse", "validationRmse",
      "bias", "autocorrelation", "rejectionReason", "selection",
    ]) assert.ok(field in candidate, `Lesson 39 candidate must retain ${field}`);

    const poor = evidence("40-poor-excitation", "poor-excitation-evidence.json");
    assert.strictEqual(poor.records.length, 3);
    for (const record of poor.records) {
      assert.ok(record.samples.length > 0);
      for (const field of [
        "requestedInput", "appliedInput", "measuredOutput", "controllerUsedMeasurement",
        "sampleInterval", "duration", "noiseAmplitude", "qualityChecks", "seed", "evidenceClass",
      ]) assert.ok(field in record.samples[0], `Lesson 40 sample must retain ${field}`);
      for (const field of [
        "inputSpan", "outputSpan", "snr", "exogenousExcitation", "correlation",
        "qualificationReason",
      ]) assert.ok(field in record, `Lesson 40 record must retain ${field}`);
    }

    const validation = evidence("41-heat-exchanger-validation-uncertainty",
      "heat-exchanger-validation-evidence.json");
    assert.strictEqual(validation.estimationRunId, "qualified_open_loop");
    assert.deepStrictEqual(validation.estimationSource, {
      lesson: 40,
      artifact: "poor-excitation-evidence.json",
      recordId: "qualified_open_loop",
    });
    for (const field of [
      "structure", "parameters", "parameterIntervals", "confidenceLevel", "initialStateRule",
    ])
      assert.ok(field in validation.frozenModel, `Lesson 41 frozen model must retain ${field}`);
    assert.strictEqual(validation.validationRuns.length, 3);
    for (const run of validation.validationRuns) {
      assert.ok(run.samples.length > 0);
      for (const field of [
        "requestedInput", "appliedInput", "measuredOutput", "prediction", "envelopeLower",
        "envelopeUpper", "residual", "seed", "evidenceClass",
      ]) assert.ok(field in run.samples[0], `Lesson 41 sample must retain ${field}`);
      for (const field of [
        "rmse", "bias", "lagOneCorrelation", "coverage", "verdict", "purpose",
        "validityRegion", "failedCriteria",
      ]) assert.ok(field in run, `Lesson 41 verdict must retain ${field}`);
    }
  });

  test("teaches the crane Routh stability interval before time-domain confirmation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "07-classical-stability-and-frequency-design", "45-crane-routh-stability");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-crane-routh-stability");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-crane-routh-stability"), lesson);
    assert.ok(catalogEntry, "Lesson 45 must be reachable from the shipped example catalog");
    assert.strictEqual(catalogEntry.path, "control/07-classical-stability-and-frequency-design/45-crane-routh-stability");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 45);
    assert.deepStrictEqual(lesson.prerequisites, ["control-model-based-tuning"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Tune", "Validate"]);
    assert.strictEqual(lesson.plant.parameters.find(
      (parameter: { name: string }) => parameter.name === "stable_gain"
    )?.unit, "%/m");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_routh_range_predicts_crane_stable_marginal_and_unstable_runs");

    for (const requirement of [
      /one clear objective.*stability interval.*stable.*marginal.*unstable/i,
      /overhead.*crane.*trolley.*suspended load.*motor force/i,
      /characteristic equation.*roots.*closed-loop poles.*natural response/i,
      /complex frequency variable \(s\).*left half-plane.*imaginary axis.*right half-plane/i,
      /asymptotic stability.*marginal stability.*instability/i,
      /polynomial coefficient.*Routh array.*first column.*sign changes/i,
      /zero first-column.*epsilon.*complete zero row.*auxiliary polynomial/i,
      /p\(s\) = s\^3 \+ 2.*s\^2 \+ 3.*s \+ K/i,
      /s\^3.*\[1, 3\].*s\^2.*\[2, K\].*\(6 - K\) \/ 2/i,
      /0 < K < 6/i,
      /K = 4.*zero sign changes.*K = 6.*marginal.*K = 8.*two sign changes/i,
      /Control Studio.*Monitor.*Tune.*Test.*Validate/i,
      /same.*initial.*displacement.*zero setpoint.*natural response/i,
      /change only.*proportional gain.*model.*sample.*duration.*limits.*seed/i,
      /position-envelope ratio.*late window.*early window/i,
      /force travel.*sum.*absolute.*percentage points/i,
      /numerical root.*oracle.*time trace.*not.*replace/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 46.*root locus.*pole motion/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.match(program, /StableGain : LREAL := 4\.0/);
    assert.match(program, /MarginalGain : LREAL := 6\.0/);
    assert.match(program, /UnstableGain : LREAL := 8\.0/);
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0,
      "the explicit proportional paths must not hide another controller form");
    for (const forbidden of ["Routh", "RightHalfPlane", "StableLabel", "EnvelopeRatio"]) {
      assert.ok(!program.includes(forbidden), `ST must not embed analysis result '${forbidden}'`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("teaches why servo gain alone fails and designs lead compensation from root locus", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "07-classical-stability-and-frequency-design", "46-servo-root-locus");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) => candidate.id === "control-servo-root-locus");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-servo-root-locus"), lesson);
    assert.strictEqual(entry?.path, "control/07-classical-stability-and-frequency-design/46-servo-root-locus");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 46);
    assert.deepStrictEqual(lesson.prerequisites, ["control-crane-routh-stability"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Tune", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_root_locus_lead_design_meets_servo_damping_and_time_requirements");
    for (const requirement of [
      /one clear objective.*gain alone.*lead compensator/i,
      /positioning.*carriage.*millimetres.*motor-current.*percent/i,
      /5-percent overshoot.*2-percent settling.*2\.0 seconds.*zero final.*no current saturation/i,
      /open-loop pole.*open-loop zero.*root-locus branch.*positive scalar gain/i,
      /breakaway point.*asymptote.*centroid.*angle condition.*magnitude condition/i,
      /natural frequency.*damping ratio.*constant-damping line.*dominant poles.*non-dominant pole/i,
      /lead compensator.*zero.*farther-left pole.*lag compensator/i,
      /natural frequency.*damping ratio.*constant-damping line.*dominant poles/i,
      /breaks away at -1.*centroid.*-1.*90 degrees/i,
      /K > 1.*-1.*sqrt\(K-1\).*real part.*-1.*4 seconds/i,
      /K = 4.*1\.732.*2 rad\/s.*0\.5.*16\.3-percent/i,
      /50.*s\+2\.5.*s\+15.*s\^3 \+ 17.*80.*125/i,
      /-3\.235.*1\.186.*-10\.530.*0\.939/i,
      /complete.*transfer function.*zero.*3\.2 percent.*1\.36.*50[- ]percent/i,
      /Control Studio.*Monitor.*Tune.*Test.*Validate/i,
      /change only.*controller structure.*plant.*initial state.*sample.*duration.*limits.*seed/i,
      /percent overshoot.*peak position.*commanded position change/i,
      /2-percent settling time.*remains.*plus\/minus 2 percent/i,
      /applied-current travel.*sum.*absolute.*percentage points/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 47.*Bode.*resonance/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 2);
    assert.match(program, /ProportionalGain : LREAL := 4\.0/);
    assert.match(program, /LeadGain : LREAL := 50\.0/);
    assert.match(program, /LeadZero : LREAL := 2\.5/);
    assert.match(program, /LeadPole : LREAL := 15\.0/);
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("teaches measured Bode resonance and qualified flexible-drive margins", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control",
      "07-classical-stability-and-frequency-design", "47-flexible-drive-bode");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) => candidate.id === "control-flexible-drive-bode");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-flexible-drive-bode"), lesson);
    assert.strictEqual(entry?.path, "control/07-classical-stability-and-frequency-design/47-flexible-drive-bode");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 47);
    assert.deepStrictEqual(lesson.prerequisites, ["control-servo-root-locus"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify", "Tune", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_bode_measurement_exposes_flexible_drive_resonance_and_margins");
    for (const requirement of [
      /one clear objective.*frequency response.*resonance.*margins/i,
      /industrial motor.*load.*flexible shaft/i,
      /torque.*percent.*load-side shaft speed.*revolutions per minute/i,
      /sinusoidal steady state.*angular frequency.*radians per second/i,
      /magnitude.*output amplitude.*input amplitude.*phase.*timing/i,
      /logarithmic frequency.*decade.*decibel.*20.*log10/i,
      /Bode magnitude plot.*Bode phase plot.*resonance.*bandwidth/i,
      /gain crossover.*phase crossover.*gain margin.*phase margin.*delay margin/i,
      /coherence.*data-quality/i,
      /required crossing.*Unavailable.*Unbounded/i,
      /static gain.*natural frequency.*damping ratio/i,
      /static gain.*0\.25 rpm\/%.*natural frequency.*2 rad\/s.*damping ratio.*0\.05/i,
      /0\.5 rad\/s.*0\.267 rpm\/%.*-1\.5 degrees/i,
      /2 rad\/s.*2\.500 rpm\/%.*-90\.0 degrees/i,
      /8 rad\/s.*0\.0167 rpm\/%.*-178\.5 degrees/i,
      /unshaped.*gain.*5.*2\.99 rad\/s.*6\.9 degrees/i,
      /resonance-shaped.*1\.44 rad\/s.*112\.6 degrees/i,
      /pole-zero cancellation.*model-dependent.*perturb/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune.*Validate/i,
      /requested.*applied torque.*separately/i,
      /change only probe frequency.*hold plant.*operating point.*sample schedule.*limits.*seed equal/i,
      /fit.*sinusoidal fundamental.*steady-state window/i,
      /every.*crossing.*not.*favourable/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 48.*Nyquist/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.match(program, /LowFrequency : LREAL := 0\.5/);
    assert.match(program, /ResonantFrequency : LREAL := 2\.0/);
    assert.match(program, /HighFrequency : LREAL := 8\.0/);
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0,
      "open-loop frequency measurement must not hide a feedback controller");
    for (const forbidden of ["MagnitudeDb", "PhaseMargin", "AcceptedCandidate", "BodeResult"]) {
      assert.ok(!program.includes(forbidden), `ST must not embed analysis result '${forbidden}'`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("teaches complete Nyquist stability counting on a delayed conveyor oven", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control",
      "07-classical-stability-and-frequency-design", "48-delayed-thermal-nyquist");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) => candidate.id === "control-delayed-thermal-nyquist");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-delayed-thermal-nyquist"), lesson);
    assert.strictEqual(entry?.path, "control/07-classical-stability-and-frequency-design/48-delayed-thermal-nyquist");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 48);
    assert.deepStrictEqual(lesson.prerequisites, ["control-flexible-drive-bode"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify", "Tune", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_nyquist_count_predicts_delayed_oven_stability");
    for (const requirement of [
      /one clear objective.*Nyquist.*stable.*unstable/i,
      /continuous conveyor oven.*heater.*downstream infrared sensor/i,
      /heater command.*percent.*product temperature.*degrees Celsius/i,
      /complex number.*real component.*imaginary component/i,
      /complex plane.*real axis.*imaginary axis/i,
      /positive-frequency path.*negative-frequency.*conjugate.*closed contour/i,
      /critical point.*minus one.*encirclement.*clockwise.*counter-clockwise/i,
      /open-loop right-half-plane pole count.*closed-loop right-half-plane pole count/i,
      /counter-clockwise encirclement count \(N_ccw\).*open-loop.*\(P\).*closed-loop.*\(Z\)/i,
      /Z = P - N_ccw/i,
      /pure transport delay.*phase.*without changing magnitude/i,
      /pure transport delay.*phase.*without changing magnitude/i,
      /proportional gain.*two heater percentage points.*gain-crossover.*0\.08660/i,
      /5 s.*-84\.81 degrees.*95\.19 degrees/i,
      /zero encirclements.*zero unstable poles.*5 seconds of delay/i,
      /25 s.*-184\.05 degrees.*-4\.05 degrees.*count -2.*two.*unstable/i,
      /60 s.*-357\.72 degrees/i,
      /60 s phase wraps to \+2\.28 degrees.*false apparent margin.*182\.28 degrees/i,
      /requested.*applied.*heater command.*separately/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune.*Validate/i,
      /change only transport delay/i,
      /wrapped phase.*unwrapped phase.*every.*encirclement/i,
      /equilibrium-deviation envelope.*early window.*late window/i,
      /late- to-early envelope ratio.*1\.20.*abort/i,
      /longer-delay envelopes.*grow.*abort/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 49.*sensitivity.*complementary sensitivity/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.strictEqual((simulation.match(/model = "first_order_dead_time"/g) ?? []).length, 3);
    assert.match(program, /ProportionalGain : LREAL := 2\.0/);
    assert.match(program, /HoldingCommand : LREAL := 50\.0/);
    assert.match(program, /PLC.*bounded time-domain.*Nyquist.*retained evidence/is,
      "the ST must explain why frequency-domain verdicts are not embedded in the PLC");
    assert.match(program, /abort.*HoldingCommand.*reviewed equilibrium/is,
      "the ST must explain the safe holding-command restoration");
    assert.strictEqual((program.match(/Control\.PID/g) ?? []).length, 0);
    const executableProgram = program.replace(/\(\*[\s\S]*?\*\)/g, "");
    for (const forbidden of ["Nyquist", "Encirclement", "RightHalfPlane", "StableLabel", "PhaseMargin"]) {
      assert.ok(!executableProgram.includes(forbidden), `ST must not embed analysis result '${forbidden}'`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_700);
  });

  test("teaches sensitivity trade-offs on an executable precision motor", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control", "07-classical-stability-and-frequency-design", "49-motor-sensitivity");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) => candidate.id === "control-motor-sensitivity-loop-shaping");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-motor-sensitivity-loop-shaping"), lesson);
    assert.strictEqual(entry?.path, "control/07-classical-stability-and-frequency-design/49-motor-sensitivity");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 49);
    assert.deepStrictEqual(lesson.prerequisites, ["control-delayed-thermal-nyquist"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify", "Tune", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_loop_shaping_balances_motor_sensitivity_and_effort");
    for (const requirement of [
      new RegExp(lesson.objective.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "i"),
      /precision motor/i,
      /load[- ]rejection/i,
      /encoder noise/i,
      /control effort|torque activity/i,
      /compliant coupling.*rated-torque command.*percent.*load-side encoder.*revolutions per minute/i,
      /speed deviations?.*1000 rpm.*actual.*speed/i,
      /true-speed channel.*encoder noise.*physical shaft motion/i,
      /load torque.*second physical plant input/i,
      /open-loop response.*loop response \(L\)/i,
      /sensitivity function.*output disturbance.*model error.*sensitivity \(S\)/i,
      /complementary-sensitivity function.*setpoint.*measurement noise.*\(T\)/i,
      /S\(s\) = 1 \/ \(1 \+ L\(s\)\).*T\(s\) = L\(s\) \/ \(1 \+ L\(s\)\).*S\(s\) \+ T\(s\) = 1/i,
      /load-disturbance path.*G\(s\)S\(s\).*control-sensitivity.*C\(s\)S\(s\)/i,
      /closed-loop bandwidth.*maximum sensitivity.*M_s.*maximum complementary sensitivity.*M_t/i,
      /roll-off.*high frequency/i,
      /stable.*proper.*minimum-phase.*waterbed effect/i,
      /G\(s\) = 10 \/ \(s\^2 \+ 0\.6s \+ 1\).*rpm\/%/i,
      /C_base\(s\) = 0\.4.*%\/rpm/i,
      /C_shape\(s\).*0\.3.*3.*0\.8.*0\.08.*8/i,
      /0\.01 rad\/s.*0\.200.*0\.201/i,
      /M_s.*3\.197.*1\.724.*M_t.*3\.009.*1\.334/i,
      /2\.187.*1\.706 rad\/s.*19\.13.*44\.11 degrees/i,
      /bandwidth.*3\.43.*2\.83 rad\/s/i,
      /10 rad\/s.*0\.0420.*0\.0247.*0\.4168.*0\.2449/i,
      /requested.*applied torque.*separately/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune.*Validate/i,
      /change only the controller/i,
      /overshoot.*settling time.*load.*departure.*torque total variation/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 50.*z-plane/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(readme.indexOf("sensitivity function") < readme.indexOf("S(s) ="));
    assert.ok(readme.indexOf("complementary-sensitivity function") < readme.indexOf("T(s) ="));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 2);
    assert.strictEqual((simulation.match(/model = "state_space"/g) ?? []).length, 2);
    assert.strictEqual((simulation.match(/frequency_hz = 1\.5915494309/g) ?? []).length, 2);
    assert.match(program, /ProportionalGain : LREAL := 0\.4/);
    assert.match(program, /LeadZero : LREAL := 0\.3/);
    assert.match(program, /LeadPole : LREAL := 3\.0/);
    assert.match(program, /LagZero : LREAL := 0\.8/);
    assert.match(program, /LagPole : LREAL := 0\.08/);
    assert.match(program, /RollOffPole : LREAL := 8\.0/);
    for (const forbidden of ["MaximumSensitivity", "Bandwidth", "AcceptedCandidate", "SensitivityResult"]) {
      assert.ok(!program.includes(forbidden), `ST must not embed analysis result '${forbidden}'`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 1_900);
  });

  test("teaches sampled-axis stability in the z-plane before retuning", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(repositoryRoot, "examples", "control",
      "07-classical-stability-and-frequency-design", "50-position-z-plane");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) => candidate.id === "control-position-z-plane-sampling");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-position-z-plane-sampling"), lesson);
    assert.strictEqual(entry?.path, "control/07-classical-stability-and-frequency-design/50-position-z-plane");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 50);
    assert.deepStrictEqual(lesson.prerequisites, ["control-motor-sensitivity-loop-shaping"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test", "Identify", "Tune", "Validate"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_z_plane_predicts_sampled_axis_failure_and_recovery");
    for (const requirement of [
      new RegExp(lesson.objective.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "i"),
      /industrial positioning table.*motor-force command.*percent.*carriage position.*millimetres/i,
      /sample.*measured value.*held output.*constant/i,
      /discrete sequence.*sample index \(k\).*z-transform.*discrete complex variable \(z\)/i,
      /one-sample delay.*next index.*z\^-1/i,
      /discrete pole.*multiplier per sample.*unit circle.*inside.*outside/i,
      /continuous pole maps exactly.*sampled pole/i,
      /zero-order hold.*sampling zero.*continuous plant.*no finite zero/i,
      /Nyquist angular frequency.*pi \/ T_s.*alias/i,
      /discrete root locus.*closed-loop.*poles.*gain/i,
      /table position.*velocity.*local mechanics.*force-to-position response/i,
      /exact zero-order-hold.*A_d.*B_d/i,
      /sampling zero.*-0\.9934.*-0\.9355/i,
      /proportional gain.*position error.*derivative gain.*measured velocity/i,
      /continuous-design gains.*Kp = 4.*Kd = 0\.4/i,
      /10 ms.*0\.97018.*0\.05688.*0\.97184/i,
      /100 ms.*0\.82299.*0\.63790.*1\.04126.*unstable/i,
      /100 ms, corrected gains.*0\.74361.*0\.53756.*0\.37370.*0\.65469/i,
      /corrected case.*Kp = 1.*Kd = 0\.4/i,
      /314\.16 rad\/s.*31\.416 rad\/s.*40 rad\/s.*22\.832 rad\/s/i,
      /requested.*pending.*applied force.*separately/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune.*Validate/i,
      /same mechanics.*fine controller.*10 ms.*coarse baseline.*100 ms.*corrected controller/i,
      /overshoot.*settling.*final-window position error.*force total variation.*abort.*controlled stop/i,
      /simulation evidence.*not.*physical/i,
      /Lesson 51.*tuning methods/i,
    ]) assert.match(readme, requirement);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    assert.ok(source.indexOf("discrete complex variable") < source.indexOf("z=e^{sT_s}"));
    assert.ok(readme.indexOf("Nyquist angular frequency") < readme.indexOf("omega_N ="));
    assert.strictEqual((simulation.match(/^\[\[plants\]\]$/gm) ?? []).length, 3);
    assert.strictEqual((simulation.match(/model = "state_space"/g) ?? []).length, 3);
    assert.match(program, /FineSampleSeconds : LREAL := 0\.01/);
    assert.match(program, /CoarseSampleSeconds : LREAL := 0\.1/);
    assert.match(program, /ContinuousProportionalGain : LREAL := 4\.0/);
    assert.match(program, /CorrectedProportionalGain : LREAL := 1\.0/);
    for (const forbidden of ["PoleMagnitude", "SamplingZero", "StableLabel", "AcceptedCandidate", "AliasFrequency"]) {
      assert.ok(!program.includes(forbidden), `ST must not embed z-plane answer '${forbidden}'`);
    }
    assert.ok(pedagogicalCoreWordCount(source) <= 2_000);
  });

  test("teaches two-tank actuator and sensor feasibility before design", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "11-state-space-estimation-and-predictive-control", "64-two-tank-observability"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    for (const suffix of [".svg", "-trace.csv", "-summary.csv"])
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `two-tank-feasibility-evidence${suffix}`)));

    assert.strictEqual(lesson.schemaVersion, 4, "Lesson 64 must use the reviewed teaching contract");
    const upstreamInitial = fs.readFileSync(
      path.join(lessonRoot, "simulation-initial-upstream.toml"), "utf8"
    );
    const downstreamInitial = fs.readFileSync(
      path.join(lessonRoot, "simulation-initial-downstream.toml"), "utf8"
    );
    assert.deepStrictEqual(parseControlLesson(lesson, "control-two-tank-observability"), lesson);
    assert.strictEqual(lesson.sequence, 64);
    assert.deepStrictEqual(lesson.prerequisites, ["control-distillation-robustness"]);
    assert.strictEqual(lesson.nextLesson, "control-magnetic-bearing-state-feedback");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_two_tank_feasibility_proves_reachable_and_hidden_states");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /pilot water-treatment buffer.*two interconnected.*tanks/i,
      /state vector \(x\).*Tank 1.*Tank 2/i,
      /input vector \(u\).*pump-flow/i,
      /measured-output vector \(y\).*level-transmitter/i,
      /state matrix \(A\).*natural.*dynamics/i,
      /input matrix \(B\).*actuator.*state rates/i,
      /output matrix \(C\).*installed sensors/i,
      /matrix rank.*independent directions/i,
      /eigenvalue \(lambda\).*negative.*decays.*positive.*grows/i,
      /stabilisability.*uncontrollable.*stable.*detectability.*unobservable.*stable/i,
      /B_upstream = \[\[0\.20\], \[0\]\].*0\.0012.*rank.*two/i,
      /B_bypass = \[\[0\], \[0\.20\]\].*rank.*one/i,
      /C_downstream = \[\[0, 1\]\].*-0\.03.*rank.*two/i,
      /C_upstream = \[\[1, 0\]\].*rank.*one/i,
      /no feedback controller or observer/i,
      /Simulation-only truth channels.*not installed controller inputs/i,
      /Control Studio Monitor.*Simulation.*Manual.*actuator.*sensor/i,
      /Identify.*controllability.*observability.*conditioning/i,
      /upstream pump.*downstream bypass.*initial.*Tank 1.*initial.*Tank 2/i,
      /first one-second.*slope.*peak level.*restoration error.*measurement-to-truth/i,
      /physical-tank.*explicit approval/i,
      /Lesson 65.*feedback poles.*unmeasured motor state/i,
    ]) assert.match(readme, requirement);
    const equations = pedagogicalMarkdownText(source);
    for (const equation of [
      /state matrix.*input matrix.*output matrix/i,
      /immediate pump-driven state direction.*second column.*controllability matrix/i,
      /immediate sensor information.*Natural dynamics.*observability matrix.*stacks those rows/i,
    ]) assert.match(equations, equation);
    assert.ok(source.indexOf("state vector ($x$)") < source.indexOf("\\dot{\\mathbf{x}}"));
    assert.ok(source.indexOf("controllability matrix ($\\mathcal{C}$)") <
      source.indexOf("\\mathcal{C}=\\begin{bmatrix}"));
    assert.ok(source.indexOf("observability matrix ($\\mathcal{O}$)") <
      source.indexOf("\\mathcal{O}=\\begin{bmatrix}"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 0);
    for (const required of [
      "ActuatorLayout", "SensorLayout", "UpstreamPumpApplied", "BypassPumpApplied",
      "Tank1Truth", "Tank2Truth", "SelectedMeasurement",
    ]) assert.ok(program.includes(required), `ST must expose ${required}`);
    assert.match(program, /layout 0.*upstream pump.*layout 1.*bypass.*one actuator.*flow request/is,
      "the ST must explain actuator-layout authority");
    assert.match(program, /layout 0.*Tank 2.*layout 1.*Tank 1.*selected\s+measurement.*controller-facing/is,
      "the ST must explain sensor-layout authority");
    for (const forbidden of ["ControllabilityRank", "ObservabilityRank", "Eigenvalue", "AcceptedLayout"])
      assert.ok(!program.includes(forbidden), `ST must not embed matrix verdict '${forbidden}'`);
    assert.match(simulation, /command_sources = \["%QL0", "%QL8"\]/);
    assert.match(simulation, /feedback_targets = \["%IL0", "%IL8"\]/);
    assert.match(upstreamInitial, /initial_state = \[5\.0, 0\.0\]/);
    assert.match(downstreamInitial, /initial_state = \[0\.0, 5\.0\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_500);
  });

  test("teaches state feedback before model-qualified state observation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "11-state-space-estimation-and-predictive-control", "65-magnetic-bearing-state-feedback"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    for (const suffix of [".svg", "-trace.csv", "-summary.csv"])
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `magnetic-bearing-state-feedback-evidence${suffix}`)));

    assert.strictEqual(lesson.schemaVersion, 4, "Lesson 65 must use the reviewed teaching contract");
    const mismatch = fs.readFileSync(
      path.join(lessonRoot, "simulation-motor-damping-mismatch.toml"), "utf8"
    );
    assert.deepStrictEqual(parseControlLesson(lesson, "control-magnetic-bearing-state-feedback"), lesson);
    assert.strictEqual(lesson.sequence, 65);
    assert.deepStrictEqual(lesson.prerequisites, ["control-two-tank-observability"]);
    assert.strictEqual(lesson.nextLesson, "control-weigh-feeder-kalman");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_state_feedback_and_observer_match_executed_modes");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /active magnetic-bearing.*conveyor motor/i,
      /state vector \(x_b\).*displacement.*velocity/i,
      /bearing input \(u_b\).*applied coil-current.*amperes/i,
      /feedback-gain row \(K_b\).*displacement.*velocity weights/i,
      /reference gain \(N_b\).*constant requested displacement/i,
      /position-only.*poles.*\+j.*-j.*does not settle/i,
      /desired poles.*-3.*-4.*16.*7.*12/i,
      /motor state vector \(x_m\).*shaft speed.*load torque/i,
      /measured motor output \(y_m\).*encoder speed/i,
      /estimated state \(x_hat\).*speed.*load prediction/i,
      /innovation \(nu\).*measured encoder speed minus estimated speed/i,
      /observer gain.*innovation.*state predictions/i,
      /observer-error poles.*-4.*-5.*8.*-20/i,
      /separation principle.*exact linear model.*limits.*robustness/i,
      /0\.8333.*1\.0000.*radians per second/i,
      /requested.*applied.*current.*requested.*applied.*torque/i,
      /Control Studio.*Monitor.*Identify.*Tune\/Design.*Test/i,
      /position-only.*Not settled.*state feedback.*observer.*damping-mismatch/i,
      /observer.*model states.*not.*truth|observer outputs?.*not.*sensor/i,
      /physical magnetic-bearing.*explicit approval/i,
      /Lesson 66.*Kalman/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /feedback-gain row.*reference gain.*bearing reference.*requested displacement/i,
      /estimated state.*observer.*Innovation.*measured encoder speed.*Applied motor torque/i,
      /Innovation.*measured encoder speed minus estimated speed/i,
      /motor speed-feedback gain.*motor reference gain.*estimated speed.*estimated load/i,
    ]) assert.match(equations, equation);
    assert.ok(source.indexOf("state vector ($\\mathbf{x}_b$)") <
      source.indexOf("u_{b,\\mathrm{requested}}"));
    assert.ok(source.indexOf("estimated state ($\\hat{\\mathbf{x}}$)") <
      source.indexOf("\\dot{\\hat{\\mathbf{x}}}"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 0);
    for (const required of [
      "BearingArchitecture", "BearingCurrentRequested", "BearingCurrentApplied",
      "MotorObserverEnabled", "MotorTorqueRequested", "MotorTorqueApplied",
      "EstimatedMotorSpeed", "EstimatedLoadTorque", "MotorInnovation",
      "MotorLoadTruth",
    ]) assert.ok(program.includes(required), `Lesson 65 ST must expose ${required}`);
    for (const forbidden of ["AcceptedDesign", "BearingPole", "ObserverPole", "SettlingVerdict"])
      assert.ok(!program.includes(forbidden), `ST must not embed analytical verdict '${forbidden}'`);
    assert.match(simulation, /id = "magnetic-bearing-training-rig"[\s\S]*id = "loaded-conveyor-motor"/);
    assert.match(simulation, /feedback_targets = \["%IL0", "%IL8"\]/);
    assert.match(simulation, /feedback_targets = \["%IL16", "%IL24"\]/);
    assert.match(mismatch, /a = \[\[-1\.2, -1\.0\], \[0\.0, 0\.0\]\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });
});
