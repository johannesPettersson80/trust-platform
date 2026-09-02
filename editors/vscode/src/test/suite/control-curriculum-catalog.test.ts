import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

suite("Control curriculum catalog", () => {
  test("publishes the canonical 84-lesson path with prerequisites, equipment, and outcomes", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const lessons = catalog.filter((entry) => entry.id.startsWith("control-") || entry.tags?.includes("control"));
    const publicIndex = fs.readFileSync(path.join(repositoryRoot, "docs", "public", "examples", "index.md"), "utf8");
    const learningPaths = fs.readFileSync(
      path.join(repositoryRoot, "docs", "public", "examples", "learning-paths.md"), "utf8"
    );
    const rows = [...learningPaths.matchAll(/^(\d+)\. \[`([^`]+)`\]\(([^)]+)\)$/gm)];

    assert.match(publicIndex, /through 84 runnable real-world lessons/);
    assert.strictEqual(rows.length, 84, "the public path must contain one row for every canonical lesson");
    assert.deepStrictEqual(rows.map((row) => Number(row[1])), lessons.map((_, index) => index + 1));
    assert.deepStrictEqual(rows.map((row) => row[2]), lessons.map((lesson) => lesson.id));
    lessons.forEach((entry, index) => {
      const descriptor = JSON.parse(fs.readFileSync(
        path.join(repositoryRoot, "examples", entry.path, "lesson.json"), "utf8"
      )) as { prerequisites?: string[]; equipment?: string; objective?: string };
      assert.strictEqual(rows[index][3], `../../../examples/${entry.path}/README.md`,
        `${entry.id} must link to its canonical learner-facing project`);
      const linkedReadme = fs.readFileSync(path.resolve(
        path.join(repositoryRoot, "docs", "public", "examples"), rows[index][3]
      ), "utf8").replace(/\s+/g, " ");
      assert.ok(descriptor.equipment && linkedReadme.toLowerCase().includes(descriptor.equipment.toLowerCase()),
        `${entry.id} linked page must expose its real equipment`);
      assert.ok(descriptor.objective && linkedReadme.toLowerCase().includes(descriptor.objective.toLowerCase()),
        `${entry.id} linked page must expose its outcome`);
      assert.match(linkedReadme, new RegExp(`Course position: Lesson ${index + 1} of 84\\.`));
      if (descriptor.prerequisites?.[0]) {
        assert.match(linkedReadme, /(?:Previous lesson|Prerequisite):/i,
          `${entry.id} linked page must expose its prerequisite in learner-facing words`);
      }
    });
  });

  test("tracks every canonical lesson on the active Specification 38 checklist", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; tags?: string[] }>;
    const expectedIds = catalog
      .filter((entry) => entry.id.startsWith("control-") || entry.tags?.includes("control"))
      .map((entry) => entry.id);
    const checklist = fs.readFileSync(path.join(
      repositoryRoot,
      "docs",
      "internal",
      "testing",
      "checklists",
      "control-engineering-curriculum-implementation-checklist.md"
    ), "utf8");
    const rows = [...checklist.matchAll(/^\| (\d{2}) \| `([^`]+)` \|/gm)];

    assert.strictEqual(rows.length, 84,
      "the active checklist must contain one evidence row for every lesson");
    assert.deepStrictEqual(rows.map((row) => row[2]), expectedIds,
      "checklist lesson identities and order must match the canonical catalog");
    assert.deepStrictEqual(rows.map((row) => Number(row[1])),
      expectedIds.map((_, index) => index + 1),
      "checklist lesson numbers must be contiguous from 1 through 84");
    assert.match(checklist, /water-heater proportional-control lesson.*mandatory pedagogical reference/is);
    for (const heading of [
      "Specification and teaching contract",
      "Per-lesson implementation and proof board",
      "Rendered learner acceptance",
      "Frozen final gates",
      "Completion boundary",
    ]) {
      assert.match(checklist, new RegExp(`^## ${heading}$`, "m"));
    }
  });

  test("publishes the complete specified 84-lesson progression", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const expectedIds = [
      "control-cold-storage-loop-anatomy",
      "control-cold-storage-on-off",
      "control-cold-storage-hysteresis-dwell",
      "control-water-heater-p",
      "control-water-heater-bias",
      "control-water-heater-integral",
      "control-water-heater-pi",
      "control-water-heater-saturation",
      "control-rate-limit",
      "control-saturation-antiwindup",
      "control-reactor-servo-regulatory",
      "control-reactor-setpoint-weighting",
      "control-measurement-noise",
      "control-sampling-interval",
      "control-water-heater-energy-balance",
      "control-heater-step-identification",
      "control-independent-heater-model-validation",
      "control-nominal-fopdt",
      "control-servo-position-p",
      "control-servo-position-pd",
      "control-loaded-position-pid",
      "control-servo-derivative-kick",
      "control-servo-filtered-derivative",
      "control-servo-mechanical-nonidealities",
      "control-servo-digital-realization",
      "control-pid-forms",
      "control-booster-pump-modes",
      "control-booster-pump-bumpless-transfer",
      "control-booster-pump-atomic-update",
      "control-booster-pump-invalid-data",
      "control-controller-selection",
      "control-plant-response-classes",
      "control-tank-heater-conservation",
      "control-gravity-tank-linearization",
      "control-pole-zero-plant-classes",
      "control-two-tank-state-space",
      "control-safe-heat-exchanger-experiment",
      "control-excitation-methods",
      "control-dryer-model-selection",
      "control-poor-excitation",
      "control-independent-model-validation",
      "control-manual-historical-tuning",
      "control-relay-autotuning",
      "control-model-based-tuning",
      "control-crane-routh-stability",
      "control-servo-root-locus",
      "control-flexible-drive-bode",
      "control-delayed-thermal-nyquist",
      "control-motor-sensitivity-loop-shaping",
      "control-position-z-plane-sampling",
      "control-tuning-method-comparison",
      "control-automatic-tuning-sequence",
      "control-conveyor-oven-feedforward",
      "control-reactor-cascade",
      "control-blending-ratio",
      "control-reactor-split-range",
      "control-compressor-override",
      "control-gravity-tank-scheduling",
      "control-heated-air-duct-smith",
      "control-valve-stiction",
      "control-distillation-mimo",
      "control-distillation-decoupling",
      "control-distillation-robustness",
      "control-two-tank-observability",
      "control-magnetic-bearing-state-feedback",
      "control-weigh-feeder-kalman",
      "control-suspension-lqr",
      "control-building-hvac-mpc",
      "control-pendulum-lyapunov",
      "control-robot-joint-nonlinear",
      "control-extruder-adaptive",
      "control-flexible-airframe-robust",
      "control-networked-pumping",
      "control-pumping-fault-aware",
      "control-loop-diagnosis",
      "control-capstone-cold-storage",
      "control-capstone-water-heater",
      "control-capstone-servo",
      "control-capstone-reactor",
      "control-capstone-gravity-tank",
      "control-capstone-conveyor-oven",
      "control-capstone-distillation",
      "control-capstone-building-hvac",
      "control-capstone-independent",
    ];
    const controlEntries = catalog.filter((entry) =>
      entry.id.startsWith("control-") || entry.tags?.includes("control")
    );

    assert.deepStrictEqual(
      controlEntries.map((entry) => entry.id),
      expectedIds,
      "the gallery catalog must follow Specification 38 exactly"
    );

    controlEntries.forEach((entry, index) => {
      const lesson = JSON.parse(
        fs.readFileSync(path.join(repositoryRoot, "examples", entry.path, "lesson.json"), "utf8")
      ) as {
        id?: string;
        level?: string;
        sequence?: number;
        prerequisites?: string[];
        nextLesson?: string | null;
      };
      assert.strictEqual(lesson.id, entry.id);
      assert.strictEqual(lesson.sequence, index + 1, `${entry.id} has the wrong sequence`);
      const expectedLevel = index < 9
        ? "Foundation"
        : index < 23
          ? "Modelling"
          : index < 40
            ? "Practitioner"
            : index < 60
              ? "Advanced"
              : "Modern";
      assert.strictEqual(
        lesson.level,
        expectedLevel,
        `${entry.id} is not reachable from its sequence-defined course`,
      );
      assert.deepStrictEqual(
        lesson.prerequisites,
        index === 0 ? [] : [expectedIds[index - 1]],
        `${entry.id} has the wrong prerequisite`
      );
      assert.strictEqual(
        lesson.nextLesson,
        index + 1 === expectedIds.length ? null : expectedIds[index + 1],
        `${entry.id} has the wrong next lesson`
      );
    });
  });

  test("replaces the superseded load-disturbance slot with Lesson 26", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const digitalIndex = catalog.findIndex((entry) => entry.id === "control-servo-digital-realization");
    const formsIndex = catalog.findIndex((entry) => entry.id === "control-pid-forms");

    assert.strictEqual(formsIndex, digitalIndex + 1,
      "Lesson 26 must immediately follow the digital-realization project");
    assert.ok(!catalog.some((entry) => entry.id === "control-load-disturbance"),
      "the superseded sequence-19 lesson must not remain in the canonical curriculum");
    assert.strictEqual(
      catalog.filter((entry) => entry.id.startsWith("control-")
        || entry.tags?.includes("control")).length,
      84,
      "the catalog must retain exactly the 84 specified curriculum identities"
    );
  });

  test("publishes Lesson 27 without assigning its slot to a modelling lesson", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string }>;
    const formsIndex = catalog.findIndex((entry) => entry.id === "control-pid-forms");
    const modesIndex = catalog.findIndex((entry) => entry.id === "control-booster-pump-modes");

    assert.strictEqual(modesIndex, formsIndex + 1,
      "Lesson 27 must immediately follow Lesson 26");
    assert.notStrictEqual(catalog[modesIndex + 1]?.id, "control-plant-response-classes",
      "Lesson 32 must not occupy the implemented Lesson 28 slot after Lesson 27");
  });

  test("publishes Lesson 28 without assigning its slot to a conservation lesson", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string }>;
    const modesIndex = catalog.findIndex((entry) => entry.id === "control-booster-pump-modes");
    const transferIndex = catalog.findIndex(
      (entry) => entry.id === "control-booster-pump-bumpless-transfer"
    );

    assert.strictEqual(transferIndex, modesIndex + 1,
      "Lesson 28 must immediately follow Lesson 27");
    assert.ok(!catalog.some((entry) => entry.id === "control-tank-mass-balance"),
      "the future conservation lesson must not occupy the implemented Lesson 28 slot");
  });

  test("keeps Lesson 29 before the later dryer model-selection lesson", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string }>;
    const transferIndex = catalog.findIndex(
      (entry) => entry.id === "control-booster-pump-bumpless-transfer"
    );
    const atomicIndex = catalog.findIndex(
      (entry) => entry.id === "control-booster-pump-atomic-update"
    );
    const modelSelectionIndex = catalog.findIndex(
      (entry) => entry.id === "control-dryer-model-selection"
    );

    assert.strictEqual(atomicIndex, transferIndex + 1,
      "Lesson 29 must immediately follow Lesson 28");
    assert.ok(modelSelectionIndex > atomicIndex,
      "dryer model selection must remain later than the Lesson 29 atomic-update lesson");
  });

  test("publishes Lesson 30 without assigning its slot to an excitation lesson", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string }>;
    const atomicIndex = catalog.findIndex(
      (entry) => entry.id === "control-booster-pump-atomic-update"
    );
    const invalidIndex = catalog.findIndex(
      (entry) => entry.id === "control-booster-pump-invalid-data"
    );

    assert.strictEqual(invalidIndex, atomicIndex + 1,
      "Lesson 30 must immediately follow Lesson 29");
    const poorExcitationIndex = catalog.findIndex(
      (entry) => entry.id === "control-poor-excitation"
    );
    assert.ok(poorExcitationIndex > invalidIndex,
      "Lesson 40 poor excitation must remain later than Lesson 30");
  });

  test("publishes Lesson 31 before the plant-modelling progression", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string }>;
    const invalidIndex = catalog.findIndex(
      (entry) => entry.id === "control-booster-pump-invalid-data"
    );
    const selectionIndex = catalog.findIndex(
      (entry) => entry.id === "control-controller-selection"
    );

    assert.strictEqual(selectionIndex, invalidIndex + 1,
      "Lesson 31 must immediately follow Lesson 30");
    assert.ok(!catalog.some((entry) => entry.id === "control-local-linear-models"),
      "the future local-linear-model lesson must not occupy Lesson 31's slot");
  });

  test("publishes the physical plant-response workshop as Lesson 32", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string }>;
    const selectionIndex = catalog.findIndex(
      (entry) => entry.id === "control-controller-selection"
    );
    const classesIndex = catalog.findIndex(
      (entry) => entry.id === "control-plant-response-classes"
    );

    assert.strictEqual(classesIndex, selectionIndex + 1,
      "Lesson 32 must immediately follow the selection workshop");
    assert.ok(!catalog.some((entry) => entry.id === "control-frequency-response"),
      "the future frequency-response lesson must not occupy Lesson 32's slot");
  });

  test("publishes the conservation-law workshop as Lesson 33", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const classesIndex = catalog.findIndex(
      (entry) => entry.id === "control-plant-response-classes"
    );
    const conservationIndex = catalog.findIndex(
      (entry) => entry.id === "control-tank-heater-conservation"
    );

    assert.strictEqual(conservationIndex, classesIndex + 1,
      "Lesson 33 must immediately follow the plant-response workshop");
    assert.strictEqual(
      catalog[conservationIndex]?.path,
      "control/04-controller-selection-and-plant-models/33-tank-heater-conservation"
    );
    assert.ok(!catalog.some((entry) => entry.id === "control-motor-sensitivity"),
      "the future sensitivity lesson must not occupy Lesson 33's slot");
    assert.strictEqual(
      catalog.filter((entry) => entry.id.startsWith("control-")
        || entry.tags?.includes("control")).length,
      84,
      "the catalog must retain exactly the 84 specified curriculum identities"
    );
  });

  test("publishes gravity-tank linearization as Lesson 34", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const conservationIndex = catalog.findIndex(
      (entry) => entry.id === "control-tank-heater-conservation"
    );
    const linearizationIndex = catalog.findIndex(
      (entry) => entry.id === "control-gravity-tank-linearization"
    );

    assert.strictEqual(linearizationIndex, conservationIndex + 1);
    assert.strictEqual(catalog[linearizationIndex]?.path,
      "control/04-controller-selection-and-plant-models/34-gravity-tank-linearization");
    const craneIndex = catalog.findIndex(
      (entry) => entry.id === "control-crane-routh-stability"
    );
    assert.ok(craneIndex > linearizationIndex,
      "Lesson 45 crane stability must remain later than Lesson 34");
    assert.strictEqual(
      catalog.filter((entry) => entry.id.startsWith("control-")
        || entry.tags?.includes("control")).length,
      84
    );
  });

  test("publishes the six-plant pole-zero workshop as Lesson 35", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const linearizationIndex = catalog.findIndex(
      (entry) => entry.id === "control-gravity-tank-linearization"
    );
    const poleZeroIndex = catalog.findIndex(
      (entry) => entry.id === "control-pole-zero-plant-classes"
    );

    assert.strictEqual(poleZeroIndex, linearizationIndex + 1);
    assert.strictEqual(catalog[poleZeroIndex]?.path,
      "control/04-controller-selection-and-plant-models/35-pole-zero-plant-classes");
    const rootLocusIndex = catalog.findIndex(
      (entry) => entry.id === "control-servo-root-locus"
    );
    assert.ok(rootLocusIndex > poleZeroIndex,
      "Lesson 46 root locus must remain later than Lesson 35");
    assert.strictEqual(
      catalog.filter((entry) => entry.id.startsWith("control-")
        || entry.tags?.includes("control")).length,
      84
    );
  });

  test("publishes the interconnected-tank state-space workshop as Lesson 36", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const catalog = JSON.parse(
      fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
    ) as Array<{ id: string; path: string; tags?: string[] }>;
    const poleZeroIndex = catalog.findIndex(
      (entry) => entry.id === "control-pole-zero-plant-classes"
    );
    const stateSpaceIndex = catalog.findIndex(
      (entry) => entry.id === "control-two-tank-state-space"
    );

    assert.strictEqual(stateSpaceIndex, poleZeroIndex + 1);
    assert.strictEqual(catalog[stateSpaceIndex]?.path,
      "control/04-controller-selection-and-plant-models/36-two-tank-state-space");
    assert.ok(!catalog.some((entry) => entry.id === "control-flexible-drive-nyquist"),
      "the future Nyquist lesson must not occupy Lesson 36's slot");
    assert.strictEqual(
      catalog.filter((entry) => entry.id.startsWith("control-")
        || entry.tags?.includes("control")).length,
      84
    );
  });
});
