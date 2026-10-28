import * as assert from "assert";
import * as vscode from "vscode";
import { proposalFor, resolveProposal } from "../../hmiBuilder/editor/proposals";
import { FakePanel, until } from "./hmiBuilderHost";
import { PAGE } from "./hmiBuilderWebview";

// Spec 43 AT-09: proposal mode has no normal-editor or runtime authority.
suite("HMI proposal host boundary", () => {
  function fixture(valid = true) {
    const calls: string[] = [];
    const panel = new FakePanel(),
      view = panel.webview;
    const session: NonNullable<ReturnType<typeof proposalFor>> = {
      uri: vscode.Uri.parse("trust-hmi-proposal:/workspace/draft/hmi/pages/review.toml"),
      project: vscode.Uri.file("/fixture"),
      model: {
        draftId: "draft",
        revision: "1",
        engineeringValid: valid,
        findings: [],
        files: [
          {
            file: "hmi/pages/review.toml",
            title: "Review",
            width: 640,
            height: 360,
            after: "image",
            changes: { added: [], removed: [], modified: [], pageProperties: [], orderChanged: false },
          },
        ],
      },
      text: "proposal",
      active: true,
      busy: false,
      status: "ready",
      ready: Promise.resolve(),
      started: () => {},
      cancelled: () => {},
      apply: async () => {
        calls.push("apply");
      },
      discard: async () => {
        calls.push("discard");
      },
    };
    resolveProposal(session, panel as unknown as vscode.WebviewPanel);
    return { session, panel, view, calls };
  }
  test("normal edit, deployment and runtime messages cannot invoke proposal actions", async () => {
    const { session, panel, view, calls } = fixture();
    try {
      view.send({ type: "edit", model: PAGE });
      view.send({ type: "deploy" });
      view.send({ type: "rollback" });
      view.send({ type: "live", on: true, paths: ["Main.Level"] });
      view.send({ type: "operateOpen", id: "injected", page: "overview" });
      await Promise.resolve();
      assert.deepStrictEqual(calls, []);
      view.send({ type: "proposalApply" });
      await until("review applied", () => session.status === "applied");
      assert.deepStrictEqual(calls, ["apply"]);
      view.send({ type: "proposalApply" });
      await Promise.resolve();
      assert.deepStrictEqual(calls, ["apply"], "terminal review cannot repeat its action");
    } finally {
      panel.dispose();
    }
  });
  test("invalid engineering cannot be applied by forging a webview message", async () => {
    const { panel, view, calls } = fixture(false);
    try {
      view.send({ type: "proposalApply" });
      await Promise.resolve();
      assert.deepStrictEqual(calls, []);
    } finally {
      panel.dispose();
    }
  });
  test("closing a view during an explicit application does not access a disposed webview", async () => {
    const { session, panel, view } = fixture();
    let finish!: () => void;
    session.apply = () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      });
    view.send({ type: "proposalApply" });
    assert.strictEqual(session.busy, true);
    panel.dispose();
    finish();
    await until("application finished after close", () => !session.busy);
    assert.strictEqual(session.status, "applied");
  });
});
