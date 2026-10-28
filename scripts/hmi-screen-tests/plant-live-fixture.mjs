// The disposable project of the Plant journey (specification 39, §8.4, §21.2 and §21.4; Plant
// library §3, §4.2 and §12): pump P101, the library's real `Plant.FB_Pump`, whose running feedback
// is its own run command, and a process alarm for the runtime's alarm list. The pump's panel
// (`plant-run`) and component definition (`plant-pump`) come from the Plant library through the
// project's dependency, as a customer's project uses them; nothing is copied into the project. The
// users are the equipment example's; panel commands and the Settings dialog need no write rules.
// No shipped file is modified.
import fs from "node:fs";
import path from "node:path";

/** The pump and the alarm, then the screens' block. The pump starts in Auto and stopped. */
const PROGRAM = `USING Plant;

PROGRAM Application
VAR
    P101 : Plant.FB_Pump;
    Trip : Alarm.DigitalAlarm;
    Display : Hmi.Display;
END_VAR
    // The running feedback is the pump's own run command, so Start and Stop are confirmed.
    P101(RunningFeedback := P101.RunCommand);
    Trip(Condition := TRUE, Message := 'Test process alarm');
    Display();
END_PROGRAM

CONFIGURATION Project
RESOURCE Cpu ON PLC
    TASK Cyclic (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Cyclic : Application;
END_RESOURCE
END_CONFIGURATION
`;

/** The HMI screen's size, 1280 × 800 (§21.7). */
const SCREEN = `format = 1
size = [1280, 800]
theme = "dark"
symbols = "flat"
start_page = "plant-live"
`;

/** The pump names its block, instance and panel, so a click opens the panel (§21.1 item 2, §8.7
 * item 3). */
const PLANT_PAGE = `format = 1
title = "Plant equipment"

[[object]]
id = "heading"
type = "text"
variant = "heading"
at = [40, 32]
size = [800, 40]
text = "Plant equipment"

[[object]]
id = "pump"
tag = "P101"
type = "pump"
block = "Plant.FB_Pump"
instance = "Main.P101"
faceplate = "plant-run"
at = [160, 160]
size = [128, 108]

[object.bind]
running = "Main.P101.Running"
fault = "Main.P101.Fault"
manual = "Main.P101.Manual"
out-of-service = "Main.P101.OutOfService"
`;

/** The runtime lists every alarm instance itself, so the list binds nothing (specification 40,
 * §5; Plant §9.3). */
const ALARMS_PAGE = `format = 1
title = "Alarms"

[[object]]
id = "alarms"
type = "alarm-list"
at = [24, 24]
size = [1200, 700]
`;

function write(project, name, text) {
  const file = path.join(project, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
}

/** The libraries of this checkout as the project's dependencies, as the equipment example
 * declares its own copies. */
function manifest(repo) {
  const lines = ["[project]", 'include_paths = ["src"]', "", "[dependencies]"];
  for (const [name, folder] of [["Alarm", "alarm"], ["Control", "control"], ["Plant", "plant"], ["Hmi", "hmi"]]) {
    const library = JSON.stringify(path.join(repo, "libraries", folder));
    lines.push(`${name} = { path = ${library}, version = "0.1.0" }`);
  }
  return `${lines.join("\n")}\n`;
}

export function plantLiveFixture(project, repo) {
  write(project, "trust-lsp.toml", manifest(repo));
  write(project, "io.toml", '[io]\ndriver = "simulated"\nparams = {}\n');
  fs.copyFileSync(path.join(repo, "examples/hmi_equipment/runtime.toml"), path.join(project, "runtime.toml"));
  write(project, "src/Main.st", PROGRAM);
  write(project, "hmi/screen.toml", SCREEN);
  write(project, "hmi/pages/plant-live.toml", PLANT_PAGE);
  write(project, "hmi/pages/alarms.toml", ALARMS_PAGE);
}
