import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { compareQualifiedTunings } from "../../controlStudio/tuning";
import type { IdentifiedModel, ModelKind } from "../../controlStudio/identification";

function fixturePath(...parts: string[]): string {
  const sourceFixture = path.resolve(
    __dirname,
    "..",
    "..",
    "..",
    "src",
    "test",
    "fixtures",
    "control-engineering",
    ...parts
  );
  if (fs.existsSync(sourceFixture)) {
    return sourceFixture;
  }
  return path.resolve(
    __dirname,
    "..",
    "fixtures",
    "control-engineering",
    ...parts
  );
}

suite("Control engineering deterministic fixtures", () => {
  test("packages every canonical control lesson byte-for-byte including dotfiles", () => {
    const sourceRoot = path.resolve(extensionRootForLessons(), "..", "..", "examples");
    const packagedRoot = path.resolve(extensionRootForLessons(), "media", "examples");
    const catalogText = fs.readFileSync(path.join(sourceRoot, "catalog.json"), "utf8");
    assert.strictEqual(
      fs.readFileSync(path.join(packagedRoot, "manifest.json"), "utf8"),
      catalogText,
      "the packaged manifest must preserve canonical order and fields byte-for-byte"
    );
    const entries = JSON.parse(catalogText) as Array<{ id: string; path: string }>;
    for (const entry of entries.filter((candidate) => candidate.id.startsWith("control-"))) {
      const source = path.join(sourceRoot, entry.path);
      const packaged = path.join(packagedRoot, entry.path);
      const sourceFiles = recursiveFilePaths(source);
      assert.deepStrictEqual(
        recursiveFilePaths(packaged),
        sourceFiles,
        `${entry.id} package paths, including dotfiles, must match its canonical project`
      );
      for (const relative of sourceFiles) {
        assert.deepStrictEqual(
          fs.readFileSync(path.join(packaged, relative)),
          fs.readFileSync(path.join(source, relative)),
          `${entry.id}/${relative} must survive packaging byte-for-byte`
        );
      }
    }
  });

  test("teaches every control lesson with lesson-specific educational prose", () => {
    const examplesRoot = path.resolve(extensionRootForLessons(), "media", "examples");
    const manifest = JSON.parse(fs.readFileSync(path.join(examplesRoot, "manifest.json"), "utf8")) as
      Array<{ id?: string; path?: string }>;
    const generatedBoilerplate = [
      "Predict which of SP, PV, MV, quality, or a limit changes first and what observation would reject the proposed explanation.",
      "Repeat the same bounded experiment and compare tracking, disturbance response, MV movement, constraints, and warnings.",
      "Explain the result from synchronized SP, PV, MV, quality, mode, limit, and event evidence rather than trend appearance alone.",
      "Perturb the dominant plant or actuator parameter by ±20% and decide whether the same control decision remains defensible.",
      "Predict the sign, dominant mode, limiting signal, and calculation that would reject the candidate before running it.",
      "Repeat the same bounded event. Compare all relevant states or outputs, MV effort, constraints, uncertainty, and failure behavior rather than one attractive trace.",
      "Explain the result from equations, synchronized signals, residuals, constraints, and uncertainty. A successful trace alone is not a stability or robustness proof.",
      "Start **truST Simulator**, open Control Studio, and confirm Simulation, SP, PV, MV, units, quality, limits, and actual cycle time before changing the setpoint.",
      "Record the response, retain the useful interval, and identify the simplest qualified model. If identification is not meaningful for this lesson, retain the configured model and explain why.",
      "Review an unapplied candidate against the configured controller. Do not apply a candidate merely because one metric improves.",
      "Export the saved JSON result for complete evidence and CSV for the selected signal table. Confirm both remain labelled Simulation.",
      "Recall the preceding industrial-loop solution and retain its plant, event, seed, duration, limits, and evidence definition.",
      "Configured parameters are simulation truth. A fitted model is identified simulation evidence and needs independent validation.",
      "Calculate or select the candidate explicitly, review assumptions and limits, and never silently apply gains to a physical loop.",
      "Recall the model and frequency-response evidence from the preceding lesson. Keep signal direction, units, limits, operating point, and evidence class explicit.",
      "Configured coefficients are simulation truth. Any fitted model is identified simulation evidence and must be validated on a separate input before use.",
      "This lesson designs or analyses the declared technique; it does not silently apply gains. Retain the previous and candidate values with their assumptions.",
      "Start in Manual with ExperimentInput = 0.0. Use a bounded step or sweep, 100 ms task, deterministic seed, and the model coefficients printed above.",
      "Capture the baseline and changed experiments separately. Abort on non-finite values, unexpected sign, output beyond ±100%, or departure from the declared local region.",
      "Use the worked calculation above to write down the expected direction or value",
      "with the declared precomputed design and retain the preceding result.",
      "The actuator, sensor, constraints, operating point, and fallback belong to the named equipment and shall not be generalized silently.",
      "relevant internal states, mode, limits, quality, and actual cycle time.",
      "Treat `simulation.toml` as configured simulation truth.",
      "calculation, assumptions, candidate\nvalues, and every rejection reason.",
      "Select the coefficient that dominates the worked calculation above, vary it by",
    ];

    for (const entry of manifest.filter((candidate) => candidate.id?.startsWith("control-"))) {
      assert.ok(entry.path, `${entry.id} must identify its project path`);
      const readme = fs.readFileSync(path.join(examplesRoot, entry.path!, "README.md"), "utf8");
      for (const boilerplate of generatedBoilerplate) {
        assert.ok(!readme.includes(boilerplate),
          `${entry.id} must replace generated boilerplate with a lesson-specific experiment: ${boilerplate}`);
      }
      assert.doesNotMatch(readme, /\. is supported without violating/,
        `${entry.id} must replace the malformed generated Tune sentence`);
      assert.doesNotMatch(readme, /\bto a [aeiou]/i,
        `${entry.id} must use a grammatically correct equipment introduction`);
      const questionSection = /## Questions\n\n([\s\S]*?)\n\n## Advanced exercise/.exec(readme)?.[1] ?? "";
      const questionNumbers = [...questionSection.matchAll(/^(\d+)\./gm)].map((match) => Number(match[1]));
      assert.deepStrictEqual(questionNumbers, questionNumbers.map((_, index) => index + 1),
        `${entry.id} must number its questions once in sequence`);
    }
  });

  test("ships the complete control curriculum in the specified order", () => {
    const extensionRoot = extensionRootForLessons();
    const sourceCatalog = JSON.parse(fs.readFileSync(
      path.resolve(extensionRoot, "..", "..", "examples", "catalog.json"),
      "utf8"
    )) as Array<{ id?: string }>;
    const packagedCatalog = JSON.parse(fs.readFileSync(
      path.resolve(extensionRoot, "media", "examples", "manifest.json"),
      "utf8"
    )) as Array<{ id?: string }>;
    const sourceIds = sourceCatalog
      .filter((entry) => entry.id?.startsWith("control-"))
      .map((entry) => entry.id);
    const packagedIds = packagedCatalog
      .filter((entry) => entry.id?.startsWith("control-"))
      .map((entry) => entry.id);

    assert.strictEqual(sourceIds.length, 84,
      "Specification 38 defines exactly 84 canonical lessons");
    assert.deepStrictEqual(packagedIds, sourceIds,
      "the shipped gallery must preserve the complete canonical curriculum order");
  });

  test("ships every curriculum lesson as an ordinary runnable simulation project", () => {
    const examplesRoot = path.resolve(extensionRootForLessons(), "media", "examples");
    const manifest = JSON.parse(fs.readFileSync(
      path.join(examplesRoot, "manifest.json"),
      "utf8"
    )) as Array<{ id?: string; path?: string; hardware?: string; tags?: string[] }>;
    const controlEntries = manifest.filter((entry) => entry.id?.startsWith("control-"));

    assert.strictEqual(controlEntries.length, 84);
    assert.deepStrictEqual(
      fs.readdirSync(examplesRoot).filter((name) => name.startsWith("control-")).sort(),
      [],
      "control lessons must not remain duplicated at the catalog root"
    );

    controlEntries.forEach((entry, index) => {
      assert.ok(entry.id && entry.path, "every lesson must identify its id and project path");
      assert.strictEqual(entry.hardware, "none");
      assert.ok(entry.tags?.includes("control"));
      const root = path.join(examplesRoot, entry.path!);
      for (const relative of [
        "trust-lsp.toml",
        "runtime.toml",
        "io.toml",
        "simulation.toml",
        "README.md",
        "lesson.json",
        path.join("src", "Main.st"),
        path.join(".vscode", "launch.json"),
        path.join(".vscode", "settings.json"),
      ]) {
        assert.ok(fs.existsSync(path.join(root, relative)), `${entry.id} must include ${relative}`);
      }

      const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8")) as {
        schemaVersion?: number;
        id?: string;
        sequence?: number;
        equipment?: string;
        objective?: string;
        principalTechnique?: string;
        workflowStages?: string[];
        proof?: { nativeTest?: string; nativeCommand?: string; projectPath?: string };
        safety?: { hardLimits?: unknown[]; abortPredicates?: string[]; simulationOnlyRestrictions?: string[] };
      };
      assert.strictEqual(lesson.schemaVersion, 4);
      assert.strictEqual(lesson.id, entry.id);
      assert.strictEqual(lesson.sequence, index + 1, `${entry.id} must match its catalog position`);
      assert.ok(lesson.equipment && lesson.objective && lesson.principalTechnique,
        `${entry.id} must define its equipment, objective, and principal technique`);
      assert.ok(lesson.proof?.nativeTest);
      assert.ok(lesson.proof?.nativeCommand?.includes(lesson.proof.nativeTest!));
      assert.strictEqual(lesson.proof?.projectPath, path.posix.join("examples", entry.path!));
      assert.ok(lesson.safety?.hardLimits?.length);
      assert.ok(lesson.safety?.abortPredicates?.length);
      assert.ok(lesson.safety?.simulationOnlyRestrictions?.length);
      assert.ok(lesson.workflowStages?.includes("Monitor"),
        `${entry.id} must begin its applicable Control Studio workflow in Monitor`);
      assert.ok((lesson.workflowStages?.length ?? 0) >= 2,
        `${entry.id} must declare the practical workflow stages it actually uses`);

      const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
      assert.match(readme,
        /simulation(?: evidence|[- ]only|[\s\S]{0,120}(?:not|rather than)[\s\S]{0,80}physical)/i,
        `${entry.id} must distinguish simulation from physical evidence`);
      const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
      assert.match(source, /\bPROGRAM\b/);

      const launch = JSON.parse(fs.readFileSync(
        path.join(root, ".vscode", "launch.json"),
        "utf8"
      )) as { configurations?: Array<{ program?: string }> };
      const configuredProgram = launch.configurations?.[0]?.program?.replace("${workspaceFolder}/", "");
      assert.ok(configuredProgram && fs.existsSync(path.join(root, configuredProgram)),
        `${entry.id} launch configuration must name its shipped Structured Text program`);
    });
  });

  test("audits all 84 learner readmes against the shared pedagogical contract", () => {
    const sourceRoot = path.resolve(extensionRootForLessons(), "..", "..", "examples");
    const catalog = JSON.parse(fs.readFileSync(path.join(sourceRoot, "catalog.json"), "utf8")) as
      Array<{ id?: string; path?: string; tags?: string[] }>;
    const lessons = catalog.filter((entry) => entry.id?.startsWith("control-") || entry.tags?.includes("control"));

    assert.strictEqual(lessons.length, 84, "Specification 38 defines exactly 84 canonical lessons");
    lessons.forEach((entry, index) => {
      assert.ok(entry.id && entry.path, "every curriculum entry must identify its project");
      const projectRoot = path.join(sourceRoot, entry.path!);
      const readme = fs.readFileSync(path.join(projectRoot, "README.md"), "utf8");
      const descriptor = JSON.parse(fs.readFileSync(path.join(projectRoot, "lesson.json"), "utf8")) as {
        objective?: string;
        equipment?: string;
      };
      const normalizedReadme = normalizePedagogicalText(readme);

      assert.ok(descriptor.objective && normalizedReadme.includes(normalizePedagogicalText(descriptor.objective)),
        `${entry.id} README objective must agree with its canonical lesson descriptor`);
      assert.ok(descriptor.equipment && normalizedReadme.includes(normalizePedagogicalText(descriptor.equipment)),
        `${entry.id} must introduce its declared real equipment`);
      assert.ok(readme.includes(`Course position: Lesson ${index + 1} of 84.`),
        `${entry.id} must expose its exact place in the learning progression`);

      const stages = {
        objective: firstMatch(readme, [/(?:^## Objective$|^\*\*Lesson objective:\*\*|^\*\*One clear objective:\*\*)/m]),
        problem: firstMatch(readme, [/^## The problem$/m]),
        need: firstMatch(readme, [/^## What we need$/m]),
        idea: firstMatch(readme, [/^## The new idea$/m]),
        prediction: firstMatch(readme, [/^## Prediction$/m]),
        implementation: firstMatch(readme, [/^## Implementation$/m]),
        experiment: firstMatch(readme, [/^## Experiment in Control Studio$/m]),
        evidence: firstMatch(readme, [/^## Evidence and decision$/m]),
        conclusion: firstMatch(readme, [/^## Conclusion$/m]),
        next: firstMatch(readme, [/^## Reason for the next lesson$/m]),
      };
      Object.entries(stages).forEach(([stage, position]) => assert.ok(position >= 0,
        `${entry.id} must teach the ${stage} stage`));
      assert.ok(stages.objective < stages.problem && stages.problem < stages.need &&
        stages.need < stages.idea && stages.idea < stages.prediction &&
        stages.prediction < stages.implementation && stages.implementation < stages.experiment &&
        stages.experiment < stages.evidence && stages.evidence < stages.conclusion &&
        stages.conclusion < stages.next,
      `${entry.id} must progress from problem and need through theory, prediction, implementation, experiment, evidence, conclusion, and the next problem`);

      assert.match(readme,
        /simulation(?: evidence|[- ]only|[\s\S]{0,120}(?:not|rather than)[\s\S]{0,80}physical)/i,
        `${entry.id} must distinguish Simulation evidence from physical-plant evidence`);
      const figures = [...readme.matchAll(/!\[([^\]]+)\]\(([^)]+\.svg)\)/gi)];
      assert.ok(figures.length > 0, `${entry.id} must include a useful teaching figure`);
      for (const figure of figures) {
        assert.ok(figure[1].trim().length >= 8, `${entry.id} figure must have useful alternative text`);
        assert.ok(fs.existsSync(path.resolve(projectRoot, figure[2])),
          `${entry.id} teaching figure ${figure[2]} must exist`);
      }
      const controllerSection = readme.slice(stages.implementation, stages.experiment);
      assert.match(controllerSection, /```(?:iecst|st)\b|^ {4}\S/m,
        `${entry.id} must show the real Structured Text controller under test`);
      assert.doesNotMatch(controllerSection, /pseudocode/i,
        `${entry.id} must not substitute pseudocode for its shipped controller`);
    });
  });

  test("publishes one complete theory and practice course map", () => {
    const sourceRoot = path.resolve(extensionRootForLessons(), "..", "..", "examples");
    const catalog = JSON.parse(fs.readFileSync(path.join(sourceRoot, "catalog.json"), "utf8")) as
      Array<{ id?: string; path?: string }>;
    const lessons = catalog.filter((entry) => entry.id?.startsWith("control-"));
    const courseReadme = fs.readFileSync(path.join(sourceRoot, "control", "README.md"), "utf8");

    assert.strictEqual(lessons.length, 84);
    assert.match(courseReadme,
      /\| Lesson \| Runnable project \| Real equipment \| Theory \| Practice in Control Studio \|/);
    const escapeCell = (value: string): string => value.replace(/\|/g, "\\|").replace(/\s+/g, " ").trim();
    for (const [index, entry] of lessons.entries()) {
      assert.ok(entry.path, `${entry.id} must have a project path`);
      const descriptor = JSON.parse(fs.readFileSync(
        path.join(sourceRoot, entry.path!, "lesson.json"), "utf8"
      )) as {
        sequence?: number;
        title?: string;
        equipment?: string;
        principalTechnique?: string;
        objective?: string;
        workflowStages?: string[];
      };
      const relativePath = `./${entry.path!.replace(/^control\//, "")}/`;
      const row = `| ${index + 1} | [${escapeCell(descriptor.title ?? "")}](${relativePath}) | ` +
        `${escapeCell(descriptor.equipment ?? "")} | ${escapeCell(descriptor.principalTechnique ?? "")} | ` +
        `${escapeCell((descriptor.workflowStages ?? []).join(" → "))} — ` +
        `${escapeCell(descriptor.objective ?? "")} |`;
      assert.strictEqual(descriptor.sequence, index + 1, `${entry.id} sequence must match the catalog`);
      assert.ok(courseReadme.includes(row), `${entry.id} must have its exact theory-and-practice course-map row`);
    }
  });

  test("numbers every canonical lesson directory in course order", () => {
    const sourceRoot = path.resolve(extensionRootForLessons(), "..", "..", "examples");
    const catalog = JSON.parse(fs.readFileSync(path.join(sourceRoot, "catalog.json"), "utf8")) as
      Array<{ id?: string; path?: string }>;
    const lessons = catalog.filter((entry) => entry.id?.startsWith("control-"));

    assert.strictEqual(lessons.length, 84);
    for (const [index, entry] of lessons.entries()) {
      assert.ok(entry.path, `${entry.id} must have a project path`);
      const expectedPrefix = `${String(index + 1).padStart(2, "0")}-`;
      assert.ok(path.posix.basename(entry.path!).startsWith(expectedPrefix),
        `${entry.id} directory must begin with ${expectedPrefix}`);
      assert.ok(fs.statSync(path.join(sourceRoot, entry.path!)).isDirectory(),
        `${entry.id} numbered project directory must exist`);
    }

    const catalogPaths = new Set(lessons.map((entry) => entry.path));
    const projectPaths = fs.readdirSync(path.join(sourceRoot, "control"), { withFileTypes: true })
      .filter((category) => category.isDirectory())
      .flatMap((category) => fs.readdirSync(path.join(sourceRoot, "control", category.name),
        { withFileTypes: true })
        .filter((project) => project.isDirectory() && fs.existsSync(path.join(
          sourceRoot, "control", category.name, project.name, "lesson.json")))
        .map((project) => path.posix.join("control", category.name, project.name)));
    assert.strictEqual(projectPaths.length, 84,
      "the canonical course must not retain a second set of unlisted lesson directories");
    assert.deepStrictEqual(new Set(projectPaths), catalogPaths,
      "every lesson directory must be one numbered catalog project");
  });

  test("groups lesson folders only into consecutive learner modules", () => {
    const sourceRoot = path.resolve(extensionRootForLessons(), "..", "..", "examples");
    const catalog = JSON.parse(fs.readFileSync(path.join(sourceRoot, "catalog.json"), "utf8")) as
      Array<{ id?: string; path?: string }>;
    const lessons = catalog.filter((entry) => entry.id?.startsWith("control-"));
    const modules = [
      [1, 18, "01-feedback-foundations-and-temperature-control"],
      [19, 25, "02-derivative-action-and-dynamic-response"],
      [26, 30, "03-production-quality-digital-pid"],
      [31, 36, "04-controller-selection-and-plant-models"],
      [37, 41, "05-experiments-identification-and-validation"],
      [42, 44, "06-pid-tuning-methods"],
      [45, 50, "07-classical-stability-and-frequency-design"],
      [51, 52, "08-tuning-qualification-and-automation"],
      [53, 60, "09-industrial-control-structures"],
      [61, 63, "10-multivariable-control"],
      [64, 68, "11-state-space-estimation-and-predictive-control"],
      [69, 72, "12-nonlinear-adaptive-and-robust-control"],
      [73, 75, "13-networked-fault-aware-and-diagnostic-control"],
      [76, 84, "14-integrated-capstones"],
    ] as const;

    for (const [index, entry] of lessons.entries()) {
      const lessonNumber = index + 1;
      const module = modules.find(([first, last]) => lessonNumber >= first && lessonNumber <= last);
      assert.ok(module, `Lesson ${lessonNumber} must belong to one learner module`);
      assert.strictEqual(path.posix.dirname(entry.path ?? ""), `control/${module![2]}`,
        `${entry.id} must live in its consecutive learner module`);
    }
    const actualModules = new Set(lessons.map((entry) => path.posix.dirname(entry.path ?? "")));
    assert.deepStrictEqual(actualModules,
      new Set(modules.map((module) => `control/${module[2]}`)),
      "the folder tree must contain exactly the 14 learner modules");
  });

  test("gives engineers technique, plant-problem, and workflow indexes", () => {
    const sourceRoot = path.resolve(extensionRootForLessons(), "..", "..", "examples");
    const readme = fs.readFileSync(path.join(sourceRoot, "control", "README.md"), "utf8");

    assert.match(readme, /^## Learner modules$/m);
    assert.match(readme, /\| Module \| Lessons \| Learning progression \|/);
    assert.match(readme, /^## For control engineers$/m);
    assert.match(readme, /\| Technique \| Use it when \| Lessons \|/);
    assert.match(readme, /\| Plant behavior or engineering problem \| Start with \|/);
    assert.match(readme,
      /\| Control Studio stage \| Engineering purpose \| Lessons that teach it explicitly \|/);
    for (const technique of [
      "Anti-windup", "Bode analysis", "Cascade control", "Gain scheduling",
      "Kalman filtering", "Model-predictive control", "Nyquist analysis",
      "Relative Gain Array", "Root locus", "Smith predictor",
    ]) {
      assert.ok(readme.includes(`| ${technique} |`),
        `${technique} must be directly discoverable`);
    }
  });

  test("keeps offline and live plant projects separate with ordinary process-image symbols", () => {
    const offlineRoot = fixturePath("offline-project");
    const liveRoot = fixturePath("live-simulation-project");
    const offlineSource = fs.readFileSync(path.join(offlineRoot, "src", "main.st"), "utf8");
    const liveSource = fs.readFileSync(path.join(liveRoot, "src", "main.st"), "utf8");

    for (const source of [offlineSource, liveSource]) {
      assert.match(source, /ManipulatedValue AT %QL0 : LREAL/);
      assert.match(source, /ProcessValue AT %IL0 : LREAL/);
      assert.match(source, /OptionalFeedback AT %IL8 : LREAL/);
    }
    assert.match(liveSource, /Controller\s*:\s*Control\.PID/,
      "the live fixture must expose the real controller used by Control Studio journeys");
    assert.match(liveSource, /Setpoint AT %ML0 : LREAL/);
    assert.match(liveSource, /AutoMode AT %MX8\.0 : BOOL/,
      "the mode bit must not overlap the eight-byte LREAL setpoint at %ML0");
    assert.ok(!fs.existsSync(path.join(offlineRoot, "runtime.toml")));
    assert.ok(!fs.existsSync(path.join(offlineRoot, "simulation.toml")));
    assert.ok(fs.existsSync(path.join(liveRoot, "runtime.toml")));
    assert.ok(fs.existsSync(path.join(liveRoot, "simulation.toml")));
    const liveIo = path.join(liveRoot, "io.toml");
    assert.ok(fs.existsSync(liveIo), "the live fixture must include ordinary simulated I/O configuration");
    assert.match(fs.readFileSync(liveIo, "utf8"), /driver = "simulated"/);
    assert.match(fs.readFileSync(path.join(liveRoot, "runtime.toml"), "utf8"), /\[resource\]\s+name = "Res"/,
      "the live runtime resource must match the compiled configuration");
  });

  test("runs both qualified formula methods across the complete lesson catalog and retains failures", () => {
    const examplesRoot = path.resolve(extensionRootForLessons(), "media", "examples");
    const manifest = JSON.parse(fs.readFileSync(path.join(examplesRoot, "manifest.json"), "utf8")) as
      Array<{ id: string; path: string }>;
    const lessons = manifest.filter((entry) => entry.id.startsWith("control-")).sort((left, right) =>
      left.id.localeCompare(right.id));
    assert.strictEqual(lessons.length, 84);
    let feasible = 0;
    let unavailable = 0;
    for (const lesson of lessons) {
      const lessonId = lesson.id;
      const source = fs.readFileSync(path.join(examplesRoot, lesson.path, "simulation.toml"), "utf8");
      const configuredModel = captureText(source, /^model\s*=\s*"([^"]+)"/m) ?? "state_space";
      const kind = ({ integrator: "integrating", first_order: "first_order",
        first_order_dead_time: "first_order_dead_time", second_order: "second_order" } as
        Partial<Record<string, ModelKind>>)[configuredModel] ?? "second_order";
      const model: IdentifiedModel = {
        kind,
        gain: captureNumber(source, /^gain\s*=\s*([-+0-9.eE]+)/m) ?? 1,
        ...(kind === "integrating" ? {} : {
          timeConstantSeconds: captureNumber(source, /^time_constant_s\s*=\s*([-+0-9.eE]+)/m) ?? 1,
        }),
        ...(kind === "second_order" ? { secondTimeConstantSeconds: 1 } : {}),
        deadTimeSeconds: captureNumber(source, /^dead_time_s\s*=\s*([-+0-9.eE]+)/m) ?? 0,
        normalizedError: 0,
        stable: lessonId !== "control-open-loop-unstable",
      };
      const comparison = compareQualifiedTunings(model, "balanced");
      assert.strictEqual(comparison.applied, false);
      assert.deepStrictEqual(comparison.model, model);
      assert.deepStrictEqual(comparison.attempts.map((attempt) => attempt.method), ["SIMC", "AMIGO"]);
      feasible += comparison.attempts.filter((attempt) => attempt.status === "Feasible").length;
      unavailable += comparison.attempts.filter((attempt) => attempt.status === "Unavailable").length;
      assert.ok(comparison.attempts.every((attempt) => attempt.status === "Feasible" || Boolean(attempt.reason)),
        `${lessonId} must retain every unavailable-method reason`);
    }
    assert.ok(feasible > 0, "the FOPDT lessons must exercise qualified methods");
    assert.ok(unavailable > 0, "unsupported lesson domains must remain explicit campaign results");
    assert.strictEqual(feasible + unavailable, lessons.length * 2);
  });

  test("pins deterministic simulation, recorded-data, and offline-model values", () => {
    const simulation = fs.readFileSync(
      fixturePath("live-simulation-project", "simulation.toml"),
      "utf8"
    );
    assert.match(simulation, /seed = 20260824/);
    assert.match(simulation, /^time_scale = 1$/m,
      "the live fixture must use the runtime's integer simulation-time scale contract");
    assert.match(simulation, /id = "fixture-fopdt"/);
    assert.match(simulation, /gain = 1\.5/);
    assert.match(simulation, /time_constant_s = 4\.0/);
    assert.match(simulation, /dead_time_s = 0\.5/);

    const rows = fs
      .readFileSync(fixturePath("recorded-data", "fopdt-step.csv"), "utf8")
      .trim()
      .split(/\r?\n/);
    assert.strictEqual(rows[0], "time_s,setpoint,process_value,manipulated_value,quality");
    assert.strictEqual(rows.length, 11);

    const model = JSON.parse(
      fs.readFileSync(fixturePath("offline-model", "fopdt.json"), "utf8")
    );
    assert.strictEqual(model.evidenceClass, "offline_model");
    assert.deepStrictEqual(model.parameters, {
      gain: 1.5,
      timeConstantSeconds: 4.0,
      deadTimeSeconds: 0.5,
    });
  });

  test("authorization fixtures can never constitute physical-plant evidence", () => {
    const simulation = JSON.parse(
      fs.readFileSync(fixturePath("authorization", "simulation.json"), "utf8")
    );
    const physicalBoundary = JSON.parse(
      fs.readFileSync(
        fixturePath("authorization", "fake-physical-boundary.json"),
        "utf8"
      )
    );

    assert.strictEqual(simulation.source, "simulation");
    assert.strictEqual(simulation.permitsPhysicalAction, false);
    assert.strictEqual(physicalBoundary.evidenceClass, "automated_fake_boundary_only");
    assert.strictEqual(physicalBoundary.requiresImmediateApproval, true);
    assert.strictEqual(physicalBoundary.mayRunExperiment, false);
    assert.strictEqual(physicalBoundary.mayApplyController, false);
    assert.strictEqual(physicalBoundary.permitsPhysicalAction, false);
  });
});

function recursiveFilePaths(root: string, relative = ""): string[] {
  const entries = fs.readdirSync(path.join(root, relative), { withFileTypes: true });
  return entries.flatMap((entry) => {
    const child = path.join(relative, entry.name);
    return entry.isDirectory() ? recursiveFilePaths(root, child) : [child];
  }).sort();
}

function extensionRootForLessons(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function captureText(source: string, pattern: RegExp): string | undefined {
  return pattern.exec(source)?.[1];
}

function captureNumber(source: string, pattern: RegExp): number | undefined {
  const value = captureText(source, pattern);
  return value === undefined ? undefined : Number(value);
}

function normalizePedagogicalText(value: string): string {
  return value.toLowerCase().replace(/[`*_]/g, "").replace(/\s+/g, " ").trim().replace(/[.!?]+$/, "");
}

function firstMatch(source: string, patterns: RegExp[]): number {
  for (const pattern of patterns) {
    const position = source.search(pattern);
    if (position >= 0) return position;
  }
  return -1;
}
