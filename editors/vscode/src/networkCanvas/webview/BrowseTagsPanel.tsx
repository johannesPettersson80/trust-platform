import React, { useEffect, useMemo, useRef, useState } from "react";
import type { RoutePlan, SymbolNode } from "../offlineComm";
import { AdsBrowseTargetControls } from "./AdsBrowseTargetControls";
import type { BrowseErrorView } from "./browseErrorModel";
import { nodeKey } from "./opcuaClientModel";
import { SymbolSelectionCheckbox } from "./SymbolSelectionCheckbox";
import { t, tint } from "./theme";

// §0.5.2 Browse tags/signals — look INSIDE a target (e.g. an ADS PLC's symbol table). Searchable
// tree, multi-select, read-only by default (writes need an explicit toggle). For ADS, "Add tags"
// feeds the existing ADS import / Generate-ST / ads.toml pipeline (not a separate store). If the
// AMS route is missing, one "Create route" button — the classic ADS gotcha, handled.
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

  const setAllowWritesChecked = (checked: boolean) => {
    allowWritesRef.current = checked;
    setAllowWrites(checked);
  };

  useEffect(() => {
    allowWritesRef.current = allowWrites;
  }, [allowWrites]);

  useEffect(() => {
    if (!routeMissing) {
      setRouteCreateAttempted(false);
    }
  }, [routeMissing]);

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
    adsPortDraftStale
      ? "Browse the edited ADS port before adding tags from that server."
      : routeMissing
        ? "Create the route and browse again before adding tags."
        : error
          ? "Resolve the browse error before adding tags."
          : loading
            ? "Wait for browse results before adding tags."
            : tree === undefined
              ? protocol === "ads"
                ? "Choose an ADS port and browse its symbols first."
                : "Start or connect the runtime, then Browse again to load symbols."
              : tree.length === 0
                ? "No symbols are available to add."
                : selectedAddKeys.length === 0
                  ? "Select at least one symbol to add."
                  : undefined;
  const writeToggleDisabled = routeMissing || Boolean(error) || loading || adsPortDraftStale || tree === undefined || tree.length === 0;
  const routeWarningText = routeCreateAttempted
    ? artifacts.length
      ? "Route needs TwinCAT administrator access. Run the generated route script on the TwinCAT computer, then reopen Browse."
      : "Route needs TwinCAT administrator access. Create the route on the TwinCAT computer, then reopen Browse."
    : "Warning: No ADS route to the TwinCAT system. Add the route, then browse again.";

  const accessLabel = (n: SymbolNode): string =>
    n.writable === true ? "read/write" : n.writable === false ? "read-only" : "";

  const leaf = (n: SymbolNode, depth: number) => (
    <div key={nodeKey(n)} style={{ ...ROW, paddingLeft: 8 + depth * 14 }}>
      <SymbolSelectionCheckbox
        checked={selected.has(nodeKey(n))}
        label={`Select ${n.path}`}
        onCheckedChange={(checked) => setSelectionChecked(nodeKey(n), checked)}
      />
      <span style={{ flex: 1, minWidth: 0, fontSize: 12, color: "var(--vscode-foreground, #eef1f5)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{n.name}</span>
      {(n.data_type || n.type) && <span style={{ flex: "none", fontSize: 10, color: "var(--vscode-descriptionForeground, #7f8794)" }}>{n.data_type || n.type}</span>}
      {accessLabel(n) && <span title={`${accessLabel(n)} on the device`} style={{ flex: "none", fontSize: 9, color: "var(--vscode-disabledForeground, #6a7280)" }}>{accessLabel(n)}</span>}
    </div>
  );

  const renderNode = (n: SymbolNode, depth: number): React.ReactNode => {
    if (n.children?.length) {
      const open = expanded.has(nodeKey(n));
      return (
        <div key={nodeKey(n)}>
          <button onClick={() => toggleExp(nodeKey(n))} style={{ ...GROUP, paddingLeft: 4 + depth * 14 }}>
            {open ? "▾" : "▸"} {n.name}
          </button>
          {open && n.children.map((c) => renderNode(c, depth + 1))}
        </div>
      );
    }
    return leaf(n, depth);
  };

  return (
    <aside style={PANEL} aria-label="Browse tags">
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "12px 14px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <span style={{ fontSize: 10, fontWeight: 700, textTransform: "uppercase", letterSpacing: 0.4, color: "var(--vscode-descriptionForeground, #7f8794)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", display: "block" }}>Devices & Connections</span>
          <strong style={{ display: "block", fontSize: 14 }}>{title}</strong>
          <span style={{ fontSize: 10.5, color: "var(--vscode-descriptionForeground, #7f8794)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", display: "block" }}>{targetLabel}</span>
        </div>
        <button onClick={onClose} aria-label="Close" style={ICON}>✕</button>
      </div>

      {protocol === "ads" && (
        <AdsBrowseTargetControls
          target={target}
          loading={loading}
          onBrowse={(nextTarget) => {
            setAdsPortDraftStale(false);
            onBrowseTarget(nextTarget);
          }}
          onDraftStaleChange={setAdsPortDraftStale}
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
            Create route
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
        </div>
      )}

      {!routeMissing && !error && !adsPortDraftStale && tree !== undefined && (
        <div style={{ padding: "9px 14px", borderBottom: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search symbols" style={SEARCH} />
        </div>
      )}

      <div style={{ flex: 1, overflow: "auto", padding: "6px 8px" }}>
        {adsPortDraftStale ? (
          <p style={EMPTY}>
            The displayed ADS port has not been browsed yet. Browse it before selecting or adding
            symbols.
          </p>
        ) : routeMissing ? (
          artifacts.length ? (
            <div style={{ padding: "2px 4px" }}>
              {routeCreateAttempted && (
                <div style={ROUTE_RESULT}>
                  Automatic route creation is not available from this canvas in this build. Run the
                  generated PowerShell as Administrator on the TwinCAT computer, or copy the static
                  route values below, then reopen Browse.
                </div>
              )}
              <p style={{ fontSize: 11.5, color: "var(--vscode-foreground, #cfd6e0)", margin: "4px 6px 10px", lineHeight: 1.5 }}>
                TwinCAT needs a route back to truST. Run one of these on the TwinCAT computer, then reopen Browse.
              </p>
              {artifacts.map((a, i) => (
                <div key={a.kind ?? a.label ?? String(i)} style={ARTCARD}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
                    <span style={{ flex: 1, minWidth: 0, fontSize: 11.5, fontWeight: 700, color: "var(--vscode-foreground, #eef1f5)" }}>{a.label}</span>
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
                  Route creation needs TwinCAT administrator access. Create the route on the
                  TwinCAT computer, then reopen Browse to load the symbol table.
                </div>
              )}
              <p style={EMPTY}>Create the route on the PLC, then reopen Browse to load the symbol table.</p>
            </>
          )
        ) : error ? (
          <p style={EMPTY}>Resolve the browse error above, then browse again.</p>
        ) : loading ? (
          <p style={EMPTY}>Loading symbols…</p>
        ) : tree === undefined ? (
          <p style={EMPTY}>
            {protocol === "ads"
              ? "Choose the ADS server port above, then browse that server's symbol namespace."
              : "Start or connect the runtime, then Browse again to load symbols."}
          </p>
        ) : matches ? (
          matches.length ? matches.map((n) => leaf(n, 0)) : <p style={EMPTY}>No matching symbols.</p>
        ) : tree.length ? (
          tree.map((n) => renderNode(n, 0))
        ) : (
          <p style={EMPTY}>
            {protocol === "ads"
              ? "The ADS server returned an empty symbol table or no compatible symbols."
              : "No symbols found."}
          </p>
        )}
      </div>

      <div style={{ padding: 12, borderTop: "1px solid var(--vscode-editorWidget-border, #2a2f3a)" }}>
        <label
          title={writeToggleDisabled ? addDisabledReason : undefined}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            fontSize: 11,
            color: "var(--vscode-foreground, #cfd6e0)",
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
  background: "var(--vscode-editorHoverWidget-background, rgba(18,21,28,.98))",
  borderLeft: "1px solid var(--vscode-editorWidget-border, #2a2f3a)",
  boxShadow: "-18px 0 50px rgba(0,0,0,.45)",
  zIndex: 8,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};
const ROW: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, padding: "4px 6px", borderRadius: 6 };
const GROUP: React.CSSProperties = { display: "block", width: "100%", textAlign: "left", border: "none", background: "transparent", color: "var(--vscode-foreground, #cfd6e0)", fontSize: 12, fontWeight: 600, cursor: "pointer", padding: "5px 6px" };
const SEARCH: React.CSSProperties = { width: "100%", background: "var(--vscode-input-background, #10141b)", border: "1px solid var(--vscode-input-border, #343b47)", borderRadius: 7, color: "var(--vscode-foreground, #eef1f5)", padding: "6px 9px", fontSize: 12 };
const WARNING_BAR: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 10,
  padding: "10px 14px",
  background: tint(t.warn, 0.12),
  borderBottom: `1px solid ${tint(t.warn, 0.4)}`,
};
const WARNING_TEXT: React.CSSProperties = { flex: 1, fontSize: 11.5, color: t.warn };
const ROUTEBTN: React.CSSProperties = { flex: "none", border: `1px solid ${t.warn}`, background: tint(t.warn, 0.16), color: t.warn, borderRadius: 6, padding: "4px 10px", fontSize: 11, cursor: "pointer" };
const ROUTE_RESULT: React.CSSProperties = { border: `1px solid ${tint(t.warn, 0.5)}`, borderRadius: 8, margin: "0 4px 10px", padding: "8px 10px", background: tint(t.warn, 0.1), color: "var(--vscode-foreground, #eef1f5)", fontSize: 11.5, lineHeight: 1.45 };
const ARTCARD: React.CSSProperties = { border: "1px solid var(--vscode-editorWidget-border, #2a2f3a)", borderRadius: 8, padding: "9px 10px", margin: "0 4px 9px", background: "var(--vscode-editor-background, rgba(13,16,22,.7))" };
const ARTPRE: React.CSSProperties = { margin: 0, maxHeight: 150, overflow: "auto", background: "var(--vscode-editor-background, #0c0f15)", border: "1px solid var(--vscode-editorWidget-border, #20262f)", borderRadius: 6, padding: "7px 9px", fontSize: 10.5, lineHeight: 1.45, color: "var(--vscode-foreground, #c4ccd8)", whiteSpace: "pre-wrap", wordBreak: "break-word", fontFamily: "ui-monospace, monospace" };
const COPYBTN: React.CSSProperties = { flex: "none", border: "1px solid var(--trust-accent)", background: "var(--trust-selected-bg)", color: "var(--trust-text)", borderRadius: 6, padding: "3px 10px", fontSize: 11, cursor: "pointer" };
const ICON: React.CSSProperties = { border: "none", background: "transparent", color: "var(--vscode-descriptionForeground, #949cab)", fontSize: 14, cursor: "pointer", padding: 0 };
const EMPTY: React.CSSProperties = { color: "var(--vscode-descriptionForeground, #7f8794)", fontSize: 11.5, padding: "8px 8px", lineHeight: 1.5 };
