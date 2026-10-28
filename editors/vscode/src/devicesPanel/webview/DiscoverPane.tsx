// The Discover pane of Devices & Connections: one search, drawn from the Rust view of the search status
// (design §4.5 "D4: Discover pane"; specification 25 §2). Rust gives every text and every rule: the breadcrumb,
// the headings and help texts, the origin choices, the `Only search` choices with their notes, groups and the
// reason of each scan that cannot run, the labels and placeholders of the known address and subnet inputs, the
// source rows with their words and reasons, one card per device with its protocols, address, identity, project
// mark (and the facts that differ from the project) and actions, the four lists of `Compare with project`, the
// empty text, and the pane's own actions. This
// component draws them as given and posts a chosen action exactly as the view gives it. The one exception is
// `Search`: it also carries the known address and subnet the user typed, as its values. Its state is what the user
// types into those two inputs, and which card action awaits its search's reply. The origin is the view's own: a
// change of it reports nothing here yet. A tick, an Ignore, `Show ignored` and `Compare with project` post the
// change that the view gives them, and the view answers with the new state.

import React, { useEffect, useState } from "react";

import { BrowseNotice } from "./BrowsePane";
import { InspectorAction } from "./Inspector";
import type { Notice } from "./formTypes";
import { pillStyle } from "./styles";
import {
  addBodyStyle,
  addCloseStyle,
  addDialogStyle,
  addHeaderStyle,
  addTitleStyle,
} from "./styles/addDialog";
import { addRowStyle, browseEmptyStyle, loadingDotStyle } from "./styles/browse";
import {
  discoverAddressStyle,
  discoverCardActionsStyle,
  discoverCardStyle,
  discoverChoiceStyle,
  discoverFieldStyle,
  discoverIdentityStyle,
  discoverIdentityValueStyle,
  discoverInputStyle,
  discoverLabelStyle,
  discoverProtocolStyle,
  discoverReasonStyle,
  discoverRowStyle,
  discoverSectionStyle,
  discoverTitleStyle,
  discoverWarningStyle,
} from "./styles/discover";
import type {
  Action,
  CardProtocol,
  CompareList,
  CompareView,
  DeviceCard,
  DiscoverView,
  OnlyChoice,
  OriginChoice,
  SourceRow,
  TargetsInput,
} from "./types";

/** The values the search action carries: the known address and the subnet, as the user typed them. */
interface TypedTargets {
  hosts: string;
  cidr: string;
}

/** The group names of the `Only search` choices, as Rust gives them (`OnlyChoice.group`). */
const FIND = "find";
const KNOWN_ADDRESS = "known_address";
const RUNTIME_HARDWARE = "runtime_hardware";

/** The name of the action that starts a search; it is the one action that carries the typed targets. */
const SEARCH_ACTION = "search";

/**
 * The origin is the view's: the next view says which origin a search uses. A change reports nothing here, so the
 * control is drawn as the view gives it.
 */
function viewDecides(): void {
  // Nothing to do: the control is drawn as the view gives it.
}

/** A heading of the pane, as the view gives it. */
function Heading({ text }: { text: string }) {
  return (
    <div data-role="discover-heading" style={headingStyle}>
      {text}
    </div>
  );
}

/** `Scan from`: this computer, then each runtime, with the chosen one as the view says. */
function OriginChoices({ origins }: { origins: OriginChoice[] }) {
  return (
    <section data-role="discover-origins" style={discoverSectionStyle}>
      {origins.map((origin) => (
        <label
          key={origin.id}
          data-role="discover-origin"
          data-origin={origin.id}
          title={origin.reason}
          style={discoverChoiceStyle(origin.enabled)}
        >
          <input
            type="radio"
            name="discover-origin"
            data-role="discover-origin-radio"
            checked={origin.selected}
            disabled={!origin.enabled}
            onChange={viewDecides}
          />
          <span>{origin.label}</span>
          {origin.reason !== undefined && (
            <span data-role="discover-origin-reason" style={discoverReasonStyle}>
              {origin.reason}
            </span>
          )}
        </label>
      ))}
    </section>
  );
}

/**
 * The `Only search` choices of one group: one tick per source, its note under its name, and disabled with its
 * reason when a scan from the origin cannot run.
 */
