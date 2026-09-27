import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import {
  controlLessonProgressLabel,
  controlLessonProgressState,
  controlModuleId,
  controlModuleTitle,
  controlModulesFromEntries,
  type ExampleEntry,
} from "../../examples/model";
import { loadExamples } from "../../examples/load";
import { captureCopiedLessonPreview } from "./examples-markdown-preview";
// Keep the pre-fix checkout compilable: these behavior seams are intentionally absent there,
// so the baseline reaches an assertion failure rather than failing during TypeScript binding.
const exampleTopicAction = (require("../../examples/topics") as {
  exampleTopicAction?: (actionId: string) => unknown;
}).exampleTopicAction;
const validateExampleFolderName = (require("../../examples/copy") as {
  validateExampleFolderName?: (value: string) => string | undefined;
}).validateExampleFolderName;

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
  suite("examples workflow contract", () => {
    test("lesson README next links use the target lesson.json sequence", () => {
      const root = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const control = path.join(root, "examples", "control");
      const lessons = new Map<string, number>();
      for (const lessonJson of walkFiles(control, "lesson.json")) {
        const lessonPath = path.dirname(lessonJson);
        const metadata = JSON.parse(fs.readFileSync(lessonJson, "utf8")) as { sequence: number };
        lessons.set(path.resolve(lessonPath), metadata.sequence);
      }
      const mismatches: string[] = [];
      for (const readme of walkFiles(control, "README.md")) {
        const directory = path.dirname(readme);
        const source = fs.readFileSync(readme, "utf8");
        for (const match of source.matchAll(/\[Lesson\s+(\d+)\]\(([^)]+)\)/g)) {
          const target = path.normalize(path.join(directory, match[2])).replace(`${path.sep}README.md`, "");
          const targetSequence = lessons.get(target);
          if (targetSequence !== undefined && targetSequence !== Number(match[1])) {
            mismatches.push(`${path.relative(control, readme)}: label ${match[1]} targets sequence ${targetSequence}`);
          }
        }
      }
      assert.deepStrictEqual(mismatches, [], `README navigation labels must match declared sequences: ${mismatches.join(", ")}`);
    });

    test("lesson detail exposes full description and a lesson-specific preview", () => {
      const lessonModule = require("../../examples/controlLesson") as {
        controlLessonDetail?: (lesson: unknown, description: string) => {
          objective: string; description: string; expectedObservation: string;
          preview: Array<{ title: string; paragraphs: string[] }>;
        };
      };
      assert.strictEqual(typeof lessonModule.controlLessonDetail, "function");
      if (!lessonModule.controlLessonDetail) return;
      const root = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const catalog = JSON.parse(fs.readFileSync(path.join(root, "examples", "catalog.json"), "utf8")) as Array<{ id: string; path: string; description: string }>;
      const entry = catalog.find((candidate) => candidate.path.startsWith("control/"));
      assert.ok(entry, "expected a canonical control lesson in catalog");
      if (!entry) return;
      const lesson = JSON.parse(fs.readFileSync(path.join(root, "examples", entry.path, "lesson.json"), "utf8"));
      const detail = lessonModule.controlLessonDetail(lesson, entry.description);
      assert.strictEqual(detail.description, entry.description);
      assert.ok(detail.objective.length > 20);
      assert.ok(detail.expectedObservation.length > 20);
      assert.ok(detail.preview.length > 0);
      assert.ok(detail.preview.some((section) => section.paragraphs.some((paragraph) => paragraph.length > 20)));
      assert.ok(detail.preview.every((section) => section.title && section.paragraphs.length > 0));
    });

    test("isolates missing, invalid JSON, and schema-invalid lessons while retaining valid entries", async () => {
      const root = await fs.promises.mkdtemp(path.join(os.tmpdir(), "trust-example-load-"));
      const examplesRoot = path.join(root, "media", "examples");
      await fs.promises.mkdir(examplesRoot, { recursive: true });
      const entries = [
        { id: "valid-starter", title: "Valid starter", description: "Runs", path: "valid-starter", hardware: "none" },
        { id: "missing-lesson", title: "Missing", description: "Unavailable", path: "missing-lesson", hardware: "none", tags: ["control"] },
        { id: "invalid-json", title: "Invalid JSON", description: "Unavailable", path: "invalid-json", hardware: "none", tags: ["control"] },
        { id: "invalid-schema", title: "Invalid schema", description: "Unavailable", path: "invalid-schema", hardware: "none", tags: ["control"] },
      ];
      await fs.promises.writeFile(path.join(examplesRoot, "manifest.json"), JSON.stringify(entries));
      await fs.promises.mkdir(path.join(examplesRoot, "invalid-json"), { recursive: true });
      await fs.promises.writeFile(path.join(examplesRoot, "invalid-json", "lesson.json"), "{not-json");
      await fs.promises.mkdir(path.join(examplesRoot, "invalid-schema"), { recursive: true });
      await fs.promises.writeFile(path.join(examplesRoot, "invalid-schema", "lesson.json"), "{}");
      try {
        const loaded = await loadExamples({ extensionUri: vscode.Uri.file(root) } as vscode.ExtensionContext);
        assert.deepStrictEqual(loaded.map((entry) => entry.id), entries.map((entry) => entry.id));
        for (const id of ["missing-lesson", "invalid-json", "invalid-schema"]) {
          const entry = loaded.find((candidate) => candidate.id === id);
          assert.strictEqual(entry?.lessonAvailability?.kind, "unavailable", id);
          assert.ok(entry?.lessonAvailability?.reason, id);
        }
        assert.strictEqual(loaded[0].lessonAvailability, undefined);
      } finally {
        await fs.promises.rm(root, { recursive: true, force: true });
      }
    });

    test("does not hide a malformed top-level examples manifest", async () => {
      const root = await fs.promises.mkdtemp(path.join(os.tmpdir(), "trust-example-manifest-"));
      const examplesRoot = path.join(root, "media", "examples");
      await fs.promises.mkdir(examplesRoot, { recursive: true });
      await fs.promises.writeFile(path.join(examplesRoot, "manifest.json"), "not-json");
      try {
        await assert.rejects(
          loadExamples({ extensionUri: vscode.Uri.file(root) } as vscode.ExtensionContext),
          /Unexpected token|JSON/
        );
      } finally {
        await fs.promises.rm(root, { recursive: true, force: true });
      }
    });

    test("topic selection stays in the bundled gallery and Read guide is the external action", () => {
      assert.strictEqual(typeof exampleTopicAction, "function");
      if (!exampleTopicAction) return;
      assert.deepStrictEqual(exampleTopicAction("hmi"), {
        type: "browseExamples",
        topicId: "hmi",
      });
      assert.deepStrictEqual(exampleTopicAction("read-guide:hmi"), {
        type: "openExamplesDocumentation",
        topicId: "hmi",
      });
    });

    test("project names reject traversal, separators, and platform-invalid names", () => {
      assert.strictEqual(typeof validateExampleFolderName, "function");
      if (!validateExampleFolderName) return;
      for (const invalid of ["", ".", "..", "lesson/name", "lesson\\name", "lesson<1>", "CON"]) {
        assert.ok(validateExampleFolderName(invalid), `expected '${invalid}' to be rejected`);
      }
      assert.strictEqual(validateExampleFolderName("cold-storage-lesson"), undefined);
    });

    test("copy boundary refuses an existing destination without changing it or writing handoff state", async () => {
      const copyModule = require("../../examples/copy") as {
        copyExample?: (context: unknown, entry: ExampleEntry) => Promise<boolean>;
      };
      assert.strictEqual(typeof copyModule.copyExample, "function");
      if (!copyModule.copyExample) return;
      const base = await fs.promises.mkdtemp(path.join(os.tmpdir(), "trust-example-copy-"));
      const destination = path.join(base, "existing-example");
      await fs.promises.mkdir(destination);
      await fs.promises.writeFile(path.join(destination, "keep.txt"), "unrelated user content");
      const updates: Array<[string, unknown]> = [];
      const context = {
        extensionUri: vscode.Uri.file(path.resolve(__dirname, "..", "..", "..")),
        globalState: {
          update: async (key: string, value: unknown) => { updates.push([key, value]); },
        },
      };
      const previous = process.env.TRUST_UX_EXAMPLE_DESTINATION;
      const previousName = process.env.TRUST_UX_EXAMPLE_NAME;
      const previousOpen = process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
      const previousWarning = vscode.window.showWarningMessage;
      process.env.TRUST_UX_EXAMPLE_DESTINATION = base;
      process.env.TRUST_UX_EXAMPLE_NAME = "existing-example";
      process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = "0";
      (vscode.window as unknown as { showWarningMessage: typeof vscode.window.showWarningMessage }).showWarningMessage = async () => "Overwrite";
      try {
        assert.ok(fs.existsSync(path.join(context.extensionUri.fsPath, "media", "examples", "empty-simulator", "src", "Main.st")));
        const copied = await copyModule.copyExample(context, {
          id: "empty-simulator",
          title: "Empty simulator",
          description: "",
          path: "empty-simulator",
          hardware: "none",
        });
        assert.strictEqual(copied, false);
        assert.strictEqual(await fs.promises.readFile(path.join(destination, "keep.txt"), "utf8"), "unrelated user content");
        assert.deepStrictEqual(updates, []);
      } finally {
        if (previous === undefined) delete process.env.TRUST_UX_EXAMPLE_DESTINATION; else process.env.TRUST_UX_EXAMPLE_DESTINATION = previous;
        if (previousName === undefined) delete process.env.TRUST_UX_EXAMPLE_NAME; else process.env.TRUST_UX_EXAMPLE_NAME = previousName;
        if (previousOpen === undefined) delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER; else process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = previousOpen;
        (vscode.window as unknown as { showWarningMessage: typeof vscode.window.showWarningMessage }).showWarningMessage = previousWarning;
        await fs.promises.rm(base, { recursive: true, force: true });
      }
    });

    test("copy boundary retries with another native name after an existing destination", async () => {
      const copyModule = require("../../examples/copy") as {
        copyExample?: (context: unknown, entry: ExampleEntry) => Promise<boolean>;
      };
      assert.strictEqual(typeof copyModule.copyExample, "function");
      if (!copyModule.copyExample) return;
      const base = await fs.promises.mkdtemp(path.join(os.tmpdir(), "trust-example-retry-"));
      const existing = path.join(base, "existing-example");
      await fs.promises.mkdir(existing);
      await fs.promises.writeFile(path.join(existing, "keep.txt"), "unrelated user content");
      const updates: Array<[string, unknown]> = [];
      const context = {
        extensionUri: vscode.Uri.file(path.resolve(__dirname, "..", "..", "..")),
        globalState: { update: async (key: string, value: unknown) => { updates.push([key, value]); } },
      };
      const previousOpen = vscode.window.showOpenDialog;
      const previousInput = vscode.window.showInputBox;
      const previousWarning = vscode.window.showWarningMessage;
      const previousExecute = vscode.commands.executeCommand;
      let inputCount = 0;
      (vscode.window as unknown as { showOpenDialog: typeof vscode.window.showOpenDialog }).showOpenDialog = async () => [vscode.Uri.file(base)];
      (vscode.window as unknown as { showInputBox: typeof vscode.window.showInputBox }).showInputBox = async () => inputCount++ === 0 ? "existing-example" : "fresh-example";
      (vscode.window as unknown as { showWarningMessage: typeof vscode.window.showWarningMessage }).showWarningMessage = async () => "Choose another name";
      (vscode.commands as unknown as { executeCommand: typeof vscode.commands.executeCommand }).executeCommand = async <T>() => undefined as T;
      try {
        const copied = await copyModule.copyExample(context, {
          id: "empty-simulator", title: "Empty simulator", description: "", path: "empty-simulator", hardware: "none",
        });
        assert.strictEqual(copied, true);
        assert.strictEqual(await fs.promises.readFile(path.join(existing, "keep.txt"), "utf8"), "unrelated user content");
        assert.ok(await fs.promises.stat(path.join(base, "fresh-example", "src", "Main.st")));
        assert.ok(updates.length > 0);
      } finally {
        (vscode.window as unknown as { showOpenDialog: typeof vscode.window.showOpenDialog }).showOpenDialog = previousOpen;
        (vscode.window as unknown as { showInputBox: typeof vscode.window.showInputBox }).showInputBox = previousInput;
        (vscode.window as unknown as { showWarningMessage: typeof vscode.window.showWarningMessage }).showWarningMessage = previousWarning;
        (vscode.commands as unknown as { executeCommand: typeof vscode.commands.executeCommand }).executeCommand = previousExecute;
        await fs.promises.rm(base, { recursive: true, force: true });
      }
    });

    test("copied lesson README replaces sibling navigation with catalog-backed gallery guidance", async () => {
      const copyModule = require("../../examples/copy") as {
        copyExample?: (context: unknown, entry: ExampleEntry) => Promise<boolean>;
      };
      assert.strictEqual(typeof copyModule.copyExample, "function");
      if (!copyModule.copyExample) return;
      const extensionRoot = path.resolve(__dirname, "..", "..", "..");
      const sourceReadme = path.join(extensionRoot, "media", "examples", "control", "01-feedback-foundations-and-temperature-control", "01-cold-storage-loop-anatomy", "README.md");
      const original = await fs.promises.readFile(sourceReadme, "utf8");
      const base = await fs.promises.mkdtemp(path.join(os.tmpdir(), "trust-example-navigation-"));
      const updates: Array<[string, unknown]> = [];
      const previousDestination = process.env.TRUST_UX_EXAMPLE_DESTINATION;
      const previousName = process.env.TRUST_UX_EXAMPLE_NAME;
      const previousOpen = process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
      process.env.TRUST_UX_EXAMPLE_DESTINATION = base;
      process.env.TRUST_UX_EXAMPLE_NAME = "lesson-one-copy";
      process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = "0";
      try {
        const copied = await copyModule.copyExample({
          extensionUri: vscode.Uri.file(extensionRoot),
          globalState: {
            get: (key: string) => [...updates].reverse().find(([storedKey]) => storedKey === key)?.[1],
            update: async (key: string, value: unknown) => { updates.push([key, value]); },
          },
        }, {
          id: "control-lesson-01", title: "1. Control-loop elements", description: "", path: "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy", hardware: "none",
          tags: ["control"], lesson: { sequence: 1, level: "Foundation", equipment: "Simulator", principalTechnique: "Feedback", prerequisites: [] },
        });
        assert.strictEqual(copied, true);
        const landing = updates.find(([key]) => key === "trust.projectLanding")?.[1] as {
          intentId?: string; projectPath?: string; kind?: string; filePath?: string; exampleId?: string;
        } | undefined;
        assert.ok(landing, "copying a lesson must publish the structured landing intent");
        assert.strictEqual(landing?.kind, "lesson");
        assert.strictEqual(landing?.exampleId, "control-lesson-01");
        assert.strictEqual(landing?.filePath, path.join(base, "lesson-one-copy", "README.md"));
        assert.strictEqual(landing?.projectPath, path.join(base, "lesson-one-copy"));
        const copiedReadme = await fs.promises.readFile(path.join(base, "lesson-one-copy", "README.md"), "utf8");
        assert.ok(copiedReadme.includes("use truST > Start from example and select Lesson 2"), copiedReadme.slice(-500));
        assert.ok(!copiedReadme.includes("../02-cold-storage-on-off/README.md"));
        const readmePath = path.join(base, "lesson-one-copy", "README.md");
        if (process.env.TRUST_COPIED_LESSON_CAPTURE) {
          await captureCopiedLessonPreview(readmePath, process.env.TRUST_COPIED_LESSON_CAPTURE);
        }
        await fs.promises.appendFile(readmePath, "\n[Lesson 999](https://example.com/guide)\n[Lesson 998](../missing-lesson/README.md)\n");
        const readmeModule = require("../../examples/copiedLessonReadme") as { adaptCopiedLessonReadme?: (context: unknown, entry: ExampleEntry, destination: vscode.Uri) => Promise<void> };
        assert.strictEqual(typeof readmeModule.adaptCopiedLessonReadme, "function");
        if (readmeModule.adaptCopiedLessonReadme) {
          await readmeModule.adaptCopiedLessonReadme({ extensionUri: vscode.Uri.file(extensionRoot) }, {
            id: "control-lesson-01", title: "1. Control-loop elements", description: "", path: "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy", hardware: "none",
            tags: ["control"], lesson: { sequence: 1, level: "Foundation", equipment: "Simulator", principalTechnique: "Feedback", prerequisites: [] },
          }, vscode.Uri.file(path.join(base, "lesson-one-copy")));
        }
        const afterNegativeLinks = await fs.promises.readFile(readmePath, "utf8");
        assert.ok(afterNegativeLinks.includes("[Lesson 999](https://example.com/guide)"));
        assert.ok(afterNegativeLinks.includes("[Lesson 998](../missing-lesson/README.md)"));
        assert.strictEqual(await fs.promises.readFile(sourceReadme, "utf8"), original);
        assert.ok(updates.length > 0);
      } finally {
        if (previousDestination === undefined) delete process.env.TRUST_UX_EXAMPLE_DESTINATION; else process.env.TRUST_UX_EXAMPLE_DESTINATION = previousDestination;
        if (previousName === undefined) delete process.env.TRUST_UX_EXAMPLE_NAME; else process.env.TRUST_UX_EXAMPLE_NAME = previousName;
        if (previousOpen === undefined) delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER; else process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = previousOpen;
        await fs.promises.rm(base, { recursive: true, force: true });
      }
    });
  });

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
        "Production quality digital PID"
      );
      assert.strictEqual(controlModuleTitle("14-integrated-capstones"), "Integrated capstones");
    });

    test("orders modules by declared sequence and reports each module's real lesson range", () => {
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

    test("groups the approved 82-lesson catalog into its twelve retained source directories", () => {
      const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const catalog = JSON.parse(
        fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")
      ) as Array<{ id: string; path: string; tags?: string[] }>;
      const entries: ExampleEntry[] = catalog
        .filter((entry) => entry.tags?.includes("control"))
        .map((entry, index) => lessonEntry(entry.id, entry.path, index + 1));

      const modules = controlModulesFromEntries(entries);

      assert.strictEqual(modules.length, 12, "the teaching specification maps 82 lessons to twelve retained source directories");
      assert.strictEqual(
        modules.reduce((total, module) => total + module.lessonCount, 0),
        82,
        "every lesson must belong to exactly one module"
      );
    });

    test("orders curriculum modules by their declared lesson sequence", () => {
      const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")) as Array<{ id: string; path: string; tags?: string[] }>;
      const entries = catalog
        .filter((entry) => entry.tags?.includes("control"))
        .map((entry) => {
          const metadata = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", entry.path, "lesson.json"), "utf8")) as { sequence: number; level: string; equipment: string; principal_technique: string; prerequisites: string[] };
          return lessonEntry(entry.id, entry.path, metadata.sequence);
        });
      const modules = controlModulesFromEntries(entries);
      const starts = modules.map((module) => Number(module.range.match(/(?:Lesson|Lessons) (\d+)/)?.[1]));
      assert.deepStrictEqual(starts, [...starts].sort((left, right) => left - right));
    });

    test("compresses each module range into contiguous declared lesson runs", () => {
      const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")) as Array<{ id: string; path: string; tags?: string[] }>;
      const entries = catalog.filter((entry) => entry.tags?.includes("control")).map((entry) => {
        const metadata = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", entry.path, "lesson.json"), "utf8")) as { sequence: number };
        return lessonEntry(entry.id, entry.path, metadata.sequence);
      });
      const modules = controlModulesFromEntries(entries);
      assert.strictEqual(modules.find((module) => module.id.startsWith("02-"))?.range, "Lessons 33–36, Lesson 45, Lesson 67");
      assert.strictEqual(modules.find((module) => module.id.startsWith("01-"))?.range, "Lessons 1–3, 5–9, 12–13, 22–24, Lesson 30, 40–42, Lesson 44");
      assert.strictEqual(modules.reduce((total, module) => total + module.lessonCount, 0), 82);
    });

    test("all real lesson prerequisites resolve to earlier stable lesson ids", () => {
      const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
        ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
        : path.resolve(__dirname, "..", "..", "..", "..", "..");
      const catalog = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", "catalog.json"), "utf8")) as Array<{ id: string; path: string; tags?: string[] }>;
      const lessons = catalog.filter((entry) => entry.tags?.includes("control")).map((entry) => {
        const metadata = JSON.parse(fs.readFileSync(path.join(repositoryRoot, "examples", entry.path, "lesson.json"), "utf8")) as { sequence: number; prerequisites?: string[] };
        return { id: entry.id, sequence: metadata.sequence, prerequisites: metadata.prerequisites ?? [] };
      });
      const byId = new Map(lessons.map((lesson) => [lesson.id, lesson]));
      const violations = lessons.flatMap((lesson) => lesson.prerequisites.map((prerequisite) => {
        const dependency = byId.get(prerequisite);
        return !dependency || dependency.sequence >= lesson.sequence
          ? `${lesson.id}: ${prerequisite}`
          : undefined;
      }).filter((value): value is string => Boolean(value)));
      assert.deepStrictEqual(violations, []);
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

  suite("retained lesson project locations (CPEX-012)", () => {
    test("retains two independently reopenable copies without completion inference", () => {
      const model = require("../../examples/model") as {
        recordControlLessonProject?: (records: unknown[], lessonId: string, projectPath: string) => unknown[];
      };
      assert.strictEqual(typeof model.recordControlLessonProject, "function");
      if (!model.recordControlLessonProject) return;
      const first = model.recordControlLessonProject([], "lesson-1", "/projects/first");
      const records = model.recordControlLessonProject(first, "lesson-1", "/projects/second");
      assert.deepStrictEqual(records, [
        { lessonId: "lesson-1", projectPath: "/projects/first" },
        { lessonId: "lesson-1", projectPath: "/projects/second" },
      ]);
      assert.notStrictEqual(records[0], true, "a retained location is not lesson completion evidence");
    });

    test("reports a deleted retained copy for explicit recovery", async () => {
      const model = require("../../examples/model") as {
        controlLessonProjectRecovery?: (record: { lessonId: string; projectPath: string }, exists: (projectPath: string) => Promise<boolean>) => Promise<unknown>;
      };
      assert.strictEqual(typeof model.controlLessonProjectRecovery, "function");
      if (!model.controlLessonProjectRecovery) return;
      const status = await model.controlLessonProjectRecovery(
        { lessonId: "lesson-1", projectPath: "/projects/moved" },
        async () => false
      );
      assert.deepStrictEqual(status, { kind: "missing", projectPath: "/projects/moved" });
    });

    test("deduplicates only an identical lesson and project pair without mutating input", () => {
      const model = require("../../examples/model") as {
        recordControlLessonProject?: (records: unknown[], lessonId: string, projectPath: string) => unknown[];
      };
      assert.strictEqual(typeof model.recordControlLessonProject, "function");
      if (!model.recordControlLessonProject) return;
      const original = [{ lessonId: "lesson-1", projectPath: "/projects/first" }];
      const next = model.recordControlLessonProject(original, "lesson-1", "/projects/first");
      assert.deepStrictEqual(original, [{ lessonId: "lesson-1", projectPath: "/projects/first" }]);
      assert.deepStrictEqual(next, original);
      assert.notStrictEqual(next, original);
    });

    test("reports a present retained copy when its project still exists", async () => {
      const model = require("../../examples/model") as {
        controlLessonProjectRecovery?: (record: { lessonId: string; projectPath: string }, exists: (projectPath: string) => Promise<boolean>) => Promise<unknown>;
      };
      assert.strictEqual(typeof model.controlLessonProjectRecovery, "function");
      if (!model.controlLessonProjectRecovery) return;
      const status = await model.controlLessonProjectRecovery(
        { lessonId: "lesson-1", projectPath: "/projects/first" },
        async () => true
      );
      assert.deepStrictEqual(status, { kind: "present", projectPath: "/projects/first" });
    });

    test("ignores malformed persisted location records while retaining valid records", () => {
      const model = require("../../examples/model") as {
        readControlLessonProjects?: (state: { get: <T>(key: string, fallback?: T) => T }) => unknown[];
      };
      assert.strictEqual(typeof model.readControlLessonProjects, "function");
      if (!model.readControlLessonProjects) return;
      const records = model.readControlLessonProjects({
        get: <T>() => [
          { lessonId: "lesson-1", projectPath: "/projects/first" },
          { lessonId: "", projectPath: "/projects/invalid" },
          { lessonId: "lesson-2", projectPath: 42 },
          "not-a-record",
        ] as unknown as T,
      });
      assert.deepStrictEqual(records, [{ lessonId: "lesson-1", projectPath: "/projects/first" }]);
    });
  });
});

function walkFiles(root: string, filename: string): string[] {
  const found: string[] = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) found.push(...walkFiles(fullPath, filename));
    else if (entry.isFile() && entry.name === filename) found.push(fullPath);
  }
  return found;
}
