import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import type { HmiObjectModel, HmiPageModel } from "../../hmiBuilder/drawing/model";
import { TOKEN_THEMES } from "../../hmiBuilder/drawing/tokens";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import {
  colourQualityWarnings,
  drawnCuts,
  LanguageTable,
  pageQualityWarnings,
  type QualityRequest,
} from "../../hmiBuilder/editor/quality";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §14.1 item 4: the editor's quality warnings. Their rules, thresholds and sentences are the language server's
// (`trust-lsp.hmiQuality`; its Rust tests hold the cases of each rule). These tests prove what the editor adds: the texts the drawing
// cuts to fit in each language, the report it sends, and the warnings it passes on; and, through the real server, the colour checks
// on the specimen's texts that only the drawing can measure.

const CATALOGUE: Catalogue = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../../../../../crates/trust-runtime/src/hmi/catalogue/components.json"),
    "utf8",
  ),
);

function object(id: string, type: string, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  const component = CATALOGUE.components.find((candidate) => candidate.type === type);
  return { id, type, at: [0, 0], size: component?.size ?? [160, 48], rotation: 0, bind: {}, ...extra };
}

function bound(
  id: string,
  type: string,
  property: string,
  variable: string,
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return object(id, type, { bind: { [property]: { path: variable, direction: "read" } }, ...extra });
}

function page(...objects: HmiObjectModel[]): HmiPageModel {
  return { title: "Checks", object: objects, pipe: [] };
}

/** A request that records what the editor sends and answers `answer`. */
function recorded(answer: unknown): { request: QualityRequest; sent: Array<{ command: string; argument: any }> } {
  const sent: Array<{ command: string; argument: any }> = [];
  return {
    sent,
    request: async (command, argument) => {
      sent.push({ command, argument });
      return answer;
    },
  };
}

/** The real server's request, as the editor sends it (`vscode.commands.executeCommand`). */
const server: QualityRequest = (command, argument) =>
  Promise.resolve(vscode.commands.executeCommand(command, argument));

