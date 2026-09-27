import * as assert from "assert";
import * as fs from "fs";
import * as vscode from "vscode";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

/** Visual follow-through for the same copied README whose host write is asserted by the caller. */
export async function captureCopiedLessonPreview(readme: string, output: string): Promise<void> {
  await captureLessonMarkdownPreview(readme, output, { readyText: "use truST > Start from example and select Lesson 2", copied: true });
}

export async function captureLessonMarkdownPreview(readme: string, output: string,
  review: { readyText: string; copied?: boolean; full?: boolean; heading?: string }): Promise<void> {
  const before = new Set(vscode.window.tabGroups.all.flatMap(group => group.tabs));
  const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
  const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
  const existingFrames = new Set((await httpJson(port, "/json")).map((target: any) => target.id));
  try {
    await vscode.commands.executeCommand("markdown.showPreview", vscode.Uri.file(readme));
    const target = JSON.parse(await pollValue("native Markdown preview iframe", async () => {
      const targets = await httpJson(port, "/json");
      return JSON.stringify(targets.find((item: any) => !existingFrames.has(item.id) && item.type === "iframe" && /index\.html/.test(item.url)) ?? null);
    }, value => value !== "null", 10000));
    const { sessionId } = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
    const inner = (body: string) => evaluateInWebview(connection, sessionId, body);
    const text = await pollValue("rendered lesson Markdown", () => inner("return d.body.innerText;"),
      value => value.includes(review.readyText), 10000);
    if (review.copied) {
      assert.ok(text.includes("ON/OFF feedback"), "preserve the author's descriptive next-lesson label");
      assert.strictEqual(await inner("return String(!!d.querySelector('a[href*=\"02-cold-storage-on-off/README.md\"]'));"), "false", "the copied preview must not retain a broken sibling-project link");
    }
    fs.writeFileSync(`${output}.txt`, text);
    await inner(review.full ? "d.scrollingElement.scrollTop=0;return 'lesson start';"
      : `[...d.querySelectorAll('h2')].find(h=>h.textContent.includes(${JSON.stringify(review.heading ?? "Reason for the next lesson")}))?.scrollIntoView({block:'start'});return 'requested lesson section';`);
    await sleep(350);
    await captureRenderedWorkbenchCdp(connection, `${output}.png`);
    if (review.full) {
      const scroll = JSON.parse(await inner("var s=d.scrollingElement;return JSON.stringify({height:s.scrollHeight,viewport:s.clientHeight});"));
      const pages: Array<{ image: string; top: number }> = [];
      const step = Math.max(1, Math.floor(scroll.viewport * .8));
      for (let top = step; top < scroll.height; top += step) {
        const actual = Number(await inner(`d.scrollingElement.scrollTop=${top};return d.scrollingElement.scrollTop;`));
        if (pages[pages.length - 1]?.top === actual) break;
        const image = `${output}-${pages.length + 1}.png`;
        await sleep(200); await captureRenderedWorkbenchCdp(connection, image);
        pages.push({ image, top: actual });
        assert.ok(pages.length < 30, "the complete lesson capture must remain bounded");
      }
      fs.writeFileSync(`${output}-pages.json`, JSON.stringify({ ...scroll, pages }, null, 2));
    }
  } finally {
    connection.close();
    const added = vscode.window.tabGroups.all.flatMap(group => group.tabs).filter(tab => !before.has(tab));
    if (added.length) await vscode.window.tabGroups.close(added, true);
  }
}
