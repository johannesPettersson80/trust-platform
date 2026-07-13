import React, { useEffect, useMemo, useState } from "react";
import { healthColor } from "./nodes";
import { protocolColor, protocolName } from "./protocolMeta";
import { t, tint } from "./theme";
import { buildParams, Field, valuesFor } from "./SchemaFields";
import { browseAction } from "./browseActions";
import {
    runtimeNodeControlsForNode,
    type RuntimeNodeControl,
} from "./runtimeNodeControls";
import { iconBtn, NodeSummaryView, PANEL_STYLE, str } from "./NodeSummaryView";
import { LOCAL_RUNTIME_NODE_ID } from "./types";
import type { LifecyclePhase } from "../../lifecycleEntryFailure";
import type {
    CommApplyResponse,
    CommProtocolSchema,
    CommSchemaResponse,
} from "../../communication/schemaForm";
import { visibleSchemaFields } from "../../communication/schemaForm";

// §4 D + settings UX (2026-06-17/18): single-click a node → ONE side panel. It opens on a
// read-only SUMMARY (what it is + its current settings); an Edit button switches to the
// schema-driven form. Editing is file-based, so it works whether the device is stopped,
// offline, or running (Save writes config; "Apply live" pushes to a running runtime only when
// one is online). Non-configurable nodes (host/container/external) are summary-only.

export interface InspectorNode {
    id: string;
    type?: string;
    data: Record<string, unknown>;
}

interface Props {
    node: InspectorNode;
    schema?: CommSchemaResponse;
    params?: Record<string, unknown>; // the endpoint's current config values
    reachable: boolean; // a runtime is online → "Apply live" is possible
    applyResult?: CommApplyResponse;
    lifecyclePhase: LifecyclePhase;
    operationInProgress: boolean;
    onClose: () => void;
    onFocus: (nodeId: string) => void;
    onBrowse?: (node: InspectorNode) => void; // §0.5.2 browse what the endpoint exposes (tags/channels/globals)
    post: (message: unknown) => void;
}

function testParamsFor(
    protocol: string,
    params?: Record<string, unknown>,
): Record<string, unknown> {
    if (!params) {
        return {};
    }
    const connections = params.connections;
    if (
        protocol === "opcua_client" &&
        Array.isArray(connections) &&
        connections.length > 0
    ) {
        const first = connections[0];
        if (first && typeof first === "object" && !Array.isArray(first)) {
            return first as Record<string, unknown>;
        }
    }
    return params;
}

function isRuntimeAuthTokenFailure(node: InspectorNode): boolean {
    if (node.type !== "runtime") {
        return false;
    }
    const endpoint = str(node.data.controlEndpoint);
    if (
        !endpoint ||
        node.id === LOCAL_RUNTIME_NODE_ID ||
        node.data.managed === true
    ) {
        return false;
    }
    const health = str(node.data.health).toLowerCase();
    const detail = str(node.data.detail).toLowerCase();
    return (
        health === "auth_failed" ||
        (detail.includes("authentication failed") &&
            detail.includes("auth token")) ||
        detail.includes("no auth token provided") ||
        detail.includes("auth token rejected")
    );
}

function editBreadcrumb(protocol: string): string {
    return `Edit ${protocolName(protocol)}`;
}

