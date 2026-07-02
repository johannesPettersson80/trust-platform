import React from "react";
import { protocolColor, protocolName } from "./protocolMeta";
import type { FilterReport } from "./filter";

// §5: hide/show connections by protocol. A checkbox per protocol present on the canvas.
export function FilterPanel({
  protocols,
  hidden,
  report,
  onToggle,
  onShowAll,
}: {
  protocols: string[];
  hidden: ReadonlySet<string>;
  report: FilterReport;
  onToggle: (protocol: string) => void;
  onShowAll: () => void;
}) {
  const anyHidden = hidden.size > 0;
  const hiddenIssueCount = Math.max(report.hiddenFaultCount, report.hiddenAttentionCount);
  const hiddenIssueLabel =
    hiddenIssueCount === 1
      ? "1 hidden item needs attention."
      : `${hiddenIssueCount} hidden items need attention.`;
  return (
    <aside className="trust-inspector" style={PANEL} aria-label="Filter connections">
      <div className="trust-inspector__header">
        <div className="trust-inspector__title" style={{ flex: 1 }}>
          Filter connections
        </div>
      </div>
      <section className="trust-section">
        <h3 className="trust-section__title">Filter status</h3>
        {anyHidden ? (
          <>
            <p className="trust-help">
              {report.hiddenEndpointCount} connection{report.hiddenEndpointCount === 1 ? "" : "s"} hidden.
            </p>
            <p className="trust-help" style={{ color: hiddenIssueCount > 0 ? "var(--trust-warn)" : "var(--trust-text-muted)", marginTop: 6 }}>
              {hiddenIssueCount > 0
                ? hiddenIssueLabel
                : "No hidden warnings or faults."}
            </p>
            {report.hiddenFaultCount > 0 && (
              <p className="trust-help" style={{ color: report.hiddenErrorCount > 0 ? "var(--trust-danger)" : "var(--trust-warn)", marginTop: 4 }}>
                {report.hiddenErrorCount} error{report.hiddenErrorCount === 1 ? "" : "s"} · {report.hiddenWarningCount} warning{report.hiddenWarningCount === 1 ? "" : "s"}
              </p>
            )}
            <button className="trust-button trust-button--primary" style={{ width: "100%", marginTop: 10 }} onClick={onShowAll}>
              Show all
            </button>
          </>
        ) : (
          <p className="trust-help">All protocols are visible.</p>
        )}
      </section>
      <section className="trust-section trust-section--grow">
        <h3 className="trust-section__title">Show protocols</h3>
      <div style={{ flex: 1, overflow: "auto", padding: 8 }}>
        {protocols.length === 0 ? (
          <p className="trust-help" style={{ padding: "4px 6px" }}>No connections.</p>
        ) : (
          protocols.map((p) => {
            const on = !hidden.has(p);
            return (
              <label key={p} style={ROW}>
                <input type="checkbox" checked={on} onChange={() => onToggle(p)} />
                <span style={{ width: 10, height: 10, borderRadius: 3, background: protocolColor(p), flex: "none" }} />
                <span style={{ fontSize: 12, opacity: on ? 1 : 0.45 }}>{protocolName(p)}</span>
              </label>
            );
          })
        )}
      </div>
      </section>
    </aside>
  );
}

const PANEL: React.CSSProperties = {
  position: "absolute",
  top: 0,
  right: 0,
  bottom: 0,
  width: 184,
  zIndex: 7,
};
const ROW: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "6px 8px",
  borderRadius: 7,
  cursor: "pointer",
};