function OnlyChoices({
  choices,
  group,
  onAction,
}: {
  choices: OnlyChoice[];
  group: string;
  onAction: (action: Action) => void;
}) {
  return (
    <section data-role="discover-only" data-group={group} style={discoverSectionStyle}>
      {choices
        .filter((choice) => choice.group === group)
        .map((choice) => (
          <label
            key={choice.source}
            data-role="discover-only-choice"
            data-source={choice.source}
            style={discoverChoiceStyle(choice.enabled)}
          >
            <input
              type="checkbox"
              data-role="discover-only-tick"
              checked={choice.checked}
              disabled={!choice.enabled}
              onChange={() => {
                if (choice.action !== undefined) {
                  onAction(choice.action);
                }
              }}
            />
            <span data-role="discover-only-label">{choice.label}</span>
            {choice.warning !== undefined && (
              <span
                data-role="discover-only-warning"
                title={choice.warning}
                style={discoverWarningStyle}
              >
                ⚠
              </span>
            )}
            {choice.note !== undefined && (
              <span data-role="discover-only-note" style={discoverReasonStyle}>
                {choice.note}
              </span>
            )}
            {choice.reason !== undefined && (
              <span data-role="discover-only-reason" style={discoverReasonStyle}>
                {choice.reason}
              </span>
            )}
          </label>
        ))}
    </section>
  );
}

/**
 * `Known address or subnet`: the two inputs, with the labels and placeholders the view gives. The text is read from
 * `input` as well as from `onChange`, as the browse search field is (BrowsePane.tsx): React answers `onChange` to an
 * `input` event only when its input-event check passed as React loaded.
 */
function Targets({
  targets,
  typed,
  onType,
}: {
  targets: TargetsInput;
  typed: TypedTargets;
  onType: (key: keyof TypedTargets, value: string) => void;
}) {
  return (
    <section data-role="discover-targets" style={discoverSectionStyle}>
      <label data-role="discover-target" style={discoverFieldStyle}>
        <span>{targets.hosts_label}</span>
        <input
          type="text"
          data-role="discover-hosts"
          placeholder={targets.hosts_placeholder}
          value={typed.hosts}
          onInput={(event) => onType("hosts", event.currentTarget.value)}
          onChange={(event) => onType("hosts", event.currentTarget.value)}
          style={discoverInputStyle}
        />
      </label>
      <label data-role="discover-target" style={discoverFieldStyle}>
        <span>{targets.cidr_label}</span>
        <input
          type="text"
          data-role="discover-cidr"
          placeholder={targets.cidr_placeholder}
          value={typed.cidr}
          onInput={(event) => onType("cidr", event.currentTarget.value)}
          onChange={(event) => onType("cidr", event.currentTarget.value)}
          style={discoverInputStyle}
        />
      </label>
    </section>
  );
}

/** The source rows: each source's word in its tone, its count, and its reason when it did not run. */
function SourceRows({ rows }: { rows: SourceRow[] }) {
  return (
    <section data-role="discover-sources" style={discoverSectionStyle}>
      {rows.map((row) => (
        <div
          key={row.id}
          data-role="discover-source"
          data-source={row.id}
          data-tone={row.tone}
          data-count={row.count}
          style={discoverRowStyle}
        >
          <span data-role="discover-source-label" style={discoverLabelStyle}>
            {row.label}
          </span>
          <span data-role="discover-source-word" style={pillStyle(row.tone)}>
            {row.word}
          </span>
          {row.reason !== undefined && (
            <span data-role="discover-source-reason" style={discoverReasonStyle}>
              {row.reason}
            </span>
          )}
        </div>
      ))}
    </section>
  );
}

/** A protocol line of a card: its name, how it was found, its confidence and, when simulated, its mark. */
function ProtocolLine({ protocol }: { protocol: CardProtocol }) {
  return (
    <div
      data-role="discover-card-protocol"
      data-simulated={protocol.simulated ? "true" : "false"}
      style={discoverProtocolStyle}
    >
      <span data-role="discover-card-protocol-name">{protocol.name}</span>
      <span data-role="discover-card-protocol-source" style={discoverReasonStyle}>
        {protocol.source}
      </span>
      <span data-role="discover-card-protocol-confidence" style={discoverReasonStyle}>
        {protocol.confidence}
      </span>
      {protocol.mark !== undefined && (
        <span data-role="discover-card-protocol-mark" style={pillStyle("warning")}>
          {protocol.mark}
        </span>
      )}
    </div>
  );
}

/** One list of `Compare with project`: its heading and its rows, or the text that says it holds none. */
function CompareListSection({ kind, list }: { kind: string; list: CompareList }) {
  return (
    <div data-role="discover-compare-list" data-list={kind} style={discoverSectionStyle}>
      <div data-role="discover-compare-heading" style={headingStyle}>
        {list.heading}
      </div>
      {list.rows.length === 0 && (
        <p data-role="discover-compare-empty" style={discoverReasonStyle}>
          {list.empty_text}
        </p>
      )}
      {list.rows.map((row, index) => (
        <div
          key={`${row.label}-${index}`}
          data-role="discover-compare-row"
          style={discoverIdentityStyle}
        >
          <span data-role="discover-compare-row-label" style={discoverReasonStyle}>
            {row.label}
          </span>
          <span data-role="discover-compare-row-value" style={discoverIdentityValueStyle}>
            {row.value}
          </span>
        </div>
      ))}
    </div>
  );
}

