import * as assert from "assert";
import { setIdentificationActionState } from "../../controlStudio/webview/identificationActions";
import { setWorkflowActionAvailability } from "../../controlStudio/webview/workflowActionAvailability";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

suite("Control Studio identification recording presentation", () => {
  test("a completed native recording selects Check and a ready Identify stage without requiring an import", () => {
    const dom = new JSDOM(`<main><button data-control-studio-mode aria-label="Identify, Needs data" data-status="needs-data">Identify</button><button data-identify-import></button><button data-identify-run></button><button data-identify-fit hidden></button><button data-identify-record-validation hidden></button><button data-identify-accept hidden></button></main>`);
    const previousDocument = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      setIdentificationActionState("initial");
      setWorkflowActionAvailability({ recording: true, result: true });
      const check = dom.window.document.querySelector("[data-identify-run]");
      assert.ok(check.classList.contains("primary") && !check.disabled, "recording readiness must select Check without an import message");
      assert.strictEqual(dom.window.document.querySelector("[data-control-studio-mode]").dataset.status, "ready");
      setIdentificationActionState("preview");
      setWorkflowActionAvailability({ candidate: false });
      assert.ok(dom.window.document.querySelector("[data-identify-record-validation]").classList.contains("primary"), "unrelated candidate updates must preserve the next identification action");
      setWorkflowActionAvailability({ model: true });
      assert.strictEqual(dom.window.document.querySelector("[data-control-studio-mode]").dataset.status, "complete");
      assert.strictEqual([...dom.window.document.querySelectorAll("button.primary")].filter((button: any) => !button.hidden && !button.disabled).length, 0);
    } finally { (globalThis as any).document = previousDocument; dom.window.close(); }
  });

  test("renders one next action for imported, qualified, preview-only, and accepted states", () => {
    const dom = new JSDOM(`<main><button data-identify-import></button><button data-identify-run></button><button data-identify-fit hidden></button><button data-identify-record-validation hidden></button><button data-identify-accept hidden></button></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    try {
      const visiblePrimary = () => [...dom.window.document.querySelectorAll("button.primary")].filter((button: any) => !button.hidden && !button.disabled);
      setIdentificationActionState("imported");
      assert.deepStrictEqual(visiblePrimary().map((button: any) => button.hasAttribute("data-identify-import") ? "import" : button.hasAttribute("data-identify-run") ? "check" : button.hasAttribute("data-identify-fit") ? "fit" : button.hasAttribute("data-identify-record-validation") ? "validation" : "accept"), ["check"]);
      setIdentificationActionState("qualified");
      assert.deepStrictEqual(visiblePrimary().map((button: any) => button.hasAttribute("data-identify-import") ? "import" : button.hasAttribute("data-identify-run") ? "check" : button.hasAttribute("data-identify-fit") ? "fit" : button.hasAttribute("data-identify-record-validation") ? "validation" : "accept"), ["fit"]);
      setIdentificationActionState("preview");
      assert.deepStrictEqual(visiblePrimary().map((button: any) => button.hasAttribute("data-identify-record-validation") ? "validation" : button.hasAttribute("data-identify-import") ? "import" : "other"), ["validation"]);
      setIdentificationActionState("acceptance");
      assert.deepStrictEqual(visiblePrimary().map((button: any) => button.hasAttribute("data-identify-accept") ? "accept" : "other"), ["accept"]);
      setIdentificationActionState("accepted");
      assert.deepStrictEqual(visiblePrimary(), []);
    } finally {
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      dom.window.close();
    }
  });

});
