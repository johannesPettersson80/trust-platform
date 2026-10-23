import React, { useEffect, useMemo, useState } from "react";

import {
  adsPortBrowseEvidence,
  respondingAdsPorts,
} from "../adsDiscoveryPorts";
import {
  adsTagBatchSummary,
  normalizeAdsTagSelections,
  type AdsTagBatchImportResult,
  type AdsTagPortPath,
  type AdsTagPortSelection,
} from "../adsTagBatch";
import type { RoutePlan, SymbolNode } from "../offlineComm";
import type { BrowseErrorView } from "./browseErrorModel";
import { classifyBrowseError } from "./browseErrorModel";
import {
  adsTargetNetId,
  adsTargetPort,
  parseAdsPortInput,
  withAdsTargetPort,
} from "./adsTargetPort";
import { nodeKey } from "./opcuaClientModel";
import { SymbolSelectionCheckbox } from "./SymbolSelectionCheckbox";
import { t, tint } from "./theme";

interface AdsPortView {
  readonly port: number;
  readonly tree?: readonly SymbolNode[];
  readonly loading: boolean;
  readonly routeMissing: boolean;
  readonly routePlan?: RoutePlan;
  readonly error?: BrowseErrorView;
}

export function AdsMultiPortTagBrowser({
  targetLabel,
  target,
  tree,
  routeMissing,
  routePlan,
  error,
  loading,
  importLoading,
  importResult,
  onCreateRoute,
  onCopy,
  onBrowseTarget,
  onAddTags,
  onRemoveTag,
  onClose,
}: {
  targetLabel: string;
  target: Record<string, unknown>;
  tree?: SymbolNode[];
  routeMissing: boolean;
  routePlan?: RoutePlan;
  error?: BrowseErrorView;
  loading: boolean;
  importLoading: boolean;
  importResult?: AdsTagBatchImportResult;
  onCreateRoute: (port?: number) => void;
  onCopy: (text: string) => void;
  onBrowseTarget: (target: Record<string, unknown>) => void;
  onAddTags: (
    selections: readonly AdsTagPortSelection[],
    writable: boolean,
    changedPath: string,
  ) => void;
  onRemoveTag: (selection: AdsTagPortPath) => void;
  onClose: () => void;
}) {
  const incomingViews = useMemo(
    () => adsPortViews(target, tree, routeMissing, routePlan, error, loading),
    [error, loading, routeMissing, routePlan, target, tree],
  );
  const [views, setViews] = useState<readonly AdsPortView[]>(incomingViews);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ReadonlySet<string>>(
    () => selectedImportedTagKeys(incomingViews, importedTagKeys(target)),
  );
  const [expandedPorts, setExpandedPorts] = useState<ReadonlySet<number>>(
    () => new Set(incomingViews.map((view) => view.port)),
  );
  const [expandedNodes, setExpandedNodes] = useState<ReadonlySet<string>>(new Set());
  const [allowWrites, setAllowWrites] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [portDraft, setPortDraft] = useState("");
  const [added, setAdded] = useState<ReadonlySet<string>>(
    () => importedTagKeys(target),
  );

  useEffect(() => {
    setViews((previous) => mergePortViews(previous, incomingViews));
  }, [incomingViews]);

  useEffect(() => {
    const imported = selectedImportedTagKeys(views, importedTagKeys(target));
    setSelected((previous) => {
      const missing = [...imported].filter((key) => !previous.has(key));
      return missing.length > 0
        ? new Set([...previous, ...missing])
        : previous;
    });
  }, [target, views]);

  useEffect(() => {
    setExpandedPorts((previous) => {
      const next = new Set(previous);
      for (const view of views) {
        next.add(view.port);
      }
      return next;
    });
  }, [views]);

  useEffect(() => {
    if (!importResult) {
      return;
    }
    const operation = importResult.operation ?? "add";
    setAdded((previous) => {
      const next = new Set(previous);
      for (const port of importResult.ports) {
        if (!port.applied) {
          continue;
        }
        for (const path of port.paths) {
          const key = importedPathKey(port.port, path);
          operation === "remove" ? next.delete(key) : next.add(key);
        }
      }
      return next;
    });
    setSelected((previous) => {
      const next = new Set(previous);
      for (const port of importResult.ports) {
        const checked = operation === "remove" ? !port.applied : port.applied;
        for (const key of selectionKeysForPaths(views, port.port, port.paths)) {
          checked ? next.add(key) : next.delete(key);
        }
      }
      return next;
    });
  }, [importResult, views]);

  const parsedAdvancedPort = parseAdsPortInput(portDraft);
  const responding = respondingAdsPorts(target);
  const portSummary = responding.length > 0
    ? `${responding.length} responding ADS ${responding.length === 1 ? "port" : "ports"}: ${responding.join(", ")}`
    : `${views.length} ADS ${views.length === 1 ? "port" : "ports"}`;

  const togglePort = (port: number) => {
    setExpandedPorts((previous) => {
      const next = new Set(previous);
      next.has(port) ? next.delete(port) : next.add(port);
      return next;
    });
  };
  const toggleNode = (key: string) => {
    setExpandedNodes((previous) => {
      const next = new Set(previous);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });
  };
  const setChecked = (
    port: number,
    node: SymbolNode,
    key: string,
    checked: boolean,
  ) => {
    const next = new Set(selected);
    checked ? next.add(key) : next.delete(key);
    setSelected(next);
    if (checked) {
      const view = views.find((candidate) => candidate.port === port);
      const paths = leafNodes(view?.tree ?? [])
        .filter((candidate) => next.has(selectionKey(port, candidate)))
        .map((candidate) => candidate.path);
      onAddTags([{ port, paths }], allowWrites, node.path);
    } else {
      onRemoveTag({ port, path: node.path });
    }
  };

  const renderLeaf = (port: number, node: SymbolNode, depth: number) => {
    const key = selectionKey(port, node);
    const wasAdded = selected.has(key) && added.has(importedPathKey(port, node.path));
    const access = node.writable === true
      ? "read/write"
      : node.writable === false
        ? "read-only"
        : "";
    return (
      <div key={key} style={{ ...ROW, paddingLeft: 12 + depth * 14 }}>
        <SymbolSelectionCheckbox
          checked={selected.has(key)}
          disabled={importLoading}
          label={`Select ${node.path}`}
          onCheckedChange={(checked) => setChecked(port, node, key, checked)}
        />
        <span title={node.path} style={TAG_NAME}>{node.name}</span>
        {(node.data_type || node.type) && <span style={TAG_META}>{node.data_type || node.type}</span>}
        {access && <span style={TAG_META}>{access}</span>}
        {wasAdded && (
          <span
            data-role="added-symbol-status"
            aria-label={`Already added: ${node.path}`}
            style={ADDED_BADGE}
          >
            Added
          </span>
        )}
      </div>
    );
  };

  const renderNode = (port: number, node: SymbolNode, depth: number): React.ReactNode => {
    if (!node.children?.length) {
      return renderLeaf(port, node, depth);
    }
    const key = `${port}:${nodeKey(node)}`;
    const open = expandedNodes.has(key);
    return (
      <div key={key}>
        <button type="button" onClick={() => toggleNode(key)} style={{ ...GROUP, paddingLeft: 8 + depth * 14 }}>
          {open ? "▾" : "▸"} {node.name}
        </button>
        {open && node.children.map((child) => renderNode(port, child, depth + 1))}
      </div>
    );
  };

  const q = query.trim().toLowerCase();
  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Browse ADS tags">
      <header className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices & Connections</div>
          <div className="trust-inspector__title">Browse tags</div>
          <div style={TARGET_LABEL}>{targetLabel}</div>
        </div>
      </header>

      <section className="trust-section" style={{ paddingBottom: 10 }}>
        <strong style={PORT_SUMMARY}>{portSummary}</strong>
        <details style={{ marginTop: 6 }}>
          <summary style={DETAILS_SUMMARY}>Connection details</summary>
          <div style={DETAILS_BODY}>
            <span>AMS Net ID: {adsTargetNetId(target)}</span>
            <span>Host: {targetHost(target)}</span>
          </div>
        </details>
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search tags on all ADS ports"
          className="trust-input"
          style={{ marginTop: 9 }}
        />
      </section>

      <div style={{ flex: 1, overflow: "auto", padding: "6px 8px" }}>
        {views.map((view) => {
          const open = expandedPorts.has(view.port);
          const leaves = leafNodes(view.tree ?? []);
          const visibleLeaves = q
            ? leaves.filter((node) =>
                node.name.toLowerCase().includes(q) ||
                node.path.toLowerCase().includes(q),
              )
            : undefined;
          return (
            <section key={view.port} style={PORT_CARD} data-ads-port={view.port}>
              <button type="button" onClick={() => togglePort(view.port)} style={PORT_HEADER}>
                <span>{open ? "▾" : "▸"} ADS port {view.port}</span>
                <span style={PORT_COUNT}>
                  {view.loading ? "Loading…" : `${leaves.length} ${leaves.length === 1 ? "tag" : "tags"}`}
                </span>
              </button>
              {open && (
                <div style={{ padding: "3px 0 6px" }}>
                  {view.routeMissing ? (
                    <RouteRecovery
                      port={view.port}
                      routePlan={view.routePlan}
                      onCreateRoute={onCreateRoute}
                      onCopy={onCopy}
                    />
                  ) : view.error ? (
                    <p style={ERROR_TEXT}>{view.error.title}: {view.error.detail}</p>
                  ) : view.loading ? (
                    <p style={EMPTY}>Loading tags…</p>
                  ) : q ? (
                    visibleLeaves?.length
                      ? visibleLeaves.map((node) => renderLeaf(view.port, node, 0))
                      : <p style={EMPTY}>No matching tags on this port.</p>
                  ) : view.tree?.length ? (
                    view.tree.map((node) => renderNode(view.port, node, 0))
                  ) : view.tree === undefined ? (
                    <div style={{ padding: "7px 9px" }}>
                      <p style={{ ...EMPTY, padding: 0 }}>Tags have not been loaded from this port yet.</p>
                      <button
                        type="button"
                        onClick={() => onBrowseTarget(withAdsTargetPort(target, view.port))}
                        className="trust-button"
                        style={{ marginTop: 7 }}
                      >
                        Browse port {view.port}
                      </button>
                    </div>
                  ) : (
                    <p style={EMPTY}>This ADS port has no browsable tags.</p>
                  )}
                </div>
              )}
            </section>
          );
        })}

        <button
          type="button"
          onClick={() => setAdvancedOpen((open) => !open)}
          className="trust-button"
          style={{ width: "100%", marginTop: 8 }}
        >
          {advancedOpen ? "Hide advanced" : "Advanced: browse another ADS port"}
        </button>
        {advancedOpen && (
          <div className="trust-section" style={{ marginTop: 6 }}>
            <label className="trust-field">
              <span className="trust-field__label">ADS port</span>
              <input
                type="number"
                min={1}
                max={65535}
                value={portDraft}
                onChange={(event) => setPortDraft(event.target.value)}
                className="trust-input"
                placeholder="851"
              />
            </label>
            {parsedAdvancedPort.error && (
              <div className="trust-field__message trust-field__message--error">
                {parsedAdvancedPort.error}
              </div>
            )}
            <button
              type="button"
              disabled={loading || !parsedAdvancedPort.port}
              title={parsedAdvancedPort.error}
              onClick={() => {
                if (parsedAdvancedPort.port) {
                  onBrowseTarget(withAdsTargetPort(target, parsedAdvancedPort.port));
                }
              }}
              className="trust-button trust-button--primary"
              style={{ width: "100%", marginTop: 8 }}
            >
              Browse this port
            </button>
          </div>
        )}
      </div>

      <footer className="trust-section" style={FOOTER}>
        {importResult && <ImportResult result={importResult} />}
        <label style={WRITE_TOGGLE}>
          <input
            type="checkbox"
            checked={allowWrites}
            disabled={importLoading}
            onChange={(event) => setAllowWrites(event.target.checked)}
          />
          Allow writes for newly added tags
        </label>
        <button type="button" onClick={onClose} className="trust-button" style={{ width: "100%" }}>
          Done
        </button>
      </footer>
    </aside>
  );
}

