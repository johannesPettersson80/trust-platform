import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { automaticRequirementKeys, decodeAutomaticTestCriteria } from "../../controlStudio/automaticTestCriteria";
import { engineerRequirements, engineerVerificationPlan } from "../../controlStudio/engineerVerificationPlan";
import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { renderValidationResults } from "../../controlStudio/webview/threePageResults";
import { channel, pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom");

/** A one-output task with a 30 s run and a 15 s response, the same values the existing planner tests accept. */
const BASE = { referenceChange: 1, maximumError: 0.2, settlingBand: 0.1, settlingTimeSeconds: 15, durationSeconds: 30 };

/** The Requirements editor's single-output values, which the existing editor tests save without error. */
const REQUIREMENTS = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 };

function fixtureLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
}

/** The first-order transfer model of the existing scenario-duration test, with a measured output in metres. */
function planModel(): any {
  return { kind: "transfer-function", numerator: [1.5], denominator: [4, 1], delaySeconds: 0.5, revision: "model-4",
    source: "provided", validation: "validated", evidence: ["declared-fixture"],
    inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [0], outputs: [0] } };
}

function planValues(): Record<string, number> {
  return { "Main.Kp": 1, "Main.Ki": 0.2, "Main.Kd": 0 };
}

/** Runs one check on a fresh page, restoring the globals that the webview modules read. */
function withPage(markup: string, check: (root: HTMLElement, window: any) => void): void {
  const dom = new JSDOM(`<main>${markup}</main>`);
  const { window } = dom;
  const saved = {
    document: (globalThis as any).document, window: (globalThis as any).window, HTMLElement: (globalThis as any).HTMLElement,
    CustomEvent: (globalThis as any).CustomEvent, Option: (globalThis as any).Option,
  };
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, CustomEvent: window.CustomEvent, Option: window.Option });
  // jsdom has no modal dialog; the editor needs only the open state and the close event.
  window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new window.Event("close")); };
  try {
    check(window.document.querySelector("main") as HTMLElement, window);
  } finally {
    Object.assign(globalThis, saved);
    window.close();
  }
}

function control<T extends Element = HTMLElement>(scope: ParentNode, selector: string): T {
  const found = scope.querySelector<T>(selector);
  assert.ok(found, `missing ${selector}`);
  return found!;
}

function submitRequirements(window: any): void {
  const form = window.document.querySelector("[data-trust-compact-editor] form") as HTMLFormElement | null;
  assert.ok(form, "the requirements editor is open");
  form!.dispatchEvent(new window.Event("submit", { cancelable: true }));
}

/** The visible message under a field, or an empty string when the field is valid. */
function messageOf(input: HTMLElement): string {
  const notice = input.closest(".trust-field")?.querySelector<HTMLElement>(".trust-field__message");
  return notice && !notice.hidden ? notice.textContent ?? "" : "";
}

