import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

suite("Control curriculum delivered bindings", () => {
  test("all 82 accepted source and packaged lessons declare valid honest loop bindings", () => {
    const root = path.resolve(__dirname, "../../../../..");
    const bundled = path.join(root, "editors/vscode/media/examples");
    const manifest = JSON.parse(fs.readFileSync(path.join(bundled, "manifest.json"), "utf8"));
    const entries = manifest.filter((entry: { path: string }) => entry.path.startsWith("control/"));
    assert.strictEqual(entries.length, 82);
    const failures: string[] = [];
    for (const entry of entries) {
      const source = path.join(root, "examples", entry.path, "control-loop.json");
      const packaged = path.join(bundled, entry.path, "control-loop.json");
      for (const file of [source, packaged]) {
        const loop = JSON.parse(fs.readFileSync(file, "utf8"));
        const result = validateControlLoopDefinition(loop, {
          projectId: loop.projectId, runtimeId: loop.runtimeId,
        });
        if (result.kind !== "accepted") failures.push(`${path.relative(root, file)}: ${JSON.stringify(result)}`);
      }
      assert.deepStrictEqual(JSON.parse(fs.readFileSync(packaged, "utf8")),
        JSON.parse(fs.readFileSync(source, "utf8")), `${entry.id}: packaged binding drift`);
    }
    assert.deepStrictEqual(failures, [], failures.join("\n"));
  });
});
