import * as assert from "assert";
import { isLoopRequest, requestedLoopIdentity } from "../../controlStudio/panelBindings";

/** An open request as the engine writes it for a configured loop; the loop's own fields are checked by the engine, not here. */
function openRequest(): Record<string, unknown> {
  return {
    schemaVersion: 1, source: "ConfiguredLoop",
    scope: { projectId: "project:tank", runtimeId: "simulator" },
    loopDefinition: { id: "tank-level", name: "Tank level", projectId: "project:tank", runtimeId: "simulator", revision: "sha256:1" },
  };
}

suite("Control Studio open request identity", () => {
  test("the panel reads the identity of the loop the request names, and nothing else of it", () => {
    assert.deepStrictEqual(requestedLoopIdentity(openRequest()), {
      id: "tank-level", name: "Tank level", projectId: "project:tank", runtimeId: "simulator", revision: "sha256:1",
    });
  });

  test("a request without a field of the identity names no loop to open", () => {
    const request = openRequest();
    (request.loopDefinition as Record<string, unknown>).revision = "";
    assert.strictEqual(requestedLoopIdentity(request), undefined);
  });

  test("only a configured loop is a loop request; the engine writes no other kind", () => {
    assert.strictEqual(isLoopRequest(openRequest()), true);
    assert.strictEqual(isLoopRequest({ ...openRequest(), source: "PidInstance" }), false);
    assert.strictEqual(isLoopRequest({ schemaVersion: 1, source: "ConfiguredLoop", scope: {} }), false, "a request without its loop is not a loop request");
  });
});
