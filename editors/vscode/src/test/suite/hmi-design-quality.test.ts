import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import * as os from "os";
import * as vscode from "vscode";

import { drawnTexts, type DrawnText } from "../../hmiBuilder/drawing/contrast";
import { el, findAll, text } from "../../hmiBuilder/drawing/svg";
import { TOKEN_THEMES } from "../../hmiBuilder/drawing/tokens";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import { SPECIMEN_STATES, specimenDrawings } from "../../hmiBuilder/editor/specimen";
import { languageServerReady } from "./hmiBuilderHost";

// Specification 39, §14.1 items 1 and 4, §3.5 item 2: what each text of the drawing lies on, and the
// specimen of every component in every state the checks measure. How well a text stands out and
// what it needs are the language server's rules (`trust-lsp.hmiQuality`, tested in Rust); the
// shipped tokens are checked through it.

const CATALOGUE: Catalogue = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../../../../../crates/trust-runtime/src/hmi/catalogue/components.json"),
    "utf8",
  ),
);

const tokensOf = (layers: Array<Array<{ token: string }>>): string[][] =>
  layers.map((layer) => layer.map((backing) => backing.token));

suite("HMI design system: measuring text", () => {
  test("each text lies on the shapes drawn before it under its middle, topmost first", () => {
    const svg = el("svg", {}, [
      el("rect", { "data-part": "page", x: 0, y: 0, width: 400, height: 200, fill: "var(--hmi-page)" }),
      el("g", { "data-object": "card", transform: "translate(100 50)" }, [
        el("rect", { x: 0, y: 0, width: 160, height: 60, fill: "var(--hmi-surface)" }),
        el("path", { d: "M140 18L152 30L140 42L128 30Z", fill: "var(--hmi-alarm)" }),
        el("path", { d: "M10 50L150 50", fill: "none", stroke: "var(--hmi-accent)" }),
        text("Speed", { x: 12, y: 24, fill: "var(--hmi-muted)", "font-size": 14 }),
        text("!", {
          x: 140,
          y: 35,
          fill: "var(--hmi-page)",
          "font-size": 14,
          "font-weight": 800,
          "text-anchor": "middle",
        }),
      ]),
      text("Outside", { x: 300, y: 150, fill: "var(--hmi-text)", "font-size": 24 }),
    ]);
    const texts = drawnTexts(svg);
    const by = (content: string) => texts.find((drawn) => drawn.text === content);
    assert.deepStrictEqual(tokensOf(by("Speed")?.behind ?? []), [["surface"], ["page"]]);
    assert.deepStrictEqual(tokensOf(by("!")?.behind ?? [])[0], ["alarm"], "the diamond under the sign");
    assert.deepStrictEqual(tokensOf(by("Outside")?.behind ?? []), [["page"]]);
    assert.strictEqual(by("Speed")?.object, "card");
    assert.strictEqual(by("Speed")?.colour, "muted");
    assert.strictEqual(by("!")?.bold, true);
    assert.strictEqual(by("Outside")?.size, 24);
  });

  test("a see-through shape and a radial glow's stops are what a text lies on, with its size and weight", () => {
    const svg = el("svg", {}, [
      el("defs", {}, [
        el("radialGradient", { id: "g" }, [
          el("stop", { offset: 0, "stop-color": "var(--hmi-light)" }),
          el("stop", { offset: 1, "stop-color": "var(--hmi-dark)" }),
        ]),
      ]),
      el("rect", { x: 0, y: 0, width: 400, height: 400, fill: "var(--hmi-page)" }),
      el("rect", { x: 0, y: 0, width: 200, height: 100, fill: "var(--hmi-veil)", "fill-opacity": 0.5 }),
      text("Veiled", { x: 10, y: 50, fill: "var(--hmi-ink)", "font-size": 14 }),
      el("rect", { x: 0, y: 200, width: 400, height: 100, fill: "url(#g)" }),
      text("Graded", { x: 10, y: 250, fill: "var(--hmi-grey)", "font-size": 19, "font-weight": 700 }),
    ]);
    const texts = drawnTexts(svg);
    const by = (content: string) => texts.find((drawn) => drawn.text === content)!;
    assert.deepStrictEqual(by("Veiled").behind, [[{ token: "veil", opacity: 0.5 }], [{ token: "page", opacity: 1 }]]);
    assert.deepStrictEqual(
      tokensOf(by("Graded").behind),
      [["light", "dark"], ["page"]],
      "each stop of the glow is one of what the text may lie on",
    );
    assert.strictEqual(by("Graded").size, 19);
    assert.strictEqual(by("Graded").bold, true);
  });

  test("a text of symbols only is a sign, words and numbers are not", () => {
    const svg = el("svg", {}, [
      el("rect", { x: 0, y: 0, width: 400, height: 400, fill: "var(--hmi-page)" }),
      ...["?", "●", "…", "!", "12 s", "→ 70", "Off"].map((content, index) =>
        text(content, { x: 10, y: 30 + index * 40, fill: "var(--hmi-ink)", "font-size": 14 }),
      ),
    ]);
    assert.deepStrictEqual(
      drawnTexts(svg).map((drawn) => [drawn.text, drawn.sign]),
      [
        ["?", true],
        ["●", true],
        ["…", true],
        ["!", true],
        ["12 s", false],
        ["→ 70", false],
        ["Off", false],
      ],
    );
  });

  test("a horizontal gradient is taken where the text lies on it, not at its far stop", () => {
    const svg = el("svg", {}, [
      el("defs", {}, [
        el("linearGradient", { id: "h", x1: 0, y1: 0, x2: 1, y2: 0 }, [
          el("stop", { offset: 0, "stop-color": "var(--hmi-light)" }),
          el("stop", { offset: 1, "stop-color": "var(--hmi-dark)" }),
        ]),
      ]),
      el("rect", { x: 0, y: 0, width: 400, height: 400, fill: "var(--hmi-page)" }),
      el("g", { transform: "translate(100 100)" }, [
        el("rect", { x: 0, y: 0, width: 200, height: 40, fill: "url(#h)" }),
        text("AB", { x: 100, y: 25, fill: "var(--hmi-ink)", "font-size": 14, "text-anchor": "middle" }),
      ]),
    ]);
    // The text is 0.55 × 14 × 2 = 15.4 wide around x 200: from 192.3 to 207.7 on the gradient that runs
    // from 100 to 300, so at its ends and middle 46.15 %, 50 % and 53.85 % of the way to the dark stop.
    const [samples] = drawnTexts(svg)[0].behind;
    assert.deepStrictEqual(
      samples.map((backing) => [
        backing.token,
        backing.mix?.token,
        Math.round((backing.mix?.share ?? -1) * 10000) / 10000,
      ]),
      [
        ["light", "dark", 0.4615],
        ["light", "dark", 0.5],
        ["light", "dark", 0.5385],
      ],
    );
  });

  test("the specimen draws every phase 1 component of the catalogue in every state", () => {
    const drawings = specimenDrawings(CATALOGUE);
    assert.deepStrictEqual(
      drawings.map((drawing) => drawing.state),
      [...SPECIMEN_STATES],
    );
    assert.deepStrictEqual(
      [...SPECIMEN_STATES],
      ["normal", "unknown", "stale", "commanded", "pending", "refused", "read-only", "missing", "alarms"],
    );
    for (const drawing of drawings) {
      for (const component of CATALOGUE.components.filter((candidate) => candidate.phase === 1)) {
        assert.strictEqual(
          findAll(drawing.svg, (node) => node.attrs["data-object"] === component.type).length,
          1,
          `${component.type} in the ${drawing.state} drawing`,
        );
      }
    }
  });
});