function adsPortViews(
  target: Record<string, unknown>,
  currentTree: SymbolNode[] | undefined,
  routeMissing: boolean,
  routePlan: RoutePlan | undefined,
  error: BrowseErrorView | undefined,
  loading: boolean,
): AdsPortView[] {
  const views = new Map<number, AdsPortView>();
  for (const port of respondingAdsPorts(target)) {
    views.set(port, {
      port,
      loading: false,
      routeMissing: false,
    });
  }
  for (const evidence of adsPortBrowseEvidence(target)) {
    views.set(evidence.port, {
      port: evidence.port,
      tree: evidence.tree,
      loading: false,
      routeMissing: evidence.routeMissing,
      routePlan: evidence.routePlan,
      error: evidence.error ? classifyBrowseError("ads", evidence.error) : undefined,
    });
  }
  const currentPort = adsTargetPort(target);
  if (currentTree !== undefined || loading || routeMissing || error || views.size === 0) {
    views.set(currentPort, {
      port: currentPort,
      tree: currentTree,
      loading,
      routeMissing,
      routePlan,
      error,
    });
  }
  return [...views.values()].sort((left, right) => left.port - right.port);
}

function mergePortViews(
  previous: readonly AdsPortView[],
  incoming: readonly AdsPortView[],
): AdsPortView[] {
  const merged = new Map(previous.map((view) => [view.port, view]));
  for (const view of incoming) {
    const existing = merged.get(view.port);
    const isPlaceholder =
      view.tree === undefined &&
      !view.loading &&
      !view.routeMissing &&
      !view.error;
    if (!existing || !isPlaceholder) {
      merged.set(view.port, view);
    }
  }
  return [...merged.values()].sort((left, right) => left.port - right.port);
}

