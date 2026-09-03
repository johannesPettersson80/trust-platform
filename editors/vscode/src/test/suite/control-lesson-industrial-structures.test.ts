import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { CONTROL_COURSES, parseControlLesson } from "../../examples/model";
import { canonicalLessonReadmeHeadings, pedagogicalCoreWordCount, pedagogicalMarkdownText } from "./controlLessonAssertions";

function readStructuredTextProject(lessonRoot: string): string {
  const sourceRoot = path.join(lessonRoot, "src");
  return fs.readdirSync(sourceRoot)
    .filter((name) => name.endsWith(".st"))
    .sort()
    .map((name) => fs.readFileSync(path.join(sourceRoot, name), "utf8"))
    .join("\n");
}

function assertRetainedLessonEvidence(lessonRoot: string, source: string, stem: string): void {
  for (const suffix of [".svg", "-trace.csv", "-summary.csv"]) {
    assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `${stem}${suffix}`)),
      `retained lesson evidence is missing: ${stem}${suffix}`);
  }
  assert.match(source, new RegExp(`!\\[[^\\]]+\\]\\(figures/${stem}\\.svg\\)`));
  assert.match(source, new RegExp(`\\[Complete trace\\]\\(figures/${stem}-trace\\.csv\\)`));
  assert.match(source, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
}

