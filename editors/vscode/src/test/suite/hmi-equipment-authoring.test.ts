// Specification 39, §21.3 items 2 and 3: the inspector of an equipment object shows its tag, its
// hardware status with one button, its settings with their real values and units, and its
// connected equipment; a change is applied at once as a project edit, never as a page edit.
import * as assert from "assert";
import { Webview, object, CATALOGUE } from "./hmiBuilderWebview";
import type { EquipmentDetails } from "../../hmiBuilder/editor/equipmentDetails";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import { equipmentConnectionSummary, missingSignalHelp } from "../../networkCanvas/equipmentConnectionsModel";

const details: EquipmentDetails = {
  revision: 7,
  instance: "Main.P101",
  block: "Plant.FB_Pump",
  title: "Pump",
  declared: true,
  owned: true,
  settings: [
    {
      member: "StartTimeout",
      title: "Start timeout",
      unit: "s",
      min: 1,
      max: 600,
      kind: "time",
      value: 10,
      source: "declaration",
      key: true,
    },
    { member: "ServiceInterval", title: "Service interval", unit: "h", kind: "real", value: 4000, source: "default" },
  ],
  dependencies: [
    {
      member: "Source",
      title: "Level instrument",
      interface: "Plant.IMeasurement",
      required: true,
      choices: [{ path: "Main.LT101", type: "Plant.FB_LevelInput" }],
    },
  ],
  terminals: [
    {
      member: "RunCommand",
      title: "Run command",
      direction: "output",
      required: true,
      choices: [{ path: "Main.Run", address: "%QX0.0" }],
    },
  ],
  hardware: "not-connected",
  bindings: [],
};

const PUMP = {
  block: "Plant.FB_Pump",
  title: "Pump",
  symbol: "pump",
  panel: "plant-run",
  variants: [],
  bindings: {},
  fields: {},
  settings: [],
  io: [{ member: "RunCommand", title: "Run command", direction: "output" as const, required: true }],
  dependencies: [],
  position: "actuators" as const,
};

async function equipmentView(shown: EquipmentDetails = details, declared = true): Promise<Webview> {
  const view = new Webview();
  const pump = object("pump", "pump", [40, 40], [120, 120], {
    block: "Plant.FB_Pump",
    instance: "Main.P101",
    faceplate: "plant-run",
  });
  await view.loaded({
    title: "Plant",
    object: [
      pump,
      object("instrument", "temperature-sensor", [320, 40], [176, 56], {
        block: "Plant.FB_LevelInput",
        instance: "Main.LT101",
      }),
    ],
    pipe: [],
  });
  view.send({ type: "catalogue", catalogue: { ...CATALOGUE, equipment: [PUMP] } });
  const children: VariableNode[] = declared
    ? [{ name: "P101", path: "Main.P101", type: "Plant.FB_Pump", kind: "function-block", kinds: [], writable: false }]
    : [];
  view.send({
    type: "variables",
    variables: [{ name: "Main", path: "Main", type: "Program", kind: "program", kinds: [], writable: false, children }],
  });
  view.mouse("mousedown", view.layer(), 80, 80);
  view.mouse("mouseup", view.window, 80, 80);
  await view.until("equipment inspected", () => view.posted.some((message) => message.type === "equipmentInspect"));
  view.send({ type: "equipmentDetails", instance: shown.instance, details: shown });
  await view.until("the settings", () => view.query('[data-part="equipment-settings"]'));
  return view;
}

