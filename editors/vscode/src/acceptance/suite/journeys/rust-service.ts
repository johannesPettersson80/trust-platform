// The Rust service walkthrough (examples/rust_service/README.md; specification 48 §13 and RU-88): the project's runtime
// runs from the command line, as the README's first step says, and the harness starts it so. The journey creates the
// service's token as the user does in a second terminal, connects the side panel's Target to the runtime, and watches
// Live Values while the stand-in price source and the price service run beside it, each started as the user starts it.
// VS Code's window has no Rust on PATH, so the harness builds the service and the stand-in with the computer's own Rust
// before the run, as the README's `cargo run` does: E2E_PRICE_SERVICE and E2E_STAND_IN_SOURCE name them, and
// E2E_TRUST_RUNTIME the installed extension's trust-runtime. Each step records what it shows; the README quotes the run,
// and a step that does not show its result fails the journey.
import * as child from "child_process";
import * as vscode from "vscode";

import * as cdp from "../cdp";
import { connectProjectRuntime, until } from "../hmi";
import { assertNoFailedStep, expect, open, Session, step } from "../session";

/** The heater's values as Live Values shows them, read by their addresses. */
type Heater = { setpoint: number; target: number; ok: string; temperature: number; heater: string; names: string[] };

/** A started program: its process and what it printed so far. */
type Started = { process: child.ChildProcess; output: () => string };

/** A script that gives every row's name and value, by the row's address. */
const rows = String.raw`var out={};[...d.querySelectorAll('.row')].forEach(function(r){var a=(r.dataset.rowKey||'').split('|')[1];if(!a)return;var n=r.querySelector('.name div'),v=r.querySelector('.value');out[a]={name:n?n.innerText.trim():'',value:v?v.innerText.trim():''};});return JSON.stringify(out);`;

/** A script that types 60.0 into the temperature's row and presses Write, as the user does. */
const writeTemperature = String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%ID0';});if(!r)return 'NO_ROW';var i=r.querySelector('input.value-input');if(!i)return 'NO_INPUT';i.value='60.0';i.dispatchEvent(new Event('input',{bubbles:true}));var w=[...r.querySelectorAll('button')].find(function(b){return (b.textContent||'').trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'WRITE_DISABLED';w.click();return 'CLICKED';`;

/** The heater's values in Live Values, or null while its rows are missing. */
async function read(s: Session, live: string): Promise<Heater | null> {
  // The first step attaches Live Values; a step after it failed reports that, not the protocol's error for an empty session.
  expect(live.length > 0, "Live Values is attached: the step runtime-alone attaches it");
  const shown = JSON.parse(await cdp.evalWebview(s.wb, live, rows)) as Record<string, { name: string; value: string }>;
  const at = (address: string) => shown[address];
  const wanted = ["%MD0", "%MD4", "%MX8.0", "%ID0", "%QX0.0"];
  if (wanted.some((address) => !at(address))) return null;
  return {
    setpoint: Number(at("%MD0").value),
    target: Number(at("%MD4").value),
    ok: at("%MX8.0").value,
    temperature: Number(at("%ID0").value),
    heater: at("%QX0.0").value,
    names: wanted.map((address) => `${at(address).name} (${address})`),
  };
}

/** Starts `program` with `args` in `folder`, keeping what it prints. */
function start(program: string, args: string[], folder: string, env: NodeJS.ProcessEnv = process.env): Started {
  const started = child.spawn(program, args, { cwd: folder, env, stdio: ["pipe", "pipe", "pipe"] });
  let printed = "";
  started.stdout?.on("data", (bytes: Buffer) => { printed += bytes.toString(); });
  started.stderr?.on("data", (bytes: Buffer) => { printed += bytes.toString(); });
  return { process: started, output: () => printed };
}

/** Waits until `started` printed a line that matches `pattern`, and gives what it printed. */
async function printed(started: Started, pattern: RegExp, ms: number): Promise<string> {
  const text = await until(async () => started.output(), (output) => pattern.test(output), ms);
  expect(pattern.test(text), `it prints ${pattern}: ${text}`);
  return text;
}

