"use strict";

function validReadPoint(point) {
  return (
    !point.invalid &&
    typeof point.symbol === "string" &&
    /^[A-Za-z_][A-Za-z0-9_]*$/.test(point.var || "") &&
    /^(?:BOOL|SINT|INT|DINT|LINT|USINT|UINT|UDINT|ULINT|REAL|LREAL|BYTE|WORD|DWORD|LWORD|STRING)$/.test(
      point.type || ""
    ) &&
    point.access === "read"
  );
}

function validConnectionRoute(route, expected) {
  return Boolean(
    route &&
      !route.invalid &&
      expected &&
      typeof route.name === "string" &&
      route.name.length > 0 &&
      typeof expected.targetNetId === "string" &&
      expected.targetNetId.length > 0 &&
      route.target_net_id === expected.targetNetId &&
      normalizeHost(expected.host).length > 0 &&
      normalizeHost(route.host) === normalizeHost(expected.host) &&
      expected.amsPort === 851 &&
      route.ams_port === expected.amsPort &&
      route.transport === "plain" &&
      route.insecure_transport === true
  );
}

function normalizeHost(value) {
  const trimmed = String(value || "").trim().toLowerCase();
  const unwrapped =
    trimmed.startsWith("[") && trimmed.endsWith("]")
      ? trimmed.slice(1, -1)
      : trimmed;
  return unwrapped.endsWith(".") ? unwrapped.slice(0, -1) : unwrapped;
}

module.exports = { validConnectionRoute, validReadPoint };
