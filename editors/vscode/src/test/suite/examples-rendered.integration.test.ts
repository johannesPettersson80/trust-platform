import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import MarkdownIt from "markdown-it";
import { controlModuleId } from "../../examples/model";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";
import { captureLessonMarkdownPreview } from "./examples-markdown-preview";
import { pollValue, sleep } from "./plant-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered examples gallery", function () {
    this.timeout(90_000);
    test("CPEX-007 full selected lesson detail", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      await vscode.commands.executeCommand("notifications.clearAll");
      await extension.activate();
      const theme = process.env.TRUST_PLANT_RENDERED_THEME;
      const lessonId = process.env.TRUST_EXAMPLES_RENDERED_LESSON ?? "control-lesson-01";
      if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      const zoom = Number(process.env.TRUST_EXAMPLES_RENDERED_ZOOM ?? "0");
      assert.ok(Number.isInteger(zoom) && Math.abs(zoom) <= 2);
      await vscode.workspace.getConfiguration("window").update("zoomLevel", zoom, vscode.ConfigurationTarget.Global);
      const gallery = await openRenderedExamplesGallery();
      try {
        const inner = gallery.inner;
        const click = async (type: string, id?: string) => {
          await pollValue(type, () => inner(`var button=[...d.querySelectorAll('button')].find(x=>x.dataset.actionType===${JSON.stringify(type)}${id ? `&&x.dataset.actionId===${JSON.stringify(id)}` : ""});if(!button)return 'WAIT';button.click();return 'CLICKED';`), (value) => value === "CLICKED", 10_000);
        };
        const catalog = JSON.parse(fs.readFileSync(path.join(extension.extensionPath, "media", "examples", "manifest.json"), "utf8")) as Array<{ id: string; path: string; description: string }>;
        const entry = catalog.find((item) => item.id === lessonId);
        assert.ok(entry);
        await click("openControlCurriculum");
        await click("openControlCourse", controlModuleId(entry.path));
        await click("openControlLesson", lessonId);
        await pollValue("lesson detail visible", () => inner("return String(Boolean(d.querySelector('.lesson-detail')));"), (value) => value === "true", 10_000);
        const lesson = JSON.parse(fs.readFileSync(path.join(extension.extensionPath, "media", "examples", entry.path, "lesson.json"), "utf8"));
        if (process.env.TRUST_EXAMPLES_FILE_PREVIEW === "1") {
          const target = process.env.TRUST_EXAMPLES_FILE_TARGET ?? "src/Main.st";
          const source = fs.readFileSync(path.join(extension.extensionPath, "media/examples", entry.path, target), "utf8");
          const workspace = vscode.workspace.workspaceFolders?.map(folder => folder.uri.toString());
          const selector = JSON.stringify(`[data-lesson-file=${JSON.stringify(target)}]`);
          assert.strictEqual(await inner(`return String(Boolean(d.querySelector(${selector})));`), "true",
            `the lesson must expose its authored ${target} reference as a file preview`);
          await inner(`d.querySelector(${selector}).click();return 'preview';`);
          await pollValue("bundled read-only source opened", async () => vscode.window.activeTextEditor?.document.uri.scheme ?? "",
            value => value === "trust-example-source", 10_000);
          assert.ok(vscode.window.tabGroups.activeTabGroup.tabs.some(tab => tab.input instanceof vscode.TabInputWebview),
            "the lesson tab must remain open in the same group while its source is previewed");
          const document = vscode.window.activeTextEditor!.document;
          assert.strictEqual(document.getText(), source);
          assert.strictEqual(document.uri.authority, "bundled");
          assert.deepStrictEqual(vscode.workspace.workspaceFolders?.map(folder => folder.uri.toString()), workspace);
          await gallery.pressKey("x", "KeyX");
          assert.strictEqual(document.getText(), source, "ordinary typing must not modify the bundled source preview");
          assert.strictEqual(document.isDirty, false);
          assert.strictEqual(fs.readFileSync(path.join(extension.extensionPath, "media/examples", entry.path, target), "utf8"), source);
          const capture = process.env.TRUST_EXAMPLES_RENDERED_CAPTURE;
          if (capture) await gallery.capture(capture.replace(/\.png$/, "-source-preview.png"));
          await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
          await vscode.commands.executeCommand("notifications.clearAll");
          await gallery.reconnect();
          await pollValue("same lesson restored after source preview", () => inner("return d.querySelector('#title')?.textContent ?? '';"),
            value => value === lesson.title, 10_000);
        }
        const normalize = (value: string) => value.replace(/\s+/g, " ").trim();
        const expectedText = async (source: string) => normalize(await inner(`var t=d.createElement('template');t.innerHTML=${JSON.stringify(new MarkdownIt({ html: false }).render(source))};return t.content.textContent;`));
        const rendered = JSON.parse(await inner("var c=d.querySelector('.lesson-detail').cloneNode(true);for(var m of c.querySelectorAll('math'))m.replaceWith('$'+m.querySelector('annotation[encoding=\"application/x-tex\"]')?.textContent+'$');return JSON.stringify({detail:c.textContent,paragraphs:[...d.querySelectorAll('.lesson-flow p')].map(x=>x.textContent)});")) as { detail: string; paragraphs: string[] };
        assert.ok(normalize(rendered.detail).includes(await expectedText(entry.description)), "lesson details must show the complete description, not the 80-character card text");
        assert.ok(normalize(rendered.detail).includes(await expectedText(lesson.objective)), "selected lesson objective must be visible");
        assert.ok(normalize(rendered.detail).includes(await expectedText(lesson.learning.validation)), "selected lesson expected observation must be visible");
        assert.strictEqual(await inner("return d.querySelector('#title').textContent;"), lesson.title,
          "the selected lesson name must be the primary page heading");
        assert.ok((await inner("return d.querySelector('#lead').textContent;")).includes(`Lesson ${lesson.sequence}`),
          "the lesson context must identify its position");
        assert.ok((await inner("return d.querySelector('#lead').textContent;")).includes(lesson.equipment),
          "the lesson context must identify its equipment");
        const actionLayout = JSON.parse(await inner("var b=d.querySelector('.lesson-detail button[data-action-type=useExample]');var r=b&&b.getBoundingClientRect();return JSON.stringify({present:Boolean(b),bottom:r&&r.bottom,viewport:d.defaultView.innerHeight});")) as { present: boolean; bottom?: number; viewport: number };
        assert.ok(actionLayout.present, "the lesson must expose its primary project action");
        assert.ok((actionLayout.bottom ?? Infinity) <= actionLayout.viewport,
          `the primary project action must be visible in the initial viewport: ${JSON.stringify(actionLayout)}`);
        const technical = JSON.parse(await inner("var dsc=d.querySelector('details.lesson-technical');return JSON.stringify({present:Boolean(dsc),open:Boolean(dsc&&dsc.open),summary:dsc&&dsc.querySelector('summary')?.textContent,detail:dsc&&dsc.textContent});")) as { present: boolean; open: boolean; summary?: string; detail?: string };
        assert.ok(technical.present, "technical lesson metadata must use a native disclosure");
        assert.strictEqual(technical.open, false, "technical metadata should be collapsed until requested");
        assert.ok(technical.summary, "the technical disclosure must have an obvious label");
        const observation = await inner("var c=d.querySelector('details.lesson-technical').cloneNode(true);for(var m of c.querySelectorAll('math'))m.replaceWith('$'+m.querySelector('annotation[encoding=\"application/x-tex\"]')?.textContent+'$');return c.textContent;");
        assert.ok(normalize(observation).includes(await expectedText(lesson.learning.validation)), "the disclosure must retain the complete expected observation");
        const format = JSON.parse(await inner("return JSON.stringify({code:d.querySelectorAll('.lesson-flow code').length,strong:d.querySelectorAll('.lesson-flow strong').length,table:d.querySelectorAll('.lesson-flow table').length,math:d.querySelectorAll('.lesson-flow math').length,files:d.querySelectorAll('.lesson-flow [data-lesson-file]').length,steps:d.querySelectorAll('.lesson-flow .lesson-markup ol > li').length});"));
        if (lessonId === "control-lesson-28") assert.deepStrictEqual(
          { code: format.code > 0, emphasis: format.strong > 0, files: format.files >= 2, steps: format.steps >= 2 },
          { code: true, emphasis: true, files: true, steps: true }, "the authored instructions must render as code, emphasis, usable file references and actual steps");
        if (lessonId === "control-lesson-28") {
          const prose = JSON.parse(await inner("var p=[...d.querySelectorAll('.lesson-flow .lesson-markup p')].find(p=>p.nextElementSibling?.tagName==='P');if(!p)return '{}';var s=d.defaultView.getComputedStyle(p);return JSON.stringify({gap:parseFloat(s.marginBottom),size:parseFloat(s.fontSize),color:s.color,body:d.defaultView.getComputedStyle(p.parentElement).color});"));
          assert.ok(prose.gap >= prose.size * .5, "authored paragraphs must retain readable separation");
          assert.strictEqual(prose.color, prose.body, "teaching prose must use the shared body text color");
        }
        if (lessonId === "control-lesson-04") assert.ok(format.table > 0, "the recorded comparison must render as a real table");
        if (lessonId === "control-water-heater-energy-balance") assert.ok(format.math > 0, "energy-balance equations must render as accessible math");
        const capture = process.env.TRUST_EXAMPLES_RENDERED_CAPTURE;
        if (capture) {
          await sleep(300);
          const metrics = await gallery.surfaceMetrics();
          const collapsed = capture.replace(/\.png$/, "-collapsed.png");
          await gallery.capture(collapsed);
          fs.writeFileSync(capture.replace(/\.png$/, "-surface.json"), JSON.stringify(metrics, null, 2));
          const surface = { width: Math.round(metrics.window.width * metrics.window.scale), height: Math.round(metrics.window.height * metrics.window.scale) };
          const requestedSurface = process.env.TRUST_EXAMPLES_RENDERED_SURFACE;
          if (requestedSurface) {
            const [width, height] = requestedSurface.split("x").map(Number);
            assert.ok(Math.abs(surface.width - width) <= 1 && Math.abs(surface.height - height) <= 1,
              `the native window must actually use the requested ${requestedSurface} surface: ${JSON.stringify(surface)}`);
          }
          const bitmap = fs.readFileSync(collapsed);
          assert.deepStrictEqual({ width: bitmap.readUInt32BE(16), height: bitmap.readUInt32BE(20) }, surface,
            "the screenshot must cover the complete native workbench at the active zoom");
        }
        await gallery.focusWebview();
        assert.strictEqual(await inner("var s=d.querySelector('details.lesson-technical summary');s.focus();return d.activeElement===s;"), "true",
          "the technical disclosure must be keyboard focusable");
        await gallery.pressKey("Enter");
        await pollValue("technical disclosure opened by keyboard", () => inner("return String(Boolean(d.querySelector('details.lesson-technical')?.open));"), (value) => value === "true", 5_000);
        if (capture) { await sleep(300); await gallery.capture(capture.replace(/\.png$/, "-expanded.png")); }
        await gallery.pressKey(" ", "Space");
        await pollValue("technical disclosure closed by Space", () => inner("return String(d.querySelector('details.lesson-technical').open);"), value => value === "false", 5_000);
        await gallery.pressKey("Enter");
        await pollValue("technical disclosure reopened", () => inner("return String(d.querySelector('details.lesson-technical').open);"), value => value === "true", 5_000);
        const frame = await gallery.webviewFrameRect();
        const summary = JSON.parse(await inner("const r=d.querySelector('details.lesson-technical summary').getBoundingClientRect();return JSON.stringify({x:r.x,y:r.y,width:r.width,height:r.height});"));
        assert.ok(frame);
        await gallery.clickWebviewPoint(frame.x + summary.x + summary.width / 2, frame.y + summary.y + summary.height / 2);
        assert.strictEqual(await inner("return String(d.querySelector('details.lesson-technical').open);"), "false", "pointer activation closes the same native disclosure");
        assert.ok((rendered.detail + rendered.paragraphs.join("\n")).includes(lesson.physicalPurpose),
          "the selected page must retain this lesson's explanation, not empty headings");
        const redundancy = JSON.parse(await inner("var intro=d.querySelector('.lesson-detail > .lesson-markup')?.textContent.trim()??d.querySelector('.lesson-detail > p')?.textContent;return JSON.stringify({repeatedIntro:[...d.querySelectorAll('.lesson-flow p')].some(p=>p.textContent.trim()===intro),readyCaption:[...d.querySelectorAll('.lesson-detail > p')].some(p=>p.textContent==='Ready to create'),incorrectPidCase:/\\b[Pp]id\\b/.test(d.querySelector('#crumb').textContent)});"));
        assert.deepStrictEqual(redundancy, { repeatedIntro: false, readyCaption: false, incorrectPidCase: false },
          "the selected lesson should not repeat its introduction or obvious action status, and must preserve PID's name");
        const layout = JSON.parse(await inner("return JSON.stringify({width:d.documentElement.clientWidth,content:d.documentElement.scrollWidth});")) as { width: number; content: number };
        assert.ok(layout.content <= layout.width + 1, `lesson detail must fit the available width: ${JSON.stringify(layout)}`);
        if (capture && process.env.TRUST_EXAMPLES_RENDERED_CONTENT_CAPTURE === "1") {
          const scroll = JSON.parse(await inner("var s=d.scrollingElement;return JSON.stringify({height:s.scrollHeight,viewport:s.clientHeight});"));
          const pages: Array<{ image: string; top: number }> = [];
          const step = Math.max(1, Math.floor(scroll.viewport * .85));
          for (let top = step; top < scroll.height; top += step) {
            const actual = Number(await inner(`d.scrollingElement.scrollTop=${top};return d.scrollingElement.scrollTop;`));
            if (pages[pages.length - 1]?.top === actual) break;
            const image = capture.replace(/\.png$/, `-content-${pages.length + 1}.png`);
            await sleep(120);
            await gallery.capture(image);
            pages.push({ image, top: actual });
            assert.ok(pages.length <= 20, "the selected lesson preview must have a bounded capture set");
          }
          fs.writeFileSync(capture.replace(/\.png$/, "-content.json"), JSON.stringify({ ...scroll, pages }, null, 2));
        }
        if (capture && process.env.TRUST_EXAMPLES_README_REVIEW === "1") {
          const readme = path.join(extension.extensionPath, "media/examples", entry.path, "README.md");
          const heading = fs.readFileSync(readme, "utf8").match(/^# (.+)$/m)?.[1];
          assert.ok(heading, "the shipped lesson must have its authored heading");
          await captureLessonMarkdownPreview(readme, capture.replace(/\.png$/, "-readme"), { readyText: heading, full: true });
        }
      } finally {
        gallery.close();
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      }
    });
  });
}
