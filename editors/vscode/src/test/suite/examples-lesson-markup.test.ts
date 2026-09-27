import * as assert from "assert";

import { renderLessonMarkup } from "../../examples/lessonMarkup";

suite("Lesson markup rendering", () => {
  test("preserves authored tables and lists", () => {
    const result = renderLessonMarkup("| Signal | Value |\n| --- | ---: |\n| PV | 35 |\n\n1. First\n2. Second\n\n- safe\n- repeat");
    assert.match(result.html, /<table[\s>]/);
    assert.match(result.html, /<ol[\s>]/);
    assert.match(result.html, /<ul[\s>]/);
    assert.strictEqual(result.files.length, 0);
  });

  test("renders inline code and emphasis without exposing raw delimiters", () => {
    const result = renderLessonMarkup("Use `Control.PID` with **PI** and *care*.");
    assert.match(result.html, /<code>Control\.PID<\/code>/);
    assert.match(result.html, /<strong>PI<\/strong>/);
    assert.match(result.html, /<em>care<\/em>/);
    assert.ok(!result.html.includes("**PI**"));
  });

  test("renders valid inline and block math as accessible KaTeX", () => {
    const result = renderLessonMarkup("Inline $K_p>0$\n\n$$\nx^2\n$$");
    assert.match(result.html, /<math[\s>]/);
    assert.strictEqual((result.html.match(/<math[\s>]/g) ?? []).length, 2);
    assert.ok(!result.html.includes("$K_p>0$"));
  });

  test("turns only safe project-relative files into lesson-file buttons", () => {
    const result = renderLessonMarkup(
      "[Main.st](src/Main.st) [data](figures/trace.csv) [escape](../outside.st) [encoded](%2e%2e/outside.st) [query](src/Main.st?raw=1) [fragment](src/Main.st#L1) [run](run.sh) [url](https://example.com)"
    );
    assert.match(result.html, /class="lesson-file-link"[^>]*data-lesson-file="src\/Main\.st"/);
    assert.match(result.html, /data-lesson-file="figures\/trace\.csv"/);
    assert.deepStrictEqual(result.files, ["src/Main.st", "figures/trace.csv"]);
    assert.ok(!result.html.includes('data-lesson-file="../outside.st"'));
    assert.ok(!result.html.includes('data-lesson-file="%2e%2e/outside.st"'));
    assert.ok(!result.html.includes('data-lesson-file="src/Main.st?raw=1"'));
    assert.ok(!result.html.includes('data-lesson-file="src/Main.st#L1"'));
    assert.ok(!result.html.includes('data-lesson-file="run.sh"'));
    assert.ok(!/<a\b[^>]*\bhref=/i.test(result.html));
    assert.ok(!/<(?:script|img)\b/i.test(result.html));
  });

  test("escapes authored text and disables raw HTML and unsafe URLs", () => {
    const result = renderLessonMarkup('<span>unsafe</span> ![diagram](diagram.png) [bad](javascript:alert(1))');
    assert.ok(!result.html.includes("<span>unsafe</span>"));
    assert.match(result.html, /&lt;span&gt;unsafe&lt;\/span&gt;/);
    assert.ok(result.html.includes("diagram"));
    assert.ok(!/<(?:a\b[^>]*\bhref=|script|img)\b/i.test(result.html));
  });

  test("preserves malformed math as authored text", () => {
    const result = renderLessonMarkup("Keep malformed $\\frac{x$ visible.");
    assert.match(result.html, /\$\\frac\{x\$/);
  });
});
