// The Rust function block walkthrough (examples/rust_function_block/README.md; specification 48 RU-88): Start builds the
// project's crate with truST's own toolchain, with no Rust on PATH, and runs the program on the Simulator. Live Values
// shows the setpoint input and the ramp's outputs by their addresses, and a Write of 50.0 and then of 20.0 to the
// setpoint moves the speed by 1.0 a run of the 100 ms task until it reaches it. Each step records what it shows; the
// README quotes the run, and a step that does not show its result fails the journey.
import * as cdp from "../cdp";
import { until } from "../hmi";
import { assertNoFailedStep, expect, open, pressHome, Session, step } from "../session";

/** The ramp as Live Values shows it: the setpoint, the speed and whether it is at speed. */
type Ramp = { setpoint: number; speed: number; atSpeed: string; names: string[] };

/** A reading of the ramp, `seconds` after the Write. */
type Reading = { seconds: number; speed: number; atSpeed: string };

/** A script that gives every row's name and value, by the row's address. */
const rows = String.raw`var out={};[...d.querySelectorAll('.row')].forEach(function(r){var a=(r.dataset.rowKey||'').split('|')[1];if(!a)return;var n=r.querySelector('.name div'),v=r.querySelector('.value');out[a]={name:n?n.innerText.trim():'',value:v?v.innerText.trim():''};});return JSON.stringify(out);`;

/** A script that types `value` into the setpoint's row and presses Write, as the user does. */
const writeSetpoint = (value: string) => String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%IL0';});if(!r)return 'NO_ROW';var i=r.querySelector('input.value-input');if(!i)return 'NO_INPUT';i.value='${value}';i.dispatchEvent(new Event('input',{bubbles:true}));var w=[...r.querySelectorAll('button')].find(function(b){return (b.textContent||'').trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'WRITE_DISABLED';w.click();return 'CLICKED';`;

/** The ramp in Live Values, or null while its rows are missing. */
async function read(s: Session, live: string): Promise<Ramp | null> {
  const shown = JSON.parse(await cdp.evalWebview(s.wb, live, rows)) as Record<string, { name: string; value: string }>;
  const wanted = ["%IL0", "%QL0", "%QX8.0"];
  if (wanted.some((address) => !shown[address])) return null;
  return {
    setpoint: Number(shown["%IL0"].value),
    speed: Number(shown["%QL0"].value),
    atSpeed: shown["%QX8.0"].value,
    names: wanted.map((address) => `${shown[address].name} (${address})`),
  };
}

/** Writes `value` to the setpoint, then reads the ramp every 250 ms until it is at `value` or `ms` pass. */
async function rampTo(s: Session, live: string, value: number, ms: number): Promise<Reading[]> {
  const wrote = await cdp.evalWebview(s.wb, live, writeSetpoint(value.toFixed(1)));
  expect(wrote === "CLICKED", `Write ${value.toFixed(1)} on the setpoint's row: ${wrote}`);
  const started = Date.now();
  const readings: Reading[] = [];
  while (Date.now() - started < ms) {
    const ramp = await read(s, live);
    readings.push({ seconds: Math.round((Date.now() - started) / 100) / 10, speed: ramp?.speed ?? Number.NaN, atSpeed: ramp?.atSpeed ?? "" });
    if (ramp?.speed === value && /true/i.test(ramp.atSpeed)) break;
    await cdp.sleep(250);
  }
  return readings;
}

/** The seconds from the first reading that left `from` to the first that reached `to`. */
function travel(readings: Reading[], from: number, to: number): number {
  const left = readings.find((reading) => reading.speed !== from);
  const reached = readings.find((reading) => reading.speed === to);
  return left && reached ? reached.seconds - left.seconds : Number.NaN;
}

suite("Rust function block: RAMP moves the speed to the setpoint you write", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "rust-function-block"); });

  test("builds the crate with truST's toolchain and ramps to each setpoint written", async () => {
    let live = "";
    await step(s, "started", async () => {
      const clicked = await pressHome(s, "^Start$");
      // The first build of the project builds the SDK too, with truST's toolchain: nothing of Rust is on PATH.
      const running = (items: string[]) => (items || []).some((item) => /Simulator running/i.test(item));
      const status = await cdp.pollValue("the Simulator running", () => cdp.statusBar(s.wb), running, 300_000);
      expect(running(status), `the status bar says the Simulator runs: ${JSON.stringify(status)}`);
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      const shown = await until(() => read(s, live), (ramp) => ramp !== null, 30_000);
      expect(shown?.setpoint === 0 && shown.speed === 0 && /true/i.test(shown.atSpeed),
        `the setpoint 0.0, the speed 0.0 and at speed: ${JSON.stringify(shown)}`);
      return { clicked, status, shown };
    });
    await step(s, "climbs-to-50", async () => {
      const readings = await rampTo(s, live, 50, 15_000);
      const last = readings[readings.length - 1];
      const seconds = travel(readings, 0, 50);
      // 1.0 a run of the 100 ms task: 4.9 s from the first run with the new setpoint to the 50th, read within Live
      // Values' refresh, about twice a second.
      expect(last.speed === 50 && /true/i.test(last.atSpeed), `the speed reaches 50.0 and is at speed: ${JSON.stringify(readings)}`);
      expect(readings.some((reading) => reading.speed > 0 && reading.speed < 50 && /false/i.test(reading.atSpeed)),
        `not at speed while it climbs: ${JSON.stringify(readings)}`);
      expect(seconds >= 3.5 && seconds <= 6.5, `about 4.9 s from 0 to 50: ${seconds} s, ${JSON.stringify(readings)}`);
      return { seconds, readings };
    });
    await step(s, "falls-to-20", async () => {
      const readings = await rampTo(s, live, 20, 12_000);
      const last = readings[readings.length - 1];
      const seconds = travel(readings, 50, 20);
      // 30 runs down, the 30th 2.9 s after the first.
      expect(last.speed === 20 && /true/i.test(last.atSpeed), `the speed reaches 20.0 and is at speed: ${JSON.stringify(readings)}`);
      expect(seconds >= 1.8 && seconds <= 4.2, `about 2.9 s from 50 to 20: ${seconds} s, ${JSON.stringify(readings)}`);
      return { seconds, readings };
    });
    assertNoFailedStep(s);
  });
});