suite("Industrial control-structure curriculum lessons", () => {
  test("exposes Lessons 53 through 60 in the Advanced course", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const projects = [
      ["09-industrial-control-structures", "53-conveyor-oven-feedforward"],
      ["09-industrial-control-structures", "54-reactor-cascade"],
      ["09-industrial-control-structures", "55-blending-ratio"],
      ["09-industrial-control-structures", "56-reactor-split-range"],
      ["09-industrial-control-structures", "57-compressor-override"],
      ["09-industrial-control-structures", "58-gravity-tank-scheduling"],
      ["09-industrial-control-structures", "59-heated-air-duct-smith"],
      ["09-industrial-control-structures", "60-valve-stiction"],
    ];
    for (const [index, [folder, project]] of projects.entries()) {
      const lessonRoot = path.join(repositoryRoot, "examples", "control", folder, project);
      const lesson = JSON.parse(fs.readFileSync(path.join(
        lessonRoot, "lesson.json"
      ), "utf8"));
      assert.strictEqual(lesson.level, "Advanced", `${project} must be reachable in Advanced`);
      assert.match(fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8"),
        new RegExp(`Course position: Lesson ${53 + index} of 84\\.`));
    }
    assert.strictEqual(CONTROL_COURSES.find((course) => course.id === "advanced")?.range,
      "Lessons 41–60");
  });

  test("teaches conveyor-oven feedforward from measured paths before implementation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "53-conveyor-oven-feedforward"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8"));
    const entry = catalog.find((candidate: { id: string }) =>
      candidate.id === "control-conveyor-oven-feedforward");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-conveyor-oven-feedforward"), lesson);
    assert.strictEqual(entry?.path, "control/09-industrial-control-structures/53-conveyor-oven-feedforward");
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 53);
    assert.deepStrictEqual(lesson.prerequisites, ["control-automatic-tuning-sequence"]);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_feedforward_compensates_measured_oven_speed_before_feedback_error");

    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /measured conveyor speed.*static.*dynamic feedforward.*feedback-only/i,
      /heater power.*0.*100 percent.*product-zone temperature.*degrees Celsius/i,
      /Lesson 52.*load-disturbance error.*wait.*process value/i,
      /setpoint \(SP\).*process value \(PV\).*temperature error \(e\)/i,
      /requested manipulated variable \(MV_requested\).*applied manipulated variable \(MV_applied\)/i,
      /measured disturbance \(d\).*disturbance deviation \(delta_d\)/i,
      /process path.*heater power.*disturbance path.*conveyor speed/i,
      /e = SP - PV/i,
      /MV_requested = MV_feedback \+ MV_feedforward/i,
      /PIDInput\.FeedForward.*rate limit.*amplitude limit.*anti-windup/i,
      /steady heater-path gain.*steady disturbance-path gain/i,
      /static feedforward.*disturbance deviation.*disturbance-path gain.*heater-path gain/i,
      /10\.0 percentage points/i,
      /heater-path time constant.*disturbance-path time constant/i,
      /filtered disturbance deviation \(z\)/i,
      /MV_feedforward_dynamic = 25 \* delta_d - 15 \* z/i,
      /25 percentage-point.*approaches.*10 percentage points/i,
      /stale.*forced.*nonfinite.*badly scaled.*reviewed range/i,
      /feedback-only.*70 percent.*static.*80 percent.*dynamic.*95 percent/i,
      /three real Control\.PID.*Control\.LowPass/i,
      /Control Studio.*Monitor.*30\.0 s.*90\.0 s/i,
      /Event-window integral absolute temperature error.*0\.1-second-weighted.*30\.0 through 60\.0.*degC/i,
      /cumulative applied-heater movement.*percentage points/i,
      /final-window mean absolute error.*80\.0.*90\.0 s.*settled/i,
      /Simulation evidence.*physical-plant/i,
      /Lesson 54.*cascade/i,
    ]) assert.match(readme, requirement);
    assert.ok(readme.indexOf("temperature error (e)") < readme.indexOf("e = SP - PV"));
    assert.ok(readme.indexOf("steady heater-path gain (K_u)") < readme.indexOf("MV_feedforward_static ="));
    assert.ok(readme.indexOf("filtered disturbance deviation (z)") < readme.indexOf("MV_feedforward_dynamic ="));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 3);
    assert.strictEqual((program.match(/Control\.LowPass;/g) ?? []).length, 1);
    assert.strictEqual((simulation.match(/^model = "state_space"$/gm) ?? []).length, 3);
    for (const forbidden of ["PreferredCase", "WinningFeedforward", "EventIAE", "AcceptDynamic"])
      assert.ok(!program.includes(forbidden), `ST must not embed lesson verdict '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_100);
    assertRetainedLessonEvidence(lessonRoot, source, "conveyor-oven-feedforward-evidence");
  });

  test("teaches inner-first reactor cascade commissioning before closing the outer loop", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "54-reactor-cascade"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-reactor-cascade"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 54);
    assert.deepStrictEqual(lesson.prerequisites, ["control-conveyor-oven-feedforward"]);
    assert.strictEqual(lesson.nextLesson, "control-blending-ratio");
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_cascade_commissions_inner_loop_before_rejecting_reactor_utility_disturbance");
    assertRetainedLessonEvidence(lessonRoot, source, "reactor-cascade-evidence");

    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /jacketed.*reactor.*inner loop.*outer loop/i,
      /reactor temperature.*primary controlled variable.*degrees Celsius/i,
      /jacket temperature.*secondary controlled variable.*degrees Celsius/i,
      /valve.*one-sided.*0.*100 percent/i,
      /Lesson 53.*feedforward.*cascade/i,
      /primary setpoint \(SP_primary\).*primary process value \(PV_primary\).*primary error \(e_primary\)/i,
      /secondary setpoint \(SP_secondary\).*secondary process value \(PV_secondary\).*secondary error \(e_secondary\)/i,
      /valve request \(MV_requested\).*applied valve position \(MV_applied\)/i,
      /e_primary = SP_primary - PV_primary/i,
      /outer controller.*secondary setpoint.*inner controller.*secondary error/i,
      /secondary error.*inner.*valve request/i,
      /outer loop owns only the secondary setpoint.*inner loop.*direct valve authority/i,
      /jacket-temperature deviation \(x_j\).*reactor-temperature deviation \(x_r\)/i,
      /2 \* dx_j\/dt = -x_j \+ 1\.4 \* u_v \+ w/i,
      /12 \* dx_r\/dt = -x_r \+ \(8\/7\) \* x_j/i,
      /inner loop.*at least three times faster/i,
      /Tune and validate the inner loop first.*Only then close the outer loop/i,
      /saturation propagation.*outer integral/i,
      /5-(?:degree-Celsius|degC).*jacket-setpoint step.*-10 (?:degrees Celsius|degC).*utility/i,
      /five.*Control\.PID/i,
      /Control Studio.*Monitor.*commissioning mode.*10\.0 s.*40\.0 s/i,
      /disturbance-detection lead.*reactor event(?:-window)? integral absolute error/i,
      /cumulative applied-valve movement.*percentage points/i,
      /Not settled.*final-window/i,
      /Simulation evidence.*physical reactor/i,
      /Lesson 55.*ratio control/i,
    ]) assert.match(readme, requirement);
    assert.ok(readme.indexOf("primary error (e_primary)") < readme.indexOf("e_primary = SP_primary - PV_primary"));
    assert.ok(readme.indexOf("jacket-temperature deviation (x_j)") < readme.indexOf("2 * dx_j/dt"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 5);
    assert.strictEqual((simulation.match(/^model = "state_space"$/gm) ?? []).length, 3);
    for (const forbidden of ["PreferredArchitecture", "CascadeWins", "EventIAE", "AcceptCascade"])
      assert.ok(!program.includes(forbidden), `ST must not embed lesson verdict '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_300);
  });

  test("teaches blending ratio action trim and low-flow authority before operation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "55-blending-ratio"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-blending-ratio"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 55);
    assert.deepStrictEqual(lesson.prerequisites, ["control-reactor-cascade"]);
    assert.strictEqual(lesson.nextLesson, "control-reactor-split-range");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_ratio_station_moves_with_wild_flow_and_fails_safe_at_low_flow");
    assertRetainedLessonEvidence(lessonRoot, source, "blending-ratio-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /wild stream.*controlled stream.*litres per minute/i,
      /Lesson 54.*cascade.*two material flows.*proportional/i,
      /measured wild-stream flow \(q_w\).*additive-flow request \(q_a\)/i,
      /ratio setpoint \(R_SP\).*measured blend ratio \(R_PV\).*ratio error \(e_R\)/i,
      /feedback trim \(q_trim\).*litres per minute/i,
      /q_a,requested = R_SP \* q_w \+ q_trim/i,
      /flow ratio.*additive fraction.*R_SP \/ \(1 \+ R_SP\)/i,
      /100 L\/min.*30 L\/min.*120 L\/min.*36 L\/min/i,
      /downstream analyser PI.*trim.*meter bias.*concentration/i,
      /low-flow singularity.*minimum wild flow.*10 L\/min/i,
      /bad.*nonfinite.*stale.*10\.\.160 L\/min.*authority/i,
      /blend-ratio deviation \(x_R\).*additive-flow deviation \(u_a\).*wild-flow deviation \(u_w\)/i,
      /2 \* dx_R\/dt = -x_R \+ 0\.01 \* u_a - 0\.003 \* u_w/i,
      /local model.*not.*near zero flow/i,
      /\+5 L\/min.*37\.5 L\/min.*1\.5 L\/min/i,
      /two real Control\.PID.*identical.*trim/i,
      /Control Studio.*Monitor.*30\.0 s.*90\.0 s/i,
      /event-window blend-ratio integral absolute error.*30.*60 s/i,
      /cumulative applied-additive-flow movement.*L\/min/i,
      /Not settled.*final-window/i,
      /Simulation evidence.*physical skid/i,
      /Lesson 56.*heating.*final elements/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("measured wild-stream flow") <
      source.indexOf("$q_{\\mathrm{a}},requested"));
    assert.ok(source.indexOf("blend-ratio deviation") <
      source.indexOf("$2 \\cdot dx_{R}/dt"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 2);
    assert.strictEqual((simulation.match(/^model = "state_space"$/gm) ?? []).length, 2);
    for (const forbidden of ["PreferredController", "RatioWins", "EventIAE", "AcceptRatio"])
      assert.ok(!program.includes(forbidden), `ST must not embed lesson verdict '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_300);
  });

  test("teaches reactor split-range mapping from installed actuator behavior", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "56-reactor-split-range"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-reactor-split-range"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 56);
    assert.deepStrictEqual(lesson.prerequisites, ["control-blending-ratio"]);
    assert.strictEqual(lesson.nextLesson, "control-compressor-override");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_split_range_exposes_gap_overlap_and_characterized_crossover");
    assertRetainedLessonEvidence(lessonRoot, source, "reactor-split-range-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /hot-water valve.*cooling[- ]+water valve.*opposing/i,
      /Lesson 55.*ratio.*heating.*cooling/i,
      /setpoint \(SP\).*process value \(PV\).*temperature error \(e\)/i,
      /signed controller demand \(D\).*heating-valve request \(H_requested\).*cooling-valve request \(C_requested\)/i,
      /applied.*H_applied.*C_applied/i,
      /e = SP - PV.*D = TemperaturePI\(e\)/i,
      /H_requested = max\(D, 0\).*C_requested = max\(-D, 0\)/i,
      /split point.*D = 0 percent/i,
      /dead zone.*H = max\(D - g, 0\).*C = max\(-D - g, 0\)/i,
      /overlap.*H = max\(D \+ o, 0\).*C = max\(o - D, 0\)/i,
      /installed heating gain \(K_H\).*installed cooling gain \(K_C\)/i,
      /0\.04 degrees Celsius.*0\.06 degrees Celsius/i,
      /C_requested = \(-D\) \* K_H \/ K_C.*2\/3/i,
      /reactor-temperature deviation \(x_T\).*heating input \(u_H\).*cooling input \(u_C\)/i,
      /dx_T\/dt = -0\.05 \* x_T \+ 0\.04 \* u_H - 0\.06 \* u_C/i,
      /D = \+30.*D = -20.*13\.33 percent cooling/i,
      /three real Control\.PID.*identical signed PI/i,
      /Control Studio.*Monitor.*20\.0 s.*80\.0 s.*140\.0 s.*200\.0 s/i,
      /simultaneous-utility exposure.*utility throughput.*dead-zone exposure/i,
      /crossover bump.*net thermal input/i,
      /Not settled.*Saturated.*Invalid.*Aborted/i,
      /Simulation evidence.*physical reactor/i,
      /Lesson 57.*constraint.*override/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("signed controller demand") < source.indexOf("$e = SP - PV$"));
    assert.ok(source.indexOf("reactor-temperature deviation") < source.indexOf("$dx_{T}/dt"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 3);
    assert.strictEqual((simulation.match(/^model = "state_space"$/gm) ?? []).length, 3);
    for (const forbidden of ["PreferredMapping", "CharacterizedWins", "EventIAE", "AcceptSplitRange"])
      assert.ok(!program.includes(forbidden), `ST must not embed lesson verdict '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_400);
  });

  test("teaches tracked compressor override authority before selector operation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "57-compressor-override"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-compressor-override"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 57);
    assert.deepStrictEqual(lesson.prerequisites, ["control-reactor-split-range"]);
    assert.strictEqual(lesson.nextLesson, "control-gravity-tank-scheduling");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_override_tracks_inactive_compressor_controllers_and_releases_once");
    assertRetainedLessonEvidence(lessonRoot, source, "compressor-override-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /centrifugal compressor.*recycle valve.*opening.*margin.*reduces.*pressure/i,
      /Lesson 56.*final elements.*two objectives.*same final element/i,
      /discharge-pressure setpoint \(SP_p\).*process value \(PV_p\).*error \(e_p\)/i,
      /minimum-flow-margin setpoint \(SP_m\).*process value \(PV_m\).*error \(e_m\)/i,
      /normal recycle request \(MV_normal\).*protective recycle request \(MV_protect\)/i,
      /selected request \(MV_selected\).*applied recycle position \(MV_applied\)/i,
      /MV_selected = max\(MV_normal, MV_protect\)/i,
      /MIN selector.*actuator.*sign.*safer/i,
      /authority.*pressure control.*margin.*priority/i,
      /external tracking.*inactive.*integral.*selected.*applied/i,
      /Track mode.*current incremental terms.*first post-selection request/i,
      /takeover threshold.*14 percent.*release threshold.*16 percent/i,
      /release hysteresis.*protected physical minimum.*12 percent/i,
      /pressure deviation \(x_p\).*margin deviation \(x_m\).*recycle deviation \(u_r\)/i,
      /dx_p\/dt = -0\.20 \* x_p - 0\.01 \* u_r/i,
      /dx_m\/dt = -0\.50 \* x_m \+ 0\.50 \* u_r/i,
      /30.*10 percent.*9 bar.*0 percent.*22 percent.*8\.4 bar/i,
      /five real Control\.PID/i,
      /Control Studio.*Monitor.*20\.0 s.*80\.0 s.*140\.0 s/i,
      /minimum margin.*time below.*takeover delay.*release delay/i,
      /exactly one takeover and one release.*1\.0 percentage point/i,
      /Simulation evidence.*anti-surge.*machinery protection/i,
      /Lesson 58.*operating point.*PI tuning/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("discharge-pressure setpoint") < source.indexOf("$e_{p} ="));
    assert.ok(source.indexOf("pressure deviation") < source.indexOf("$dx_{p}/dt"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 5);
    assert.strictEqual((simulation.match(/^model = "state_space"$/gm) ?? []).length, 3);
    for (const forbidden of ["PreferredCase", "TrackedWins", "MinimumMarginMetric", "MachineryProtected"])
      assert.ok(!program.includes(forbidden), `ST must not embed lesson verdict '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_500);
  });

  test("teaches gravity-tank scheduling from qualified local models and transitions", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "58-gravity-tank-scheduling"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-gravity-tank-scheduling"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 58);
    assert.deepStrictEqual(lesson.prerequisites, ["control-compressor-override"]);
    assert.strictEqual(lesson.nextLesson, "control-heated-air-duct-smith");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_gain_scheduling_crosses_gravity_tank_regions_without_chatter_or_bumps");
    assertRetainedLessonEvidence(lessonRoot, source, "gravity-tank-scheduling-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /vented.*water tank.*1\.0 square metre.*0\.060 cubic metres per second/i,
      /Lesson 57.*priority.*plant dynamics.*level/i,
      /liquid level \(h\).*inlet pump flow \(q_in\).*outlet flow \(q_out\)/i,
      /tank area \(A\).*outlet coefficient \(c\)/i,
      /outlet flow.*square root.*inlet flow minus outlet flow/i,
      /0\.02739.*0\.03708.*0\.04472.*cubic metres per second/i,
      /level deviation \(delta_h\).*pump-flow deviation \(delta_q\)/i,
      /local steady gain.*local time constant.*equilibrium level/i,
      /21\.91.*29\.66.*35\.78/i,
      /proportional gain \(Kp\).*integral gain \(Ki\)/i,
      /0\.00456.*0\.00337.*0\.00280/i,
      /scheduling variable \(rho\).*requested operating level/i,
      /interpolation fraction.*lower point.*upper point/i,
      /controller parameter.*lower point.*upper point.*scheduler filters/i,
      /atomically.*no scan.*mixed/i,
      /0\.425.*0\.675.*0\.010 m.*hysteresis/i,
      /inactive controller.*applied pump flow.*external tracking/i,
      /five real Control\.PID.*Control\.PIDGainScheduler/i,
      /Control Studio.*Monitor.*Identify.*Tune.*30 s.*120 s.*240 s.*330 s.*420 s/i,
      /regional integral absolute level error.*pump-flow total variation/i,
      /exactly four.*region transitions.*no.*chatter.*0\.0005/i,
      /Simulation evidence.*physical tank/i,
      /Lesson 59.*transport delay.*stability margin/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("liquid level ($h$)") < source.indexOf("q_{\\mathrm{out}} = c\\sqrt{h}"));
    assert.ok(readme.indexOf("scheduling variable (rho)") < readme.indexOf("alpha ="));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 5);
    assert.strictEqual((program.match(/Control\.PIDGainScheduler;/g) ?? []).length, 1);
    assert.strictEqual((simulation.match(/^model = "gravity_tank"$/gm) ?? []).length, 3);
    for (const forbidden of ["PreferredArchitecture", "ScheduledWins", "RegionalIAE", "AcceptSchedule"])
      assert.ok(!program.includes(forbidden), `ST must not embed lesson verdict '${forbidden}'`);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_600);
  });

  test("teaches Smith prediction only after qualifying ordinary PI and model uncertainty", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "59-heated-air-duct-smith"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-heated-air-duct-smith"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 59);
    assert.deepStrictEqual(lesson.prerequisites, ["control-gravity-tank-scheduling"]);
    assert.strictEqual(lesson.nextLesson, "control-valve-stiction");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_smith_predictor_improves_nominal_delay_response_and_exposes_mismatch");
    assertRetainedLessonEvidence(lessonRoot, source, "heated-air-duct-smith-evidence");
    assert.match(fs.readFileSync(path.join(
      lessonRoot, "figures", "heated-air-duct-smith-evidence.svg"
    ), "utf8"), /Smith prediction handles transport delay/);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp("^##(?:#)? " + heading + "$", "m"));
    for (const requirement of [
      /continuous.*food-drying conveyor.*recirculated.*heated-air duct.*downstream.*temperature sensor/i,
      /Lesson 58.*operating.*gain scheduling.*transport delay/i,
      /heater command.*product temperature.*operating heater command.*operating temperature/i,
      /process gain \(K\).*time constant \(T\).*transport delay \(L\)/i,
      /operating heater command.*operating temperature.*process gain.*time constant.*transport delay/i,
      /10 percent.*8 degC.*first 16 s.*63 percent.*12 s.*1\.333/i,
      /strongest.*ordinary PI.*overshoot.*phase margin.*delay margin.*next-more-aggressive/i,
      /delay-free model output \(y_m0\).*delayed model output \(y_mL\).*measured product temperature \(y_meas\)/i,
      /model correction \(c_m\).*predicted current temperature \(y_pred\)/i,
      /c_m = y_meas - y_mL.*y_pred = y_m0 \+ c_m/i,
      /5 degC.*correction.*invalid.*measured feedback/i,
      /one real Control\.PID.*one real Control\.SmithPredictorFOPDT/i,
      /Control Studio.*Monitor.*Identify.*Tune.*10 s.*35 degC.*70 s.*180 s/i,
      /integral absolute temperature error.*peak overshoot.*heater.*total variation.*predictor-active.*fallback/i,
      /at least 15 percent.*moderate mismatch.*severe mismatch/i,
      /Simulation evidence.*physical.*oven/i,
      /Lesson 60.*sticking.*final control element/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("heater command ($u$)") < source.indexOf("T\\frac{dy(t)}{dt}"));
    assert.ok(readme.indexOf("delay-free model output (y_m0)") < readme.indexOf("c_m ="));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 1);
    assert.strictEqual((program.match(/Control\.SmithPredictorFOPDT;/g) ?? []).length, 1);
    assert.strictEqual((simulation.match(/^model = "first_order_dead_time"$/gm) ?? []).length, 1);
    for (const forbidden of ["SmithWins", "PreferredController", "NominalIAE", "AcceptPredictor"])
      assert.ok(!program.includes(forbidden), "ST must not embed lesson verdict '" + forbidden + "'");
    assert.ok(pedagogicalCoreWordCount(source) <= 2_600);
  });

  test("teaches installed-valve characterization before diagnosing stiction", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "09-industrial-control-structures", "60-valve-stiction"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-valve-stiction"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 60);
    assert.deepStrictEqual(lesson.prerequisites, ["control-heated-air-duct-smith"]);
    assert.strictEqual(lesson.nextLesson, "control-distillation-mimo");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_valve_characterization_distinguishes_deadband_from_stiction");
    assertRetainedLessonEvidence(lessonRoot, source, "valve-stiction-evidence");
    assert.match(fs.readFileSync(path.join(
      lessonRoot, "figures", "valve-stiction-evidence.svg"
    ), "utf8"), /Valve tests reveal deadband and stiction/);
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp("^##(?:#)? " + heading + "$", "m"));
    for (const requirement of [
      /cooling-water flow bench/i,
      /regulated cooling-water header/i,
      /pneumatically actuated globe valve/i,
      /cooling-water flow setpoint \(SP\).*cubic metres per hour/i,
      /process value \(PV\).*flow measured.*same unit/i,
      /digital positioner.*physical stem position.*percent/i,
      /controller requests stem position.*percent/i,
      /Lesson 59.*process model.*final control element.*retuning/i,
      /inherent valve characteristic.*installed characteristic.*pressure drop/i,
      /valve stem position \(x\).*rangeability \(R\).*normalized flow capacity \(c\).*measured flow \(q\)/i,
      /c = \(R\^\(x\/100\) - 1\) \/ \(R - 1\)/i,
      /equal-percentage.*inherent reference.*not automatically.*installed flow curve/i,
      /installed target flow \(q_target\).*hydraulic response time \(T_q\)/i,
      /q_target = 100 \* \(x\/100\)\^2.*T_q \* dq\/dt = q_target - q/i,
      /requested manipulated variable \(MV_requested\).*applied manipulated variable \(MV_applied\)/i,
      /command deadband.*last accepted request.*backlash.*direction.*stiction.*static friction/i,
      /separate positioner signal.*localize lost motion/i,
      /Confirmed diagnosis.*signature to repeat.*corresponding flow movement/i,
      /0\.00.*4\.00.*16\.00.*36\.00.*64\.00.*100\.00 cubic metres per hour/i,
      /0\.20.*1\.80 cubic metres per hour per percentage point/i,
      /Restore every stem to 70 percent.*0\.5-percentage-point increments/i,
      /deadband stem.*two-percentage-point request change/i,
      /stiction stem.*five-percentage-point.*approximately 72 percent.*slip jump/i,
      /three real Control\.PID instances/i,
      /one parameter record.*80 percent bias.*identical/i,
      /Structured Text.*not.*fake position feedback.*diagnosis labels/i,
      /Control Studio.*Monitor.*Manual.*0.*20.*40.*60.*80.*100 percent/i,
      /installed-curve error.*request-to-position gap.*stationary-position time.*breakaway size/i,
      /Reversal lost motion.*request travel after a direction reversal/i,
      /Process-flow excursion.*maximum flow minus minimum flow/i,
      /Cumulative applied-position movement.*sum of absolute successive/i,
      /within 0\.05 cubic metres per hour.*At least three.*hold\/breakaway/i,
      /position feedback.*absent.*stale.*invalid.*unconfirmed/i,
      /Simulation evidence.*physical valve/i,
      /Lesson 61.*two manipulated variables.*two controlled compositions/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("valve stem position") < source.indexOf("$c ="));
    assert.ok(readme.indexOf("requested manipulated variable (MV_requested)") <
      readme.indexOf("request-to-position gap"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 3);
    assert.strictEqual((simulation.match(/^model = "first_order"$/gm) ?? []).length, 3);
    for (const forbidden of ["DiagnosedFault", "MaintenanceRequired", "StictionWins", "BreakawayCount"])
      assert.ok(!program.includes(forbidden), "ST must not embed lesson verdict '" + forbidden + "'");
    assert.ok(pedagogicalCoreWordCount(source) <= 2_500);
  });

  test("teaches measured distillation interaction before selecting an RGA pairing", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot,
      "examples",
      "control",
      "10-multivariable-control",
      "61-distillation-mimo",
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-distillation-mimo"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 61);
    assert.deepStrictEqual(lesson.prerequisites, ["control-valve-stiction"]);
    assert.strictEqual(lesson.nextLesson, "control-distillation-decoupling");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_distillation_steps_identify_gain_matrix_and_rga_pairing");
    assert.strictEqual(lesson.level, "Modern");
    assertRetainedLessonEvidence(lessonRoot, source, "distillation-rga-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp("^##(?:#)? " + heading + "$", "m"));
    for (const requirement of [
      /continuous binary distillation column.*qualified.*operating point/i,
      /overhead light-product purity deviation \(y1\).*composition percentage points/i,
      /bottoms heavy-product purity deviation \(y2\).*composition percentage points/i,
      /reflux-valve change \(u1\).*reboiler-steam-valve change \(u2\)/i,
      /Lesson 60.*final elements are healthy.*both product compositions/i,
      /steady-state gain.*settled output change.*independently applied input change.*written gij/i,
      /first index.*output row.*second.*input column/i,
      /G = \[\[g11, g12\], \[g21, g22\]\]/i,
      /Relative Gain Array.*open-loop.*closed-loop effective gains/i,
      /invert.*transpose.*element-by-element multiplication/i,
      /transpose that inverse.*exchanging.*rows.*columns.*element-by-element/i,
      /negative relative gain.*feedback direction reverses/i,
      /20 seconds.*35 seconds.*40 seconds.*30 seconds.*predicted settled gain matrix.*0\.8.*-0\.4.*-0\.3.*0\.7/i,
      /\+5.*reflux.*\+4\.00.*-1\.50 composition-percentage-point/i,
      /\+5.*reboiler.*-2\.00.*\+3\.50 composition-percentage-point/i,
      /determinant.*0\.44.*Relative Gain Array.*1\.2727.*-0\.2727/i,
      /diagonal pairing.*eligible.*cross pairing.*negative/i,
      /open-loop interaction-identification lesson.*shall not.*centralized controller/i,
      /Control Studio.*Monitor.*Simulation.*Manual.*0\.1-second/i,
      /change only reflux.*new run.*change only reboiler steam/i,
      /zero-input baseline for 20 seconds/i,
      /change only reflux.*hold it for 180 seconds/i,
      /change only reboiler steam.*for 180 seconds/i,
      /final 10 seconds.*0\.02 composition percentage points/i,
      /input change.*output change.*steady-state gain/i,
      /row and column.*sum.*one/i,
      /path is missing or invalid.*Unconfirmed/i,
      /does not remove interaction.*specify controller gains.*robust stability/i,
      /Lesson 62.*decentralized proportional-integral.*decoupling/i,
      /simulation only.*physical-column tests.*explicit approval/i,
    ]) assert.match(readme, requirement);
    assert.ok(readme.indexOf("steady-state gain from input") < readme.indexOf("gij"));
    assert.ok(readme.indexOf("Relative Gain Array") < readme.indexOf("Lambda ="));
    assert.strictEqual((simulation.match(/^model = "state_space"$/gm) ?? []).length, 1);
    assert.match(simulation, /a = \[\[-0\.05,0\.0,0\.0,0\.0\],\[0\.0,-0\.0285714285714286,0\.0,0\.0\],\[0\.0,0\.0,-0\.025,0\.0\],\[0\.0,0\.0,0\.0,-0\.0333333333333333\]\]/);
    assert.match(simulation, /b = \[\[0\.04,0\.0\],\[0\.0,-0\.0114285714285714\],\[-0\.0075,0\.0\],\[0\.0,0\.0233333333333333\]\]/);
    for (const forbidden of ["RelativeGain", "PairingVerdict", "Rga11", "Determinant"])
      assert.ok(!program.includes(forbidden), "ST must not embed analytical verdict '" + forbidden + "'");
    assert.ok(pedagogicalCoreWordCount(source) <= 2_500);
  });

  test("teaches evidence-qualified static and dynamic distillation decoupling", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot,
      "examples",
      "control",
      "10-multivariable-control",
      "62-distillation-decoupling",
    );
    assert.ok(
      fs.existsSync(lessonRoot) && fs.statSync(lessonRoot).isDirectory(),
      "Lesson 62 executable project must exist",
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const nominal = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const perturbed = fs.readFileSync(path.join(lessonRoot, "simulation-perturbed.toml"), "utf8");

    assert.deepStrictEqual(parseControlLesson(lesson, "control-distillation-decoupling"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 62);
    assert.deepStrictEqual(lesson.prerequisites, ["control-distillation-mimo"]);
    assert.strictEqual(lesson.nextLesson, "control-distillation-robustness");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_distillation_decoupling_compares_direct_static_and_dynamic_evidence");
    assert.strictEqual(lesson.level, "Modern");
    assertRetainedLessonEvidence(lessonRoot, source, "distillation-decoupling-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp("^##(?:#)? " + heading + "$", "m"));
    for (const requirement of [
      /same local binary distillation column.*Lesson 61/i,
      /overhead purity setpoint \(SP1\).*process value \(PV1\)/i,
      /bottoms purity setpoint \(SP2\).*process value \(PV2\)/i,
      /virtual loop corrections \(v1.*v2\).*requested.*reflux.*reboiler/i,
      /relative gains.*1\.2727.*unequal.*response times/i,
      /decentralized control.*two.*feedback controllers.*same coupled plant/i,
      /parallel.*positional.*Control\.PID.*0\.1-second/i,
      /normalized interaction matrix.*1.*-0\.5.*-0\.428571/i,
      /physical-input vector.*virtual-correction vector.*static mapping/i,
      /predicted product.*0\.8.*0\.7.*off-diagonal/i,
      /static cancellation.*final gains.*transient/i,
      /dynamic decoupler.*causal transfer function/i,
      /measured path ratios.*proper and causal.*first-order filtered part/i,
      /measured path ratios.*0\.428571.*30s.*40s.*proper and causal/i,
      /s is the Laplace frequency variable.*not seconds/i,
      /proper and causal.*direct part.*first-order filtered part/i,
      /prediction.*overhead-only.*static.*transient.*dynamic/i,
      /two real Control\.PID.*StaticDecoupler2x2.*two.*Control\.LowPass/i,
      /architecture selector.*direct.*static.*dynamic/i,
      /Control Studio.*Monitor.*Simulation.*Auto.*requested.*applied/i,
      /overhead-only.*0.*\+2.*restore.*bottoms-only/i,
      /feed-composition disturbance.*perturbed interaction model/i,
      /primary-loop integral absolute error.*cross-output peak excursion/i,
      /requested and applied input total variation.*saturation duration/i,
      /model mismatch.*dynamic cancellation.*degrade/i,
      /select dynamic decoupling only if.*materially improves/i,
      /simulation evidence.*Physical-column.*explicit approval/i,
      /Lesson 63.*single-loop.*robustness/i,
    ]) assert.match(readme, requirement);
    assert.ok(readme.indexOf("setpoint (SP1)") < readme.indexOf("SP1 - PV1"));
    assert.ok(readme.indexOf("dynamic decoupler") < readme.indexOf("D12(s)"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 2);
    assert.strictEqual((program.match(/Control\.LowPass;/g) ?? []).length, 2);
    assert.strictEqual((program.match(/Control\.StaticDecoupler2x2;/g) ?? []).length, 1);
    for (const forbidden of ["DirectWins", "StaticWins", "DynamicWins", "CrossPeak", "CombinedIAE"])
      assert.ok(!program.includes(forbidden), "ST must not embed analytical verdict '" + forbidden + "'");
    assert.match(nominal, /^model = "state_space"$/m);
    assert.match(perturbed, /^model = "state_space"$/m);
    assert.notStrictEqual(nominal, perturbed);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_500);
  });

  test("teaches singular-direction robustness and honest architecture selection", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot,
      "examples",
      "control",
      "10-multivariable-control",
      "63-distillation-robustness",
    );
    assert.ok(
      fs.existsSync(lessonRoot) && fs.statSync(lessonRoot).isDirectory(),
      "Lesson 63 executable project must exist",
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const nominal = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const interaction = fs.readFileSync(
      path.join(lessonRoot, "simulation-strong-interaction.toml"),
      "utf8",
    );

    assert.deepStrictEqual(parseControlLesson(lesson, "control-distillation-robustness"), lesson);
    assert.strictEqual(lesson.schemaVersion, 4);
    assert.strictEqual(lesson.sequence, 63);
    assert.deepStrictEqual(lesson.prerequisites, ["control-distillation-decoupling"]);
    assert.strictEqual(lesson.nextLesson, "control-two-tank-observability");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_distillation_robustness_selects_from_coupled_evidence");
    assert.strictEqual(lesson.level, "Modern");
    assertRetainedLessonEvidence(lessonRoot, source, "distillation-robustness-evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp("^##(?:#)? " + heading + "$", "m"));
    for (const requirement of [
      /same.*binary distillation column.*Lessons 61 and 62/i,
      /input vector \(u\).*reflux.*reboiler/i,
      /output vector \(y\).*overhead.*bottoms/i,
      /Euclidean length.*combined magnitude/i,
      /largest singular value \(sigma_max\).*largest output-vector magnitude/i,
      /smallest singular value \(sigma_min\).*smallest output-vector magnitude/i,
      /normalized nominal gain matrix.*different diagonal.*directions/i,
      /1\.4649.*0\.5364.*2\.73/i,
      /stronger-interaction.*0\.8.*0\.7/i,
      /1\.7512.*0\.2512.*6\.97/i,
      /inverse-command amplification.*1\.86.*3\.98/i,
      /interaction uncertainty \(DeltaM\).*0\.30.*0\.5364/i,
      /sensitivity matrix \(S\).*S\(s\) = inverse\(I \+ L\(s\)\)/i,
      /frequency and direction.*diagonal elements alone/i,
      /direct decentralized PI.*static decoupling.*centralized constraint coordinator/i,
      /common scale factor \(alpha\).*preserved/i,
      /not model predictive control.*no prediction horizon.*optimizer/i,
      /Control Studio.*Monitor.*Simulation.*requested and applied/i,
      /Identify.*uncertainty set.*singular values/i,
      /overhead-only.*bottoms-only.*simultaneous.*feed-composition/i,
      /\+2\/\+2.*Infeasible/i,
      /final-window combined absolute error.*unintended cross peak/i,
      /multivariable sensitivity peak.*0\.002.*2.*radians per second/i,
      /simplest architecture.*20 percent/i,
      /simulation evidence.*Physical-column.*explicit approval/i,
      /Lessons 64.*68.*state space.*predictive control/i,
    ]) assert.match(readme, requirement);
    assert.ok(source.indexOf("input vector ($u$)") < source.indexOf("\\left\\lVert u\\right\\rVert_2"));
    assert.ok(readme.indexOf("smallest singular value (sigma_min)") < readme.indexOf("1 / sigma_min"));
    assert.strictEqual((program.match(/Control\.PID;/g) ?? []).length, 2);
    assert.strictEqual((program.match(/Control\.StaticDecoupler2x2;/g) ?? []).length, 1);
    assert.match(program, /CommonScale\s*:=\s*MIN\(/);
    assert.match(program, /RefluxApplied\s*:=\s*CommonScale\s*\*\s*RefluxRequested/);
    assert.match(program, /ReboilerApplied\s*:=\s*CommonScale\s*\*\s*ReboilerRequested/);
    for (const forbidden of ["SelectedArchitecture", "SingularValue", "SensitivityPeak", "RobustWinner"])
      assert.ok(!program.includes(forbidden), "ST must not embed analysis verdict '" + forbidden + "'");
    assert.match(nominal, /^model = "state_space"$/m);
    assert.match(interaction, /^model = "state_space"$/m);
    assert.notStrictEqual(nominal, interaction);
    assert.ok(pedagogicalCoreWordCount(source) <= 2_500);
  });

  test("diagnoses blinded steam-loop evidence before selecting a remedy", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "13-networked-fault-aware-and-diagnostic-control", "75-loop-diagnosis"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const scenarioRoot = path.join(lessonRoot, "scenarios");
    assertRetainedLessonEvidence(lessonRoot, source, "loop-diagnosis-evidence");

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 75 must replace the labelled prose example with blinded executable evidence");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-loop-diagnosis"), lesson);
    assert.strictEqual(lesson.sequence, 75);
    assert.deepStrictEqual(lesson.prerequisites, ["control-pumping-fault-aware"]);
    assert.strictEqual(lesson.nextLesson, "control-capstone-cold-storage");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_loop_diagnosis_classifies_blinded_evidence");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /industrial steam-distribution area.*steam-flow setpoint.*t\/h.*valve-stem position.*percent/i,
      /upstream header pressure.*bar.*reference flow meter.*adjacent.*valve demand.*operating region/i,
      /performance baseline.*same operating region.*controller revision.*sample interval/i,
      /event detection.*synchronized.*cause.*controller reaction.*actuator motion.*process response/i,
      /process error \(e\).*setpoint \(SP\).*process value \(PV\).*requested manipulated variable \(MV_requested\).*applied manipulated variable \(MV_applied\)/i,
      /oscillation period \(P_osc\).*cross-correlation lag.*which signal.*leads/i,
      /poor tuning.*requested valve.*first.*stem follows.*no limit.*external.*does not lead/i,
      /valve stiction.*holds measured stem.*demand.*move.*breakaway jump/i,
      /held-demand travel \(TV_hold\).*stem jump \(Delta_stem\)/i,
      /flow disagreement \(d_flow\).*primary.*reference.*drift slope/i,
      /limit occupancy.*100 percent.*cannot create.*capacity/i,
      /header -> flow -> valve response.*precedence/i,
      /other MV -> header\/flow -> local MV.*interaction/i,
      /model residual \(epsilon_model\).*operating-region transition.*schedule/i,
      /diagnostic hypothesis.*discriminating test.*Inconclusive/i,
      /healthy baseline.*poor controller tuning.*valve stiction.*transmitter drift.*saturation.*upstream disturbance.*interaction.*model\/schedule mismatch/i,
      /Control Studio.*Monitor.*Test/i,
      /correct diagnosis count.*incorrect diagnosis count.*justified inconclusive count.*remedy-selection accuracy/i,
      /Simulation.*not.*physical-plant evidence/i,
      /Lesson 76.*capstone/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /integrated absolute error.*sum.*absolute.*setpoint.*process value/i,
      /flow disagreement.*primary flow minus.*reference flow/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("Integrated absolute error") < readme.indexOf("IAE ="));
    assert.ok(source.indexOf("Flow disagreement") <
      source.indexOf("d_{\\mathrm{flow},k} = PV_{k} - PV_{\\mathrm{reference},k}"));
    for (const required of [
      "Setpoint", "ProcessValue", "ReferenceFlow", "UpstreamPressure",
      "AdjacentValveDemand", "ValveRequested", "ValveStemPosition",
      "OperatingRegion", "ScheduleTransition", "FlowQualityGood",
      "ReferenceQualityGood", "ControllerMode", "MeasuredCycleTime", "Controller",
    ]) assert.ok(program.includes(required), `Lesson 75 ST must expose ${required}`);
    for (const forbidden of [
      "Diagnosis", "PoorTuning", "StictionCase", "SensorDriftCase",
      "SaturationCase", "InteractionCase", "RecommendedRemedy",
    ]) assert.ok(!program.includes(forbidden), `controller must not embed '${forbidden}'`);
    assert.ok(fs.existsSync(path.join(scenarioRoot, "answer-key.json")));
    const scenarioFiles = fs.readdirSync(scenarioRoot)
      .filter((name) => /^case-[0-7]\.toml$/.test(name));
    assert.strictEqual(scenarioFiles.length, 8,
      "one healthy baseline plus seven degraded blinded executable cases are required");
    assert.ok(pedagogicalCoreWordCount(source) <= 3_800);
  });
});
