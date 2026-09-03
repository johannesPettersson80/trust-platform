import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  controlLessonProgressLabel,
  controlLessonProgressState,
  controlModuleId,
  controlModuleTitle,
  controlModulesFromEntries,
  type ExampleEntry,
} from "../../examples/model";

function lessonEntry(id: string, entryPath: string, sequence: number): ExampleEntry {
  return {
    id,
    title: id,
    description: id,
    path: entryPath,
    hardware: "none",
    tags: ["control"],
    lesson: {
      sequence,
      level: "Foundation",
      equipment: "Test equipment",
      principalTechnique: "Test technique",
      prerequisites: [],
    },
  };
}

function starterEntry(id: string): ExampleEntry {
  return { id, title: id, description: id, path: id, hardware: "none" };
}

suite("Control gallery model", () => {
  suite("module grouping (W-04)", () => {
    test("derives a module id from a lesson's catalog path", () => {
      assert.strictEqual(
        controlModuleId(
          "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy"
        ),
        "01-feedback-foundations-and-temperature-control"
      );
      assert.strictEqual(
        controlModuleId("control/14-integrated-capstones/84-independent"),
        "14-integrated-capstones"
      );
    });

    test("returns undefined for a non-curriculum example path", () => {
      assert.strictEqual(controlModuleId("empty-simulator"), undefined);
      assert.strictEqual(controlModuleId("plcopen-motion-single-axis"), undefined);
    });

    test("humanizes a module directory basename without inventing new words", () => {
      assert.strictEqual(
        controlModuleTitle("01-feedback-foundations-and-temperature-control"),
        "Feedback foundations and temperature control"
      );
      assert.strictEqual(
        controlModuleTitle("03-production-quality-digital-pid"),
        "Production quality digital pid"
      );
      assert.strictEqual(controlModuleTitle("14-integrated-capstones"), "Integrated capstones");
    });

    test("orders modules by directory and reports each module's real lesson range", () => {
      const entries = [
        lessonEntry(
          "control-servo-position-p",
          "control/02-derivative-action-and-dynamic-response/19-servo-position-p",
          19
        ),
        lessonEntry(
          "control-cold-storage-loop-anatomy",
          "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy",
          1
        ),
        lessonEntry(
          "control-cold-storage-on-off",
          "control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off",
          2
        ),
      ];

      const modules = controlModulesFromEntries(entries);

      assert.deepStrictEqual(modules.map((module) => module.id), [
        "01-feedback-foundations-and-temperature-control",
        "02-derivative-action-and-dynamic-response",
      ]);
      assert.deepStrictEqual(modules.map((module) => module.title), [
        "Feedback foundations and temperature control",
        "Derivative action and dynamic response",
      ]);
      assert.strictEqual(modules[0].range, "Lessons 1–2");
      assert.strictEqual(modules[0].lessonCount, 2);
      assert.strictEqual(modules[1].range, "Lesson 19");
      assert.strictEqual(modules[1].lessonCount, 1);
    });

    test("ignores starter examples that are not curriculum lessons", () => {
      const modules = controlModulesFromEntries([
        starterEntry("empty-simulator"),
        starterEntry("conveyor"),
      ]);
      assert.deepStrictEqual(modules, []);
    });

    test("groups the full 84-lesson catalog into exactly fourteen modules", () => {
      const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const catalog = JSON.parse(
        fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
      ) as Array<{ id: string; path: string; tags?: string[] }>;
      const entries: ExampleEntry[] = catalog
        .filter((entry) => entry.id.startsWith("control-") || entry.tags?.includes("control"))
        .map((entry, index) => lessonEntry(entry.id, entry.path, index + 1));

      const modules = controlModulesFromEntries(entries);

      assert.strictEqual(modules.length, 14, "examples/control/ ships fourteen module directories");
      assert.strictEqual(
        modules.reduce((total, module) => total + module.lessonCount, 0),
        84,
        "every lesson must belong to exactly one module"
      );
    });
  });

  suite("lesson progress state (W-05)", () => {
    test("reports not_started until a project exists for the lesson", () => {
      assert.strictEqual(controlLessonProgressState("control-a", []), "not_started");
      assert.strictEqual(controlLessonProgressLabel("not_started"), "Not started");
    });

    test("reports project_created, never a completion claim, once the project folder exists", () => {
      const state = controlLessonProgressState("control-a", ["control-a"]);
      assert.strictEqual(state, "project_created");
      assert.strictEqual(controlLessonProgressLabel(state), "Project created");
      assert.notStrictEqual(
        controlLessonProgressLabel(state),
        "Completed",
        "creating a project folder is not retained evidence of a finished Monitor -> Test run"
      );
    });

    test("does not report project_created for an unrelated lesson id", () => {
      assert.strictEqual(controlLessonProgressState("control-b", ["control-a"]), "not_started");
    });

    test("labels are exhaustive and honest for every state", () => {
      for (const state of ["not_started", "project_created"] as const) {
        const label = controlLessonProgressLabel(state);
        assert.ok(
          label === "Not started" || label === "Project created",
          `unexpected progress label '${label}' for state '${state}'`
        );
      }
    });
  });
});