suite("HMI design system: the shipped tokens", function () {
  this.timeout(60000);

  suiteSetup(languageServerReady);

  for (const theme of ["dark", "light"] as const) {
    test(`the ${theme} tokens give every text of the drawing enough contrast, in every state`, async () => {
      // The controls of a read-only page are inactive and need no contrast (WCAG 2.1, 1.4.3).
      const texts: DrawnText[] = specimenDrawings(CATALOGUE)
        .filter((candidate) => candidate.state !== "read-only")
        .flatMap((drawing) => drawnTexts(drawing.svg));
      // Every shipped token given as one of the project's colours, so the language server's check of
      // a project's colours checks them all.
      const answer = await vscode.commands.executeCommand<{
        ok?: boolean;
        colours?: Array<{ kind: string; message: string }>;
      }>("trust-lsp.hmiQuality", {
        root_uri: vscode.Uri.file(os.tmpdir()).toString(),
        colours: { defaults: TOKEN_THEMES, own: { [theme]: Object.entries(TOKEN_THEMES[theme]) }, texts },
      });
      assert.strictEqual(answer?.ok, true);
      assert.deepStrictEqual(
        (answer?.colours ?? []).filter((warning) => warning.kind === "contrast").map((warning) => warning.message),
        [],
      );
    });
  }

  test("the drawing draws no words or numbers below 14 px at the design size, in any state", () => {
    const small = new Set<string>();
    for (const drawing of specimenDrawings(CATALOGUE)) {
      for (const drawn of drawnTexts(drawing.svg)) {
        if (!drawn.sign && drawn.size < 14)
          small.add(`${drawing.state} ${drawn.object ?? "page"} "${drawn.text}" ${drawn.size}px`);
      }
    }
    assert.deepStrictEqual([...small], []);
  });
});
