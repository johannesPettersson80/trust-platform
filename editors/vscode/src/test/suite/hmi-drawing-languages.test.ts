import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { drawPage } from "../../hmiBuilder/drawing";
import { collectCuts } from "../../hmiBuilder/drawing/cuts";
import { textWidth } from "../../hmiBuilder/drawing/format";
import type { DrawFrame, HmiObjectModel, HmiPageModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";
import { WORDS } from "../../hmiBuilder/drawing/words";

// Specification 39, §14.5: every word the drawing shows goes through the screen's text table, a
// text with a value keeps it, a project's own texts are shown in the language, and numbers follow
// the language.

const ROOT = path.resolve(__dirname, "../../../../..");

function draw(object: HmiObjectModel, values: Record<string, LiveValue>, frame: Partial<DrawFrame> = {}): SvgNode {
  const svg = drawPage({ title: "Languages", object: [object], pipe: [] }, { size: [1280, 720], values, ...frame });
  const found = findAll(svg, (node) => node.attrs["data-object"] === object.id);
  assert.strictEqual(found.length, 1, renderToString(svg));
  return found[0];
}

function bound(type: string, bind: Record<string, string>, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  const bindings = Object.fromEntries(
    Object.entries(bind).map(([property, at]) => [property, { path: at, direction: "read" as const }]),
  );
  return { id: type, type, at: [0, 0], size: [128, 108], rotation: 0, bind: bindings, ...extra };
}

const confirmed = (value: boolean | number): LiveValue => ({ value, state: "confirmed" });

suite("HMI drawing: languages", () => {
  test("the words the drawing shows are exactly truST's English words", () => {
    const table = fs.readFileSync(path.join(ROOT, "crates/trust-runtime/src/hmi/texts/builtin/en.toml"), "utf8");
    const keys = [...table.matchAll(/^("(?:[^"\\]|\\.)*") = /gm)].map((match) => JSON.parse(match[1]) as string).sort();
    assert.deepStrictEqual([...WORDS].sort(), keys);
  });

  test("the simulation badge's words are truST's words, and every built-in language translates them (§8 item 8)", () => {
    assert.ok(WORDS.includes("SIMULATION") && WORDS.includes("{achieved} of {configured}"));
    const badge: Record<string, [string, string]> = {
      en: ["SIMULATION", "{achieved} of {configured}"],
      de: ["SIMULATION", "{achieved} von {configured}"],
      es: ["SIMULACIÓN", "{achieved} de {configured}"],
      fr: ["SIMULATION", "{achieved} sur {configured}"],
      it: ["SIMULAZIONE", "{achieved} di {configured}"],
      ja: ["シミュレーション", "{achieved} / {configured}"],
      ko: ["시뮬레이션", "{achieved} / {configured}"],
      "pt-BR": ["SIMULAÇÃO", "{achieved} de {configured}"],
      sv: ["SIMULERING", "{achieved} av {configured}"],
      tr: ["SİMÜLASYON", "{achieved} / {configured}"],
      "zh-Hans": ["仿真", "{achieved} / {configured}"],
    };
    for (const [tag, [label, pattern]] of Object.entries(badge)) {
      const table = fs.readFileSync(path.join(ROOT, `crates/trust-runtime/src/hmi/texts/builtin/${tag}.toml`), "utf8");
      const texts = new Map(
        [...table.matchAll(/^("(?:[^"\\]|\\.)*") = ("(?:[^"\\]|\\.)*")/gm)].map(
          (match) => [JSON.parse(match[1]) as string, JSON.parse(match[2]) as string] as const,
        ),
      );
      assert.strictEqual(texts.get("SIMULATION"), label, `${tag}: the badge's label`);
      assert.strictEqual(texts.get("{achieved} of {configured}"), pattern, `${tag}: the speed pattern`);
    }
  });

  test("a symbol's state word and signs come from the screen's text table", () => {
    const pump = bound("pump", { running: "P.Running", manual: "P.Manual" }, { tag: "P-101" });
    const drawn = draw(
      pump,
      { "P.Running": confirmed(true), "P.Manual": confirmed(true) },
      { texts: { Running: "I drift", M: "H" }, language: "sv" },
    );
    assert.ok(textsOf(drawn).includes("I drift"), renderToString(drawn));
    assert.ok(textsOf(drawn).includes("H"), renderToString(drawn));
    assert.ok(textsOf(drawn).includes("P-101"), "a tag is a name and stays as it is");
    const plain = draw(pump, { "P.Running": confirmed(true), "P.Manual": confirmed(false) });
    assert.ok(textsOf(plain).includes("Running"), "a word without a translation shows as written");
  });

  test("a text with a value keeps the value in the place the language puts it", () => {
    const valve = bound("control-valve", { position: "FV.Position" });
    const turkish = draw(
      valve,
      { "FV.Position": confirmed(62) },
      { texts: { "{value} %": "%{value}" }, language: "tr" },
    );
    assert.ok(textsOf(turkish).includes("%62"), renderToString(turkish));
    const stale = draw(
      bound("pump", { running: "P.Running" }),
      { "P.Running": { value: true, state: "confirmed", staleMs: 12_000 } },
      { texts: { "{seconds} s": "{seconds} sn" }, language: "tr" },
    );
    assert.ok(textsOf(stale).includes("12 sn"), renderToString(stale));
  });

  test("a project's own texts are shown in the language", () => {
    const card = bound("value", { value: "P.Speed" }, { label: "Pump speed", size: [240, 96] });
    const drawn = draw(
      card,
      { "P.Speed": confirmed(1180) },
      { texts: { "Pump speed": "Pumpvarvtal" }, language: "sv" },
    );
    assert.ok(textsOf(drawn).includes("Pumpvarvtal"), renderToString(drawn));
  });

  test("the longest preview lengthens every text by 40 % and leaves tags and numbers as they are (§14.5 item 7)", () => {
    const pump = bound("pump", { running: "P.Running" }, { tag: "P-101" });
    const drawn = textsOf(draw(pump, { "P.Running": confirmed(true) }, { longest: true }));
    const word = drawn.find((shown) => shown.startsWith("Running"));
    assert.ok(word !== undefined && word !== "Running", String(drawn));
    assert.ok(textWidth(word, 14, 400) >= 1.4 * textWidth("Running", 14, 400), word);
    assert.ok(drawn.includes("P-101"), "a tag is a name, not a text");
    const card = bound("value", { value: "P.Speed" }, { label: "Pump speed", size: [320, 96], decimals: 0 });
    const shown = textsOf(draw(card, { "P.Speed": confirmed(1180) }, { longest: true }));
    const label = shown.find((text) => text.startsWith("Pump speed"));
    assert.ok(
      label !== undefined && textWidth(label, 14, 400) >= 1.4 * textWidth("Pump speed", 14, 400),
      String(shown),
    );
    const number = shown.find((text) => text.includes("1180"));
    assert.ok(number !== undefined && !number.includes("·"), `the number as it is: ${String(shown)}`);
  });

  test("the texts a drawing cuts to fit are given with their object; a value is not a text (§14.5 item 6)", () => {
    const narrow = (
      id: string,
      type: string,
      at: [number, number],
      extra: Partial<HmiObjectModel>,
    ): HmiObjectModel => ({ id, type, at, size: [96, 40], rotation: 0, bind: {}, ...extra });
    const page: HmiPageModel = {
      title: "Cuts",
      object: [
        narrow("heading", "text", [0, 0], { text: "A heading far too long for its box" }),
        narrow("start", "button", [0, 48], { text: "Start the feed pump now" }),
        narrow("fits", "text", [0, 96], { text: "Fits" }),
        narrow("name", "text", [0, 144], { bind: { text: { path: "P.Name", direction: "read" } } }),
      ],
      pipe: [],
    };
    const values: Record<string, LiveValue> = {
      "P.Name": { value: "A very long name from the running program", state: "confirmed" },
    };
    const english = collectCuts(() => drawPage(page, { size: [1280, 720], values }));
    assert.deepStrictEqual(english.cuts, [
      { object: "heading", text: "A heading far too long for its box" },
      { object: "start", text: "Start the feed pump now" },
    ]);
    const heading = findAll(english.drawn, (node) => node.attrs["data-object"] === "heading")[0];
    assert.ok(
      textsOf(heading).some((text) => text.endsWith("…")),
      `the text is cut with …: ${renderToString(heading)}`,
    );
    const swedish = collectCuts(() =>
      drawPage(page, { size: [1280, 720], values, texts: { Fits: "Passar inte alls i rutan" }, language: "sv" }),
    );
    assert.ok(
      swedish.cuts.some((cut) => cut.object === "fits" && cut.text === "Passar inte alls i rutan"),
      JSON.stringify(swedish.cuts),
    );
  });

  test("numbers follow the language: a decimal comma in Swedish and German", () => {
    const card = bound("value", { value: "P.Flow" }, { decimals: 1, size: [240, 96] });
    const shown = (language: string | undefined): string[] =>
      textsOf(draw(card, { "P.Flow": confirmed(1234.5) }, language ? { language } : {}));
    assert.ok(
      shown("sv").some((words) => words.includes("1234,5")),
      String(shown("sv")),
    );
    assert.ok(
      shown("de").some((words) => words.includes("1234,5")),
      String(shown("de")),
    );
    assert.ok(
      shown(undefined).some((words) => words.includes("1234.5")),
      String(shown(undefined)),
    );
  });
});
