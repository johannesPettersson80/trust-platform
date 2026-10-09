import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { contrastRatio, drawnTexts, oklabDistance, textContrast } from "../../hmiBuilder/drawing/contrast";
import { el, findAll, text } from "../../hmiBuilder/drawing/svg";
import { TOKEN_THEMES } from "../../hmiBuilder/drawing/tokens";
import type { Catalogue } from "../../hmiBuilder/editor/catalogue";
import { SPECIMEN_STATES, specimenDrawings } from "../../hmiBuilder/editor/specimen";

// Specification 39, §14.1 items 1 and 4, §3.5 item 2: what each text of the drawing lies on, how
// well it stands out, and the specimen of every component in every state the checks measure.

const CATALOGUE: Catalogue = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../../../../../crates/trust-runtime/src/hmi/catalogue/components.json"),
    "utf8",
  ),
);

const tokensOf = (layers: Array<Array<{ token: string }>>): string[][] =>
  layers.map((layer) => layer.map((backing) => backing.token));

suite("HMI design system: measuring text", () => {
  test("the contrast ratio follows WCAG 2.1", () => {
    assert.strictEqual(contrastRatio("#000000", "#ffffff"), 21);
    assert.strictEqual(Math.round(contrastRatio("#777777", "#ffffff") * 100) / 100, 4.48);
    assert.strictEqual(contrastRatio("#fff", "#ffffff"), 1);
  });

  test("the OKLab distance is 0 for one colour and 1 from black to white", () => {
    assert.strictEqual(oklabDistance("#22d3ee", "#22d3ee"), 0);
    assert.ok(Math.abs(oklabDistance("#000000", "#ffffff") - 1) < 0.001, String(oklabDistance("#000000", "#ffffff")));
  });

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

  test("contrast is taken at a radial glow's strongest stop and through see-through shapes, with the size deciding what is needed", () => {
    const palette = {
      page: "#ffffff",
      veil: "#000000",
      ink: "#000000",
      grey: "#777777",
      light: "#ffffff",
      dark: "#000000",
    };
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
      text("Plain", { x: 300, y: 380, fill: "var(--hmi-grey)", "font-size": 19 }),
    ]);
    const texts = drawnTexts(svg);
    const measure = (content: string) =>
      textContrast(
        texts.find((drawn) => drawn.text === content)!,
        palette,
      );
    const veiled = measure("Veiled");
    assert.ok(
      Math.abs(veiled.ratio - contrastRatio("#000000", "#808080")) < 0.05,
      `black on half-black over white: ${veiled.ratio}`,
    );
    assert.strictEqual(veiled.needed, 4.5);
    const graded = measure("Graded");
    assert.ok(
      Math.abs(graded.ratio - contrastRatio("#777777", "#ffffff")) < 0.01,
      `the worse of the two stops: ${graded.ratio}`,
    );
    assert.strictEqual(graded.needed, 3, "19 px bold is large text");
    assert.strictEqual(measure("Plain").needed, 4.5, "19 px regular is not");
  });

  test("a sign needs 3:1 and words and numbers 4.5:1", () => {
    const palette = { page: "#ffffff", ink: "#777777" };
    const svg = el("svg", {}, [
      el("rect", { x: 0, y: 0, width: 400, height: 400, fill: "var(--hmi-page)" }),
      ...["?", "●", "…", "!", "12 s", "→ 70", "Off"].map((content, index) =>
        text(content, { x: 10, y: 30 + index * 40, fill: "var(--hmi-ink)", "font-size": 14 }),
      ),
    ]);
    const needed = drawnTexts(svg).map((drawn) => [drawn.text, drawn.sign, textContrast(drawn, palette).needed]);
    assert.deepStrictEqual(needed, [
      ["?", true, 3],
      ["●", true, 3],
      ["…", true, 3],
      ["!", true, 3],
      ["12 s", false, 4.5],
      ["→ 70", false, 4.5],
      ["Off", false, 4.5],
    ]);
  });

  test("a horizontal gradient counts where the text lies on it, not at its far stop", () => {
    const palette = { page: "#000000", light: "#ffffff", dark: "#000000", ink: "#ffffff" };
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
    const { ratio } = textContrast(drawnTexts(svg)[0], palette);
    // The text spans about 46 % to 54 % of the gradient; its lighter end is the worst.
    const grey = Math.round((1 - (100 - 0.55 * 14) / 200) * 255)
      .toString(16)
      .padStart(2, "0");
    assert.ok(
      Math.abs(ratio - contrastRatio("#ffffff", `#${grey}${grey}${grey}`)) < 0.1,
      `${ratio} against #${grey}${grey}${grey}`,
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

suite("HMI design system: the shipped tokens", () => {
  const describe = (layers: Array<Array<{ token: string; opacity: number }>>): string =>
    layers
      .map((layer) =>
        layer.map((backing) => (backing.opacity < 1 ? `${backing.token}@${backing.opacity}` : backing.token)).join("|"),
      )
      .join(" over ");

  for (const theme of ["dark", "light"] as const) {
    test(`the ${theme} tokens give every text of the drawing enough contrast, in every state`, () => {
      const failures = new Set<string>();
      // The controls of a read-only page are inactive and need no contrast (WCAG 2.1, 1.4.3).
      for (const drawing of specimenDrawings(CATALOGUE).filter((candidate) => candidate.state !== "read-only")) {
        for (const drawn of drawnTexts(drawing.svg)) {
          const { ratio, needed } = textContrast(drawn, TOKEN_THEMES[theme]);
          if (ratio < needed) {
            failures.add(
              `${drawn.object ?? "page"} "${drawn.text}" ${drawn.size}px: ${drawn.colour} on ${describe(drawn.behind)} = ${ratio.toFixed(2)} < ${needed}`,
            );
          }
        }
      }
      assert.deepStrictEqual([...failures], []);
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
