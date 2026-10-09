import * as assert from "assert";
import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";

import { tokensCss } from "../../hmiBuilder/drawing/tokens";

// Specification 39, §14.5 item 2: the HMI ships a Chinese, a Japanese and a Korean font beside
// Inter, so a panel without such fonts still shows those languages, each in its own forms.

const ROOT = path.resolve(__dirname, "../../../../..");
const FONTS = path.join(ROOT, "crates/trust-runtime/src/web/ui/assets/fonts");
const EXTENSION_FONTS = path.join(ROOT, "editors/vscode/media/fonts");

interface CjkFont {
  language: string;
  family: string;
  file: string;
  sha256: string;
  characters: string;
}

function manifest(): CjkFont[] {
  const file = path.join(FONTS, "cjk-fonts.json");
  assert.ok(fs.existsSync(file), "the CJK fonts are shipped with their manifest");
  return (JSON.parse(fs.readFileSync(file, "utf8")) as { fonts: CjkFont[] }).fonts;
}

const sha256 = (file: string): string => crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");

/** Every character of truST's own words in a language. */
function builtinCharacters(language: string): Set<string> {
  const table = fs.readFileSync(
    path.join(ROOT, "crates/trust-runtime/src/hmi/texts/builtin", `${language}.toml`),
    "utf8",
  );
  const values = [...table.matchAll(/^"(?:[^"\\]|\\.)*" = ("(?:[^"\\]|\\.)*")$/gm)].map(
    (match) => JSON.parse(match[1]) as string,
  );
  return new Set(values.join("").replace(/\s/g, ""));
}

suite("HMI drawing: Chinese, Japanese and Korean fonts", () => {
  test("a font per language ships with its licence and covers truST's words and the words of a plant", () => {
    const fonts = manifest();
    assert.deepStrictEqual(
      fonts.map((font) => [font.language, font.family, font.file]),
      [
        ["zh-Hans", "truST CJK SC", "NotoSansSC-Subset.woff2"],
        ["ja", "truST CJK JP", "NotoSansJP-Subset.woff2"],
        ["ko", "truST CJK KR", "NotoSansKR-Subset.woff2"],
      ],
    );
    assert.ok(fs.existsSync(path.join(FONTS, "LICENSE-NOTO-CJK.txt")), "the SIL Open Font License ships with them");
    const plant: Record<string, string> = {
      "zh-Hans": "泵阀罐液位压力温度流量电机启动停止",
      ja: "ポンプ弁タンク液位圧力温度流量運転停止",
      ko: "펌프밸브탱크수위압력온도유량운전정지",
    };
    for (const font of fonts) {
      assert.strictEqual(
        sha256(path.join(FONTS, font.file)),
        font.sha256,
        `${font.file} is the file its manifest describes`,
      );
      const covered = new Set(font.characters);
      const lacking = [...builtinCharacters(font.language), ...plant[font.language]].filter(
        (character) => !covered.has(character),
      );
      assert.deepStrictEqual(lacking, [], `${font.file} lacks characters of ${font.language}`);
    }
  });

  test("the drawing declares them beside Inter, loaded only for the text they cover, each language's forms first", () => {
    const css = tokensCss({}, "/hmi/assets/fonts/InterVariable.woff2");
    for (const [family, file] of [
      ["truST CJK SC", "NotoSansSC-Subset.woff2"],
      ["truST CJK JP", "NotoSansJP-Subset.woff2"],
      ["truST CJK KR", "NotoSansKR-Subset.woff2"],
    ]) {
      const face = css.match(new RegExp(`@font-face \\{[^}]*font-family: "${family}";[^}]*\\}`))?.[0] ?? "";
      assert.ok(face.includes(`src: url("/hmi/assets/fonts/${file}") format("woff2")`), `${family}: ${face}`);
      assert.match(face, /unicode-range: [^;]*U\+4E00-9FFF/, `${family} loads only for CJK text`);
      assert.match(face, /font-weight: 100 900/);
    }
    const order = (font: string): string[] => [...font.matchAll(/"truST CJK (SC|JP|KR)"/g)].map((match) => match[1]);
    const defaultFont = css.match(/\.hmi-theme-dark \{[^}]*--hmi-font: ([^;]*);/)?.[1] ?? "";
    assert.ok(defaultFont.startsWith("Inter, "), defaultFont);
    assert.deepStrictEqual(order(defaultFont), ["SC", "JP", "KR"]);
    const japanese = css.match(/:lang\(ja\)[^{]*\{\s*--hmi-font: ([^;]*);/)?.[1] ?? "";
    assert.deepStrictEqual(order(japanese), ["JP", "SC", "KR"], css);
    const korean = css.match(/:lang\(ko\)[^{]*\{\s*--hmi-font: ([^;]*);/)?.[1] ?? "";
    assert.deepStrictEqual(order(korean), ["KR", "JP", "SC"], css);
  });

  test("the extension carries the same fonts for the editor's canvas", () => {
    for (const font of manifest()) {
      const copy = path.join(EXTENSION_FONTS, font.file);
      assert.ok(fs.existsSync(copy), `${font.file} is in the extension`);
      assert.strictEqual(sha256(copy), font.sha256, `${font.file} is the runtime's`);
    }
    assert.ok(fs.existsSync(path.join(EXTENSION_FONTS, "LICENSE-NOTO-CJK.txt")));
  });
});
