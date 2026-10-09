import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { textWidth } from "../../hmiBuilder/drawing/format";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";
import type { ProjectTexts } from "../../hmiBuilder/editor/messages";
import { FakeWebview, languageServerReady, openBuilder, postedOfType, until } from "./hmiBuilderHost";
import { object, Webview } from "./hmiBuilderWebview";

// Specification 39, §14.5 items 4, 7 and 8: the editor is given the project's languages and texts,
// shows the page in each language, and writes the translations to the text files.

const REPO = path.resolve(__dirname, "../../../../..");
const SCREEN = 'format = 1\nsize = [1280, 720]\ntheme = "dark"\nsymbols = "flat"\nlanguages = ["en", "sv"]\n';
const PAGE =
  'format = 1\ntitle = "Overview"\n\n[[object]]\nid = "heading"\ntype = "text"\nat = [40, 40]\nsize = [240, 40]\ntext = "Pumps"\n';
const SWEDISH = 'format = 1\nlanguage = "sv"\n\n[texts]\n"Pumps" = "Pumpar"\n';

async function project(
  files: Record<string, string>,
): Promise<{ root: string } & Awaited<ReturnType<typeof openBuilder>>> {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-languages-"));
  for (const [file, text] of Object.entries({ "hmi/screen.toml": SCREEN, "hmi/pages/overview.toml": PAGE, ...files })) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), text);
  }
  return { root, ...(await openBuilder(path.join(root, "hmi", "pages", "overview.toml"))) };
}

function lastTexts(webview: FakeWebview): ProjectTexts | undefined {
  const sent = postedOfType(webview, "texts");
  return sent[sent.length - 1]?.texts;
}

suite("HMI Builder editor: languages on the host", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  test("on ready the editor sends the project's languages and texts as the language server reads them (§14.5 items 7, 8)", async () => {
    const { webview, panel } = await project({ "hmi/texts/sv.toml": SWEDISH });
    webview.send({ type: "ready" });
    const texts = await until("the texts", () => lastTexts(webview));
    assert.deepStrictEqual(texts.languages, [
      { tag: "en", name: "English" },
      { tag: "sv", name: "Svenska" },
    ]);
    assert.deepStrictEqual(texts.texts, [
      { text: "Overview", places: ["overview"] },
      { text: "Pumps", places: ["overview"] },
    ]);
    assert.strictEqual(texts.project.sv?.Pumps, "Pumpar");
    assert.strictEqual(texts.builtin.sv?.["No alarms"], "Inga larm");
    panel.dispose();
  });

  test("a saved change to a text file, and a text typed on the page before it is saved, reach the open editor", async () => {
    const { root, document, webview, panel } = await project({ "hmi/texts/sv.toml": SWEDISH });
    webview.send({ type: "ready" });
    await until("the texts", () => lastTexts(webview));
    fs.writeFileSync(path.join(root, "hmi", "texts", "sv.toml"), SWEDISH.replace("Pumpar", "Matarpumpar"));
    await until("the changed translation", () => lastTexts(webview)?.project.sv?.Pumps === "Matarpumpar");
    const line = document
      .getText()
      .split("\n")
      .findIndex((text) => text.startsWith("text = "));
    const edit = new vscode.WorkspaceEdit();
    edit.replace(document.uri, new vscode.Range(line, 0, line + 1, 0), 'text = "Feed pumps"\n');
    assert.ok(await vscode.workspace.applyEdit(edit));
    await until("the unsaved text", () => lastTexts(webview)?.texts.some((shown) => shown.text === "Feed pumps"));
    panel.dispose();
  });

  test("opening an HMI does not execute PLC calls to obtain dynamic alarm text", async () => {
    const { webview, panel } = await project({
      "src/alarm.st": fs.readFileSync(path.join(REPO, "libraries/alarm/src/alarm.st"), "utf8"),
      "src/main.st":
        "USING Alarm;\nPROGRAM Main\nVAR Level : LREAL; High : LimitAlarm; END_VAR\n    High(Value := Level, Limit := 90.0, Message := 'T-101 level high');\nEND_PROGRAM\n",
    });
    webview.send({ type: "ready" });
    await until("declaration variables", () => postedOfType(webview, "variables").length > 0);
    await until("project texts", () => lastTexts(webview));
    assert.ok(
      !lastTexts(webview)?.texts.some((shown) => shown.text === "T-101 level high"),
      "computed alarm text is only observed in Live mode",
    );
    panel.dispose();
  });

  test("a translation is written to the language's text file, canonical and saved; an empty one removes it (§14.5 item 7)", async () => {
    const { root, webview, panel } = await project({});
    webview.send({ type: "ready" });
    await until("the texts", () => lastTexts(webview));
    const file = path.join(root, "hmi", "texts", "sv.toml");
    const written = (): string => (fs.existsSync(file) ? fs.readFileSync(file, "utf8") : "");
    webview.send({ type: "translate", language: "sv", text: "Pumps", translation: "Pumpar" });
    await until("the new text file", () => written().includes("Pumpar"));
    assert.strictEqual(written(), SWEDISH);
    await until("the texts with it", () => lastTexts(webview)?.project.sv?.Pumps === "Pumpar");
    webview.send({ type: "translate", language: "sv", text: "Overview", translation: "Översikt" });
    await until("the second translation", () => written().includes("Översikt"));
    assert.strictEqual(
      written(),
      'format = 1\nlanguage = "sv"\n\n[texts]\n"Overview" = "Översikt"\n"Pumps" = "Pumpar"\n',
    );
    webview.send({ type: "translate", language: "sv", text: "Pumps", translation: "" });
    await until("the removed translation", () => !written().includes("Pumpar"));
    assert.strictEqual(written(), 'format = 1\nlanguage = "sv"\n\n[texts]\n"Overview" = "Översikt"\n');
    const document = vscode.workspace.textDocuments.find((open) => open.uri.fsPath === file);
    assert.ok(!document?.isDirty, "the text file is saved");
    panel.dispose();
  });

  test("the languages chosen in the Languages dialog are written to the screen layout, saved, and sent back (§14.5 item 2)", async () => {
    const { root, webview, panel } = await project({});
    webview.send({ type: "ready" });
    const texts = await until("the texts", () => lastTexts(webview));
    assert.deepStrictEqual(
      texts.shipped?.map((entry) => entry.tag),
      ["en", "zh-Hans", "de", "es", "ja", "pt-BR", "fr", "ko", "it", "tr", "sv"],
      "the languages truST ships, for the dialog",
    );
    const screen = path.join(root, "hmi", "screen.toml");
    webview.send({ type: "screenLanguages", languages: ["sv", "en", "de"] });
    await until("the new languages", () => fs.readFileSync(screen, "utf8").includes('languages = ["sv", "en", "de"]'));
    await until(
      "the editor given them",
      () =>
        lastTexts(webview)
          ?.languages.map((entry) => entry.tag)
          .join() === "sv,en,de",
    );
    const document = vscode.workspace.textDocuments.find((open) => open.uri.fsPath === screen);
    assert.ok(!document?.isDirty, "the screen layout is saved");
    panel.dispose();
  });
});

