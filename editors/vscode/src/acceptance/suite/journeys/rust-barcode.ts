// The Rust barcode reader walkthrough (examples/rust_barcode/README.md): Start builds the project's crate with truST's
// toolchain and runs the program on the Simulator; Live Values shows the label the simulated scanner reads, at %IW0, and
// the line's verdict, at %QX0.0 to %QX0.3 and %QW2. A Write of each label's number gives the README's verdict: accepted,
// expired, unreadable with no problem from the reader, and another product. Each step records what it shows; the README
// quotes the run, and a step that does not show its result fails the journey.
import * as cdp from "../cdp";
import { until } from "../hmi";
import { assertNoFailedStep, expect, open, pressHome, Session, step } from "../session";

/** The line's verdict as Live Values shows it, and the label it is for. */
type Verdict = { label: string; accepted: string; expired: string; unreadable: string; wrong: string; problem: string; names: string[] };

/** A script that gives every row's name and value, by the row's address. */
const rows = String.raw`var out={};[...d.querySelectorAll('.row')].forEach(function(r){var a=(r.dataset.rowKey||'').split('|')[1];if(!a)return;var n=r.querySelector('.name div'),v=r.querySelector('.value');out[a]={name:n?n.innerText.trim():'',value:v?v.innerText.trim():''};});return JSON.stringify(out);`;

/** A script that types `value` into the label's row and presses Write, as the user does. */
const writeLabel = (value: string) => String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%IW0';});if(!r)return 'NO_ROW';var i=r.querySelector('input.value-input');if(!i)return 'NO_INPUT';i.value='${value}';i.dispatchEvent(new Event('input',{bubbles:true}));var w=[...r.querySelectorAll('button')].find(function(b){return (b.textContent||'').trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'WRITE_DISABLED';w.click();return 'CLICKED';`;

/** The verdict in Live Values, or null while its rows are missing. */
async function read(s: Session, live: string): Promise<Verdict | null> {
  const shown = JSON.parse(await cdp.evalWebview(s.wb, live, rows)) as Record<string, { name: string; value: string }>;
  const wanted = ["%IW0", "%QX0.0", "%QX0.1", "%QX0.2", "%QX0.3", "%QW2"];
  if (wanted.some((address) => !shown[address])) return null;
  return {
    label: shown["%IW0"].value,
    accepted: shown["%QX0.0"].value,
    expired: shown["%QX0.1"].value,
    unreadable: shown["%QX0.2"].value,
    wrong: shown["%QX0.3"].value,
    problem: shown["%QW2"].value,
    names: wanted.map((address) => `${shown[address].name} (${address})`),
  };
}

/** The four flags, TRUE or FALSE, in the order accepted, expired, unreadable, wrong product. */
const flags = (verdict: Verdict | null) =>
  verdict ? [verdict.accepted, verdict.expired, verdict.unreadable, verdict.wrong].map((flag) => flag.toUpperCase()).join(" ") : "";

suite("Rust barcode reader: the line's verdict on each label", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "rust-barcode"); });

  test("reads each label in Rust and gives its verdict", async () => {
    let live = "";
    await step(s, "started", async () => {
      const clicked = await pressHome(s, "^Start$");
      const running = (items: string[]) => (items || []).some((item) => /Simulator running/i.test(item));
      const status = await cdp.pollValue("the Simulator running", () => cdp.statusBar(s.wb), running, 300_000);
      expect(running(status), `the status bar says the Simulator runs: ${JSON.stringify(status)}`);
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      const shown = await until(() => read(s, live), (verdict) => verdict !== null, 30_000);
      expect(shown?.label === "0" && flags(shown) === "FALSE FALSE FALSE FALSE" && shown.problem === "0",
        `no label, no verdict: ${JSON.stringify(shown)}`);
      return { clicked, status, shown };
    });
    for (const [label, expected, why] of [
      ["1", "TRUE FALSE FALSE FALSE", "accepted: the line's product, in date"],
      ["2", "FALSE TRUE FALSE FALSE", "expired: the end of June 2025"],
      ["3", "FALSE FALSE TRUE FALSE", "unreadable: a check digit that is wrong"],
      ["4", "FALSE FALSE FALSE TRUE", "another product"],
    ]) {
      await step(s, `label-${label}`, async () => {
        const wrote = await cdp.evalWebview(s.wb, live, writeLabel(label));
        expect(wrote === "CLICKED", `Write ${label} on the label's row: ${wrote}`);
        const shown = await until(() => read(s, live), (verdict) => verdict?.label === label && flags(verdict) === expected, 10_000);
        expect(flags(shown) === expected && shown?.problem === "0", `label ${label}, ${why}: ${JSON.stringify(shown)}`);
        return { shown };
      });
    }
    assertNoFailedStep(s);
  });
});
