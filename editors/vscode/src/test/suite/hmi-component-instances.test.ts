import * as assert from "assert";
import { bindEquipment, memberPath, newEquipmentInstance, paletteComponents } from "../../hmiBuilder/editor/equipment";
import { paletteGroup } from "../../hmiBuilder/editor/webview/names";
import type { Catalogue, EquipmentDefinition } from "../../hmiBuilder/editor/catalogue";
import type { HmiPageModel, HmiObjectModel } from "../../hmiBuilder/drawing/model";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import { buildPage } from "../../hmiBuilder/editor/buildPresentation";

const pump: EquipmentDefinition = {
  block: "Plant.FB_Pump",
  title: "Pump",
  symbol: "pump",
  panel: "plant-run",
  variants: [],
  bindings: { running: "Running", fault: "Fault" },
  fields: {},
  settings: [],
  io: [],
  dependencies: [],
  position: "actuators",
};
const object: HmiObjectModel = { id: "pump-1", type: "pump", at: [0, 0], size: [128, 108], rotation: 0, bind: {} };
const program: VariableNode = {
  name: "Main",
  path: "Main",
  kind: "program",
  type: "MainProgram",
  kinds: [],
  writable: false,
  children: [
    { name: "Pump1", path: "Main.Pump1", kind: "function-block", type: "Plant.FB_Pump", kinds: [], writable: false },
  ],
};

suite("FB-backed HMI instances", () => {
  test("placement avoids source and page instance names without guessing among programs", () => {
    const page: HmiPageModel = { title: "Plant", object: [bindEquipment(object, pump, "Main.Pump2")], pipe: [] };
    assert.strictEqual(newEquipmentInstance(pump, [program], page), "Main.Pump3");
    assert.strictEqual(newEquipmentInstance(pump, [], page), "Main.Pump1");
    assert.strictEqual(newEquipmentInstance(pump, [program, { ...program, path: "Other" }], page), undefined);
  });
  test("rebinding changes relative fields once and keeps click disabled until requested", () => {
    const first = bindEquipment(object, pump, "Main.P101");
    const second = bindEquipment(first, pump, "Main.P102");
    assert.strictEqual(second.tag, "P102");
    assert.strictEqual(second.bind.running.path, "Main.P102.Running");
    assert.strictEqual(first.bind.running.path, "Main.P101.Running");
    assert.strictEqual(second.click, undefined);
    const open = bindEquipment(
      { ...first, click: { faceplate: "plant-run", instance: "Main.P101" } },
      pump,
      "Main.P102",
    );
    assert.strictEqual(open.click?.instance, "Main.P102");
  });
  test("multiple FB variants share a renderer without disappearing from the palette", () => {
    const catalogue: Catalogue = {
      format: 1,
      components: [{ type: "pump", category: "equipment", phase: 2, size: [128, 108] }],
      equipment: [
        pump,
        { ...pump, block: "Plant.FB_VariableSpeedPump", title: "Variable-speed pump", panel: "plant-drive" },
      ],
    };
    const entries = paletteComponents(catalogue);
    assert.deepStrictEqual(
      entries.map((entry) => entry.equipment?.block),
      ["Plant.FB_Pump", "Plant.FB_VariableSpeedPump"],
    );
    assert.ok(entries.every((entry) => entry.type === "pump"));
  });
  test("the palette lists equipment, instruments, controllers and groups before the general elements", () => {
    const art = (type: string, category: string, group?: string) => ({
      type,
      category,
      group,
      phase: 2,
      size: [128, 108] as [number, number],
    });
    const catalogue: Catalogue = {
      format: 1,
      components: [
        art("pump", "equipment", "liquid"),
        art("level-sensor", "equipment", "instruments"),
        art("controller", "equipment", "instruments"),
        art("value", "display"),
        art("mixer", "equipment", "process"),
        art("panel", "layout"),
        art("page-button", "layout"),
      ],
      equipment: [
        pump,
        {
          ...pump,
          block: "Plant.FB_LevelInput",
          title: "Level",
          symbol: "level-sensor",
          panel: "plant-measurement",
          position: "measurements",
        },
        {
          ...pump,
          block: "Control.PID",
          title: "PID loop",
          symbol: "controller",
          panel: "plant-pid",
          position: "controllers",
        },
        {
          ...pump,
          block: "Plant.FB_DutyStandbyGroup",
          title: "Duty/standby",
          symbol: "value",
          panel: "plant-group",
          position: "controllers",
        },
      ],
    };
    const groups = Object.fromEntries(
      paletteComponents(catalogue).map((entry) => [entry.equipment?.block ?? entry.type, paletteGroup(entry)]),
    );
    assert.deepStrictEqual(groups, {
      "Plant.FB_Pump": "equipment",
      "Plant.FB_LevelInput": "instruments",
      "Control.PID": "controllers",
      "Plant.FB_DutyStandbyGroup": "sequences",
      value: "display",
      mixer: "symbols",
      panel: "shapes",
      "page-button": "navigation",
    });
  });
  test("a PID loop's symbol binds the variables beside its PID (spec 39 §8.5, §10.5)", () => {
    const loop: EquipmentDefinition = {
      ...pump,
      block: "Control.PID",
      title: "PID controller",
      symbol: "controller",
      panel: "plant-pid",
      bindings: { pv: "{instance}_Input.ProcessValue", sp: "{instance}_Setpoint", manual: "{instance}_Manual" },
      position: "controllers",
    };
    const bound = bindEquipment({ ...object, id: "controller-1", type: "controller" }, loop, "Main.TIC101");
    assert.strictEqual(bound.bind.pv.path, "Main.TIC101_Input.ProcessValue");
    assert.strictEqual(bound.bind.sp.path, "Main.TIC101_Setpoint");
    assert.strictEqual(bound.bind.manual.path, "Main.TIC101_Manual");
    assert.strictEqual(memberPath("Main.P101", "Running"), "Main.P101.Running");
    assert.strictEqual(
      memberPath("Main.TIC101", "{instance}_Parameters.ProportionalGain"),
      "Main.TIC101_Parameters.ProportionalGain",
    );
  });
  test("static build strips observations only in presentation and preserves authored bindings", () => {
    const bound = bindEquipment(object, pump, "Main.P101");
    const page: HmiPageModel = { title: "Plant", object: [bound], pipe: [] };
    const built = buildPage(page);
    assert.deepStrictEqual(built.object[0].bind, {});
    assert.strictEqual(page.object[0].bind.running.path, "Main.P101.Running");
    assert.strictEqual(buildPage(page, { "pump-1": "fault" }).object[0], bound);
  });
});