suite("HMI Builder quality checks", () => {
  test("the drawing reports a label it cuts to fit, and none of one that fits", () => {
    const report = drawnCuts(
      page(
        bound("long", "value", "value", "Station.Speed", {
          size: [160, 48],
          label: "Feed pump P101 speed",
          unit: "rpm",
        }),
        bound("short", "value", "value", "Station.Speed", { size: [160, 48], label: "Speed", unit: "rpm" }),
      ),
      CATALOGUE,
      undefined,
    );
    assert.deepStrictEqual(report, [{ cuts: [{ object: "long", text: "Feed pump P101 speed" }] }]);
  });

  test("the drawing reports a text it cuts in one of the project's languages, with that language (§14.5 item 6)", () => {
    const languages: LanguageTable[] = [
      { tag: "en", name: "English", texts: {} },
      { tag: "de", name: "Deutsch", texts: { Start: "Pumpe jetzt sofort starten" } },
      { tag: "sv", name: "Svenska", texts: { Start: "Starta" } },
    ];
    const report = drawnCuts(page(object("start", "text", { size: [120, 40], text: "Start" })), CATALOGUE, languages);
    assert.deepStrictEqual(
      report.map((drawn) => [drawn.language?.tag, drawn.cuts]),
      [
        ["en", []],
        ["de", [{ object: "start", text: "Pumpe jetzt sofort starten" }]],
        ["sv", []],
      ],
    );
  });

  test("the drawing reports one of truST's own words it cuts in a language (§14.5 item 6)", () => {
    const languages: LanguageTable[] = [
      { tag: "en", name: "English", texts: {} },
      { tag: "sv", name: "Svenska", texts: { Running: "Pumpen går för fullt hela tiden" } },
    ];
    const report = drawnCuts(
      page(bound("pump", "pump", "running", "Station.Start", { tag: "P-101" })),
      CATALOGUE,
      languages,
    );
    assert.deepStrictEqual(report[1].cuts, [{ object: "pump", text: "Pumpen går för fullt hela tiden" }]);
  });

  test("a page's report holds its objects, whether it is a P&ID and the drawing's cuts, and the server's warnings are passed on", async () => {
    const warning = { kind: "unit", object: "speed", message: "speed shows a REAL value without a unit." };
    const { request, sent } = recorded({ ok: true, objects: [warning], colours: [] });
    const checked = page(bound("speed", "value", "value", "Station.Speed"));
    const warnings = await pageQualityWarnings(request, "file:///project", checked, CATALOGUE, undefined, "pid");
    assert.deepStrictEqual(warnings, [warning], "the server's warnings, unchanged");
    assert.strictEqual(sent[0].command, "trust-lsp.hmiQuality");
    assert.strictEqual(sent[0].argument.root_uri, "file:///project");
    assert.deepStrictEqual(sent[0].argument.page.objects, checked.object);
    assert.strictEqual(sent[0].argument.page.pid, true);
    assert.deepStrictEqual(sent[0].argument.page.cuts, [{ cuts: [] }]);
    // A server that cannot answer gives no warnings, as a program that does not compile gives none (§14.1 item 4).
    assert.deepStrictEqual(
      await pageQualityWarnings(recorded(undefined).request, "file:///project", checked, CATALOGUE, undefined),
      [],
    );
  });

  test("a colour report holds the drawing's colours, the project's in their order and the specimen's texts", async () => {
    const warning = {
      kind: "look-alike",
      colour: { theme: "dark", token: "accent" },
      message: "accent looks like an alarm.",
    };
    const { request, sent } = recorded({
      ok: true,
      objects: [],
      colours: [warning],
      display: "No program calls Hmi.Display.",
    });
    const answer = await colourQualityWarnings(
      request,
      "file:///project",
      { dark: { "pen-3": "#ff5470", accent: "#f97316" } },
      CATALOGUE,
    );
    assert.deepStrictEqual(answer, { warnings: [warning], display: "No program calls Hmi.Display." });
    const colours = sent[0].argument.colours;
    assert.deepStrictEqual(colours.defaults, TOKEN_THEMES);
    assert.deepStrictEqual(colours.own, {
      dark: [
        ["pen-3", "#ff5470"],
        ["accent", "#f97316"],
      ],
      light: [],
    });
    assert.ok(colours.texts.length > 0, "the specimen's texts, which only the drawing measures");
  });

  suite("through the language server", function () {
    this.timeout(60000);
    let root = "";

    suiteSetup(async () => {
      await languageServerReady();
      root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-quality-"));
    });

    suiteTeardown(() => {
      fs.rmSync(root, { recursive: true, force: true });
    });

    test("the shipped colours raise no colour warnings", async () => {
      const { warnings } = await colourQualityWarnings(server, vscode.Uri.file(root).toString(), {}, CATALOGUE);
      assert.deepStrictEqual(warnings, []);
    });

    test("a project colour that takes text below its contrast is flagged at the colour, with the ratio", async () => {
      const { warnings } = await colourQualityWarnings(
        server,
        vscode.Uri.file(root).toString(),
        { dark: { muted: "#1f2430" } },
        CATALOGUE,
      );
      const contrast = warnings.filter((warning) => warning.kind === "contrast");
      assert.deepStrictEqual(
        contrast.map((warning) => `${warning.colour?.theme}.${warning.colour?.token}`),
        ["dark.muted"],
      );
      assert.match(contrast[0].message, /\d+\.\d\d:1/);
      assert.match(contrast[0].message, /4\.5:1|3:1/);
    });

    test("a project colour that looks like the warning or an alarm colour is flagged", async () => {
      const { warnings } = await colourQualityWarnings(
        server,
        vscode.Uri.file(root).toString(),
        { dark: { accent: "#f97316", "pen-3": "#ff5470" }, light: { "pen-1": "#0e7490" } },
        CATALOGUE,
      );
      const alike = warnings.filter((warning) => warning.kind === "look-alike");
      assert.deepStrictEqual(
        alike.map((warning) => `${warning.colour?.theme}.${warning.colour?.token}`),
        ["dark.accent", "dark.pen-3"],
      );
      assert.match(alike[0].message, /alarm/);
    });

    test("a page's touch targets and unbound objects are flagged by the server", async () => {
      const warnings = await pageQualityWarnings(
        server,
        vscode.Uri.file(root).toString(),
        page(
          object("start", "button", { size: [120, 36], text: "Start" }),
          object("lonely", "value", { label: "Level" }),
          object("title", "text", { text: "Feed station" }),
        ),
        CATALOGUE,
        undefined,
      );
      assert.deepStrictEqual(
        warnings.map((warning) => `${warning.object}:${warning.kind}`),
        ["start:touch-target", "start:unbound", "lonely:unbound"],
      );
    });
  });
});
