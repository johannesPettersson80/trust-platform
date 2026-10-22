import * as assert from "assert";

import { ThreePageMethod } from "../../controlStudio/webview/threePageMethod";

const { JSDOM } = require("jsdom");

/** The methods whose offer says the study records them (`records`, B3.2); the others are the page's own view. */
const RECORDED = new Set(["automatic", "mv_step", "relay"]);

/** The Identify page's Method list as the engine offers it, with a page that records what the list asks of it (specification 35, B3.2). */
function surface() {
  const dom = new JSDOM(`<main><select data-engineer-acquisition-method>
    <option value="automatic">Automatic</option><option value="mv_step">Step</option><option value="relay">Feedback relay</option>
    <option value="project">Project test</option><option value="recorded-fit">Recorded-data fit</option>
  </select></main>`);
  const list = dom.window.document.querySelector("select") as HTMLSelectElement;
  const shown: string[] = [];
  const kept: string[] = [];
  const method = new ThreePageMethod(list, () => shown.push(list.value), () => kept.push(list.value));
  method.holds("automatic");
  /** The engineer chooses a method in the list and the page sends it, as the page's change handler does for a method the engine records. */
  const choose = (value: string): void => {
    list.value = value;
    if (RECORDED.has(value)) method.posted(value);
  };
  return { dom, list, method, shown, kept, choose };
}

suite("Control Studio Method list follows the engine", () => {
  test("shows the method the study holds again when the engine refuses the chosen one, and tells the page", () => {
    const { list, method, shown, kept, choose } = surface();
    choose("relay");
    method.answered(true, false);
    assert.strictEqual(list.value, "automatic", "Identify runs the method the study holds, so the list shows it");
    assert.deepStrictEqual(shown, ["automatic"], "the sections of the method shown again follow");
    assert.deepStrictEqual(kept, ["automatic"], "the page view the study keeps is the method shown");
  });

  test("keeps the method that the engine records", () => {
    const { list, method, shown, kept, choose } = surface();
    choose("relay");
    method.answered(false, false);
    assert.strictEqual(list.value, "relay");
    assert.deepStrictEqual([shown, kept], [[], []], "nothing is shown again, and nothing is kept again");
  });

  test("shows the earlier method of the study after a refusal, not the page's first one", () => {
    const { list, method, choose } = surface();
    choose("mv_step");
    method.answered(false, false);
    choose("relay");
    method.answered(true, false);
    assert.strictEqual(list.value, "mv_step", "the method the engine recorded last is the method the study holds");
  });

  test("leaves a refused limit to the Limits editor, whose Done sends the method again", () => {
    const { list, method, shown, choose } = surface();
    choose("mv_step");
    method.answered(true, true);
    assert.strictEqual(list.value, "mv_step");
    assert.deepStrictEqual(shown, []);
  });

  test("does not put back a method that the engineer has replaced by another choice meanwhile", () => {
    const { list, method, shown, choose } = surface();
    choose("relay");
    choose("mv_step");
    // The engine refuses the older choice first: the newer choice is still on its way, so the list stays on it.
    method.answered(true, false);
    assert.strictEqual(list.value, "mv_step");
    method.answered(false, false);
    assert.strictEqual(list.value, "mv_step", "the answer to the newer choice decides");
    assert.deepStrictEqual(shown, []);
  });

  test("ties each answer to the change it answers, so a refused change of another input moves no method", () => {
    const { list, method, shown, choose } = surface();
    choose("mv_step");
    method.answered(false, false);
    // A change that asked for no method (the requirements, say) is refused: the list shows the method the study holds already.
    method.posted();
    method.answered(true, false);
    assert.strictEqual(list.value, "mv_step");
    // A method that the engineer chose but the page has not sent is not put back by that refusal either.
    list.value = "relay";
    method.posted();
    method.answered(true, false);
    assert.strictEqual(list.value, "relay", "the refusal belongs to a change that asked for no method");
    assert.deepStrictEqual(shown, []);
  });

  test("leaves the page's own methods as they are: the engine records no method for them", () => {
    const { list, method, shown, choose } = surface();
    choose("relay");
    choose("project");
    method.answered(true, false);
    assert.strictEqual(list.value, "project", "the project test is the page's view; the offer does not say the study records it");
    assert.deepStrictEqual(shown, []);
  });

  test("puts nothing back for a method that the list no longer offers, and forgets the waiting changes when a loop opens", () => {
    const { list, method, choose } = surface();
    method.holds("prbs");
    choose("relay");
    method.answered(true, false);
    assert.strictEqual(list.value, "relay", "the list offers no prbs, so the choice stays");
    method.holds("automatic");
    choose("relay");
    method.reset();
    method.answered(true, false);
    assert.strictEqual(list.value, "relay", "a refusal of an earlier open's change moves nothing");
  });
});