const TEXTS: ProjectTexts = {
  languages: [
    { tag: "en", name: "English" },
    { tag: "sv", name: "Svenska" },
    { tag: "de", name: "Deutsch" },
  ],
  texts: [
    { text: "Overview", places: ["overview"] },
    { text: "Pumps", places: ["overview"] },
    { text: "Start", places: ["overview"] },
  ],
  words: ["No alarms", "Running"],
  project: { sv: { Pumps: "Pumpar" } },
  builtin: {
    en: { "No alarms": "No alarms", Running: "Running" },
    sv: { "No alarms": "Inga larm", Running: "I drift" },
    de: { "No alarms": "Keine Alarme", Running: "Läuft" },
  },
  errors: [],
};

/** The languages truST ships, as the language server gives them. */
const SHIPPED = [
  { tag: "en", name: "English" },
  { tag: "zh-Hans", name: "简体中文" },
  { tag: "de", name: "Deutsch" },
  { tag: "es", name: "Español" },
  { tag: "ja", name: "日本語" },
  { tag: "pt-BR", name: "Português (Brasil)" },
  { tag: "fr", name: "Français" },
  { tag: "ko", name: "한국어" },
  { tag: "it", name: "Italiano" },
  { tag: "tr", name: "Türkçe" },
  { tag: "sv", name: "Svenska" },
];

const LANGUAGE_PAGE: HmiPageModel = {
  title: "Overview",
  object: [
    object("heading", "text", [40, 40], [240, 40], { text: "Pumps" }),
    object("start", "text", [40, 120], [160, 40], { text: "Start" }),
  ],
  pipe: [],
};

const headingText = (view: Webview): string | undefined =>
  view.query('svg [data-object="heading"] text')?.textContent ?? undefined;

