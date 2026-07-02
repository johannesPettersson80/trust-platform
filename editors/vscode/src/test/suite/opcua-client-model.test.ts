import * as assert from "assert";

import {
  buildOpcuaConnection,
  classifyOpcuaBrowseError,
  deriveOpcuaConnectionName,
  deriveOpcuaVarName,
  nodeKey,
  opcuaAccess,
  opcuaDisplayType,
  opcuaPointFromNode,
  selectedLeaves,
} from "../../networkCanvas/webview/opcuaClientModel";
import type { SymbolNode } from "../../networkCanvas/offlineComm";

// Pure model for the OPC-UA CLIENT browse→select→save flow. Guards the B1/B2/B3 contract:
// raw node_id round-trips into apply, data_type is apply-ready, and structured browse errors map to
// exactly one recovery action (esp. the explicit cert-trust path).
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

  test("var name is deterministic and folds non-identifier chars", () => {
    assert.strictEqual(deriveOpcuaVarName({ name: "Temperature", path: "Temperature" }), "global.Temperature");
    assert.strictEqual(
      deriveOpcuaVarName({ name: "X", path: "Objects.Server.X" }),
      "global.Objects_Server_X"
    );
  });

  test("display type prefers resolved data_type, falls back to raw type", () => {
    assert.strictEqual(opcuaDisplayType({ data_type: "double", type: "i=11" }), "double");
    assert.strictEqual(opcuaDisplayType({ type: "i=11" }), "i=11");
    assert.strictEqual(opcuaDisplayType({}), "");
  });

  test("write access requires server-writable AND user opt-in", () => {
    assert.strictEqual(opcuaAccess(true, true), "read_write");
    assert.strictEqual(opcuaAccess(true, false), "read");
    assert.strictEqual(opcuaAccess(false, true), "read");
    assert.strictEqual(opcuaAccess(undefined, true), "read");
  });

  test("point round-trips the raw node_id and apply-ready type", () => {
    const p = opcuaPointFromNode(leaf({}), true);
    assert.ok(p);
    assert.strictEqual(p?.node_id, "ns=2;i=1"); // the REAL NodeId, not the sanitized id
    assert.strictEqual(p?.var, "global.Temperature");
    assert.strictEqual(p?.type, "double");
    assert.strictEqual(p?.access, "read_write");
  });

  test("a leaf with no raw node_id cannot be saved (guards pre-B1 sanitized-only ids)", () => {
    const p = opcuaPointFromNode(leaf({ node_id: undefined }), true);
    assert.strictEqual(p, undefined);
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
    assert.strictEqual(picked[0].node_id, "ns=3;i=1");
    const conn = buildOpcuaConnection({ endpoint_url: "opc.tcp://h:4840" }, "x", picked, false);
    assert.strictEqual(conn?.points.length, 1);
    assert.strictEqual(conn?.points[0].node_id, "ns=3;i=1"); // the exact node the user picked
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

  test("buildOpcuaConnection assembles connection + points from target", () => {
    const target = {
      endpoint_url: "opc.tcp://127.0.0.1:4840/trust-test",
      security_policy: "none",
      security_mode: "none",
      auth: "anonymous",
      trust_server_certificate: true,
    };
    const conn = buildOpcuaConnection(target, "FreeOpcUa Python Server", [leaf({})], false);
    assert.ok(conn);
    assert.strictEqual(conn?.endpoint_url, "opc.tcp://127.0.0.1:4840/trust-test");
    assert.strictEqual(conn?.trust_server_certificate, true);
    assert.strictEqual(conn?.points.length, 1);
    assert.strictEqual(conn?.points[0].access, "read"); // allowWrites=false
    assert.strictEqual(conn?.name, "freeopcua-python-server");
  });

  test("buildOpcuaConnection returns undefined with no usable points or endpoint", () => {
    const target = { endpoint_url: "opc.tcp://h:4840" };
    assert.strictEqual(buildOpcuaConnection(target, "x", [leaf({ node_id: undefined })], true), undefined);
    assert.strictEqual(buildOpcuaConnection({}, "x", [leaf({})], true), undefined);
  });

  test("username auth carries credentials; anonymous does not", () => {
    const base = { endpoint_url: "opc.tcp://h:4840", security_policy: "none", security_mode: "none" };
    const anon = buildOpcuaConnection({ ...base, auth: "anonymous", username: "u" }, "x", [leaf({})], false);
    assert.strictEqual(anon?.username, undefined);
    const user = buildOpcuaConnection({ ...base, auth: "username", username: "u", password: "p" }, "x", [leaf({})], false);
    assert.strictEqual(user?.username, "u");
    assert.strictEqual(user?.password, "p");
  });

  test("connection name slugs from endpoint when label is the raw protocol id", () => {
    assert.strictEqual(
      deriveOpcuaConnectionName("opcua_client", "opc.tcp://10.0.0.5:4840/Server"),
      "10-0-0-5-4840-server"
    );
  });

  test("browse errors map to exactly one recovery action", () => {
    assert.strictEqual(classifyOpcuaBrowseError({ code: "cert_untrusted", message: "m" }).action, "trust");
    assert.strictEqual(classifyOpcuaBrowseError({ code: "auth_required", message: "m" }).action, "credentials");
    assert.strictEqual(classifyOpcuaBrowseError({ code: "unsupported_security_profile" }).action, "security");
    assert.strictEqual(classifyOpcuaBrowseError({ code: "endpoint_unreachable" }).action, "retry");
    assert.strictEqual(classifyOpcuaBrowseError({ code: "browse_denied" }).action, "none");
    assert.strictEqual(classifyOpcuaBrowseError({ code: "weird" }).action, "none");
  });

  test("browse error details are user-facing recovery text, not raw status tokens", () => {
    const auth = classifyOpcuaBrowseError({
      code: "auth_required",
      message: "OPC UA node browse failed: control error 'OPC UA status: BadSecurityPolicyRejected'",
    });
    assert.match(auth.detail, /username authentication/i);
    assert.doesNotMatch(auth.detail, /BadSecurityPolicyRejected/);

    const cert = classifyOpcuaBrowseError({
      code: "cert_untrusted",
      message: "OPC UA node browse failed: control error 'BadCertificateUntrusted'",
    });
    assert.match(cert.detail, /Trust certificate/i);
    assert.doesNotMatch(cert.detail, /BadCertificateUntrusted/);
  });
});
