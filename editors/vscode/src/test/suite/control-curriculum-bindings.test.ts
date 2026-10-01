import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

suite("Control curriculum delivered bindings", () => {
  test("all 89 accepted source and packaged lessons ship valid honest loop bindings, or set the loop up with the form", () => {
    const root = path.resolve(__dirname, "../../../../..");
    const bundled = path.join(root, "editors/vscode/media/examples");
    const manifest = JSON.parse(fs.readFileSync(path.join(bundled, "manifest.json"), "utf8"));
    const entries = manifest.filter((entry: { path: string }) => entry.path.startsWith("control/"));
    assert.strictEqual(entries.length, 89);
    const failures: string[] = [];
    for (const entry of entries) {
      // A lesson ships control-loop.json and control-loops/*.json; one set up with the form ships none (teaching spec §6).
      const loopsDirectory = path.join(root, "examples", entry.path, "control-loops");
      const files = [
        ...(fs.existsSync(path.join(root, "examples", entry.path, "control-loop.json")) ? ["control-loop.json"] : []),
        ...(fs.existsSync(loopsDirectory) ? fs.readdirSync(loopsDirectory).filter(name => name.endsWith(".json")).sort().map(name => `control-loops/${name}`) : []),
      ];
      if (!files.length) {
        const readme = fs.readFileSync(path.join(root, "examples", entry.path, "README.md"), "utf8");
        if (!readme.includes("**Configure control loop**")) failures.push(`${entry.id}: ships no loop definition and does not set one up with the form`);
      }
      for (const relative of files) {
        const source = path.join(root, "examples", entry.path, relative);
        const packaged = path.join(bundled, entry.path, relative);
        for (const file of [source, packaged]) {
          const loop = JSON.parse(fs.readFileSync(file, "utf8"));
          const result = validateControlLoopDefinition(loop, {
            projectId: loop.projectId, runtimeId: loop.runtimeId,
          });
          if (result.kind !== "accepted") failures.push(`${path.relative(root, file)}: ${JSON.stringify(result)}`);
        }
        assert.deepStrictEqual(JSON.parse(fs.readFileSync(packaged, "utf8")),
          JSON.parse(fs.readFileSync(source, "utf8")), `${entry.id} ${relative}: packaged binding drift`);
      }
    }
    assert.deepStrictEqual(failures, [], failures.join("\n"));
  });
});