function leafNodes(nodes: readonly SymbolNode[]): SymbolNode[] {
  const leaves: SymbolNode[] = [];
  const walk = (items: readonly SymbolNode[]) => {
    for (const item of items) {
      if (item.children?.length) {
        walk(item.children);
      } else {
        leaves.push(item);
      }
    }
  };
  walk(nodes);
  return leaves;
}

function selectionKey(port: number, node: SymbolNode): string {
  return `${port}:${nodeKey(node)}`;
}

function selectionKeysForPaths(
  views: readonly AdsPortView[],
  port: number,
  paths: readonly string[],
): string[] {
  const selectedPaths = new Set(paths);
  const view = views.find((candidate) => candidate.port === port);
  return leafNodes(view?.tree ?? [])
    .filter((node) => selectedPaths.has(node.path))
    .map((node) => selectionKey(port, node));
}

function importedPathKey(port: number, path: string): string {
  return `${port}:${path}`;
}

function importedTagKeys(target: Record<string, unknown>): ReadonlySet<string> {
  const rawSelections = normalizeAdsTagSelections(target.imported_ads_symbols);
  const directPoints = Array.isArray(target.points) ? target.points : [];
  const directPort = adsTargetPort(target);
  const pointPaths = directPoints.flatMap((point): string[] => {
    if (!isRecord(point)) {
      return [];
    }
    const path = point.symbol ?? point.path;
    return typeof path === "string" && path.trim().length > 0
      ? [path.trim()]
      : [];
  });
  return new Set([
    ...rawSelections.flatMap((selection) =>
      selection.paths.map((path) => importedPathKey(selection.port, path)),
    ),
    ...pointPaths.map((path) => importedPathKey(directPort, path)),
  ]);
}

