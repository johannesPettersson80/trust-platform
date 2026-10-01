import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { parseControlLessonContract, validateControlCurriculum } from "../../examples/controlLesson";

suite("Accepted control course navigation", () => {
  test("accepts the real manual PI lesson route and complete catalog", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const lessonPath = path.join(root, "examples/control/06-pid-tuning-methods/28-manual-pi-tuning/lesson.json");
    const lesson = JSON.parse(fs.readFileSync(lessonPath, "utf8")) as Record<string, unknown>;
    const parsed = parseControlLessonContract(lesson, "control-lesson-28");
    assert.ok("workflowStages" in parsed);
    assert.deepStrictEqual(parsed.workflowStages, ["Monitor", "Tune", "Test", "Validate"]);

    const catalog = JSON.parse(fs.readFileSync(path.join(root, "examples/catalog.json"), "utf8")) as Array<{ path: string }>;
    const controlLessons = catalog
      .filter((entry) => entry.path.startsWith("control/"))
      .map((entry) => JSON.parse(fs.readFileSync(path.join(root, "examples", entry.path, "lesson.json"), "utf8")));
    assert.deepStrictEqual(validateControlCurriculum(controlLessons), { kind: "accepted" });
  });

  test("rejects duplicate and unsupported workflow stages", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const lessonPath = path.join(root, "examples/control/06-pid-tuning-methods/28-manual-pi-tuning/lesson.json");
    const lesson = JSON.parse(fs.readFileSync(lessonPath, "utf8")) as Record<string, unknown>;
    assert.throws(
      () => parseControlLessonContract({ ...lesson, workflowStages: ["Monitor", "Tune", "Tune"] }, "control-lesson-28"),
      /workflowStages contains duplicate stage 'Tune'/,
    );
    assert.throws(
      () => parseControlLessonContract({ ...lesson, workflowStages: ["Monitor", "Unsupported"] }, "control-lesson-28"),
      /workflowStages contains unsupported stage 'Unsupported'/,
    );
  });

  test("catalog and lesson metadata follow the normative 89-lesson source map", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const specificationPath = path.join(root, "docs/specs/control-course-teaching-specification.md");
    const specification = fs.readFileSync(specificationPath, "utf8");
    const map = specification.split("### 10.4 Existing-source map")[1];
    assert.ok(map, "the normative existing-source map must exist");
    const rows = [...map.matchAll(/^\| (\d+)\. (.*?) \| \[[^\]]+\]\(([^)]+\/README\.md)\) \|$/gm)].map(match => ({
      sequence: Number(match[1]), title: match[2],
      project: path.dirname(path.resolve(path.dirname(specificationPath), match[3])),
    }));
    assert.strictEqual(rows.length, 89);
    const catalog = JSON.parse(fs.readFileSync(path.join(root, "examples/catalog.json"), "utf8")) as Array<{id: string; title: string; path: string; description: string}>;
    const outcomes = new Map([...specification.matchAll(/^\| \*\*(\d+)\. .*?\*\* \| (.*?) \|/gm)].map(match => [Number(match[1]), match[2]]));
    const entries = catalog.filter(entry => entry.path.startsWith("control/"));
    assert.deepStrictEqual(entries.map(entry => entry.path), rows.map(row => path.relative(path.join(root, "examples"), row.project).split(path.sep).join("/")),
      "the gallery must expose the accepted source projects in teaching order, without historical extras");
    const lessons = rows.map((row, index) => {
      const lesson = JSON.parse(fs.readFileSync(path.join(row.project, "lesson.json"), "utf8"));
      assert.strictEqual(lesson.id, entries[index].id);
      assert.strictEqual(lesson.sequence, row.sequence);
      assert.strictEqual(lesson.title, row.title);
      assert.strictEqual(entries[index].title, `${row.sequence}. ${row.title}`);
      assert.strictEqual(entries[index].description, outcomes.get(row.sequence));
      assert.strictEqual(fs.readFileSync(path.join(row.project, "README.md"), "utf8").split("\n")[0], `# ${row.sequence}. ${row.title}`);
      assert.strictEqual(lesson.nextLesson, entries[index + 1]?.id ?? null);
      if (index > 0) assert.ok(lesson.prerequisites.includes(entries[index - 1].id), `${lesson.id}: predecessor must be a prerequisite`);
      else assert.deepStrictEqual(lesson.prerequisites, []);
      return lesson;
    });
    assert.deepStrictEqual(validateControlCurriculum(lessons), { kind: "accepted" });
  });
});