/** `Compare with project`: the four lists of the comparison, as the view gives them. */
function CompareLists({ compare }: { compare: CompareView }) {
  return (
    <section data-role="discover-compare" style={discoverSectionStyle}>
      <Heading text={compare.heading} />
      <CompareListSection kind="missing" list={compare.missing} />
      <CompareListSection kind="unused" list={compare.unused} />
      <CompareListSection kind="differs" list={compare.differs} />
      <CompareListSection kind="not_compared" list={compare.not_compared} />
    </section>
  );
}

/**
 * One device card: its title, its addresses, the project's mark and the facts that differ from the project, every
 * protocol found on it, its identity, when it was last seen, its warnings, and its actions.
 */
function Card({
  card,
  busy,
  onAction,
}: {
  card: DeviceCard;
  /** Whether a card action awaits its reply: the actions are drawn disabled until the next view. */
  busy: boolean;
  onAction: (action: Action) => void;
}) {
  return (
    <div data-role="discover-card" data-card={card.id} style={discoverCardStyle}>
      <span data-role="discover-card-title" style={discoverTitleStyle}>
        {card.title}
      </span>
      {card.addresses.map((address) => (
        <span key={address} data-role="discover-card-address" style={discoverAddressStyle}>
          {address}
        </span>
      ))}
      {card.responding_ports !== undefined && (
        <span data-role="discover-card-ports-count" style={discoverReasonStyle}>
          {card.responding_ports}
        </span>
      )}
      {card.project !== undefined && (
        <span
          data-role="discover-card-project"
          data-tone={card.project.tone}
          style={pillStyle(card.project.tone)}
        >
          {card.project.word}
        </span>
      )}
      {card.differs.map((row, index) => (
        <div
          key={`${row.label}-${index}`}
          data-role="discover-card-difference"
          style={discoverIdentityStyle}
        >
          <span data-role="discover-card-difference-label" style={discoverReasonStyle}>
            {row.label}
          </span>
          <span data-role="discover-card-difference-value" style={discoverIdentityValueStyle}>
            {row.value}
          </span>
        </div>
      ))}
      {card.ports.length > 0 && (
        <div data-role="discover-card-ports" style={discoverCardActionsStyle}>
          {card.ports.map((port) => (
            <InspectorAction
              key={port.id}
              action={busy ? { ...port, enabled: false } : port}
              onAction={onAction}
            />
          ))}
        </div>
      )}
      {card.protocols.map((protocol, index) => (
        <ProtocolLine key={index} protocol={protocol} />
      ))}
      {card.identity.map((row) => (
        <div key={row.label} data-role="discover-card-identity" style={discoverIdentityStyle}>
          <span data-role="discover-card-identity-label" style={discoverReasonStyle}>
            {row.label}
          </span>
          <span data-role="discover-card-identity-value" style={discoverIdentityValueStyle}>
            {row.value}
          </span>
        </div>
      ))}
      <span data-role="discover-card-last-seen" style={discoverReasonStyle}>
        {card.last_seen}
      </span>
      {card.warnings.map((warning, index) => (
        <span key={index} data-role="discover-card-warning" style={discoverWarningStyle}>
          {warning}
        </span>
      ))}
      {card.stale_reason !== undefined && (
        <span data-role="discover-card-stale" style={discoverReasonStyle}>
          {card.stale_reason}
        </span>
      )}
      <div data-role="discover-card-actions" style={discoverCardActionsStyle}>
        {card.actions.map((action) => (
          <InspectorAction
            key={action.id}
            action={busy ? { ...action, enabled: false } : action}
            onAction={onAction}
          />
        ))}
      </div>
    </div>
  );
}

/**
 * The Discover pane: its breadcrumb, title and close button; its `Only search` choices, its scan settings with the
 * origin, the known address and subnet, and the runtime's hardware scans; its source rows and cards; its empty
 * text; and its footer with the pane's actions and the result of the last one.
 */
