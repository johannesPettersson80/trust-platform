// The ADS tag browser, one card per ADS port, drawn from the Rust view model (design §4.5 "D5: browse
// panes" and D5b; specification 25 §5.4 and its lines 878-883 and 387-394). Rust gives every text and
// rule: each port's title, state, body text, tags, route help with `Create route`, error box and browse
// action; the typed port with its action; the port suggestions and their help; the search box; and the
// connection's details. Ticking a tag posts that tag's own `ads.tags_set` action, which Rust built from
// the tag's added state, with the write toggle as its values; the tick box shows the project's state.
// This component draws them and keeps only what the user sets: the search text, the typed port, which
// ports are open and the write toggle.

import React from "react";

import { InspectorAction } from "./Inspector";
import type { Notice } from "./formTypes";
import { fieldInputStyle, formNoticeStyle, formNoticeTextStyle, formOwnerStyle } from "./styles";
import {
  addBodyStyle,
  addDialogStyle,
  addHeaderStyle,
  addTitleStyle,
} from "./styles/addDialog";
import {
  addRowStyle,
  advancedToggleStyle,
  browseBreadcrumbStyle,
  browseEmptyStyle,
  cardActionsStyle,
  disclosureStyle,
  doneButtonStyle,
  detailLabelStyle,
  detailRowStyle,
  detailValueStyle,
  loadingDotStyle,
  portBodyStyle,
  portCardStyle,
  portHeaderButtonStyle,
  portHelpStyle,
  portInputLabelStyle,
  portInputPanelStyle,
  portStateStyle,
  portSummaryStyle,
  portTitleStyle,
  routeLineStyle,
  suggestionButtonStyle,
  suggestionsStyle,
} from "./styles/browse";
import {
  BrowseErrorBox,
  BrowseNotice,
  leafMarks,
  leafNodes,
  matchesSearch,
  SearchField,
  TagTree,
  WriteToggle,
  type BrowseAction,
  type LeafMarks,
} from "./BrowsePane";
import type { AdsPortCard, BrowseView, RouteHelp, TreeNode } from "./types";

/** One port's route help: its text, its lines, its copy actions and `Create route`, which Rust opens. */
function RouteHelpBlock({ route, onAction }: { route: RouteHelp; onAction: BrowseAction }) {
  return (
    <div data-role="ads-route-help" style={formNoticeStyle("warning")}>
      <span data-role="ads-route-text" style={formNoticeTextStyle("warning")}>
        {route.text}
      </span>
      {route.help.map((line, index) => (
        <span key={index} data-role="ads-route-line" style={routeLineStyle}>
          {line}
        </span>
      ))}
      <div data-role="ads-route-actions" style={cardActionsStyle}>
        {route.artifacts.map((artifact, index) => (
          <InspectorAction
            key={`${index}-${artifact.id}`}
            action={artifact}
            onAction={(chosen) => onAction(chosen)}
          />
        ))}
        <InspectorAction action={route.create_route} onAction={(chosen) => onAction(chosen)} />
      </div>
    </div>
  );
}

/**
 * One ADS port's card: its header, which opens and closes, and its body: the route help or the error,
 * the browse action, and the tags, which the search narrows (presentation only).
 */
