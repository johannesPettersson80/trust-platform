/// <reference lib="dom" />
import * as assert from "assert";
import { engineerScenarioPresentation } from "../../controlStudio/engineerScenarioPresentation";
import { initEngineerScenarioEditor } from "../../controlStudio/webview/engineerScenarioEditor";
import { validEngineerScenarios } from "../../controlStudio/engineerVerificationCampaign";
const { JSDOM } = require("jsdom");

suite("Control Studio Verify scenario editor", () => {
  test("renders a human-readable list and keeps advanced fields through edits", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = JSON.stringify([{ id: "load-step", initialValues: { Tank: 2 }, events: [{ atMs: 100, symbol: "Load", value: 3 }],
      requirements: [{ id: "r", kind: "bounds", channel: "Tank", window: { start: 0, end: 1 }, minimum: 0, maximum: 5 }],
      effects: [{ id: "disturbance", target: "process_input", channel: 0, mode: "add", startSeconds: 0, generator: "constant", value: 1 }], custom: { keep: true } }]);
    initEngineerScenarioEditor(section);
    assert.strictEqual(section.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!.value, "load-step");
    const id = section.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
    id.value = "renamed";
    id.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    const saved = JSON.parse(raw.value);
    assert.strictEqual(saved[0].id, "renamed");
    assert.deepStrictEqual(saved[0].requirements[0].window, { start: 0, end: 1 });
    assert.deepStrictEqual(saved[0].custom, { keep: true });
    assert.deepStrictEqual(saved[0].effects[0].target, "process_input");
  });

  test("adds and removes scenarios through bounded controls", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = "[]";
    initEngineerScenarioEditor(section);
    section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")!.click();
    assert.strictEqual(JSON.parse(raw.value).length, 1);
    section.querySelector<HTMLButtonElement>("[data-engineer-scenario-remove]")!.click();
    assert.deepStrictEqual(JSON.parse(raw.value), []);
  });

  test("rejects duplicate scenario names without changing the existing definitions", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = "[]";
    initEngineerScenarioEditor(section);
    const add = section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")!;
    add.click(); add.click();
    const names = section.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-id]");
    names[0].value = "step-response";
    names[0].dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    const beforeDuplicate = JSON.parse(raw.value);
    names[1].value = "step-response";
    names[1].dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    assert.match(section.querySelector("[data-engineer-scenario-error]")!.textContent!, /unique|duplicate|already exists/i);
    assert.deepStrictEqual(JSON.parse(raw.value), beforeDuplicate);
  });

  test("keeps focus and multi-character names while typing", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = "[]"; initEngineerScenarioEditor(section);
    section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")!.click();
    const id = section.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!;
    for (const character of "step-up") { id.value += character; id.dispatchEvent(new dom.window.Event("input", { bubbles: true })); }
    assert.strictEqual(section.querySelector<HTMLInputElement>("[data-engineer-scenario-id]"), id);
    assert.strictEqual(JSON.parse(raw.value)[0].id, "scenario-1step-up");
  });

  test("adds schema-shaped scenarios and exposes an editable initial-value symbol", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = "[]"; initEngineerScenarioEditor(section, { processSymbol: "PV", processMinimum: 0, processMaximum: 10, setpointSymbol: "SP", durationSeconds: 4 });
    section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add]")!.click();
    const scenario = JSON.parse(raw.value)[0];
    assert.ok(validEngineerScenarios([scenario]));
    assert.deepStrictEqual(scenario.requirements[0], { id: "requirement-1", kind: "bounds", channel: "PV", window: { start: 0, end: 4 }, minimum: 0, maximum: 10 });
    section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
    const symbol = section.querySelector<HTMLInputElement>("[data-engineer-scenario-initial-symbol]")!;
    symbol.value = "Tank"; symbol.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    assert.deepStrictEqual(JSON.parse(raw.value)[0].initialValues, { Tank: 0 });
  });

  test("adding a second initial value does not overwrite the existing setpoint", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = JSON.stringify([{ id: "case", initialValues: { SP: 0.2 }, events: [], requirements: [{}] }]);
    initEngineerScenarioEditor(section, { processSymbol: "PV", setpointSymbol: "SP", durationSeconds: 4 });
    section.querySelector<HTMLButtonElement>("[data-engineer-scenario-add-value]")!.click();
    assert.deepStrictEqual(JSON.parse(raw.value)[0].initialValues, { SP: 0.2 });
    assert.deepStrictEqual(Array.from(section.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-initial-symbol]"), input => input.value), ["SP", ""]);
  });

  test("renaming an initial value to an existing symbol does not overwrite its value", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = JSON.stringify([{ id: "case", initialValues: { SP: 0.2, PV: 1 }, events: [], requirements: [{}] }]);
    initEngineerScenarioEditor(section);
    const symbols = section.querySelectorAll<HTMLInputElement>("[data-engineer-scenario-initial-symbol]");
    symbols[1].value = "SP";
    symbols[1].dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    assert.deepStrictEqual(JSON.parse(raw.value)[0].initialValues, { SP: 0.2, PV: 1 });
  });

  test("renaming an initial symbol replaces the prior key and rejects invalid numbers", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = JSON.stringify([{ id: "case", initialValues: { Main: 2 }, events: [], requirements: [{}] }]);
    initEngineerScenarioEditor(section);
    const symbol = section.querySelector<HTMLInputElement>("[data-engineer-scenario-initial-symbol]")!;
    for (const value of ["M", "Ma", "Main2"]) { symbol.value = value; symbol.dispatchEvent(new dom.window.Event("input", { bubbles: true })); }
    const number = section.querySelector<HTMLInputElement>("[data-engineer-scenario-initial]")!;
    number.value = "bad"; number.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    assert.deepStrictEqual(JSON.parse(raw.value)[0].initialValues, { Main2: 2 });
  });

  test("shows manual JSON errors without destroying the last valid editor state", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = "[{\"id\":\"safe\",\"initialValues\":{},\"events\":[],\"requirements\":[{}]}]";
    initEngineerScenarioEditor(section);
    raw.value = "{";
    raw.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    assert.match(section.querySelector("[data-engineer-scenario-error]")!.textContent!, /valid JSON/i);
    assert.strictEqual(section.querySelector<HTMLInputElement>("[data-engineer-scenario-id]")!.value, "safe");
  });

  test("marks invalid numeric and advanced JSON controls as natively invalid", () => {
    const dom = new JSDOM(`<div id="root">${engineerScenarioPresentation()}</div>`);
    const section = dom.window.document.querySelector("[data-engineer-scenario-controls]") as HTMLElement;
    const raw = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenarios]")!;
    raw.value = JSON.stringify([{ id: "case", initialValues: { PV: 2 }, events: [], requirements: [{}], custom: { keep: true } }]);
    initEngineerScenarioEditor(section);
    const number = section.querySelector<HTMLInputElement>("[data-engineer-scenario-initial]")!;
    number.value = ""; number.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    assert.ok(number.matches(":invalid"));
    const advanced = section.querySelector<HTMLTextAreaElement>("[data-engineer-scenario-advanced]")!;
    advanced.value = "{"; advanced.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
    assert.ok(advanced.matches(":invalid"));
  });
});
