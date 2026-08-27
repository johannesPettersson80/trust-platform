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
  test("ships the sixteen benchmark lessons as ordinary runnable simulation projects", () => {
    const lessons = [
      ["control-nominal-fopdt", "Foundation", "paper_pi_meets_nominal_robustness_and_sampling_gates"],
      ["control-sampling-interval", "Foundation", "paper_pi_meets_nominal_robustness_and_sampling_gates"],
      ["control-load-disturbance", "Foundation", "paper_pi_rejects_a_plant_side_load_disturbance"],
      ["control-saturation-antiwindup", "Foundation", "paper_pid_recovers_from_saturation_and_respects_rate_limits"],
      ["control-measurement-noise", "Foundation", "separate_low_pass_reduces_seeded_measurement_noise_activity"],
      ["control-rate-limit", "Foundation", "paper_pid_recovers_from_saturation_and_respects_rate_limits"],
      ["control-integrating-plant", "Practitioner", "pid_benchmark_tracks_an_integrating_process"],
      ["control-underdamped-pid", "Practitioner", "pid_benchmark_damps_an_underdamped_second_order_process"],
      ["control-delay-dominant", "Practitioner", "pid_benchmark_tracks_a_delay_dominant_process"],
      ["control-nonlinear-actuator", "Practitioner", "pid_benchmark_handles_nonlinear_actuator_dynamics"],
      ["control-bounded-variation", "Practitioner", "paper_pi_meets_nominal_robustness_and_sampling_gates"],
      ["control-open-loop-unstable", "Advanced", "pid_advanced_benchmark_stabilizes_an_open_loop_unstable_plant"],
      ["control-scheduled-gain", "Advanced", "pid_advanced_benchmark_recovers_from_a_scheduled_gain_change"],
      ["control-severe-jitter", "Advanced", "pid_advanced_benchmark_uses_actual_cycle_time_under_severe_jitter"],
      ["control-coupled-two-loop", "Advanced", "pid_advanced_benchmark_controls_a_coupled_two_loop_plant"],
      ["control-evidence-integrity", "Advanced", "simulation_process_image_ranges_detect_lreal_overlap"],
    ] as const;
    const examplesRoot = path.resolve(extensionRootForLessons(), "media", "examples");
    const learningPaths = fs.readFileSync(path.resolve(
      extensionRootForLessons(), "..", "..", "docs", "public", "examples", "learning-paths.md"
    ), "utf8");
    const manifest = JSON.parse(fs.readFileSync(path.join(examplesRoot, "manifest.json"), "utf8")) as
      Array<{ id?: string; path?: string; hardware?: string; tags?: string[] }>;
    for (const [id, level, nativeTest] of lessons) {
      const entry = manifest.find((candidate) => candidate.id === id);
      assert.ok(entry, `example catalog must include lesson '${id}'`);
      assert.match(learningPaths, new RegExp(`\\b${id}\\b`),
        `public learning paths must include lesson '${id}'`);
      assert.strictEqual(entry?.hardware, "none");
      assert.ok(entry?.tags?.includes("control"));
      const root = path.join(examplesRoot, entry?.path ?? id);
      for (const relative of ["trust-lsp.toml", "runtime.toml", "io.toml", "simulation.toml",
        "README.md", "lesson.json", path.join("src", "Main.st"),
        path.join(".vscode", "launch.json"), path.join(".vscode", "settings.json")]) {
        assert.ok(fs.existsSync(path.join(root, relative)), `${id} must include ${relative}`);
      }
      const lesson = JSON.parse(fs.readFileSync(path.join(root, "lesson.json"), "utf8")) as
        { id?: string; level?: string; nativeTest?: string; evidenceSource?: string; stages?: string[] };
      assert.strictEqual(lesson.id, id);
      assert.strictEqual(lesson.level, level);
      assert.strictEqual(lesson.nativeTest, nativeTest);
      assert.strictEqual(lesson.evidenceSource, "simulation");
      assert.deepStrictEqual(lesson.stages,
        ["Predict", "Configure", "Monitor", "Test", "Identify", "Tune", "Validate", "Explain", "Export"]);
      const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
      assert.match(readme, /simulation evidence/i);
      assert.match(readme, /SP means setpoint.*PV means process value.*MV means manipulated variable.*FOPDT means first-order plus dead time/is,
        `${id} must expand the control terms used by the workflow`);
      if (id === "control-nominal-fopdt") {
        assert.match(readme, /Lambda.*desired closed-loop time constant/i,
          "the foundation lesson must define its robustness-speed control");
      }
      if (id === "control-bounded-variation") {
        for (const values of ["0.8 | 0.8 s | 0.4 s", "1.2 | 1.2 s | 0.6 s",
          "1.2 | 0.8 s | 0.6 s", "0.8 | 1.2 s | 0.4 s"]) {
          assert.ok(readme.includes(values), `bounded-variation lesson must list ${values}`);
        }
      }
      if (id === "control-scheduled-gain") {
        assert.match(readme, /(?:45 s run|run for 45 s)/i);
        assert.match(readme, /at 15 s/i);
      }
      for (const heading of ["Objective", "Predict", "Configure", "Monitor", "Test", "Identify",
        "Tune", "Validate", "Explain", "Export", "Questions", "Advanced exercise"]) {
        assert.match(readme, new RegExp(`^## ${heading}$`, "m"), `${id} must explain ${heading}`);
      }
      const source = fs.readFileSync(path.join(root, "src", "Main.st"), "utf8");
      assert.match(source, /Controller\s*:\s*Control\.PID/);
      if (id === "control-coupled-two-loop") {
        assert.match(source, /Setpoint1\s*:\s*LREAL\s*:=\s*0\.0/);
        assert.match(source, /Setpoint2\s*:\s*LREAL\s*:=\s*0\.0/);
        assert.match(source, /IF AutoMode1 THEN\s+Input1\.Mode := PIDMode#Auto;/);
        assert.match(source, /IF AutoMode2 THEN\s+Input2\.Mode := PIDMode#Auto;/);
      } else {
        assert.match(source, /Setpoint\s*:\s*LREAL\s*:=\s*0\.0/,
          `${id} must wait for the learner's bounded step instead of stepping at startup`);
        assert.match(source, /IF AutoMode THEN\s+Input\.Mode := PIDMode#Auto;/,
          `${id} must use its exposed controller-mode input`);
        assert.doesNotMatch(source, /Input\.Mode := PIDMode#Auto;\s*Input\.CycleTime/,
          `${id} must not ignore AutoMode`);
      }
      if (id !== "control-evidence-integrity") {
        assert.doesNotMatch(source, /OptionalFeedback/,
          `${id} must not expose an unused feedback signal`);
      }
      assert.match(source, /ManipulatedValue AT %QL0 : LREAL/);
      assert.match(source, /ProcessValue AT %IL0 : LREAL/);
      const launch = JSON.parse(fs.readFileSync(path.join(root, ".vscode", "launch.json"), "utf8")) as
        { configurations?: Array<{ program?: string }> };
      const configuredProgram = launch.configurations?.[0]?.program?.replace("${workspaceFolder}/", "");
      assert.ok(configuredProgram && fs.existsSync(path.join(root, configuredProgram)),
        `${id} launch configuration must name its shipped Structured Text program`);
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
    const lessonIds = fs.readdirSync(examplesRoot).filter((name) => name.startsWith("control-")).sort();
    assert.strictEqual(lessonIds.length, 16);
    let feasible = 0;
    let unavailable = 0;
    for (const lessonId of lessonIds) {
      const source = fs.readFileSync(path.join(examplesRoot, lessonId, "simulation.toml"), "utf8");
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
    assert.strictEqual(feasible + unavailable, 32);
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
