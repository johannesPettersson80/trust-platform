// The Rust-first press cell walkthrough (examples/rust_first_cell/README.md, step 5; specification 48 RU-88): Start builds
// the crate with truST's own toolchain, with no Rust on PATH, and runs the program on the Simulator. Live Values shows the
// fixture's seven signals by their addresses, and Ready (%QX0.2) is on while the cell waits. A Write of TRUE to the part
// sensor (%IX0.1) and then to Start (%IX0.0) runs one part: the clamp's valve (%QX0.0) and, 300 ms later, its sensor
// (%IX0.2); the press's valve (%QX0.1) and, 800 ms later, its sensor (%IX0.3); the hold, the lift and the unclamp, until
// both valves and both sensors are off again 2.7 s after Start and the cell waits in UNLOAD for the part to be taken out.
// A Write of FALSE to Start and then to the part sensor takes it out, and Ready comes on again. Each step records what it
// shows; the README quotes the run, and a step that does not show its result fails the journey.
import * as cdp from "../cdp";
import { until } from "../hmi";
import { assertNoFailedStep, expect, open, pressHome, Session, step } from "../session";

/** The fixture's signals by address, as `trust.toml` binds them. */
const SIGNALS = {
  start: "%IX0.0",
  part: "%IX0.1",
  clampClosed: "%IX0.2",
  pressDown: "%IX0.3",
  clamp: "%QX0.0",
  press: "%QX0.1",
  ready: "%QX0.2",
} as const;

type Signal = keyof typeof SIGNALS;

/** What Live Values shows of each signal: TRUE or FALSE, and the row's name. */
type Cell = Record<Signal, string> & { names: string[] };

/** A reading of the cell, `seconds` after the Write of Start. */
type Sample = Cell & { seconds: number };

/** The first row of `address` that offers actions, else the first row of it: `row(a)` in the scripts below. */
const rowOf = String.raw`var rows=[...d.querySelectorAll('.row')];function row(a){var all=rows.filter(function(r){return (r.dataset.rowKey||'').split('|')[1]===a;});return all.find(function(r){return r.querySelector('.actions');})||all[0];}`;

/** A script that gives every signal's name and value, by its address. */
const shown = rowOf + String.raw`var out={};${JSON.stringify(Object.values(SIGNALS))}.forEach(function(a){var r=row(a);if(!r)return;var n=r.querySelector('.name div'),v=r.querySelector('.value');out[a]={name:n?n.innerText.trim():'',value:v?v.innerText.trim():''};});return JSON.stringify(out);`;

/** A script that chooses `value` on the row of `address` and presses Write, as the user does. */
const write = (address: string, value: "TRUE" | "FALSE") => rowOf + String.raw`var r=row('${address}');if(!r)return 'NO_ROW';var t=r.querySelector('.bool-toggle');if(!t)return 'NO_CHOOSER';if(t.value!=='${value}')t.click();var w=[...r.querySelectorAll('button')].find(function(b){return b.innerText.trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'DISABLED';w.click();return 'CLICKED';`;

/** The cell in Live Values, or null while one of its rows is missing. */
async function read(s: Session, live: string): Promise<Cell | null> {
  const rows = JSON.parse(await cdp.evalWebview(s.wb, live, shown)) as Record<string, { name: string; value: string }>;
  const signals = Object.keys(SIGNALS) as Signal[];
  if (signals.some((signal) => !rows[SIGNALS[signal]])) return null;
  const cell = { names: signals.map((signal) => `${rows[SIGNALS[signal]].name} (${SIGNALS[signal]})`) } as Cell;
  for (const signal of signals) cell[signal] = rows[SIGNALS[signal]].value.toUpperCase();
  return cell;
}

/** Writes `value` to the signal's input and expects the click to land. */
async function writeInput(s: Session, live: string, signal: Signal, value: "TRUE" | "FALSE"): Promise<void> {
  const wrote = await cdp.evalWebview(s.wb, live, write(SIGNALS[signal], value));
  expect(wrote === "CLICKED", `Write ${value} on the row of ${signal} (${SIGNALS[signal]}): ${wrote}`);
}

