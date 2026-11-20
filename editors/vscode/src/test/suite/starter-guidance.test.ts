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

/** The text of the README's section `## <title>`, up to the next section; empty when it has none. */
function section(text: string, title: string): string {
  const heading = `\n## ${title}\n`;
  const start = text.indexOf(heading);
  if (start < 0) return "";
  const rest = text.slice(start + heading.length);
  const end = rest.search(/^## /m);
  return end < 0 ? rest : rest.slice(0, end);
}

suite("Starter example guidance", () => {
  // Each starter names its first action, what the user sees, that it runs on the Simulator and which hardware it needs, VS Code,
  // and the truST surface it uses. A README converted to the example template (specification 25 §3.5), which is off the
  // pending list of scripts/example_readmes.json, says these in its sections: the first numbered step of `## Step by step` names
  // the first action, and `## What you need` names VS Code and the hardware. A README not converted yet says them in its four
  // bullets (First action, Expected result, Simulation and hardware, truST surface).
  test("documents the first action, result, execution mode, and truST surface for every non-control starter", () => {
    const root = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const pending = new Set<string>(
      JSON.parse(fs.readFileSync(path.join(root, "scripts", "example_readmes.json"), "utf8")).pending
    );

    for (const starter of starters) {
      const readme = path.join(root, "examples", starter.directory, "README.md");
      assert.ok(fs.existsSync(readme), `${starter.directory} must ship README.md guidance`);
      const guidance = fs.readFileSync(readme, "utf8");
      const action = new RegExp(starter.firstAction.replace(/[&]/g, "\\&"), "i");
      if (pending.has(`examples/${starter.directory}/README.md`)) {
        assert.match(guidance, /First action/i, `${starter.directory}: first action`);
        assert.match(guidance, /Expected result/i, `${starter.directory}: expected result`);
        assert.match(guidance, /Simulation/i, `${starter.directory}: simulation mode`);
        assert.match(guidance, /hardware/i, `${starter.directory}: hardware boundary`);
        assert.match(guidance, /VS Code/i, `${starter.directory}: VS Code entry point`);
        assert.match(guidance, action, `${starter.directory}: action`);
        assert.match(guidance, new RegExp(starter.expected, "i"), `${starter.directory}: observable result`);
        assert.match(guidance, new RegExp(starter.surface, "i"), `${starter.directory}: truST surface`);
      } else {
        const need = section(guidance, "What you need");
        const steps = section(guidance, "Step by step");
        const first = steps.match(/^1\. .*$/m)?.[0] ?? "";
        assert.match(need, /VS Code/i, `${starter.directory}: What you need names VS Code`);
        assert.match(need, /hardware/i, `${starter.directory}: What you need names the hardware`);
        assert.match(guidance, /Simulator/i, `${starter.directory}: the Simulator`);
        assert.match(first, action, `${starter.directory}: the first step names the first action: ${first}`);
        assert.match(steps, new RegExp(starter.expected, "i"), `${starter.directory}: what the steps show`);
        assert.match(steps, new RegExp(starter.surface, "i"), `${starter.directory}: the truST surface in the steps`);
      }
      if (!process.env.TRUST_REPOSITORY_ROOT) {
        const packaged = path.join(root, "editors", "vscode", "media", "examples", starter.directory, "README.md");
        assert.ok(fs.existsSync(packaged), `${starter.directory}: packaged guidance`);
        assert.strictEqual(fs.readFileSync(packaged, "utf8"), guidance, `${starter.directory}: source/package guidance must match exactly`);
      }
    }
  });
});
