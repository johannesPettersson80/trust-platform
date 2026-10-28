import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
  connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment,
} from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered unavailable examples", function () {
    this.timeout(90_000);

    test("CPEX-013 keeps valid starters usable and disables damaged lesson creation", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      const examplesRoot = path.join(extension.extensionPath, "media", "examples");
      const manifestPath = path.join(examplesRoot, "manifest.json");
      const originalManifest = await fs.promises.readFile(manifestPath);
      const damagedPath = path.join(examplesRoot, "control", "__render-damaged-entry__");
      const fixtureManifest = [
        { id: "empty-simulator", title: "Empty simulator", description: "A valid runnable starter", path: "empty-simulator", hardware: "none" },
        { id: "damaged-control", title: "Damaged control lesson", description: "Unavailable lesson", path: "control/__render-damaged-entry__", hardware: "none", tags: ["control"] },
      ];
      assert.strictEqual(fs.existsSync(damagedPath), false, "fixture must not replace an existing example");
      try {
        await fs.promises.writeFile(manifestPath, JSON.stringify(fixtureManifest));
        await extension.activate();
        await vscode.commands.executeCommand("trust.examples.start");
        const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
        const version = await httpJson(port, "/json/version");
        const connection = await connectCdp(version.webSocketDebuggerUrl);
        try {
          const sessionId = await pollValue("examples webview", async () => {
            const targets = await httpJson(port, "/json") as Array<{ id: string; url: string }>;
            for (const target of targets.filter((item) => /index\.html/.test(item.url))) {
              const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
              await connection.send("Runtime.enable", {}, attached.sessionId);
              const marker = await evaluateInWebview(connection, attached.sessionId, "return String(Boolean(d.querySelector('#grid') && d.querySelector('#title')));");
              if (marker === "true") return attached.sessionId as string;
              await connection.send("Target.detachFromTarget", { sessionId: attached.sessionId });
            }
            return "";
          }, Boolean, 20_000);
          const inner = (body: string) => evaluateInWebview(connection, sessionId, body);
          await pollValue("browse action", () => inner("var b=[...d.querySelectorAll('button')].find(x=>x.dataset.actionType==='browseExamples');if(!b)return 'WAIT';b.click();return 'CLICKED';"), (value) => value === "CLICKED", 10_000);
          await pollValue("damaged card", () => inner("return String(Boolean([...d.querySelectorAll('[data-example-id=damaged-control]')].length));"), (value) => value === "true", 10_000);
          const rendered = JSON.parse(await inner("var c=d.querySelector('[data-example-id=damaged-control]');return JSON.stringify({text:c?.textContent||'',create:!![...c?.querySelectorAll('button')||[]].find(x=>x.textContent==='Create project'),valid:!!d.querySelector('[data-example-id=empty-simulator] button')});")) as { text: string; create: boolean; valid: boolean };
          assert.match(rendered.text, /Not yet available|metadata is unavailable/i);
          assert.strictEqual(rendered.create, false);
          assert.strictEqual(rendered.valid, true);
        } finally {
          connection.close();
        }
      } finally {
        await fs.promises.writeFile(manifestPath, originalManifest);
      }
    });
  });
}
