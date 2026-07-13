"use strict";

const { readTomlAssignment } = require("./PackagedTomlAssignment");
const { validConnectionRoute, validReadPoint } = require("./PackagedAdsRouteProof");

const CONNECTION_KEYS = new Set([
  "name",
  "target_net_id",
  "host",
  "ams_port",
  "transport",
  "insecure_transport",
]);
const POINT_KEYS = new Set(["symbol", "var", "type", "access"]);

function readSelectedPointProof(source, remoteSymbol, expectedRoute) {
  const matches = parsePointMappings(source).filter(
    (point) => point.symbol === remoteSymbol
  );
  const pointExact = matches.length === 1 && validReadPoint(matches[0]);
  const routeExact =
    pointExact && validConnectionRoute(matches[0].route, expectedRoute);
  return {
    pointExact,
    routeExact,
    mapping: routeExact
      ? { ...matches[0], connection: matches[0].route.name }
      : undefined,
  };
}

function parsePointMappings(source) {
  const connections = [];
  let connection;
  let point;
  for (const line of source.replace(/\r\n?/g, "\n").split("\n")) {
    if (/^\s*\[\[connections\]\]\s*(?:#.*)?$/.test(line)) {
      connection = record({ points: [] });
      connections.push(connection);
      point = undefined;
      continue;
    }
    if (/^\s*\[\[connections\.points\]\]\s*(?:#.*)?$/.test(line)) {
      point = connection ? record({ route: connection }) : undefined;
      if (point) connection.points.push(point);
      continue;
    }
    if (/^\s*\[/.test(line)) {
      connection = undefined;
      point = undefined;
      continue;
    }
    const assignment = readTomlAssignment(line);
    if (!assignment) continue;
    if (point && POINT_KEYS.has(assignment.key)) {
      assign(point, assignment);
    } else if (connection && !point && CONNECTION_KEYS.has(assignment.key)) {
      assign(connection, assignment);
    }
  }
  return connections.flatMap((candidate) => candidate.points);
}

function record(initial) {
  return { ...initial, invalid: false, seen: new Set() };
}

function assign(target, assignment) {
  if (target.seen.has(assignment.key)) {
    target.invalid = true;
    return;
  }
  target.seen.add(assignment.key);
  target[assignment.key] = assignment.value;
}

module.exports = { readSelectedPointProof };