function selectedImportedTagKeys(
  views: readonly AdsPortView[],
  imported: ReadonlySet<string>,
): ReadonlySet<string> {
  return new Set(
    views.flatMap((view) =>
      leafNodes(view.tree ?? [])
        .filter((node) => imported.has(importedPathKey(view.port, node.path)))
        .map((node) => selectionKey(view.port, node)),
    ),
  );
}

function targetHost(target: Record<string, unknown>): string {
  const host = target.host ?? target.ip;
  return typeof host === "string" && host.trim().length > 0
    ? host.trim()
    : "Unknown";
}

function RouteRecovery({
  port,
  routePlan,
  onCreateRoute,
  onCopy,
}: {
  port: number;
  routePlan?: RoutePlan;
  onCreateRoute: (port?: number) => void;
  onCopy: (text: string) => void;
}) {
  return (
    <div style={ROUTE_WARNING}>
      <p style={{ margin: 0 }}>An ADS route is required before tags can be read from port {port}.</p>
      <button type="button" onClick={() => onCreateRoute(port)} className="trust-button" style={{ marginTop: 7 }}>
        Create route
      </button>
      {(routePlan?.artifacts ?? []).map((artifact, index) => (
        <button
          key={artifact.kind ?? artifact.label ?? index}
          type="button"
          onClick={() => onCopy(artifact.content)}
          className="trust-button"
          style={{ marginTop: 7, marginLeft: 5 }}
        >
          Copy {artifact.label}
        </button>
      ))}
    </div>
  );
}

