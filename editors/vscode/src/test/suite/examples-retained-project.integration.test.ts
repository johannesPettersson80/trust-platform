import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";
import { pollValue } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered retained lesson projects", function () {
    this.timeout(90_000);

    test("CPEX-012 selected lesson detail offers Open existing project", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      await extension.activate();
      const destination = await fs.mkdtemp(path.join(os.tmpdir(), "trust-cpex012-rendered-"));
      const previousDestination = process.env.TRUST_UX_EXAMPLE_DESTINATION;
      const previousName = process.env.TRUST_UX_EXAMPLE_NAME;
      const previousOpen = process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
      process.env.TRUST_UX_EXAMPLE_DESTINATION = destination;
      process.env.TRUST_UX_EXAMPLE_NAME = "rendered-lesson-copy";
      process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = "0";
      const gallery = await openRenderedExamplesGallery();
      try {
        const inner = gallery.inner;
        const click = async (action: string, id?: string) => pollValue(
          action,
          () => inner(`var b=[...d.querySelectorAll('button')].find(x=>x.dataset.actionType===${JSON.stringify(action)}${id ? `&&x.dataset.actionId===${JSON.stringify(id)}` : ""});if(!b)return 'WAIT';b.click();return 'CLICKED';`),
          (value) => value === "CLICKED",
          10_000
        );
        await click("openControlCurriculum");
        await click("openControlLesson", "control-lesson-01");
        await pollValue("lesson create action", () => inner("return String(Boolean([...d.querySelectorAll('button')].find(x=>x.dataset.actionType==='useExample')));"), (value) => value === "true", 10_000);
        await click("useExample", "control-lesson-01");
        await pollValue("created lesson state", () => inner("return String((d.querySelector('.lesson-detail')?.textContent||'').includes('Project created'));"), (value) => value === "true", 10_000);
        assert.strictEqual(await inner("return String([...d.querySelectorAll('button')].some(x=>x.textContent==='Open existing project'));"), "true");
      } finally {
        gallery.close();
        if (previousDestination === undefined) delete process.env.TRUST_UX_EXAMPLE_DESTINATION; else process.env.TRUST_UX_EXAMPLE_DESTINATION = previousDestination;
        if (previousName === undefined) delete process.env.TRUST_UX_EXAMPLE_NAME; else process.env.TRUST_UX_EXAMPLE_NAME = previousName;
        if (previousOpen === undefined) delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER; else process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = previousOpen;
        await fs.rm(destination, { recursive: true, force: true });
      }
    });
  });
}
