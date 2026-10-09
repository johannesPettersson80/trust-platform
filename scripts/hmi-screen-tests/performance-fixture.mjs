// The same workload as spec 39 §19.6. Generated only inside a disposable example copy.
import fs from 'node:fs';
import path from 'node:path';
export function performanceFixture(project) {
  // Produce values faster than the stream's 4 Hz cap so PLC scan quantization cannot
  // starve the browser workload. The measured input, not the requested task period, owns the gate.
  const runtimeConfig = path.join(project, 'runtime.toml');
  fs.writeFileSync(runtimeConfig, fs.readFileSync(runtimeConfig, 'utf8').replace('cycle_interval_ms = 100', 'cycle_interval_ms = 20'));
  // This workload generates its own signals; the equipment plant uses different variables.
  fs.rmSync(path.join(project, "simulation.toml"), {force:true});
  fs.writeFileSync(path.join(project, 'src/Main.st'), `PROGRAM Perf
VAR
    Running : ARRAY[1..30] OF BOOL := [30(TRUE)];
    Values : ARRAY[1..270] OF LREAL;
    Phase : LREAL;
    I : INT;
END_VAR
    Phase := Phase + 0.1;
    IF Phase >= 80.0 THEN Phase := 0.0; END_IF;
    FOR I := 1 TO 270 DO
        Values[I] := 10.0 + Phase + INT_TO_LREAL(I) * 0.01;
    END_FOR;
END_PROGRAM
PROGRAM Screens
VAR Display : Hmi.Display; END_VAR
    Display();
END_PROGRAM
CONFIGURATION Config
RESOURCE Cpu ON PLC
    TASK Fast(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Perf;
    TASK ScreenTask(INTERVAL := T#100ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
END_CONFIGURATION
`);
  fs.writeFileSync(path.join(project, 'hmi/screen.toml'), 'format = 1\nsize = [1856, 960]\ntheme = "dark"\nsymbols = "flat"\n[navigation]\nstyle = "none"\n');
  const pages = path.join(project, 'hmi/pages');
  fs.rmSync(pages, {recursive:true,force:true});fs.mkdirSync(pages);
  const rows = ['format = 1\ntitle = "Performance workload"\n'];
  for (let index=0; index<30; index++) rows.push(`\n[[object]]\nid = "p${index+1}"\ntype = "pump"\nat = [${30+(index%10)*180}, ${16+Math.floor(index/10)*112}]\nsize = [128, 100]\n[object.bind]\nrunning = "Main.Running[${index+1}]"\n`);
  for (let index=0; index<270; index++) rows.push(`\n[[object]]\nid = "v${index+1}"\ntype = "value"\nat = [${30+(index%17)*106}, ${360+Math.floor(index/17)*36}]\nsize = [104, 32]\ndecimals = 1\n[object.bind]\nvalue = "Main.Values[${index+1}]"\n`);
  fs.writeFileSync(path.join(pages,'performance.toml'),rows.join(''));
}