function ImportResult({ result }: { result: AdsTagBatchImportResult }) {
  const failures = result.ports.filter((port) => !port.applied);
  return (
    <div style={result.applied ? SUCCESS : ROUTE_WARNING}>
      <strong>{adsTagBatchSummary(result)}</strong>
      {result.restartRequired && (
        <div style={{ marginTop: 4 }}>Start or restart the simulator to use the new tags.</div>
      )}
      {failures.map((failure) => (
        <div key={failure.port} style={{ marginTop: 4 }}>
          Port {failure.port}: {failure.message}
        </div>
      ))}
    </div>
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const PANEL: React.CSSProperties = { position: "absolute", inset: "0 0 0 auto", width: 390, maxWidth: "95vw", zIndex: 8 };
const TARGET_LABEL: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" };
const PORT_SUMMARY: React.CSSProperties = { display: "block", color: "var(--trust-text)", fontSize: 12 };
const DETAILS_SUMMARY: React.CSSProperties = { color: "var(--trust-text-muted)", cursor: "pointer", fontSize: 10.5 };
const DETAILS_BODY: React.CSSProperties = { display: "flex", flexDirection: "column", gap: 2, marginTop: 5, color: "var(--trust-text-muted)", fontSize: 10.5 };
const PORT_CARD: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: "var(--trust-radius-lg)", marginBottom: 7, overflow: "hidden", background: "var(--trust-surface)" };
const PORT_HEADER: React.CSSProperties = { display: "flex", justifyContent: "space-between", width: "100%", border: 0, padding: "8px 10px", background: "var(--trust-surface-raised)", color: "var(--trust-text)", cursor: "pointer", fontSize: 12, fontWeight: 650 };
const PORT_COUNT: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 500 };
const ROW: React.CSSProperties = { display: "flex", alignItems: "center", gap: 7, minHeight: 28, paddingRight: 8 };
const GROUP: React.CSSProperties = { display: "block", width: "100%", border: 0, background: "transparent", color: "var(--trust-text)", cursor: "pointer", textAlign: "left", paddingTop: 5, paddingBottom: 5, fontSize: 11.5, fontWeight: 600 };
const TAG_NAME: React.CSSProperties = { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: "var(--trust-text)", fontSize: 11.5 };
const TAG_META: React.CSSProperties = { flex: "none", color: "var(--trust-text-muted)", fontSize: 9.5 };
const ADDED_BADGE: React.CSSProperties = { flex: "none", color: t.ok, fontSize: 9.5, fontWeight: 650 };
const EMPTY: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 11, margin: 0, padding: "8px 10px" };
const ERROR_TEXT: React.CSSProperties = { ...EMPTY, color: t.danger };
const ROUTE_WARNING: React.CSSProperties = { margin: "6px 8px", border: `1px solid ${tint(t.warn, 0.5)}`, borderRadius: 7, padding: "8px 9px", background: tint(t.warn, 0.1), color: "var(--trust-text)", fontSize: 10.5, lineHeight: 1.4 };
const SUCCESS: React.CSSProperties = { border: `1px solid ${tint(t.ok, 0.55)}`, borderRadius: 7, padding: "8px 9px", background: tint(t.ok, 0.1), color: "var(--trust-text)", fontSize: 10.5, lineHeight: 1.4 };
const FOOTER: React.CSSProperties = { display: "flex", flexDirection: "column", gap: 7, borderTop: "1px solid var(--trust-border)" };
const WRITE_TOGGLE: React.CSSProperties = { display: "flex", alignItems: "center", gap: 7, color: "var(--trust-text)", fontSize: 10.5 };
