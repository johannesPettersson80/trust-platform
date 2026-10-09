import * as assert from "assert";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import * as vm from "vm";

import { tokensCss } from "../../hmiBuilder/drawing/tokens";

// Specification 39, §3.4 and §17.3: the runtime draws with the same module as the editor; §14.1:
// the HMI ships its own open font.

const ROOT = path.resolve(__dirname, "../../../../..");
const BUNDLE = path.join(ROOT, "crates/trust-runtime/src/web/ui/modules/hmi-drawing.js");
const DISPLAY_BUNDLE = path.join(ROOT, "crates/trust-runtime/src/web/ui/modules/hmi-display.js");
const FONTS = path.join(ROOT, "crates/trust-runtime/src/web/ui/assets/fonts");
/** SHA-256 of `web/InterVariable.woff2` in the official Inter 4.1 release. */
const INTER_4_1 = "693b77d4f32ee9b8bfc995589b5fad5e99adf2832738661f5402f9978429a8e3";

const lf = (text: string): string => text.replace(/\r\n/g, "\n");

interface DrawingBundle {
  drawPage(page: unknown, frame: unknown): unknown;
  renderToString(node: unknown): string;
  tokensCss(): string;
}

suite("HMI drawing: runtime bundle", () => {
  test("the runtime's copy of the drawing module is built from the current source", async () => {
    assert.ok(fs.existsSync(BUNDLE), "the runtime bundle exists; build it with `npm run build:hmi-drawing`");
    const { buildDrawingBundle } = require(path.join(ROOT, "editors/vscode/scripts/build-hmi-drawing.js"));
    const fresh: string = await buildDrawingBundle({ write: false });
    assert.ok(
      lf(fs.readFileSync(BUNDLE, "utf8")) === lf(fresh),
      "the runtime bundle is stale; run `npm run build:hmi-drawing`",
    );
  });

  test("the runtime's copy of the display module is built from the current source", async () => {
    assert.ok(fs.existsSync(DISPLAY_BUNDLE), "the display bundle exists; build it with `npm run build:hmi-drawing`");
    const { buildDisplayBundle } = require(path.join(ROOT, "editors/vscode/scripts/build-hmi-drawing.js"));
    const fresh: string = await buildDisplayBundle({ write: false });
    assert.ok(
      lf(fs.readFileSync(DISPLAY_BUNDLE, "utf8")) === lf(fresh),
      "the display bundle is stale; run `npm run build:hmi-drawing`",
    );
  });

  test("the bundle draws a page on its own, without a DOM", () => {
    assert.ok(fs.existsSync(BUNDLE), "the runtime bundle exists");
    const context: Record<string, unknown> = {};
    vm.runInNewContext(fs.readFileSync(BUNDLE, "utf8"), context);
    const drawing = context.TrustHmiDrawing as DrawingBundle | undefined;
    assert.ok(drawing, "the bundle defines TrustHmiDrawing");
    const page = {
      title: "Bundle",
      object: [
        {
          id: "speed",
          type: "value",
          at: [0, 0],
          size: [200, 80],
          rotation: 0,
          bind: { value: { path: "Pump.Speed", direction: "read" } },
        },
      ],
      pipe: [],
    };
    const svg = drawing.renderToString(
      drawing.drawPage(page, { size: [1280, 720], values: { "Pump.Speed": { value: 1180, state: "confirmed" } } }),
    );
    assert.ok(svg.includes('data-object="speed"') && svg.includes("1180"), svg);
    assert.ok(drawing.tokensCss().includes("--hmi-accent"));
  });

  test("the HMI font is the official Inter release and ships with its licence", () => {
    const font = path.join(FONTS, "InterVariable.woff2");
    assert.ok(fs.existsSync(font), "the font file exists");
    const bytes = fs.readFileSync(font);
    assert.strictEqual(bytes.subarray(0, 4).toString("latin1"), "wOF2");
    assert.strictEqual(crypto.createHash("sha256").update(bytes).digest("hex"), INTER_4_1);
    const licence = path.join(FONTS, "LICENSE-INTER.txt");
    assert.ok(fs.existsSync(licence), "the licence exists");
    assert.ok(fs.readFileSync(licence, "utf8").includes("SIL Open Font License, Version 1.1"));
  });

  test("the editor ships the same font file and licence, so pages look the same there (§3.4)", () => {
    const editorFonts = path.join(ROOT, "editors/vscode/media/fonts");
    for (const file of ["InterVariable.woff2", "LICENSE-INTER.txt"]) {
      const copy = path.join(editorFonts, file);
      assert.ok(fs.existsSync(copy), `the editor's ${file}; build it with \`npm run build:hmi-builder\``);
      assert.ok(
        fs.readFileSync(copy).equals(fs.readFileSync(path.join(FONTS, file))),
        `the editor's ${file} is the runtime's`,
      );
    }
  });

  test("the tokens CSS declares the bundled font when it is given the font's address", () => {
    const css = tokensCss({}, "/hmi/assets/fonts/InterVariable.woff2");
    assert.match(
      css,
      /@font-face\s*\{[^}]*font-family:\s*"Inter";[^}]*src:\s*url\("\/hmi\/assets\/fonts\/InterVariable\.woff2"\)\s*format\("woff2"\);[^}]*font-weight:\s*100 900;/,
    );
    assert.ok(!tokensCss().includes("@font-face"), "no font face without an address");
    assert.ok(
      !tokensCss({}, 'x"); } body { color: red').includes("@font-face"),
      "an address that could leave the rule is refused",
    );
  });
});
