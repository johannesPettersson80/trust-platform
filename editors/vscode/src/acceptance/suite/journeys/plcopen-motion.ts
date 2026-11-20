// The plcopen-motion-single-axis walkthrough (specification 25, sections 3.5 and 4; PLCopen Motion Control part 1): on the
// unmodified example, Start runs the program on the Simulator and opens Live Values. MC_Power enables the virtual axis and
// MC_MoveAbsolute moves it to 25: power on, standstill, at the target, one move counted, no error. Writing TRUE to the
// move request (%IX0.0) moves the axis to the other end, 0, because a request's rising edge starts the next move; writing
// TRUE again, while it is TRUE, starts nothing; FALSE and then TRUE move it back to 25. Each step reads Live Values and
// records what it shows; the README's walkthrough quotes these values. A step that does not show its result fails the
// journey.
import * as cdp from "../cdp";
import { assertNoFailedStep, expect, open, Session, step } from "../session";
import { startSimulator } from "../studio";

/** The first row of `address` that offers actions, else the first row of it: `row(a)` in the scripts below. */
const rowOf = String.raw`var rows=[...d.querySelectorAll('.row')];function row(a){var all=rows.filter(function(r){return (r.dataset.rowKey||'').split('|')[1]===a;});return all.find(function(r){return r.querySelector('.actions');})||all[0];}`;

/** The axis's signals as Live Values shows them, by address. */
const signals = rowOf + String.raw`function v(a){var r=row(a);if(!r)return null;var c=r.querySelector('.value');return c?c.innerText.trim():'';}return JSON.stringify({move:v('%IX0.0'),powerOn:v('%QX0.0'),targetOk:v('%QX0.1'),standstill:v('%QX0.2'),cycles:v('%MW0'),error:v('%MW2'),position:v('%MD4')});`;

/** On the move request's row, chooses `value` and presses Write, as the user does. */
const write = (value: "TRUE" | "FALSE") => rowOf +
  `var r=row('%IX0.0');if(!r)return 'NO_ROW';var t=r.querySelector('.bool-toggle');if(!t)return 'NO_CHOOSER';if(t.value!=='${value}')t.click();var w=[...r.querySelectorAll('button')].find(function(b){return b.innerText.trim()==='Write';});if(!w)return 'NO_WRITE';if(w.disabled)return 'DISABLED';w.click();return 'CLICKED';`;

type Signals = { move: string; powerOn: string; targetOk: string; standstill: string; cycles: string; error: string; position: string };

suite("PLCopen Motion: a single axis in Live Values", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "plcopen-motion"); });

  test("powers the axis, moves it on each rising edge of the request, and not on a held one", async () => {
    let live = "";
    const read = async () => JSON.parse(await cdp.evalWebview(s.wb, live, signals)) as Signals;
    /** Reads until `holds` or `ms` pass; gives the last reading. */
    const until = async (holds: (shown: Signals) => boolean, ms: number) => {
      const deadline = Date.now() + ms;
      let shown = await read();
      while (!holds(shown) && Date.now() < deadline) {
        await cdp.sleep(250);
        shown = await read();
      }
      return shown;
    };
    const pressWrite = async (value: "TRUE" | "FALSE") => {
      const pressed = await cdp.evalWebview(s.wb, live, write(value));
      expect(pressed === "CLICKED", `Write ${value} on the move request's row: ${pressed}`);
    };

    await step(s, "started", () => startSimulator(s));
    await step(s, "moved-to-25", async () => {
      // Live Values opens with Start; its webview is the one with the force policy banner.
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      const shown = await until((x) => x.cycles === "1" && x.targetOk === "TRUE", 30_000);
      expect(shown.powerOn === "TRUE", `MC_Power powers the axis: ${JSON.stringify(shown)}`);
      expect(shown.standstill === "TRUE" && shown.targetOk === "TRUE" && Number(shown.position) === 25,
        `the axis stands at 25: ${JSON.stringify(shown)}`);
      expect(shown.cycles === "1" && shown.error === "0", `one move counted, no error: ${JSON.stringify(shown)}`);
      return { shown };
    });
    await step(s, "moved-to-0", async () => {
      await pressWrite("TRUE");
      const shown = await until((x) => x.cycles === "2", 15_000);
      expect(shown.cycles === "2" && Number(shown.position) === 0, `the request's rising edge moves the axis to 0: ${JSON.stringify(shown)}`);
      expect(shown.standstill === "TRUE" && shown.targetOk === "TRUE", `it stands at its new target: ${JSON.stringify(shown)}`);
      return { shown };
    });
    await step(s, "held-request", async () => {
      // The request is TRUE already: writing TRUE again is no rising edge, so no move starts.
      await pressWrite("TRUE");
      await cdp.sleep(3_000);
      const shown = await read();
      expect(shown.cycles === "2" && Number(shown.position) === 0, `a held request starts nothing: ${JSON.stringify(shown)}`);
      return { shown };
    });
    await step(s, "moved-back", async () => {
      await pressWrite("FALSE");
      await until((x) => x.move === "FALSE", 10_000);
      await pressWrite("TRUE");
      const shown = await until((x) => x.cycles === "3", 15_000);
      expect(shown.cycles === "3" && Number(shown.position) === 25, `FALSE and then TRUE move the axis back to 25: ${JSON.stringify(shown)}`);
      return { shown };
    });
    assertNoFailedStep(s);
  });
});
