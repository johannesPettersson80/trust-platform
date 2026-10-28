import * as assert from "assert";

import {
  nodeKey,
  selectedLeaves,
} from "../../networkCanvas/webview/opcuaClientModel";
import type { SymbolNode } from "../../networkCanvas/offlineComm";

// The OPC UA client browse pane: selection keyed by the raw node_id, and structured browse errors
// mapped to exactly one recovery action (esp. the explicit cert-trust path). The connection the
// selected nodes are saved as (variable names, apply-ready types, write access, credentials, the
// connection name) is the language server's `opcua_add`: the Rust tests `opcua_*` and
// `opcua_connection_from_selected_nodes` in crates/trust-devices/src/actions/tests/plans.rs.
suite("opcua client model", () => {
  const leaf = (over: Partial<SymbolNode>): SymbolNode => ({
    id: "opcua:node:ns_2_i_1",
    node_id: "ns=2;i=1",
    name: "Temperature",
    path: "Temperature",
    type: "i=11",
    data_type: "double",
    writable: true,
    ...over,
  });

  test("nodeKey prefers the raw node_id, then the React id, then the path", () => {
    assert.strictEqual(nodeKey({ node_id: "ns=2;i=1", id: "x", path: "p" }), "ns=2;i=1");
    assert.strictEqual(nodeKey({ id: "x", path: "p" }), "x");
    assert.strictEqual(nodeKey({ path: "p" }), "p");
  });

  test("two leaves sharing a path but different node_id are NOT conflated (B1 integrity)", () => {
    const a = leaf({ id: "opcua:node:ns_2_i_1", node_id: "ns=2;i=1", path: "Temperature" });
    const b = leaf({ id: "opcua:node:ns_3_i_1", node_id: "ns=3;i=1", path: "Temperature" });
    // Selecting ONLY b's key must pick b alone — not both, and not a.
    const picked = selectedLeaves([a, b], new Set([nodeKey(b)]));
    assert.strictEqual(picked.length, 1);
    assert.strictEqual(picked[0].node_id, "ns=3;i=1"); // the exact node the user picked
  });

  test("leaves whose sanitized id collides stay distinct by nodeKey (React-key safety)", () => {
    // Two different NodeIds that sanitize to the SAME id (=/;/. → _) — the residual key-collision class.
    const a = leaf({ id: "opcua:node:ns_2_s_Tag", node_id: "ns=2;s=Tag" });
    const b = leaf({ id: "opcua:node:ns_2_s_Tag", node_id: "ns=2;s=Ta;g" });
    assert.notStrictEqual(nodeKey(a), nodeKey(b)); // distinct → no duplicate React key / wrong-row reuse
    const picked = selectedLeaves([a, b], new Set([nodeKey(b)]));
    assert.strictEqual(picked.length, 1);
    assert.strictEqual(picked[0].node_id, "ns=2;s=Ta;g");
  });

  test("selectedLeaves returns only chosen leaves in tree order", () => {
    const tree: SymbolNode[] = [
      { id: "a", name: "A", path: "A", children: [leaf({ id: "t", name: "Temperature", path: "A.Temperature" }), leaf({ id: "c", name: "Counter", path: "A.Counter", node_id: "ns=2;i=2", writable: false })] },
    ];
    const picked = selectedLeaves(tree, new Set(["ns=2;i=2"])); // Counter's node_id key
    assert.strictEqual(picked.length, 1);
    assert.strictEqual(picked[0].name, "Counter");
  });

  // A structured browse error's one recovery action and its user-facing texts are the server's: the
  // Rust test `a_failed_browse_names_its_one_recovery` (crates/trust-devices/src/canvas/local_tests.rs)
  // and the panel's browse projection tests (crates/trust-devices/src/browse/tests/projection.rs).
});
