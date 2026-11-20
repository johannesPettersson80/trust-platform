// The memory_marker_counter walkthrough (specification 25, sections 3.5 and 4; specification 10, 4.7): on the unmodified
// example, Start runs the program on the Simulator and opens Live Values. `Counter` lives in the memory word %MW0: each 50 ms
// scan takes its value from %MW0, the program copies it into `CounterLatched` (%QW0) and adds 1, and the scan's end writes it
// back, so between scans %QW0 is one below %MW0 and %MW0 grows by one a scan, 20 a second. A breakpoint on
// `Counter := Counter + 1;` holds the scan before that line: the Variables show `Counter` and `CounterLatched` equal. Step
// Over runs the line: `Counter` is one more. Without the breakpoint, Continue lets the scans run again. The README's last
// Try-it: Force of 41 on %MW0 shows 41 in both words, since the force writes the marker when the scan starts and again when it
// ends (specification 44, 13.3), and after Release the count goes on from 41. Each step records what it shows; the README
// quotes these values. A step that does not show its result fails the journey.
import * as path from "path";
import * as vscode from "vscode";

import * as cdp from "../cdp";
import { assertNoFailedStep, expect, open, Session, step } from "../session";
import { startSimulator } from "../studio";

const INCREMENT = "Counter := Counter + 1;";

/** The two words as Live Values shows them, by address: each row's name and value. */
const words = String.raw`var rows=[...d.querySelectorAll('.row')];function read(a){var r=rows.find(function(x){return (x.dataset.rowKey||'').split('|')[1]===a;});if(!r)return null;var n=r.querySelector('.name div'),v=r.querySelector('.value');return {name:n?n.innerText.trim():'',value:v?v.innerText.trim():''};}return JSON.stringify({counter:read('%MW0'),latched:read('%QW0')});`;

type Row = { name: string; value: string } | null;
type Words = { counter: Row; latched: Row };
type Sample = Words & { seconds: number };

const number = (row: Row) => (row ? Number(row.value) : Number.NaN);

async function readWords(s: Session, live: string): Promise<Words> {
  return JSON.parse(await cdp.evalWebview(s.wb, live, words));
}

/** Reads the two words every half second for `ms`; each sample carries its seconds since the first. */
async function trend(s: Session, live: string, ms: number): Promise<Sample[]> {
  const samples: Sample[] = [];
  const started = Date.now();
  while (Date.now() - started < ms) {
    samples.push({ seconds: Math.round((Date.now() - started) / 100) / 10, ...(await readWords(s, live)) });
    await cdp.sleep(500);
  }
  return samples;
}

type Frame = { id: number; line: number; name: string; source?: { path?: string; name?: string } };

/** The debug session of the Simulator that Start launched. */
function simulatorSession(): vscode.DebugSession {
  const session = vscode.debug.activeDebugSession;
  if (!session || session.type !== "structured-text") throw new Error(`no truST debug session: ${session?.type ?? "none"}`);
  return session;
}

/** The top frame while the debugger holds the scan, or nothing while the program runs. */
async function topFrame(session: vscode.DebugSession): Promise<Frame | undefined> {
  const threads = (await session.customRequest("threads")) as { threads?: Array<{ id: number }> };
  const threadId = threads.threads?.[0]?.id;
  if (typeof threadId !== "number") return undefined;
  try {
    const stack = (await session.customRequest("stackTrace", { threadId, startFrame: 0, levels: 1 })) as { stackFrames?: Frame[] };
    return stack.stackFrames?.[0];
  } catch {
    // The program runs: no thread is held, so there is no stack to give.
    return undefined;
  }
}

/** Waits until the debugger holds the scan in Main.st at a line that `holds` accepts; gives the top frame. */
async function heldAt(session: vscode.DebugSession, what: string, holds: (line: number) => boolean, ms: number): Promise<Frame> {
  const deadline = Date.now() + ms;
  let frame: Frame | undefined;
  while (Date.now() < deadline) {
    frame = await topFrame(session).catch(() => undefined);
    if (frame && /Main\.st$/i.test(frame.source?.path ?? frame.source?.name ?? "") && holds(frame.line)) return frame;
    await cdp.sleep(200);
  }
  throw new Error(`${what}: the top frame is ${JSON.stringify(frame)}`);
}

