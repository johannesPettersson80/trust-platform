// The conveyor walkthrough (specification 25, sections 3.5 and 4): on the unmodified example, Start runs the program on the
// Simulator and opens Live Values. The line runs: `running` (%QX2.0) is TRUE, the speed on %QW0 rises by 5 each 50 ms cycle up
// to 100, and on the cycle after it reaches 100 the part count on %MW0 goes up by one and the speed starts again from 0, a part
// each 21 cycles (1.05 s). Forcing the stop input `e_stop` (%IX0.0) TRUE stops the line: running FALSE, speed 0, the count
// held. Release removes the force without writing a value back (specification 11, 6.9.2), so the simulated input, which no
// wiring drives, stays TRUE and the line stays stopped; writing FALSE starts it again. Each step reads Live Values every half
// second and records what it shows; the README's walkthrough quotes these values. A step that does not show its result fails
// the journey.
import * as cdp from "../cdp";
import { assertNoFailedStep, expect, open, Session, step } from "../session";
import { startSimulator } from "../studio";

/** The first row of `address` that offers actions, else the first row of it: `row(a)` in the scripts below. */
const rowOf = String.raw`var rows=[...d.querySelectorAll('.row')];function row(a){var all=rows.filter(function(r){return (r.dataset.rowKey||'').split('|')[1]===a;});return all.find(function(r){return r.querySelector('.actions');})||all[0];}`;

/** The four signals as Live Values shows them, by address: each row's name, value and state (`live` or `FORCED`). */
const signals = rowOf + String.raw`function read(a){var r=row(a);if(!r)return null;var n=r.querySelector('.name div'),v=r.querySelector('.value'),s=r.querySelector('.state-badge');return {name:n?n.innerText.trim():'',value:v?v.innerText.trim():'',state:s?s.innerText.trim():''};}return JSON.stringify({stop:read('%IX0.0'),running:read('%QX2.0'),speed:read('%QW0'),parts:read('%MW0')});`;

/** On the stop input's row, chooses TRUE and presses Force, as the user does. */
const forceStop = rowOf + String.raw`var r=row('%IX0.0');if(!r)return 'NO_ROW';var t=r.querySelector('.bool-toggle');if(!t)return 'NO_CHOOSER';if(t.value!=='TRUE')t.click();var f=[...r.querySelectorAll('button')].find(function(b){return b.innerText.trim()==='Force';});if(!f)return 'NO_FORCE';if(f.disabled)return 'DISABLED';f.click();return 'CLICKED';`;

/** On the stop input's row, presses Release. */
const releaseStop = rowOf + String.raw`var r=row('%IX0.0');if(!r)return 'NO_ROW';var b=[...r.querySelectorAll('button')].find(function(x){return x.innerText.trim()==='Release';});if(!b)return 'NO_RELEASE';if(b.disabled)return 'DISABLED';b.click();return 'CLICKED';`;

/** On the stop input's row, chooses FALSE and presses Write. */
const writeGo = rowOf + String.raw`var r=row('%IX0.0');if(!r)return 'NO_ROW';var t=r.querySelector('.bool-toggle');if(!t)return 'NO_CHOOSER';if(t.value!=='FALSE')t.click();var w=[...r.querySelectorAll('button')].find(function(b){return b.innerText.trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'DISABLED';w.click();return 'CLICKED';`;

type Row = { name: string; value: string; state: string } | null;
type Signals = { stop: Row; running: Row; speed: Row; parts: Row };
type Sample = Signals & { seconds: number };

async function read(s: Session, live: string): Promise<Signals> {
  return JSON.parse(await cdp.evalWebview(s.wb, live, signals));
}

/** Waits until Live Values shows `holds`, at most `ms`. */
async function until(s: Session, live: string, what: string, holds: (shown: Signals) => boolean, ms: number): Promise<void> {
  await cdp.pollValue(what, () => cdp.evalWebview(s.wb, live, signals), (text) => holds(JSON.parse(text || "{}")), ms);
}

/** Reads the signals every half second for `ms`; each sample carries its seconds since the first. */
async function trend(s: Session, live: string, ms: number): Promise<Sample[]> {
  const samples: Sample[] = [];
  const started = Date.now();
  while (Date.now() - started < ms) {
    samples.push({ seconds: Math.round((Date.now() - started) / 100) / 10, ...(await read(s, live)) });
    await cdp.sleep(500);
  }
  return samples;
}

const value = (row: Row) => row?.value ?? "";
const number = (row: Row) => (row ? Number(row.value) : Number.NaN);
const listed = (samples: Sample[], pick: (sample: Sample) => Row) => samples.map((sample) => value(pick(sample))).join(" ");