/** A press and a release in one place on the canvas at zoom 1: a click. */
function click(view: Webview, at: [number, number]): void {
  view.mouse("mousedown", view.layer(), at[0], at[1]);
  view.mouse("mouseup", view.window, at[0], at[1]);
}

suite("HMI Builder editor: languages in the webview", function () {
  this.timeout(20000);

  test("a picker above the canvas shows the page in each of the project's languages (§14.5 item 7)", async () => {
    const view = new Webview();
    await view.loaded(LANGUAGE_PAGE);
    view.send({ type: "texts", texts: { ...TEXTS, languages: [TEXTS.languages[0]] } });
    await view.until("the language tools", () => view.query('[data-action="longest"]'));
    assert.strictEqual(
      view.query('[data-part="language-picker"]'),
      null,
      "a project in one language has nothing to pick",
    );
    view.send({ type: "texts", texts: TEXTS });
    const picker = await view.until("the picker", () => view.query('[data-part="language-picker"]'));
    assert.deepStrictEqual(
      [...picker.querySelectorAll("option")].map((option: { textContent: string }) => option.textContent),
      ["English", "Svenska", "Deutsch"],
    );
    view.choose(picker, "sv");
    await view.until("the page in Swedish", () => headingText(view) === "Pumpar");
    assert.strictEqual(
      view.query(".hb-drawing")?.getAttribute("lang"),
      "sv",
      "the drawing speaks the language shown, for the forms of its font",
    );
    view.choose(picker, "de");
    await view.until("as written where German has no text", () => headingText(view) === "Pumps");
  });

  test("Languages… offers truST's languages as tick boxes and gives the chosen ones, the default first (§14.5 item 2)", async () => {
    const view = new Webview();
    await view.loaded(LANGUAGE_PAGE);
    view.send({ type: "texts", texts: { ...TEXTS, languages: [TEXTS.languages[0]], shipped: SHIPPED } });
    (await view.until("the Languages button", () => view.query('[data-action="languages"]'))).click();
    const dialog = await view.until("the dialog", () => view.query('[data-part="languages-dialog"]'));
    const rows = [...dialog.querySelectorAll("[data-language]")].map((row: { getAttribute(name: string): string }) =>
      row.getAttribute("data-language"),
    );
    assert.deepStrictEqual(
      rows,
      SHIPPED.map((entry) => entry.tag),
      "every language truST ships, by its own name",
    );
    const box = (tag: string) => dialog.querySelector(`[data-language="${tag}"] input[type="checkbox"]`);
    const fallback = (tag: string) => dialog.querySelector(`[data-language="${tag}"] input[type="radio"]`);
    assert.ok(box("en").checked && box("en").disabled, "the only language stays: a project has at least one");
    assert.ok(dialog.querySelector('[data-action="apply"]').disabled, "nothing changed yet");
    view.check(box("sv"), true);
    view.check(box("de"), true);
    await view.until("English may go once others are chosen", () => !box("en").disabled);
    fallback("sv").click();
    await view.until("Svenska the default", () => fallback("sv").checked);
    dialog.querySelector('[data-action="apply"]').click();
    await view.until("the languages sent", () => view.posted.some((message) => message.type === "screenLanguages"));
    const sent = view.posted.filter((message) => message.type === "screenLanguages").pop();
    assert.deepStrictEqual(sent?.languages, ["sv", "en", "de"], "the default first, then the others as listed");
    assert.strictEqual(view.query('[data-part="languages-dialog"]'), null, "the dialog closes");
  });

  test("in a language other than the first, a click on a text edits its translation in place (§14.5 item 7)", async () => {
    const view = new Webview();
    await view.loaded(LANGUAGE_PAGE);
    view.send({ type: "texts", texts: TEXTS });
    const picker = await view.until("the picker", () => view.query('[data-part="language-picker"]'));
    click(view, [60, 60]);
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.strictEqual(
      view.query('[data-part="translation"]'),
      null,
      "in the first language a click selects, as always",
    );
    view.choose(picker, "sv");
    await view.until("Swedish", () => headingText(view) === "Pumpar");
    click(view, [60, 60]);
    const field = await view.until("the translation field", () => view.query('[data-part="translation"]'));
    assert.match(field.textContent, /Pumps/, "it shows the text as written");
    const input = field.querySelector("input");
    assert.strictEqual(input.value, "Pumpar");
    const posted = view.posted.length;
    const edits = view.edits().length;
    view.type(input, "Matarpumpar");
    const sent = await view.until("the translation", () =>
      view.posted.slice(posted).find((message) => message.type === "translate"),
    );
    assert.deepStrictEqual(sent, { type: "translate", language: "sv", text: "Pumps", translation: "Matarpumpar" });
    assert.strictEqual(view.edits().length, edits, "the page itself is not edited");
    await view.until("the field closes", () => !view.query('[data-part="translation"]'));
    click(view, [60, 140]);
    const missing = await view.until("the field of a text without a translation", () =>
      view.query('[data-part="translation"] input'),
    );
    assert.strictEqual(missing.value, "", "it starts empty");
    assert.strictEqual(missing.placeholder, "Start", "and shows the text as written");
  });

  test("in a language other than the first, the inspector's text fields say the language they are written in", async () => {
    const view = new Webview();
    await view.loaded(LANGUAGE_PAGE);
    view.send({ type: "texts", texts: TEXTS });
    const picker = await view.until("the picker", () => view.query('[data-part="language-picker"]'));
    click(view, [60, 60]);
    const label = (name: string): string | undefined =>
      [...view.document.querySelectorAll(".hb-field")]
        .find((field: any) => field.querySelector(`input[name="${name}"]`))
        ?.querySelector(".hb-field-label")?.textContent;
    await view.until("the inspector of the heading", () => label("text"));
    assert.strictEqual(label("text"), "Text");
    view.choose(picker, "sv");
    await view.until("the language the text is written in", () => label("text") === "Text · English");
    assert.strictEqual(label("label"), "Label · English");
    assert.strictEqual(label("tag"), "Tag", "a tag is a name in every language");
  });

  test("the Texts table lists every text in every language, marks the missing ones, and a cell edits the translation (§14.5 items 4, 7)", async () => {
    const view = new Webview();
    await view.loaded(LANGUAGE_PAGE);
    view.send({ type: "texts", texts: TEXTS });
    (await view.until("the Texts tab", () => view.query('[data-action="toggle-texts"]'))).click();
    const table = await view.until("the table", () => view.query('[data-part="texts-table"]'));
    assert.deepStrictEqual(
      [...table.querySelectorAll("thead th")].map((cell: { textContent: string }) => cell.textContent),
      ["English", "Svenska", "Deutsch"],
    );
    const cell = (text: string, language: string): any =>
      [...table.querySelectorAll("tbody tr")]
        .find((row: any) => row.getAttribute("data-text") === text)
        ?.querySelector(`td[data-language="${language}"]`);
    assert.strictEqual(cell("Pumps", "en")?.textContent, "Pumps", "the first language as written");
    assert.strictEqual(cell("Pumps", "sv")?.querySelector("input")?.value, "Pumpar");
    assert.strictEqual(cell("Pumps", "sv")?.getAttribute("data-missing"), null);
    assert.strictEqual(cell("Pumps", "de")?.getAttribute("data-missing"), "true");
    assert.strictEqual(
      cell("Running", "sv")?.querySelector("input")?.placeholder,
      "I drift",
      "truST's words come translated",
    );
    assert.strictEqual(cell("Running", "sv")?.getAttribute("data-missing"), null);
    assert.match(view.query('[data-part="texts-summary"]')?.textContent ?? "", /5 missing/);
    const posted = view.posted.length;
    view.type(cell("Pumps", "de").querySelector("input"), "Pumpen");
    const sent = await view.until("the translation", () =>
      view.posted.slice(posted).find((message) => message.type === "translate"),
    );
    assert.deepStrictEqual(sent, { type: "translate", language: "de", text: "Pumps", translation: "Pumpen" });
  });

  test("Longest lengthens every text on the canvas by 40 % to find the tight objects before translating (§14.5 item 7)", async () => {
    const view = new Webview();
    await view.loaded(LANGUAGE_PAGE);
    view.send({ type: "texts", texts: TEXTS });
    const longest = await view.until("Longest", () => view.query('[data-action="longest"]'));
    longest.click();
    const shown = await view.until("the lengthened text", () => {
      const text = headingText(view);
      return text !== undefined && text.startsWith("Pumps") && text !== "Pumps" ? text : undefined;
    });
    assert.ok(textWidth(shown, 14, 400) >= 1.4 * textWidth("Pumps", 14, 400), shown);
    assert.strictEqual(longest.getAttribute("aria-pressed"), "true");
    longest.click();
    await view.until("as written again", () => headingText(view) === "Pumps");
  });
});
