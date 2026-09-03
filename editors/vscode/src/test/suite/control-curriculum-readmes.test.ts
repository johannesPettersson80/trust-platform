import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { pedagogicalMarkdownText } from "./controlLessonAssertions";

function repositoryRoot(): string {
  return process.env.TRUST_REPOSITORY_ROOT
    ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
    : path.resolve(__dirname, "..", "..", "..", "..", "..");
}

function lessonRoot(relative: string): string {
  return path.join(repositoryRoot(), "examples", "control", relative);
}

function bundledLessonRoot(relative: string): string {
  return path.join(repositoryRoot(), "editors", "vscode", "media", "examples", "control", relative);
}

function pedagogicalText(markdown: string): string {
  return pedagogicalMarkdownText(markdown);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Blank out fenced code blocks and strip inline code spans, keeping line numbers intact. */
function stripFencedAndInlineCode(markdown: string): string[] {
  let inFence = false;
  return markdown.split("\n").map((line) => {
    if (/^```/.test(line)) {
      inFence = !inFence;
      return "";
    }
    return inFence ? "" : line.replace(/`[^`]*`/g, "");
  });
}

const previewMathSpec = "Specification 38 §2.1.4 (docs/specs/38-control-engineering-curriculum.md)";

/** Find every VS Code preview-unsafe mathematics usage in one lesson README. */
function findPreviewUnsafeMathViolations(readmePath: string, controlRoot: string): string[] {
  const relative = path.relative(controlRoot, readmePath);
  const violations: string[] = [];
  stripFencedAndInlineCode(fs.readFileSync(readmePath, "utf8")).forEach((line, index) => {
    const lineNumber = index + 1;
    const report = (rule: string): void => {
      violations.push(`${relative}:${lineNumber}: ${rule} — ${previewMathSpec}`);
    };
    if (line.includes("\\(")) {
      report("uses a \\( delimiter; the VS Code preview renders only $ mathematics");
    }
    if (line.includes("\\[")) {
      report("uses a \\[ delimiter; the VS Code preview renders only $ mathematics");
    }
    if (line.includes("$$")) {
      report("uses $$; a standalone relationship must be its own left-aligned paragraph holding one $...$ expression");
    }
    for (const match of line.matchAll(/\$([^$]*)\$/g)) {
      if (match[0].includes("\\ ")) {
        report(`${match[0]} contains \\ (backslash-space); textual labels must use \\text{} or \\operatorname{}`);
      }
    }
  });
  return violations;
}

const canonicalLessonSections = [
  "The problem",
  "What we need",
  "The new idea",
  "Prediction",
  "Implementation",
  "Experiment in Control Studio",
  "Evidence and decision",
  "Conclusion",
  "Reason for the next lesson",
];

suite("Control curriculum pedagogical READMEs", () => {
  test("all lesson READMEs use the canonical problem-driven teaching sequence", () => {
    const controlRoot = path.join(repositoryRoot(), "examples", "control");
    const readmes: string[] = [];
    const visit = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const candidate = path.join(directory, entry.name);
        if (entry.isDirectory()) {
          visit(candidate);
        } else if (
          entry.name === "README.md"
          && directory !== controlRoot
          && fs.existsSync(path.join(directory, "lesson.json"))
        ) {
          readmes.push(candidate);
        }
      }
    };
    visit(controlRoot);
    assert.strictEqual(readmes.length, 84, "the canonical sequence must cover all 84 lessons");
    for (const readmePath of readmes.sort()) {
      const readme = fs.readFileSync(readmePath, "utf8");
      assert.deepStrictEqual(
        [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
        canonicalLessonSections,
        `${path.relative(controlRoot, readmePath)} must use the canonical nine-section sequence`
      );
      assert.doesNotMatch(
        readme,
        /\*\*(?:Controller under test|Comparison)\*\*/,
        `${path.relative(controlRoot, readmePath)} must not link learners to retired section names`
      );
      assert.doesNotMatch(
        readme,
        /^```mermaid$/m,
        `${path.relative(controlRoot, readmePath)} must use checked-in SVG diagrams that render in the shipped preview`
      );
      assert.doesNotMatch(
        readme,
        /^\$\$$/m,
        `${path.relative(controlRoot, readmePath)} must keep standalone equations left aligned with their introducing prose`
      );
      assert.doesNotMatch(
        readme,
        /^```(?:iecst|structured-text)$/m,
        `${path.relative(controlRoot, readmePath)} must not use a Markdown language name that renders Structured Text without shipped highlighting`
      );
      const implementation = readme.match(
        /^## Implementation\s+([\s\S]*?)^## Experiment in Control Studio$/m
      )?.[1] ?? "";
      assert.match(
        implementation,
        /^```st$/m,
        `${path.relative(controlRoot, readmePath)} must show actual Structured Text with the shipped st syntax-highlighting alias`
      );
      const experiment = readme.match(
        /^## Experiment in Control Studio\s+([\s\S]*?)^## Evidence and decision$/m
      )?.[1] ?? "";
      assert.deepStrictEqual(
        [...experiment.matchAll(/^### (.+)$/gm)].map((match) => match[1]),
        ["Control Studio actions"],
        `${path.relative(controlRoot, readmePath)} must use the single Lesson 8 Control Studio procedure`
      );
      const lesson = JSON.parse(fs.readFileSync(path.join(path.dirname(readmePath), "lesson.json"), "utf8"));
      const problem = readme.match(/^## The problem\s+([\s\S]*?)^## What we need$/m)?.[1] ?? "";
      assert.ok(
        problem.toLocaleLowerCase().includes(String(lesson.equipment).toLocaleLowerCase()),
        `${path.relative(controlRoot, readmePath)} must name its exact equipment in the problem before teaching a technique`
      );
      const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
        .replace(/\s+/g, " ").trim().toLocaleLowerCase() ?? "";
      const need = readme.match(/^## What we need\s+([\s\S]*?)^## The new idea$/m)?.[1]
        .replace(/\s+/g, " ").trim().toLocaleLowerCase() ?? "";
      assert.match(
        need,
        /^we need\b/,
        `${path.relative(controlRoot, readmePath)} must state the missing capability in learner-facing words rather than restating an objective`
      );
      assert.doesNotMatch(
        need,
        /\*\*equipment:|one clear objective|after this lesson, you can|you will /i,
        `${path.relative(controlRoot, readmePath)} must teach the engineering need rather than carry objective or catalog prose`
      );
      assert.notStrictEqual(
        need,
        objective,
        `${path.relative(controlRoot, readmePath)} must explain the missing physical capability instead of repeating its objective`
      );
      assert.notStrictEqual(
        need,
        `we need to be able to ${objective.replace(/[.]$/, "")}.`,
        `${path.relative(controlRoot, readmePath)} must make the engineering need understandable before naming the new technique`
      );
      const stageStarts = [...experiment.matchAll(/^(\d+)\. \*\*([^*]+)\*\* —/gm)];
      const visibleWorkflowStages = lesson.workflowStages.map((stage: string) =>
        stage === "Tune" ? "Tune / Design" : stage
      );
      assert.deepStrictEqual(
        stageStarts.map((match) => match[2]),
        visibleWorkflowStages,
        `${path.relative(controlRoot, readmePath)} must list only its declared Control Studio stages in order`
      );
      const requiredActionLabels = [
        "Baseline check",
        "Decision before action",
        "Successful evidence",
        "Recovery",
        "Record",
      ];
      for (let index = 0; index < stageStarts.length; index += 1) {
        assert.strictEqual(Number(stageStarts[index][1]), index + 1,
          `${path.relative(controlRoot, readmePath)} must number Control Studio stages consecutively`);
        const stageText = experiment.slice(
          stageStarts[index].index,
          stageStarts[index + 1]?.index ?? experiment.length
        );
        assert.deepStrictEqual(
          [...stageText.matchAll(/^\s+- \*\*([^*]+):\*\*/gm)].map((match) => match[1]),
          requiredActionLabels,
          `${path.relative(controlRoot, readmePath)} stage ${stageStarts[index][2]} must use the exact Lesson 8 action sequence`
        );
      }
    }
  });

  test("Lesson 1 teaches cold-room loop anatomy without borrowing later model theory", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    const definitions = [
      /process.*physical system whose behavior/i,
      /actuator.*physical device.*changes the process/i,
      /sensor.*device.*measures/i,
      /setpoint \(SP\).*temperature.*operator wants/i,
      /process value \(PV\).*temperature.*sensor reports/i,
      /manipulated variable \(MV\).*compressor command/i,
      /disturbance.*affects.*temperature.*not commanded/i,
      /operating point.*conditions.*before.*experiment/i,
      /safe state.*compressor command.*0 percent/i,
      /reverse process direction.*more compressor.*lower temperature/i,
      /feedback.*measurement.*controller.*future lesson/i,
      /error \(e\).*setpoint.*minus.*process value/i,
      /requested manipulated variable \(MV_requested\).*operator.*asks/i,
      /applied manipulated variable \(MV_applied\).*compressor.*receives/i,
      /door-open indication.*zero.*closed.*one.*open/i,
      /effective surrounding temperature.*thermal condition.*degrees Celsius/i,
    ];
    for (const definition of definitions) {
      assert.match(prose, definition);
    }
    assert.ok(!/time constant|0\.63|63%|63 percent/i.test(readme),
      "Lesson 1 must not borrow time-constant theory taught later");
    assert.match(readme, /ManualDemand : LREAL := 0\.0/);
    assert.match(readme, /ManipulatedValue := MIN\(MAX\(ManualDemand/);
    assert.match(readme, /Cooling endpoint at 20\.0 s.*≤ 1\.50 °C.*0\.84 °C/is);
    assert.match(readme, /Recovery change by 40\.0 s.*≥ 1\.50 °C.*2\.00 °C/is);
    assert.match(readme, /0\.05 °C.*tolerance/i);
    assert.match(readme, /door.*60\.0.*90\.0 seconds/is);
    assert.doesNotMatch(readme, /The equipment in this lesson is Cold-storage room refrigeration\./,
      "Lesson 1 must introduce the equipment naturally instead of repeating its metadata label");
    assert.match(prose, /SP.*2\.0.*PV.*4\.0.*e.*-2\.0.*room.*warmer.*desired/is,
      "Lesson 1 must translate its first error equation into a numerical physical meaning");
    assert.match(prose, /below 0.*0 percent.*between 0 and 100.*unchanged.*above 100.*100 percent/is,
      "Lesson 1 must derive the clamp from the compressor's three possible request regions");
    assert.match(prose, /150.*requested.*100.*applied/is,
      "Lesson 1 must work the exact requested-versus-applied calculation before the experiment");

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "the gallery descriptor and rendered README must state the same lesson objective");
    const pvLimit = lesson.safety.hardLimits.find(
      (limit: { signal: string }) => limit.signal === "Cold-room air temperature"
    );
    assert.deepStrictEqual(pvLimit, {
      signal: "Cold-room air temperature", minimum: -10, maximum: 10, unit: "degC",
    });
    assert.ok(pvLimit.minimum <= lesson.plant.operatingPoint.PV
      && lesson.plant.operatingPoint.PV <= pvLimit.maximum,
    "the declared initial operating point must be inside the abort limits");
    const manualRequest = lesson.signals.find(
      (signal: { name: string }) => signal.name === "Operator manual demand"
    );
    assert.ok(manualRequest.maximum >= 150,
      "the declared request range must include the lesson's intentional 150 percent request");
    assert.strictEqual(lesson.change.settings.requestedManualDemandPercent, 150);
    assert.strictEqual(lesson.change.settings.expectedAppliedDemandPercent, 100);
    for (const signalName of ["Door-open indication", "Effective surrounding temperature"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `lesson metadata must expose ${signalName}`);
    }
    assert.ok(lesson.safety.abortPredicates.some(
      (predicate: string) => predicate.includes("-10") && predicate.includes("10")
    ), "the abort predicate must state the same temperature range as the hard limit");
    assert.doesNotMatch(JSON.stringify(lesson), /3 degC food-temperature limit|3 degC limit/,
      "superseded safety limits must not remain in learner-facing metadata");
    assert.match(lesson.learning.change, /150%.*100%/,
      "the learning summary must preserve the requested-versus-applied comparison");
    const workflowLesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(workflowLesson.workflowStages, ["Monitor", "Test"],
      "Lesson 1 must use only the Control Studio stages that the learner can actually execute");

    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.signals.manipulatedVariable, {
      symbol: "P1.ManualDemand", address: "%ML8", iecType: "LREAL", direction: "read", unit: "%",
      range: { minimum: 0, maximum: 150 },
    }, "the project, not Control Studio Test, must drive the reviewed operator request through the PLC clamp");
    assert.deepStrictEqual(loop.signals.actuator, {
      symbol: "P1.ManipulatedValue", address: "%QL0", iecType: "LREAL", direction: "read", unit: "%",
      range: { minimum: 0, maximum: 100 },
    }, "the applied compressor demand must remain separately observable after the clamp");
    assert.deepStrictEqual(loop.experiment, {
      execution: "project-scripted", kind: "observation", amplitude: 0,
      durationSeconds: 120.1, seed: 20260828,
      activation: { symbol: "P1.ExperimentEnabled", address: "%MX24.0", iecType: "BOOL", direction: "write" },
    }, "Lesson 1 Test must only enable the project-owned 120.1-second observation sequence");
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        processVariable: { ...loop.signals.processVariable, value: 4, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 150 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120.1, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 1 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      affectedSignal: plan.plan?.affectedSignal, address: plan.plan?.address,
      to: plan.plan?.to, restorationValue: plan.plan?.restorationValue,
      durationMs: plan.plan?.durationMs,
    }, {
      affectedSignal: "P1.ExperimentEnabled", address: "%MX24.0", to: true,
      restorationValue: false, durationMs: 120_100,
    });
    assert.match(readme,
      /Experiment.*Observe without excitation.*Amplitude.*0.*Maximum duration.*120\.1[\s\S]*door event/is,
      "Lesson 1 must tell the learner how to execute the cooling interval and continue through the door event");
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m,
      "Lesson 1 must not send a beginner to a disabled shared PID validation stage");

    const requiredFigures = [
      ["figures/cold-room-loop.svg", "Cold-storage room loop elements"],
      ["figures/manual-cooling-response.svg", "Cold-room response to a bounded manual compressor command"],
    ] as const;
    for (const [relativePath, alternativeText] of requiredFigures) {
      assert.ok(readme.includes(`![${alternativeText}](${relativePath})`));
      assert.ok(fs.statSync(path.join(root, relativePath)).isFile());
    }
    const introductoryDiagram = fs.readFileSync(
      path.join(root, "figures", "cold-room-loop.svg"), "utf8"
    );
    assert.doesNotMatch(introductoryDiagram, /\b(?:SP|PV|MV|MV_requested|MV_applied|e)\b/,
      "the diagram shown before Theory must use physical words before control symbols");
    const csv = fs.readFileSync(path.join(root, "figures", "manual-cooling-response.csv"), "utf8")
      .trim().split("\n");
    assert.strictEqual(csv[0], "time_s,setpoint_degC,requested_mv_percent,applied_mv_percent,pv_degC,door_open,effective_surroundings_degC,evidence_class");
    assert.strictEqual(csv.length, 1201, "the figure must retain every 100 ms sample of the 120 s run");
    const simulation = fs.readFileSync(path.join(root, "simulation.toml"), "utf8");
    for (const [plantId, feedbackTarget] of [
      ["door-open-indication", "%IL8"],
      ["effective-surroundings", "%IL16"],
    ]) {
      assert.match(simulation, new RegExp(`id = "${plantId}"[\\s\\S]*?feedback_targets = \\["${feedbackTarget}"\\]`));
    }
    const figure = fs.readFileSync(path.join(root, "figures", "manual-cooling-response.svg"), "utf8");
    for (const label of ["Simulation evidence", "Time (s)", "Cold-room temperature (°C)",
      "Compressor demand (%)", "Command returns to safe state at 20 s"]) {
      assert.ok(figure.includes(label), `manual response graph must contain '${label}'`);
    }
  });

  test("Lesson 2 derives and evaluates single-threshold ON/OFF control from Lesson 1 evidence", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/02-cold-storage-on-off");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(readme, /\*\*Lesson objective:\*\*.*derive.*Boolean.*run.*temperature.*switch count/is);
    for (const definition of [
      /binary actuator.*two physical states/i,
      /switching threshold.*temperature.*changes.*state/i,
      /Boolean control law.*true or false/i,
      /control direction.*temperature.*compressor/i,
      /steady cycling.*repeated.*temperature/i,
      /switching frequency.*output changes.*unit of time/i,
      /chatter.*rapid.*switching.*noise/i,
    ]) {
      assert.match(prose, definition);
    }
    const theory = prose.indexOf("## The new idea");
    const equation = prose.indexOf("RequestedCooling := ProcessValue >= Setpoint");
    assert.ok(theory >= 0 && equation > theory,
      "the Boolean law must follow its ordinary-language explanation");
    assert.match(prose, /process value \(PV\).*measured.*degrees Celsius/i);
    assert.match(prose, /setpoint \(SP\).*desired.*degrees Celsius/i);
    assert.match(prose, /manipulated variable \(MV\).*compressor.*0.*100 percent/i);
    assert.match(readme, /RequestedCooling := ProcessValue >= Setpoint/);
    assert.match(readme, /IF AutoMode THEN/);
    assert.match(readme, /ManipulatedValue := LREAL#100\.0/);
    assert.match(readme, /ManipulatedValue := LREAL#0\.0/);
    assert.match(readme, /ExperimentEnabled AT %MX16\.2/,
      "the controller excerpt must show the mode binding used by the shipped project");
    assert.match(readme, /disable Auto.*0 percent.*safe restoration/is);
    assert.match(readme, /eight.*crossings.*seven.*transitions/is);
    assert.match(readme, /temperature range.*maximum.*minus.*minimum/is);
    assert.match(readme, /2\.40.*1\.90.*0\.50.*same unit/is,
      "Lesson 2 must work a temperature-range calculation and explain its unit");
    for (const frequencyStep of [
      /398 - 78 = 320/,
      /60\.0 s - 20\.0 s = 40\.0 s/,
      /320 transitions \/ 40\.0 s = 8\.00 transitions\/s/,
      /Dividing a\s+transition count by seconds.*unit transitions per\s+second/is,
    ]) {
      assert.match(readme, frequencyStep,
        "Lesson 2 must show every switching-frequency substitution and explain its unit");
    }
    assert.match(readme, /first sample establishes.*no preceding output.*next seven.*seven threshold crossings.*seven output transitions/is,
      "Lesson 2 must distinguish samples, crossings, and output transitions");
    assert.match(readme, /2\.019948.*1\.995103.*0\.024845.*two-decimal.*unrounded retained samples/is,
      "Lesson 2 must expose the unrounded range calculation and explain displayed precision");
    assert.match(readme, /switch count.*number.*changes/is);
    assert.match(readme, /validity prerequisite.*finite.*Auto.*door event/is);
    const validity = prose.indexOf("Validity prerequisite");
    const decisionDirection = prose.indexOf("Decision direction");
    const decisionContractComplete = prose.indexOf("All of these windows, limits, and directions");
    const firstResultPlot = prose.indexOf(
      "![Cold-room response under single-threshold ON/OFF control]"
    );
    const results = prose.indexOf("| Run status |");
    const firstReportedFrequency = prose.indexOf("8.00 transitions/s");
    assert.ok(validity >= 0 && decisionDirection > validity
      && decisionContractComplete > decisionDirection
      && firstResultPlot > decisionContractComplete
      && results > decisionContractComplete
      && firstReportedFrequency > decisionContractComplete,
    "eligibility, criteria, and preferred directions must precede every retained result");
    assert.match(readme, /lower.*preferred.*no physical.*acceptance\s+threshold/is);
    assert.match(readme,
      /\| Numerically valid; physical acceptance not established \|.*0\.02 °C.*2\.02 °C.*760.*8\.00/is);
    assert.match(readme, /60\.0.*90\.0.*second.*door/is);
    assert.match(readme, /Simulation evidence/i);
    assert.doesNotMatch(readme, /The equipment in this lesson is Cold-storage room refrigeration\./,
      "Lesson 2 must continue naturally from Lesson 1 instead of repeating a metadata sentence");
    const frequencyDefinition = prose.indexOf("f_switch = N_switch / T_window");
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    assert.ok(frequencyDefinition > theory && frequencyDefinition < experimentStart,
      "Lesson 2 must explain and derive switching frequency before the learner uses it");
    assert.doesNotMatch(readme, /PV_\{min\}/,
      "Lesson 2 must use one consistent minimum-temperature symbol");
    const workflowLesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(workflowLesson.workflowStages, ["Monitor", "Test"],
      "Lesson 2 must not send the learner to disabled shared PID validation");

    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX16.2", iecType: "BOOL", direction: "write",
    }, "Confirm and start must actually enable the shipped ON/OFF law and restore it disabled");
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 2, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 4, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120.1, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 2 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX16.2",
      from: false, to: true, restorationValue: false, durationMs: 120_100,
    });
    assert.match(readme,
      /Observe without excitation[\s\S]*Amplitude.*0.*disabled[\s\S]*Maximum duration.*120\.1[\s\S]*AutoMode[\s\S]*experiment authority/is,
      "Lesson 2 must explain exactly what the project-scripted Test controls");
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);
    assert.doesNotMatch(readme, /For a sump pump, pressure switch, or domestic thermostat/,
      "Lesson 2 must hand the observed chatter directly to Lesson 3");

    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /AutoMode : BOOL := FALSE/,
      "the shipped lesson must start in the declared disabled safe mode");

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective);
    assert.strictEqual(lesson.plant.seed, 20260828);
    assert.strictEqual(lesson.experiment.seed, 20260828);
    assert.deepStrictEqual(lesson.plant.operatingPoint, { PV: 4, MV: 0 });
    const temperatureLimit = lesson.safety.hardLimits.find(
      (limit: { signal: string }) => limit.signal === "Cold-room air temperature"
    );
    assert.deepStrictEqual(temperatureLimit, {
      signal: "Cold-room air temperature", minimum: -10, maximum: 10, unit: "degC",
    });
    assert.ok(temperatureLimit.minimum <= lesson.plant.operatingPoint.PV
      && lesson.plant.operatingPoint.PV <= temperatureLimit.maximum);
    for (const signalName of ["Door-open indication", "Effective surrounding temperature"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `lesson metadata must expose ${signalName}`);
    }
    assert.strictEqual(lesson.experiment.durationSeconds, 120.1);
    assert.ok(lesson.acceptance.some((metric: { metric: string }) => metric.metric === "temperature_range"));
    assert.ok(lesson.acceptance.some((metric: { metric: string }) => metric.metric === "switch_count"));
    assert.ok(lesson.acceptance.some((metric: { metric: string }) => metric.metric === "noise_crossing_transitions"));

    const simulation = fs.readFileSync(path.join(root, "simulation.toml"), "utf8");
    assert.match(simulation, /seed = 20260828/);
    for (const [plantId, feedbackTarget] of [
      ["door-open-indication", "%IL8"],
      ["effective-surroundings", "%IL16"],
    ]) {
      assert.match(simulation, new RegExp(`id = "${plantId}"[\\s\\S]*?feedback_targets = \\["${feedbackTarget}"\\]`));
    }

    const csv = fs.readFileSync(path.join(root, "figures", "on-off-response.csv"), "utf8")
      .trim().split("\n");
    assert.strictEqual(csv[0], "time_s,setpoint_degC,pv_degC,mv_percent,switch_count,auto_mode,phase,door_open,effective_surroundings_degC,evidence_class");
    assert.strictEqual(csv.length, 1202,
      "the artifact must retain 1,200 experiment samples and the explicit disabled restoration sample");
    const chatter = fs.readFileSync(path.join(root, "figures", "threshold-chatter.csv"), "utf8")
      .trim().split("\n");
    assert.strictEqual(chatter[0], "sample,underlying_temperature_degC,measurement_noise_degC,measured_pv_degC,mv_percent,transition,evidence_class");
    assert.strictEqual(chatter.length, 9);
    for (const [relativePath, alternativeText] of [
      ["figures/on-off-response.svg", "Cold-room response under single-threshold ON/OFF control"],
      ["figures/threshold-chatter.svg", "Noisy threshold crossings cause compressor chatter"],
    ] as const) {
      assert.ok(readme.includes(`![${alternativeText}](${relativePath})`));
      assert.ok(fs.statSync(path.join(root, relativePath)).isFile());
    }
    const chatterFigure = fs.readFileSync(path.join(root, "figures", "threshold-chatter.svg"), "utf8");
    for (const label of ["Measured PV", "Switching threshold", "Applied MV"]) {
      assert.ok(chatterFigure.includes(label),
        `the chatter figure legend must identify '${label}'`);
    }
  });

  test("Lesson 3 derives hysteresis and dwell protection from measured compressor chatter", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/03-cold-storage-hysteresis-dwell");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(readme,
      /\*\*Lesson objective:\*\*.*compare.*single threshold.*hysteresis.*dwell.*select/is);
    assert.match(prose, /Lesson 2.*760.*transitions.*8\.00 transitions per second/is);
    for (const definition of [
      /upper switching threshold \(T_on\).*starts cooling.*degrees Celsius/i,
      /lower switching threshold \(T_off\).*stops cooling.*degrees Celsius/i,
      /hysteresis width \(H\).*distance.*thresholds.*degrees Celsius/i,
      /state memory.*retains.*requested.*inside the band/i,
      /limit cycle.*repeated.*temperature.*binary actuator/i,
      /minimum on-time \(t_on_min\).*seconds.*remain on/i,
      /minimum off-time \(t_off_min\).*seconds.*remain off/i,
      /requested cooling state \(Cooling_requested\).*threshold.*memory/i,
      /applied cooling state \(Cooling_applied\).*dwell.*compressor/i,
      /dwell inhibit.*requested.*cannot yet be applied/i,
    ]) {
      assert.match(prose, definition);
    }
    const definitions = prose.indexOf("upper switching threshold (T_on)");
    const firstThresholdEquation = prose.indexOf("T_on = SP + H / 2");
    assert.ok(definitions >= 0 && firstThresholdEquation > definitions,
      "the switching thresholds must be named physically before their equations");
    assert.match(prose, /H = T_on - T_off.*centered.*half.*above.*half.*below/is,
      "Lesson 3 must derive the centered thresholds from the hysteresis width");
    assert.match(prose, /8\.00 s \/ 0\.10 s = 80 scans.*opposite request.*scan 80/is,
      "Lesson 3 must derive the dwell decision from the task interval");
    assert.match(readme, /RequestedCooling := TRUE/);
    assert.match(readme, /RequestedCooling := FALSE/);
    assert.match(readme, /CoolingEnabled := RequestedCooling/);
    assert.match(readme, /DwellElapsed >= MinimumOnTime/);
    assert.match(readme, /DwellElapsed >= MinimumOffTime/);
    assert.match(prose, /2\.50 °C.*1\.50 °C/is);
    assert.match(readme, /temperature range.*maximum.*minus.*minimum/is);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const measure of ["N_switch", "t_on", "t_off", "N_inhibit"]) {
      const definition = prose.indexOf(measure);
      assert.ok(definition >= 0 && definition < experimentStart,
        `${measure} must be defined and derived before the learner uses it`);
    }
    assert.doesNotMatch(readme, /PV_\{min\}/,
      "Lesson 3 must use one consistent minimum-temperature symbol");
    assert.match(readme, /transition count.*changes.*applied cooling state/is);
    assert.match(readme, /shortest completed on-time.*seconds/is);
    assert.match(readme, /shortest completed off-time.*seconds/is);
    assert.match(readme, /same.*plant.*initial\s+state.*door event.*sample\s+interval.*duration.*seed/is);
    assert.match(readme, /minimum on-time.*8\.00 seconds/is);
    assert.match(readme, /minimum off-time.*8\.00 seconds/is);
    assert.match(readme, /760 applied compressor transitions.*8\.00 transitions per second/is);
    const validity = prose.indexOf("Validity prerequisite");
    const decision = prose.indexOf("Decision direction");
    const baseline = prose.indexOf("### Baseline");
    const comparisonStart = prose.indexOf("### Measures and comparison");
    const firstResultPlot = prose.indexOf(
      "![Cold-room comparison of a single threshold, hysteresis, and dwell protection]"
    );
    const results = prose.indexOf("| Candidate | Run status |");
    assert.doesNotMatch(readme.slice(baseline, comparisonStart), /\b760\b/,
      "the current baseline result must not be revealed before the decision contract");
    assert.ok(validity >= 0 && decision > validity && firstResultPlot > decision && results > decision,
      "the complete validity and decision contract must precede retained Lesson 3 results");
    assert.match(readme,
      /Single threshold.*transition criterion failed.*not eligible.*0\.02 °C.*2\.02 °C.*760.*0\.10 s.*0\.10 s/is);
    assert.match(readme,
      /Hysteresis \|.*simulation criteria satisfied.*eligible.*selected.*1\.01 °C.*2\.51 °C.*12.*6\.80 s.*5\.10 s/is);
    assert.match(readme,
      /Hysteresis with dwell.*simulation criteria satisfied.*eligible.*not selected.*1\.26 °C.*2\.76 °C.*12.*8\.00 s.*8\.00 s.*65/is);
    assert.match(readme,
      /select.*Hysteresis.*simplest eligible.*same 12 transitions.*smaller.*temperature range/is);
    assert.match(readme, /no physical acceptance\s+threshold.*compressor\s+life/is);
    assert.match(readme, /Simulation evidence/i);

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"],
      "Lesson 3 must not send the learner to disabled shared PID validation");
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX44.4", iecType: "BOOL", direction: "write",
    }, "Confirm and start must enable the shipped dwell candidate and restore it disabled");
    for (const [symbol, address] of [
      ["P1.RequestedCooling", "%MX44.1"],
      ["P1.CoolingEnabled", "%MX44.2"],
      ["P1.DwellInhibit", "%MX44.3"],
    ]) {
      assert.ok(loop.signals.additional.some((signal: { symbol: string; address: string; direction: string }) =>
        signal.symbol === symbol && signal.address === address && signal.direction === "read"),
      `${symbol} must be observable in Control Studio`);
    }
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 2, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 4, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120.1, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 3 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX44.4",
      from: false, to: true, restorationValue: false, durationMs: 120_100,
    });
    assert.match(readme,
      /Test runs the shipped hysteresis-with-dwell\s+candidate.*retained three-candidate comparison.*native evidence test/is,
      "Lesson 3 must distinguish the candidate run in Control Studio from the retained comparison");
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);
    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective);
    assert.strictEqual(lesson.plant.seed, 20260828);
    assert.strictEqual(lesson.experiment.seed, 20260828);
    assert.strictEqual(lesson.experiment.durationSeconds, 120.1);
    const temperatureLimit = lesson.safety.hardLimits.find(
      (limit: { signal: string }) => limit.signal === "Cold-room air temperature"
    );
    assert.deepStrictEqual(temperatureLimit, {
      signal: "Cold-room air temperature", minimum: -10, maximum: 10, unit: "degC",
    });
    for (const signalName of ["Requested cooling state", "Applied cooling state",
      "Door-open indication", "Effective surrounding temperature"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `lesson metadata must expose ${signalName}`);
    }
    for (const metric of ["temperature_range", "transition_count",
      "minimum_on_duration", "minimum_off_duration"]) {
      assert.ok(lesson.acceptance.some((entry: { metric: string }) => entry.metric === metric),
        `lesson metadata must define ${metric}`);
    }
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /MinimumOnTime : LREAL := 8\.0/);
    assert.match(source, /MinimumOffTime : LREAL := 8\.0/);
    assert.match(source, /RequestedCooling <> CoolingEnabled/);
    assert.match(source, /DwellInhibit := TRUE/);

    const simulation = fs.readFileSync(path.join(root, "simulation.toml"), "utf8");
    for (const [plantId, feedbackTarget] of [
      ["door-open-indication", "%IL8"],
      ["effective-surroundings", "%IL16"],
    ]) {
      assert.match(simulation,
        new RegExp(`id = "${plantId}"[\\s\\S]*?feedback_targets = \\["${feedbackTarget}"\\]`));
    }

    const comparison = fs.readFileSync(
      path.join(root, "figures", "hysteresis-dwell-comparison.csv"), "utf8"
    ).trim().split("\n");
    assert.strictEqual(comparison[0],
      "case,time_s,setpoint_degC,pv_degC,requested_cooling,applied_cooling,mv_percent,switch_count,dwell_elapsed_s,dwell_inhibit,door_open,effective_surroundings_degC,evidence_class");
    assert.strictEqual(comparison.length, 3601,
      "the artifact must retain all 1,200 samples for each of the three equivalent runs");
    const memory = fs.readFileSync(
      path.join(root, "figures", "threshold-memory.csv"), "utf8"
    ).trim().split("\n");
    assert.strictEqual(memory[0],
      "sample,pv_degC,requested_cooling,applied_cooling,dwell_inhibit,evidence_class");
    assert.ok(memory.length >= 5);
    for (const [relativePath, alternativeText] of [
      ["figures/hysteresis-dwell-comparison.svg", "Cold-room comparison of a single threshold, hysteresis, and dwell protection"],
      ["figures/threshold-memory.svg", "Hysteresis state memory and delayed dwell transition"],
    ] as const) {
      assert.ok(readme.includes(`![${alternativeText}](${relativePath})`));
      assert.ok(fs.statSync(path.join(root, relativePath)).isFile());
    }
    const comparisonFigure = fs.readFileSync(
      path.join(root, "figures", "hysteresis-dwell-comparison.svg"), "utf8"
    );
    for (const label of ["Simulation evidence", "Cold-room temperature (°C)",
      "Applied compressor state", "Single threshold", "Hysteresis", "Dwell"] as const) {
      assert.ok(comparisonFigure.includes(label), `comparison figure must contain '${label}'`);
    }
    const memoryFigure = fs.readFileSync(
      path.join(root, "figures", "threshold-memory.svg"), "utf8"
    );
    for (const label of ["Simulation evidence", "Requested", "Applied", "Inhibit",
      "Above 2.50 °C", "Inside band", "Below 1.50 °C"] as const) {
      assert.ok(memoryFigure.includes(label), `state-memory figure must contain '${label}'`);
    }
  });

  test("Lesson 4 derives proportional action and runs the selected heater candidate", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/04-water-heater-p");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]), canonicalLessonSections
    );
    assert.match(readme, /\*\*Lesson objective:\*\*.*compare.*three proportional gains.*run.*selected.*steady-state.*error/is);
    assert.match(prose, /PB = 100% \/ Kp.*100-percent output span.*percent per degree Celsius.*degrees Celsius/is,
      "Lesson 4 must derive proportional band and its unit");
    assert.match(prose, /MV_required = Kp.*e_ss.*divide both sides by Kp.*e_ss = MV_required \/ Kp/is,
      "Lesson 4 must show the algebra that produces the offset equation");
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["travel =", "e_final =", "PV_range,final", "MV_range,final"]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be defined and explained before the experiment`);
    }
    const validity = prose.indexOf("Validity prerequisite");
    const decision = prose.indexOf("Decision direction");
    const firstPlot = prose.indexOf("![Water-heater responses for three proportional gains]");
    assert.ok(validity >= 0 && validity < experimentStart && decision > validity
      && decision < experimentStart && firstPlot > decision,
    "Lesson 4 must declare validity and selection before practice and retained results");
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX24.1", iecType: "BOOL", direction: "write",
    });
    assert.ok(loop.signals.additional.some((signal: { symbol: string; address: string; direction: string }) =>
      signal.symbol === "P1.ProportionalGain" && signal.address === "%ML8" && signal.direction === "read"),
    "the selected proportional gain must be visible in Monitor");
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 35, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 20, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120.1, seed: 20260824,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 4 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX24.1",
      from: false, to: true, restorationValue: false, durationMs: 120_100,
    });
    assert.match(readme,
      /Test runs the shipped Kp 8.*retained\s+three-gain comparison.*native evidence\s+test/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ProportionalGain : LREAL := 8\.0/);
    assert.match(source, /Setpoint : LREAL := 35\.0/);
  });

  test("Lesson 5 measures operating-point bias and preserves the selected P controller", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/05-water-heater-bias");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(readme,
      /\*\*Lesson objective:\*\*.*measure.*holding.*bias.*compare.*changed.*load/is);
    assert.match(prose,
      /Lesson 4.*gain.*8 percent per degree Celsius.*6\.9 degrees Celsius.*error/is);
    for (const definition of [
      /operating point.*temperature.*heater output.*remain steady/i,
      /required steady heater output \(MV_required\).*balances.*heat loss.*percent/i,
      /controller bias \(u0\).*measured holding output.*percent/i,
      /temperature deviation.*current temperature.*operating-point temperature.*degrees Celsius/i,
      /requested manipulated variable \(MV_requested\).*controller calculates.*percent/i,
      /applied manipulated variable \(MV_applied\).*heater receives.*0.*100 percent/i,
    ]) {
      assert.match(prose, definition);
    }
    const definitions = prose.indexOf("requested manipulated variable (MV_requested)");
    const requestedEquation = prose.indexOf("MV_requested = u0 + Kp * e");
    const appliedEquation = prose.indexOf("MV_applied = clamp(MV_requested, 0%, 100%)");
    assert.ok(definitions >= 0 && requestedEquation > definitions
      && appliedEquation > requestedEquation,
    "requested and applied outputs must be introduced physically before separate equations");
    assert.match(readme, /Parameters\.Bias := OperatingPointBias/);
    assert.match(readme, /RequestedHeaterPower :=/);
    assert.match(readme, /AppliedHeaterPower := Controller\.Update/);
    assert.match(readme, /70.*75.*80 percent.*manual.*trial/is);
    assert.match(readme, /bias.*initialization-only.*restore.*restart/is);
    assert.match(readme, /enable Auto.*explicit/is);
    assert.match(readme, /manual mode.*zero percent.*restoration/is);
    assert.doesNotMatch(readme, /The equipment in this lesson is Electric recirculating water heater\./);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["MAE =", "requested range =", "applied range =", "Delta MV_applied ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 5 practice`);
    }

    const validity = prose.indexOf("Validity prerequisite");
    const decision = prose.indexOf("Decision direction");
    const firstResultPlot = prose.indexOf(
      "![Measured holding output and biased proportional-control comparison]"
    );
    const results = prose.indexOf("| Candidate | Run status |");
    assert.ok(validity >= 0 && decision > validity
      && firstResultPlot > decision && results > decision,
    "the complete validity and decision contract must precede every retained Lesson 5 result");
    assert.match(readme, /final-window mean absolute temperature error.*degrees Celsius/is);
    assert.match(readme, /requested-output range.*percentage points/is);
    assert.match(readme, /applied-output range.*percentage points/is);
    assert.match(readme, /no physical.*acceptance threshold/is);
    assert.match(readme, /Simulation evidence/i);

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective);
    assert.strictEqual(lesson.baseline.settings.proportionalGain, 8,
      "Lesson 5 must inherit the Lesson 4 selected gain");
    assert.strictEqual(lesson.change.settings.proportionalGain, 8,
      "adding bias must not silently change proportional gain");
    for (const signalName of ["Requested heater power", "Applied heater power",
      "Operating-point bias", "Automatic mode", "Cold-water flow load"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `lesson metadata must expose ${signalName}`);
    }
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ProportionalGain : LREAL := 8\.0/);
    assert.match(source, /RequestedHeaterPower/);
    assert.match(source, /AppliedHeaterPower/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX40.1", iecType: "BOOL", direction: "write",
    });
    assert.ok(loop.signals.additional.some((signal: { symbol: string; address: string; direction: string }) =>
      signal.symbol === "P1.ProportionalGain" && signal.address === "%ML8" && signal.direction === "read"),
    "the inherited proportional gain must be visible in Monitor");
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 35, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 20, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120.1, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 5 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX40.1",
      from: false, to: true, restorationValue: false, durationMs: 120_100,
    });
    assert.match(readme,
      /Test\s+runs the shipped measured-bias candidate.*retained manual trials and\s+zero-bias comparison.*native evidence test/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);

    const csv = fs.readFileSync(path.join(root, "figures", "bias-comparison.csv"), "utf8")
      .trim().split("\n");
    assert.strictEqual(csv[0],
      "case,time_s,setpoint_degC,pv_degC,requested_mv_percent,applied_mv_percent,bias_percent,kp_percent_per_degC,auto_mode,load_event,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,evidence_class");
    assert.strictEqual(csv.length, 2403,
      "the artifact must retain 1,200 samples and one safe restoration for each controller case");
    const trials = fs.readFileSync(path.join(root, "figures", "holding-output-trials.csv"), "utf8")
      .trim().split("\n");
    assert.strictEqual(trials[0],
      "trial,time_s,setpoint_degC,pv_degC,requested_mv_percent,applied_mv_percent,bias_percent,kp_percent_per_degC,auto_mode,load_event,sample_time_s,seed,pv_min_degC,pv_max_degC,mv_min_percent,mv_max_percent,phase,evidence_class");
    assert.strictEqual(trials.length, 1804,
      "the artifact must retain 600 samples and a safe restoration for each manual trial");
    for (const output of [70, 75, 80]) {
      const restoration = trials.find((row) => row.startsWith(`manual_${output}_percent,60.1,`));
      assert.ok(restoration?.includes(",0.000000,0.000000,0.0,8.0,false,disabled,0.1,20260828,0.0,80.0,0.0,100.0,safe_restoration,simulation"),
        `manual ${output}% evidence must retain its own zero-output restoration`);
    }
    const qualification = fs.readFileSync(
      path.join(root, "figures", "qualification-summary.csv"), "utf8"
    ).trim().split("\n");
    assert.strictEqual(qualification[0],
      "case,window,start_s,end_s,settled,pv_range_degC,requested_range_pp,applied_range_pp,mean_absolute_error_degC,simulation_criterion,eligible,selected,evidence_class");
    assert.strictEqual(qualification.length, 8,
      "the artifact must retain three manual and four controller-window qualifications");
    for (const [relativePath, alternativeText] of [
      ["figures/holding-output-trials.svg", "Manual trials used to measure the water-heater holding output"],
      ["figures/bias-comparison.svg", "Measured holding output and biased proportional-control comparison"],
    ] as const) {
      assert.ok(readme.includes(`![${alternativeText}](${relativePath})`));
      assert.ok(fs.statSync(path.join(root, relativePath)).isFile());
    }
  });

  test("Lesson 6 isolates integral memory from the inherited biased-P baseline", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/06-water-heater-integral");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(readme,
      /\*\*Lesson objective:\*\*.*integral.*per scan.*constant.*offset.*transient/is);
    assert.match(prose,
      /Lesson 5.*gain.*8 percent per degree Celsius.*75 percent.*1\.15 degrees Celsius/is);
    for (const definition of [
      /integral contribution \(uI\).*stored.*heater.*percent/i,
      /integral gain \(Ki\).*percentage points.*degree Celsius-second/i,
      /cycle time \(dt\).*seconds.*controller updates/i,
      /requested manipulated variable \(MV_requested\).*before.*limit.*percent/i,
      /applied manipulated variable \(MV_applied\).*heater receives.*0.*100 percent/i,
    ]) assert.match(prose, definition);
    // Findings L06-1/L06-2 (docs/internal/reviews/control-course-review-2026-09-03.md):
    // the README now writes the cycle time as \Delta t and the integral state as
    // u_I (LaTeX) consistently, instead of the former unintroduced "dt" and four
    // spellings of "uI". The per-scan update also now uses sample indices k and
    // k-1 (the same convention Lesson 7 uses) rather than a "new"/"old" pair,
    // because pedagogicalText has no \text{} unwrapping rule and a k/k-1 index
    // needs no \text{} label. pedagogicalText's legacy-alias pass renders
    // \Delta t back to "dt" but keeps the u_I underscore, so the literal
    // update-equation substring below reflects that alias, not the former
    // no-underscore "uI,new = uI,old + Ki × e × dt" spelling.
    // (The block previously here asserted Lesson 16 step-identification
    // derivations, e.g. "1-e^{-(t-L)/T}" and decibels, against Lesson 6's README;
    // that content belongs to Lesson 16 and is covered by its own test below.)
    const definitions = prose.indexOf("requested manipulated variable (MV_requested)");
    const equation = prose.indexOf("u_I,k = u_I,k-1 + Ki × e_k × dt");
    assert.ok(definitions >= 0 && equation > definitions,
      "all integral and actuation quantities must be explained before the update equation");
    assert.match(prose, /2 °C.*0\.1 s.*0\.10 percentage point.*ten scans.*1\.0 percentage point/is);
    assert.match(readme, /Parameters\.ProportionalGain := LREAL#0\.0/);
    assert.match(readme, /RequestedHeaterPower/);
    assert.match(readme, /AppliedHeaterPower := Controller\.Update/);
    assert.match(readme, /Manual.*75 percent.*settled.*transfer.*Auto/is);
    assert.match(readme, /each (?:run|case).*Manual.*zero.*restoration/is);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["early MAE =", "final MAE =", "applied movement ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 6 practice`);
    }
    const decision = prose.indexOf("Validity prerequisite");
    const firstResult = prose.indexOf("| Controller case | Run status |");
    assert.ok(decision >= 0 && firstResult > decision,
      "Lesson 6 must define validity and measures before retained results");
    assert.match(readme, /early-load mean absolute temperature error.*60\.1.*75\.0 seconds/is);
    assert.match(readme, /cumulative applied-output movement.*percentage points/is);
    assert.match(readme, /no physical.*acceptance threshold/is);
    assert.match(readme, /integrating plant.*boundary/i);

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.baseline.settings.proportionalGain, 8,
      "Lesson 6 baseline must retain the Lesson 5 selected gain");
    assert.strictEqual(lesson.baseline.settings.bias, 75,
      "Lesson 6 baseline must retain the measured holding output");
    for (const signalName of ["Requested heater power", "Applied heater power",
      "Integral contribution", "Manual heater power", "Automatic mode"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `Lesson 6 metadata must expose ${signalName}`);
    }
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /MeasuredHoldingOutput : LREAL := 75\.0/);
    assert.match(source, /InitialState\.NominalOutput := MeasuredHoldingOutput/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX48.1", iecType: "BOOL", direction: "write",
    });
    assert.ok(loop.signals.additional.some((signal: { symbol: string; address: string; direction: string }) =>
      signal.symbol === "P1.MeasuredHoldingOutput" && signal.address === "%ML40" && signal.direction === "read"),
    "the inherited measured holding output must be visible in Monitor");
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 35, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 20, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120.1, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 6 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX48.1",
      from: false, to: true, restorationValue: false, durationMs: 120_100,
    });
    assert.match(readme,
      /Test\s+runs the shipped integral-only candidate.*retained\s+biased-P comparison.*native\s+evidence test/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);
  });

  test("Lesson 7 tunes the inherited P and I mechanisms with one bounded Ki sweep", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/07-water-heater-pi");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(readme,
      /\*\*Lesson objective:\*\*.*proportional gain.*fixed.*integral gain.*bounded comparison.*PI/is);
    assert.match(prose,
      /Lesson 5.*1\.15 degrees Celsius.*Lesson 6.*0\.02 degrees Celsius.*1\.62.*17\.38 percentage points/is);
    for (const definition of [
      /proportional increment \(delta_uP\).*change in error.*percentage points/i,
      /integral increment \(delta_uI\).*present error.*cycle time.*percentage points/i,
      /requested manipulated variable \(MV_requested\).*preliminary.*before.*limit/i,
      /applied manipulated variable \(MV_applied\).*bounded.*heater/i,
      /integral time \(Ti\).*seconds.*same output contribution/i,
    ]) assert.match(prose, definition);
    const definitions = readme.indexOf("proportional increment");
    // Notation corrected per review finding L07-2 (one consistent Delta u_{P,k}/
    // Delta u_{I,k} subscript form, replacing the four incompatible renderings)
    // and L07-4 (cycle time written as Delta t, not dt, in every formula).
    const discreteEquation = readme.indexOf("\\Delta u_{P,k} = K_p \\times (e_{k} - e_{k-1})");
    assert.ok(definitions >= 0 && discreteEquation > definitions,
      "Lesson 7 must explain its actual incremental PI quantities before equations");
    assert.match(readme, /\\Delta u_\{I,k\} = K_i \\times e_\{k\} \\times \\Delta t/);
    assert.match(readme,
      /MV_\{\\mathrm\{requested\},k\} = MV_\{\\mathrm\{previous\},k-1\} \+ \\Delta u_\{P,k\} \+ \\Delta u_\{I,k\}/);
    assert.match(readme, /T_i = K_p \/ K_i/);
    assert.match(prose,
      /2 (?:°C|degrees Celsius).*Ki.*4.*0\.1 s.*75 percent.*0 percentage points.*0\.8 percentage point.*75\.8 percent/is);
    assert.match(prose, /Ki.*0\.5.*2.*4.*8.*Ti.*16.*4.*2.*1 second/is);
    assert.match(readme, /Parameters\.ProportionalGain := ProportionalGain/);
    assert.match(readme, /Parameters\.IntegralGain := IntegralGain/);
    assert.match(readme,
      /live-mapped.*next 0\.1-second scan.*without\s+reinitializing.*controller state/is);
    assert.match(readme, /RequestedHeaterPower/);
    assert.match(readme, /AppliedHeaterPower := Controller\.Update/);
    assert.match(readme, /Manual.*75 percent.*settled.*Auto/is);
    assert.match(readme, /gain edit.*not.*case reset/is);
    assert.match(readme,
      /restore.*Manual holding condition.*before (?:starting|beginning).*next candidate/is);
    assert.match(readme, /every run.*120\.1 seconds.*Manual.*zero requested and applied output/is);

    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["recovery time =", "post-load overshoot =",
      "applied movement =", "saturation duration ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 7 practice`);
    }

    const decision = prose.indexOf("Validity prerequisite");
    const firstResult = prose.indexOf("| Controller case | Run status |");
    assert.ok(decision >= 0 && firstResult > decision,
      "Lesson 7 must define validity, criteria and ranking before retained results");
    assert.match(readme, /recovery time.*0\.2 degrees Celsius.*remains.*end/is);
    assert.match(readme, /post-load overshoot.*60\.1.*120\.0 seconds/is);
    assert.match(readme, /cumulative applied-output movement.*percentage points/is);
    assert.match(readme, /zero normal-run saturation duration/is);
    assert.match(readme, /select.*shortest recovery.*one sample.*lower.*movement/is);
    assert.match(readme, /simulation teaching criteria.*not physical.*acceptance threshold/is);
    assert.match(readme, /separate.*45 (?:°C|degrees Celsius).*exposure/is);
    assert.match(readme,
      /selected controller requests 159\.00 percent.*heater applies at most 100\.00 percent/is);

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.baseline.settings.biasedP.proportionalGain, 8,
      "Lesson 7 must retain the Lesson 5 selected proportional gain");
    assert.deepStrictEqual(lesson.change.settings.integralGainCandidates, [0.5, 2, 4, 8],
      "Lesson 7 must expose the complete bounded one-factor Ki sweep");
    for (const signalName of ["Requested heater power", "Applied heater power",
      "Proportional increment", "Integral increment", "Manual heater power", "Automatic mode"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `Lesson 7 metadata must expose ${signalName}`);
    }
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /MeasuredHoldingOutput : LREAL := 75\.0/);
    assert.match(source, /InitialState\.NominalOutput := MeasuredHoldingOutput/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX64.0", iecType: "BOOL", direction: "write",
    });
    assert.ok(loop.signals.additional.some((signal: { symbol: string; address: string; direction: string }) =>
      signal.symbol === "P1.MeasuredHoldingOutput" && signal.address === "%ML56" && signal.direction === "read"),
    "the inherited measured holding output must be visible in Monitor");
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 35, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 20, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 120, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 7 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX64.0",
      from: false, to: true, restorationValue: false, durationMs: 120_000,
    });
    assert.match(readme,
      /Test\s+runs the shipped selected PI candidate.*retained Ki sweep.*native\s+evidence test/is);
    assert.doesNotMatch(readme, /^3\. \*\*(?:Tune \/ Design|Validate)\*\*/m);
    for (const artifact of ["pi-comparison.csv", "qualification-summary.csv",
      "unreachable-exposure.csv", "pi-comparison.svg"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 7 must retain ${artifact}`);
    }
  });

  test("Lesson 8 diagnoses external amplitude saturation from one isolated setpoint experiment", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/08-water-heater-saturation");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lessonObjective = JSON.parse(
      fs.readFileSync(path.join(root, "lesson.json"), "utf8")
    ).objective;
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(readme,
      /\*\*Lesson objective:\*\*.*requested.*applied.*reachab.*saturation.*integral windup/is);
    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([^\n]+)/)?.[1]?.trim();
    assert.strictEqual(objective, lessonObjective,
      "Lesson 8 README and gallery metadata must state the same objective");
    const need = readme.match(/## What we need\s+([\s\S]*?)\n## The new idea/)?.[1]?.trim();
    assert.notStrictEqual(need, objective,
      "Lesson 8 must explain the needed physical capability instead of repeating its objective");
    for (const duplicate of ["### Preparation", "### Baseline", "### Change"]) {
      assert.ok(!readme.includes(duplicate),
        `Lesson 8 must use one Control Studio procedure rather than a duplicate ${duplicate} section`);
    }
    assert.ok(!readme.includes("```mermaid"),
      "Lesson 8 must not rely on Mermaid, which renders as an empty block in the shipped VS Code preview");
    assert.match(readme, /!\[[^\]]*signal path[^\]]*\]\(figures\/[^)]+\.svg\)/i,
      "Lesson 8 must ship its controller-to-plant signal-path diagram as an SVG");
    assert.match(readme, /```st\n[\s\S]*?```/,
      "Lesson 8 must use the registered ST alias for highlighted code");
    assert.match(prose,
      /Lesson 7.*Kp.*8.*Ki.*4.*159 percent.*100 percent.*measured mismatch/is);
    for (const definition of [
      /setpoint \(SP\).*desired outlet-water temperature.*degrees Celsius/i,
      /process value \(PV\).*measured outlet-water temperature.*degrees Celsius/i,
      /error \(e\).*setpoint minus.*process value.*degrees Celsius/i,
      /requested manipulated variable \(MV_requested\).*controller calculates.*before.*physical limit.*percent/i,
      /applied manipulated variable \(MV_applied\).*heater.*physically receives.*percent/i,
      /lower heater limit \(MV_min\).*0 percent/i,
      /upper heater limit \(MV_max\).*100 percent/i,
      /saturation error \(e_sat\).*requested.*minus.*applied.*percentage points/i,
      /stored integral contribution \(u_?I\).*heater.*percent/i,
    ]) assert.match(prose, definition);
    const appliedDefinition = prose.indexOf("applied manipulated variable (MV_applied)");
    const lowerLimitDefinition = prose.indexOf("lower heater limit (MV_min)");
    const upperLimitDefinition = prose.indexOf("upper heater limit (MV_max)");
    const clampEquation = prose.indexOf("MV_applied = max(min(MV_requested, MV_max), MV_min)");
    assert.ok(appliedDefinition >= 0 && lowerLimitDefinition > appliedDefinition
      && upperLimitDefinition > lowerLimitDefinition && clampEquation > upperLimitDefinition,
    "Lesson 8 must explain every quantity used by the clamp before its equation");
    const saturationDefinition = prose.indexOf("saturation error (e_sat)");
    const saturationEquation = prose.indexOf("e_sat = MV_requested - MV_applied");
    assert.ok(saturationDefinition >= 0 && saturationEquation > saturationDefinition,
      "Lesson 8 must explain saturation error before its equation");
    assert.match(prose,
      /ambient.*temperature \(T0\).*20 degrees Celsius.*steady heater gain \(Kh\).*0\.2 degrees Celsius per percent/is);
    assert.match(readme,
      /MV_\{\\mathrm\{required\}\} = \(SP - T_\{0\}\) \/ K_h/);
    assert.match(prose,
      /At 35 degrees Celsius.*75%.*positive 25 percentage points.*reachable/is);
    assert.match(prose,
      /At 45 degrees Celsius.*125%.*minus 25 percentage points.*unreachable/is);
    assert.match(prose,
      /proportional increment \(delta_uP\).*80 percentage points.*integral increment \(delta_uI\).*4 percentage points.*159 percent.*59 percentage points/is);
    assert.ok(readme.includes(
      "\\Delta u_P = K_p \\times (e_{\\mathrm{new}} - e_{\\mathrm{previous}})"),
      "Lesson 8 must show how the proportional increment is calculated");
    assert.ok(readme.includes(
      "\\Delta u_I = K_i \\times e_{\\mathrm{new}} \\times \\Delta t"),
      "Lesson 8 must show how the integral increment is calculated");
    assert.match(readme, /Parameters\.ProportionalGain := ProportionalGain/);
    assert.match(readme, /Parameters\.IntegralGain := IntegralGain/);
    assert.match(readme, /InitialState\.NominalOutput := ManualHeaterPower/);
    assert.match(readme, /RequestedHeaterPower := Controller\.Update/);
    assert.match(readme,
      /AppliedHeaterPower := MAX\(MIN\(RequestedHeaterPower, LREAL#100\.0\), LREAL#0\.0\)/);
    assert.match(readme,
      /live.*next 0\.1-second scan.*without.*reset.*integral state/is);
    assert.match(readme,
      /sequence begins at Manual zero.*applies 75 percent.*reaches 35 degrees\s+Celsius.*transfers to Auto.*requests 45 degrees\s+Celsius.*for 10 seconds.*restores 35 degrees Celsius.*Manual zero/is);
    assert.match(readme, /without.*cold-water.*disturbance/is);

    const decision = prose.indexOf("Validity prerequisite");
    const firstResult = prose.indexOf("| Phase or target | Run status |");
    assert.ok(decision >= 0 && firstResult > decision,
      "Lesson 8 must define validity and measures before retained results");
    assert.match(readme, /reachability margin.*percentage points.*negative.*unreachable/is);
    assert.match(readme, /post-restoration saturation hold.*elapsed time.*leaves.*upper limit/is);
    assert.match(readme, /recovery time.*0\.2 degrees Celsius.*2\.0 (?:continuous )?seconds/is);
    assert.match(readme,
      /426\.05 percent.*326\.05 percentage points.*10\.0[- ]second.*12\.7 seconds.*25\.1 seconds/is);
    assert.match(readme, /simulation evidence.*not.*physical/is);
    assert.match(readme, /proportional-only controller.*no integral state.*anti-windup/is);

    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["reachability margin =", "post-restoration saturation hold =",
      "recovery time ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 8 practice`);
    }
    assert.match(prose,
      /steady state.*heater model.*PV_ss = T0 \+ K_?h \* MV_required.*subtract T0.*divide by K_?h.*MV_required = \(SP - T0\) \/ K_?h/is);
    assert.match(prose,
      /inner minimum.*first.*prevents.*exceeding.*upper limit.*outer maximum.*second.*prevents.*falling below.*lower limit/is);
    // Notation corrected per review findings L08-2 (u_{I,N}/u_{I,0}, consistent
    // with the u_I defined one paragraph earlier), L08-5 (Delta t named at
    // first use instead of bare dt) and L08-6 (quotes Lesson 6's actual
    // unsubscripted-e notation rather than an e_k Lesson 6 never wrote).
    assert.match(prose,
      /u_I,N = u_I,0 \+ sum.*Ki.*e_k.*dt.*positive.*continues.*grow/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.baseline.settings.proportionalGain, 8);
    assert.strictEqual(lesson.baseline.settings.integralGain, 4);
    assert.strictEqual(lesson.experiment.disturbances.length, 0,
      "Lesson 8 metadata must not schedule a simultaneous plant disturbance");
    for (const signalName of ["Requested heater power", "Applied heater power",
      "Proportional increment", "Integral increment", "Manual heater power", "Automatic mode"]) {
      assert.ok(lesson.signals.some((signal: { name: string }) => signal.name === signalName),
        `Lesson 8 metadata must expose ${signalName}`);
    }
    for (const artifact of ["saturation-trace.csv", "saturation-summary.csv", "saturation-response.svg"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 8 must retain ${artifact}`);
    }
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /ExperimentEnabled : BOOL := FALSE/);
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ExperimentCycle/);
    assert.match(source, /ExperimentCycle <= ULINT#500[\s\S]*ManualHeaterPower := LREAL#75\.0/);
    assert.match(source, /ExperimentCycle <= ULINT#550[\s\S]*AutoMode := TRUE/);
    assert.match(source, /ExperimentCycle <= ULINT#650[\s\S]*Setpoint := LREAL#45\.0/);
    assert.match(source, /ExperimentCycle <= ULINT#950[\s\S]*Setpoint := LREAL#35\.0/);
    assert.match(source, /AutoMode := FALSE[\s\S]*ManualHeaterPower := LREAL#0\.0/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX72.0", iecType: "BOOL", direction: "write",
    });
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 35, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 20, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      },
      kind: "project_sequence", amplitude: 0, durationSeconds: 95.1, seed: 20260828,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 8 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({
      kind: plan.plan?.kind, affectedSignal: plan.plan?.affectedSignal,
      address: plan.plan?.address, from: plan.plan?.from, to: plan.plan?.to,
      restorationValue: plan.plan?.restorationValue, durationMs: plan.plan?.durationMs,
    }, {
      kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX72.0",
      from: false, to: true, restorationValue: false, durationMs: 95_100,
    });
    assert.match(readme,
      /Test runs the shipped project-owned saturation sequence.*native\s+evidence test.*same sequence/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);
  });

  test("Lesson 9 proves asymmetric valve-rate limits from requested and applied trajectories", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/09-rate-limit");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 9 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(prose,
      /Lesson 8.*requested.*applied.*amplitude.*in-range position.*travel.*current update/is);
    for (const definition of [
      /setpoint \(SP\).*desired heating-water flow.*litres per minute/i,
      /process value \(PV\).*flow transmitter.*litres per minute/i,
      /requested manipulated variable \(MV_requested\).*controller calculates.*percent/i,
      /applied manipulated variable \(MV_applied\).*rate.*amplitude.*plant.*percent/i,
      /previous applied position \(MV_previous\).*current update.*percent/i,
      /configured controller task interval \(dt\).*seconds/i,
      /maximum opening rate \(r_open\).*percentage points per second/i,
      /maximum closing rate \(r_close\).*percentage points per second/i,
      /lowest admissible position \(MV_low\).*percent/i,
      /highest admissible position \(MV_high\).*percent/i,
      /measured travel rate \(r_measured\).*percentage points per second/i,
    ]) assert.match(prose, definition);
    const quantityDefinition = prose.search(/highest admissible position\s+\(MV_high\)/);
    // L09-2: cycle time now reads Delta t in this formula (not dt); "× dt" is
    // the pedagogicalText normalization of the corrected Delta t, matching
    // the equivalent check already used in control-lesson-schema.test.ts.
    const firstEquation = prose.indexOf("MV_low = max(0%, MV_previous - r_close × dt)");
    assert.ok(quantityDefinition >= 0 && firstEquation > quantityDefinition,
      "Lesson 9 must introduce every actuator-rate quantity before its equations");
    assert.match(prose, /0\.15 percentage point.*0\.05 percentage point.*6\.67 seconds.*20\.0 seconds/is);
    assert.match(readme, /RequestedValvePosition := Controller\.Update/);
    assert.match(readme, /LowerAdmissiblePosition := MAX/);
    assert.match(readme, /AppliedValvePosition := MAX/);
    assert.match(readme, /PreviousPositionUsedForUpdate := PreviousAppliedValvePosition/);
    assert.match(readme, /MeasuredTravelRate :=.*PreviousPositionUsedForUpdate.*ConfiguredCycleTime/is);
    assert.match(prose, /configured nominal interval.*not.*scheduler-derived measurement/is);
    assert.match(prose, /TASK.*100 ms.*cycle_interval_ms.*100.*configured.*0\.1 second/is);
    assert.match(prose, /Lesson 14.*measured elapsed time.*jitter/is);
    assert.match(prose,
      /SP = 0.*closed valve.*10 litres per minute.*15 seconds.*restores? SP = 0.*without resetting.*Manual.*zero/is);

    const metricContract = prose.indexOf("Maximum measured opening rate");
    const firstResult = prose.indexOf("| Measure | Validity");
    assert.ok(metricContract >= 0 && firstResult > metricContract,
      "Lesson 9 must define every decision measure before its retained result");
    assert.match(readme, /5-second flow.*transient.*not.*steady-state/is);
    assert.match(readme, /Simulation evidence.*not.*physical/is);
    assert.match(readme, /integral.*does not track.*rate-constrained.*Lesson 10/is);

    assert.doesNotMatch(readme, /The equipment in this lesson is Motor-operated/);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["maximum opening change =", "maximum closing change =",
      "return-to-closed time ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 9 practice`);
    }
    // L09-2: "r_open × dt" (not "* dt") is the pedagogicalText normalization
    // of the corrected Delta t formula.
    assert.match(prose,
      /rate.*change divided by time.*multiply(?:ing)? both sides by.*(?:dt|update time).*maximum opening change = r_open × dt/is);
    assert.match(prose,
      /previous position minus.*closing change.*previous position plus.*opening change/is);
    assert.match(prose,
      /measured travel rate.*finite-difference.*change in applied position.*divide by.*dt/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /ExperimentEnabled : BOOL := FALSE/);
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ExperimentCycle <= ULINT#151[\s\S]*Setpoint := LREAL#10\.0/);
    assert.match(source, /ExperimentCycle <= ULINT#900[\s\S]*Setpoint := LREAL#0\.0/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.modeSignal, "%MX72.0");
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX80.0", iecType: "BOOL", direction: "write",
    });
    for (const [symbol, address] of [["P1.LowerAdmissiblePosition", "%ML56"],
      ["P1.UpperAdmissiblePosition", "%ML64"]]) {
      assert.ok(loop.signals.additional.some((signal: { symbol: string; address: string }) =>
        signal.symbol === symbol && signal.address === address), `${symbol} must be visible in Monitor`);
    }
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 0, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 0, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      }, kind: "project_sequence", amplitude: 0, durationSeconds: 90.1, seed: 20260824,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 9 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({ affectedSignal: plan.plan?.affectedSignal, address: plan.plan?.address,
      from: plan.plan?.from, to: plan.plan?.to, restorationValue: plan.plan?.restorationValue,
      durationMs: plan.plan?.durationMs }, {
      affectedSignal: "P1.ExperimentEnabled", address: "%MX80.0", from: false, to: true,
      restorationValue: false, durationMs: 90_100,
    });
    assert.match(readme, /Test runs the shipped project-owned rate-limit sequence.*native\s+evidence test.*same sequence/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);

    for (const artifact of ["rate-limit-trace.csv", "rate-limit-summary.csv",
      "rate-limit-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 9 must retain ${artifact}`);
    }
  });

  test("Lesson 10 selects anti-windup from equivalent PI recovery evidence", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/10-saturation-antiwindup");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 10 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(prose,
      /Lessons 8 and 9.*requested.*applied.*retained state.*limit after.*restored/is);
    for (const definition of [
      /setpoint \(SP\).*desired batch-temperature deviation.*degrees Celsius/i,
      /process value \(PV\).*measured batch-temperature deviation.*degrees Celsius/i,
      /requested manipulated variable \(MV_requested\).*controller.*percent/i,
      /applied manipulated variable \(MV_applied\).*valve.*percent/i,
      /nominal controller output \(MV_nominal\).*before.*output limit.*percent/i,
      /saturated candidate \(MV_saturated\).*constrained.*internal output range.*percent/i,
      /saturation error \(e_sat\).*nominal.*minus.*saturated.*percentage points/i,
      /integral increment \(delta_MV_I\).*integral action.*current update.*percentage points/i,
      /cycle time \(dt\).*seconds/i,
      /tracking time \(Tt\).*seconds/i,
      /tracking coefficient \(alpha\).*fraction.*removed.*one update/i,
    ]) assert.match(prose, definition);
    const definitions = prose.search(/tracking coefficient\s+\(alpha\)/i);
    const equation = prose.indexOf("e_sat = MV_nominal - MV_saturated");
    assert.ok(definitions >= 0 && equation > definitions,
      "Lesson 10 must define every anti-windup quantity before equations");
    assert.match(readme, /Parameters\.DerivativeGain := LREAL#0/);
    assert.match(prose, /derivative gain.*zero.*Lesson 20/is);
    assert.match(prose, /strategy selection.*initialization.*fresh run/is);
    assert.match(prose, /0\.030769.*0\.30769 percentage point.*alpha = 1.*complete 10/is);
    assert.match(prose,
      /Manual.*SP = 0.*zero trim.*4\.5 degrees Celsius.*1\.0.*20\.0 seconds.*restore SP = 0.*without resetting/is);
    assert.match(prose,
      /retain.*initial Manual sample.*before.*Auto/is,
      "Lesson 10 must retain the declared initial Manual state before entering Auto");

    const metricContract = prose.indexOf("Recovery time");
    const firstResult = prose.indexOf("| Strategy | Validity");
    assert.ok(metricContract >= 0 && firstResult > metricContract,
      "Lesson 10 must define every decision measure before retained results");
    assert.match(readme, /not recovered.*never.*remains\s+within/is);
    assert.match(prose,
      /automatic recovery observation.*through 50\.0 seconds.*Manual zero.*50\.1 seconds/is,
      "Lesson 10 must declare the complete recovery and restoration timeline");
    assert.match(prose,
      /20\.0-to-50\.0-second automatic observation window/is,
      "Lesson 10 must declare the numerical window used by its recovery metric");
    assert.match(prose,
      /total applied-valve travel.*cumulative absolute applied movement.*percentage points/is);
    assert.match(prose,
      /complete run.*initial Manual sample.*final Manual.*Auto-to-Manual transition/is,
      "Lesson 10 complete-run travel must include both boundary samples and the final transition");
    const selectionRule = prose.search(/Among eligible strategies, select the shortest recovery/i);
    assert.ok(selectionRule >= 0 && selectionRule < firstResult,
      "Lesson 10 must declare its eligibility and selection rule before retained results");
    assert.match(prose, /recovery differences smaller than 0\.1 second.*lower.*valve travel/is);
    assert.match(prose, /travel differences smaller than 0\.1 percentage point.*simpler state equation/is);
    assert.doesNotMatch(prose, /simplest (?:eligible )?fast strategy/i,
      "Lesson 10 must not use an undeclared physical 'fast' threshold");
    assert.doesNotMatch(lesson.learning.explanation, /fast strategy/i,
      "Lesson 10 metadata must not restore the README's rejected undeclared threshold");
    assert.match(lesson.learning.explanation,
      /shortest recovery among eligible strategies/i,
      "Lesson 10 metadata must state the same predeclared selection result as the README");
    assert.match(readme, /Combined.*not.*because.*more\s+elaborate/is);
    assert.match(readme, /Simulation evidence.*not.*physical/is);
    assert.match(prose,
      /setpoint.*does not prove.*disturbance.*Lesson 11.*servo.*regulatory/is,
      "Lesson 10 must hand its unresolved servo-regulatory distinction to Lesson 11");

    assert.doesNotMatch(readme, /The equipment in this lesson is Steam-heated/);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    // Cycle time now reads Delta t (task rule: written as Delta t in words and
    // formulas, not dt); "alpha = delta_t / Tt" is the pedagogicalText
    // normalization of that corrected formula.
    for (const formula of ["e_sat = MV_nominal - MV_saturated", "alpha = delta_t / Tt",
      "MV_stored = MV_nominal - alpha * e_sat", "travel = sum("]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 10 practice`);
    }
    assert.match(prose,
      /saturation error.*unavailable.*difference.*nominal.*saturated/is);
    assert.match(prose,
      /close.*gap.*tracking time.*correction rate.*e_sat\s*\/\s*Tt.*multiply.*dt.*alpha = dt \/ Tt/is);
    assert.match(prose,
      /Tt = dt.*alpha = 1.*substitut.*MV_stored = MV_saturated/is);
    assert.match(prose,
      /product.*delta_MV_I.*e_sat.*same sign.*farther into saturation/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /ExperimentEnabled : BOOL := FALSE/);
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /SelectedStrategy : INT := 2/);
    assert.match(source, /ExperimentCycle < ULINT#10[\s\S]*Setpoint := LREAL#0\.0/);
    assert.match(source, /ExperimentCycle < ULINT#200[\s\S]*Setpoint := LREAL#4\.5/);
    assert.match(source, /ExperimentCycle <= ULINT#500[\s\S]*Setpoint := LREAL#0\.0/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX80.0", iecType: "BOOL", direction: "write",
    });
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 0, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 0, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: -3.5, maximum: 3.5 }, rateLimits: {}, saturation: "none", recording: true,
      }, kind: "project_sequence", amplitude: 0, durationSeconds: 50.1, seed: 20260824,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 10 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({ affectedSignal: plan.plan?.affectedSignal, address: plan.plan?.address,
      from: plan.plan?.from, to: plan.plan?.to, restorationValue: plan.plan?.restorationValue,
      durationMs: plan.plan?.durationMs }, {
      affectedSignal: "P1.ExperimentEnabled", address: "%MX80.0", from: false, to: true,
      restorationValue: false, durationMs: 50_100,
    });
    assert.match(readme, /Test runs the shipped project-owned selected-strategy sequence.*native\s+evidence test.*same sequence/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);

    for (const artifact of ["anti-windup-trace.csv", "anti-windup-summary.csv",
      "anti-windup-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 10 must retain ${artifact}`);
    }
    const figure = fs.readFileSync(path.join(root, "figures", "anti-windup-response.svg"), "utf8");
    assert.match(figure, /forced-color-adjust:\s*none/i,
      "Lesson 10 teaching evidence must retain its encoded distinctions in forced-color themes");
    assert.match(figure, /<svg[^>]+width="900"[^>]+height="760"/i,
      "Lesson 10 figure must fit completely inside the rendered lesson viewport");
  });

  test("Lesson 11 separates servo and regulatory decisions on unchanged PI evidence", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/11-reactor-servo-regulatory");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 11 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(prose,
      /Lesson 10.*setpoint-restoration.*did not.*disturbance.*Lesson 11/is);
    for (const definition of [
      /setpoint \(SP\).*desired reactor temperature.*degrees Celsius/i,
      /process value \(PV\).*measured product temperature.*degrees Celsius/i,
      /applied manipulated variable \(MV_applied\).*valve opening.*percent/i,
      /error \(e\).*setpoint minus.*process value.*degrees Celsius/i,
      /sample index \(k\).*retained controller update/i,
      /cycle time \(dt\).*seconds/i,
      /event-elapsed time \(t_event,k\).*seconds/i,
      /servo test.*changes.*setpoint.*disturbance remains fixed/i,
      /regulatory test.*setpoint remains fixed.*disturbance/i,
    ]) assert.match(prose, definition);
    const iaeEquation = prose.indexOf("IAE =");
    assert.ok(iaeEquation > prose.search(/integral of absolute error \(IAE\).*accumulated error magnitude/is));
    assert.match(readme, /ProportionalGainUsed := LREAL#8\.0/);
    assert.match(readme, /IntegralGainUsed := LREAL#2\.0/);
    assert.match(readme, /DerivativeGainUsed := LREAL#0\.0/);
    assert.match(readme, /BiasUsed := LREAL#50\.0/);
    assert.match(readme, /Parameters\.ProportionalGain := ProportionalGainUsed/);
    assert.match(readme, /Parameters\.IntegralGain := IntegralGainUsed/);
    assert.match(readme, /Parameters\.DerivativeGain := DerivativeGainUsed/);
    assert.match(readme, /Parameters\.Bias := BiasUsed/);
    assert.match(readme, /Input\.CycleTime := ConfiguredCycleTime/);
    assert.match(prose, /5.*degrees Celsius.*40 percentage points.*90 percent/is);
    assert.match(prose, /5.*degree.*cold-feed.*20.*percentage points.*70 percent/is);
    assert.match(prose,
      /initial Manual sample.*Auto.*80.*through 5\.0 seconds.*85.*5\.1 seconds.*55\.0 seconds.*Manual zero.*55\.1 seconds.*until.*zero/is);
    assert.match(prose,
      /fresh regulatory runtime.*60\.0-second.*60\.1 seconds.*120\.0 seconds.*Manual zero.*120\.1 seconds.*reaches zero/is);
    assert.match(prose, /final-window absolute error.*mean absolute error.*110\.1.*120\.0 seconds/is);
    assert.doesNotMatch(prose, /regulatory final error.*final sample/is,
      "Lesson 11 must not present one final sample as equilibrium evidence");

    const criteria = prose.search(/servo run fails.*overshoot.*1\.0 degree Celsius/is);
    const firstResult = prose.indexOf("| Objective | Status");
    assert.ok(criteria >= 0 && firstResult > criteria,
      "Lesson 11 must declare objective-specific criteria before retained results");
    assert.match(prose, /regulatory.*peak deviation.*at least 1\.0 degree Celsius/is);
    assert.match(prose, /recovery.*no greater than 40\.0 seconds.*final-window.*0\.5 degree Celsius/is);
    assert.match(prose,
      /unrounded.*upper[- ]bound.*threshold.*plus.*tolerance.*lower[- ]bound.*threshold.*minus.*tolerance/is,
      "Lesson 11 must explain how tolerances affect decisions before results");
    assert.match(prose, /servo overshoot.*0\.01[- ]degree[- ]Celsius.*tolerance/is);
    assert.match(prose, /0\.5-degree-Celsius settling band.*0\.01-degree-Celsius numerical\s+comparison tolerance/is,
      "Lesson 11 must declare the numerical tolerance used by the settling band");
    assert.match(prose, /Servo settling time.*0\.1-second reproduction\s+tolerance/is,
      "Lesson 11 must declare a reproduction tolerance for its reported servo settling time");
    assert.match(prose, /regulatory peak deviation.*0\.01[- ]degree[- ]Celsius.*tolerance/is);
    assert.match(prose, /recovery time.*0\.1[- ]second.*tolerance/is);
    assert.match(prose, /final-window absolute error.*0\.001[- ]degree[- ]Celsius.*tolerance/is);
    assert.match(prose,
      /IAE.*ISE.*ITAE.*0\.01.*applied-valve travel.*0\.01 percentage point.*reproduc/is,
      "Lesson 11 must declare diagnostic reproduction tolerances");
    assert.match(prose, /IAE.*ISE.*ITAE.*diagnostic.*no physical acceptance threshold/is);
    assert.match(prose, /Simulation evidence.*not.*physical reactor/is);
    assert.match(prose,
      /Lesson 12.*setpoint weighting.*without weakening.*disturbance/is);

    assert.doesNotMatch(readme, /The equipment in this lesson is Jacketed/);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["IAE=", "ISE =", "ITAE=", "servo overshoot =",
      "regulatory peak deviation =", "MV_travel ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 11 practice`);
    }
    assert.match(prose,
      /one retained sample.*rectangle.*height.*error magnitude.*width.*dt.*Summing.*IAE/is);
    assert.match(prose,
      /large errors.*square.*Doubling.*four.*ISE.*degree-Celsius squared seconds/is);
    assert.match(prose,
      /multiply.*absolute error.*event-elapsed time.*late.*ITAE/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /ExperimentEnabled : BOOL := FALSE/);
    assert.match(source, /SelectedObjective : INT := 0/);
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ExperimentCycle <= ULINT#50[\s\S]*Setpoint := LREAL#80\.0/);
    assert.match(source, /ExperimentCycle <= ULINT#550[\s\S]*Setpoint := LREAL#85\.0/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.modeSignal, "%MX72.0");
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX80.0", iecType: "BOOL", direction: "write",
    });
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 80, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 80, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 50, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      }, kind: "project_sequence", amplitude: 0, durationSeconds: 56.5, seed: 20260827,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 11 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({ affectedSignal: plan.plan?.affectedSignal, address: plan.plan?.address,
      from: plan.plan?.from, to: plan.plan?.to, restorationValue: plan.plan?.restorationValue,
      durationMs: plan.plan?.durationMs }, {
      affectedSignal: "P1.ExperimentEnabled", address: "%MX80.0", from: false, to: true,
      restorationValue: false, durationMs: 56_500,
    });
    assert.match(readme, /Control Studio runs the shipped servo objective.*native\s+evidence test.*regulatory objective.*same project-owned sequence/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);

    for (const artifact of ["servo-regulatory-trace.csv", "servo-regulatory-summary.csv",
      "servo-regulatory-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 11 must retain ${artifact}`);
    }
    const figure = fs.readFileSync(path.join(root, "figures", "servo-regulatory-response.svg"), "utf8");
    assert.match(figure, /forced-color-adjust:\s*none/i);
    assert.match(figure, /<svg[^>]+width="900"[^>]+height="760"/i,
      "Lesson 11 four-panel evidence must fit the rendered lesson viewport");
  });

  test("Lesson 12 selects proportional setpoint weight from qualified servo and regulatory evidence", function () {
    this.timeout(30000);
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/12-reactor-setpoint-weighting");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 12 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.match(prose,
      /Lesson 11.*1\.049.*overshoot.*failed.*1\.0.*11\.6.*met.*regulatory/is);
    for (const definition of [
      /setpoint \(SP\).*desired product temperature.*degrees Celsius/i,
      /process value \(PV\).*measured product temperature.*degrees Celsius/i,
      /true error \(e\).*setpoint minus.*process value.*degrees Celsius/i,
      /sample index \(k\).*retained controller update/i,
      /cycle time \(dt\).*0\.1 second/i,
      /applied manipulated variable \(MV_applied\).*valve opening.*percent/i,
      /setpoint change \(delta_SP\).*current.*setpoint.*preceding.*degrees Celsius/i,
      /process-value change \(delta_PV\).*current.*process value.*preceding.*degrees Celsius/i,
      /proportional setpoint weight \(b\).*dimensionless.*zero.*one/i,
      /proportional valve increment \(delta_MV_P\).*percentage points/i,
      /integral valve increment \(delta_MV_I\).*percentage points/i,
    ]) assert.match(prose, definition);
    assert.ok(prose.indexOf("delta_MV_P =") > prose.search(/proportional valve increment \(delta_MV_P\)/i));
    assert.ok(prose.indexOf("delta_MV_I =") > prose.search(/integral valve increment \(delta_MV_I\)/i));
    assert.match(readme, /Parameters\.ProportionalSetpointWeight := ProportionalSetpointWeightUsed/);
    assert.match(readme, /Input\.ManualOutput := ManualOutput/);
    assert.match(readme, /ManipulatedValue := Controller\.Update/);
    assert.match(prose, /sampled.*initialized.*restore.*restart/is,
      "Lesson 12 must not present its initialization-only weight as a live setting");
    assert.match(prose, /40 percentage points.*20 percentage points.*zero percentage points/is);
    assert.match(prose, /six independently initialized runs/is);
    assert.match(prose,
      /5\.0 seconds.*5\.1 seconds.*55\.0 seconds.*55\.1 seconds/is);
    assert.match(prose,
      /60\.0-second.*60\.1 seconds.*120\.0 seconds.*120\.1 seconds/is);

    const prediction = readme.match(/## Prediction\n([\s\S]*?)\n## Implementation/)?.[1] ?? "";
    const comparisonProse = pedagogicalText(prediction);
    const firstResult = readme.indexOf("| Weight");
    for (const contract of [
      /servo overshoot.*largest positive.*PV.*85.*upper.*requirement.*1\.0.*tolerance.*0\.01/is,
      /settling time.*0\.5.*band.*0\.01.*tolerance.*Not\s+settled/is,
      /applied-valve travel.*sum.*absolute.*percentage points.*0\.01/is,
      /regulatory peak deviation.*lower requirement.*1\.0.*tolerance.*0\.01/is,
      /recovery time.*40\.0.*tolerance.*0\.1/is,
      /final-window absolute error.*mean absolute.*110\.1.*120\.0.*0\.5.*0\.001/is,
      /path disagreement.*separately for PV and MV_applied.*maximum absolute difference.*aligned samples.*1e-10/is,
      /unrounded.*display precision.*presentation/is,
    ]) {
      assert.match(comparisonProse, contract);
    }
    assert.ok(firstResult > readme.indexOf("Decisions use unrounded retained values"),
      "Lesson 12 decision contracts must precede retained results");
    assert.doesNotMatch(prose, /regulatory final error.*final sample/i,
      "Lesson 12 must not present one final sample as equilibrium evidence");
    assert.match(prose, /select.*largest.*b.*pass.*servo.*regulatory/is);
    assert.match(prose, /numerical validity.*physical acceptability.*regulatory equivalence.*selection/is);
    assert.match(prose, /Simulation evidence.*not.*physical/is);
    assert.match(prose, /measurement noise.*still enters.*proportional.*Lesson 13/is);

    assert.doesNotMatch(readme, /The equipment in this lesson is Jacketed/);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["delta_SP =", "delta_PV =", "delta_MV_P =", "delta_MV_I =",
      "servo overshoot =", "MV_travel =", "regulatory peak deviation =",
      "delta_PV_max =", "delta_MV_max ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 12 practice`);
    }
    assert.match(prose,
      /subtract.*preceding.*setpoint.*delta_SP.*subtract.*preceding.*process value.*delta_PV/is);
    assert.match(prose,
      /ordinary incremental PI.*delta_SP - delta_PV.*replace.*delta_SP.*b.*delta_SP/is);
    assert.match(prose,
      /fixed setpoint.*delta_SP.*zero.*weighted term.*zero.*every.*b/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /ExperimentEnabled : BOOL := FALSE/);
    assert.match(source, /SelectedObjective : INT := 0/);
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ExperimentCycle <= ULINT#50[\s\S]*Setpoint := LREAL#80\.0/);
    assert.match(source, /ExperimentCycle <= ULINT#550[\s\S]*Setpoint := LREAL#85\.0/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.modeSignal, "%MX72.0");
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX80.0", iecType: "BOOL", direction: "write",
    });
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 80, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 80, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 50, quality: "good" },
        outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: true,
      }, kind: "project_sequence", amplitude: 0, durationSeconds: 56.5, seed: 20260827,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 12 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({ affectedSignal: plan.plan?.affectedSignal, address: plan.plan?.address,
      from: plan.plan?.from, to: plan.plan?.to, restorationValue: plan.plan?.restorationValue,
      durationMs: plan.plan?.durationMs }, {
      affectedSignal: "P1.ExperimentEnabled", address: "%MX80.0", from: false, to: true,
      restorationValue: false, durationMs: 56_500,
    });
    assert.match(readme, /Control Studio runs the shipped selected-weight servo objective.*native\s+evidence.*six fresh runtimes.*same project-owned sequence/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);

    for (const artifact of ["setpoint-weighting-trace.csv", "setpoint-weighting-summary.csv",
      "setpoint-weighting-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 12 must retain ${artifact}`);
    }
    const figure = fs.readFileSync(path.join(root, "figures", "setpoint-weighting-response.svg"), "utf8");
    assert.match(figure, /forced-color-adjust:\s*none/i);
    assert.match(figure, /<svg[^>]+width="900"[^>]+height="760"/i,
      "Lesson 12 four-panel evidence must fit the rendered lesson viewport");
  });

  test("Lesson 13 selects the weakest qualified pump-pressure measurement filter", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/13-measurement-noise");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 13 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );

    assert.match(prose,
      /Lesson 12.*setpoint weighting.*does not filter.*process value.*recycle valve moves unnecessarily/is);
    for (const definition of [
      /physical pressure \(P_true\).*plant.*bar/i,
      /measurement noise \(n\).*transmitter.*bar/i,
      /raw process value \(PV_raw\).*sampled transmitter.*bar/i,
      /controller process value \(PV_controller\).*after.*measurement path.*bar/i,
      /spectrum.*variation.*frequency/i,
      /quantization.*rounds.*discrete levels/i,
      /cycle time \(dt\).*0\.1 second/i,
      /sampling frequency \(fs\).*10 hertz/i,
      /Nyquist frequency \(f_N\).*5 hertz/i,
      /filter time constant \(tau_f\).*0\.1 second/i,
      /filter coefficient \(a\).*fraction.*difference/i,
      /applied manipulated variable \(MV_applied\).*recycle-valve trim.*percentage points/i,
    ]) assert.match(prose, definition);
    assert.ok(prose.indexOf("PV_raw = P_true + n")
      > prose.search(/raw process value \(PV_raw\)/i));
    assert.ok(prose.indexOf("a = dt / (tau_f + 0.5 * dt)")
      > prose.search(/filter coefficient \(a\)/i));
    assert.match(prose, /resolution.*0\.001 bar.*metadata.*no separate quantizer/is);
    assert.match(prose, /after sampling.*cannot.*undo.*aliasing/is);
    assert.match(prose,
      /continuous reference cutoff.*1\.5915.*not.*exact sampled.*minus-three-decibel/is);
    assert.match(prose, /one-stage.*1\.9591.*cascaded.*1\.2118/is);
    assert.match(prose, /two thirds.*0\.0667 bar.*0\.0444 bar/is);

    for (const sourceContract of [
      /FilterParameters\.TimeConstant := FilterTimeConstantUsed/,
      /FilterParameters\.Order := FilterOrder#FirstOrder/,
      /FilterParameters\.Order := FilterOrder#SecondOrder/,
      // L13-10: the excerpt now quotes the shipped multi-line call verbatim
      // instead of the earlier single-line rewrite.
      /FilterOutput := Filter\.Update\(\s*Input := ProcessValue,\s*CycleTime := ConfiguredCycleTime\s*\)/,
      /Input\.ProcessValue := FilteredValue/,
      /Input\.ManualOutput := ManualOutput/,
      /ManipulatedValue := Controller\.Update/,
    ]) assert.match(readme, sourceContract);
    assert.match(prose, /selector.*initializ.*restart/is);
    assert.match(prose, /invalid.*selection.*Manual zero/is);
    assert.match(prose,
      /three fresh.*same.*seed.*noise sequence.*PI.*timing.*limits/is);
    assert.match(prose,
      /5\.0 seconds.*5\.1 seconds.*40\.0 seconds.*40\.1 seconds.*Manual.*zero/is);

    const prediction = readme.match(/## Prediction\n([\s\S]*?)\n## Implementation/)?.[1] ?? "";
    const comparisonProse = pedagogicalText(prediction);
    const firstResult = readme.indexOf("| Filter");
    for (const contract of [
      /controller-PV increment RMS.*20\.1.*40\.0.*bar/is,
      /applied-valve increment RMS.*20\.1.*40\.0.*percentage points/is,
      /complete-window applied-valve travel.*sum.*absolute.*percentage points.*no physical acceptance threshold/is,
      /response window.*5\.1.*15\.0.*measurement-path lag area.*signed.*bar-seconds/is,
      /final-window absolute mean controller error.*35\.1.*40\.0.*not.*true-plant error/is,
      /pressure-noise attenuation.*no greater than 0\.80.*0\.01.*tolerance/is,
      /valve-activity attenuation.*no greater than 0\.80.*0\.01.*tolerance/is,
      /lag.*no greater than 0\.08.*0\.005.*tolerance/is,
      /pressure bias.*no greater than 0\.05.*0\.001.*tolerance/is,
      /aligned noise realizations.*1e-12/is,
      /unrounded.*display.*presentation/is,
    ]) assert.match(comparisonProse, contract);
    assert.ok(firstResult > readme.search(/select.*fewest filter poles/is),
      "Lesson 13 decision contract and selection rule must precede retained results");
    assert.match(prose, /select.*fewest.*filter poles.*satisf/i);
    assert.match(prose, /smoothest.*not automatically.*engineering choice/is);
    assert.match(prose, /Simulation evidence.*not.*physical/is);
    assert.match(prose, /Lesson 14.*sampling.*timing/is);

    assert.doesNotMatch(readme, /The equipment in this lesson is Centrifugal/);
    const experimentStart = prose.indexOf("## Experiment in Control Studio");
    for (const formula of ["a =", "RMS_delta_PV =", "R_pressure =", "R_valve =",
      "MV_travel =", "lag_area =", "E_final ="]) {
      const position = prose.indexOf(formula);
      assert.ok(position >= 0 && position < experimentStart,
        `${formula} must be derived before Lesson 13 practice`);
    }
    assert.match(prose,
      /continuous lag equation.*approximate.*derivative.*midpoint.*solve.*coefficient/is);
    assert.match(prose,
      /minus three decibels.*one over square root of two.*solve.*cutoff/is);
    assert.match(prose,
      /root mean square.*square.*average.*square root/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
    assert.match(source, /ExperimentEnabled : BOOL := FALSE/);
    assert.match(source, /FilterSelection : INT := 1/);
    assert.match(source, /AutoMode : BOOL := FALSE/);
    assert.match(source, /ExperimentCycle <= ULINT#50[\s\S]*Setpoint := LREAL#0\.0/);
    assert.match(source, /ExperimentCycle <= ULINT#400[\s\S]*Setpoint := LREAL#1\.0/);
    const loop = JSON.parse(fs.readFileSync(path.join(root, "control-loop.json"), "utf8"));
    assert.deepStrictEqual(loop.experiment.activation, {
      symbol: "P1.ExperimentEnabled", address: "%MX88.0", iecType: "BOOL", direction: "write",
    });
    const experimentApi = require("../../controlStudio/experiment") as {
      buildControlExperimentPlan(input: unknown): { kind: string; plan?: any; reason?: string };
    };
    const plan = experimentApi.buildControlExperimentPlan({
      loop,
      sample: {
        availability: "available", source: "Simulation", loopId: loop.id,
        loopRevision: loop.revision, scan: 1, observedAtMs: 1,
        controller: { instance: loop.controller.instance, revision: loop.controller.revision, mode: "Manual" },
        setpoint: { ...loop.signals.setpoint, value: 0, quality: "good" },
        processVariable: { ...loop.signals.processVariable, value: 0, quality: "good" },
        manipulatedVariable: { ...loop.signals.manipulatedVariable, value: 0, quality: "good" },
        outputLimits: { minimum: -10, maximum: 10 }, rateLimits: {}, saturation: "none", recording: true,
      }, kind: "project_sequence", amplitude: 0, durationSeconds: 40.1, seed: 20260824,
    });
    assert.strictEqual(plan.kind, "accepted", `Lesson 13 reviewed Test must be executable: ${plan.reason ?? "unknown"}`);
    assert.deepStrictEqual({ affectedSignal: plan.plan?.affectedSignal, address: plan.plan?.address,
      from: plan.plan?.from, to: plan.plan?.to, restorationValue: plan.plan?.restorationValue,
      durationMs: plan.plan?.durationMs }, {
      affectedSignal: "P1.ExperimentEnabled", address: "%MX88.0", from: false, to: true,
      restorationValue: false, durationMs: 40_100,
    });
    assert.match(readme, /Control Studio runs the shipped selected first-order filter.*native\s+evidence.*three fresh runtimes.*same project-owned sequence/is);
    assert.doesNotMatch(readme, /^3\. \*\*Validate\*\*/m);

    for (const artifact of ["measurement-filter-trace.csv", "measurement-filter-summary.csv",
      "measurement-filter-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 13 must retain ${artifact}`);
    }
    const figure = fs.readFileSync(path.join(root, "figures", "measurement-filter-response.svg"), "utf8");
    assert.match(figure, /forced-color-adjust:\s*none/i);
    assert.match(figure, /<svg[^>]+width="900"[^>]+height="760"/i,
      "Lesson 13 four-panel evidence must fit the rendered lesson viewport");
    for (const [label, expectedY] of [["-0.1", 304], ["0.4", 220.67], ["0.9", 137.33]] as const) {
      const match = figure.match(new RegExp(
        `<text x="67" y="([0-9.]+)" class="tick" text-anchor="end">${label.replace("-", "\\-")}</text>`
      ));
      assert.ok(match, `Lesson 13 response plot must render the ${label} bar tick`);
      assert.ok(Math.abs(Number(match[1]) - expectedY) <= 0.01,
        `Lesson 13 response plot must place ${label} bar at its physical y-coordinate`);
    }
  });

  test("Lesson 14 validates measured controller timing and explicit missed-scan recovery", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/14-sampling-interval");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 14 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.doesNotMatch(prose,
      /The equipment in this lesson is[\s\S]{0,240}The equipment is:/i,
      "Lesson 14 must introduce its equipment once in natural prose");

    assert.match(prose,
      /Lesson 13.*filter.*after sampling.*cannot reconstruct.*alias.*actual elapsed time/is);
    for (const definition of [
      /setpoint \(SP\).*desired.*temperature deviation.*degrees Celsius/i,
      /process value \(PV\).*measured.*temperature deviation.*degrees Celsius/i,
      /temperature error \(e\).*setpoint.*minus.*process value.*degrees Celsius/i,
      /requested manipulated-variable trim \(MV_requested\).*PI controller.*percentage points/i,
      /applied manipulated-variable trim \(MV_applied\).*held.*oven.*percentage points/i,
      /actual cycle time \(dt_actual\).*elapsed timestamp time.*seconds/i,
      /controller cycle time \(dt_controller\).*passed.*time-scaled.*seconds/i,
      /nominal cycle time \(dt_nominal\).*configured.*0\.1 second/i,
      /sampling frequency \(fs\).*samples per second.*hertz/i,
      /Nyquist frequency \(f_N\).*half.*sampling frequency.*hertz/i,
      /timing jitter \(j\).*actual.*minus.*nominal.*seconds/i,
      /missed scan.*greater than.*0\.25 second/i,
    ]) assert.match(prose, definition);
    const lastDefinition = readme.search(/missed scan.*0\.25 second/is);
    for (const equation of [
      "e_{k} = SP_{k} - PV_{k}",
      "\\Delta MV_{I,k} = K_i e_k \\Delta t_{\\mathrm{controller}}",
      "f_s = \\frac{1}{\\Delta t_{\\mathrm{actual}}}",
      "f_{\\mathrm{N}} = \\frac{f_s}{2}",
    ]) assert.ok(readme.indexOf(equation) > lastDefinition,
      `Lesson 14 must define timing quantities before ${equation}`);
    for (const derivation of [
      /integral correction rate.*multiplying.*gain.*error.*percentage points per second/is,
      /rate.*multiplied by elapsed time.*integral contribution/is,
      /one sample arrives every.*one second contains.*samples.*reciprocal/is,
      /one complete.*cycle.*positive half.*negative half.*at least two samples/is,
    ]) assert.match(prose, derivation,
      `Lesson 14 must derive each timing relationship before using it: ${derivation}`);
    assert.match(readme,
      /j_k\s*=\s*\\Delta t_\{\\mathrm\{actual\},k\}\s*-\s*\\Delta t_\{\\mathrm\{nominal\}\}/,
      "Lesson 14 must derive jitter from actual minus nominal interval");
    assert.match(prose, /last output.*between.*updates.*zero-order hold/is);
    const conceptFigure = prose.indexOf(
      "![Continuous oven temperature, controller sample instants, sampled values, and held heater output]"
    );
    assert.ok(conceptFigure >= 0 && conceptFigure < prose.indexOf("sampling frequency (fs)"),
      "Lesson 14 must show the physical sample-and-hold concept before frequency notation");
    assert.match(prose, /negative trim.*less heating.*does not mean.*actively cool/is);
    assert.match(prose, /measured elapsed time.*cannot.*undo aliasing.*slow sampling/is);
    assert.doesNotMatch(prose, /first.order.dead.time|FOPDT/i,
      "Lesson 14 must not borrow plant-model terminology taught in Lessons 15 and 16");

    for (const sourceContract of [
      /TimingValid :=/,
      /TimingFaultLatched := TRUE/,
      /Input\.CycleTime := ControllerCycleTime/,
      /RequestedHeaterTrim := Controller\.Update/,
      /AppliedHeaterTrim := RequestedHeaterTrim/,
      /ResetTimingFault/,
      /Controller\.Initialize/,
      /MaximumCycleTime <= LREAL#0\.25/,
    ]) assert.match(readme, sourceContract);
    assert.match(prose, /invalid.*not clamp.*not call.*PID\.Update/is);
    assert.match(prose, /explicit reset.*automatic.*separate.*enable/is);
    assert.match(prose, /configured 100 ms task.*metadata.*harness.*deterministic intervals/is);

    assert.match(prose,
      /50 milliseconds.*0\.03335.*100 milliseconds.*0\.0667.*200 milliseconds.*0\.1334/is);
    assert.match(prose, /20\/10.*10\/5.*5\/2\.5.*hertz/is);
    assert.match(prose,
      /20\/180\/50\/150\/100 milliseconds.*zero mean jitter.*not.*uniform/is);
    assert.match(prose,
      /fresh.*same.*plant.*gains.*setpoint.*belt speed.*duration.*limits.*seed/is);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.match(prose,
      /Control Studio runs.*selected measured-100-millisecond.*project-owned sequence authority/is);
    assert.match(prose,
      /native evidence.*fresh runtimes.*fixed.*jitter.*nominal.*fault/is);
    assert.match(prose, /does not pretend.*one UI runtime.*all timing cases/is);
    assert.match(prose,
      /Monitor.*Test.*Identify.*Tune.*Validate.*not used/is);
    assert.doesNotMatch(prose, /^3\. \*\*Validate\*\*/m,
      "Lesson 14 must not instruct the learner to use a disabled Validate stage");
    assert.match(readme, /ExperimentEnabled\s*:\s*BOOL\s*:=\s*FALSE/);
    assert.match(readme, /TimingCase\s*:\s*INT\s*:=\s*1/);
    assert.match(readme, /ExperimentCycle\s*<=\s*10/);
    assert.match(readme, /ExperimentCycle\s*<=\s*300/);
    assert.match(readme, /ExperimentCycle\s*=\s*301/);
    assert.match(readme, /ExperimentCycle\s*=\s*303/);
    assert.match(readme, /ExperimentCycle\s*=\s*305/);
    assert.match(readme, /P1\.ExperimentEnabled/);
    const measures = readme.indexOf("### Measures and decision rules");
    const experiment = readme.indexOf("## Experiment in Control Studio");
    assert.ok(measures >= 0 && measures < experiment,
      "Lesson 14 must teach every decision measure before the learner runs Test");

    const comparison = readme.match(/### Measures and decision rules\n([\s\S]*?)\n## Implementation/)?.[1] ?? "";
    const comparisonProse = pedagogicalText(comparison);
    const firstResult = readme.indexOf("| Timing case");
    for (const contract of [
      /first integral contribution.*percentage points.*1e-9/is,
      /timing-use error.*maximum absolute.*dt_controller.*dt_actual.*seconds.*1e-12/is,
      /peak overshoot.*percent.*no more than 20.*0\.1.*tolerance/is,
      /settling.*remain.*0\.02 degrees Celsius.*8\.0 seconds.*0\.1.*tolerance/is,
      /final-window absolute mean error.*25\.0.*30\.0.*0\.02 degrees Celsius.*0\.001.*tolerance/is,
      /requested.*applied.*total variation.*percentage points.*no physical acceptance threshold/is,
      /belt speed.*0\.5 metres per second.*1e-12/is,
      /unrounded.*display.*presentation/is,
    ]) assert.match(comparisonProse, contract);
    assert.ok(firstResult > experiment && measures < experiment,
      "Lesson 14 decision contract must precede retained results");
    assert.match(prose, /Not settled.*no final-window error/is);
    assert.match(prose, /Timing fault.*latched.*Manual zero/is);
    assert.match(prose, /Simulation evidence.*not.*physical.*task period/is);
    assert.match(prose, /Lesson 15.*physical thermal model/is);

    for (const artifact of ["sampling-timing-trace.csv", "sampling-timing-summary.csv",
      "sampling-hold-concept.svg",
      "sampling-timing-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 14 must retain ${artifact}`);
    }
    const figure = fs.readFileSync(path.join(root, "figures", "sampling-timing-response.svg"), "utf8");
    assert.match(figure, /forced-color-adjust:\s*none/i);
    assert.match(figure, /<svg[^>]+width="900"[^>]+height="760"/i,
      "Lesson 14 timing evidence must fit the rendered lesson viewport");
    const concept = fs.readFileSync(path.join(root, "figures", "sampling-hold-concept.svg"), "utf8");
    assert.match(concept, /<svg[^>]+width="900"[^>]+height="360"/i);
    assert.match(concept, /continuous oven temperature/i);
    assert.match(concept, /sample instants/i);
    assert.match(concept, /held heater output/i);
  });

  test("Lesson 15 derives and validates one physical heater model before identification", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));

    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective,
      "Lesson 15 gallery metadata and README must state the same single objective");
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.deepStrictEqual(
      lesson.signals
        .filter((signal: { name: string }) => /heater.*command/i.test(signal.name))
        .map((signal: { name: string; binding: string }) => [signal.name, signal.binding]),
      [
        ["Requested heater power command", "%ML32"],
        ["Applied heater power command", "%QL0"],
      ],
      "Lesson 15 metadata must not conflate requested and applied heater power"
    );
    assert.deepStrictEqual(
      lesson.signals
        .filter((signal: { name: string }) => /input validity|Experiment authority|Manual mode/i.test(signal.name))
        .map((signal: { name: string; binding: string }) => [signal.name, signal.binding]),
      [
        ["Input validity", "%MX40.1"],
        ["Experiment authority", "%MX40.2"],
        ["Manual mode active", "%MX40.3"],
      ],
      "Lesson 15 must expose executable input validity, Experiment authority, and Manual restoration state"
    );
    assert.ok(lesson.acceptance.some((entry: { metric: string; window: string }) =>
      entry.metric === "final_window_equilibrium_error" && entry.window === "29.0 s through 30.0 s"));
    assert.ok(lesson.acceptance.some((entry: { metric: string; limit: number }) =>
      entry.metric === "final_window_pv_range" && entry.limit === 0.004));
    assert.ok(lesson.proof.runArtifactFields.includes("requestedHeaterPower"));
    assert.ok(lesson.proof.runArtifactFields.includes("appliedHeaterPower"));
    assert.ok(lesson.proof.runArtifactFields.includes("safeRestoration"));
    assert.ok(lesson.proof.runArtifactFields.includes("sixSecondAbsoluteError"));
    assert.ok(!lesson.proof.runArtifactFields.includes("residual"),
      "Lesson 15 must not claim a generic residual series that its artifacts do not retain");
    for (const field of ["physicalParameters", "parameterUnits", "assumptions",
      "validityRange", "modelProvenance", "simulationProvenance", "testAuthority",
      "manualMode"]) {
      assert.ok(lesson.proof.runArtifactFields.includes(field),
        `Lesson 15 retained evidence must declare ${field}`);
    }
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    assert.doesNotMatch(prose,
      /\*\*Equipment:\*\*[\s\S]{0,500}The equipment in this lesson is/i,
      "Lesson 15 must introduce the equipment once inside the problem");

    assert.match(prose,
      /Lesson 14.*timing.*cannot explain.*equilibrium.*response speed.*delay/is);
    for (const definition of [
      /water temperature \(T\).*degrees Celsius/is,
      /elapsed time \(t\).*seconds/is,
      /temperature rate \(dT\/dt\).*degrees Celsius per second/is,
      /thermal capacitance \(C\).*kilojoules per degree Celsius/is,
      /heater heat input \(Qin\).*kilowatts/is,
      /heat-loss coefficient \(H\).*kilowatts per degree Celsius/is,
      /ambient temperature \(Tambient\).*degrees Celsius/is,
      /rated heater power \(P100\).*kilowatts/is,
      /requested heater command \(MV_requested\).*percent/is,
      /applied heater command \(MV_applied\).*percent/is,
      /equilibrium temperature \(Teq\).*temperature.*stops changing/is,
      /static gain \(K\).*degrees Celsius per percentage point/is,
      /time constant \(tau\).*63\.2 percent/is,
      /apparent dead time \(L\).*seconds.*response begins/is,
    ]) assert.match(prose, definition);
    const lastDefinition = readme.search(/apparent dead time.*response begins/is);
    for (const equation of [
      "C\\frac{dT}{dt} = Q_{\\mathrm{in}} - H\\left(T-T_{\\mathrm{ambient}}\\right)",
      "Q_{\\mathrm{in}} = \\frac{P_{100}}{100}MV_{\\mathrm{applied}}",
      "T_{\\mathrm{eq}} = T_{\\mathrm{ambient}} + \\frac{Q_{\\mathrm{in}}}{H}",
      "K = \\frac{P_{100}}{100H}",
      "\\tau = \\frac{C}{H}",
    ]) assert.ok(readme.indexOf(equation) > lastDefinition,
      `Lesson 15 must define every physical quantity before ${equation}`);
    assert.match(prose, /one kilowatt.*one kilojoule per second.*every term.*kilowatts/is);
    for (const derivation of [
      "stored thermal energy is",
      "differentiating stored energy",
      "set $dT/dt=0$",
      "Move the loss term to the left, divide by $H$",
      "One percentage point changes heat input",
      "subtract the equilibrium balance",
      "time constant is the reciprocal of that rate",
      "remaining fraction after the apparent dead time",
      "$e^{-1}=0.367879$",
      "$1-0.367879=0.632121$",
    ]) assert.ok(readme.includes(derivation),
      `Lesson 15 must derive its model relationship: ${derivation}`);
    const boundaryFigure = prose.indexOf(
      "![Energy entering, leaving, and accumulating in the recirculating water heater]"
    );
    assert.ok(boundaryFigure >= 0 && readme.indexOf(
      "C\\frac{dT}{dt} = Q_{\\mathrm{in}} - H\\left(T-T_{\\mathrm{ambient}}\\right)"
    ) > readme.indexOf("![Energy entering, leaving, and accumulating"),
      "Lesson 15 must show the energy boundary before the balance equation");

    for (const sourceContract of [
      /ModelValid :=/,
      /InputsValid :=/,
      /ManualMode := NOT ExperimentEnabled/,
      /ModelValid AND InputsValid AND ExperimentEnabled/,
      /AppliedHeaterPower := LREAL#0\.0/,
      /AppliedHeaterPower := RequestedHeaterPower/,
      /PredictedEquilibrium := AmbientTemperature\s+\+ StaticGain \* AppliedHeaterPower/,
    ]) assert.match(readme, sourceContract);
    assert.match(prose, /typed runtime.*nonfinite.*before.*Structured Text.*scan/is);
    assert.match(prose, /Test[- ]authority.*nonzero.*applied/is);
    assert.match(prose, /remove.*Test authority.*Manual mode.*safe zero/is);
    assert.match(prose, /1e6.*diagnostic.*not.*physical operating approval/is);
    assert.match(prose, /heater.*add heat.*cannot actively cool/is);
    assert.match(prose, /Control Studio runs.*project-owned 20-percent heater step/is);
    assert.match(prose, /Test changes only.*P1\.ExperimentEnabled.*false.*true.*false/is);
    assert.match(prose, /Observe without excitation.*Amplitude = 0 \(disabled\).*30\.1/is);
    assert.doesNotMatch(prose, /^3\. \*\*Validate\*\*/m);
    assert.match(prose, /Monitor.*Test.*Validate.*Identify.*not used/is);
    assert.match(prose, /step.*0\.00.*20\.00 percent/is);
    assert.match(prose, /30\.00-second active.*30\.10-second.*safe-restoration/is);
    assert.match(prose, /100-millisecond.*interval/is);
    assert.match(prose, /seed 20260824/is);
    assert.match(prose, /Manual.*requested.*applied.*0 percent.*retained/is);

    const measures = readme.indexOf("### Measures and decision rules");
    const experiment = readme.indexOf("## Experiment in Control Studio");
    assert.ok(measures >= 0 && measures < experiment,
      "Lesson 15 must teach decision measures before practice");
    const comparison = readme.match(/### Measures and decision rules\n([\s\S]*?)\n## Implementation/)?.[1] ?? "";
    const comparisonProse = pedagogicalText(comparison);
    const firstResult = readme.indexOf("| Measure");
    for (const contract of [
      /six-second prediction error.*absolute difference.*22\.52848.*0\.08.*0\.001/is,
      /final-window equilibrium error.*mean absolute.*29\.0.*30\.0/is,
      /peak-to-peak.*0\.004.*Not settled/is,
      /equilibrium error.*0\.02.*0\.001.*tolerance/is,
      /unrounded.*display.*presentation/is,
    ]) assert.match(comparisonProse, contract);
    assert.ok(firstResult > experiment,
      "Lesson 15 decision contract must precede retained results");
    assert.match(prose,
      /30\.10 seconds.*safe-restoration sample.*0\.00 percent requested.*0\.00 percent applied/is,
      "Lesson 15 must tell the learner exactly when the retained safe-restoration sample occurs");
    assert.match(prose, /configured simulation truth.*not.*identification.*physical-plant/is);
    assert.match(prose, /Lesson 16.*estimate.*safe measured experiment/is);

    for (const artifact of ["heater-model-trace.csv", "heater-model-summary.csv",
      "heater-energy-boundary.svg", "heater-model-response.svg", "generate_figures.py"]) {
      assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
        `Lesson 15 must retain ${artifact}`);
    }
    const response = fs.readFileSync(path.join(root, "figures", "heater-model-response.svg"), "utf8");
    assert.match(response, /forced-color-adjust:\s*none/i);
    assert.match(response, /<svg[^>]+width="900"[^>]+height="760"/i);
    const legendLabelY = Number(response.match(
      /<text x="140" y="([0-9.]+)" class="small">configured simulation truth<\/text>/
    )?.[1]);
    const temperatureHeadingY = Number(response.match(
      /<text x="78" y="([0-9.]+)" class="heading">Outlet-water temperature/
    )?.[1]);
    const temperaturePlotTop = Number(response.match(
      /<rect x="78\.0" y="([0-9.]+)" width="780\.0"[^>]+class="panel"/
    )?.[1]);
    const responseEventY = Number(response.match(
      /<text x="[^\"]+" y="([0-9.]+)" class="small">response begins after/
    )?.[1]);
    assert.ok(temperatureHeadingY - legendLabelY >= 20,
      "Lesson 15 response legend needs a separate band above the first plot heading");
    assert.ok(temperaturePlotTop - temperatureHeadingY >= 10,
      "Lesson 15 first plot heading must not collide with the plot frame");
    assert.ok(responseEventY >= temperaturePlotTop + 15,
      "Lesson 15 response-begins annotation belongs inside the first plot, not in its legend band");
    assert.match(response, /safe restoration at 30\.10 s/i,
      "Lesson 15 plot must label the retained 30.10-second restoration event");
    const generator = fs.readFileSync(path.join(root, "figures", "generate_figures.py"), "utf8");
    assert.match(generator, /summary\["restoration_time_s"\]/,
      "Lesson 15 plot must derive restoration timing from retained evidence");
    const boundary = fs.readFileSync(path.join(root, "figures", "heater-energy-boundary.svg"), "utf8");
    assert.match(boundary, /<svg[^>]+width="900"[^>]+height="360"/i);
    assert.match(boundary,
      /<text x="455" y="235" text-anchor="middle" class="small">stored energy<\/text>/i,
      "Lesson 15 boundary figure must keep the stored-energy label inside the water box");
    assert.match(boundary,
      /<text x="455" y="253" text-anchor="middle" class="small">thermal capacitance \(C\)<\/text>/i,
      "Lesson 15 boundary figure must keep the capacitance label inside the water box");
  });

  test("Lesson 16 identifies one validation-required heater model from retained measurements", () => {
    const root = lessonRoot("01-feedback-foundations-and-temperature-control/16-heater-step-identification");
    const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
    const prose = pedagogicalText(readme);
    const specification = fs.readFileSync(
      path.join(repositoryRoot(), "docs", "specs", "38-control-engineering-curriculum.md"),
      "utf8"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
    const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
      .replace(/\s+/g, " ");
    assert.strictEqual(objective, lesson.objective);
    assert.deepStrictEqual(
      [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
      canonicalLessonSections
    );
    // These identification rules were previously asserted inside the Lesson 6 test,
    // which read Lesson 6's README and so could never see them. They belong here.
    for (const derivation of [
      /1-e\^\{-1\}=0\.632120/,
      /0\.632120/,
      /20\\log_\{10\}/,
      /square root/i,
    ]) {
      assert.match(readme, derivation,
        "Lesson 16 must derive each identification rule before practice");
    }
    assert.deepStrictEqual(
      lesson.signals
        .filter((signal: { name: string }) => /heater power/i.test(signal.name))
        .map((signal: { name: string; binding: string }) => [signal.name, signal.binding]),
      [
        ["Requested heater power", "%ML0"],
        ["Applied heater power", "%QL0"],
      ]
    );
    assert.deepStrictEqual(
      lesson.signals
        .filter((signal: { name: string }) => /Safety abort|Input validity|Automatic mode/i.test(signal.name))
        .map((signal: { name: string; binding: string }) => [signal.name, signal.binding]),
      [
        ["Safety abort active", "%MX16.1"],
        ["Input validity", "%MX16.2"],
        ["Automatic mode active", "%MX16.3"],
      ]
    );

    for (const definition of [
      /baseline input.*mean applied heater command.*percent/is,
      /new input.*mean applied.*percent/is,
      /input step amplitude.*percentage points/is,
      /baseline output.*mean outlet.*degrees Celsius/is,
      /final output.*settled mean.*degrees Celsius/is,
      /output change.*degrees Celsius/is,
      /process gain.*degrees Celsius per percentage point/is,
      /apparent dead time.*seconds.*response.*baseline/is,
      /model time constant.*seconds.*63\.2 percent/is,
      /prediction residual.*measured.*minus.*model-predicted/is,
      /root-mean-square residual.*degrees Celsius/is,
      /normalized root-mean-square error.*output span/is,
    ]) assert.match(prose, definition);
    const notationReady = prose.indexOf("Only after defining those physical quantities");
    assert.ok(notationReady >= 0, "Lesson 16 must explicitly transition from words to notation");
    const equations = readme.slice(notationReady);
    for (const equation of [
      /\\Delta u\s*=\s*u_\{1\}\s*-\s*u_\{0\}/,
      /\\Delta y\s*=\s*y_\{1\}\s*-\s*y_\{0\}/,
      /K\s*=\s*\\Delta y\s*\/\s*\\Delta u/,
      /T\s*\\cdot\s*dy\/dt\s*\+\s*y\s*=\s*y_\{0\}/,
      /r_\{k\}\s*=\s*PV_\{\\mathrm\{measured\},k\}\s*-\s*PV_\{\\mathrm\{predicted\},k\}/,
    ]) assert.match(equations, equation, "Lesson 16 must explain quantities before displaying equations");

    for (const contract of [
      /500 active samples.*0\.10.*50\.00 seconds/is,
      /50\.10 seconds.*Manual restoration.*20\.00 percent/is,
      /gap.*0\.15 seconds/is,
      /baseline drift.*0\.001.*degrees Celsius per second/is,
      /input span.*9\.99 percentage points/is,
      /output span.*1\.90 degrees Celsius/is,
      /signal-to-noise ratio.*20 decibels/is,
      /dead time.*two sample intervals.*2 percent lower/is,
      /dead-time rule.*overrides.*least-complex preference/is,
      /sample-resolution envelope.*not.*statistical confidence/is,
      /validation required.*Tune.*closed/is,
    ]) assert.match(prose, contract);
    assert.deepStrictEqual(lesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(lesson.experiment.kind, "observation");
    assert.strictEqual(lesson.experiment.signal, "Experiment authority");
    assert.strictEqual(lesson.experiment.amplitude, 0);
    assert.match(prose, /complete learner workflow is Monitor\s*→\s*Test/is);
    assert.match(prose, /reviewed authority.*Experiment authority.*P1\.ExperimentEnabled/is);
    assert.match(prose, /project owns.*scans 1 through 100.*scans 101 through 500/is);
    assert.doesNotMatch(readme, /^3\. \*\*Identify\*\*/m);
    assert.doesNotMatch(readme, /^4\. \*\*Validate\*\*/m);
    assert.match(
      specification,
      /resolved-dead-time rule overrides (?:the )?simplicity preference[\s\S]*select (?:the )?FOPDT/i,
      "Lesson 16's authoritative specification must make model-selection precedence explicit"
    );
    for (const sourceContract of [
      /Input\.ManualOutput := ExperimentInput/,
      /SafetyAbort OR NOT InputsValid/,
      /Input\.Mode := PIDMode#Manual/,
      /ManipulatedValue := LREAL#0\.0/,
    ]) assert.match(readme, sourceContract);
    assert.match(prose, /configured simulation truth.*after.*fit.*not.*input/is);
    assert.match(prose, /physical heater.*reviewed.*approval/is);
    for (const retainedField of [
      "projectContentRevisionSha256", "structuredTextRevisionSha256", "declaredLimits",
      "exclusions", "abort", "qualification.reasons", "selectionContract", "candidateFits",
    ]) assert.ok(lesson.proof.runArtifactFields.includes(retainedField),
      `Lesson 16 metadata must name retained field ${retainedField}`);

    for (const artifact of [
      "heater-identification-trace.csv", "heater-identification-run-summary.csv",
      "heater-identification-model-evidence.json", "heater-identification-construction.svg",
      "heater-identification-fit.svg", "generate_figures.py",
    ]) assert.ok(fs.statSync(path.join(root, "figures", artifact)).isFile(),
      `Lesson 16 must retain ${artifact}`);
    const construction = fs.readFileSync(path.join(root, "figures", "heater-identification-construction.svg"), "utf8");
    const fit = fs.readFileSync(path.join(root, "figures", "heater-identification-fit.svg"), "utf8");
    assert.match(construction, /<svg[^>]+width="900"[^>]+height="440"/i);
    assert.match(construction, /baseline.*applied step.*2% crossing.*63\.2% crossing/is);
    assert.match(fit, /<svg[^>]+width="900"[^>]+height="760"/i);
    assert.match(fit, /measured simulation PV/i);
    assert.match(fit, /identified FOPDT prediction/i);
    assert.match(fit, /residual.*degC/i);
    assert.match(fit, /validation required/i);
  });

  test("Lessons 17 and 18 validate then tune the same heater with executable evidence", () => {
    const validateRoot = lessonRoot("01-feedback-foundations-and-temperature-control/17-independent-model-validation");
    const tuneRoot = lessonRoot("01-feedback-foundations-and-temperature-control/18-nominal-fopdt");
    const validateReadme = fs.readFileSync(path.join(validateRoot, "README.md"), "utf8");
    const tuneReadme = fs.readFileSync(path.join(tuneRoot, "README.md"), "utf8");
    const validateProse = pedagogicalText(validateReadme);
    const tuneProse = pedagogicalText(tuneReadme);
    const validateLesson = JSON.parse(fs.readFileSync(path.join(validateRoot, "lesson.json"), "utf8"));
    const tuneLesson = JSON.parse(fs.readFileSync(path.join(tuneRoot, "lesson.json"), "utf8"));
    const expectedHeadings = canonicalLessonSections;

    for (const [readme, lesson] of [
      [validateReadme, validateLesson], [tuneReadme, tuneLesson],
    ] as const) {
      assert.deepStrictEqual(
        [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]),
        expectedHeadings
      );
      const objective = readme.match(/\*\*Lesson objective:\*\*\s+([\s\S]*?)\n\n/)?.[1]
        .replace(/\s+/g, " ");
      assert.strictEqual(objective, lesson.objective);
    }

    for (const definition of [
      /sample number \(k\).*recorded update/is,
      /measured process value \(PV_measured\).*degrees Celsius/is,
      /model-predicted process value \(PV_predicted\).*degrees Celsius/is,
      /prediction residual \(r_k\).*measured.*minus.*predicted.*degrees Celsius/is,
      /root-mean-square residual.*typical.*degrees Celsius/is,
      /residual bias.*signed mean.*degrees Celsius/is,
      /lag-one residual correlation.*one residual.*next.*-1.*1/is,
      /parameter uncertainty interval.*plausible range/is,
      /prediction envelope.*lower.*upper/is,
    ]) assert.match(validateProse, definition);
    for (const derivation of [
      /squar.*cannot cancel.*divide by.*mean squared error.*square root.*temperature unit/is,
      /average.*signed residuals.*residual bias/is,
      /two aligned sequences.*subtract.*mean.*divide by.*departure-energy/is,
      /normalizes.*units cancel.*bounded from -1 to 1/is,
      /every lower\/upper parameter-bound combination.*minimum and maximum.*prediction envelope/is,
      /five seconds.*subtracting the 1-second delay.*30\.639106/is,
    ]) assert.match(validateProse, derivation, "Lesson 17 must derive validation measures before practice");
    const residualDefinition = validateProse.search(/prediction residual \(r_k\)/i);
    assert.ok(residualDefinition >= 0);
    assert.ok(validateReadme.indexOf(
      "r_{k} = PV_{\\mathrm{measured},k} - PV_{\\mathrm{predicted},k}"
    ) > validateReadme.search(/prediction residual/i));
    assert.match(validateReadme, /ExperimentEnabled/);
    assert.match(validateReadme, /SafetyAbort OR NOT InputsValid/);
    assert.match(validateReadme, /Input\.Mode := PIDMode#Manual/);
    assert.match(validateReadme, /60\.10 seconds.*Manual.*20\.00 percent/is);
    assert.match(validateReadme, /fixed-flow.*accepted.*cold-flow.*restricted/is);
    assert.match(validateReadme, /Simulation evidence.*not.*physical/is);
    assert.deepStrictEqual(validateLesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(validateLesson.experiment.kind, "observation");
    assert.strictEqual(validateLesson.experiment.signal, "Experiment authority");
    assert.strictEqual(validateLesson.experiment.amplitude, 0);
    assert.strictEqual(validateLesson.experiment.durationSeconds, 60.1);
    assert.match(validateProse, /complete learner workflow is Monitor\s*→\s*Test/is);
    assert.match(validateProse, /reviewed authority.*Experiment authority.*P1\.ExperimentEnabled/is);
    assert.match(validateProse, /project owns.*scans 1 through 100.*scans 101 through 600/is);
    assert.doesNotMatch(validateReadme, /^3\. \*\*Identify\*\*/m);
    assert.doesNotMatch(validateReadme, /^4\. \*\*Validate\*\*/m);
    const validationEvidence = JSON.parse(fs.readFileSync(
      path.join(validateRoot, "figures", "heater-validation-evidence.json"), "utf8"));
    const identificationEvidence = JSON.parse(fs.readFileSync(path.join(
      lessonRoot("01-feedback-foundations-and-temperature-control/16-heater-step-identification"), "figures",
      "heater-identification-model-evidence.json"), "utf8"));
    assert.deepStrictEqual(validationEvidence.frozenModel, identificationEvidence.selected,
      "Lesson 17 must execute the frozen model selected by Lesson 16");
    assert.strictEqual(validationEvidence.runsAreDistinct, true);
    assert.deepStrictEqual(
      validateLesson.signals
        .filter((signal: { name: string }) => /heater power/i.test(signal.name))
        .map((signal: { name: string; binding: string }) => [signal.name, signal.binding]),
      [
        ["Requested heater power", "%ML0"],
        ["Applied heater power", "%QL0"],
      ]
    );
    for (const signal of ["Experiment authority", "Safety abort active", "Input validity", "Manual mode active"]) {
      assert.ok(validateLesson.signals.some((candidate: { name: string }) => candidate.name === signal));
    }

    for (const definition of [
      /desired closed-loop response time \(lambda\).*seconds/is,
      /model gain \(K\).*degrees Celsius per percentage point/is,
      /model time constant \(T\).*seconds/is,
      /model dead time \(L\).*seconds/is,
      /proportional gain \(Kp\).*percent.*degree Celsius/is,
      /integral time \(Ti\).*seconds/is,
      /integral gain \(Ki\).*percent.*degree-Celsius-second/is,
      /integral absolute error \(IAE\).*degree-Celsius-seconds/is,
      /accumulated manipulated-variable travel.*percentage points/is,
    ]) assert.match(tuneProse, definition);
    const gainDefinition = tuneReadme.search(/integral gain.*\$K_i\$/i);
    assert.ok(gainDefinition >= 0);
    for (const equation of [
      "K_p = T / [K \\cdot (\\lambda + L)]",
      "T_i = min[T, 4 \\cdot (\\lambda + L)]",
      "K_i = K_p / T_i",
    ]) assert.ok(tuneReadme.indexOf(equation) > gainDefinition, `Lesson 18 must explain quantities before ${equation}`);
    for (const derivation of [
      /invertible first-order part.*first-order filter.*Q\(s\)/is,
      /equivalent feedback controller.*C\(s\).*Q\(s\).*1 - G\(s\)Q\(s\)/is,
      /delay cannot be cancelled safely.*e\^-Ls.*1-Ls/is,
      /denominator.*lambda\+L.*matching like terms.*Kp.*Ti/is,
      /parallel form.*ideal form.*expanding.*coefficient.*Kp\/Ti/is,
      /absolute value prevents positive and negative errors from cancelling/is,
      /safe 0% to initial command.*every later movement/is,
      /subtraction is the error removed.*division.*relative.*multiplication by 100/is,
    ]) assert.match(tuneProse, derivation,
      `Lesson 18 must derive its controller and measures before practice: ${derivation}`);
    assert.match(tuneReadme, /RunApproved/);
    for (const action of ["GenerateCandidate", "ValidationPassed", "ApplyApproved", "RollbackRequested"]) {
      assert.match(tuneReadme, new RegExp(action), `Lesson 18 must teach the real ${action} action`);
    }
    assert.match(tuneReadme, /candidate.*does not.*apply/is);
    assert.match(tuneReadme, /requested.*applied.*heater/is);
    assert.match(tuneReadme, /Simulation evidence.*not.*physical/is);
    assert.ok(tuneLesson.signals.some((signal: { name: string }) => signal.name === "Simulation run approval"));
    const tuningEvidence = JSON.parse(fs.readFileSync(
      path.join(tuneRoot, "figures", "lambda-tuning-evidence.json"), "utf8"));
    assert.deepStrictEqual(tuningEvidence.modelSource, validationEvidence.modelSource,
      "Lesson 18 must retain the immutable provenance accepted by Lesson 17");
    assert.deepStrictEqual(tuningEvidence.parameterIntervals, validationEvidence.parameterIntervals,
      "Lesson 18 must retain the accepted model parameter intervals");
    assert.deepStrictEqual(tuningEvidence.results.saturationSeconds, {
      empirical: 4, lambda: 3.8000000000000003, perturbed: 3.6,
    });
    assert.strictEqual(tuningEvidence.lifecycle.applyApprovalRequired, true);
    assert.strictEqual(tuningEvidence.restoration.appliedHeaterPercent, 0);
    assert.strictEqual(tuningEvidence.physicalPlantEvidence, false);
    assert.deepStrictEqual(tuningEvidence.validityRange, {
      appliedHeaterPercent: [20, 25], outletTemperatureDegC: [30, 31],
    });
    assert.deepStrictEqual(tuningEvidence.plan, {
      sampleTimeSeconds: 0.1, durationSeconds: 120,
      initialTemperatureDegC: 30.2, initialHeaterPercent: 20,
      setpointDegC: 30.6, coldFlowEventSeconds: 60,
      coldFlowOutputBiasDegC: 29.8, seed: 20260824,
    });
    assert.deepStrictEqual(tuneLesson.plant.initialState, {
      outletTemperature: 30.2, heaterPower: 20,
    });
    assert.deepStrictEqual(tuneLesson.plant.validityRange, [
      { signal: "Outlet-water temperature", minimum: 30, maximum: 31, unit: "degC" },
      { signal: "Applied heater power", minimum: 20, maximum: 25, unit: "%" },
    ]);
    assert.match(tuneReadme, /start.*30\.2.*20 percent.*30\.6.*29\.8/is);
    assert.match(tuneReadme, /6\.08 °C·s.*5\.49 °C·s/is);
    assert.match(tuneReadme, /qualification-limit exposure.*4\.0 s.*3\.8 s/is);
    assert.match(tuneReadme, /212\.79 percentage points/is);
    assert.deepStrictEqual(tuneLesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(tuneLesson.experiment.kind, "observation");
    assert.strictEqual(tuneLesson.experiment.signal, "Experiment authority");
    assert.strictEqual(tuneLesson.experiment.amplitude, 0);
    assert.strictEqual(tuneLesson.experiment.durationSeconds, 120.1);
    assert.match(tuneProse, /complete learner workflow is Monitor\s*→\s*Test/is);
    assert.match(tuneProse, /reviewed authority.*Experiment authority.*P1\.ExperimentEnabled/is);
    assert.match(tuneProse, /rising edge.*generates the candidate.*applies it.*grants run authority/is);
    assert.match(tuneProse, /falling edge.*removes run authority.*rollback.*empirical gains/is);
    assert.doesNotMatch(tuneReadme, /^3\. \*\*Identify\*\*/m);
    assert.doesNotMatch(tuneReadme, /^4\. \*\*Tune/m);
    assert.doesNotMatch(tuneReadme, /^5\. \*\*Validate\*\*/m);

    const catalog = JSON.parse(fs.readFileSync(path.join(
      validateRoot, "..", "..", "..", "catalog.json"), "utf8"));
    const catalogEntry = catalog.find((entry: { id: string }) => entry.id === "control-nominal-fopdt");
    assert.strictEqual(catalogEntry.title, tuneLesson.title,
      "the gallery and lesson descriptor must use the same specific title");
    assert.match(catalogEntry.description, /validated model.*Lambda.*PI/i,
      "the gallery description must state Lesson 18's actual technique");

    for (const [root, files] of [
      [validateRoot, ["validation-response.svg", "validation-residual.svg"]],
      [tuneRoot, ["lambda-comparison.svg", "lambda-uncertainty.svg"]],
    ] as const) {
      for (const file of files) {
        assert.match(
          fs.readFileSync(path.join(root, "figures", file), "utf8"),
          /<svg[^>]+width="900"[^>]+height="(?:440|760)"/i,
          `${file} must be a deterministic readable lesson figure`
        );
        assert.match(
          root === validateRoot ? validateReadme : tuneReadme,
          new RegExp(`figures/${file.replace(".", "\\.")}`)
        );
      }
    }
  });

  test("shows the retained motion-control experiment in Lessons 19 through 25", () => {
    const lessons = [
      ["02-derivative-action-and-dynamic-response/19-servo-position-p", "position-p", "Position stiffness and damping"],
      ["02-derivative-action-and-dynamic-response/20-servo-position-pd", "position-pd", "Derivative action adds damping"],
      ["02-derivative-action-and-dynamic-response/21-underdamped-pid", "loaded-position-pid", "Integral action removes gravity-load position error"],
      ["02-derivative-action-and-dynamic-response/22-servo-derivative-kick", "derivative-kick", "Derivative setpoint handling removes the torque-command kick"],
      ["02-derivative-action-and-dynamic-response/23-servo-filtered-derivative", "filtered-derivative", "Derivative filtering trades encoder-noise activity for lag"],
      ["02-derivative-action-and-dynamic-response/24-servo-mechanical-nonidealities", "mechanical-nonidealities", "Mechanical limits can look like poor controller tuning"],
      ["02-derivative-action-and-dynamic-response/25-servo-digital-realization", "digital-realization", "Digital choices change the implemented controller"],
    ] as const;

    for (const [relative, stem, title] of lessons) {
      const root = lessonRoot(relative);
      const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
      assert.match(readme, new RegExp(`!\\[${title}\\]\\(figures/${stem}\\.svg\\)`));
      assert.match(readme, new RegExp(`\\[Retained samples\\]\\(figures/${stem}-trace\\.csv\\)`));
      assert.match(readme, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
      for (const suffix of [".svg", "-trace.csv", "-summary.csv"]) {
        assert.ok(fs.statSync(path.join(root, "figures", `${stem}${suffix}`)).isFile(),
          `${relative} must ship ${stem}${suffix}`);
      }
      for (const suffix of [".svg", "-trace.csv", "-summary.csv"]) {
        assert.ok(fs.statSync(path.join(root, "figures", `${stem}${suffix}`)).isFile());
      }
      assert.match(
        fs.readFileSync(path.join(root, "figures", `${stem}.svg`), "utf8"),
        new RegExp(`<title id="title">${title}`)
      );
    }

    const positionRoot = lessonRoot("02-derivative-action-and-dynamic-response/19-servo-position-p");
    const positionReadme = fs.readFileSync(path.join(positionRoot, "README.md"), "utf8");
    const positionProse = pedagogicalText(positionReadme);
    const positionLesson = JSON.parse(fs.readFileSync(path.join(positionRoot, "lesson.json"), "utf8"));
    for (const definition of [
      /setpoint.*desired table angle.*process value.*measured angle.*radians/is,
      /proportional gain.*requested motor torque per radian.*N.*m\/rad/is,
      /rotational inertia.*resistance to angular acceleration.*kg.*m/is,
      /viscous damping coefficient.*opposing torque.*angular velocity/is,
      /natural frequency.*oscillation rate.*rad\/s/is,
      /damping ratio.*critical amount.*avoid oscillation/is,
    ]) assert.match(positionProse, definition);
    for (const derivation of [
      /rotational balance.*inertial torque plus viscous-friction torque/is,
      /substitute.*Kp.*r-theta.*x=theta-r.*J.*x.*B.*x.*Kp.*x.*0/is,
      /divide by J.*standard second-order form.*matching coefficients/is,
      /omega_n.*sqrt.*Kp\/J.*zeta.*B.*sqrt.*J.*Kp/is,
      /four times the stiffness.*doubles natural frequency.*halves damping ratio/is,
      /first retained sample.*PV.*0\.9.*first arrival.*not permanent settling/is,
      /maximum absolute value counts positive acceleration and negative braking equally/is,
    ]) assert.match(positionProse, derivation,
      `Lesson 19 must derive mechanics and measures before practice: ${derivation}`);
    assert.deepStrictEqual(positionLesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(positionLesson.experiment.kind, "observation");
    assert.strictEqual(positionLesson.experiment.signal, "Experiment authority");
    assert.strictEqual(positionLesson.experiment.amplitude, 0);
    assert.strictEqual(positionLesson.experiment.durationSeconds, 20.02);
    assert.match(positionProse, /complete learner workflow is Monitor\s*→\s*Test/is);
    assert.match(positionProse, /sole reviewed write is Boolean authority/is);
    assert.match(positionProse, /rising edge.*configures.*Kp=8.*SP.*1 rad.*Auto/is);
    assert.match(positionProse, /falling edge.*restores.*Kp=2.*SP = 0 rad.*Manual.*zero torque/is);
    assert.doesNotMatch(positionReadme, /^3\. \*\*Validate\*\*/m);

    const pdRoot = lessonRoot("02-derivative-action-and-dynamic-response/20-servo-position-pd");
    const pdReadme = fs.readFileSync(path.join(pdRoot, "README.md"), "utf8");
    const pdProse = pedagogicalText(pdReadme);
    const pdLesson = JSON.parse(fs.readFileSync(path.join(pdRoot, "lesson.json"), "utf8"));
    for (const derivation of [
      /minus sign.*opposite direction.*opposes motion/is,
      /substitute.*fixed target angle.*move all motion terms.*x=theta-r/is,
      /standard second-order form.*matching the coefficient.*natural frequency/is,
      /derivative gain appears in the numerator.*raise damping.*stiffness.*unchanged/is,
      /sinusoidal displacement.*differentiation.*one quarter cycle/is,
      /kinetic energy.*differentiate.*substitution.*terms cancel.*energy decrease/is,
      /ratios are dimensionless because radians cancel.*same plant.*measurement windows/is,
    ]) assert.match(pdProse, derivation,
      `Lesson 20 must derive damping and measures before practice: ${derivation}`);
    assert.deepStrictEqual(pdLesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(pdLesson.experiment.kind, "observation");
    assert.strictEqual(pdLesson.experiment.signal, "Experiment authority");
    assert.strictEqual(pdLesson.experiment.amplitude, 0);
    assert.strictEqual(pdLesson.experiment.durationSeconds, 15.02);
    assert.match(pdProse, /learner workflow is exactly Monitor\s*→\s*Test/is);
    assert.match(pdProse, /sole reviewed write.*P1\.ExperimentEnabled/is);
    assert.match(pdProse, /rising edge.*selects.*Kd=3.*1 rad.*Auto/is);
    assert.match(pdProse, /returns false.*restores.*Kd=0.*setpoint zero.*Manual.*zero torque/is);
    assert.match(pdProse, /two fresh runtimes.*same initial conditions.*measurement plan/is);
    assert.doesNotMatch(pdReadme, /^3\. \*\*Validate\*\*/m);

    const loadedRoot = lessonRoot("02-derivative-action-and-dynamic-response/21-underdamped-pid");
    const loadedReadme = fs.readFileSync(path.join(loadedRoot, "README.md"), "utf8");
    const loadedProse = pedagogicalText(loadedReadme);
    const loadedLesson = JSON.parse(fs.readFileSync(path.join(loadedRoot, "lesson.json"), "utf8"));
    for (const derivation of [
      /gravity.*changes with table angle.*does not mean.*always applies 4 N m/is,
      /area under the error curve.*rectangle.*state update/is,
      /PD.*At rest.*rotational balance.*nonlinear equation.*small-angle approximation/is,
      /Expand and collect the angle terms.*remaining error/is,
      /PID equilibrium.*stored state.*angle-dependent holding torque/is,
      /Linearize gravity.*Differentiate the motion equation.*characteristic polynomial/is,
      /Routh table.*first column.*condition.*increasing Ki.*remove stability margin/is,
      /Absolute value prevents.*cancelling.*division by sample count/is,
      /saturation exposure.*0\.02 s.*50 consecutive samples.*one full second/is,
    ]) assert.match(loadedProse, derivation,
      `Lesson 21 must derive gravity equilibrium and integral dynamics before practice: ${derivation}`);
    assert.deepStrictEqual(loadedLesson.workflowStages, ["Monitor", "Test"]);
    assert.strictEqual(loadedLesson.experiment.kind, "observation");
    assert.strictEqual(loadedLesson.experiment.signal, "Experiment authority");
    assert.strictEqual(loadedLesson.experiment.amplitude, 0);
    assert.strictEqual(loadedLesson.experiment.durationSeconds, 30.02);
    assert.match(loadedProse, /complete learner workflow is exactly Monitor\s*→\s*Test/is);
    assert.match(loadedProse, /writes only the reviewed Boolean authority.*P1\.ExperimentEnabled/is);
    assert.match(loadedProse, /rising edge.*selects.*Ki=2.*SP 0\.5 rad.*Auto/is);
    assert.match(loadedProse, /falling edge.*restores.*Ki=0.*SP zero.*Manual mode.*zero torque/is);
    assert.match(loadedProse, /separate fresh PD and PID runtimes.*third fresh saturation-recovery runtime/is);
    assert.doesNotMatch(loadedReadme, /^3\. \*\*Validate\*\*/m);

    for (const [relative, stems] of [
      ["02-derivative-action-and-dynamic-response/21-underdamped-pid", ["loaded-position-recovery"]],
      ["02-derivative-action-and-dynamic-response/25-servo-digital-realization", ["digital-method-comparison", "digital-update-timing", "digital-alias-spectrum"]],
    ] as const) {
      for (const root of [lessonRoot(relative), bundledLessonRoot(relative)]) {
        const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
        for (const stem of stems) {
          assert.match(readme, new RegExp(`figures/${stem}\\.svg`));
          for (const suffix of [".svg", "-trace.csv", "-summary.csv"]) {
            assert.ok(fs.statSync(path.join(root, "figures", `${stem}${suffix}`)).isFile());
          }
        }
      }
    }

    const filtered = fs.readFileSync(path.join(lessonRoot("02-derivative-action-and-dynamic-response/23-servo-filtered-derivative"), "README.md"), "utf8");
    assert.match(filtered, /0\.250 N m.*0\.020 rad s/is,
      "Lesson 23 must declare its activity and lag selection limits before choosing a cutoff");
    const mechanical = fs.readFileSync(path.join(lessonRoot("02-derivative-action-and-dynamic-response/24-servo-mechanical-nonidealities"), "README.md"), "utf8");
    assert.match(mechanical, /six one-effect cases/i);
    assert.match(mechanical, /isolated torque.*(?:±|plus or minus)1\.0 N m/is);
    assert.match(mechanical, /combined.*(?:±|plus or minus)2\.5 N m/is);
    const digital = fs.readFileSync(path.join(lessonRoot("02-derivative-action-and-dynamic-response/25-servo-digital-realization"), "README.md"), "utf8");
    assert.match(digital, /Forward Euler, 20 ms.*0\.02 s.*0\.02 s/i);
    assert.match(digital, /Tustin, 10 ms.*0\.01 s.*0\.01 s/i);
    assert.match(digital, /Tustin, 100 ms.*0\.10 s.*0\.10 s/i);
    assert.match(digital, /final-window.*position span.*torque span/is);
    assert.match(digital, /estimated alias frequency/i);
  });

  test("keeps Lessons 26 through 30 on one production-PID teaching path", () => {
    const headings = canonicalLessonSections;
    const lessons = [
      ["03-production-quality-digital-pid/26-pid-forms", "pid-forms", "Valid realization follows controller structure"],
      ["03-production-quality-digital-pid/27-booster-pump-modes", "booster-pump-startup", "Measured-state initialization prevents a startup bump"],
      ["03-production-quality-digital-pid/28-booster-pump-bumpless-transfer", "bumpless-transfer", "Tracking aligns authority transfer"],
      ["03-production-quality-digital-pid/29-booster-pump-atomic-update", "atomic-update", "Every rate interval starts at the prior applied output"],
      ["03-production-quality-digital-pid/30-booster-pump-invalid-data", "invalid-data-recovery", "Recovery authority advances in separate qualified scans"],
    ] as const;

    for (const [relative, stem, title] of lessons) {
      const root = lessonRoot(relative);
      const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
      const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8"));
      assert.deepStrictEqual(
        [...readme.matchAll(/^## (.+)$/gm)].map((match) => match[1]), headings,
        `${relative} must use the reviewed focused lesson path`
      );
      assert.match(readme.replace(/\s+/g, " "), new RegExp(escapeRegExp(lesson.objective), "i"));
      assert.match(readme, new RegExp(escapeRegExp(lesson.equipment), "i"));
      assert.match(readme, new RegExp(`!\\[${title}\\]\\(figures/${stem}\\.svg\\)`));
      assert.match(readme, new RegExp(`\\[Retained run\\]\\(figures/${stem}-run\\.csv\\)`));
      assert.ok(fs.statSync(path.join(root, "figures", `${stem}-run.csv`)).size > 0);
      assert.match(readme, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
    }
    const handover = fs.readFileSync(path.join(lessonRoot("03-production-quality-digital-pid/28-booster-pump-bumpless-transfer"), "README.md"), "utf8");
    assert.match(handover, /stale tracking.*infeasible/is);
    assert.match(handover, /post-transition recovery.*6\.266 bar/is);
    const atomic = fs.readFileSync(path.join(lessonRoot("03-production-quality-digital-pid/29-booster-pump-atomic-update"), "README.md"), "utf8");
    assert.match(atomic, /six consecutive.*rate intervals.*previous applied output/is);
    const recovery = fs.readFileSync(path.join(lessonRoot("03-production-quality-digital-pid/30-booster-pump-invalid-data"), "README.md"), "utf8");
    assert.match(recovery, /acknowledgement scan.*count.*zero/is);
    assert.match(recovery, /valid scan 1.*valid scan 2.*valid scan 3/is);
  });

  test("all lesson READMEs use preview-safe left-aligned mathematics", () => {
    const controlRoot = path.join(repositoryRoot(), "examples", "control");
    const readmes: string[] = [];
    const visit = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const candidate = path.join(directory, entry.name);
        if (entry.isDirectory()) {
          visit(candidate);
        } else if (
          entry.name === "README.md"
          && directory !== controlRoot
          && fs.existsSync(path.join(directory, "lesson.json"))
        ) {
          readmes.push(candidate);
        }
      }
    };
    visit(controlRoot);
    assert.strictEqual(readmes.length, 84, "the preview-safe mathematics check must cover all 84 lessons");
    const violations = readmes.sort()
      .flatMap((readmePath) => findPreviewUnsafeMathViolations(readmePath, controlRoot));
    assert.ok(
      violations.length === 0,
      `preview-unsafe mathematics in lesson READMEs:\n${violations.join("\n")}`
    );
  });
});
