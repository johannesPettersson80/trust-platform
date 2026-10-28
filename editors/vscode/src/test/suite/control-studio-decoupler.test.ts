import * as assert from "assert";
import { DEFAULT_STRUCTURES, open } from "./control-studio-loop-setup-form-harness";

const member = (index: number) => ({ instance: `P1.Zone${index}`, parameterRecord: `P1.Zone${index}Parameters`,
  processVariable: { symbol: `P1.Zone${index}C`, unit: "degC", minimum: 0, maximum: 100 },
  manipulatedVariable: { symbol: `P1.Heater${index}Pct`, unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: `P1.Zone${index}SetpointC` }, manualOutput: `P1.Zone${index}ManualPct` });

/** Two coupled zones, one library PID each; the program passes their outputs through a decoupler (Specification 35 §17.15). */
const decoupler = [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]];

/** The structures the engine offers a program with several library PIDs: the group joins the default offer (C7, `structures` of the discover answer). */
const GROUP_OFFER = ["group", ...DEFAULT_STRUCTURES];

suite("Control Studio static decoupler", () => {

  // V7.17: nothing sends `design.decoupler`. A group's Tune answer carries its decoupler in `group` (structureResults.ts, tested in
  // control-studio-structure-results.test.ts), so the page's second table of D, drawn from `design.decoupler`, was dead code and went with its test.

  test("behind a decoupler, the form posts the members' outputs as typed and shows the engine's refusal under the member's output", function () {
    this.timeout(10000);
    const view = open({ structure: "group", structures: GROUP_OFFER, addressedReal: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Zone1C", "P1.Zone1ManualPct", "P1.Zone1SetpointC",
      "P1.Zone2C", "P1.Zone2ManualPct", "P1.Zone2SetpointC"], signals: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Request1Pct", "P1.Request2Pct",
      "P1.Zone1C", "P1.Zone1ManualPct", "P1.Zone1SetpointC", "P1.Zone2C", "P1.Zone2ManualPct", "P1.Zone2SetpointC"],
      booleans: ["P1.ZonesAuto"], modes: ["P1.ZonesAuto"], records: ["P1.Zone1Parameters", "P1.Zone2Parameters"], writes: [], initializes: [], instances: ["P1"],
      pids: [{ instance: "P1.Zone1", task: { name: "MainTask", intervalMs: 100 } }, { instance: "P1.Zone2", task: { name: "MainTask", intervalMs: 100 } }],
      reals: ["P1.D11", "P1.D12", "P1.D21", "P1.D22"],
      prefills: [1, 2].map(n => ({ instance: `P1.Zone${n}`, parameterRecord: `P1.Zone${n}Parameters`, manipulatedVariable: `P1.Request${n}Pct`,
        setpoint: `P1.Zone${n}SetpointC`, processValue: `P1.Zone${n}C`, manualOutput: `P1.Zone${n}ManualPct` })) });
    try {
      view.set("structure", "group");
      for (const index of [0, 1]) {
        const box = view.root.querySelector<HTMLInputElement>(`[data-setup-field="group.${index}.include"]`)!;
        box.checked = true; box.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
        for (const [field, value] of [["processVariable.unit", "degC"], ["processVariable.minimum", "0"], ["processVariable.maximum", "100"],
          ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"], ["manipulatedVariable.maximum", "100"]]) {
          view.set(`group.${index}.${field}`, value);
        }
      }
      for (const [field, value] of [["name", "Zones"], ["group.mode", "P1.ZonesAuto"]]) view.set(field, value);
      const decoupled = view.root.querySelector<HTMLInputElement>('[data-setup-field="group.decoupled"]')!;
      decoupled.checked = true; decoupled.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      decoupler.forEach((row, i) => row.forEach((symbol, j) => view.set(`group.decoupler.${i}.${j}`, symbol)));
      // The engine checks the decoupler's rule (setup/group_loop.rs): the form posts the prefilled controller outputs as typed.
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, "the form does not decide the decoupler's rule");
      save.click();
      const posted = view.posts[view.posts.length - 1]?.draft as { members?: Array<{ manipulatedVariable?: { symbol?: string } }> } | undefined;
      assert.deepStrictEqual(posted?.members?.map(member => member.manipulatedVariable?.symbol), ["P1.Request1Pct", "P1.Request2Pct"]);
      // The engine's refusal names the member's output field, and the page shows its sentence.
      const sentence = "Behind a decoupler, P1.Zone1's output is the plant input the program writes from D·v, not the controller's own output "
        + "P1.Request1Pct. Choose the plant input.";
      view.showError(sentence, "group.0.manipulatedVariable");
      assert.ok((view.root.textContent ?? "").includes(sentence), "the engine's sentence is shown");
    } finally { view.close(); }
  });

  test("sets up the decoupler's variables with the group in the form", function () {
    // About thirty fields through JSDOM, each re-rendering the form.
    this.timeout(10000);
    const view = open({ structure: "group", structures: GROUP_OFFER, addressedReal: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Zone1C", "P1.Zone1ManualPct", "P1.Zone1SetpointC",
      "P1.Zone2C", "P1.Zone2ManualPct", "P1.Zone2SetpointC"], booleans: ["P1.ZonesAuto"], modes: ["P1.ZonesAuto"],
      records: ["P1.Zone1Parameters", "P1.Zone2Parameters"], writes: [], initializes: [], instances: ["P1"],
      pids: [{ instance: "P1.Zone1", task: { name: "MainTask", intervalMs: 100 } }, { instance: "P1.Zone2", task: { name: "MainTask", intervalMs: 100 } }],
      reals: ["P1.D11", "P1.D12", "P1.D21", "P1.D22", "P1.Heater1Pct", "P1.Heater2Pct", "P1.Zone1C", "P1.Zone2C"] });
    try {
      view.set("structure", "group");
      for (const index of [0, 1]) {
        const n = index + 1;
        for (const [field, value] of [["include", "on"], ["parameterRecord", `P1.Zone${n}Parameters`], ["processVariable", `P1.Zone${n}C`],
          ["processVariable.unit", "degC"], ["processVariable.minimum", "0"], ["processVariable.maximum", "100"], ["manipulatedVariable", `P1.Heater${n}Pct`],
          ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"], ["manipulatedVariable.maximum", "100"], ["setpoint", `P1.Zone${n}SetpointC`],
          ["manualOutput", `P1.Zone${n}ManualPct`]]) {
          if (field === "include") { const box = view.root.querySelector<HTMLInputElement>(`[data-setup-field="group.${index}.include"]`)!; box.checked = true; box.dispatchEvent(new view.dom.window.Event("change", { bubbles: true })); }
          else view.set(`group.${index}.${field}`, value);
        }
      }
      for (const [field, value] of [["name", "Zones"], ["group.mode", "P1.ZonesAuto"]]) view.set(field, value);
      const box = view.root.querySelector<HTMLInputElement>('[data-setup-field="group.decoupled"]');
      assert.ok(box, "the group offers a decoupler");
      box!.checked = true; box!.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      const message = () => view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "";
      decoupler.forEach((row, i) => row.forEach((symbol, j) => view.set(`group.decoupler.${i}.${j}`, symbol)));
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, message());
      save.click();
      assert.deepStrictEqual(view.posts[0]?.draft?.decoupler, decoupler);
    } finally { view.close(); }
  });
});
