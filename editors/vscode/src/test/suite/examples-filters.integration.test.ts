import * as assert from "assert";
import * as vscode from "vscode";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";
import { captureRenderedWorkbench, pollValue, sleep } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered example filters", function () {
    this.timeout(90_000);
    test("CPEX-005 compact filters combine with search and reset empty results", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      await extension.activate();
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const theme = process.env.TRUST_PLANT_RENDERED_THEME;
      if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      const gallery = await openRenderedExamplesGallery();
      const inner = gallery.inner;
      try {
        await pollValue("browse all", () => inner("var b=d.querySelector('button[data-action-type=browseExamples]');if(!b)return '';b.click();return 'clicked';"), Boolean, 10_000);
        await pollValue("browse visible", () => inner("return String(!d.querySelector('#toolbar').hidden);"), (v) => v === "true", 10_000);
        const filters = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('#chips select')].map(x=>({kind:x.dataset.filterKind,label:x.labels?.[0]?.textContent})));")) as Array<{ kind: string; label: string }>;
        assert.deepStrictEqual(filters.map((f) => f.kind), ["hardware", "category"], "hardware and category must be compact labelled selects");
        assert.ok(filters.every((f) => f.label?.trim()));
        const select = async (kind: string, value: string) => inner(`var s=d.querySelector('select[data-filter-kind=${kind}]');s.value=${JSON.stringify(value)};s.dispatchEvent(new s.ownerDocument.defaultView.Event('change',{bubbles:true}));return s.value;`);
        await select("hardware", "no-hardware");
        await select("category", "control");
        await inner("var s=d.querySelector('#search');s.value='Routh';s.dispatchEvent(new s.ownerDocument.defaultView.Event('input',{bubbles:true}));return s.value;");
        const result = JSON.parse(await inner("return JSON.stringify({cards:[...d.querySelectorAll('#grid [data-example-id]')].map(x=>x.dataset.exampleId),count:d.querySelector('[role=status]')?.textContent,hardware:d.querySelector('select[data-filter-kind=hardware]').value,category:d.querySelector('select[data-filter-kind=category]').value});")) as { cards: string[]; count: string; hardware: string; category: string };
        assert.deepStrictEqual(result.cards, ["control-routh-hurwitz"]);
        assert.ok(result.count.includes(String(result.cards.length)), "visible count must match results");
        assert.strictEqual(result.hardware, "no-hardware");
        assert.strictEqual(result.category, "control");
        await inner("var s=d.querySelector('#search');s.value='__no_matching_example__';s.dispatchEvent(new s.ownerDocument.defaultView.Event('input',{bubbles:true}));return s.value;");
        const reset = await inner("var b=[...d.querySelectorAll('button')].find(x=>x.textContent==='Clear search and filters');if(!b)return '';b.click();return 'reset';");
        assert.strictEqual(reset, "reset");
        assert.strictEqual(await inner("return [d.querySelector('#search').value,d.querySelector('select[data-filter-kind=hardware]').value,d.querySelector('select[data-filter-kind=category]').value].join('|');"), "|all|all");
        assert.strictEqual(await inner("return String(d.documentElement.scrollWidth<=d.documentElement.clientWidth+1);"), "true");
        if (process.env.TRUST_EXAMPLES_RENDERED_CAPTURE) {
          await vscode.commands.executeCommand("notifications.clearAll");
          await sleep(300);
          captureRenderedWorkbench(process.env.TRUST_EXAMPLES_RENDERED_CAPTURE);
        }
      } finally { gallery.close(); }
    });
  });
}
