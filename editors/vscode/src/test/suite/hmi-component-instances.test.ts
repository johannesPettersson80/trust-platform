import * as assert from "assert";
import { paletteGroup } from "../../hmiBuilder/editor/webview/names";
import type { ComponentDefinition, EquipmentDefinition } from "../../hmiBuilder/editor/catalogue";
import type { HmiPageModel, HmiObjectModel } from "../../hmiBuilder/drawing/model";
import { buildPage } from "../../hmiBuilder/editor/buildPresentation";

// Specification 39, §8.5 and §21.3. The palette's entries, the instance a placed object gets and
// the bindings an instance gives are the language server's rules (`trust-lsp.hmiEquipmentEdit`),
// proven in crates/trust-lsp/src/handlers/commands/hmi_builder/equipment_tests.rs; these cases
// prove what the editor shows of its answers.

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

suite("FB-backed HMI instances", () => {
  test("the palette lists equipment, instruments, controllers and groups before the general elements", () => {
    const art = (type: string, category: string, group?: string): ComponentDefinition => ({
      type,
      category,
      group,
      phase: 2,
      size: [128, 108],
    });
    const defined = (component: ComponentDefinition, definition: EquipmentDefinition): ComponentDefinition => ({
      ...component,
      category: "equipment",
      equipment: definition,
    });
    // The entries as the language server answers them: each definition with its drawing, then the
    // components no definition maps.
    const entries = [
      defined(art("pump", "equipment", "liquid"), pump),
      defined(art("level-sensor", "equipment", "instruments"), {
        ...pump,
        block: "Plant.FB_LevelInput",
        title: "Level",
        symbol: "level-sensor",
        panel: "plant-measurement",
        position: "measurements",
      }),
      defined(art("controller", "equipment", "instruments"), {
        ...pump,
        block: "Control.PID",
        title: "PID loop",
        symbol: "controller",
        panel: "plant-pid",
        position: "controllers",
      }),
      defined(art("value", "display"), {
        ...pump,
        block: "Plant.FB_DutyStandbyGroup",
        title: "Duty/standby",
        symbol: "value",
        panel: "plant-group",
        position: "controllers",
      }),
      art("value", "display"),
      art("mixer", "equipment", "process"),
      art("panel", "layout"),
      art("page-button", "layout"),
    ];
    const groups = Object.fromEntries(
      entries.map((entry) => [entry.equipment?.block ?? entry.type, paletteGroup(entry)]),
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
  test("static build strips observations only in presentation and preserves authored bindings", () => {
    // A pump bound to `Main.P101` as the language server binds it (§8.5).
    const bound: HmiObjectModel = {
      id: "pump-1",
      type: "pump",
      block: "Plant.FB_Pump",
      instance: "Main.P101",
      faceplate: "plant-run",
      tag: "P101",
      at: [0, 0],
      size: [128, 108],
      rotation: 0,
      bind: {
        running: { path: "Main.P101.Running", direction: "read" },
        fault: { path: "Main.P101.Fault", direction: "read" },
      },
    };
    const page: HmiPageModel = { title: "Plant", object: [bound], pipe: [] };
    const built = buildPage(page);
    assert.deepStrictEqual(built.object[0].bind, {});
    assert.strictEqual(page.object[0].bind.running.path, "Main.P101.Running");
    assert.strictEqual(buildPage(page, { "pump-1": "fault" }).object[0], bound);
  });
});
