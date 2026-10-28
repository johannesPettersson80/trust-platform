import * as assert from "assert";
import { Webview } from "./hmiBuilderWebview";

// Spec 43 AT-09: the existing builder has a read-only proposal review mode with explicit actions.
suite("HMI Builder proposal review", () => {
  test("shows synthetic comparison and changes without design or runtime controls", async () => {
    const view = new Webview("proposal" as "page");
    try {
      await view.ready();
      view.window.dispatchEvent(
        new view.window.MessageEvent("message", {
          data: {
            type: "proposal",
            active: true,
            proposal: {
              draftId: "fixture",
              revision: "2",
              engineeringValid: true,
              findings: [],
              intent: {
                operatorTask: "Monitor the storage tank",
                taskSource: "request",
                rationale: "Keep level and transfer status together.",
                assumptions: ["Connections are supplied by the engineer."],
              },
              files: [
                {
                  file: "hmi/pages/process.toml",
                  title: "Tank process",
                  width: 640,
                  height: 360,
                  before: "before-image",
                  after: "after-image",
                  changes: {
                    added: [],
                    removed: [],
                    modified: [{ id: "level", properties: ["label"] }],
                    pageProperties: [],
                    orderChanged: false,
                  },
                },
              ],
            },
          },
        }),
      );
      await new Promise((resolve) => setTimeout(resolve, 50));
      const review = view.query("[data-proposal-review]");
      assert.ok(review, "the builder must show the isolated proposal review, not its normal design editor");
      assert.match(review.textContent, /Synthetic preview/i);
      assert.match(review.textContent, /Monitor the storage tank/);
      assert.match(review.textContent, /Connections are supplied by the engineer/);
      assert.match(review.textContent, /level/);
      assert.match(review.textContent, /label/i);
      assert.strictEqual(view.query("[data-proposal-image]").getAttribute("src"), "data:image/png;base64,after-image");
      view.query('[data-proposal-view="before"]').click();
      await view.until(
        "base image",
        () => view.query("[data-proposal-image]")?.getAttribute("src") === "data:image/png;base64,before-image",
      );
      assert.strictEqual(view.query('[data-action="deploy"]'), null);
      assert.strictEqual(view.query('[data-action="live"]'), null);
      assert.strictEqual(view.query("[data-proposal-apply]").disabled, false);
      view.query("[data-proposal-apply]").click();
      assert.ok(
        view.posted.some((message) => message.type === "proposalApply"),
        "Apply is an explicit host action",
      );
      assert.ok(
        !view.posted.some((message) => ["edit", "deploy", "live"].includes(message.type)),
        "review itself sends no source edit or runtime action",
      );
    } finally {
      view.window.close();
    }
  });
});
