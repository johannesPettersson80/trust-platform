import { textWidth } from "../../hmiBuilder/drawing/format";
import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { drawPage } from "../../hmiBuilder/drawing";
import type { DrawFrame, HmiObjectModel, HmiPageModel, LiveValue } from "../../hmiBuilder/drawing/model";
import { findAll, renderToString, SvgNode, textsOf } from "../../hmiBuilder/drawing/svg";
import { TOKEN_THEMES, tokensCss } from "../../hmiBuilder/drawing/tokens";

// Specification 39, §3.4 and §5.3: the drawing module.

function page(...objects: HmiObjectModel[]): HmiPageModel {
  return { title: "Test", object: objects, pipe: [] };
}

function object(id: string, type: string, extra: Partial<HmiObjectModel> = {}): HmiObjectModel {
  return { id, type, at: [10, 20], size: [160, 48], rotation: 0, bind: {}, ...extra };
}

function bound(
  id: string,
  type: string,
  property: string,
  path: string,
  extra: Partial<HmiObjectModel> = {},
): HmiObjectModel {
  return object(id, type, { bind: { [property]: { path, direction: "read" } }, ...extra });
}

function draw(model: HmiPageModel, values: Record<string, LiveValue> = {}): SvgNode {
  const frame: DrawFrame = { size: [1280, 720], values };
  return drawPage(model, frame);
}

function objectNode(svg: SvgNode, id: string): SvgNode {
  const found = findAll(svg, (node) => node.attrs["data-object"] === id);
  assert.strictEqual(found.length, 1, `object ${id}: ${renderToString(svg)}`);
  return found[0];
}

function joinedText(node: SvgNode): string {
  return textsOf(node).join(" ");
}