suite("HMI equipment authoring", function () {
  // Each test first loads a whole webview, as the other webview suites do.
  this.timeout(20000);
  test("an equipment object is inspected with its block, before its page is saved", async () => {
    const view = await equipmentView();
    const asked = view.posted.find((message) => message.type === "equipmentInspect");
    assert.strictEqual(asked?.instance, "Main.P101");
    assert.strictEqual(asked?.block, "Plant.FB_Pump");
  });

  test("Connect hardware hands the selected FB to Devices and Connections without a page edit", async () => {
    const view = await equipmentView();
    assert.strictEqual(view.query('[data-part="equipment-hardware"] .hb-status')?.textContent, "Not connected");
    const button = Array.from(view.document.querySelectorAll("button")).find(
      (node: any) => node.textContent === "Connect hardware",
    ) as any;
    assert.ok(button);
    button.click();
    assert.ok(view.posted.some((message) => message.type === "equipmentHardware" && message.instance === "Main.P101"));
    assert.strictEqual(view.edits().length, 0);
  });

  test("every setting shows with its real value and unit, the key ones first and the others under More settings", async () => {
    const view = await equipmentView();
    const input = view.query('input[name="setting-StartTimeout"]');
    assert.strictEqual(input?.value, "10");
    assert.strictEqual(input?.closest("label")?.querySelector(".hb-field-label")?.textContent, "Start timeout (s)");
    assert.strictEqual(input?.closest("details"), null, "a key setting is shown at once");
    const more = view.query('[data-part="equipment-more-settings"]');
    assert.strictEqual(more?.querySelector("summary")?.textContent, "More settings (1)");
    assert.strictEqual(more?.open, false, "the others wait behind More settings");
    const interval = more?.querySelector('input[name="setting-ServiceInterval"]');
    assert.strictEqual(interval?.value, "4000");
    assert.strictEqual(
      interval?.closest("label")?.querySelector(".hb-field-label")?.textContent,
      "Service interval (h)",
    );
  });

  test("a changed setting is applied at once and a value out of range is refused beside it", async () => {
    const view = await equipmentView();
    const input = view.query('input[name="setting-StartTimeout"]');
    view.leave(input, "700");
    await view.until("the refusal", () => view.query('[role="alert"]'));
    assert.strictEqual(view.query('[role="alert"]').textContent, "Not accepted: Start timeout is at most 600");
    assert.ok(!view.posted.some((message) => message.type === "equipmentConfigure"));
    view.leave(input, "12");
    await view.until("the change", () => view.posted.some((message) => message.type === "equipmentConfigure"));
    const message = view.posted.find((candidate) => candidate.type === "equipmentConfigure")!;
    assert.strictEqual(message.revision, 7);
    assert.deepStrictEqual((message.changes as any).settings, { StartTimeout: 12 });
    assert.strictEqual(view.edits().length, 0, "a setting is a project edit, not a page edit");
  });

  test("Pick selects a compatible instance on the page and applies it", async () => {
    const view = await equipmentView();
    const button = Array.from(view.document.querySelectorAll('[data-part="equipment-dependencies"] button')).find(
      (node: any) => node.textContent === "Pick",
    ) as any;
    button.click();
    await view.until("compatible target highlighted", () => view.query('[data-dependency-choice="instrument"]'));
    view.mouse("mousedown", view.layer(), 355, 65);
    view.mouse("mouseup", view.window, 355, 65);
    await view.until("the change", () => view.posted.some((message) => message.type === "equipmentConfigure"));
    const message = view.posted.find((candidate) => candidate.type === "equipmentConfigure")!;
    assert.strictEqual(message.instance, "Main.P101");
    assert.deepStrictEqual((message.changes as any).dependencies, { Source: "Main.LT101" });
    assert.strictEqual(view.edits().length, 0);
  });

  test("equipment declared in PLC code shows its values read-only", async () => {
    const view = await equipmentView({ ...details, owned: false, dependencies: [] });
    assert.ok(view.query('input[name="setting-StartTimeout"]').readOnly);
    assert.ok(view.document.body.textContent.includes("Declared in your PLC code: change these values there."));
  });

  test("a new instance's tag is edited in the inspector", async () => {
    const view = await equipmentView({ ...details, declared: false }, false);
    // The language server says what the inspector offers (§21.3 item 2).
    const tag = await view.until("the tag", () => view.query('input[name="equipment-tag"]'));
    assert.strictEqual(tag?.value, "P101");
    view.leave(tag, "P102");
    const changed = await view.nextEdit(0);
    const pump = changed.object.find((candidate) => candidate.id === "pump");
    assert.strictEqual(pump?.instance, "Main.P102");
    assert.strictEqual(pump?.tag, "P102");
  });

  test("a declared instance's tag renames it with its program, pages and design", async () => {
    const view = await equipmentView();
    const tag = await view.until("the tag", () => view.query('input[name="equipment-tag"]'));
    assert.strictEqual(tag?.value, "P101", "a declared instance's tag is a field");
    view.leave(tag, "FeedPump");
    await view.until("the rename", () => view.posted.some((message) => message.type === "equipmentRename"));
    const renamed = view.posted.find((message) => message.type === "equipmentRename");
    assert.deepStrictEqual(
      { instance: renamed?.instance, name: renamed?.name },
      { instance: "Main.P101", name: "FeedPump" },
    );
    assert.strictEqual(view.edits().length, 0, "the language server renames; the page is not edited here");
  });

  test("a terminal no PLC signal fits says which declaration would", () => {
    assert.strictEqual(
      missingSignalHelp("Main.TT101", { member: "Raw", direction: "input", type: "LREAL" }),
      "No input of type LREAL is declared yet. Declare one with an address in your program, for example TT101_Raw AT %IL0 : LREAL;, then press Refresh.",
    );
    assert.match(
      missingSignalHelp("Main.P101", { member: "RunCommand", direction: "output", type: "BOOL" }),
      /P101_RunCommand AT %QX0\.0 : BOOL;/,
    );
    assert.match(missingSignalHelp("Main.FT1", { member: "Raw", direction: "input", type: "INT" }), /AT %IW0 : INT;/);
  });

  test("the hardware summary uses the language server's judgement", () => {
    assert.strictEqual(equipmentConnectionSummary(details), "Not connected");
    assert.strictEqual(equipmentConnectionSummary({ ...details, hardware: "partly" }), "Partly connected");
    assert.strictEqual(equipmentConnectionSummary({ ...details, hardware: "connected" }), "Connected");
    assert.strictEqual(equipmentConnectionSummary({ ...details, hardware: "check" }), "Check connections");
    assert.strictEqual(equipmentConnectionSummary({ ...details, hardware: "none" }), "No hardware connections needed");
  });
});