function PortCard({
  card,
  needle,
  noMatch,
  marks,
  busy,
  writable,
  onAction,
}: {
  /** The port's card, as the view gives it. */
  card: AdsPortCard;
  /** The search text, lower-cased and trimmed; empty when there is no search. */
  needle: string;
  /** The text shown when the search matches no tag on this port. */
  noMatch: string | undefined;
  /** The words of the tags' marks, as the view gives them. */
  marks: LeafMarks;
  /** Whether a Rust action is running. */
  busy: boolean;
  /** The write toggle's state, posted with each tick. */
  writable: boolean;
  /** Posts a Rust action, a copy or a browse, with its values. */
  onAction: BrowseAction;
}) {
  const [open, setOpen] = React.useState(true);
  const matches = needle === "" ? undefined : leafNodes(card.tags).filter((node) => matchesSearch(node, needle));
  // A tick posts the tag's own action; Rust chose it from the tag's added state, so the box shows that state.
  const tick = (node: TreeNode) => {
    const action = node.action;
    if (action !== undefined) {
      onAction(action, { writable });
    }
  };
  return (
    <section data-role="ads-port" data-port={card.port} style={portCardStyle}>
      <button
        type="button"
        data-role="ads-port-toggle"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        style={portHeaderButtonStyle}
      >
        <span data-role="ads-port-glyph">{open ? "▾" : "▸"}</span>
        <span data-role="ads-port-title" style={portTitleStyle}>
          {card.title}
        </span>
        <span data-role="ads-port-state" style={portStateStyle}>
          {card.state_text}
        </span>
      </button>
      {open && (
        <div data-role="ads-port-body" style={portBodyStyle}>
          {card.error !== undefined && <BrowseErrorBox error={card.error} onAction={onAction} />}
          {card.route !== undefined && <RouteHelpBlock route={card.route} onAction={onAction} />}
          {card.browse !== undefined && (
            <InspectorAction action={card.browse} onAction={(chosen) => onAction(chosen)} />
          )}
          {matches !== undefined ? (
            matches.length > 0 ? (
              <TagTree
                nodes={matches}
                marks={marks}
                checked={(node) => node.added}
                onTick={tick}
                disabled={busy}
              />
            ) : (
              <p data-role="ads-port-no-match" style={browseEmptyStyle}>
                {noMatch}
              </p>
            )
          ) : card.tags.length > 0 ? (
            <TagTree
              nodes={card.tags}
              marks={marks}
              checked={(node) => node.added}
              onTick={tick}
              disabled={busy}
            />
          ) : (
            card.empty_text !== undefined && (
              <p data-role="ads-port-empty" style={browseEmptyStyle}>
                {card.empty_text}
              </p>
            )
          )}
        </div>
      )}
    </section>
  );
}

/** The text field of the typed port, with the view's label and placeholder. */
function PortField({
  label,
  placeholder,
  value,
  onChange,
}: {
  /** The field's label, from Rust. */
  label: string;
  /** The placeholder, from Rust. */
  placeholder: string;
  /** The text typed so far. */
  value: string;
  /** Called with the text as typed. */
  onChange: (value: string) => void;
}) {
  // The text is read from `input` as well as from `onChange`, as the search field is (BrowsePane.tsx).
  return (
    <label data-role="ads-port-field" style={portInputLabelStyle}>
      <span data-role="ads-port-label">{label}</span>
      <input
        type="text"
        data-role="ads-port-input"
        placeholder={placeholder}
        value={value}
        onInput={(event) => onChange(event.currentTarget.value)}
        onChange={(event) => onChange(event.currentTarget.value)}
        style={fieldInputStyle}
      />
    </label>
  );
}

/**
 * The ADS tag browser: its title, its port summary and details, its search, one card per port, the typed
 * port with its suggestions and help, and its footer with the write toggle, the result of the last action
 * and `Done`.
 */