suite("Rust service: a price service beside the runtime, and the program's heartbeat gate", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "rust-service"); });

  test("takes the service's setpoint while its heartbeat is fresh, and falls back when it stops", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
    const runtime = process.env.E2E_TRUST_RUNTIME ?? "";
    const service = process.env.E2E_PRICE_SERVICE ?? "";
    const standIn = process.env.E2E_STAND_IN_SOURCE ?? "";
    expect(Boolean(runtime && service && standIn), "the harness names trust-runtime, the price service and the stand-in");
    let live = "";
    let source: Started | undefined;
    let price: Started | undefined;
    try {
      await step(s, "runtime-alone", async () => {
        const token = child.spawnSync(runtime,
          ["ctl", "--project", ".", "service-token", "price", "--out", "secrets/price.token", "--role", "engineer"],
          { cwd: folder, encoding: "utf8" });
        expect(token.status === 0, `the service's token is created: ${token.stdout}${token.stderr}`);
        // The runtime allows debugging, so Connect attaches, and the attach opens Live Values.
        const connected = await connectProjectRuntime(s, true);
        live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
        const before = await until(() => read(s, live), (shown) => shown !== null, 30_000);
        expect(before?.target === 55 && /false/i.test(before.ok) && /true/i.test(before.heater),
          `Live Values lists the heater: the safe 55.0, not fresh, the heater on at the tank's 0.0: ${JSON.stringify(before)}`);
        const wrote = await cdp.evalWebview(s.wb, live, writeTemperature);
        expect(wrote === "CLICKED", `Write 60.0 on the temperature's row: ${wrote}`);
        const after = await until(() => read(s, live), (shown) => shown?.temperature === 60, 10_000);
        expect(after?.temperature === 60 && after.target === 55 && /false/i.test(after.ok) && /false/i.test(after.heater),
          `without the service: the safe 55.0, not fresh, the heater off at 60: ${JSON.stringify(after)}`);
        return { token: token.stdout.trim(), status: connected.status, before, after };
      });
      await step(s, "service-cheap", async () => {
        source = start(standIn, ["38.2"], folder);
        const serving = await printed(source, /Serving 38\.2 EUR\/MWh at http:\/\/127\.0\.0\.1:47801\/price/, 15_000);
        const started = Date.now();
        price = start(service, [], folder, { ...process.env, TRUST_PROJECT: folder });
        const fetched = await printed(price, /38\.2 EUR\/MWh: setpoint 65\.0 °C/, 15_000);
        const shown = await until(() => read(s, live), (heater) => heater?.target === 65, 15_000);
        const seconds = (Date.now() - started) / 1000;
        expect(shown?.setpoint === 65 && shown.target === 65 && /true/i.test(shown.ok) && /true/i.test(shown.heater),
          `the service's 65.0 is the target, fresh, and the heater is on: ${JSON.stringify(shown)}`);
        return { serving: serving.trim(), fetched: fetched.trim(), shown, seconds };
      });
      await step(s, "price-120", async () => {
        source?.process.stdin?.write("120\n");
        const now = await printed(source as Started, /Now 120 EUR\/MWh\./, 10_000);
        const fetched = await printed(price as Started, /120\.0 EUR\/MWh: setpoint 50\.0 °C/, 10_000);
        const shown = await until(() => read(s, live), (heater) => heater?.target === 50, 10_000);
        expect(shown?.target === 50 && /true/i.test(shown.ok) && /false/i.test(shown.heater),
          `the service's 50.0 is the target, and the heater is off: ${JSON.stringify(shown)}`);
        return { now: now.trim().split("\n").pop(), fetched: fetched.trim().split("\n").pop(), shown };
      });
      await step(s, "service-stopped", async () => {
        const stopped = Date.now();
        price?.process.kill("SIGINT");
        const readings: Array<{ seconds: number; ok: string; target: number }> = [];
        const shown = await until(async () => {
          const heater = await read(s, live);
          readings.push({ seconds: Math.round((Date.now() - stopped) / 100) / 10, ok: heater?.ok ?? "", target: heater?.target ?? Number.NaN });
          return heater;
        }, (heater) => /false/i.test(heater?.ok ?? ""), 10_000);
        const seconds = (Date.now() - stopped) / 1000;
        // The last heartbeat came at most 0.5 s before the stop; the gate falls back 2 s after it, at the next scan, and
        // Live Values refreshes about twice a second: between 1.5 s and 3 s after the stop, read within a margin.
        expect(seconds >= 1 && seconds <= 4, `the program falls back about 2 s after the service stops: ${seconds} s, ${JSON.stringify(readings)}`);
        expect(shown?.target === 55 && shown.setpoint === 50 && /false/i.test(shown.heater),
          `the safe 55.0 again, while the service's last 50.0 stays unused: ${JSON.stringify(shown)}`);
        return { seconds, readings, shown };
      });
    } finally {
      price?.process.kill("SIGINT");
      source?.process.kill("SIGINT");
    }
    assertNoFailedStep(s);
  });
});
