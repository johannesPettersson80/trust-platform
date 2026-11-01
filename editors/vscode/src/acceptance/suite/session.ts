// The shared steps of the acceptance journeys: activation, the truST side panel, pressing buttons in a webview as a user does,
// waiting for the screen that opens, and recording each step (its result, any question or notification, the truST status and
// a screenshot) in the session's journal (specification 22, "Acceptance journeys on the packaged extension").
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { setWebviewValue } from "../../test/suite/plant-rendered-harness";
import * as cdp from "./cdp";

/** Where the session keeps its journal and screenshots, and the profile of its window. */
const SHOTS = required("E2E_SHOTS");
const USER_DATA = required("E2E_UD");

function required(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is set by the acceptance runner`);
  return value;
}

type Outcome = "ok" | "fail" | "info";

/** What a step reports: an `outcome` of its own (`info` for a step that only records), or its result. */
export type StepResult = { outcome?: Outcome; [detail: string]: unknown } | unknown;

type Entry = { step: string; outcome: Outcome; [detail: string]: unknown };

export type Session = {
  wb: cdp.Workbench;
  home: string | undefined;
  name: string;
  entries: Entry[];
  shots: number;
};

/** Opens the session: the truST extension under test, the workbench and the side panel. With `welcome`, the side panel is the
 * one shown without a truST project (Start from example, Create project or Initialize, Open project). */
export async function open(name: string, { welcome = false } = {}): Promise<Session> {
  const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
  if (!extension) throw new Error("the truST extension is not installed");
  await extension.activate();
  const wb = await cdp.workbench(USER_DATA);
  await vscode.commands.executeCommand("workbench.view.extension.trust");
  const marker = welcome
    ? "d.body && /Start from example/.test(d.body.innerText)"
    : "d.body && /TARGET/.test(d.body.innerText) && [...d.querySelectorAll('button')].some(function (b) { return /Compile/.test(b.innerText); })";
  const home = await cdp.attachWebview(wb, "truST side panel", marker, 60_000).catch(() => undefined);
  await cdp.sleep(1_500);
  return { wb, home, name, entries: [], shots: 0 };
}

/** Records a step: its outcome, its details and a screenshot, in the journal `<session>.json`. */
async function record(session: Session, step: string, outcome: Outcome, details: Record<string, unknown>): Promise<void> {
  session.shots += 1;
  const file = path.join(SHOTS, `${session.name}-${String(session.shots).padStart(2, "0")}-${step}.png`);
  try {
    await cdp.screenshot(session.wb, file);
  } catch (error) {
    details.screenshotError = (error as Error).message;
  }
  session.entries.push({
    step,
    outcome,
    ...details,
    question: await cdp.quickInput(session.wb).catch(() => undefined),
    notifications: await cdp.notifications(session.wb).catch(() => []),
    status: (await cdp.statusBar(session.wb).catch(() => [] as string[])).filter((item) => /truST|Simulator|Runtime|Debug/i.test(item)),
    shot: path.basename(file),
  });
  fs.writeFileSync(path.join(SHOTS, `${session.name}.json`), JSON.stringify(session.entries, null, 2));
}

/** Runs `body` as the step `name`. A thrown error is recorded as the step's failure and the journey goes on, so that one run
 * shows every step that fails. */
export async function step(session: Session, name: string, body: () => Promise<StepResult>): Promise<void> {
  try {
    const result = await body();
    const own = result && typeof result === "object" && "outcome" in result ? (result as { outcome: Outcome }) : undefined;
    await record(session, name, own?.outcome ?? "ok", own ? { ...own } : { result });
  } catch (error) {
    await record(session, name, "fail", { error: (error as Error).message });
  }
}

/** Fails the journey when one of its steps failed, naming each. */
export function assertNoFailedStep(session: Session): void {
  const failed = session.entries.filter((entry) => entry.outcome === "fail");
  if (failed.length) {
    throw new Error(`failed steps: ${failed.map((entry) => `${entry.step}: ${String(entry.error)}`).join(" | ")}`);
  }
}

/** Fails the step with `what` unless `holds`. */
export function expect(holds: unknown, what: string): void {
  if (!holds) throw new Error(what);
}

/** Presses the side panel's button matching `pattern`. */
export async function pressHome(session: Session, pattern: string): Promise<string> {
  const home = session.home;
  if (!home) throw new Error("the truST side panel did not load");
  // The panel renders again after an edit (its Compile button carries the error count meanwhile).
  let clicked = "";
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    clicked = await cdp.clickButton(session.wb, home, pattern).catch((error: Error) => error.message);
    if (clicked.startsWith("CLICKED")) return clicked;
    await cdp.sleep(500);
  }
  throw new Error(`side panel button ${pattern}: ${clicked}`);
}

/** Opens a screen with `opener` and attaches to the webview it opens; gives the webview's session. */
export async function openScreen(session: Session, label: string, opener: () => Promise<unknown>, timeoutMs = 30_000): Promise<string> {
  const before = await cdp.webviewTargetIds(session.wb);
  await opener();
  const id = await cdp.attachNewWebview(session.wb, label, before, timeoutMs);
  await cdp.sleep(2_500);
  return id;
}

/** The visible texts of a webview (`cdp.describeWebview`). */
export async function describe(session: Session, webview: string): Promise<{ body: string; [part: string]: unknown }> {
  return JSON.parse(await cdp.describeWebview(session.wb, webview));
}

/** Clicks the webview's button matching `pattern`, or fails the step. */
export async function click(session: Session, webview: string, pattern: string): Promise<string> {
  const result = await cdp.clickButton(session.wb, webview, pattern);
  if (!result.startsWith("CLICKED")) throw new Error(`button ${pattern}: ${result}`);
  return result;
}

/** Waits until the webview's text matches `pattern`. */
export function waitText(session: Session, webview: string, pattern: string, timeoutMs = 30_000): Promise<string> {
  return cdp.pollValue(`text ${pattern}`, () => cdp.evalWebview(session.wb, webview, "return d.body.innerText;"),
    (text) => new RegExp(pattern, "i").test(text || ""), timeoutMs);
}

/** Sets the webview's input found by `selector` as typing does; gives the value it holds then. */
export function setValue(session: Session, webview: string, selector: string, value: string): Promise<string> {
  return setWebviewValue((body) => cdp.evalWebview(session.wb, webview, body), selector, value);
}