export function NodeInspector({
    node,
    schema,
    params,
    reachable,
    applyResult,
    lifecyclePhase,
    operationInProgress,
    onClose,
    onFocus,
    onBrowse,
    post,
}: Props) {
    const protocol = str(node.data.protocol);
    const protoSchema = useMemo(
        () =>
            node.type === "endpoint"
                ? schema?.protocols.find((p) => p.id === protocol)
                : undefined,
        [schema, protocol, node.type],
    );
    // §0.5.2: the browse/expose button is now schema-driven — show it only when the backend advertises
    // the capability (comm.schema actions ∋ "browse_symbols") AND the UI has a presentation for it.
    const browse =
        node.type === "endpoint" &&
        protoSchema?.actions.includes("browse_symbols")
            ? browseAction(protocol)
            : undefined;
    const [editing, setEditing] = useState(false);
    // Every node opens on its summary; reset when a different node is selected.
    useEffect(() => setEditing(false), [node.id]);

    // §8 P3b: a runtime node is where per-runtime lifecycle lives. Honest verbs — Start/Stop for the
    // local simulator we own, Connect/Disconnect for a remote we don't (never a fake remote "Stop").
    const isManaged = node.data.managed === true;
    const managedName = isManaged ? str(node.data.managedName) : "";
    const runtimeControls =
        node.type === "runtime"
            ? runtimeNodeControlsForNode({
                  nodeId: node.id,
                  isLocal: false,
                  health: str(node.data.health),
                  attached: node.data.attached === true,
                  controlEndpoint: node.data.controlEndpoint
                      ? str(node.data.controlEndpoint)
                      : undefined,
                  managed: isManaged,
                  authTokenRequired: isRuntimeAuthTokenFailure(node),
                  // Local sim + managed runtimes have logs; remote logs are phase 14 (gated).
                  logsAvailable: node.id === LOCAL_RUNTIME_NODE_ID || isManaged,
                  lifecyclePhase,
                  operationInProgress,
              })
            : undefined;
    const onControl = (control: RuntimeNodeControl) => {
        switch (control.action) {
            case "runtimeConnect":
                post({
                    type: "runtimeConnect",
                    endpoint: str(node.data.controlEndpoint),
                    label: str(node.data.label),
                });
                return;
            case "runtimeDisconnect":
                post({
                    type: "runtimeDisconnect",
                    endpoint: str(node.data.controlEndpoint),
                });
                return;
            case "setAuthToken":
                post({
                    type: "setRuntimeAuthToken",
                    endpoint: str(node.data.controlEndpoint),
                });
                return;
            case "managedStart":
                post({
                    type: "runtimeManagedStart",
                    name: managedName,
                    endpoint: str(node.data.controlEndpoint),
                });
                return;
            case "managedStop":
                post({
                    type: "runtimeManagedStop",
                    name: managedName,
                    endpoint: str(node.data.controlEndpoint),
                });
                return;
            case "setAsRunTarget":
                post({
                    type: "setAsRunTarget",
                    endpoint: str(node.data.controlEndpoint),
                    isLocal: node.id === LOCAL_RUNTIME_NODE_ID,
                    managedName,
                });
                return;
            case "openRuntimeLogs":
                // Managed runtimes have their own logs (fleet runtime logs); the sim uses the debug channel.
                if (managedName) {
                    post({ type: "runtimeManagedLogs", name: managedName });
                } else {
                    post({ type: "action", action: control.action });
                }
                return;
            case "none":
                return;
            default:
                // Local lifecycle + settings reuse the existing canvas action channel.
                post({ type: "action", action: control.action });
        }
    };

    if (editing && protoSchema) {
        return (
            <EditableEndpoint
                node={node}
                protoSchema={protoSchema}
                params={params}
                reachable={reachable}
                applyResult={applyResult}
                onBack={() => setEditing(false)}
                onClose={onClose}
                post={post}
            />
        );
    }

    return (
        <NodeSummaryView
            node={node}
            protoSchema={protoSchema}
            params={params}
            applyResult={applyResult}
            onEdit={protoSchema ? () => setEditing(true) : undefined}
            onTest={
                node.type === "endpoint" &&
                protoSchema?.supports_test &&
                reachable
                    ? () =>
                          post({
                              type: "commTest",
                              protocol,
                              params: testParamsFor(protocol, params),
                              target: node.id,
                          })
                    : undefined
            }
            onBrowse={onBrowse && browse ? () => onBrowse(node) : undefined}
            browseLabel={browse?.label}
            runtimeControls={runtimeControls}
            onControl={onControl}
            onClose={onClose}
            onFocus={onFocus}
        />
    );
}

