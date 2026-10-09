import * as assert from "assert";
import * as fs from "fs/promises";
import * as path from "path";
import * as vscode from "vscode";
import { languageServerReady } from "./hmiBuilderHost";
import { attachWebview } from "./control-studio-rendered-harness";
import {
  captureRenderedWorkbenchCdp,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
} from "./plant-rendered-harness";

// Spec 43 AT-09: actual builder review, explicit source application and Undo; no runtime operation.
suite("HMI agent proposal rendered", () => {
  test("reviews HMI themes and high contrast and applies through the visible builder action", async function () {
    if (process.env.TRUST_HMI_PROPOSAL_RENDERED !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
    await vscode.commands.executeCommand("workbench.action.closeSidebar");
    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    const root = vscode.workspace.workspaceFolders![0].uri;
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const screenshot = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT");
    async function call(name: string, input: object) {
      const cancellation = new vscode.CancellationTokenSource();
      try {
        const result = await vscode.lm.invokeTool(
          name,
          { input: { contractVersion: 1, input }, toolInvocationToken: undefined },
          cancellation.token,
        );
        const payload = JSON.parse((result.content[0] as vscode.LanguageModelTextPart).value);
        assert.strictEqual(payload.ok, true, JSON.stringify(payload));
        return payload.data;
      } finally {
        cancellation.dispose();
      }
    }
    try {
      for (const theme of ["dark", "light", "high-contrast"] as const) {
        if (process.env.TRUST_HMI_PROPOSAL_THEME && process.env.TRUST_HMI_PROPOSAL_THEME !== theme) continue;
        await vscode.workspace
          .getConfiguration("workbench")
          .update(
            "colorTheme",
            theme === "dark"
              ? "Default Dark Modern"
              : theme === "light"
                ? "Default Light Modern"
                : "Default High Contrast",
            vscode.ConfigurationTarget.Workspace,
          );
        const screen = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(root, "hmi/screen.toml"));
        const edit = new vscode.WorkspaceEdit();
        edit.replace(
          screen.uri,
          new vscode.Range(0, 0, screen.lineCount, 0),
          screen.getText().replace(/theme = "(?:dark|light)"/, `theme = "${theme === "light" ? "light" : "dark"}"`),
        );
        assert.ok(await vscode.workspace.applyEdit(edit));
        const file = `hmi/pages/agent-review-${theme}.toml`;
        const created = await call("hmi_compose_page", {
          key: `review-ui-${theme}`,
          file,
          title: "T-101 · Tank level control",
          operatorTask: "Monitor tank level and transfer status",
          rationale: "Keep the process and its key readings together.",
          assumptions: ["The transfer connection is supplied by the project."],
          pattern: "process-overview",
          items: [
            {
              id: "pump",
              type: "pump",
              tag: "P-101",
              label: "Inlet pump",
              region: "process",
              bind: { running: { path: "Tank.Pump.Running", direction: "read" } },
            },
            {
              id: "tank",
              type: "tank",
              tag: "T-101",
              label: "Buffer tank",
              region: "process",
              min: 0,
              max: 3,
              bind: { level: { path: "Tank.Level", direction: "read" } },
            },
            {
              id: "level",
              type: "value",
              label: "Tank level",
              unit: "m",
              decimals: 2,
              region: "measurements",
              bind: { value: { path: "Tank.Level", direction: "read" } },
            },
            {
              id: "setpoint",
              type: "value",
              label: "Setpoint",
              unit: "m",
              decimals: 2,
              min: 0,
              max: 3,
              region: "measurements",
              bind: { value: { path: "Tank.Setpoint", direction: "read" } },
            },
          ],
          connections: [{ from: "pump.outlet", to: "tank.inlet" }],
        });
        await call("hmi_apply_draft", {
          draftId: created.draftId,
          revision: created.revision,
          action: "review",
          explore: true,
        });
        const source = vscode.Uri.joinPath(root, file);
        await assert.rejects(fs.stat(source.fsPath), "review must not create the proposed source file");
        const session = await attachWebview(connection, port, "[data-proposal-review]");
        const inner = (body: string) => evaluateInWebview(connection, session, body);
        await pollValue(
          "proposal image and enabled Apply",
          () =>
            inner(
              "const image=d.querySelector('[data-proposal-image]');return String(image?.complete&&image.naturalWidth>0&&!d.querySelector('[data-proposal-apply]').disabled);",
            ),
          (value) => value === "true",
          15000,
        );
        assert.match(await inner("return d.querySelector('[data-proposal-review]').textContent;"), /Synthetic preview/);
        assert.match(
          await inner("return d.querySelector('[aria-label=\"Design intent\"]').textContent;"),
          /Monitor tank level and transfer status/,
        );
        // Inspect the final shared color, not the transient disabled-to-enabled animation.
        await inner("d.querySelector('[data-proposal-apply]').style.transition='none';return 'settled';");
        const colors = JSON.parse(
          await inner(
            "const reference=d.createElement('button');reference.className='trust-button trust-button--primary';d.body.append(reference);const get=element=>{const style=d.defaultView.getComputedStyle(element);return [style.backgroundColor,style.color];};const result={actual:get(d.querySelector('[data-proposal-apply]')),expected:get(reference)};reference.remove();return JSON.stringify(result);",
          ),
        );
        assert.deepStrictEqual(
          colors.actual,
          colors.expected,
          "Apply uses the shared primary action colors in the real browser",
        );

        assert.strictEqual(
          await inner("return String(!!d.querySelector('[data-action=deploy],[data-action=live]'));"),
          "false",
        );
        await fs.mkdir(path.dirname(screenshot), { recursive: true });
        await fs.writeFile(
          screenshot.replace(/\.png$/, `-${theme}-diagnostics.json`),
          JSON.stringify(
            vscode.languages.getDiagnostics().map(([uri, diagnostics]) => ({
              uri: uri.toString(),
              diagnostics: diagnostics.map((item) => ({
                message: item.message,
                source: item.source,
                severity: item.severity,
              })),
            })),
            null,
            2,
          ),
        );
        await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, `-${theme}.png`));
        await inner("d.querySelector('[aria-label=\"Design intent\"]').scrollIntoView({block:'end'});return 'intent';");
        await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, `-${theme}-intent.png`));
        await inner("d.querySelector('.hb-proposal-summary').scrollTop=0;return 'top';");
        const initialScale = Number(
          await inner(
            "const image=d.querySelector('[data-proposal-image]');return String(image.getBoundingClientRect().width/image.naturalWidth);",
          ),
        );
        assert.ok(initialScale >= 0.75, "the initial review keeps the readable 75% minimum");
        if (theme === "dark") {
          await inner(
            "const button=[...d.querySelectorAll('button')].find(button=>button.textContent==='Explore navigation'); if(!button)throw new Error('Missing exploration action');button.click();return 'clicked';",
          );
          await pollValue(
            "synthetic exploration renders",
            () => inner("return String(!!d.querySelector('[aria-label=\"Explore synthetic HMI\"] svg'));"),
            (value) => value === "true",
            15000,
          );
          await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, `-${theme}-exploration.png`));
          await inner(
            "[...d.querySelectorAll('button')].find(button=>button.textContent==='Return to images').click();return 'clicked';",
          );
        }

        await inner("[...d.querySelectorAll('button')].find(button=>button.textContent==='Fit').click();return 'fit';");
        await pollValue(
          "explicit full-page fit",
          () =>
            inner(
              "const area=d.querySelector('.hb-proposal-viewport');return String(area.scrollWidth<=area.clientWidth+3&&area.scrollHeight<=area.clientHeight+3);",
            ),
          (value) => value === "true",
          5000,
        );
        await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, `-${theme}-fit.png`));

        if (theme === "high-contrast") {
          assert.strictEqual(
            await inner(
              "const button=d.querySelector('[data-proposal-apply]');button.focus();return String(d.activeElement===button);",
            ),
            "true",
            "Apply is keyboard focusable",
          );
          await connection.send(
            "Input.dispatchKeyEvent",
            {
              type: "keyDown",
              key: "Enter",
              code: "Enter",
              text: "\r",
              unmodifiedText: "\r",
              windowsVirtualKeyCode: 13,
              nativeVirtualKeyCode: 13,
            },
            session,
          );
          await connection.send(
            "Input.dispatchKeyEvent",
            { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 },
            session,
          );
        } else await inner("d.querySelector('[data-proposal-apply]').click();return 'apply';");
        await pollValue(
          "proposal applied",
          () => inner("return d.querySelector('[data-proposal-apply]').textContent;"),
          (value) => value === "Applied",
          15000,
        );
        const document = await vscode.workspace.openTextDocument(source);
        assert.ok(document.getText().includes("Tank level control"));
        assert.ok(document.isDirty, "Apply does not silently save");
        // Source diagnostics are asynchronous and debounced; capture the settled editor state.
        await new Promise((resolve) => setTimeout(resolve, 500));
        await pollValue(
          "valid applied source diagnostics",
          async () =>
            String(
              vscode.languages
                .getDiagnostics(source)
                .filter((item) => item.source === "truST HMI" && item.severity === vscode.DiagnosticSeverity.Error)
                .length,
            ),
          (value) => value === "0",
          5000,
        );
        // Workbench tab decorations consume diagnostic changes on their own debounce.
        await new Promise((resolve) => setTimeout(resolve, 1000));
        assert.strictEqual(
          vscode.languages.getDiagnostics(source).filter((item) => item.severity === vscode.DiagnosticSeverity.Error)
            .length,
          0,
        );
        await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, `-${theme}-applied.png`));
        await vscode.window.showTextDocument(document);
        await vscode.commands.executeCommand("undo");
        const remaining = vscode.workspace.textDocuments.find(
          (candidate) => candidate.uri.toString() === source.toString() && !candidate.isClosed,
        );
        assert.ok(
          !remaining || !remaining.getText().includes("Tank level control"),
          "one Undo removes the proposed source change",
        );
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      }
    } finally {
      connection.close();
    }
  });
  test("explores existing navigation inferred faceplates and alarm equipment without runtime access", async function () {
    if (process.env.TRUST_HMI_EXPLORATION_RENDERED !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await languageServerReady();
    await vscode.commands.executeCommand("workbench.action.closeSidebar");
    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const cancellation = new vscode.CancellationTokenSource();
    const call = async (name: string, input: object) => {
      const reply = await vscode.lm.invokeTool(
        name,
        { input: { contractVersion: 1, input }, toolInvocationToken: undefined },
        cancellation.token,
      );
      const result = JSON.parse((reply.content[0] as vscode.LanguageModelTextPart).value);
      assert.strictEqual(result.ok, true, JSON.stringify(result));
      return result.data;
    };
    try {
      const created = await call("hmi_compose_page", {
        key: "exploration",
        file: "hmi/pages/agent-exploration.toml",
        title: "Transfer review",
        pattern: "process-overview",
        items: [
          {
            id: "level",
            type: "value",
            label: "Tank level",
            unit: "m",
            bind: { value: { path: "Main.T101.Level", direction: "read" } },
          },
        ],
      });
      await call("hmi_apply_draft", {
        draftId: created.draftId,
        revision: created.revision,
        action: "review",
        explore: true,
      });
      const session = await attachWebview(connection, port, "[data-proposal-review]");
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const click = async (selector: string) =>
        inner(
          `const element=d.querySelector(${JSON.stringify(selector)});if(!element)throw new Error('Missing '+${JSON.stringify(selector)});element.dispatchEvent(new d.defaultView.MouseEvent('click',{bubbles:true,cancelable:true}));return 'clicked';`,
        );
      await pollValue(
        "proposal available",
        () => inner("return String(!!d.querySelector('[data-proposal-image]'));"),
        (value) => value === "true",
        15000,
      );
      await inner(
        "[...d.querySelectorAll('button')].find(button=>button.textContent==='Explore navigation').click();return 'explore';",
      );
      const scope = '[aria-label="Explore synthetic HMI"]';
      await pollValue(
        "navigation available",
        () => inner(`return String(!!d.querySelector('${scope} a[href="/hmi/overview"]'));`),
        (value) => value === "true",
        15000,
      );
      await pollValue(
        "synthetic values delivered",
        () => inner(`return String(!!d.querySelector('${scope} [data-object="level"][data-state="confirmed"]'));`),
        (value) => value === "true",
        10000,
      );
      assert.strictEqual(
        await inner(
          `const preview=d.querySelector('${scope}').getBoundingClientRect();const frame=d.querySelector('${scope} .hmi-frame').getBoundingClientRect();return String(frame.top>=preview.top&&frame.left>=preview.left&&frame.bottom<=preview.bottom+1&&frame.right<=preview.right+1);`,
        ),
        "true",
        "preview cannot cover the review toolbar or findings",
      );
      await click(`${scope} a[href="/hmi/overview"]`);
      await pollValue(
        "equipment overview",
        () => inner(`return String(!!d.querySelector('${scope} [data-object="p-101"]'));`),
        (value) => value === "true",
        10000,
      );
      await click(`${scope} [data-object="p-101"]`);
      await pollValue(
        "inferred pump faceplate",
        () => inner(`return String(!!d.querySelector('${scope} .hmi-popup svg'));`),
        (value) => value === "true",
        10000,
      );
      const screenshot = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT");
      await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-faceplate.png"));
      await click(`${scope} .hmi-popup [data-action="close"]`);
      assert.strictEqual(await inner(`return String(!!d.querySelector('${scope} .hmi-popup'));`), "false");
      await click(`${scope} a[href="/hmi/cards"]`);
      await pollValue(
        "placed equipment cards",
        () => inner(`return String(!!d.querySelector('${scope} [data-placed="p101"]'));`),
        (value) => value === "true",
        10000,
      );
      await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-cards.png"));
      await click(`${scope} a[href="/hmi/alarms"]`);
      await pollValue(
        "synthetic alarm rows",
        () => inner(`return String(!!d.querySelector('${scope} [data-part="alarm-row"]'));`),
        (value) => value === "true",
        10000,
      );
      await click(`${scope} [data-part="alarm-row"]`);
      await pollValue(
        "alarm details with equipment destination",
        () =>
          inner(
            `const button=d.querySelector('${scope} [data-action="show-equipment"]');return String(!!button&&!button.disabled);`,
          ),
        (value) => value === "true",
        10000,
      );
      await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-alarm-details.png"));
      await click(`${scope} [data-action="show-equipment"]`);
      await pollValue(
        "equipment focus",
        () => inner(`return String(!!d.querySelector('${scope} [data-focused="true"]'));`),
        (value) => value === "true",
        10000,
      );
      await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-equipment-focus.png"));
      const root = vscode.workspace.workspaceFolders![0].uri;
      await assert.rejects(fs.stat(vscode.Uri.joinPath(root, "hmi/pages/agent-exploration.toml").fsPath));
    } finally {
      cancellation.dispose();
      connection.close();
    }
  });
});
