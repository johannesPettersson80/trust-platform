import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

suite("Control Studio identification and tuning", () => {
  test("renders the approved Identify page shell and keeps model evidence separate from Tune", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /Identify.*Tune.*Validate/);
    assert.match(template, /data-studio-inputs/);
    assert.match(template, /data-studio-outputs/);
    assert.match(template, /data-engineer-acquisition-method/);
    assert.match(template, /data-engineer-find[^>]*>Identify plant</);
    assert.match(template, /data-identify-import[^>]*>Open recording…</);
    assert.match(template, /data-studio-limits[^>]*>Limits…</);
    assert.match(template, /data-studio-model-result/);
    assert.match(template, /data-control-studio-chart/);
    assert.doesNotMatch(template, /Check recording|Fit estimation run|Accept model for design/);
    assert.doesNotMatch(template, /data-identify-qualification|plot-toolbar|bottom status/);
    assert.match(workflow, /new ThreePageOperation/);
    assert.match(workflow, /new ThreePageBindings/);
    assert.match(workflow, /new ThreePageApplication/);
    const model = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageModel.ts"), "utf8");
    assert.match(model, /renderModelPresentation/);
    assert.match(model, /data-studio-model-type/);
    assert.match(model, /data-studio-model-equation/);
  });

  test("keeps automatic method choices and comparison evidence in the current Tune result owner", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    // The method options come from the engine's answer beside Automatic (specification 35, Tuning method), so the template holds Automatic alone.
    assert.match(template, /data-engineer-tuning-method><option value="automatic">Automatic<\/option><\/select>/);
    assert.match(workflow, /offerMethods\(methods, titles\)/);
    assert.match(template, /data-studio-tuning-result/);
    assert.match(workflow, /new ThreePageTuning/);
    assert.match(workflow, /tuningPreferences/);
    assert.doesNotMatch(template, /data-auto-tune-attempts|data-saved-tuning-attempts|Method comparison/,
      "retained comparison evidence belongs in the current Tune result owner, not the retired panel markup");
  });

  test("renders the approved Tune controls and keeps tuning evidence in the result area", () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const template = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "threePageTemplate.ts"), "utf8");
    const workflow = fs.readFileSync(path.join(extensionRoot, "src", "controlStudio", "webview", "threePageWorkflow.ts"), "utf8");
    assert.match(template, /data-studio-controller/);
    assert.match(template, /data-engineer-tuning-method/);
    assert.match(template, /data-engineer-tune[^>]*>Tune controller</);
    assert.match(template, /data-studio-requirements[^>]*>Requirements…</);
    assert.match(template, /data-studio-tuning-result/);
    assert.match(template, /data-control-studio-chart/);
    assert.doesNotMatch(template, /Design method|data-tune-method-evidence|Assumptions|Design inputs|Equations/);
    assert.match(workflow, /new ThreePageTuning/);
    assert.match(workflow, /tuningPreferences/);
    assert.match(workflow, /data-studio-requirements/);
  });
});
