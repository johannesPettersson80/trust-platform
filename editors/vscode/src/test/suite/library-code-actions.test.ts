import * as assert from "assert";

import { curatedLibraryActionForSymbol } from "../../libraryCodeActions";

suite("Library code actions", () => {
  test("offers OSCAT for known OSCAT symbols when the dependency is missing", () => {
    const action = curatedLibraryActionForSymbol("INC", []);
    assert.deepStrictEqual(action, {
      id: "oscat",
      label: "OSCAT",
      dependencyName: "OSCAT",
    });
  });

  test("does not offer OSCAT when the project already depends on it", () => {
    assert.strictEqual(curatedLibraryActionForSymbol("INC", ["OSCAT"]), undefined);
  });

  test("offers PLCopen Motion for MC symbols and ignores unknown symbols", () => {
    assert.strictEqual(
      curatedLibraryActionForSymbol("MC_MoveAbsolute", [])?.id,
      "plcopen_motion"
    );
    assert.strictEqual(curatedLibraryActionForSymbol("NotALibrarySymbol", []), undefined);
  });
});