export function DiscoverPane({
  title,
  discover,
  notice,
  busy,
  onAction,
  onClose,
}: {
  /** The pane's title: the words of the toolbar action that opened it. */
  title: string;
  /** Whether a Rust or host action that the panel posted awaits its reply, such as a card's `Adopt`: the cards'
   * actions are drawn disabled until it arrives, as the browse pane's are. */
  busy: boolean;
  /** The Discover view, as Rust gives it. */
  discover: DiscoverView;
  /** The result of the last action, when it names this pane. */
  notice?: Notice;
  /** Called with a chosen action; `Search` also gets the known address and subnet the user typed. */
  onAction: (action: Action, values?: Record<string, unknown>) => void;
  /** Called when the close button is chosen. */
  onClose: () => void;
}) {
  const [typed, setTyped] = useState<TypedTargets>({ hosts: "", cidr: "" });
  // A card action that is a search awaits the view that answers it: until that view arrives, no card action posts again.
  const [awaiting, setAwaiting] = useState(false);
  useEffect(() => {
    setAwaiting(false);
  }, [discover]);
  const chooseCardAction = (action: Action) => {
    if (action.op.kind === "search") {
      setAwaiting(true);
    }
    onAction(action);
  };
  const onType = (key: keyof TypedTargets, value: string) =>
    setTyped((current) => ({ ...current, [key]: value }));
  const chooseFooterAction = (action: Action) => {
    if (action.id === SEARCH_ACTION) {
      onAction(action, { ...typed });
    } else {
      onAction(action);
    }
  };
  return (
    <div
      data-role="discover-view"
      data-running={discover.running ? "true" : "false"}
      style={addDialogStyle}
    >
      <header style={addHeaderStyle}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div data-role="discover-breadcrumb" style={discoverReasonStyle}>
            {discover.breadcrumb}
          </div>
          <span data-role="discover-title" title={title} style={addTitleStyle}>
            {title}
          </span>
        </div>
        {discover.running && <span data-role="discover-running" style={loadingDotStyle} />}
        <button
          type="button"
          data-role="discover-close"
          aria-label={discover.close_label}
          onClick={onClose}
          style={addCloseStyle}
        >
          ✕
        </button>
      </header>
      <div style={addBodyStyle}>
        {discover.unavailable_text !== undefined && (
          <p data-role="discover-unavailable" style={discoverReasonStyle}>
            {discover.unavailable_text}
          </p>
        )}
        {discover.choose_text !== undefined && (
          <p data-role="discover-choose" style={discoverReasonStyle}>
            {discover.choose_text}
          </p>
        )}
        {discover.progress_text !== undefined && (
          <p data-role="discover-progress" style={discoverReasonStyle}>
            {discover.progress_text}
          </p>
        )}
        {discover.error_text !== undefined && (
          <p data-role="discover-error" style={discoverWarningStyle}>
            {discover.error_text}
          </p>
        )}
        <Heading text={discover.heading_find} />
        <OnlyChoices choices={discover.only} group={FIND} onAction={onAction} />
        <section data-role="discover-advanced" style={discoverSectionStyle}>
          <Heading text={discover.heading_advanced} />
          <Heading text={discover.heading_scan_from} />
          <OriginChoices origins={discover.origins} />
          <Heading text={discover.heading_targets} />
          <OnlyChoices choices={discover.only} group={KNOWN_ADDRESS} onAction={onAction} />
          <Targets targets={discover.targets} typed={typed} onType={onType} />
          <Heading text={discover.heading_runtime} />
          <OnlyChoices choices={discover.only} group={RUNTIME_HARDWARE} onAction={onAction} />
        </section>
        <SourceRows rows={discover.sources} />
        {discover.update_text !== undefined && (
          <p data-role="discover-update" style={discoverWarningStyle}>
            {discover.update_text}
          </p>
        )}
        <label data-role="discover-show-ignored" style={discoverChoiceStyle(true)}>
          <input
            type="checkbox"
            data-role="discover-show-ignored-tick"
            checked={discover.show_ignored.checked}
            onChange={() => onAction(discover.show_ignored.action)}
          />
          <span>{discover.show_ignored.label}</span>
        </label>
        {discover.cards.length > 0 && (
          <section data-role="discover-cards" style={discoverSectionStyle}>
            {discover.cards.map((card) => (
              <Card key={card.id} card={card} busy={busy || awaiting} onAction={chooseCardAction} />
            ))}
          </section>
        )}
        {discover.empty_text !== undefined && (
          <p data-role="discover-empty" style={browseEmptyStyle}>
            {discover.empty_text}
          </p>
        )}
        {discover.compare !== undefined && <CompareLists compare={discover.compare} />}
        <div data-role="discover-footer" style={addRowStyle}>
          {discover.actions.map((action) => (
            <InspectorAction key={action.id} action={action} onAction={chooseFooterAction} />
          ))}
          {notice !== undefined && <BrowseNotice notice={notice} onAction={onAction} />}
        </div>
      </div>
    </div>
  );
}

const headingStyle: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 600,
  color: "var(--trust-text-subtle)",
  letterSpacing: 0.2,
  margin: "10px 0 4px",
};
