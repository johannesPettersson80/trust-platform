// Specification 39, §21.9: the project's own function blocks are listed under My equipment;
// placing one for the first time asks for its files, which the host writes and saves; and the
// inspector chooses its picture and the member driving each of the picture's inputs.
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import type { Catalogue, EquipmentDefinition } from "../../hmiBuilder/editor/catalogue";
import type { EquipmentDetails } from "../../hmiBuilder/editor/equipmentDetails";
import { languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";
import { CATALOGUE, object, Webview } from "./hmiBuilderWebview";

/** The plain device and one of My Symbols, beside the test catalogue's components. */
const PICTURES: Catalogue = {
  ...CATALOGUE,
  components: [
    ...CATALOGUE.components,
    {
      type: "device",
      category: "equipment",
      group: "process",
      phase: 3,
      size: [128, 108],
      properties: {
        running: { accepts: ["boolean"], directions: ["read"] },
        fault: { accepts: ["boolean"], directions: ["read"] },
      },
    },
    {
      type: "my:mixer",
      category: "equipment",
      group: "my-symbols",
      phase: 3,
      size: [128, 108],
      properties: { stirring: { accepts: ["boolean"], directions: ["read"] } },
    },
  ],
};

const MIXER: EquipmentDefinition = {
  block: "FB_MixingTank",
  title: "Mixing tank",
  symbol: "device",
  panel: "fb-mixing-tank",
  variants: [],
  bindings: { running: "Running" },
  fields: {},
  settings: [],
  io: [],
  dependencies: [],
  proposed: true,
  mine: true,
};

suite("HMI Builder: My equipment", function () {
  this.timeout(20000);

  test("the project's own blocks are listed under My equipment, and placing one asks for its files", async () => {
    const view = new Webview();
    await view.loaded();
    view.send({ type: "catalogue", catalogue: { ...PICTURES, equipment: [MIXER] } });
    const card = await view.until("the mixer under My equipment", () =>
      view.query('[data-component-group="my-equipment"] [data-block="FB_MixingTank"]'),
    );
    assert.strictEqual(view.query('[data-component-group="my-equipment"] h2')?.textContent, "My equipment");
    card.click();
    await view.until("its files asked for", () => view.posted.some((message) => message.type === "ensureEquipment"));
    assert.deepStrictEqual(
      view.posted.filter((message) => message.type === "ensureEquipment").map((message) => message.block),
      ["FB_MixingTank"],
    );
  });

  test("the inspector chooses the block's picture and the member driving each of its inputs", async () => {
    const view = new Webview();
    const mixer = object("mixer", "device", [40, 40], [128, 108], {
      block: "FB_MixingTank",
      instance: "Main.M101",
      faceplate: "fb-mixing-tank",
    });
    await view.loaded({ title: "Plant", object: [mixer], pipe: [] });
    view.send({ type: "catalogue", catalogue: { ...PICTURES, equipment: [{ ...MIXER, proposed: false }] } });
    view.mouse("mousedown", view.layer(), 80, 80);
    view.mouse("mouseup", view.window, 80, 80);
    await view.until("the mixer inspected", () => view.posted.some((message) => message.type === "equipmentInspect"));
    const details: EquipmentDetails = {
      revision: 1,
      instance: "Main.M101",
      block: "FB_MixingTank",
      title: "Mixing tank",
      declared: false,
      owned: true,
      settings: [],
      dependencies: [],
      terminals: [],
      hardware: "none",
      bindings: [],
      picture: {
        editable: true,
        symbol: "device",
        inputs: [{ property: "running", member: "Running", choices: ["Running", "MotorFault"] }],
      },
    };
    view.send({ type: "equipmentDetails", instance: "Main.M101", details });
    const shown = await view.until("the Picture section", () => view.query('select[name="equipment-picture"]'));
    const offered = [...shown.querySelectorAll("option")].map((option: { value: string }) => option.value);
    assert.deepStrictEqual(offered, ["device", "my:mixer"], "the plain device and My Symbols");
    view.choose(view.query('select[name="picture-input-running"]'), "MotorFault");
    await view.until("the input's member sent", () =>
      view.posted.some((message) => message.type === "equipmentPicture"),
    );
    const bound = view.posted.filter((message) => message.type === "equipmentPicture").pop();
    assert.deepStrictEqual(
      { block: bound?.block, symbol: bound?.symbol, bind: bound?.bind },
      { block: "FB_MixingTank", symbol: "device", bind: { running: "MotorFault" } },
    );
    view.choose(shown, "my:mixer");
    await view.until(
      "the picture sent",
      () => view.posted.filter((message) => message.type === "equipmentPicture").length === 2,
    );
    assert.strictEqual(view.posted.filter((message) => message.type === "equipmentPicture").pop()?.symbol, "my:mixer");
  });
});

/** A project with one block of its own, placed in its program but not yet defined for the HMI. */
const OWN_PROJECT: Record<string, string> = {
  "hmi/screen.toml": 'format = 1\nsize = [1280, 720]\ntheme = "dark"\n',
  "hmi/pages/overview.toml": 'format = 1\ntitle = "Overview"\n',
  "src/mixer.st":
    "FUNCTION_BLOCK FB_Mixer\nVAR_OUTPUT\n    Running : BOOL;\n    Level : REAL;\nEND_VAR\nEND_FUNCTION_BLOCK\n",
  "src/main.st": "PROGRAM Main\nVAR\n    M101 : FB_Mixer;\nEND_VAR\n    M101();\nEND_PROGRAM\n",
};

suite("HMI Builder editor: My equipment on the host", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  test("placing an own block for the first time writes its definition and automatic panel, saved", async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-my-equipment-"));
    for (const [file, text] of Object.entries(OWN_PROJECT)) {
      fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
      fs.writeFileSync(path.join(root, file), text);
    }
    const { webview, panel } = await openBuilder(path.join(root, "hmi", "pages", "overview.toml"));
    const mixer = (): EquipmentDefinition | undefined => {
      const sent = postedOfType(webview, "catalogue");
      return sent[sent.length - 1]?.catalogue.equipment?.find((definition) => definition.block === "FB_Mixer");
    };
    webview.send({ type: "ready" });
    const proposed = await until("FB_Mixer proposed under My equipment", () => mixer()?.proposed && mixer());
    assert.strictEqual(proposed.mine, true);
    webview.send({ type: "ensureEquipment", block: "FB_Mixer" });
    const pair = path.join(root, "hmi", "pairs", "fb-mixer.toml");
    const faceplate = path.join(root, "hmi", "panels", "fb-mixer.toml");
    const written = (file: string): string | undefined =>
      fs.existsSync(file) ? fs.readFileSync(file, "utf8") || undefined : undefined;
    await until("both files saved", () => written(pair) && written(faceplate));
    assert.match(written(pair) ?? "", /^block = "FB_Mixer"$/m);
    assert.match(written(faceplate) ?? "", /^title = "Mixer"$/m);
    const defined = await until("FB_Mixer defined", () => mixer()?.proposed !== true && mixer());
    assert.deepStrictEqual({ mine: defined.mine, panel: defined.panel }, { mine: true, panel: "fb-mixer" });
    panel.dispose();
  });
});