// ---- editable form (after pressing Edit) ----
function EditableEndpoint({
    node,
    protoSchema,
    params,
    reachable,
    applyResult,
    onBack,
    onClose,
    post,
}: {
    node: InspectorNode;
    protoSchema: CommProtocolSchema;
    params?: Record<string, unknown>;
    reachable: boolean;
    applyResult?: CommApplyResponse;
    onBack: () => void;
    onClose: () => void;
    post: (message: unknown) => void;
}) {
    const protocol = str(node.data.protocol);
    const health = str(node.data.health);
    const [values, setValues] = useState<Record<string, string>>(() =>
        valuesFor(protoSchema, params),
    );
    const [confirmRemove, setConfirmRemove] = useState(false);
    const paramsKey = JSON.stringify(params ?? {});
    const schemaKey = `${protoSchema.id}:${protoSchema.fields.map((field) => field.id).join("|")}`;
    useEffect(() => {
        setValues(valuesFor(protoSchema, params));
    }, [node.id, schemaKey, paramsKey]);
    useEffect(() => {
        setConfirmRemove(false);
    }, [node.id]);

    const fieldErrors = new Map(
        (applyResult?.field_errors ?? []).map((e) => [e.field, e.message]),
    );
    const visibleFields = visibleSchemaFields(protoSchema, values);
    const isDisabled = health === "disabled";
    const canDisable = protoSchema.actions.includes("disable") && !isDisabled;
    const send = (type: string, extra?: Record<string, unknown>) => {
        if (type !== "commRemove") {
            setConfirmRemove(false);
        }
        post({
            type,
            protocol,
            params: buildParams(protoSchema, values),
            target: node.id,
            ...extra,
        });
    };
    const requestRemove = () => {
        if (!confirmRemove) {
            setConfirmRemove(true);
            return;
        }
        send("commRemove");
    };

    const ok =
        applyResult &&
        (applyResult.applied || applyResult.lifecycle_effect === "test_ok");
    const blocked = applyResult && applyResult.lifecycle_effect === "blocked";

    return (
        <aside
            className="trust-inspector"
            style={PANEL_STYLE}
            aria-label="Node settings"
        >
            <header className="trust-inspector__header">
                <button
                    onClick={onBack}
                    aria-label="Back"
                    title="Back to summary"
                    style={iconBtn}
                >
                    ‹
                </button>
                <span
                    style={{
                        flex: "none",
                        width: 10,
                        height: 10,
                        borderRadius: 3,
                        background: protocolColor(protocol),
                    }}
                />
                <div style={{ flex: 1, minWidth: 0 }}>
                    <div className="trust-inspector__eyebrow">
                        Devices & Connections / {editBreadcrumb(protocol)}
                    </div>
                    <div className="trust-inspector__title">
                        {protocolName(protocol)}
                    </div>
                </div>
                {health && (
                    <span
                        title={health}
                        style={{
                            flex: "none",
                            width: 10,
                            height: 10,
                            borderRadius: "50%",
                            background: healthColor(health),
                            boxShadow: `0 0 0 2px ${tint(healthColor(health), 0.18)}`,
                        }}
                    />
                )}
                <button onClick={onClose} aria-label="Close" style={iconBtn}>
                    ✕
                </button>
            </header>

            <div className="trust-section trust-section--grow">
                {protoSchema.purpose && (
                    <p className="trust-help" style={{ marginBottom: 14 }}>
                        {protoSchema.purpose}
                    </p>
                )}
                {visibleFields.map((field) => (
                    <Field
                        key={field.id}
                        field={field}
                        value={values[field.id] ?? ""}
                        error={fieldErrors.get(field.id)}
                        onChange={(v) =>
                            setValues((prev) => ({ ...prev, [field.id]: v }))
                        }
                    />
                ))}
            </div>

            {/* Pinned between the scroll body and the footer so the save/validation result and
          the disabled note are never hidden behind the footer buttons. */}
            {applyResult && (applyResult.message || ok || blocked) && (
                <div
                    className={`trust-message ${ok ? "trust-message--ok" : blocked ? "trust-message--error" : ""}`}
                    style={{ margin: "0 14px 10px" }}
                >
                    {applyResult.message || (ok ? "Saved." : "")}
                </div>
            )}
            {isDisabled && !applyResult && (
                <div
                    className="trust-message"
                    style={{ margin: "0 14px 10px" }}
                >
                    This endpoint is disabled. Use Enable to turn it back on;
                    restart the runtime to apply the change.
                </div>
            )}

            <footer
                className="trust-section"
                style={{
                    display: "flex",
                    flexWrap: "wrap",
                    gap: 8,
                    borderBottom: "none",
                }}
            >
                {confirmRemove ? (
                    <>
                        <div
                            className="trust-message trust-message--error endpoint-remove-confirmation"
                            style={{ flexBasis: "100%", margin: 0 }}
                        >
                            Remove this endpoint from the project? This writes
                            the config file and takes effect after restart.
                        </div>
                        <button
                            onClick={() => setConfirmRemove(false)}
                            className="trust-button"
                            style={{ flex: 1 }}
                        >
                            Cancel
                        </button>
                        <button
                            onClick={requestRemove}
                            className="trust-button trust-button--danger"
                            style={{ flex: 1 }}
                        >
                            Confirm remove
                        </button>
                    </>
                ) : (
                    <>
                        <button
                            onClick={() =>
                                send("commSave", { action: "upsert" })
                            }
                            className="trust-button trust-button--primary"
                            style={{ flex: 1 }}
                        >
                            {isDisabled ? "Enable" : "Save"}
                        </button>
                        <button onClick={onClose} className="trust-button">
                            Cancel
                        </button>
                        {protoSchema.supports_test && reachable && (
                            <button
                                onClick={() => send("commTest")}
                                className="trust-button"
                            >
                                Test
                            </button>
                        )}
                        {canDisable && (
                            <button
                                onClick={() =>
                                    send("commDisable", { action: "disable" })
                                }
                                className="trust-button"
                            >
                                Disable
                            </button>
                        )}
                        <button
                            onClick={requestRemove}
                            className="trust-button trust-button--danger"
                        >
                            Remove
                        </button>
                        {reachable && (
                            <button
                                onClick={() =>
                                    send("commApplyLive", { action: "upsert" })
                                }
                                title="Push this config to the running runtime now"
                                className="trust-button"
                                style={{ flexBasis: "100%" }}
                            >
                                Apply to running runtime
                            </button>
                        )}
                    </>
                )}
            </footer>
        </aside>
    );
}
