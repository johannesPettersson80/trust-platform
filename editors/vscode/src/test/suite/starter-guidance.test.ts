import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

const starters = [
  {
    directory: "empty-simulator",
    firstAction: "Start",
    expected: "no hardware",
    surface: "VS Code",
  },
  {
    directory: "conveyor",
    firstAction: "Start",
    expected: "live values",
    surface: "Live Values",
  },
  {
    directory: "twincat-ads",
    firstAction: "Devices & Connections",
    expected: "Simulator",
    surface: "Live Values",
  },
  {
    directory: "raspberry-pi",
    firstAction: "Devices & Connections",
    expected: "Simulator",
    surface: "Live Values",
  },
  {
    directory: "plcopen-motion-single-axis",
    firstAction: "Start",
    expected: "simulator",
    surface: "Live Values",
  },
] as const;

suite("Starter example guidance", () => {
  test("documents the first action, result, execution mode, and truST surface for every non-control starter", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");

    for (const starter of starters) {
      const readme = path.join(root, "examples", starter.directory, "README.md");
      assert.ok(fs.existsSync(readme), `${starter.directory} must ship README.md guidance`);
      const guidance = fs.readFileSync(readme, "utf8");
      assert.match(guidance, /First action/i, `${starter.directory}: first action`);
      assert.match(guidance, /Expected result/i, `${starter.directory}: expected result`);
      assert.match(guidance, /Simulation/i, `${starter.directory}: simulation mode`);
      assert.match(guidance, /hardware/i, `${starter.directory}: hardware boundary`);
      assert.match(guidance, /VS Code/i, `${starter.directory}: VS Code entry point`);
      assert.match(guidance, new RegExp(starter.firstAction.replace(/[&]/g, "\\&"), "i"), `${starter.directory}: action`);
      assert.match(guidance, new RegExp(starter.expected, "i"), `${starter.directory}: observable result`);
      assert.match(guidance, new RegExp(starter.surface, "i"), `${starter.directory}: truST surface`);
      if (!process.env.TRUST_REPOSITORY_ROOT) {
        const packaged = path.join(root, "editors", "vscode", "media", "examples", starter.directory, "README.md");
        assert.ok(fs.existsSync(packaged), `${starter.directory}: packaged guidance`);
        assert.strictEqual(fs.readFileSync(packaged, "utf8"), guidance, `${starter.directory}: source/package guidance must match exactly`);
      }
    }
  });
});
