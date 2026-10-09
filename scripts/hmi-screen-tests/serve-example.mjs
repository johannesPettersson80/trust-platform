// Serves an example for the screen tests, the tank-level example (examples/hmi_tank_level) unless
// another is named: a copy of the project with its own ports, built and run by `trust-runtime` in
// simulation, as one of Playwright's web servers. A fixture adapts the copy only; `plant-live`
// names the disposable Plant project instead of an example. TRUST_RUNTIME names the binary; by
// default the debug build of this checkout.
//   node serve-example.mjs [example] [web port] [control port] [fixture]
import { plantLiveFixture } from "./plant-live-fixture.mjs";
import { performanceFixture } from "./performance-fixture.mjs";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../..");
const runtime = process.env.TRUST_RUNTIME || path.join(repo, "target/debug/trust-runtime");
const [example = "hmi_tank_level", webPort, controlPort, fixture] = process.argv.slice(2);
const equipmentRanges = example === "hmi_equipment" && fixture === "equipment-ranges";
export const WEB_PORT = Number(webPort || process.env.HMI_SCREEN_WEB_PORT || 18480);
const CONTROL_PORT = Number(controlPort || process.env.HMI_SCREEN_CONTROL_PORT || 19480);

// The equipment-ranges fixture: a Plant analog input whose unit and high limit the PLC switches,
// so its live presentation changes while the value stays at 100 (specification 39, §5.9; Plant
// §7.1). Raw 12 mA on a 0-200 range reads 100; all four limits are enabled and rise from low-low
// to high-high for both high limits, 90 and 140.
const METADATA_DECLARATION = `    Inflow : LREAL;
    Metadata : Plant.FB_AnalogInput := (RangeHigh := 200.0,
        LowLowEnabled := TRUE, LowEnabled := TRUE, HighEnabled := TRUE,
        HighHighEnabled := TRUE, HighHighLimit := 180.0);
    MetadataAlternate : BOOL;`;
const METADATA_CALL = `    IF MetadataAlternate THEN
        Metadata.Unit := 'deg C';
        Metadata.HighLimit := 140.0;
    ELSE
        Metadata.Unit := '°C';
        Metadata.HighLimit := 90.0;
    END_IF;
    Metadata(Raw := 12.0);
    LT101(Raw := Level);`;

/** `text` with `anchor` replaced; a fixture whose anchor is gone stops the server, since a copy
 * patched in no place would test the shipped example instead. */
function patched(text, anchor, replacement, file) {
  if (!text.includes(anchor)) {
    console.error(`${file} has no "${anchor.trim()}" to patch`);
    process.exit(1);
  }
  return text.replace(anchor, replacement);
}

const project = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-screens-"));
if (example === "plant-live") plantLiveFixture(project, repo);
else fs.cpSync(path.join(repo, "examples", example), project, { recursive: true });
// Screen-test fixtures live only in the disposable project, not in the shipped example.
if (example === "hmi_tank_level") {
  for (const fixture of ["trend-scales", "display-limits"]) {
    fs.copyFileSync(path.join(here, `fixtures/${fixture}.toml`), path.join(project, `hmi/pages/${fixture}.toml`));
  }
}
// The Plant panels carry no symbol: a panel of the fixture's own, placed scaled, keeps a turning
// impeller inside a placed panel (specification 39, §8.9; §19.6).
if (example === "hmi_equipment" && fixture === undefined) {
  fs.mkdirSync(path.join(project, "hmi/panels"), { recursive: true });
  fs.copyFileSync(path.join(here, "fixtures/turning-panel.toml"), path.join(project, "hmi/panels/turning-panel.toml"));
  fs.copyFileSync(path.join(here, "fixtures/placed-turning.toml"), path.join(project, "hmi/pages/placed-turning.toml"));
}
// The alarm screens need alarms: in the copy the level controller starts in Manual with its valve
// shut, so the tank drains below LT-101's low limits. The shipped example starts in Auto.
if (example === "hmi_equipment") {
  const main = path.join(project, "src/Main.st");
  fs.writeFileSync(
    main,
    patched(fs.readFileSync(main, "utf8"), "LIC101_Manual : BOOL := FALSE;", "LIC101_Manual : BOOL := TRUE;", main),
  );
}
if (equipmentRanges) {
  fs.copyFileSync(path.join(here, "fixtures/equipment-ranges.toml"), path.join(project, "hmi/pages/equipment-ranges.toml"));
  const main = path.join(project, "src/Main.st");
  let source = fs.readFileSync(main, "utf8");
  source = patched(source, "    Inflow : LREAL;", METADATA_DECLARATION, main);
  source = patched(source, "    LT101(Raw := Level);", METADATA_CALL, main);
  fs.writeFileSync(main, source);
}
if (fixture === "performance") performanceFixture(project);
const config = path.join(project, "runtime.toml");
fs.writeFileSync(
  config,
  fs
    .readFileSync(config, "utf8")
    .replace('listen = "127.0.0.1:8080"', `listen = "127.0.0.1:${WEB_PORT}"`)
    .replace('endpoint = "tcp://127.0.0.1:9100"', `endpoint = "tcp://127.0.0.1:${CONTROL_PORT}"`),
);
if (equipmentRanges) fs.appendFileSync(config, '\n[[runtime.web.hmi.write]]\npath = "Main.MetadataAlternate"\nrole = "operator"\n');
const built = spawnSync(runtime, ["build", "--project", project], { stdio: "inherit" });
if (built.error || built.status !== 0) {
  console.error(`trust-runtime build failed: ${built.error ?? built.status}`);
  process.exit(1);
}
const running = spawn(runtime, ["run", "--project", project], { stdio: "inherit" });
const stop = () => running.kill();
process.on("SIGINT", stop);
process.on("SIGTERM", stop);
running.on("exit", (code) => process.exit(code ?? 0));
