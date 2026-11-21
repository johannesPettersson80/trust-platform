// The Rust signal filter and CRC walkthrough (examples/rust_signal/README.md): Start builds the project's crate with
// truST's toolchain and runs the program on the Simulator. Live Values shows the simulated sensor's reading, its
// median and its average; over one cycle of the sensor's readings the median stays between 49.9 and 50.2 while the
// average jumps near 56 at the spike and near 44 at the dip. Then a Write of the frame that comes in: 1 is trusted with
// the register value 42, 2 has a bit changed and is refused and counted with the register kept, and 1 is trusted again.
// Each step records what it shows; the README quotes the run, and a step that does not show its result fails the
// journey.
import * as cdp from "../cdp";
import { until } from "../hmi";
import { assertNoFailedStep, expect, open, pressHome, Session, step } from "../session";

/** What Live Values shows, by address. */
type Shown = Record<string, { name: string; value: string }>;

/** A script that gives every row's name and value, by the row's address. */
const rows = String.raw`var out={};[...d.querySelectorAll('.row')].forEach(function(r){var a=(r.dataset.rowKey||'').split('|')[1];if(!a)return;var n=r.querySelector('.name div'),v=r.querySelector('.value');out[a]={name:n?n.innerText.trim():'',value:v?v.innerText.trim():''};});return JSON.stringify(out);`;

/** A script that types `value` into the frame's row and presses Write, as the user does. */
const writeFrame = (value: string) => String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%IW0';});if(!r)return 'NO_ROW';var i=r.querySelector('input.value-input');if(!i)return 'NO_INPUT';i.value='${value}';i.dispatchEvent(new Event('input',{bubbles:true}));var w=[...r.querySelectorAll('button')].find(function(b){return (b.textContent||'').trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'WRITE_DISABLED';w.click();return 'CLICKED';`;

const ADDRESSES = ["%QD0", "%QD4", "%QD8", "%IW0", "%QX12.0", "%QW14", "%QW16"];

async function read(s: Session, live: string): Promise<Shown | null> {
  const shown = JSON.parse(await cdp.evalWebview(s.wb, live, rows)) as Shown;
  return ADDRESSES.every((address) => shown[address]) ? shown : null;
}

/** The frame check's outputs: FrameOk, Register and BadFrames. */
const line = (shown: Shown | null) =>
  shown ? `${shown["%QX12.0"].value.toUpperCase()} ${shown["%QW14"].value} ${shown["%QW16"].value}` : "";

suite("Rust signal filter and CRC: the median holds, the CRC refuses a changed frame", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "rust-signal"); });

  test("filters the sensor's spikes and trusts only a frame with its CRC", async () => {
    let live = "";
    await step(s, "started", async () => {
      const clicked = await pressHome(s, "^Start$");
      const running = (items: string[]) => (items || []).some((item) => /Simulator running/i.test(item));
      const status = await cdp.pollValue("the Simulator running", () => cdp.statusBar(s.wb), running, 300_000);
      expect(running(status), `the status bar says the Simulator runs: ${JSON.stringify(status)}`);
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      const shown = await until(() => read(s, live), (rowsShown) => rowsShown !== null, 30_000);
      expect(shown !== null, "Live Values lists the sensor's and the frame's rows");
      return { clicked, status, names: ADDRESSES.map((address) => `${shown?.[address]?.name} (${address})`) };
    });
    await step(s, "spike-and-dip", async () => {
      // One cycle of the twenty readings is 10 s: read four times a second for 12 s.
      const samples: Array<{ seconds: number; raw: number; level: number; mean: number }> = [];
      const started = Date.now();
      while (Date.now() - started < 12_000) {
        const shown = await read(s, live);
        if (shown) {
          samples.push({
            seconds: Math.round((Date.now() - started) / 100) / 10,
            raw: Number(shown["%QD0"].value),
            level: Number(shown["%QD4"].value),
            mean: Number(shown["%QD8"].value),
          });
        }
        await cdp.sleep(250);
      }
      const levels = samples.map((sample) => sample.level);
      const means = samples.map((sample) => sample.mean);
      expect(levels.every((level) => level >= 49.9 && level <= 50.2), `the median stays between 49.9 and 50.2: ${levels.join(" ")}`);
      expect(Math.max(...means) >= 55.9, `the average jumps near 56 at the spike: ${means.join(" ")}`);
      expect(Math.min(...means) <= 44.1, `the average falls near 44 at the dip: ${means.join(" ")}`);
      return { highest: Math.max(...means), lowest: Math.min(...means), raws: [...new Set(samples.map((sample) => sample.raw))], samples };
    });
    for (const [frame, expected, why] of [
      ["1", "TRUE 42 0", "the frame as sent: trusted"],
      ["2", "FALSE 42 1", "a bit changed: refused, counted, the register kept"],
      ["1", "TRUE 42 1", "the frame as sent again: trusted"],
    ]) {
      await step(s, `frame-${frame}-${expected.split(" ")[2]}`, async () => {
        const wrote = await cdp.evalWebview(s.wb, live, writeFrame(frame));
        expect(wrote === "CLICKED", `Write ${frame} on the frame's row: ${wrote}`);
        const shown = await until(() => read(s, live), (rowsShown) => rowsShown?.["%IW0"].value === frame && line(rowsShown) === expected, 10_000);
        expect(line(shown) === expected, `frame ${frame}, ${why}: ${line(shown)}`);
        return { frame, line: line(shown) };
      });
    }
    assertNoFailedStep(s);
  });
});
