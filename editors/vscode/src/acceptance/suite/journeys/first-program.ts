// The empty-simulator walkthrough, "Your first program" (specification 25, sections 3.5 and 4): in the copy of the empty
// starter, the user writes a counter into src/Main.st and saves it, presses Start, and sees the counter count in Live Values;
// then changes the step and starts again. The counter sits at %MD0 because Live Values lists the values that have an address
// (specification 25, 4.3). The task runs every 10 ms, so the count grows by about 100 a second, and by about 1000 with a step
// of 10. Each step records what it shows; the README's walkthrough quotes these values. A step that does not show its result
// fails the journey.
import * as path from "path";
import * as vscode from "vscode";

import * as cdp from "../cdp";
import { assertNoFailedStep, expect, open, pressHome, Session, step } from "../session";
import { startSimulator } from "../studio";

/** The program the README has the user write, with the step `increment`. */
const program = (increment: number) =>
  ["PROGRAM Main", "VAR", "    Count AT %MD0 : DINT;", "END_VAR", `    Count := Count + ${increment};`, "END_PROGRAM", ""].join("\n");

/** The counter's row in Live Values: its name and value. */
const counter = String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%MD0';});if(!r)return 'null';var n=r.querySelector('.name div'),v=r.querySelector('.value');return JSON.stringify({name:n?n.innerText.trim():'',value:v?v.innerText.trim():''});`;

type Row = { name: string; value: string } | null;

suite("Your first program: write a counter, start it, watch it count", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "first-program"); });

  test("writes the counter, runs it in the Simulator and changes its step", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
    const main = vscode.Uri.file(path.join(folder, "src", "Main.st"));
    let live = "";

    /** Replaces Main.st's text in its editor and saves it, as the user's edit does. */
    const writeProgram = async (increment: number) => {
      const document = await vscode.workspace.openTextDocument(main);
      const editor = await vscode.window.showTextDocument(document, { preview: false });
      const whole = new vscode.Range(document.positionAt(0), document.positionAt(document.getText().length));
      const edited = await editor.edit((builder) => builder.replace(whole, program(increment)));
      expect(edited, "the editor takes the program");
      expect(await document.save(), "Main.st is saved");
      return document.getText();
    };
    /** Reads the counter every half second for `ms`. */
    const trend = async (ms: number) => {
      const samples: Array<{ seconds: number; value: number; name: string }> = [];
      const started = Date.now();
      while (Date.now() - started < ms) {
        const row = JSON.parse(await cdp.evalWebview(s.wb, live, counter)) as Row;
        samples.push({ seconds: Math.round((Date.now() - started) / 100) / 10, value: row ? Number(row.value) : Number.NaN, name: row?.name ?? "" });
        await cdp.sleep(500);
      }
      return samples;
    };

    await step(s, "written", async () => {
      const text = await writeProgram(1);
      expect(text.includes("Count AT %MD0 : DINT;"), `Main.st holds the counter: ${text}`);
      return { text };
    });
    await step(s, "counting", async () => {
      const started = await startSimulator(s);
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      await cdp.pollValue("the counter's row", () => cdp.evalWebview(s.wb, live, counter), (text) => text !== "null", 30_000);
      const samples = await trend(3_000);
      const counted = samples[samples.length - 1].value - samples[0].value;
      // One count per 10 ms scan: about 250 in the 2.5 s between the first reading and the last.
      expect(counted >= 150 && counted <= 350, `the counter grows by about 100 a second: ${samples.map((x) => x.value).join(" ")}`);
      return { status: started.status, name: samples[0].name, samples, counted };
    });
    await step(s, "stopped", async () => {
      const pressed = await pressHome(s, "^Stop$");
      const status = await cdp.pollValue("the Simulator stopped", () => cdp.statusBar(s.wb),
        (items) => (items || []).some((item) => /Simulator stopped/i.test(item)), 60_000);
      return { pressed, status };
    });
    await step(s, "step-of-ten", async () => {
      const text = await writeProgram(10);
      expect(text.includes("Count := Count + 10;"), `Main.st adds 10: ${text}`);
      await startSimulator(s);
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      await cdp.pollValue("the counter's row", () => cdp.evalWebview(s.wb, live, counter), (row) => row !== "null", 30_000);
      const samples = await trend(3_000);
      const counted = samples[samples.length - 1].value - samples[0].value;
      // Ten per scan: about 2500 in 2.5 s; every reading is a multiple of 10, since the count starts at 0 again.
      expect(counted >= 1_500 && counted <= 3_500, `the counter grows by about 1000 a second: ${samples.map((x) => x.value).join(" ")}`);
      expect(samples.every((sample) => sample.value % 10 === 0), `every reading is a multiple of 10: ${samples.map((x) => x.value).join(" ")}`);
      return { samples, counted };
    });
    assertNoFailedStep(s);
  });
});