suite("HMI drawing", () => {
  test("the page is drawn at its design size with each object at its position, in z-order", () => {
    const svg = draw(
      page(
        object("first", "value", { at: [10, 20] }),
        object("second", "value", { at: [200, 40], rotation: 90, size: [100, 50] }),
      ),
    );
    assert.strictEqual(svg.tag, "svg");
    assert.strictEqual(svg.attrs.viewBox, "0 0 1280 720");
    const ids = findAll(svg, (node) => node.attrs["data-object"] !== undefined).map(
      (node) => node.attrs["data-object"],
    );
    assert.deepStrictEqual(ids, ["first", "second"]);
    assert.strictEqual(objectNode(svg, "first").attrs.transform, "translate(10 20)");
    assert.strictEqual(objectNode(svg, "second").attrs.transform, "translate(200 40) rotate(90 50 25)");
  });

  test("a value shows its number with the object's decimals and unit", () => {
    const svg = draw(page(bound("level", "value", "value", "Main.Level", { decimals: 1, unit: "%" })), {
      "Main.Level": { value: 62.54, state: "confirmed" },
    });
    const node = objectNode(svg, "level");
    assert.match(joinedText(node), /62\.5\b/);
    assert.match(joinedText(node), /%/);
    assert.strictEqual(node.attrs["data-state"], "confirmed");
  });

  test("an unknown value shows a question mark and a neutral hatch, never the last good value", () => {
    const svg = draw(page(bound("level", "value", "value", "Main.Level", { decimals: 1 })), {
      "Main.Level": { value: 62.5, state: "unknown" },
    });
    const node = objectNode(svg, "level");
    assert.ok(textsOf(node).includes("?"), joinedText(node));
    assert.doesNotMatch(joinedText(node), /62\.5/);
    assert.strictEqual(node.attrs["data-state"], "unknown");
    assert.ok(findAll(node, (child) => child.attrs.fill === "url(#hmi-unknown-hatch)").length > 0);
    assert.ok(findAll(svg, (child) => child.attrs.id === "hmi-unknown-hatch").length === 1);
  });

  test("a bound path without a live value is unknown", () => {
    const node = objectNode(draw(page(bound("level", "value", "value", "Main.Level"))), "level");
    assert.strictEqual(node.attrs["data-state"], "unknown");
    assert.ok(textsOf(node).includes("?"));
  });

  test("an object bound to a path the running program does not have says so", () => {
    const model = page(bound("level", "value", "value", "Main.Gone"), bound("speed", "value", "value", "Main.Speed"));
    const svg = drawPage(model, { size: [1280, 720], values: {}, missing: ["Main.Gone"] });
    const level = objectNode(svg, "level");
    assert.strictEqual(level.attrs["data-missing"], "true");
    assert.match(joinedText(level), /Not in running program/);
    const speed = objectNode(svg, "speed");
    assert.strictEqual(speed.attrs["data-missing"], undefined);
    assert.doesNotMatch(joinedText(speed), /Not in running program/);
  });

  test("an object with a binding problem is outlined in the alarm colour, its problems on hover", () => {
    const model = page(bound("level", "value", "value", "Main.Gone"), bound("speed", "value", "value", "Main.Speed"));
    const svg = drawPage(model, {
      size: [1280, 720],
      values: {},
      problems: { level: ['"Main.Gone" is not a variable of the program'] },
    });
    const level = objectNode(svg, "level");
    assert.strictEqual(level.attrs["data-problems"], "true");
    const mark = findAll(level, (child) => child.attrs["data-part"] === "problem")[0];
    assert.ok(mark, joinedText(level));
    assert.ok(findAll(mark, (child) => child.attrs.stroke === "var(--hmi-alarm)").length > 0, "the alarm colour");
    assert.ok(textsOf(mark).includes("!"), "a sign beside the colour");
    const title = findAll(level, (child) => child.tag === "title")[0];
    assert.strictEqual(title?.text, '"Main.Gone" is not a variable of the program');
    assert.strictEqual(objectNode(svg, "speed").attrs["data-problems"], undefined);
  });

  test("the problem mark leaves the object's label and value readable", () => {
    const model = page({ ...bound("level", "value", "value", "Main.Gone"), label: "Feed pump P101", unit: "rpm" });
    const svg = drawPage(model, { size: [1280, 720], values: {}, problems: { level: ["gone"] } });
    const level = objectNode(svg, "level");
    const mark = findAll(level, (child) => child.attrs["data-part"] === "problem")[0];
    const circle = findAll(mark, (child) => child.tag === "circle")[0];
    assert.ok(circle, "the mark's circle");
    const [cx, cy, r] = [Number(circle.attrs.cx), Number(circle.attrs.cy), Number(circle.attrs.r)];
    // Each text of the object as a box: from its baseline up one font size, as wide as its letters.
    const texts = findAll(
      level,
      (child) => child.tag === "text" && typeof child.text === "string" && child.text !== "!",
    );
    assert.ok(
      texts.some((node) => node.text === "Feed pump P101"),
      "the label is drawn",
    );
    for (const node of texts) {
      const size = Number(node.attrs["font-size"] ?? 14);
      const width = 0.6 * size * String(node.text).length;
      const anchor = node.attrs["text-anchor"] ?? "start";
      const left = Number(node.attrs.x) - (anchor === "middle" ? width / 2 : anchor === "end" ? width : 0);
      const [top, bottom] = [Number(node.attrs.y) - size, Number(node.attrs.y)];
      const apart = cx + r <= left || cx - r >= left + width || cy + r <= top || cy - r >= bottom;
      assert.ok(
        apart,
        `the mark at (${cx}, ${cy}) r ${r} covers "${node.text}" at ${left},${top}–${left + width},${bottom}`,
      );
    }
  });

  test("a stale value keeps its last value with a stale marking and its age", () => {
    const node = objectNode(
      draw(page(bound("level", "value", "value", "Main.Level", { decimals: 1 })), {
        "Main.Level": { value: 62.5, state: "confirmed", staleMs: 12_000 },
      }),
      "level",
    );
    assert.match(joinedText(node), /62\.5/);
    assert.match(joinedText(node), /12 s/);
    assert.strictEqual(node.attrs["data-stale"], "true");
  });

  test("an estimated value is marked est.", () => {
    const node = objectNode(
      draw(page(bound("level", "value", "value", "Main.Level", { decimals: 1 })), {
        "Main.Level": { value: 62.5, state: "estimated" },
      }),
      "level",
    );
    assert.match(joinedText(node), /62\.5/);
    assert.ok(textsOf(node).includes("est."), joinedText(node));
    assert.strictEqual(node.attrs["data-state"], "estimated");
  });

  test("a commanded value shows its target as pending", () => {
    const node = objectNode(
      draw(page(bound("speed", "value", "value", "Main.Speed", { decimals: 0 })), {
        "Main.Speed": { value: 40, state: "commanded", target: 50 },
      }),
      "speed",
    );
    assert.match(joinedText(node), /40/);
    assert.match(joinedText(node), /→ 50/);
    assert.strictEqual(node.attrs["data-state"], "commanded");
  });

  test("a status indicator lights in the accent when on, stays dark when off, and turns alarm red", () => {
    const on = objectNode(
      draw(page(bound("run", "status", "state", "Main.Run")), { "Main.Run": { value: true, state: "confirmed" } }),
      "run",
    );
    const off = objectNode(
      draw(page(bound("run", "status", "state", "Main.Run")), { "Main.Run": { value: false, state: "confirmed" } }),
      "run",
    );
    const alarm = objectNode(
      draw(page(bound("trip", "status", "state", "Main.Trip", { variant: "alarm" })), {
        "Main.Trip": { value: true, state: "confirmed" },
      }),
      "trip",
    );
    assert.strictEqual(on.attrs["data-on"], "true");
    assert.strictEqual(off.attrs["data-on"], "false");
    const lamp = (node: SvgNode): SvgNode => findAll(node, (child) => child.attrs["data-part"] === "lamp")[0];
    assert.strictEqual(lamp(on).attrs.fill, "var(--hmi-accent)");
    assert.strictEqual(lamp(on).attrs.filter, "url(#hmi-glow)");
    // Off is a hollow ring and on a filled disc: the shape carries the state as well as the
    // colour (§3.5).
    assert.strictEqual(lamp(off).attrs.fill, "none");
    assert.ok(Number(lamp(off).attrs["stroke-width"]) >= 2, "a ring");
    assert.strictEqual(lamp(off).attrs.filter, undefined);
    assert.strictEqual(lamp(alarm).attrs.fill, "var(--hmi-alarm)");
    assert.strictEqual(lamp(alarm).attrs.filter, "url(#hmi-glow)");
    assert.ok(textsOf(alarm).includes("!"), "a lit alarm lamp carries a mark, not only red");
    assert.ok(!textsOf(on).includes("!"), "an ordinary lamp has none");
  });

  test("a stale status shows its age as a chip inside the pill, centred in its height", () => {
    const node = objectNode(
      draw(page(bound("run", "status", "state", "Main.Run", { size: [240, 44], label: "Pump running" })), {
        "Main.Run": { value: true, state: "confirmed", staleMs: 5000 },
      }),
      "run",
    );
    const chip = findAll(node, (child) => child.attrs["data-part"] === "chip")[0];
    assert.ok(chip, renderToString(node));
    assert.ok(textsOf(chip).includes("5 s"), renderToString(chip));
    const box = findAll(chip, (child) => child.tag === "rect")[0];
    const [x, y, width, height] = ["x", "y", "width", "height"].map((name) => Number(box.attrs[name]));
    assert.ok(Math.abs(y + height / 2 - 22) <= 1, `centred: ${y} + ${height}/2`);
    assert.ok(x + width <= 240 - 8 && y >= 4 && y + height <= 40, `inside the pill: ${x}, ${y}, ${width}, ${height}`);
  });

  test("a stale status pill takes the stale outline, as a stale card does", () => {
    const pill = (values: Record<string, LiveValue>): SvgNode =>
      findAll(
        objectNode(draw(page(bound("run", "status", "state", "Main.Run", { size: [240, 44] })), values), "run"),
        (child) => child.attrs["data-part"] === "pill",
      )[0];
    assert.strictEqual(
      pill({ "Main.Run": { value: true, state: "confirmed", staleMs: 5000 } }).attrs.stroke,
      "var(--hmi-stale)",
    );
    assert.strictEqual(pill({ "Main.Run": { value: true, state: "confirmed" } }).attrs.stroke, "var(--hmi-border)");
  });

  test("a short value card puts its label and number on one line, apart", () => {
    const node = objectNode(
      draw(page(bound("speed", "value", "value", "P.S", { size: [160, 48], label: "Speed" })), {
        "P.S": { value: 60, state: "confirmed" },
      }),
      "speed",
    );
    const label = findAll(node, (child) => child.text === "Speed")[0];
    const number = findAll(node, (child) => child.tag === "text" && textsOf(child).includes("60"))[0];
    assert.ok(label && number, renderToString(node));
    assert.strictEqual(label.attrs.y, number.attrs.y, "one line");
    assert.strictEqual(number.attrs["text-anchor"], "end");
    assert.ok(Number(number.attrs.x) > 120, `the number at the far side: ${number.attrs.x}`);
  });

  test("a short value card cuts a long label so it stays clear of the number", () => {
    const node = objectNode(
      draw(page(bound("speed", "value", "value", "P.S", { size: [160, 48], label: "Feed pump P101", unit: "rpm" })), {
        "P.S": { value: 60, state: "confirmed" },
      }),
      "speed",
    );
    const texts = findAll(node, (child) => child.tag === "text");
    const label = texts.find((child) => (child.text ?? "").startsWith("Feed"));
    const number = texts.find((child) => textsOf(child).includes("60.0") || textsOf(child).includes("60"));
    assert.ok(label && number, renderToString(node));
    assert.ok((label.text ?? "").endsWith("…"), `cut: ${label.text}`);
    const labelRight = Number(label.attrs.x) + textWidth(label.text ?? "", Number(label.attrs["font-size"]));
    const spans = number.children.filter((child) => child.tag === "tspan");
    const numberWidth = spans.reduce(
      (sum, span) =>
        sum +
        Number(span.attrs.dx ?? 0) +
        textWidth(span.text ?? "", Number(span.attrs["font-size"] ?? number.attrs["font-size"])),
      0,
    );
    assert.ok(labelRight > 16 + 20, `some of the label stays: ${label.text}`);
    assert.ok(
      labelRight <= Number(number.attrs.x) - numberWidth - 4,
      `the label ends at ${labelRight}, the number starts at ${Number(number.attrs.x) - numberWidth}`,
    );
  });

  test("a value card is rounded on the surface tone with a faint border", () => {
    const node = objectNode(
      draw(page(bound("level", "value", "value", "Main.Level")), { "Main.Level": { value: 1, state: "confirmed" } }),
      "level",
    );
    const card = findAll(node, (child) => child.attrs["data-part"] === "card")[0];
    assert.ok(card, renderToString(node));
    assert.ok(Number(card.attrs.rx) >= 12);
    assert.strictEqual(card.attrs.fill, "var(--hmi-surface)");
    assert.strictEqual(card.attrs.stroke, "var(--hmi-border)");
  });

  test("an object of an unknown type draws a labelled placeholder", () => {
    const node = objectNode(draw(page(object("dial-1", "dial"))), "dial-1");
    assert.strictEqual(node.attrs["data-placeholder"], "true");
    assert.ok(
      textsOf(node).some((value) => value.includes("dial")),
      joinedText(node),
    );
  });

  test("the drawing colours only through design tokens", () => {
    const svg = draw(
      page(
        bound("level", "value", "value", "Main.Level"),
        bound("run", "status", "state", "Main.Run"),
        object("dial-1", "dial"),
      ),
      { "Main.Level": { value: 1, state: "unknown" }, "Main.Run": { value: true, state: "confirmed" } },
    );
    for (const node of findAll(svg, () => true)) {
      for (const name of ["fill", "stroke"]) {
        const value = node.attrs[name];
        if (value === undefined) continue;
        assert.ok(
          value === "none" || /^var\(--hmi-[a-z-]+\)$/.test(value) || /^url\(#hmi-[a-z-]+\)$/.test(value),
          `${node.tag} ${name}="${value}"`,
        );
      }
    }
  });
});

suite("HMI drawing: tokens", () => {
  test("every token the drawing uses is defined in the light and the dark theme", () => {
    const values: Record<string, LiveValue> = {
      "A.Level": { value: 50, state: "confirmed", staleMs: 5_000 },
      "A.Run": { value: true, state: "confirmed" },
      "A.Trip": { value: true, state: "confirmed" },
      "A.Speed": { value: 130, state: "commanded", target: 90 },
      "A.Gone": { value: null, state: "unknown" },
      "A.Est": { value: 3, state: "estimated" },
    };
    const svg = draw(
      page(
        bound("v1", "value", "value", "A.Level"),
        bound("v2", "value", "value", "A.Est"),
        bound("v3", "value", "value", "A.Gone"),
        bound("s1", "status", "state", "A.Run"),
        bound("s2", "status", "state", "A.Trip", { variant: "alarm" }),
        bound("b1", "bar", "value", "A.Speed"),
        bound("g1", "gauge", "value", "A.Level"),
        object("p1", "dial"),
      ),
      values,
    );
    const used = new Set([...renderToString(svg).matchAll(/var\(--hmi-([a-z-]+)\)/g)].map((match) => match[1]));
    assert.ok(used.size > 5, [...used].join(", "));
    for (const theme of ["light", "dark"] as const) {
      for (const name of [...used, "page", "font"]) {
        assert.ok(TOKEN_THEMES[theme][name], `--hmi-${name} is not defined in the ${theme} theme`);
      }
    }
    assert.match(tokensCss(), /\.hmi-theme-dark\s*\{[^}]*--hmi-alarm:/);
  });
});

suite("HMI drawing: project colours", () => {
  const themeBlock = (css: string, theme: string): string => {
    const start = css.indexOf(`.hmi-theme-${theme}`);
    return css.slice(start, css.indexOf("}", start));
  };

  test("a project's colours replace the defaults in their theme only", () => {
    const css = tokensCss({ dark: { accent: "#ff8800" } });
    assert.match(themeBlock(css, "dark"), /--hmi-accent: #ff8800;/);
    assert.ok(themeBlock(css, "dark").includes(`--hmi-page: ${TOKEN_THEMES.dark.page};`));
    assert.ok(themeBlock(css, "light").includes(`--hmi-accent: ${TOKEN_THEMES.light.accent};`));
  });

  test("only known tokens and hex colours reach the CSS", () => {
    const css = tokensCss({ dark: { accent: "red; background: url(x)", bogus: "#123456", page: "#000" } });
    assert.doesNotMatch(css, /url\(x\)/);
    assert.doesNotMatch(css, /bogus/);
    assert.match(themeBlock(css, "dark"), /--hmi-page: #000;/);
    assert.ok(themeBlock(css, "dark").includes(`--hmi-accent: ${TOKEN_THEMES.dark.accent};`));
  });

  test("alarm priorities have their own colours, none of them the accent's", () => {
    for (const theme of ["dark", "light"] as const) {
      const tokens = TOKEN_THEMES[theme];
      const colours = ["alarm-high", "alarm-medium", "alarm-low"].map((name) => tokens[name]);
      colours.forEach((colour, index) => assert.match(colour ?? "", /^#[0-9a-f]{6}$/i, `${theme} ${index}`));
      assert.strictEqual(new Set(colours).size, 3, `${theme}: three different colours`);
      assert.ok(!colours.includes(tokens.accent), `${theme}: not the accent`);
    }
    assert.match(themeBlock(tokensCss({ dark: { "alarm-low": "#00aaff" } }), "dark"), /--hmi-alarm-low: #00aaff;/);
  });

  test("the colour tokens are the ones the runtime accepts", () => {
    const root = path.resolve(__dirname, "../../../../..");
    const source = fs.readFileSync(path.join(root, "crates/trust-runtime/src/hmi/page_files/colors.rs"), "utf8");
    const list = source.slice(source.indexOf("COLOR_TOKENS"), source.indexOf("];", source.indexOf("COLOR_TOKENS")));
    const runtime = [...list.matchAll(/"([a-z0-9-]+)"/g)].map((match) => match[1]).sort();
    const drawing = Object.keys(TOKEN_THEMES.dark)
      .filter((name) => name !== "font")
      .sort();
    assert.deepStrictEqual(runtime, drawing);
    assert.deepStrictEqual(Object.keys(TOKEN_THEMES.light).sort(), Object.keys(TOKEN_THEMES.dark).sort());
  });
});
