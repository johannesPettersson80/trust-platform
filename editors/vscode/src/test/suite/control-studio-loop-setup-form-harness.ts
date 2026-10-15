import * as assert from "assert";
import { renderLoopSetupBody } from "../../controlStudio/loopSetup/loopSetupTemplate";
import { initializeLoopSetupForm } from "../../controlStudio/webview/loopSetupForm";

/** Static imports: the webview module is compiled for tests only when a test imports it. */
function form(): any {
  return { renderLoopSetupBody, initializeLoopSetupForm };
}

/** The setup form rendered in a DOM, with a setter that fires the events a user's edit fires. */
export function open(model: any): { dom: any; posts: any[]; root: HTMLElement; set: (field: string, value: string) => void;
  showError: (reason: string, field?: string) => void; close: () => void } {
  const api = form();
  assert.strictEqual(typeof api.renderLoopSetupBody, "function", "setup needs one form instead of a prompt chain");
  assert.strictEqual(typeof api.initializeLoopSetupForm, "function");
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main>${api.renderLoopSetupBody(model, (text: string) => text)}</main>`);
  const root = dom.window.document.querySelector("main") as HTMLElement;
  const posts: any[] = [];
  const setup = api.initializeLoopSetupForm(root, (message: unknown) => posts.push(message));
  const set = (field: string, value: string) => {
    const element = root.querySelector<HTMLInputElement>(`[data-setup-field="${field}"]`);
    assert.ok(element, `field ${field} is present`);
    element!.value = value;
    element!.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
    element!.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
  };
  return { dom, posts, root, set, showError: (reason: string, field?: string) => setup.showError(reason, field), close: () => dom.window.close() };
}
