"use strict";

const REQUIRED_ADS_PORTS = Object.freeze([851, 852, 853, 854, 301, 501]);
const REQUIRED_SET = new Set(REQUIRED_ADS_PORTS);

function parseExpectedCustomAdsPorts(raw, required) {
  const value = String(raw || "").trim();
  if (!required) return [];
  if (!value) {
    throw new Error("Missing required environment value TRUST_PACKAGED_ADS_EXPECTED_CUSTOM_PORTS.");
  }
  const tokens = value.split(",").map((token) => token.trim());
  if (tokens.length < 1 || tokens.length > 4 || tokens.some((token) => !/^\d+$/.test(token))) {
    throw new Error("Expected custom ADS ports must contain 1-4 comma-separated decimal ports.");
  }
  const ports = tokens.map(Number);
  if (ports.some((port) => port < 1 || port > 65_535)) {
    throw new Error("Expected custom ADS ports must be inside 1-65535.");
  }
  if (new Set(ports).size !== ports.length) {
    throw new Error("Expected custom ADS ports must be unique.");
  }
  if (ports.some((port) => REQUIRED_SET.has(port))) {
    throw new Error("Expected custom ADS ports must not duplicate built-in ADS service ports.");
  }
  return ports;
}

module.exports = { parseExpectedCustomAdsPorts, REQUIRED_ADS_PORTS };
