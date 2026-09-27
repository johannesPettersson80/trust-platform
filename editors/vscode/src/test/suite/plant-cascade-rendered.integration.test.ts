import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import * as assert from "assert";
import { beginPlantDraft, openRenderedPlantSession, pollValue } from "./plant-rendered-harness";
suite("Rendered cascaded gravity tank editor", () => {
  test("offers coupled tank dynamics with two level states and feedback ports", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(60000);
    const root = vscode.workspace.workspaceFolders![0].uri.fsPath;
    fs.mkdirSync(path.join(root, "src"), { recursive: true });
    fs.writeFileSync(path.join(root, "src/Main.st"), `PROGRAM Main
VAR Flow : LREAL; H1 : LREAL; H2 : LREAL; END_VAR
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
P1.Flow AT %QL0 : LREAL;
P1.H1 AT %IL0 : LREAL;
P1.H2 AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION`);
    fs.writeFileSync(path.join(root, "runtime.toml"), "[bundle]\nversion=1\n[resource]\nname=\"Res\"\ncycle_interval_ms=100\n");
    const session = await openRenderedPlantSession();
    try {
      await beginPlantDraft(session);
      assert.strictEqual(await session.inner("return String(Boolean(d.querySelector('select[name=plant-model] option[value=gravity_tank_cascade]')));"), "true");
      await session.setValue("select[name=plant-model]", "gravity_tank_cascade");
      const fields = await pollValue("two cascade level fields", () => session.inner("return String(d.querySelectorAll('[data-plant-starting-state]').length);"), v => v === "2", 10000);
      assert.strictEqual(fields, "2");
      assert.strictEqual(await session.inner("return String(Boolean(d.querySelector('[name=plant-feedback-target-1]')));"), "true");
      assert.match(await session.inner("return d.querySelector('[data-plant-model-diagram-summary]').textContent;"), /upstream.*downstream/);
    } finally { session.close(); }
  });
});