/** The variables of `frame` that the Variables view lists, by name: every scope's variables. */
async function variables(session: vscode.DebugSession, frame: Frame): Promise<Record<string, string>> {
  const scopes = (await session.customRequest("scopes", { frameId: frame.id })) as { scopes?: Array<{ name: string; variablesReference: number }> };
  const found: Record<string, string> = {};
  for (const scope of scopes.scopes ?? []) {
    if (!scope.variablesReference) continue;
    const listed = (await session.customRequest("variables", { variablesReference: scope.variablesReference })) as
      { variables?: Array<{ name: string; value: string }> };
    for (const variable of listed.variables ?? []) found[`${scope.name}/${variable.name}`] = variable.value;
  }
  return found;
}

/** The value of the variable named `name` in any scope. */
function valueOf(found: Record<string, string>, name: string): number {
  const key = Object.keys(found).find((candidate) => candidate.split("/").pop() === name);
  return key === undefined ? Number.NaN : Number(found[key]);
}

/** On the row of %MW0, types 41 in its box and presses Force, as the user does. */
const force41 = String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%MW0'&&x.querySelector('.actions');});if(!r)return 'NO_ROW';var el=r.querySelector('input.value-input');if(!el)return 'NO_FIELD';var w=el.ownerDocument.defaultView;Object.getOwnPropertyDescriptor(w.HTMLInputElement.prototype,'value').set.call(el,'41');el.dispatchEvent(new w.Event('input',{bubbles:true}));var f=[...r.querySelectorAll('button')].find(function(b){return b.innerText.trim()==='Force';});if(!f)return 'NO_FORCE';if(f.disabled)return 'DISABLED';f.click();return 'CLICKED';`;

/** On the row of %MW0, presses Release. */
const release = String.raw`var r=[...d.querySelectorAll('.row')].find(function(x){return (x.dataset.rowKey||'').split('|')[1]==='%MW0'&&x.querySelector('.actions');});if(!r)return 'NO_ROW';var b=[...r.querySelectorAll('button')].find(function(x){return x.innerText.trim()==='Release';});if(!b)return 'NO_RELEASE';if(b.disabled)return 'DISABLED';b.click();return 'CLICKED';`;

/** The text of the Run and Debug view's Variables section as the user sees it. */
const variablesView = "(function(){var v=document.querySelector('.debug-variables');return v?v.innerText.slice(0,600):'';})()";

