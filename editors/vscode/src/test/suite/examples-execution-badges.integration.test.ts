import * as assert from "assert";
import * as vscode from "vscode";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";
import { captureRenderedWorkbench, pollValue, sleep } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered starter execution badges", function () {
    this.timeout(90_000);

    test("CPEX-005 shows simulator execution independently from optional hardware", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      await extension.activate();
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const theme = process.env.TRUST_PLANT_RENDERED_THEME;
      if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      const gallery = await openRenderedExamplesGallery();
      try {
        const inner = gallery.inner;
        await pollValue(
          "browse all examples",
          () => inner("var b=d.querySelector('button[data-action-type=browseExamples]');if(!b)return 'WAIT';b.click();return 'CLICKED';"),
          (value) => value === "CLICKED",
          10_000
        );
        await pollValue(
          "all starter cards",
          () => inner("return String(['empty-simulator','conveyor','plcopen-motion-single-axis','twincat-ads','raspberry-pi'].every(id=>[...d.querySelectorAll('[data-example-id]')].some(x=>x.dataset.exampleId===id)));"),
          (value) => value === "true",
          10_000
        );

        const cards = JSON.parse(await inner(`return JSON.stringify(${JSON.stringify([
          "empty-simulator",
          "conveyor",
          "plcopen-motion-single-axis",
          "twincat-ads",
          "raspberry-pi",
        ])}.map(id=>{const c=d.querySelector('[data-example-id="'+id+'"]');return {id,text:c?.textContent||'',badges:[...c?.querySelectorAll('.badge')||[]].map(x=>x.textContent||'')}}));`)) as Array<{ id: string; text: string; badges: string[] }>;
        assert.strictEqual(cards.length, 5);
        assert.ok(cards.every((card) => card.text.includes("Runs in simulator")),
          "every non-control starter must visibly identify its runnable simulator default");

        const byId = new Map(cards.map((card) => [card.id, card]));
        for (const id of ["empty-simulator", "conveyor", "plcopen-motion-single-axis"]) {
          assert.ok(byId.get(id)?.text.includes("No hardware needed"), `${id} hardware badge`);
        }
        assert.ok(byId.get("twincat-ads")?.text.includes("Hardware optional: TwinCAT"));
        assert.ok(byId.get("raspberry-pi")?.text.includes("Hardware optional: Raspberry Pi"));
        assert.strictEqual(await inner("return String(d.documentElement.scrollWidth<=d.documentElement.clientWidth+1);"), "true",
          "starter badges must not introduce horizontal overflow");
        if (process.env.TRUST_EXAMPLES_RENDERED_CAPTURE) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await inner("if(d.documentElement.clientWidth<1200){var s=d.querySelector('#search');s.value='TwinCAT ADS';s.dispatchEvent(new s.ownerDocument.defaultView.Event('input',{bubbles:true}));}return 'focused';");
          await sleep(300);
          captureRenderedWorkbench(process.env.TRUST_EXAMPLES_RENDERED_CAPTURE);
        }
      } finally {
        gallery.close();
      }
    });
  });
}