suite("Control Studio maximum overshoot requirement", () => {
  test("parses a declared overshoot limit and leaves a blank or absent one undeclared", () => {
    assert.ok(!("maximumOvershootPercent" in engineerRequirements({ ...BASE })), "an absent limit declares none");
    assert.ok(!("maximumOvershootPercent" in engineerRequirements({ ...BASE, maximumOvershootPercent: undefined })), "an undefined limit declares none");
    assert.ok(!("maximumOvershootPercent" in engineerRequirements({ ...BASE, maximumOvershootPercent: "  " })), "a blank entry declares none");
    assert.strictEqual(engineerRequirements({ ...BASE, maximumOvershootPercent: 5 }).maximumOvershootPercent, 5);
    assert.strictEqual(engineerRequirements({ ...BASE, maximumOvershootPercent: 1000 }).maximumOvershootPercent, 1000,
      "1000 % is the largest permitted limit");
  });

  test("refuses an invalid overshoot limit and names the field and its permitted range", () => {
    for (const value of [0, -1, 1000.5, Number.NaN, Number.POSITIVE_INFINITY, "5"]) {
      assert.throws(() => engineerRequirements({ ...BASE, maximumOvershootPercent: value }),
        /Maximum overshoot must be blank for no limit, or a finite value above 0 and at most 1000 % of the setpoint change\./,
        `refuses ${String(value)}`);
    }
  });

  test("names the output whose overshoot limit is invalid and keeps a valid limit with its output", () => {
    const output = (maximumOvershootPercent: unknown) => ({ referenceChange: 1, maximumError: 0.2, settlingBand: 0.1,
      settlingTimeSeconds: 15, maximumOvershootPercent });
    assert.throws(() => engineerRequirements({ ...BASE, channelRequirements: { temperature: output(0) } }),
      /Maximum overshoot for temperature must be blank for no limit/);
    const parsed = engineerRequirements({ ...BASE, channelRequirements: { temperature: output(4) } }) as any;
    assert.strictEqual(parsed.channelRequirements.temperature.maximumOvershootPercent, 4, "a valid output limit stays with its output");
  });

  test("adds one overshoot requirement to a setpoint step, scaled to its percent of the step", () => {
    const loop = pidLoop();
    const task = engineerRequirements({ ...BASE, maximumOvershootPercent: 5 });
    const requirements = engineerVerificationPlan(loop, planModel(), task, planValues()).requirements as any[];
    const overshoot = requirements.filter(item => item.kind === "overshoot");
    assert.strictEqual(overshoot.length, 1, "one overshoot requirement per setpoint step");
    assert.strictEqual(overshoot[0].id, "overshoot");
    assert.strictEqual(overshoot[0].channel, loop.signals.processVariable.symbol, "the limit bounds the measured output");
    assert.strictEqual(overshoot[0].initial, 0, "the step starts at the operating output");
    assert.strictEqual(overshoot[0].target, 1);
    assert.ok(Math.abs(overshoot[0].maximum - 0.05) < 1e-12, "5 % of a 1-unit step is 0.05 output units");
  });

  test("adds no overshoot requirement when no limit is declared or when a run applies no setpoint step", () => {
    const loop = pidLoop();
    const undeclared = engineerVerificationPlan(loop, planModel(), engineerRequirements({ ...BASE }), planValues()).requirements as any[];
    assert.ok(!undeclared.some(item => item.kind === "overshoot"), "an undeclared limit leaves no overshoot requirement");
    const regulation = engineerVerificationPlan(loop, planModel(),
      { ...engineerRequirements({ ...BASE, maximumOvershootPercent: 5 }), referenceChange: 0 }, planValues()).requirements as any[];
    assert.ok(!regulation.some(item => item.kind === "overshoot"), "a run with no setpoint change has no step to bound");
  });

  test("shows an overshoot result in percent of its setpoint change and converts its limit the same way", () => {
    withPage(`<table data-studio-validation-result><tbody></tbody></table>`, root => {
      const specification = { id: "overshoot", kind: "overshoot", channel: "Main.PV", window: { start: 0, end: 30 },
        initial: 0, target: 2, maximum: 0.1 };
      const requirements = [{ id: "overshoot", kind: "overshoot", status: "Pass", value: 0.06, limit: 0.1, unit: "degC",
        reason: "Observed measurement satisfies the declared limit." }];
      renderValidationResults(root, { status: "verified", scenarioResults: [{ id: "setpoint", result: { evaluation: {
        status: "Pass", run: { samples: [] }, specification: [specification], requirements, scope: "test" } } }] });
      const row = root.querySelector<HTMLElement>("tr[data-studio-requirement='overshoot']");
      assert.ok(row, "the overshoot result has its own row");
      const cells = Array.from(row!.querySelectorAll("td")).map(cell => cell.textContent ?? "");
      assert.match(cells[0] ?? "", /Maximum overshoot/);
      assert.deepStrictEqual(cells.slice(1), ["3 %", "≤ 5 %", "Pass"],
        "0.06 of a 2-unit step is 3 %, judged against a 5 % limit that is 0.1 output units");
    });
  });

  test("automatic criteria never generate an overshoot limit", () => {
    assert.ok(!automaticRequirementKeys.some(key => /overshoot/i.test(key)), "automatic setup fills no overshoot limit");
    assert.strictEqual(decodeAutomaticTestCriteria({ modelRevision: "r1", fields: ["maximumOvershootPercent"] }), undefined,
      "a stored automatic claim for an overshoot limit is refused");
  });

  test("the Requirements editor shows a blank Maximum overshoot row, saves it blank, and keeps other requirements on save", () => {
    withPage("", (_root, window) => {
      const commits: any[] = [];
      const open = (current: Record<string, unknown>) => editControlRequirements(fixtureLoop(), current as any, next => commits.push(next));
      const field = (key: string) => control<HTMLInputElement>(window.document, `[data-studio-requirement='${key}']`);
      const cancel = () => control<HTMLButtonElement>(window.document, "[data-studio-editor-cancel]").click();

      open({ ...REQUIREMENTS });
      const overshoot = field("maximumOvershootPercent");
      assert.strictEqual(overshoot.value, "", "an undeclared limit leaves the row empty");
      assert.strictEqual(overshoot.required, false, "a blank limit is accepted");
      assert.strictEqual(overshoot.closest(".trust-field")?.querySelector("label")?.textContent, "Maximum overshoot (% of setpoint change)");
      assert.strictEqual(overshoot.closest("details"), null, "the row is visible, not under More…");
      submitRequirements(window);
      assert.strictEqual(commits.length, 1, "a blank row saves");
      assert.ok(!("maximumOvershootPercent" in commits[0]), "a blank row declares no limit");

      open({ ...REQUIREMENTS, maximumOvershootPercent: 4 });
      assert.strictEqual(field("maximumOvershootPercent").value, "4", "a declared limit fills its row");
      field("maximumOvershootPercent").value = "-1";
      field("maximumError").value = "";
      submitRequirements(window);
      assert.strictEqual(commits.length, 1, "invalid edits are not saved");
      assert.notStrictEqual(messageOf(field("maximumOvershootPercent")), "", "the invalid limit is reported");
      assert.notStrictEqual(messageOf(field("maximumError")), "", "the invalid error limit is reported in the same pass");
      field("maximumOvershootPercent").value = "0";
      field("maximumError").value = "1";
      submitRequirements(window);
      assert.strictEqual(commits.length, 1, "a zero limit is not saved");
      assert.match(messageOf(field("maximumOvershootPercent")), /above 0/);
      cancel();

      open({ ...REQUIREMENTS, constantInputDisturbance: 0.3, maximumOvershootPercent: 4 });
      field("maximumOvershootPercent").value = "6";
      submitRequirements(window);
      const changed = commits[commits.length - 1];
      assert.strictEqual(changed.maximumOvershootPercent, 6);
      assert.deepStrictEqual([changed.referenceChange, changed.maximumError, changed.settlingBand, changed.settlingTimeSeconds,
        changed.durationSeconds, changed.constantInputDisturbance], [5, 1, 0.2, 8, 16, 0.3], "every other declared requirement is saved unchanged");

      open({ ...REQUIREMENTS, constantInputDisturbance: 0.3, maximumOvershootPercent: 4 });
      field("maximumOvershootPercent").value = "";
      submitRequirements(window);
      const cleared = commits[commits.length - 1];
      assert.ok(!("maximumOvershootPercent" in cleared), "clearing the row removes the declared limit");
      assert.strictEqual(cleared.constantInputDisturbance, 0.3, "clearing the limit keeps the load trial");
    });
  });
});