suite("Memory marker counter: inside a running program", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "memory-marker-counter"); });

  test("counts in Live Values, holds at a breakpoint, steps one statement, runs on, and holds at a forced 41", async () => {
    let live = "";
    let held: Record<string, string> = {};
    const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
    const main = vscode.Uri.file(path.join(folder, "src", "Main.st"));
    let breakpoint: vscode.SourceBreakpoint | undefined;
    let line = -1;

    await step(s, "started", () => startSimulator(s));
    await step(s, "counting", async () => {
      // Live Values opens with Start; its webview is the one with the force policy banner.
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      await cdp.pollValue("the counter's rows", () => cdp.evalWebview(s.wb, live, words),
        (text) => { const shown = JSON.parse(text || "{}") as Words; return Boolean(shown.counter && shown.latched); }, 30_000);
      const samples = await trend(s, live, 3_000);
      const counts = samples.map((sample) => number(sample.counter));
      // One per 50 ms scan: about 50 in the 2.5 s between the first sample and the last.
      const counted = counts[counts.length - 1] - counts[0];
      expect(counted >= 30 && counted <= 70, `%MW0 grows by about 20 a second: ${counts.join(" ")}`);
      expect(samples.every((sample) => number(sample.latched) === number(sample.counter) - 1),
        `%QW0 is one below %MW0 in every reading: ${samples.map((x) => `${x.counter?.value}/${x.latched?.value}`).join(" ")}`);
      return { samples, counted };
    });
    await step(s, "held", async () => {
      const document = await vscode.workspace.openTextDocument(main);
      await vscode.window.showTextDocument(document, { preview: false });
      line = document.getText().split(/\r?\n/).findIndex((text) => text.includes(INCREMENT));
      expect(line >= 0, `Main.st has the line ${INCREMENT}`);
      breakpoint = new vscode.SourceBreakpoint(new vscode.Location(main, new vscode.Position(line, 0)));
      vscode.debug.addBreakpoints([breakpoint]);
      const session = simulatorSession();
      const frame = await heldAt(session, "the breakpoint", (at) => at === line + 1, 15_000);
      held = await variables(session, frame);
      await vscode.commands.executeCommand("workbench.view.debug");
      await cdp.sleep(1_500);
      const counter = valueOf(held, "Counter");
      const latched = valueOf(held, "CounterLatched");
      expect(Number.isFinite(counter) && counter === latched, `held before the increment, Counter equals CounterLatched: ${JSON.stringify(held)}`);
      return { line: frame.line, variables: held, view: await cdp.evalPage(s.wb, variablesView).catch(() => "") };
    });
    await step(s, "stepped", async () => {
      const session = simulatorSession();
      await vscode.commands.executeCommand("workbench.action.debug.stepOver");
      const frame = await heldAt(session, "the step", (at) => at !== line + 1, 15_000);
      const after = await variables(session, frame);
      await cdp.sleep(1_000);
      const counter = valueOf(after, "Counter");
      expect(counter === valueOf(held, "Counter") + 1, `Step Over ran the increment: Counter ${valueOf(held, "Counter")} -> ${counter}`);
      expect(valueOf(after, "CounterLatched") === valueOf(held, "CounterLatched"),
        `CounterLatched keeps the scan's first value: ${JSON.stringify(after)}`);
      return { line: frame.line, variables: after, view: await cdp.evalPage(s.wb, variablesView).catch(() => "") };
    });
    await step(s, "running-again", async () => {
      if (breakpoint) vscode.debug.removeBreakpoints([breakpoint]);
      await vscode.commands.executeCommand("workbench.action.debug.continue");
      await cdp.sleep(1_500);
      const frame = await topFrame(simulatorSession()).catch(() => undefined);
      const samples = await trend(s, live, 2_500);
      const counts = samples.map((sample) => number(sample.counter));
      expect(counts[counts.length - 1] - counts[0] >= 20, `the scans run again: ${counts.join(" ")}`);
      return { frame: frame ?? null, samples };
    });
    await step(s, "forced", async () => {
      // Back to the truST view and the Live Values tab, the first of the editor group, which Start opened.
      await vscode.commands.executeCommand("workbench.view.extension.trust");
      await vscode.commands.executeCommand("workbench.action.openEditorAtIndex1");
      await cdp.sleep(1_000);
      const pressed = await cdp.evalWebview(s.wb, live, force41);
      expect(pressed === "CLICKED", `Force on the row of %MW0: ${pressed}`);
      await cdp.pollValue("both words at 41", () => cdp.evalWebview(s.wb, live, words),
        (text) => { const shown = JSON.parse(text || "{}") as Words; return number(shown.counter) === 41 && number(shown.latched) === 41; }, 15_000);
      const samples = await trend(s, live, 2_000);
      expect(samples.every((sample) => number(sample.counter) === 41 && number(sample.latched) === 41),
        `the force holds both words at 41: ${samples.map((x) => `${x.counter?.value}/${x.latched?.value}`).join(" ")}`);
      return { pressed, samples };
    });
    await step(s, "released-41", async () => {
      const pressed = await cdp.evalWebview(s.wb, live, release);
      expect(pressed === "CLICKED", `Release on the row of %MW0: ${pressed}`);
      await cdp.pollValue("the count going on", () => cdp.evalWebview(s.wb, live, words),
        (text) => number((JSON.parse(text || "{}") as Words).counter) > 41, 15_000);
      const samples = await trend(s, live, 2_000);
      const counts = samples.map((sample) => number(sample.counter));
      expect(counts[0] > 41 && counts[0] < 41 + 60, `the count goes on from 41: ${counts.join(" ")}`);
      expect(counts[counts.length - 1] > counts[0], `the count grows: ${counts.join(" ")}`);
      expect(samples.every((sample) => number(sample.latched) === number(sample.counter) - 1),
        `%QW0 is one below %MW0 again: ${samples.map((x) => `${x.counter?.value}/${x.latched?.value}`).join(" ")}`);
      return { pressed, samples };
    });
    assertNoFailedStep(s);
  });
});
