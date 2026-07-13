import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RoutePlan, SymbolNode } from "../offlineComm";
import { AdsBrowseTargetControls } from "./AdsBrowseTargetControls";
import type { BrowseErrorView } from "./browseErrorModel";
import { nodeKey } from "./opcuaClientModel";
import { SymbolSelectionCheckbox } from "./SymbolSelectionCheckbox";
import { t, tint } from "./theme";

// §0.5.2 Browse remote values — look INSIDE a target (e.g. an ADS PLC's variable table). Searchable
// tree, multi-select, read-only by default (writes need an explicit toggle). For ADS, "Add variables"
// feeds the existing ADS import / Generate-ST / ads.toml pipeline (not a separate store). If the
// AMS route is missing, one honest "Route setup" action exposes the required instructions.
export function BrowseTagsPanel({
  title = "Browse tags",
  actionLabel = "Add tags",
  targetLabel,
  protocol,
  target,
  tree,
  routeMissing,
  routePlan,
  error,
  loading,
  onCreateRoute,
  onTrustCertificate,
  onEditCredentials,
  onCopy,
  onBrowseTarget,
  onAddTags,
  onClose,
}: {
  title?: string;
  actionLabel?: string;
  targetLabel: string;
  protocol: string;
  target: Record<string, unknown>;
  tree: SymbolNode[] | undefined;
  routeMissing: boolean;
  routePlan?: RoutePlan;
  error?: BrowseErrorView;
  loading: boolean;
  onCreateRoute: () => void;
  onTrustCertificate?: () => void; // explicit cert-trust + re-browse (opcua_client)
  onEditCredentials?: () => void; // opcua_client auth recovery: reopen the endpoint form prefilled
  onCopy: (text: string) => void;
  onBrowseTarget: (target: Record<string, unknown>) => void;
  onAddTags: (paths: string[], writable: boolean) => void;
  onClose: () => void;
}) {
  // §0.5.2 route setup: the route_plan carries ready-to-run scripts (PowerShell / StaticRoutes.xml /
  // manual) the user runs on the TwinCAT — no credentials handled here, no mutation from the canvas.
  const artifacts = routePlan?.artifacts ?? [];
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [allowWrites, setAllowWrites] = useState(false);
  const [routeCreateAttempted, setRouteCreateAttempted] = useState(false);
  const [adsPortDraftStale, setAdsPortDraftStale] = useState(false);
  const allowWritesRef = useRef(false);
  const lastAutoExpandedTreeRef = useRef<SymbolNode[] | undefined>(undefined);
  const isAds = protocol === "ads";
  const remoteDiscoveryReadOnly =
    isAds && typeof target.discovery_origin_runtime_id === "string";

  const setAllowWritesChecked = useCallback((checked: boolean) => {
    allowWritesRef.current = checked;
    setAllowWrites(checked);
  }, []);

  const handleAdsDraftStaleChange = useCallback(
    (stale: boolean) => {
      setAdsPortDraftStale(stale);
      if (stale) {
        setSelected(new Set());
        setAllowWritesChecked(false);
      }
    },
    [setAllowWritesChecked]
  );

  useEffect(() => {
    allowWritesRef.current = allowWrites;
  }, [allowWrites]);

  useEffect(() => {
    if (!routeMissing) {
      setRouteCreateAttempted(false);
    }
  }, [routeMissing]);

  useEffect(() => {
    if (
      !tree ||
      tree === lastAutoExpandedTreeRef.current ||
      tree.length !== 1 ||
      !tree[0].children?.length
    ) {
      return;
    }
    lastAutoExpandedTreeRef.current = tree;
    const rootKey = nodeKey(tree[0]);
    setExpanded((previous) => new Set([...previous, rootKey]));
  }, [tree]);

  // `selected` holds stable node keys (nodeKey: node_id ?? id ?? path), never the display path, so
  // two leaves sharing a path can't be conflated.
  const setSelectionChecked = (key: string, checked: boolean) =>
    setSelected((prev) => {
      if (prev.has(key) === checked) {
        return prev;
      }
      const next = new Set(prev);
      checked ? next.add(key) : next.delete(key);
      return next;
    });
  const toggleExp = (id: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  // Search: flatten to matching leaves; otherwise show the tree.
  const q = query.trim().toLowerCase();
  const matches = useMemo(() => {
    if (!q) {
      return undefined;
    }
    const out: SymbolNode[] = [];
    const walk = (nodes: SymbolNode[]) => {
      for (const n of nodes) {
        if (n.children?.length) {
          walk(n.children);
        } else if (n.path.toLowerCase().includes(q) || n.name.toLowerCase().includes(q)) {
          out.push(n);
        }
      }
    };
    walk(tree ?? []);
    return out;
  }, [q, tree]);
  const selectableKeys = useMemo(
    () =>
      routeMissing || error || loading || adsPortDraftStale
        ? new Set<string>()
        : collectLeafKeys(tree ?? []),
    [adsPortDraftStale, error, loading, routeMissing, tree]
  );
  const selectedAddKeys = useMemo(
    () => [...selected].filter((key) => selectableKeys.has(key)),
    [selectableKeys, selected]
  );

  useEffect(() => {
    setSelected((prev) => {
      if (prev.size === 0) {
        return prev;
      }
      const next = new Set([...prev].filter((key) => selectableKeys.has(key)));
      return next.size === prev.size ? prev : next;
    });
  }, [selectableKeys]);

  const addDisabledReason =
    remoteDiscoveryReadOnly
      ? "Remote discovery is read-only. Run the project on that computer before adding variables."
      : adsPortDraftStale
      ? "Browse the edited ADS service before adding variables from it."
      : routeMissing
        ? isAds
          ? "Create the route and retry browse before adding variables."
          : "Create the route and browse again before adding tags."
        : error
          ? isAds
            ? "Resolve the browse error and retry browse before adding variables."
            : "Resolve the browse error before adding tags."
          : loading
            ? isAds
              ? "Wait for browse results before adding variables."
              : "Wait for browse results before adding tags."
            : tree === undefined
              ? isAds
                ? "Choose an ADS service port and browse its variables first."
                : "Start or connect the runtime, then Browse again to load symbols."
              : tree.length === 0
                ? isAds
                  ? "No variables are available to add."
                  : "No symbols are available to add."
                : selectedAddKeys.length === 0
                  ? isAds
                    ? "Select at least one variable to add."
                    : "Select at least one symbol to add."
                  : undefined;
  const writeToggleDisabled = remoteDiscoveryReadOnly || routeMissing || Boolean(error) || loading || adsPortDraftStale || tree === undefined || tree.length === 0;
  const routeWarningText = routeCreateAttempted
    ? artifacts.length
      ? "Route needs administrator access. Run the generated route script on the ADS device, then select Retry browse."
      : "Route needs administrator access. Create the route on the ADS device, then select Retry browse."
    : "Warning: No ADS route to the remote ADS device. Add the route, then select Retry browse.";

  const accessLabel = (n: SymbolNode): string =>
    n.writable === true ? "read/write" : n.writable === false ? "read-only" : "";

  const leaf = (n: SymbolNode, depth: number) => (
    <div
      key={nodeKey(n)}
      data-role="symbol-leaf"
      style={{ ...ROW, paddingLeft: 8 + depth * 14 }}
    >
      <SymbolSelectionCheckbox
        checked={selected.has(nodeKey(n))}
        label={`Select ${n.path}`}
        onCheckedChange={(checked) => setSelectionChecked(nodeKey(n), checked)}
      />
      <span style={{ flex: 1, minWidth: 0, fontSize: 12, color: "var(--trust-text)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{n.name}</span>
      {(n.data_type || n.type) && <span style={{ flex: "none", fontSize: 10, color: "var(--trust-text-muted)" }}>{n.data_type || n.type}</span>}
      {accessLabel(n) && <span title={`${accessLabel(n)} on the device`} style={{ flex: "none", fontSize: 9, color: "var(--trust-text-subtle)" }}>{accessLabel(n)}</span>}
    </div>
  );

  const renderNode = (n: SymbolNode, depth: number): React.ReactNode => {
    if (n.children?.length) {
      const open = expanded.has(nodeKey(n));
      return (
        <div key={nodeKey(n)}>
          <button
            data-role="symbol-group"
            data-expanded={open}
            onClick={() => toggleExp(nodeKey(n))}
            style={{ ...GROUP, paddingLeft: 4 + depth * 14 }}
          >
            {open ? "▾" : "▸"} {n.name}
          </button>
          {open && n.children.map((c) => renderNode(c, depth + 1))}
        </div>
      );
    }
    return leaf(n, depth);
  };

  return (
    <aside className="trust-inspector" style={PANEL} aria-label={isAds ? "Browse variables" : "Browse tags"}>
      <div className="trust-inspector__header">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="trust-inspector__eyebrow">Devices & Connections</div>
          <div className="trust-inspector__title">{title}</div>
          <span style={{ fontSize: 10.5, color: "var(--trust-text-muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", display: "block" }}>{targetLabel}</span>
        </div>
        <button onClick={onClose} aria-label="Close" className="trust-button" style={ICON}>✕</button>
      </div>

      {protocol === "ads" && (
        <AdsBrowseTargetControls
          target={target}
          loading={loading}
          browseFailed={routeMissing || Boolean(error)}
          onBrowse={(nextTarget) => {
            setAdsPortDraftStale(false);
            onBrowseTarget(nextTarget);
          }}
          onDraftStaleChange={handleAdsDraftStaleChange}
        />
      )}

      {routeMissing && !adsPortDraftStale && (
        <div style={WARNING_BAR}>
          <span style={WARNING_TEXT}>{routeWarningText}</span>
          <button
            onClick={() => {
              setRouteCreateAttempted(true);
              onCreateRoute();
            }}
            style={ROUTEBTN}
          >
            Route setup
          </button>
        </div>
      )}

      {error && !adsPortDraftStale && (
        <div style={WARNING_BAR}>
          <span style={WARNING_TEXT}>Warning: {error.title}: {error.detail}</span>
          {error.action === "trust" && onTrustCertificate && (
            <button onClick={onTrustCertificate} style={ROUTEBTN}>Trust certificate</button>
          )}
          {error.action === "credentials" && onEditCredentials && (
            <button onClick={onEditCredentials} style={ROUTEBTN}>Edit credentials</button>
          )}
          {error.technicalDetail && (
            <details data-role="browse-error-technical" style={ERROR_DETAILS}>
              <summary>Technical details</summary>
              <div>{error.technicalDetail}</div>
            </details>
          )}
        </div>
      )}

      {!routeMissing && !error && !adsPortDraftStale && tree !== undefined && (
        <div style={{ padding: "9px 14px", borderBottom: "1px solid var(--trust-border)" }}>
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={isAds ? "Search variables" : "Search symbols"} style={SEARCH} />
        </div>
      )}

      <div style={{ flex: 1, overflow: "auto", padding: "6px 8px" }}>
        {adsPortDraftStale ? (
          <p style={EMPTY}>
            The displayed ADS port has not been browsed yet. Browse it before selecting or adding
            variables.
          </p>
        ) : routeMissing ? (
          artifacts.length ? (
            <div style={{ padding: "2px 4px" }}>
              {routeCreateAttempted && (
                <div style={ROUTE_RESULT}>
                  Automatic route creation is not available from this canvas in this build. Run the
                  generated PowerShell as Administrator on the ADS device, or copy the static
                  route values below, then select Retry browse.
                </div>
              )}
              <p style={{ fontSize: 11.5, color: "var(--trust-text)", margin: "4px 6px 10px", lineHeight: 1.5 }}>
                The remote ADS router needs a route back to truST. Run one of these on the ADS device, then select Retry browse.
              </p>
              {artifacts.map((a, i) => (
                <div key={a.kind ?? a.label ?? String(i)} style={ARTCARD}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
                    <span style={{ flex: 1, minWidth: 0, fontSize: 11.5, fontWeight: 700, color: "var(--trust-text)" }}>{a.label}</span>
                    <button onClick={() => onCopy(a.content)} style={COPYBTN}>Copy</button>
                  </div>
                  <pre style={ARTPRE}>{a.content}</pre>
                </div>
              ))}
            </div>
          ) : (
            <>
              {routeCreateAttempted && (
                <div style={ROUTE_RESULT}>
                  Route setup needs administrator access. Add the route on the ADS device, then
                  select Retry browse to load variables.
                </div>
              )}
              <p style={EMPTY}>Create the route on the remote ADS device, then select Retry browse.</p>
            </>
          )
        ) : error ? (
          <p style={EMPTY}>Resolve the browse error above, then select Retry browse.</p>
        ) : loading ? (
          <p style={EMPTY}>{isAds ? "Loading variables…" : "Loading symbols…"}</p>
        ) : tree === undefined ? (
          <p style={EMPTY}>
            {isAds
              ? "Choose the ADS service port above, then browse that service's variables."
              : "Start or connect the runtime, then Browse again to load symbols."}
          </p>
        ) : matches ? (
          matches.length ? matches.map((n) => leaf(n, 0)) : <p style={EMPTY}>{isAds ? "No matching variables." : "No matching symbols."}</p>
        ) : tree.length ? (
          tree.map((n) => renderNode(n, 0))
        ) : (
          <p style={EMPTY}>
            {isAds
              ? "The ADS service returned no variables."
              : "No symbols found."}
          </p>
        )}
      </div>

      <div style={{ padding: 12, borderTop: "1px solid var(--trust-border)" }}>
        <label
          title={writeToggleDisabled ? addDisabledReason : undefined}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            fontSize: 11,
            color: "var(--trust-text)",
            marginBottom: 9,
            cursor: writeToggleDisabled ? "not-allowed" : "pointer",
            opacity: writeToggleDisabled ? 0.7 : 1,
          }}
        >
          <input
            data-role="allow-writes"
            type="checkbox"
            checked={allowWrites}
            disabled={writeToggleDisabled}
            onChange={(e) => setAllowWritesChecked(e.target.checked)}
          />
          Allow writes (default: read-only)
        </label>
        <button
          onClick={(event) => {
            const visibleAllowWrites =
              event.currentTarget
                .closest("aside")
                ?.querySelector<HTMLInputElement>('input[data-role="allow-writes"]')
                ?.checked ?? allowWritesRef.current;
            onAddTags(selectedAddKeys, visibleAllowWrites);
          }}
          disabled={Boolean(addDisabledReason)}
          title={addDisabledReason}
          className={addDisabledReason ? "trust-button" : "trust-button trust-button--primary"}
          style={{ width: "100%" }}
        >
          {selectedAddKeys.length > 0 ? `${actionLabel} (${selectedAddKeys.length})` : actionLabel}
        </button>
        {addDisabledReason && <p className="trust-help" style={{ marginTop: 6 }}>{addDisabledReason}</p>}
      </div>
    </aside>
  );
}

function collectLeafKeys(nodes: SymbolNode[]): Set<string> {
  const keys = new Set<string>();
  const walk = (items: SymbolNode[]) => {
    for (const item of items) {
      if (item.children?.length) {
        walk(item.children);
      } else {
        keys.add(nodeKey(item));
      }
    }
  };
  walk(nodes);
  return keys;
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 340,
  maxWidth: "92vw",
  zIndex: 8,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};
const ROW: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "4px 6px", borderRadius: 6 };
const GROUP: React.CSSProperties = { display: "block", width: "100%", textAlign: "left", border: "none", background: "transparent", color: "var(--trust-text)", fontSize: 12, fontWeight: 600, cursor: "pointer", padding: "5px 6px" };
const SEARCH: React.CSSProperties = { width: "100%", background: "var(--trust-input-bg)", border: "1px solid var(--trust-input-border)", borderRadius: "var(--trust-radius)", color: "var(--trust-text)", padding: "6px 9px", fontSize: 12 };
const WARNING_BAR: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  flexWrap: "wrap",
  gap: 10,
  padding: "10px 14px",
  background: tint(t.warn, 0.12),
  borderBottom: `1px solid ${tint(t.warn, 0.4)}`,
};
const WARNING_TEXT: React.CSSProperties = { flex: 1, fontSize: 11.5, color: t.warn };
const ERROR_DETAILS: React.CSSProperties = { flexBasis: "100%", color: "var(--trust-text-muted)", fontSize: 10, lineHeight: 1.4 };
const ROUTEBTN: React.CSSProperties = { flex: "none", border: `1px solid ${t.warn}`, background: tint(t.warn, 0.16), color: t.warn, borderRadius: 6, padding: "4px 10px", fontSize: 11, cursor: "pointer" };
const ROUTE_RESULT: React.CSSProperties = { border: `1px solid ${tint(t.warn, 0.5)}`, borderRadius: 8, margin: "0 4px 10px", padding: "8px 10px", background: tint(t.warn, 0.1), color: "var(--trust-text)", fontSize: 11.5, lineHeight: 1.45 };
const ARTCARD: React.CSSProperties = { border: "1px solid var(--trust-border)", borderRadius: "var(--trust-radius-lg)", padding: "9px 10px", margin: "0 4px 9px", background: "var(--trust-surface)" };
const ARTPRE: React.CSSProperties = { margin: 0, maxHeight: 150, overflow: "auto", background: "var(--trust-canvas)", border: "1px solid var(--trust-border)", borderRadius: "var(--trust-radius)", padding: "7px 9px", fontSize: 10.5, lineHeight: 1.45, color: "var(--trust-text)", whiteSpace: "pre-wrap", wordBreak: "break-word", fontFamily: "var(--trust-mono)" };
const COPYBTN: React.CSSProperties = { flex: "none", border: "1px solid var(--trust-accent)", background: "var(--trust-selected-bg)", color: "var(--trust-text)", borderRadius: 6, padding: "3px 10px", fontSize: 11, cursor: "pointer" };
const ICON: React.CSSProperties = { minHeight: 24, width: 26, padding: 0 };
const EMPTY: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 11.5, padding: "8px 8px", lineHeight: 1.5 };
