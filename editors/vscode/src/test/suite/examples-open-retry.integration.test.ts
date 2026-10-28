import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { captureRenderedWorkbench, connectCdp, devToolsPort, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";
import { openRenderedExamplesGallery } from "./examples-rendered-harness";

if (process.env.TRUST_PLANT_RENDERED_TEST === "1") {
  suite("Rendered example open recovery", function () {
    this.timeout(90_000);

    test("CPEX-009 retries the same copied destination from the real notification", async () => {
      const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
      assert.ok(extension);
      await extension.activate();
      const destination = await fs.mkdtemp(path.join(os.tmpdir(), "trust-cpex009-retry-"));
      const previousDestination = process.env.TRUST_UX_EXAMPLE_DESTINATION;
      const previousName = process.env.TRUST_UX_EXAMPLE_NAME;
      const previousOpen = process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
      const originalExecute = vscode.commands.executeCommand;
      const gitConfig = vscode.workspace.getConfiguration("git");
      const previousGitParentSearch = gitConfig.get("openRepositoryInParentFolders");
      process.env.TRUST_UX_EXAMPLE_DESTINATION = destination;
      process.env.TRUST_UX_EXAMPLE_NAME = "retry-lesson-copy";
      delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
      const openPaths: string[] = [];
      let attempts = 0;
      vscode.commands.executeCommand = (async (command: string, ...args: unknown[]) => {
        if (command === "vscode.openFolder") {
          openPaths.push((args[0] as vscode.Uri).fsPath);
          attempts++;
          if (attempts === 1) throw new Error("deliberate first open failure");
          return undefined;
        }
        return originalExecute(command, ...args);
      }) as typeof vscode.commands.executeCommand;
      let gallery: Awaited<ReturnType<typeof openRenderedExamplesGallery>> | undefined;
      let connection: Awaited<ReturnType<typeof connectCdp>> | undefined;
      try {
        await gitConfig.update("openRepositoryInParentFolders", "never", vscode.ConfigurationTarget.Workspace);
        gallery = await openRenderedExamplesGallery();
        await originalExecute("workbench.action.toggleFullScreen");
        await new Promise((resolve) => setTimeout(resolve, 250));
        const inner = gallery.inner;
        const click = async (action: string, id?: string) => pollValue(
          action,
          () => inner(`var b=[...d.querySelectorAll('button')].find(x=>x.dataset.actionType===${JSON.stringify(action)}${id ? `&&x.dataset.actionId===${JSON.stringify(id)}` : ""});if(!b)return 'WAIT';b.click();return 'CLICKED';`),
          (value) => value === "CLICKED", 10_000
        );
        await click("openControlCurriculum");
        await click("openControlLesson", "control-lesson-01");
        await click("useExample", "control-lesson-01");
        await pollValue("first open failure", async () => String(attempts), (value) => value === "1", 10_000);
        const readmePath = path.join(destination, "retry-lesson-copy", "README.md");
        const sentinel = "\nRetry proof sentinel — existing learner edit\n";
        await fs.appendFile(readmePath, sentinel);
        const beforeRetry = await fs.readFile(readmePath, "utf8");
        await originalExecute("notifications.showList");
        await new Promise((resolve) => setTimeout(resolve, 250));

        const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
        const version = await httpJson(port, "/json/version") as { webSocketDebuggerUrl: string };
        connection = await connectCdp(version.webSocketDebuggerUrl);
        const page = await pollValue("workbench notification", async () => {
          const targets = await httpJson(port, "/json") as Array<{ id: string; type?: string; url: string }>;
          return targets.find((target) => target.type === "page" && !/index\.html/.test(target.url));
        }, Boolean, 10_000);
        assert.ok(page);
        const attached = await connection.send("Target.attachToTarget", { targetId: page.id, flatten: true });
        await connection.send("Runtime.enable", {}, attached.sessionId);
        const buttonPoint = async (label: string, notificationText?: string) => {
          const result = await connection!.send("Runtime.evaluate", {
            expression: `(()=>{const button=[...document.querySelectorAll('button,[role=button]')].find((candidate)=>(candidate.textContent||'').trim()===${JSON.stringify(label)});if(!button)return JSON.stringify({button:false});const row=button.closest('.notification-list-item');if(!row${notificationText ? `||!(row.textContent||'').includes(${JSON.stringify(notificationText)})` : ""})return JSON.stringify({button:false});const r=button.getBoundingClientRect();const hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return JSON.stringify({button:true,x:r.x+r.width/2,y:r.y+r.height/2,visible:r.width>0&&r.height>0&&r.top>=0&&r.left>=0&&r.bottom<=innerHeight&&r.right<=innerWidth,exactHit:hit===button||button.contains(hit)})})()`,
            returnByValue: true,
          }, attached.sessionId);
          return JSON.parse(String(result.result?.value ?? "{}")) as {
            button: boolean; x?: number; y?: number; visible?: boolean; exactHit?: boolean;
          };
        };
        const clickPoint = async (point: { x: number; y: number }) => {
          await connection!.send("Input.dispatchMouseEvent", {
            type: "mouseMoved", x: point.x, y: point.y,
          }, attached.sessionId);
          await connection!.send("Input.dispatchMouseEvent", {
            type: "mousePressed", x: point.x, y: point.y, button: "left", clickCount: 1,
          }, attached.sessionId);
          await connection!.send("Input.dispatchMouseEvent", {
            type: "mouseReleased", x: point.x, y: point.y, button: "left", clickCount: 1,
          }, attached.sessionId);
        };

        const gitNotification = await buttonPoint("Never", "A git repository was found in the parent folders");
        if (gitNotification.button) {
          assert.strictEqual(gitNotification.visible, true, "Git notification Never action must be visible");
          assert.strictEqual(gitNotification.exactHit, true, "Git notification Never action must be unobscured");
          await clickPoint({ x: gitNotification.x!, y: gitNotification.y! });
          await pollValue("Git parent notification dismissed", async () => {
            const result = await connection!.send("Runtime.evaluate", {
              expression: "![...document.querySelectorAll('.notification-list-item')].some((row)=>(row.textContent||'').includes('A git repository was found in the parent folders'))",
              returnByValue: true,
            }, attached.sessionId);
            return result.result?.value;
          }, (value) => value === true, 10_000);
        }

        const retainedPath = openPaths[0];
        assert.ok(retainedPath, "the failed open must retain one copied destination");
        const notification = await pollValue("Retry opening notification", async () => {
          const result = await connection!.send("Runtime.evaluate", {
            expression: `(()=>{const button=[...document.querySelectorAll('button,[role=button]')].find((candidate)=>(candidate.textContent||'').trim()==='Retry opening');if(!button)return JSON.stringify({button:false,text:document.body?.innerText||''});const row=button.closest('.notification-list-item');if(!row)return JSON.stringify({button:false,text:document.body?.innerText||''});const r=button.getBoundingClientRect();const rr=row.getBoundingClientRect();const hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);const message=[...row.querySelectorAll('*')].find((candidate)=>(candidate.textContent||'').includes(${JSON.stringify(retainedPath)})&&candidate.children.length===0);const mr=message?.getBoundingClientRect();const style=getComputedStyle(row);const rowOpacity=Number.parseFloat(style.opacity);const rowTransform=style.transform;const visible=r.width>0&&r.height>0&&r.top>=0&&r.left>=0&&r.bottom<=innerHeight&&r.right<=innerWidth;const rowVisible=rr.width>0&&rr.height>0&&rr.top>=0&&rr.left>=0&&rr.bottom<=innerHeight&&rr.right<=innerWidth;const exactHit=hit===button||button.contains(hit);const messageVisible=Boolean(message&&mr&&mr.width>0&&mr.height>0&&mr.top>=0&&mr.left>=0&&mr.bottom<=innerHeight&&mr.right<=innerWidth);const settled=rowOpacity>=0.99&&(rowTransform==='none'||rowTransform==='matrix(1, 0, 0, 1, 0, 0)');return JSON.stringify({button:true,x:r.x+r.width/2,y:r.y+r.height/2,visible,rowVisible,exactHit,messageVisible,settled,rowOpacity,rowTransform,rowText:row.textContent||'',text:document.body?.innerText||''})})()`,
            returnByValue: true,
          }, attached.sessionId);
          return result.result?.value;
        }, (value) => {
          if (!value) return false;
          const details = JSON.parse(value);
          return details.button
            && details.visible
            && details.rowVisible
            && details.messageVisible
            && details.exactHit
            && details.settled;
        }, 10_000);
        const notificationDetails = JSON.parse(notification);
        if (process.env.TRUST_EXAMPLES_RETRY_CAPTURE) captureRenderedWorkbench(process.env.TRUST_EXAMPLES_RETRY_CAPTURE);
        const geometry = JSON.stringify(notificationDetails);
        assert.ok(notificationDetails.rowText.includes(retainedPath), `retry notification must name the exact retained destination: ${geometry}`);
        assert.strictEqual(notificationDetails.visible, true, `Retry action must have visible bounds: ${geometry}`);
        assert.strictEqual(notificationDetails.rowVisible, true, `whole Retry notification must be inside the workbench viewport: ${geometry}`);
        assert.strictEqual(notificationDetails.messageVisible, true, `retained destination text must be visibly laid out: ${geometry}`);
        assert.strictEqual(notificationDetails.exactHit, true, `Retry action must be the exact unobscured hit target: ${geometry}`);
        assert.strictEqual(notificationDetails.settled, true, `Retry notification must finish its opacity/transform transition: ${geometry}`);
        await clickPoint({ x: notificationDetails.x, y: notificationDetails.y });
        await pollValue("retry open attempt", async () => String(attempts), (value) => value === "2", 10_000);
        assert.strictEqual(openPaths[1], openPaths[0]);
        assert.strictEqual(await fs.readFile(readmePath, "utf8"), beforeRetry, "retry must not recopy over learner edits");
      } finally {
        connection?.close();
        gallery?.close();
        vscode.commands.executeCommand = originalExecute;
        if (previousDestination === undefined) delete process.env.TRUST_UX_EXAMPLE_DESTINATION; else process.env.TRUST_UX_EXAMPLE_DESTINATION = previousDestination;
        if (previousName === undefined) delete process.env.TRUST_UX_EXAMPLE_NAME; else process.env.TRUST_UX_EXAMPLE_NAME = previousName;
        if (previousOpen === undefined) delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER; else process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = previousOpen;
        await gitConfig.update("openRepositoryInParentFolders", previousGitParentSearch, vscode.ConfigurationTarget.Workspace);
        await fs.rm(destination, { recursive: true, force: true });
      }
    });
  });
}