export function AdsTagBrowser({
  browse,
  notice,
  busy = false,
  onAction,
  onClose,
}: {
  /** The browse view, as Rust gives it. */
  browse: BrowseView;
  /** The result of the last action, when it names this pane. */
  notice?: Notice;
  /** Whether a Rust action is running: the ticks and the typed port wait for its reply. */
  busy?: boolean;
  /** Called with a chosen action, and the values it carries. */
  onAction: BrowseAction;
  /** Called when `Done` is chosen. */
  onClose: () => void;
}) {
  const [writable, setWritable] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [advancedOpen, setAdvancedOpen] = React.useState(false);
  const [typedPort, setTypedPort] = React.useState("");
  const needle = query.trim().toLowerCase();
  // Only a notice that names this pane is drawn here (specification 25 §5.13).
  const own = notice !== undefined && notice.pane === browse.id ? notice : undefined;
  const input = browse.port_input;
  return (
    <div
      data-role="browse-view"
      data-kind={browse.kind}
      data-loading={browse.loading ? "true" : "false"}
      style={addDialogStyle}
    >
      <header style={addHeaderStyle}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <span data-role="browse-breadcrumb" style={browseBreadcrumbStyle}>
            {browse.breadcrumb}
          </span>
          <span data-role="browse-title" title={browse.title} style={addTitleStyle}>
            {browse.title}
          </span>
        </div>
      </header>
      <div style={addBodyStyle}>
        <span data-role="browse-subtitle" style={formOwnerStyle}>
          {browse.subtitle}
        </span>
        {browse.port_summary !== undefined && (
          <span data-role="ads-port-summary" style={portSummaryStyle}>
            {browse.port_summary}
          </span>
        )}
        {browse.details !== undefined && (
          <details data-role="browse-details" style={disclosureStyle}>
            <summary data-role="browse-details-summary">{browse.details.summary}</summary>
            {browse.details.rows.map((row) => (
              <div key={row.label} data-role="browse-detail-row" style={detailRowStyle}>
                <span data-role="browse-detail-label" style={detailLabelStyle}>
                  {row.label}
                </span>
                <span data-role="browse-detail-value" style={detailValueStyle}>
                  {row.value}
                </span>
              </div>
            ))}
          </details>
        )}
        {browse.loading && <span data-role="browse-loading" style={loadingDotStyle} />}
        {browse.error !== undefined && <BrowseErrorBox error={browse.error} onAction={onAction} />}
        {browse.search !== undefined && (
          <SearchField search={browse.search} value={query} onChange={setQuery} />
        )}
        <div data-role="ads-ports" style={{ display: "flex", flexDirection: "column" }}>
          {browse.ports.map((card) => (
            <PortCard
              key={card.port}
              card={card}
              needle={needle}
              noMatch={browse.search?.no_match}
              marks={leafMarks(browse)}
              busy={busy}
              writable={writable}
              onAction={onAction}
            />
          ))}
        </div>
        {input !== undefined && (
          <div data-role="ads-port-input-block">
            <button
              type="button"
              data-role="ads-advanced-toggle"
              aria-expanded={advancedOpen}
              onClick={() => setAdvancedOpen((current) => !current)}
              style={advancedToggleStyle}
            >
              {advancedOpen ? input.hide_label : input.show_label}
            </button>
            {advancedOpen && (
              <div data-role="ads-port-input-panel" style={portInputPanelStyle}>
                <PortField
                  label={input.label}
                  placeholder={input.placeholder}
                  value={typedPort}
                  onChange={setTypedPort}
                />
                <div data-role="ads-port-suggestions" style={suggestionsStyle}>
                  {browse.port_suggestions.map((option) => (
                    <button
                      key={option.value}
                      type="button"
                      data-role="ads-port-suggestion"
                      data-port={option.value}
                      onClick={() => setTypedPort(option.value)}
                      style={suggestionButtonStyle}
                    >
                      {option.value} {option.label}
                    </button>
                  ))}
                </div>
                {browse.port_help !== "" && (
                  <p data-role="ads-port-help" style={portHelpStyle}>
                    {browse.port_help}
                  </p>
                )}
                <button
                  type="button"
                  data-role="ads-browse-port"
                  disabled={!input.browse.enabled || busy}
                  title={input.browse.reason ?? input.browse.label}
                  onClick={() => onAction(input.browse, { port: typedPort })}
                  style={advancedToggleStyle}
                >
                  {input.browse.label}
                </button>
                {!input.browse.enabled && input.browse.reason !== undefined && (
                  <span data-role="ads-browse-port-reason" style={formOwnerStyle}>
                    {input.browse.reason}
                  </span>
                )}
              </div>
            )}
          </div>
        )}
        <div data-role="browse-footer" style={addRowStyle}>
          <WriteToggle
            toggle={browse.write_toggle}
            writable={writable}
            busy={busy}
            onWritable={setWritable}
          />
          {own !== undefined && <BrowseNotice notice={own} onAction={onAction} />}
          {browse.done_label !== undefined && (
            <button
              type="button"
              data-role="ads-done"
              onClick={onClose}
              style={doneButtonStyle}
            >
              {browse.done_label}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