suite("The conveyor: a cyclic program in Live Values", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "conveyor"); });

  test("runs, stops on the forced stop input, stays stopped on Release and runs again when FALSE is written", async () => {
    let live = "";
    await step(s, "started", () => startSimulator(s));
    await step(s, "line-running", async () => {
      // Live Values opens with Start; its webview is the one with the force policy banner.
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      await until(s, live, "the conveyor's rows", (shown) => Boolean(shown.running && shown.speed && shown.parts && shown.stop), 30_000);
      const samples = await trend(s, live, 6_000);
      const speeds = samples.map((sample) => number(sample.speed));
      const parts = samples.map((sample) => number(sample.parts));
      expect(samples.every((sample) => value(sample.running) === "TRUE"), `running is TRUE while the line runs: ${listed(samples, (x) => x.running)}`);
      expect(samples.every((sample) => value(sample.stop) === "FALSE"), `the stop input is FALSE: ${listed(samples, (x) => x.stop)}`);
      expect(speeds.every((speed) => Number.isInteger(speed) && speed % 5 === 0 && speed >= 0 && speed <= 100),
        `the speed moves in steps of 5 from 0 to 100: ${speeds.join(" ")}`);
      expect(new Set(speeds).size > 1, `the speed changes: ${speeds.join(" ")}`);
      // A part each 21 cycles of 50 ms (1.05 s): about five in the 5.5 s between the first sample and the last.
      const counted = parts[parts.length - 1] - parts[0];
      expect(counted >= 4 && counted <= 7, `about one part a second: ${parts.join(" ")}`);
      return { samples, counted };
    });
    await step(s, "stopped", async () => {
      const pressed = await cdp.evalWebview(s.wb, live, forceStop);
      expect(pressed === "CLICKED", `Force on the stop input's row: ${pressed}`);
      await until(s, live, "the line stopped", (shown) => value(shown.running) === "FALSE", 15_000);
      const samples = await trend(s, live, 3_000);
      expect(samples.every((sample) => value(sample.stop) === "TRUE" && sample.stop?.state === "FORCED"),
        `the stop input is forced TRUE: ${samples.map((x) => `${value(x.stop)} ${x.stop?.state}`).join(", ")}`);
      expect(samples.every((sample) => value(sample.running) === "FALSE"), `running is FALSE: ${listed(samples, (x) => x.running)}`);
      expect(samples.every((sample) => number(sample.speed) === 0), `the speed is 0: ${listed(samples, (x) => x.speed)}`);
      expect(new Set(samples.map((sample) => value(sample.parts))).size === 1, `the count holds: ${listed(samples, (x) => x.parts)}`);
      return { pressed, samples };
    });
    await step(s, "released", async () => {
      const pressed = await cdp.evalWebview(s.wb, live, releaseStop);
      expect(pressed === "CLICKED", `Release on the stop input's row: ${pressed}`);
      await until(s, live, "the force released", (shown) => Boolean(shown.stop) && shown.stop?.state !== "FORCED", 15_000);
      const samples = await trend(s, live, 3_000);
      expect(samples.every((sample) => sample.stop?.state !== "FORCED"), `the stop input is no longer forced: ${samples.map((x) => x.stop?.state).join(" ")}`);
      expect(samples.every((sample) => value(sample.stop) === "TRUE"), `Release writes no value back: the stop input stays TRUE: ${listed(samples, (x) => x.stop)}`);
      expect(samples.every((sample) => value(sample.running) === "FALSE" && number(sample.speed) === 0),
        `the line stays stopped: ${listed(samples, (x) => x.running)} / ${listed(samples, (x) => x.speed)}`);
      return { pressed, samples };
    });
    await step(s, "restarted", async () => {
      const pressed = await cdp.evalWebview(s.wb, live, writeGo);
      expect(pressed === "CLICKED", `Write FALSE on the stop input's row: ${pressed}`);
      await until(s, live, "the line running again", (shown) => value(shown.running) === "TRUE", 15_000);
      const samples = await trend(s, live, 4_000);
      const parts = samples.map((sample) => number(sample.parts));
      expect(samples.every((sample) => value(sample.stop) === "FALSE"), `the stop input is FALSE: ${listed(samples, (x) => x.stop)}`);
      expect(samples.every((sample) => value(sample.running) === "TRUE"), `running is TRUE again: ${listed(samples, (x) => x.running)}`);
      expect(parts[parts.length - 1] > parts[0], `the count goes on: ${parts.join(" ")}`);
      return { pressed, samples };
    });
    assertNoFailedStep(s);
  });
});