/** Reads the cell every 100 ms until `done` holds or `ms` pass; each sample carries its seconds since `started`. */
async function watch(s: Session, live: string, started: number, done: (cell: Cell) => boolean, ms: number): Promise<Sample[]> {
  const samples: Sample[] = [];
  while (Date.now() - started < ms) {
    const cell = await read(s, live);
    if (cell) samples.push({ seconds: Math.round((Date.now() - started) / 100) / 10, ...cell });
    if (cell && done(cell)) break;
    await cdp.sleep(100);
  }
  return samples;
}

const on = (cell: Cell | null | undefined, signal: Signal) => cell?.[signal] === "TRUE";
const off = (cell: Cell | null | undefined, signal: Signal) => cell?.[signal] === "FALSE";
/** The cell at rest: both valves and both sensors off. */
const still = (cell: Cell) => off(cell, "clamp") && off(cell, "press") && off(cell, "clampClosed") && off(cell, "pressDown");
/** The first sample at which `signal` was on, or -1. */
const firstOn = (samples: Sample[], signal: Signal) => samples.findIndex((sample) => on(sample, signal));
const listed = (samples: Sample[]) =>
  samples.map((sample) => `${sample.seconds}s ${(Object.keys(SIGNALS) as Signal[]).filter((signal) => on(sample, signal)).join("+") || "-"}`).join(", ");

suite("Rust-first press cell: one part from Start to the unload", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "rust-first-cell"); });

  test("builds the crate with truST's toolchain, presses one part in turn and is ready again once it is taken out", async () => {
    let live = "";
    await step(s, "started", async () => {
      const clicked = await pressHome(s, "^Start$");
      // The first build of the project builds the SDK too, with truST's toolchain: nothing of Rust is on PATH.
      const running = (items: string[]) => (items || []).some((item) => /Simulator running/i.test(item));
      const status = await cdp.pollValue("the Simulator running", () => cdp.statusBar(s.wb), running, 300_000);
      expect(running(status), `the status bar says the Simulator runs: ${JSON.stringify(status)}`);
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      const cell = await until(() => read(s, live), (shown) => shown !== null && on(shown, "ready"), 30_000);
      expect(on(cell, "ready") && still(cell as Cell) && off(cell, "start") && off(cell, "part"),
        `Ready on, the valves and sensors off, no part and no Start: ${JSON.stringify(cell)}`);
      return { clicked, status, cell };
    });
    await step(s, "presses-one-part", async () => {
      await writeInput(s, live, "part", "TRUE");
      const part = await until(() => read(s, live), (shown) => on(shown, "part"), 5_000);
      expect(on(part, "part") && on(part, "ready"), `the part lies in the fixture and the cell still waits for Start: ${JSON.stringify(part)}`);
      const started = Date.now();
      await writeInput(s, live, "start", "TRUE");
      // The cycle that sees Start turns Ready off and opens the clamp, so Ready off with the cell at rest is UNLOAD alone,
      // 2.7 s after Start (the README's table), read within Live Values' refresh, about twice a second.
      const samples = await watch(s, live, started, (cell) => off(cell, "ready") && still(cell), 8_000);
      const last = samples[samples.length - 1];
      const [clamp, closed, press, down] = [firstOn(samples, "clamp"), firstOn(samples, "clampClosed"), firstOn(samples, "press"), firstOn(samples, "pressDown")];
      expect(clamp >= 0 && closed >= 0 && press >= 0 && down >= 0, `the clamp, its sensor, the press and its sensor each came on: ${listed(samples)}`);
      expect(clamp <= closed && closed <= press && press <= down, `in that order: ${listed(samples)}`);
      expect(off(last, "ready") && still(last) && on(last, "part"),
        `at rest in UNLOAD, the part still in the fixture and Ready off: ${listed(samples)}`);
      expect(last.seconds >= 2.2 && last.seconds <= 4.5, `about 2.7 s from Start to the unload: ${last.seconds} s, ${listed(samples)}`);
      return { seconds: last.seconds, samples: listed(samples), names: last.names };
    });
    await step(s, "part-taken-out", async () => {
      await writeInput(s, live, "start", "FALSE");
      const started = Date.now();
      await writeInput(s, live, "part", "FALSE");
      const samples = await watch(s, live, started, (cell) => on(cell, "ready"), 5_000);
      const last = samples[samples.length - 1];
      expect(on(last, "ready") && still(last) && off(last, "part") && off(last, "start"),
        `Ready on again once the part is out, the valves and sensors off: ${listed(samples)}`);
      return { seconds: last.seconds, samples: listed(samples) };
    });
    assertNoFailedStep(s);
  });
});
