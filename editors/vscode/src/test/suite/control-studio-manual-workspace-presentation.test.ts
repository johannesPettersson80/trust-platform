import { initializeManualWorkspacePresentation } from "../../controlStudio/webview/manualWorkspacePresentation";
import { RUN_VALIDATION_CONTROLS } from "../../controlStudio/runValidationPresentation";
import * as assert from "assert";
const { JSDOM } = require("jsdom");

suite("Control Studio manual workspace presentation", () => {
  test("keeps saved-result controls before an optional imported-run disclosure", () => {
    const dom = new JSDOM(`<main data-control-studio><section data-workflow-pane="Validate">
      <h2>Validate</h2>${RUN_VALIDATION_CONTROLS}
      <div class="workflow-grid"><button data-result-save>Save result</button></div>
      </section></main>`);
    const document = dom.window.document;
    const originalFile = document.querySelector("#run-validation-current");
    let changed = false;
    originalFile.addEventListener("change", () => { changed = true; });
    initializeManualWorkspacePresentation(document);
    initializeManualWorkspacePresentation(document);
    const details = document.querySelector("details[data-manual-run-validation]");
    assert.ok(details, "importing matched runs is optional when inspecting a saved observation");
    assert.strictEqual(details.open, false);
    assert.strictEqual(details.querySelector("summary")?.textContent, "Evaluate imported runs");
    assert.ok(details.previousElementSibling.querySelector("[data-result-save]"));
    assert.strictEqual(details.querySelector("#run-validation-current"), originalFile);
    details.querySelector("summary").click();
    assert.strictEqual(details.open, true);
    originalFile.dispatchEvent(new dom.window.Event("change"));
    assert.strictEqual(changed, true, "disclosure preserves the existing file handler");
    assert.strictEqual(document.querySelectorAll("[data-manual-run-validation]").length, 1);
  });
  test("moves repeated pane guidance into closed Help disclosures without hiding actions", () => {
    const dom = new JSDOM(`
      <main data-control-studio>
        ${["Monitor", "Test", "Identify", "Tune", "Validate"].map(stage => `
          <section class="workflow-pane" data-workflow-pane="${stage}">
            <h2>${stage}</h2><p>${stage} guidance text.</p>
            <div class="workflow-grid"><button data-${stage.toLowerCase()}-action>${stage} action</button></div>
          </section>`).join("")}
        <section class="auto-plan" data-auto-tune-plan>
          <div class="auto-header"><div><h2>Automatic Tune</h2><p>Automatic Tune guidance text.</p></div></div>
          <button data-auto-tune-approve>Confirm and run</button>
        </section>
      </main>`);
    const api = presentationApi();
    api.initializeManualWorkspacePresentation(dom.window.document);

    for (const stage of ["Monitor", "Test", "Identify", "Tune", "Validate"]) {
      const pane = dom.window.document.querySelector(`[data-workflow-pane="${stage}"]`)!;
      assert.strictEqual(pane.querySelector(":scope > h2"), null, `${stage} heading should not duplicate stage navigation`);
      assert.strictEqual(pane.querySelector(":scope > p"), null, `${stage} guidance should be behind Help`);
      const help = pane.querySelector("details.manual-workspace-help")!;
      assert.strictEqual(help.open, false);
      assert.strictEqual(help.querySelector("summary")?.textContent, "Help");
      assert.strictEqual(help.querySelector("p")?.textContent, `${stage} guidance text.`);
      assert.ok(pane.querySelector(`[data-${stage.toLowerCase()}-action]`), `${stage} action must remain reachable`);
    }
    const auto = dom.window.document.querySelector("[data-auto-tune-plan]")!;
    assert.strictEqual(auto.querySelector(":scope > .auto-header h2")?.textContent, "Automatic Tune");
    assert.strictEqual(auto.querySelector(".auto-header > div > p"), null);
    assert.strictEqual(auto.querySelector("details.manual-workspace-help summary")?.textContent, "Help");
    assert.ok(auto.querySelector("[data-auto-tune-approve]"));
  });

  test("is idempotent and preserves safety/error content", () => {
    const dom = new JSDOM(`<main data-control-studio>
      <section class="workflow-pane" data-workflow-pane="Test"><h2>Test</h2><p>Safety guidance.</p>
        <label data-physical-topology><input type="checkbox">Topology approved</label>
        <button data-test-approve>Confirm and start</button><button data-test-abort>Abort test</button>
        <p class="error">Runtime error</p></section>
    </main>`);
    const api = presentationApi();
    api.initializeManualWorkspacePresentation(dom.window.document);
    api.initializeManualWorkspacePresentation(dom.window.document);
    assert.strictEqual(dom.window.document.querySelectorAll("details.manual-workspace-help").length, 1);
    assert.ok(dom.window.document.querySelector("[data-physical-topology]"));
    assert.ok(dom.window.document.querySelector("[data-test-approve]"));
    assert.ok(dom.window.document.querySelector("[data-test-abort]"));
    assert.strictEqual(dom.window.document.querySelector(".error")?.textContent, "Runtime error");
  });

  test("keeps supplied-model controls visible while collapsing generic help, metadata, and channel data", () => {
    const dom = new JSDOM(`<main data-control-studio>
      <section class="workflow-pane" data-workflow-pane="Tune">
        <h2>Tune</h2><p>Choose a tuning method.</p>
        <section aria-label="Supplied model frequency analysis">
          <h3>Analyze a supplied plant model</h3>
          <p>Inspect each input/output channel without connecting to a live controller.</p>
          <label>Import model JSON<input id="general-model-file" type="file"></label>
          <input id="general-model-min" type="number" value="0.01">
          <button data-general-model-analyze>Analyze model</button>
          <section data-general-model-result>
            <p data-general-model-provenance>Model analysis complete.</p>
            <p data-general-model-poles>Model poles.</p>
            <p data-general-model-rga>DC relative gain.</p>
            <details open><summary>Calculated channel response</summary>
              <table><tbody data-general-model-table><tr><td>1</td></tr></tbody></table>
            </details>
          </section>
        </section>
      </section>
    </main>`);
    initializeManualWorkspacePresentation(dom.window.document);
    const pane = dom.window.document.querySelector('[data-workflow-pane="Tune"]')!;
    const model = pane.querySelector('[aria-label="Supplied model frequency analysis"]')!;
    assert.strictEqual(model.querySelector(":scope > p")?.textContent, undefined,
      "generic supplied-model explanation should be behind Help");
    const help = model.querySelector("details.manual-workspace-help")!;
    assert.strictEqual(help.open, false);
    assert.match(help.textContent, /Inspect each input\/output channel/);
    const metadata = model.querySelector("details.manual-model-metadata")!;
    assert.strictEqual(metadata.open, false);
    assert.ok(metadata.querySelector("[data-general-model-provenance]"));
    assert.ok(metadata.querySelector("[data-general-model-poles]"));
    assert.ok(metadata.querySelector("[data-general-model-rga]"));
    const tableDetails = model.querySelector("details.manual-model-channel-response")!;
    assert.strictEqual(tableDetails.open, false);
    assert.ok(tableDetails.querySelector("[data-general-model-table]"));
    assert.ok(model.querySelector("#general-model-file"));
    assert.ok(model.querySelector("#general-model-min"));
    assert.ok(model.querySelector("[data-general-model-analyze]"));
  });

});

function presentationApi(): { initializeManualWorkspacePresentation(document?: Document): void } {
  return { initializeManualWorkspacePresentation };
}
