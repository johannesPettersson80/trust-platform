import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import {
  buildNetworkCanvasModel,
  type BuildNetworkCanvasModelInput,
} from "../../networkCanvas/model";
import {
  mergeFleetTopologies,
  offlineTopologyForTarget,
  type FleetTopologyResponse,
} from "../../networkCanvas/fleetTopology";
import { ensureAdsRuntimeEnabled } from "../../networkCanvas/offlineComm";
import { mergeConnectorStatusIntoTopology } from "../../networkCanvas/connectorsStatus";
import { buildCanvasGraph } from "../../networkCanvas/graphData";
import { buildGraph } from "../../networkCanvas/webview/layout";
import type { EndpointNodeData } from "../../networkCanvas/webview/types";
import type { RuntimeTarget } from "../../runtimeTarget";
import { runtimeNodeControls } from "../../networkCanvas/webview/runtimeNodeControls";
import { ADD_PICKER_GROUPS, groupForAddPicker } from "../../networkCanvas/webview/grouping";
import { applyFilter, filterReport } from "../../networkCanvas/webview/filter";
import { buildExposeApplyParams } from "../../networkCanvas/exposeConfig";
import { commTestMessage } from "../../communication/runtimeComm";
import {
  connectorConnectionLabel,
  connectorHealthLabel,
  connectorSignalsSummary,
  discoveryConfidenceLabel,
  discoverySourceLabel,
} from "../../networkCanvas/webview/connectorPresentation";
import {
  validateSchemaValues,
  visibleSchemaFields,
  type CommProtocolSchema,
} from "../../communication/schemaForm";
import { protocolName } from "../../networkCanvas/webview/protocolMeta";
import {
  formatExposedGlobals,
  serverEndpointSummaryRows,
} from "../../networkCanvas/webview/serverEndpointSummary";
import { visibleFaultsForValidationState } from "../../networkCanvas/webview/faults";

const RUNNING = {
  running: true,
  runtimeState: "running" as const,
  runtimeMode: "simulate" as const,
};

function fleetTopology(): FleetTopologyResponse {
  return {
    schema_version: 1,
    hosts: [
      {
        host_id: "host:trust-pi",
        hostname: "trust-pi",
        arch: "aarch64",
        os: "linux",
        ips: ["192.0.2.10"],
        containers: [],
        runtimes: [
          {
            runtime_id: "runtime-a",
            name: "Line runtime",
            web_listen: "0.0.0.0:8080",
            mode: "simulate",
            cycle_ms: 10,
            health: "connected",
            detail: "Runtime answered fleet.topology.",
            endpoints: [
              {
                id: "endpoint:runtime-a:modbus_tcp",
                kind: "field",
                protocol: "modbus_tcp",
                name: "Modbus meter",
                role: "owned_driver",
                health: "connected",
                detail: "Driver is healthy.",
                owned: true,
                supports_test: true,
              },
              {
                id: "endpoint:runtime-a:mqtt",
                kind: "field",
                protocol: "mqtt",
                name: "MQTT broker",
                role: "owned_driver",
                health: "degraded",
                detail: "Broker connection refused.",
                owned: true,
                supports_test: true,
              },
            ],
          },
        ],
      },
    ],
    links: [
      {
        id: "link:mqtt:broker",
        from: "endpoint:runtime-a:mqtt",
        to: "shared:mqtt:broker",
        protocol: "mqtt",
        role: "publish_subscribe",
        direction: "publish_subscribe",
        same_host: false,
        status: "configured_policy",
        secure: false,
        detail: "MQTT broker referenced by io.toml",
      },
      {
        from: "endpoint:runtime-a:mqtt",
        to: "external:mesh:0",
        protocol: "mesh",
        direction: "outbound",
        same_host: false,
        status: "degraded",
        secure: true,
        detail: "tcp/192.168.50.42:7447",
      },
    ],
    shared: [
      {
        id: "shared:mqtt:broker",
        kind: "broker",
        name: "MQTT broker",
        address: "127.0.0.1:1883",
        used_by: ["runtime-a"],
      },
    ],
    external: [
      {
        id: "external:mesh:0",
        kind: "peer",
        name: "tcp/192.168.50.42:7447",
        via_protocol: ["mesh"],
        direction: "outbound",
      },
    ],
  };
}


export {
  assert,
  fs,
  os,
  path,
  buildNetworkCanvasModel,
  mergeFleetTopologies,
  offlineTopologyForTarget,
  ensureAdsRuntimeEnabled,
  mergeConnectorStatusIntoTopology,
  buildCanvasGraph,
  buildGraph,
  runtimeNodeControls,
  ADD_PICKER_GROUPS,
  groupForAddPicker,
  applyFilter,
  filterReport,
  buildExposeApplyParams,
  commTestMessage,
  connectorConnectionLabel,
  connectorHealthLabel,
  connectorSignalsSummary,
  discoveryConfidenceLabel,
  discoverySourceLabel,
  validateSchemaValues,
  visibleSchemaFields,
  protocolName,
  formatExposedGlobals,
  serverEndpointSummaryRows,
  visibleFaultsForValidationState,
  RUNNING,
  fleetTopology,
};
export type {
  BuildNetworkCanvasModelInput,
  FleetTopologyResponse,
  EndpointNodeData,
  RuntimeTarget,
  CommProtocolSchema,
};
