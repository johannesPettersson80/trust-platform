import * as assert from "assert";
import { IDENTIFICATION_IMPORT_CONTROLS, initializeIdentificationRecordingImport } from "../../controlStudio/webview/identificationRecordingImport";
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
  test("keeps import as the initial primary action and moves provenance/action details into the summary panel", () => {
    const dom = new JSDOM(`<section data-workflow-pane="Identify"><div class="workflow-grid"><div class="control-group">${IDENTIFICATION_IMPORT_CONTROLS}</div><div class="summary-panel"><h3>Model qualification</h3></div></div></section>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    try {
      const document = dom.window.document;
      const controls = require("../../controlStudio/webview/identificationRecordingImport") as any;
      initializeIdentificationRecordingImport(() => undefined, () => undefined);
      assert.strictEqual(document.querySelector("[data-identify-import]")?.classList.contains("primary"), true);
      assert.strictEqual(typeof controls.mountIdentificationRecordingDetails, "function");
      controls.mountIdentificationRecordingDetails();
      assert.ok(document.querySelector("[data-identify-recording-details]")?.closest(".summary-panel"));
      assert.ok(document.querySelector("[data-identify-record-validation]")?.closest(".summary-panel"));
    } finally {
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      dom.window.close();
    }
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

  test("preview validation action selects Test without dispatching a runtime command", () => {
    const dom = new JSDOM(`<main data-control-studio data-project-id="p" data-editor-session-id="s" data-loop-id="l" data-loop-revision="r"><button data-control-studio-mode>Test</button>${IDENTIFICATION_IMPORT_CONTROLS}<button data-identify-run></button><button data-identify-fit hidden></button><button data-identify-accept hidden></button></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    try {
      const posts: string[] = [];
      let selectedTest = false;
      dom.window.document.querySelector("[data-control-studio-mode]")!.addEventListener("click", () => { selectedTest = true; });
      initializeIdentificationRecordingImport((kind) => posts.push(kind), () => undefined);
      setIdentificationActionState("preview");
      (dom.window.document.querySelector("[data-identify-record-validation]") as HTMLButtonElement).click();
      assert.strictEqual(selectedTest, true, "validation preview must navigate through the existing Test tab");
      assert.deepStrictEqual(posts, [], "validation preview must not start a test or write runtime state");
    } finally {
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      dom.window.close();
    }
  });
});
