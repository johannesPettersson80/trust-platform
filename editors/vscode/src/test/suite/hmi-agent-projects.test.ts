import * as assert from "assert";
import * as fs from "fs/promises";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { workspaceIdentity } from "../../hmiBuilder/agent/tools";
import type { VariableNode } from "../../hmiBuilder/editor/messages";
import { languageServerReady } from "./hmiBuilderHost";
import { disposeFixtureProject } from "./fixtureDocuments";

// Public tool/native/editor journeys. Actual model-client acceptance is recorded separately;
// a scripted tool caller is never labelled a Codex or VS Code Agent conversation.
suite("HMI authoring across independent projects", function () {
  // A complete project journey includes bounded discovery plus several independent mutations.
  this.timeout(300000);
  const repo = path.resolve(__dirname, "../../../../..");
  async function folder(root: vscode.Uri, add: boolean): Promise<void> {
    const current = vscode.workspace.workspaceFolders ?? [];
    const index = add ? current.length : current.findIndex((item) => item.uri.toString() === root.toString());
    if (index < 0) return;
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        change.dispose();
        reject(new Error("workspace change timed out"));
      }, 10000);
      const change = vscode.workspace.onDidChangeWorkspaceFolders(() => {
        clearTimeout(timer);
        change.dispose();
        resolve();
      });
      if (
        !vscode.workspace.updateWorkspaceFolders(
          index,
          add ? 0 : 1,
          ...(add ? [{ uri: root, name: "Independent HMI project" }] : []),
        )
      ) {
        clearTimeout(timer);
        change.dispose();
        reject(new Error("workspace change refused"));
      }
    });
  }
  const leaves = (nodes: VariableNode[]): VariableNode[] =>
    nodes.flatMap((node) => [node, ...leaves(node.children ?? [])]);
  for (const fixture of ["hmi_tank_level", "hmi_equipment", "branching-process", "withheld-names"]) {
    test(`${fixture}: discover, compose, render, repair, validate, apply and Undo`, async () => {
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      await languageServerReady();
      const root = vscode.Uri.file(await fs.mkdtemp(path.join(os.tmpdir(), "trust-hmi-project-")));
      if (fixture.startsWith("hmi_"))
        await fs.cp(path.join(repo, "examples", fixture), root.fsPath, { recursive: true });
      else {
        await fs.mkdir(path.join(root.fsPath, "src"), { recursive: true });
        await fs.mkdir(path.join(root.fsPath, "hmi/pages"), { recursive: true });
        const name = fixture === "withheld-names" ? `District_${Date.now().toString(36)}` : "ThermalMixing";
        await fs.writeFile(
          path.join(root.fsPath, "src/Main.st"),
          `PROGRAM ${name}\nVAR Measurement : REAL := 32.0; Running : BOOL := TRUE; END_VAR\nEND_PROGRAM\n`,
        );
        await fs.writeFile(
          path.join(root.fsPath, "hmi/screen.toml"),
          'format = 1\nsize = [1280, 800]\ntheme = "dark"\nsymbols = "flat"\n',
        );
      }
      await folder(root, true);
      const cancellation = new vscode.CancellationTokenSource();
      const call = async (name: string, input: object) => {
        const reply = await vscode.lm.invokeTool(
          name,
          {
            input: { contractVersion: 1, workspaceId: workspaceIdentity(root), input },
            toolInvocationToken: undefined,
          },
          cancellation.token,
        );
        const result = JSON.parse((reply.content[0] as vscode.LanguageModelTextPart).value);
        assert.strictEqual(result.ok, true, JSON.stringify(result));
        return { data: result.data, content: reply.content };
      };
      try {
        const context = (await call("hmi_get_context", {})).data;
        const values = leaves(context.variables as VariableNode[]);
        const numeric = values.find((node) => node.kind === "value" && node.kinds.includes("number"));
        assert.ok(numeric, "native numeric variable available");
        const file = "hmi/pages/agent-acceptance.toml";
        const created = (
          await call("hmi_compose_page", {
            key: `project-${fixture}`,
            file,
            title: `${fixture} overview`,
            operatorTask: "Read the process and identify equipment needing attention",
            pattern: "process-overview",
            items: [
              {
                id: "vessel-a",
                type: "tank",
                region: "process",
                size: [192, 256],
                label: "Source vessel",
                bind: { level: { path: numeric!.path, direction: "read" } },
              },
              {
                id: "vessel-b",
                type: "tank",
                region: "process",
                size: [192, 256],
                label: "Receiving vessel",
                bind: { level: { path: numeric!.path, direction: "read" } },
              },
              {
                id: "reading",
                type: "value",
                region: "measurements",
                label: numeric!.name,
                bind: { value: { path: numeric!.path, direction: "read" } },
              },
            ],
            connections: fixture === "branching-process" ? [{ from: "vessel-a.outlet", to: "vessel-b.inlet" }] : [],
          })
        ).data;
        let revision = created.revision;
        const inspection = (
          await call("hmi_inspect_page", { file, draftId: created.draftId, revision, ids: ["vessel-a"] })
        ).data;
        assert.strictEqual(inspection.origin, "draft");
        assert.deepStrictEqual(
          inspection.objects.map((object: { id: string }) => object.id),
          ["vessel-a"],
        );
        const rendered = await call("hmi_render_draft", {
          draftId: created.draftId,
          revision,
          file,
          viewport: { width: 1280, height: 800 },
        });
        assert.ok(
          rendered.content.some(
            (part) => part instanceof vscode.LanguageModelDataPart && part.mimeType === "image/png",
          ),
        );
        if (process.env.HMI_AGENT_RENDER_OUT) {
          await fs.mkdir(process.env.HMI_AGENT_RENDER_OUT, { recursive: true });
          const image = rendered.content.find(
            (part) => part instanceof vscode.LanguageModelDataPart && part.mimeType === "image/png",
          ) as vscode.LanguageModelDataPart;
          await fs.writeFile(path.join(process.env.HMI_AGENT_RENDER_OUT, `${fixture}.png`), image.data);
        }
        const original = created.model.object.find((object: { id: string }) => object.id === "vessel-b").at as number[];
        const first = created.model.object.find((object: { id: string }) => object.id === "vessel-a").at as number[];
        revision = (
          await call("hmi_edit_draft", {
            draftId: created.draftId,
            revision,
            key: "seed-overlap",
            operations: [{ op: "move", ids: ["vessel-b"], delta: [first[0] - original[0], first[1] - original[1]] }],
          })
        ).data.revision;
        const flawed = (await call("hmi_validate_draft", { draftId: created.draftId, revision })).data;
        assert.ok(flawed.findings.some((finding: { rule: string }) => finding.rule === "object-overlap"));
        revision = (
          await call("hmi_edit_draft", {
            draftId: created.draftId,
            revision,
            key: "repair-overlap",
            operations: [{ op: "move", ids: ["vessel-b"], delta: [original[0] - first[0], original[1] - first[1]] }],
          })
        ).data.revision;
        const valid = (await call("hmi_validate_draft", { draftId: created.draftId, revision })).data;
        assert.strictEqual(valid.engineeringValid, true);
        await call("hmi_apply_draft", { draftId: created.draftId, revision, action: "review" });
        assert.strictEqual(
          await fs.stat(path.join(root.fsPath, file)).then(
            () => true,
            () => false,
          ),
          false,
        );
        await call("hmi_apply_draft", { draftId: created.draftId, revision, action: "apply" });
        const document = await vscode.workspace.openTextDocument(vscode.Uri.joinPath(root, file));
        assert.ok(document.getText().includes("vessel-a"));
        await vscode.window.showTextDocument(document);
        await vscode.commands.executeCommand("undo");
        assert.ok(
          !vscode.workspace.textDocuments
            .find((item) => item.uri.toString() === document.uri.toString() && !item.isClosed)
            ?.getText()
            .includes("vessel-a"),
        );
      } finally {
        cancellation.dispose();
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
        for (const document of vscode.workspace.textDocuments)
          if (document.uri.fsPath.startsWith(root.fsPath + path.sep) && document.isDirty) await document.save();
        await disposeFixtureProject(root.fsPath);
        await folder(root, false);
      }
    });
  }
});
